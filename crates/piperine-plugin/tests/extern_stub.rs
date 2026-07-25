//! Plugin `extern` ban (plugin-interface v2: PLG-07, PLG-08, PLG-09). The
//! per-plugin `extern.phdl` stub mechanism is deleted — a plugin cannot
//! mint new attribute-schema names; only the stdlib `@device`/`@port`
//! schemas (`headers/device_port.phdl`) are seeded when a plugin loads.
//! Driven through the real `load_for_project` path (build the fixture
//! cdylib, point a throwaway project's `[plugins]` at it).

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
/// `extern.phdl` file shipped alongside the manifest.
fn project_with_fixture_and_stub(dir: &std::path::Path, artifact: &std::path::Path, stub: &str) {
    let plugin_dir = dir.join("fixture-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let entry = artifact.file_name().unwrap().to_str().unwrap();
    std::fs::copy(artifact, plugin_dir.join(entry)).unwrap();
    std::fs::write(
        plugin_dir.join("piperine-plugin.toml"),
        format!("[plugin]\nname = \"fixture\"\ndevice = {{ path = \"{entry}\" }}\n"),
    )
    .unwrap();
    std::fs::write(plugin_dir.join("extern.phdl"), stub).unwrap();
    std::fs::write(
        dir.join("Piperine.toml"),
        "[project]\nname = \"stub-smoke\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
         [plugins.fixture]\npath = \"fixture-plugin\"\n",
    )
    .unwrap();
}

/// PLG-07/08/09: a plugin shipping an `extern.phdl` loads fine (the file
/// is inert — no per-plugin stub is parsed or imported), a schema name the
/// stub declares does NOT resolve (no plugin-schema path), and the stdlib
/// `@device` schema still seeds for the same host.
#[test]
fn shipped_extern_stub_is_inert_and_only_stdlib_schemas_seed() {
    let artifact = fixture_cdylib();
    let dir = tempfile::tempdir().unwrap();
    project_with_fixture_and_stub(
        dir.path(),
        &artifact,
        "extern attribute widget_meta { rating: Real }\n",
    );

    // Shipping the file is not an error — it is simply never loaded.
    let host = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll).expect("load");
    assert_eq!(host.plugin_names(), vec!["fixture"]);

    // The stdlib `@device`/`@port` schemas still seed (PLG-09).
    let stdlib_src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
                      @device(plugin = \"fixture\", type = \"Fixture::Resistor\")\n\
                      mod PluginResistor(inout p: Electrical, inout n: Electrical) {\n\
                          param r: Real = 100.0;\n\
                      }\n\
                      mod Top() {}\n";
    let design = piperine_lang::parse_and_elaborate_seeded(stdlib_src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("the stdlib `@device` schema must still resolve");
    assert!(design.module("Top").is_some());

    // The plugin-shipped schema name does NOT resolve (PLG-07/08): the
    // stub is never imported, so `@widget_meta` is an unknown schema.
    let stub_src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
                    mod Top ( inout p : Electrical ) { @widget_meta(rating = 4.5) wire w : Electrical; }";
    let err = piperine_lang::parse_and_elaborate_seeded(stub_src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect_err("a plugin-shipped `extern.phdl` must not mint schema names");
    assert!(
        format!("{err:?}").contains("widget_meta"),
        "the error names the unresolvable schema: {err:?}"
    );
}
