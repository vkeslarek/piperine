//! The reference interpreter (LOV-10) — a direct IR walker. Slow,
//! obvious, no Cranelift. It evaluates blocks, advances cells, honours
//! triggers. It buys two things nothing else does:
//!
//! 1. **Differential testing.** Interpreter and JIT must agree on every
//!    input. (Phase 3's harness, T14.)
//! 2. **Traceability.** Step the IR, watch cells change — the direct
//!    answer to "I lost traceability of what is happening in my own code."
//!
//! ## Scope (this task)
//!
//! The interpreter evaluates the op graph and produces the contribution/
//! force VALUES; it does not stamp a matrix. A full analog solver is the
//! JIT backend's job (Phase 4). For T13, the interpreter is given analog
//! node values as inputs (via [`Interpreter::set_node_values`]) and
//! produces contribution/force values as outputs (via
//! [`Interpreter::contribution`] / [`Interpreter::force`]). That keeps
//! it pure and lets it be a differential oracle.
//!
//! ## Owner
//!
//! An [`Interpreter`] struct holding a `&Module` (read-only) and a
//! [`State`] (the runtime cell bank, net value cache, and per-block
//! trigger bookkeeping). Methods on the struct (MD-13 rule 2: not free
//! functions).

use crate::catalog::Catalog;
use crate::core::{
    BlockId, Cell, CellId, Const, CrossDir, EdgeKind, Module, NetId, Op,
    PureOp, Trigger, Ty, ValueId,
};
use std::collections::HashMap;

/// A runtime cell value — the interpreter's representation of a cell's
/// current contents. Mirrors the IR's two value classes (real / bits)
/// plus the aggregate (array).
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Real(f64),
    Bits(u128),
    Array(Vec<CellValue>),
}

/// A runtime net value — across/through for a conservative node, or a
/// discrete signal value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetValue {
    /// A node's across and through natures (e.g. voltage/current pair).
    Node { across: f64, through: f64 },
    /// A signal's value (a `bits<N>` word, zero-extended).
    Signal(u128),
}

/// The outcome of a [`Interpreter::step`] — which blocks fired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepOutcome {
    /// The ids of blocks that fired this step, in firing order.
    pub fired_blocks: Vec<BlockId>,
}

/// Why an interpreter step failed. Closed enum, fail-loud (no silent
/// 0.0 / no-op per the project rule). The interpreter is the executable
/// semantics; a failure here is a defect in the IR under evaluation,
/// never something to paper over.
#[derive(Debug, Clone, PartialEq)]
pub enum InterpError {
    /// A value was referenced before it was computed (a dangling
    /// ValueId). Names the value and the failing op site.
    UncomputedValue { value: ValueId },
    /// A cell was read before any write to it AND has no init (the
    /// init pool is empty for this cell). Names the cell.
    UninitializedCell { cell: CellId },
    /// A net access on the wrong kind (the A0b mechanism — analog
    /// stamping on a Signal). Names the net.
    NetKindMismatch { net: NetId },
    /// A call to an unknown intrinsic id (catalog has no entry at this
    /// index). Names the id.
    UnknownIntrinsic { id: u32 },
    /// An op variant the interpreter does not yet handle. Loud failure
    /// (no silent skip); names the kind.
    UnsupportedOp { kind: &'static str },
    /// A memory cell access at an out-of-range address.
    AddressOutOfRange { cell: CellId, addr: u64, len: u32 },
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpError::UncomputedValue { value } => {
                write!(f, "interpreter: value {value:?} referenced before it was computed")
            }
            InterpError::UninitializedCell { cell } => {
                write!(f, "interpreter: cell {cell:?} read before any write and has no init")
            }
            InterpError::NetKindMismatch { net } => {
                write!(f, "interpreter: net {net:?} accessed with the wrong op for its kind")
            }
            InterpError::UnknownIntrinsic { id } => {
                write!(f, "interpreter: call to unknown intrinsic id {id}")
            }
            InterpError::UnsupportedOp { kind } => {
                write!(f, "interpreter: unsupported op kind `{kind}`")
            }
            InterpError::AddressOutOfRange { cell, addr, len } => {
                write!(f, "interpreter: address {addr} out of range for cell {cell:?} (len {len})")
            }
        }
    }
}

