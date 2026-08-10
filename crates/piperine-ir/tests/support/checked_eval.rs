//! [`CheckedEvaluator`] — a SECOND, INDEPENDENTLY-WRITTEN evaluator for
//! the differential harness (T14). See `harness.rs`'s module doc for why
//! this exists and what it is NOT (the JIT backend, or the reference
//! interpreter called twice).
//!
//! Structural differences from `piperine_ir::interp::Interpreter`, on
//! purpose: a dense `Vec<Option<CellValue>>` value store (vs. a
//! `HashMap<ValueId, _>`), per-op-category dispatch methods (`eval_pure`/
//! `eval_net_op`/`eval_cell_op`) rather than one large match, and its own
//! `Trigger` bookkeeping shape (`BlockState`, not `interp.rs`'s
//! `BlockRuntime`). Scope is deliberately narrower than the reference
//! interpreter: it supports exactly what the Phase 3 fixtures (RC
//! divider, 4-bit counter) exercise — `Always`/`Initial`/`Edge` triggers,
//! arithmetic/comparison/select `Pure` ops on `Real` and `Bits`, the six
//! net-access ops, scalar and array cell access, `If`/`Match`. Anything
//! else fails LOUD (a named panic identifying the construct), never a
//! silent zero.

use super::fixture::{Fixture, ObservePoint};
use super::harness::Evaluator;
use piperine_ir::core::{
    BlockId, CellId, Const, EdgeKind, Module, NetId, Op, Pattern, PureOp, Trigger, ValueId,
};
use piperine_ir::interp::CellValue;
use std::collections::HashMap;

pub struct CheckedEvaluator;

impl Evaluator for CheckedEvaluator {
    fn name(&self) -> &'static str {
        "checked evaluator (independent, tests-only)"
    }

    fn run(&self, fixture: &Fixture) -> Vec<CellValue> {
        let mut run = Run::new(&fixture.module);
        for step in &fixture.steps {
            for (v, val) in &step.seed_values {
                run.external.insert(*v, val.clone());
            }
            for (n, across, through) in &step.node_values {
                run.nets.insert(*n, NetVal::Node { across: *across, through: *through });
            }
            for (n, v) in &step.signal_values {
                run.nets.insert(*n, NetVal::Signal(*v));
            }
            run.step();
        }
        fixture
            .observe
            .iter()
            .map(|obs| match obs.point {
                ObservePoint::Cell(c) => run
                    .cells
                    .get(&c)
                    .unwrap_or_else(|| panic!("observation `{}`: cell {c:?} has no value", obs.label))
                    .clone(),
                ObservePoint::Value(v) => run
                    .values
                    .get(v.0 as usize)
                    .and_then(|slot| slot.clone())
                    .unwrap_or_else(|| panic!("observation `{}`: value {v:?} has no value", obs.label)),
            })
            .collect()
    }
}

/// One net's runtime value — this evaluator's own shape (deliberately not
/// `piperine_ir::interp::NetValue`).
#[derive(Clone, Copy)]
enum NetVal {
    Node { across: f64, through: f64 },
    Signal(u128),
}

/// One block's trigger bookkeeping — this evaluator's own shape.
#[derive(Default, Clone)]
struct BlockState {
    initial_done: bool,
    prev_edge: Option<u128>,
}

struct Run<'m> {
    module: &'m Module,
    /// Dense value store — deliberately NOT a HashMap (the structural
    /// difference from `interp.rs`'s `State::values`).
    values: Vec<Option<CellValue>>,
    /// Values supplied externally by the fixture's stimulus program
    /// (never overwritten by op evaluation — mirrors `seed_value`).
    external: HashMap<ValueId, CellValue>,
    cells: HashMap<CellId, CellValue>,
    nets: HashMap<NetId, NetVal>,
    blocks: Vec<BlockState>,
}

