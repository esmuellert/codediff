use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use super::super::common::wait_for_output;
use super::super::pty::{Output, collect, drawn, drawn_after};

const COLS: u16 = 120;
const FULL_ROWS: u16 = 512;
const ROWS: u16 = 18;
const PANE_LEFT: u16 = 35;

type Row = Vec<vt100::Cell>;

struct Repository(PathBuf);

impl Repository {
    fn new(layout: &str, wrap: bool) -> Self {
        let path = std::env::temp_dir().join(format!(
            "codediff-directory-scroll-{layout}-{}",
            std::process::id(),
        ));
        fs::create_dir_all(path.join("src")).unwrap();
        for name in ["a", "b"] {
            fs::write(path.join(format!("src/{name}.txt")), lines("OLD", name)).unwrap();
        }
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "codediff@example.com"],
            vec!["config", "user.name", "codediff e2e"],
            vec!["add", "src"],
            vec!["commit", "-qm", "initial"],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&path)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        for name in ["a", "b"] {
            fs::write(path.join(format!("src/{name}.txt")), lines("NEW", name)).unwrap();
        }
        fs::write(path.join(".git/codediff-test.json"), format!(
            r#"{{"ui":{{"layout":"{layout}","wrap":{wrap},"explorer_width":30,"theme":"catppuccin-mocha"}}}}"#,
        )).unwrap();
        Self(path)
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lines(version: &str, file: &str) -> String {
    (1..=32)
        .map(|line| format!("{version}_{file}_{line:03} a_long_identifier_with_a_wrapped_tail\n"))
        .collect()
}

struct Terminal {
    master: Option<Box<dyn MasterPty + Send>>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    collector: Option<JoinHandle<()>>,
    output: Arc<Mutex<Output>>,
    parser: vt100::Parser,
    consumed: usize,
}

