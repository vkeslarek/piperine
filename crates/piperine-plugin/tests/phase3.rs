//! Phase-3 gates (Plugin plan): `transform_design` staging injection (the
//! rc-parasitics case), no-netlist-magic and conflict failures, plugin
//! bench tasks through the allowlist gate, scripts under capability
//! enforcement, and the read-only hooks.

use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use piperine_bench::{BenchOutcome, BenchRunner};
use piperine_lang::eval::Value;
use piperine_lang::SourceMap;
use piperine_plugin::{
    Abi, Design, DesignStaging, HostCtx, Manifest, Permissions, Plugin, PluginBenchTask,
    PluginError, PluginHost, PluginResult, Registrar, ScriptHandler, SolveResultView,
};

fn manifest(name: &str) -> Manifest {
    Manifest {
        name: name.into(),
        abi: Abi::Native,
        entry: String::new(),
        description: None,
        permissions: Permissions::default(),
    }
}

fn elab(src: &str, host: &PluginHost) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("elaborate")
}

fn run(src: &str, host: Rc<PluginHost>, module: &str, entry: &str) -> BenchOutcome {
    let design = elab(src, &host);
    BenchRunner::new(&design)
        .with_device_provider(host.clone())
        .with_plugins(host)
        .run_entry(module, entry)
}

fn assert_passed(outcome: BenchOutcome) {
    match outcome {
        BenchOutcome::Passed => {}
        BenchOutcome::Failed(m) => panic!("bench assert failed: {m}"),
        BenchOutcome::Error(m) => panic!("bench errored: {m}"),
    }
}

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
    bench Top {
        fn divider() {
            var r = $op();
            $assert(r.v(out, gnd) > 2.49, \"divider low\");
            $assert(r.v(out, gnd) < 2.51, \"divider high\");
        }
        fn divider_twice() {
            var a = $op();
            var b = $op();
            $assert(b.v(out, gnd) > 2.49, \"second analysis identical\");
            $assert(b.v(out, gnd) < 2.51, \"second analysis identical\");
        }
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
    assert_passed(run(DIVIDER, host, "Top", "divider"));
}

#[test]
fn restaging_across_analyses_is_idempotent() {
    let host = Rc::new(PluginHost::from_plugins(vec![parasitics("para", "Resistor")]).unwrap());
    assert_passed(run(DIVIDER, host, "Top", "divider_twice"));
}

#[test]
fn undeclared_type_fails_loud() {
    // No-netlist-magic (SPEC Part VI §2): `Varistor` was never declared.
    let host = Rc::new(PluginHost::from_plugins(vec![parasitics("para", "Varistor")]).unwrap());
    match run(DIVIDER, host, "Top", "divider") {
        BenchOutcome::Error(msg) => {
            assert!(msg.contains("not declared"), "unexpected message: {msg}");
        }
        other => panic!("expected loud error, got {other:?}"),
    }
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
    match run(&src, host, "Top", "divider") {
        BenchOutcome::Error(msg) => {
            // Typed P0008: names both plugins and the staging path.
            assert!(msg.contains("aaa") && msg.contains("bbb") && msg.contains("Top.r_par"),
                "unexpected message: {msg}");
        }
        other => panic!("expected loud error, got {other:?}"),
    }
}

// ─── Plugin bench tasks ────────────────────────────────────────────────────────

struct GainTask;
impl PluginBenchTask for GainTask {
    fn run(&self, _args: Vec<Value>, _cx: &mut HostCtx) -> Result<Value, String> {
        Ok(Value::Real(42.0))
    }
}

struct TaskPlugin {
    manifest: Manifest,
}
impl Plugin for TaskPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn register(&self, r: &mut Registrar) {
        r.bench_task("gain", Box::new(GainTask));
    }
}

const TASK_BENCH: &str = "
    mod Top() {}
    bench Top {
        fn uses_plugin_task() {
            $assert($gain() == 42.0, \"plugin task value\");
        }
    }
";

#[test]
fn plugin_bench_task_passes_the_allowlist_and_runs() {
    let host =
        Rc::new(PluginHost::from_plugins(vec![Box::new(TaskPlugin { manifest: manifest("t") })]).unwrap());
    assert_passed(run(TASK_BENCH, host, "Top", "uses_plugin_task"));
}

#[test]
fn unseeded_plugin_task_is_an_elaboration_error() {
    // Without the host's seeding, `$gain` must fail the allowlist gate.
    let err = piperine_lang::parse_and_elaborate(TASK_BENCH, &SourceMap::dummy())
        .expect_err("gate must reject");
    assert!(format!("{err:?}").contains("gain"), "{err:?}");
}

#[test]
fn builtin_task_names_cannot_be_shadowed() {
    struct Shadow {
        manifest: Manifest,
    }
    impl Plugin for Shadow {
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }
        fn register(&self, r: &mut Registrar) {
            r.bench_task("op", Box::new(GainTask));
        }
    }
    let err = PluginHost::from_plugins(vec![Box::new(Shadow { manifest: manifest("s") })])
        .map(|_| ())
        .expect_err("shadowing $op must fail");
    assert!(matches!(err, PluginError::SchemaConflict { .. }), "{err}");
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
    let outcome = BenchRunner::new(&design)
        .with_device_provider(host.clone())
        .with_plugins(host)
        .run_entry("Top", "divider");
    assert_passed(outcome);
    assert_eq!(elaborated.load(Ordering::SeqCst), 1);
    assert_eq!(solved.load(Ordering::SeqCst), 1);
}
