//! The environment in reactions and discrete functions on the solver (feature
//! `solver-crossings`, ENV-03; docs/spec Part IV §12.1.12, §14.4; abi.md §4.2): a reaction reads
//! `temperature()` through its `GLOBAL TEMPERATURE` pin and `gmin()`, `source_scale()` as the
//! ambient inputs `GMIN` and `SOURCE_SCALE`, which hold the session's `gmin` and 1.0 at every
//! instant, in the operating point and in the transient; a discrete function reads the same.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::solver_devices::environment::{EnvProbe, FLAGS, FlagProbe, GminLoad, READ};
use piperine_device_fixtures::mixed;

use piperine_abi::raw::Module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver, TransientAnalysisOptions};

/// The host's temperature and the session's `gmin` of every test here.
const KELVIN: f64 = 350.0;
const GMIN: f64 = 3.0e-12;

#[test]
fn a_reaction_reads_temperature_gmin_and_source_scale() {
    // ENV-03 AC 3 and 4: in the operating point (START) and in the transient (go at 100 ns) the
    // reaction reads the host's 350 K, the session's 3e-12 S and 1.0, and commits them
    let mut solver = EnvironmentProbe::solver();
    solver.dc().expect("dc").solve().expect("the operating point");
    assert_eq!(READ.with(|r| r.borrow().clone()), vec![[KELVIN, GMIN, 1.0]], "START in the operating point");
    assert_eq!([EnvironmentProbe::carrier(&solver, "rt"), EnvironmentProbe::carrier(&solver, "rg"), EnvironmentProbe::carrier(&solver, "rs")], [KELVIN, GMIN, 1.0]);
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
    let mut solver = EnvironmentProbe::solver();
    solver.dc().expect("dc").solve().expect("the operating point");
    assert_eq!([EnvironmentProbe::carrier(&solver, "fg"), EnvironmentProbe::carrier(&solver, "fs")], [GMIN, 1.0]);
    solver.tran().expect("tran").solve().expect("the transient");
    assert_eq!([EnvironmentProbe::carrier(&solver, "fg"), EnvironmentProbe::carrier(&solver, "fs")], [GMIN, 1.0]);
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
    b.device(EnvironmentProbe::gmin_load(), "g", &[], &[Port::Node(out.clone())]);
    b.device(mixed::Delay::module(), "kick", &[("at", ParamValue::Real(1.0e-7))], &[Port::Net("go".into())]);
    let ports: Vec<Port> = ["fg", "fs", "rt", "rg", "rs", "go"].map(|n| Port::Net(n.into())).into();
    b.device(EnvironmentProbe::probe(), "env", &[], &ports);
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

#[test]
fn an_ac_operating_point_carries_the_ac_flag() {
    // FIX-11 AC 3 (§11.1.1): the operating point of an AC analysis is `analysis(Ac)`, not
    // `analysis(Dc)`; `START` fires once there
    let (mut circuit, _) = EnvironmentProbe::flag_circuit();
    circuit.ac(Context::default()).expect("ac");
    assert_eq!(FLAGS.with(|f| f.borrow().clone()), vec![[false, false, true, false]]);
}

#[test]
fn a_noise_operating_point_carries_the_noise_flag() {
    // FIX-11 AC 4: the operating point of a noise analysis is `analysis(Noise)`
    let (mut circuit, mid) = EnvironmentProbe::flag_circuit();
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
    let (circuit, _) = EnvironmentProbe::flag_circuit();
    let mut solver = Solver::new(circuit).with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8)).build();
    solver.tran().expect("tran").solve().expect("the transient");
    assert_eq!(FLAGS.with(|f| f.borrow().clone()), vec![[false, true, false, false]]);
    let (circuit, _) = EnvironmentProbe::flag_circuit();
    let mut solver = Solver::new(circuit).build();
    solver.dc().expect("dc").solve().expect("the operating point");
    assert_eq!(FLAGS.with(|f| f.borrow().clone()), vec![[true, false, false, false]]);
}

/// The environment a device reads: its probe, solver, carrier and gmin load.
struct EnvironmentProbe;

impl EnvironmentProbe {

    /// The probe helper `probe`.
    fn probe() -> &'static Module {
        EnvProbe::module()
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
        b.device(EnvironmentProbe::probe(), "env", &[], &ports);
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

    /// The gmin load helper `gmin_load`.
    fn gmin_load() -> &'static Module {
        GminLoad::module()
    }

    /// The flag probe helper `flag_probe`.
    fn flag_probe() -> &'static Module {
        FlagProbe::module()
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
        b.device(EnvironmentProbe::flag_probe(), "flags", &[], &[]);
        FLAGS.with(|f| f.borrow_mut().clear());
        (b.build().expect("build"), CircuitBuilding::node_id(&mid))
    }
}
