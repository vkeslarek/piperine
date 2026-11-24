//! Number widths run from MSL source on the Rust backend (composite-values phase D, CV-29 to CV-32, CV-36): one test per
//! type family, each model a table of expressions over carriers (`tests/number_widths/*.msl`, written from the table of
//! this file) elaborated by the front end, compiled on the transpiled backend and run on the solver. Every expected
//! value is computed in the test, never read back from the model: `f32` by Rust's own `f32`; the exact operations of
//! `f16` and `f128` by exact rational arithmetic over `num-bigint` (`piperine-numbers`' own test oracle, which shares no
//! code with the library); conversions by that same exact arithmetic for every pair; wide integers by `num-bigint`
//! reduced modulo `2^N`.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/run.rs"]
mod run;
#[allow(dead_code)]
#[path = "source/widths.rs"]
mod widths;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/exact.rs"]
mod exact;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/exact_nearest.rs"]
mod exact_nearest;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/exact_algebra.rs"]
mod exact_algebra;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/exact_integers.rs"]
mod exact_integers;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/exact_parts.rs"]
mod exact_parts;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/exact_root.rs"]
mod exact_root;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/exact_values.rs"]
mod exact_values;
#[allow(dead_code)]
#[path = "../../piperine-numbers/tests/support/oracle.rs"]
mod oracle;

use exact::ExactRational;
use num_bigint::BigInt;
use oracle::ExactOracle;
use piperine_numbers::convert::NumberConversion;
use piperine_numbers::format::BinaryFormat;
use piperine_numbers::quad::Float128;
use std::cmp::Ordering;
use widths::{ValueBytes, WidthBench, WidthCase};

/// The operations of one float format on bit patterns: what a table of that format expects.
trait Reference {
    /// The MSL name of the type.
    fn name(&self) -> &'static str;
    /// The format.
    fn format(&self) -> BinaryFormat;
    /// A one-argument row of H.2 (`sin`, `floor`, `neg`, ...).
    fn unary(&self, row: &str, x: u128) -> u128;
    /// A row with one argument of the type (`min`, `atan2`, `add`, ...).
    fn binary(&self, row: &str, x: u128, y: u128) -> u128;
    /// `x.powi(n)`.
    fn power_integer(&self, x: u128, n: i32) -> u128;
    /// `x.clamp(lo, hi)`.
    fn clamp(&self, x: u128, lo: u128, hi: u128) -> u128;
    /// A comparison (`lt`, `ge`, `eq`, ...), IEEE 754: NaN is unordered.
    fn compares(&self, row: &str, x: u128, y: u128) -> bool;
    /// `is_nan`, `is_finite` or `is_infinite`.
    fn classifies(&self, row: &str, x: u128) -> bool;
    /// The bytes a carrier of the type holds for `pattern`.
    fn bytes(&self, pattern: u128) -> Vec<u8> {
        ValueBytes::of_pattern(pattern, self.format().bits as usize / 8)
    }
}

/// Rust's own `f32`: arithmetic and every H.2 method.
struct SingleReference;

impl SingleReference {
    fn value(pattern: u128) -> f32 {
        f32::from_bits(pattern as u32)
    }

    fn pattern(value: f32) -> u128 {
        u128::from(value.to_bits())
    }
}