impl Terminal {
    fn open(repo: &Path) -> Self {
        let pty = native_pty_system()
            .openpty(PtySize {
                rows: FULL_ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
        command.arg("--config");
        command.arg(repo.join(".git/codediff-test.json"));
        command.cwd(repo);
        command.env("TERM", "xterm-256color");
        let child = pty.slave.spawn_command(command).unwrap();
        drop(pty.slave);
        let (collector, output) = collect(pty.master.try_clone_reader().unwrap());
        let writer = pty.master.take_writer().unwrap();
        drawn(&output);
        let mut terminal = Self {
            master: Some(pty.master),
            child,
            writer,
            collector: Some(collector),
            output,
            parser: vt100::Parser::new(FULL_ROWS, COLS, 0),
            consumed: 0,
        };
        terminal.send(b"j");
        wait_for_output(&terminal.output, "src/b.txt");
        drawn(&terminal.output);
        terminal.update();
        let screen = terminal.parser.screen().contents();
        assert!(
            screen.contains("NEW_b_032"),
            "full-height reference is incomplete:\n{screen}"
        );
        terminal
    }

    fn update(&mut self) {
        let output = self.output.lock().unwrap();
        self.parser.process(&output.bytes[self.consumed..]);
        self.consumed = output.bytes.len();
    }

    fn send(&mut self, input: &[u8]) {
        let before = self.output.lock().unwrap().bytes.len();
        self.writer.write_all(input).unwrap();
        self.writer.flush().unwrap();
        drawn_after(&self.output, before);
        self.update();
    }

    fn resize(&mut self, rows: u16) {
        let before = self.output.lock().unwrap().bytes.len();
        self.parser.screen_mut().set_size(rows, COLS);
        self.master
            .as_ref()
            .unwrap()
            .resize(PtySize {
                rows,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        drawn_after(&self.output, before);
        self.update();
    }

    fn pane(&self, rows: u16) -> Vec<Row> {
        (1..rows - 1)
            .map(|row| {
                (PANE_LEFT..COLS - 1)
                    .map(|col| self.parser.screen().cell(row, col).unwrap().clone())
                    .collect()
            })
            .collect()
    }

    fn explorer_text(&self, rows: u16) -> String {
        (1..rows - 1)
            .map(|row| {
                (0..PANE_LEFT)
                    .map(|col| self.parser.screen().cell(row, col).unwrap().contents())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn quit(&mut self) {
        self.writer.write_all(b"q").unwrap();
        self.writer.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline, "codediff did not exit");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.master.take();
        if let Some(collector) = self.collector.take() {
            let _ = collector.join();
        }
    }
}

fn row_text(row: &Row) -> String {
    row.iter()
        .map(|cell| {
            if cell.contents().is_empty() {
                " "
            } else {
                cell.contents()
            }
        })
        .collect()
}

#[test]
fn real_directory_cards_keep_every_row_when_scrolled_down_and_back_up() {
    for (layout, wrap) in [
        ("side-by-side", false),
        ("side-by-side", true),
        ("inline", false),
        ("inline", true),
    ] {
        let repo = Repository::new(layout, wrap);
        let mut terminal = Terminal::open(&repo.0);
        let reference = terminal.pane(FULL_ROWS);
        let content_height = reference
            .iter()
            .rposition(|row| !row_text(row).trim().is_empty())
            .unwrap()
            + 1;
        assert!(content_height > usize::from(ROWS));
        let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/terminal-scroll/directory-cards-{layout}-wrap-{wrap}"
        ));
        fs::create_dir_all(&artifacts).unwrap();
        fs::write(
            artifacts.join("reference.txt"),
            terminal.parser.screen().contents(),
        )
        .unwrap();
        terminal.resize(ROWS);
        let viewport_height = usize::from(ROWS - 2);
        let max_top = content_height - viewport_height;
        let mut top = 0;
        for direction in [1, -1] {
            loop {
                if top == 15 {
                    terminal.resize(24);
                    let resized: Vec<_> = terminal.pane(24).iter().map(row_text).collect();
                    let expected: Vec<_> = reference[top..top + 22].iter().map(row_text).collect();
                    assert_eq!(resized, expected, "resized at {top}: {layout} wrap={wrap}");
                    terminal.resize(ROWS);
                    let expected = &reference[top..top + viewport_height];
                    let expected_text: Vec<_> = expected.iter().map(row_text).collect();
                    for expanded in [false, true] {
                        terminal.send(b"\x1b[<0;4;3M\x1b[<0;4;3m");
                        assert_eq!(
                            terminal.explorer_text(ROWS).contains("a.txt"),
                            expanded,
                            "directory did not toggle"
                        );
                        fs::write(
                            artifacts.join(format!("toggle-{direction}-{expanded}.txt")),
                            terminal.parser.screen().contents(),
                        )
                        .unwrap();
                        let actual = terminal.pane(ROWS);
                        let actual_text: Vec<_> = actual.iter().map(row_text).collect();
                        assert_eq!(
                            actual_text, expected_text,
                            "folding reset the cards at {top}: {layout} wrap={wrap}"
                        );
                        for (actual, expected) in
                            actual.iter().flatten().zip(expected.iter().flatten())
                        {
                            assert_eq!(actual.fgcolor(), expected.fgcolor());
                            assert_eq!(actual.bgcolor(), expected.bgcolor());
                        }
                    }
                }
                let actual = terminal.pane(ROWS);
                fs::write(
                    artifacts.join(format!("{direction}-{top:03}.txt")),
                    terminal.parser.screen().contents(),
                )
                .unwrap();
                let expected = &reference[top..top + viewport_height];
                let actual_text: Vec<_> = actual.iter().map(row_text).collect();
                let expected_text: Vec<_> = expected.iter().map(row_text).collect();
                if actual_text != expected_text {
                    fs::write(
                        artifacts.join("failure.ansi"),
                        &terminal.output.lock().unwrap().bytes,
                    )
                    .unwrap();
                }
                assert_eq!(
                    actual_text,
                    expected_text,
                    "terminal lost rows: layout={layout} top={top} direction={direction}; captures: {}",
                    artifacts.display()
                );
                for (y, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    for (x, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                        assert_eq!(
                            actual.fgcolor(),
                            expected.fgcolor(),
                            "foreground at {x},{y} top={top}"
                        );
                        assert_eq!(
                            actual.bgcolor(),
                            expected.bgcolor(),
                            "background at {x},{y} top={top}"
                        );
                    }
                }
                if (direction > 0 && top == max_top) || (direction < 0 && top == 0) {
                    terminal.send(if direction > 0 {
                        b"\x1b[<65;100;10M"
                    } else {
                        b"\x1b[<64;100;10M"
                    });
                    let clamped: Vec<_> = terminal.pane(ROWS).iter().map(row_text).collect();
                    assert_eq!(clamped, expected_text, "scrolled past the content boundary");
                    break;
                }
                top = if direction > 0 {
                    (top + 3).min(max_top)
                } else {
                    top.saturating_sub(3)
                };
                terminal.send(if direction > 0 {
                    b"\x1b[<65;100;10M"
                } else {
                    b"\x1b[<64;100;10M"
                });
            }
        }
        terminal.quit();
    }
}
