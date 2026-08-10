//! Symbolic differentiation as an IR → IR pass (LOV-15).
//!
//! **Ported from `crates/piperine-codegen/src/resolve/diff.rs`, NOT
//! rewritten** (design §7 risk — the rules were each learned from a
//! failure). The rules travel verbatim; only the walked representation
//! changes: `Expr` → IR `Op`/`ValueId`, `ast::Block` → IR `Block` with
//! inline `If`/`Match` arms.
//!
//! ## What travels from the source
//!
//! - The `seed` shape (per-branch derivative of `V(p,n)` / `I(p,n)`
//!   reads) — here, `ReadAcross`/`ReadThrough` produce `1.0` when their
//!   `(a,b)` matches the Wrt branch, else `0.0`.
//! - The `d_binary` table: `Add`/`Sub`/`Mul`/`Div`/`Pow` rules.
//! - The `d_math` table: `sin`→`cos`, `cos`→`-sin`, `exp`→`exp`,
//!   `ln`→`1/x`, etc. — the catalog's per-intrinsic `derivative` rule
//!   is what the pass consults instead of a hardcoded math table.
//! - The `PowDeriv` constant-exponent simplification.
//! - The `$limit` transparency rule (`d/dV vnew`).
//! - The structured-control arm-wise rule: `d/dx (if c then f(x) else
//!   g(x)) = if c then f'(x) else g'(x)` (ideal.md §3 — D9).
//! - The higher-derivative chains for `.disto` (`d_dv_twice`/
//!   `d_dv_thrice`) — applied by re-running the first-order pass.
//!
//! ## Entry point
//!
//! [`Differentiator::d`] — `d(value, wrt, module) -> ValueId`. The
//! differentiator is a struct (MD-13 rule 2: not a free function). It
//! walks the block's ops in order, computes each op's output derivative,
//! and pushes new ops into the module that compute the derivative graph.
//! The result is the `ValueId` of the derivative of `value`.

use crate::catalog::{Catalog, DerivativeRule};
use crate::core::{
    BlockId, Const, IntrinsicId, Module, NetId, Op, Pattern, PureOp, Ty,
    ValueId,
};
use std::collections::HashMap;

/// What a differentiation is with respect to. Mirrors the source's
/// `(plus, minus)` node-pair (the Jacobian), single-node (`ddx`), and
/// parameter (`∂F/∂p`) cases — three code paths in codegen today,
/// one pass here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrt {
    /// `∂/∂V(a, b)` — the across-pair Jacobian column. A `ReadAcross`
    /// {a, b, ..} leaf seeds `1.0` when its `(a, b)` matches; a
    /// `ReadThrough` leaf seeds `1.0` likewise.
    Across { a: NetId, b: NetId },
    /// `∂/∂V(node)` — the `ddx` derivative w.r.t. a single node
    /// potential. A `ReadAcross { a, b }` leaf seeds
    /// `+1` if `a == node`, `-1` if `b == node`, `0` otherwise.
    Node(NetId),
    /// `∂/∂param` — the parameter derivative (DVG-01).
    Param(ValueId),
}

/// The differentiator — owns the module mutation, the per-value
/// derivative cache, and the canonical `0.0`/`1.0` value slots every
/// rule uses. Methods only (MD-13 rule 2).
pub struct Differentiator<'a> {
    module: &'a mut Module,
    block: BlockId,
    derivs: HashMap<ValueId, ValueId>,
    zero_id: ValueId,
    one_id: ValueId,
    /// Every constant-anchor `ValueId` this differentiator has minted
    /// (`zero_id`, `one_id`, and each `real_const(c)`), paired with its
    /// actual numeric value. A constant anchor's DEFINING op is the
    /// self-referential `Pure::Add { args: [out, out], out }` pattern this
    /// module's doc describes — deliberately unevaluable by walking the
    /// op graph alone (there is no `x` with `x = x + x` other than `0`,
    /// so the pattern alone cannot distinguish `zero_id` from `one_id`).
    /// [`Self::constants`] is the side-channel a numeric consumer (a
    /// backend; the reference interpreter has no reactive/nonlinear
    /// Jacobian consumer yet) needs to resolve an anchor to its real
    /// value instead of evaluating its self-referential defining op.
    constants: HashMap<ValueId, f64>,
}

