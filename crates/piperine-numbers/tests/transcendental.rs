//! The binary128 transcendentals against committed reference vectors (CV-31): `tests/vectors`
//! holds, per function, inputs and the correctly rounded result computed by `generate.py` with
//! `mpmath` at 1 024 bits and rounded to binary128 by exact rational comparison. Every
//! comparison is bit for bit; there is no tolerance anywhere.

use piperine_numbers::quad::Float128;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

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
