//! Reading keys, drawing frames, and stopping.
//!
//! One loop, entirely synchronous: block for an event, resolve it to a
//! command, dispatch, draw. Nothing here computes a diff or touches a
//! repository, so this loop cannot be made slow by anything except drawing —
//! which is the whole point of separating it from the work that reads files.
//!
//! [`Session::dispatch`] is where the three kinds of command diverge, and it
//! is the only place that can see all three of their executors. That is why
//! resolving and dispatching are separate: [`crate::input`] would have to
//! reach the view, the terminal and the task runner at once to do both.

use ratatui::backend::Backend;

use crate::draw;
use crate::input::{Action, Command, ProgramAction, Resolution, Resolver, ViewAction};
use crate::terminal::Screen;
use crate::theme::Theme;
use crate::view::Buffer;
use crate::view::View;

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
}

impl Session {
    /// Opens one buffer.
    ///
    /// One constructor, because how a buffer is drawn follows from its kind
    /// rather than from which function the caller chose.
    pub fn new(buffer: Buffer, theme: Theme) -> Self {
        Self {
            view: View::single(buffer),
            theme,
            resolver: Resolver::new(),
        }
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
        // Colouring happens here, outside the frame, because `draw` holds no
        // state and how far a file has been read is state. The height is the
        // body's, one row short of the terminal's, which is the same
        // arithmetic the layout does — a row out either way would colour one
        // line too many or too few, and neither is visible.
        let height = terminal.size()?.height;
        self.view.reach(u32::from(height.saturating_sub(1)));
        terminal.draw(|frame| {
            let area = frame.area();
            draw::render(frame.buffer_mut(), area, &mut self.view, &self.theme);
        })?;
        Ok(())
    }

    /// Colours a little more of the file, and says whether anything changed.
    ///
    /// What an idle moment calls. A redraw is only worth it if this returns
    /// true, which it stops doing once the file is fully read — so an idle
    /// session settles down to doing nothing at all rather than spinning.
    pub fn read_more(&mut self) -> bool {
        self.view.read_more()
    }

    /// Whether what is on screen has been coloured yet.
    ///
    /// False only just after a leap through a very long file, where the frame
    /// drew what it had and left the rest plain.
    pub fn caught_up(&self) -> bool {
        self.view.caught_up()
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

    // Whether it is worth interrupting the wait for a key to colour a little
    // more. Set again after every keypress because a key can move to a part
    // of the file, or a buffer, that has not been read.
    let mut reading = true;

    loop {
        let event = if reading {
            // Colour a little more while the reader is deciding what to press.
            // This is VS Code's background tokenizer without a thread, a
            // channel or a clock: an idle terminal is idle for whole seconds,
            // which is hundreds of thousands of lines, so by the time anyone
            // scrolls to the end of a long file it has usually already been
            // read.
            //
            // **No redraw.** Every frame colours what it is about to show
            // before it shows it, so this is always work on lines that are not
            // on screen — redrawing would repaint an identical screen at five
            // hundred frames a second.
            match screen.next_event_or_idle()? {
                Some(event) => event,
                None => {
                    // Redraw only when the reader is looking at something that
                    // has not been coloured yet — which happens after a leap
                    // through a very long file and at no other time. Ordinary
                    // background reading is always below the screen, so this
                    // is the difference between one extra frame and five
                    // hundred a second.
                    let behind = !session.caught_up();
                    reading = session.read_more();
                    if behind {
                        session.draw(screen.terminal())?;
                    }
                    continue;
                }
            }
        } else {
            // Nothing left to colour, so wait properly rather than spin.
            screen.next_event()?
        };

        match session.handle(&event) {
            Flow::Quit => return Ok(()),
            Flow::Suspend => screen.suspend()?,
            Flow::Continue => {}
        }
        reading = true;
        session.draw(screen.terminal())?;
    }
}
