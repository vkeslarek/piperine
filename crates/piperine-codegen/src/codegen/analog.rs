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

use super::cranelift_helpers::{accumulate_f64, load_f64, ExprCtx};
use super::ir_emit::AnalogExpr;
use super::{CodegenError, JitAnalogDevice};

fn branch_key(plus: &str, minus: &str) -> String {
    format!("V({plus},{minus})")
}

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
    pub extern "C" fn sinh(x: f64) -> f64  { x.sinh() }
    pub extern "C" fn cosh(x: f64) -> f64  { x.cosh() }
    pub extern "C" fn tanh(x: f64) -> f64  { x.tanh() }
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
        ("sinh",  1, libm_wrappers::sinh  as *const u8),
        ("cosh",  1, libm_wrappers::cosh  as *const u8),
        ("tanh",  1, libm_wrappers::tanh  as *const u8),
    ]
}

// ── Contribution extracted from behavior body ─────────────────────────────────

/// A single `I(plus, minus) <+ expr` contribution extracted from the behavior.
///
/// Generic over the expression representation `E` so the shared Cranelift
/// skeleton compiles `IrExpr` via the [`AnalogExpr`] trait.
pub struct Contribution<E> {
    pub plus:  String,   // port name of the + terminal
    pub minus: String,   // port name of the − terminal
    pub expr:  E,        // current expression
}

/// GAPS §D.1 — a single `V(plus, minus) <- expr` force statement extracted
/// from the behavior. The compiled force-residual function writes
/// `V(plus) − V(minus) − expr` to one row of the output (one row per
/// force statement). The MNA matrix adds one branch-current unknown
/// per force; see also GAPS §H.4 for the MNA branch-current rows.
pub struct ForceContribution<E> {
    pub plus:  String,
    pub minus: String,
    pub expr:  E,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Inputs that the Cranelift core needs.
pub struct CompileInputs<E> {
    pub name: String,
    pub port_index: HashMap<String, usize>,
    pub num_terminals: usize,
    pub param_names: Vec<String>,
    pub param_index: HashMap<String, usize>,
    pub num_params: usize,
    pub contributions: Vec<Contribution<E>>,
    /// Reactive charge contributions: `Q(V)` whose `ddt` is stamped via the
    /// companion model.  Empty for purely resistive devices.
    pub react_contributions: Vec<Contribution<E>>,
    /// GAPS §D.1 — ideal voltage-source forces (`V(plus, minus) <- expr`).
    /// One row per force in the output of `force_residual`. The actual
    /// MNA stamping (branch-current unknowns, `V+ − V− − expr` rows) lives
    /// in the solver; see GAPS §H.4.
    pub force_contributions: Vec<ForceContribution<E>>,
    pub branches: Vec<(String, String)>,
}

impl<E: AnalogExpr> CompileInputs<E> {
    pub fn from_contributions(
        module_name: &str,
        port_names: Vec<String>,
        param_names: Vec<String>,
        contributions: Vec<Contribution<E>>,
        react_contributions: Vec<Contribution<E>>,
        force_contributions: Vec<ForceContribution<E>>,
    ) -> Self {
        let num_terminals = port_names.len();
        let port_index: HashMap<String, usize> = port_names.iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i))
            .collect();
        let param_index: HashMap<String, usize> = param_names.iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i))
            .collect();
        let mut branches: Vec<(String, String)> = Vec::new();
        for c in contributions.iter().chain(react_contributions.iter()) {
            c.expr.collect_branches(&mut branches);
        }
        for c in contributions.iter().chain(react_contributions.iter()) {
            let pair = (c.plus.clone(), c.minus.clone());
            if !branches.contains(&pair) {
                branches.push(pair);
            }
        }
        // Branches for forces are implicit (V+ − V− is a single branch).
        for f in &force_contributions {
            let pair = (f.plus.clone(), f.minus.clone());
            if !branches.contains(&pair) {
                branches.push(pair);
            }
        }
        Self {
            name: module_name.to_string(),
            port_index,
            num_terminals,
            num_params: param_names.len(),
            param_names,
            param_index,
            contributions,
            react_contributions,
            force_contributions,
            branches,
        }
    }
}

