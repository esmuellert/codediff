//! Reading keys, drawing frames, and stopping.
//!
//! One loop: take whatever the worker has finished, ask for anything newly on
//! screen, wait for a key, dispatch it, draw. Nothing here computes a diff,
//! touches a repository, **or colours a line** — so this loop cannot be made
//! slow by any of them, which is the whole point of separating it from the
//! work.
//!
//! The wait is where the thread sleeps, and it has two speeds. With a file
//! being coloured it wakes at most once a frame to collect what has arrived;
//! with nothing outstanding it blocks until a key is pressed, at no cost at
//! all. Neither is a poll for *work* — the work is on another thread — and
//! there is nothing to tune: sixteen milliseconds is one frame at sixty hertz,
//! past which nobody can see the difference.
//!
//! [`Session::dispatch`] is where the three kinds of command diverge, and it
//! is the only place that can see all three of their executors. That is why
//! resolving and dispatching are separate: [`crate::input`] would have to
//! reach the view, the terminal and the task runner at once to do both.

use pipeline::file::{Answer, Files};
use ratatui::backend::Backend;

use crate::draw;
use crate::input::tab::RESIZE_STEP;
use crate::input::{Action, Command, ProgramAction, Resolution, Resolver, TabAction, ViewAction};
use crate::syntax::{Store, Syntax};
use crate::terminal::Screen;
use crate::theme::Theme;
use crate::view::Buffer;
use crate::view::View;

/// Answers in a row that install nothing before [`Session::settle`] gives up.
///
/// Generous: every ordinary reason for an answer to install nothing happens
/// once, not eight times running.
const IDLE_ANSWERS: u32 = 8;

/// What the loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// Give the terminal back until the reader brings us forward again.
    Suspend,
    Quit,
}

/// One review, from opening the terminal to giving it back.
pub struct Session {
    view: View,
    theme: Theme,
    resolver: Resolver,
    /// The thread that colours, and the queues to it.
    syntax: Syntax,
    /// The thread that compares, and the queues to it.
    files: Files,
    /// The row the reader last chose, until its comparison is installed.
    ///
    /// Held rather than sent straight on, because only one comparison runs at
    /// a time: a reader moving down a list faster than git can answer leaves
    /// this pointing at wherever they got to, and that is what is asked for
    /// next. An answer for anything else is discarded on arrival.
    wanted: Option<file_types::ChangedFile>,
    /// Every colour anything open has. Owned here rather than by a buffer, so
    /// a file keeps its colours when the reader moves away and comes back.
    store: Store,
    /// What went wrong the last time something was asked for.
    ///
    /// Cleared by the next key, which is how vim's echo area behaves and the
    /// reason this needs no clock. It used to be cleared only by the next
    /// thing that *worked*, so "picture.png is binary" stayed on the status
    /// line while the reader moved on to other files.
    notice: Option<String>,
}

impl Session {
    /// Opens one buffer.
    ///
    /// One constructor, because how a buffer is drawn follows from its kind
    /// rather than from which function the caller chose.
    pub fn new(buffer: Buffer, theme: Theme) -> Self {
        Self::with_files(buffer, theme, Files::start())
    }

    /// The same, with a worker supplied.
    ///
    /// **For tests, and for nothing else** — [`Files::canned`] is the reason
    /// it exists. A review always starts the real one.
    pub fn with_files(buffer: Buffer, theme: Theme, files: Files) -> Self {
        let mut syntax = Syntax::start();
        let mut store = Store::new();
        let mut view = View::single(buffer);
        // Asked for before the first frame, so the colours are already on
        // their way while the terminal is still being set up.
        view.request(&mut syntax, &mut store);
        Self {
            view,
            theme,
            resolver: Resolver::new(),
            syntax,
            files,
            wanted: None,
            store,
            notice: None,
        }
    }