impl Reference for SingleReference {
    fn name(&self) -> &'static str {
        "f32"
    }
    fn format(&self) -> BinaryFormat {
        BinaryFormat::SINGLE
    }
    fn unary(&self, row: &str, x: u128) -> u128 {
        let x = Self::value(x);
        Self::pattern(match row {
            "neg" => -x,
            "abs" => x.abs(),
            "sqrt" => x.sqrt(),
            "exp" => x.exp(),
            "ln" => x.ln(),
            "log10" => x.log10(),
            "log2" => x.log2(),
            "sin" => x.sin(),
            "cos" => x.cos(),
            "tan" => x.tan(),
            "asin" => x.asin(),
            "acos" => x.acos(),
            "atan" => x.atan(),
            "sinh" => x.sinh(),
            "cosh" => x.cosh(),
            "tanh" => x.tanh(),
            "floor" => x.floor(),
            "ceil" => x.ceil(),
            "round" => x.round(),
            "trunc" => x.trunc(),
            "fract" => x.fract(),
            "signum" => x.signum(),
            other => panic!("no f32 row {other}"),
        })
    }
    fn binary(&self, row: &str, x: u128, y: u128) -> u128 {
        let (x, y) = (Self::value(x), Self::value(y));
        Self::pattern(match row {
            "add" => x + y,
            "sub" => x - y,
            "mul" => x * y,
            "div" => x / y,
            "min" => x.min(y),
            "max" => x.max(y),
            "atan2" => x.atan2(y),
            "powf" => x.powf(y),
            "hypot" => x.hypot(y),
            "rem_euclid" => x.rem_euclid(y),
            other => panic!("no f32 row {other}"),
        })
    }
    fn power_integer(&self, x: u128, n: i32) -> u128 {
        Self::pattern(Self::value(x).powi(n))
    }
    fn clamp(&self, x: u128, lo: u128, hi: u128) -> u128 {
        Self::pattern(Self::value(x).clamp(Self::value(lo), Self::value(hi)))
    }
    fn compares(&self, row: &str, x: u128, y: u128) -> bool {
        let (x, y) = (Self::value(x), Self::value(y));
        match row {
            "lt" => x < y,
            "le" => x <= y,
            "gt" => x > y,
            "ge" => x >= y,
            "eq" => x == y,
            "ne" => x != y,
            other => panic!("no comparison {other}"),
        }
    }
    fn classifies(&self, row: &str, x: u128) -> bool {
        let x = Self::value(x);
        match row {
            "is_nan" => x.is_nan(),
            "is_finite" => x.is_finite(),
            "is_infinite" => x.is_infinite(),
            other => panic!("no row {other}"),
        }
    }
}

/// An IEEE 754 format other than `f32` by the exact oracle: the exact rows (`+ - * /`, `sqrt`, `floor`, `min`, `hypot`, ...)
/// from `ExactOracle`, every transcendental row of `f16` from the `f128` library rounded once to `f16` (that library is
/// proven against the independent vectors by `piperine-numbers`' own tests; the guard below refuses a result near a
/// midpoint). `f128`'s transcendental rows are checked against the vectors, not here.
struct ExactReference {
    format: BinaryFormat,
    name: &'static str,
}

impl ExactReference {
    /// The `f128` pattern of `x`, exact (a binary16 value is a binary128 value).
    fn widened(&self, x: u128) -> Float128 {
        Float128::from_bits(Conversions::float_to_float(self.format, x, BinaryFormat::QUAD))
    }

    /// A transcendental row, computed in `f128` and rounded once to this format; a result within eight `f128` ulps of a
    /// midpoint of the format is refused (the rounding would depend on the library's last bits).
    fn transcendental(&self, row: &str, x: u128, y: Option<u128>) -> u128 {
        assert!(self.format.bits < 128, "the {row} row of {} is checked against the reference vectors", self.name);
        let (a, b) = (self.widened(x), y.map(|y| self.widened(y)));
        let wide = match (row, b) {
            ("exp", _) => a.exp(),
            ("ln", _) => a.ln(),
            ("log10", _) => a.log10(),
            ("log2", _) => a.log2(),
            ("sin", _) => a.sin(),
            ("cos", _) => a.cos(),
            ("tan", _) => a.tan(),
            ("asin", _) => a.asin(),
            ("acos", _) => a.acos(),
            ("atan", _) => a.atan(),
            ("sinh", _) => a.sinh(),
            ("cosh", _) => a.cosh(),
            ("tanh", _) => a.tanh(),
            ("atan2", Some(b)) => a.atan2(b),
            ("powf", Some(b)) => a.powf(b),
            (other, _) => panic!("no transcendental row {other}"),
        };
        self.round_once(wide.to_bits())
    }

    /// The pattern of the `f128` pattern `wide` in this format, refusing a result near a midpoint.
    fn round_once(&self, wide: u128) -> u128 {
        let round = |pattern: u128| NumberConversion::float_to_float(BinaryFormat::QUAD, pattern, self.format);
        let (low, high) = (round(wide - 8), round(wide + 8));
        assert!(low == high && low == round(wide), "an f128 result within 8 ulps of a midpoint of {}: {wide:#x}", self.name);
        round(wide)
    }
}

