//! Phase-3 gates (Plugin plan): `transform_design` staging injection (the
//! rc-parasitics case), no-netlist-magic and conflict failures, scripts
//! under capability enforcement, and the read-only hooks — all driven
//! through the root host API (`SimSession` + `SimHooks`).

use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use piperine::{NetRef, OpResult, SimSession, SolverConfig};
use piperine_lang::{SourceMap, Value};
use piperine_plugin::{
    Abi, Design, DesignStaging, HostCtx, Manifest, Plugin, PluginError, PluginHost, PluginResult,
    Registrar, ScriptHandler, SolveResultView,
};

fn manifest(name: &str) -> Manifest {
    Manifest {
        name: name.into(),
        abi: Abi::Native,
        entry: String::new(),
        description: None,
        permissions: Default::default(),
    }
}

fn elab(src: &str, host: &PluginHost) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("elaborate")
}

/// An operating point of `Top` through a session wired with the host's
/// device provider + lifecycle hooks.
fn run_top_op(host: Rc<PluginHost>, src: &str) -> Result<OpResult, piperine::Error> {
    let design = elab(src, &host);
    let mut session = SimSession::new(design, "Top".to_string());
    session.set_device_provider(host.clone());
    session.set_hooks(host);
    session.run_op(&SolverConfig::default(), None)
}

fn v(op: &OpResult, net: &str) -> f64 {
    op.v(&NetRef { name: net.to_string() }, None).expect("net readable")
}

/// r1 dangles until the plugin injects `r_par` from `out` to `gnd`,
/// turning the circuit into a divider: `out = 5 V · 1k/(1k+1k) = 2.5 V`.
const DIVIDER: &str = "
    discipline Electrical { potential v: Real; flow i: Real; }

    mod VoltageSource(inout p: Electrical, inout n: Electrical) {
        param voltage: Real = 0.0;
    }
    analog VoltageSource { V(p, n) <- voltage; }

    mod Resistor(inout p: Electrical, inout n: Electrical) {
        param r: Real = 1e3;
    }
    analog Resistor { I(p, n) <+ V(p, n) / r; }

    mod Top() {
        wire gnd : Electrical;
        wire vin : Electrical;
        wire out : Electrical;
        src : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
        r1  : Resistor (.p = vin, .n = out);
    }
";

/// The rc-parasitics reference case (SPEC Part VI §8.3): stages a declared
/// `Resistor` from `out` to `gnd`, turning the dangling r1 into a divider.
struct Parasitics {
    manifest: Manifest,
    module: &'static str,
}

impl Plugin for Parasitics {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn transform_design(&self, _cx: &mut HostCtx, staging: &DesignStaging<'_>) -> PluginResult<()> {
        staging.add_instance(
            "Top",
            "r_par",
            self.module,
            vec!["out".into(), "gnd".into()],
            vec![("r".into(), Value::Real(1e3))],
        )
    }
}

fn parasitics(name: &str, module: &'static str) -> Box<dyn Plugin> {
    Box::new(Parasitics { manifest: manifest(name), module })
}

#[test]
fn transform_design_injects_a_declared_instance() {
    let host = Rc::new(PluginHost::from_plugins(vec![parasitics("para", "Resistor")]).unwrap());
    let out = v(&run_top_op(host, DIVIDER).expect("op solves"), "out");
    assert!(out > 2.49 && out < 2.51, "divider at 2.5 V, got {out}");
}

#[test]
fn restaging_across_analyses_is_idempotent() {
    let host = Rc::new(PluginHost::from_plugins(vec![parasitics("para", "Resistor")]).unwrap());
    let design = elab(DIVIDER, &host);
    let mut session = SimSession::new(design, "Top".to_string());
    session.set_device_provider(host.clone());
    session.set_hooks(host);
    let first = session.run_op(&SolverConfig::default(), None).expect("first op");
    let second = session.run_op(&SolverConfig::default(), None).expect("second op");
    assert!((v(&first, "out") - 2.5).abs() < 0.01, "first analysis at 2.5 V");
    assert!((v(&second, "out") - 2.5).abs() < 0.01, "second analysis identical");
}

#[test]
fn undeclared_type_fails_loud() {
    // No-netlist-magic (SPEC Part VI §2): `Varistor` was never declared.
    let host = Rc::new(PluginHost::from_plugins(vec![parasitics("para", "Varistor")]).unwrap());
    let msg = run_top_op(host, DIVIDER).expect_err("must fail").to_string();
    assert!(msg.contains("not declared"), "unexpected message: {msg}");
}

