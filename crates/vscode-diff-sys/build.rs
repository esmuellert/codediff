//! Builds the canonical C diff engine as a static library.

use std::path::{Path, PathBuf};

/// Mirrors `DIFF_CORE_SOURCES`; `utf8proc_data.c` is included by `utf8proc.c`.
const SOURCES: &[&str] = &[
    "default_lines_diff_computer.c",
    "src/char_level.c",
    "src/line_level.c",
    "src/myers.c",
    "src/optimize.c",
    "src/sequence.c",
    "src/range_mapping.c",
    "src/string_hash_map.c",
    "src/utils.c",
    "src/print_utils.c",
    "src/utf8_utils.c",
    "src/compute_moved_lines.c",
    "vendor/utf8proc.c",
];

fn main() {
    let engine = workspace_root().join("libvscode-diff");
    if !engine.is_dir() {
        panic!(
            "libvscode-diff is missing (expected at {})",
            engine.display()
        );
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let generated = out_dir.join("include");
    std::fs::create_dir_all(&generated).expect("creating generated include dir");
    write_version_header(&engine, &generated);

    let mut build = cc::Build::new();
    build
        .include(engine.join("include"))
        .include(engine.join("vendor"))
        .include(&generated)
        .define("UTF8PROC_STATIC", None)
        .warnings(false);

    // Avoid `dllimport` declarations while linking the engine statically.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        build.define("BUILDING_DLL", None);
    }

    for source in SOURCES {
        build.file(engine.join(source));
    }
    build.compile("vscode_diff");

    println!("cargo:rerun-if-changed={}", engine.display());
}

/// Generates the version header normally written by CMake.
fn write_version_header(engine: &Path, generated: &Path) {
    let raw = std::fs::read_to_string(engine.join("VERSION"))
        .expect("libvscode-diff/VERSION must be present");
    let full = raw.trim();

    let base: String = full
        .split('.')
        .take(3)
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(".");

    let template =
        std::fs::read_to_string(engine.join("include/version.h.in")).expect("reading version.h.in");
    let rendered = template.replace("@PROJECT_VERSION@", &base);
    std::fs::write(generated.join("version.h"), rendered).expect("writing version.h");
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/<name>/ lives two levels below the workspace root")
        .to_path_buf()
}
