//! The Appendix H rows run from MSL source on the Rust backend (feature `msl-fixes`, CAT-01 to
//! CAT-04): one design per receiver type, elaborated by the front end, compiled on the
//! transpiled backend and run on the solver. Rust's own methods are the oracle (Appendix H rule
//! 3): every expected value is computed here by the `f64` or integer method the row names, or is
//! the value the criterion states. A reaction commits each value to a `Bit` carrier, so its bytes
//! are compared bit for bit; an `analog` block drives each value as a current into 1 ohm, so the
//! operating point is compared after the solver's own round-off.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier, Solver, TransientAnalysisOptions};

#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::{build, first_edge_value};

/// How far the solver may move a value it was given as a current into 1 ohm: its `gmin` and
/// round-off, not a tolerance of the rows (the rows' values are bit for bit in a reaction).
const SOLVE: f64 = 1e-9;

/// A free-running oscillator: its first rising edge is at 2.5 ns.
const OSC: &str = "pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\n\
                   digital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n";

/// A source written under `CARGO_TARGET_TMPDIR`, as a project.
fn project(name: &str, source: &str) -> Project {
    let dir: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join("catalogue");
    std::fs::create_dir_all(&dir).expect("a directory for the sources");
    let path = dir.join(format!("{name}.msl"));
    std::fs::write(&path, source).expect("the source is written");
    Project::new(&path).cache(&dir.join("cache"))
}

/// A reaction's output: the type of its carrier and what the row writes to it.
struct Row {
    ty: &'static str,
    /// The initial value of the carrier: the type's zero (a carrier has a value from the start, §3.3.4).
    init: &'static str,
    expr: String,
}

impl Row {
    fn new(ty: &'static str, expr: impl Into<String>) -> Row {
        let zero = match ty {
            "bool" => "false",
            "f64" => "0.0",
            _ => "0",
        };
        Row { ty, init: zero, expr: expr.into() }
    }

    /// `Bit<ty>`, or `Bit` for a `bool`; the initial value on the declaring module's `out` port
    /// only (a signal handed to an `out` port with an initial value is `E0223`, §5.2.6).
    fn carrier(&self, with_init: bool) -> String {
        match (self.ty, with_init) {
            ("bool", true) => format!("Bit = {}", self.init),
            ("bool", false) => "Bit".to_string(),
            (ty, true) => format!("Bit<{ty}> = {}", self.init),
            (ty, false) => format!("Bit<{ty}>"),
        }
    }
}

/// The module `Rows` of one reaction that commits every row at the first rising edge (`lets`
/// first), the bench `Bench` that places it under the oscillator, and `items` before both.
fn reaction_design(items: &str, lets: &str, rows: &[Row]) -> String {
    let ports: Vec<String> = rows.iter().enumerate().map(|(k, row)| format!("out r{k}: {}", row.carrier(true))).collect();
    let commits: Vec<String> = rows.iter().enumerate().map(|(k, row)| format!("r{k} <- {};", row.expr)).collect();
    let signals: Vec<String> = rows.iter().enumerate().map(|(k, row)| format!("signal r{k}: {};", row.carrier(false))).collect();
    let names: Vec<String> = (0..rows.len()).map(|k| format!("r{k}")).collect();
    format!(
        "{OSC}{items}\nmodule Rows(in clk: Bit, {}) {{}}\ndigital for Rows {{ on clk.rise() {{ {lets} {} }} }}\n\
         module Bench() {{ signal clk: Bit; {} let osc = Osc(clk) {{ f: 200M }}; let m = Rows(clk, {}); }}\n",
        ports.join(", "),
        commits.join(" "),
        signals.join(" "),
        names.join(", ")
    )
}

