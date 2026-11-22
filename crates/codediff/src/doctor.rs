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

/// Reports terminal capabilities and variables used for theme detection.
fn terminal() {
    let probed = ui::probe();
    let env_lookup = |key: &str| std::env::var(key).ok();
    let theme = ui::Theme::resolve(probed, env_lookup);
    let show = |key: &str| std::env::var(key).unwrap_or_else(|_| "unset".to_owned());
    let background = match probed.background {
        Some(bg) => {
            let tone = if bg.is_dark() { "dark" } else { "light" };
            format!("rgb({}, {}, {}) ({tone})", bg.0, bg.1, bg.2)
        }
        None => "unknown".to_owned(),
    };
    let truecolor = match probed.truecolor {
        Some(true) => "yes (XTGETTCAP)",
        Some(false) => "no (XTGETTCAP)",
        None if probed.background.is_some() => "inferred (OSC 11)",
        None => "unprobed",
    };
    let graph_symbols = match (
        probed.branch_drawing,
        probed.supports_branch_drawing(env_lookup),
    ) {
        (Some(true), _) => "branch-drawing (XTVERSION)",
        (Some(false), _) => "box-drawing (XTVERSION)",
        (None, true) => "branch-drawing (env)",
        (None, false) => "box-drawing",
    };

    println!("terminal");
    println!("  TERM          {}", show("TERM"));
    println!("  COLORTERM     {}", show("COLORTERM"));
    println!("  COLORFGBG     {}", show("COLORFGBG"));
    println!("  background    {background}");
    println!("  truecolor     {truecolor}");
    println!("  graph symbols {graph_symbols}");
    println!("  theme         {} (default)", theme.name);
}
