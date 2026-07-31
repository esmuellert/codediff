//! Repository automation.
//!
//! Rust rather than shell so that it is cross-platform without duplication,
//! type-checked, and able to use the workspace crates. This is not a build
//! system: `cargo build` and `build.rs` compile everything, including the
//! vendored C. These are the chores cargo has no opinion about.
//!
//! Three of these tasks — `verify-c`, `lint-size` and `lint-arch` — exist to
//! turn the rules in docs/plan into build failures.

mod lint_arch;
mod lint_size;
mod lock;
mod oracle_output;
mod sync_c;
mod verify_c;
mod verify_oracle;

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str);

    match task {
        Some("sync-c") => sync_c::run(&args[1..]),
        Some("verify-c") => verify_c::run(),
        Some("verify-oracle") => verify_oracle::run(),
        Some("lint-size") => lint_size::run(),
        Some("lint-arch") => lint_arch::run(),
        Some("help") | Some("--help") | Some("-h") | None => {
            help();
            Ok(())
        }
        Some(other) => {
            help();
            bail!("unknown task: {other}");
        }
    }
}

fn help() {
    eprintln!(
        "\
cargo xtask <task>

Vendored C engine
  sync-c --tag <tag> [--from <path>]   refresh vendor/ from an upstream tag
  verify-c                             fail if vendor/ drifted from UPSTREAM.lock
  verify-oracle                        compare our binding against upstream diff_tool

Architecture enforcement
  lint-size                            fail if a file exceeds the line cap
  lint-arch                            fail on forbidden crate edges and unsafe policy
"
    );
}

/// The workspace root, resolved from this crate's manifest directory.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}
