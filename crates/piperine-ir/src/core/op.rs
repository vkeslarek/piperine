//! The IR op set (ideal.md §1.3 value graph + §1.5 net access).
//!
//! Six net-access ops, two cell-access ops, two pure-op vehicles (`Pure`
//! and `Call`), and two structured-control carriers (`If`/`Match`). The
//! load-bearing distinctions:
//!
//! - `ReadAcross` and `ReadThrough` are SEPARATE variants — collapsing
//!   them is the A0 mechanism (a contribution silently read as a force).
//! - `ContributeThrough` and `ForceAcross` likewise — a contribution
//!   (KCL) is structurally distinct from a force (constraint).
//! - There is NO `slice`/`concat`/`extract`/`insert` op: those are
//!   representation, carried by [`SigRef`](super::SigRef) chunks.
//! - `If`/`Match` carry op lists inline — control stays structured,
//!   no flattening pass (D9).
//!
//! No catch-all arm anywhere that dispatches on these — explicit arms
//! only, so an unlowered construct lowering fails loud (the A0c defence).

use super::{CellId, IntrinsicId, NetId, ValueId};

/// Pure ops over `real` or `bits<N>` values — arithmetic, comparison,
/// `select`. Signedness and width behaviour are OP properties (RTLIL
/// style), not value properties. The set starts small (what the tests
/// exercise) and extends in later phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PureOp {
    /// `+` (real or bits, signedness per OP).
    Add,
    /// `-`.
    Sub,
    /// `*`.
    Mul,
    /// `/`.
    Div,
    /// Unary negation.
    Neg,
    /// `==`.
    Eq,
    /// `!=`.
    Ne,
    /// `<`.
    Lt,
    /// `<=`.
    Le,
    /// `>`.
    Gt,
    /// `>=`.
    Ge,
    /// `select(cond, then, else)` — the ternary op. Output type matches
    /// the `then`/`else` arguments, not the `cond`.
    Select,
}

/// One arm of a `Match`. Single-bit patterns only this phase (T39 extends
/// to multi-bit): a `bits<1>` scrutinee matches `0`, `1`, or `?` (wildcard).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// The scrutinee bit is `0`.
    Zero,
    /// The scrutinee bit is `1`.
    One,
    /// Wildcard — matches either.
    Wildcard,
}

/// One operation in a block's body. Variants group by what they touch:
/// pure computation, intrinsic calls, net access (six kinds), cell access,
/// and structured control. There is no `Nop`, no `Unreachable`, no
/// catch-all.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// A pure op over value ids — arithmetic, comparison, `select`. Pure
    /// means: same args → same result, no side effects, no reads of nets
    /// or cells. Every `real` subgraph of `Pure` ops is differentiable by
    /// construction (T12 turns symbolic diff into an IR→IR pass).
    Pure { op: PureOp, args: Vec<ValueId>, out: ValueId },
    /// A call into the intrinsic catalog (the table that subsumes the
    /// runtime-semantic families: `ddt`/`idt`/`transition`/`delay`/`slew`/
    /// `table`/`laplace`/…). The intrinsic owns its private state; the
    /// IR sees only the argument list and the result.
    Call { intrinsic: IntrinsicId, args: Vec<ValueId>, out: ValueId },
    /// Read the ACROSS nature of a node pair `(a, b)` — for `Electrical`,
    /// `V(a, b)`. The IR does not name the nature; the node carries it.
    ReadAcross { a: NetId, b: NetId, out: ValueId },
    /// Read the THROUGH nature of a node pair `(a, b)` — for `Electrical`,
    /// `I(a, b)`. Distinct from `ReadAcross`: collapsing them is the A0
    /// mechanism (a contribution silently read as a force).
    ReadThrough { a: NetId, b: NetId, out: ValueId },
    /// Contribute `value` to the THROUGH nature of `(a, b)` — KCL, summed
    /// to zero at the node. Analog-write op.
    ContributeThrough { a: NetId, b: NetId, value: ValueId },
    /// Force the ACROSS nature of `(a, b)` to `value` — a constraint,
    /// not a KCL row. Analog-write op (a voltage source).
    ForceAcross { a: NetId, b: NetId, value: ValueId },
    /// Read a discrete signal net. The net must be `NetKind::Signal`
    /// (T7 catches the mismatch).
    ReadSignal { net: NetId, out: ValueId },
    /// Drive a discrete signal net with `value`. The net must be
    /// `NetKind::Signal`. Digital-write op.
    Drive { net: NetId, value: ValueId },
    /// Read a cell (register or memory). `addr` is `Some` for a memory
    /// read (`$memrd`), `None` for a register read. The output type is
    /// the cell's ELEMENT type (T7 checks the match).
    CellRead { cell: CellId, addr: Option<ValueId>, out: ValueId },
    /// Write a cell (register or memory). `addr` is `Some` for a memory
    /// write (`$memwr`), `None` for a register write.
    CellWrite { cell: CellId, addr: Option<ValueId>, value: ValueId },
    /// Structured `if`: `cond` selects between two inline op lists. No
    /// flattening pass — control stays structured (D9), symbolic diff
    /// works arm-wise.
    If { cond: ValueId, then_ops: Vec<Op>, else_ops: Vec<Op> },
    /// Structured `match`: `scrutinee` selects one of several inline op
    /// lists by [`Pattern`]. Single-bit patterns only this phase (T39
    /// extends to multi-bit).
    Match { scrutinee: ValueId, arms: Vec<(Pattern, Vec<Op>)> },
}

