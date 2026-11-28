//! Number widths run from MSL source on the Rust backend (composite-values phase D, CV-29 to CV-32, CV-36): one test per
//! type family, each model a table of expressions over carriers (`tests/number_widths/*.msl`, written from the table of
//! this file) elaborated by the front end, compiled on the transpiled backend and run on the solver. Every expected
//! value is computed in the test, never read back from the model: `f32` by Rust's own `f32`; the exact operations of
//! `f16` and `f128` by exact rational arithmetic over `num-bigint` (`piperine-numbers`' own test oracle, which shares no
//! code with the library); conversions by that same exact arithmetic for every pair; wide integers by `num-bigint`
//! reduced modulo `2^N`.
#![cfg(unix)]

use piperine_parity::source::widths;

use piperine_numbers_fixtures::exact::ExactRational;
use num_bigint::BigInt;
use piperine_numbers_fixtures::oracle::ExactOracle;
use piperine_abi::raw::leaf;
use piperine_ir::ValueType;
use piperine_plan::{CarrierLayout, NumberLayout};
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
    /// Whether the transcendental rows (`exp`, `sin`, `powf`, `powi`, ...) have a reference here; `f128`'s are the vectors'.
    fn transcendental(&self) -> bool {
        true
    }
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
    fn transcendental(&self) -> bool {
        self.format.bits < 128
    }
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
    /// The decimal `text` (`-3.75`, `1.5e-7`) read in the format (A.8 rule 8), by the exact reference.
    fn decimal(&self, text: &str) -> u128 {
        let format = self.reference.format();
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (mantissa, exponent) = match digits.split_once(['e', 'E']) {
            Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().expect("an exponent")),
            None => (digits, 0),
        };
        let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let numerator: BigInt = format!("{integer}{fraction}").parse().expect("digits");
        let scale = exponent - fraction.len() as i64;
        let (numerator, denominator) = if scale >= 0 { (numerator * BigInt::from(10).pow(scale as u32), BigInt::from(1)) } else { (numerator, BigInt::from(10).pow((-scale) as u32)) };
        let exact = ExactRational { numerator: if negative { -numerator } else { numerator }, denominator: denominator.magnitude().clone() };
        ExactOracle::new(format).result(&exact, negative)
    }

    /// One case of a float result.
    fn float(&self, name: &str, expression: &str, pattern: u128) -> WidthCase {
        WidthCase { name: name.into(), carrier_type: self.reference.name().into(), zero: "0.0".into(), expression: expression.into(), expected: self.reference.bytes(pattern) }
    }

    /// One case of a `bool` result.
    fn boolean(&self, name: &str, expression: &str, value: bool) -> WidthCase {
        WidthCase { name: name.into(), carrier_type: "bool".into(), zero: "false".into(), expression: expression.into(), expected: ValueBytes::of_bool(value) }
    }

    /// One case of an integer result of `bits` bits.
    fn integer(&self, name: &str, expression: &str, ty: &str, bits: u32, value: &BigInt) -> WidthCase {
        WidthCase { name: name.into(), carrier_type: ty.into(), zero: "0".into(), expression: expression.into(), expected: Conversions::integer_bytes(value, bits) }
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
        for row in ["min", "max", "atan2", "powf", "hypot"].into_iter().filter(|row| reference.transcendental() || !matches!(*row, "atan2" | "powf")) {
            cases.push(self.float(row, &format!("xa.{row}(xb)"), reference.binary(row, a, b)));
        }
        cases.push(self.float("rem_euclid", "xc.rem_euclid(xa)", reference.binary("rem_euclid", c, a)));
        if reference.transcendental() {
            cases.push(self.float("powi", "xa.powi(3)", reference.power_integer(a, 3)));
        }
        cases.push(self.float("clamp", "xc.clamp(xb, xa)", reference.clamp(c, b, a)));
        for row in ["floor", "ceil", "round", "trunc", "fract", "signum", "abs", "sin", "cos", "tan", "atan", "sinh", "cosh", "tanh"].into_iter().filter(|row| reference.transcendental() || !matches!(*row, "sin" | "cos" | "tan" | "atan" | "sinh" | "cosh" | "tanh")) {
            cases.push(self.float(row, &format!("xc.{row}()"), reference.unary(row, c)));
        }
        for row in ["sqrt", "exp", "ln", "log10", "log2"].into_iter().filter(|row| reference.transcendental() || *row == "sqrt") {
            cases.push(self.float(row, &format!("xa.{row}()"), reference.unary(row, a)));
        }
        for row in ["asin", "acos"].into_iter().filter(|_| reference.transcendental()) {
            cases.push(self.float(row, &format!("xb.{row}()"), reference.unary(row, b)));
        }
        for row in ["is_nan", "is_finite", "is_infinite"] {
            cases.push(self.boolean(row, &format!("xa.{row}()"), reference.classifies(row, a)));
        }
        self.casts(&mut cases, a, c);
        // the same expressions over consts, folded at elaboration, are the values the run commits (CV-36)
        for (row, expression) in [("k_add", "KA + KB"), ("k_mul", "KA * KB"), ("k_div", "KA / KB"), ("k_sin", "KC.sin()"), ("k_floor", "KC.floor()")].into_iter().filter(|(row, _)| reference.transcendental() || *row != "k_sin") {
            let twin = cases.iter().find(|case| case.name == row.trim_start_matches("k_")).expect("the run's twin").expected.clone();
            cases.push(WidthCase { name: row.into(), carrier_type: ty.into(), zero: "0.0".into(), expression: expression.into(), expected: twin });
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
            cases.push(WidthCase { name: format!("to_{name}"), carrier_type: name.into(), zero: "0.0".into(), expression: format!("xa as {name}"), expected: ValueBytes::of_pattern(pattern, bytes) });
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

/// The reference vectors of `piperine-numbers` (`tests/vectors/<function>.tsv`): binary128 inputs, the exact bits of the
/// correctly rounded result computed by an independent arbitrary-precision library.
struct Vectors;

/// One row: the inputs (one or two binary128 patterns, or a pattern and an `i32` exponent) and the expected pattern.
struct VectorRow {
    x: u128,
    y: Option<String>,
    expected: u128,
}

impl Vectors {
    /// The rows of `function`.
    fn rows(function: &str) -> Vec<VectorRow> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../piperine-numbers/tests/vectors").join(format!("{function}.tsv"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let bits = |field: &str| u128::from_str_radix(field, 16).expect("32 hex digits");
        text.lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .map(|line| {
                let fields: Vec<&str> = line.split('\t').collect();
                match fields.as_slice() {
                    [x, expected] => VectorRow { x: bits(x), y: None, expected: bits(expected) },
                    [x, y, expected] => VectorRow { x: bits(x), y: Some((*y).to_string()), expected: bits(expected) },
                    other => panic!("a row of {function}.tsv has {} fields", other.len()),
                }
            })
            .collect()
    }
}

/// The `f128` rows the exact oracle does not cover: every transcendental method on inputs drawn from the vectors.
struct VectorTable;

impl VectorTable {
    /// How many vectors each method is run on (CV-31 AC 1).
    const PER_METHOD: usize = 50;

    /// The table: for each method, `PER_METHOD` rows spread evenly over its vector file, among those whose inputs a
    /// source literal can write (finite, within `f64`'s range, A.2 rule 8); the literal's text is checked to read back to
    /// the row's bits by the exact reader.
    fn bench() -> WidthBench {
        let table = FloatTable { reference: &ExactReference { format: BinaryFormat::QUAD, name: "f128" } };
        let mut inputs = Vec::new();
        let mut cases = Vec::new();
        for (method, function) in [("exp", "exp"), ("ln", "ln"), ("log10", "log10"), ("log2", "log2"), ("sin", "sin"), ("cos", "cos"), ("tan", "tan"), ("asin", "asin"), ("acos", "acos"), ("atan", "atan"), ("sinh", "sinh"), ("cosh", "cosh"), ("tanh", "tanh"), ("atan2", "atan2"), ("powf", "powf"), ("powi", "powi")] {
            let rows: Vec<VectorRow> = Vectors::rows(function);
            assert!(rows.len() >= Self::PER_METHOD, "{function}: {} vectors", rows.len());
            let step = rows.len() / Self::PER_METHOD;
            for (k, row) in rows.iter().step_by(step).take(Self::PER_METHOD).enumerate() {
                let x = format!("x_{method}_{k}");
                inputs.push((x.clone(), "f128".to_string(), Self::text(row.x)));
                let expression = match (&row.y, function) {
                    (Some(exponent), "powi") => format!("{x}.powi({exponent})"),
                    (Some(y), _) => {
                        let y_name = format!("y_{method}_{k}");
                        inputs.push((y_name.clone(), "f128".to_string(), Self::text(u128::from_str_radix(y, 16).expect("a pattern"))));
                        format!("{x}.{method}({y_name})")
                    }
                    (None, _) => format!("{x}.{method}()"),
                };
                cases.push(table.float(&format!("{method}_{k}"), &expression, row.expected));
            }
        }
        WidthBench {
            file: "f128_vectors.msl",
            about: format!("`f128` transcendental methods from source (composite-values CV-31 AC 1, Appendix H.2 rule 6): {} vectors of each, drawn evenly from `piperine-numbers/tests/vectors`.", Self::PER_METHOD),
            constants: Vec::new(),
            inputs,
            cases,
        }
    }

    /// The source of a pattern: its decimal text as an MSL float literal (a fraction point always present), or, for the
    /// values no literal spells, the division that gives them (`1.0 / 0.0` is infinite, `0.0 / 0.0` is NaN).
    fn text(pattern: u128) -> String {
        let oracle = ExactOracle::new(BinaryFormat::QUAD);
        if oracle.is_nan(pattern) {
            return "0.0 / 0.0".to_string();
        }
        if oracle.is_infinite(pattern) {
            return if pattern & BinaryFormat::QUAD.sign_bit() == 0 { "1.0 / 0.0" } else { "-1.0 / 0.0" }.to_string();
        }
        let text = piperine_numbers::decimal::DecimalText::shortest_scientific(pattern, BinaryFormat::QUAD);
        let (mantissa, exponent) = match text.split_once(['e', 'E']) {
            Some((mantissa, exponent)) => (mantissa.to_string(), format!("e{exponent}")),
            None => (text.clone(), String::new()),
        };
        let mantissa = if mantissa.contains('.') { mantissa } else { format!("{mantissa}.0") };
        format!("{mantissa}{exponent}")
    }
}

#[test]
fn f128_is_ieee_binary128_bit_for_bit_in_every_operation_and_method() {
    // CV-31 AC 1, user Q4 (a): `f128` arithmetic, comparisons, `as` and the exact methods give the correctly rounded
    // binary128 value by exact rational arithmetic; every transcendental method gives the bits of the independent
    // vectors, 50 inputs of each drawn from across its file (zeros, subnormals, exact cases, random and extreme values)
    FloatTable { reference: &ExactReference { format: BinaryFormat::QUAD, name: "f128" } }.bench("f128.msl").assert_first_edge();
    VectorTable::bench().assert_first_edge();
}

/// What the model of a table declares as carriers: the pin layout each carrier of a type has.
struct PinReading;

impl PinReading {
    /// The `(leaf, count)` fields of the pin of the carrier `name` of `Bench` in `file`, and the carrier's IR type.
    fn fields(file: &str, name: &str) -> (ValueType, Vec<(u32, u32)>) {
        let ir = widths::WidthBench::project(file).elaborate("Bench").unwrap_or_else(|r| panic!("{r}"));
        let bench = ir.bodies().find(|body| body.name() == "Bench").expect("Bench");
        let carrier = bench.carriers().iter().find(|carrier| carrier.name == name).unwrap_or_else(|| panic!("no carrier {name}"));
        match NumberLayout::field_of(&carrier.ty) {
            // an integer wider than 64 bits is one `U64` field of `ceil(N/64)` limbs
            Some((leaf, count, _)) => (carrier.ty.clone(), vec![(leaf, count)]),
            None => {
                let layout = CarrierLayout::of(&carrier.ty).expect("a one-leaf pin");
                (carrier.ty.clone(), layout.fields.iter().map(|field| (field.leaf, field.count)).collect())
            }
        }
    }
}

#[test]
fn a_carried_f32_is_the_f32_leaf() {
    // CV-29 AC 2, abi.md §3.1 P3: a `Bit<f32>` pin is the `F32` leaf of count 1, and the value committed reads back as the
    // four little-endian bytes of the `f32` (the table's `add` is `2.5 + 0.1`)
    assert_eq!(PinReading::fields("f32.msl", "xa"), (ValueType::F32, vec![(leaf::F32, 1)]));
    let committed = widths::WidthBench::first_commits("f32.msl", &["add"]);
    assert_eq!(committed, vec![(2.5f32 + 0.1f32).to_le_bytes().to_vec()]);
}

#[test]
fn a_carried_f128_is_a_u64_field_of_count_two() {
    // CV-31 AC 4, §2: a `Bit<f128>` pin is one `U64` field of count 2 (the 16 bytes of the binary128 bits, low limb first),
    // and `2.5 + 0.1` committed reads back as the exact binary128 sum of the two
    assert_eq!(PinReading::fields("f128.msl", "xa"), (ValueType::F128, vec![(leaf::U64, 2)]));
    let committed = widths::WidthBench::first_commits("f128.msl", &["add"]);
    let wanted = ExactOracle::new(BinaryFormat::QUAD).add(FloatTable { reference: &ExactReference { format: BinaryFormat::QUAD, name: "f128" } }.decimal("2.5"), FloatTable { reference: &ExactReference { format: BinaryFormat::QUAD, name: "f128" } }.decimal("0.1"));
    assert_eq!(committed, vec![wanted.to_le_bytes().to_vec()]);
}

/// An integer type of `bits` bits by exact `num-bigint` arithmetic reduced modulo `2^bits`.
struct IntegerReference {
    bits: u32,
    signed: bool,
}

impl IntegerReference {
    /// The MSL name: `UInt<200>`, `Int<100>`.
    fn msl(&self) -> String {
        format!("{}Int<{}>", if self.signed { "" } else { "U" }, self.bits)
    }

    /// The prefix of the carriers of the type in the model: `u200`, `i100`.
    fn prefix(&self) -> String {
        format!("{}{}", if self.signed { "i" } else { "u" }, self.bits)
    }

    /// `value` wrapped to the type (§1 rule 6).
    fn wrap(&self, value: &BigInt) -> BigInt {
        let modulus = BigInt::from(1) << self.bits;
        let low = ((value % &modulus) + &modulus) % &modulus;
        if self.signed && low >= (BigInt::from(1) << (self.bits - 1)) { low - modulus } else { low }
    }

    /// The least and the greatest value.
    fn range(&self) -> (BigInt, BigInt) {
        Conversions::range(self.bits, self.signed)
    }

    /// `value` held in the range of the type.
    fn saturate(&self, value: &BigInt) -> BigInt {
        let (low, high) = self.range();
        value.clone().max(low).min(high)
    }

    /// The bytes of a carrier holding `value`.
    fn bytes(&self, value: &BigInt) -> Vec<u8> {
        Conversions::integer_bytes(value, self.bits)
    }

    /// One case of this type's value.
    fn case(&self, name: &str, expression: &str, value: &BigInt) -> WidthCase {
        WidthCase { name: format!("{}_{name}", self.prefix()), carrier_type: self.msl(), zero: "0".into(), expression: expression.into(), expected: self.bytes(value) }
    }

    /// One `bool` case.
    fn boolean(&self, name: &str, expression: &str, value: bool) -> WidthCase {
        WidthCase { name: format!("{}_{name}", self.prefix()), carrier_type: "bool".into(), zero: "false".into(), expression: expression.into(), expected: ValueBytes::of_bool(value) }
    }

    /// Every operator, shift, comparison, H.1 method and `as` of the type over the inputs `{p}a`, `{p}b`, `{p}c`.
    fn cases(&self) -> Vec<WidthCase> {
        let p = self.prefix();
        let (a, b, c) = (self.magnitude(), BigInt::from(7), BigInt::from(if self.signed { -3 } else { 3 }));
        let (xa, xb, xc) = (format!("{p}a"), format!("{p}b"), format!("{p}c"));
        let mut cases = Vec::new();
        for (name, symbol, value) in [("add", "+", &a + &b), ("sub", "-", &a - &b), ("mul", "*", &a * &b), ("div", "/", &a / &b), ("rem", "%", &a % &b), ("and", "&", &a & &b), ("or", "|", &a | &b), ("xor", "^", &a ^ &b)] {
            cases.push(self.case(name, &format!("{xa} {symbol} {xb}"), &self.wrap(&value)));
        }
        // a signed negative operand: the sign of the quotient, of the remainder, and the shift of a negative value
        if self.signed {
            for (name, symbol, value) in [("div_neg", "/", &c / &b), ("rem_neg", "%", &c % &b)] {
                cases.push(self.case(name, &format!("{xc} {symbol} {xb}"), &self.wrap(&value)));
            }
        }
        for (name, symbol, holds) in [("lt", "<", a < b), ("le", "<=", a <= b), ("gt", ">", a > b), ("ge", ">=", a >= b), ("eq", "==", a == b), ("ne", "!=", a != b)] {
            cases.push(self.boolean(name, &format!("{xa} {symbol} {xb}"), holds));
        }
        cases.push(self.case("neg", &format!("-{xa}"), &self.wrap(&-&a)));
        cases.push(self.case("not", &format!("!{xa}"), &self.wrap(&!&a)));
        for (name, amount) in [("shl", 70u32), ("shl_edge", self.bits), ("shr", 3), ("shr_edge", self.bits)] {
            let (symbol, value) = if name.starts_with("shl") { ("<<", self.wrap(&(&a << amount))) } else { (">>", self.wrap(&(&a >> amount))) };
            cases.push(self.case(name, &format!("{xa} {symbol} {amount}u32"), &value));
        }
        cases.push(self.case("shr_neg", &format!("{xc} >> 1u8"), &self.wrap(&(&c >> 1u32))));
        cases.push(self.case("min", &format!("{xa}.min({xb})"), &a.clone().min(b.clone())));
        cases.push(self.case("max", &format!("{xa}.max({xb})"), &a.clone().max(b.clone())));
        cases.push(self.case("clamp", &format!("{xa}.clamp({xc}, {xb})"), &a.clone().max(c.clone()).min(b.clone())));
        cases.push(self.case("pow", &format!("{xb}.pow(40)"), &self.wrap(&b.pow(40))));
        cases.push(self.case("rem_euclid", &format!("{xc}.rem_euclid({xb})"), &self.wrap(&Self::euclid(&c, &b))));
        cases.push(self.case("saturating_add", &format!("{xa}.saturating_add({xa})"), &self.saturate(&(&a + &a))));
        cases.push(self.case("saturating_sub", &format!("{xb}.saturating_sub({xa})"), &self.saturate(&(&b - &a))));
        cases.push(self.case("saturating_mul", &format!("{xa}.saturating_mul({xa})"), &self.saturate(&(&a * &a))));
        if self.signed {
            cases.push(self.case("abs", &format!("{xc}.abs()"), &self.wrap(&-&c)));
            cases.push(self.case("signum", &format!("{xc}.signum()"), &BigInt::from(-1)));
        }
        self.casts(&mut cases, &a);
        cases
    }

    /// The Euclidean remainder: `0 <= r < |d|`.
    fn euclid(value: &BigInt, divisor: &BigInt) -> BigInt {
        let magnitude = BigInt::from(divisor.magnitude().clone());
        ((value % &magnitude) + &magnitude) % &magnitude
    }

    /// Every `as` of the type: to narrow, wide and float types, and from bool, narrow integers and floats.
    fn casts(&self, cases: &mut Vec<WidthCase>, a: &BigInt) {
        let xa = format!("{}a", self.prefix());
        let narrow = IntegerReference { bits: 8, signed: false };
        cases.push(WidthCase { name: format!("{}_to_u8", self.prefix()), carrier_type: "u8".into(), zero: "0".into(), expression: format!("{xa} as u8"), expected: narrow.bytes(&narrow.wrap(a)) });
        for target in [IntegerReference { bits: 130, signed: true }, IntegerReference { bits: 70, signed: false }] {
            let name = format!("{}_to_{}", self.prefix(), target.prefix());
            cases.push(WidthCase { name, carrier_type: target.msl(), zero: "0".into(), expression: format!("{xa} as {}", target.msl()), expected: target.bytes(&target.wrap(a)) });
        }
        for (name, format, bytes) in [("f32", BinaryFormat::SINGLE, 4usize), ("f64", BinaryFormat::DOUBLE, 8), ("f128", BinaryFormat::QUAD, 16)] {
            let pattern = Conversions::integer_to_float(a, format);
            cases.push(WidthCase { name: format!("{}_to_{name}", self.prefix()), carrier_type: name.into(), zero: "0.0".into(), expression: format!("{xa} as {name}"), expected: ValueBytes::of_pattern(pattern, bytes) });
        }
        let big = self.wrap(&(a << (self.bits.saturating_sub(10))));
        let pattern = Conversions::integer_to_float(&big, BinaryFormat::SINGLE);
        cases.push(WidthCase { name: format!("{}_big_to_f32", self.prefix()), carrier_type: "f32".into(), zero: "0.0".into(), expression: format!("({xa} << {}u32) as f32", self.bits.saturating_sub(10)), expected: ValueBytes::of_pattern(pattern, 4) });
        let from_bool = BigInt::from(1);
        cases.push(self.case("from_bool", &format!("{}t as {}", self.prefix(), self.msl()), &from_bool));
        let from_narrow = self.wrap(&BigInt::from(-5));
        cases.push(self.case("from_i8", &format!("{}n as {}", self.prefix(), self.msl()), &from_narrow));
        for (name, value) in [("from_f64", 1.5e30f64), ("from_f64_negative", -2.5e20f64)] {
            let input = if value < 0.0 { format!("{}g", self.prefix()) } else { format!("{}f", self.prefix()) };
            let converted = Conversions::float_to_integer(BinaryFormat::DOUBLE, u128::from(value.to_bits()), self.bits, self.signed);
            cases.push(self.case(name, &format!("{input} as {}", self.msl()), &converted));
        }
    }

    /// The value of the input `a`: the 128-bit magnitude cut below the sign bit, so a literal of the type holds it.
    fn magnitude(&self) -> BigInt {
        BigInt::from(A_MAGNITUDE) % (BigInt::from(1) << (self.bits - 1))
    }

    /// The inputs of the type: `a` the 128-bit magnitude, `b` 7, `c` -3 (a signed type only), and the bool, `i8`, `f64` sources.
    fn inputs(&self) -> Vec<(String, String, String)> {
        let p = self.prefix();
        let mut inputs = vec![(format!("{p}a"), self.msl(), self.magnitude().to_string()), (format!("{p}b"), self.msl(), "7".into())];
        inputs.push((format!("{p}c"), self.msl(), if self.signed { "-3".into() } else { "3".into() }));
        inputs.push((format!("{p}t"), "bool".into(), "true".into()));
        inputs.push((format!("{p}n"), "i8".into(), "-5".into()));
        inputs.push((format!("{p}f"), "f64".into(), "1.5e30".into()));
        inputs.push((format!("{p}g"), "f64".into(), "-2.5e20".into()));
        inputs
    }
}

/// The 128-bit magnitude every wide table starts from: above `2^127`, so a signed type of 128 bits wraps it to a negative value.
const A_MAGNITUDE: u128 = 0xfedc_ba98_7654_3210_0123_4567_89ab_cdef;

/// The bench of the wide integers: every width at once.
struct WideTable;

impl WideTable {
    /// The widths of CV-32 AC 1, each as `UInt` and `Int`.
    const WIDTHS: [u32; 5] = [65, 100, 128, 200, 1000];

    /// The table.
    fn bench() -> WidthBench {
        let types: Vec<IntegerReference> = Self::WIDTHS.iter().flat_map(|bits| [IntegerReference { bits: *bits, signed: false }, IntegerReference { bits: *bits, signed: true }]).collect();
        WidthBench {
            file: "wide.msl",
            about: "Integers wider than 64 bits from source (composite-values CV-32, part I §1 rules 5 and 6, Appendix H.1): every operator, shift, comparison, method and `as` at 65, 100, 128, 200 and 1000 bits.".into(),
            constants: Vec::new(),
            inputs: types.iter().flat_map(IntegerReference::inputs).collect(),
            cases: types.iter().flat_map(IntegerReference::cases).collect(),
        }
    }
}

#[test]
fn wide_integers_wrap_modulo_two_to_the_n_in_every_operation() {
    // CV-32 AC 1, §1 rule 6: at 65, 100, 128, 200 and 1000 bits every operator wraps modulo 2^N, a shift of the width or
    // more gives 0 (or the sign), `/` truncates and `%` takes the dividend's sign, every H.1 method and `as` gives the
    // value `num-bigint` gives reduced to the type, and a `Bit<UInt<200>>` pin is one `U64` field of count 4
    WideTable::bench().assert_first_edge();
    assert_eq!(PinReading::fields("wide.msl", "u200_add"), (ValueType::UInt(200), vec![(leaf::U64, 4)]));
    assert_eq!(PinReading::fields("wide.msl", "i100_add"), (ValueType::Int(100), vec![(leaf::U64, 2)]));
}

#[test]
fn a_wide_integer_division_by_zero_fails_the_analysis_naming_it() {
    // §1 rule 11: `/` and `%` of a wide integer by zero panic naming the operation and the type
    let fault = widths::WidthBench::fault("wide_division_by_zero.msl", "Bench");
    assert!(fault.contains("integer division by zero") && fault.contains("UInt<200>"), "{fault}");
}

#[test]
fn a_wide_literal_past_its_type_is_e0909() {
    // CV-32 AC 3, A.2 rule 8: `UInt<70>` holds up to 2^70 - 1, so a literal of 2^70 is `E0909` at elaboration
    let report = widths::WidthBench::project("wide_literal_past_its_type.msl").elaborate("Bench").expect_err("a literal past its type is refused");
    assert!(report.to_string().contains("E0909"), "{report}");
}

#[test]
fn a_param_of_each_width_holds_its_exact_default() {
    // CV-29 to CV-32, §3.2: a param of type `f32`, `f16`, `f128` or `UInt<200>` is a pin of that type's own bytes, so a
    // reaction that copies it to a carrier commits the exact default: `1.5f32`, the nearest binary16 and binary128 to 0.1
    // (A.8 rule 8, by the exact reader) and 12345 over four limbs
    let quad = FloatTable { reference: &ExactReference { format: BinaryFormat::QUAD, name: "f128" } };
    let half = FloatTable { reference: &ExactReference { format: BinaryFormat::HALF, name: "f16" } };
    let committed = widths::WidthBench::first_commits("params.msl", &["rp", "rh", "rw", "rq"]);
    assert_eq!(committed[0], 1.5f32.to_le_bytes().to_vec());
    assert_eq!(committed[1], ValueBytes::of_pattern(half.decimal("0.1"), 2));
    assert_eq!(committed[2], ValueBytes::of_limbs(&[12345, 0, 0, 0]));
    assert_eq!(committed[3], ValueBytes::of_pattern(quad.decimal("0.1"), 16));
}
