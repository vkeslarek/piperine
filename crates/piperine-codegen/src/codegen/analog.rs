//! Cranelift JIT compilation of PHDL analog behavior blocks.
//!
//! Compiles two `extern "C"` functions per analog module:
//!
//! - `residual(node_voltages, params, rhs)` — accumulates KCL current contributions.
//! - `jacobian(node_voltages, params, jac)` — accumulates conductance stamps.
//!
//! Both functions use the same signature:
//! ```c
//! void fn(*const f64 node_voltages, *const f64 params, *mut f64 out);
//! ```
//! `node_voltages[i]` is the voltage at terminal `i` (GND = 0.0 by convention).
//! `params[i]` is the value of the i-th declared `param` of the module.
//! `out[i]` accumulates into rhs (residual) or row-major jac (jacobian).

use std::collections::HashMap;

use cranelift_codegen::ir::{types::F64, AbiParam, FuncRef, Function, InstBuilder, Signature};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use piperine_lang::elab::ir::{ElabBehaviorStmt, ElabProgram};
use piperine_lang::parse::ast::{BindOp, Expr};

use super::autodiff::{branch_key, collect_branches, diff};
use super::expr::{accumulate_f64, emit_phdl_expr, load_f64, ExprCtx};
use super::{CodegenError, JitAnalogDevice};

// ── libm wrappers ─────────────────────────────────────────────────────────────

mod libm_wrappers {
    pub extern "C" fn sin(x: f64) -> f64   { x.sin() }
    pub extern "C" fn cos(x: f64) -> f64   { x.cos() }
    pub extern "C" fn tan(x: f64) -> f64   { x.tan() }
    pub extern "C" fn asin(x: f64) -> f64  { x.asin() }
    pub extern "C" fn acos(x: f64) -> f64  { x.acos() }
    pub extern "C" fn atan(x: f64) -> f64  { x.atan() }
    pub extern "C" fn atan2(y: f64, x: f64) -> f64 { y.atan2(x) }
    pub extern "C" fn exp(x: f64) -> f64   { x.exp() }
    pub extern "C" fn log(x: f64) -> f64   { x.ln() }
    pub extern "C" fn log10(x: f64) -> f64 { x.log10() }
    pub extern "C" fn sqrt(x: f64) -> f64  { x.sqrt() }
    pub extern "C" fn pow(b: f64, e: f64) -> f64 { b.powf(e) }
    pub extern "C" fn fabs(x: f64) -> f64  { x.abs() }
    pub extern "C" fn fmin(a: f64, b: f64) -> f64 { a.min(b) }
    pub extern "C" fn fmax(a: f64, b: f64) -> f64 { a.max(b) }
    pub extern "C" fn floor(x: f64) -> f64 { x.floor() }
    pub extern "C" fn ceil(x: f64) -> f64  { x.ceil() }
}

fn libm_funs() -> Vec<(&'static str, usize, *const u8)> {
    vec![
        ("sin",   1, libm_wrappers::sin   as *const u8),
        ("cos",   1, libm_wrappers::cos   as *const u8),
        ("tan",   1, libm_wrappers::tan   as *const u8),
        ("asin",  1, libm_wrappers::asin  as *const u8),
        ("acos",  1, libm_wrappers::acos  as *const u8),
        ("atan",  1, libm_wrappers::atan  as *const u8),
        ("atan2", 2, libm_wrappers::atan2 as *const u8),
        ("exp",   1, libm_wrappers::exp   as *const u8),
        ("log",   1, libm_wrappers::log   as *const u8),
        ("log10", 1, libm_wrappers::log10 as *const u8),
        ("sqrt",  1, libm_wrappers::sqrt  as *const u8),
        ("pow",   2, libm_wrappers::pow   as *const u8),
        ("fabs",  1, libm_wrappers::fabs  as *const u8),
        ("fmin",  2, libm_wrappers::fmin  as *const u8),
        ("fmax",  2, libm_wrappers::fmax  as *const u8),
        ("floor", 1, libm_wrappers::floor as *const u8),
        ("ceil",  1, libm_wrappers::ceil  as *const u8),
    ]
}

// ── Contribution extracted from behavior body ─────────────────────────────────