impl Reference for ExactReference {
    fn name(&self) -> &'static str {
        self.name
    }
    fn format(&self) -> BinaryFormat {
        self.format
    }
    fn unary(&self, row: &str, x: u128) -> u128 {
        let oracle = ExactOracle::new(self.format);
        match row {
            "neg" => x ^ self.format.sign_bit(),
            "abs" => x & !self.format.sign_bit(),
            "sqrt" => oracle.square_root(x),
            "floor" => oracle.floor(x),
            "ceil" => oracle.ceiling(x),
            "round" => oracle.round_half_away(x),
            "trunc" => oracle.truncate(x),
            "fract" => oracle.fraction(x),
            "signum" => oracle.signum(x),
            other => self.transcendental(other, x, None),
        }
    }
    fn binary(&self, row: &str, x: u128, y: u128) -> u128 {
        let oracle = ExactOracle::new(self.format);
        match row {
            "add" => oracle.add(x, y),
            "sub" => oracle.subtract(x, y),
            "mul" => oracle.multiply(x, y),
            "div" => oracle.divide(x, y),
            "min" => oracle.minimum(x, y),
            "max" => oracle.maximum(x, y),
            "hypot" => oracle.hypot(x, y),
            "rem_euclid" => oracle.remainder_euclid(x, y),
            other => self.transcendental(other, x, Some(y)),
        }
    }
    fn power_integer(&self, x: u128, n: i32) -> u128 {
        assert!(self.format.bits < 128, "powi of {} is checked against the reference vectors", self.name);
        self.round_once(self.widened(x).powi(n).to_bits())
    }
    fn clamp(&self, x: u128, lo: u128, hi: u128) -> u128 {
        ExactOracle::new(self.format).clamp(x, lo, hi).expect("ordered bounds")
    }
    fn compares(&self, row: &str, x: u128, y: u128) -> bool {
        let order = ExactOracle::new(self.format).compare(x, y);
        match (row, order) {
            (_, None) => row == "ne",
            ("lt", Some(order)) => order == Ordering::Less,
            ("le", Some(order)) => order != Ordering::Greater,
            ("gt", Some(order)) => order == Ordering::Greater,
            ("ge", Some(order)) => order != Ordering::Less,
            ("eq", Some(order)) => order == Ordering::Equal,
            ("ne", Some(order)) => order != Ordering::Equal,
            (other, _) => panic!("no comparison {other}"),
        }
    }
    fn classifies(&self, row: &str, x: u128) -> bool {
        let oracle = ExactOracle::new(self.format);
        match row {
            "is_nan" => oracle.is_nan(x),
            "is_infinite" => oracle.is_infinite(x),
            "is_finite" => !oracle.is_nan(x) && !oracle.is_infinite(x),
            other => panic!("no row {other}"),
        }
    }
}

/// `as` between every float and integer type, by exact rational arithmetic: each result is the
/// nearest value of its type (§1.4.5), computed from the exact value of the source.
struct Conversions;

impl Conversions {
    /// The pattern of the float `pattern` of `from` as a float of `to`, rounded once.
    fn float_to_float(from: BinaryFormat, pattern: u128, to: BinaryFormat) -> u128 {
        let (source, target) = (ExactOracle::new(from), ExactOracle::new(to));
        if source.is_nan(pattern) {
            return target.nan();
        }
        if source.is_infinite(pattern) {
            return to.infinity(source.negative(pattern));
        }
        let exact = source.value(pattern).expect("a finite value");
        target.result(&exact, source.negative(pattern))
    }

    /// The pattern of the integer `value` as a float of `to`.
    fn integer_to_float(value: &BigInt, to: BinaryFormat) -> u128 {
        ExactOracle::new(to).result(&ExactRational::integer(value), false)
    }

    /// The integer `pattern` converts to: truncated toward zero, clamped to the type, NaN as 0 (§1.4.5).
    fn float_to_integer(from: BinaryFormat, pattern: u128, bits: u32, signed: bool) -> BigInt {
        let source = ExactOracle::new(from);
        let (low, high) = Self::range(bits, signed);
        if source.is_nan(pattern) {
            return BigInt::from(0);
        }
        if source.is_infinite(pattern) {
            return if source.negative(pattern) { low } else { high };
        }
        source.value(pattern).expect("a finite value").truncated().max(low).min(high)
    }

    /// The least and the greatest value of an integer type.
    fn range(bits: u32, signed: bool) -> (BigInt, BigInt) {
        let one = BigInt::from(1);
        if signed { (-(one.clone() << (bits - 1)), (one << (bits - 1)) - 1) } else { (BigInt::from(0), (one << bits) - 1) }
    }

