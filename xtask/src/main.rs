//! Repository automation.
//!
//! Rust rather than shell so that it is cross-platform without duplication,
//! type-checked, and able to use the workspace crates. This is not a build
//! system: `cargo build` and `build.rs` compile everything, including the C
//! engine. These are the chores cargo has no opinion about.
//!
//! The lint tasks turn the rules in docs/plan into build failures.

#[cfg(test)]
mod attribution;
mod dev;
mod lint_arch;
mod lint_size;
mod oracle_output;
#[cfg(test)]
mod release_policy;
mod verify_oracle;
mod verify_vscode;

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str);

    match task {
        Some("verify-oracle") => verify_oracle::run(),
        Some("verify-vscode") => verify_vscode::run(&args[1..]),
        Some("lint-size") => lint_size::run(),
        Some("lint-arch") => lint_arch::run(),
        Some("dev") => dev::run(&args[1..]),
        Some("fixture-repo") => match args.get(1) {
            Some(dir) => fixtures::repo(std::path::Path::new(dir)).map_err(Into::into),
            None => anyhow::bail!("usage: cargo xtask fixture-repo <dir>"),
        },
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

C engine
  verify-oracle                        compare our binding against the C diff tool
  verify-vscode [repo] [--files N --versions N --max-lines N]
                                       compare VS Code Web highlighting on Git history

Architecture enforcement
  lint-size                            fail if a file exceeds the line cap
  lint-arch                            fail on forbidden crate edges and unsafe policy

Fixtures
  fixture-repo <dir>                   build a git repository in a known state

Development
  dev [dir] [args...]                  run codediff, rebuilding it on F5
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