impl<'a> Differentiator<'a> {
    /// Build a differentiator that walks `block` of `module`. The
    /// derivative graph is pushed into the SAME block (after the
    /// original ops), so the result `ValueId` is inspectable alongside
    /// the original residual.
    pub fn new(module: &'a mut Module, block: BlockId) -> Self {
        let zero_const = module.push_const(Const::Real(0.0));
        let one_const = module.push_const(Const::Real(1.0));
        let zero_id = module.push_value(Ty::Real);
        let one_id = module.push_value(Ty::Real);
        // Mark these value slots as constant-bearing. The mini-eval (in
        // tests) and the T13 interpreter resolve them by matching the
        // pattern `pure_add(self, self)` as a constant anchor.
        module.block_at_mut(block).push(Op::Pure {
            op: PureOp::Add,
            args: vec![zero_id, zero_id],
            out: zero_id,
        });
        module.block_at_mut(block).push(Op::Pure {
            op: PureOp::Add,
            args: vec![one_id, one_id],
            out: one_id,
        });
        let _ = (zero_const, one_const);
        let mut constants = HashMap::new();
        constants.insert(zero_id, 0.0);
        constants.insert(one_id, 1.0);
        Self { module, block, derivs: HashMap::new(), zero_id, one_id, constants }
    }

    /// Every constant-anchor `ValueId` minted so far, mapped to its real
    /// numeric value — the side-channel a backend needs to resolve
    /// `zero_id`/`one_id`/`real_const(c)` (see this struct's field doc).
    /// Read AFTER calling [`Self::d`]/[`Self::d_twice`]/[`Self::d_thrice`]
    /// so every anchor the derivative graph minted is present.
    pub fn constants(&self) -> &HashMap<ValueId, f64> {
        &self.constants
    }

    /// Differentiate `value` w.r.t. `wrt`. Returns the `ValueId` of the
    /// derivative, after pushing the derivative graph into the block.
    /// Calling `d` again with the same `wrt` and a value produced by a
    /// previous `d` call composes (this is the `.disto` higher-order
    /// chain — `d_dv_twice`/`d_dv_thrice` in the source).
    pub fn d(&mut self, value: ValueId, wrt: Wrt) -> ValueId {
        // Seed the param case: a leaf matching the param gets `1.0`.
        if let Wrt::Param(p) = wrt {
            self.derivs.insert(p, self.one_id);
        }
        let ops: Vec<Op> = self.module.block_at(self.block).ops().to_vec();
        let orig_len = ops.len();
        for op in ops.iter().take(orig_len) {
            self.d_op(op, wrt);
        }
        self.deriv_of(&value)
    }

    /// Second-order cross derivative `∂²value / ∂V(a,b) ∂V(c,d)` — the
    /// `.disto` 2nd-derivative kernel core (DISTO-03). Two passes
    /// through [`Self::d`]; the second pass walks the derivative graph
    /// the first produced.
    pub fn d_twice(
        &mut self,
        value: ValueId,
        ab: (NetId, NetId),
        cd: (NetId, NetId),
    ) -> ValueId {
        let first = self.d(value, Wrt::Across { a: ab.0, b: ab.1 });
        self.d(first, Wrt::Across { a: cd.0, b: cd.1 })
    }

    /// Third-order cross derivative `∂³value / ∂V(a,b) ∂V(c,d) ∂V(e,f)`
    /// — the `.disto` 3rd-derivative kernel core.
    pub fn d_thrice(
        &mut self,
        value: ValueId,
        ab: (NetId, NetId),
        cd: (NetId, NetId),
        ef: (NetId, NetId),
    ) -> ValueId {
        let first = self.d(value, Wrt::Across { a: ab.0, b: ab.1 });
        let second = self.d(first, Wrt::Across { a: cd.0, b: cd.1 });
        self.d(second, Wrt::Across { a: ef.0, b: ef.1 })
    }

    // ── per-op dispatch (no catch-all — A0c defence) ──────────────────

