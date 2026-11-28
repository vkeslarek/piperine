//! The compiled digital bodies of the buck slice in a circuit (feature `ir-rust`, RS-05, RS-12,
//! RS-01 AC 1; checks C98, C99): the oscillator drives the flip-flop through a design net and
//! the flip-flop's output is read by its net name on the solver; the ten bodies of the slice
//! compile together with the compiler's standard error empty. Expected values are written from
//! the MSL the bodies transcribe (`dpwm.msl`, `lib.msl`), not from the printed source. A parent's
//! `signal` its children's carrier ports are bound to is the net they meet on, and a net starts
//! at its own `init` (msl-digital T65, checks C175, C176; `ir-rust` F-B1, F-B2, whose refusals
//! these runs replace).
#![cfg(unix)]

use std::sync::Arc;

use piperine_abi::layout::{HIGH, LOW};
use piperine_ir::{
    BodyId, BodySpec, Carrier, CarrierRole, Child, Constant, Event, EventSource, InstanceSpec, Ir, IrBuilder, NetSpec, Origin, Reaction,
    UnaryOp, ValueType,
};
use piperine_plan::{Modules, Netlist};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};
use piperine_transpile::{Settings, Transpiler};

use piperine_ir_fixtures::digital;
use piperine_ir_fixtures::alone;

use alone::Alone;
use digital::Buck;

/// The designs of the carrier-port tests.
struct CircuitDesigns;

impl CircuitDesigns {
    fn at() -> Origin {
        Origin { file: "top.msl".to_string(), line: 1, col: 1 }
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
        let mut b = IrBuilder::new();
        let osc = Buck::osc(&mut b);
        let dff = Buck::dff(&mut b);
        let top_body = b.body(BodySpec {
            name: "Top".into(),
            origin: CircuitDesigns::at(),
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
            origin: CircuitDesigns::at(),
        });
        let place = |b: &mut IrBuilder, name: &str, body: BodyId, params: Vec<Constant>| {
            b.instance(InstanceSpec {
                path: format!("Top.{name}"),
                name: name.into(),
                body,
                parent: Some(top),
                nodes: vec![],
                params,
                origin: CircuitDesigns::at(),
            })
        };
        let osc_instance = place(&mut b, "osc", osc, vec![Constant::Real(piperine_ir::Real::new(200e6))]);
        let ff_instance = place(&mut b, "ff", dff, vec![]);
        let net = |b: &mut IrBuilder, path: &str| {
            b.net(NetSpec { path: path.into(), ty: ValueType::Bool, declared_by: top, init: None, origin: CircuitDesigns::at() })
        };
        let clk = net(&mut b, "Top.clk");
        let q = net(&mut b, "Top.q");
        let d = if d_on_the_clock { clk } else { net(&mut b, "Top.d") };
        // Osc: clk; Dff: clk, d, q
        b.bind_nets(osc_instance, vec![clk]).unwrap();
        b.bind_nets(ff_instance, vec![clk, d, q]).unwrap();
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        ir
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

    /// The flip-flop clocked by a `signal` of its parent:
    ///
    /// ```text
    /// module Top() {
    ///     signal clk: Bit = <init>;
    ///     ff = Dff(clk: clk, d: clk, q: q);         // `bound`; else ff's clk and d are on a net of their own
    /// }
    /// digital for Top { on clk.changed() { clk <- !clk after 2.5n; } }
    /// ```
    ///
    /// The top body records the placement and what its carrier ports are bound to
    /// (`Body::child_carriers`, as the elaborator does for every placement), and `Top.clk` is the
    /// net its signal declares, with the signal's initial value as its own (msl-digital T28).
    fn parent_clocked(bound: bool, init: Option<bool>) -> Ir {
        let mut b = IrBuilder::new();
        let dff = Buck::dff(&mut b);
        let top_body = b.body(BodySpec {
            children: vec![Child { name: "ff".into(), body: dff, nodes: vec![], params: vec![], origin: CircuitDesigns::at() }],
            ..Buck::spec("Top", "top.msl", 1)
        });
        let init = init.map(Constant::Bool);
        b.carrier(top_body, Carrier { name: "clk".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init, origin: CircuitDesigns::at(), count: None });
        // the ports of `Dff` are clk, d, q: clk and d on the signal, q on a net of the top
        let bindings = if bound { vec![Some(0), Some(0), None] } else { vec![None, None, None] };
        b.bind_child_carriers(top_body, 0, bindings);
        let changed = b.event(top_body, Event { source: EventSource::Changed(0), guard: None, origin: CircuitDesigns::at() });
        let mut x = digital::Dx::new(&mut b, "top.msl", 6, 40);
        let clk = x.carrier(0, ValueType::Bool);
        let toggled = x.un(UnaryOp::Not, clk, ValueType::Bool);
        let half = x.real(2.5e-9);
        let commit = x.commit(0, toggled, Some(half));
        b.reaction(top_body, Reaction { on: vec![changed], stmts: vec![commit], locals: vec![], origin: CircuitDesigns::at() });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![], params: vec![], origin: CircuitDesigns::at() });
        let ff = b.instance(InstanceSpec { path: "Top.ff".into(), name: "ff".into(), body: dff, parent: Some(top), nodes: vec![], params: vec![], origin: CircuitDesigns::at() });
        let net = |b: &mut IrBuilder, path: &str, init: Option<Constant>| {
            b.net(NetSpec { path: path.into(), ty: ValueType::Bool, declared_by: top, init, origin: CircuitDesigns::at() })
        };
        let clk = net(&mut b, "Top.clk", init);
        let q = net(&mut b, "Top.q", None);
        let ff_clk = if bound { clk } else { net(&mut b, "Top.ff_clk", None) };
        b.bind_nets(top, vec![clk]).unwrap();
        b.bind_nets(ff, vec![ff_clk, ff_clk, q]).unwrap();
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        ir
    }
}

