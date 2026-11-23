//! The binary128 transcendentals against committed reference vectors (CV-31): `tests/vectors`
//! holds, per function, inputs and the correctly rounded result computed by `generate.py` with
//! `mpmath` at 1 024 bits and rounded to binary128 by exact rational comparison. Every
//! comparison is bit for bit; there is no tolerance anywhere.

use piperine_numbers::convert::NumberConversion;
use piperine_numbers::format::BinaryFormat;
use piperine_numbers::half::Float16;
use piperine_numbers::power::ExactPower;
use piperine_numbers::quad::Float128;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;

/// The functions a vector file exists for.
const FUNCTIONS: [&str; 16] = ["exp", "ln", "log2", "log10", "powf", "powi", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh"];

/// One row of `exact.tsv`: an exact case of design-d.md and its exact result.
struct ExactRow {
    /// The function name.
    function: String,
    /// The first argument's bits.
    x: u128,
    /// The second argument: bits, a decimal `powi` exponent, or none.
    y: Option<String>,
    /// The expected bits.
    expected: u128,
}

/// The reader of the `.tsv` vector files.
struct ReferenceVectors {
    /// The directory holding the files.
    directory: PathBuf,
}

impl ReferenceVectors {
    /// The committed vectors of this crate.
    fn committed() -> Self {
        ReferenceVectors { directory: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors") }
    }

    /// The text lines of a vector file, header lines (starting with `#`) first.
    fn lines(&self, file: &str) -> Vec<String> {
        fs::read_to_string(self.directory.join(file)).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// The header lines of a file.
    fn header(&self, file: &str) -> Vec<String> {
        self.lines(file).into_iter().filter(|line| line.starts_with('#')).collect()
    }

    /// The data rows of a file, split into fields.
    fn rows(&self, file: &str) -> Vec<Vec<String>> {
        self.lines(file).into_iter().filter(|line| !line.starts_with('#') && !line.is_empty()).map(|line| line.split('\t').map(str::to_string).collect()).collect()
    }

    /// Parses a 32-digit hex field into bits.
    fn bits(field: &str) -> Option<u128> {
        (field.len() == 32).then(|| u128::from_str_radix(field, 16).ok()).flatten()
    }

    /// The `(x, expected)` rows of a one-argument function.
    fn unary(&self, function: &str) -> Vec<(u128, u128)> {
        self.rows(&format!("{function}.tsv")).iter().filter_map(|row| Some((Self::bits(row.first()?)?, Self::bits(row.get(1)?)?))).collect()
    }

    /// The `(x, y, expected)` rows of a two-argument function whose second argument is binary128.
    fn binary(&self, function: &str) -> Vec<(u128, u128, u128)> {
        self.rows(&format!("{function}.tsv")).iter().filter_map(|row| Some((Self::bits(row.first()?)?, Self::bits(row.get(1)?)?, Self::bits(row.get(2)?)?))).collect()
    }

    /// The `(x, e, expected)` rows of `powi`.
    fn integer_power(&self) -> Vec<(u128, i32, u128)> {
        self.rows("powi.tsv").iter().filter_map(|row| Some((Self::bits(row.first()?)?, row.get(1)?.parse().ok()?, Self::bits(row.get(2)?)?))).collect()
    }

    /// The rows of `exact.tsv`.
    fn exact(&self) -> Vec<ExactRow> {
        self.rows("exact.tsv").iter().filter_map(|row| Some(ExactRow { function: row.first()?.clone(), x: Self::bits(row.get(1)?)?, y: row.get(2).filter(|field| *field != "-").cloned(), expected: Self::bits(row.get(3)?)? })).collect()
    }

    /// Whether `bits` is a valid binary128 result: any pattern but a NaN that is not the canonical quiet NaN.
    fn is_valid_result(bits: u128) -> bool {
        let exponent_all_ones = (bits >> 112) & 0x7FFF == 0x7FFF;
        let fraction = bits & ((1u128 << 112) - 1);
        !exponent_all_ones || fraction == 0 || bits == 0x7FFF_8000_0000_0000_0000_0000_0000_0000
    }
}

#[test]
fn the_reference_vectors_are_complete_and_well_formed() {
    let vectors = ReferenceVectors::committed();
    for function in FUNCTIONS {
        let file = format!("{function}.tsv");
        let header = vectors.header(&file);
        assert!(header.iter().any(|line| line.contains("generator: generate.py; mpmath")), "{file} names its generator and mpmath's version");
        let fields = if matches!(function, "powf" | "powi" | "atan2") { 3 } else { 2 };
        let rows = vectors.rows(&file);
        assert!(rows.len() >= 1_000, "{file} has {} rows, at least 1 000 are expected", rows.len());
        assert!(rows.iter().all(|row| row.len() == fields), "{file}: every row has {fields} fields");
        let parsed = match function {
            "powf" | "atan2" => vectors.binary(function).len(),
            "powi" => vectors.integer_power().len(),
            _ => vectors.unary(function).len(),
        };
        assert_eq!(parsed, rows.len(), "{file}: every row parses (32 hex digits per pattern)");
        let inputs: HashSet<Vec<String>> = rows.iter().map(|row| row[..fields - 1].to_vec()).collect();
        assert_eq!(inputs.len(), rows.len(), "{file}: inputs are unique");
        let expected: Vec<u128> = rows.iter().filter_map(|row| ReferenceVectors::bits(&row[fields - 1])).collect();
        assert!(expected.iter().all(|bits| ReferenceVectors::is_valid_result(*bits)), "{file}: every expected value is a valid binary128 (a NaN is the canonical quiet NaN)");
    }
    let exact = vectors.exact();
    assert!(exact.len() >= 300, "exact.tsv has {} rows", exact.len());
    assert_eq!(exact.len(), vectors.rows("exact.tsv").len(), "every exact row parses");
    for function in ["exp", "ln", "log2", "log10", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "powf", "powi"] {
        assert!(exact.iter().any(|row| row.function == function), "exact.tsv covers {function}");
    }
    for row in &exact {
        assert!(ReferenceVectors::is_valid_result(row.expected), "exact {} {:#x}: invalid result", row.function, row.x);
        match (row.function.as_str(), row.y.as_deref()) {
            ("powi", Some(exponent)) => assert!(exponent.parse::<i32>().is_ok(), "powi exponent {exponent}"),
            (_, Some(second)) => assert!(ReferenceVectors::bits(second).is_some(), "second argument {second}"),
            (name, None) => assert!(!matches!(name, "powf" | "powi" | "atan2"), "{name} takes two arguments"),
        }
    }
}

/// Compares a library function with the vectors of its reference file and of `exact.tsv`.
struct VectorComparison {
    /// The reader of the committed vectors.
    vectors: ReferenceVectors,
}

impl VectorComparison {
    /// A comparison over the committed vectors.
    fn committed() -> Self {
        VectorComparison { vectors: ReferenceVectors::committed() }
    }

    /// Whether two results are the same value: equal bits, or both NaN.
    fn same(expected: u128, actual: Float128) -> bool {
        let nan = (expected >> 112) & 0x7FFF == 0x7FFF && expected & ((1u128 << 112) - 1) != 0;
        (nan && actual.is_nan()) || expected == actual.to_bits()
    }

    /// Checks `function` on every row of `<name>.tsv` and every `exact.tsv` row of that name.
    fn check_unary(&self, name: &str, function: fn(Float128) -> Float128) {
        let mut rows = 0;
        for (x, expected) in self.vectors.unary(name) {
            let actual = function(Float128::from_bits(x));
            assert!(Self::same(expected, actual), "{name}({x:#034x}): expected {expected:#034x}, got {:#034x}", actual.to_bits());
            rows += 1;
        }
        for row in self.vectors.exact().iter().filter(|row| row.function == name) {
            let actual = function(Float128::from_bits(row.x));
            assert!(Self::same(row.expected, actual), "exact {name}({:#034x}): expected {:#034x}, got {:#034x}", row.x, row.expected, actual.to_bits());
            rows += 1;
        }
        assert!(rows >= 1_000, "{name}: only {rows} rows compared");
    }
}

#[test]
fn exp_and_logarithms_match_the_reference_vectors_bit_for_bit() {
    let comparison = VectorComparison::committed();
    comparison.check_unary("exp", Float128::exp);
    comparison.check_unary("ln", Float128::ln);
    comparison.check_unary("log2", Float128::log2);
    comparison.check_unary("log10", Float128::log10);
}

/// Compares the two-argument functions with their vector files.
impl VectorComparison {
    /// Checks `atan2` on every row of `atan2.tsv` and the `exact.tsv` rows of that name.
    fn check_atan2(&self) {
        let mut rows = 0;
        for (y, x, expected) in self.vectors.binary("atan2") {
            let actual = Float128::from_bits(y).atan2(Float128::from_bits(x));
            assert!(Self::same(expected, actual), "atan2({y:#034x}, {x:#034x}): expected {expected:#034x}, got {:#034x}", actual.to_bits());
            rows += 1;
        }
        for row in self.vectors.exact().iter().filter(|row| row.function == "atan2") {
            let x = row.y.as_deref().and_then(ReferenceVectors::bits).unwrap_or(0);
            let actual = Float128::from_bits(row.x).atan2(Float128::from_bits(x));
            assert!(Self::same(row.expected, actual), "exact atan2({:#034x}, {x:#034x}): expected {:#034x}, got {:#034x}", row.x, row.expected, actual.to_bits());
            rows += 1;
        }
        assert!(rows >= 2_000, "only {rows} atan2 rows compared");
    }

    /// Checks `powf` on every row of `powf.tsv` and the `exact.tsv` rows of that name.
    fn check_power(&self) {
        let mut rows = 0;
        for (x, y, expected) in self.vectors.binary("powf") {
            let actual = Float128::from_bits(x).powf(Float128::from_bits(y));
            assert!(Self::same(expected, actual), "powf({x:#034x}, {y:#034x}): expected {expected:#034x}, got {:#034x}", actual.to_bits());
            rows += 1;
        }
        for row in self.vectors.exact().iter().filter(|row| row.function == "powf") {
            let y = row.y.as_deref().and_then(ReferenceVectors::bits).unwrap_or(0);
            let actual = Float128::from_bits(row.x).powf(Float128::from_bits(y));
            assert!(Self::same(row.expected, actual), "exact powf({:#034x}, {y:#034x}): expected {:#034x}, got {:#034x}", row.x, row.expected, actual.to_bits());
            rows += 1;
        }
        for (x, exponent, expected) in self.vectors.integer_power() {
            let actual = Float128::from_bits(x).powi(exponent);
            assert!(Self::same(expected, actual), "powi({x:#034x}, {exponent}): expected {expected:#034x}, got {:#034x}", actual.to_bits());
            rows += 1;
        }
        for row in self.vectors.exact().iter().filter(|row| row.function == "powi") {
            let exponent: i32 = row.y.as_deref().and_then(|text| text.parse().ok()).unwrap_or(0);
            let actual = Float128::from_bits(row.x).powi(exponent);
            assert!(Self::same(row.expected, actual), "exact powi({:#034x}, {exponent}): expected {:#034x}, got {:#034x}", row.x, row.expected, actual.to_bits());
            rows += 1;
        }
        assert!(rows >= 4_000, "only {rows} power rows compared");
    }
}

#[test]
fn powers_match_the_reference_vectors_bit_for_bit() {
    VectorComparison::committed().check_power();
}

#[test]
fn circular_functions_match_the_reference_vectors_bit_for_bit() {
    let comparison = VectorComparison::committed();
    comparison.check_unary("sin", Float128::sin);
    comparison.check_unary("cos", Float128::cos);
    comparison.check_unary("tan", Float128::tan);
}

#[test]
fn inverse_circular_functions_match_the_reference_vectors_bit_for_bit() {
    let comparison = VectorComparison::committed();
    comparison.check_unary("asin", Float128::asin);
    comparison.check_unary("acos", Float128::acos);
    comparison.check_unary("atan", Float128::atan);
    comparison.check_atan2();
}

#[test]
fn hyperbolic_functions_match_the_reference_vectors_bit_for_bit() {
    let comparison = VectorComparison::committed();
    comparison.check_unary("sinh", Float128::sinh);
    comparison.check_unary("cosh", Float128::cosh);
    comparison.check_unary("tanh", Float128::tanh);
}

/// The oracle of the `f16` transcendentals is the `f128` library (proven bit for bit above),
/// rounded to `f16`. An input whose `f128` result lies within one `f128` unit of an `f16`
/// midpoint cannot be decided by it and fails the test naming the input (an exact `powf` result
/// is the one exception: its `f128` value is exact, so a tie is a true tie).
struct HalfOracle {
    /// The failures found, collected across threads.
    failures: Mutex<Vec<String>>,
}

impl HalfOracle {
    /// An oracle with no failures yet.
    fn new() -> Self {
        HalfOracle { failures: Mutex::new(Vec::new()) }
    }

    /// The `f128` of an `f16` pattern (exact).
    fn widen(bits: u16) -> Float128 {
        Float128::from_bits(NumberConversion::float_to_float(BinaryFormat::HALF, bits as u128, BinaryFormat::QUAD))
    }

    /// The `f16` pattern an `f128` result rounds to.
    fn narrow(bits: u128) -> u16 {
        NumberConversion::float_to_float(BinaryFormat::QUAD, bits, BinaryFormat::HALF) as u16
    }

    /// The expected `f16` pattern of an `f128` result, or the ambiguity message.
    fn expected(result: Float128, exact: bool) -> Result<u16, String> {
        let center = Self::narrow(result.to_bits());
        if result.is_nan() || result.is_infinite() || result.is_zero() || exact {
            return Ok(center);
        }
        for neighbour in [result.to_bits() - 1, result.to_bits() + 1] {
            if Self::narrow(neighbour) != center {
                return Err("its f128 result lies within one f128 unit of an f16 midpoint".to_string());
            }
        }
        Ok(center)
    }

    /// Whether two `f16` patterns are the same value (equal bits, or both NaN).
    fn same(expected: u16, actual: Float16) -> bool {
        (actual.is_nan() && Float16::from_bits(expected).is_nan()) || expected == actual.to_bits()
    }

    /// Records a failure.
    fn fail(&self, message: String) {
        if let Ok(mut failures) = self.failures.lock() {
            failures.push(message);
        }
    }

    /// Runs `check` on every index below `count`, split over the available threads.
    fn parallel<F>(count: u32, check: F)
    where
        F: Fn(u32) + Sync,
    {
        let workers = thread::available_parallelism().map(|number| number.get() as u32).unwrap_or(4);
        thread::scope(|scope| {
            for worker in 0..workers {
                let check = &check;
                scope.spawn(move || {
                    let mut index = worker;
                    while index < count {
                        check(index);
                        index += workers;
                    }
                });
            }
        });
    }

    /// One-argument methods over all 65 536 inputs.
    fn check_unary(&self, name: &str, quad: fn(Float128) -> Float128, half: fn(Float16) -> Float16) {
        Self::parallel(1 << 16, |bits| {
            let bits = bits as u16;
            match Self::expected(quad(Self::widen(bits)), false) {
                Err(why) => self.fail(format!("{name}({bits:#06x}): {why}")),
                Ok(expected) if !Self::same(expected, half(Float16::from_bits(bits))) => self.fail(format!("{name}({bits:#06x}): expected {expected:#06x}, got {:#06x}", half(Float16::from_bits(bits)).to_bits())),
                Ok(_) => {}
            }
        });
    }

    /// `powi` over all inputs with exponents -4 to 4.
    fn check_integer_power(&self) {
        Self::parallel(9 << 16, |index| {
            let (bits, exponent) = ((index % (1 << 16)) as u16, (index >> 16) as i32 - 4);
            let exponent_bits = Float128::from_bits(NumberConversion::integer_to_float(exponent < 0, &[u64::from(exponent.unsigned_abs())], BinaryFormat::QUAD));
            let exact = exponent != 0 && Self::widen(bits).is_finite() && !Self::widen(bits).is_zero() && ExactPower::of(Self::widen(bits).abs(), exponent_bits).is_some();
            match Self::expected(Self::widen(bits).powi(exponent), exact) {
                Err(why) => self.fail(format!("powi({bits:#06x}, {exponent}): {why}")),
                Ok(expected) if !Self::same(expected, Float16::from_bits(bits).powi(exponent)) => self.fail(format!("powi({bits:#06x}, {exponent}): expected {expected:#06x}, got {:#06x}", Float16::from_bits(bits).powi(exponent).to_bits())),
                Ok(_) => {}
            }
        });
    }

    /// A deterministic pair of `f16` patterns for one index of a sequence (splitmix64 of the seed and the index).
    fn pair(seed: u64, index: u32) -> (u16, u16) {
        let mut mixed = seed.wrapping_add(u64::from(index).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^= mixed >> 31;
        (mixed as u16, (mixed >> 32) as u16)
    }

    /// The special values of `f16` as patterns, both signs.
    fn specials() -> Vec<u16> {
        let positive = [0x0000u16, 0x0001, 0x03FF, 0x0400, 0x3400, 0x3800, 0x3C00, 0x3C01, 0x4000, 0x4200, 0x4900, 0x5640, 0x7BFF, 0x7C00, 0x7C01, 0x7E00];
        positive.iter().flat_map(|bits| [*bits, *bits | 0x8000]).collect()
    }

    /// A two-argument method over every pair of special values and `2^20` deterministic random pairs.
    fn check_pairs(&self, name: &str, seed: u64, quad: fn(Float128, Float128) -> (Float128, bool), half: fn(Float16, Float16) -> Float16) {
        let specials = Self::specials();
        let total = (specials.len() * specials.len()) as u32 + (1 << 20);
        Self::parallel(total, |index| {
            let (x, y) = if (index as usize) < specials.len() * specials.len() {
                (specials[index as usize / specials.len()], specials[index as usize % specials.len()])
            } else {
                Self::pair(seed, index)
            };
            let (result, exact) = quad(Self::widen(x), Self::widen(y));
            match Self::expected(result, exact) {
                Err(why) => self.fail(format!("{name}({x:#06x}, {y:#06x}): {why}")),
                Ok(expected) if !Self::same(expected, half(Float16::from_bits(x), Float16::from_bits(y))) => self.fail(format!("{name}({x:#06x}, {y:#06x}): expected {expected:#06x}, got {:#06x}", half(Float16::from_bits(x), Float16::from_bits(y)).to_bits())),
                Ok(_) => {}
            }
        });
    }

    /// Asserts nothing failed.
    fn finish(&self) {
        let failures = self.failures.lock().map(|list| list.clone()).unwrap_or_default();
        let mut shown: Vec<String> = Vec::new();
        for failure in &failures {
            let name = failure.split('(').next().unwrap_or("");
            if shown.iter().filter(|line| line.starts_with(name)).count() < 3 {
                shown.push(failure.clone());
            }
        }
        assert!(failures.is_empty(), "{} f16 results are wrong or undecided; the first of each function:\n{}", failures.len(), shown.join("\n"));
    }
}

#[test]
fn every_f16_transcendental_is_correctly_rounded_over_all_inputs() {
    let oracle = HalfOracle::new();
    oracle.check_unary("exp", Float128::exp, Float16::exp);
    oracle.check_unary("ln", Float128::ln, Float16::ln);
    oracle.check_unary("log2", Float128::log2, Float16::log2);
    oracle.check_unary("log10", Float128::log10, Float16::log10);
    oracle.check_unary("sin", Float128::sin, Float16::sin);
    oracle.check_unary("cos", Float128::cos, Float16::cos);
    oracle.check_unary("tan", Float128::tan, Float16::tan);
    oracle.check_unary("asin", Float128::asin, Float16::asin);
    oracle.check_unary("acos", Float128::acos, Float16::acos);
    oracle.check_unary("atan", Float128::atan, Float16::atan);
    oracle.check_unary("sinh", Float128::sinh, Float16::sinh);
    oracle.check_unary("cosh", Float128::cosh, Float16::cosh);
    oracle.check_unary("tanh", Float128::tanh, Float16::tanh);
    oracle.check_integer_power();
    oracle.check_pairs("atan2", 1091, |y, x| (y.atan2(x), false), Float16::atan2);
    oracle.check_pairs("powf", 1092, |x, y| (x.powf(y), x.is_finite() && y.is_finite() && !x.is_zero() && !y.is_zero() && ExactPower::of(x.abs(), y).is_some()), Float16::powf);
    oracle.finish();
}
