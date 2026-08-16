//! User-defined primitive (UDP) state-table evaluation (T49, VAMS-19).
//!
//! [`UdpTable`] evaluates an already-parsed `PrimitiveDecl`'s state table
//! (both combinational and sequential, LRM Annex A.5.3 / §7.4.3-7.4.6)
//! against live input values. `piperine_lang::ast::item::PrimitiveDecl`
//! is lowered into the POM (`Design::udps`, T39/LRMC-C7-13) and a UDP
//! instance is dispatched against this table by
//! `piperine-codegen::device::prim::UdpElement` (T40) — a plain `Element`
//! impl, not a Cranelift kernel, since a UDP's state table is a closed
//! LRM-fixed lookup with no user-authored expression to JIT. See
//! `device/prim.rs`'s own module doc for the dispatch side (including its
//! sequential-UDP scope note) and root `tests/vams_gates_and_udps_e2e.rs`
//! for the end-to-end proof.
//!
//! **Symbol model.** A `UdpTableRow`'s `inputs`/`current_state`/
//! `next_state` fields carry raw one-token symbol text (`piperine-vams`'s
//! own doc comment on `UdpTableRow`). This module interprets:
//! - level symbols (input/current-state columns): `0`, `1`, `x`, `?` (any
//!   of 0/1/x), `b` (0 or 1, i.e. not x) — Table 7-13-style.
//! - edge symbols (input columns of a sequential table only): `r` (0→1),
//!   `f` (1→0), `p` (potential positive edge: 0→1, 0→x, x→1), `n`
//!   (potential negative edge: 1→0, 1→x, x→0), `*` (any value change).
//! - `-` (output column of a sequential table only): "no change" — the
//!   state holds its previous value.
//!
//! `Z` on any UDP input is coerced to `X` before matching — UDPs are a
//! 0/1/x model (LRM §7.4: "the UDP has no concept of unknown drive
//! strength or high impedance on its inputs"), not a documented gap.

use piperine_solver::abi::LogicValue;
use piperine_lang::ast::PrimitiveDecl;

/// A parsed UDP's state table, ready to evaluate.
pub struct UdpTable<'a> {
    decl: &'a PrimitiveDecl,
}

/// Errors evaluating a UDP state table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpError {
    /// The number of supplied input values doesn't match the table's
    /// declared input-column count.
    InputArityMismatch { expected: usize, got: usize },
    /// A sequential table was evaluated without a previous-input vector
    /// the same length as the current one (needed for edge matching).
    MissingPreviousInputs,
    /// A row's symbol text isn't one this module recognizes.
    UnknownSymbol { symbol: String },
    /// `eval_combinational` was called on a table whose `is_sequential`
    /// flag is set, or vice versa.
    WrongTableKind,
}

impl<'a> UdpTable<'a> {
    pub fn new(decl: &'a PrimitiveDecl) -> Self {
        Self { decl }
    }

    /// Evaluates a combinational UDP (LRM §7.4.3) for one input vector.
    /// An input combination matching no row yields `X` — LRM: "any input
    /// combination... not listed... is assumed to produce `x`" — not an
    /// error.
    pub fn eval_combinational(&self, inputs: &[LogicValue]) -> Result<LogicValue, UdpError> {
        if self.decl.is_sequential {
            return Err(UdpError::WrongTableKind);
        }
        for row in &self.decl.table {
            if row.inputs.len() != inputs.len() {
                return Err(UdpError::InputArityMismatch { expected: row.inputs.len(), got: inputs.len() });
            }
            let matched = row
                .inputs
                .iter()
                .zip(inputs)
                .map(|(sym, &v)| level_matches(sym, coerce_z(v)))
                .collect::<Result<Vec<bool>, _>>()?;
            if matched.into_iter().all(|m| m) {
                return level_value(&row.next_state);
            }
        }
        Ok(LogicValue::X)
    }

