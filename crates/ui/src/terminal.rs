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
/// How long to wait for a key while a file is still being coloured.
///
/// One frame at sixty hertz. The loop does no work between waits — the work is
/// on the painter's thread — so this is only how stale the screen may be, and
/// past a frame nobody can see the difference. It costs sixty wake-ups a
/// second while a file is being coloured, and none at all otherwise.
///
/// A keypress ends the wait immediately whatever this is, so it has no bearing
/// on how quickly a key is answered.
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

/// How long to wait for a key when there is nothing else to do.
///
/// Not a blocking wait, because a blocking wait cannot be interrupted: a
/// signal wakes the process, and crossterm goes straight back to sleep rather
/// than reporting it. Waking four times a second is what lets a `kill` be
/// noticed. Each wake is one `poll` that returns at once, which is a few
/// microseconds — measured at under 0.1% of a core while idle.
const IDLE: std::time::Duration = std::time::Duration::from_millis(250);

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

    /// Waits for the next terminal event.
    ///
    /// `waiting` says whether anything is still being coloured. If it is, the
    /// wait gives up after [`FRAME`] and answers `None`, so the caller can
    /// collect what the painter has finished; if it is not, this blocks until
    /// a key is pressed and the thread costs nothing at all.
    ///
    /// Either way a keypress ends the wait at once — the timeout paces
    /// *collection*, never the answer to a key.
    pub fn next_event(&self, waiting: bool) -> io::Result<Option<event::Event>> {
        let wait = if waiting { FRAME } else { IDLE };
        if event::poll(wait)? {
            return event::read().map(Some);
        }
        // Nothing arrived. The one thing that could have happened while we
        // slept is a signal, which is checked here rather than in the handler
        // it came from — see [`stop_if_killed`].
        stop_if_killed();
        Ok(None)
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

/// Notices a signal asking us to stop, restores the terminal, and goes.
///
/// Called from the wait, not from the handler that set the flag. A handler may
/// only do what is safe to interrupt anything with — setting one atomic — and
/// `signal_hook`'s API for running arbitrary code in one is `unsafe`, which
/// this workspace forbids. So the handler sets a flag and this does the work,
/// on the thread that owns the terminal, in order.
///
/// The exit code follows the convention for a signal, so a shell reports the
/// same thing it would have reported without a handler at all.
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

/// The signals that mean stop, and whether each has arrived.
///
/// `SIGINT` is not here: raw mode delivers `Ctrl-C` as an ordinary key, and
/// the quit path already restores.
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
            // Ignored deliberately: failing to install a handler leaves the
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
