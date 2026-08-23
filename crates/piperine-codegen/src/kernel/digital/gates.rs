//! IEEE 1364 gate-primitive evaluation (T48, VAMS-18).
//!
//! [`GatePrimitive::eval_n`]/`eval1` — every §7.2/§7.3 logic-gate
//! primitive's truth table (`and`/`nand`/`or`/`nor`/`xor`/`xnor`, Table
//! 7-4; `buf`/`not`, Table 7-9; `bufif0`/`bufif1`/`notif0`/`notif1`,
//! Table 7-10) — plus [`Strength`]/`Strength::resolve` (§7.9's
//! drive-strength table, Table 7-3), dispatched against real
//! `GateInstance`s (T39's POM lowering) by `piperine-codegen::device::
//! prim::GateElement` (T40/LRMC-C7-13) — a plain `Element` impl, not a
//! Cranelift kernel, since a gate's truth table is a closed LRM-fixed
//! lookup with no user-authored expression to JIT. See
//! `device/prim.rs`'s own module doc for the dispatch side; root
//! `tests/vams_gates_and_udps_e2e.rs` is the end-to-end proof.
//!
//! **Out of scope, documented not silently dropped:** the switch-level
//! primitives (`nmos`/`pmos`/`cmos`/`r{n,p,c}mos`, `tran`/`rtran`/
//! `tranif0/1`/`rtranif0/1`, `pullup`/`pulldown`) are bidirectional
//! resistive connections between nets, not value-producing functions of
//! their inputs — they need a switch-level (charge-sharing/resistive)
//! network model, which is a different mechanism from a truth table and a
//! larger undertaking than this task's scope. `GatePrimitive` covers the
//! unidirectional logic-gate family only; `piperine_lang::ast::item::
//! GateKind`'s switch variants have no `GatePrimitive` counterpart yet.

use piperine_solver::abi::LogicValue;

/// LRM §7.9 Table 7-2/7-3 drive-strength levels and their resolution
/// rule — re-exported directly from `piperine-solver`'s own copy (T54/
/// LRMW2-46 consolidation) rather than duplicated here. Prior to T54
/// this module carried a second, independently-written `Strength` enum
/// with the identical variant set and `resolve` behavior (only
/// reachable in this direction — `piperine-codegen` already depends on
/// `piperine-solver`, never the reverse, CLAUDE.md — so the duplication
/// was never structurally necessary, just historical). This alias keeps
/// every existing `kernel::digital::gates::Strength`/`Strength::resolve`
/// call site in this crate compiling unchanged.
pub use piperine_solver::abi::Strength;

/// One IEEE 1364 unidirectional logic-gate primitive (§7.2 `n`-input
/// gates, §7.3 `buf`/`not`, §7.4 tristate `bufif`/`notif`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatePrimitive {
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Buf,
    Not,
    /// `bufif1`: `data` passed through while `control` is 1, else `Z`
    /// (Table 7-10).
    Bufif1,
    /// `bufif0`: `data` passed through while `control` is 0, else `Z`.
    Bufif0,
    /// `notif1`: `~data` passed through while `control` is 1, else `Z`.
    Notif1,
    /// `notif0`: `~data` passed through while `control` is 0, else `Z`.
    Notif0,
}

impl GatePrimitive {
    /// Evaluates an `n`-input gate (`and`/`nand`/`or`/`nor`/`xor`/`xnor`)
    /// over its input list per LRM Table 7-4/7-5/7-6, reducing pairwise
    /// (associative for these primitives, matching 1364's own n-ary
    /// truth-table extension). `inputs` must be non-empty.
    ///
    /// Fails loud (`None`) if called on a non-`n`-input variant (`Buf`/
    /// `Not`/tristate) or with an empty input list — use [`Self::eval1`]
    /// for the single-input variants.
    pub fn eval_n(self, inputs: &[LogicValue]) -> Option<LogicValue> {
        let mut it = inputs.iter().copied();
        let first = it.next()?;
        match self {
            GatePrimitive::And => Some(it.fold(first, and2)),
            GatePrimitive::Nand => Some(negate(it.fold(first, and2))),
            GatePrimitive::Or => Some(it.fold(first, or2)),
            GatePrimitive::Nor => Some(negate(it.fold(first, or2))),
            GatePrimitive::Xor => Some(it.fold(first, xor2)),
            GatePrimitive::Xnor => Some(negate(it.fold(first, xor2))),
            GatePrimitive::Buf | GatePrimitive::Not | GatePrimitive::Bufif0
            | GatePrimitive::Bufif1 | GatePrimitive::Notif0 | GatePrimitive::Notif1 => None,
        }
    }

