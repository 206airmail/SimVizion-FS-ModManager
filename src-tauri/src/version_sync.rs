//! The app version is written in three places (plan M8): the workspace `Cargo.toml` (which this
//! crate inherits), `tauri.conf.json` (the installer and the updater compare it) and
//! `package.json`. A release with them out of step would report one version and install another.

#[test]
fn every_manifest_carries_the_same_version() {
    let cargo = env!("CARGO_PKG_VERSION");
    let field = |json: &str, name: &str| -> String {
        let value: serde_json::Value = serde_json::from_str(json).expect(name);
        value["version"].as_str().expect(name).to_string()
    };
    let tauri = field(include_str!("../tauri.conf.json"), "tauri.conf.json");
    let npm = field(include_str!("../../package.json"), "package.json");
    assert_eq!(tauri, cargo, "tauri.conf.json vs Cargo.toml");
    assert_eq!(npm, cargo, "package.json vs Cargo.toml");
}
