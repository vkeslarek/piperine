//! What one ngspice run produced: its captured output and the exports it wrote.

use std::path::{Path, PathBuf};

use crate::scratch::ScratchDir;

/// A complex `wrdata` value: real and imaginary part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn magnitude(&self) -> f64 {
        self.re.hypot(self.im)
    }

    /// The phase in radians, in (-pi, pi].
    pub fn phase(&self) -> f64 {
        self.im.atan2(self.re)
    }
}

/// The result of running a `.cir`: the captured streams, the first line of `ngspice --version`
/// and the scratch directory holding the exports, removed when this value is dropped.
#[derive(Debug)]
pub struct Golden {
    fixture: String,
    stdout: String,
    stderr: String,
    version: String,
    scratch: ScratchDir,
}

impl Golden {
    pub(crate) fn new(fixture: &str, stdout: String, stderr: String, version: String, scratch: ScratchDir) -> Self {
        Golden { fixture: fixture.to_string(), stdout, stderr, version, scratch }
    }

    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    /// The first line of `ngspice --version`, for the message of a failed comparison.
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn scratch_path(&self) -> &Path {
        self.scratch.path()
    }

    /// The file `wrdata <name>` wrote, found under that name or, as older ngspice builds write it,
    /// with the suffix `.data`. Fails naming the fixture and ngspice's captured output when the run
    /// wrote no such file.
    pub fn export_file(&self, name: &str) -> PathBuf {
        for candidate in [name.to_string(), format!("{name}.data")] {
            let path = self.scratch.path().join(candidate);
            if path.is_file() {
                return path;
            }
        }
        panic!("{}: ngspice wrote no export {name:?}\n{}", self.fixture, self.captured());
    }

    /// The value ngspice printed as `<name> = <value>`. Fails when no such line exists or its value
    /// is not a number.
    pub fn printed(&self, name: &str) -> f64 {
        for (index, line) in self.stdout.lines().enumerate() {
            let Some((left, right)) = line.split_once('=') else { continue };
            if left.trim() != name {
                continue;
            }
            return right.trim().parse().unwrap_or_else(|_| {
                panic!("{}: stdout row {}: {line:?} does not hold a number for {name}", self.fixture, index + 1)
            });
        }
        panic!("{}: ngspice printed no value for {name}\n{}", self.fixture, self.captured());
    }

    /// The rows `x y` of a real `wrdata` export.
    pub fn real(&self, export: &str) -> Vec<(f64, f64)> {
        self.rows(export, 2).into_iter().map(|row| (row[0], row[1])).collect()
    }

    /// The rows `x re im` of a complex `wrdata` export.
    pub fn complex(&self, export: &str) -> Vec<(f64, Complex)> {
        self.rows(export, 3).into_iter().map(|row| (row[0], Complex { re: row[1], im: row[2] })).collect()
    }

    /// Fails naming the fixture, the export and both counts unless `rows` holds `expected` rows.
    pub fn expect_rows<T>(&self, export: &str, rows: &[T], expected: usize) {
        if rows.len() != expected {
            panic!("{}: export {export} has {} rows, expected {expected}", self.fixture, rows.len());
        }
    }

    /// Fails naming the first point where the two grids differ by more than 1e-9 relative, or
    /// naming both lengths when they differ.
    pub fn same_grid(&self, export: &str, golden: &[f64], actual: &[f64]) {
        if golden.len() != actual.len() {
            panic!("{}: grid of {export} has {} points in the export and {} in the solver", self.fixture, golden.len(), actual.len());
        }
        for (index, (wanted, got)) in golden.iter().zip(actual).enumerate() {
            if (wanted - got).abs() > Self::GRID_RELATIVE * wanted.abs().max(got.abs()) {
                panic!("{}: grid of {export} differs at point {index}: export {wanted:e}, solver {got:e}", self.fixture);
            }
        }
    }

    const GRID_RELATIVE: f64 = 1e-9;

