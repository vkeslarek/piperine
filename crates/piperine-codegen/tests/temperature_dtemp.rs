//! Temperature protocol contract tests (ABI-21/22): per-instance `dtemp`
//! composition by the `PiperineDevice` override + the codegen-side cache.

use std::collections::HashMap;

use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::{AnalogKernel, CompiledModule};
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{
    AnalogDevice, AnalogReference, DcAnalysisState, DigitalDevice, Element, ElementCapabilities,
    Introspect, Netlist, NodeIdentifier, Stamp,
};
use piperine_solver::prelude::Context;

/// A module that declares `dtemp` (the SPICE convention) so the codegen
/// device exposes it as an instance param the override composes against.
const DTEMP_MODULE: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod TempRes (inout p : Electrical, inout n : Electrical) {
        param r    : Real = 1.0e3;
        param dtemp: Real = 0.0;
    }
    analog TempRes { I(p, n) <+ V(p, n) / r; }
"#;

fn compile_module(src: &str, name: &str) -> (std::sync::Arc<AnalogKernel>, LoweredBody) {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("fixture elaborates");
    let bodies: HashMap<String, LoweredBody> =
        piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let body = bodies.get(name).expect("module").clone();
    let compiled = CompiledModule::compile(&body).expect("compile");
    let kernel = compiled.analog().expect("analog").clone();
    (kernel, body)
}

/// ABI-21: a `PiperineDevice` with `dtemp = 10` composes the effective
/// temperature `t_nominal + dtemp` inside its `set_temperature` override —
/// the override receives the ambient `t_nominal` (passed by the solver's
/// setup path or a host sweep) and caches `t_nominal + dtemp` on the analog
/// instance.
#[test]
fn dtemp_instance_composes_into_effective_temperature() {
    use piperine_codegen::device::PiperineDevice;
    let (kernel, _body) = compile_module(DTEMP_MODULE, "TempRes");

    let mut netlist = Netlist::new();
    let terms = vec![NodeIdentifier::Anonymous(1), NodeIdentifier::Anonymous(2)];
    // dtemp = 10 (the second param after r = 1e3).
    let a_inst = piperine_codegen::device::AnalogInstance::new(
        "r1",
        kernel,
        &terms,
        vec![1000.0, 10.0],
        1,
        &mut netlist,
    )
    .expect("instance builds");

    assert_eq!(a_inst.param("dtemp"), Some(10.0));

    let mut dev = PiperineDevice::new("r1", Some(a_inst), None);

    // The override receives the AMBIENT temperature (the value the solver
    // passes); it composes `t_nominal + dtemp` internally.
    let tnom = 300.15;
    let dtemp = 10.0;
    dev.set_temperature(tnom);

    let cached = dev
        .analog()
        .and_then(|a| a.cached_temperature())
        .expect("override cached an effective temperature");
    assert!(
        (cached - (tnom + dtemp)).abs() < 1e-9,
        "cached effective temperature = {cached}, want {} (tnom {tnom} + dtemp {dtemp})",
        tnom + dtemp
    );
}

/// ABI-21: a `PiperineDevice` with no `dtemp` param caches the received
/// ambient temperature unchanged (no composition to do — `dtemp` defaults
/// to 0 through `param("dtemp").unwrap_or(0.0)`).
#[test]
fn no_dtemp_param_caches_received_temperature() {
    use piperine_codegen::device::PiperineDevice;
    let (kernel, _body) = compile_module(
        r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod R (inout p : Electrical, inout n : Electrical) {
            param r : Real = 1.0e3;
        }
        analog R { I(p, n) <+ V(p, n) / r; }
    "#,
        "R",
    );

    let mut netlist = Netlist::new();
    let terms = vec![NodeIdentifier::Anonymous(1), NodeIdentifier::Anonymous(2)];
    let a_inst =
        piperine_codegen::device::AnalogInstance::new("r1", kernel, &terms, vec![1000.0], 1, &mut netlist)
            .expect("instance builds");

    assert!(a_inst.param("dtemp").is_none(), "no dtemp param declared");

    let mut dev = PiperineDevice::new("r1", Some(a_inst), None);
    let tnom = 325.0;
    dev.set_temperature(tnom);
    let cached = dev
        .analog()
        .and_then(|a| a.cached_temperature())
        .expect("cache populated");
    assert!(
        (cached - tnom).abs() < 1e-9,
        "device with no dtemp caches the ambient value {tnom}, got {cached}"
    );
}

/// ABI-21 (AnalogInstance unit): the analog-instance cache records exactly
/// the effective value the override forwards, and is `None` until
/// `set_temperature` runs — the device reads `$temperature` at eval time
/// before the seam is seeded.
#[test]
fn analog_instance_caches_effective_temperature() {
    let (kernel, _body) = compile_module(DTEMP_MODULE, "TempRes");

    let mut netlist = Netlist::new();
    let terms = vec![NodeIdentifier::Anonymous(1), NodeIdentifier::Anonymous(2)];
    let mut a_inst = piperine_codegen::device::AnalogInstance::new(
        "r1",
        kernel,
        &terms,
        vec![1000.0, 10.0],
        1,
        &mut netlist,
    )
    .expect("instance builds");

    assert!(a_inst.cached_temperature().is_none(), "cache empty before set_temperature");

    let t_eff = 310.15;
    a_inst.set_temperature(t_eff);
    let cached = a_inst.cached_temperature().expect("cache populated");
    assert!((cached - t_eff).abs() < 1e-9, "cache holds the effective value");
}

/// ABI-22: the `Invalidation` returned by `CircuitInstance::set_temperature`
/// is `Temperature` (the structure shape stays untouched at a higher level
/// than restamp). A structural `Rebuild` from a temperature change is
/// impossible by construction — temperature recomputes constants only, so
/// the analysis can always honor it via restamp. A `Rebuild` outcome would
/// ride the same fail-loud path as a parameter `Rebuild` (surfaced by
/// `set_param`); the solver never silently no-ops.
#[test]
fn set_temperature_returns_temperature_invalidation() {
    struct TempDevice {
        received: Option<f64>,
    }
    impl AnalogDevice for TempDevice {
        fn set_temperature(&mut self, t: f64) {
            self.received = Some(t);
        }
        fn load_dc(
            &mut self,
            _s: &DcAnalysisState<'_>,
            _c: &Context,
        ) -> Vec<Stamp<AnalogReference, f64>> {
            Vec::new()
        }
    }
    impl DigitalDevice for TempDevice {}
    impl Introspect for TempDevice {}
    impl Element for TempDevice {
        fn name(&self) -> &str { "td" }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
    }

    let mut netlist = Netlist::new();
    let _ = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let _ = netlist.connect_node(NodeIdentifier::Gnd);
    let elements: Vec<Box<dyn Element>> = vec![Box::new(TempDevice { received: None })];
    let mut circuit =
        piperine_solver::prelude::CircuitInstance::from_devices_and_netlist(
            "inv-check",
            elements,
            netlist,
        );

    // Circuit-wide set_temperature passes the ambient straight through to
    // each element (composition is the device's job); the invalidation is
    // always Temperature — never Rebuild.
    let inv = circuit.set_temperature(300.0);
    assert_eq!(
        inv,
        piperine_solver::prelude::Invalidation::Temperature,
        "circuit-wide set_temperature returns Temperature invalidation (never Rebuild)"
    );
}
