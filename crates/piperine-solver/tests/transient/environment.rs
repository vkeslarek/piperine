//! The environment in reactions and discrete functions on the solver (feature
//! `solver-crossings`, ENV-03; docs/spec Part IV §12.1.12, §14.4; abi.md §4.2): a reaction reads
//! `temperature()` through its `GLOBAL TEMPERATURE` pin and `gmin()`, `source_scale()` as the
//! ambient inputs `GMIN` and `SOURCE_SCALE`, which hold the session's `gmin` and 1.0 at every
//! instant, in the operating point and in the transient; a discrete function reads the same.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::partials::PartialsCheck;
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, Statement, ambient, cell, global, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions};

const ORIGIN: &str = "environment.rs";
/// The host's temperature and the session's `gmin` of every test here.
const KELVIN: f64 = 350.0;
const GMIN: f64 = 3.0e-12;

thread_local! {
    /// `(temperature, gmin, source_scale)` as each reaction call read them.
    static READ: RefCell<Vec<[f64; 3]>> = const { RefCell::new(Vec::new()) };
}

/// Reads the environment.
/// pins: t=0 (GLOBAL TEMPERATURE F64); fg=1, fs=2, rt=3, rg=4, rs=5 (PORT F64);
///       start=6 (GLOBAL START); go=7 (PORT event)
/// fn 0 "env": reads [GMIN, SOURCE_SCALE] writes [fg, fs]
/// reaction 0 "env": on [start, go] reads [t, GMIN, SOURCE_SCALE] writes [S0 {rt}, S1 {rg}, S2 {rs}]
struct EnvProbe;
static PROBE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static PROBE: OnceLock<Module> = OnceLock::new();