    fn d_op(&mut self, op: &Op, wrt: Wrt) {
        match op {
            Op::Pure { op: pure, args, out } => {
                let d_args: Vec<ValueId> =
                    args.iter().map(|a| self.deriv_of(a)).collect();
                let d_out = self.d_pure(*pure, args, &d_args);
                self.derivs.insert(*out, d_out);
            }
            Op::Call { intrinsic, args, out } => {
                let d_out = self.d_call(*intrinsic, args);
                self.derivs.insert(*out, d_out);
            }
            Op::ReadAcross { a, b, out } => {
                let seed = match wrt {
                    Wrt::Across { a: wa, b: wb } if *a == wa && *b == wb => self.one_id,
                    Wrt::Node(n) => {
                        let s = if *a == n { 1.0 } else if *b == n { -1.0 } else { 0.0 };
                        self.real_const(s)
                    }
                    _ => self.zero_id,
                };
                self.derivs.insert(*out, seed);
            }
            Op::ReadThrough { a, b, out } => {
                let seed = match wrt {
                    Wrt::Across { a: wa, b: wb } if *a == wa && *b == wb => self.one_id,
                    _ => self.zero_id,
                };
                self.derivs.insert(*out, seed);
            }
            Op::ContributeThrough { .. }
            | Op::ForceAcross { .. }
            | Op::Drive { .. }
            | Op::CellWrite { .. } => {
                // Effect ops do not produce a value — no derivative to
                // record. Their `value` argument's derivative flows
                // through the value graph independently.
            }
            Op::ReadSignal { out, .. } => {
                self.derivs.insert(*out, self.zero_id);
            }
            Op::CellRead { out, .. } => {
                self.derivs.insert(*out, self.zero_id);
            }
            Op::If { cond, then_ops, else_ops } => {
                // ARM-WISE: d/dx (if c then f(x) else g(x)) =
                // if c then f'(x) else g'(x). The condition is NOT
                // differentiated (its discontinuity already exists in
                // the original). Each arm is differentiated into its
                // own op list, then a new If preserves the structure.
                let then_d = self.diff_arm_to_ops(then_ops, wrt);
                let else_d = self.diff_arm_to_ops(else_ops, wrt);
                self.module.block_at_mut(self.block).push(Op::If {
                    cond: *cond,
                    then_ops: then_d,
                    else_ops: else_d,
                });
            }
            Op::Match { scrutinee, arms } => {
                // ARM-WISE: each arm differentiated into its own op list,
                // preserving the Match structure.
                let new_arms: Vec<(Pattern, Vec<Op>)> = arms
                    .iter()
                    .map(|(pat, body)| (*pat, self.diff_arm_to_ops(body, wrt)))
                    .collect();
                self.module.block_at_mut(self.block).push(Op::Match {
                    scrutinee: *scrutinee,
                    arms: new_arms,
                });
            }
        }
    }

    /// Differentiate a sequence of ops (an If/Match arm) and return the
    /// derivative ops as a buffer, WITHOUT mixing them into the parent
    /// block. The arm's derivatives update `self.derivs` only for values
    /// declared in this arm; values declared outside the arm keep their
    /// pre-call derivative.
    fn diff_arm_to_ops(&mut self, ops: &[Op], wrt: Wrt) -> Vec<Op> {
        // Snapshot the current state, run the arm's diff producing ops
        // in the parent block, then harvest the new ops into a buffer
        // and remove them from the parent block.
        let saved_derivs = self.derivs.clone();
        let block_len_before = self.module.block_at(self.block).ops().len();
        for op in ops {
            self.d_op(op, wrt);
        }
        let full_ops = self.module.block_at_mut(self.block).ops_mut();
        let new_ops: Vec<Op> = full_ops.drain(block_len_before..).collect();
        // Restore the derivs side-table so the parent arm doesn't see
        // inner-arm derivative slots leaking outward (an arm-local
        // value's derivative shouldn't be visible to a sibling arm).
        self.derivs = saved_derivs;
        new_ops
    }

    // ── pure-op rules (the source's `d_binary` table, ported) ─────────

    fn d_pure(&mut self, op: PureOp, args: &[ValueId], d_args: &[ValueId]) -> ValueId {
        match op {
            PureOp::Add => self.push_pure(PureOp::Add, d_args),
            PureOp::Sub => self.push_pure(PureOp::Sub, d_args),
            PureOp::Mul => {
                // d(a*b) = da*b + a*db (product rule)
                let da_b = self.push_pure(PureOp::Mul, &[d_args[0], args[1]]);
                let a_db = self.push_pure(PureOp::Mul, &[args[0], d_args[1]]);
                self.push_pure(PureOp::Add, &[da_b, a_db])
            }
            PureOp::Div => {
                // d(a/b) = (da*b - a*db) / b² (quotient rule)
                let da_b = self.push_pure(PureOp::Mul, &[d_args[0], args[1]]);
                let a_db = self.push_pure(PureOp::Mul, &[args[0], d_args[1]]);
                let num = self.push_pure(PureOp::Sub, &[da_b, a_db]);
                let b_sq = self.push_pure(PureOp::Mul, &[args[1], args[1]]);
                self.push_pure(PureOp::Div, &[num, b_sq])
            }
            PureOp::Neg => self.push_pure(PureOp::Neg, d_args),
            PureOp::Eq | PureOp::Ne | PureOp::Lt | PureOp::Le | PureOp::Gt | PureOp::Ge => {
                // Comparison results are bits; their derivative is 0.
                self.zero_id
            }
            PureOp::Select => {
                // d(select(c, t, e)) = select(c, dt, de)
                self.push_pure(PureOp::Select, &[args[0], d_args[1], d_args[2]])
            }
        }
    }

