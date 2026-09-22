use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::super::pty::{ENTER_ALT, LEAVE_ALT, collect, drawn, drawn_after, written};

#[derive(Debug, Default)]
struct Peak {
    rss_kb: u64,
    cpu_percent: f64,
}

#[test]
fn large_scroll_workload_stays_responsive() {
    let repo = std::env::temp_dir().join(format!(
        "codediff-scroll-stress-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&repo).expect("creating the stress repository");
    let original = (1..=2_000)
        .map(|line| format!("line {line:04} {}", "x".repeat(120)))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(repo.join("sample.txt"), &original).expect("writing the original file");
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "codediff@example.com"],
        vec!["config", "user.name", "codediff stress"],
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
    let changed = (1..=2_050)
        .map(|line| {
            if line == 2 {
                format!("changed {line:04} {}", "y".repeat(120))
            } else {
                format!("line {line:04} {}", "x".repeat(120))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(repo.join("sample.txt"), changed).expect("writing the changed file");
    let config = repo.join("config.json");
    fs::write(
        &config,
        r#"{"version":1,"ui":{"layout":"side-by-side","wrap":false}}"#,
    )
    .expect("writing the stress config");

    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");
    let binary = std::env::var_os("CODEDIFF_SCROLL_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_codediff")));
    let mut command = CommandBuilder::new(binary);
    command.args(["--config", config.to_str().expect("config path")]);
    command.cwd(&repo);
    command.env("TERM", "xterm-256color");
    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    let pid = child.process_id().expect("a process id");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);
    drawn(&output);
    let stop_sampling = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(Mutex::new(Peak::default()));
    let sampler_stop = Arc::clone(&stop_sampling);
    let sampler_peak = Arc::clone(&peak);
    let sampler = thread::spawn(move || {
        while !sampler_stop.load(Ordering::Relaxed) {
            if let Some((rss_kb, cpu_percent)) = sample_process(pid) {
                let mut peak = sampler_peak.lock().expect("peak lock");
                peak.rss_kb = peak.rss_kb.max(rss_kb);
                peak.cpu_percent = peak.cpu_percent.max(cpu_percent);
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    let before_open = output.lock().expect("output lock").bytes.len();
    writer
        .write_all(b"\x1b[<0;10;5M\x1b[<0;10;5m\x1b[<0;70;2M\x1b[<0;70;2m\x1b[C")
        .expect("opening the changed file and focusing the diff");
    writer.flush().expect("flushing the file click");
    drawn_after(&output, before_open);

    let started = Instant::now();
    for _ in 0..20 {
        writer
            .write_all(b"jjjjjjjjjjkkkkkkkkkkllllllllllhhhhhhhhhh")
            .expect("sending repeated scroll keys");
    }
    writer
        .write_all(b"\x1b[<65;70;12M\x1b[<65;70;12M\x1b[<67;70;12M\x1b[<67;70;12M")
        .expect("sending repeated wheel input");
    writer.flush().expect("flushing repeated scroll input");
    let before_resize = output.lock().expect("output lock").bytes.len();
    pty.master
        .resize(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resizing the pty larger");
    drawn_after(&output, before_resize);

    let before_layout = output.lock().expect("output lock").bytes.len();
    writer
        .write_all(b"tw$t0w")
        .expect("changing view layout and wrap");
    writer.flush().expect("flushing layout changes");
    pty.master
        .resize(PtySize {
            rows: 18,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resizing the pty smaller");
    drawn_after(&output, before_layout);

    let elapsed_ms = started.elapsed().as_millis() as u64;
    stop_sampling.store(true, Ordering::Relaxed);
    sampler.join().expect("joining process sampler");
    let peak = peak.lock().expect("peak lock");
    let peak_rss_kb = peak.rss_kb;
    let peak_cpu_percent = peak.cpu_percent;
    let metrics = format!(
        "{{\"elapsed_ms\":{elapsed_ms},\"peak_rss_kb\":{peak_rss_kb},\"peak_cpu_percent\":{peak_cpu_percent:.3}}}"
    );
    eprintln!("scroll stress: {metrics}");
    if let Some(path) = std::env::var_os("CODEDIFF_SCROLL_METRICS") {
        fs::write(path, &metrics).expect("writing scroll stress metrics");
    }

    writer.write_all(b"q").expect("sending quit");
    writer.flush().expect("flushing quit");
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(Instant::now() < deadline, "scroll stress process hung");
        thread::sleep(Duration::from_millis(25));
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
    if let Some(limit) = env_u64("CODEDIFF_SCROLL_MAX_RSS_KB") {
        assert!(
            peak_rss_kb <= limit,
            "peak RSS {} KB exceeded {} KB",
            peak_rss_kb,
            limit
        );
    }
    if let Some(limit) = env_u64("CODEDIFF_SCROLL_MAX_ELAPSED_MS") {
        assert!(
            elapsed_ms <= limit,
            "scroll workload took {} ms, limit is {} ms",
            elapsed_ms,
            limit
        );
    }
}

fn sample_process(pid: u32) -> Option<(u64, f64)> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "rss=", "-o", "%cpu="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fields = text.split_whitespace();
    Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}
