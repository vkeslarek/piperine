//! Direct IR → Cranelift analog emitter.
//!
//! This is the IR-native replacement for the old `ir_expr_to_phdl` round-trip
//! (which silently collapsed unsupported [`IrExpr`] variants to `0.0`).  An
//! [`IrExpr`] is emitted straight to Cranelift, symbolically differentiated in
//! IR form for the Jacobian, and walked for branch collection.
//!
//! The [`AnalogExpr`] trait abstracts the three operations the shared Cranelift
//! residual/Jacobian skeleton in [`super::analog`] needs — `emit`, `diff`, and
//! `collect_branches` — so the same skeleton compiles both PHDL `Expr`
//! contributions (the `from_elab` path) and `IrExpr` contributions (the IR
//! front door) without duplication.
//!
//! ## Scope
//!
//! The emitter covers the full *algebraic* IR: literals, params, branch
//! voltages, all binary/unary operators, ternary `Select`, and built-in math
//! calls.  `StateRef` (the `ddt`/`idt` reactive operators) emits `0.0` here —
//! the resistive/DC contribution — and is stamped reactively elsewhere.
//!
//! Anything that has no meaning as a scalar analog quantity (concatenations,
//! arrays, bit-selects, unknown user calls, …) is rejected up-front by
//! [`validate_ir_contrib`] with a [`CodegenError::Unsupported`], so a model
//! never silently compiles to the wrong value.

use cranelift_codegen::ir::{condcodes::FloatCC, types::F64, InstBuilder, MemFlags, Value};


use super::cranelift_helpers::{emit_math, is_builtin_math, ExprCtx};
use super::CodegenError;
use crate::ir::{IrBinOp, IrExpr, IrUnOp, SimQuery};

fn branch_key(plus: &str, minus: &str) -> String {
    format!("V({plus},{minus})")
}

/// The three operations the shared Cranelift skeleton needs from a
/// contribution expression, regardless of whether it is a PHDL `Expr` or an
/// `IrExpr`.
pub trait AnalogExpr: Clone {
    /// Emit this expression as a scalar f64 Cranelift [`Value`].
    fn emit(&self, ctx: &mut ExprCtx) -> Value;
    /// Symbolically differentiate w.r.t. the branch voltage keyed by `wrt`
    /// (a canonical `"V(plus,minus)"` key).
    fn diff(&self, wrt: &str) -> Self;
    /// Collect every `V(a,b)` branch appearing in this expression.
    fn collect_branches(&self, out: &mut Vec<(String, String)>);
}

// ── IrExpr impl (IR front door) ───────────────────────────────────────────────

impl AnalogExpr for IrExpr {
    fn emit(&self, ctx: &mut ExprCtx) -> Value {
        emit_ir_expr(ctx, self)
    }
    fn diff(&self, wrt: &str) -> Self {
        diff_ir(self, wrt)
    }
    fn collect_branches(&self, out: &mut Vec<(String, String)>) {
        collect_branches_ir(self, out)
    }
}

// ── Emitter ───────────────────────────────────────────────────────────────────

/// Emit an [`IrExpr`] as a scalar f64 Cranelift [`Value`].
///
/// Assumes the expression has passed [`validate_ir_contrib`]; constructs that
/// would otherwise be unreachable fall back to `0.0`.
pub fn emit_ir_expr(ctx: &mut ExprCtx, e: &IrExpr) -> Value {
    match e {
        IrExpr::Real(v) => ctx.builder.ins().f64const(*v),
        IrExpr::Int(v) => ctx.builder.ins().f64const(*v as f64),
        IrExpr::Bool(b) => ctx.builder.ins().f64const(if *b { 1.0 } else { 0.0 }),

        // Params and surviving local vars both resolve from the param array;
        // an unresolved name emits 0.0 (same convention as the PHDL emitter).
        IrExpr::Param(name) | IrExpr::Var(name) => match ctx.param_values.get(name.as_str()) {
            Some(&v) => v,
            None => ctx.builder.ins().f64const(0.0),
        },

        IrExpr::BranchAccess { access, plus, minus } => {
            if access == "V" {
                let key = branch_key(plus, minus);
                match ctx.branch_voltages.get(&key) {
                    Some(&v) => v,
                    None => ctx.builder.ins().f64const(0.0),
                }
            } else {
                // I(a,b) and other flows are not available in the KCL stamp
                // context; their reactive/source handling lives elsewhere.
                ctx.builder.ins().f64const(0.0)
            }
        }

        // ddt/idt reactive operators: the resistive (DC) part is 0.
        IrExpr::StateRef(_) => ctx.builder.ins().f64const(0.0),

        IrExpr::Sim(sq) => emit_sim(ctx, sq),

        IrExpr::Unary(op, x) => emit_unary(ctx, *op, x),
        IrExpr::Binary(op, a, b) => emit_binary(ctx, *op, a, b),

        IrExpr::Select(c, t, f) => {
            let cv = emit_ir_expr(ctx, c);
            let zero = ctx.builder.ins().f64const(0.0);
            let cond = ctx.builder.ins().fcmp(FloatCC::NotEqual, cv, zero);
            let tv = emit_ir_expr(ctx, t);
            let fv = emit_ir_expr(ctx, f);
            ctx.builder.ins().select(cond, tv, fv)
        }

        IrExpr::Call(name, args) => emit_call(ctx, name, args),

        // Validated-out elsewhere.
        _ => ctx.builder.ins().f64const(0.0),
    }
}

