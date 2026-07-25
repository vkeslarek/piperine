//! Manifest parsing, shape inference, and removed-backend errors
//! (plugin-interface v2: PLG-02, PLG-21). The shape is inferred from which
//! keys are present — `python` → scripted, `device` → device, neither →
//! pure-PHDL — with no `abi` field anywhere.

use piperine_plugin::{Manifest, PluginError, PluginShape};

#[test]
fn device_manifest_infers_device_shape() {
    let m = Manifest::parse(
        "avr-cosim",
        r#"
        [plugin]
        name        = "avr-cosim"
        description = "AVR co-simulation"
        device      = { path = "libavr_cosim.so" }

        [permissions]
        filesystem     = ["read *.hex"]
        network        = false
        process_spawn  = ["simavr"]
        "#,
    )
    .expect("parse");
    assert_eq!(m.name, "avr-cosim");
    assert_eq!(m.shape(), PluginShape::Device);
    let device = m.device.as_ref().expect("device source present");
    assert_eq!(device.path.as_deref(), Some(std::path::Path::new("libavr_cosim.so")));
    assert_eq!(m.permissions.filesystem, vec!["read *.hex"]);
    assert_eq!(m.permissions.process_spawn, vec!["simavr"]);
    assert!(!m.permissions.network);
}

#[test]
fn python_manifest_infers_scripted_shape() {
    let m = Manifest::parse(
        "lint",
        r#"
        [plugin]
        name   = "lint"
        python = "plugin.py"
        "#,
    )
    .expect("parse");
    assert_eq!(m.shape(), PluginShape::Scripted);
    assert_eq!(m.python.as_deref(), Some(std::path::Path::new("plugin.py")));
    assert!(m.device.is_none());
}

#[test]
fn bare_plugin_section_infers_pure_phdl_shape() {
    let m = Manifest::parse(
        "models",
        r#"
        [plugin]
        name = "models"
        "#,
    )
    .expect("parse");
    assert_eq!(m.shape(), PluginShape::Pure);
    assert!(m.python.is_none());
    assert!(m.device.is_none());
}

/// A plugin may carry BOTH a device binary and a python entry (spec Edge
/// Cases — both load); the declared shape classifies by the load-bearing
/// binary, and both keys stay visible on the manifest.
#[test]
fn device_and_python_both_parse() {
    let m = Manifest::parse(
        "combo",
        r#"
        [plugin]
        name   = "combo"
        python = "glue.py"
        device = { path = "libcombo.so" }
        "#,
    )
    .expect("parse");
    assert_eq!(m.shape(), PluginShape::Device);
    assert!(m.device.is_some());
    assert_eq!(m.python.as_deref(), Some(std::path::Path::new("glue.py")));
}

/// PLG-02: a manifest declaring a removed backend fails with a targeted
/// `RemovedBackend` naming the backend — never a generic unknown-field or
/// unknown-value error.
#[test]
fn removed_backends_are_a_targeted_error() {
    for backend in ["wasm", "process"] {
        let src = format!("[plugin]\nname = \"x\"\nabi = \"{backend}\"\nentry = \"x.bin\"\n");
        let err = Manifest::parse("x", &src).expect_err(&src);
        match &err {
            PluginError::RemovedBackend { backend: named, .. } => {
                assert_eq!(named, backend, "the error names the removed backend");
            }
            other => panic!("{backend}: expected RemovedBackend, got {other}"),
        }
        let msg = err.to_string();
        assert!(msg.contains(backend), "message names the removed backend: {msg}");
        assert!(msg.contains("removed"), "message says it was removed: {msg}");
    }
}

/// Any other `abi` value is not a removed-backend case but still not a v2
/// field: a bad-manifest error that names the field, never silently parsed.
#[test]
fn any_other_abi_field_is_rejected() {
    for src in [
        "[plugin]\nname = \"x\"\nabi = \"native\"\nentry = \"x.so\"\n",
        "[plugin]\nname = \"x\"\nabi = \"exe\"\n",
    ] {
        let err = Manifest::parse("x", src).expect_err(src);
        assert!(matches!(err, PluginError::BadManifest { .. }), "{src}: {err}");
        assert!(!matches!(err, PluginError::RemovedBackend { .. }), "{src}: {err}");
        assert!(err.to_string().contains("abi"), "{src}: {err}");
    }
}

/// A `device` table needs exactly one source: a local `path` or a
/// `release` coordinate — never neither, never both.
#[test]
fn device_source_needs_exactly_one_of_path_or_release() {
    for src in [
        "[plugin]\nname = \"x\"\ndevice = {}\n",
        "[plugin]\nname = \"x\"\ndevice = { path = \"x.so\", release = \"github:a/b@v1\" }\n",
    ] {
        let err = Manifest::parse("x", src).expect_err(src);
        assert!(matches!(err, PluginError::BadManifest { .. }), "{src}: {err}");
    }
    let m = Manifest::parse(
        "x",
        "[plugin]\nname = \"x\"\ndevice = { release = \"github:a/b@v1\", verify = \"sha256:ab\" }\n",
    )
    .expect("a release-only device source parses");
    assert_eq!(m.shape(), PluginShape::Device);
}

#[test]
fn minimal_manifest_gets_default_permissions() {
    let m = Manifest::parse("x", "[plugin]\nname = \"x\"\n").expect("parse");
    assert!(m.permissions.filesystem.is_empty());
    assert!(!m.permissions.network);
    assert!(m.permissions.process_spawn.is_empty());
}

#[test]
fn empty_name_and_malformed_toml_are_bad_manifest() {
    for src in ["[plugin]\nname = \"\"\n", "not toml at all ["] {
        let err = Manifest::parse("x", src).expect_err(src);
        assert!(matches!(err, PluginError::BadManifest { .. }), "{src}: {err}");
    }
}

#[test]
fn unknown_permission_field_is_rejected() {
    let err = Manifest::parse(
        "x",
        r#"
        [plugin]
        name = "x"

        [permissions]
        sudo = true
        "#,
    )
    .expect_err("unknown permission must not parse");
    assert!(matches!(err, PluginError::BadManifest { .. }));
}

/// A manifest declaring `bench_tasks` fails loud with the removal notice —
/// the in-language bench (and its plugin extension point) no longer exists;
/// the generic "unknown field" would send authors hunting the wrong trail.
#[test]
fn bench_tasks_manifest_is_a_clear_removal_error() {
    let err = Manifest::parse(
        "x",
        r#"
        [plugin]
        name = "x"

        bench_tasks = ["gain"]
        "#,
    )
    .expect_err("bench_tasks must be rejected");
    let msg = err.to_string();
    assert!(msg.contains("bench"), "names the removed surface: {msg}");
    assert!(msg.contains("removed"), "says it is removed: {msg}");
    assert!(msg.contains("*_tb.py"), "points at python testbenches: {msg}");
}
