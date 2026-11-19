//! `Printed`: a number as the Book prints it, so a chapter test compares a run's value at the
//! precision the sentence states it.

use piperine_parity::golden::PrintedNumber;

/// A number as the Book prints it: the printed digits are the resolution of the comparison.
pub struct Printed;

impl Printed {
    /// Asserts that `x` rounds to the number the Book prints as `printed` (`4.990040e-1`); `what`
    /// says what the number is.
    pub fn holds(printed: &str, x: f64, what: &str) {
        let number = PrintedNumber::parse(printed).unwrap_or_else(|| panic!("`{printed}` is a printed number"));
        assert!(number.rounds_from(x), "{what}: the Book prints {printed}, the run gives {x:e}");
    }
}