/// The bytes of an `f64` carrier value: its eight bytes.
fn real_bytes(v: f64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// The bytes of a one-byte carrier value (`u8`, `i8`, `bool`).
fn byte(v: u8) -> Vec<u8> {
    vec![v]
}

/// The value of each `rN` once the first rising edge has committed, in row order: its bytes.
fn first_commits(project: &Project, count: usize) -> Vec<Vec<u8>> {
    let seen = build(project, "Bench").run(10e-9, 1e-9);
    (0..count).map(|k| first_edge_value(&seen, &format!("Bench.r{k}"))).collect()
}

/// What the run of `Bench` says when it faults: the message of the error, or a failure of the
/// test when the run went through.
fn the_fault(project: &Project) -> String {
    let built = build(project, "Bench");
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let mut options = TransientAnalysisOptions::new(10e-9, 1e-9);
    options.dt_max = 1e-9;
    let mut solver = Solver::new(circuit).with_context(Context::default()).with_tran_opts(options).build();
    match solver.tran().and_then(|mut t| t.solve()) {
        Ok(_) => panic!("the run was expected to fault"),
        Err(e) => e.to_string(),
    }
}

/// The values the f64 rows read: `x`, `y`, `z`, `w` of the designs below.
const X: f64 = 0.5;
const Y: f64 = 2.0;
const Z: f64 = -2.5;
const W: f64 = 7.5;

/// The `f64` rows of H.2 with the value Rust's method gives, one expression of `x y z w` each.
fn f64_rows() -> Vec<(&'static str, f64)> {
    let (x, y, z, w) = (X, Y, Z, W);
    vec![
        ("z.abs()", z.abs()),
        ("w.sqrt()", w.sqrt()),
        ("x.powf(y)", x.powf(y)),
        ("w.powf(x)", w.powf(x)),
        ("z.powi(3)", z.powi(3)),
        ("y.powi(-2)", y.powi(-2)),
        ("x.exp()", x.exp()),
        ("w.ln()", w.ln()),
        ("w.log10()", w.log10()),
        ("w.log2()", w.log2()),
        ("x.sin()", x.sin()),
        ("z.cos()", z.cos()),
        ("x.tan()", x.tan()),
        ("x.asin()", x.asin()),
        ("x.acos()", x.acos()),
        ("w.atan()", w.atan()),
        ("z.atan2(y)", z.atan2(y)),
        ("x.sinh()", x.sinh()),
        ("z.cosh()", z.cosh()),
        ("z.tanh()", z.tanh()),
        ("z.floor()", z.floor()),
        ("z.ceil()", z.ceil()),
        ("z.round()", z.round()),
        ("x.round()", x.round()),
        ("z.trunc()", z.trunc()),
        ("z.fract()", z.fract()),
        ("x.min(y)", x.min(y)),
        ("z.max(x)", z.max(x)),
        ("w.clamp(x, y)", w.clamp(x, y)),
        ("z.clamp(-1.0, 1.0)", z.clamp(-1.0, 1.0)),
        ("z.signum()", z.signum()),
        ("w.signum()", w.signum()),
        ("z.rem_euclid(y)", z.rem_euclid(y)),
        ("w.rem_euclid(y)", w.rem_euclid(y)),
        ("z.hypot(w)", z.hypot(w)),
    ]
}

/// The `bool` rows of H.2: the expression of `x`, `not_a_number` and `plus_inf`, and Rust's answer.
fn bool_rows() -> Vec<(&'static str, bool)> {
    let (x, nan, inf) = (X, f64::NAN, f64::INFINITY);
    vec![
        ("not_a_number.is_nan()", nan.is_nan()),
        ("x.is_nan()", x.is_nan()),
        ("x.is_finite()", x.is_finite()),
        ("plus_inf.is_finite()", inf.is_finite()),
        ("plus_inf.is_infinite()", inf.is_infinite()),
        ("x.is_infinite()", x.is_infinite()),
    ]
}

/// The operands of `f64_rows` and `bool_rows`, as `let`s.
const REAL_LETS: &str = "let x = 0.5; let y = 2.0; let z = -2.5; let w = 7.5; let zero = 0.0; let not_a_number = zero / zero; let plus_inf = x / zero;";

/// The same operands as params of the `analog` module: a contribution that reads nothing is B1
/// (FIX-04), so the operands are read from the module, as a model reads its own params.
const REAL_PARAMS: &str = "x: f64 = 0.5; y: f64 = 2.0; z: f64 = -2.5; w: f64 = 7.5; zero: f64 = 0.0;";
const ANALOG_LETS: &str = "let not_a_number = zero / zero; let plus_inf = x / zero;";

#[test]
fn f64_rows_equal_rusts_bit_for_bit_in_analog_and_in_a_reaction() {
    // CAT-01 AC 2: each H.2 row gives the value Rust's `f64` method gives. In a reaction the
    // committed bytes are Rust's bits; in an `analog` block the operating point is Rust's value
    // after the solve.
    let rows = f64_rows();
    let bools = bool_rows();

    let mut design: Vec<Row> = rows.iter().map(|(expr, _)| Row::new("f64", *expr)).collect();
    design.extend(bools.iter().map(|(expr, _)| Row::new("bool", *expr)));
    let source = reaction_design("", REAL_LETS, &design);
    let committed = first_commits(&project("f64_reaction", &source), design.len());
    for (k, (expr, want)) in rows.iter().enumerate() {
        assert_eq!(committed[k], real_bytes(*want), "reaction: {expr} is {want:?} (bits {:#x})", want.to_bits());
    }
    for (k, (expr, want)) in bools.iter().enumerate() {
        assert_eq!(committed[rows.len() + k], byte(u8::from(*want)), "reaction: {expr} is {want}");
    }

    // The same expressions in an `analog` block, each a current into 1 ohm.
    let count = rows.len() + bools.len();
    let ports: Vec<String> = (0..count).map(|k| format!("p{k}: Electrical")).collect();
    let drives: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(k, (expr, _))| format!("i(gnd, p{k}) +> {expr};"))
        .chain(bools.iter().enumerate().map(|(k, (expr, _))| format!("i(gnd, p{}) +> if {expr} {{ 1.0 }} else {{ 0.0 }};", rows.len() + k)))
        .collect();
    let signals: Vec<String> = (0..count).map(|k| format!("signal n{k}: Electrical;")).collect();
    let loads: Vec<String> = (0..count).map(|k| format!("let l{k} = Load(n{k}, gnd);")).collect();
    let names: Vec<String> = (0..count).map(|k| format!("n{k}")).collect();
    let source = format!(
        "module Load(p: Electrical, n: Electrical) {{}}\nanalog for Load {{ i(p, n) +> v(p, n); }}\n\
         module Rows({}) {{ {REAL_PARAMS} }}\nanalog for Rows {{ {ANALOG_LETS} {} }}\n\
         module Bench() {{ {} {} let rows = Rows({}); }}\n",
        ports.join(", "),
        drives.join(" "),
        signals.join(" "),
        loads.join(" "),
        names.join(", ")
    );
    let mut running = project("f64_analog", &source).compile("Bench").unwrap_or_else(|r| panic!("{r}"));
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    let wanted: Vec<f64> = rows.iter().map(|(_, v)| *v).chain(bools.iter().map(|(_, b)| f64::from(u8::from(*b)))).collect();
    let labels: Vec<&str> = rows.iter().map(|(e, _)| *e).chain(bools.iter().map(|(e, _)| *e)).collect();
    for k in 0..count {
        let node = running.ir.node_at(&format!("Bench.n{k}")).unwrap_or_else(|| panic!("node n{k}"));
        let actual = solved.get_node(&NodeIdentifier::Anonymous(node.id().index())).expect("v(n)");
        let bound = SOLVE * wanted[k].abs().max(1.0);
        assert!((actual - wanted[k]).abs() <= bound, "analog: {} is {:?}, the operating point has {actual:?}", labels[k], wanted[k]);
    }
}