#[test]
fn conflicting_specs_from_two_plugins_fail_loud() {
    // Both stage `Top.r_par` with different modules — a staging conflict
    // (SPEC Part VI §8.2). `Extra` is declared, so the type check passes
    // and the conflict is the failure.
    let src = format!("{DIVIDER}\n mod Extra(inout p: Electrical, inout n: Electrical) {{ param r: Real = 1.0; }} analog Extra {{ I(p,n) <+ V(p,n)/r; }}");
    let host = Rc::new(
        PluginHost::from_plugins(vec![parasitics("aaa", "Resistor"), parasitics("bbb", "Extra")])
            .unwrap(),
    );
    let msg = run_top_op(host, &src).expect_err("must fail").to_string();
    // Typed P0008: names both plugins and the staging path.
    assert!(
        msg.contains("aaa") && msg.contains("bbb") && msg.contains("Top.r_par"),
        "unexpected message: {msg}"
    );
}

// ─── Scripts + capability enforcement ─────────────────────────────────────────

struct WriterScript;
impl ScriptHandler for WriterScript {
    fn invoke(&self, args: &[String], cx: &mut HostCtx) -> Result<i32, String> {
        let out = args.first().cloned().unwrap_or_else(|| "converted.phdl".into());
        cx.fs_write(&out, "// transcribed\n").map_err(|e| e.to_string())?;
        Ok(0)
    }
}

struct ScriptPlugin {
    manifest: Manifest,
}
impl Plugin for ScriptPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn register(&self, r: &mut Registrar) {
        r.script("transcribe", Box::new(WriterScript));
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

// ─── Read-only hooks ───────────────────────────────────────────────────────────

struct Observer {
    manifest: Manifest,
    elaborated: Arc<AtomicUsize>,
    solved: Arc<AtomicUsize>,
}
impl Plugin for Observer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn after_elaborate(&self, _cx: &mut HostCtx, design: &Design) -> PluginResult<()> {
        assert!(design.module("Top").is_some(), "hook must see the design");
        self.elaborated.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn after_solve(&self, _cx: &mut HostCtx, result: &SolveResultView) -> PluginResult<()> {
        assert_eq!(result.analysis, "op");
        assert!(
            result.node_voltages.iter().any(|(n, v)| n == "vin" && (*v - 5.0).abs() < 1e-6),
            "op voltages must be visible: {:?}",
            result.node_voltages
        );
        self.solved.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn read_only_hooks_observe_the_pipeline() {
    let elaborated = Arc::new(AtomicUsize::new(0));
    let solved = Arc::new(AtomicUsize::new(0));
    let host = Rc::new(
        PluginHost::from_plugins(vec![
            parasitics("para", "Resistor"),
            Box::new(Observer {
                manifest: manifest("watch"),
                elaborated: elaborated.clone(),
                solved: solved.clone(),
            }),
        ])
        .unwrap(),
    );
    let design = elab(DIVIDER, &host);
    host.fire_after_elaborate(&design).expect("after_elaborate");
    let mut session = SimSession::new(design, "Top".to_string());
    session.set_device_provider(host.clone());
    session.set_hooks(host);
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    assert!((v(&op, "out") - 2.5).abs() < 0.01);
    assert_eq!(elaborated.load(Ordering::SeqCst), 1);
    assert_eq!(solved.load(Ordering::SeqCst), 1);
}

/// Registration-time collision surface: two plugins contributing the same
/// schema name is P0003 (kept from the contribution registry's contract).
#[test]
fn schema_collisions_are_p0003() {
    struct SchemaPlugin {
        manifest: Manifest,
    }
    impl Plugin for SchemaPlugin {
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }
        fn register(&self, r: &mut Registrar) {
            r.attr_schema("dup", vec![]);
        }
    }
    let err = PluginHost::from_plugins(vec![
        Box::new(SchemaPlugin { manifest: manifest("a") }),
        Box::new(SchemaPlugin { manifest: manifest("b") }),
    ])
    .map(|_| ())
    .expect_err("duplicate schema must fail");
    assert!(matches!(err, PluginError::SchemaConflict { .. }), "{err}");
}