impl std::error::Error for InterpError {}

/// Per-block runtime trigger state. Lives in [`State::block_runtime`].
#[derive(Debug, Clone, Default)]
struct BlockRuntime {
    /// For `Initial`: has fired once already?
    initial_fired: bool,
    /// For `Edge`: the previous signal value (None = first evaluation).
    prev_edge_value: Option<u128>,
    /// For `Cross`: the previous expression value (None = unseeded).
    prev_cross_value: Option<f64>,
    /// For `Timer`: the next fire time.
    next_timer_fire: f64,
}

/// The runtime state — cell bank, net value cache, and per-block
/// trigger bookkeeping. Owned by [`Interpreter`]; consult via
/// [`Interpreter::cell_value`] etc.
#[derive(Debug, Clone)]
pub struct State {
    /// Cell bank, keyed by CellId. Initialised from each Cell's `init`
    /// constant when the state is constructed.
    cells: HashMap<CellId, CellValue>,
    /// Net value cache, keyed by NetId. Set externally by the caller
    /// (analog node values are inputs the interpreter does not solve).
    nets: HashMap<NetId, NetValue>,
    /// Computed value slots, keyed by ValueId. Populated by op
    /// evaluation as each op runs.
    values: HashMap<ValueId, CellValue>,
    /// The current step clock.
    time: f64,
    /// Per-block runtime trigger state.
    block_runtime: HashMap<BlockId, BlockRuntime>,
    /// Contributions accumulated in the last step (per net pair). The
    /// caller reads these via [`Interpreter::contribution`].
    contributions: HashMap<(NetId, NetId), f64>,
    /// Forces accumulated in the last step (per net pair).
    forces: HashMap<(NetId, NetId), f64>,
    /// Drives accumulated in the last step (per net).
    drives: HashMap<NetId, u128>,
}

impl State {
    /// Build a fresh state for `module`: every cell at its `init`
    /// constant, every block's runtime at its zero state, time = 0.
    pub fn for_module(module: &Module) -> Self {
        let mut cells = HashMap::new();
        for (cell_id, cell) in module.cells() {
            let v = Self::initial_value(module, cell);
            cells.insert(cell_id, v);
        }
        let mut block_runtime = HashMap::new();
        for (block_id, block) in module.blocks() {
            let mut rt = BlockRuntime::default();
            // Timer: first fire at phase (if > 0) else period. The two
            // are ValueIds we cannot evaluate here (the inputs may not
            // be set yet); the interpreter resolves them at step time.
            // For init, we set next_timer_fire = 0 (the first step
            // recomputes it if the trigger is a Timer).
            let _ = block;
            rt.next_timer_fire = 0.0;
            block_runtime.insert(block_id, rt);
        }
        Self {
            cells,
            nets: HashMap::new(),
            values: HashMap::new(),
            time: 0.0,
            block_runtime,
            contributions: HashMap::new(),
            forces: HashMap::new(),
            drives: HashMap::new(),
        }
    }

    /// Build the initial cell value from the cell's `init` constant.
    fn initial_value(module: &Module, cell: &Cell) -> CellValue {
        let init = module.const_at(cell.init);
        Self::const_to_value(init, &cell.ty)
    }

    /// Materialise a constant into a runtime value of the given type.
    fn const_to_value(c: &Const, ty: &Ty) -> CellValue {
        match (c, ty) {
            (Const::Real(v), Ty::Real) => CellValue::Real(*v),
            (Const::Bits(v), Ty::Bits(_)) => CellValue::Bits(*v),
            (Const::Bits(_), Ty::Array(elem, dim)) => {
                // Replicate the init constant across the array.
                let mut arr = Vec::with_capacity(*dim as usize);
                for _ in 0..*dim {
                    arr.push(Self::const_to_value(c, elem));
                }
                CellValue::Array(arr)
            }
            (Const::Real(_), Ty::Array(elem, dim)) => {
                let mut arr = Vec::with_capacity(*dim as usize);
                for _ in 0..*dim {
                    arr.push(Self::const_to_value(c, elem));
                }
                CellValue::Array(arr)
            }
            _ => CellValue::Real(0.0),
        }
    }