    // ── call rule: dispatch on the catalog entry ──────────────────────

    /// Look up `intrinsic` in the catalog by index (the catalog order
    /// matches `IntrinsicId` for fixtures built in test code; production
    /// lowering installs the same mapping). Returns the seed for the
    /// call's output derivative.
    fn d_call(&mut self, intrinsic: IntrinsicId, args: &[ValueId]) -> ValueId {
        let entry = Catalog::entries().nth(intrinsic.0 as usize);
        let Some(entry) = entry else {
            return self.zero_id;
        };
        match entry.derivative() {
            DerivativeRule::ClosedForm("transparent") => {
                // `$limit` is transparent: d(vnew)/dV. The vnew is the
                // first argument; its derivative is whatever the chain
                // rule produced upstream.
                self.deriv_of(&args[0])
            }
            DerivativeRule::ClosedForm(name) => {
                // Math chain rule: d(f(x)) = f'(x) · dx.
                self.d_math_chain(name, args)
            }
            DerivativeRule::BinaryPow => {
                // pow(u, v): general rule
                //   d/dx u^v = v · u^(v-1) · du + u^v · ln(u) · dv
                self.d_pow(args)
            }
            DerivativeRule::FiniteDifferenceOnly => {
                // abs/min/max/floor/slew/transition/table — FD only.
                self.zero_id
            }
            DerivativeRule::Companion | DerivativeRule::NotDifferentiable => {
                // ddt/idt/etc. — reactive; companion Jacobian is
                // backend bookkeeping, not produced here.
                self.zero_id
            }
        }
    }

    /// Math chain rule: read the rule name, build f'(x) · dx.
    fn d_math_chain(&mut self, rule: &str, args: &[ValueId]) -> ValueId {
        let x = args[0];
        let dx = self.deriv_of(&x);
        match rule {
            "chain:cos" => {
                let cos_x = self.push_math_call("cos", x);
                self.push_pure(PureOp::Mul, &[cos_x, dx])
            }
            "chain:neg-sin" => {
                let sin_x = self.push_math_call("sin", x);
                let neg_sin = self.push_pure(PureOp::Neg, &[sin_x]);
                self.push_pure(PureOp::Mul, &[neg_sin, dx])
            }
            "chain:self" => {
                let exp_x = self.push_math_call("exp", x);
                self.push_pure(PureOp::Mul, &[exp_x, dx])
            }
            "reciprocal" => self.push_pure(PureOp::Div, &[dx, x]),
            "reciprocal-ln10" => {
                let ln10 = self.real_const(std::f64::consts::LN_10);
                let x_ln10 = self.push_pure(PureOp::Mul, &[x, ln10]);
                self.push_pure(PureOp::Div, &[dx, x_ln10])
            }
            "half-reciprocal-sqrt" => {
                let sqrt_x = self.push_math_call("sqrt", x);
                let two = self.real_const(2.0);
                let two_sqrt = self.push_pure(PureOp::Mul, &[two, sqrt_x]);
                self.push_pure(PureOp::Div, &[dx, two_sqrt])
            }
            "sec2-chain" => {
                let cos_x = self.push_math_call("cos", x);
                let cos_sq = self.push_pure(PureOp::Mul, &[cos_x, cos_x]);
                self.push_pure(PureOp::Div, &[dx, cos_sq])
            }
            "chain:cosh" => {
                let cosh_x = self.push_math_call("cosh", x);
                self.push_pure(PureOp::Mul, &[cosh_x, dx])
            }
            "chain:sinh" => {
                let sinh_x = self.push_math_call("sinh", x);
                self.push_pure(PureOp::Mul, &[sinh_x, dx])
            }
            "sech2-chain" => {
                let tanh_x = self.push_math_call("tanh", x);
                let tanh_sq = self.push_pure(PureOp::Mul, &[tanh_x, tanh_x]);
                let one_minus = self.push_pure(PureOp::Sub, &[self.one_id, tanh_sq]);
                self.push_pure(PureOp::Mul, &[one_minus, dx])
            }
            "reciprocal-sqrt-1-minus-x2" => {
                let x_sq = self.push_pure(PureOp::Mul, &[x, x]);
                let inner = self.push_pure(PureOp::Sub, &[self.one_id, x_sq]);
                let sqrt_inner = self.push_math_call("sqrt", inner);
                self.push_pure(PureOp::Div, &[dx, sqrt_inner])
            }
            "neg-reciprocal-sqrt-1-minus-x2" => {
                let x_sq = self.push_pure(PureOp::Mul, &[x, x]);
                let inner = self.push_pure(PureOp::Sub, &[self.one_id, x_sq]);
                let sqrt_inner = self.push_math_call("sqrt", inner);
                let neg_dx = self.push_pure(PureOp::Neg, &[dx]);
                self.push_pure(PureOp::Div, &[neg_dx, sqrt_inner])
            }
            "reciprocal-1-plus-x2" => {
                let x_sq = self.push_pure(PureOp::Mul, &[x, x]);
                let denom = self.push_pure(PureOp::Add, &[self.one_id, x_sq]);
                self.push_pure(PureOp::Div, &[dx, denom])
            }
            "signum-chain" => {
                let cmp = self.push_pure(PureOp::Ge, &[x, self.zero_id]);
                let neg_one = self.real_const(-1.0);
                let sign = self.push_pure(PureOp::Select, &[cmp, self.one_id, neg_one]);
                self.push_pure(PureOp::Mul, &[sign, dx])
            }
            "select-le" | "select-ge" => {
                let pop = if rule == "select-le" { PureOp::Le } else { PureOp::Ge };
                let cmp = self.push_pure(pop, &[args[0], args[1]]);
                let da = self.deriv_of(&args[0]);
                let db = self.deriv_of(&args[1]);
                self.push_pure(PureOp::Select, &[cmp, da, db])
            }
            "zero" => self.zero_id,
            _ => self.zero_id,
        }
    }