#[test]
fn a_crossed_or_nan_clamp_in_analog_faults_naming_it() {
    // CAT-01 AC 2 (H.2: `clamp` panics when `lo > hi` or a bound is NaN, as Rust's does, and as a
    // reaction does): in an `analog` block the operating point fails and the fault names `clamp`;
    // an ordered pair of bounds solves.
    let bench = |name: &str, lo: &str, hi: &str| {
        let source = format!(
            "module Load(p: Electrical, n: Electrical) {{}}\nanalog for Load {{ i(p, n) +> v(p, n); }}\n\
             module Row(p: Electrical) {{ x: f64 = 0.5; lo: f64 = 0.0; hi: f64 = 1.0; zero: f64 = 0.0; }}\n\
             analog for Row {{ let not_a_number = zero / zero; i(gnd, p) +> x.clamp({lo}, {hi}); }}\n\
             module Bench() {{ signal a: Electrical; let r = Row(a); let l = Load(a, gnd); }}\n"
        );
        project(name, &source).compile("Bench").unwrap_or_else(|r| panic!("{r}"))
    };
    for (name, lo, hi) in [("clamp_crossed", "hi + 1.0", "hi"), ("clamp_nan_lo", "not_a_number", "hi"), ("clamp_nan_hi", "lo", "not_a_number")] {
        let mut running = bench(name, lo, hi);
        // the fault surfaces where the operating point is first evaluated
        let err = match running.circuit.dc(Context::default()) {
            Err(e) => e.to_string(),
            Ok(mut dc) => dc.solve().err().unwrap_or_else(|| panic!("{name}: the operating point solved")).to_string(),
        };
        assert!(err.contains("clamp"), "{name}: the fault names `clamp`: {err}");
    }
    let mut running = bench("clamp_ordered", "lo", "hi");
    running.circuit.dc(Context::default()).expect("dc").solve().expect("an ordered clamp solves");
}

