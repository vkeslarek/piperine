//! The compiled functions against the interpreter's (feature `ir-rust`, RS-06 AC 1, 2 and 7;
//! checks C33, C35, C41): every function of the five M1 bodies is evaluated through
//! `caller::Handle` on the interpreter's module and on the compiled one with the same input
//! bytes, at three bias points, the six `AGE`/`STATIC` cases of a `limit` included, with every
//! partial seeded and with none; the written cells are equal bit for bit and the partials equal
//! as `f64`. Every compiled function passes `verify_partials` at `rel_tol` 1e-6. A non-finite
//! value is written as it is, on both backends.
#![cfg(unix)]

use piperine_abi::caller::Handle;
use piperine_abi::device::{Access, cell_lens};
use piperine_abi::raw::{Cell, Shape, ambient, cell, part, role};
use piperine_abi::verify::{Verdict, verify_partials};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/both.rs"]
mod both;

use both::{Both, Written};

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
        let lens = cell_lens(shape, &f.reads, Access::FunctionRead).expect("read lens");
        f.reads.iter().zip(&lens).map(|(c, len)| self.bytes(shape, c, *len, age, is_static)).collect()
    }
}

/// The `AGE`/`STATIC` cases of a limit (abi.md A2): all six.
const LIMIT_CASES: [(u8, bool); 6] = [(0, false), (0, true), (1, false), (1, true), (2, false), (2, true)];

/// The cases a function is run over: the six when it reads `AGE` or `STATIC`, else one.
fn cases(shape: &Shape<'_>, k: usize) -> Vec<(u8, bool)> {
    let f = shape.functions.get(k).expect("a function");
    let limited = f.reads.iter().any(|c| (c.kind == cell::INPUT && c.part == part::AGE) || (c.kind == cell::AMBIENT && c.index == ambient::STATIC));
    if limited { LIMIT_CASES.to_vec() } else { vec![(2, false)] }
}

#[test]
fn every_m1_function_gives_the_interpreters_bytes() {
    let both = Both::five();
    let mut functions = 0;
    let mut calls = 0;
    let mut limited = 0;
    for body in both.bodies() {
        let (interp, compiled) = both.instances(&body);
        let shape = interp.shape();
        assert_eq!(shape.functions.len(), compiled.shape().functions.len(), "{body}: one function list");
        for (k, f) in shape.functions.iter().enumerate() {
            let name = f.name.as_str().to_string();
            assert_eq!(name, compiled.shape().functions.get(k).expect("a function").name.as_str(), "{body}: function {k}");
            functions += 1;
            for bias in &Bias::POINTS {
                for (age, is_static) in cases(shape, k) {
                    let input = bias.input(shape, k, age, is_static);
                    let none = vec![0u8; f.partials.len()];
                    for seed in [None, Some(none.as_slice())] {
                        let want = Written::eval(&interp, k as u32, &input, seed).unwrap_or_else(|e| panic!("{body} {name}: interpreter: {e}"));
                        let got = Written::eval(&compiled, k as u32, &input, seed).unwrap_or_else(|e| panic!("{body} {name}: compiled: {e}"));
                        let at = format!("{body} {name} at ({}, {}) last {} AGE {age} STATIC {is_static} seeded {}", bias.first, bias.second, bias.last, seed.is_none());
                        assert_eq!(got.cells, want.cells, "written cells differ: {at}");
                        assert_eq!(got.jac, want.jac, "partials differ as f64: {at}");
                        assert_eq!(got.jac_bits(), want.jac_bits(), "partials differ in bits: {at}");
                        calls += 1;
                    }
                    limited += usize::from(f.name.as_str().starts_with("limit"));
                }
            }
        }
    }
    // resistor 2, capacitor 2, vsource 2, isource 2, diode 5 (one of them the limit)
    assert_eq!(functions, 13);
    assert_eq!(limited, 3 * 6, "the diode's limit: three points, six AGE/STATIC cases (each run with and without seeds)");
    assert!(calls > 100, "{calls} calls compared");
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
                match verify_partials(&handle, k as u32, &input, 1e-6) {
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
        // terms reads [vd, is, n, cj0, temperature, GMIN]
        [vd, is, 1.0, 1e-12, 300.0, 1e-12].iter().map(|v| v.to_le_bytes().to_vec()).collect()
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
