//! Analog backend — reactive state and operators (T20, LOV-16).
//!
//! The `ddt` companion model (`ddt(q) -> i`, backward Euler:
//! `i = (q(t) - q(t-h)) / h`) needs history — `q(t-h)`. Per
//! `piperine_ir::core::cell`'s module doc, that history is exactly a
//! [`Cell`]: "operator state slots... advance on accept" (this task's
//! Done-when) is the same mechanism `piperine_ir`'s five concepts already
//! give a digital register — a scalar cell whose single writer is the SAME
//! block that reads it. [`Companion::backward_euler`] is the reusable
//! construction helper: given the charge `Q(V)` already computed (as
//! ordinary `Pure` ops — `Q(V) = C * V` needs no new op or intrinsic),
//! it reads the previous charge from a cell, computes the companion
//! current, and writes the new charge back — three ops any analog backend
//! (this one, T18/T19's residual/Jacobian machinery, and the reference
//! interpreter, which already supports `CellRead`/`Pure`/`CellWrite`)
//! executes identically.
//!
//! This deliberately does NOT add a `ddt` `Call`/intrinsic to the op graph:
//! `piperine_ir::interp::Interpreter::eval_call` (T13) has no case for
//! `"ddt"` (its module doc states the interpreter does not solve KCL or
//! carry reactive companion state — that is a backend concern, this task).
//! Expressing the companion model as ordinary `Pure`/`CellRead`/`CellWrite`
//! ops keeps the reactive fixture's differential test a REAL cross-check
//! against the reference interpreter (both evaluators already implement
//! every op this pattern uses) rather than inventing an op only one side
//! understands.

use piperine_ir::core::{BlockId, CellId, Module, Op, PureOp, Ty, ValueId};

/// Builds the backward-Euler `ddt` companion model out of ordinary IR ops.
pub struct Companion;

impl Companion {
    /// Push `i = (q_now - q_prev) / h` into `block`, where `q_prev` is read
    /// from `history` (a scalar `real` cell whose declared writer is
    /// `block`) and `q_now` is written back into `history` at the end —
    /// "operator state slots... advance on accept". Returns the `ValueId`
    /// of the companion current `i`.
    pub fn backward_euler(module: &mut Module, block: BlockId, q_now: ValueId, history: CellId, h: ValueId) -> ValueId {
        let q_prev = module.push_value(Ty::Real);
        module.block_at_mut(block).push(Op::CellRead { cell: history, addr: None, out: q_prev });

        let dq = module.push_value(Ty::Real);
        module.block_at_mut(block).push(Op::Pure { op: PureOp::Sub, args: vec![q_now, q_prev], out: dq });

        let i = module.push_value(Ty::Real);
        module.block_at_mut(block).push(Op::Pure { op: PureOp::Div, args: vec![dq, h], out: i });

        module.block_at_mut(block).push(Op::CellWrite { cell: history, addr: None, value: q_now });

        i
    }
}
