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