    fn rows(&self, export: &str, columns: usize) -> Vec<Vec<f64>> {
        let path = self.export_file(export);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: cannot read {}: {error}", self.fixture, path.display()));
        let mut rows = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: Result<Vec<f64>, _> = line.split_whitespace().map(str::parse::<f64>).collect();
            match parsed {
                Ok(values) if values.len() == columns => rows.push(values),
                Ok(values) => panic!("{}: {} row {}: {line:?} has {} columns, expected {columns}", self.fixture, path.display(), index + 1, values.len()),
                Err(_) => panic!("{}: {} row {}: {line:?} does not parse as numbers", self.fixture, path.display(), index + 1),
            }
        }
        rows
    }

    pub(crate) fn captured(&self) -> String {
        format!("--- ngspice stdout ---\n{}\n--- ngspice stderr ---\n{}", self.stdout, self.stderr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A golden over a scratch directory holding the given files, with no ngspice involved.
    fn golden_with(files: &[(&str, &str)]) -> Golden {
        let scratch = ScratchDir::create("unit");
        for (name, content) in files {
            std::fs::write(scratch.path().join(name), content).expect("write an export");
        }
        Golden::new("unit", "v(mid) = 7.500000e+00\nv(bad) = oops\n".to_string(), String::new(), "ngspice-test".to_string(), scratch)
    }

    fn panic_message(work: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let payload = std::panic::catch_unwind(work).expect_err("the work must panic");
        match payload.downcast::<String>() {
            Ok(text) => *text,
            Err(other) => other.downcast::<&str>().map(|text| text.to_string()).expect("a text panic"),
        }
    }

    #[test]
    fn an_unparsable_row_names_file_and_row() {
        let golden = golden_with(&[("sweep", " 0.0 1.0 \n 1.0 abc \n"), ("short", "1.0\n")]);
        let message = panic_message(std::panic::AssertUnwindSafe(|| {
            golden.real("sweep");
        }));
        assert!(message.contains("sweep") && message.contains("row 2") && message.contains("1.0 abc"), "{message}");
        let message = panic_message(std::panic::AssertUnwindSafe(|| {
            golden.real("short");
        }));
        assert!(message.contains("short") && message.contains("row 1") && message.contains("1 columns"), "{message}");
        let message = panic_message(std::panic::AssertUnwindSafe(|| {
            golden.printed("v(bad)");
        }));
        assert!(message.contains("stdout row 2") && message.contains("v(bad)"), "{message}");
        assert_eq!(golden.printed("v(mid)"), 7.5);
        let message = panic_message(std::panic::AssertUnwindSafe(|| {
            golden.printed("v(none)");
        }));
        assert!(message.contains("v(none)") && message.contains("unit"), "{message}");
    }

    #[test]
    fn an_export_is_found_with_or_without_data_suffix() {
        let plain = golden_with(&[("one", "1.0 2.0\n")]);
        let suffixed = golden_with(&[("one.data", "1.0 2.0\n")]);
        assert_eq!(plain.real("one"), vec![(1.0, 2.0)]);
        assert_eq!(suffixed.real("one"), vec![(1.0, 2.0)]);
        let complex = golden_with(&[("ac", "1.0e3 0.5 -0.5\n")]);
        let rows = complex.complex("ac");
        assert_eq!(rows[0].1, Complex { re: 0.5, im: -0.5 });
        assert!((rows[0].1.magnitude() - 0.5_f64.sqrt()).abs() < 1e-15);
        assert!((rows[0].1.phase() + std::f64::consts::FRAC_PI_4).abs() < 1e-15);
    }

    #[test]
    fn a_grid_mismatch_names_the_point() {
        let golden = golden_with(&[]);
        golden.same_grid("g", &[1.0, 2.0], &[1.0 + 0.5e-9, 2.0]);
        let message = panic_message(std::panic::AssertUnwindSafe(|| golden.same_grid("g", &[1.0, 2.0], &[1.0, 2.0 * (1.0 + 2e-9)])));
        assert!(message.contains("point 1"), "{message}");
        let message = panic_message(std::panic::AssertUnwindSafe(|| golden.same_grid("g", &[1.0, 2.0], &[1.0])));
        assert!(message.contains('2') && message.contains('1'), "{message}");
    }

    #[test]
    fn a_wrong_row_count_names_both_counts() {
        let golden = golden_with(&[("e", "1 2\n2 3\n3 4\n")]);
        let rows = golden.real("e");
        golden.expect_rows("e", &rows, 3);
        let message = panic_message(std::panic::AssertUnwindSafe(|| golden.expect_rows("e", &rows, 37)));
        assert!(message.contains("unit") && message.contains(" 3 rows") && message.contains("37"), "{message}");
    }
}
