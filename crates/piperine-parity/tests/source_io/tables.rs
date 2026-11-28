//! `table_model` run from MSL source on the Rust backend (feature `params-and-io`, PI-10): each model is written in
//! `tests/tables/maps.msl`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is computed here by Rust from the rule the test cites (§22.2.15, Appendix C.2), never read back from the
//! model.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use piperine_parity::project::{Project, Running};
use piperine_solver::prelude::{Context, NodeIdentifier, ParamValue};

/// The samples of `SAMPLES` in the model, as Rust holds them.
const SAMPLES: [(f64, f64); 5] = [(0.0, 0.0), (1.0, 2.0), (2.0, 3.0), (4.0, 7.0), (5.0, 5.0)];

/// The model of the tests and the rules they check values against: the owner of the helpers the tests share.
struct Maps;

impl Maps {
    /// The project of `tests/tables/maps.msl`.
    fn project() -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tables/maps.msl");
        Project::new(&path)
    }

    /// The line of the samples at `x` (§22.2.15, control `1`): between the two samples around `x`, and the line of the end segment
    /// outside the table (no extrapolation character is `L` at both ends, rule 3).
    fn line(x: f64) -> f64 {
        let at = SAMPLES.windows(2).position(|pair| x <= pair[1].0).unwrap_or(SAMPLES.len() - 2);
        let ((x0, y0), (x1, y1)) = (SAMPLES[at], SAMPLES[at + 1]);
        y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    }

    /// `got` is `wanted` to `relative` of its size (an exact zero is wanted to `1e-15` absolute).
    fn holds(what: &str, wanted: f64, got: f64, relative: f64) {
        let allowed = if wanted == 0.0 { 1e-15 } else { relative * wanted.abs() };
        assert!((got - wanted).abs() <= allowed, "{what}: wanted {wanted}, got {got} (allowed {allowed:e})");
    }

    /// `v(o)` of the DC sweep of the source `src` of the bench `top` from `from` to `to` in steps of `step`, as `(x, v(o))`: the
    /// analysis a host asks for with `--analysis dc --sweep`.
    fn sweep(top: &str, from: f64, to: f64, step: f64) -> Vec<(f64, f64)> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tables/maps.msl");
        let host = piperine_api::Project::file(&path).unwrap_or_else(|f| panic!("project: {f}"));
        let overrides = piperine_api::Overrides {
            analysis: Some("dc".to_string()),
            top: Some(top.to_string()),
            backend: Some("rust".to_string()),
            sweep: Some(format!("{top}.src.volts")),
            from: Some(from),
            to: Some(to),
            step: Some(step),
            print: vec!["o".to_string()],
            ..piperine_api::Overrides::default()
        };
        let spec = host.request(None, &overrides).unwrap_or_else(|f| panic!("request: {f}"));
        let mut session = piperine_api::Session::open(&host, &spec, &piperine_parity::host::HostRun::settings()).unwrap_or_else(|f| panic!("open: {f}"));
        let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("the sweep runs: {f}"));
        let real = |cell: &piperine_api::Cell| match cell {
            piperine_api::Cell::Real(value) => *value,
            other => panic!("a real, found {other:?}"),
        };
        table.rows.iter().map(|row| (real(&row[0]), real(&row[1]))).collect()
    }
}

/// A sink that keeps nothing: these runs read the table.
struct Ignore;

impl piperine_api::RecordSink for Ignore {
    fn record(&mut self, _: &piperine_api::Record) {}
}

/// A bench of `maps.msl` compiled once: `v(o)` of its operating point for each value the source `src` is set to.
struct Mapped {
    running: Running,
    top: &'static str,
}

impl Mapped {
    /// The bench `top`, compiled.
    fn new(top: &'static str) -> Mapped {
        Mapped { running: Maps::project().compile(top).unwrap_or_else(|rejection| panic!("{rejection}")), top }
    }

    /// `v(o)` at the operating point with `v(i)` held at `x`.
    fn at(&mut self, x: f64) -> f64 {
        self.running.circuit.set(&format!("{}.src", self.top), "volts", ParamValue::Real(x)).unwrap_or_else(|error| panic!("set volts: {error}"));
        let out = NodeIdentifier::Anonymous(self.running.ir.node_at(&format!("{}.o", self.top)).expect("the node o").id().index());
        self.running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves").get_node(&out).expect("v(o)")
    }
}

#[test]
fn linear_table_model_is_the_interpolation_at_op_and_dc_sweep() {
    // PI-10 AC 1 (§22.2.15, control `1`), to 1e-15 relative: inside the table the value is the straight line between the two samples
    // around `x`. The operating point at the default x of 0 is the first sample, and the DC sweep of the source from -1 to 6 V in steps of
    // 0.25 V (outside the table too: no extrapolation character is `L` at both ends) follows the line of the samples.
    let mut bench = Mapped::new("LinearBench");
    Maps::holds("operating point", 0.0, bench.at(0.0), 1e-15);
    let swept = Maps::sweep("LinearBench", -1.0, 6.0, 0.25);
    assert_eq!(swept.len(), 29, "-1 to 6 in steps of 0.25 is 29 points");
    for (x, got) in swept {
        Maps::holds(&format!("sweep at {x}"), Maps::line(x), got, 1e-15);
    }
}

