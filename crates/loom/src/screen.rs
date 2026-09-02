//! The terminal, taken on entry and given back however the program ends.
//!
//! Raw mode and the alternate screen must both be undone, or the shell is
//! left with no echo and an invisible cursor.

use std::io::{self, Stdout, Write};

use crossterm::{ExecutableCommand, QueueableCommand, cursor, event, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

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

    /// Draws one frame, bracketed in synchronized output (DEC mode 2026).
    ///
    /// The terminal holds the previous frame while receiving updates and
    /// flips once at the end, so a redraw spanning several pty reads does
    /// not tear.
    pub fn draw<F>(&mut self, paint: F) -> io::Result<()>
    where
        F: FnOnce(&mut Cells, Rect),
    {
        io::stdout().queue(terminal::BeginSynchronizedUpdate)?;
        let result = self.terminal.draw(|frame| {
            let area = frame.area();
            paint(frame.buffer_mut(), area);
        });
        let _ = io::stdout().execute(terminal::EndSynchronizedUpdate);
        result.map(|_| ())
    }

    /// Gives the terminal back, stops this process, and takes it again when
    /// the shell resumes us.
    #[cfg(unix)]
    pub fn suspend(&mut self) -> io::Result<()> {
        restore();
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

/// Switches the terminal into the state an interface needs.
fn take() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(terminal::EnterAlternateScreen)?;
    stdout.execute(event::EnableMouseCapture)?;
    stdout.execute(cursor::Hide)?;
    Ok(())
}

/// Undoes [`take`]. Ignores errors: it runs from `Drop` and from a panic.
pub fn restore() {
    let mut stdout = io::stdout();
    // End any synchronized update in progress, or the terminal stays frozen.
    let _ = stdout.write_all(b"\x1b[?2026l");
    let _ = stdout.execute(cursor::Show);
    let _ = stdout.execute(event::DisableMouseCapture);
    let _ = stdout.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    let _ = stdout.flush();
}

/// Restores the terminal before the default hook prints anything.
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
