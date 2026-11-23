//! The binary128 transcendentals against committed reference vectors (CV-31): `tests/vectors`
//! holds, per function, inputs and the correctly rounded result computed by `generate.py` with
//! `mpmath` at 1 024 bits and rounded to binary128 by exact rational comparison. Every
//! comparison is bit for bit; there is no tolerance anywhere.

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