fn emit_sim(ctx: &mut ExprCtx, sq: &SimQuery) -> Value {
    // Threaded from the live `SimCtx` (`sim_ctx` pointer on the JIT stack).
    // Layout (see codegen::SimCtx):
    //   offset 0: temperature (Kelvin)
    //   offset 8: abstime (seconds)
    //   offset 16: mfactor
    //   offset 24: gmin
    //   offset 32: step        (NEW — reserved for A.14 stage 2)
    //   offset 40: tfinal      (NEW — reserved for A.14 stage 2)
    //
    // GAPS §A.2 + §A.3 — these used to silently emit 0.0 (or a hardcoded
    // 0.025852 for `Vt`); now they read the live simulator state.
    //
    // GAPS §A.14 — `$simparam("gmin", d)` reads from SimCtx for known keys
    // (`gmin`, `temperature`); unknown keys fall back to the default
    // expression provided as the second argument to `$simparam`.
    //
    // GAPS §A.15 — `$param_given(...)` still requires per-instance
    // metadata threading (not wired yet, GAPS §A.15). The validator
    // currently rejects it; the emitter's `_ => 0.0` arm is the
    // unreachable-safeguard.
    //
    // `FuncInstBuilder` is not `Copy`, so we call `ctx.builder.ins()` for
    // each emission rather than binding a single builder reference.
    match sq {
        SimQuery::Temperature => {
            // (*sim_ctx).temperature — offset 0.
            ctx.builder.ins().load(F64, MemFlags::trusted(), ctx.sim_ctx, 0)
        }
        SimQuery::Abstime => {
            // (*sim_ctx).abstime — offset 8.
            ctx.builder.ins().load(F64, MemFlags::trusted(), ctx.sim_ctx, 8)
        }
        SimQuery::Mfactor => {
            // (*sim_ctx).mfactor — offset 16.
            ctx.builder.ins().load(F64, MemFlags::trusted(), ctx.sim_ctx, 16)
        }
        SimQuery::Vt(t_opt) => {
            // vt = kT/q where T = (*sim_ctx).temperature (or the optional
            // argument if present).
            let temp = ctx.builder.ins().load(F64, MemFlags::trusted(), ctx.sim_ctx, 0);
            let temp = match t_opt {
                Some(arg_expr) => {
                    // Multiply by optional arg expr (used as a scaling factor).
                    let arg = emit_ir_expr(ctx, arg_expr);
                    ctx.builder.ins().fmul(temp, arg)
                }
                None => temp,
            };
            let kb_over_q = ctx.builder.ins().f64const(super::super::SimCtx::K_B_OVER_Q_EV_PER_K);
            ctx.builder.ins().fmul(temp, kb_over_q)
        }
        SimQuery::Simparam { key, default } => {
            // $simparam reads known solver-side state from SimCtx; unknown
            // keys fall back to the default expr. The validator at the
            // bottom of this file accepts all keys (the fall-back is the
            // canonical behaviour). GAPS §A.14.
            match key.as_str() {
                "gmin" | "temperature" => {
                    // offset 24 (gmin); for "temperature" alias, use offset 0.
                    let off = if key == "temperature" { 0 } else { 24 };
                    ctx.builder.ins().load(F64, MemFlags::trusted(), ctx.sim_ctx, off)
                }
                // "step"/"tfinal" are reserved at offsets 32/40 — solver
                // does not yet write them (GAPS §A.14 stage 2). Fall back
                // to default.
                "step" | "tfinal" | _ => emit_ir_expr(ctx, default),
            }
        }
        // Simparam and the rest are not yet wired — fall through to a
        // clear error via the validator; emitter's `_ => 0.0` arm is the
        // safety net for unvalidated code paths.
        _ => ctx.builder.ins().f64const(0.0),
    }
}

