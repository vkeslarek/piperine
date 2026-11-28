//! The compiled functions against the interpreter's (feature `ir-rust`, RS-06 AC 1, 2 and 7, and
//! the edge case on MSL names the printer generates; checks C33, C35, C41, C104): every function
//! of the five M1 bodies is evaluated through `caller::Handle` on the interpreter's module and on
//! the compiled one with the same input bytes, at three bias points and over a seeded sweep of
//! 300 more (wide and negative biases, `LAST` on both sides of 0 and of the diode's `vcrit`, every
//! arm of `pnjlim` reached), the six `AGE`/`STATIC` cases of a `limit` included, with every
//! partial seeded and with none; the written cells are equal bit for bit and the partials equal
//! as `f64`. Every compiled function passes `verify_partials`
//! at `rel_tol` 1e-6. A non-finite value is written as it is, on both backends. Bodies whose MSL
//! names are the names the printer generates (`cx`, `self` beside `self_`, `limit_vd_last`, a
//! param named like an environment read, duplicate locals beside `<name>_<index>`) compile and
//! give the interpreter's bytes the same way.
#![cfg(unix)]

use piperine_abi::caller::Handle;
use piperine_abi::device::Access;
use piperine_abi::raw::{Cell, Shape, ambient, cell, part, role};
use piperine_abi::verify::Verdict;
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Constant, Env, FunctionId, FunctionSpec, IrBuilder,
    Local, MathFn, NodeRole, Operator, Origin, Param, QuantityRole, Real, Site, Stmt, Terminal, ValueType,
};
use piperine_transpile::Transpiler;
use piperine_transpile::print::Text;

use piperine_ir_fixtures::devices;
use crate::both;

use both::{Both, Rng, Written};
use devices::{Devices, Ex};

/// One bias point: the two node potentials, and the `LAST` a limit starts from.
#[derive(Clone, Copy)]
struct Bias {
    first: f64,
    second: f64,
    last: f64,
}

impl Bias {
    /// Three points from the interpreter's own `partials.rs`; the third stands at the knee, with
    /// a `LAST` far enough back that the diode's `pnjlim` limits.
    const POINTS: [Bias; 3] = [
        Bias { first: 0.30, second: 0.0, last: 0.25 },
        Bias { first: 0.55, second: 0.05, last: 0.45 },
        Bias { first: 0.95, second: 0.05, last: 0.70 },
    ];

