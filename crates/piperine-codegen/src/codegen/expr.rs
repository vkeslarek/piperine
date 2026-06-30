//! PHDL [`Expr`] → Cranelift [`Value`] emitter.

use std::collections::HashMap;

use cranelift_codegen::ir::{
    condcodes::FloatCC, types::F64, FuncRef, InstBuilder, MemFlags, Value,
};
use cranelift_frontend::FunctionBuilder;

use piperine_lang::parse::ast::{BinaryOp, Expr, Literal, UnaryOp};

/// Context passed through expression emission.
pub struct ExprCtx<'b, 'f: 'b> {
    pub builder: &'b mut FunctionBuilder<'f>,
    /// Branch key ("V(plus,minus)") → precomputed voltage Value.
    pub branch_voltages: &'b HashMap<String, Value>,
    /// Param name → precomputed Value (loaded from param array).
    pub param_values: &'b HashMap<String, Value>,
    /// libm function name → FuncRef.
    pub libm: &'b HashMap<&'static str, FuncRef>,
    /// Pointer to the live `SimCtx` struct (temperature, abstime, …) for
    /// `$temperature`/`$abstime`/`$vt` reads. See GAPS §A.2, §A.3.
    pub sim_ctx: cranelift_codegen::ir::Value,
}

/// Emit a PHDL expression as a Cranelift f64 Value.
///
/// Non-f64 constructs (arrays, lambdas, boolean-only) return 0.0.
pub fn emit_phdl_expr(ctx: &mut ExprCtx, expr: &Expr) -> Value {
    match expr {
        Expr::Literal(lit) => emit_literal(ctx, lit),

        Expr::Ident(name) => {
            if let Some(&v) = ctx.param_values.get(name.as_str()) {
                v
            } else {
                ctx.builder.ins().f64const(0.0)
            }
        }

        Expr::Unary(UnaryOp::Neg, inner) => {
            let v = emit_phdl_expr(ctx, inner);
            ctx.builder.ins().fneg(v)
        }
        Expr::Unary(UnaryOp::Not, _) => ctx.builder.ins().f64const(0.0),

        Expr::Binary(lhs, op, rhs) => emit_binary(ctx, lhs, op, rhs),

        Expr::Call(func, args) => emit_call(ctx, func, args),

        Expr::If { cond, then_body, else_body } => {
            let c = emit_phdl_expr(ctx, cond);
            let zero = ctx.builder.ins().f64const(0.0);
            let is_nonzero = ctx.builder.ins().fcmp(FloatCC::NotEqual, c, zero);
            let t_val = block_value(ctx, then_body);
            let e_val = block_value(ctx, else_body);
            ctx.builder.ins().select(is_nonzero, t_val, e_val)
        }

        Expr::Block(block) => block_value(ctx, block),

        // Not meaningful in scalar analog context
        Expr::Path(_) | Expr::SysCall(_, _) | Expr::Array(_)
        | Expr::Lambda { .. } | Expr::BundleLit { .. }
        | Expr::Index(_, _) | Expr::Slice(_, _) | Expr::Field(_, _) => {
            ctx.builder.ins().f64const(0.0)
        }
    }
}

fn emit_literal(ctx: &mut ExprCtx, lit: &Literal) -> Value {
    match lit {
        Literal::Real(v) => ctx.builder.ins().f64const(*v),
        Literal::Int(n)  => ctx.builder.ins().f64const(*n as f64),
        Literal::Bool(b) => ctx.builder.ins().f64const(if *b { 1.0 } else { 0.0 }),
        _                => ctx.builder.ins().f64const(0.0),
    }
}

fn emit_binary(ctx: &mut ExprCtx, lhs: &Expr, op: &BinaryOp, rhs: &Expr) -> Value {
    let l = emit_phdl_expr(ctx, lhs);
    let r = emit_phdl_expr(ctx, rhs);
    let ins = ctx.builder.ins();
    match op {
        BinaryOp::Add => ins.fadd(l, r),
        BinaryOp::Sub => ins.fsub(l, r),
        BinaryOp::Mul => ins.fmul(l, r),
        BinaryOp::Div => ins.fdiv(l, r),
        BinaryOp::Rem => {
            // fmod via libm: a - floor(a/b)*b
            let q = ins.fdiv(l, r);
            let fl = emit_libm1(ctx, "floor", q);
            let sub = ctx.builder.ins().fmul(fl, r);
            ctx.builder.ins().fsub(l, sub)
        }
        BinaryOp::Eq  => fcmp_to_float(ctx, FloatCC::Equal, l, r),
        BinaryOp::Neq => fcmp_to_float(ctx, FloatCC::NotEqual, l, r),
        BinaryOp::Lt  => fcmp_to_float(ctx, FloatCC::LessThan, l, r),
        BinaryOp::Le  => fcmp_to_float(ctx, FloatCC::LessThanOrEqual, l, r),
        BinaryOp::Gt  => fcmp_to_float(ctx, FloatCC::GreaterThan, l, r),
        BinaryOp::Ge  => fcmp_to_float(ctx, FloatCC::GreaterThanOrEqual, l, r),
        // Bitwise on floats: not meaningful
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
            ctx.builder.ins().f64const(0.0)
        }
    }
}

