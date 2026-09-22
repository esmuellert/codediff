//! End-to-end coverage for user configuration.
#![cfg(unix)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use screen::Screen;
use serde_json::{Value, json};

mod keybindings;
mod screen;

const COLS: u16 = 100;
const HEIGHT: u16 = 24;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codediff-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("creating a fixture directory");
        }
        std::fs::write(path, text).expect("writing a fixture file");
    }

    fn config(&self, value: Value) -> PathBuf {
        let path = self.dir.join(".codediff-config.json");
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&value).expect("encoding config"),
        )
        .expect("writing config");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    last_write: Option<Instant>,
}

struct Run {
    output: String,
    success: bool,
    timed_out: bool,
}

fn run_app(fixture: &Fixture, config: Value, path: Option<&str>, keys: &[u8]) -> Run {
    run_app_steps(fixture, config, path, &[(keys, Duration::ZERO)])
}

fn run_app_steps(
    fixture: &Fixture,
    config: Value,
    path: Option<&str>,
    steps: &[(&[u8], Duration)],
) -> Run {
    let config = fixture.config(config);
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: HEIGHT,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.arg("--config");
    command.arg(&config);
    if let Some(path) = path {
        command.arg(path);
    }
    command.cwd(&fixture.dir);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "");
    command.env("COLORFGBG", "0;0");

    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let output = Arc::new(Mutex::new(Output::default()));
    let filling = Arc::clone(&output);
    let collector = std::thread::spawn(move || {
        let mut reader = reader;
        let mut chunk = [0u8; 4096];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            let mut held = filling.lock().expect("output lock");
            held.bytes.extend_from_slice(&chunk[..read]);
            held.last_write = Some(Instant::now());
        }
    });

    wait_for_draw(&output);
    if steps.iter().any(|(keys, _)| !keys.is_empty())
        && child.try_wait().expect("checking codediff").is_none()
    {
        let mut writer = pty.master.take_writer().expect("writing to the pty");
        if path.is_some() {
            let _ = writer.write_all(b"j\r");
            let _ = writer.flush();
            std::thread::sleep(Duration::from_millis(600));
        }
        for (keys, pause) in steps {
            if !keys.is_empty() {
                let _ = writer.write_all(keys);
                let _ = writer.flush();
            }
            if *pause > Duration::ZERO {
                std::thread::sleep(*pause);
            }
        }
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let (success, timed_out) = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break (status.success(), false);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break (false, true);
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    drop(pty.master);
    collector.join().expect("collecting terminal output");
    let bytes = output.lock().expect("output lock").bytes.clone();
    Run {
        output: String::from_utf8_lossy(&bytes).into_owned(),
        success,
        timed_out,
    }
}