    /// `n` points from a fixed seed (verification F4): every other one wide, the first potential
    /// in [-25 V, 17.5 V), the second in [-0.5 V, 0.5 V) and `LAST` in [-30 V, 5 V), so
    /// `pnjlim`'s negative arms run and the diode's exponential reaches 1e300 (below 18 V, where
    /// it stays finite; a non-finite value is `a_non_finite_value_is_written_as_is`'s); the rest
    /// narrow, both in [-1.5 V, 1.5 V), around 0 and the diode's `vcrit` (0.73 V).
    fn sweep(n: usize) -> Vec<Bias> {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    Bias { first: rng.range(-25.0, 17.5), second: rng.range(-0.5, 0.5), last: rng.range(-30.0, 5.0) }
                } else {
                    Bias { first: rng.range(-1.5, 1.5), second: rng.range(-0.05, 0.05), last: rng.range(-1.5, 1.5) }
                }
            })
            .collect()
    }

    /// The arm of `pnjlim(new, old, nvt, vcrit)` (std/analog.msl:21-31) the diode's limit takes
    /// at this point with `old` the held `LAST` (`AGE` 2), at the params `Bias::param` gives
    /// (`is` 1e-14, `n` 1) and the temperature `Bias::bytes` gives (300.15 K).
    fn pnjlim_arm(&self) -> &'static str {
        let nvt = 1.0 * (1.380649e-23 * 300.15 / 1.602176634e-19);
        let vcrit = nvt * (nvt / (2.0_f64.sqrt() * 1e-14)).ln();
        let (new, old) = (self.first - self.second, self.last);
        if new > vcrit && (new - old).abs() > 2.0 * nvt {
            match (old > 0.0, (new - old) / nvt > 0.0) {
                (true, true) => "old > 0, arg > 0",
                (true, false) => "old > 0, arg <= 0",
                (false, _) => "old <= 0",
            }
        } else if new < 0.0 {
            let arg = if old > 0.0 { -old - 1.0 } else { 2.0 * old - 1.0 };
            match (old > 0.0, new < arg) {
                (true, true) => "new < 0, old > 0, new < arg",
                (true, false) => "new < 0, old > 0, new >= arg",
                (false, true) => "new < 0, old <= 0, new < arg",
                (false, false) => "new < 0, old <= 0, new >= arg",
            }
        } else {
            "new"
        }
    }

    /// The value of a param by name, within what every body accepts.
    fn param(name: &str) -> f64 {
        match name {
            "is" => 1e-14,
            "n" => 1.0,
            "cj0" => 1e-12,
            "r" => 1000.0,
            "c" => 1e-9,
            "dc" => 1.5,
            "ac" => 1.0,
            // the bodies of `msl_names_that_equal_generated_names_give_the_interpreters_bytes`
            "self_" => 1000.0,
            "at_zero" => 2.0,
            "at_iterate" => 0.5,
            "temperature" => 7.0,
            "gmin" => 3.0,
            "cx_" => 0.25,
            "limit_vd_last" => 1e-14,
            "vd_last_" => 1.0,
            "is_static" => 1e-12,
            other => panic!("no bias for param `{other}`"),
        }
    }

    /// The bytes of read `c` of a function of `shape`, `len` long, for a limit at `age` and `is_static`.
    fn bytes(&self, shape: &Shape<'_>, c: &Cell, len: usize, age: u8, is_static: bool) -> Vec<u8> {
        let vd = self.first - self.second;
        if c.kind == cell::AMBIENT && c.index == ambient::STATIC {
            return vec![u8::from(is_static)];
        }
        if c.kind == cell::INPUT && c.part == part::AGE {
            return vec![age];
        }
        if c.kind == cell::PIN && shape.pins.get(c.index as usize).is_some_and(|pin| pin.name.as_str() == "temperature_given") {
            // the run set its temperature: the flag is one byte, not an `f64`
            return vec![1];
        }
        let value = match c.kind {
            cell::PIN => {
                let pin = shape.pins.get(c.index as usize).expect("a pin of the shape");
                match pin.role {
                    role::PARAM => Self::param(pin.name.as_str()),
                    role::GLOBAL => 300.15,
                    role::PORT => {
                        let ports = shape.pins.iter().filter(|p| p.role == role::PORT);
                        let at = ports.take_while(|p| p.name.as_str() != pin.name.as_str()).count();
                        if at == 0 { self.first } else { self.second }
                    }
                    _ => 0.002,
                }
            }
            cell::INPUT => match c.part {
                part::LAST => self.last,
                _ => vd,
            },
            cell::AMBIENT => match c.index {
                ambient::GMIN => 1e-12,
                ambient::SOURCE_SCALE => 1.0,
                _ => 0.0,
            },
            other => panic!("a read of kind {other}"),
        };
        match len {
            8 => value.to_le_bytes().to_vec(),
            n => panic!("a read cell of {n} bytes that is neither AGE nor STATIC"),
        }
    }

    /// The input of a function: one byte buffer per read.
    fn input(&self, shape: &Shape<'_>, k: usize, age: u8, is_static: bool) -> Vec<Vec<u8>> {
        let f = shape.functions.get(k).expect("a function");
        let lens = shape.cell_lens(&f.reads, Access::FunctionRead).expect("read lens");
        f.reads.iter().zip(&lens).map(|(c, len)| self.bytes(shape, c, *len, age, is_static)).collect()
    }
}

/// The `AGE`/`STATIC` cases of a limit (abi.md A2): all six.
const LIMIT_CASES: [(u8, bool); 6] = [(0, false), (0, true), (1, false), (1, true), (2, false), (2, true)];

/// The compiled functions against the interpreter, on both backends.
struct BackendComparison;

impl BackendComparison {
    /// The cases a function is run over: the six when it reads `AGE` or `STATIC`, else one.
    fn cases(shape: &Shape<'_>, k: usize) -> Vec<(u8, bool)> {
        let f = shape.functions.get(k).expect("a function");
        let limited = f.reads.iter().any(|c| (c.kind == cell::INPUT && c.part == part::AGE) || (c.kind == cell::AMBIENT && c.index == ambient::STATIC));
        if limited { LIMIT_CASES.to_vec() } else { vec![(2, false)] }
    }