/// A single `I(plus, minus) <+ expr` contribution extracted from the behavior.
struct Contribution {
    plus:  String,   // port name of the + terminal
    minus: String,   // port name of the − terminal
    expr:  Expr,     // current expression
}

/// Extract all current contributions from a flat list of behavior statements.
fn extract_contributions(stmts: &[ElabBehaviorStmt]) -> Vec<Contribution> {
    let mut out = Vec::new();
    for stmt in stmts {
        extract_from_stmt(stmt, &mut out);
    }
    out
}

fn extract_from_stmt(stmt: &ElabBehaviorStmt, out: &mut Vec<Contribution>) {
    match stmt {
        ElabBehaviorStmt::Bind { dest, op: BindOp::Contrib, src } => {
            // I(plus, minus) <+ expr
            if let Expr::Call(func, args) = dest {
                if let Expr::Ident(fname) = func.as_ref() {
                    if fname == "I" {
                        if let (Some(Expr::Ident(p)), Some(Expr::Ident(m))) =
                            (args.first(), args.get(1))
                        {
                            out.push(Contribution {
                                plus:  p.clone(),
                                minus: m.clone(),
                                expr:  src.clone(),
                            });
                            return;
                        }
                    }
                    // V(p,n) <+ expr  (voltage force) — treat as I contribution
                    // from the KCL perspective we skip voltage forces here;
                    // they require branch equation handling beyond simple stamping.
                }
            }
        }
        ElabBehaviorStmt::If { then_body, else_body, .. } => {
            for s in then_body { extract_from_stmt(s, out); }
            if let Some(eb) = else_body { for s in eb { extract_from_stmt(s, out); } }
        }
        _ => {}
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Compile an analog module from an [`ElabProgram`] into a JIT device.
///
/// `module_name` must match both:
/// - A key in `prog.modules` (for port/param declarations)
/// - A name in `prog.behaviors` with `kind == BehaviorKind::Analog`
pub fn compile_analog_module(
    prog: &ElabProgram,
    module_name: &str,
) -> Result<JitAnalogDevice, CodegenError> {
    use piperine_lang::parse::ast::BehaviorKind;

    // ── Locate module and behavior ────────────────────────────────────────────
    let elab_mod = prog.modules.get(module_name).ok_or_else(|| {
        CodegenError::ModuleNotFound(module_name.to_string())
    })?;

    let behavior = prog.behaviors.iter()
        .find(|b| b.name == module_name && b.kind == BehaviorKind::Analog)
        .ok_or_else(|| CodegenError::BehaviorNotFound(module_name.to_string()))?;

    // ── Port index map (name → index, 0-based) ────────────────────────────────
    let port_index: HashMap<String, usize> = elab_mod.ports.iter()
        .enumerate()
        .map(|(i, p)| (p.name.clone(), i))
        .collect();
    let num_terminals = elab_mod.ports.len();

    // ── Param index map (name → index) ────────────────────────────────────────
    let param_names: Vec<String> = elab_mod.params.iter().map(|p| p.name.clone()).collect();
    let param_index: HashMap<String, usize> = param_names.iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), i))
        .collect();
    let num_params = param_names.len();

    // ── Extract contributions ─────────────────────────────────────────────────
    let contributions = extract_contributions(&behavior.body);

    // ── Collect unique branches (for Jacobian differentiation) ────────────────
    let mut branches: Vec<(String, String)> = Vec::new();
    for c in &contributions {
        collect_branches(&c.expr, &mut branches);
    }
    // Also include the I(p,n) terminal pairs themselves as branches
    for c in &contributions {
        let pair = (c.plus.clone(), c.minus.clone());
        if !branches.contains(&pair) {
            branches.push(pair);
        }
    }

    // ── Build JIT module ──────────────────────────────────────────────────────
    let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| CodegenError::Module(e.to_string()))?;
    for (name, _arity, ptr) in libm_funs() {
        jit_builder.symbol(name, ptr);
    }

    let mut module = JITModule::new(jit_builder);
    let libm_ids = declare_libm_imports(&mut module)?;

    let residual_id = compile_residual(
        &mut module, &libm_ids,
        &contributions, &port_index, &param_index, &branches,
        num_terminals, num_params,
    )?;

    let jacobian_id = compile_jacobian(
        &mut module, &libm_ids,
        &contributions, &port_index, &param_index, &branches,
        num_terminals, num_params,
    )?;

    module.finalize_definitions()
        .map_err(|e| CodegenError::Module(e.to_string()))?;

    let residual_ptr = module.get_finalized_function(residual_id);
    let jacobian_ptr = module.get_finalized_function(jacobian_id);

    let residual: unsafe extern "C" fn(*const f64, *const f64, *mut f64) =
        unsafe { std::mem::transmute(residual_ptr) };
    let jacobian: unsafe extern "C" fn(*const f64, *const f64, *mut f64) =
        unsafe { std::mem::transmute(jacobian_ptr) };

    Ok(JitAnalogDevice {
        name: module_name.to_string(),
        param_names,
        num_terminals,
        num_params,
        residual,
        jacobian,
        _module: module,
    })
}

