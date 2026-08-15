//! Digital kernel compilation: an [`crate::resolve::DigitalBody`] to native
//! code. There is no digital interpreter — combinational logic, register
//! updates, and event watching all compile through Cranelift.
//!
//! One [`DigitalKernel`] per module, shared across instances. Per-instance
//! signal values and register banks live in the device (`crate::device`).
//!
//! ## Value encoding
//!
//! Digital signals are 4-state (`Quad`), encoded in `i64` as 0, 1, 2 (X),
//! 3 (Z). Integers/booleans are plain `i64`; reals are `f64`. Variables live
//! in two per-instance banks (int and real) addressed by compile-time slots.
//!
//! ## Compiled functions
//!
//! - `comb(*abi)` — evaluates the combinational statements in source order:
//!   reads inputs and the live variable banks, writes outputs and the banks.
//!   Unassigned-before-read variables hold their previous value (a latch).
//! - `seq(*abi)` — for each clocked block whose `fired` flag is set, runs the
//!   register updates: reads see the *pre-edge* bank copies, writes go to the
//!   live banks (SPEC §9).
//! - `watch(*abi, *out)` — evaluates each atomic event term (the signal under
//!   a `posedge`/`negedge`/`change`); the device compares against the
//!   previous values to derive the per-block `fired` flags.

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, FuncRef, InstBuilder, MemFlags, Signature, Value};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use crate::emit::OwnedJit;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use piperine_lang::ast::{EventExpr, Expr, Stmt};

use crate::emit::{Builder, Codegen, DigTy, Resolver};
use crate::resolve::{
    DigitalBody, EdgeKind, LoweredBody, ParamId,
};

use piperine_lang::math;
use crate::error::CodegenError;

use super::abi::*;
use super::layout::*;

pub(crate) struct DigitalCompiler<'m> {
    module: &'m LoweredBody,
    body: &'m DigitalBody,
    layout: DigitalLayout,
    jit: JITModule,
    math_ids: HashMap<&'static str, FuncId>,
    /// `pip_task_display`/`pip_task_write`/`pip_task_fatal` — the `$display`-
    /// family trampolines (LRMC-C7-07, T43), all sharing one signature
    /// (`extern "C" fn(ctx: *mut SimCtx, fmt_id: u32, argc: u32, argv: *const
    /// TaskArg)`). `TASK_FNS` is already registered as a JIT symbol in `new`
    /// below (was — before this task — never actually declared/called from
    /// either compiled function, only registered).
    task_ids: HashMap<&'static str, FuncId>,
    fb_ctx: FunctionBuilderContext,
    resolver: Resolver,
}

