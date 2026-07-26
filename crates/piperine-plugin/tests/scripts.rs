//! Plugin-contributed CLI scripts under capability enforcement: a
//! `#[pip::script]` writes only what its `[permissions] filesystem` globs
//! allow, and an unknown script name is `None` (the CLI maps that to P0009).

use piperine_plugin::{Ctx, Manifest, Plugin, PluginHost};

fn manifest(name: &str) -> Manifest {
    Manifest {
        name: name.into(),
        description: None,
        python: None,
        device: None,
        permissions: Default::default(),
    }
}

/// Declared+bound by the one attribute (PLG-06); the default
/// `Plugin::collect()` surfaces it from this binary's registry.
#[pip::script("transcribe")]
fn transcribe(args: &[String], ctx: &Ctx) -> Result<i32, String> {
    let out = args.first().cloned().unwrap_or_else(|| "converted.phdl".into());
    ctx.fs_write(&out, "// transcribed\n").map_err(|e| e.to_string())?;
    Ok(0)
}

struct ScriptPlugin {
    manifest: Manifest,
}
impl Plugin for ScriptPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

#[test]
fn script_runs_under_its_filesystem_capability() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = manifest("importer");
    m.permissions.filesystem = vec!["write *.phdl".into()];
    let host = PluginHost::from_plugins(vec![Box::new(ScriptPlugin { manifest: m })])
        .unwrap()
        .with_project_root(dir.path());

    // Allowed: matches `write *.phdl`.
    let code = host
        .run_script("transcribe", &["converted.phdl".to_string()])
        .expect("script registered")
        .expect("script ok");
    assert_eq!(code, 0);
    assert!(dir.path().join("converted.phdl").exists());

    // Denied: `.cir` matches no write glob → P0002 inside the script error.
    let err = host
        .run_script("transcribe", &["converted.cir".to_string()])
        .expect("script registered")
        .expect_err("must be denied");
    assert!(err.to_string().contains("P0002") || err.to_string().contains("capability"), "{err}");

    // Unknown script name → None (the CLI maps it to P0009).
    assert!(host.run_script("nope", &[]).is_none());
}