fn emit_call(ctx: &mut ExprCtx, func: &Expr, args: &[Expr]) -> Value {
    let fname = match func {
        Expr::Ident(n) => n.as_str(),
        _ => return ctx.builder.ins().f64const(0.0),
    };

    // Branch voltage V(plus, minus)
    if fname == "V" {
        if let (Some(Expr::Ident(a)), Some(Expr::Ident(b))) = (args.first(), args.get(1)) {
            let key = super::autodiff::branch_key(a, b);
            if let Some(&v) = ctx.branch_voltages.get(&key) {
                return v;
            }
        }
        return ctx.builder.ins().f64const(0.0);
    }

    // Branch current I(a,b) — not available in KCL stamp context
    if fname == "I" {
        return ctx.builder.ins().f64const(0.0);
    }

    // ddt / idt — zero for DC/steady-state
    if fname == "ddt" || fname == "idt" {
        return ctx.builder.ins().f64const(0.0);
    }

    // Math functions
    let zero = ctx.builder.ins().f64const(0.0);
    let a0 = args.first().map(|a| emit_phdl_expr(ctx, a)).unwrap_or(zero);
    let a1 = args.get(1).map(|a| emit_phdl_expr(ctx, a)).unwrap_or(zero);

    emit_math(ctx, fname, a0, a1).unwrap_or_else(|| ctx.builder.ins().f64const(0.0))
}

/// Shared math-function dispatch over already-emitted argument [`Value`]s.
///
/// Returns `None` for names that are not recognised built-in math functions,
/// so callers can decide how to handle user functions / unsupported names.
/// Used by both the PHDL emitter ([`emit_phdl_expr`]) and the IR emitter.
pub fn emit_math(ctx: &mut ExprCtx, name: &str, a0: Value, a1: Value) -> Option<Value> {
    let v = match name {
        "exp"   => emit_libm1(ctx, "exp",   a0),
        "ln"    => emit_libm1(ctx, "log",   a0),
        "log"   => emit_libm1(ctx, "log",   a0),
        "log10" => emit_libm1(ctx, "log10", a0),
        "sqrt"  => emit_libm1(ctx, "sqrt",  a0),
        "abs"   => emit_libm1(ctx, "fabs",  a0),
        "sin"   => emit_libm1(ctx, "sin",   a0),
        "cos"   => emit_libm1(ctx, "cos",   a0),
        "tan"   => emit_libm1(ctx, "tan",   a0),
        "asin"  => emit_libm1(ctx, "asin",  a0),
        "acos"  => emit_libm1(ctx, "acos",  a0),
        "atan"  => emit_libm1(ctx, "atan",  a0),
        "atan2" => emit_libm2(ctx, "atan2", a0, a1),
        "pow"   => emit_libm2(ctx, "pow",   a0, a1),
        "min"   => emit_libm2(ctx, "fmin",  a0, a1),
        "max"   => emit_libm2(ctx, "fmax",  a0, a1),
        "floor" => emit_libm1(ctx, "floor", a0),
        "ceil"  => emit_libm1(ctx, "ceil",  a0),
        // limexp: exp(min(u, 80))
        "limexp" => {
            let cap = ctx.builder.ins().f64const(80.0);
            let clamped = emit_libm2(ctx, "fmin", a0, cap);
            emit_libm1(ctx, "exp", clamped)
        }
        _ => return None,
    };
    Some(v)
}

/// True if `name` is a built-in math function understood by [`emit_math`].
pub fn is_builtin_math(name: &str) -> bool {
    matches!(
        name,
        "exp" | "ln" | "log" | "log10" | "sqrt" | "abs" | "sin" | "cos" | "tan"
            | "asin" | "acos" | "atan" | "atan2" | "pow" | "min" | "max"
            | "floor" | "ceil" | "limexp"
    )
}

/// Evaluate the value of a block (trailing expr or last Stmt::Return/Expr).
fn block_value(ctx: &mut ExprCtx, block: &piperine_lang::parse::ast::Block) -> Value {
    use piperine_lang::parse::ast::Stmt;
    if let Some(e) = &block.expr {
        return emit_phdl_expr(ctx, e);
    }
    match block.stmts.last() {
        Some(Stmt::Expr(e)) | Some(Stmt::Return(e)) => emit_phdl_expr(ctx, e),
        _ => ctx.builder.ins().f64const(0.0),
    }
}

fn fcmp_to_float(ctx: &mut ExprCtx, cc: FloatCC, a: Value, b: Value) -> Value {
    let flag = ctx.builder.ins().fcmp(cc, a, b);
    let one  = ctx.builder.ins().f64const(1.0);
    let zero = ctx.builder.ins().f64const(0.0);
    ctx.builder.ins().select(flag, one, zero)
}

// ── libm call helpers ─────────────────────────────────────────────────────────

pub fn emit_libm1(ctx: &mut ExprCtx, name: &'static str, a: Value) -> Value {
    let fref = *ctx.libm.get(name)
        .unwrap_or_else(|| panic!("libm '{}' not declared", name));
    let inst = ctx.builder.ins().call(fref, &[a]);
    ctx.builder.inst_results(inst)[0]
}

pub fn emit_libm2(ctx: &mut ExprCtx, name: &'static str, a: Value, b: Value) -> Value {
    let fref = *ctx.libm.get(name)
        .unwrap_or_else(|| panic!("libm '{}' not declared", name));
    let inst = ctx.builder.ins().call(fref, &[a, b]);
    ctx.builder.inst_results(inst)[0]
}

// ── Memory access helpers (used by analog.rs) ────────────────────────────────

/// Load f64 from `ptr[idx]` (8-byte stride).
pub fn load_f64(builder: &mut FunctionBuilder, ptr: Value, idx: usize) -> Value {
    builder.ins().load(F64, MemFlags::trusted(), ptr, (idx * 8) as i32)
}

/// `ptr[idx] += delta`.
pub fn accumulate_f64(builder: &mut FunctionBuilder, delta: Value, ptr: Value, idx: usize) {
    let old = load_f64(builder, ptr, idx);
    let new = builder.ins().fadd(old, delta);
    builder.ins().store(MemFlags::trusted(), new, ptr, (idx * 8) as i32);
}