impl<'m> DigitalCompiler<'m> {
    pub(crate) fn new(module: &'m LoweredBody) -> Result<Self, CodegenError> {
        let body = module
            .digital
            .as_ref()
            .ok_or_else(|| CodegenError::Invalid(format!("`{}` has no digital body", module.name)))?;

        let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        for f in math::MATH_FNS {
            jit_builder.symbol(f.name, f.symbol);
        }
        for f in crate::emit::runtime_io::TASK_FNS {
            jit_builder.symbol(f.name, f.ptr);
        }
        let mut jit = JITModule::new(jit_builder);
        let mut math_ids = HashMap::new();
        for f in math::MATH_FNS {
            let mut sig = jit.make_signature();
            for _ in 0..f.arity {
                sig.params.push(AbiParam::new(types::F64));
            }
            sig.returns.push(AbiParam::new(types::F64));
            let id = jit
                .declare_function(f.name, Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            math_ids.insert(f.name, id);
        }
        let mut task_ids = HashMap::new();
        for name in ["pip_task_display", "pip_task_write", "pip_task_fatal"] {
            let mut sig = jit.make_signature();
            sig.params.push(AbiParam::new(types::I64)); // ctx: *mut SimCtx
            sig.params.push(AbiParam::new(types::I32)); // fmt_id
            sig.params.push(AbiParam::new(types::I32)); // argc
            sig.params.push(AbiParam::new(types::I64)); // argv: *const TaskArg
            let id = jit
                .declare_function(name, Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            task_ids.insert(name, id);
        }

        Ok(Self {
            layout: DigitalLayout::build(module, body),
            module,
            body,
            jit,
            math_ids,
            task_ids,
            fb_ctx: FunctionBuilderContext::new(),
            resolver: Resolver::from_symbols(&module.symbols),
        })
    }

    pub(crate) fn compile(mut self) -> Result<DigitalKernel, CodegenError> {
        // Split the body: combinational statements vs clocked blocks.
        // Register power-on inits come from `DigitalBody::reg_inits`
        // directly (T30, `resolve/pom/body.rs`) — VAMS has no
        // `VarDecl`-with-default statement to scan for anymore.
        let mut comb_stmts: Vec<&Stmt> = Vec::new();
        let mut clocked: Vec<(&[EventExpr], &Stmt)> = Vec::new();
        for stmt in &self.body.stmts {
            match stmt {
                Stmt::EventControl { events, body, .. } if is_clocked(events) => {
                    clocked.push((events, body));
                }
                // A non-edge event control (`@*`/`@(sig)` — level-sensitive)
                // is combinational in this step-based (not event-scheduled)
                // execution model: `comb` already re-evaluates every pass,
                // which already gives "re-run on any input change"
                // semantics, so its body folds directly into the plain
                // combinational statement list rather than through the
                // clocked/`fired`-flag path.
                Stmt::EventControl { body, .. } => comb_stmts.push(body.as_ref()),
                other => comb_stmts.push(other),
            }
        }
        let reg_inits: Vec<RegInit> = self
            .body
            .reg_inits
            .iter()
            .map(|(var, init)| RegInit { var: *var, init: init.clone() })
            .collect();

        // Number the atomic watch terms across all clocked blocks. Each
        // event-control entry contributes one term: the watched expression,
        // with the edge polarity from `Posedge`/`Negedge`/`Level`
        // ("change").
        let mut watch_terms: Vec<Expr> = Vec::new();
        let mut clocked_specs = Vec::new();
        for (events, _) in &clocked {
            let mut terms = Vec::new();
            let mut event_terms = Vec::new();
            extract_event_terms(events, &mut event_terms);
            for (edge, arg) in event_terms {
                let index = match watch_terms.iter().position(|t| crate::emit::expr_structural_eq(t, &arg)) {
                    Some(i) => i,
                    None => {
                        watch_terms.push(arg.clone());
                        watch_terms.len() - 1
                    }
                };
                terms.push((index, edge));
            }
            clocked_specs.push(ClockedSpec { terms, is_initial: false });
        }

        // `comb` reads live variable values: after `seq` writes register
        // updates to the live bank, `comb` sees the new values and drives
        // outputs from them. Within a clocked block (`seq`), reads see
        // pre-edge values — register updates are non-blocking within the
        // block (SPEC §10.3: "within the block reads see the pre-edge
        // value, a chain of register writes is a pipeline").
        let comb_id = self.compile_body_fn("comb", &comb_stmts, VarReads::Live)?;
        let seq_id = if clocked.is_empty() {
            None
        } else {
            let stmts: Vec<&Stmt> = self
                .body
                .stmts
                .iter()
                .filter(|s| matches!(s, Stmt::EventControl { events, .. } if is_clocked(events)))
                .collect();
            Some(self.compile_body_fn("seq", &stmts, VarReads::PreEdge)?)
        };
        let watch_id = if watch_terms.is_empty() {
            None
        } else {
            Some(self.compile_watch_fn("watch", &watch_terms)?)
        };

        self.jit
            .finalize_definitions()
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        let comb: DigitalFn = unsafe { std::mem::transmute(self.jit.get_finalized_function(comb_id)) };
        let seq: Option<DigitalFn> =
            seq_id.map(|id| unsafe { std::mem::transmute(self.jit.get_finalized_function(id)) });
        let watch: Option<WatchFn> =
            watch_id.map(|id| unsafe { std::mem::transmute(self.jit.get_finalized_function(id)) });

        // Build the param name → ParamId index for `RegInit.init` evaluation.
        let param_index: HashMap<String, ParamId> = self
            .module
            .symbols
            .params()
            .map(|(id, p)| (p.name.clone(), id))
            .collect();

        let input_names: Vec<String> = self
            .body
            .inputs
            .iter()
            .map(|&id| self.module.symbols.node(id).name.clone())
            .collect();
        let output_names: Vec<String> = self
            .body
            .outputs
            .iter()
            .map(|&id| self.module.symbols.node(id).name.clone())
            .collect();

        Ok(DigitalKernel {
            name: self.module.name.clone(),
            inputs: self.body.inputs.clone(),
            outputs: self.body.outputs.clone(),
            input_names,
            output_names,
            layout: self.layout,
            clocked_blocks: clocked_specs,
            num_watch_terms: watch_terms.len(),
            reg_inits,
            param_index,
            comb,
            seq,
            watch,
            _jit: self.jit.into(),
        })
    }

    /// Compile a statement-body function (`comb` or `seq`).
    pub(crate) fn compile_body_fn(
        &mut self,
        name: &str,
        stmts: &[&Stmt],
        reads: VarReads,
    ) -> Result<FuncId, CodegenError> {
        self.build_fn(name, false, |b| {
            let mut clocked_index = 0usize;
            for stmt in stmts {
                if let Stmt::EventControl { events, body, .. } = stmt
                    && is_clocked(events)
                {
                    b.emit_guarded_block(clocked_index, body)?;
                    clocked_index += 1;
                } else {
                    b.emit_stmt(stmt)?;
                }
            }
            Ok(())
        }, reads)
    }

    /// Compile the watch function: each term's quad value to `out[i]`.
    pub(crate) fn compile_watch_fn(&mut self, name: &str, terms: &[Expr]) -> Result<FuncId, CodegenError> {
        self.build_fn(name, true, |b| {
            let out_ptr = b.watch_out.expect("watch fn has an out pointer");
            for (i, term) in terms.iter().enumerate() {
                let value = term.emit(b)?;
                let quad = b.coerce(value, DigTy::Quad)?;
                b.builder.ins().store(
                    MemFlags::trusted(),
                    quad.value,
                    out_ptr,
                    (i * 8) as i32,
                );
            }
            Ok(())
        }, VarReads::Live)
    }

    fn build_fn(
        &mut self,
        name: &str,
        with_out: bool,
        body: impl FnOnce(&mut Builder) -> Result<(), CodegenError>,
        reads: VarReads,
    ) -> Result<FuncId, CodegenError> {
        let ptr_ty = self.jit.target_config().pointer_type();
        let mut sig = Signature::new(self.jit.isa().default_call_conv());
        sig.params.push(AbiParam::new(ptr_ty));
        if with_out {
            sig.params.push(AbiParam::new(ptr_ty));
        }

        let func_id = self
            .jit
            .declare_function(name, Linkage::Export, &sig)
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        let mut ctx = self.jit.make_context();
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut self.fb_ctx);

        let math: HashMap<&'static str, FuncRef> = self
            .math_ids
            .iter()
            .map(|(&name, &id)| (name, self.jit.declare_func_in_func(id, builder.func)))
            .collect();
        let tasks: HashMap<&'static str, FuncRef> = self
            .task_ids
            .iter()
            .map(|(&name, &id)| (name, self.jit.declare_func_in_func(id, builder.func)))
            .collect();

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let abi_ptr = builder.block_params(entry)[0];
        let watch_out = with_out.then(|| builder.block_params(entry)[1]);

        let load_field = |builder: &mut FunctionBuilder, field: AbiField| {
            builder
                .ins()
                .load(ptr_ty, MemFlags::trusted(), abi_ptr, field as i32)
        };
        let pointers = Pointers {
            inputs: load_field(&mut builder, AbiField::Inputs),
            outputs: load_field(&mut builder, AbiField::Outputs),
            vars_int_old: load_field(&mut builder, AbiField::VarsIntOld),
            vars_real_old: load_field(&mut builder, AbiField::VarsRealOld),
            vars_int: load_field(&mut builder, AbiField::VarsInt),
            vars_real: load_field(&mut builder, AbiField::VarsReal),
            params: load_field(&mut builder, AbiField::Params),
            fired: load_field(&mut builder, AbiField::Fired),
            sim: load_field(&mut builder, AbiField::Sim),
            analog_voltages: load_field(&mut builder, AbiField::AnalogVoltages),
        };

        let mut b = Builder::new_digital(
            &mut builder,
            self.module,
            &self.resolver,
            &self.layout,
            pointers,
            reads,
            &math,
            &tasks,
            watch_out,
        );
        body(&mut b)?;

        builder.ins().return_(&[]);
        builder.finalize();

        self.jit
            .define_function(func_id, &mut ctx)
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        Ok(func_id)
    }
}

// ─── Event-control helpers ─────────────────────────────────────────────────────

/// `true` if any entry in an event-control's `events` list is edge-sensitive
/// (`posedge`/`negedge`) — the digital clocked-block marker (LRM/1364).
/// A control with only `Level`/`ImplicitAll` entries is level-sensitive
/// (`@(sig)`/`@*`), which this step-based execution model already covers by
/// re-evaluating combinational statements every pass — see `compile`'s own
/// classification.
fn is_clocked(events: &[EventExpr]) -> bool {
    events.iter().any(|e| matches!(e, EventExpr::Posedge(_) | EventExpr::Negedge(_)))
}

/// Walk a clocked event-control's `events` list and collect
/// `(edge, watched_expr)` pairs for each atomic term. `ImplicitAll` (`@*`)
/// contributes no digital watch term — a clocked block never spells its
/// sensitivity that way (mixing `@*` with `posedge`/`negedge` in the same
/// control has no LRM meaning); defensive, not a live path once `is_clocked`
/// has already gated the caller.
fn extract_event_terms(events: &[EventExpr], terms: &mut Vec<(EdgeKind, Expr)>) {
    for event in events {
        match event {
            EventExpr::Posedge(e) => terms.push((EdgeKind::Rising, e.clone())),
            EventExpr::Negedge(e) => terms.push((EdgeKind::Falling, e.clone())),
            EventExpr::Level(e) => terms.push((EdgeKind::Any, e.clone())),
            EventExpr::ImplicitAll => {}
        }
    }
}

// ─── ABI helpers (shared with the Builder) ────────────────────────────────────

/// Whether variable reads see the live banks (comb) or the pre-edge copies
/// (seq register semantics).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VarReads {
    Live,
    PreEdge,
}