fn emit_unary(ctx: &mut ExprCtx, op: IrUnOp, x: &IrExpr) -> Value {
    let v = emit_ir_expr(ctx, x);
    match op {
        IrUnOp::Neg => ctx.builder.ins().fneg(v),
        IrUnOp::Not => {
            let zero = ctx.builder.ins().f64const(0.0);
            let is_zero = ctx.builder.ins().fcmp(FloatCC::Equal, v, zero);
            bool_to_f64(ctx, is_zero)
        }
        // Bitwise / reduction unops are validated out.
        _ => ctx.builder.ins().f64const(0.0),
    }
}

fn emit_binary(ctx: &mut ExprCtx, op: IrBinOp, a: &IrExpr, b: &IrExpr) -> Value {
    // Pow lowers to a libm call rather than an arithmetic instruction.
    if op == IrBinOp::Pow {
        let l = emit_ir_expr(ctx, a);
        let r = emit_ir_expr(ctx, b);
        return emit_math(ctx, "pow", l, r).expect("pow is a builtin");
    }

    let l = emit_ir_expr(ctx, a);
    let r = emit_ir_expr(ctx, b);
    match op {
        IrBinOp::Add => ctx.builder.ins().fadd(l, r),
        IrBinOp::Sub => ctx.builder.ins().fsub(l, r),
        IrBinOp::Mul => ctx.builder.ins().fmul(l, r),
        IrBinOp::Div => ctx.builder.ins().fdiv(l, r),
        IrBinOp::Rem => {
            // fmod: a - floor(a/b)*b
            let q = ctx.builder.ins().fdiv(l, r);
            let fl = emit_math(ctx, "floor", q, q).expect("floor is a builtin");
            let prod = ctx.builder.ins().fmul(fl, r);
            ctx.builder.ins().fsub(l, prod)
        }
        IrBinOp::Eq => cmp(ctx, FloatCC::Equal, l, r),
        IrBinOp::Ne => cmp(ctx, FloatCC::NotEqual, l, r),
        IrBinOp::Lt => cmp(ctx, FloatCC::LessThan, l, r),
        IrBinOp::Le => cmp(ctx, FloatCC::LessThanOrEqual, l, r),
        IrBinOp::Gt => cmp(ctx, FloatCC::GreaterThan, l, r),
        IrBinOp::Ge => cmp(ctx, FloatCC::GreaterThanOrEqual, l, r),
        IrBinOp::And => logical(ctx, l, r, true),
        IrBinOp::Or => logical(ctx, l, r, false),
        // Bitwise / shift ops are validated out.
        _ => ctx.builder.ins().f64const(0.0),
    }
}

fn emit_call(ctx: &mut ExprCtx, name: &str, args: &[IrExpr]) -> Value {
    let zero = ctx.builder.ins().f64const(0.0);
    let a0 = args.first().map(|a| emit_ir_expr(ctx, a)).unwrap_or(zero);
    let a1 = args.get(1).map(|a| emit_ir_expr(ctx, a)).unwrap_or(zero);
    emit_math(ctx, name, a0, a1).unwrap_or_else(|| ctx.builder.ins().f64const(0.0))
}

fn cmp(ctx: &mut ExprCtx, cc: FloatCC, a: Value, b: Value) -> Value {
    let flag = ctx.builder.ins().fcmp(cc, a, b);
    bool_to_f64(ctx, flag)
}

fn logical(ctx: &mut ExprCtx, a: Value, b: Value, and: bool) -> Value {
    let zero = ctx.builder.ins().f64const(0.0);
    let an = ctx.builder.ins().fcmp(FloatCC::NotEqual, a, zero);
    let bn = ctx.builder.ins().fcmp(FloatCC::NotEqual, b, zero);
    let combined = if and {
        ctx.builder.ins().band(an, bn)
    } else {
        ctx.builder.ins().bor(an, bn)
    };
    bool_to_f64(ctx, combined)
}

fn bool_to_f64(ctx: &mut ExprCtx, flag: Value) -> Value {
    let one = ctx.builder.ins().f64const(1.0);
    let zero = ctx.builder.ins().f64const(0.0);
    ctx.builder.ins().select(flag, one, zero)
}