    /// Evaluates a single-input primitive (`buf`/`not`, Table 7-9) or a
    /// tristate primitive (`bufif0/1`/`notif0/1`, Table 7-10 — `control`
    /// is required for these). Fails loud (`None`) for an `n`-input
    /// variant or a tristate variant called without `control`.
    pub fn eval1(self, data: LogicValue, control: Option<LogicValue>) -> Option<LogicValue> {
        match (self, control) {
            (GatePrimitive::Buf, _) => Some(data),
            (GatePrimitive::Not, _) => Some(negate1(data)),
            (GatePrimitive::Bufif1, Some(c)) => Some(tristate(data, c, LogicValue::One)),
            (GatePrimitive::Bufif0, Some(c)) => Some(tristate(data, c, LogicValue::Zero)),
            (GatePrimitive::Notif1, Some(c)) => Some(tristate(negate1(data), c, LogicValue::One)),
            (GatePrimitive::Notif0, Some(c)) => Some(tristate(negate1(data), c, LogicValue::Zero)),
            _ => None,
        }
    }
}

/// `data` passed through when `control == enable_when`, `Z` when it's the
/// opposite level, `X` when `control` is `X`/`Z` (Table 7-10's own
/// "unknown enable ⇒ unknown/high-Z contention" row, collapsed to `X` —
/// the LRM's exact table lists `X` for every data value when control is
/// unknown).
fn tristate(data: LogicValue, control: LogicValue, enable_when: LogicValue) -> LogicValue {
    let disable_when = if enable_when == LogicValue::One { LogicValue::Zero } else { LogicValue::One };
    match control {
        c if c == enable_when => data,
        c if c == disable_when => LogicValue::Z,
        _ => LogicValue::X,
    }
}

fn negate1(v: LogicValue) -> LogicValue {
    match v {
        LogicValue::Zero => LogicValue::One,
        LogicValue::One => LogicValue::Zero,
        LogicValue::X | LogicValue::Z => LogicValue::X,
    }
}

/// `Z` on a gate's *output* has no meaning (only tristate primitives can
/// drive `Z`) — 1364 treats a `Z` operand to a logic gate as `X` (Table
/// 7-4's footnote). `negate`/`and2`/`or2`/`xor2` all route through this.
fn as_gate_operand(v: LogicValue) -> LogicValue {
    if v == LogicValue::Z { LogicValue::X } else { v }
}

fn negate(v: LogicValue) -> LogicValue {
    negate1(as_gate_operand(v))
}

fn and2(a: LogicValue, b: LogicValue) -> LogicValue {
    use LogicValue::*;
    match (as_gate_operand(a), as_gate_operand(b)) {
        (Zero, _) | (_, Zero) => Zero,
        (One, One) => One,
        _ => X,
    }
}

fn or2(a: LogicValue, b: LogicValue) -> LogicValue {
    use LogicValue::*;
    match (as_gate_operand(a), as_gate_operand(b)) {
        (One, _) | (_, One) => One,
        (Zero, Zero) => Zero,
        _ => X,
    }
}

