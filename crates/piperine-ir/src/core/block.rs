//! IR blocks — ops + a domain + a trigger (ideal.md §1.5).
//!
//! A block is the unit of execution: ops run in their recorded order when
//! the trigger fires, in the block's domain. Op order is EXPLICIT and
//! preserved (LOV-07 — order-dependent semantics are explicit, never
//! implied by traversal).
//!
//! The two domains produce different things and the IR keeps them apart:
//! an `analog` block reads `read_across`/`read_through`/`read_signal` and
//! writes `contribute_through`/`force_across`; a `digital` block reads
//! `read_signal`/`read_across` (cross-sampling) and writes `drive`. The
//! verifier (T7) checks domain/op consistency.

use super::{Domain, Op, Trigger};

/// One block of ops plus the domain it produces into and the trigger that
/// fires it. Ops run in their recorded order — order is the source of
/// truth, never implied by traversal.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Which world this block produces into (`analog` residual or
    /// `digital` drive).
    pub domain: Domain,
    /// What fires the block — `always`, `edge`, `cross`, `timer`, `initial`.
    pub trigger: Trigger,
    /// The op sequence, in execution order. The verifier (T7) checks
    /// domain/trigger/opcode consistency; here it is just a list.
    pub ops: Vec<Op>,
}

impl Block {
    /// Construct a block with a domain and a trigger; ops start empty and
    /// are pushed in execution order.
    pub fn new(domain: Domain, trigger: Trigger) -> Self {
        Self { domain, trigger, ops: Vec::new() }
    }

    /// Push an op in execution order. The order is preserved verbatim —
    /// no reordering, no implicit traversal.
    pub fn push(&mut self, op: Op) {
        self.ops.push(op);
    }

    /// The op at `index`, in execution order.
    pub fn op_at(&self, index: usize) -> &Op {
        &self.ops[index]
    }

    /// The full op sequence, in execution order — for passes that walk
    /// the block (the verifier's op-width check, the eventual diff pass).
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CellId, NetId, ValueId};

    #[test]
    fn an_analog_block_holds_a_trigger_and_an_op_sequence() {
        let mut block = Block::new(Domain::Analog, Trigger::Always);
        let n = NetId(0);
        let g = NetId(1);
        let out = ValueId(0);
        let value = ValueId(1);
        block.push(Op::ReadAcross { a: n, b: g, out });
        block.push(Op::ContributeThrough { a: n, b: g, value });
        assert_eq!(block.domain, Domain::Analog);
        assert_eq!(block.trigger, Trigger::Always);
        assert_eq!(block.ops().len(), 2);
    }

    #[test]
    fn a_digital_block_carries_a_clock_edge_trigger() {
        let sr = crate::core::SigRef(vec![crate::core::Chunk::Slice {
            value: ValueId(0),
            lo: 0,
            hi: 1,
        }]);
        let mut block = Block::new(Domain::Digital, Trigger::Edge(sr, crate::core::EdgeKind::Rising));
        let sig = NetId(2);
        let value = ValueId(3);
        block.push(Op::Drive { net: sig, value });
        assert_eq!(block.domain, Domain::Digital);
        match &block.trigger {
            Trigger::Edge(_, crate::core::EdgeKind::Rising) => {}
            _ => panic!("expected Edge Rising trigger"),
        }
        assert_eq!(block.ops().len(), 1);
    }

    /// LOV-07 — op order within a block is EXPLICIT and preserved. The
    /// block does not reorder; the verifier and the backends see the
    /// sequence as authored.
    #[test]
    fn op_order_is_explicit_and_preserved_on_round_trip() {
        let mut block = Block::new(Domain::Analog, Trigger::Always);
        let net = NetId(0);
        let g = NetId(1);
        let v0 = ValueId(0);
        let v1 = ValueId(1);
        let v2 = ValueId(2);
        let cell = CellId(0);

        // Push a deliberately ordered mix: read, call, contribute, write.
        block.push(Op::ReadAcross { a: net, b: g, out: v0 });
        block.push(Op::CellWrite { cell, addr: None, value: v1 });
        block.push(Op::ContributeThrough { a: net, b: g, value: v2 });

        // Round-trip — the order must match exactly.
        assert_eq!(block.ops().len(), 3);
        assert!(matches!(block.op_at(0), Op::ReadAcross { .. }));
        assert!(matches!(block.op_at(1), Op::CellWrite { .. }));
        assert!(matches!(block.op_at(2), Op::ContributeThrough { .. }));
    }

    #[test]
    fn nested_if_round_trips_with_its_inner_op_order_preserved() {
        // Op order matters inside If/Match too — the verifier and the
        // backends see the inner sequence as authored, in order.
        let mut block = Block::new(Domain::Digital, Trigger::Always);
        let net = NetId(0);
        let v0 = ValueId(0);
        let v1 = ValueId(1);
        let inner_a = Op::Drive { net, value: v0 };
        let inner_b = Op::ReadSignal { net, out: v1 };
        let if_op = Op::If {
            cond: v0,
            then_ops: vec![inner_a.clone(), inner_b.clone()],
            else_ops: vec![],
        };
        block.push(if_op);

        match block.op_at(0) {
            Op::If { then_ops, .. } => {
                assert_eq!(then_ops.len(), 2);
                assert!(matches!(then_ops[0], Op::Drive { .. }));
                assert!(matches!(then_ops[1], Op::ReadSignal { .. }));
            }
            _ => panic!("expected If"),
        }
    }
}
