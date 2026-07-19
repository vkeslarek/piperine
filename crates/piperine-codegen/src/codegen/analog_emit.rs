//! Analog-context emission methods for [`Builder`].
//!
//! These methods are called only when the Builder is constructed via
//! [`Builder::new_analog`] — they access the analog-context fields
//! (`branch_voltages`, `params`, `state_ptr`, `cse`, etc.) that are `None`
//! in a digital Builder. Split into a separate file for readability;
//! the type is the same `Builder`.

use cranelift_codegen::ir::{types, InstBuilder, MemFlags, Value};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_frontend::FunctionBuilder;

use piperine_lang::parse::ast::{BinaryOp, Expr, Literal, Stmt, UnaryOp};
use piperine_lang::math;
use crate::jit::{CodegenError, SimCtx};

use crate::codegen::builder::*;

impl<'a, 'f, 'm> Builder<'a, 'f, 'm> {

    /// Emit a POM `Expr` as a scalar `f64` Cranelift `Value` in analog context.
    /// This is the analog counterpart of the digital `Codegen::emit` trait —
    /// same role as the former `AnalogEmitter::emit` but dispatching on POM
    /// `Expr` instead of `IrExpr`.
    pub fn emit_analog(&mut self, expr: &Expr) -> Result<Value, CodegenError> {
        match expr {
            Expr::Literal(Literal::Real(v)) => Ok(self.cse_const(*v)),
            Expr::Literal(Literal::Int(v)) => Ok(self.cse_const(*v as f64)),
            Expr::Literal(Literal::Bool(b)) => Ok(self.cse_const(f64::from(*b))),

            // A bare identifier: param, module-level var, or state marker.
            Expr::Ident(name) => {
                if let Some(&id) = self.resolver.params.get(name) {
                    return self.params.as_ref().expect("analog context")
                        .get(id.0 as usize)
                        .copied()
                        .ok_or_else(|| CodegenError::Invalid(format!("param #{} out of range", id.0)));
                }
                if let Some(&id) = self.resolver.vars.get(name) {
                    return Ok(self.cse_load(BANK_VARS, self.vars_ptr(), (id.0 * 8) as i32));
                }
                Err(CodegenError::Invalid(format!("unresolved analog identifier `{name}`")))
            }

            // Branch access: V(p,n) / I(p,n).
            Expr::Call(func, args) => {
                if let Expr::Ident(name) = func.as_ref() {
                    match name.as_str() {
                        "V" | "I" => return self.emit_analog_branch(args),
                        "__state_load" => return self.emit_state_load(args),
                        "__temp" | "__dtemp" | "__ddtemp" | "__dtemp_inner" => {
                            let id = match args.first() {
                                Some(Expr::Literal(Literal::Int(n))) => *n as usize,
                                _ => return Err(CodegenError::Invalid(
                                    format!("{name} needs an integer id"))),
                            };
                            return match name.as_str() {
                                "__temp" => self.emit_temp(id),
                                "__dtemp" => self.emit_dtemp(id),
                                "__dtemp_inner" => self.emit_dtemp2(id),
                                _ => self.emit_ddtemp(id),
                            };
                        }
                        _ => {}
                    }
                    // Math builtins (exp, ln, sqrt, sin, …)
                    if math::math_fn(name).is_some() {
                        return self.emit_analog_math_call(name, args);
                    }
                }
                Err(CodegenError::unsupported(format!(
                    "call `{}` in an analog expression (should be inlined)",
                    ident_from_expr(Some(func)).unwrap_or_default()
                )))
            }

            // Syscalls: $temperature, $abstime, $vt, $simparam, $limit, …
            Expr::SysCall(name, args) => self.emit_analog_syscall(name, args),

            Expr::Unary(op, x) => self.emit_analog_unary(op.clone(), x),

            Expr::Binary(lhs, op, rhs) => self.emit_analog_binary(op.clone(), lhs, rhs),

            // Ternary select: If { cond, then, else } → Cranelift select.
            Expr::If { cond, then_body, else_body } => {
                let c = self.emit_analog_truthy(cond)?;
                let t = self.emit_analog_block_value(then_body)?;
                let e = self.emit_analog_block_value(else_body)?;
                Ok(self.cse_op3(T_SELECT, c, t, e, |b| b.ins().select(c, t, e)))
            }

            Expr::Block(b) => self.emit_analog_block_value(b),

            Expr::Cast(_, inner) => self.emit_analog(inner),

            Expr::Field(base, field) => {
                // Flattened bundle field: "base_field" as a combined name.
                if let Expr::Ident(base_name) = base.as_ref() {
                    let combined = format!("{base_name}_{field}");
                    if let Some(&id) = self.resolver.params.get(&combined) {
                        return self.params.as_ref().expect("analog context")
                            .get(id.0 as usize)
                            .copied()
                            .ok_or_else(|| CodegenError::Invalid(format!("param #{} out of range", id.0)));
                    }
                    if let Some(&id) = self.resolver.vars.get(&combined) {
                        return Ok(self.cse_load(BANK_VARS, self.vars_ptr(), (id.0 * 8) as i32));
                    }
                }
                Err(CodegenError::unsupported(format!("unresolved field access in analog: {expr:?}")))
            }

            Expr::Literal(Literal::String(_)) | Expr::Literal(Literal::None) | Expr::Literal(Literal::Quad(_)) => {
                Err(CodegenError::unsupported("non-real literal in an analog expression"))
            }
            Expr::Path(_) => Err(CodegenError::unsupported("path in an analog expression")),
            Expr::Index(_, _) | Expr::Slice(_, _) | Expr::Array(_) | Expr::Tuple(_)
            | Expr::BundleLit { .. } | Expr::MapLit(_) | Expr::SetLit(_) | Expr::Lambda { .. } => {
                Err(CodegenError::unsupported("vector/value-layer expression in an analog contribution"))
            }
        }
    }

