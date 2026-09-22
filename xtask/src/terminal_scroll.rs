use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct Metrics {
    elapsed_ms: u64,
    peak_rss_kb: u64,
    peak_cpu_percent: f64,
}

pub fn run(args: &[String]) -> Result<()> {
    let baseline = fs::canonicalize(argument(args, "--baseline")?)
        .context("resolving baseline binary path")?;
    let candidate = fs::canonicalize(argument(args, "--candidate")?)
        .context("resolving candidate binary path")?;
    let max_ratio = argument(args, "--max-ratio")?
        .parse::<f64>()
        .with_context(|| "--max-ratio must be a positive number, for example 1.25")?;
    if !max_ratio.is_finite() || max_ratio <= 0.0 {
        bail!("--max-ratio must be a positive number");
    }

    let output_dir = argument(args, "--output")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().join("target/terminal-scroll"));
    fs::create_dir_all(&output_dir).context("creating terminal scroll output directory")?;

    let baseline_metrics_path = output_dir.join("baseline.json");
    let candidate_metrics_path = output_dir.join("candidate.json");
    run_case(&baseline, &baseline_metrics_path)?;
    run_case(&candidate, &candidate_metrics_path)?;

    let baseline_metrics: Metrics = read_metrics(&baseline_metrics_path)?;
    let candidate_metrics: Metrics = read_metrics(&candidate_metrics_path)?;
    println!(
        "baseline: elapsed={}ms rss={}KB cpu={:.1}%",
        baseline_metrics.elapsed_ms,
        baseline_metrics.peak_rss_kb,
        baseline_metrics.peak_cpu_percent
    );
    println!(
        "candidate: elapsed={}ms rss={}KB cpu={:.1}%",
        candidate_metrics.elapsed_ms,
        candidate_metrics.peak_rss_kb,
        candidate_metrics.peak_cpu_percent
    );

    compare(
        "elapsed_ms",
        baseline_metrics.elapsed_ms as f64,
        candidate_metrics.elapsed_ms as f64,
        max_ratio,
    )?;
    compare(
        "peak_rss_kb",
        baseline_metrics.peak_rss_kb as f64,
        candidate_metrics.peak_rss_kb as f64,
        max_ratio,
    )?;
    compare(
        "peak_cpu_percent",
        baseline_metrics.peak_cpu_percent,
        candidate_metrics.peak_cpu_percent,
        max_ratio,
    )?;
    Ok(())
}

fn run_case(binary: &Path, metrics: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "test",
            "-j",
            "4",
            "-p",
            "codediff",
            "--test",
            "terminal",
            "scroll::stress",
            "--",
            "--nocapture",
        ])
        .env("CODEDIFF_SCROLL_BINARY", binary)
        .env("CODEDIFF_SCROLL_METRICS", metrics)
        .status()
        .with_context(|| format!("running scroll stress for {}", binary.display()))?;
    if !status.success() {
        bail!("scroll stress failed for {}", binary.display());
    }
    Ok(())
}

fn read_metrics(path: &Path) -> Result<Metrics> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("reading scroll metrics {}", path.display()))?;
    serde_json::from_str(&data)
        .with_context(|| format!("parsing scroll metrics {}", path.display()))
}

fn compare(name: &str, baseline: f64, candidate: f64, max_ratio: f64) -> Result<()> {
    if baseline == 0.0 {
        return Ok(());
    }
    let ratio = candidate / baseline;
    if ratio > max_ratio {
        bail!(
            "scroll {name} regressed: candidate={candidate:.2}, baseline={baseline:.2}, ratio={ratio:.2} > {max_ratio:.2}"
        );
    }
    println!("{name}: ratio={ratio:.2}");
    Ok(())
}

fn argument(args: &[String], name: &str) -> Result<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .with_context(|| format!("missing {name}"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives below the workspace root")
        .to_owned()
}