// ── libm import helpers ───────────────────────────────────────────────────────

fn make_f64_sig(module: &JITModule, arity: usize) -> Signature {
    let mut sig = module.make_signature();
    for _ in 0..arity { sig.params.push(AbiParam::new(F64)); }
    sig.returns.push(AbiParam::new(F64));
    sig
}

fn declare_libm_imports(module: &mut JITModule) -> Result<HashMap<&'static str, FuncId>, CodegenError> {
    let mut ids = HashMap::new();
    for (name, arity, _) in libm_funs() {
        let sig = make_f64_sig(module, arity);
        let id = module.declare_function(name, Linkage::Import, &sig)
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        ids.insert(name, id);
    }
    Ok(ids)
}

fn make_body_sig(module: &JITModule) -> Signature {
    let ptr = module.target_config().pointer_type();
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(ptr));
    sig.params.push(AbiParam::new(ptr));
    sig.params.push(AbiParam::new(ptr));
    sig
}

fn import_libm_into_func(
    module: &mut JITModule,
    libm_ids: &HashMap<&'static str, FuncId>,
    func: &mut Function,
) -> HashMap<&'static str, FuncRef> {
    let mut map = HashMap::new();
    for (&name, &id) in libm_ids {
        let fref = module.declare_func_in_func(id, func);
        map.insert(name, fref);
    }
    map
}

// ── Value setup helpers ───────────────────────────────────────────────────────

/// Precompute all branch voltages from the node-voltage array.
/// Returns map: branch_key → Value.
fn build_branch_voltages(
    builder: &mut FunctionBuilder,
    node_ptr: cranelift_codegen::ir::Value,
    branches: &[(String, String)],
    port_index: &HashMap<String, usize>,
) -> HashMap<String, cranelift_codegen::ir::Value> {
    let mut map = HashMap::new();
    for (plus, minus) in branches {
        let key = branch_key(plus, minus);
        if map.contains_key(&key) { continue; }
        let vp = match port_index.get(plus.as_str()) {
            Some(&idx) => load_f64(builder, node_ptr, idx),
            None       => builder.ins().f64const(0.0),
        };
        let vm = match port_index.get(minus.as_str()) {
            Some(&idx) => load_f64(builder, node_ptr, idx),
            None       => builder.ins().f64const(0.0),
        };
        let v = builder.ins().fsub(vp, vm);
        map.insert(key, v);
    }
    map
}

/// Precompute all param values from the param array.
fn build_param_values(
    builder: &mut FunctionBuilder,
    param_ptr: cranelift_codegen::ir::Value,
    param_index: &HashMap<String, usize>,
) -> HashMap<String, cranelift_codegen::ir::Value> {
    param_index.iter()
        .map(|(name, &idx)| (name.clone(), load_f64(builder, param_ptr, idx)))
        .collect()
}

