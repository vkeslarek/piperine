//! The analog expression emitter: a flattened [`IrExpr`] to a scalar `f64`
//! Cranelift [`Value`].
//!
//! Emission is fallible — any construct without a faithful scalar lowering is
//! a named [`CodegenError`], never a silent `0.0`.

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, FuncRef, InstBuilder, MemFlags, Value};
use cranelift_frontend::FunctionBuilder;

use crate::ir::{IrBinOp, IrExpr, IrUnOp, SimQuery};

use super::{math, CodegenError, SimCtx};

/// Byte offsets of [`SimCtx`] fields, as read by JIT code.
struct SimField;

impl SimField {
    const TEMPERATURE: i32 = 0;
    const ABSTIME: i32 = 8;
    const MFACTOR: i32 = 16;
    const GMIN: i32 = 24;
    const STEP: i32 = 32;
    const TFINAL: i32 = 40;
    const PARAM_GIVEN_MASK: i32 = 48;
    const CURRENT_ANALYSIS: i32 = 56;
}

/// Emits flattened analog expressions inside one Cranelift function.
pub struct AnalogEmitter<'a, 'f> {
    pub builder: &'a mut FunctionBuilder<'f>,
    /// Precomputed `V(plus) − V(minus)` per branch pair.
    pub branch_voltages: &'a HashMap<(crate::ir::NodeId, crate::ir::NodeId), Value>,
    /// Parameter values, indexed by `ParamId`.
    pub params: &'a [Value],
    /// `*const f64` runtime-state values, indexed by `StateId`.
    pub state_ptr: Value,
    /// `*const f64` module-level persistent variable values, indexed by
    /// `VarId`. The D2A bridge: the analog body reads digital register
    /// values through this bank.
    pub vars_ptr: Value,
    /// `*const SimCtx`.
    pub sim_ptr: Value,
    /// Imported libm functions, keyed by canonical name.
    pub math: &'a HashMap<&'static str, FuncRef>,
    /// Unique `$limit` expressions, in slot order. The limited (previous-
    /// iteration) voltage `vold` for slot `i` lives in the state bank at
    /// `state_ptr[(limit_base + i) * 8]`.
    pub limits: &'a [IrExpr],
    /// State-bank offset (in slots) where the `$limit` vold slots begin — i.e.
    /// the number of module runtime-state slots.
    pub limit_base: usize,
    /// Common-subexpression cache. The analog residual is one straight-line
    /// block (all control flow is folded into branchless `select`s), so every
    /// emitted `Value` dominates all later uses and structurally-identical
    /// subexpressions can share one `Value`. Without this, the fully-inlined
    /// device bodies (each `var` and helper `fn` expanded into every use)
    /// explode into millions of instructions — past Cranelift's per-function
    /// size limit. Keyed by `(op-tag, child Value ids)`, which is exact because
    /// equal subtrees resolve to the same cached child `Value`.
    pub cse: HashMap<CseKey, Value>,
}

