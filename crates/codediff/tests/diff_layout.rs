//! Layout selection at the shipped rendering-record boundary.

use std::path::PathBuf;
use std::process::Command;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "codediff-diff-layout-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("creating parity fixture");
        std::fs::write(dir.join("original.txt"), "one\ntwo\n").expect("writing original");
        std::fs::write(dir.join("modified.txt"), "one\nTWO\n").expect("writing modified");
        Self { dir }
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_codediff"))
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running codediff parity")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn explicit_side_by_side_is_the_unchanged_default() {
    let fixture = Fixture::new("default");
    let default = fixture.run(&["debug", "parity", "original.txt", "modified.txt"]);
    let explicit = fixture.run(&[
        "debug",
        "parity",
        "--layout",
        "side-by-side",
        "original.txt",
        "modified.txt",
    ]);

    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    assert!(
        explicit.status.success(),
        "{}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    assert_eq!(explicit.stdout, default.stdout);
}

#[test]
fn inline_emits_unified_rendering_records() {
    let fixture = Fixture::new("inline");
    let output = fixture.run(&[
        "debug",
        "parity",
        "--layout",
        "inline",
        "original.txt",
        "modified.txt",
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records: Vec<serde_json::Value> = std::str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        records,
        [
            serde_json::json!({"type":"row","index":0,"original":1,"modified":1}),
            serde_json::json!({"type":"row","index":1,"original":2,"modified":null}),
            serde_json::json!({"type":"row","index":2,"original":null,"modified":2}),
            serde_json::json!({"type":"row","index":3,"original":3,"modified":3}),
            serde_json::json!({
                "type":"highlight","side":"original","line":2,
                "line_background":"delete","gutter_background":"delete",
                "characters":[{"start":0,"end":null,"fill_to_edge":true}],
                "empty_markers":[]
            }),
            serde_json::json!({
                "type":"highlight","side":"modified","line":2,
                "line_background":"insert","gutter_background":"insert",
                "characters":[{"start":0,"end":null,"fill_to_edge":true}],
                "empty_markers":[]
            }),
        ]
    );
}

#[test]
fn an_unknown_layout_is_command_line_misuse() {
    let fixture = Fixture::new("unknown");
    let output = fixture.run(&[
        "debug",
        "parity",
        "--layout",
        "stacked",
        "original.txt",
        "modified.txt",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value 'stacked'"));
}
