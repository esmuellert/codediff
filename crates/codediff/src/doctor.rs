//! Reports build information, engine linkage, and terminal detection.

pub fn run() {
    println!("codediff {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("build");
    // Reading the version verifies that the FFI is callable.
    println!(
        "  diff engine   libvscode-diff {} (static, call succeeded)",
        vscode_diff::engine_version()
    );
    // The bundled engine is built without OpenMP.
    println!("  openmp        disabled, no libgomp dependency");
    println!("  target        {}", env!("CODEDIFF_TARGET"));
    println!("  profile       {}", env!("CODEDIFF_PROFILE"));
    println!("  rustc         {}", env!("CODEDIFF_RUSTC"));
    println!();
    terminal();
}

/// Reports terminal variables used for theme detection.
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