// ── Branch collection ─────────────────────────────────────────────────────────

/// Collect every `V(a,b)` branch appearing in an [`IrExpr`].
pub fn collect_branches_ir(e: &IrExpr, out: &mut Vec<(String, String)>) {
    match e {
        IrExpr::BranchAccess { access, plus, minus } if access == "V" => {
            let pair = (plus.clone(), minus.clone());
            if !out.contains(&pair) {
                out.push(pair);
            }
        }
        IrExpr::Unary(_, x) => collect_branches_ir(x, out),
        IrExpr::Binary(_, a, b) => {
            collect_branches_ir(a, out);
            collect_branches_ir(b, out);
        }
        IrExpr::Select(c, t, f) => {
            collect_branches_ir(c, out);
            collect_branches_ir(t, out);
            collect_branches_ir(f, out);
        }
        IrExpr::Call(_, args) => {
            for a in args {
                collect_branches_ir(a, out);
            }
        }
        _ => {}
    }
}

// ── Symbolic differentiation over IrExpr ──────────────────────────────────────

/// Differentiate `e` w.r.t. the branch voltage keyed by `wrt`.
///
/// Mirrors the PHDL [`autodiff::diff`] rules but stays in IR form.  Reactive
/// operators (`StateRef`) and non-`V` accesses have derivative 0 here.
pub fn diff_ir(e: &IrExpr, wrt: &str) -> IrExpr {
    match e {
        IrExpr::Real(_) | IrExpr::Int(_) | IrExpr::Bool(_) | IrExpr::String(_)
        | IrExpr::Quad(_) | IrExpr::Param(_) | IrExpr::Var(_) | IrExpr::Sim(_)
        | IrExpr::StateRef(_) => lit(0.0),

        IrExpr::BranchAccess { access, plus, minus } => {
            if access == "V" && branch_key(plus, minus) == wrt {
                lit(1.0)
            } else {
                lit(0.0)
            }
        }

        IrExpr::Unary(IrUnOp::Neg, x) => neg(diff_ir(x, wrt)),
        IrExpr::Unary(_, _) => lit(0.0),

        IrExpr::Binary(op, a, b) => diff_binary(*op, a, b, wrt),

        IrExpr::Select(c, t, f) => IrExpr::Select(
            c.clone(),
            Box::new(diff_ir(t, wrt)),
            Box::new(diff_ir(f, wrt)),
        ),

        IrExpr::Call(name, args) => diff_call(name, args, wrt),

        // No meaningful derivative (validated out of contributions anyway).
        _ => lit(0.0),
    }
}

fn diff_binary(op: IrBinOp, a: &IrExpr, b: &IrExpr, wrt: &str) -> IrExpr {
    let du = diff_ir(a, wrt);
    let dv = diff_ir(b, wrt);
    match op {
        IrBinOp::Add => add(du, dv),
        IrBinOp::Sub => sub(du, dv),
        // (u*v)' = u'v + uv'
        IrBinOp::Mul => add(mul(du, b.clone()), mul(a.clone(), dv)),
        // (u/v)' = (u'v − uv') / v²
        IrBinOp::Div => div(
            sub(mul(du, b.clone()), mul(a.clone(), dv)),
            mul(b.clone(), b.clone()),
        ),
        // pow(u, v)' ≈ v * pow(u, v-1) * u'  (constant-exponent common case)
        IrBinOp::Pow => mul(
            mul(
                b.clone(),
                IrExpr::Binary(
                    IrBinOp::Pow,
                    Box::new(a.clone()),
                    Box::new(sub(b.clone(), lit(1.0))),
                ),
            ),
            du,
        ),
        // Comparisons / logical / remainder: derivative 0 almost everywhere.
        _ => lit(0.0),
    }
}