    /// Every function of every body of `both` on both backends at each of `points`, over its
    /// `AGE`/`STATIC` cases, seeded and not: the written cells equal in bits, the partials equal as
    /// `f64` and in bits.
    fn compare(both: &Both, points: &[Bias]) -> Compared {
        let mut seen = Compared { functions: 0, calls: 0, limited: 0 };
        for body in both.bodies() {
            let (interp, compiled) = both.instances(&body);
            let shape = interp.shape();
            assert_eq!(shape.functions.len(), compiled.shape().functions.len(), "{body}: one function list");
            for (k, f) in shape.functions.iter().enumerate() {
                let name = f.name.as_str().to_string();
                assert_eq!(name, compiled.shape().functions.get(k).expect("a function").name.as_str(), "{body}: function {k}");
                seen.functions += 1;
                for bias in points {
                    for (age, is_static) in BackendComparison::cases(shape, k) {
                        let input = bias.input(shape, k, age, is_static);
                        let none = vec![0u8; f.partials.len()];
                        for seed in [None, Some(none.as_slice())] {
                            let want = Written::eval(&interp, k as u32, &input, seed).unwrap_or_else(|e| panic!("{body} {name}: interpreter: {e}"));
                            let got = Written::eval(&compiled, k as u32, &input, seed).unwrap_or_else(|e| panic!("{body} {name}: compiled: {e}"));
                            let at = format!("{body} {name} at ({}, {}) last {} AGE {age} STATIC {is_static} seeded {}", bias.first, bias.second, bias.last, seed.is_none());
                            assert_eq!(got.cells, want.cells, "written cells differ: {at}");
                            assert_eq!(got.jac, want.jac, "partials differ as f64: {at}");
                            assert_eq!(got.jac_bits(), want.jac_bits(), "partials differ in bits: {at}");
                            seen.calls += 1;
                        }
                        seen.limited += usize::from(f.name.as_str().starts_with("limit"));
                    }
                }
            }
        }
        seen
    }
}

/// What `compare` ran: the functions, the calls, and the calls of a `limit` function.
struct Compared {
    functions: usize,
    calls: usize,
    limited: usize,
}

#[test]
fn every_m1_function_gives_the_interpreters_bytes() {
    let both = Both::five();
    let seen = BackendComparison::compare(&both, &Bias::POINTS);
    // resistor 2, capacitor 2, vsource 2, isource 2, diode 5 (one of them the limit)
    assert_eq!(seen.functions, 13);
    assert_eq!(seen.limited, 3 * 6, "the diode's limit: three points, six AGE/STATIC cases (each run with and without seeds)");
    assert!(seen.calls > 100, "{} calls compared", seen.calls);

    // verification F4: 300 seeded points more, every arm of pnjlim among them
    let sweep = Bias::sweep(300);
    let arms: std::collections::BTreeSet<&str> = sweep.iter().map(Bias::pnjlim_arm).collect();
    assert_eq!(arms.len(), 8, "the sweep reaches every arm of pnjlim: {arms:?}");
    assert!(sweep.iter().any(|b| b.last < 0.0) && sweep.iter().any(|b| b.last > 0.73), "LAST on both sides of 0 and of vcrit");
    assert!(sweep.iter().any(|b| b.first - b.second > 15.0), "a bias where the diode's exponential passes 1e250");
    let swept = BackendComparison::compare(&both, &sweep);
    assert_eq!(swept.functions, 13);
    assert_eq!(swept.limited, 300 * 6, "the diode's limit at every swept point, six AGE/STATIC cases each");
}

