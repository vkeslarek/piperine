//! `delay` run from MSL source on the Rust backend (analog-operators AO-02, Part IV §14.6): each
//! module is written in `tests/operators/delay.msl`, elaborated by the front end, compiled on the
//! transpiled backend and run on the solver. Every expected value is written from the criterion
//! it cites: the input is a ramp, so its value `td` ago is a closed form.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::build;

/// The project of `tests/operators/delay.msl`, and the runs the tests make of it.
struct Delayed;

impl Delayed {
    fn project() -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/operators/delay.msl");
        Project::new(&path)
    }

    /// The node `<top>.<name>`.
    fn node(ir: &piperine_ir::Ir, top: &str, name: &str) -> NodeIdentifier {
        NodeIdentifier::Anonymous(ir.node_at(&format!("{top}.{name}")).expect("a node").id().index())
    }

    /// `(time, v(o))` of every accepted point of the transient of `top` to `stop`, steps of at most `step`.
    fn output(top: &str, stop: f64, step: f64) -> Vec<(f64, f64)> {
        let built = build(&Self::project(), top);
        let o = Self::node(&built.ir, top, "o");
        let (_, result) = built.run_with(stop, step);
        result.iter().map(|point| (point.time(), point.get_node(&o).expect("v(o)"))).collect()
    }
}

#[test]
fn before_td_the_delay_reads_the_operating_point() {
    // AO-02 AC 2: `delay(x, 1 ms)` of `x = 1 + 1000 t` is the operating point's 1 V until 1 ms, then
    // `x` as it was 1 ms ago, `1 + 1000 (t - 1 ms)`: 1.5 V at 1.5 ms.
    let points = Delayed::output("ConstantDelayBench", 2e-3, 20e-6);
    assert!(points.len() > 20, "the transient takes steps");
    for (t, v) in points {
        let wanted = if t <= 1e-3 { 1.0 } else { 1.0 + 1000.0 * (t - 1e-3) };
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("delay 1 ms", "v(o)", Coordinate::Time(t)), wanted, v);
    }
}

#[test]
fn a_let_reading_a_limit_is_a_delay_input() {
    // AO-02 AC 2 (design mechanism 2): a `let` that reads a `limit` and is the input of a `delay` is read
    // at a solution, where the limited iterate is the unlimited one: the delay gives what the plain input
    // gives, the operating point's 1 V until 1 ms, `1 + 1000 (t - 1 ms)` after.
    let points = Delayed::output("LimitedDelayBench", 2e-3, 20e-6);
    assert!(points.len() > 20, "the transient takes steps");
    for (t, v) in points {
        let wanted = if t <= 1e-3 { 1.0 } else { 1.0 + 1000.0 * (t - 1e-3) };
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("delay of a limit", "v(o)", Coordinate::Time(t)), wanted, v);
    }
}

#[test]
fn an_operating_point_and_a_zero_delay_give_the_input() {
    // AO-02 AC 3: in an equilibrium solve `delay(x, td)` is `x`: 1 V at the operating point of the
    // 1 ms bench; and `td = 0` gives `x` in every solve: `1 + 1000 t` along the transient.
    let mut running = Delayed::project().compile("ConstantDelayBench").unwrap_or_else(|r| panic!("{r}"));
    let o = Delayed::node(&running.ir, "ConstantDelayBench", "o");
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("delay op", "v(o)", Coordinate::OperatingPoint), 1.0, solved.get_node(&o).expect("v(o)"));
    for (t, v) in Delayed::output("ZeroDelayBench", 1e-3, 20e-6) {
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("delay 0", "v(o)", Coordinate::Time(t)), 1.0 + 1000.0 * t, v);
    }
}

#[test]
fn a_varying_delay_reads_back_td_and_max_while_td_exceeds_it() {
    // AO-02 AC 4: `delay(1000 t, 0.5 t, 0.1 ms)`: until 0.2 ms `td = 0.5 t` reads `x` as it was `0.5 t`
    // ago, `500 t`; from there `td` exceeds `max` and reads `max` ago, `1000 (t - 0.1 ms)`. At
    // exactly `td = max` (0.2 ms) both give 0.1 V.
    let points = Delayed::output("VaryingDelayBench", 0.6e-3, 5e-6);
    assert!(points.len() > 20, "the transient takes steps");
    for (t, v) in points {
        let wanted = if t <= 0.2e-3 { 500.0 * t } else { 1000.0 * (t - 0.1e-3) };
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("delay varying", "v(o)", Coordinate::Time(t)), wanted, v);
    }
}

#[test]
fn a_negative_td_or_a_non_positive_max_fails_with_e0453() {
    // AO-02 AC 6: a negative `td`, or a `max` that is not positive, fails the run with `E0453`
    // naming the instance.
    for top in ["NegativeDelayBench", "NonPositiveMaxBench"] {
        let mut running = Delayed::project().compile(top).unwrap_or_else(|r| panic!("{r}"));
        let error = match running.circuit.dc(Context::default()) {
            Err(error) => error.to_string(),
            Ok(mut dc) => dc.solve().expect_err(top).to_string(),
        };
        assert!(error.contains("E0453"), "{top}: {error}");
        assert!(error.contains(top), "{top} names the instance: {error}");
    }
}