/// The integer functions the integer tests call, one expression each: arguments are runtime
/// values of the call, never folded.
const INT_FNS: &str = "\
fn pw(x: u8, e: u32) -> u8 { x.pow(e) }
fn pws(x: i8, e: u32) -> i8 { x.pow(e) }
fn sat_add(a: u8, b: u8) -> u8 { a.saturating_add(b) }
fn sat_sub_u(a: u8, b: u8) -> u8 { a.saturating_sub(b) }
fn sat_sub(a: i8, b: i8) -> i8 { a.saturating_sub(b) }
fn sat_mul(a: u8, b: u8) -> u8 { a.saturating_mul(b) }
fn sat_mul_s(a: i8, b: i8) -> i8 { a.saturating_mul(b) }
fn add_none(a: u8, b: u8) -> bool { a.checked_add(b).is_none() }
fn sub_none(a: u8, b: u8) -> bool { a.checked_sub(b).is_none() }
fn mul_none(a: u8, b: u8) -> bool { a.checked_mul(b).is_none() }
fn add_or(a: u8, b: u8) -> u8 { a.checked_add(b).unwrap_or(0) }
fn sub_or(a: u8, b: u8) -> u8 { a.checked_sub(b).unwrap_or(0) }
fn mul_or(a: u8, b: u8) -> u8 { a.checked_mul(b).unwrap_or(0) }
fn add_or_s(a: i8, b: i8) -> i8 { a.checked_add(b).unwrap_or(0) }
fn rem(a: i8, d: i8) -> i8 { a.rem_euclid(d) }
fn rem_u(a: u8, d: u8) -> u8 { a.rem_euclid(d) }
fn ab(a: i8) -> i8 { a.abs() }
fn sg(a: i8) -> i8 { a.signum() }
";

#[test]
fn pow_wraps() {
    // CAT-02 AC 2 (H.1: "wraps"): `2u8.pow(9)` is 0; the results Rust's `wrapping_pow` gives.
    let rows = vec![
        Row::new("u8", "pw(2, 9)"),
        Row::new("u8", "pw(3, 2)"),
        Row::new("u8", "pw(3, 6)"),
        Row::new("i8", "pws(-2, 7)"),
        Row::new("i8", "pws(-2, 8)"),
    ];
    let committed = first_commits(&project("int_pow", &reaction_design(INT_FNS, "", &rows)), rows.len());
    let want = [2u8.wrapping_pow(9), 3u8.wrapping_pow(2), 3u8.wrapping_pow(6)].map(byte);
    assert_eq!(&committed[..3], &want);
    assert_eq!(committed[0], byte(0), "2u8.pow(9) is 0");
    assert_eq!(committed[3], byte((-2i8).wrapping_pow(7) as u8));
    assert_eq!(committed[4], byte((-2i8).wrapping_pow(8) as u8));
}