/// A design run under cached settings.
struct CircuitRun;

impl CircuitRun {
    fn settings(cache: &tempfile::TempDir) -> Settings {
        Settings { rustc: None, cache: cache.path().to_path_buf() }
    }

    /// Compiles `ir`, runs the circuit for 30 ns and returns the bytes of the net `Top.q`.
    fn q_after(ir: Ir) -> Vec<u8> {
        let ir = Arc::new(ir);
        let cache = tempfile::tempdir().expect("a temp dir");
        let compiled = Transpiler::new(ir.clone())
            .unwrap_or_else(|e| panic!("{e}"))
            .compile(&CircuitRun::settings(&cache))
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
        let bound = circuit.bound();
        let net = *bound
            .names
            .nets
            .get("net:Top.q")
            .unwrap_or_else(|| panic!("no net `Top.q`: {:?}", bound.names.nets.keys().collect::<Vec<_>>()));
        bound.digital.read(net).to_vec()
    }
}

#[test]
fn carrier_ports_bind_to_design_nets() {
    // dpwm.msl:10 the oscillator runs, and lib.msl's flip-flop takes `d` (the same net as its
    // clock) high at a rising edge: `q`, read by its net name, is true
    assert_eq!(CircuitRun::q_after(CircuitDesigns::oscillator_into_flip_flop(true)), HIGH, "q holds the level d had at the first rising edge");
    // the binding is what carries it: with `d` on a net nothing writes, the flip-flop samples an
    // unknown and `q` is never true
    assert_ne!(CircuitRun::q_after(CircuitDesigns::oscillator_into_flip_flop(false)), HIGH, "an undriven d gives no true q");
}

#[test]
fn every_buck_slice_body_compiles_with_no_warning() {
    let (ir, names) = CircuitDesigns::buck_slice();
    let ir = Arc::new(ir);
    let cache = tempfile::tempdir().expect("a temp dir");
    let compiled = Transpiler::new(ir.clone())
        .unwrap_or_else(|e| panic!("{e}"))
        .compile(&CircuitRun::settings(&cache))
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

#[test]
fn a_parent_signal_reaches_its_childrens_ports() {
    // part II §5: a parent's `signal` is the net its children's ports meet on, so the parent's
    // commits reach them. The parent toggles clk every 2.5 ns from false, so its first rise is
    // at 2.5 ns, where the flip-flop samples d (clk itself, high): q is true from then on
    assert_eq!(CircuitRun::q_after(CircuitDesigns::parent_clocked(true, Some(false))), HIGH, "q holds the level d had at the first rising edge");
    // the binding is what carries it: with ff's clk and d on a net of their own, nothing writes
    // them, the flip-flop sees no edge and q never leaves its initial value (`q: Bit = false`,
    // abi.md §11.4: q is `init LOW`)
    assert_eq!(CircuitRun::q_after(CircuitDesigns::parent_clocked(false, Some(false))), LOW, "q is never true with the signal bound to no child");
    // the signal is a PORT pin of the top's shape, and the shape is valid (abi.md §3.1: a PORT is
    // bound by the netlist)
    let ir = CircuitDesigns::parent_clocked(true, Some(false));
    let plan = piperine_plan::BodyPlan::build(ir.top().body()).expect("the body plans");
    let shape = plan.shape().shape();
    let clk = shape.pins.iter().find(|p| p.name.as_str() == "clk").expect("the signal's pin");
    assert_eq!(clk.role, piperine_abi::raw::role::PORT);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_net_starts_at_its_own_init() {
    use piperine_digital_solver::engine::{Horizon, Mode};
    use piperine_digital_solver::tick::Tick;
    // part II §5: a net's `init` is its value before any write. At (0, 0) the parent's
    // reaction has only scheduled its first toggle (2.5 ns later): the net holds its init
    let clk_at_start = |init: Option<bool>| {
        let ir = Arc::new(CircuitDesigns::parent_clocked(true, init));
        let cache = tempfile::tempdir().expect("a temp dir");
        let compiled = Transpiler::new(ir.clone()).unwrap_or_else(|e| panic!("{e}")).compile(&CircuitRun::settings(&cache)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(compiled.report().warnings, "");
        let mut circuit = Netlist::new(&ir).circuit(&compiled).unwrap_or_else(|e| panic!("{e}"));
        let bound = circuit.bound_mut();
        let net = *bound.names.nets.get("net:Top.clk").unwrap_or_else(|| panic!("no net `Top.clk`"));
        bound.digital.start().unwrap_or_else(|e| panic!("{e:?}"));
        bound.digital.run_instant(Tick(0), &[], Horizon::uniform(Tick(0)), Mode::default()).unwrap_or_else(|e| panic!("{e:?}"));
        bound.digital.read(net).to_vec()
    };
    assert_eq!(clk_at_start(Some(false)), LOW, "the net's init, false");
    assert_eq!(clk_at_start(Some(true)), HIGH, "the net's init, true");
    // with no init the net holds its zero bytes at (0, 0) (abi.md P3)
    assert_eq!(clk_at_start(None), [0]);
}
