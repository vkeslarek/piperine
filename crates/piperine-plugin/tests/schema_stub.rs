//! End-to-end plugin schema stub exercise + enforcement flip
//! (declared-language-surface T25, DLS-22): a plugin contributing a custom
//! attribute schema (`Registrar::attr_schema`, the dynamic runtime path)
//! must also publish a matching `extern.phdl` stub — the textual anchor
//! `@widget_meta(...)` actually resolves through, ctrl+click-able like any
//! other `extern attribute`. A plugin that contributes a schema but
//! publishes no stub fails loud at load time (`PluginError::
//! MissingExternStub`), never silently falling back to the old dynamic
//! `register_declared` path (spec Edge Cases).

use std::path::PathBuf;

use piperine_lang::SourceMap;
use piperine_plugin::{PluginError, PluginHost, TrustMode};

/// Build the `fixture_schema_plugin` example cdylib.
fn schema_fixture_cdylib() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.parent().unwrap().parent().unwrap();
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "-p", "piperine-plugin", "--example", "fixture_schema_plugin"])
        .current_dir(workspace)
        .status()
        .expect("cargo build fixture example");
    assert!(status.success(), "fixture build failed");
    let lib = format!(
        "{}fixture_schema_plugin{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    workspace.join("target").join("debug").join("examples").join(lib)
}

/// A throwaway project whose `[plugins]` names the schema fixture by path,
/// publishing an `extern.phdl` stub alongside the manifest when `stub` is
/// `Some`.
fn project_with_schema_fixture(dir: &std::path::Path, artifact: &std::path::Path, stub: Option<&str>) {
    let plugin_dir = dir.join("schema-fixture-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let entry = artifact.file_name().unwrap().to_str().unwrap();
    std::fs::copy(artifact, plugin_dir.join(entry)).unwrap();
    std::fs::write(
        plugin_dir.join("piperine-plugin.toml"),
        format!("[plugin]\nname = \"schema-fixture\"\nabi = \"native\"\nentry = \"{entry}\"\n"),
    )
    .unwrap();
    if let Some(text) = stub {
        std::fs::write(plugin_dir.join("extern.phdl"), text).unwrap();
    }
    std::fs::write(
        dir.join("Piperine.toml"),
        "[project]\nname = \"schema-stub-smoke\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
         [plugins.schema-fixture]\npath = \"schema-fixture-plugin\"\n",
    )
    .unwrap();
}

/// The fixture's contributed `widget_meta` schema, published as a matching
/// `extern.phdl` stub, resolves end-to-end: `@widget_meta(rating = ...)`
/// elaborates with no explicit `use`, exactly like `@device`/`@port`.
#[test]
fn plugin_contributed_schema_resolves_via_its_published_stub() {
    let artifact = schema_fixture_cdylib();
    let dir = tempfile::tempdir().unwrap();
    project_with_schema_fixture(
        dir.path(),
        &artifact,
        Some("extern attribute widget_meta { rating: Real }\n"),
    );

    let host = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll)
        .expect("plugin with a matching stub must load");
    assert_eq!(host.plugin_names(), vec!["schema-fixture"]);

    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
               mod Top ( inout p : Electrical ) { @widget_meta(rating = 9.5) wire w : Electrical; }";
    let design = piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("`@widget_meta` must resolve via the plugin's published stub");

    let module = design.module("Top").expect("module Top");
    let widget = module.wires[0]
        .attributes()
        .iter()
        .find(|a| a.schema() == "widget_meta")
        .expect("@widget_meta attribute present");
    assert_eq!(widget.field("rating"), Some(&piperine_lang::Value::Real(9.5)));
}

/// A plugin that contributes an attribute schema (`Registrar::attr_schema`)
/// but publishes **no** `extern.phdl` stub fails loud at load time — never
/// silently falls back to the old dynamic-registration path (spec Edge
/// Cases, DLS-22).
#[test]
fn plugin_without_stub_fails_loud_naming_missing_stub() {
    let artifact = schema_fixture_cdylib();
    let dir = tempfile::tempdir().unwrap();
    project_with_schema_fixture(dir.path(), &artifact, None);

    let err = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll)
        .map(|_| ())
        .expect_err("a schema-contributing plugin with no stub must fail to load");
    match &err {
        PluginError::MissingExternStub { plugin, schema, expected_path } => {
            assert_eq!(plugin, "schema-fixture");
            assert_eq!(schema, "widget_meta");
            assert!(expected_path.ends_with("extern.phdl"), "{expected_path}");
        }
        other => panic!("expected MissingExternStub, got {other}"),
    }
}
