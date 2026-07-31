//! `codediff` — a standalone, read-only terminal diff reviewer.
//!
//! This crate is the composition root: it parses arguments, loads
//! configuration, constructs concrete backends and wires them together. It is
//! the only place in the workspace that names concrete implementations, and
//! nothing depends on it.

mod debug;
mod doctor;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("doctor") => {
            doctor::run();
            Ok(())
        }
        Some("debug") => match args.get(1).map(String::as_str) {
            Some("diff") => match (args.get(2), args.get(3)) {
                (Some(original), Some(modified)) => debug::run(original, modified),
                _ => bail!("usage: codediff debug diff <original> <modified>"),
            },
            Some(other) => bail!("unknown debug command: {other}"),
            None => bail!("usage: codediff debug diff <original> <modified>"),
        },
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
    codediff debug diff <old> <new>          print the raw diff of two files
    codediff --version                       print the version
    codediff --help                          print this message

The review interface is not built yet; see docs/plan/04-milestones.md.",
        version = env!("CARGO_PKG_VERSION")
    );
}