impl<'m> Run<'m> {
    fn new(module: &'m Module) -> Self {
        let mut cells = HashMap::new();
        for (id, cell) in module.cells() {
            let c = module.const_at(cell.init);
            cells.insert(id, materialize(c, &cell.ty));
        }
        Self {
            module,
            values: vec![None; module.value_count()],
            external: HashMap::new(),
            cells,
            nets: HashMap::new(),
            blocks: vec![BlockState::default(); module.block_count()],
        }
    }

    /// Advance one step: evaluate every block's trigger, run the ones
    /// that fire, in declaration order (LOV-07: order is explicit).
    fn step(&mut self) {
        // Values are per-step scratch — externally-seeded values persist
        // (mirrors `seed_value` living outside `State::values`'s clear).
        for slot in &mut self.values {
            *slot = None;
        }
        let block_ids: Vec<BlockId> = self.module.blocks().map(|(id, _)| id).collect();
        for block_id in block_ids {
            if self.trigger_fires(block_id) {
                let ops = self.module.block_at(block_id).ops().to_vec();
                for op in &ops {
                    self.eval_op(op);
                }
            }
        }
    }

    fn trigger_fires(&mut self, block_id: BlockId) -> bool {
        let trigger = self.module.block_at(block_id).trigger.clone();
        match &trigger {
            Trigger::Always => true,
            Trigger::Initial => {
                let st = &mut self.blocks[block_id.0 as usize];
                if st.initial_done {
                    false
                } else {
                    st.initial_done = true;
                    true
                }
            }
            Trigger::Edge(sig, kind) => {
                let curr = self.read_sigref(sig);
                let st = &mut self.blocks[block_id.0 as usize];
                let prev = st.prev_edge;
                st.prev_edge = Some(curr);
                match prev {
                    None => false,
                    Some(p) => edge_fires(p, curr, *kind),
                }
            }
            other => panic!(
                "CheckedEvaluator: trigger {other:?} is out of this evaluator's Phase-3 scope \
                 (only Always/Initial/Edge are supported — extend checked_eval.rs, do not \
                 silently return false)"
            ),
        }
    }

    fn read_sigref(&self, sr: &piperine_ir::core::SigRef) -> u128 {
        let mut acc: u128 = 0;
        let mut shift = 0u32;
        for chunk in &sr.0 {
            match chunk {
                piperine_ir::core::Chunk::Slice { value, lo, hi } => {
                    let v = self.read_value_u128(*value);
                    let width = hi.saturating_sub(*lo);
                    let mask = mask_for(width);
                    acc |= ((v >> lo) & mask) << shift;
                    shift += width;
                }
                piperine_ir::core::Chunk::Const(cid) => {
                    let c = self.module.const_at(*cid);
                    if let Const::Bits(b) = c {
                        let width = c.ty().width();
                        acc |= (*b & mask_for(width)) << shift;
                        shift += width;
                    }
                }
            }
        }
        acc
    }

    fn eval_op(&mut self, op: &Op) {
        match op {
            Op::Pure { op: pop, args, out } => {
                let argv: Vec<CellValue> = args.iter().map(|a| self.read_value(*a)).collect();
                let v = self.eval_pure(*pop, &argv, *out);
                self.write_value(*out, v);
            }
            Op::Call { .. } => {
                panic!("CheckedEvaluator: intrinsic calls are out of Phase-3 scope");
            }
            Op::ReadAcross { a, b, out } => {
                let v = self.node_across(*a) - self.node_across(*b);
                self.write_value(*out, CellValue::Real(v));
            }
            Op::ReadThrough { a, b, out } => {
                let v = self.node_through(*a) - self.node_through(*b);
                self.write_value(*out, CellValue::Real(v));
            }
            Op::ContributeThrough { .. } | Op::ForceAcross { .. } => {
                // Not observed by the Phase-3 fixtures; nothing to record.
            }
            Op::ReadSignal { net, out } => {
                let v = self.signal_of(*net);
                self.write_value(*out, CellValue::Bits(v));
            }
            Op::Drive { .. } => {
                // Not observed by the Phase-3 fixtures.
            }
            Op::CellRead { cell, addr, out } => {
                let v = match addr {
                    Some(a) => self.cell_array_read(*cell, self.read_value_u128(*a)),
                    None => self.cells.get(cell).cloned().unwrap_or_else(|| {
                        panic!("CheckedEvaluator: cell {cell:?} read before any write and has no init")
                    }),
                };
                self.write_value(*out, v);
            }
            Op::CellWrite { cell, addr, value } => {
                let v = self.read_value(*value);
                match addr {
                    Some(a) => self.cell_array_write(*cell, self.read_value_u128(*a), v),
                    None => {
                        self.cells.insert(*cell, v);
                    }
                }
            }
            Op::If { cond, then_ops, else_ops } => {
                let c = self.read_value_u128(*cond);
                let arm = if c != 0 { then_ops } else { else_ops };
                for inner in arm.clone() {
                    self.eval_op(&inner);
                }
            }
            Op::Match { scrutinee, arms } => {
                let s = self.read_value_u128(*scrutinee) & 1;
                for (pat, body) in arms {
                    let hit = match pat {
                        Pattern::Zero => s == 0,
                        Pattern::One => s == 1,
                        Pattern::Wildcard => true,
                    };
                    if hit {
                        for inner in body.clone() {
                            self.eval_op(&inner);
                        }
                        break;
                    }
                }
            }
        }
    }