    /// Waits until everything on screen has been coloured.
    ///
    /// **For tests, and for nothing else.** The interface must never wait for
    /// a colour — that is the whole reason the painter has a thread — but a
    /// test asserting *about* colour has to know when to look. It uses the
    /// same two calls the loop does, so what it exercises is the real path
    /// rather than a shortcut past it, and it blocks on the channel rather
    /// than sleeping, so it is exactly as fast as the work and no slower.
    ///
    /// Returns whether anything arrived, so a caller can tell "settled" from
    /// "there was nothing to settle".
    pub fn settle(&mut self) -> bool {
        // Progress is lines installed. An answer that installs none is
        // ordinary on its own — a stale piece is refused, and a file no
        // language claims answers with nothing — but a run of them means the
        // store and the worker disagree about how far the file has been read
        // and neither will give way. Stopping is better than a test that
        // hangs, which is how that showed up.
        let mut changed = false;
        let mut idle = 0;
        while self.painting() && idle < IDLE_ANSWERS {
            let held = self.store.held();
            match self.syntax.next() {
                Some(answer) => changed |= self.store.install(answer),
                // The worker stopped, which can only mean it panicked. The
                // review continues in plain text rather than hanging.
                None => break,
            }
            idle = if self.store.held() > held {
                0
            } else {
                idle + 1
            };
            // As the loop does, and for the same reason: what was wanted
            // while that request was running was dropped, not queued.
            self.request();
        }
        changed
    }

    /// Whether anything on screen is still waiting to be coloured.
    ///
    /// What decides whether the loop waits for a frame or for a key. False
    /// almost always, since a file is coloured within a few frames of being
    /// opened and then stays coloured.
    pub fn painting(&self) -> bool {
        self.syntax.working()
    }

    /// Installs whatever the painter has finished, and says whether the screen
    /// changed.
    ///
    /// Never waits. Costs a few nanoseconds when there is nothing.
    pub fn collect(&mut self) -> bool {
        let mut changed = false;
        for answer in self.syntax.take() {
            changed |= self.store.install(answer);
        }
        changed
    }