/// Structural key for common-subexpression elimination (see `AnalogEmitter::cse`).
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum CseKey {
    /// f64/const bit pattern.
    Const(u64),
    /// A load: `(bank tag, byte offset)`.
    Load(u8, i32),
    /// Unary op `(tag, child)`.
    Op1(u8, u32),
    /// Binary op / comparison `(tag, lhs, rhs)`.
    Op2(u8, u32, u32),
    /// Ternary (select) `(tag, a, b, c)`.
    Op3(u8, u32, u32, u32),
    /// Math builtin call `(name, args)`.
    Call(&'static str, Vec<u32>),
    /// Voltage-limited value for `$limit` slot `i` (depends only on the current
    /// volts and the stored vold, both fixed within one function eval).
    Limit(u32),
}

// Load-bank tags.
const BANK_STATE: u8 = 0;
const BANK_VARS: u8 = 1;
const BANK_SIM: u8 = 2;
// Op tags (namespaced across unary/binary/select/cmp).
const T_NEG: u8 = 0;
const T_SELECT: u8 = 1;
const T_NOT: u8 = 2;
const T_FCMP_BASE: u8 = 16; // + FloatCC as u8 (16..30)
const T_BIN_BASE: u8 = 40; // + IrBinOp index

/// Distinct CSE tag per binary op (offset past the fcmp/select tags).
fn bin_tag(op: IrBinOp) -> u8 {
    T_BIN_BASE
        + match op {
            IrBinOp::Add => 0,
            IrBinOp::Sub => 1,
            IrBinOp::Mul => 2,
            IrBinOp::Div => 3,
            IrBinOp::Rem => 4,
            IrBinOp::Eq => 5,
            IrBinOp::Ne => 6,
            IrBinOp::Lt => 7,
            IrBinOp::Le => 8,
            IrBinOp::Gt => 9,
            IrBinOp::Ge => 10,
            IrBinOp::And => 11,
            IrBinOp::Or => 12,
            IrBinOp::Pow => 13,
            IrBinOp::BitAnd => 14,
            IrBinOp::BitOr => 15,
            IrBinOp::BitXor => 16,
            IrBinOp::Shl => 17,
            IrBinOp::Shr => 18,
        }
}

impl AnalogEmitter<'_, '_> {
    pub fn emit(&mut self, expr: &IrExpr) -> Result<Value, CodegenError> {
        match expr {
            IrExpr::Real(v) => Ok(self.cse_const(*v)),
            IrExpr::Int(v) => Ok(self.cse_const(*v as f64)),
            IrExpr::Bool(b) => Ok(self.cse_const(f64::from(*b))),

            // Params and branch voltages are pre-loaded once into a shared
            // `Value`, so every reference already dedups — no CSE needed.
            IrExpr::Param(id) => self
                .params
                .get(id.0 as usize)
                .copied()
                .ok_or_else(|| CodegenError::Invalid(format!("param #{} out of range", id.0))),

            IrExpr::Branch { plus, minus, .. } => self
                .branch_voltages
                .get(&(*plus, *minus))
                .copied()
                .ok_or_else(|| {
                    CodegenError::Invalid(format!(
                        "branch V(#{}, #{}) missing from the precomputed set",
                        plus.0, minus.0
                    ))
                }),

            // Only runtime states (delay/slew) survive flattening; their
            // current value is serviced by the device into the state array.
            IrExpr::State(id) => Ok(self.cse_load(BANK_STATE, self.state_ptr, (id.0 * 8) as i32)),

            IrExpr::Sim(SimQuery::Limit { .. }) => self.emit_limit(expr),
            IrExpr::Sim(query) => self.emit_sim(query),

            IrExpr::Unary(op, x) => self.emit_unary(*op, x),
            IrExpr::Binary(op, a, b) => self.emit_binary(*op, a, b),

            IrExpr::Select(c, t, e) => {
                let cond = self.emit_truthy(c)?;
                let then_ = self.emit(t)?;
                let else_ = self.emit(e)?;
                Ok(self.cse_op3(T_SELECT, cond, then_, else_, |b| {
                    b.ins().select(cond, then_, else_)
                }))
            }

            IrExpr::MathCall(name, args) => self.emit_math_call(name, args),

            IrExpr::Call(id, _) => Err(CodegenError::Invalid(format!(
                "user call #{} survived inlining",
                id.0
            ))),
            IrExpr::Quad(_) => Err(CodegenError::unsupported("4-state literal in an analog expression")),
            IrExpr::Net(_) => Err(CodegenError::Invalid(
                "digital net read in an analog expression".into(),
            )),
            IrExpr::AcStim { .. } => Err(CodegenError::unsupported("ac_stim lowering")),
            IrExpr::Array(_) | IrExpr::Index(..) | IrExpr::Slice(..) => Err(
                CodegenError::unsupported("vector expression in an analog contribution"),
            ),
            IrExpr::Var(id) => Ok(self.cse_load(BANK_VARS, self.vars_ptr, (id.0 * 8) as i32)),
        }
    }

    /// Cached f64 constant.
    fn cse_const(&mut self, v: f64) -> Value {
        let key = CseKey::Const(v.to_bits());
        if let Some(&hit) = self.cse.get(&key) {
            return hit;
        }
        let val = self.builder.ins().f64const(v);
        self.cse.insert(key, val);
        val
    }

    /// Cached f64 load from a bank pointer.
    fn cse_load(&mut self, bank: u8, ptr: Value, offset: i32) -> Value {
        let key = CseKey::Load(bank, offset);
        if let Some(&hit) = self.cse.get(&key) {
            return hit;
        }
        let val = self
            .builder
            .ins()
            .load(types::F64, MemFlags::trusted(), ptr, offset);
        self.cse.insert(key, val);
        val
    }

    /// Cached unary op over an already-emitted operand.
    fn cse_op1(&mut self, tag: u8, x: Value, build: impl FnOnce(&mut FunctionBuilder) -> Value) -> Value {
        let key = CseKey::Op1(tag, x.as_u32());
        if let Some(&hit) = self.cse.get(&key) {
            return hit;
        }
        let val = build(self.builder);
        self.cse.insert(key, val);
        val
    }

    /// Cached binary op over already-emitted operands.
    fn cse_op2(&mut self, tag: u8, a: Value, b: Value, build: impl FnOnce(&mut FunctionBuilder) -> Value) -> Value {
        let key = CseKey::Op2(tag, a.as_u32(), b.as_u32());
        if let Some(&hit) = self.cse.get(&key) {
            return hit;
        }
        let val = build(self.builder);
        self.cse.insert(key, val);
        val
    }

    /// Cached ternary op over already-emitted operands.
    fn cse_op3(&mut self, tag: u8, a: Value, b: Value, c: Value, build: impl FnOnce(&mut FunctionBuilder) -> Value) -> Value {
        let key = CseKey::Op3(tag, a.as_u32(), b.as_u32(), c.as_u32());
        if let Some(&hit) = self.cse.get(&key) {
            return hit;
        }
        let val = build(self.builder);
        self.cse.insert(key, val);
        val
    }

    fn emit_sim(&mut self, query: &SimQuery) -> Result<Value, CodegenError> {
        match query {
            SimQuery::Temperature => Ok(self.load_sim_f64(SimField::TEMPERATURE)),
            SimQuery::Abstime => Ok(self.load_sim_f64(SimField::ABSTIME)),
            SimQuery::Mfactor => Ok(self.load_sim_f64(SimField::MFACTOR)),
            SimQuery::Vt(arg) => {
                let temperature = match arg {
                    Some(expr) => self.emit(expr)?,
                    None => self.load_sim_f64(SimField::TEMPERATURE),
                };
                let kb_over_q = self.cse_const(SimCtx::K_B_OVER_Q);
                Ok(self.cse_op2(bin_tag(IrBinOp::Mul), temperature, kb_over_q, |b| {
                    b.ins().fmul(temperature, kb_over_q)
                }))
            }
            SimQuery::Simparam { key, default } => match key.as_str() {
                "gmin" => Ok(self.load_sim_f64(SimField::GMIN)),
                "temperature" => Ok(self.load_sim_f64(SimField::TEMPERATURE)),
                // `step`/`tfinal` are only meaningful inside a transient; in
                // DC/AC the field is 0. Honor the caller's default there so a
                // model that reads `$simparam("tfinal", 1e-3)` never divides by
                // zero at the operating point.
                "step" => self.sim_field_or_default(SimField::STEP, default),
                "tfinal" => self.sim_field_or_default(SimField::TFINAL, default),
                _ => self.emit(default),
            },
            SimQuery::ParamGiven(id) => {
                let mask = self.builder.ins().load(
                    types::I64,
                    MemFlags::trusted(),
                    self.sim_ptr,
                    SimField::PARAM_GIVEN_MASK,
                );
                let shifted = self.builder.ins().ushr_imm(mask, i64::from(id.0));
                let bit = self.builder.ins().band_imm(shifted, 1);
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_set = self.builder.ins().icmp(IntCC::NotEqual, bit, zero);
                Ok(self.bool_to_f64(is_set))
            }
            SimQuery::Analysis(kind) => {
                let current = self.builder.ins().load(
                    types::I64,
                    MemFlags::trusted(),
                    self.sim_ptr,
                    SimField::CURRENT_ANALYSIS,
                );
                let target = self.builder.ins().iconst(types::I64, *kind as i64);
                let matches = self.builder.ins().icmp(IntCC::Equal, current, target);
                Ok(self.bool_to_f64(matches))
            }
            // Handled in `emit` (needs the full expression to locate its slot).
            SimQuery::Limit { .. } => unreachable!("Limit routed via emit_limit"),
            other @ (SimQuery::Position(_)
            | SimQuery::Angle
            | SimQuery::Random { .. }
            | SimQuery::PortConnected(_)) => Err(CodegenError::unsupported(format!(
                "simulator query {other:?}"
            ))),
        }
    }

    /// Emit a `$limit("pnjlim"/"fetlim", vnew, vseed, vte, vcrit)` call as its
    /// limited voltage. The previous-iteration limited value `vold` is read from
    /// the state bank (updated by the device each Newton iteration); at
    /// convergence `vlim == vnew`, so the residual and its symbolic Jacobian
    /// (which treats the limiter as transparent, `d/dV = d(vnew)/dV`) stay
    /// consistent — the DC solution is exact and the limiter only shapes the
    /// Newton path (ngspice DEVpnjlim / DEVfetlim).
    fn emit_limit(&mut self, expr: &IrExpr) -> Result<Value, CodegenError> {
        let (kind, args) = match expr {
            IrExpr::Sim(SimQuery::Limit { kind, args }) => (kind.as_str(), args),
            _ => unreachable!("emit_limit called on a non-Limit"),
        };
        if args.len() < 4 {
            return Err(CodegenError::unsupported(
                "$limit expects (kind, vnew, vseed, vte, vcrit)",
            ));
        }
        let slot = self
            .limits
            .iter()
            .position(|l| l == expr)
            .ok_or_else(|| CodegenError::Invalid("$limit expression missing from slot table".into()))?;
        let key = CseKey::Limit(slot as u32);
        if let Some(&hit) = self.cse.get(&key) {
            return Ok(hit);
        }
        let vnew = self.emit(&args[0])?;
        let vte = self.emit(&args[2])?;
        let vcrit = self.emit(&args[3])?;
        // vold from the state bank (previous iteration's limited value).
        let vold = self.cse_load(
            BANK_STATE,
            self.state_ptr,
            ((self.limit_base + slot) * 8) as i32,
        );
        let vlim = match kind {
            "pnjlim" => self.emit_pnjlim(vnew, vold, vte, vcrit)?,
            // fetlim not used by the current device set; fall back to the
            // identity (exact solution, weaker damping) until a device needs it.
            "fetlim" => vnew,
            other => {
                return Err(CodegenError::unsupported(format!("$limit kind `{other}`")));
            }
        };
        self.cse.insert(key, vlim);
        Ok(vlim)
    }

    /// Branchless ngspice DEVpnjlim:
    /// ```text
    /// if vnew > vcrit && |vnew-vold| > 2*vte:
    ///     if vold > 0: arg = 1 + (vnew-vold)/vte;  vlim = arg>0 ? vold+vte*ln(arg) : vcrit
    ///     else:        vlim = vte * ln(vnew/vte)
    /// else: vlim = vnew
    /// ```
    /// `ln` of a non-positive argument only occurs in a `select` arm that is not
    /// taken, and Cranelift `select` picks a value bitwise (no NaN propagation).
    fn emit_pnjlim(&mut self, vnew: Value, vold: Value, vte: Value, vcrit: Value) -> Result<Value, CodegenError> {
        let dv = self.builder.ins().fsub(vnew, vold);
        let absdv = self.builder.ins().fabs(dv);
        let two = self.cse_const(2.0);
        let two_vte = self.builder.ins().fmul(two, vte);
        let cond1 = self.builder.ins().fcmp(FloatCC::GreaterThan, vnew, vcrit);
        let cond2 = self.builder.ins().fcmp(FloatCC::GreaterThan, absdv, two_vte);
        let cond = self.builder.ins().band(cond1, cond2);
        // vold > 0 branch
        let one = self.cse_const(1.0);
        let dv_over_vte = self.builder.ins().fdiv(dv, vte);
        let arg = self.builder.ins().fadd(one, dv_over_vte);
        let ln_arg = self.call_math("ln", &[arg])?;
        let vte_ln = self.builder.ins().fmul(vte, ln_arg);
        let vold_plus = self.builder.ins().fadd(vold, vte_ln);
        let zero = self.cse_const(0.0);
        let arg_pos = self.builder.ins().fcmp(FloatCC::GreaterThan, arg, zero);
        let posval = self.builder.ins().select(arg_pos, vold_plus, vcrit);
        // vold <= 0 branch: vte * ln(vnew/vte)
        let vnew_over_vte = self.builder.ins().fdiv(vnew, vte);
        let ln_vnew = self.call_math("ln", &[vnew_over_vte])?;
        let negval = self.builder.ins().fmul(vte, ln_vnew);
        let vold_pos = self.builder.ins().fcmp(FloatCC::GreaterThan, vold, zero);
        let limited = self.builder.ins().select(vold_pos, posval, negval);
        Ok(self.builder.ins().select(cond, limited, vnew))
    }

    fn emit_unary(&mut self, op: IrUnOp, x: &IrExpr) -> Result<Value, CodegenError> {
        match op {
            IrUnOp::Neg => {
                let v = self.emit(x)?;
                Ok(self.cse_op1(T_NEG, v, |b| b.ins().fneg(v)))
            }
            IrUnOp::Not => {
                let v = self.emit(x)?;
                let key = CseKey::Op1(T_NOT, v.as_u32());
                if let Some(&hit) = self.cse.get(&key) {
                    return Ok(hit);
                }
                let zero = self.cse_const(0.0);
                let is_zero = self.builder.ins().fcmp(FloatCC::Equal, v, zero);
                let val = self.bool_to_f64(is_zero);
                self.cse.insert(key, val);
                Ok(val)
            }
            IrUnOp::BitNot | IrUnOp::RedAnd | IrUnOp::RedOr | IrUnOp::RedXor => Err(
                CodegenError::unsupported(format!("unary {op:?} in an analog expression")),
            ),
        }
    }

    fn emit_binary(&mut self, op: IrBinOp, a: &IrExpr, b: &IrExpr) -> Result<Value, CodegenError> {
        if op == IrBinOp::Pow {
            let lhs = self.emit(a)?;
            let rhs = self.emit(b)?;
            return self.call_math("pow", &[lhs, rhs]);
        }
        let lhs = self.emit(a)?;
        let rhs = self.emit(b)?;
        let key = CseKey::Op2(bin_tag(op), lhs.as_u32(), rhs.as_u32());
        if let Some(&hit) = self.cse.get(&key) {
            return Ok(hit);
        }
        let cmp = |e: &mut Self, cc: FloatCC| {
            let flag = e.builder.ins().fcmp(cc, lhs, rhs);
            e.bool_to_f64(flag)
        };
        let val = match op {
            IrBinOp::Add => self.builder.ins().fadd(lhs, rhs),
            IrBinOp::Sub => self.builder.ins().fsub(lhs, rhs),
            IrBinOp::Mul => self.builder.ins().fmul(lhs, rhs),
            IrBinOp::Div => self.builder.ins().fdiv(lhs, rhs),
            IrBinOp::Rem => {
                // fmod: a − floor(a/b)·b
                let quotient = self.builder.ins().fdiv(lhs, rhs);
                let floored = self.call_math("floor", &[quotient])?;
                let product = self.builder.ins().fmul(floored, rhs);
                self.builder.ins().fsub(lhs, product)
            }
            IrBinOp::Eq => cmp(self, FloatCC::Equal),
            IrBinOp::Ne => cmp(self, FloatCC::NotEqual),
            IrBinOp::Lt => cmp(self, FloatCC::LessThan),
            IrBinOp::Le => cmp(self, FloatCC::LessThanOrEqual),
            IrBinOp::Gt => cmp(self, FloatCC::GreaterThan),
            IrBinOp::Ge => cmp(self, FloatCC::GreaterThanOrEqual),
            IrBinOp::And | IrBinOp::Or => {
                let zero = self.cse_const(0.0);
                let a_true = self.builder.ins().fcmp(FloatCC::NotEqual, lhs, zero);
                let b_true = self.builder.ins().fcmp(FloatCC::NotEqual, rhs, zero);
                let combined = if op == IrBinOp::And {
                    self.builder.ins().band(a_true, b_true)
                } else {
                    self.builder.ins().bor(a_true, b_true)
                };
                self.bool_to_f64(combined)
            }
            IrBinOp::BitAnd | IrBinOp::BitOr | IrBinOp::BitXor | IrBinOp::Shl | IrBinOp::Shr => {
                return Err(CodegenError::unsupported(format!(
                    "bitwise/shift {op:?} in an analog expression"
                )));
            }
            IrBinOp::Pow => unreachable!("handled above"),
        };
        self.cse.insert(key, val);
        Ok(val)
    }

    fn emit_math_call(&mut self, name: &str, args: &[IrExpr]) -> Result<Value, CodegenError> {
        let values = args
            .iter()
            .map(|a| self.emit(a))
            .collect::<Result<Vec<_>, _>>()?;
        self.call_math(name, &values)
    }

    fn call_math(&mut self, name: &str, args: &[Value]) -> Result<Value, CodegenError> {
        let math_fn = math::math_fn(name)
            .ok_or_else(|| CodegenError::unsupported(format!("math builtin `{name}`")))?;
        if args.len() != math_fn.arity {
            return Err(CodegenError::Invalid(format!(
                "`{name}` expects {} args, got {}",
                math_fn.arity,
                args.len()
            )));
        }
        let key = CseKey::Call(math_fn.name, args.iter().map(|v| v.as_u32()).collect());
        if let Some(&hit) = self.cse.get(&key) {
            return Ok(hit);
        }
        let func = self.math[math_fn.name];
        let call = self.builder.ins().call(func, args);
        let val = self.builder.inst_results(call)[0];
        self.cse.insert(key, val);
        Ok(val)
    }

    /// Emit `expr` and compare it against zero, yielding an i1 flag.
    fn emit_truthy(&mut self, expr: &IrExpr) -> Result<Value, CodegenError> {
        let value = self.emit(expr)?;
        let zero = self.cse_const(0.0);
        let key = CseKey::Op2(T_FCMP_BASE + FloatCC::NotEqual as u8, value.as_u32(), zero.as_u32());
        if let Some(&hit) = self.cse.get(&key) {
            return Ok(hit);
        }
        let flag = self.builder.ins().fcmp(FloatCC::NotEqual, value, zero);
        self.cse.insert(key, flag);
        Ok(flag)
    }

    fn bool_to_f64(&mut self, flag: Value) -> Value {
        let one = self.cse_const(1.0);
        let zero = self.cse_const(0.0);
        self.cse_op3(T_SELECT, flag, one, zero, |b| b.ins().select(flag, one, zero))
    }

    pub fn f64const(&mut self, v: f64) -> Value {
        self.cse_const(v)
    }

    /// `out[idx] = value` (f64 array store).
    pub fn store_f64(&mut self, value: Value, ptr: Value, idx: usize) {
        self.builder
            .ins()
            .store(MemFlags::trusted(), value, ptr, (idx * 8) as i32);
    }

    /// `out[idx] += value` (f64 array accumulate).
    pub fn accumulate_f64(&mut self, value: Value, ptr: Value, idx: usize) {
        let current = self
            .builder
            .ins()
            .load(types::F64, MemFlags::trusted(), ptr, (idx * 8) as i32);
        let sum = self.builder.ins().fadd(current, value);
        self.builder
            .ins()
            .store(MemFlags::trusted(), sum, ptr, (idx * 8) as i32);
    }

    fn load_sim_f64(&mut self, offset: i32) -> Value {
        self.cse_load(BANK_SIM, self.sim_ptr, offset)
    }

    /// Load a `SimCtx` f64 field, falling back to `default` when the field is 0
    /// (its "unset" sentinel — e.g. `step`/`tfinal` outside a transient).
    fn sim_field_or_default(
        &mut self,
        offset: i32,
        default: &crate::ir::IrExpr,
    ) -> Result<Value, CodegenError> {
        let field = self.load_sim_f64(offset);
        let default = self.emit(default)?;
        let zero = self.f64const(0.0);
        let is_zero = self.builder.ins().fcmp(FloatCC::Equal, field, zero);
        Ok(self.builder.ins().select(is_zero, default, field))
    }
}