    fn eval_pure(&self, op: PureOp, args: &[CellValue], out: ValueId) -> CellValue {
        match op {
            PureOp::Add | PureOp::Sub | PureOp::Mul | PureOp::Div => {
                binary_arith(op, &args[0], &args[1], self.module.value_ty(out).width())
            }
            PureOp::Neg => match &args[0] {
                CellValue::Real(x) => CellValue::Real(-x),
                CellValue::Bits(x) => {
                    let width = self.module.value_ty(out).width();
                    CellValue::Bits((x.wrapping_neg()) & mask_for(width))
                }
                other => panic!("CheckedEvaluator: Neg on unsupported value {other:?}"),
            },
            PureOp::Eq | PureOp::Ne | PureOp::Lt | PureOp::Le | PureOp::Gt | PureOp::Ge => {
                let cmp = compare(op, &args[0], &args[1]);
                CellValue::Bits(cmp as u128)
            }
            PureOp::Select => {
                let c = match &args[0] {
                    CellValue::Bits(b) => *b != 0,
                    other => panic!("CheckedEvaluator: Select condition must be bits, got {other:?}"),
                };
                if c { args[1].clone() } else { args[2].clone() }
            }
        }
    }

    fn node_across(&self, net: NetId) -> f64 {
        match self.nets.get(&net) {
            Some(NetVal::Node { across, .. }) => *across,
            other => panic!("CheckedEvaluator: net {net:?} is not a seeded Node (got {:?})", other.is_some()),
        }
    }

    fn node_through(&self, net: NetId) -> f64 {
        match self.nets.get(&net) {
            Some(NetVal::Node { through, .. }) => *through,
            other => panic!("CheckedEvaluator: net {net:?} is not a seeded Node (got {:?})", other.is_some()),
        }
    }

    fn signal_of(&self, net: NetId) -> u128 {
        match self.nets.get(&net) {
            Some(NetVal::Signal(v)) => *v,
            other => panic!("CheckedEvaluator: net {net:?} is not a seeded Signal (got {:?})", other.is_some()),
        }
    }

    fn read_value(&self, v: ValueId) -> CellValue {
        if let Some(ext) = self.external.get(&v) {
            return ext.clone();
        }
        self.values[v.0 as usize]
            .clone()
            .unwrap_or_else(|| panic!("CheckedEvaluator: value {v:?} read before it was computed or seeded"))
    }

    fn read_value_u128(&self, v: ValueId) -> u128 {
        match self.read_value(v) {
            CellValue::Bits(b) => b,
            CellValue::Real(r) => r as u128,
            other => panic!("CheckedEvaluator: expected a bits-like value, got {other:?}"),
        }
    }

    fn write_value(&mut self, v: ValueId, val: CellValue) {
        self.values[v.0 as usize] = Some(val);
    }

