//! `codediff` — a standalone, read-only terminal diff reviewer.
//!
//! This crate is the composition root: it parses arguments, loads
//! configuration, constructs concrete backends and wires them together. It is
//! the only place in the workspace that names concrete implementations, and
//! nothing depends on it.

mod debug;
mod doctor;
mod text;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("doctor") => {
            doctor::run();
            Ok(())
        }
        Some("debug") => debug::run(&args[1..]),
        Some("--version") | Some("-V") => {
            println!("codediff {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help") | Some("-h") | None => {
            help();
            Ok(())
        }
        Some(other) => {
            help();
            bail!("unknown command: {other}");
        }
    }
}

fn help() {
    println!(
        "\
codediff {version} — a standalone, read-only terminal diff reviewer

USAGE:
    codediff doctor                          report how this binary was built
    codediff debug <command>                 inspect one layer; run bare to list
    codediff --version                       print the version
    codediff --help                          print this message

The review interface is not built yet; see docs/plan/04-milestones.md.",
        version = env!("CARGO_PKG_VERSION")
    );
}
