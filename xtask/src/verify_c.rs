//! `cargo xtask verify-c`
//!
//! Fails if the vendored C no longer matches the hash recorded in
//! `UPSTREAM.lock`. Runs offline, so it is safe in CI.
//!
//! This is what makes "copy the C rather than submodule it" safe: without it,
//! a local patch to `vendor/` would work, be forgotten, and become a silent
//! fork of upstream.

use anyhow::{Result, bail};

use crate::lock::{self, Lock};
use crate::workspace_root;

pub fn run() -> Result<()> {
    let root = workspace_root();
    lock::require_vendored(&root)?;

    let lock_path = lock::vendor_dir(&root).join(lock::LOCK_NAME);
    let recorded = Lock::read(&lock_path)?;
    let actual = lock::hash_tree(&lock::engine_dir(&root))?;

    if actual != recorded.tree_sha256 {
        bail!(
            "vendor/libvscode-diff has been modified locally.\n\
             \n\
             recorded {}\n\
             actual   {}\n\
             \n\
             The vendored C must match {} {} exactly.\n\
             Patch upstream and re-run: cargo xtask sync-c --tag <tag>",
            recorded.tree_sha256,
            actual,
            recorded.repository,
            recorded.tag,
        );
    }

    println!(
        "vendor/libvscode-diff matches {} {} ({})",
        recorded.repository, recorded.tag, recorded.version
    );
    Ok(())
}
