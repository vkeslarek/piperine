//! The read-only lifecycle hooks: `after_elaborate` sees the real design and
//! `after_solve` sees the operating point, each firing exactly once per run.

use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use piperine::{OpResult, Session, SolverConfig};
use piperine_lang::{SourceMap, Value};
use piperine_plugin::{
    Design, DesignStaging, HostCtx, Manifest, Plugin, PluginHost, PluginResult, SolveResultView,
};

fn manifest(name: &str) -> Manifest {
    Manifest {
        name: name.into(),
        description: None,
        python: None,
        device: None,
        permissions: Default::default(),
    }
}

fn elab(src: &str, host: &PluginHost) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("elaborate")
}

fn v(op: &OpResult, net: &str) -> f64 {
    op.v(net.to_string()).expect("net readable")
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
    let mut session = Session::builder(&design, "Top")
        .provider(host.clone())
        .hooks(host)
        .compile()
        .expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    assert!((v(&op, "out") - 2.5).abs() < 0.01);
    assert_eq!(elaborated.load(Ordering::SeqCst), 1);
    assert_eq!(solved.load(Ordering::SeqCst), 1);
}
