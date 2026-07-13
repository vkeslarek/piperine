//! Native-backend smoke test (the Phase-1 gate of `Plugin plan.md`): build
//! the fixture cdylib, point a project's `[plugins]` at it via a manifest,
//! and load it through the full path — resolve → manifest → hash → TOFU →
//! dlopen → register.

use std::path::PathBuf;

use piperine_plugin::{PluginError, PluginHost, TrustMode};

/// Build the fixture example cdylib and return its path. `cargo test` does
/// not build example targets, so build it explicitly.
fn fixture_cdylib() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.parent().unwrap().parent().unwrap();
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "-p", "piperine-plugin", "--example", "fixture_plugin"])
        .current_dir(workspace)
        .status()
        .expect("cargo build fixture example");
    assert!(status.success(), "fixture build failed");
    let lib = format!(
        "{}fixture_plugin{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    workspace.join("target").join("debug").join("examples").join(lib)
}

/// A throwaway project whose `[plugins]` names the fixture by path.
fn project_with_fixture(dir: &std::path::Path, artifact: &std::path::Path) {
    let plugin_dir = dir.join("fixture-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let entry = artifact.file_name().unwrap().to_str().unwrap();
    std::fs::copy(artifact, plugin_dir.join(entry)).unwrap();
    std::fs::write(
        plugin_dir.join("piperine-plugin.toml"),
        format!("[plugin]\nname = \"fixture\"\nabi = \"native\"\nentry = \"{entry}\"\n"),
    )
    .unwrap();
    std::fs::write(
        dir.join("Piperine.toml"),
        "[project]\nname = \"smoke\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
         [plugins.fixture]\npath = \"fixture-plugin\"\n",
    )
    .unwrap();
}

#[test]
fn dlopen_load_register_and_trust_flow() {
    let artifact = fixture_cdylib();
    let dir = tempfile::tempdir().unwrap();
    project_with_fixture(dir.path(), &artifact);

    // Untrusted + reject mode → P0001, before any plugin code runs.
    let err = PluginHost::load_for_project(dir.path(), TrustMode::RejectUntrusted)
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(err, PluginError::Untrusted(_)), "{err}");

    // Accept: dlopen, ABI check, register() → contributions visible.
    let host = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll).expect("load");
    assert_eq!(host.plugin_names(), vec!["fixture"]);

    // Second load re-uses the recorded trust (strictest mode passes).
    let host2 = PluginHost::load_for_project(dir.path(), TrustMode::RejectUntrusted).expect("reload");
    assert!(!host2.is_empty());

    // Tampering with the artifact bytes flips the hash → P0007. The original
    // file is still mapped by the live dlopen, so replace it via a *new*
    // inode (remove + write) — truncating a mapped ELF in place corrupts the
    // running process.
    let entry = dir.path().join("fixture-plugin").join(artifact.file_name().unwrap());
    let mut bytes = std::fs::read(&entry).unwrap();
    bytes.push(0);
    std::fs::remove_file(&entry).unwrap();
    std::fs::write(&entry, bytes).unwrap();
    let err = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll)
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(err, PluginError::HashMismatch { .. }), "{err}");
}

#[test]
fn project_without_plugins_is_inert() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Piperine.toml"),
        "[project]\nname = \"empty\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n",
    )
    .unwrap();
    let host = PluginHost::load_for_project(dir.path(), TrustMode::RejectUntrusted).expect("inert");
    assert!(host.is_empty());
}