#[test]
fn every_compiled_function_passes_verify_partials() {
    let both = Both::five();
    let mut checked = 0;
    for body in both.bodies() {
        let handle = Handle::create(both.compiled_module(&body), &[]).unwrap_or_else(|e| panic!("create {body}: {e}"));
        let shape = handle.shape();
        for (k, f) in shape.functions.iter().enumerate() {
            let mut exact = 0;
            let mut points = 0;
            for bias in &Bias::POINTS {
                // LAST held at the proposed value so pnjlim returns it: LIMITED = PROPOSED.
                let held = Bias { last: bias.first - bias.second, ..*bias };
                let input = held.input(shape, k, 2, false);
                points += 1;
                match Verdict::of_partials(&handle, k as u32, &input, 1e-6) {
                    Ok(Verdict::Exact) => exact += 1,
                    Ok(other) => panic!("{body} {} at {}: {other:?}", f.name.as_str(), bias.first),
                    Err(e) => panic!("{body} {} at {} faulted: {e}", f.name.as_str(), bias.first),
                }
            }
            assert_eq!(exact, points, "{body} {}", f.name.as_str());
            checked += 1;
        }
    }
    assert_eq!(checked, 13, "resistor 2, capacitor 2, vsource 2, isource 2, diode 5");
}

#[test]
fn a_non_finite_value_is_written_as_is() {
    // abi.md A4: a non-finite value is written, the solver fails the solve; here it is the
    // diode's flow at 100 V (exp overflows to +inf) and a NaN `is`, on both backends.
    let both = Both::five();
    let (interp, compiled) = both.instances("Diode");
    let shape = interp.shape();
    let k = shape.functions.iter().position(|f| f.name.as_str() == "terms").expect("a terms function");
    let reads = |vd: f64, is: f64| -> Vec<Vec<u8>> {
        // terms reads [vd, is, n, cj0, temperature, temperature given, GMIN]; the run set its temperature
        let mut reads: Vec<Vec<u8>> = [vd, is, 1.0, 1e-12, 300.0].iter().map(|v| v.to_le_bytes().to_vec()).collect();
        reads.push(vec![1u8]);
        reads.push(1e-12_f64.to_le_bytes().to_vec());
        reads
    };
    for (what, input) in [("overflow", reads(100.0, 1e-14)), ("NaN param", reads(0.5, f64::NAN))] {
        let want = Written::eval(&interp, k as u32, &input, None).unwrap_or_else(|e| panic!("interpreter {what}: a non-finite value is no fault: {e}"));
        let got = Written::eval(&compiled, k as u32, &input, None).unwrap_or_else(|e| panic!("compiled {what}: a non-finite value is no fault: {e}"));
        assert_eq!(got.cells.len(), want.cells.len());
        for (g, w) in got.cells.iter().zip(&want.cells) {
            let (g, w) = (f64::from_le_bytes(g[..8].try_into().unwrap()), f64::from_le_bytes(w[..8].try_into().unwrap()));
            assert_eq!(g.is_nan(), w.is_nan(), "{what}: NaN on both or neither");
            if !w.is_nan() {
                assert_eq!(g.to_bits(), w.to_bits(), "{what}");
            }
        }
    }
    let overflow = Written::eval(&compiled, k as u32, &reads(100.0, 1e-14), None).expect("terms");
    assert_eq!(overflow.real(0), f64::INFINITY, "exp(100 V / vt) overflows to +inf and is written as it is");
    assert_eq!(overflow.real(1), 1e-12 * 100.0);
    let nan = Written::eval(&compiled, k as u32, &reads(0.5, f64::NAN), None).expect("terms");
    assert!(nan.real(0).is_nan(), "a NaN param is carried through");
}

/// Bodies whose MSL names are the names the printer generates, or were before they moved to a
/// namespace of their own: the evaluator's `cx`, the evaluations `at_zero` and `at_iterate`, the
/// environment variables `temperature` and `gmin`, a limit's `limit_vd_last`, `is_static`,
/// `proposed`, `seed`, `f`, `old`, `from_seed` and `limited`, `self` beside `self_`, a name
/// ending in `_` (`cx_`, `vd_last_`), duplicate locals beside `<name>_<index>`, and a body whose
/// module would be the runtime's `rt`.
struct Names;

impl Names {
    const FILE: &'static str = "names.msl";

    fn at(line: u32) -> Origin {
        Origin { file: Self::FILE.into(), line, col: 5 }
    }

    fn local(name: &str) -> Local {
        Local { name: name.into(), ty: ValueType::F64, mutable: false }
    }

