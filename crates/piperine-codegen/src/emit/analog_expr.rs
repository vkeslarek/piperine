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

use piperine_lang::ast::{BinOp, Expr, UnOp};
use piperine_lang::lexer::NumberLit;
use piperine_lang::math;
use crate::error::CodegenError;
use crate::emit::abi::SimCtx;

use crate::emit::builder::*;
use crate::emit::cse::*;

impl<'a, 'f, 'm> Builder<'a, 'f, 'm> {

    /// Emit a POM `Expr` as a scalar `f64` Cranelift `Value` in analog context.
    /// This is the analog counterpart of the digital `Codegen::emit` trait,
    /// dispatching on POM `Expr`.
    pub fn emit_analog(&mut self, expr: &Expr) -> Result<Value, CodegenError> {
        match expr {
            Expr::Number { value: NumberLit::Real(v), .. } => Ok(self.cse_const(*v)),
            Expr::Number { value: NumberLit::Integer(v), .. } => Ok(self.cse_const(*v as f64)),
            Expr::Number { value: NumberLit::Based(_), .. } => {
                Err(CodegenError::unsupported("based (4-state) literal in an analog expression"))
            }
            Expr::Str { .. } => Err(CodegenError::unsupported("string literal in an analog expression")),

            // A bare identifier: param, module-level var, or state marker.
            Expr::Ident { name, .. } => {
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

            // Branch access: V(p,n) / I(p,n) — LRM's dedicated AccessFunction
            // node, not a generic call.
            Expr::AccessFunction { pos, neg, .. } => self.emit_analog_branch(pos, neg.as_deref()),

            // Marker calls (`__state_load`, tape markers) and math builtins.
            Expr::Call { name, args, .. } => {
                match name.as_str() {
                    "__state_load" => return self.emit_state_load(args),
                    n if Self::is_tape_marker(n) => {
                        let id = match args.first() {
                            Some(Expr::Number { value: NumberLit::Integer(n), .. }) => *n as usize,
                            _ => return Err(CodegenError::Invalid(
                                format!("{n} needs an integer id"))),
                        };
                        return self.emit_tape(n, id);
                    }
                    _ => {}
                }
                // Math builtins (exp, ln, sqrt, sin, …)
                if math::math_fn(name).is_some() {
                    return self.emit_analog_math_call(name, args);
                }
                Err(CodegenError::unsupported(format!(
                    "call `{name}` in an analog expression (should be inlined)"
                )))
            }

            // Syscalls: $temperature, $abstime, $vt, $simparam, $limit, …
            Expr::SystemFunctionCall { name, args, .. } => self.emit_analog_syscall(name, args),

            Expr::Unary { op, operand, .. } => self.emit_analog_unary(*op, operand),

            Expr::Binary { op, lhs, rhs, .. } => self.emit_analog_binary(*op, lhs, rhs),

            // Ternary select: VAMS's expression-level conditional (§4.1.7) →
            // Cranelift select.
            Expr::Ternary { cond, then_branch, else_branch, .. } => {
                let c = self.emit_analog_truthy(cond)?;
                let t = self.emit_analog(then_branch)?;
                let e = self.emit_analog(else_branch)?;
                Ok(self.cse_op3(T_SELECT, c, t, e, |b| b.ins().select(c, t, e)))
            }

            Expr::Concat { .. } | Expr::Replication { .. } | Expr::Index { .. } | Expr::PartSelect { .. } => {
                Err(CodegenError::unsupported("vector/bit-select expression in an analog contribution"))
            }

            // An assignment pattern (`'{...}`) is a literal list, not a
            // scalar — it has no `f64` Cranelift value of its own. Its
            // legal use sites (parameter-array initializers, `laplace_*`
            // coefficient arguments) consume it before it ever reaches
            // scalar emission; reaching this arm means it flowed
            // somewhere emission can't yet handle it.
            Expr::AssignmentPattern { .. } => {
                Err(CodegenError::unsupported("assignment pattern in a scalar analog expression"))
            }

            // A PortAccess (`I(<port>)`) never reaches codegen:
            // elab::lower::vams rewrites every occurrence into a sum of
            // AccessFunction reads before a Module leaves the elaborator
            // (LRMC-C1-10). Reaching this arm means that invariant broke.
            Expr::PortAccess { .. } => {
                Err(CodegenError::unsupported("unresolved port access in a scalar analog expression"))
            }
        }
    }

    /// Emit a branch voltage V(plus, minus) lookup from precomputed values.
    fn emit_analog_branch(&mut self, pos: &Expr, neg: Option<&Expr>) -> Result<Value, CodegenError> {
        let plus_name = ident_from_expr(Some(pos)).unwrap_or_else(|| "?".into());
        let minus_name = neg.and_then(|n| ident_from_expr(Some(n))).unwrap_or_else(|| "0".into());
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
            Some(Expr::Number { value: NumberLit::Integer(v), .. }) => *v as u32,
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
                Ok(self.cse_op2(bin_tag(crate::resolve::BinOp::Mul), temperature, kb_over_q, |b| {
                    b.ins().fmul(temperature, kb_over_q)
                }))
            }
            "simparam" => {
                let sim_key = match args.first() {
                    Some(Expr::Str { value, .. }) => value.clone(),
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
                    Some(Expr::Str { value, .. }) => value.clone(),
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
                    Some(Expr::Str { value, .. }) => match value.as_str() {
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
            // §9.19 (LRMC-C7-09, T50): `$port_connected(port_identifier)`
            // — a bare port identifier (Syntax 9-14), not a string
            // literal (unlike `$param_given`'s own, differently-shaped
            // argument). `SimCtx.port_connected_mask` (T50) is a
            // per-instance constant, computed once at instance
            // construction (`device/builder.rs`) exactly the way
            // `param_given_mask` already is.
            "port_connected" => {
                let pname = match args.first() {
                    Some(Expr::Ident { name, .. }) => name.clone(),
                    _ => {
                        return Err(CodegenError::Invalid(
                            "$port_connected expects a bare port identifier".to_string(),
                        ));
                    }
                };
                let node = self.resolver.nodes.get(&pname).copied().ok_or_else(|| {
                    CodegenError::Invalid(format!("$port_connected: `{pname}` is not a declared net"))
                })?;
                let idx = self.module.ports.iter().position(|p| p.node == node).ok_or_else(|| {
                    CodegenError::Invalid(format!("$port_connected: `{pname}` is not a port of this module"))
                })?;
                let sim_ptr = self.sim_ptr();
                let mask = self.builder.ins().load(
                    types::I64,
                    MemFlags::trusted(),
                    sim_ptr,
                    SimField::PORT_CONNECTED_MASK,
                );
                let shifted = self.builder.ins().ushr_imm(mask, idx as i64);
                let bit = self.builder.ins().band_imm(shifted, 1);
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_set = self.builder.ins().icmp(IntCC::NotEqual, bit, zero);
                Ok(self.bool_to_f64(is_set))
            }
            "limit" => self.emit_analog_limit(name, args),
            // §9.20 (LRMC-C7-09, T50): `$analog_node_alias`/
            // `$analog_port_alias` unify a local node with an arbitrary
            // hierarchical-reference-string net at runtime, re-evaluated
            // "each sweep point of a dc sweep" (§9.20's own text) — a
            // genuine per-sweep-point *topology* change, which MD-18
            // ("elaboration fixes devices; sweeps restamp on the
            // compiled circuit, never re-elaborate") forbids outright,
            // not merely a feature this task didn't reach. Fails loud
            // naming the real architectural reason, not a generic
            // "unsupported syscall" — a future implementation would need
            // a documented MD exception, not just more code here.
            "analog_node_alias" | "analog_port_alias" => Err(CodegenError::unsupported(format!(
                "${name}: dynamic per-sweep-point node aliasing conflicts with MD-18 \
                 (no re-JIT/re-topology during simulation — elaboration fixes devices once)"
            ))),
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
            Some(Expr::Str { value, .. }) => value.as_str(),
            _ => return Err(CodegenError::unsupported("$limit expects a kind string")),
        };
        if args.len() < 5 {
            return Err(CodegenError::unsupported("$limit expects (kind, vnew, vseed, vte, vcrit)"));
        }
        // Find the slot by structural equality against the limits table.
        let limits = self.limits.as_ref().expect("analog context");
        let nospan = piperine_lang::error::Span::new(0, 0);
        let probe = Expr::SystemFunctionCall { span: nospan, name: full_name.to_string(), args: args.to_vec() };
        let slot = limits.iter().position(|l| expr_structural_eq(l, &probe))
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
        // `"vdslim"` (LRM Table E.2's own preferred name, T58/LRMC-C10-17)
        // is accepted as a synonym for `"limvds"` (ngspice's own naming,
        // kept working so existing callers don't break) — both link
        // against the identical `wrappers::limvds` JIT symbol.
        let vlim = match kind {
            "pnjlim" => self.emit_limit_call("pnjlim", &[vnew, vold, vte, vcrit])?,
            "fetlim" => self.emit_limit_call("fetlim", &[vnew, vold, vte])?,
            "limvds" | "vdslim" => self.emit_limit_call("limvds", &[vnew, vold])?,
            other => return Err(CodegenError::unsupported(format!("$limit kind `{other}`"))),
        };
        self.cse.as_mut().expect("analog context").insert(key, vlim);
        Ok(vlim)
    }

    /// Call a registered `piperine_ir::limit` symbol (`pnjlim`/`fetlim`/
    /// `limvds`, imported into the analog JIT — `kernel/analog/compile.rs`'s
    /// `LIMIT_FNS` loop). No CSE here: the caller (`emit_analog_limit`)
    /// already CSEs the whole `$limit` under `CseKey::Limit(slot)`, so a
    /// repeated `$limit` never reaches this twice for the same slot.
    fn emit_limit_call(&mut self, name: &'static str, args: &[Value]) -> Result<Value, CodegenError> {
        let func = *self.limit_fns.expect("analog context").get(name)
            .ok_or_else(|| CodegenError::unsupported(format!("$limit kind `{name}`")))?;
        let call = self.builder.ins().call(func, args);
        Ok(self.builder.inst_results(call)[0])
    }

    // ── Unary / binary / math ──

    fn emit_analog_unary(&mut self, op: UnOp, x: &Expr) -> Result<Value, CodegenError> {
        match op {
            UnOp::Plus => self.emit_analog(x),
            UnOp::Neg => {
                let v = self.emit_analog(x)?;
                Ok(self.cse_op1(T_NEG, v, |b| b.ins().fneg(v)))
            }
            UnOp::LogNot => {
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
            other => Err(CodegenError::unsupported(format!("unary {other:?} in an analog expression"))),
        }
    }

    fn emit_analog_binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> Result<Value, CodegenError> {
        let ir_op = lower_binop_pom(op);
        let lhs = self.emit_analog(a)?;
        let rhs = self.emit_analog(b)?;
        if ir_op == crate::resolve::BinOp::Pow {
            return self.analog_call_math("pow", &[lhs, rhs]);
        }
        let key = CseKey::Op2(bin_tag(ir_op), lhs.as_u32(), rhs.as_u32());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return Ok(hit);
        }
        let cmp = |e: &mut Self, cc: FloatCC| {
            let flag = e.builder.ins().fcmp(cc, lhs, rhs);
            e.bool_to_f64(flag)
        };
        let val = match ir_op {
            crate::resolve::BinOp::Add => self.builder.ins().fadd(lhs, rhs),
            crate::resolve::BinOp::Sub => self.builder.ins().fsub(lhs, rhs),
            crate::resolve::BinOp::Mul => self.builder.ins().fmul(lhs, rhs),
            crate::resolve::BinOp::Div => self.builder.ins().fdiv(lhs, rhs),
            crate::resolve::BinOp::Rem => {
                let quotient = self.builder.ins().fdiv(lhs, rhs);
                let floored = self.analog_call_math("floor", &[quotient])?;
                let product = self.builder.ins().fmul(floored, rhs);
                self.builder.ins().fsub(lhs, product)
            }
            crate::resolve::BinOp::Eq => cmp(self, FloatCC::Equal),
            crate::resolve::BinOp::Ne => cmp(self, FloatCC::NotEqual),
            crate::resolve::BinOp::Lt => cmp(self, FloatCC::LessThan),
            crate::resolve::BinOp::Le => cmp(self, FloatCC::LessThanOrEqual),
            crate::resolve::BinOp::Gt => cmp(self, FloatCC::GreaterThan),
            crate::resolve::BinOp::Ge => cmp(self, FloatCC::GreaterThanOrEqual),
            crate::resolve::BinOp::And | crate::resolve::BinOp::Or => {
                let zero = self.cse_const(0.0);
                let a_true = self.builder.ins().fcmp(FloatCC::NotEqual, lhs, zero);
                let b_true = self.builder.ins().fcmp(FloatCC::NotEqual, rhs, zero);
                let combined = if ir_op == crate::resolve::BinOp::And {
                    self.builder.ins().band(a_true, b_true)
                } else {
                    self.builder.ins().bor(a_true, b_true)
                };
                self.bool_to_f64(combined)
            }
            crate::resolve::BinOp::BitAnd | crate::resolve::BinOp::BitOr
            | crate::resolve::BinOp::BitXor | crate::resolve::BinOp::BitXnor
            | crate::resolve::BinOp::Shl | crate::resolve::BinOp::Shr => {
                return Err(CodegenError::unsupported(format!("bitwise/shift {ir_op:?} in an analog expression")));
            }
            crate::resolve::BinOp::Pow => unreachable!("handled above"),
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
}