impl Device for EnvProbe {
    fn prototype() -> &'static Shape<'static> {
        PROBE_SHAPE.get_or_init(EnvironmentCases::probe_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((EnvProbe, Arc::clone(PROBE_SHAPE.get_or_init(EnvironmentCases::probe_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (g, s) = (cx.f64(0), cx.f64(1));
        cx.set(0, g);
        cx.set(1, s);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let read = [cx.f64(0), cx.f64(1), cx.f64(2)];
        READ.with(|r| r.borrow_mut().push(read));
        for (k, v) in read.into_iter().enumerate() {
            cx.commit(k as u32, &v.to_le_bytes(), 0.0)?;
        }
        Ok(Flow::Continue)
    }
}

/// Every function of this target's own devices passes the partials verifier (the guard of
/// `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    PartialsCheck::verify_every_function("EnvProbe", EnvironmentCases::probe());
    PartialsCheck::verify_every_function("GminLoad", EnvironmentCases::gmin_load());
    PartialsCheck::verify_every_function("FlagProbe", EnvironmentCases::flag_probe());
}

#[test]
fn a_reaction_reads_temperature_gmin_and_source_scale() {
    // ENV-03 AC 3 and 4: in the operating point (START) and in the transient (go at 100 ns) the
    // reaction reads the host's 350 K, the session's 3e-12 S and 1.0, and commits them
    let mut solver = EnvironmentCases::solver();
    solver.dc().expect("dc").solve().expect("the operating point");
    assert_eq!(READ.with(|r| r.borrow().clone()), vec![[KELVIN, GMIN, 1.0]], "START in the operating point");
    assert_eq!([EnvironmentCases::carrier(&solver, "rt"), EnvironmentCases::carrier(&solver, "rg"), EnvironmentCases::carrier(&solver, "rs")], [KELVIN, GMIN, 1.0]);
    READ.with(|r| r.borrow_mut().clear());
    solver.tran().expect("tran").solve().expect("the transient");
    assert_eq!(
        READ.with(|r| r.borrow().clone()),
        vec![[KELVIN, GMIN, 1.0], [KELVIN, GMIN, 1.0]],
        "START at the transient's operating point, then go at 100 ns"
    );
}

#[test]
fn a_discrete_function_reads_gmin_and_source_scale() {
    // ENV-03 AC 3: the discrete function writes GMIN and SOURCE_SCALE to fg and fs
    let mut solver = EnvironmentCases::solver();
    solver.dc().expect("dc").solve().expect("the operating point");
    assert_eq!([EnvironmentCases::carrier(&solver, "fg"), EnvironmentCases::carrier(&solver, "fs")], [GMIN, 1.0]);
    solver.tran().expect("tran").solve().expect("the transient");
    assert_eq!([EnvironmentCases::carrier(&solver, "fg"), EnvironmentCases::carrier(&solver, "fs")], [GMIN, 1.0]);
}

/// Draws `gmin · 1e9` amperes out of `p`, `gmin` its ambient `GMIN`: into 1 kΩ, v(p) = gmin · 1e12.
/// pins: p=0 (PORT continuous);  terms: T0 FLOW{p, GROUND} "i";  fn 0: reads [p, GMIN] writes [T0]
struct GminLoad;
static LOAD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LOAD: OnceLock<Module> = OnceLock::new();

impl Device for GminLoad {
    fn prototype() -> &'static Shape<'static> {
        LOAD_SHAPE.get_or_init(EnvironmentCases::load_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((GminLoad, Arc::clone(LOAD_SHAPE.get_or_init(EnvironmentCases::load_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let gmin = cx.f64(1);
        cx.set(0, -gmin * 1.0e9);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn analog_and_reactions_read_one_gmin_at_the_transient_start() {
    // ENV-03 AC 3 (verification finding): the transient's operating point solves with the
    // session's context, so an analog function reads the session's GMIN there, as the reaction
    // of that operating point does. gmin = 3e-12 S drawn as gmin·1e9 A into 1 kΩ: v(out) = 3 V at
    // t = 0 and after (the default 1e-12 S would give 1 V at t = 0).
    let mut b = CircuitBuilder::new("gmin");
    let gnd = b.ground();
    let out = b.node("out");
    CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &out, &gnd);
    b.device(EnvironmentCases::gmin_load(), "g", &[], &[Port::Node(out.clone())]);
    b.device(mixed::Delay::module(), "kick", &[("at", ParamValue::Real(1.0e-7))], &[Port::Net("go".into())]);
    let ports: Vec<Port> = ["fg", "fs", "rt", "rg", "rs", "go"].map(|n| Port::Net(n.into())).into();
    b.device(EnvironmentCases::probe(), "env", &[], &ports);
    let mut context = Context::default();
    context.tolerances.gmin = GMIN;
    let mut solver = Solver::new(b.build().expect("build"))
        .with_context(context)
        .with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8))
        .build();
    READ.with(|r| r.borrow_mut().clear());
    let result = solver.tran().expect("tran").solve().expect("the transient");
    let read_at_start = READ.with(|r| r.borrow()[0][1]);
    assert_eq!(read_at_start, GMIN, "the reaction of the transient's operating point");
    let id = CircuitBuilding::node_id(&out);
    for s in result.iter() {
        let v = s.get_node(&id).expect("v(out)");
        assert!((v - read_at_start * 1.0e12).abs() < 1.0e-9, "v(out) = {v} at t = {:e}: gmin as the analog side read it", s.time());
    }
    assert_eq!(result.iter().next().expect("t = 0").time(), 0.0);
}

// ─── The analysis flags of an operating point (FIX-11) ──────────────────────

thread_local! {
    /// `[dc, tran, ac, noise]` as each `START` reaction of `FlagProbe` read its global pins.
    static FLAGS: RefCell<Vec<[bool; 4]>> = const { RefCell::new(Vec::new()) };
}

/// Reads the analysis globals in a reaction.
/// pins: dc=0, tran=1, ac=2, noise=3 (GLOBAL BIT); start=4 (GLOBAL START)
/// reaction 0 "flags": on [start] reads [dc, tran, ac, noise]
struct FlagProbe;
static FLAG_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLAG_PROBE: OnceLock<Module> = OnceLock::new();

impl Device for FlagProbe {
    fn prototype() -> &'static Shape<'static> {
        FLAG_SHAPE.get_or_init(EnvironmentCases::flag_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlagProbe, Arc::clone(FLAG_SHAPE.get_or_init(EnvironmentCases::flag_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let read = [0, 1, 2, 3].map(|i| cx.bytes(i) == [1]);
        FLAGS.with(|f| f.borrow_mut().push(read));
        Ok(Flow::Continue)
    }
}

#[test]
fn an_ac_operating_point_carries_the_ac_flag() {
    // FIX-11 AC 3 (§11.1.1): the operating point of an AC analysis is `analysis(Ac)`, not
    // `analysis(Dc)`; `START` fires once there
    let (mut circuit, _) = EnvironmentCases::flag_circuit();
    circuit.ac(Context::default()).expect("ac");
    assert_eq!(FLAGS.with(|f| f.borrow().clone()), vec![[false, false, true, false]]);
}

#[test]
fn a_noise_operating_point_carries_the_noise_flag() {
    // FIX-11 AC 4: the operating point of a noise analysis is `analysis(Noise)`
    let (mut circuit, mid) = EnvironmentCases::flag_circuit();
    let options = piperine_solver::prelude::NoiseAnalysisOptions {
        sweep_options: piperine_solver::prelude::AcSweepAnalysisOptions { start_frequency: 1.0e3, stop_frequency: 1.0e5, steps: 5, logarithmic: true },
        output_node: mid,
        reference_node: piperine_solver::prelude::NodeIdentifier::Gnd,
        input_source_name: None,
        output_flow: None,
    };
    circuit.noise(options, Context::default()).expect("noise");
    assert_eq!(FLAGS.with(|f| f.borrow().clone()), vec![[false, false, false, true]]);
}

#[test]
fn a_transient_operating_point_carries_the_tran_flag() {
    // FIX-11 AC 2: the operating point of a transient is `analysis(Transient)`; the plain operating
    // point stays `analysis(Dc)`
    let (circuit, _) = EnvironmentCases::flag_circuit();
    let mut solver = Solver::new(circuit).with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8)).build();
    solver.tran().expect("tran").solve().expect("the transient");
    assert_eq!(FLAGS.with(|f| f.borrow().clone()), vec![[false, true, false, false]]);
    let (circuit, _) = EnvironmentCases::flag_circuit();
    let mut solver = Solver::new(circuit).build();
    solver.dc().expect("dc").solve().expect("the operating point");
    assert_eq!(FLAGS.with(|f| f.borrow().clone()), vec![[true, false, false, false]]);
}

/// The helpers of the environment tests.
pub struct EnvironmentCases;

impl EnvironmentCases {
    /// The probe shape helper `probe_shape`.
    fn probe_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let t = s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("t", role::GLOBAL, &F64) });
        let [fg, fs, rt, rg, rs] = ["fg", "fs", "rt", "rg", "rs"].map(|n| s.pin(PinSpec::discrete(n, role::PORT, &F64)));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let go = s.pin(PinSpec::event("go", role::PORT));
        let amb = |index| Cell { kind: cell::AMBIENT, index, part: part::VALUE };
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "env",
            origin: ORIGIN,
            reads: vec![amb(ambient::GMIN), amb(ambient::SOURCE_SCALE)],
            writes: vec![pin(fg), pin(fs)],
            ..FunctionSpec::default()
        });
        let st = |pin| Statement { pin, offset: 0, len: 8, flags: 0, min_after: 0.0 };
        s.reaction(ReactionSpec {
            name: "env",
            origin: ORIGIN,
            on: vec![start, go],
            reads: vec![pin(t), amb(ambient::GMIN), amb(ambient::SOURCE_SCALE)],
            writes: vec![st(rt), st(rg), st(rs)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The probe helper `probe`.
    fn probe() -> &'static Module {
        PROBE.get_or_init(|| module::<EnvProbe>("EnvProbe"))
    }

    /// A divider (so the circuit has analog unknowns) and the probe, its `go` kicked at 100 ns; the
    /// session at `KELVIN` with `gmin = GMIN`.
    fn solver() -> Solver {
        let mut b = CircuitBuilder::new("environment");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
        CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
        b.device(mixed::Delay::module(), "kick", &[("at", ParamValue::Real(1.0e-7))], &[Port::Net("go".into())]);
        let ports: Vec<Port> = ["fg", "fs", "rt", "rg", "rs", "go"].map(|n| Port::Net(n.into())).into();
        b.device(EnvironmentCases::probe(), "env", &[], &ports);
        let mut context = Context::default();
        context.tolerances.temperature = KELVIN;
        context.tolerances.gmin = GMIN;
        Solver::new(b.build().expect("build"))
            .with_context(context)
            .with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8))
            .build()
    }

    /// The `F64` net `name` holds.
    fn carrier(solver: &Solver, name: &str) -> f64 {
        let bound = solver.circuit().bound();
        let net = bound.names.nets[&format!("net:{name}")];
        f64::from_le_bytes(bound.digital.read(net).try_into().expect("an F64"))
    }

    /// The load shape helper `load_shape`.
    fn load_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![Cell { kind: cell::PIN, index: p, part: part::VALUE }, Cell { kind: cell::AMBIENT, index: ambient::GMIN, part: part::VALUE }],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The gmin load helper `gmin_load`.
    fn gmin_load() -> &'static Module {
        LOAD.get_or_init(|| module::<GminLoad>("GminLoad"))
    }

    /// The flag shape helper `flag_shape`.
    fn flag_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let flags = [("dc", global::DC), ("tran", global::TRAN), ("ac", global::AC), ("noise", global::NOISE)]
            .map(|(name, kind)| s.pin(PinSpec { global: kind, ..PinSpec::discrete(name, role::GLOBAL, &piperine_abi::layout::BIT) }));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        s.reaction(ReactionSpec { name: "flags", origin: ORIGIN, on: vec![start], reads: flags.map(pin).to_vec(), ..ReactionSpec::default() });
        s.freeze()
    }

    /// The flag probe helper `flag_probe`.
    fn flag_probe() -> &'static Module {
        FLAG_PROBE.get_or_init(|| module::<FlagProbe>("FlagProbe"))
    }

    /// A divider and the flag probe.
    fn flag_circuit() -> (piperine_solver::prelude::CircuitInstance, piperine_solver::prelude::NodeIdentifier) {
        let mut b = CircuitBuilder::new("flags");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
        CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
        b.device(EnvironmentCases::flag_probe(), "flags", &[], &[]);
        FLAGS.with(|f| f.borrow_mut().clear());
        (b.build().expect("build"), CircuitBuilding::node_id(&mid))
    }
}