fn diff_call(name: &str, args: &[IrExpr], wrt: &str) -> IrExpr {
    let u = args.first().cloned().unwrap_or_else(|| lit(0.0));
    let du = args.first().map(|a| diff_ir(a, wrt)).unwrap_or_else(|| lit(0.0));
    match name {
        "exp" | "limexp" => mul(call1("exp", u), du),
        "ln" | "log" => div(du, u),
        "log10" => div(du, mul(u, lit(std::f64::consts::LN_10))),
        "sqrt" => div(du, mul(lit(2.0), call1("sqrt", u))),
        "sin" => mul(call1("cos", u), du),
        "cos" => mul(neg(call1("sin", u)), du),
        "tan" => div(du, mul(call1("cos", u.clone()), call1("cos", u))),
        "asin" => div(du, call1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        "acos" => div(neg(du), call1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        "atan" => div(du, add(lit(1.0), mul(u.clone(), u))),
        // (|u|)' = sign(u) * u'
        "abs" => mul(
            IrExpr::Select(
                Box::new(IrExpr::Binary(IrBinOp::Ge, Box::new(u), Box::new(lit(0.0)))),
                Box::new(lit(1.0)),
                Box::new(lit(-1.0)),
            ),
            du,
        ),
        "pow" => {
            let v = args.get(1).cloned().unwrap_or_else(|| lit(1.0));
            mul(
                mul(v.clone(), call2("pow", u.clone(), sub(v, lit(1.0)))),
                du,
            )
        }
        // floor/ceil/min/max and unknown calls: 0 almost everywhere.
        _ => lit(0.0),
    }
}

// ── Smart constructors (constant-folding) for IrExpr ──────────────────────────

fn lit(v: f64) -> IrExpr {
    IrExpr::Real(v)
}

fn is_lit(e: &IrExpr, v: f64) -> bool {
    matches!(e, IrExpr::Real(x) if *x == v)
}

fn add(a: IrExpr, b: IrExpr) -> IrExpr {
    if is_lit(&a, 0.0) {
        return b;
    }
    if is_lit(&b, 0.0) {
        return a;
    }
    IrExpr::Binary(IrBinOp::Add, Box::new(a), Box::new(b))
}

fn sub(a: IrExpr, b: IrExpr) -> IrExpr {
    if is_lit(&b, 0.0) {
        return a;
    }
    IrExpr::Binary(IrBinOp::Sub, Box::new(a), Box::new(b))
}

fn mul(a: IrExpr, b: IrExpr) -> IrExpr {
    if is_lit(&a, 0.0) || is_lit(&b, 0.0) {
        return lit(0.0);
    }
    if is_lit(&a, 1.0) {
        return b;
    }
    if is_lit(&b, 1.0) {
        return a;
    }
    IrExpr::Binary(IrBinOp::Mul, Box::new(a), Box::new(b))
}

fn div(a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::Binary(IrBinOp::Div, Box::new(a), Box::new(b))
}

fn neg(a: IrExpr) -> IrExpr {
    if let IrExpr::Real(v) = &a {
        return lit(-v);
    }
    IrExpr::Unary(IrUnOp::Neg, Box::new(a))
}

fn call1(name: &str, a: IrExpr) -> IrExpr {
    IrExpr::Call(name.to_string(), vec![a])
}

fn call2(name: &str, a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::Call(name.to_string(), vec![a, b])
}

// ── Validation (fail loud, never silent 0.0) ──────────────────────────────────

/// Reject any [`IrExpr`] construct that cannot be faithfully lowered to a
/// scalar analog quantity, so a model never silently compiles to a wrong
/// value.  Returns `Ok` for everything the emitter handles correctly.
///
/// `known_names` is the set of param/var names that the emitter will
/// resolve via the param_values map. Any `Param(name)` or `Var(name)` not
/// in this set is rejected (GAPS §A.8). The single-argument form
/// `validate_ir_contrib(e)` defaults to accepting every Param/Var name
/// for callers that have not threaded the known-names set yet (these
/// callers typically never exercise the path — see GAPS §K.1 for the
/// plan to deprecate the from_elab path entirely).
pub fn validate_ir_contrib(e: &IrExpr) -> Result<(), CodegenError> {
    validate_ir_contrib_with(e, None)
}

/// Like [`validate_ir_contrib`] but with the set of known param/var
/// names. Names not in the set produce `CodegenError::Unsupported` with a
/// clear message — never a silent 0.0 (GAPS §A.8).
///
/// Also rejects `V(plus, minus)` where `plus` or `minus` is not in the
/// module's terminal set (GAPS §A.9): unknown terminals used to read as
/// 0 via `f64const(0.0)`.
pub fn validate_ir_contrib_with(
    e: &IrExpr,
    known_names: Option<&std::collections::HashSet<String>>,
) -> Result<(), CodegenError> {
    validate_ir_contrib_with2(e, known_names, None)
}

/// Same as [`validate_ir_contrib_with`] but with both known param/var
/// names AND known terminal names. `known_terminals` is the union of the
/// module's ports, wires, and grounds.
pub fn validate_ir_contrib_with2(
    e: &IrExpr,
    known_names: Option<&std::collections::HashSet<String>>,
    known_terminals: Option<&std::collections::HashSet<String>>,
) -> Result<(), CodegenError> {
    match e {
        IrExpr::Real(_) | IrExpr::Int(_) | IrExpr::Bool(_) | IrExpr::StateRef(_) => Ok(()),

        IrExpr::Param(name) | IrExpr::Var(name) => {
            match known_names {
                Some(set) if !set.contains(name) => Err(unsupported(format!(
                    "unresolved name `{name}` in analog body (GAPS §A.8)"
                ))),
                _ => Ok(()),
            }
        }

        IrExpr::BranchAccess { access, plus, minus } => {
            // A.1: only `V(...)` reads are supported inside an analog
            // contribution expression. Flow (`I(...)`) reads require a
            // voltage-source branch-current unknown in the solver (H.4),
            // which is not yet wired up. Fail loud so users are not lied
            // to with a silent zero.
            //
            // If you need a controlled source today, use an indirect
            // contribution: `I(cp, cm) : V(pp, pm) = expr;`.
            if access != "V" {
                return Err(unsupported(format!(
                    "reading branch access `{access}(...)` inside a contribution is not yet \
                     supported; use an indirect contribution `I(cp,cm) : V(pp,pm) = expr` \
                     instead (see docs/GAPS.md §A.1)"
                )));
            }
            // A.9: reject unknown terminal names. The literal "0" is the
            // implicit ground reference used by single-arg `V(a)` —
            // always allow it. (Analog ref nodes are the implicit
            // 0V reference in MNA.)
            if let Some(set) = known_terminals {
                if plus != "0" && !set.contains(plus) {
                    return Err(unsupported(format!(
                        "unknown terminal `{plus}` in V({plus}, {minus}) (GAPS §A.9)"
                    )));
                }
                if minus != "0" && !set.contains(minus) {
                    return Err(unsupported(format!(
                        "unknown terminal `{minus}` in V({plus}, {minus}) (GAPS §A.9)"
                    )));
                }
            }
            Ok(())
        }

        IrExpr::Sim(sq) => match sq {
            SimQuery::Temperature
            | SimQuery::Vt(_)
            | SimQuery::Abstime
            | SimQuery::Mfactor
            | SimQuery::Simparam { .. } => Ok(()),
            SimQuery::ParamGiven(_) => {
                // GAPS §A.15 — `$param_given` reads per-instance metadata
                // that must be threaded through elaboration. The per-instance
                // bitmask (`Device::param_given`) is not yet wired into the
                // JIT path. Fail loud so users are not lied to.
                Err(unsupported(
                    "$param_given requires per-instance param-given metadata \
                     threading — GAPS §A.15 (not yet implemented)",
                ))
            }
            other => Err(unsupported(format!("simulator query {other:?}"))),
        },

        IrExpr::Unary(op, x) => match op {
            IrUnOp::Neg | IrUnOp::Not => {
                validate_ir_contrib_with2(x, known_names, known_terminals)
            }
            _ => Err(unsupported(format!("unary operator {op:?}"))),
        },

        IrExpr::Binary(op, a, b) => match op {
            IrBinOp::BitAnd | IrBinOp::BitOr | IrBinOp::BitXor | IrBinOp::Shl
            | IrBinOp::Shr | IrBinOp::AShl | IrBinOp::AShr => {
                Err(unsupported(format!("bitwise/shift operator {op:?}")))
            }
            _ => {
                validate_ir_contrib_with2(a, known_names, known_terminals)?;
                validate_ir_contrib_with2(b, known_names, known_terminals)
            }
        },

        IrExpr::Select(c, t, f) => {
            validate_ir_contrib_with2(c, known_names, known_terminals)?;
            validate_ir_contrib_with2(t, known_names, known_terminals)?;
            validate_ir_contrib_with2(f, known_names, known_terminals)
        }

        IrExpr::Call(name, args) => {
            if !is_builtin_math(name) {
                return Err(unsupported(format!("call to non-builtin `{name}`")));
            }
            for a in args {
                validate_ir_contrib_with2(a, known_names, known_terminals)?;
            }
            Ok(())
        }

        other => Err(unsupported(format!("{other:?}"))),
    }
}

fn unsupported(what: impl Into<String>) -> CodegenError {
    CodegenError::Unsupported(what.into())
}