    /// Emit a branch voltage V(plus, minus) lookup from precomputed values.
    fn emit_analog_branch(&mut self, args: &[Expr]) -> Result<Value, CodegenError> {
        let plus_name = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
        let minus_name = ident_from_expr(args.get(1)).unwrap_or_else(|| "0".into());
        let plus = self.resolve_node(&plus_name)?;
        let minus = self.resolve_node(&minus_name)?;
        self.branch_voltages.as_ref().expect("analog context")
            .get(&(plus, minus))
            .copied()
            .ok_or_else(|| CodegenError::Invalid(format!(
                "branch V(#{}, #{}) missing from the precomputed set", plus.0, minus.0
            )))
    }

    /// Emit `__state_load(id)` → load from the state bank.
    fn emit_state_load(&mut self, args: &[Expr]) -> Result<Value, CodegenError> {
        let id = match args.first() {
            Some(Expr::Literal(Literal::Int(v))) => *v as u32,
            _ => return Err(CodegenError::unsupported("__state_load expects a state id")),
        };
        Ok(self.cse_load(BANK_STATE, self.state_ptr(), (id * 8) as i32))
    }

    // ── CSE helpers ──

    fn cse_const(&mut self, v: f64) -> Value {
        let key = CseKey::Const(v.to_bits());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return hit;
        }
        let val = self.builder.ins().f64const(v);
        self.cse.as_mut().expect("analog context").insert(key, val);
        val
    }

    fn cse_load(&mut self, bank: u8, ptr: Value, offset: i32) -> Value {
        let key = CseKey::Load(bank, offset);
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return hit;
        }
        let val = self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, offset);
        self.cse.as_mut().expect("analog context").insert(key, val);
        val
    }

    fn cse_op1(&mut self, tag: u8, x: Value, build: impl FnOnce(&mut FunctionBuilder) -> Value) -> Value {
        let key = CseKey::Op1(tag, x.as_u32());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return hit;
        }
        let val = build(self.builder);
        self.cse.as_mut().expect("analog context").insert(key, val);
        val
    }

    fn cse_op2(&mut self, tag: u8, a: Value, b: Value, build: impl FnOnce(&mut FunctionBuilder) -> Value) -> Value {
        let key = CseKey::Op2(tag, a.as_u32(), b.as_u32());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return hit;
        }
        let val = build(self.builder);
        self.cse.as_mut().expect("analog context").insert(key, val);
        val
    }

    fn cse_op3(&mut self, tag: u8, a: Value, b: Value, c: Value, build: impl FnOnce(&mut FunctionBuilder) -> Value) -> Value {
        let key = CseKey::Op3(tag, a.as_u32(), b.as_u32(), c.as_u32());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return hit;
        }
        let val = build(self.builder);
        self.cse.as_mut().expect("analog context").insert(key, val);
        val
    }

    // ── Sim queries ──

    fn emit_analog_syscall(&mut self, name: &str, args: &[Expr]) -> Result<Value, CodegenError> {
        let key = name.trim_start_matches('$').to_lowercase();
        match key.as_str() {
            "temperature" => Ok(self.load_sim_f64(SimField::TEMPERATURE)),
            "abstime" => Ok(self.load_sim_f64(SimField::ABSTIME)),
            "mfactor" => Ok(self.load_sim_f64(SimField::MFACTOR)),
            "vt" => {
                let temperature = match args.first() {
                    Some(e) => self.emit_analog(e)?,
                    None => self.load_sim_f64(SimField::TEMPERATURE),
                };
                let kb_over_q = self.cse_const(SimCtx::K_B_OVER_Q);
                Ok(self.cse_op2(bin_tag(crate::ir::BinOp::Mul), temperature, kb_over_q, |b| {
                    b.ins().fmul(temperature, kb_over_q)
                }))
            }
            "simparam" => {
                let sim_key = match args.first() {
                    Some(Expr::Literal(Literal::String(s))) => s.clone(),
                    _ => "?".into(),
                };
                let default = args.get(1);
                match sim_key.as_str() {
                    "gmin" => Ok(self.load_sim_f64(SimField::GMIN)),
                    "temperature" => Ok(self.load_sim_f64(SimField::TEMPERATURE)),
                    "step" => self.sim_field_or_default(SimField::STEP, default),
                    "tfinal" => self.sim_field_or_default(SimField::TFINAL, default),
                    // ngspice CKTsrcFact: the DC source-stepping ramp scale.
                    "sourceScaleFactor" => Ok(self.load_sim_f64(SimField::SRCFACT)),
                    _ => default.map(|d| self.emit_analog(d)).unwrap_or(Ok(self.cse_const(0.0))),
                }
            }
            "param_given" => {
                let pname = match args.first() {
                    Some(Expr::Literal(Literal::String(s))) => s.clone(),
                    _ => "?".into(),
                };
                let id = self.resolver.param_given(&pname).ok_or_else(|| {
                    CodegenError::Invalid(format!("$param_given: unresolved param `{pname}`"))
                })?;
                let sim_ptr = self.sim_ptr();
                let mask = self.builder.ins().load(
                    types::I64,
                    MemFlags::trusted(),
                    sim_ptr,
                    SimField::PARAM_GIVEN_MASK,
                );
                let shifted = self.builder.ins().ushr_imm(mask, i64::from(id.0));
                let bit = self.builder.ins().band_imm(shifted, 1);
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_set = self.builder.ins().icmp(IntCC::NotEqual, bit, zero);
                Ok(self.bool_to_f64(is_set))
            }
            "analysis" => {
                let kind = match args.first() {
                    Some(Expr::Literal(Literal::String(s))) => match s.as_str() {
                        "ac" => 1u64,
                        "dc" => 0,
                        "tran" => 2,
                        "noise" => 3,
                        _ => 0,
                    },
                    _ => 0,
                };
                let sim_ptr = self.sim_ptr();
                let current = self.builder.ins().load(
                    types::I64,
                    MemFlags::trusted(),
                    sim_ptr,
                    SimField::CURRENT_ANALYSIS,
                );
                let target = self.builder.ins().iconst(types::I64, kind as i64);
                let matches = self.builder.ins().icmp(IntCC::Equal, current, target);
                Ok(self.bool_to_f64(matches))
            }
            "limit" => self.emit_analog_limit(name, args),
            _ => Err(CodegenError::unsupported(format!("syscall `{name}` in an analog expression"))),
        }
    }

    /// Load a `SimCtx` f64 field.
    fn load_sim_f64(&mut self, offset: i32) -> Value {
        self.cse_load(BANK_SIM, self.sim_ptr(), offset)
    }

    /// Load a `SimCtx` f64 field, falling back to `default` when the field
    /// is 0 (its "unset" sentinel).
    fn sim_field_or_default(&mut self, offset: i32, default: Option<&Expr>) -> Result<Value, CodegenError> {
        let field = self.load_sim_f64(offset);
        let default = match default {
            Some(e) => self.emit_analog(e)?,
            None => self.cse_const(0.0),
        };
        let zero = self.cse_const(0.0);
        let is_zero = self.builder.ins().fcmp(FloatCC::Equal, field, zero);
        Ok(self.builder.ins().select(is_zero, default, field))
    }

    // ── $limit ──

    fn emit_analog_limit(&mut self, full_name: &str, args: &[Expr]) -> Result<Value, CodegenError> {
        // The first arg is the kind string ("pnjlim"/"fetlim"), the rest
        // are (vnew, vseed, vte, vcrit).
        let kind = match args.first() {
            Some(Expr::Literal(Literal::String(s))) => s.as_str(),
            _ => return Err(CodegenError::unsupported("$limit expects a kind string")),
        };
        if args.len() < 5 {
            return Err(CodegenError::unsupported("$limit expects (kind, vnew, vseed, vte, vcrit)"));
        }
        // Find the slot by structural equality against the limits table.
        let limits = self.limits.as_ref().expect("analog context");
        let slot = limits.iter().position(|l| expr_structural_eq(l, &Expr::SysCall(full_name.to_string(), args.to_vec())))
            .ok_or_else(|| CodegenError::Invalid("$limit expression missing from slot table".into()))?;
        let key = CseKey::Limit(slot as u32);
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return Ok(hit);
        }
        let vnew = self.emit_analog(&args[1])?;
        let vte = self.emit_analog(&args[3])?;
        let vcrit = self.emit_analog(&args[4])?;
        let vold = self.cse_load(BANK_STATE, self.state_ptr(), ((self.limit_base + slot) * 8) as i32);
        // For `fetlim`/`limvds` the `vte` argument slot carries `vto` (the
        // threshold); `limvds` ignores it. `vcrit` still seeds `vold`.
        let vlim = match kind {
            "pnjlim" => self.emit_pnjlim(vnew, vold, vte, vcrit)?,
            "fetlim" => self.emit_fetlim(vnew, vold, vte),
            "limvds" => self.emit_limvds(vnew, vold),
            other => return Err(CodegenError::unsupported(format!("$limit kind `{other}`"))),
        };
        self.cse.as_mut().expect("analog context").insert(key, vlim);
        Ok(vlim)
    }

    /// Branchless ngspice DEVpnjlim (copied from emit.rs).
    fn emit_pnjlim(&mut self, vnew: Value, vold: Value, vte: Value, vcrit: Value) -> Result<Value, CodegenError> {
        let dv = self.builder.ins().fsub(vnew, vold);
        let absdv = self.builder.ins().fabs(dv);
        let two = self.cse_const(2.0);
        let two_vte = self.builder.ins().fmul(two, vte);
        let cond1 = self.builder.ins().fcmp(FloatCC::GreaterThan, vnew, vcrit);
        let cond2 = self.builder.ins().fcmp(FloatCC::GreaterThan, absdv, two_vte);
        let cond = self.builder.ins().band(cond1, cond2);
        let one = self.cse_const(1.0);
        let dv_over_vte = self.builder.ins().fdiv(dv, vte);
        let arg = self.builder.ins().fadd(one, dv_over_vte);
        let ln_arg = self.analog_call_math("ln", &[arg])?;
        let vte_ln = self.builder.ins().fmul(vte, ln_arg);
        let vold_plus = self.builder.ins().fadd(vold, vte_ln);
        let zero = self.cse_const(0.0);
        let arg_pos = self.builder.ins().fcmp(FloatCC::GreaterThan, arg, zero);
        let posval = self.builder.ins().select(arg_pos, vold_plus, vcrit);
        let vnew_over_vte = self.builder.ins().fdiv(vnew, vte);
        let ln_vnew = self.analog_call_math("ln", &[vnew_over_vte])?;
        let negval = self.builder.ins().fmul(vte, ln_vnew);
        let vold_pos = self.builder.ins().fcmp(FloatCC::GreaterThan, vold, zero);
        let limited = self.builder.ins().select(vold_pos, posval, negval);
        Ok(self.builder.ins().select(cond, limited, vnew))
    }

    /// ngspice `MIN(a, b)` — a plain comparison (not IEEE fmin), matching the
    /// C source exactly.
    fn emit_min(&mut self, a: Value, b: Value) -> Value {
        let c = self.builder.ins().fcmp(FloatCC::LessThan, a, b);
        self.builder.ins().select(c, a, b)
    }

    /// ngspice `MAX(a, b)`.
    fn emit_max(&mut self, a: Value, b: Value) -> Value {
        let c = self.builder.ins().fcmp(FloatCC::GreaterThan, a, b);
        self.builder.ins().select(c, a, b)
    }

    /// Branchless ngspice `DEVfetlim(vnew, vold, vto)` (devsup.c) — the MOSFET
    /// gate-voltage limiter. Every C `if`/`else` becomes a `select` on the
    /// same predicate; leaf assignments are the candidate values.
    fn emit_fetlim(&mut self, vnew: Value, vold: Value, vto: Value) -> Value {
        let two = self.cse_const(2.0);
        let one = self.cse_const(1.0);
        let zero = self.cse_const(0.0);
        let half = self.cse_const(0.5);
        let four = self.cse_const(4.0);

        let vold_m_vto = self.builder.ins().fsub(vold, vto);
        let two_diff = self.builder.ins().fmul(two, vold_m_vto);
        let abs_two_diff = self.builder.ins().fabs(two_diff);
        let vtsthi = self.builder.ins().fadd(abs_two_diff, two);           // |2(vold-vto)|+2
        let abs_diff = self.builder.ins().fabs(vold_m_vto);
        let vtstlo = self.builder.ins().fadd(abs_diff, one);              // |vold-vto|+1
        let c35 = self.cse_const(3.5);
        let vtox = self.builder.ins().fadd(vto, c35);                     // vto+3.5
        let delv = self.builder.ins().fsub(vnew, vold);
        let neg_delv = self.builder.ins().fneg(delv);

        let on = self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, vold, vto);
        let high = self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, vold, vtox);
        let delv_le0 = self.builder.ins().fcmp(FloatCC::LessThanOrEqual, delv, zero);

        // on & high & going off (delv<=0):
        //   vnew>=vtox ? (-delv>vtstlo ? vold-vtstlo : vnew) : max(vnew, vto+2)
        let vnew_ge_vtox = self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, vnew, vtox);
        let vold_m_vtstlo = self.builder.ins().fsub(vold, vtstlo);
        let negdelv_gt_vtstlo = self.builder.ins().fcmp(FloatCC::GreaterThan, neg_delv, vtstlo);
        let off_step = self.builder.ins().select(negdelv_gt_vtstlo, vold_m_vtstlo, vnew);
        let vto_p2 = self.builder.ins().fadd(vto, two);
        let max_vto_p2 = self.emit_max(vnew, vto_p2);
        let high_gooff = self.builder.ins().select(vnew_ge_vtox, off_step, max_vto_p2);
        // on & high & staying on (delv>0): delv>=vtsthi ? vold+vtsthi : vnew
        let delv_ge_vtsthi = self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, delv, vtsthi);
        let vold_p_vtsthi = self.builder.ins().fadd(vold, vtsthi);
        let high_stayon = self.builder.ins().select(delv_ge_vtsthi, vold_p_vtsthi, vnew);
        let high_val = self.builder.ins().select(delv_le0, high_gooff, high_stayon);

        // on & mid (vold<vtox): delv<=0 ? max(vnew, vto-0.5) : min(vnew, vto+4)
        let vto_m05 = self.builder.ins().fsub(vto, half);
        let mid_dec = self.emit_max(vnew, vto_m05);
        let vto_p4 = self.builder.ins().fadd(vto, four);
        let mid_inc = self.emit_min(vnew, vto_p4);
        let mid_val = self.builder.ins().select(delv_le0, mid_dec, mid_inc);

        let on_val = self.builder.ins().select(high, high_val, mid_val);

        // off (vold<vto):
        //   delv<=0: -delv>vtsthi ? vold-vtsthi : vnew
        let negdelv_gt_vtsthi = self.builder.ins().fcmp(FloatCC::GreaterThan, neg_delv, vtsthi);
        let vold_m_vtsthi = self.builder.ins().fsub(vold, vtsthi);
        let off_dec = self.builder.ins().select(negdelv_gt_vtsthi, vold_m_vtsthi, vnew);
        //   delv>0: vnew<=vto+0.5 ? (delv>vtstlo ? vold+vtstlo : vnew) : vto+0.5
        let vto_p05 = self.builder.ins().fadd(vto, half);
        let vnew_le_vtemp = self.builder.ins().fcmp(FloatCC::LessThanOrEqual, vnew, vto_p05);
        let delv_gt_vtstlo = self.builder.ins().fcmp(FloatCC::GreaterThan, delv, vtstlo);
        let vold_p_vtstlo = self.builder.ins().fadd(vold, vtstlo);
        let off_inc_lo = self.builder.ins().select(delv_gt_vtstlo, vold_p_vtstlo, vnew);
        let off_inc = self.builder.ins().select(vnew_le_vtemp, off_inc_lo, vto_p05);
        let off_val = self.builder.ins().select(delv_le0, off_dec, off_inc);

        self.builder.ins().select(on, on_val, off_val)
    }

    /// Branchless ngspice `DEVlimvds(vnew, vold)` (devsup.c) — the MOSFET
    /// drain-source voltage limiter.
    fn emit_limvds(&mut self, vnew: Value, vold: Value) -> Value {
        let c35 = self.cse_const(3.5);
        let c3 = self.cse_const(3.0);
        let two = self.cse_const(2.0);
        let c4 = self.cse_const(4.0);
        let cm05 = self.cse_const(-0.5);

        let hi = self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, vold, c35);
        let gt = self.builder.ins().fcmp(FloatCC::GreaterThan, vnew, vold);

        // hi & vnew>vold: min(vnew, 3*vold+2)
        let three_vold = self.builder.ins().fmul(c3, vold);
        let lim1 = self.builder.ins().fadd(three_vold, two);
        let hi_gt = self.emit_min(vnew, lim1);
        // hi & !gt: vnew<3.5 ? max(vnew,2) : vnew
        let lt35 = self.builder.ins().fcmp(FloatCC::LessThan, vnew, c35);
        let max2 = self.emit_max(vnew, two);
        let hi_le = self.builder.ins().select(lt35, max2, vnew);
        let hi_val = self.builder.ins().select(gt, hi_gt, hi_le);

        // lo & gt: min(vnew,4); lo & !gt: max(vnew,-0.5)
        let lo_gt = self.emit_min(vnew, c4);
        let lo_le = self.emit_max(vnew, cm05);
        let lo_val = self.builder.ins().select(gt, lo_gt, lo_le);

        self.builder.ins().select(hi, hi_val, lo_val)
    }

    // ── Unary / binary / math ──

    fn emit_analog_unary(&mut self, op: UnaryOp, x: &Expr) -> Result<Value, CodegenError> {
        match op {
            UnaryOp::Neg => {
                let v = self.emit_analog(x)?;
                Ok(self.cse_op1(T_NEG, v, |b| b.ins().fneg(v)))
            }
            UnaryOp::Not => {
                let v = self.emit_analog(x)?;
                let key = CseKey::Op1(T_NOT, v.as_u32());
                if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
                    return Ok(hit);
                }
                let zero = self.cse_const(0.0);
                let is_zero = self.builder.ins().fcmp(FloatCC::Equal, v, zero);
                let val = self.bool_to_f64(is_zero);
                self.cse.as_mut().expect("analog context").insert(key, val);
                Ok(val)
            }
        }
    }

    fn emit_analog_binary(&mut self, op: BinaryOp, a: &Expr, b: &Expr) -> Result<Value, CodegenError> {
        let ir_op = lower_binop_pom(op);
        // POM `BinaryOp` has no `Pow` variant, so the resolved/differentiated
        // form carries exponentiation as `BitXor` (see `lower/expr.rs`
        // `to_pom`; `d_math("pow")` and `d_via_pow` produce it). In an analog
        // expression a real bitwise xor is meaningless, so `BitXor` is always
        // that pow carrier.
        if ir_op == crate::ir::BinOp::Pow || ir_op == crate::ir::BinOp::BitXor {
            let lhs = self.emit_analog(a)?;
            let rhs = self.emit_analog(b)?;
            return self.analog_call_math("pow", &[lhs, rhs]);
        }
        let lhs = self.emit_analog(a)?;
        let rhs = self.emit_analog(b)?;
        let key = CseKey::Op2(bin_tag(ir_op), lhs.as_u32(), rhs.as_u32());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return Ok(hit);
        }
        let cmp = |e: &mut Self, cc: FloatCC| {
            let flag = e.builder.ins().fcmp(cc, lhs, rhs);
            e.bool_to_f64(flag)
        };
        let val = match ir_op {
            crate::ir::BinOp::Add => self.builder.ins().fadd(lhs, rhs),
            crate::ir::BinOp::Sub => self.builder.ins().fsub(lhs, rhs),
            crate::ir::BinOp::Mul => self.builder.ins().fmul(lhs, rhs),
            crate::ir::BinOp::Div => self.builder.ins().fdiv(lhs, rhs),
            crate::ir::BinOp::Rem => {
                let quotient = self.builder.ins().fdiv(lhs, rhs);
                let floored = self.analog_call_math("floor", &[quotient])?;
                let product = self.builder.ins().fmul(floored, rhs);
                self.builder.ins().fsub(lhs, product)
            }
            crate::ir::BinOp::Eq => cmp(self, FloatCC::Equal),
            crate::ir::BinOp::Ne => cmp(self, FloatCC::NotEqual),
            crate::ir::BinOp::Lt => cmp(self, FloatCC::LessThan),
            crate::ir::BinOp::Le => cmp(self, FloatCC::LessThanOrEqual),
            crate::ir::BinOp::Gt => cmp(self, FloatCC::GreaterThan),
            crate::ir::BinOp::Ge => cmp(self, FloatCC::GreaterThanOrEqual),
            crate::ir::BinOp::And | crate::ir::BinOp::Or => {
                let zero = self.cse_const(0.0);
                let a_true = self.builder.ins().fcmp(FloatCC::NotEqual, lhs, zero);
                let b_true = self.builder.ins().fcmp(FloatCC::NotEqual, rhs, zero);
                let combined = if ir_op == crate::ir::BinOp::And {
                    self.builder.ins().band(a_true, b_true)
                } else {
                    self.builder.ins().bor(a_true, b_true)
                };
                self.bool_to_f64(combined)
            }
            crate::ir::BinOp::BitAnd | crate::ir::BinOp::BitOr
            | crate::ir::BinOp::BitXor | crate::ir::BinOp::Shl | crate::ir::BinOp::Shr => {
                return Err(CodegenError::unsupported(format!("bitwise/shift {ir_op:?} in an analog expression")));
            }
            crate::ir::BinOp::Pow => unreachable!("handled above"),
        };
        self.cse.as_mut().expect("analog context").insert(key, val);
        Ok(val)
    }

    fn emit_analog_math_call(&mut self, name: &str, args: &[Expr]) -> Result<Value, CodegenError> {
        let values = args.iter()
            .map(|a| self.emit_analog(a))
            .collect::<Result<Vec<_>, _>>()?;
        self.analog_call_math(name, &values)
    }

    fn analog_call_math(&mut self, name: &str, args: &[Value]) -> Result<Value, CodegenError> {
        let math_fn = math::math_fn(name)
            .ok_or_else(|| CodegenError::unsupported(format!("math builtin `{name}`")))?;
        if args.len() != math_fn.arity {
            return Err(CodegenError::Invalid(format!(
                "`{name}` expects {} args, got {}", math_fn.arity, args.len()
            )));
        }
        let key = CseKey::Call(math_fn.name, args.iter().map(|v| v.as_u32()).collect());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return Ok(hit);
        }
        let func = self.math[math_fn.name];
        let call = self.builder.ins().call(func, args);
        let val = self.builder.inst_results(call)[0];
        self.cse.as_mut().expect("analog context").insert(key, val);
        Ok(val)
    }

    /// Emit `expr` and compare against zero, yielding an i1 flag.
    fn emit_analog_truthy(&mut self, expr: &Expr) -> Result<Value, CodegenError> {
        let value = self.emit_analog(expr)?;
        let zero = self.cse_const(0.0);
        let key = CseKey::Op2(T_FCMP_BASE + FloatCC::NotEqual as u8, value.as_u32(), zero.as_u32());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return Ok(hit);
        }
        let flag = self.builder.ins().fcmp(FloatCC::NotEqual, value, zero);
        self.cse.as_mut().expect("analog context").insert(key, flag);
        Ok(flag)
    }

    fn bool_to_f64(&mut self, flag: Value) -> Value {
        let one = self.cse_const(1.0);
        let zero = self.cse_const(0.0);
        self.cse_op3(T_SELECT, flag, one, zero, |b| b.ins().select(flag, one, zero))
    }

    /// Cached f64 constant (analog context).
    pub fn analog_f64const(&mut self, v: f64) -> Value {
        self.cse_const(v)
    }

    /// `out[idx] = value` (f64 array store).
    pub fn store_f64(&mut self, value: Value, ptr: Value, idx: usize) {
        self.builder.ins().store(MemFlags::trusted(), value, ptr, (idx * 8) as i32);
    }

    /// `out[idx] += value` (f64 array accumulate).
    pub fn accumulate_f64(&mut self, value: Value, ptr: Value, idx: usize) {
        let current = self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, (idx * 8) as i32);
        let sum = self.builder.ins().fadd(current, value);
        self.builder.ins().store(MemFlags::trusted(), sum, ptr, (idx * 8) as i32);
    }

    /// Evaluate a POM `Block` to its expression value (analog context).
    fn emit_analog_block_value(&mut self, block: &piperine_lang::parse::ast::Block) -> Result<Value, CodegenError> {
        if let Some(e) = &block.expr {
            return self.emit_analog(e);
        }
        for s in block.stmts.iter().rev() {
            if let Stmt::Expr(e) = s {
                return self.emit_analog(e);
            }
        }
        Ok(self.cse_const(0.0))
    }
}
