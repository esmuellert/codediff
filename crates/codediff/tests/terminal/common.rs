use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::config::screen::Screen;
use super::pty::{Output, drawn_after};

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

pub(super) fn send_and_wait(
    writer: &mut dyn Write,
    output: &Arc<Mutex<Output>>,
    input: &[u8],
) -> String {
    let before = output
        .lock()
        .expect("nothing else holds the lock")
        .bytes
        .len();
    writer.write_all(input).expect("sending input");
    writer.flush().expect("flushing input");
    drawn_after(output, before);
    let held = output.lock().expect("nothing else holds the lock");
    String::from_utf8_lossy(&held.bytes[before..]).into_owned()
}

pub(super) fn wait_for_screen(
    output: &Arc<Mutex<Output>>,
    expected: impl Fn(&Screen) -> bool,
) -> Screen {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = all_output(output);
        let screen = Screen::parse(&current);
        if expected(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "terminal screen did not reach the expected state: {current:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn send_and_wait_for_screen(
    writer: &mut dyn Write,
    output: &Arc<Mutex<Output>>,
    input: &[u8],
    expected: impl Fn(&Screen) -> bool,
) -> Screen {
    writer.write_all(input).expect("sending input");
    writer.flush().expect("flushing input");
    wait_for_screen(output, expected)
}

pub(super) fn all_output(output: &Arc<Mutex<Output>>) -> String {
    String::from_utf8_lossy(&output.lock().expect("nothing else holds the lock").bytes).into_owned()
}
