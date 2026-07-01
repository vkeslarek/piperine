//! Shared Cranelift utility helpers (no PHDL dependency).
//!
//! These are the minimum functions needed by the IR → Cranelift emitter
//! (`ir_emit.rs`) and the analog codegen (`analog.rs`). They deal only
//! with Cranelift types (`Value`, `FunctionBuilder`, etc.) — no
//! PHDL `Expr` or `BehaviorStmt` types.

use std::collections::HashMap;
use cranelift_codegen::ir::{condcodes::FloatCC, types::F64, FuncRef, InstBuilder, MemFlags, Value};
use cranelift_frontend::FunctionBuilder;

/// Context passed through expression emission.
pub struct ExprCtx<'b, 'f: 'b> {
    pub builder: &'b mut FunctionBuilder<'f>,
    pub branch_voltages: &'b HashMap<String, Value>,
    pub param_values: &'b HashMap<String, Value>,
    pub libm: &'b HashMap<&'static str, FuncRef>,
    pub sim_ctx: cranelift_codegen::ir::Value,
}

/// Emit a built-in math call (`exp`, `ln`, `sqrt`, …).
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

pub fn fcmp_to_float(ctx: &mut ExprCtx, cc: FloatCC, a: Value, b: Value) -> Value {
    let flag = ctx.builder.ins().fcmp(cc, a, b);
    bool_to_f64(ctx, flag)
}

pub fn emit_libm1(ctx: &mut ExprCtx, name: &'static str, a: Value) -> Value {
    let fref = ctx.libm.get(name).unwrap_or_else(|| panic!("libm '{}' not declared", name));
    let call = ctx.builder.ins().call(*fref, &[a]);
    ctx.builder.inst_results(call)[0]
}

pub fn emit_libm2(ctx: &mut ExprCtx, name: &'static str, a: Value, b: Value) -> Value {
    let fref = ctx.libm.get(name).unwrap_or_else(|| panic!("libm '{}' not declared", name));
    let call = ctx.builder.ins().call(*fref, &[a, b]);
    ctx.builder.inst_results(call)[0]
}

pub fn bool_to_f64(ctx: &mut ExprCtx, flag: Value) -> Value {
    let one = ctx.builder.ins().f64const(1.0);
    let zero = ctx.builder.ins().f64const(0.0);
    ctx.builder.ins().select(flag, one, zero)
}

/// Load an f64 at offset `idx` from a base pointer.
pub fn load_f64(builder: &mut FunctionBuilder, ptr: Value, idx: usize) -> Value {
    builder.ins().load(F64, MemFlags::trusted(), ptr, (idx * 8) as i32)
}

/// Accumulate `delta` into `out_ptr[idx]`.
pub fn accumulate_f64(builder: &mut FunctionBuilder, delta: Value, ptr: Value, idx: usize) {
    let curr = load_f64(builder, ptr, idx);
    let sum = builder.ins().fadd(curr, delta);
    builder.ins().store(MemFlags::trusted(), sum, ptr, (idx * 8) as i32);
}