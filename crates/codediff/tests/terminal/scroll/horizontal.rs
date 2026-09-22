use std::fs;
use std::io::Write;
use std::process::Command;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::super::common::{output_since, strip_csi};
use super::super::pty::{ENTER_ALT, LEAVE_ALT, collect, drawn, drawn_after, written};

#[test]
fn a_real_repo_mouse_click_keeps_side_by_side_horizontal_endpoints() {
    let repo = std::env::temp_dir().join(format!(
        "codediff-browser-horizontal-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&repo).expect("creating the test repository");
    let original = "LEFT_LONG_START ".to_owned() + &"abcdefghij".repeat(4) + " LEFT_END\n";
    fs::write(repo.join("sample.txt"), original).expect("writing the original file");
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "codediff@example.com"],
        vec!["config", "user.name", "codediff e2e"],
        vec!["add", "sample.txt"],
        vec!["commit", "-qm", "initial"],
    ] {
        assert!(
            Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .status()
                .expect("running git")
                .success(),
            "git {:?} failed",
            args
        );
    }
    let modified = "RIGHT_LONG_START ".to_owned() + &"0123456789".repeat(8) + " RIGHT_END\n";
    fs::write(repo.join("sample.txt"), modified).expect("writing the changed file");
    let config = repo.join("config.json");
    fs::write(
        &config,
        r#"{"version":1,"ui":{"layout":"side-by-side","wrap":false}}"#,
    )
    .expect("writing the test config");

    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.args(["--config", config.to_str().expect("config path")]);
    command.cwd(&repo);
    command.env("TERM", "xterm-256color");
    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);
    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);
    drawn(&output);
    let mut writer = pty.master.take_writer().expect("writing to the pty");

    let before_click = output.lock().expect("output lock").bytes.len();
    writer
        .write_all(b"\x1b[<0;10;5M\x1b[<0;10;5m\x1b[<0;70;2M\x1b[<0;70;2m")
        .expect("sending the Explorer and diff clicks");
    writer
        .flush()
        .expect("flushing the Explorer and diff clicks");
    drawn_after(&output, before_click);

    let before_focus = output.lock().expect("output lock").bytes.len();
    writer.write_all(b"\x1b[C").expect("focusing the diff view");
    writer.flush().expect("flushing the focus change");
    drawn_after(&output, before_focus);

    let before_end = output.lock().expect("output lock").bytes.len();
    writer.write_all(b"$").expect("sending horizontal end");
    writer.flush().expect("flushing horizontal end");
    drawn_after(&output, before_end);
    let end_text = strip_csi(&output_since(&output, before_end));
    assert!(
        end_text.contains("LEFT_END"),
        "shorter original side did not reach its own end: {end_text:?}"
    );
    assert!(
        end_text.contains("RIGHT_END"),
        "longer modified side did not reach its own end: {end_text:?}"
    );

    writer.write_all(b"q").expect("sending quit");
    writer.flush().expect("flushing quit");
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "codediff did not exit after horizontal scrolling"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    drop(pty.master);
    let output = written(collector, &output);
    let _ = fs::remove_dir_all(&repo);
    assert!(
        status.success(),
        "codediff exited unsuccessfully: {output:?}"
    );
    assert!(output.contains(ENTER_ALT));
    assert!(output.contains(LEAVE_ALT));
}