    /// Evaluates a sequential UDP (LRM §7.4.4-7.4.6) for one time step:
    /// `prev_inputs`/`inputs` bracket the transition edge symbols match
    /// against, `state` is the current register value. Returns the new
    /// state (row order is first-match, mirroring simulator practice for
    /// non-exhaustively-disjoint tables). No matching row leaves the state
    /// unchanged (LRM: an unlisted combination has no effect).
    pub fn eval_sequential(
        &self,
        prev_inputs: &[LogicValue],
        inputs: &[LogicValue],
        state: LogicValue,
    ) -> Result<LogicValue, UdpError> {
        if !self.decl.is_sequential {
            return Err(UdpError::WrongTableKind);
        }
        if prev_inputs.len() != inputs.len() {
            return Err(UdpError::MissingPreviousInputs);
        }
        for row in &self.decl.table {
            if row.inputs.len() != inputs.len() {
                return Err(UdpError::InputArityMismatch { expected: row.inputs.len(), got: inputs.len() });
            }
            let Some(state_sym) = &row.current_state else {
                return Err(UdpError::WrongTableKind);
            };
            if !level_matches(state_sym, coerce_z(state))? {
                continue;
            }
            let cols_matched = row
                .inputs
                .iter()
                .zip(prev_inputs.iter().zip(inputs))
                .map(|(sym, (&prev, &curr))| column_matches(sym, coerce_z(prev), coerce_z(curr)))
                .collect::<Result<Vec<bool>, _>>()?;
            if cols_matched.into_iter().all(|m| m) {
                return match row.next_state.as_str() {
                    "-" => Ok(state),
                    sym => level_value(sym),
                };
            }
        }
        Ok(state)
    }
}

fn coerce_z(v: LogicValue) -> LogicValue {
    if v == LogicValue::Z { LogicValue::X } else { v }
}

/// Matches a level symbol (`0`/`1`/`x`/`?`/`b`) against a value.
fn level_matches(symbol: &str, value: LogicValue) -> Result<bool, UdpError> {
    use LogicValue::*;
    Ok(match symbol {
        "0" => value == Zero,
        "1" => value == One,
        "x" | "X" => value == X,
        "?" => matches!(value, Zero | One | X),
        "b" | "B" => matches!(value, Zero | One),
        other => return Err(UdpError::UnknownSymbol { symbol: other.to_string() }),
    })
}

/// Matches an input column that may carry either a level symbol (matched
/// against the current value only) or an edge symbol (matched against the
/// `prev -> curr` transition).
fn column_matches(symbol: &str, prev: LogicValue, curr: LogicValue) -> Result<bool, UdpError> {
    use LogicValue::*;
    Ok(match symbol {
        "r" => prev == Zero && curr == One,
        "f" => prev == One && curr == Zero,
        "p" => matches!((prev, curr), (Zero, One) | (Zero, X) | (X, One)),
        "n" => matches!((prev, curr), (One, Zero) | (One, X) | (X, Zero)),
        "*" => prev != curr,
        _ => return level_matches(symbol, curr),
    })
}

