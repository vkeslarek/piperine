//! Manifest parsing and validation (SPEC Part VI §4, P0006).

use piperine_plugin::{Abi, Manifest, PluginError};

#[test]
fn full_manifest_parses() {
    let m = Manifest::parse(
        "avr-cosim",
        r#"
        [plugin]
        name        = "avr-cosim"
        abi         = "native"
        entry       = "libavr_cosim.so"
        description = "AVR co-simulation"

        [permissions]
        filesystem     = ["read *.hex"]
        network        = false
        process_spawn  = ["simavr"]
        timeout_ms     = 2000
        "#,
    )
    .expect("parse");
    assert_eq!(m.name, "avr-cosim");
    assert_eq!(m.abi, Abi::Native);
    assert_eq!(m.entry, "libavr_cosim.so");
    assert_eq!(m.permissions.filesystem, vec!["read *.hex"]);
    assert_eq!(m.permissions.process_spawn, vec!["simavr"]);
    assert_eq!(m.permissions.timeout_ms, 2000);
    assert!(!m.permissions.network);
}

#[test]
fn minimal_manifest_gets_default_permissions() {
    let m = Manifest::parse(
        "x",
        r#"
        [plugin]
        name  = "x"
        abi   = "wasm"
        entry = "x.wasm"
        "#,
    )
    .expect("parse");
    assert!(m.permissions.filesystem.is_empty());
    assert!(!m.permissions.network);
    assert!(m.permissions.process_spawn.is_empty());
    assert_eq!(m.permissions.timeout_ms, 5000);
}

#[test]
fn missing_fields_and_unknown_abi_are_bad_manifest() {
    for src in [
        "[plugin]\nname = \"x\"\nabi = \"native\"",             // no entry
        "[plugin]\nname = \"x\"\nabi = \"exe\"\nentry = \"x\"", // unknown abi
        "[plugin]\nname = \"\"\nabi = \"wasm\"\nentry = \"x\"", // empty name
        "not toml at all [",
    ] {
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
        abi = "wasm"
        entry = "x.wasm"

        [permissions]
        sudo = true
        "#,
    )
    .expect_err("unknown permission must not parse");
    assert!(matches!(err, PluginError::BadManifest { .. }));
}
