use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

pub fn render(root: &Path, workspace: &Path, results: &Path) -> Result<()> {
    let web = root.join("xtask/src/verify_vscode/web");
    let output = Command::new("pnpm")
        .current_dir(&web)
        .args(["install", "--frozen-lockfile", "--reporter=append-only"])
        .output()
        .context("installing VS Code Web test dependencies with pnpm")?;
    if !output.status.success() {
        bail!("pnpm install failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let output = Command::new("pnpm")
        .current_dir(&web)
        .args(["run", "verify"])
        .arg(workspace)
        .arg(results)
        .arg(root.join("target/vscode-web"))
        .output()
        .context("running VS Code Web highlight oracle")?;
    if !output.status.success() {
        bail!("VS Code Web highlight oracle failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}