    /// The current simulation time.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Advance the clock to `t`.
    pub fn advance_to(&mut self, t: f64) {
        self.time = t;
    }
}

/// The reference interpreter. Owns a `&Module` reference and a [`State`]
/// it mutates as it steps. Methods only (MD-13 rule 2). The first
/// executable form — keep it OBVIOUS, not fast.
pub struct Interpreter<'a> {
    module: &'a Module,
    state: State,
}

impl<'a> Interpreter<'a> {
    /// Build an interpreter over `module`, with state initialised from
    /// the module's cells (each cell at its `init`) and blocks (each
    /// runtime at its zero state).
    pub fn new(module: &'a Module) -> Self {
        Self { module, state: State::for_module(module) }
    }

    /// Step: fire every block whose trigger fires at the current time,
    /// in declaration order. Returns the list of blocks that fired.
    /// Idempotent over a single time point (firing Initial twice in a
    /// row does not re-fire it).
    pub fn step(&mut self) -> Result<StepOutcome, InterpError> {
        // Clear per-step accumulators (contributions/forces/drives).
        self.state.contributions.clear();
        self.state.forces.clear();
        self.state.drives.clear();
        let mut fired = Vec::new();
        let block_ids: Vec<BlockId> = self.module.blocks().map(|(id, _)| id).collect();
        for block_id in block_ids {
            if self.trigger_fires(block_id)? {
                self.eval_block(block_id)?;
                fired.push(block_id);
            }
        }
        Ok(StepOutcome { fired_blocks: fired })
    }

    /// Advance the clock to `t`, then step.
    pub fn step_to(&mut self, t: f64) -> Result<StepOutcome, InterpError> {
        self.state.advance_to(t);
        self.step()
    }

    /// Set the across/through values of an analog node net (the
    /// interpreter is GIVEN node values; it does not solve KCL).
    pub fn set_node_values(&mut self, net: NetId, across: f64, through: f64) {
        self.state.nets.insert(net, NetValue::Node { across, through });
    }

    /// Set a digital signal value.
    pub fn set_signal(&mut self, net: NetId, value: u128) {
        self.state.nets.insert(net, NetValue::Signal(value));
    }

    /// Seed a value slot (a "param" / externally-supplied input value).
    /// Used by tests to set inputs the interpreter does not compute
    /// itself (e.g. a value referenced by a Timer's period).
    pub fn seed_value(&mut self, v: ValueId, value: CellValue) {
        self.state.values.insert(v, value);
    }

    /// Read a cell's current value.
    pub fn cell_value(&self, cell: CellId) -> Option<&CellValue> {
        self.state.cells.get(&cell)
    }

    /// Read a computed ValueId's value (the last op that wrote it).
    pub fn value(&self, v: ValueId) -> Option<&CellValue> {
        self.state.values.get(&v)
    }

    /// The accumulated `contribute_through` value for `(a, b)` in the
    /// last step. `None` if no contribution was made.
    pub fn contribution(&self, a: NetId, b: NetId) -> Option<f64> {
        self.state.contributions.get(&(a, b)).copied()
    }

    /// The accumulated `force_across` value for `(a, b)` in the last
    /// step.
    pub fn force(&self, a: NetId, b: NetId) -> Option<f64> {
        self.state.forces.get(&(a, b)).copied()
    }

    /// The accumulated `drive` value for `net` in the last step.
    pub fn drive(&self, net: NetId) -> Option<u128> {
        self.state.drives.get(&net).copied()
    }

    /// The current simulation time.
    pub fn time(&self) -> f64 {
        self.state.time
    }

    // ── trigger evaluation ────────────────────────────────────────────

