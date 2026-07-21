//! Plugin extern-stub auto-import (declared-language-surface T24, DLS-22
//! groundwork): a loaded plugin's published `extern.phdl` stub is parsed
//! into the project's `ElabContext` automatically at `seed_schemas` time —
//! no explicit `use` required, mirroring `headers/spice/`'s availability.
//! Reuses the same on-disk-plugin harness as `native_smoke.rs` (build the
//! fixture cdylib, point a throwaway project's `[plugins]` at it), since
//! the auto-import mechanism only exists for `load_for_project`'s real
//! filesystem plugin path — `from_plugins` (in-process/test plugins) has
//! no directory an `extern.phdl` stub could live in.

use std::path::PathBuf;

use piperine_lang::SourceMap;
use piperine_plugin::{PluginHost, TrustMode};

/// Build the fixture example cdylib and return its path (same helper as
/// `native_smoke.rs` — `cargo test` does not build example targets).
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

/// A throwaway project whose `[plugins]` names the fixture by path, with an
/// `extern.phdl` stub published alongside the manifest when `stub` is
/// `Some`.
fn project_with_fixture(dir: &std::path::Path, artifact: &std::path::Path, stub: Option<&str>) {
    let plugin_dir = dir.join("fixture-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let entry = artifact.file_name().unwrap().to_str().unwrap();
    std::fs::copy(artifact, plugin_dir.join(entry)).unwrap();
    std::fs::write(
        plugin_dir.join("piperine-plugin.toml"),
        format!("[plugin]\nname = \"fixture\"\nabi = \"native\"\nentry = \"{entry}\"\n"),
    )
    .unwrap();
    if let Some(text) = stub {
        std::fs::write(plugin_dir.join("extern.phdl"), text).unwrap();
    }
    std::fs::write(
        dir.join("Piperine.toml"),
        "[project]\nname = \"stub-smoke\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
         [plugins.fixture]\npath = \"fixture-plugin\"\n",
    )
    .unwrap();
}

/// A plugin publishing an `extern.phdl` stub declaring a custom attribute
/// schema — its declaration is available in a project using that plugin
/// with **no** explicit `use`.
#[test]
fn published_stub_attribute_resolves_without_explicit_use() {
    let artifact = fixture_cdylib();
    let dir = tempfile::tempdir().unwrap();
    project_with_fixture(
        dir.path(),
        &artifact,
        Some("extern attribute widget_meta { rating: Real }\n"),
    );

    let host = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll).expect("load");
    assert_eq!(host.plugin_names(), vec!["fixture"]);

    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
               mod Top ( inout p : Electrical ) { @widget_meta(rating = 4.5) wire w : Electrical; }";
    let design = piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("`@widget_meta` must resolve via the plugin's auto-imported extern.phdl stub");

    // The attribute round-trips: it's on the wire, with the declared value.
    let module = design.module("Top").expect("module Top");
    let widget = module.wires[0]
        .attributes()
        .iter()
        .find(|a| a.schema() == "widget_meta")
        .expect("@widget_meta attribute present");
    assert_eq!(widget.field("rating"), Some(&piperine_lang::Value::Real(4.5)));
}

/// A project using a plugin that publishes **no** `extern.phdl` stub still
/// elaborates normally as long as nothing references a plugin-contributed
/// schema — T24 imposes no requirement that every plugin publish one;
/// enforcement of "plugin contributes a schema ⇒ must publish a stub" is
/// T25's job.
#[test]
fn no_stub_is_fine_when_nothing_needs_one() {
    let artifact = fixture_cdylib();
    let dir = tempfile::tempdir().unwrap();
    project_with_fixture(dir.path(), &artifact, None);

    let host = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll).expect("load");
    assert_eq!(host.plugin_names(), vec!["fixture"]);

    let src = "mod Top() {}\n";
    let design = piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("elaboration without any stub-backed attribute use must still succeed");
    assert!(design.module("Top").is_some());
}
