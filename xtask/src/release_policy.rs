#[test]
fn zero_major_releases_require_an_explicit_one_x_transition() {
    let config: toml::Table = include_str!("../../cliff.toml").parse().unwrap();
    let bump = config
        .get("bump")
        .and_then(toml::Value::as_table)
        .expect("cliff.toml must define [bump]");

    assert_eq!(
        bump.get("features_always_bump_minor")
            .and_then(toml::Value::as_bool),
        Some(true),
        "0.x features must increment the minor version"
    );
    assert_eq!(
        bump.get("breaking_always_bump_major")
            .and_then(toml::Value::as_bool),
        Some(false),
        "0.x breaking changes must not cross into 1.x implicitly"
    );
}

#[test]
fn c_engine_version_tracks_the_workspace() {
    let manifest: toml::Table = include_str!("../../Cargo.toml").parse().unwrap();
    let workspace_version = manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("package"))
        .and_then(toml::Value::as_table)
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .expect("Cargo.toml must define workspace.package.version");

    assert_eq!(
        include_str!("../../libvscode-diff/VERSION").trim(),
        workspace_version
    );
}
