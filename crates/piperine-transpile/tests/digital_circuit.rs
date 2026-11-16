//! The compiled digital bodies of the buck slice in a circuit (feature `ir-rust`, RS-05, RS-12,
//! RS-01 AC 1; checks C98, C99): the oscillator drives the flip-flop through a design net and
//! the flip-flop's output is read by its net name on the solver; the ten bodies of the slice
//! compile together with the compiler's standard error empty. Expected values are written from
//! the MSL the bodies transcribe (`dpwm.msl`, `lib.msl`), not from the printed source. The
//! wiring the netlist cannot give the solver yet is refused by name (checks C106, C107;
//! verification F-B1, F-B2).
#![cfg(unix)]

use std::sync::Arc;

use piperine_abi::layout::HIGH;
use piperine_abi::raw::Module;
use piperine_ir::{
    BodyId, BodySpec, Carrier, CarrierRole, Constant, InstanceSpec, Ir, IrBuilder, NetSpec, Origin, ValueType,
};
use piperine_plan::{Modules, Netlist};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};
use piperine_transpile::{Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/alone.rs"]
mod alone;

use alone::Alone;
use digital::Buck;

fn at() -> Origin {
    Origin { file: "top.msl".to_string(), line: 1, col: 1 }
}

fn settings(cache: &tempfile::TempDir) -> Settings {
    Settings { rustc: None, cache: cache.path().to_path_buf() }
}

/// The oscillator and the flip-flop under one top, wired by two design nets:
///
/// ```text
/// top.msl:  osc = Osc();  ff = Dff(clk: osc.clk, d: osc.clk, q: q);
/// ```
///
/// The flip-flop's data input is the oscillator's own output, so at the first rising edge the
/// flip-flop samples a high level: `q` is true from then on. With `d_on_the_clock` false, `d`
/// is on a net of its own that nothing writes: unknown, so `q` never becomes true.
fn oscillator_into_flip_flop(d_on_the_clock: bool) -> Ir {
    oscillator_into_flip_flop_with(d_on_the_clock, Wiring::Ports)
}

/// How the design of `oscillator_into_flip_flop_with` holds its clock net.
#[derive(Clone, Copy, PartialEq)]
enum Wiring {
    /// `Top.clk` is a net of the top, met by the children's ports only.
    Ports,
    /// The top body declares `signal clk: Bit` and binds it to `Top.clk`, the net its children's
    /// ports meet on.
    ParentSignal,
    /// `Top.clk` has its own initial value, `false`.
    NetInit,
}

fn oscillator_into_flip_flop_with(d_on_the_clock: bool, wiring: Wiring) -> Ir {
    let mut b = IrBuilder::new();
    let osc = Buck::osc(&mut b);
    let dff = Buck::dff(&mut b);
    let top_body = b.body(BodySpec {
        name: "Top".into(),
        origin: at(),
        params: vec![],
        nodes: vec![],
        branches: vec![],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: None,
        children: vec![],
    });
    let top = b.instance(InstanceSpec {
        path: "Top".into(),
        name: "Top".into(),
        body: top_body,
        parent: None,
        nodes: vec![],
        params: vec![],
        origin: at(),
    });
    let place = |b: &mut IrBuilder, name: &str, body: BodyId, params: Vec<Constant>| {
        b.instance(InstanceSpec {
            path: format!("Top.{name}"),
            name: name.into(),
            body,
            parent: Some(top),
            nodes: vec![],
            params,
            origin: at(),
        })
    };
    let osc_instance = place(&mut b, "osc", osc, vec![Constant::Real(piperine_ir::Real::new(200e6))]);
    let ff_instance = place(&mut b, "ff", dff, vec![]);
    let net = |b: &mut IrBuilder, path: &str, init: Option<Constant>| {
        b.net(NetSpec { path: path.into(), ty: ValueType::Bool, declared_by: top, init, origin: at() })
    };
    let clk_init = (wiring == Wiring::NetInit).then_some(Constant::Bool(false));
    let clk = net(&mut b, "Top.clk", clk_init);
    if wiring == Wiring::ParentSignal {
        b.carrier(
            top_body,
            Carrier { name: "clk".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: None, origin: at() },
        );
        b.bind_nets(top, vec![clk]);
    }
    let q = net(&mut b, "Top.q", None);
    let d = if d_on_the_clock { clk } else { net(&mut b, "Top.d", None) };
    // Osc: clk; Dff: clk, d, q
    b.bind_nets(osc_instance, vec![clk]);
    b.bind_nets(ff_instance, vec![clk, d, q]);
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the fixture passes check()");
    ir
}

/// Compiles `ir`, runs the circuit for 30 ns and returns the bytes of the net `Top.q`.
fn q_after(ir: Ir) -> Vec<u8> {
    let ir = Arc::new(ir);
    let cache = tempfile::tempdir().expect("a temp dir");
    let compiled = Transpiler::new(ir.clone())
        .unwrap_or_else(|e| panic!("{e}"))
        .compile(&settings(&cache))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(compiled.report().warnings, "");
    // `Osc.clk` and `Dff.clk` meet on the net `Top.clk`: the solver has the nets by the names the
    // IR gave them, and each instance's carrier ports are on them
    let mut circuit = Netlist::new(&ir).circuit(&compiled).unwrap_or_else(|e| panic!("{e}"));
    // 200 MHz: the oscillator toggles every 2.5 ns, so rising edges at 2.5 ns, 7.5 ns, …; 30 ns
    // holds six of them
    circuit
        .transient(TransientAnalysisOptions::new(30e-9, 1e-9), Context::default())
        .unwrap_or_else(|e| panic!("{e}"))
        .solve()
        .unwrap_or_else(|e| panic!("{e}"));
    let bound = circuit.bound().expect("a bound circuit");
    let net = *bound
        .names
        .nets
        .get("net:Top.q")
        .unwrap_or_else(|| panic!("no net `Top.q`: {:?}", bound.names.nets.keys().collect::<Vec<_>>()));
    bound.digital.read(net).to_vec()
}

#[test]
fn carrier_ports_bind_to_design_nets() {
    // dpwm.msl:10 the oscillator runs, and lib.msl's flip-flop takes `d` (the same net as its
    // clock) high at a rising edge: `q`, read by its net name, is true
    assert_eq!(q_after(oscillator_into_flip_flop(true)), HIGH, "q holds the level d had at the first rising edge");
    // the binding is what carries it: with `d` on a net nothing writes, the flip-flop samples an
    // unknown and `q` is never true
    assert_ne!(q_after(oscillator_into_flip_flop(false)), HIGH, "an undriven d gives no true q");
}

/// The ten bodies of the buck slice in one design (`tests/golden/buck`).
fn buck_slice() -> (Ir, Vec<&'static str>) {
    let mut b = IrBuilder::new();
    for make in [
        Buck::osc as fn(&mut IrBuilder) -> BodyId,
        Buck::ref_ramp,
        Buck::adc,
        Buck::comparator,
        Buck::dff,
        Buck::setpoint_reg,
        Buck::dpwm,
        Buck::pid,
        Buck::gate_driver,
        Buck::switch,
    ] {
        make(&mut b);
    }
    let names = vec!["Osc", "RefRamp", "Adc", "A2D", "Dff", "SetpointReg", "Dpwm", "Pid", "GateDriver", "Switch"];
    (Alone::finish(b), names)
}

#[test]
fn every_buck_slice_body_compiles_with_no_warning() {
    let (ir, names) = buck_slice();
    let ir = Arc::new(ir);
    let cache = tempfile::tempdir().expect("a temp dir");
    let compiled = Transpiler::new(ir.clone())
        .unwrap_or_else(|e| panic!("{e}"))
        .compile(&settings(&cache))
        .unwrap_or_else(|e| panic!("{e}"));
    // RS-01 AC 1: the compiler's standard error is empty, so no warning either
    assert_eq!(compiled.report().warnings, "", "rustc standard error");
    for name in names {
        let body = ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body {name}"));
        let module = Modules::module(&compiled, body.id());
        assert!(module.is_some(), "body {name} has a module");
    }
    // the ten bodies and the empty top: eleven bodies, ten of which run
    assert_eq!(ir.bodies().count(), 11);
    assert_eq!(ir.bodies().filter(|b| Modules::module(&compiled, b.id()).is_some()).count(), 10);
}

/// A design whose bodies run nowhere: the netlist's refusals come before any module is asked for.
struct NoModules;

impl Modules for NoModules {
    fn module(&self, _: BodyId) -> Option<&'static Module> {
        None
    }
}

#[test]
fn a_parent_signal_its_children_meet_on_is_refused() {
    // part II §5: a parent's `signal` is the net its children's ports meet on, so the parent's
    // drives and commits reach them; the builder cannot bind a private pin to a named net yet, so
    // the netlist refuses the design by name instead of leaving the parent's pin disconnected
    // (verification F-B1; the binding is msl-digital's)
    let ir = oscillator_into_flip_flop_with(true, Wiring::ParentSignal);
    let error = match Netlist::new(&ir).circuit(&NoModules) {
        Ok(_) => panic!("a parent signal on its children's net gave a circuit"),
        Err(e) => e.to_string(),
    };
    for named in ["signal `clk`", "`Top`", "net `Top.clk`", "`Top.osc`", "not supported yet"] {
        assert!(error.contains(named), "the error names {named}: {error}");
    }
    // the same design with the net held by the children's ports alone is a circuit
    assert!(Netlist::new(&oscillator_into_flip_flop(true)).circuit(&NoModules).is_ok());
}