    /// Does `block_id`'s trigger fire at the current step? Updates the
    /// block's runtime state (Initial marking as fired; Edge/Cross
    /// storing the previous value; Timer advancing next_fire).
    fn trigger_fires(&mut self, block_id: BlockId) -> Result<bool, InterpError> {
        let trigger = self.module.block_at(block_id).trigger.clone();
        // Pre-compute any value lookups the trigger needs (avoids
        // borrowing self.state.block_runtime while looking up values).
        let timer_inputs = match &trigger {
            Trigger::Timer { period, phase } => {
                Some((self.value_as_real(*period)?, self.value_as_real(*phase)?))
            }
            _ => None,
        };
        let edge_curr = match &trigger {
            Trigger::Edge(sr, _) => Some(self.eval_sigref(sr)?),
            _ => None,
        };
        let cross_curr = match &trigger {
            Trigger::Cross(v, _) => Some(self.value_as_real(*v)?),
            _ => None,
        };
        let rt = self.state.block_runtime.entry(block_id).or_default();
        let fires = match &trigger {
            Trigger::Always => true,
            Trigger::Initial => {
                if rt.initial_fired {
                    false
                } else {
                    rt.initial_fired = true;
                    true
                }
            }
            Trigger::Edge(_, kind) => {
                let curr = edge_curr.unwrap();
                let prev = rt.prev_edge_value;
                rt.prev_edge_value = Some(curr);
                match prev {
                    None => false,
                    Some(p) => Self::edge_fires(p, curr, *kind),
                }
            }
            Trigger::Cross(_, dir) => {
                let curr = cross_curr.unwrap();
                let prev = rt.prev_cross_value;
                rt.prev_cross_value = Some(curr);
                match prev {
                    None => false,
                    Some(p) => Self::cross_fires(p, curr, *dir),
                }
            }
            Trigger::Timer { .. } => {
                let (p, ph) = timer_inputs.unwrap();
                if rt.next_timer_fire == 0.0 && p > 0.0 {
                    rt.next_timer_fire = if ph > 0.0 { ph } else { p };
                }
                let fires = p > 0.0 && self.state.time >= rt.next_timer_fire;
                if fires {
                    while rt.next_timer_fire <= self.state.time {
                        rt.next_timer_fire += p;
                    }
                }
                fires
            }
        };
        Ok(fires)
    }

    fn edge_fires(prev: u128, curr: u128, kind: EdgeKind) -> bool {
        match kind {
            EdgeKind::Rising => prev != 1 && curr == 1,
            EdgeKind::Falling => prev != 0 && curr == 0,
            EdgeKind::Any => prev != curr,
        }
    }

    fn cross_fires(prev: f64, curr: f64, dir: CrossDir) -> bool {
        let rising = prev <= 0.0 && curr > 0.0;
        let falling = prev >= 0.0 && curr < 0.0;
        match dir {
            CrossDir::Rising => rising,
            CrossDir::Falling => falling,
            CrossDir::Either => rising || falling,
        }
    }

    /// Evaluate a SigRef to its current integer value (single-chunk Slice
    /// only for T13; multi-chunk is the printer/parse concern).
    fn eval_sigref(&self, sr: &crate::core::SigRef) -> Result<u128, InterpError> {
        let mut acc: u128 = 0;
        let mut shift: u32 = 0;
        for chunk in &sr.0 {
            match chunk {
                crate::core::Chunk::Slice { value, lo, hi } => {
                    let v = self.value_as_u128(*value)?;
                    let width = hi.saturating_sub(*lo);
                    let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
                    let bits = (v >> lo) & mask;
                    acc |= bits << shift;
                    shift += width;
                }
                crate::core::Chunk::Const(cid) => {
                    let c = self.module.const_at(*cid);
                    let v = match c {
                        Const::Bits(b) => *b,
                        Const::Real(_) => 0,
                    };
                    let width = c.ty().width();
                    let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
                    acc |= (v & mask) << shift;
                    shift += width;
                }
            }
        }
        Ok(acc)
    }

    // ── block / op evaluation ─────────────────────────────────────────

    fn eval_block(&mut self, block_id: BlockId) -> Result<(), InterpError> {
        let ops: Vec<Op> = self.module.block_at(block_id).ops().to_vec();
        for op in &ops {
            self.eval_op(op)?;
        }
        Ok(())
    }