    /// Asks for the colours of anything newly on screen.
    ///
    /// Called after every event, because a motion is what brings new lines
    /// into view. Silent when the store already has them, which after the
    /// first screen it usually does.
    pub fn request(&mut self) {
        self.view.request(&mut self.syntax, &mut self.store);
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    /// Keys typed but not yet resolved, for a `showcmd` display.
    pub fn pending(&self) -> &Resolver {
        &self.resolver
    }

    /// Draws one frame into a backend's buffer.
    /// Draws into a cell grid directly, without a terminal.
    ///
    /// The same call [`draw`](Self::draw) makes, exposed so a test can render
    /// a screen and read it back. Without it, checking what the explorer looks
    /// like would need a real terminal.
    pub fn draw_into(&mut self, cells: &mut ratatui::buffer::Buffer, area: ratatui::layout::Rect) {
        draw::render(
            cells,
            area,
            &mut self.view,
            &self.theme,
            &self.store,
            self.notice.as_deref(),
        );
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        terminal.draw(|frame| {
            let area = frame.area();
            draw::render(
                frame.buffer_mut(),
                area,
                &mut self.view,
                &self.theme,
                &self.store,
                self.notice.as_deref(),
            );
        })?;
        Ok(())
    }

    /// Asks for whatever the list has selected.
    ///
    /// **Returns at once.** The comparison arrives on a later frame, and until
    /// it does the screen keeps showing whatever was there — the list stays
    /// live, and every key still answers. A 50,000-line file takes about a
    /// second to compare, which used to be a second of dead terminal.
    ///
    /// Nothing is sent here. [`request_file`](Self::request_file) does that,
    /// once, when the worker is free.
    pub fn open(&mut self) {
        self.wanted = self.selected_file();
    }

    /// Whether a comparison is outstanding.
    ///
    /// What decides, with [`painting`](Self::painting), whether the loop waits
    /// for a frame or for a key.
    pub fn opening(&self) -> bool {
        self.files.working()
    }

    /// Asks the worker for the row the reader chose.
    ///
    /// Called after every event, beside [`request`](Self::request), and silent
    /// unless there is something to ask for. Does nothing while an answer is
    /// outstanding: what the reader wanted meanwhile was not queued, so this
    /// is where it is asked for again, with a row that is current by now.
    pub fn request_file(&mut self) {
        if let Some(file) = &self.wanted {
            self.files.want(file);
        }
    }

    /// Installs a comparison if one has arrived, and says whether the screen
    /// changed.
    ///
    /// Never waits. Costs a few nanoseconds when there is nothing.
    pub fn collect_file(&mut self) -> bool {
        let Some(answer) = self.files.take() else {
            return false;
        };
        // The reader moved off this row while it was being compared, so this
        // answers a question nobody is asking any more. Dropped rather than
        // shown: `request_file` asks again for wherever they are now.
        if self.wanted.as_ref() != Some(&answer.file) {
            return false;
        }
        self.wanted = None;
        self.install(answer)
    }

    /// Waits for the comparison that is outstanding.
    ///
    /// **For tests, and for nothing else.** The interface must never wait for
    /// a comparison — that is the whole reason the pipeline has a thread — but
    /// a test asserting about a pane has to know when to look. It uses the
    /// same calls the loop does, so it exercises the real path, and it blocks
    /// on the channel rather than sleeping.
    ///
    /// Returns whether anything was installed.
    pub fn opened(&mut self) -> bool {
        self.request_file();
        let Some(answer) = self.files.wait() else {
            return false;
        };
        if self.wanted.as_ref() != Some(&answer.file) {
            return false;
        }
        self.wanted = None;
        self.install(answer)
    }

    /// Puts a finished comparison on screen, or says why there is none.
    ///
    /// A failure is a notice on the status line rather than an exit: a
    /// repository can hold one file that cannot be read, and quitting over it
    /// would lose the review.
    fn install(&mut self, answer: Answer) -> bool {
        // Where the reader was, if this is the file they are already on. Kept
        // rather than used to refuse the work: a working-tree file can change
        // under an open review, and this is the only gesture that re-reads it,
        // so refusing would show yesterday's bytes for ever — the staleness
        // D51 deleted the diff cache to avoid. A new pane starts at the top,
        // which is right for a different file and wrong for this one.
        // The cursor of the pane showing the *file*, not of the focused pane —
        // which is the list, since that is what the reader pressed enter in.
        let keep = self
            .showing(&answer.file)
            .then(|| self.view.tab().shown())
            .flatten()
            .map(|id| self.view.pane_for(id).viewport.cursor());
        match answer.content {
            Ok(content) => {
                self.view.show(Buffer::diff(content));
                if let Some(line) = keep {
                    let id = self.view.tab().shown().expect("just shown");
                    let rows = self.view.buffer(id).view_lines();
                    self.view
                        .pane_mut_for(id)
                        .viewport
                        .jump(line.min(rows.saturating_sub(1)), rows);
                }
                self.notice = None;
                // A buffer that has just arrived on screen has no colours, and
                // nothing else will ask for them until the reader presses a
                // key. The first file is asked for before the loop starts, so
                // without this it stays in plain text until they touch
                // something — which is what happened.
                self.request();
            }
            Err(why) => self.notice = Some(why),
        }
        true
    }

    /// Whether this exact comparison is already in the pane beside the list.
    ///
    /// Compared by revisions and path, not by path: the file that is staged
    /// and then edited again is listed twice, and those two rows are two
    /// different comparisons that must each be openable.
    fn showing(&self, file: &file_types::ChangedFile) -> bool {
        let Some(id) = self.view.tab().shown() else {
            return false;
        };
        self.view.buffer(id).file() == Some(&file.file)
    }

    /// The file the list has selected, if a list has focus.
    fn selected_file(&self) -> Option<file_types::ChangedFile> {
        let pane = self.view.focused();
        let buffer = self.view.buffer(pane.buffer);
        let cursor = pane.viewport.cursor();
        match buffer.buffer_type() {
            crate::view::BufferType::Explorer(explorer) => {
                Some(explorer.entry(cursor)?.file.clone())
            }
            _ => None,
        }
    }

    /// Applies one key, without a terminal event to wrap it in.
    ///
    /// What a test presses. `handle` takes a crossterm event because that is
    /// what the loop receives; a test has a key and nothing to put it in.
    pub fn press(&mut self, key: crokey::KeyCombination) -> Flow {
        // Any key answers the last notice, so a message about a file the
        // reader has moved away from does not sit under the file they are
        // reading now. The same rule as the buffer's `exhausted` note, and the
        // same reason: it is vim's echo area, which needs no clock.
        self.notice = None;
        match self.resolver.key(key, self.view.keymap_type()) {
            Resolution::Run(command) => self.dispatch(command),
            Resolution::Pending | Resolution::Cancelled | Resolution::Unbound => Flow::Continue,
        }
    }

    /// Applies one terminal event.
    pub fn handle(&mut self, event: &crossterm::event::Event) -> Flow {
        let Some(key) = crate::input::press(event) else {
            // A resize needs no command: the next frame simply has a different
            // height, and the viewport re-examines itself when told.
            return Flow::Continue;
        };
        // Which keymap is live is the focused buffer's answer, not a constant.
        // That is the whole mechanism by which the explorer gets its own keys
        // without this function learning what an explorer is.
        self.press(key)
    }

    /// Sends a command to whichever level can execute it.
    ///
    /// The arms are the levels, in containment order. An action goes to the
    /// lowest level that contains everything it affects: a motion to the
    /// focused buffer, quitting to the terminal's owner, opening to the view
    /// that will hold what comes back.
    fn dispatch(&mut self, command: Command) -> Flow {
        match command.action {
            Action::Buffer(action) => {
                let count = command.repeat();
                let (buffer, viewport) = self.view.focused_mut();
                buffer.act(action, count, viewport);
                Flow::Continue
            }
            // The two view levels between the buffer and the view are
            // uninhabited: nothing yet affects more than one buffer within a
            // tab. Each is a level with an executor of its own, so when the
            // explorer gives it a command the arm is already here and the
            // compiler names what is missing.
            Action::Pane(action) => match action {},
            Action::Tab(TabAction::FocusNext) => {
                self.view.tab_mut().focus_next();
                Flow::Continue
            }
            Action::Tab(action @ (TabAction::WidenLeft | TabAction::NarrowLeft)) => {
                let step = match action {
                    TabAction::WidenLeft => RESIZE_STEP,
                    _ => -RESIZE_STEP,
                };
                // In i32, and saturating: `5000>` is a count a reader can
                // type by accident, and the product of two i16 overflows long
                // before the border reaches either end. The tab clamps it.
                let by = i32::from(step).saturating_mul(command.repeat() as i32);
                self.view.tab_mut().resize(by);
                Flow::Continue
            }
            Action::View(ViewAction::ToggleLayout) => {
                self.view.toggle_layout();
                Flow::Continue
            }
            Action::View(ViewAction::ToggleSyntax) => {
                self.view.toggle_syntax();
                Flow::Continue
            }
            // A directory folds where it stands, and only a file is asked for.
            // Which of the two the row is, is the buffer's answer: this level
            // knows that a list can deal with its own selection, not what a
            // list is.
            Action::View(ViewAction::Open) => {
                let (buffer, viewport) = self.view.focused_mut();
                let cursor = viewport.cursor();
                if buffer.select(cursor) {
                    let lines = buffer.view_lines();
                    viewport.jump(cursor.min(lines.saturating_sub(1)), lines);
                } else {
                    self.open();
                }
                Flow::Continue
            }
            Action::Program(ProgramAction::Quit) => Flow::Quit,
            Action::Program(ProgramAction::Suspend) => Flow::Suspend,
            // Everything is redrawn after every event already; this exists so
            // a reader whose screen has been corrupted by another program has
            // something to press.
            Action::Program(ProgramAction::Redraw) => Flow::Continue,
        }
    }
}

/// Takes over the terminal and reviews until the reader quits.
///
/// One argument, because nothing in here has to be supplied from outside any
/// more. The two workers are the session's, and both answer through a queue.
///
/// The terminal is restored by [`Screen`]'s `Drop`, so an error returned from
/// anywhere in here still leaves a usable shell.
pub fn run(session: &mut Session) -> std::io::Result<()> {
    let mut screen = Screen::open()?;
    // Before the first frame, so the file the caller asked for is already
    // being compared while the terminal is set up. Without it nothing is
    // outstanding, the wait below blocks for a key rather than for a frame,
    // and the first file arrives only once the reader presses something.
    session.request_file();
    session.draw(screen.terminal())?;

    loop {
        // Whatever the workers finished since the last frame. Neither waits.
        // Both are asked, not one and then the other, because a colour and a
        // comparison can land on the same frame.
        let coloured = session.collect();
        let compared = session.collect_file();
        if coloured || compared {
            // Anything wanted while a request was running was dropped rather
            // than queued. This is where it is asked for again, from a
            // starting point that is now current.
            session.request();
            session.request_file();
            session.draw(screen.terminal())?;
        }

        // Where the thread sleeps. With work outstanding it gives up after a
        // frame's worth of time so the next piece can be collected; otherwise
        // it waits for a key and costs nothing until one arrives.
        let Some(event) = screen.next_event(session.painting() || session.opening())? else {
            continue;
        };

        match session.handle(&event) {
            Flow::Quit => return Ok(()),
            Flow::Suspend => screen.suspend()?,
            Flow::Continue => {}
        }
        // A motion may have brought lines into view that nothing has coloured,
        // and enter may have chosen a row that nothing has compared.
        session.request();
        session.request_file();
        session.draw(screen.terminal())?;
    }
}
