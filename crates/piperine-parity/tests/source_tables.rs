//! `table_model` run from MSL source on the Rust backend (feature `params-and-io`, PI-10): each model is written in
//! `tests/tables/maps.msl`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is computed here by Rust from the rule the test cites (§22.2.15, Appendix C.2), never read back from the
//! model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::project::{Project, Running};
use piperine_solver::prelude::{Context, NodeIdentifier, ParamValue};

/// The project of `tests/tables/maps.msl`.
fn project() -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tables/maps.msl");
    Project::new(&path)
}

/// The samples of `SAMPLES` in the model, as Rust holds them.
const SAMPLES: [(f64, f64); 5] = [(0.0, 0.0), (1.0, 2.0), (2.0, 3.0), (4.0, 7.0), (5.0, 5.0)];

/// A bench of `maps.msl` compiled once: `v(o)` of its operating point for each value the source `src` is set to.
struct Mapped {
    running: Running,
    top: &'static str,
}

impl Mapped {
    /// The bench `top`, compiled.
    fn new(top: &'static str) -> Mapped {
        Mapped { running: project().compile(top).unwrap_or_else(|rejection| panic!("{rejection}")), top }
    }

    /// `v(o)` at the operating point with `v(i)` held at `x`.
    fn at(&mut self, x: f64) -> f64 {
        self.running.circuit.set(&format!("{}.src", self.top), "volts", ParamValue::Real(x)).unwrap_or_else(|error| panic!("set volts: {error}"));
        let out = NodeIdentifier::Anonymous(self.running.ir.node_at(&format!("{}.o", self.top)).expect("the node o").id().index());
        self.running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves").get_node(&out).expect("v(o)")
    }
}

/// The line of the samples at `x`, `x` inside them (§22.2.15, control `1`).
fn line(x: f64) -> f64 {
    let at = SAMPLES.windows(2).position(|pair| x <= pair[1].0).unwrap_or(SAMPLES.len() - 2);
    let ((x0, y0), (x1, y1)) = (SAMPLES[at], SAMPLES[at + 1]);
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// `got` is `wanted` to `relative` of its size, with `1e-12` of slack at zero.
fn holds(what: &str, wanted: f64, got: f64, relative: f64) {
    assert!((got - wanted).abs() <= relative * wanted.abs() + 1e-12, "{what}: wanted {wanted}, got {got}");
}

#[test]
fn linear_table_model_is_the_interpolation_at_op_and_dc_sweep() {
    // PI-10 AC 1 (§22.2.15, control `1`): inside the table the value is the straight line between the two samples around `x`:
    // the operating point at the default x of 0 is the first sample, and a sweep of the source over 0 to 5 in steps of 0.25
    // follows the line of the samples (Rust's own interpolation, written from the rule).
    let mut bench = Mapped::new("LinearBench");
    holds("operating point", 0.0, bench.at(0.0), 1e-12);
    let mut x = 0.0;
    while x <= 5.0 {
        holds(&format!("sweep at {x}"), line(x), bench.at(x), 1e-12);
        x += 0.25;
    }
}

#[test]
fn discrete_control_gives_the_closest_sample() {
    // PI-10 AC 2: control `D` gives the value of the sample closest to `x`: 1.4 is closest to 1 (value 2), 1.6 to 2 (value 3),
    // 3.2 to 4 (value 7), and a sample itself is its own value.
    let mut bench = Mapped::new("ClosestBench");
    for (x, wanted) in [(0.0, 0.0), (0.4, 0.0), (1.4, 2.0), (1.6, 3.0), (3.2, 7.0), (4.6, 5.0), (5.0, 5.0)] {
        holds(&format!("closest at {x}"), wanted, bench.at(x), 1e-12);
    }
}

#[test]
fn cubic_reproduces_a_cubic_polynomial() {
    // PI-10 AC 3 (Appendix C.2: not-a-knot): a cubic sampled at 6 points is reproduced exactly by control `3` between them, since
    // the not-a-knot spline through at least 4 samples of a cubic is that cubic: p(x) = x^3 - 2 x^2 + x / 2 + 1.
    let mut bench = Mapped::new("CubicBench");
    for k in 0..=40 {
        let x = f64::from(k) * 0.125;
        holds(&format!("cubic at {x}"), x * x * x - 2.0 * x * x + 0.5 * x + 1.0, bench.at(x), 1e-10);
    }
}

#[test]
fn extrapolation_c_and_l_at_each_end() {
    // PI-10 AC 4 (§22.2.15 rule 3): `1CL` holds the end value below the table and extends the end slope above it; `1LC` is the
    // reverse. Below 0 the first segment's slope is 2, above 5 the last segment's slope is (5 - 7) / (5 - 4) = -2.
    let mut constant_low = Mapped::new("ConstantLowLineHighBench");
    holds("C below", 0.0, constant_low.at(-3.0), 1e-12);
    holds("L above", 5.0 - 2.0 * 1.5, constant_low.at(6.5), 1e-12);
    holds("inside is the line", line(3.0), constant_low.at(3.0), 1e-12);
    let mut line_low = Mapped::new("LineLowConstantHighBench");
    holds("L below", 2.0 * -3.0, line_low.at(-3.0), 1e-12);
    holds("C above", 5.0, line_low.at(6.5), 1e-12);
}

#[test]
fn extrapolation_e_outside_is_e0705() {
    // PI-10 AC 5 (§22.2.15 rule 6): `1EE` outside the table fails the analysis with `E0705` naming the instance and the call.
    use std::cell::RefCell;
    use std::rc::Rc;

    use piperine_api::{Failure, Overrides, Record, RecordSink, Session};
    use piperine_parity::host::HostRun;

    struct Ignore;
    impl RecordSink for Ignore {
        fn record(&mut self, _: &Record) {}
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tables/maps.msl");
    let host = piperine_api::Project::file(&path).unwrap_or_else(|f| panic!("project: {f}"));
    let overrides = Overrides { analysis: Some("op".to_string()), top: Some("ErrorBench".to_string()), backend: Some("rust".to_string()), ..Overrides::default() };
    let spec = host.request(None, &overrides).unwrap_or_else(|f| panic!("request: {f}"));
    let mut session = Session::open(&host, &spec, &HostRun::settings()).unwrap_or_else(|f| panic!("open: {f}"));
    let failure = session.run(Rc::new(RefCell::new(Ignore))).expect_err("v(i) of 6 V is outside the table");
    let text = failure.to_string();
    assert!(matches!(failure, Failure::Host { code: "E0705", .. }), "{text}");
    assert!(text.contains("ErrorBench.map"), "the instance: {text}");
    assert!(text.contains("table_model") && text.contains("x = 6e0"), "the call and the value: {text}");
}
