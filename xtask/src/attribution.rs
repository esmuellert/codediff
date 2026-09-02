use std::path::Path;

#[test]
fn c_engine_bundles_only_attributed_sources() {
    let bundled = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("libvscode-diff")
        .join("vendor");
    let mut unexpected: Vec<_> = std::fs::read_dir(bundled)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with("utf8proc") && name != "README.md")
        .collect();
    unexpected.sort();

    assert!(
        unexpected.is_empty(),
        "add bundled dependencies to ATTRIBUTION.md: {unexpected:?}"
    );
}