    fn param(name: &str, default: f64, line: u32) -> Param {
        Param { name: name.into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(default))), check: None, structural: false, origin: Self::at(line) }
    }

    fn port(name: &str, nature: piperine_ir::NatureId) -> BodyNode {
        BodyNode { name: name.into(), nature, role: NodeRole::Port, init: None, origin: Self::at(1) }
    }

    fn contribute(b: &mut IrBuilder, branch: u32, operator: Operator, value: piperine_ir::ExprId) -> piperine_ir::StmtId {
        b.stmt(Stmt::Contribute { branch, quantity: QuantityRole::Flow, addends: vec![Addend { operator, negated: false, value }] })
    }

    /// `fn twice(x: f64) -> f64 { let y = x * 2.0; let y = y + 1.0; let y_1 = y * 3.0; let y_2 = y_1 - x; y + y_1 + y_2 }`:
    /// the two `y` are locals 1 and 2, whose old printed names `y_1`, `y_2` were locals 3 and 4.
    fn twice(b: &mut IrBuilder) -> FunctionId {
        let mut e = Ex::new(b, Self::FILE, 2, 5);
        let (x, two) = (e.local(0), e.real(2.0));
        let init = e.binary(BinaryOp::Mul, x, two);
        let s1 = e.b.stmt(Stmt::Let { local: 1, init });
        let (y, one) = (e.local(1), e.real(1.0));
        let init = e.binary(BinaryOp::Add, y, one);
        let s2 = e.b.stmt(Stmt::Let { local: 2, init });
        let (y, three) = (e.local(2), e.real(3.0));
        let init = e.binary(BinaryOp::Mul, y, three);
        let s3 = e.b.stmt(Stmt::Let { local: 3, init });
        let (y1, x) = (e.local(3), e.local(0));
        let init = e.binary(BinaryOp::Sub, y1, x);
        let s4 = e.b.stmt(Stmt::Let { local: 4, init });
        let (y, y1) = (e.local(2), e.local(3));
        let sum = e.binary(BinaryOp::Add, y, y1);
        let y2 = e.local(4);
        let value = e.binary(BinaryOp::Add, sum, y2);
        let body = e.block(vec![s1, s2, s3, s4], Some(value), ValueType::F64);
        b.function(FunctionSpec {
            name: "twice".into(),
            origin: Self::at(2),
            params: vec![Self::local("x")],
            ret: ValueType::F64,
            locals: ["x", "y", "y", "y_1", "y_2"].into_iter().map(Self::local).collect(),
            body,
        })
    }

    /// `module Rt(cx, self) { self_, at_zero, at_iterate, temperature, cx_, gmin }` with
    /// `let proposed = twice(v(cx, self)) / self_; let seed = proposed * at_zero;
    /// let seed = seed + temperature * temperature() * 1e-3; let seed_1 = seed * cx_ + gmin * gmin();
    /// let f = (seed_1 * 1e-3).exp() * at_iterate; i(cx, self) +> f + seed_1; i(cx) +> v(cx) / at_zero;`:
    /// the first term is evaluated at the iterate, the second (affine) with the pins read as 0.
    fn rt(b: &mut IrBuilder) -> BodyId {
        let el = Devices::electrical(b);
        let twice = Self::twice(b);
        let mut e = Ex::new(b, Self::FILE, 10, 5);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        let call = e.call(twice, vec![v]);
        let p = e.param(0);
        let init = e.binary(BinaryOp::Div, call, p);
        let s0 = e.b.stmt(Stmt::Let { local: 0, init });
        let (l, p) = (e.local(0), e.param(1));
        let init = e.binary(BinaryOp::Mul, l, p);
        let s1 = e.b.stmt(Stmt::Let { local: 1, init });
        let (l, p, t) = (e.local(1), e.param(3), e.env(Env::Temperature));
        let pt = e.binary(BinaryOp::Mul, p, t);
        let milli = e.real(1e-3);
        let scaled = e.binary(BinaryOp::Mul, pt, milli);
        let init = e.binary(BinaryOp::Add, l, scaled);
        let s2 = e.b.stmt(Stmt::Let { local: 2, init });
        let (l, p) = (e.local(2), e.param(4));
        let m = e.binary(BinaryOp::Mul, l, p);
        let (g, gmin) = (e.param(5), e.env(Env::Gmin));
        let shunt = e.binary(BinaryOp::Mul, g, gmin);
        let init = e.binary(BinaryOp::Add, m, shunt);
        let s3 = e.b.stmt(Stmt::Let { local: 3, init });
        let (l, milli) = (e.local(3), e.real(1e-3));
        let m = e.binary(BinaryOp::Mul, l, milli);
        let ex = e.math(MathFn::Exp, m);
        let p = e.param(2);
        let init = e.binary(BinaryOp::Mul, ex, p);
        let s4 = e.b.stmt(Stmt::Let { local: 4, init });
        let (f, seed_1) = (e.local(4), e.local(3));
        let value = e.binary(BinaryOp::Add, f, seed_1);
        let v = e.probe(Terminal::Node(0), Terminal::Ground);
        let p = e.param(1);
        let linear = e.binary(BinaryOp::Div, v, p);
        let c0 = Self::contribute(b, 0, Operator::Direct, value);
        let c1 = Self::contribute(b, 1, Operator::Direct, linear);
        b.body(BodySpec {
            name: "Rt".into(),
            origin: Self::at(1),
            params: vec![
                Self::param("self_", 1000.0, 3),
                Self::param("at_zero", 2.0, 4),
                Self::param("at_iterate", 0.5, 5),
                Self::param("temperature", 7.0, 6),
                Self::param("cx_", 0.25, 7),
                Self::param("gmin", 3.0, 8),
            ],
            nodes: vec![Self::port("cx", el), Self::port("self", el)],
            branches: vec![
                Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow },
                Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow },
            ],
            limits: vec![],
            stims: vec![],
            locals: ["proposed", "seed", "seed", "seed_1", "f"].into_iter().map(Self::local).collect(),
            analog: Some(vec![s0, s1, s2, s3, s4, c0, c1]),
            children: vec![],
        })
    }

    /// The diode of `tests/e2e/twins/devices.msl:31-42`, renamed: params `limit_vd_last` (is),
    /// `vd_last_` (n), `is_static` (cj0); nodes `cx` (a), `old` (k); locals `f` (nvt), `seed`
    /// (vcrit), `vd`, `limited` (id), and the closure's `proposed` (new) and `from_seed` (old).
    fn limiter(b: &mut IrBuilder) -> BodyId {
        let el = Devices::electrical(b);
        let vt = Devices::vt(b);
        let pnjlim = Devices::pnjlim(b);
        let (is, n, cj0) = (0u32, 1u32, 2u32);
        let (nvt, vcrit, vd, id) = (0u32, 1u32, 2u32, 3u32);
        let mut e = Ex::new(b, Self::FILE, 20, 5);
        let p = e.param(n);
        let kelvin = e.env(Env::Temperature);
        let call = e.call(vt, vec![kelvin]);
        let init = e.binary(BinaryOp::Mul, p, call);
        let let_nvt = e.b.stmt(Stmt::Let { local: nvt, init });
        let (l, l2, two) = (e.local(nvt), e.local(nvt), e.real(2.0));
        let root = e.math(MathFn::Sqrt, two);
        let p = e.param(is);
        let denom = e.binary(BinaryOp::Mul, root, p);
        let ratio = e.binary(BinaryOp::Div, l2, denom);
        let ln = e.math(MathFn::Ln, ratio);
        let init = e.binary(BinaryOp::Mul, l, ln);
        let let_vcrit = e.b.stmt(Stmt::Let { local: vcrit, init });
        let x = e.probe(Terminal::Node(0), Terminal::Node(1));
        let (a_new, a_old, a_nvt, a_vcrit) = (e.local(4), e.local(5), e.local(nvt), e.local(vcrit));
        let call = e.call(pnjlim, vec![a_new, a_old, a_nvt, a_vcrit]);
        let f = e.closure(vec![4, 5], call);
        let seed = e.local(vcrit);
        let init = e.limit(0, x, f, Some(seed));
        let let_vd = e.b.stmt(Stmt::Let { local: vd, init });
        let (p, v, nv) = (e.param(is), e.local(vd), e.local(nvt));
        let ratio = e.binary(BinaryOp::Div, v, nv);
        let ex = e.math(MathFn::Exp, ratio);
        let one = e.real(1.0);
        let minus = e.binary(BinaryOp::Sub, ex, one);
        let init = e.binary(BinaryOp::Mul, p, minus);
        let let_id = e.b.stmt(Stmt::Let { local: id, init });
        let (i, g, v) = (e.local(id), e.env(Env::Gmin), e.local(vd));
        let shunt = e.binary(BinaryOp::Mul, g, v);
        let direct = e.binary(BinaryOp::Add, i, shunt);
        let (c, v) = (e.param(cj0), e.local(vd));
        let charge = e.binary(BinaryOp::Mul, c, v);
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![
                Addend { operator: Operator::Direct, negated: false, value: direct },
                Addend { operator: Operator::Ddt, negated: false, value: charge },
            ],
        });
        b.body(BodySpec {
            name: "Limiter".into(),
            origin: Self::at(19),
            params: vec![Self::param("limit_vd_last", 1e-14, 20), Self::param("vd_last_", 1.0, 21), Self::param("is_static", 0.0, 22)],
            nodes: vec![Self::port("cx", el), Self::port("old", el)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![Site { name: "vd".into(), origin: Self::at(23), enabled: None }],
            stims: vec![],
            locals: ["f", "seed", "vd", "limited", "proposed", "from_seed"].into_iter().map(Self::local).collect(),
            analog: Some(vec![let_nvt, let_vcrit, let_vd, let_id, contribute]),
            children: vec![],
        })
    }
}