    fn eval_op(&mut self, op: &Op) -> Result<(), InterpError> {
        match op {
            Op::Pure { op: pop, args, out } => {
                let argv: Vec<CellValue> = args
                    .iter()
                    .map(|a| self.value(*a).cloned().ok_or(InterpError::UncomputedValue { value: *a }))
                    .collect::<Result<_, _>>()?;
                let v = self.eval_pure(*pop, &argv)?;
                self.state.values.insert(*out, v);
            }
            Op::Call { intrinsic, args, out } => {
                let argv: Vec<f64> = args
                    .iter()
                    .map(|a| self.value_as_real(*a))
                    .collect::<Result<_, _>>()?;
                let v = self.eval_call(*intrinsic, &argv)?;
                self.state.values.insert(*out, CellValue::Real(v));
            }
            Op::ReadAcross { a, b, out } => {
                let v = self.read_across(*a, *b)?;
                self.state.values.insert(*out, CellValue::Real(v));
            }
            Op::ReadThrough { a, b, out } => {
                let v = self.read_through(*a, *b)?;
                self.state.values.insert(*out, CellValue::Real(v));
            }
            Op::ContributeThrough { a, b, value } => {
                let v = self.value_as_real(*value)?;
                let acc = self.state.contributions.entry((*a, *b)).or_insert(0.0);
                *acc += v;
            }
            Op::ForceAcross { a, b, value } => {
                let v = self.value_as_real(*value)?;
                self.state.forces.insert((*a, *b), v);
            }
            Op::ReadSignal { net, out } => {
                let v = match self.state.nets.get(net) {
                    Some(NetValue::Signal(s)) => *s,
                    _ => return Err(InterpError::NetKindMismatch { net: *net }),
                };
                self.state.values.insert(*out, CellValue::Bits(v));
            }
            Op::Drive { net, value } => {
                let v = self.value_as_u128(*value)?;
                self.state.drives.insert(*net, v);
            }
            Op::CellRead { cell, addr, out } => {
                let v = if let Some(addr_v) = addr {
                    let a = self.value_as_u128(*addr_v)?;
                    self.read_cell_array(*cell, a)?
                } else {
                    self.read_cell_scalar(*cell)?
                };
                self.state.values.insert(*out, v);
            }
            Op::CellWrite { cell, addr, value } => {
                let v = self.value(*value).cloned().ok_or(InterpError::UncomputedValue { value: *value })?;
                if let Some(addr_v) = addr {
                    let a = self.value_as_u128(*addr_v)?;
                    self.write_cell_array(*cell, a, v)?;
                } else {
                    self.state.cells.insert(*cell, v);
                }
            }
            Op::If { cond, then_ops, else_ops } => {
                let c = self.value_as_u128(*cond)?;
                let arm = if c != 0 { then_ops } else { else_ops };
                for inner in arm {
                    self.eval_op(inner)?;
                }
            }
            Op::Match { scrutinee, arms } => {
                let s = self.value_as_u128(*scrutinee)?;
                let bit = s & 1;
                for (pat, body) in arms {
                    let matches = match pat {
                        crate::core::Pattern::Zero => bit == 0,
                        crate::core::Pattern::One => bit == 1,
                        crate::core::Pattern::Wildcard => true,
                    };
                    if matches {
                        for inner in body {
                            self.eval_op(inner)?;
                        }
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    /// Evaluate a pure op over real or bits operands.
    fn eval_pure(&self, op: PureOp, args: &[CellValue]) -> Result<CellValue, InterpError> {
        match op {
            PureOp::Add | PureOp::Sub | PureOp::Mul | PureOp::Div => {
                let a = cell_as_real(&args[0]).ok_or(InterpError::UnsupportedOp { kind: "non-real Add" })?;
                let b = cell_as_real(&args[1]).ok_or(InterpError::UnsupportedOp { kind: "non-real Add" })?;
                let v = match op {
                    PureOp::Add => a + b,
                    PureOp::Sub => a - b,
                    PureOp::Mul => a * b,
                    PureOp::Div => a / b,
                    _ => unreachable!(),
                };
                Ok(CellValue::Real(v))
            }
            PureOp::Neg => {
                let a = cell_as_real(&args[0]).ok_or(InterpError::UnsupportedOp { kind: "non-real Neg" })?;
                Ok(CellValue::Real(-a))
            }
            PureOp::Eq | PureOp::Ne | PureOp::Lt | PureOp::Le | PureOp::Gt | PureOp::Ge => {
                let a = cell_as_real(&args[0]).ok_or(InterpError::UnsupportedOp { kind: "non-real cmp" })?;
                let b = cell_as_real(&args[1]).ok_or(InterpError::UnsupportedOp { kind: "non-real cmp" })?;
                let v: bool = match op {
                    PureOp::Eq => a == b,
                    PureOp::Ne => a != b,
                    PureOp::Lt => a < b,
                    PureOp::Le => a <= b,
                    PureOp::Gt => a > b,
                    PureOp::Ge => a >= b,
                    _ => unreachable!(),
                };
                Ok(CellValue::Bits(if v { 1 } else { 0 }))
            }
            PureOp::Select => {
                let c = cell_as_u128(&args[0]).ok_or(InterpError::UnsupportedOp { kind: "non-bits select cond" })?;
                Ok(if c != 0 { args[1].clone() } else { args[2].clone() })
            }
        }
    }

    /// Evaluate a math/limit call via the catalog.
    fn eval_call(&self, intrinsic: crate::core::IntrinsicId, args: &[f64]) -> Result<f64, InterpError> {
        let entry = Catalog::entries()
            .nth(intrinsic.0 as usize)
            .ok_or(InterpError::UnknownIntrinsic { id: intrinsic.0 })?;
        let v = match entry.name() {
            "sin" => args[0].sin(),
            "cos" => args[0].cos(),
            "tan" => args[0].tan(),
            "exp" | "limexp" => args[0].exp(),
            "ln" | "log" => args[0].ln(),
            "log10" => args[0].log10(),
            "sqrt" => args[0].sqrt(),
            "sinh" => args[0].sinh(),
            "cosh" => args[0].cosh(),
            "tanh" => args[0].tanh(),
            "asin" => args[0].asin(),
            "acos" => args[0].acos(),
            "atan" => args[0].atan(),
            "pow" => args[0].powf(args[1]),
            "abs" => args[0].abs(),
            "min" => args[0].min(args[1]),
            "max" => args[0].max(args[1]),
            "floor" => args[0].floor(),
            "pnjlim" | "fetlim" | "limvds" => args[0],
            _ => 0.0,
        };
        Ok(v)
    }

    // ── net / cell accessors ──────────────────────────────────────────

    fn read_across(&self, a: NetId, b: NetId) -> Result<f64, InterpError> {
        let va = self.node_across(a)?;
        let vb = self.node_across(b)?;
        Ok(va - vb)
    }

    fn read_through(&self, a: NetId, b: NetId) -> Result<f64, InterpError> {
        let ia = self.node_through(a)?;
        let ib = self.node_through(b)?;
        Ok(ia - ib)
    }

    fn node_across(&self, net: NetId) -> Result<f64, InterpError> {
        match self.state.nets.get(&net) {
            Some(NetValue::Node { across, .. }) => Ok(*across),
            _ => Err(InterpError::NetKindMismatch { net }),
        }
    }

    fn node_through(&self, net: NetId) -> Result<f64, InterpError> {
        match self.state.nets.get(&net) {
            Some(NetValue::Node { through, .. }) => Ok(*through),
            _ => Err(InterpError::NetKindMismatch { net }),
        }
    }

    fn value_as_real(&self, v: ValueId) -> Result<f64, InterpError> {
        match self.state.values.get(&v) {
            Some(CellValue::Real(x)) => Ok(*x),
            Some(_) => Err(InterpError::UnsupportedOp { kind: "non-real where real required" }),
            None => {
                // Maybe it's a param cell — params appear as values. For
                // T13 we don't have a params-as-values mapping; treat
                // uncomputed as 0.0 with a loud fallback.
                Err(InterpError::UncomputedValue { value: v })
            }
        }
    }

    fn value_as_u128(&self, v: ValueId) -> Result<u128, InterpError> {
        match self.state.values.get(&v) {
            Some(CellValue::Bits(x)) => Ok(*x),
            Some(CellValue::Real(x)) => Ok(*x as u128),
            Some(_) => Err(InterpError::UnsupportedOp { kind: "non-bits where bits required" }),
            None => Err(InterpError::UncomputedValue { value: v }),
        }
    }

    fn read_cell_scalar(&self, cell: CellId) -> Result<CellValue, InterpError> {
        self.state
            .cells
            .get(&cell)
            .cloned()
            .ok_or(InterpError::UninitializedCell { cell })
    }

    fn read_cell_array(&self, cell: CellId, addr: u128) -> Result<CellValue, InterpError> {
        let v = self
            .state
            .cells
            .get(&cell)
            .ok_or(InterpError::UninitializedCell { cell })?;
        match v {
            CellValue::Array(arr) => {
                let i = addr as usize;
                let _ = addr;
                arr.get(i)
                    .cloned()
                    .ok_or_else(|| InterpError::AddressOutOfRange { cell, addr: i as u64, len: arr.len() as u32 })
            }
            _ => Ok(v.clone()),
        }
    }

    fn write_cell_array(&mut self, cell: CellId, addr: u128, value: CellValue) -> Result<(), InterpError> {
        let v = self
            .state
            .cells
            .get_mut(&cell)
            .ok_or(InterpError::UninitializedCell { cell })?;
        match v {
            CellValue::Array(arr) => {
                let i = addr as usize;
                let _ = addr;
                if i >= arr.len() {
                    return Err(InterpError::AddressOutOfRange {
                        cell,
                        addr: i as u64,
                        len: arr.len() as u32,
                    });
                }
                arr[i] = value;
                Ok(())
            }
            _ => {
                *v = value;
                Ok(())
            }
        }
    }
}

// ── small helpers ─────────────────────────────────────────────────────────

fn cell_as_real(v: &CellValue) -> Option<f64> {
    match v {
        CellValue::Real(x) => Some(*x),
        _ => None,
    }
}

fn cell_as_u128(v: &CellValue) -> Option<u128> {
    match v {
        CellValue::Bits(x) => Some(*x),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Inline unit tests. The integration tests with hand-computed
    //! expected values live in `tests/interp.rs` (T13's integration
    //! target per the matrix).
    use super::*;
    use crate::core::{Block, Domain, Net, Trigger};

    /// An interpreter over an empty module has zero cells, zero values,
    /// and time = 0.
    #[test]
    fn empty_module_interpreter_is_empty() {
        let m = Module::new();
        let interp = Interpreter::new(&m);
        assert_eq!(interp.time(), 0.0);
        assert!(interp.cell_value(CellId(0)).is_none());
    }

    /// A cell's init round-trips through State::for_module.
    #[test]
    fn cell_init_round_trips() {
        let mut m = Module::new();
        let init = m.push_const(Const::Real(7.5));
        let writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
        let cell = m.push_cell(crate::core::Cell::new(Ty::Real, init, writer));
        let interp = Interpreter::new(&m);
        assert_eq!(interp.cell_value(cell), Some(&CellValue::Real(7.5)));
    }

    /// A bits init round-trips.
    #[test]
    fn bits_cell_init_round_trips() {
        let mut m = Module::new();
        let init = m.push_const(Const::Bits(42));
        let writer = m.push_block(Block::new(Domain::Digital, Trigger::Always));
        let cell = m.push_cell(crate::core::Cell::new(Ty::Bits(8), init, writer));
        let interp = Interpreter::new(&m);
        assert_eq!(interp.cell_value(cell), Some(&CellValue::Bits(42)));
    }

    /// An Initial trigger fires exactly once.
    #[test]
    fn initial_trigger_fires_once() {
        let mut m = Module::new();
        let _ = m.push_block(Block::new(Domain::Digital, Trigger::Initial));
        let mut interp = Interpreter::new(&m);
        let s1 = interp.step().unwrap();
        assert_eq!(s1.fired_blocks, vec![BlockId(0)]);
        let s2 = interp.step().unwrap();
        assert!(s2.fired_blocks.is_empty(), "Initial does not fire twice");
    }

    /// An Always trigger fires every step.
    #[test]
    fn always_trigger_fires_every_step() {
        let mut m = Module::new();
        let _ = m.push_block(Block::new(Domain::Analog, Trigger::Always));
        let mut interp = Interpreter::new(&m);
        assert_eq!(interp.step().unwrap().fired_blocks, vec![BlockId(0)]);
        assert_eq!(interp.step().unwrap().fired_blocks, vec![BlockId(0)]);
        assert_eq!(interp.step().unwrap().fired_blocks, vec![BlockId(0)]);
    }

    /// A Timer trigger fires at period boundaries — period 1.0 fires at
    /// t=1, 2, 3 but not at t=0.5.
    #[test]
    fn timer_fires_at_period_boundaries() {
        let mut m = Module::new();
        let period = m.push_value(Ty::Real);
        let phase = m.push_value(Ty::Real);
        let block = m.push_block(Block::new(
            Domain::Analog,
            Trigger::Timer { period, phase },
        ));
        let mut interp = Interpreter::new(&m);
        interp.seed_value(period, CellValue::Real(1.0));
        interp.seed_value(phase, CellValue::Real(0.0));
        // t=0: timer arms next_fire=1.0, does not fire.
        let _ = interp.step().unwrap();
        // t=0.5: not yet.
        interp.step_to(0.5).unwrap();
        // t=1.0: fires.
        let s = interp.step_to(1.0).unwrap();
        assert!(s.fired_blocks.contains(&block), "timer fires at t=1.0");
        // t=1.5: not yet.
        let s = interp.step_to(1.5).unwrap();
        assert!(!s.fired_blocks.contains(&block), "timer does not fire at t=1.5");
        // t=2.0: fires.
        let s = interp.step_to(2.0).unwrap();
        assert!(s.fired_blocks.contains(&block), "timer fires at t=2.0");
    }

    /// Edge trigger fires on a 0→1 transition (Rising).
    #[test]
    fn edge_rising_fires_on_0_to_1() {
        let mut m = Module::new();
        let v = m.push_value(Ty::Bits(1));
        let sr = crate::core::SigRef(vec![crate::core::Chunk::Slice { value: v, lo: 0, hi: 1 }]);
        let block = m.push_block(Block::new(Domain::Digital, Trigger::Edge(sr, EdgeKind::Rising)));
        let sig = m.push_net(Net::signal(Ty::Bits(1)));
        let mut interp = Interpreter::new(&m);
        // Set signal low, step (arms the edge detector).
        interp.set_signal(sig, 0);
        interp.seed_value(v, CellValue::Bits(0));
        let _ = interp.step().unwrap();
        // Set signal high, step (fires).
        interp.set_signal(sig, 1);
        interp.seed_value(v, CellValue::Bits(1));
        let s = interp.step().unwrap();
        assert!(s.fired_blocks.contains(&block), "rising edge fires");
        // Hold high, step (does not fire again).
        let s = interp.step().unwrap();
        assert!(!s.fired_blocks.contains(&block), "no second rising fire on hold");
    }

    /// Cross fires on a sign change in the right direction.
    #[test]
    fn cross_fires_on_rising_sign_change() {
        let mut m = Module::new();
        let v = m.push_value(Ty::Real);
        let block = m.push_block(Block::new(Domain::Analog, Trigger::Cross(v, CrossDir::Rising)));
        let mut interp = Interpreter::new(&m);
        // First sample negative: arms.
        interp.seed_value(v, CellValue::Real(-1.0));
        let _ = interp.step().unwrap();
        // Positive: fires.
        interp.seed_value(v, CellValue::Real(1.0));
        let s = interp.step().unwrap();
        assert!(s.fired_blocks.contains(&block), "rising cross fires");
    }

    /// Unseeded cross never fires (the seeding asymmetry).
    #[test]
    fn unseeded_cross_never_fires() {
        let mut m = Module::new();
        let v = m.push_value(Ty::Real);
        let block = m.push_block(Block::new(Domain::Analog, Trigger::Cross(v, CrossDir::Either)));
        let mut interp = Interpreter::new(&m);
        interp.seed_value(v, CellValue::Real(1.0));
        let s = interp.step().unwrap();
        assert!(!s.fired_blocks.contains(&block), "unseeded cross never fires on first sample");
    }

    /// A Pure::Add over two reals computes their sum.
    #[test]
    fn pure_add_computes_sum() {
        let mut m = Module::new();
        let a = m.push_value(Ty::Real);
        let b = m.push_value(Ty::Real);
        let out = m.push_value(Ty::Real);
        let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
        m.block_at_mut(block).push(Op::Pure { op: PureOp::Add, args: vec![a, b], out });
        let mut interp = Interpreter::new(&m);
        interp.seed_value(a, CellValue::Real(2.0));
        interp.seed_value(b, CellValue::Real(3.5));
        let _ = interp.step().unwrap();
        assert_eq!(interp.value(out), Some(&CellValue::Real(5.5)));
    }
}
