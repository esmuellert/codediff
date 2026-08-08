//! Terminal ownership: alternate screen + raw mode on entry, restored on exit.
//!
//! Both must be undone however the program ends, or the shell is left with no
//! echo and an invisible cursor.

use std::io::{self, Stdout, Write};

use crossterm::{ExecutableCommand, cursor, event, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// The terminal, restored when this is dropped.
///
/// Poll interval while a file is being coloured: one frame at 60 Hz.
/// A keypress always ends the wait immediately.
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

/// Poll interval when idle. Not truly blocking, so signals are noticed.
const IDLE: std::time::Duration = std::time::Duration::from_millis(250);

pub struct Screen {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Screen {
    /// Takes over the terminal. Installs a panic hook so panics restore it.
    pub fn open() -> io::Result<Self> {
        install_panic_hook();
        #[cfg(unix)]
        std::sync::LazyLock::force(&KILLED);
        take()?;
        Ok(Self {
            terminal: Terminal::new(CrosstermBackend::new(io::stdout()))?,
        })
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Waits for the next terminal event, or returns `None` after a timeout.
    ///
    /// When `waiting` is true (something is being coloured), times out after
    /// one frame so the caller can collect results. Otherwise blocks until a
    /// key arrives.
    pub fn next_event(&self, waiting: bool) -> io::Result<Option<event::Event>> {
        let wait = if waiting { FRAME } else { IDLE };
        if event::poll(wait)? {
            return event::read().map(Some);
        }
        stop_if_killed();
        Ok(None)
    }

    /// Restores the terminal, sends SIGTSTP, and re-takes on resume.
    pub fn suspend(&mut self) -> io::Result<()> {
        restore();

        #[cfg(unix)]
        signal_hook::low_level::raise(signal_hook::consts::SIGTSTP)?;

        // After resume the alt screen is blank but ratatui thinks the old frame
        // is still there. A fresh Terminal forces a full repaint.
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

/// Undoes [`take`]. Ignores errors (runs from Drop and from the panic hook).
pub fn restore() {
    let mut stdout = io::stdout();
    let _ = stdout.execute(cursor::Show);
    let _ = stdout.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    let _ = stdout.flush();
}

/// If a kill signal has been received, restore the terminal and exit.
///
/// Called from the poll loop rather than from the signal handler, because
/// signal handlers can only safely set an atomic flag.
#[cfg(unix)]
fn stop_if_killed() {
    use std::sync::atomic::Ordering;
    let Some(signal) = KILLED.iter().find(|(_, flag)| flag.load(Ordering::Relaxed)) else {
        return;
    };
    restore();
    std::process::exit(128 + signal.0);
}

#[cfg(not(unix))]
fn stop_if_killed() {}

/// Kill signals we handle. SIGINT is not here — raw mode delivers Ctrl-C as a key.
#[cfg(unix)]
static KILLED: std::sync::LazyLock<[(i32, std::sync::Arc<std::sync::atomic::AtomicBool>); 3]> =
    std::sync::LazyLock::new(|| {
        [
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
            signal_hook::consts::SIGQUIT,
        ]
        .map(|signal| {
            let flag = std::sync::Arc::<std::sync::atomic::AtomicBool>::default();
            // Ignored: failing to install a handler leaves the
            // signal's default action, which is what happened before this
            // existed. It must not stop the review.
            let _ = signal_hook::flag::register(signal, std::sync::Arc::clone(&flag));
            (signal, flag)
        })
    });

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
