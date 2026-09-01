//! Applies the pinned VS Code parity patches to the vendored C sources,
//! compiles them into a static archive, and links it into this crate.
//!
//! OpenMP is disabled. Measured reasons:
//! with upstream's libgomp packaging troubles — those were a *runtime dynamic
//! linking* failure of a prebuilt `.so`, which does not apply to a build from
//! source.
//!
//! Measured on this engine (aarch64, gcc 15.2, best of several runs):
//!
//! | input                        | sequential | OpenMP  |
//! |------------------------------|------------|---------|
//! | 500 lines, 20 changed        | 5.00 ms    | 4.93 ms |
//! | 2,000 lines, 50 changed      | 2.59 ms    | 2.43 ms |
//! | 20,000 lines, 2,858 changed  | 264 ms     | 208 ms  |
//!
//! On realistic files the difference is noise. On a deliberately pathological
//! file it is ~21% wall clock at 1.31x parallelism — Amdahl's law, since only
//! character-level refinement is parallel while Myers, line-level optimisation
//! and move detection stay sequential — and it costs ~5% more total CPU.
//!
//! Against that: `-fopenmp` re-appends `-lgomp` at link time, so a static
//! OpenMP build means hand-rolling link lines; Apple clang ships no OpenMP at
//! all, so macOS would require `brew install libomp` for every contributor and
//! CI runner; and MSVC's runtime is the `vcomp140.dll` redistributable with no
//! static option.
//!
//! The decisive argument is placement, not portability: diffs are computed
//! concurrently *across files*, which scales with core count and beats 1.31x
//! inside a single file. Enabling both would oversubscribe, with each worker
//! spawning its own uncoordinated OpenMP team.

use std::path::{Path, PathBuf};

/// Mirrors DIFF_CORE_SOURCES in vendor/libvscode-diff/CMakeLists.txt.
///
/// `vendor/utf8proc_data.c` is absent on purpose: `utf8proc.c` `#include`s it,
/// so compiling it separately would define every symbol twice.
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

const PATCHES: &[(&str, &str)] = &[
    ("src/char_level.c", "char-level-text.patch"),
    ("src/myers.c", "myers-typed-array.patch"),
];

fn main() {
    let vendor = workspace_root().join("vendor");
    let engine = vendor.join("libvscode-diff");

    if !engine.is_dir() {
        panic!(
            "vendor/libvscode-diff is missing.\nRun: cargo xtask sync-c --tag <tag>\n\
             (expected at {})",
            engine.display()
        );
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let generated = out_dir.join("include");
    std::fs::create_dir_all(&generated).expect("creating generated include dir");
    write_version_header(&vendor, &engine, &generated);

    let mut build = cc::Build::new();
    build
        .include(engine.join("include"))
        .include(engine.join("vendor"))
        .include(&generated)
        // Bundled utf8proc, so no dllimport/dllexport decoration.
        .define("UTF8PROC_STATIC", None)
        .warnings(false);

    // On Windows the public header decorates its exports with
    // `__declspec(dllimport)` unless BUILDING_DLL is set, because upstream only
    // ever ships a DLL there. We link statically, and dllimport would make the
    // linker look for symbols in a DLL that does not exist. Defining
    // BUILDING_DLL selects dllexport instead, which is inert when the object
    // ends up in an executable.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        build.define("BUILDING_DLL", None);
    }

    let patches = Path::new(env!("CARGO_MANIFEST_DIR")).join("patches");
    for source in SOURCES {
        build.file(patched_source(&engine, &patches, &out_dir, source));
    }

    build.compile("vscode_diff");

    println!("cargo:rerun-if-changed={}", engine.display());
    println!("cargo:rerun-if-changed={}", patches.display());
    println!(
        "cargo:rerun-if-changed={}",
        vendor.join("VERSION").display()
    );
}

fn patched_source(engine: &Path, patches: &Path, out_dir: &Path, source: &str) -> PathBuf {
    let Some((_, patch_name)) = PATCHES.iter().find(|(path, _)| *path == source) else {
        return engine.join(source);
    };
    let base = std::fs::read_to_string(engine.join(source)).expect("reading C source to patch");
    let text = std::fs::read_to_string(patches.join(patch_name)).expect("reading C parity patch");
    let patch = diffy::Patch::from_str(&text).expect("parsing C parity patch");
    let patched = diffy::apply(&base, &patch).expect("C parity patch no longer applies");
    let destination = out_dir.join(source.replace('/', "-"));
    std::fs::write(&destination, patched).expect("writing patched C source");
    destination
}

/// CMake generates `version.h` from `version.h.in`, substituting the version
/// read from the repository's VERSION file. `sync-c` copies that file to
/// `vendor/VERSION`; this reproduces the substitution.
fn write_version_header(vendor: &Path, engine: &Path, generated: &Path) {
    let raw = std::fs::read_to_string(vendor.join("VERSION"))
        .expect("vendor/VERSION is written by `cargo xtask sync-c`");
    let full = raw.trim();

    // CMake reduces the version to MAJOR.MINOR.PATCH before substituting.
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
