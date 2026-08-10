//! Terminal ownership: alternate screen + raw mode on entry, restored on exit.
//!
//! Both must be undone however the program ends, or the shell is left with no
//! echo and an invisible cursor.

use std::io::{self, Stdout, Write};

use crossterm::{ExecutableCommand, QueueableCommand, cursor, event, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// The terminal, restored when this is dropped.
pub struct Screen {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Screen {
    /// Takes over the terminal. Installs a panic hook so panics restore it.
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

    /// Draws one frame, bracketed in synchronized output (DEC mode 2026).
    ///
    /// The terminal holds the previous frame on screen while receiving
    /// updates, then flips once at the end. This prevents partial-frame
    /// tearing that otherwise occurs when a redraw spans multiple pty reads.
    pub fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), io::Error>,
    {
        io::stdout().queue(terminal::BeginSynchronizedUpdate)?;
        let result = f(&mut self.terminal);
        let _ = io::stdout().execute(terminal::EndSynchronizedUpdate);
        result
    }

    /// Restores the terminal, sends SIGTSTP, and re-takes on resume.
    pub fn suspend(&mut self) -> io::Result<()> {
        restore();

        #[cfg(unix)]
        signal_hook::low_level::raise(signal_hook::consts::SIGTSTP)?;

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
    stdout.execute(event::EnableMouseCapture)?;
    stdout.execute(cursor::Hide)?;
    Ok(())
}

/// Undoes [`take`]. Ignores errors (runs from Drop and from the panic hook).
pub fn restore() {
    let mut stdout = io::stdout();
    // End any in-progress synchronized update so the terminal isn't frozen.
    let _ = stdout.write_all(b"\x1b[?2026l");
    let _ = stdout.execute(cursor::Show);
    let _ = stdout.execute(event::DisableMouseCapture);
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