    /// The bytes of a carrier of an integer type of `bits` bits holding `value` (two's complement; the narrowest of 1, 2, 4, 8
    /// bytes up to 64 bits, `U64` limbs past it, abi.md §3.1).
    fn integer_bytes(value: &BigInt, bits: u32) -> Vec<u8> {
        let length = if bits > 64 { 8 * bits.div_ceil(64) as usize } else { [1usize, 2, 4, 8].into_iter().find(|bytes| bits <= 8 * *bytes as u32).unwrap_or(8) };
        let modulus = BigInt::from(1) << (8 * length);
        let wrapped = ((value % &modulus) + &modulus) % &modulus;
        let mut bytes = wrapped.to_bytes_le().1;
        bytes.resize(length, 0);
        bytes
    }
}

/// The table of one float format: operators, comparisons, every H.2 method and every `as`.
struct FloatTable<'r> {
    reference: &'r dyn Reference,
}

impl FloatTable<'_> {
    /// The decimal `text` read in the format (A.8 rule 8), by the exact reference.
    fn decimal(&self, text: &str) -> u128 {
        let format = self.reference.format();
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (integer, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let numerator: BigInt = format!("{integer}{fraction}").parse().expect("digits");
        let denominator = BigInt::from(10).pow(fraction.len() as u32);
        let exact = ExactRational { numerator: if negative { -numerator } else { numerator }, denominator: denominator.magnitude().clone() };
        ExactOracle::new(format).result(&exact, negative)
    }

    /// One case of a float result.
    fn float(&self, name: &str, expression: &str, pattern: u128) -> WidthCase {
        WidthCase { name: name.into(), ty: self.reference.name().into(), zero: "0.0".into(), expression: expression.into(), expected: self.reference.bytes(pattern) }
    }

    /// One case of a `bool` result.
    fn boolean(&self, name: &str, expression: &str, value: bool) -> WidthCase {
        WidthCase { name: name.into(), ty: "bool".into(), zero: "false".into(), expression: expression.into(), expected: ValueBytes::of_bool(value) }
    }

    /// One case of an integer result of `bits` bits.
    fn integer(&self, name: &str, expression: &str, ty: &str, bits: u32, value: &BigInt) -> WidthCase {
        WidthCase { name: name.into(), ty: ty.into(), zero: "0".into(), expression: expression.into(), expected: Conversions::integer_bytes(value, bits) }
    }

    /// The bench of the format: the inputs `xa = 2.5`, `xb = 0.1`, `xc = -3.75`, the consts `KA`, `KB`, `KC` of the same
    /// values (folded at elaboration, so a case over them is the const evaluator against the run), and the cases.
    fn bench(&self, file: &'static str) -> WidthBench {
        let reference = self.reference;
        let ty = reference.name();
        let (a, b, c) = (self.decimal("2.5"), self.decimal("0.1"), self.decimal("-3.75"));
        let mut cases = Vec::new();
        for (row, symbol) in [("add", "+"), ("sub", "-"), ("mul", "*"), ("div", "/")] {
            cases.push(self.float(row, &format!("xa {symbol} xb"), reference.binary(row, a, b)));
        }
        for (row, symbol) in [("lt", "<"), ("le", "<="), ("gt", ">"), ("ge", ">="), ("eq", "=="), ("ne", "!=")] {
            cases.push(self.boolean(row, &format!("xa {symbol} xb"), reference.compares(row, a, b)));
        }
        cases.push(self.float("neg", "-xa", reference.unary("neg", a)));
        for row in ["min", "max", "atan2", "powf", "hypot"] {
            cases.push(self.float(row, &format!("xa.{row}(xb)"), reference.binary(row, a, b)));
        }
        cases.push(self.float("rem_euclid", "xc.rem_euclid(xa)", reference.binary("rem_euclid", c, a)));
        cases.push(self.float("powi", "xa.powi(3)", reference.power_integer(a, 3)));
        cases.push(self.float("clamp", "xc.clamp(xb, xa)", reference.clamp(c, b, a)));
        for row in ["floor", "ceil", "round", "trunc", "fract", "signum", "abs", "sin", "cos", "tan", "atan", "sinh", "cosh", "tanh"] {
            cases.push(self.float(row, &format!("xc.{row}()"), reference.unary(row, c)));
        }
        for row in ["sqrt", "exp", "ln", "log10", "log2"] {
            cases.push(self.float(row, &format!("xa.{row}()"), reference.unary(row, a)));
        }
        for row in ["asin", "acos"] {
            cases.push(self.float(row, &format!("xb.{row}()"), reference.unary(row, b)));
        }
        for row in ["is_nan", "is_finite", "is_infinite"] {
            cases.push(self.boolean(row, &format!("xa.{row}()"), reference.classifies(row, a)));
        }
        self.casts(&mut cases, a, c);
        // the same expressions over consts, folded at elaboration, are the values the run commits (CV-36)
        for (row, expression) in [("k_add", "KA + KB"), ("k_mul", "KA * KB"), ("k_div", "KA / KB"), ("k_sin", "KC.sin()"), ("k_floor", "KC.floor()")] {
            let twin = cases.iter().find(|case| case.name == row.trim_start_matches("k_")).expect("the run's twin").expected.clone();
            cases.push(WidthCase { name: row.into(), ty: ty.into(), zero: "0.0".into(), expression: expression.into(), expected: twin });
        }
        WidthBench {
            file,
            about: format!("`{ty}` from source (composite-values CV-29 to CV-31, part I §1.4, Appendix H.2): operators, comparisons, methods and `as`."),
            constants: vec![format!("const KA: {ty} = 2.5;"), format!("const KB: {ty} = 0.1;"), format!("const KC: {ty} = -3.75;")],
            inputs: vec![
                ("xa".into(), ty.into(), "2.5".into()),
                ("xb".into(), ty.into(), "0.1".into()),
                ("xc".into(), ty.into(), "-3.75".into()),
                ("xf".into(), "f64".into(), "1.1".into()),
                ("xi".into(), "i32".into(), "-7".into()),
            ],
            cases,
        }
    }

    /// Every `as` of the format: to and from `f64`, to the other formats, to and from integers of 8, 32, 100 bits.
    fn casts(&self, cases: &mut Vec<WidthCase>, a: u128, c: u128) {
        let from = self.reference.format();
        let ty = self.reference.name();
        let targets = [("f64", BinaryFormat::DOUBLE, 8usize), ("f32", BinaryFormat::SINGLE, 4), ("f16", BinaryFormat::HALF, 2), ("f128", BinaryFormat::QUAD, 16)];
        for (name, format, bytes) in targets {
            if name == ty {
                continue;
            }
            let pattern = Conversions::float_to_float(from, a, format);
            cases.push(WidthCase { name: format!("to_{name}"), ty: name.into(), zero: "0.0".into(), expression: format!("xa as {name}"), expected: ValueBytes::of_pattern(pattern, bytes) });
        }
        let pattern = Conversions::float_to_float(BinaryFormat::DOUBLE, u128::from(1.1f64.to_bits()), from);
        cases.push(self.float("from_f64", &format!("xf as {ty}"), pattern));
        let pattern = Conversions::integer_to_float(&BigInt::from(-7), from);
        cases.push(self.float("from_i32", &format!("xi as {ty}"), pattern));
        for (name, bits, signed) in [("u8", 8u32, false), ("i32", 32, true), ("u100", 100, false), ("i100", 100, true)] {
            let msl = if bits == 100 { format!("{}Int<100>", if signed { "" } else { "U" }) } else { name.to_string() };
            for (case, source) in [("a", a), ("c", c)] {
                let value = Conversions::float_to_integer(from, source, bits, signed);
                cases.push(self.integer(&format!("to_{name}_{case}"), &format!("x{case} as {msl}"), &msl, bits, &value));
            }
        }
    }
}

#[test]
fn f32_computes_what_rust_f32_computes() {
    // CV-29 AC 1, H.2 rule 6: `f32` arithmetic, comparisons, every method and every `as` give the bits Rust's `f32` gives
    // (conversions: the exact nearest value), and the same expressions folded at elaboration commit the same bits (CV-36)
    let table = FloatTable { reference: &SingleReference };
    table.bench("f32.msl").assert_first_edge();
}

#[test]
fn f16_is_ieee_binary16_bit_for_bit() {
    // CV-30 AC 1, user Q4 (a): `f16` arithmetic, comparisons, every H.2 method and every `as` give the correctly rounded
    // binary16 value: the exact rows by exact rational arithmetic, the transcendental rows by the `f128` library rounded
    // once (with a guard against a near-midpoint result), conversions by exact arithmetic for every pair
    let table = FloatTable { reference: &ExactReference { format: BinaryFormat::HALF, name: "f16" } };
    table.bench("f16.msl").assert_first_edge();
}