fn wait_for_draw(output: &Arc<Mutex<Output>>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        {
            let held = output.lock().expect("output lock");
            if !held.bytes.is_empty()
                && held
                    .last_write
                    .is_some_and(|last| last.elapsed() >= Duration::from_millis(250))
            {
                return;
            }
        }
        assert!(Instant::now() < deadline, "codediff never drew a frame");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn screen(run: &Run) -> Screen {
    Screen::parse(&run.output)
}

#[test]
fn layout_from_config_changes_the_running_app() {
    let fixture = Fixture::new("layout");
    fixture.write("modified.txt", "one\nTWO\nthree\n");
    let inline = run_app(
        &fixture,
        json!({"ui": {"layout": "inline"}}),
        Some("modified.txt"),
        b"q",
    );
    let side_by_side = run_app(&fixture, json!({}), Some("modified.txt"), b"q");

    assert!(
        inline.success,
        "codediff did not exit cleanly: {}",
        inline.output
    );
    assert!(
        side_by_side.success,
        "default layout did not exit cleanly: {}",
        side_by_side.output
    );
    let inline_screen = screen(&inline);
    let side_by_side_screen = screen(&side_by_side);
    assert!(
        inline_screen.contains("TWO"),
        "the selected diff was not drawn"
    );
    assert!(
        inline_screen.vertical_bar_columns().len()
            < side_by_side_screen.vertical_bar_columns().len(),
        "inline view did not remove the center divider: inline={:?}, side_by_side={:?}",
        inline_screen.vertical_bar_columns(),
        side_by_side_screen.vertical_bar_columns()
    );
}

#[test]
fn wrap_from_config_changes_where_the_following_line_is_drawn() {
    let fixture = Fixture::new("wrap");
    let long = "0123456789 ".repeat(12);
    fixture.write("modified.txt", &format!("one\nWRAP_FLAG {long}\nTAIL\n"));

    let wrapped = run_app(&fixture, json!({}), Some("modified.txt"), b"q");
    let unwrapped = run_app(
        &fixture,
        json!({"ui": {"wrap": false}}),
        Some("modified.txt"),
        b"q",
    );
    assert!(wrapped.success, "wrapped app failed: {}", wrapped.output);
    assert!(
        unwrapped.success,
        "unwrapped app failed: {}",
        unwrapped.output
    );

    let wrapped_tail = screen(&wrapped).line_of("TAIL");
    let unwrapped_tail = screen(&unwrapped).line_of("TAIL");
    assert!(
        unwrapped_tail < wrapped_tail,
        "wrap=false did not reduce continuation lines: wrapped={wrapped_tail:?}, unwrapped={unwrapped_tail:?}"
    );
}

#[test]
fn theme_from_config_overrides_terminal_detection() {
    let fixture = Fixture::new("theme");
    let run = run_app(
        &fixture,
        json!({"ui": {"theme": "catppuccin-latte"}}),
        Some("modified.txt"),
        b"q",
    );

    assert!(run.success, "codediff did not exit cleanly: {}", run.output);
    assert!(
        screen(&run).has_rgb_colour(),
        "the configured true-colour theme was not used"
    );
}

#[test]
fn explorer_width_from_config_moves_the_pane_boundary() {
    let fixture = Fixture::new("explorer-width");
    let narrow = run_app(&fixture, json!({"ui": {"explorer_width": 20}}), None, b"q");
    let wide = run_app(&fixture, json!({"ui": {"explorer_width": 50}}), None, b"q");
    assert!(narrow.success, "narrow explorer failed: {}", narrow.output);
    assert!(wide.success, "wide explorer failed: {}", wide.output);

    let narrow_bars = screen(&narrow).vertical_bar_columns();
    let wide_bars = screen(&wide).vertical_bar_columns();
    assert_ne!(
        narrow_bars, wide_bars,
        "explorer width did not move a pane boundary"
    );
}

#[test]
fn explorer_mode_from_config_changes_the_file_lines() {
    let fixture = Fixture::new("explorer-mode");
    let run = run_app(
        &fixture,
        json!({"ui": {"explorer_mode": "list"}}),
        None,
        b"q",
    );

    assert!(run.success, "codediff did not exit cleanly: {}", run.output);
    let screen = screen(&run);
    assert!(
        screen.contains("deep/only/one/chain/leaf.txt"),
        "list mode did not show full paths"
    );
    assert!(
        !screen.contains("├ ") && !screen.contains("└ "),
        "list mode still drew tree guides"
    );
}

#[test]
fn whitespace_policy_from_config_changes_diff_highlighting() {
    let fixture = Fixture::new("whitespace");
    fixture.write("modified.txt", "one\n two \nthree\n");

    let strict = run_app(&fixture, json!({}), Some("modified.txt"), b"q");
    let ignored = run_app(
        &fixture,
        json!({"diff": {"ignore_trim_whitespace": true}}),
        Some("modified.txt"),
        b"q",
    );
    assert!(strict.success, "strict diff failed: {}", strict.output);
    assert!(
        ignored.success,
        "ignored-whitespace diff failed: {}",
        ignored.output
    );

    let strict_screen = screen(&strict);
    let ignored_screen = screen(&ignored);
    let strict_line = strict_screen.line_of("two").expect("strict line");
    let ignored_line = ignored_screen.line_of("two").expect("ignored line");
    assert!(
        strict_screen.styled_cells_in_line(strict_line, 45)
            > ignored_screen.styled_cells_in_line(ignored_line, 45),
        "ignoring trim whitespace did not remove the diff highlight: strict={:?}, ignored={:?}",
        strict_screen.styled_cells_in_line(strict_line, 45),
        ignored_screen.styled_cells_in_line(ignored_line, 45),
    );
}

#[test]
fn quit_keybinding_from_config_exits_on_the_configured_key() {
    let fixture = Fixture::new("keybinding");
    let run = run_app(
        &fixture,
        json!({"keybindings": {"quit": ["x"]}}),
        Some("modified.txt"),
        b"x",
    );

    assert!(
        run.success && !run.timed_out,
        "configured quit key did not exit: timed_out={}, output={}",
        run.timed_out,
        run.output
    );
}