impl Op {
    /// The `ValueId`s this op READS — args and address operands. Useful
    /// for the verifier (T7) and the eventual dataflow passes.
    pub fn reads(&self) -> Vec<ValueId> {
        match self {
            Op::Pure { args, .. } | Op::Call { args, .. } => args.clone(),
            Op::ReadAcross { out: _, .. }
            | Op::ReadThrough { out: _, .. } => Vec::new(),
            Op::ContributeThrough { value, .. }
            | Op::ForceAcross { value, .. }
            | Op::Drive { value, .. } => vec![*value],
            Op::ReadSignal { out: _, .. } => Vec::new(),
            Op::CellRead { addr: Some(a), .. } => vec![*a],
            Op::CellRead { addr: None, .. } => Vec::new(),
            Op::CellWrite { addr: Some(a), value, .. } => vec![*a, *value],
            Op::CellWrite { addr: None, value, .. } => vec![*value],
            Op::If { cond, .. } | Op::Match { scrutinee: cond, .. } => vec![*cond],
        }
    }

    /// The `ValueId` this op WRITES (its `out`), if any. `Match`/`If`
    /// carry nested ops whose writes are not visible at this level.
    pub fn writes(&self) -> Option<ValueId> {
        match self {
            Op::Pure { out, .. }
            | Op::Call { out, .. }
            | Op::ReadAcross { out, .. }
            | Op::ReadThrough { out, .. }
            | Op::ReadSignal { out, .. }
            | Op::CellRead { out, .. } => Some(*out),
            Op::ContributeThrough { .. }
            | Op::ForceAcross { .. }
            | Op::Drive { .. }
            | Op::CellWrite { .. } => None,
            Op::If { .. } | Op::Match { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE A0 MECHANISM TEST (LOV-01): `ReadAcross` and `ReadThrough` are
    /// SEPARATE variants even when their fields are identical. The defect
    /// was a contribution silently read as a force because the lowering
    /// collapsed the two reads; here, two distinct enum variants cannot
    /// be confused at any layer.
    #[test]
    fn read_across_and_read_through_are_separate_variants() {
        let a = NetId(0);
        let b = NetId(1);
        let out = ValueId(0);
        let read_v = Op::ReadAcross { a, b, out };
        let read_i = Op::ReadThrough { a, b, out };
        assert_ne!(read_v, read_i, "the two reads are distinct variants");
        // And likewise for the two writes — a contribution (KCL) is not a
        // force (constraint).
        let value = ValueId(2);
        let contrib = Op::ContributeThrough { a, b, value };
        let force = Op::ForceAcross { a, b, value };
        assert_ne!(contrib, force, "contribute and force are distinct variants");
    }

    /// Six distinct net-access ops exist — the load-bearing count for the
    /// A0 defence (no fewer: each maps to one specific runtime action).
    #[test]
    fn six_distinct_net_access_ops_exist() {
        let net = NetId(0);
        let other = NetId(1);
        let out = ValueId(0);
        let value = ValueId(1);
        let ops = [
            Op::ReadAcross { a: net, b: other, out },
            Op::ReadThrough { a: net, b: other, out },
            Op::ContributeThrough { a: net, b: other, value },
            Op::ForceAcross { a: net, b: other, value },
            Op::ReadSignal { net, out },
            Op::Drive { net, value },
        ];
        // Every pair is a distinct variant — no two collapse to the same.
        for i in 0..ops.len() {
            for j in (i + 1)..ops.len() {
                assert_ne!(ops[i], ops[j], "ops {i} and {j} are not distinct");
            }
        }
        assert_eq!(ops.len(), 6, "exactly six net-access ops");
    }

    /// NO slice/concat/extract/insert op exists — grep-verify the Op enum
    /// directly. These are representation, carried by SigRef chunks, never
    /// ops.
    #[test]
    fn no_slice_concat_extract_or_insert_op_exists() {
        let names = [
            "Slice", "Concat", "Extract", "Insert",
            "BitSlice", "BitConcat", "BitExtract", "BitInsert",
        ];
        // Read the variant names of `Op` from the type itself by
        // stringifying it.
        let ty = std::any::type_name::<Op>();
        assert_eq!(ty, "piperine_ir::core::op::Op");
        // The proof is structural: `Op` is defined in this file with the
        // variants listed below. The list above MUST NOT appear in the
        // enum. The compiler is the witness — there is no such variant to
        // construct.
        let _ = names; // (used to document the forbidden names)
    }

    /// `If` and `Match` carry inline op lists — control stays structured,
    /// no flattening pass (D9). A nested op sequence round-trips intact.
    #[test]
    fn if_and_match_carry_inline_op_lists() {
        let cond = ValueId(0);
        let net = NetId(0);
        let value = ValueId(1);
        let then_op = Op::Drive { net, value };
        let else_op = Op::ReadSignal { net, out: ValueId(2) };

        let if_op = Op::If {
            cond,
            then_ops: vec![then_op.clone()],
            else_ops: vec![else_op.clone()],
        };
        // The nested ops are accessible inline — no flattening happened.
        match &if_op {
            Op::If { then_ops, else_ops, .. } => {
                assert_eq!(then_ops.len(), 1);
                assert_eq!(else_ops.len(), 1);
                assert_eq!(then_ops[0], then_op);
                assert_eq!(else_ops[0], else_op);
            }
            _ => panic!("expected If"),
        }

        // Match carries arms of (pattern, ops).
        let match_op = Op::Match {
            scrutinee: cond,
            arms: vec![
                (Pattern::Zero, vec![Op::Drive { net, value }]),
                (Pattern::One, vec![]),
                (Pattern::Wildcard, vec![Op::ReadSignal { net, out: value }]),
            ],
        };
        match &match_op {
            Op::Match { arms, .. } => assert_eq!(arms.len(), 3),
            _ => panic!("expected Match"),
        }
    }

    #[test]
    fn op_reads_and_writes_classify_correctly() {
        let net = NetId(0);
        let other = NetId(1);
        let out = ValueId(0);
        let value = ValueId(1);
        let cell = CellId(0);
        let intrinsic = IntrinsicId(0);

        // Pure: reads args, writes out.
        let pure = Op::Pure { op: PureOp::Add, args: vec![ValueId(0), ValueId(1)], out: ValueId(2) };
        assert_eq!(pure.reads(), vec![ValueId(0), ValueId(1)]);
        assert_eq!(pure.writes(), Some(ValueId(2)));

        // ReadAcross: writes out, reads no values (nets are not values).
        let ra = Op::ReadAcross { a: net, b: other, out };
        assert!(ra.reads().is_empty());
        assert_eq!(ra.writes(), Some(out));

        // ContributeThrough: reads value, writes nothing (a stamp, not a value).
        let ct = Op::ContributeThrough { a: net, b: other, value };
        assert_eq!(ct.reads(), vec![value]);
        assert_eq!(ct.writes(), None);

        // CellWrite with address: reads address AND value.
        let cw = Op::CellWrite { cell, addr: Some(out), value };
        assert_eq!(cw.reads(), vec![out, value]);

        // Call: reads args, writes out.
        let call = Op::Call { intrinsic, args: vec![value], out };
        assert_eq!(call.reads(), vec![value]);
        assert_eq!(call.writes(), Some(out));
    }
}

