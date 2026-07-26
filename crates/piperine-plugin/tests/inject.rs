//! Device injection through `transform_design` staging (plugin-interface v2,
//! PLG-13/14/15): a hook stages `add_instance(parent, label, module, ports…)`
//! for a **`@device` module** — the injected device appears in the analysed
//! design and solves through the native-device path (never a parallel one);
//! a bad parent or undeclared module fails loud at staging time; authored
//! structure is never overwritten (MD-25).

use std::rc::Rc;

use piperine::{OpResult, SimSession, SolverConfig};
use piperine_lang::{SourceMap, Value};
use piperine_plugin::{
    DesignStaging, HostCtx, Manifest, Permissions, Plugin, PluginHost, PluginResult,
};

#[path = "../examples/fixture_plugin.rs"]
mod fixture_plugin;
use fixture_plugin::FixturePlugin;

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

/// `r1` (authored, 1 kΩ) dangles at `out` until the hook injects a
/// `@device` PluginResistor from `out` to `gnd` — a divider whose lower
/// leg is the plugin binary's device, not a PHDL module.
const DEVICE_DIVIDER: &str = "
    discipline Electrical { potential v: Real; flow i: Real; }

    mod VoltageSource(inout p: Electrical, inout n: Electrical) {
        param voltage: Real = 0.0;
    }
    analog VoltageSource { V(p, n) <- voltage; }

    mod Resistor(inout p: Electrical, inout n: Electrical) {
        param r: Real = 1e3;
    }
    analog Resistor { I(p, n) <+ V(p, n) / r; }

    @device(plugin = \"fixture\", type = \"Fixture::Resistor\")
    mod PluginResistor(inout p: Electrical, inout n: Electrical) {
        param r: Real = 100.0;
    }

    mod Top() {
        wire gnd : Electrical;
        wire vin : Electrical;
        wire out : Electrical;
        src : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
        r1  : Resistor (.p = vin, .n = out);
    }
";

/// A `transform_design` hook staging one instance injection.
struct Injector {
    manifest: Manifest,
    parent: &'static str,
    label: &'static str,
    module: &'static str,
    r: f64,
}

impl Plugin for Injector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn transform_design(&self, _cx: &mut HostCtx, staging: &DesignStaging<'_>) -> PluginResult<()> {
        staging.add_instance(
            self.parent,
            self.label,
            self.module,
            vec!["out".into(), "gnd".into()],
            vec![("r".into(), Value::Real(self.r))],
        )
    }
}

fn injector(parent: &'static str, label: &'static str, module: &'static str, r: f64) -> Box<dyn Plugin> {
    Box::new(Injector {
        manifest: Manifest {
            name: "injector".into(),
            description: None,
            python: None,
            device: None,
            permissions: Permissions::default(),
        },
        parent,
        label,
        module,
        r,
    })
}

fn host_with(plugins: Vec<Box<dyn Plugin>>) -> Rc<PluginHost> {
    let mut all: Vec<Box<dyn Plugin>> = vec![Box::new(FixturePlugin::new())];
    all.extend(plugins);
    Rc::new(PluginHost::from_plugins(all).expect("host"))
}

#[test]
fn injected_device_instance_solves_through_the_device_path() {
    // PLG-13/14: the staged `@device` instance appears in the analysed
    // design and stamps through the native-device path — out = 5 V ·
    // 1k/(1k+1k) = 2.5 V only if the plugin binary's resistor solves.
    let host = host_with(vec![injector("Top", "r_par", "PluginResistor", 1e3)]);
    let out = v(&run_top_op(host, DEVICE_DIVIDER).expect("op solves"), "out");
    assert!(out > 2.49 && out < 2.51, "divider at 2.5 V, got {out}");
}

#[test]
fn injected_device_honors_staged_params() {
    // The staged param reaches the device binary's `from_spec`: r = 3 kΩ →
    // out = 5 V · 3k/(1k+3k) = 3.75 V (not the module default 100 Ω).
    let host = host_with(vec![injector("Top", "r_par", "PluginResistor", 3e3)]);
    let out = v(&run_top_op(host, DEVICE_DIVIDER).expect("op solves"), "out");
    assert!(out > 3.74 && out < 3.76, "divider at 3.75 V, got {out}");
}

#[test]
fn injection_to_an_unknown_parent_fails_loud() {
    // PLG-15: no silent drop — a non-existent parent module is an error at
    // staging time.
    let host = host_with(vec![injector("NoSuchModule", "r_par", "PluginResistor", 1e3)]);
    let msg = run_top_op(host, DEVICE_DIVIDER).expect_err("must fail").to_string();
    assert!(
        msg.contains("NoSuchModule") && msg.contains("not found"),
        "unexpected message: {msg}"
    );
}

#[test]
fn injection_of_an_undeclared_module_fails_loud() {
    // PLG-15 / no-netlist-magic (Part VI §2): the staged module type was
    // never declared in the design.
    let host = host_with(vec![injector("Top", "r_par", "Varistor", 1e3)]);
    let msg = run_top_op(host, DEVICE_DIVIDER).expect_err("must fail").to_string();
    assert!(msg.contains("not declared"), "unexpected message: {msg}");
}

#[test]
fn authored_structure_is_never_overwritten() {
    // MD-25: staging the authored instance's own label (`r1`) must fail
    // loud — the injection is a side artifact, never a rewrite of what the
    // author wrote.
    let host = host_with(vec![injector("Top", "r1", "PluginResistor", 1e3)]);
    let msg = run_top_op(host, DEVICE_DIVIDER).expect_err("must fail").to_string();
    assert!(
        msg.contains("already has an instance") && msg.contains("r1"),
        "unexpected message: {msg}"
    );
}
