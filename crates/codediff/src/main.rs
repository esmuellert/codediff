//! `codediff` — a standalone, read-only terminal diff reviewer.
//!
//! This crate is the composition root: it parses arguments, loads
//! configuration, constructs concrete backends and wires them together. It is
//! the only place in the workspace that names concrete implementations, and
//! nothing depends on it.

mod align;
mod debug;
mod doctor;
mod measure;
mod text;

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
            Some("measure") => match args.get(2) {
                Some(path) => {
                    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
                    measure::run(path, verbose)
                }
                None => bail!("usage: codediff debug measure <file> [--verbose]"),
            },
            Some("align") => match (args.get(2), args.get(3)) {
                (Some(original), Some(modified)) => {
                    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
                    align::run(original, modified, verbose)
                }
                _ => bail!("usage: codediff debug align <original> <modified> [--verbose]"),
            },
            Some(other) => bail!("unknown debug command: {other}"),
            None => bail!("usage: codediff debug <diff|align|measure> ..."),
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
    codediff debug align <old> <new> [-v]    print the two files paired up
    codediff debug measure <file> [-v]       print where each character lives
    codediff --version                       print the version
    codediff --help                          print this message

The review interface is not built yet; see docs/plan/04-milestones.md.",
        version = env!("CARGO_PKG_VERSION")
    );
}
