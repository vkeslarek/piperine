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
}

impl AnalogEmitter<'_, '_> {
    pub fn emit(&mut self, expr: &IrExpr) -> Result<Value, CodegenError> {
        match expr {
            IrExpr::Real(v) => Ok(self.f64const(*v)),
            IrExpr::Int(v) => Ok(self.f64const(*v as f64)),
            IrExpr::Bool(b) => Ok(self.f64const(f64::from(*b))),

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
            IrExpr::State(id) => Ok(self.builder.ins().load(
                types::F64,
                MemFlags::trusted(),
                self.state_ptr,
                (id.0 * 8) as i32,
            )),

            IrExpr::Sim(query) => self.emit_sim(query),

            IrExpr::Unary(op, x) => self.emit_unary(*op, x),
            IrExpr::Binary(op, a, b) => self.emit_binary(*op, a, b),

            IrExpr::Select(c, t, e) => {
                let cond = self.emit_truthy(c)?;
                let then_ = self.emit(t)?;
                let else_ = self.emit(e)?;
                Ok(self.builder.ins().select(cond, then_, else_))
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
            IrExpr::Var(id) => Ok(self.builder.ins().load(
                types::F64,
                MemFlags::trusted(),
                self.vars_ptr,
                (id.0 * 8) as i32,
            )),
        }
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
                let kb_over_q = self.f64const(SimCtx::K_B_OVER_Q);
                Ok(self.builder.ins().fmul(temperature, kb_over_q))
            }
            SimQuery::Simparam { key, default } => match key.as_str() {
                "gmin" => Ok(self.load_sim_f64(SimField::GMIN)),
                "temperature" => Ok(self.load_sim_f64(SimField::TEMPERATURE)),
                "step" => Ok(self.load_sim_f64(SimField::STEP)),
                "tfinal" => Ok(self.load_sim_f64(SimField::TFINAL)),
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
            other @ (SimQuery::Position(_)
            | SimQuery::Angle
            | SimQuery::Limit { .. }
            | SimQuery::Random { .. }
            | SimQuery::PortConnected(_)) => Err(CodegenError::unsupported(format!(
                "simulator query {other:?}"
            ))),
        }
    }

    fn emit_unary(&mut self, op: IrUnOp, x: &IrExpr) -> Result<Value, CodegenError> {
        match op {
            IrUnOp::Neg => {
                let v = self.emit(x)?;
                Ok(self.builder.ins().fneg(v))
            }
            IrUnOp::Not => {
                let v = self.emit(x)?;
                let zero = self.f64const(0.0);
                let is_zero = self.builder.ins().fcmp(FloatCC::Equal, v, zero);
                Ok(self.bool_to_f64(is_zero))
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
        let ins = |e: &mut Self, cc: FloatCC| {
            let flag = e.builder.ins().fcmp(cc, lhs, rhs);
            e.bool_to_f64(flag)
        };
        match op {
            IrBinOp::Add => Ok(self.builder.ins().fadd(lhs, rhs)),
            IrBinOp::Sub => Ok(self.builder.ins().fsub(lhs, rhs)),
            IrBinOp::Mul => Ok(self.builder.ins().fmul(lhs, rhs)),
            IrBinOp::Div => Ok(self.builder.ins().fdiv(lhs, rhs)),
            IrBinOp::Rem => {
                // fmod: a − floor(a/b)·b
                let quotient = self.builder.ins().fdiv(lhs, rhs);
                let floored = self.call_math("floor", &[quotient])?;
                let product = self.builder.ins().fmul(floored, rhs);
                Ok(self.builder.ins().fsub(lhs, product))
            }
            IrBinOp::Eq => Ok(ins(self, FloatCC::Equal)),
            IrBinOp::Ne => Ok(ins(self, FloatCC::NotEqual)),
            IrBinOp::Lt => Ok(ins(self, FloatCC::LessThan)),
            IrBinOp::Le => Ok(ins(self, FloatCC::LessThanOrEqual)),
            IrBinOp::Gt => Ok(ins(self, FloatCC::GreaterThan)),
            IrBinOp::Ge => Ok(ins(self, FloatCC::GreaterThanOrEqual)),
            IrBinOp::And | IrBinOp::Or => {
                let zero = self.f64const(0.0);
                let a_true = self.builder.ins().fcmp(FloatCC::NotEqual, lhs, zero);
                let b_true = self.builder.ins().fcmp(FloatCC::NotEqual, rhs, zero);
                let combined = if op == IrBinOp::And {
                    self.builder.ins().band(a_true, b_true)
                } else {
                    self.builder.ins().bor(a_true, b_true)
                };
                Ok(self.bool_to_f64(combined))
            }
            IrBinOp::BitAnd | IrBinOp::BitOr | IrBinOp::BitXor | IrBinOp::Shl | IrBinOp::Shr => {
                Err(CodegenError::unsupported(format!(
                    "bitwise/shift {op:?} in an analog expression"
                )))
            }
            IrBinOp::Pow => unreachable!("handled above"),
        }
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
        let func = self.math[math_fn.name];
        let call = self.builder.ins().call(func, args);
        Ok(self.builder.inst_results(call)[0])
    }

    /// Emit `expr` and compare it against zero, yielding an i1 flag.
    fn emit_truthy(&mut self, expr: &IrExpr) -> Result<Value, CodegenError> {
        let value = self.emit(expr)?;
        let zero = self.f64const(0.0);
        Ok(self.builder.ins().fcmp(FloatCC::NotEqual, value, zero))
    }

    fn bool_to_f64(&mut self, flag: Value) -> Value {
        let one = self.f64const(1.0);
        let zero = self.f64const(0.0);
        self.builder.ins().select(flag, one, zero)
    }

    pub fn f64const(&mut self, v: f64) -> Value {
        self.builder.ins().f64const(v)
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
        self.builder
            .ins()
            .load(types::F64, MemFlags::trusted(), self.sim_ptr, offset)
    }
}
