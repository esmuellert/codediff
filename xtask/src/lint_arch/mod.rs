//! `cargo xtask lint-arch`
//!
//! Cargo enforces exactly one architectural rule for free: crate dependencies
//! must be acyclic. Every other rule in docs/plan is project-specific and has
//! to be encoded somewhere. This is that somewhere.
//!
//! Split three ways so the tables can be read without the machinery:
//!
//! | | |
//! |---|---|
//! | [`rules`] | what is forbidden, and why |
//! | [`checks`] | one function per rule |
//! | [`files`] | finding crates and reading manifests |

mod checks;
mod files;
mod rules;

use anyhow::{Result, bail};

use checks::{
    check_banned_names, check_blind_dirs, check_clock_free, check_edges, check_engine_confinement,
    check_inherited_metadata, check_non_blocking, check_purity, check_threads, check_type_names,
    check_unsafe_policy, pending_names,
};

pub fn run() -> Result<()> {
    let root = crate::workspace_root();
    let mut failures = Vec::new();

    let (applied, pending) = check_edges(&root, &mut failures)?;
    check_purity(&root, &mut failures)?;
    check_clock_free(&root, &mut failures)?;
    check_unsafe_policy(&root, &mut failures)?;
    check_inherited_metadata(&root, &mut failures)?;
    check_engine_confinement(&root, &mut failures)?;
    check_type_names(&root, &mut failures)?;
    check_blind_dirs(&root, &mut failures)?;
    check_threads(&root, &mut failures)?;
    check_non_blocking(&root, &mut failures)?;
    check_banned_names(&root, &mut failures)?;

    if !failures.is_empty() {
        let mut msg = format!("{} architecture violation(s):\n", failures.len());
        for f in &failures {
            msg.push_str(&format!("  {f}\n"));
        }
        bail!(msg);
    }

    println!(
        "lint-arch: purity, clocks, threads, blocking, unsafe policy, engine\n            confinement, names, module boundaries and inherited metadata clean"
    );
    println!("  edge rules: {applied} applied, {pending} awaiting their crate");
    if !pending_names(&root)?.is_empty() {
        // Named so that a rule cannot quietly stay dead because of a typo in
        // FORBIDDEN_EDGES.
        println!("  pending:    {}", pending_names(&root)?.join(", "));
    }
    Ok(())
}