/// Loaded ABI pointer table.
#[derive(Clone, Copy)]
pub struct Pointers {
    pub inputs: Value,
    pub outputs: Value,
    pub vars_int_old: Value,
    pub vars_real_old: Value,
    pub vars_int: Value,
    pub vars_real: Value,
    pub params: Value,
    pub fired: Value,
    pub sim: Value,
    pub analog_voltages: Value,
}

// ─── Fused combinational network (Verilator-style whole-cone JIT) ───────────────

/// One member of a fused combinational network: its IR module plus how it binds
/// into the network-wide arrays. `in_net_slots[i]`/`out_net_slots[i]` are the
/// global net ids wired to kernel input/output `i`; the `*_base` fields are the
/// member's offsets into the shared int/real variable banks and the params bank.
pub struct NetworkMemberSpec<'m> {
    pub module: &'m crate::resolve::LoweredBody,
    pub in_net_slots: Vec<usize>,
    pub out_net_slots: Vec<usize>,
    pub int_base: usize,
    pub real_base: usize,
    pub param_base: usize,
}

/// The fused combinational function: evaluates every member's `comb` body in
/// rank order over the shared arrays. One native call settles an acyclic cone.
/// Args: `(nets, vars_int, vars_real, params, sim, analog)`.
pub type NetworkCombFn = unsafe extern "C" fn(
    *mut i64,
    *mut i64,
    *mut f64,
    *const f64,
    *const crate::emit::abi::SimCtx,
    *const f64,
);