/// Parses an output-column symbol (`0`/`1`/`x`) into its value.
fn level_value(symbol: &str) -> Result<LogicValue, UdpError> {
    match symbol {
        "0" => Ok(LogicValue::Zero),
        "1" => Ok(LogicValue::One),
        "x" | "X" => Ok(LogicValue::X),
        other => Err(UdpError::UnknownSymbol { symbol: other.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LogicValue::*;
    use piperine_lang::ast::{PreprocessedSpan, UdpTableRow};

    fn span() -> PreprocessedSpan {
        PreprocessedSpan::new(0, 0)
    }

    fn row(inputs: &[&str], current_state: Option<&str>, next_state: &str) -> UdpTableRow {
        UdpTableRow {
            span: span(),
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            current_state: current_state.map(str::to_string),
            next_state: next_state.to_string(),
        }
    }

    fn primitive(is_sequential: bool, table: Vec<UdpTableRow>) -> PrimitiveDecl {
        PrimitiveDecl { span: span(), attrs: vec![], name: "test_udp".to_string(), ports: vec![], table, is_sequential }
    }

    /// A 2-input `and`-equivalent combinational UDP.
    fn and_udp() -> PrimitiveDecl {
        primitive(
            false,
            vec![
                row(&["1", "1"], None, "1"),
                row(&["0", "?"], None, "0"),
                row(&["?", "0"], None, "0"),
            ],
        )
    }

    #[test]
    fn combinational_matches_exact_row() {
        let decl = and_udp();
        let table = UdpTable::new(&decl);
        assert_eq!(table.eval_combinational(&[One, One]).unwrap(), One);
        assert_eq!(table.eval_combinational(&[Zero, One]).unwrap(), Zero);
        assert_eq!(table.eval_combinational(&[One, Zero]).unwrap(), Zero);
    }

    #[test]
    fn combinational_unmatched_input_defaults_to_x() {
        let decl = and_udp();
        let table = UdpTable::new(&decl);
        assert_eq!(table.eval_combinational(&[X, One]).unwrap(), X);
    }

    #[test]
    fn combinational_wildcard_row_matches_any_second_input() {
        let decl = and_udp();
        let table = UdpTable::new(&decl);
        assert_eq!(table.eval_combinational(&[Zero, X]).unwrap(), Zero);
    }

    #[test]
    fn combinational_z_input_is_coerced_to_x() {
        let decl = and_udp();
        let table = UdpTable::new(&decl);
        assert_eq!(table.eval_combinational(&[Z, One]).unwrap(), X);
    }

    #[test]
    fn combinational_rejects_sequential_table() {
        let decl = primitive(true, vec![]);
        let table = UdpTable::new(&decl);
        assert_eq!(table.eval_combinational(&[One]).unwrap_err(), UdpError::WrongTableKind);
    }

    /// A positive-edge-triggered D-flip-flop-equivalent sequential UDP
    /// (`clk`, `d`) — `(r d : ? : d)` "on clk rising edge, state follows
    /// d", `(? * : ? : -)` "any other input change: no change".
    fn dff_udp() -> PrimitiveDecl {
        primitive(
            true,
            vec![
                row(&["r", "0"], Some("?"), "0"),
                row(&["r", "1"], Some("?"), "1"),
                row(&["?", "*"], Some("?"), "-"),
            ],
        )
    }

    #[test]
    fn sequential_clocks_in_data_on_rising_edge() {
        let decl = dff_udp();
        let table = UdpTable::new(&decl);
        // clk 0->1, d=1: state should follow d.
        let new_state = table.eval_sequential(&[Zero, One], &[One, One], Zero).unwrap();
        assert_eq!(new_state, One);
    }

    #[test]
    fn sequential_holds_state_when_clk_does_not_rise() {
        let decl = dff_udp();
        let table = UdpTable::new(&decl);
        // clk stays 0, d changes: no clock edge, state holds via the '?','*' row.
        let new_state = table.eval_sequential(&[Zero, Zero], &[Zero, One], One).unwrap();
        assert_eq!(new_state, One);
    }

    #[test]
    fn sequential_falling_edge_does_not_clock_in_data() {
        let decl = dff_udp();
        let table = UdpTable::new(&decl);
        // clk 1->0 (falling), d=1: not a rising edge, no row for "f" so falls
        // through to the '?','*' catch-all -> holds.
        let new_state = table.eval_sequential(&[One, One], &[Zero, One], Zero).unwrap();
        assert_eq!(new_state, Zero);
    }

    #[test]
    fn sequential_requires_matching_prev_and_current_length() {
        let decl = dff_udp();
        let table = UdpTable::new(&decl);
        let err = table.eval_sequential(&[Zero], &[Zero, One], Zero).unwrap_err();
        assert_eq!(err, UdpError::MissingPreviousInputs);
    }

    #[test]
    fn sequential_rejects_combinational_table() {
        let decl = and_udp();
        let table = UdpTable::new(&decl);
        let err = table.eval_sequential(&[Zero, Zero], &[One, One], Zero).unwrap_err();
        assert_eq!(err, UdpError::WrongTableKind);
    }

    #[test]
    fn edge_symbols_match_per_table_semantics() {
        assert!(column_matches("r", Zero, One).unwrap());
        assert!(!column_matches("r", One, Zero).unwrap());
        assert!(column_matches("f", One, Zero).unwrap());
        assert!(column_matches("p", Zero, X).unwrap());
        assert!(column_matches("p", X, One).unwrap());
        assert!(column_matches("n", One, X).unwrap());
        assert!(column_matches("n", X, Zero).unwrap());
        assert!(column_matches("*", Zero, One).unwrap());
        assert!(!column_matches("*", One, One).unwrap());
    }

    #[test]
    fn unknown_symbol_fails_loud() {
        let err = level_matches("q", One).unwrap_err();
        assert_eq!(err, UdpError::UnknownSymbol { symbol: "q".to_string() });
    }
}
