//! `codediff doctor` — what this binary is, and what it found.
//!
//! Two jobs. It is how S1 proves the FFI works from the shipped binary rather
//! than only from a test: printing the engine version requires a successful
//! call through the C ABI. And it is the thing to ask for in a bug report, so
//! that "which build, which engine, which compiler" never costs a round trip.
//!
//! Environment checks arrive with the subsystems they test — the watcher at
//! S15, configuration at S17.

pub fn run() {
    println!("codediff {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("build");
    // The version is obtained by calling through the FFI, so printing one is
    // also evidence that the engine is linked and callable.
    println!(
        "  diff engine   libvscode-diff {} (static, call succeeded)",
        vscode_diff::engine_version()
    );
    // Compiled from source with OpenMP off, so there is no libgomp to locate —
    // the dependency that broke real users of the upstream Neovim plugin
    // (codediff.nvim issues #48 and #58).
    println!("  openmp        disabled, no libgomp dependency");
    println!("  target        {}", env!("CODEDIFF_TARGET"));
    println!("  profile       {}", env!("CODEDIFF_PROFILE"));
    println!("  rustc         {}", env!("CODEDIFF_RUSTC"));
    println!();
    terminal();
}

/// What the interface would look like if it were opened right now.
///
/// The first question a "my colours are wrong" report has to answer, and one
/// nobody can answer by looking: whether the terminal advertised 24-bit colour
/// decides which theme is chosen, and the variable that says so is easy to
/// lose across `sudo`, `ssh` and `tmux`.
fn terminal() {
    let show = |key: &str| std::env::var(key).unwrap_or_else(|_| "unset".to_owned());
    println!("terminal");
    println!("  TERM          {}", show("TERM"));
    println!("  COLORTERM     {}", show("COLORTERM"));
    println!("  COLORFGBG     {}", show("COLORFGBG"));
    println!(
        "  theme         {} (default)",
        ui::Theme::from_environment().name
    );
}
