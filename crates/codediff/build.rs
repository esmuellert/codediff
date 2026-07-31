//! Captures build-time facts that `codediff doctor` reports.
//!
//! These are not available to the compiled program otherwise: `TARGET` and
//! `PROFILE` exist only while the build script runs.

use std::process::Command;

fn main() {
    emit(
        "CODEDIFF_TARGET",
        std::env::var("TARGET").unwrap_or_default(),
    );
    emit(
        "CODEDIFF_PROFILE",
        std::env::var("PROFILE").unwrap_or_default(),
    );
    emit("CODEDIFF_RUSTC", rustc_version());

    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");
}

fn emit(key: &str, value: String) {
    let value = if value.is_empty() {
        "unknown".to_owned()
    } else {
        value
    };
    println!("cargo:rustc-env={key}={value}");
}

fn rustc_version() -> String {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}
