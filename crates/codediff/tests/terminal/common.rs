use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::pty::Output;

pub(super) fn wait_for_output(output: &Arc<Mutex<Output>>, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = all_output(output);
        if current.contains(needle) {
            return current;
        }
        assert!(
            Instant::now() < deadline,
            "terminal output never contained {needle:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// What a `rows` × `cols` terminal shows after everything written so far.
pub(super) fn screen_text(output: &Arc<Mutex<Output>>, rows: u16, cols: u16) -> String {
    // Copy first: parsing under the lock would stall the reader thread.
    let bytes = output
        .lock()
        .expect("nothing else holds the lock")
        .bytes
        .clone();
    let mut screen = vt100::Parser::new(rows, cols, 0);
    screen.process(&bytes);
    screen.screen().contents()
}

/// Waits until the screen shows every one of `expected`, and returns it.
///
/// The screen, not the bytes: a redraw writes only the cells that changed,
/// and work that finishes in the background draws when it is ready.
#[track_caller]
pub(super) fn wait_for_screen(
    output: &Arc<Mutex<Output>>,
    (rows, cols): (u16, u16),
    expected: &[&str],
) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let screen = screen_text(output, rows, cols);
        if expected.iter().all(|text| screen.contains(text)) {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "the screen never showed {expected:?}: {screen}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Sends `input`, then waits until the screen shows every one of `expected`.
#[track_caller]
pub(super) fn send_and_expect(
    writer: &mut dyn Write,
    output: &Arc<Mutex<Output>>,
    size: (u16, u16),
    input: &[u8],
    expected: &[&str],
) -> String {
    writer.write_all(input).expect("sending input");
    writer.flush().expect("flushing input");
    wait_for_screen(output, size, expected)
}

pub(super) fn all_output(output: &Arc<Mutex<Output>>) -> String {
    String::from_utf8_lossy(&output.lock().expect("nothing else holds the lock").bytes).into_owned()
}