#[test]
fn saturating_rows_clamp() {
    // CAT-02 AC 3: `250u8.saturating_add(10)` is 255 and `(-100i8).saturating_sub(100)` is -128;
    // `saturating_mul` and an unsigned `saturating_sub` clamp the same way.
    let rows = vec![
        Row::new("u8", "sat_add(250, 10)"),
        Row::new("i8", "sat_sub(-100, 100)"),
        Row::new("u8", "sat_mul(16, 16)"),
        Row::new("i8", "sat_mul_s(-100, 2)"),
        Row::new("u8", "sat_sub_u(3, 5)"),
        Row::new("u8", "sat_add(250, 5)"),
    ];
    let committed = first_commits(&project("int_saturating", &reaction_design(INT_FNS, "", &rows)), rows.len());
    assert_eq!(committed[0], byte(255));
    assert_eq!(committed[1], byte(0x80), "-128 as a byte");
    assert_eq!(committed[2], byte(255));
    assert_eq!(committed[3], byte(0x80));
    assert_eq!(committed[4], byte(0));
    assert_eq!(committed[5], byte(250u8.saturating_add(5)));
    assert_eq!(committed[1], byte(((-100i8).saturating_sub(100)) as u8));
}

#[test]
fn checked_rows_give_none_on_overflow() {
    // CAT-02 AC 4: `None` on overflow and `Some` of the exact result otherwise:
    // `250u8.checked_add(10).is_none()` is true, `250u8.checked_add(5).unwrap_or(0)` is 255.
    let rows = vec![
        Row::new("bool", "add_none(250, 10)"),
        Row::new("bool", "add_none(250, 5)"),
        Row::new("bool", "sub_none(3, 5)"),
        Row::new("bool", "sub_none(5, 3)"),
        Row::new("bool", "mul_none(16, 16)"),
        Row::new("bool", "mul_none(15, 17)"),
        Row::new("u8", "add_or(250, 5)"),
        Row::new("u8", "add_or(250, 10)"),
        Row::new("u8", "sub_or(5, 3)"),
        Row::new("u8", "mul_or(15, 17)"),
        Row::new("i8", "add_or_s(100, 27)"),
        Row::new("i8", "add_or_s(100, 28)"),
    ];
    let committed = first_commits(&project("int_checked", &reaction_design(INT_FNS, "", &rows)), rows.len());
    let wants_none = [true, false, true, false, true, false];
    for (k, none) in wants_none.iter().enumerate() {
        assert_eq!(committed[k], byte(u8::from(*none)), "{}", rows[k].expr);
    }
    assert_eq!(committed[6], byte(255));
    assert_eq!(committed[7], byte(0));
    assert_eq!(committed[8], byte(2));
    assert_eq!(committed[9], byte(15u8.checked_mul(17).unwrap_or(0)));
    assert_eq!(committed[10], byte(127));
    assert_eq!(committed[11], byte(0));
}

#[test]
fn rem_euclid_by_zero_faults_naming_it() {
    // CAT-02 AC 5 (H.1: "panics if `d` is zero", §1 rule 11): the run faults and the fault
    // names `rem_euclid`.
    let rows = vec![Row::new("u8", "rem_u(7, 0)")];
    let fault = the_fault(&project("int_rem_zero", &reaction_design(INT_FNS, "", &rows)));
    assert!(fault.contains("rem_euclid"), "the fault names the row: {fault}");
    // A non-zero divisor runs: the same function with `3`.
    let rows = vec![Row::new("u8", "rem_u(7, 3)")];
    assert_eq!(first_commits(&project("int_rem_three", &reaction_design(INT_FNS, "", &rows)), 1), vec![byte(7u8.rem_euclid(3))]);
}

#[test]
fn signed_abs_signum_rem_euclid_follow_rust() {
    // CAT-02 AC 6: `(-128i8).abs()` is -128, `(-5i8).signum()` is -1, `(-7i8).rem_euclid(3)` is 2.
    let rows = vec![
        Row::new("i8", "ab(-128)"),
        Row::new("i8", "ab(-5)"),
        Row::new("i8", "sg(-5)"),
        Row::new("i8", "sg(0)"),
        Row::new("i8", "sg(9)"),
        Row::new("i8", "rem(-7, 3)"),
        Row::new("i8", "rem(7, -3)"),
    ];
    let committed = first_commits(&project("int_signed", &reaction_design(INT_FNS, "", &rows)), rows.len());
    assert_eq!(committed[0], byte(0x80), "(-128i8).abs() wraps to -128");
    assert_eq!(committed[1], byte(5));
    assert_eq!(committed[2], byte((-5i8).signum() as u8));
    assert_eq!(committed[2], byte(0xff), "-1");
    assert_eq!(committed[3], byte(0));
    assert_eq!(committed[4], byte(1));
    assert_eq!(committed[5], byte(2));
    assert_eq!(committed[5], byte((-7i8).rem_euclid(3) as u8));
    assert_eq!(committed[6], byte(7i8.rem_euclid(-3) as u8));
}