fn xor2(a: LogicValue, b: LogicValue) -> LogicValue {
    use LogicValue::*;
    match (as_gate_operand(a), as_gate_operand(b)) {
        (Zero, Zero) | (One, One) => Zero,
        (Zero, One) | (One, Zero) => One,
        _ => X,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LogicValue::*;

    fn all_values() -> [LogicValue; 4] {
        [Zero, One, X, Z]
    }

    #[test]
    fn and_gate_truth_table_table_7_4() {
        assert_eq!(GatePrimitive::And.eval_n(&[Zero, Zero]), Some(Zero));
        assert_eq!(GatePrimitive::And.eval_n(&[Zero, One]), Some(Zero));
        assert_eq!(GatePrimitive::And.eval_n(&[One, One]), Some(One));
        assert_eq!(GatePrimitive::And.eval_n(&[One, X]), Some(X));
        assert_eq!(GatePrimitive::And.eval_n(&[Zero, X]), Some(Zero));
        assert_eq!(GatePrimitive::And.eval_n(&[One, Z]), Some(X));
    }

    #[test]
    fn nand_or_nor_xor_xnor_truth_tables() {
        assert_eq!(GatePrimitive::Nand.eval_n(&[One, One]), Some(Zero));
        assert_eq!(GatePrimitive::Nand.eval_n(&[Zero, One]), Some(One));
        assert_eq!(GatePrimitive::Or.eval_n(&[Zero, Zero]), Some(Zero));
        assert_eq!(GatePrimitive::Or.eval_n(&[Zero, One]), Some(One));
        assert_eq!(GatePrimitive::Nor.eval_n(&[Zero, Zero]), Some(One));
        assert_eq!(GatePrimitive::Nor.eval_n(&[Zero, One]), Some(Zero));
        assert_eq!(GatePrimitive::Xor.eval_n(&[Zero, One]), Some(One));
        assert_eq!(GatePrimitive::Xor.eval_n(&[One, One]), Some(Zero));
        assert_eq!(GatePrimitive::Xnor.eval_n(&[Zero, One]), Some(Zero));
        assert_eq!(GatePrimitive::Xnor.eval_n(&[One, One]), Some(One));
    }

    #[test]
    fn n_input_and_reduces_associatively() {
        assert_eq!(GatePrimitive::And.eval_n(&[One, One, One]), Some(One));
        assert_eq!(GatePrimitive::And.eval_n(&[One, One, Zero]), Some(Zero));
        assert_eq!(GatePrimitive::Or.eval_n(&[Zero, Zero, One]), Some(One));
    }

    #[test]
    fn buf_and_not_truth_table_table_7_9() {
        for v in all_values() {
            assert_eq!(GatePrimitive::Buf.eval1(v, None), Some(v));
        }
        assert_eq!(GatePrimitive::Not.eval1(Zero, None), Some(One));
        assert_eq!(GatePrimitive::Not.eval1(One, None), Some(Zero));
        assert_eq!(GatePrimitive::Not.eval1(X, None), Some(X));
        assert_eq!(GatePrimitive::Not.eval1(Z, None), Some(X));
    }

    #[test]
    fn bufif1_truth_table_table_7_10() {
        assert_eq!(GatePrimitive::Bufif1.eval1(Zero, Some(One)), Some(Zero));
        assert_eq!(GatePrimitive::Bufif1.eval1(One, Some(One)), Some(One));
        assert_eq!(GatePrimitive::Bufif1.eval1(One, Some(Zero)), Some(Z));
        assert_eq!(GatePrimitive::Bufif1.eval1(One, Some(X)), Some(X));
    }

    #[test]
    fn bufif0_notif0_notif1_truth_tables() {
        assert_eq!(GatePrimitive::Bufif0.eval1(One, Some(Zero)), Some(One));
        assert_eq!(GatePrimitive::Bufif0.eval1(One, Some(One)), Some(Z));
        assert_eq!(GatePrimitive::Notif1.eval1(One, Some(One)), Some(Zero));
        assert_eq!(GatePrimitive::Notif1.eval1(One, Some(Zero)), Some(Z));
        assert_eq!(GatePrimitive::Notif0.eval1(One, Some(Zero)), Some(Zero));
        assert_eq!(GatePrimitive::Notif0.eval1(One, Some(One)), Some(Z));
    }

    #[test]
    fn eval1_rejects_n_input_and_missing_control() {
        assert_eq!(GatePrimitive::And.eval1(One, None), None);
        assert_eq!(GatePrimitive::Bufif1.eval1(One, None), None);
    }

    #[test]
    fn eval_n_rejects_single_input_variants() {
        assert_eq!(GatePrimitive::Buf.eval_n(&[One]), None);
        assert_eq!(GatePrimitive::Not.eval_n(&[One]), None);
    }

    #[test]
    fn eval_n_rejects_empty_input_list() {
        assert_eq!(GatePrimitive::And.eval_n(&[]), None);
    }

    #[test]
    fn strength_resolution_stronger_wins_table_7_3() {
        let (v, s) = Strength::resolve((One, Strength::Strong), (Zero, Strength::Weak));
        assert_eq!((v, s), (One, Strength::Strong));
    }

    #[test]
    fn strength_resolution_highz_yields() {
        let (v, s) = Strength::resolve((Zero, Strength::HighZ), (One, Strength::Weak));
        assert_eq!((v, s), (One, Strength::Weak));
    }

    #[test]
    fn strength_resolution_equal_strength_same_value_holds() {
        let (v, s) = Strength::resolve((One, Strength::Pull), (One, Strength::Pull));
        assert_eq!((v, s), (One, Strength::Pull));
    }

    #[test]
    fn strength_resolution_equal_strength_conflict_is_x() {
        let (v, _) = Strength::resolve((One, Strength::Pull), (Zero, Strength::Pull));
        assert_eq!(v, X);
    }

    #[test]
    fn strength_resolution_supply_beats_strong() {
        let (v, s) = Strength::resolve((One, Strength::Supply), (Zero, Strength::Strong));
        assert_eq!((v, s), (One, Strength::Supply));
    }
}