/// A compiled fused combinational network kernel.
pub struct NetworkComb {
    func: NetworkCombFn,
    _jit: OwnedJit,
}

unsafe impl Send for NetworkComb {}
unsafe impl Sync for NetworkComb {}

impl NetworkComb {
    /// Run the fused comb over the network arrays (one rank-ordered pass).
    ///
    /// # Safety
    /// Pointers must be valid for the network's bank sizes; `sim`/`analog`
    /// non-null (pass a dummy `analog` when the cone samples none).
    pub unsafe fn run(
        &self,
        nets: *mut i64,
        vars_int: *mut i64,
        vars_real: *mut f64,
        params: *const f64,
        sim: *const crate::emit::abi::SimCtx,
        analog: *const f64,
    ) {
        unsafe { (self.func)(nets, vars_int, vars_real, params, sim, analog) }
    }

    /// Fuse the combinational bodies of `members` into one Cranelift function.
    ///
    /// Members must be pure combinational digital: no clocked blocks and no
    /// analog sampling (those stay per-device; the network builder only pulls
    /// eligible instances into a cone). Fails loud otherwise — never a silent
    /// wrong fuse.
    pub fn compile(members: &[NetworkMemberSpec]) -> Result<Self, CodegenError> {
        let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        for f in math::MATH_FNS {
            jit_builder.symbol(f.name, f.symbol);
        }
        for f in crate::emit::runtime_io::TASK_FNS {
            jit_builder.symbol(f.name, f.ptr);
        }
        let mut jit = JITModule::new(jit_builder);
        let mut math_ids: HashMap<&'static str, FuncId> = HashMap::new();
        for f in math::MATH_FNS {
            let mut sig = jit.make_signature();
            for _ in 0..f.arity {
                sig.params.push(AbiParam::new(types::F64));
            }
            sig.returns.push(AbiParam::new(types::F64));
            let id = jit
                .declare_function(f.name, Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            math_ids.insert(f.name, id);
        }
        let mut task_ids: HashMap<&'static str, FuncId> = HashMap::new();
        for name in ["pip_task_display", "pip_task_write", "pip_task_fatal"] {
            let mut sig = jit.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I32));
            sig.params.push(AbiParam::new(types::I32));
            sig.params.push(AbiParam::new(types::I64));
            let id = jit
                .declare_function(name, Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            task_ids.insert(name, id);
        }

        let ptr_ty = jit.target_config().pointer_type();
        let mut sig = Signature::new(jit.isa().default_call_conv());
        for _ in 0..6 {
            sig.params.push(AbiParam::new(ptr_ty));
        }
        let func_id = jit
            .declare_function("network_comb", Linkage::Export, &sig)
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        // Pre-build each member's remapped layout: port nodes → global net
        // slots; variable/analog slots stay module-local (bank pointers carry
        // the per-member base offset instead).
        let mut layouts: Vec<DigitalLayout> = Vec::with_capacity(members.len());
        for m in members {
            let body = m.module.digital.as_ref().ok_or_else(|| {
                CodegenError::Invalid(format!("`{}` has no digital body", m.module.name))
            })?;
            if body.stmts.iter().any(|s| matches!(s, Stmt::EventControl { events, .. } if is_clocked(events))) {
                return Err(CodegenError::unsupported(format!(
                    "network fusion of clocked module `{}` (comb-only cones for now)",
                    m.module.name
                )));
            }
            let mut layout = DigitalLayout::build(m.module, body);
            if layout.num_analog() > 0 {
                return Err(CodegenError::unsupported(format!(
                    "network fusion of analog-sampling module `{}`",
                    m.module.name
                )));
            }
            if m.in_net_slots.len() != body.inputs.len() || m.out_net_slots.len() != body.outputs.len() {
                return Err(CodegenError::Invalid(format!(
                    "`{}` net wiring does not match its port count",
                    m.module.name
                )));
            }
            // Remap port nodes to global net slots (both banks are the shared
            // net array, so inputs and outputs index the same pointer).
            layout.input_index.clear();
            for (i, &node) in body.inputs.iter().enumerate() {
                layout.input_index.insert(node, m.in_net_slots[i]);
            }
            layout.output_index.clear();
            for (i, &node) in body.outputs.iter().enumerate() {
                layout.output_index.insert(node, m.out_net_slots[i]);
            }
            layouts.push(layout);
        }

        let mut fb_ctx = FunctionBuilderContext::new();
        let mut ctx = jit.make_context();
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);