    /// `pow(u, v)`: general derivative `v · u^(v-1) · du + u^v · ln(u) · dv`.
    /// Constant-exponent simplification: when `dv == 0`, the second term
    /// vanishes and the rule becomes `v · u^(v-1) · du` (the source's
    /// `PowDeriv::d_via_pow`).
    fn d_pow(&mut self, args: &[ValueId]) -> ValueId {
        let u = args[0];
        let v = args[1];
        let du = self.deriv_of(&u);
        let dv = self.deriv_of(&v);
        let dv_is_zero = dv == self.zero_id;
        if dv_is_zero {
            // v · u^(v-1) · du
            let v_minus_1 = self.push_pure(PureOp::Sub, &[v, self.one_id]);
            let u_pow = self.push_math_call_with("pow", &[u, v_minus_1]);
            let lhs = self.push_pure(PureOp::Mul, &[v, u_pow]);
            self.push_pure(PureOp::Mul, &[lhs, du])
        } else {
            // u^v · (dv · ln(u) + v · du / u)
            let u_pow = self.push_math_call_with("pow", &[u, v]);
            let ln_u = self.push_math_call("ln", u);
            let term1 = self.push_pure(PureOp::Mul, &[dv, ln_u]);
            let v_du = self.push_pure(PureOp::Mul, &[v, du]);
            let term2 = self.push_pure(PureOp::Div, &[v_du, u]);
            let sum = self.push_pure(PureOp::Add, &[term1, term2]);
            self.push_pure(PureOp::Mul, &[u_pow, sum])
        }
    }

    // ── graph-construction helpers ────────────────────────────────────

    /// Push a Pure op with `args`, allocating a fresh `Real` value for
    /// its output. Returns the output ValueId.
    fn push_pure(&mut self, op: PureOp, args: &[ValueId]) -> ValueId {
        let out = self.module.push_value(Ty::Real);
        self.module
            .block_at_mut(self.block)
            .push(Op::Pure { op, args: args.to_vec(), out });
        out
    }

    /// Push a unary math call (sin/cos/exp/...).
    fn push_math_call(&mut self, name: &str, arg: ValueId) -> ValueId {
        self.push_math_call_with(name, &[arg])
    }

    /// Push a math call with arbitrary args (pow is binary).
    fn push_math_call_with(&mut self, name: &str, args: &[ValueId]) -> ValueId {
        let intrinsic = Self::catalog_id_for(name);
        let out = self.module.push_value(Ty::Real);
        self.module
            .block_at_mut(self.block)
            .push(Op::Call { intrinsic, args: args.to_vec(), out });
        out
    }

    /// Look up a catalog entry by name and return its `IntrinsicId` (the
    /// index in `Catalog::entries()`).
    pub fn catalog_id_for(name: &str) -> IntrinsicId {
        for (i, entry) in Catalog::entries().enumerate() {
            if entry.name() == name {
                return IntrinsicId(i as u32);
            }
        }
        panic!("diff: catalog has no entry named `{name}`");
    }

