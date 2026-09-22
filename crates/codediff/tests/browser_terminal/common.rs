use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::support::{Output, drawn_after};

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

pub(super) fn all_output(output: &Arc<Mutex<Output>>) -> String {
    String::from_utf8_lossy(&output.lock().expect("nothing else holds the lock").bytes).into_owned()
}

pub(super) fn output_since(output: &Arc<Mutex<Output>>, before: usize) -> String {
    let held = output.lock().expect("nothing else holds the lock");
    String::from_utf8_lossy(&held.bytes[before..]).into_owned()
}

pub(super) fn strip_csi(input: &str) -> String {
    let mut text = String::new();
    let mut saw_escape = false;
    let mut in_csi = false;
    for character in input.chars() {
        if in_csi {
            if ('@'..='~').contains(&character) {
                in_csi = false;
            }
        } else if saw_escape {
            saw_escape = false;
            in_csi = character == '[';
        } else if character == '\u{1b}' {
            saw_escape = true;
        } else if !character.is_control() {
            text.push(character);
        }
    }
    text
}