/// The `Option` functions the `Option` tests call: `c` picks `Some(7)` (above 1) or `None`, `z` is
/// a divisor a row must not evaluate for the other arm.
const OPTION_FNS: &str = "\
fn unw(c: u8) -> u8 { let o = if c > 1 { Some(7u8) } else { None }; o.unwrap() }
fn exp_msg(c: u8) -> u8 { let o = if c > 1 { Some(7u8) } else { None }; o.expect(\"the reading is gone\") }
fn else_(c: u8, z: u8) -> u8 { let o = if c > 1 { Some(7u8) } else { None }; o.unwrap_or_else(|| 90 / z) }
fn mapped(c: u8, z: u8) -> u8 { let o = if c > 1 { Some(7u8) } else { None }; o.map(|x| x / z).unwrap_or(5) }
fn chained(c: u8) -> u8 { let o = if c > 1 { Some(7u8) } else { None }; o.and_then(|x| if x > 5 { Some(x * 2) } else { None }).unwrap_or(0) }
fn chained_low(c: u8) -> u8 { let o = if c > 1 { Some(3u8) } else { None }; o.and_then(|x| if x > 5 { Some(x * 2) } else { None }).unwrap_or(0) }
fn either(a: u8, b: u8) -> u8 {
    let p = if a > 1 { Some(a) } else { None };
    let q = if b > 1 { Some(b) } else { None };
    p.or(q).unwrap_or(0)
}
fn lazily(c: u8) -> u8 { (c > 1).then(|| 7u8).unwrap_or(0) }
fn eagerly(c: u8) -> u8 { (c > 1).then_some(8u8).unwrap_or(0) }
";

#[test]
fn unwrap_and_expect_of_some_give_the_payload() {
    // CAT-03 AC 1: `unwrap` and `expect` of `Some(v)` give `v`.
    let rows = vec![Row::new("u8", "unw(2)"), Row::new("u8", "exp_msg(2)")];
    let committed = first_commits(&project("option_unwrap", &reaction_design(OPTION_FNS, "", &rows)), rows.len());
    assert_eq!(committed, vec![byte(7), byte(7)]);
}

#[test]
fn unwrap_of_none_faults_naming_unwrap() {
    // CAT-03 AC 2: `unwrap` of `None` faults the run, naming `unwrap`.
    let rows = vec![Row::new("u8", "unw(1)")];
    let fault = the_fault(&project("option_unwrap_none", &reaction_design(OPTION_FNS, "", &rows)));
    assert!(fault.contains("unwrap"), "the fault names `unwrap`: {fault}");
}

#[test]
fn expect_of_none_faults_with_its_message() {
    // CAT-03 AC 3: `expect("msg")` of `None` faults the run with `msg` in the fault.
    let rows = vec![Row::new("u8", "exp_msg(1)")];
    let fault = the_fault(&project("option_expect_none", &reaction_design(OPTION_FNS, "", &rows)));
    assert!(fault.contains("the reading is gone"), "the fault carries the message: {fault}");
}

#[test]
fn unwrap_or_else_evaluates_its_default_for_none_only() {
    // CAT-03 AC 4: the default is evaluated for `None` only. `90 / z` faults for `z` = 0, so a
    // `Some` with `z` = 0 must not evaluate it; `None` with `z` = 9 gives 10.
    let rows = vec![Row::new("u8", "else_(2, 0)"), Row::new("u8", "else_(1, 9)")];
    let committed = first_commits(&project("option_else", &reaction_design(OPTION_FNS, "", &rows)), rows.len());
    assert_eq!(committed, vec![byte(7), byte(10)]);
    // and the default of `None` is evaluated: `z` = 0 faults.
    let fault = the_fault(&project("option_else_none", &reaction_design(OPTION_FNS, "", &[Row::new("u8", "else_(1, 0)")])));
    assert!(fault.contains("division by zero"), "the default ran: {fault}");
}

#[test]
fn map_and_and_then_bind_the_payload() {
    // CAT-03 AC 5: with `x` bound to the payload for `Some`, `None` for `None`; the closure of
    // `None` is not run (`x / z` with `z` = 0 would fault).
    let rows = vec![
        Row::new("u8", "mapped(2, 7)"),
        Row::new("u8", "mapped(1, 0)"),
        Row::new("u8", "chained(2)"),
        Row::new("u8", "chained(1)"),
        Row::new("u8", "chained_low(2)"),
    ];
    let committed = first_commits(&project("option_map", &reaction_design(OPTION_FNS, "", &rows)), rows.len());
    assert_eq!(committed[0], byte(1), "Some(7).map(|x| x / 7) is Some(1)");
    assert_eq!(committed[1], byte(5), "None.map(..) is None, the default 5");
    assert_eq!(committed[2], byte(14), "Some(7).and_then(|x| Some(x * 2))");
    assert_eq!(committed[3], byte(0));
    assert_eq!(committed[4], byte(0), "Some(3).and_then(|x| if x > 5 ..) is None");
}

#[test]
fn or_takes_the_first_some() {
    // CAT-03 AC 6: `a.or(b)` is `a` when it is `Some`, else `b`.
    let rows = vec![Row::new("u8", "either(4, 9)"), Row::new("u8", "either(0, 9)"), Row::new("u8", "either(0, 0)"), Row::new("u8", "either(3, 0)")];
    let committed = first_commits(&project("option_or", &reaction_design(OPTION_FNS, "", &rows)), rows.len());
    assert_eq!(committed, vec![byte(4), byte(9), byte(0), byte(3)]);
}

#[test]
fn then_and_then_some_follow_the_bool() {
    // CAT-03 AC 7: `c.then(|| v)` and `c.then_some(v)` are `Some(v)` when `c` is true, else
    // `None`.
    let rows = vec![Row::new("u8", "lazily(2)"), Row::new("u8", "lazily(1)"), Row::new("u8", "eagerly(2)"), Row::new("u8", "eagerly(1)")];
    let committed = first_commits(&project("option_then", &reaction_design(OPTION_FNS, "", &rows)), rows.len());
    assert_eq!(committed, vec![byte(7), byte(0), byte(8), byte(0)]);
}

#[test]
fn db10_and_db20_run() {
    // CAT-04 AC 1: `db10(100.0)` and `db20(-10.0)` are 20.0 and 20.0 on the Rust backend: the
    // stdlib's `10.0 * x.log10()` and `20.0 * x.abs().log10()`.
    let rows = vec![Row::new("f64", "db10(100.0)"), Row::new("f64", "db20(-10.0)"), Row::new("f64", "db10(0.1)")];
    let source = format!("use std::analog::{{db10, db20}};\n{}", reaction_design("", "", &rows));
    let committed = first_commits(&project("db_run", &source), rows.len());
    assert_eq!(committed[0], real_bytes(20.0));
    assert_eq!(committed[1], real_bytes(20.0));
    assert_eq!(committed[2], real_bytes(10.0 * 0.1f64.log10()));
}

#[test]
fn db10_and_db20_type_check_from_the_stdlib() {
    // CAT-04 AC 2: nothing in `db10` and `db20` is refused any more: a design that calls them in
    // a reaction and in an `analog` block type-checks and elaborates.
    let source = format!(
        "use std::analog::{{db10, db20}};\n{OSC}\n\
         module Gain(in clk: Bit, p: Electrical, out r: Bit<f64> = 0.0) {{}}\n\
         digital for Gain {{ on clk.rise() {{ r <- db20(-10.0); }} }}\n\
         analog for Gain {{ i(gnd, p) +> db10(100.0); }}\n\
         module Bench() {{ signal clk: Bit; signal r: Bit<f64>; signal n: Electrical; let osc = Osc(clk) {{ f: 200M }}; let g = Gain(clk, n, r); }}\n"
    );
    let project = project("db_types", &source);
    let design = project.design().unwrap_or_else(|r| panic!("{r}"));
    piperine_lang::types::Types::check(&design, "Bench").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    project.elaborate("Bench").unwrap_or_else(|r| panic!("{r}"));
}