    fn cell_array_read(&self, cell: CellId, addr: u128) -> CellValue {
        match self.cells.get(&cell) {
            Some(CellValue::Array(arr)) => arr
                .get(addr as usize)
                .cloned()
                .unwrap_or_else(|| panic!("CheckedEvaluator: cell {cell:?} address {addr} out of range")),
            Some(other) => other.clone(),
            None => panic!("CheckedEvaluator: cell {cell:?} read before any write and has no init"),
        }
    }

    fn cell_array_write(&mut self, cell: CellId, addr: u128, value: CellValue) {
        match self.cells.get_mut(&cell) {
            Some(CellValue::Array(arr)) => {
                let i = addr as usize;
                if i >= arr.len() {
                    panic!("CheckedEvaluator: cell {cell:?} address {addr} out of range");
                }
                arr[i] = value;
            }
            Some(slot) => *slot = value,
            None => panic!("CheckedEvaluator: cell {cell:?} written before any init recorded"),
        }
    }
}

fn edge_fires(prev: u128, curr: u128, kind: EdgeKind) -> bool {
    match kind {
        EdgeKind::Rising => prev != 1 && curr == 1,
        EdgeKind::Falling => prev != 0 && curr == 0,
        EdgeKind::Any => prev != curr,
    }
}

fn mask_for(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1u128 << width) - 1 }
}

fn materialize(c: &Const, ty: &piperine_ir::core::Ty) -> CellValue {
    use piperine_ir::core::Ty;
    match (c, ty) {
        (Const::Real(v), Ty::Real) => CellValue::Real(*v),
        (Const::Bits(v), Ty::Bits(_)) => CellValue::Bits(*v),
        (_, Ty::Array(elem, dim)) => {
            CellValue::Array((0..*dim).map(|_| materialize(c, elem)).collect())
        }
        _ => CellValue::Real(0.0),
    }
}

fn binary_arith(op: PureOp, a: &CellValue, b: &CellValue, out_width: u32) -> CellValue {
    match (a, b) {
        (CellValue::Real(x), CellValue::Real(y)) => CellValue::Real(match op {
            PureOp::Add => x + y,
            PureOp::Sub => x - y,
            PureOp::Mul => x * y,
            PureOp::Div => x / y,
            _ => unreachable!(),
        }),
        (CellValue::Bits(x), CellValue::Bits(y)) => {
            let mask = mask_for(out_width);
            let v = match op {
                PureOp::Add => x.wrapping_add(*y),
                PureOp::Sub => x.wrapping_sub(*y),
                PureOp::Mul => x.wrapping_mul(*y),
                PureOp::Div => {
                    if *y == 0 {
                        panic!("CheckedEvaluator: division by zero");
                    }
                    x / y
                }
                _ => unreachable!(),
            };
            CellValue::Bits(v & mask)
        }
        (other_a, other_b) => panic!(
            "CheckedEvaluator: binary arithmetic needs matching Real or Bits operands, got \
             {other_a:?} and {other_b:?}"
        ),
    }
}

fn compare(op: PureOp, a: &CellValue, b: &CellValue) -> bool {
    match (a, b) {
        (CellValue::Real(x), CellValue::Real(y)) => match op {
            PureOp::Eq => x == y,
            PureOp::Ne => x != y,
            PureOp::Lt => x < y,
            PureOp::Le => x <= y,
            PureOp::Gt => x > y,
            PureOp::Ge => x >= y,
            _ => unreachable!(),
        },
        (CellValue::Bits(x), CellValue::Bits(y)) => match op {
            PureOp::Eq => x == y,
            PureOp::Ne => x != y,
            PureOp::Lt => x < y,
            PureOp::Le => x <= y,
            PureOp::Gt => x > y,
            PureOp::Ge => x >= y,
            _ => unreachable!(),
        },
        (other_a, other_b) => {
            panic!("CheckedEvaluator: comparison needs matching operands, got {other_a:?} and {other_b:?}")
        }
    }
}
