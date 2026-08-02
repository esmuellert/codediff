//! Owning the terminal while the program runs, and giving it back intact.
//!
//! Two things are switched on: the **alternate screen**, a blank secondary
//! buffer so the shell's scrollback is untouched underneath, and **raw mode**,
//! so keys arrive as keys rather than being interpreted — `q` reaches us, and
//! scrolling redraws the grid instead of moving the terminal's own scrollback.
//!
//! Both must be undone however the program ends. Left on, they leave a shell
//! with no echo and an invisible cursor, and the user has to type `reset`
//! blind. That is why this is the only module allowed to touch either, and why
//! it is tested through a pty rather than by looking at a screen.

use std::io::{self, Stdout, Write};

use crossterm::{ExecutableCommand, cursor, event, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// The terminal, restored when this is dropped.
pub struct Screen {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Screen {
    /// Takes over the terminal.
    ///
    /// Installs a panic hook first, so that a panic between here and [`Drop`]
    /// still restores the terminal before printing its message — otherwise the
    /// backtrace lands on the alternate screen and vanishes with it.
    pub fn open() -> io::Result<Self> {
        install_panic_hook();
        take()?;
        Ok(Self {
            terminal: Terminal::new(CrosstermBackend::new(io::stdout()))?,
        })
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Blocks until something happens.
    pub fn next_event(&self) -> io::Result<event::Event> {
        event::read()
    }

    /// Hands the terminal back, stops, and picks up where it left off.
    ///
    /// Raw mode turns off the terminal's own interpretation of `Ctrl-Z`, so
    /// the key arrives as an ordinary keypress and the signal has to be raised
    /// deliberately. Doing nothing would be safe but wrong: a reviewer
    /// expects `Ctrl-Z` to drop them at a shell and `fg` to bring the diff
    /// back, exactly as it does in an editor.
    ///
    /// On Windows there is no such signal and this does nothing.
    pub fn suspend(&mut self) -> io::Result<()> {
        restore();

        #[cfg(unix)]
        signal_hook::low_level::raise(signal_hook::consts::SIGTSTP)?;

        // Resumed. The alternate screen we come back to is blank, while
        // ratatui still believes the previous frame is on it, so the next draw
        // would send only the difference — which is nothing. A fresh
        // `Terminal` has no previous frame and therefore repaints everything.
        //
        // Deliberately not `Terminal::clear`: that round-trips to the terminal
        // to read the cursor position back, which is a reply we have to wait
        // for and which anything else reading the same stream can swallow.
        // Nothing here needs the cursor preserved — the screen is blank.
        take()?;
        self.terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        Ok(())
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        restore();
    }
}

/// Switches the terminal into the state the interface needs.
fn take() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(terminal::EnterAlternateScreen)?;
    stdout.execute(cursor::Hide)?;
    Ok(())
}

/// Undoes everything [`take`] did.
///
/// Deliberately ignores its own errors and never panics: it runs from `Drop`
/// and from the panic hook, where failing would replace a useful message with
/// a confusing one, or abort the process outright.
pub fn restore() {
    let mut stdout = io::stdout();
    let _ = stdout.execute(cursor::Show);
    let _ = stdout.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    let _ = stdout.flush();
}

/// Restores the terminal before the default hook prints anything.
///
/// Installed once, however many `Screen`s are opened.
fn install_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
    });
}
