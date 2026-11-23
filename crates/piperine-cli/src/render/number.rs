//! `Number`: the one place a value of a table or a record becomes text (CLI-05 AC 10).

use piperine_api::Cell;

/// How a value is written.
pub struct Number;

impl Number {
    /// A real as `{:.6e}`, a `bool` as `true` or `false`, an integer in decimal, a name as it
    /// is, a composite value in the language's literal syntax (`Pair { a: 7, b: true }`,
    /// `[1, 2, 3]`, `Tx::Data { n: 5 }`, `Some(4)`, `None`) with each scalar written the same way, and the
    /// absent cell (part VIII) as an empty field.
    pub fn text(cell: &Cell) -> String {
        match cell {
            Cell::Real(v) => format!("{v:.6e}"),
            Cell::Bool(v) => v.to_string(),
            Cell::Int(v) => v.to_string(),
            Cell::Text(v) => v.clone(),
            Cell::Composite(value) => value.literal(&Number::text),
            Cell::Absent => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_have_one_text_each() {
        // CLI-05 AC 10: a real as `{:.6e}`, a bool as `true` or `false`, an integer in decimal,
        // the reals are the Book's (`1.000000e0`, `6.321206e-1`)
        let cases = [
            (Cell::Real(1.0), "1.000000e0"),
            (Cell::Real(0.0), "0.000000e0"),
            (Cell::Real(0.632_120_558_828_557_7), "6.321206e-1"),
            (Cell::Real(-2.5e-7), "-2.500000e-7"),
            (Cell::Real(1234.5), "1.234500e3"),
            (Cell::Bool(true), "true"),
            (Cell::Bool(false), "false"),
            (Cell::Int(128), "128"),
            (Cell::Int(-7), "-7"),
            (Cell::Int(u64::MAX as i128), "18446744073709551615"),
            (Cell::Text("Step.vout".into()), "Step.vout"),
            // part VIII: the absent cell is an empty field, never a value written in its place
            (Cell::Absent, ""),
        ];
        for (cell, want) in cases {
            assert_eq!(Number::text(&cell), want, "{cell:?}");
        }
    }
}
