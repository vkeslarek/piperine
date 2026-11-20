//! [`NamedColumn`]: a column of a run's table, found by name.

use piperine_api::Table;

/// A column of a [`Table`], found by its full label or by a dotted suffix of it, so a test names
/// `vout` for the label `buck.vout` without spelling the instance path of the bench.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedColumn {
    /// The position of the column in every row of its table.
    index: usize,
    /// The full label of the column, as the table spells it.
    label: String,
}

impl NamedColumn {
    /// The column whose label is `name` or ends in `.name`. A name that matches no label, or two
    /// labels, panics naming the name and every label: a scenario must never read a column by
    /// guess.
    pub fn find(table: &Table, name: &str) -> NamedColumn {
        let dotted = format!(".{name}");
        let matches: Vec<NamedColumn> = table
            .columns
            .iter()
            .enumerate()
            .filter(|(_, label)| *label == name || label.ends_with(&dotted))
            .map(|(index, label)| NamedColumn { index, label: label.clone() })
            .collect();
        match matches.as_slice() {
            [only] => only.clone(),
            [] => panic!("no column `{name}` in {:?}", table.columns),
            several => {
                let labels: Vec<&str> = several.iter().map(NamedColumn::label).collect();
                panic!("column `{name}` is ambiguous: it matches {labels:?} in {:?}", table.columns)
            }
        }
    }

    /// The position of the column in every row of its table.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The full label of the column.
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::*;

    /// Tables the tests read, built by hand.
    struct Tables;

    impl Tables {
        /// An empty table with the given column labels.
        fn labelled(labels: &[&str]) -> Table {
            Table { columns: labels.iter().map(|label| label.to_string()).collect(), rows: Vec::new() }
        }

        /// The text a lookup panics with.
        fn panic_text(table: &Table, name: &str) -> String {
            let payload = catch_unwind(AssertUnwindSafe(|| NamedColumn::find(table, name))).expect_err("the lookup must panic");
            payload.downcast_ref::<String>().cloned().expect("a formatted panic message")
        }
    }

    #[test]
    fn a_label_equal_to_the_name_and_a_label_ending_in_it_are_both_found() {
        let table = Tables::labelled(&["time", "mode", "buck.vout"]);
        let whole = NamedColumn::find(&table, "mode");
        assert_eq!((whole.index(), whole.label()), (1, "mode"));
        let suffix = NamedColumn::find(&table, "vout");
        assert_eq!((suffix.index(), suffix.label()), (2, "buck.vout"));
    }

    #[test]
    fn a_missing_column_panics_naming_the_name_and_the_columns() {
        let table = Tables::labelled(&["time", "buck.vout"]);
        let text = Tables::panic_text(&table, "isense");
        assert!(text.contains("`isense`") && text.contains("buck.vout") && text.contains("time"), "{text}");
    }

    #[test]
    fn two_matching_labels_panic_naming_both() {
        let table = Tables::labelled(&["time", "a.vout", "b.vout"]);
        let text = Tables::panic_text(&table, "vout");
        assert!(text.contains("a.vout") && text.contains("b.vout") && text.contains("ambiguous"), "{text}");
    }
}