/// Shared Cranelift core: compiles the inputs into a `JitAnalogDevice`.
pub fn compile<E: AnalogExpr>(inputs: &CompileInputs<E>) -> Result<JitAnalogDevice, CodegenError> {
    let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| CodegenError::Module(e.to_string()))?;
    for (name, _arity, ptr) in libm_funs() {
        jit_builder.symbol(name, ptr);
    }

    let mut module = JITModule::new(jit_builder);
    let libm_ids = declare_libm_imports(&mut module)?;

    let residual_id = compile_named_residual(
        &mut module, &libm_ids, "residual",
        &inputs.contributions, &inputs.port_index, &inputs.param_index, &inputs.branches,
    )?;

    let jacobian_id = compile_named_jacobian(
        &mut module, &libm_ids, "jacobian",
        &inputs.contributions, &inputs.port_index, &inputs.param_index, &inputs.branches,
        inputs.num_terminals,
    )?;

    // Reactive charge functions: `Q(V)` (residual shape) and `dQ/dV`
    // (jacobian shape), compiled from the charge contributions.  Reusing the
    // resistive emitters is exact — a charge contribution is just another
    // expression stamped at the same terminals.
    let (charge_id, charge_jac_id) = if inputs.react_contributions.is_empty() {
        (None, None)
    } else {
        let q_id = compile_named_residual(
            &mut module, &libm_ids, "charge",
            &inputs.react_contributions, &inputs.port_index, &inputs.param_index,
            &inputs.branches,
        )?;
        let qj_id = compile_named_jacobian(
            &mut module, &libm_ids, "charge_jacobian",
            &inputs.react_contributions, &inputs.port_index, &inputs.param_index,
            &inputs.branches, inputs.num_terminals,
        )?;
        (Some(q_id), Some(qj_id))
    };

    // GAPS §D.1 — force-residual function (one row per `V(p,n) <- expr`).
    let force_id = if inputs.force_contributions.is_empty() {
        None
    } else {
        Some(compile_named_force(
            &mut module, &libm_ids, "force",
            &inputs.force_contributions, &inputs.port_index, &inputs.param_index,
            &inputs.branches,
        )?)
    };

    module.finalize_definitions()
        .map_err(|e| CodegenError::Module(e.to_string()))?;

    let residual_ptr = module.get_finalized_function(residual_id);
    let jacobian_ptr = module.get_finalized_function(jacobian_id);

    let residual: unsafe extern "C" fn(*const f64, *const f64, *const super::SimCtx, *mut f64) =
        unsafe { std::mem::transmute(residual_ptr) };
    let jacobian: unsafe extern "C" fn(*const f64, *const f64, *const super::SimCtx, *mut f64) =
        unsafe { std::mem::transmute(jacobian_ptr) };

    let transmute_fn = |id: FuncId|
        -> unsafe extern "C" fn(*const f64, *const f64, *const super::SimCtx, *mut f64)
    {
        unsafe { std::mem::transmute(module.get_finalized_function(id)) }
    };
    let charge = charge_id.map(transmute_fn);
    let charge_jacobian = charge_jac_id.map(transmute_fn);
    let force = force_id.map(|id| (inputs.force_contributions.len(), transmute_fn(id)));

    Ok(JitAnalogDevice {
        name: inputs.name.clone(),
        num_terminals: inputs.num_terminals,
        num_params: inputs.num_params,
        param_names: inputs.param_names.clone(),
        residual,
        jacobian,
        charge,
        charge_jacobian,
        force,
        _module: module,
    })
}

