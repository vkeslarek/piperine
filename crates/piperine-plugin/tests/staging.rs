//! Design staging through `transform_design`: the rc-parasitics reference case
//! (SPEC Part VI §8.3), idempotent restaging across analyses, and the typed
//! conflict when two plugins stage the same path (§8.2) — driven through the
//! root host API (`SimSession` + `SimHooks`).
//!
//! Device-shaped injection (`@device` modules through the plugin device path)
//! lives in `inject.rs`.

use std::rc::Rc;

use piperine::{OpResult, SimSession, SolverConfig};
use piperine_lang::{SourceMap, Value};
use piperine_plugin::{DesignStaging, HostCtx, Manifest, Plugin, PluginHost, PluginResult};

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