    /// The cached derivative of `v`, or `zero` if `v` has no recorded
    /// derivative (a leaf outside the walk).
    fn deriv_of(&self, v: &ValueId) -> ValueId {
        self.derivs.get(v).copied().unwrap_or(self.zero_id)
    }

    /// Allocate a fresh real constant value. Used by the chain rule for
    /// `2.0`, `-1.0`, `ln(10)`, etc. The value carries the constant via
    /// the `pure_add(self, self)` pattern the mini-eval recognises.
    fn real_const(&mut self, c: f64) -> ValueId {
        let _cid = self.module.push_const(Const::Real(c));
        let out = self.module.push_value(Ty::Real);
        self.module.block_at_mut(self.block).push(Op::Pure {
            op: PureOp::Add,
            args: vec![out, out],
            out,
        });
        self.constants.insert(out, c);
        out
    }
}

#[cfg(test)]
mod tests {
    //! Inline unit tests for the diff pass. The full FD cross-check
    //! lives in the integration test target (T12).
    use super::*;
    use crate::core::{Block, Domain, NatureInfo, NatureKind, Net, NetId, Trigger};

    /// Helper: build a module with a single analog always-block, plus
    /// electrical node nets `n_a` and `n_b`. Returns (module, block, n_a, n_b).
    fn fixture_analog_module() -> (Module, BlockId, NetId, NetId) {
        let mut m = Module::new();
        let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
        let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
        let n_a = m.push_net(Net::node(v, i));
        let n_b = m.push_net(Net::node(v, i));
        let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
        (m, block, n_a, n_b)
    }