        let math: HashMap<&'static str, FuncRef> = math_ids
            .iter()
            .map(|(&name, &id)| (name, jit.declare_func_in_func(id, builder.func)))
            .collect();
        let tasks: HashMap<&'static str, FuncRef> = task_ids
            .iter()
            .map(|(&name, &id)| (name, jit.declare_func_in_func(id, builder.func)))
            .collect();

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let p = builder.block_params(entry);
        let (nets_ptr, vars_int_ptr, vars_real_ptr, params_ptr, sim_ptr, analog_ptr) =
            (p[0], p[1], p[2], p[3], p[4], p[5]);

        for (m, layout) in members.iter().zip(&layouts) {
            // Offset the member's bank pointers so its module-local slots index
            // the network-wide banks.
            let vint = builder.ins().iadd_imm(vars_int_ptr, (m.int_base * 8) as i64);
            let vreal = builder.ins().iadd_imm(vars_real_ptr, (m.real_base * 8) as i64);
            let par = builder.ins().iadd_imm(params_ptr, (m.param_base * 8) as i64);
            let pointers = Pointers {
                inputs: nets_ptr,
                outputs: nets_ptr,
                // Combinational members read live vars (no pre-edge/fired use).
                vars_int_old: vint,
                vars_real_old: vreal,
                vars_int: vint,
                vars_real: vreal,
                params: par,
                fired: nets_ptr, // unused: no clocked blocks in a fused member
                sim: sim_ptr,
                analog_voltages: analog_ptr,
            };
            let resolver = Resolver::from_symbols(&m.module.symbols);
            let mut b = Builder::new_digital(
                &mut builder,
                m.module,
                &resolver,
                layout,
                pointers,
                VarReads::Live,
                &math,
                &tasks,
                None,
            );
            let body = m.module.digital.as_ref().unwrap();
            // Mirror the per-device `compile()` split (lines 99-116 above):
            // a non-edge event control (`@*`/`@(sig)` — level-sensitive) is
            // combinational in this step-based execution model, so its body
            // folds directly into the plain combinational statement list
            // rather than being emitted as a (rejected) nested event
            // control. Without this fold a fused cone containing any
            // `always @(*) begin ... end` member dies with "nested event
            // control outside a top-level clocked block" — the per-device
            // path never hits this because it splits first.
            for stmt in &body.stmts {
                match stmt {
                    Stmt::EventControl { events, body, .. } if is_clocked(events) => {
                        // Already rejected at line 489 above; defensive only.
                        return Err(CodegenError::unsupported(format!(
                            "network fusion of clocked module `{}` (comb-only cones for now)",
                            m.module.name
                        )));
                    }
                    Stmt::EventControl { body, .. } => b.emit_stmt(body)?,
                    other => b.emit_stmt(other)?,
                }
            }
        }

        builder.ins().return_(&[]);
        builder.finalize();
        jit.define_function(func_id, &mut ctx)
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        jit.finalize_definitions()
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        let func: NetworkCombFn =
            unsafe { std::mem::transmute(jit.get_finalized_function(func_id)) };
        Ok(NetworkComb { func, _jit: jit.into() })
    }
}
