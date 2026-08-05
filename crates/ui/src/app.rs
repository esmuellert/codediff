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

use ratatui::backend::Backend;

use crate::draw;
use crate::input::{Action, Command, ProgramAction, Resolution, Resolver, ViewAction};
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
    /// Every colour anything open has. Owned here rather than by a buffer, so
    /// a file keeps its colours when the reader moves away and comes back.
    store: Store,
}

impl Session {
    /// Opens one buffer.
    ///
    /// One constructor, because how a buffer is drawn follows from its kind
    /// rather than from which function the caller chose.
    pub fn new(buffer: Buffer, theme: Theme) -> Self {
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
            store,
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
            );
        })?;
        Ok(())
    }

    /// Applies one terminal event.
    pub fn handle(&mut self, event: &crossterm::event::Event) -> Flow {
        let Some(key) = crate::input::press(event) else {
            // A resize needs no command: the next frame simply has a different
            // height, and the viewport re-examines itself when told.
            return Flow::Continue;
        };
        // Which keymap is live is the focused buffer's answer, not a constant.
        // That is the whole mechanism by which the explorer will get its own
        // keys without this function learning what an explorer is.
        match self.resolver.key(key, self.view.keymap_type()) {
            Resolution::Run(command) => self.dispatch(command),
            Resolution::Pending | Resolution::Cancelled | Resolution::Unbound => Flow::Continue,
        }
    }

    /// Sends a command to whichever level can execute it.
    ///
    /// The arms are the levels, in containment order. An action goes to the
    /// lowest level that contains everything it affects: a motion to the
    /// focused buffer, quitting to the terminal's owner, anything needing IO
    /// out of the crate entirely.
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
            Action::Tab(action) => match action {},
            Action::View(ViewAction::ToggleLayout) => {
                self.view.toggle_layout();
                Flow::Continue
            }
            Action::View(ViewAction::ToggleSyntax) => {
                self.view.toggle_syntax();
                Flow::Continue
            }
            Action::Program(ProgramAction::Quit) => Flow::Quit,
            Action::Program(ProgramAction::Suspend) => Flow::Suspend,
            // Everything is redrawn after every event already; this exists so
            // a reader whose screen has been corrupted by another program has
            // something to press.
            Action::Program(ProgramAction::Redraw) => Flow::Continue,
            // Uninhabited until the explorer gives a key something to ask for.
            // Written now so that adding one is an addition rather than a
            // reshaping of this match.
            Action::Task(task) => match task {},
        }
    }
}

/// Takes over the terminal and reviews until the reader quits.
///
/// The terminal is restored by [`Screen`]'s `Drop`, so an error returned from
/// anywhere in here still leaves a usable shell.
pub fn run(session: &mut Session) -> std::io::Result<()> {
    let mut screen = Screen::open()?;
    session.draw(screen.terminal())?;

    loop {
        // Whatever the worker finished since the last frame. Never waits.
        if session.collect() {
            // A file only ever has one request out at a time, so anything
            // wanted while that one was running was dropped rather than
            // queued. This is where it is asked for again, with a starting
            // point that is now current.
            session.request();
            session.draw(screen.terminal())?;
        }

        // Where the thread sleeps. With colouring outstanding it gives up
        // after a frame's worth of time so the next piece can be collected;
        // otherwise it waits for a key and costs nothing until one arrives.
        let Some(event) = screen.next_event(session.painting())? else {
            continue;
        };

        match session.handle(&event) {
            Flow::Quit => return Ok(()),
            Flow::Suspend => screen.suspend()?,
            Flow::Continue => {}
        }
        // A motion may have brought lines into view that nothing has coloured.
        session.request();
        session.draw(screen.terminal())?;
    }
}