#[test]
fn msl_names_that_equal_generated_names_give_the_interpreters_bytes() {
    let mut b = IrBuilder::new();
    let rt = Names::rt(&mut b);
    Names::limiter(&mut b);
    let top = Devices::alone(&mut b, rt, "R", 1, vec![1000.0, 2.0, 0.5, 7.0, 0.25, 3.0]);
    let ir = b.finish(top);
    let source = Transpiler::new(std::sync::Arc::new(ir.clone())).and_then(|t| t.source()).unwrap_or_else(|e| panic!("{e}"));

    // the MSL names print as written, a name ending in `_` or reserved takes one more `_`, and the
    // printer's own names end in one `_`: `self`/`self_`, `cx`/`cx_`, the param `temperature`
    // beside the environment's `temperature_`, the two `y` beside `y_1` and `y_2`
    for line in [
        "pub mod rt_ {",
        "let cx = cx_.f64(",
        "let self_ = cx_.f64(",
        "let self__ = cx_.f64(",
        "let cx__ = cx_.f64(",
        "let temperature = cx_.f64(",
        "let temperature_ = cx_.f64(",
        "let gmin = cx_.f64(",
        "let gmin_ = cx_.f64(",
        "let y_1_ = x * R::constant(2.0);",
        "let y_2_ = y_1_ + R::constant(1.0);",
        "let y_1 = y_2_ * R::constant(3.0);",
        "let y_2 = y_1 - x;",
        "let seed_1_ = proposed * at_zero;",
        "let at_iterate_ = Self::terms_block(",
        "let at_zero_ = Self::terms_block(",
        "let limit_vd_last = cx_.f64(",
        "let vd_last__ = cx_.f64(",
        "let vd_last_ = cx_.f64(",
        "let is_static = cx_.f64(",
        "let is_static_ = cx_.bool(",
        "move |proposed, from_seed| pnjlim(proposed, from_seed, f, seed)",
    ] {
        assert!(source.contains(line), "`{line}` not in the source:\n{source}");
    }
    // every name the source imports is one an MSL name cannot print as
    for line in source.lines().filter(|l| l.starts_with("use ")) {
        let names = line.trim_end_matches(';').rsplit("::").next().unwrap_or("");
        for name in names.trim_matches(|c| c == '{' || c == '}').split(", ") {
            assert_ne!(Text::ident(name), name, "the MSL name `{name}` would print as the import of `{line}`");
        }
    }

    // and both bodies compile and give the interpreter's bytes, function by function
    let both = Both::of(ir);
    let seen = BackendComparison::compare(&both, &Bias::POINTS);
    assert_eq!(seen.functions, 3, "Rt: terms; Limiter: limit:vd, terms");
    assert_eq!(seen.limited, 3 * 6, "Limiter's limit: three points, six AGE/STATIC cases");
}