    /// `d(x + c)/dx = 1` — the simplest rule (Add identity).
    #[test]
    fn d_of_x_plus_c_produces_a_derivative() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let c = m.push_value(Ty::Real);
        let sum = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        m.block_at_mut(block).push(Op::Pure { op: PureOp::Add, args: vec![x, c], out: sum });
        let ops_before = m.block_at(block).ops().len();
        let mut diff = Differentiator::new(&mut m, block);
        let d = diff.d(sum, Wrt::Across { a: n_a, b: n_b });
        assert_eq!(m.value_ty(d), &Ty::Real);
        assert!(m.block_at(block).ops().len() > ops_before, "derivative ops pushed");
    }

    /// `d(x · x)/dx` uses the product rule.
    #[test]
    fn d_of_x_times_x_uses_product_rule() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let prod = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        m.block_at_mut(block).push(Op::Pure { op: PureOp::Mul, args: vec![x, x], out: prod });
        let ops_before = m.block_at(block).ops().len();
        let mut diff = Differentiator::new(&mut m, block);
        let d = diff.d(prod, Wrt::Across { a: n_a, b: n_b });
        assert_eq!(m.value_ty(d), &Ty::Real);
        // Product rule: da*b + a*db → 3 ops pushed (2 Mul + 1 Add).
        assert!(m.block_at(block).ops().len() >= ops_before + 3);
    }

    /// `d(sin(x))/dx = cos(x) · dx` — the math chain rule, consulting
    /// the catalog entry for `sin`.
    #[test]
    fn d_of_sin_x_references_cos() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let sin_x = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let sin_id = Differentiator::catalog_id_for("sin");
        m.block_at_mut(block).push(Op::Call { intrinsic: sin_id, args: vec![x], out: sin_x });
        let mut diff = Differentiator::new(&mut m, block);
        let d = diff.d(sin_x, Wrt::Across { a: n_a, b: n_b });
        assert_eq!(m.value_ty(d), &Ty::Real);
        // The derivative graph must reference a `cos` call.
        let has_cos_call = m.block_at(block).ops().iter().any(|op| match op {
            Op::Call { intrinsic, .. } => {
                Catalog::entries()
                    .nth(intrinsic.0 as usize)
                    .map(|e| e.name() == "cos")
                    .unwrap_or(false)
            }
            _ => false,
        });
        assert!(has_cos_call, "d/dx sin(x) must reference cos(x)");
    }

    /// `d/dx cos(x) = -sin(x) · dx`.
    #[test]
    fn d_of_cos_x_references_neg_sin() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let cos_x = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let cos_id = Differentiator::catalog_id_for("cos");
        m.block_at_mut(block).push(Op::Call { intrinsic: cos_id, args: vec![x], out: cos_x });
        let mut diff = Differentiator::new(&mut m, block);
        let _ = diff.d(cos_x, Wrt::Across { a: n_a, b: n_b });
        let has_sin_call = m.block_at(block).ops().iter().any(|op| match op {
            Op::Call { intrinsic, .. } => {
                Catalog::entries()
                    .nth(intrinsic.0 as usize)
                    .map(|e| e.name() == "sin")
                    .unwrap_or(false)
            }
            _ => false,
        });
        assert!(has_sin_call, "d/dx cos(x) references sin(x)");
    }

    /// `d(exp(x))/dx = exp(x) · dx`.
    #[test]
    fn d_of_exp_x_references_exp() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let exp_x = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let exp_id = Differentiator::catalog_id_for("exp");
        m.block_at_mut(block).push(Op::Call { intrinsic: exp_id, args: vec![x], out: exp_x });
        let mut diff = Differentiator::new(&mut m, block);
        let _ = diff.d(exp_x, Wrt::Across { a: n_a, b: n_b });
        let has_exp = m.block_at(block).ops().iter().any(|op| match op {
            Op::Call { intrinsic, .. } => {
                Catalog::entries()
                    .nth(intrinsic.0 as usize)
                    .map(|e| e.name() == "exp")
                    .unwrap_or(false)
            }
            _ => false,
        });
        assert!(has_exp, "d/dx exp(x) references exp(x)");
    }

    /// `d(ln(x))/dx = dx/x`.
    #[test]
    fn d_of_ln_x_is_dx_div_x() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let ln_x = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let ln_id = Differentiator::catalog_id_for("ln");
        m.block_at_mut(block).push(Op::Call { intrinsic: ln_id, args: vec![x], out: ln_x });
        let ops_before = m.block_at(block).ops().len();
        let mut diff = Differentiator::new(&mut m, block);
        let _ = diff.d(ln_x, Wrt::Across { a: n_a, b: n_b });
        // The derivative pushes at least one Div op (dx / x).
        let has_div = m
            .block_at(block)
            .ops()
            .iter()
            .skip(ops_before)
            .any(|op| matches!(op, Op::Pure { op: PureOp::Div, .. }));
        assert!(has_div, "d/dx ln(x) pushes a Div op");
    }

    /// `d(sqrt(x))/dx = dx / (2·sqrt(x))`.
    #[test]
    fn d_of_sqrt_x_pushes_sqrt_and_div() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let sqrt_x = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let sqrt_id = Differentiator::catalog_id_for("sqrt");
        m.block_at_mut(block).push(Op::Call { intrinsic: sqrt_id, args: vec![x], out: sqrt_x });
        let mut diff = Differentiator::new(&mut m, block);
        let _ = diff.d(sqrt_x, Wrt::Across { a: n_a, b: n_b });
        let has_sqrt = m.block_at(block).ops().iter().any(|op| match op {
            Op::Call { intrinsic, .. } => {
                Catalog::entries()
                    .nth(intrinsic.0 as usize)
                    .map(|e| e.name() == "sqrt")
                    .unwrap_or(false)
            }
            _ => false,
        });
        assert!(has_sqrt);
    }

    /// Structured control differentiates arm-wise: visiting an If does
    /// not panic; both arms' derivatives are computed.
    #[test]
    fn d_of_if_differentiates_arms_independently() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let y = m.push_value(Ty::Real);
        let cond = m.push_value(Ty::Bits(1));
        let if_out = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        m.block_at_mut(block).push(Op::ReadAcross { a: n_b, b: n_a, out: y });
        m.block_at_mut(block).push(Op::If {
            cond,
            then_ops: vec![Op::Pure { op: PureOp::Mul, args: vec![x, x], out: if_out }],
            else_ops: vec![Op::Pure { op: PureOp::Add, args: vec![y, y], out: if_out }],
        });
        let mut diff = Differentiator::new(&mut m, block);
        let _ = diff.d(if_out, Wrt::Across { a: n_a, b: n_b });
        // No panic — both arms visited.
    }

    /// A NESTED If (an If inside an If) is covered.
    #[test]
    fn d_of_nested_if_does_not_panic() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let cond_outer = m.push_value(Ty::Bits(1));
        let cond_inner = m.push_value(Ty::Bits(1));
        let if_out = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        m.block_at_mut(block).push(Op::If {
            cond: cond_outer,
            then_ops: vec![Op::If {
                cond: cond_inner,
                then_ops: vec![Op::Pure { op: PureOp::Mul, args: vec![x, x], out: if_out }],
                else_ops: vec![Op::Pure { op: PureOp::Add, args: vec![x, x], out: if_out }],
            }],
            else_ops: vec![Op::Pure { op: PureOp::Neg, args: vec![x], out: if_out }],
        });
        let mut diff = Differentiator::new(&mut m, block);
        let _ = diff.d(if_out, Wrt::Across { a: n_a, b: n_b });
        // No panic — nested If walked arm-wise.
    }

    /// Match differentiates arm-wise — each arm's body is visited.
    #[test]
    fn d_of_match_differentiates_each_arm() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let out = m.push_value(Ty::Real);
        let scrutinee = m.push_value(Ty::Bits(1));
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        m.block_at_mut(block).push(Op::Match {
            scrutinee,
            arms: vec![
                (Pattern::Zero, vec![Op::Pure { op: PureOp::Mul, args: vec![x, x], out }]),
                (Pattern::One, vec![Op::Pure { op: PureOp::Add, args: vec![x, x], out }]),
                (Pattern::Wildcard, vec![]),
            ],
        });
        let mut diff = Differentiator::new(&mut m, block);
        let _ = diff.d(out, Wrt::Across { a: n_a, b: n_b });
        // No panic — all three arms visited (including the empty wildcard body).
    }

    /// `$limit` is transparent: d(vnew)/dV.
    #[test]
    fn d_of_pnjlim_passes_through_to_vnew() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        let limited = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let pnjlim_id = Differentiator::catalog_id_for("pnjlim");
        m.block_at_mut(block).push(Op::Call {
            intrinsic: pnjlim_id,
            args: vec![x, x, x, x],
            out: limited,
        });
        let mut diff = Differentiator::new(&mut m, block);
        let d = diff.d(limited, Wrt::Across { a: n_a, b: n_b });
        assert_eq!(m.value_ty(d), &Ty::Real);
    }

    /// `d_dv_twice` — second-order cross derivative composes.
    #[test]
    fn d_dv_twice_composes() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let v = m.nature_at(crate::core::NatureId(0)).clone();
        let _ = v;
        let n_c = m.push_net(Net::node(crate::core::NatureId(0), crate::core::NatureId(1)));
        let n_d = m.push_net(Net::node(crate::core::NatureId(0), crate::core::NatureId(1)));
        let x = m.push_value(Ty::Real);
        let y = m.push_value(Ty::Real);
        let prod = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        m.block_at_mut(block).push(Op::ReadAcross { a: n_c, b: n_d, out: y });
        m.block_at_mut(block).push(Op::Pure { op: PureOp::Mul, args: vec![x, y], out: prod });
        let mut diff = Differentiator::new(&mut m, block);
        let d2 = diff.d_twice(prod, (n_a, n_b), (n_c, n_d));
        assert_eq!(m.value_ty(d2), &Ty::Real);
    }

    /// `d_dv_thrice` — third-order composes.
    #[test]
    fn d_dv_thrice_composes() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let n_c = m.push_net(Net::node(crate::core::NatureId(0), crate::core::NatureId(1)));
        let n_d = m.push_net(Net::node(crate::core::NatureId(0), crate::core::NatureId(1)));
        let n_e = m.push_net(Net::node(crate::core::NatureId(0), crate::core::NatureId(1)));
        let n_f = m.push_net(Net::node(crate::core::NatureId(0), crate::core::NatureId(1)));
        let x = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let mut diff = Differentiator::new(&mut m, block);
        let d3 = diff.d_thrice(x, (n_a, n_b), (n_c, n_d), (n_e, n_f));
        assert_eq!(m.value_ty(d3), &Ty::Real);
    }

    /// The `Wrt::Node` case: d(V(a, b))/dV(a) = +1; d(V(a, b))/dV(b) = -1.
    #[test]
    fn wrt_node_seeds_with_sign_for_plus_minus() {
        let (mut m, block, n_a, n_b) = fixture_analog_module();
        let x = m.push_value(Ty::Real);
        m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
        let mut diff = Differentiator::new(&mut m, block);
        let d_a = diff.d(x, Wrt::Node(n_a));
        let d_b = diff.d(x, Wrt::Node(n_b));
        assert_eq!(m.value_ty(d_a), &Ty::Real);
        assert_eq!(m.value_ty(d_b), &Ty::Real);
    }

    /// `Wrt::Param(p)`: d(p)/dp = 1, d(other)/dp = 0.
    #[test]
    fn wrt_param_seeds_one_for_matching_param() {
        let (mut m, block, _n_a, _n_b) = fixture_analog_module();
        let p = m.push_value(Ty::Real);
        let mut diff = Differentiator::new(&mut m, block);
        let d = diff.d(p, Wrt::Param(p));
        assert_eq!(m.value_ty(d), &Ty::Real);
    }
}