#[test]
fn discrete_control_gives_the_closest_sample() {
    // PI-10 AC 2: control `D` gives the value of the sample closest to `x`: 1.4 is closest to 1 (value 2), 1.6 to 2 (value 3),
    // 3.2 to 4 (value 7), and a sample itself is its own value.
    let mut bench = Mapped::new("ClosestBench");
    for (x, wanted) in [(0.0, 0.0), (0.4, 0.0), (1.4, 2.0), (1.6, 3.0), (3.2, 7.0), (4.6, 5.0), (5.0, 5.0)] {
        Maps::holds(&format!("closest at {x}"), wanted, bench.at(x), 1e-15);
    }
}

#[test]
fn cubic_reproduces_a_cubic_polynomial() {
    // PI-10 AC 3 (Appendix C.2: not-a-knot), to 1e-12 relative: a cubic sampled at 6 points is reproduced exactly by control `3` between
    // them, since the not-a-knot spline through at least 4 samples of a cubic is that cubic: p(x) = x^3 - 2 x^2 + x / 2 + 1. Outside
    // the table the default extrapolation (`L` at both ends) is the line of the end slope: p'(0) = 1/2 and p'(5) = 111/2.
    let polynomial = |x: f64| x * x * x - 2.0 * x * x + 0.5 * x + 1.0;
    let mut bench = Mapped::new("CubicBench");
    for k in 0..=40 {
        let x = f64::from(k) * 0.125;
        Maps::holds(&format!("cubic at {x}"), polynomial(x), bench.at(x), 1e-12);
    }
    Maps::holds("L below", 1.0 + 0.5 * -2.0, bench.at(-2.0), 1e-12);
    Maps::holds("L above", 78.5 + 55.5 * 1.5, bench.at(6.5), 1e-12);
}

#[test]
fn extrapolation_c_and_l_at_each_end() {
    // PI-10 AC 4 (§22.2.15 rule 3): `1CL` holds the end value below the table and extends the end slope above it; `1LC` is the
    // reverse; no character is `L` at both ends (`LinearBench`, `1`). Below 0 the first segment's slope is 2, above 5 the last
    // segment's slope is (5 - 7) / (5 - 4) = -2.
    let mut constant_low = Mapped::new("ConstantLowLineHighBench");
    Maps::holds("C below", 0.0, constant_low.at(-3.0), 1e-15);
    Maps::holds("L above", 5.0 - 2.0 * 1.5, constant_low.at(6.5), 1e-15);
    Maps::holds("inside is the line", Maps::line(3.0), constant_low.at(3.0), 1e-15);
    let mut line_low = Mapped::new("LineLowConstantHighBench");
    Maps::holds("L below", 2.0 * -3.0, line_low.at(-3.0), 1e-15);
    Maps::holds("C above", 5.0, line_low.at(6.5), 1e-15);
    let mut default = Mapped::new("LinearBench");
    Maps::holds("default below is L", 2.0 * -3.0, default.at(-3.0), 1e-15);
    Maps::holds("default above is L", 5.0 - 2.0 * 1.5, default.at(6.5), 1e-15);
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

#[test]
fn the_table_diode_passes_the_partials_verifier() {
    // PI-10 AC 8 (the spec's Independent Test): the diode of `tests/tables/diode.msl` reads a measured table (`diode_iv.tbl`, 64
    // rows, `parse_table`) and a boot image (`boot.hex`, `parse_hex`); every function of its device passes `verify_partials`
    // at three bias points, the declared partial being the derivative of the cubic spline through the table.
    use piperine_abi::caller::Handle;
    use piperine_abi::device::Access;
    use piperine_abi::raw::{Shape, ambient, cell, part, role};
    use piperine_abi::verify::Verdict;
    use piperine_plan::Modules;

    let diode = Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tables/diode.msl")).compile("TableDiodeBench").unwrap_or_else(|rejection| panic!("{rejection}"));
    let body = diode.ir.bodies().find(|body| body.name() == "TableDiode").expect("the diode body").id();
    let module = Modules::module(diode.compiled(), body).expect("the diode has a compiled module");
    let handle = Handle::create(module, &[]).unwrap_or_else(|error| panic!("create the diode: {error}"));
    let shape: &Shape<'_> = handle.shape();
    let mut verified = 0;
    for (k, function) in shape.functions.iter().enumerate() {
        for (first, second) in [(0.2503, 0.0), (0.4107, 0.0), (0.5611, 0.0)] {
            let lens = shape.cell_lens(&function.reads, Access::FunctionRead).expect("read lens");
            let input: Vec<Vec<u8>> = function
                .reads
                .iter()
                .zip(&lens)
                .map(|(read, len)| {
                    let value: f64 = match read.kind {
                        cell::PIN => match shape.pins.get(read.index as usize).map(|pin| (pin.role, pin.name.as_str())) {
                            Some((role::GLOBAL, _)) => 300.15,
                            Some((role::PORT, name)) => if name == shape.pins.iter().find(|pin| pin.role == role::PORT).map_or("", |pin| pin.name.as_str()) { first } else { second },
                            _ => 0.002,
                        },
                        cell::INPUT if read.part == part::LAST => first - second,
                        cell::INPUT => first - second,
                        cell::AMBIENT if read.index == ambient::GMIN => 1e-12,
                        cell::AMBIENT if read.index == ambient::SOURCE_SCALE => 1.0,
                        _ => 0.0,
                    };
                    assert_eq!(*len, 8, "every read of the diode is an `f64`");
                    value.to_le_bytes().to_vec()
                })
                .collect();
            match Verdict::of_partials(&handle, k as u32, &input, 1e-6) {
                Ok(Verdict::Exact) => verified += 1,
                other => panic!("`{}` at ({first}, {second}): {other:?}", function.name.as_str()),
            }
        }
    }
    assert!(verified >= 3, "the diode has a function with partials: {verified} points verified");
}