/// Compile an analog module from an [`Design`] into a JIT device.
///
/// Lower a pre-built list of contributions into a `JitAnalogDevice`.
///
/// Used by the IR front door:  `from_ir` translates IR contributions into
/// `[Contribution; N]` and hands them off here.
pub fn compile_analog_module_ir<E: AnalogExpr>(
    module_name: &str,
    port_names: Vec<String>,
    param_names: Vec<String>,
    contributions: Vec<Contribution<E>>,
    react_contributions: Vec<Contribution<E>>,
    force_contributions: Vec<ForceContribution<E>>,
) -> Result<JitAnalogDevice, CodegenError> {
    let inputs = CompileInputs::from_contributions(
        module_name,
        port_names,
        param_names,
        contributions,
        react_contributions,
        force_contributions,
    );
    compile(&inputs)
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
    sig.params.push(AbiParam::new(ptr));   // node_voltages
    sig.params.push(AbiParam::new(ptr));   // params
    sig.params.push(AbiParam::new(ptr));   // sim_ctx (see codegen::SimCtx)
    sig.params.push(AbiParam::new(ptr));   // rhs / jac
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
fn compile_named_residual<E: AnalogExpr>(
    module: &mut JITModule,
    libm_ids: &HashMap<&'static str, FuncId>,
    fn_name: &str,
    contributions: &[Contribution<E>],
    port_index: &HashMap<String, usize>,
    param_index: &HashMap<String, usize>,
    branches: &[(String, String)],
) -> Result<FuncId, CodegenError> {
    let sig = make_body_sig(module);
    let func_id = module.declare_function(fn_name, Linkage::Export, &sig)
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
    let sim_ptr   = builder.block_params(entry)[2];
    let rhs_ptr   = builder.block_params(entry)[3];

    let branch_voltages = build_branch_voltages(&mut builder, node_ptr, branches, port_index);
    let param_values    = build_param_values(&mut builder, param_ptr, param_index);

    for contrib in contributions {
        let mut ectx = ExprCtx {
            builder: &mut builder,
            branch_voltages: &branch_voltages,
            param_values: &param_values,
            param_index,
            libm: &libm,
            sim_ctx: sim_ptr,
        };
        let current = contrib.expr.emit(&mut ectx);

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
fn compile_named_jacobian<E: AnalogExpr>(
    module: &mut JITModule,
    libm_ids: &HashMap<&'static str, FuncId>,
    fn_name: &str,
    contributions: &[Contribution<E>],
    port_index: &HashMap<String, usize>,
    param_index: &HashMap<String, usize>,
    branches: &[(String, String)],
    num_terminals: usize,
) -> Result<FuncId, CodegenError> {
    let sig = make_body_sig(module);
    let func_id = module.declare_function(fn_name, Linkage::Export, &sig)
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
    let sim_ptr   = builder.block_params(entry)[2];
    let jac_ptr   = builder.block_params(entry)[3];

    let branch_voltages = build_branch_voltages(&mut builder, node_ptr, branches, port_index);
    let param_values    = build_param_values(&mut builder, param_ptr, param_index);

    let n = num_terminals;

    for contrib in contributions {
        let plus_idx  = port_index.get(contrib.plus.as_str()).copied();
        let minus_idx = port_index.get(contrib.minus.as_str()).copied();

        for (a, b) in branches {
            let wrt = branch_key(a, b);
            let dexpr = contrib.expr.diff(&wrt);

            let mut ectx = ExprCtx {
                builder: &mut builder,
                branch_voltages: &branch_voltages,
                param_values: &param_values,
                param_index,
                libm: &libm,
                sim_ctx: sim_ptr,
            };
            let g = dexpr.emit(&mut ectx);

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

/// GAPS §D.1 — compile the force-residual function. One row per force:
/// `out[i] = V(plus_i) − V(minus_i) − expr_i`. The output slot for row `i`
/// is determined by the solver (see GAPS §H.4: each force adds one
/// branch-current unknown to the MNA matrix).
#[allow(clippy::too_many_arguments)]
fn compile_named_force<E: AnalogExpr>(
    module: &mut JITModule,
    libm_ids: &HashMap<&'static str, FuncId>,
    fn_name: &str,
    forces: &[ForceContribution<E>],
    port_index: &HashMap<String, usize>,
    param_index: &HashMap<String, usize>,
    _branches: &[(String, String)],
) -> Result<FuncId, CodegenError> {
    let sig = make_body_sig(module);
    let func_id = module
        .declare_function(fn_name, Linkage::Export, &sig)
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

    let node_ptr = builder.block_params(entry)[0];
    let param_ptr = builder.block_params(entry)[1];
    let sim_ptr = builder.block_params(entry)[2];
    let out_ptr = builder.block_params(entry)[3];

    // The force function doesn't need precomputed branch voltages (it
    // directly loads V+ and V− from node_ptr). Pass a minimal dummy
    // branches list so `build_branch_voltages` is happy; it produces
    // an entry that is never read.
    let branch_voltages = build_branch_voltages(
        &mut builder, node_ptr,
        &[("".to_string(), "".to_string())],
        port_index,
    );
    let param_values = build_param_values(&mut builder, param_ptr, param_index);

    for (i, f) in forces.iter().enumerate() {
        let mut ectx = ExprCtx {
            builder: &mut builder,
            branch_voltages: &branch_voltages,
            param_values: &param_values,
            param_index,
            libm: &libm,
            sim_ctx: sim_ptr,
        };
        let expr_val = f.expr.emit(&mut ectx);
        let _ = sim_ptr; // force residual does not currently read SimCtx
        let vp = match port_index.get(f.plus.as_str()) {
            Some(&idx) => load_f64(&mut builder, node_ptr, idx),
            None       => builder.ins().f64const(0.0),
        };
        let vm = match port_index.get(f.minus.as_str()) {
            Some(&idx) => load_f64(&mut builder, node_ptr, idx),
            None       => builder.ins().f64const(0.0),
        };
        let diff = builder.ins().fsub(vp, vm);
        let row = builder.ins().fsub(diff, expr_val);
        accumulate_f64(&mut builder, row, out_ptr, i);
    }

    builder.ins().return_(&[]);
    builder.finalize();

    module.define_function(func_id, &mut ctx)
        .map_err(|e| CodegenError::Module(e.to_string()))?;
    Ok(func_id)
}
