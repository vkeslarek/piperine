//! `ddt` off a whole addend, run from MSL source on the Rust backend (feature `analog-structure`,
//! AS-07 and AS-04 AC 1): a factor of a product with a probe, a result read by an event, with an
//! `abstol`, and the held state of §11.2's `Bounded`. Each module is written in
//! `tests/structure/derivative.msl`; every expected value is written from the criterion it cites.
#![cfg(unix)]

use piperine_parity::probe::OutputProbe;
use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::ObservationKind;

use piperine_parity::mixed::BuiltDesign;

/// The model file `tests/structure/derivative.msl`, loaded as a project.
struct DerivativeModels;

impl DerivativeModels {
    fn project() -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structure/derivative.msl");
        Project::new(&path)
    }
}

/// What the derivative benches record and how many steps they take.
struct DerivativeReadings;

impl DerivativeReadings {
    /// The `x` of the one record the watcher of `top` emits at 1 ms of a transient to 2 ms.
    fn derivative_recorded_at_one_millisecond(top: &str) -> f64 {
        let seen = BuiltDesign::of(&DerivativeModels::project(), top).run(2e-3, 0.1e-3);
        let records: Vec<(f64, Vec<u8>)> = seen.all().iter().filter_map(|o| match &o.kind {
            ObservationKind::Record { bytes, .. } => Some((o.time, bytes.clone())),
            _ => None,
        }).collect();
        assert_eq!(records.len(), 1, "one record: {records:?}");
        assert!((records[0].0 - 1e-3).abs() <= 1e-12, "at 1 ms, got {}", records[0].0);
        f64::from_le_bytes(records[0].1[..8].try_into().expect("the field x"))
    }

    /// The accepted steps of the transient of `top` to 10 us with the step cap of the whole run.
    fn accepted_steps(top: &str) -> usize {
        let built = BuiltDesign::of(&DerivativeModels::project(), top);
        let mut options = TransientAnalysisOptions::new(10e-6, 10e-6);
        options.dt_max = 10e-6;
        let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
        let mut solver = Solver::new(circuit).with_tran_opts(options).build();
        solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}")).iter().count()
    }
}

#[test]
fn a_derivative_times_a_probe_is_k_v_dv_dt() {
    // AS-07 AC 2: `i +> k * v(c) * ddt(v(a))` into 1 ohm, k = 1 m, v(c) = 2 V. On a 1 kV/s ramp the
    // current is k * v(c) * dv/dt = 1e-3 * 2 * 1000 = 2 A: v(o) = 2 V at every step after the first
    // (the operating point has no derivative). In AC it is k * v(c) * j w * v(a): |v(o)| =
    // 1e-3 * 2 * 2 pi f for a unit stimulus at `a`.
    let built = BuiltDesign::of(&DerivativeModels::project(), "ProductBench");
    let o = OutputProbe::node(&built.ir, "ProductBench", "o");
    let (_, result) = built.run_with(1e-3, 0.1e-3);
    let points: Vec<_> = result.iter().skip(1).collect();
    assert!(points.len() > 2, "the transient takes steps");
    for point in points {
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("product", "v(o)", Coordinate::Time(point.time())), 2.0, point.get_node(&o).expect("v(o)"));
    }
    let mut running = DerivativeModels::project().compile("ProductAcBench").unwrap_or_else(|r| panic!("{r}"));
    let o = OutputProbe::node(&running.ir, "ProductAcBench", "o");
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e5, steps: 3, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    for step in sweep.iter() {
        let wanted = 1e-3 * 2.0 * 2.0 * std::f64::consts::PI * step.frequency;
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("product ac", "|v(o)|", Coordinate::Frequency(step.frequency)), wanted, step.get_node(&o).expect("v(o)").norm());
    }
}

#[test]
fn an_event_reads_the_derivative_of_the_step() {
    // AS-07 AC 4 (§14.5.4): on `v(a)` = 1 kV/s * t, `on at(1m) { emit!("d", x = dv); }` with
    // `let dv = ddt(v(a));` records 1000 V/s within 1e-6 relative; on a node at equilibrium (held at
    // 2 V) the same record is 0.
    let dv = DerivativeReadings::derivative_recorded_at_one_millisecond("EventBench");
    assert!((dv - 1000.0).abs() <= 1e-6 * 1000.0, "dv/dt = {dv}");
    let at_rest = DerivativeReadings::derivative_recorded_at_one_millisecond("EquilibriumBench");
    assert!(at_rest.abs() <= 1e-6, "dv/dt at equilibrium = {at_rest}");
}

#[test]
fn a_looser_derivative_abstol_takes_fewer_steps() {
    // AS-07 AC 5: the RC of the twins with `ddt(q, 1e-3)` takes fewer accepted steps than the same RC
    // with `ddt(q, 1e-18)`: the `abstol` is the state tolerance of that `ddt` term (solver-hygiene V1).
    let tight = DerivativeReadings::accepted_steps("RcBench");
    let loose = DerivativeReadings::accepted_steps("LooseRcBench");
    assert!(loose < tight, "1e-3 takes {loose} steps, 1e-18 takes {tight}");
}

#[test]
fn bounded_holds_its_state_at_the_bound() {
    // AS-04 AC 1 (§11.2, `hi` = 1.0): with v(p, n) = +1 kV until 2 s and -1 kV after, `drift` is
    // +1 per second then -1 per second. `pos(xs)` integrates it: 0.5 at 0.5 s; at 1 s it reaches
    // `hi`, the branch becomes a potential source and holds 1.0 from 1 s to 2 s; once `drift` goes
    // negative at 2 s the branch is a flow again and `pos(xs)` falls: 1.0 - 0.5 = 0.5 at 2.5 s.
    let built = BuiltDesign::of(&DerivativeModels::project(), "BoundedBench");
    let xs = OutputProbe::node(&built.ir, "BoundedBench", "b.xs");
    let (_, result) = built.run_with(2.6, 0.01);
    let steps: Vec<_> = result.iter().collect();
    let waveform = piperine_parity::waveform::Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&xs).expect("pos(xs)")).collect());
    for (time, wanted) in [(0.5, 0.5), (1.0, 1.0), (1.5, 1.0), (2.0, 1.0), (2.5, 0.5)] {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("bounded", "pos(xs)", Coordinate::Time(time)), wanted, actual);
    }
}