// ── Residual function ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn compile_residual(
    module: &mut JITModule,
    libm_ids: &HashMap<&'static str, FuncId>,
    contributions: &[Contribution],
    port_index: &HashMap<String, usize>,
    param_index: &HashMap<String, usize>,
    branches: &[(String, String)],
    _num_terminals: usize,
    _num_params: usize,
) -> Result<FuncId, CodegenError> {
    let sig = make_body_sig(module);
    let func_id = module.declare_function("residual", Linkage::Export, &sig)
        .map_err(|e| CodegenError::Module(e.to_string()))?;

    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut fb_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);

    let libm = import_libm_into_func(module, libm_ids, builder.func);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let node_ptr  = builder.block_params(entry)[0];
    let param_ptr = builder.block_params(entry)[1];
    let rhs_ptr   = builder.block_params(entry)[2];

    let branch_voltages = build_branch_voltages(&mut builder, node_ptr, branches, port_index);
    let param_values    = build_param_values(&mut builder, param_ptr, param_index);

    for contrib in contributions {
        let mut ectx = ExprCtx { builder: &mut builder, branch_voltages: &branch_voltages, param_values: &param_values, libm: &libm };
        let current = emit_phdl_expr(&mut ectx, &contrib.expr);

        if let Some(&p_idx) = port_index.get(contrib.plus.as_str()) {
            accumulate_f64(&mut builder, current, rhs_ptr, p_idx);
        }
        if let Some(&n_idx) = port_index.get(contrib.minus.as_str()) {
            let neg = builder.ins().fneg(current);
            accumulate_f64(&mut builder, neg, rhs_ptr, n_idx);
        }
    }

    builder.ins().return_(&[]);
    builder.finalize();

    module.define_function(func_id, &mut ctx)
        .map_err(|e| CodegenError::Module(e.to_string()))?;
    Ok(func_id)
}

// ── Jacobian function ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn compile_jacobian(
    module: &mut JITModule,
    libm_ids: &HashMap<&'static str, FuncId>,
    contributions: &[Contribution],
    port_index: &HashMap<String, usize>,
    param_index: &HashMap<String, usize>,
    branches: &[(String, String)],
    num_terminals: usize,
    _num_params: usize,
) -> Result<FuncId, CodegenError> {
    let sig = make_body_sig(module);
    let func_id = module.declare_function("jacobian", Linkage::Export, &sig)
        .map_err(|e| CodegenError::Module(e.to_string()))?;

    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut fb_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);

    let libm = import_libm_into_func(module, libm_ids, builder.func);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let node_ptr  = builder.block_params(entry)[0];
    let param_ptr = builder.block_params(entry)[1];
    let jac_ptr   = builder.block_params(entry)[2];

    let branch_voltages = build_branch_voltages(&mut builder, node_ptr, branches, port_index);
    let param_values    = build_param_values(&mut builder, param_ptr, param_index);

    let n = num_terminals;

    for contrib in contributions {
        let plus_idx  = port_index.get(contrib.plus.as_str()).copied();
        let minus_idx = port_index.get(contrib.minus.as_str()).copied();

        for (a, b) in branches {
            let wrt = branch_key(a, b);
            let dexpr = diff(&contrib.expr, &wrt);

            let mut ectx = ExprCtx {
                builder: &mut builder,
                branch_voltages: &branch_voltages,
                param_values: &param_values,
                libm: &libm,
            };
            let g = emit_phdl_expr(&mut ectx, &dexpr);

            let a_idx = port_index.get(a.as_str()).copied();
            let b_idx = port_index.get(b.as_str()).copied();

            // Stamp conductance into Jacobian (row-major, n×n).
            // I(plus,minus) <+ f(V(a,b)):
            //   J[plus, a]  += g
            //   J[plus, b]  -= g
            //   J[minus, a] -= g
            //   J[minus, b] += g
            if let Some(p) = plus_idx {
                if let Some(ai) = a_idx {
                    accumulate_f64(&mut builder, g, jac_ptr, p * n + ai);
                }
                if let Some(bi) = b_idx {
                    let neg_g = builder.ins().fneg(g);
                    accumulate_f64(&mut builder, neg_g, jac_ptr, p * n + bi);
                }
            }
            if let Some(m) = minus_idx {
                if let Some(ai) = a_idx {
                    let neg_g = builder.ins().fneg(g);
                    accumulate_f64(&mut builder, neg_g, jac_ptr, m * n + ai);
                }
                if let Some(bi) = b_idx {
                    accumulate_f64(&mut builder, g, jac_ptr, m * n + bi);
                }
            }
        }
    }

    builder.ins().return_(&[]);
    builder.finalize();

    module.define_function(func_id, &mut ctx)
        .map_err(|e| CodegenError::Module(e.to_string()))?;
    Ok(func_id)
}
