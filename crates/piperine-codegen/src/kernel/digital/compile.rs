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

use piperine_lang::ast::{BinOp, EventExpr, Expr, Stmt, UnOp};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::NumberLit;

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
        // §8.5.1 (T62/LRMW2-59): `$strobe`/`$monitor` defer through this
        // trampoline instead of `pip_task_display` — a distinct, smaller
        // signature (no argc/argv yet, see its own doc comment).
        {
            let mut sig = jit.make_signature();
            sig.params.push(AbiParam::new(types::I64)); // ctx: *mut SimCtx
            sig.params.push(AbiParam::new(types::I32)); // fmt_id
            let id = jit
                .declare_function("pip_task_defer_monitor", Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            task_ids.insert("pip_task_defer_monitor", id);
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
        // `Vec<EventExpr>` (owned), not `&[EventExpr]`: an analog-event
        // control (`@(cross(..))`/`@(above(..))`) is rewritten into its
        // Posedge/Negedge/Level equivalent below (`rewrite_analog_watch_events`)
        // before classification, so the events list a clocked entry carries
        // is not always a borrow of the original `Stmt`. The second field
        // is the *wrapper* `Stmt::EventControl` itself (not just its body)
        // — `compile_body_fn`'s "seq" call needs the wrapper to gate
        // execution on the matching `fired` flag; see the `stmts` build
        // below, which now reads straight off this vec instead of
        // re-deriving clocked-ness from the original (pre-rewrite) events.
        let mut clocked: Vec<(Vec<EventExpr>, &Stmt)> = Vec::new();
        for stmt in &self.body.stmts {
            match stmt {
                Stmt::EventControl { events, body, .. } => {
                    let rewritten = rewrite_analog_watch_events(events);
                    if is_clocked(&rewritten) {
                        clocked.push((rewritten, stmt));
                    } else {
                        // A non-edge event control (`@*`/`@(sig)` —
                        // level-sensitive) is combinational in this
                        // step-based (not event-scheduled) execution
                        // model: `comb` already re-evaluates every pass,
                        // which already gives "re-run on any input
                        // change" semantics, so its body folds directly
                        // into the plain combinational statement list
                        // rather than through the clocked/`fired`-flag
                        // path.
                        comb_stmts.push(body.as_ref());
                    }
                }
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

        // `comb` and `seq` both read the live variable banks (LRM
        // §8.5.3.3): a blocking (`=`) write lands live immediately, so a
        // later statement in the same block sees it, and `comb` sees
        // `seq`'s committed writes once the device has run both. A
        // non-blocking (`<=`) write's right-hand side is evaluated against
        // the same live banks but the write itself is staged (T23,
        // LRMW2-14) — see `DigitalAbi`'s own doc comment — so it is
        // invisible to a same-block read until the device commits it after
        // the call returns (LRM §8.5.3.4).
        let comb_id = self.compile_body_fn("comb", &comb_stmts, VarReads::Live)?;
        let seq_id = if clocked.is_empty() {
            None
        } else {
            // Read straight off `clocked` (already the exact set + order
            // `clocked_specs` was built from above) rather than
            // re-filtering `self.body.stmts` by `is_clocked` on the
            // ORIGINAL events — an analog-event control's original events
            // (`Level(Call("cross", ..))`) never satisfy `is_clocked`, only
            // its rewritten form does, so re-deriving here would silently
            // drop those entries and desync `clocked_specs`'s indices from
            // `compile_body_fn`'s per-entry `clocked_index` counter.
            let stmts: Vec<&Stmt> = clocked.iter().map(|(_, s)| *s).collect();
            Some(self.compile_body_fn("seq", &stmts, VarReads::Live)?)
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
        // §A.2.2.2 (LRMW3-09): resolve each output's authored keyword pair
        // (if any) into the pair of `Strength` levels
        // `device::digital::DigitalInstance::push_event` picks between by
        // the value it's about to emit.
        let mut output_strength: Vec<Option<(super::abi::Strength, super::abi::Strength)>> =
            Vec::with_capacity(output_names.len());
        for name in &output_names {
            output_strength.push(match self.body.output_drive_strength.get(name) {
                Some((l0, l1)) => Some(super::abi::parse_drive_strength_pair(l0, l1)?),
                None => None,
            });
        }

        // T26/LRMW2-26: LRM's own `` `timescale `` -> precision-grid rule
        // (§8.4.3.3): the grid step is `10^precision_exponent` seconds.
        let timescale_grid = self.module.timescale.map(|ts| 10f64.powi(ts.precision_exponent));

        Ok(DigitalKernel {
            name: self.module.name.clone(),
            inputs: self.body.inputs.clone(),
            outputs: self.body.outputs.clone(),
            input_names,
            output_names,
            output_strength,
            layout: self.layout,
            clocked_blocks: clocked_specs,
            num_watch_terms: watch_terms.len(),
            reg_inits,
            param_index,
            comb,
            seq,
            watch,
            timescale_grid,
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
            // `is_clocked` is intentionally NOT re-checked here (it used
            // to be): the caller passes either `comb_stmts` (never an
            // `EventControl` wrapper — `compile`'s classification loop
            // above already unwrapped every non-clocked control's body)
            // or `seq`'s `stmts` (every entry an `EventControl` wrapper
            // that classification already decided is clocked, including
            // an analog-event control rewritten to Posedge/Negedge/Level
            // form — `is_clocked` on the ORIGINAL un-rewritten events
            // would wrongly say "no" for those). So every `EventControl`
            // reaching this loop is clocked by construction; the
            // `clocked_index` counter's order matches `clocked_specs`'s
            // because both are built from the same `clocked` vec in
            // `compile`.
            let mut clocked_index = 0usize;
            for stmt in stmts {
                if let Stmt::EventControl { body, .. } = stmt {
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
            outputs_nba: load_field(&mut builder, AbiField::OutputsNba),
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
/// classification. Callers pass this the ALREADY-rewritten events list
/// (`rewrite_analog_watch_events`), so an analog-event control
/// (`@(cross(..))`/`@(above(..))`, LRM §7.3.5) reads as clocked too once its
/// `Level(Call(..))` form has been turned into `Posedge`/`Negedge`/`Level`
/// on the crossing predicate.
fn is_clocked(events: &[EventExpr]) -> bool {
    events.iter().any(|e| matches!(e, EventExpr::Posedge(_) | EventExpr::Negedge(_)))
}

/// LRM §7.3.5 "Detecting continuous events in a discrete context": a
/// digital block's event control may be sensitive to an analog event
/// function (`cross`/`above`) instead of a digital signal — these parse as
/// an ordinary `Level(Call { name: "cross"/"above", .. })` event (the
/// grammar has no dedicated production for them), which `is_clocked` would
/// otherwise never recognize, sending the block down the *combinational*
/// path: re-evaluated (and its body re-run) every digital pass instead of
/// once per crossing. Rewritten here, before classification, into the
/// equivalent digital watch form: `cross(expr, dir)` -> a boolean predicate
/// `expr > 0.0`, watched as `Posedge`/`Negedge`/`Level` per `dir`'s
/// Rising/Falling/Either (the same mapping `flatten/analog.rs`'s
/// `cross_dir_from_arg` uses for the analog-domain form); `above(expr)` ->
/// `Posedge` on `expr > 0.0` (fires once, on the negative-or-zero -> positive
/// transition). `timer`/`absdelta` are time-scheduled / delta-threshold
/// conditions, not level-watchable through this mechanism, so they are left
/// unrewritten — SPEC_DEVIATION.md 1.2's disclosed remainder for this task,
/// not a regression (their pre-fix behavior — folding into `comb`, re-run
/// every pass — is unchanged).
///
/// A control with events already Posedge/Negedge (a genuine digital
/// clocked block), a bare digital signal (`Level(Ident(..))`), or
/// `ImplicitAll` passes through unchanged.
fn rewrite_analog_watch_events(events: &[EventExpr]) -> Vec<EventExpr> {
    events
        .iter()
        .map(|e| match e {
            EventExpr::Level(expr) => match analog_event_as_edge(expr) {
                Some((EdgeKind::Rising, watched)) => EventExpr::Posedge(watched),
                Some((EdgeKind::Falling, watched)) => EventExpr::Negedge(watched),
                Some((EdgeKind::Any, watched)) => EventExpr::Level(watched),
                None => e.clone(),
            },
            other => other.clone(),
        })
        .collect()
}

const NOSPAN: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

/// Recognize `cross(expr[, dir])` / `above(expr)` (LRM §5.10.3.1/§5.10.3.2)
/// and return the boolean watch expression (`expr > 0.0`) plus the edge
/// kind implied by the crossing direction.
fn analog_event_as_edge(expr: &Expr) -> Option<(EdgeKind, Expr)> {
    let Expr::Call { name, args, .. } = expr else { return None };
    let watched = args.first()?.clone();
    let predicate = Expr::Binary {
        span: NOSPAN,
        op: BinOp::Gt,
        lhs: Box::new(watched),
        rhs: Box::new(Expr::Number { span: NOSPAN, value: NumberLit::Real(0.0) }),
    };
    match name.as_str() {
        "cross" => Some((cross_dir_edge(args.get(1)), predicate)),
        "above" => Some((EdgeKind::Rising, predicate)),
        _ => None,
    }
}

/// `cross`'s optional 2nd arg (LRM §5.10.3.1): `+1`/`-1`/omitted ->
/// Rising/Falling/Either, an LRM-mandated compile-time constant. Mirrors
/// `flatten/analog.rs::cross_dir_from_arg`'s literal handling (a negative
/// literal lexes as `Unary{Neg, Number(1)}`, not a single negative token).
fn cross_dir_edge(arg: Option<&Expr>) -> EdgeKind {
    let Some(expr) = arg else { return EdgeKind::Any };
    let (expr, negate) = match expr {
        Expr::Unary { op: UnOp::Neg, operand, .. } => (operand.as_ref(), true),
        other => (other, false),
    };
    let n = match expr {
        Expr::Number { value: NumberLit::Integer(n), .. } => *n,
        Expr::Number { value: NumberLit::Real(r), .. } => *r as i64,
        _ => return EdgeKind::Any,
    };
    match if negate { -n } else { n } {
        1 => EdgeKind::Rising,
        -1 => EdgeKind::Falling,
        _ => EdgeKind::Any,
    }
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

/// Whether variable reads see the live banks. `comb` and `seq` both compile
/// with `Live` today (T23, LRMW2-14) — a blocking write's effect on a later
/// same-block read, and a non-blocking write's staged (not-yet-committed)
/// value being invisible to a same-block read, are both handled on the
/// *write* side (`emit_assign`'s blocking/non-blocking split), not by
/// switching the read bank. `PreEdge` is kept as a distinct, documented mode
/// (rather than deleted outright) in case a future body legitimately needs a
/// blanket pre-call snapshot read; nothing constructs it today.
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
    /// Non-blocking (`<=`) write staging bank for `outputs` — see
    /// `DigitalAbi::outputs_nba`.
    pub outputs_nba: Value,
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
        {
            let mut sig = jit.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I32));
            let id = jit
                .declare_function("pip_task_defer_monitor", Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            task_ids.insert("pip_task_defer_monitor", id);
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
                // A fused cone has no per-member staging buffer to alias a
                // non-blocking write into, so `vars_int_old`/`vars_real_old`/
                // `outputs_nba` alias straight back onto the live pointers —
                // `<=` inside a fused comb-only member degrades to blocking
                // (`=`) semantics, same as before T23. Disclosed scope limit:
                // network fusion already rejects clocked bodies outright
                // (`compile`'s own check above), so this only affects the
                // rare non-blocking assign inside a plain `always @*`/`@(sig)`
                // member — no clocked-block idiom (the case T23 fixes) can
                // reach this path.
                vars_int_old: vint,
                vars_real_old: vreal,
                vars_int: vint,
                vars_real: vreal,
                params: par,
                fired: nets_ptr, // unused: no clocked blocks in a fused member
                sim: sim_ptr,
                analog_voltages: analog_ptr,
                outputs_nba: nets_ptr,
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
