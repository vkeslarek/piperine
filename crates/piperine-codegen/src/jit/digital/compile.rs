//! Digital kernel compilation: an [`crate::ir::IrDigitalBody`] to native
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

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, AbiParam, FuncRef, InstBuilder, MemFlags, Signature, Value};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use crate::ir::{
    IrBinOp, IrDigitalBody, IrExpr, IrModule, IrStmt, IrType, IrUnOp, Lval,
    NodeId, Pattern, SimQuery, Trit, VarId,
};

use super::super::flatten::Inliner;
use super::super::{math, CodegenError};

use super::abi::*;
use super::layout::*;

pub(crate) struct DigitalCompiler<'m> {
    module: &'m IrModule,
    body: &'m IrDigitalBody,
    layout: DigitalLayout,
    jit: JITModule,
    math_ids: HashMap<&'static str, FuncId>,
    fb_ctx: FunctionBuilderContext,
}

impl<'m> DigitalCompiler<'m> {
    pub(crate) fn new(module: &'m IrModule) -> Result<Self, CodegenError> {
        let body = module
            .digital
            .as_ref()
            .ok_or_else(|| CodegenError::Invalid(format!("`{}` has no digital body", module.name)))?;

        let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        for f in math::MATH_FNS {
            jit_builder.symbol(f.name, f.symbol);
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

        Ok(Self {
            layout: DigitalLayout::build(module, body),
            module,
            body,
            jit,
            math_ids,
            fb_ctx: FunctionBuilderContext::new(),
        })
    }

    pub(crate) fn compile(mut self) -> Result<DigitalKernel, CodegenError> {
        // Split the body: combinational statements vs clocked blocks, and
        // collect register power-on inits.
        let mut comb_stmts: Vec<&IrStmt> = Vec::new();
        let mut clocked: Vec<(&crate::ir::DigitalEvent, &[IrStmt])> = Vec::new();
        let mut reg_inits = Vec::new();
        for stmt in &self.body.stmts {
            match stmt {
                IrStmt::ClockedBlock { event, body } => clocked.push((event, body)),
                IrStmt::VarDecl { var, init: Some(init) } if self.body.regs.contains(var) => {
                    reg_inits.push(RegInit { var: *var, init: init.clone() });
                }
                other => comb_stmts.push(other),
            }
        }

        // Number the atomic watch terms across all clocked blocks.
        let mut watch_terms: Vec<IrExpr> = Vec::new();
        let mut clocked_specs = Vec::new();
        for (event, _) in &clocked {
            let mut terms = Vec::new();
            for (expr, edge) in event.terms() {
                let index = match watch_terms.iter().position(|t| t == expr) {
                    Some(i) => i,
                    None => {
                        watch_terms.push(expr.clone());
                        watch_terms.len() - 1
                    }
                };
                terms.push((index, edge));
            }
            clocked_specs.push(ClockedSpec { terms, is_initial: event.is_initial() });
        }

        // `comb` reads live variable values: after `seq` writes register
        // updates to the live bank, `comb` sees the new values and drives
        // outputs from them. Within a clocked block (`seq`), reads see
        // pre-edge values — register updates are non-blocking within the
        // block (SPEC §10.3: "within the block reads see the pre-edge
        // value, a chain of register writes is a pipeline").
        let comb_id = self.compile_body_fn("comb", &comb_stmts, VarReads::Live, None)?;
        let seq_id = if clocked.is_empty() {
            None
        } else {
            let stmts: Vec<&IrStmt> = self
                .body
                .stmts
                .iter()
                .filter(|s| matches!(s, IrStmt::ClockedBlock { .. }))
                .collect();
            Some(self.compile_body_fn("seq", &stmts, VarReads::PreEdge, None)?)
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

        Ok(DigitalKernel {
            name: self.module.name.clone(),
            inputs: self.body.inputs.clone(),
            outputs: self.body.outputs.clone(),
            layout: self.layout,
            clocked_blocks: clocked_specs,
            num_watch_terms: watch_terms.len(),
            reg_inits,
            comb,
            seq,
            watch,
            _jit: self.jit,
        })
    }

    /// Compile a statement-body function (`comb` or `seq`).
    pub(crate) fn compile_body_fn(
        &mut self,
        name: &str,
        stmts: &[&IrStmt],
        reads: VarReads,
        _unused: Option<()>,
    ) -> Result<FuncId, CodegenError> {
        self.build_fn(name, false, |emitter| {
            let mut clocked_index = 0usize;
            for stmt in stmts {
                if let IrStmt::ClockedBlock { body, .. } = stmt {
                    emitter.emit_guarded_block(clocked_index, body)?;
                    clocked_index += 1;
                } else {
                    emitter.emit_stmt(stmt)?;
                }
            }
            Ok(())
        }, reads)
    }

    /// Compile the watch function: each term's quad value to `out[i]`.
    pub(crate) fn compile_watch_fn(&mut self, name: &str, terms: &[IrExpr]) -> Result<FuncId, CodegenError> {
        self.build_fn(name, true, |emitter| {
            let out_ptr = emitter.watch_out.expect("watch fn has an out pointer");
            for (i, term) in terms.iter().enumerate() {
                let value = emitter.emit_expr(term)?;
                let quad = emitter.coerce(value, DigTy::Quad)?;
                emitter.builder.ins().store(
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
        body: impl FnOnce(&mut DigitalEmitter) -> Result<(), CodegenError>,
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

        let mut emitter = DigitalEmitter {
            builder: &mut builder,
            module: self.module,
            layout: &self.layout,
            pointers,
            reads,
            math: &math,
            watch_out,
        };
        body(&mut emitter)?;

        builder.ins().return_(&[]);
        builder.finalize();

        self.jit
            .define_function(func_id, &mut ctx)
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        Ok(func_id)
    }
}

// ─── Emitter ──────────────────────────────────────────────────────────────────

/// Whether variable reads see the live banks (comb) or the pre-edge copies
/// (seq register semantics).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarReads {
    Live,
    PreEdge,
}

/// Loaded ABI pointer table.
struct Pointers {
    inputs: Value,
    outputs: Value,
    vars_int_old: Value,
    vars_real_old: Value,
    vars_int: Value,
    vars_real: Value,
    params: Value,
    fired: Value,
    sim: Value,
    analog_voltages: Value,
}

/// A value plus its digital type.
#[derive(Clone, Copy)]
struct Typed {
    value: Value,
    ty: DigTy,
}

impl Typed {
    fn real(value: Value) -> Self {
        Self { value, ty: DigTy::Real }
    }

    fn int(value: Value) -> Self {
        Self { value, ty: DigTy::Int }
    }

    fn quad(value: Value) -> Self {
        Self { value, ty: DigTy::Quad }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DigTy {
    /// Two-state integer/boolean (`i64`).
    Int,
    /// `f64`.
    Real,
    /// Four-state logic in `i64`: 0, 1, 2 = X, 3 = Z.
    Quad,
}

struct DigitalEmitter<'a, 'f, 'm> {
    builder: &'a mut FunctionBuilder<'f>,
    module: &'m IrModule,
    layout: &'a DigitalLayout,
    pointers: Pointers,
    reads: VarReads,
    math: &'a HashMap<&'static str, FuncRef>,
    watch_out: Option<Value>,
}

impl DigitalEmitter<'_, '_, '_> {
    // ── Statements ──

    fn emit_stmt(&mut self, stmt: &IrStmt) -> Result<(), CodegenError> {
        match stmt {
            IrStmt::Assign { lval, expr } => self.emit_assign(lval, expr),
            IrStmt::VarDecl { var, init } => match init {
                Some(init) => self.emit_assign(&Lval::Var(*var), init),
                None => Ok(()),
            },
            IrStmt::If { cond, then_, else_ } => {
                let cond = self.emit_expr(cond)?;
                let flag = self.truthy(cond)?;
                self.emit_branch(flag, then_, else_)
            }
            IrStmt::Match { scrutinee, arms, default } => {
                let scrutinee = self.emit_expr(scrutinee)?;
                self.emit_match(scrutinee, arms, default)
            }
            IrStmt::Diagnostic { .. } => Ok(()), // collected, not executed (SPEC §12)
            IrStmt::ClockedBlock { .. } => Err(CodegenError::Invalid(
                "nested clocked block — clocked blocks must be top-level".into(),
            )),
            other => Err(CodegenError::unsupported(format!(
                "statement {other:?} in a digital body"
            ))),
        }
    }

    /// `if fired[index] { body }` around a clocked block's statements.
    fn emit_guarded_block(&mut self, index: usize, body: &[IrStmt]) -> Result<(), CodegenError> {
        let fired = self.builder.ins().load(
            types::I64,
            MemFlags::trusted(),
            self.pointers.fired,
            (index * 8) as i32,
        );
        let zero = self.builder.ins().iconst(types::I64, 0);
        let flag = self.builder.ins().icmp(IntCC::NotEqual, fired, zero);
        self.emit_branch(flag, body, &[])
    }

    /// Structured two-way branch over statement bodies.
    fn emit_branch(
        &mut self,
        flag: Value,
        then_: &[IrStmt],
        else_: &[IrStmt],
    ) -> Result<(), CodegenError> {
        let then_block = self.builder.create_block();
        let else_block = self.builder.create_block();
        let merge_block = self.builder.create_block();

        self.builder.ins().brif(flag, then_block, &[], else_block, &[]);

        self.builder.switch_to_block(then_block);
        self.builder.seal_block(then_block);
        for stmt in then_ {
            self.emit_stmt(stmt)?;
        }
        self.builder.ins().jump(merge_block, &[]);

        self.builder.switch_to_block(else_block);
        self.builder.seal_block(else_block);
        for stmt in else_ {
            self.emit_stmt(stmt)?;
        }
        self.builder.ins().jump(merge_block, &[]);

        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);
        Ok(())
    }

    fn emit_match(
        &mut self,
        scrutinee: Typed,
        arms: &[(Pattern, Vec<IrStmt>)],
        default: &[IrStmt],
    ) -> Result<(), CodegenError> {
        match arms {
            [] => {
                for stmt in default {
                    self.emit_stmt(stmt)?;
                }
                Ok(())
            }
            [(pattern, body), rest @ ..] => {
                let flag = self.pattern_flag(scrutinee, pattern)?;
                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                self.builder.ins().brif(flag, then_block, &[], else_block, &[]);

                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                for stmt in body {
                    self.emit_stmt(stmt)?;
                }
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                self.emit_match(scrutinee, rest, default)?;
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);
                Ok(())
            }
        }
    }

    /// The i1 flag for "scrutinee matches pattern" (case equality: X matches X).
    fn pattern_flag(&mut self, scrutinee: Typed, pattern: &Pattern) -> Result<Value, CodegenError> {
        match pattern {
            Pattern::Wildcard => Ok(self.builder.ins().iconst(types::I8, 1)),
            Pattern::Value(expr) => {
                let value = self.emit_expr(expr)?;
                let value = self.coerce(value, scrutinee.ty)?;
                match scrutinee.ty {
                    DigTy::Real => Ok(self
                        .builder
                        .ins()
                        .fcmp(FloatCC::Equal, scrutinee.value, value.value)),
                    DigTy::Int | DigTy::Quad => Ok(self
                        .builder
                        .ins()
                        .icmp(IntCC::Equal, scrutinee.value, value.value)),
                }
            }
            Pattern::BitPattern(trits) => match trits.as_slice() {
                [Trit::DontCare] => Ok(self.builder.ins().iconst(types::I8, 1)),
                [trit] => {
                    let target = i64::from(*trit == Trit::One);
                    let scrutinee = self.coerce(scrutinee, DigTy::Quad)?;
                    let target = self.builder.ins().iconst(types::I64, target);
                    Ok(self
                        .builder
                        .ins()
                        .icmp(IntCC::Equal, scrutinee.value, target))
                }
                _ => Err(CodegenError::unsupported(
                    "multi-bit patterns in a digital `match` (bus signals)",
                )),
            },
        }
    }

    fn emit_assign(&mut self, lval: &Lval, expr: &IrExpr) -> Result<(), CodegenError> {
        let value = self.emit_expr(expr)?;
        match lval {
            Lval::Var(var) => {
                let info = self.module.symbols.var(*var);
                match info.ty {
                    IrType::Real => {
                        let slot = self.layout.real_slot(*var).expect("layout covers all vars");
                        let value = self.coerce(value, DigTy::Real)?;
                        self.builder.ins().store(
                            MemFlags::trusted(),
                            value.value,
                            self.pointers.vars_real,
                            (slot * 8) as i32,
                        );
                    }
                    IrType::Quad => {
                        let slot = self.layout.int_slot(*var).expect("layout covers all vars");
                        let value = self.coerce(value, DigTy::Quad)?;
                        self.builder.ins().store(
                            MemFlags::trusted(),
                            value.value,
                            self.pointers.vars_int,
                            (slot * 8) as i32,
                        );
                    }
                    IrType::Integer | IrType::Bool => {
                        let slot = self.layout.int_slot(*var).expect("layout covers all vars");
                        let value = self.coerce(value, DigTy::Int)?;
                        self.builder.ins().store(
                            MemFlags::trusted(),
                            value.value,
                            self.pointers.vars_int,
                            (slot * 8) as i32,
                        );
                    }
                }
                Ok(())
            }
            Lval::Net(node) => {
                let index = self.layout.output_index.get(node).copied().ok_or_else(|| {
                    CodegenError::Invalid(format!(
                        "assignment to net `{}` which is not a digital output",
                        self.module.symbols.node(*node).name
                    ))
                })?;
                let value = self.coerce(value, DigTy::Quad)?;
                self.builder.ins().store(
                    MemFlags::trusted(),
                    value.value,
                    self.pointers.outputs,
                    (index * 8) as i32,
                );
                Ok(())
            }
            Lval::Index(..) | Lval::Slice(..) => Err(CodegenError::unsupported(
                "indexed/sliced assignment targets (bus signals)",
            )),
        }
    }

    // ── Expressions ──

    fn emit_expr(&mut self, expr: &IrExpr) -> Result<Typed, CodegenError> {
        match expr {
            IrExpr::Real(v) => Ok(Typed::real(self.builder_f64(*v))),
            IrExpr::Int(v) => Ok(Typed::int(self.builder_i64(*v))),
            IrExpr::Bool(b) => Ok(Typed::int(self.builder_i64(i64::from(*b)))),
            IrExpr::Quad(q) => Ok(Typed::quad(self.builder_i64(i64::from(*q)))),

            IrExpr::Param(id) => {
                let value = self.builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    self.pointers.params,
                    (id.0 * 8) as i32,
                );
                match self.module.symbols.param(*id).ty {
                    IrType::Real => Ok(Typed::real(value)),
                    _ => {
                        let as_int = self.builder.ins().fcvt_to_sint(types::I64, value);
                        Ok(Typed::int(as_int))
                    }
                }
            }

            IrExpr::Var(id) => Ok(self.load_var(*id)),

            IrExpr::Net(node) => self.load_net(*node),

            IrExpr::Sim(query) => self.emit_sim(query),

            IrExpr::Unary(op, x) => {
                let x = self.emit_expr(x)?;
                self.emit_unary(*op, x)
            }
            IrExpr::Binary(op, a, b) => {
                let a = self.emit_expr(a)?;
                let b = self.emit_expr(b)?;
                self.emit_binary(*op, a, b)
            }
            IrExpr::Select(c, t, e) => {
                let cond = self.emit_expr(c)?;
                let flag = self.truthy(cond)?;
                let then_ = self.emit_expr(t)?;
                let else_ = self.emit_expr(e)?;
                let (then_, else_) = self.unify(then_, else_)?;
                let value = self.builder.ins().select(flag, then_.value, else_.value);
                Ok(Typed { value, ty: then_.ty })
            }
            IrExpr::MathCall(name, args) => self.emit_math(name, args),
            IrExpr::Call(id, args) => {
                // Inline the user function symbolically, then emit the
                // resulting expression.
                let mut inliner = Inliner::new(self.module);
                let expanded = inliner.expand(*id, args.to_vec())?;
                self.emit_expr(&expanded)
            }

            IrExpr::Branch { plus, minus, .. } => {
                // A2D bridge: read the analog voltage difference
                // V(plus) − V(minus) from the analog_voltages array.
                // Ground (NodeId::GROUND) is always 0 V.
                let load_analog = |builder: &mut FunctionBuilder, node: NodeId| -> Result<Value, CodegenError> {
                    if node.is_ground() {
                        Ok(builder.ins().f64const(0.0))
                    } else if let Some(idx) = self.layout.analog_index(node) {
                        Ok(builder.ins().load(
                            types::F64,
                            MemFlags::trusted(),
                            self.pointers.analog_voltages,
                            (idx * 8) as i32,
                        ))
                    } else {
                        Err(CodegenError::Invalid(format!(
                            "analog node `{}` is not in the analog voltage array",
                            self.module.symbols.node(node).name
                        )))
                    }
                };
                let vp = load_analog(self.builder, *plus)?;
                let vm = load_analog(self.builder, *minus)?;
                Ok(Typed::real(self.builder.ins().fsub(vp, vm)))
            }

            IrExpr::State(_) | IrExpr::AcStim { .. } => Err(
                CodegenError::Invalid("analog state operator in a digital body".into()),
            ),
            IrExpr::Array(_) | IrExpr::Index(..) | IrExpr::Slice(..) => Err(
                CodegenError::unsupported("bus/vector expressions in digital codegen"),
            ),
        }
    }

    /// Read a net (digital input or output) as a quad value.
    fn load_net(&mut self, node: NodeId) -> Result<Typed, CodegenError> {
        if let Some(&i) = self.layout.input_index.get(&node) {
            let value = self.builder.ins().load(
                types::I64,
                MemFlags::trusted(),
                self.pointers.inputs,
                (i * 8) as i32,
            );
            return Ok(Typed::quad(value));
        }
        if let Some(&i) = self.layout.output_index.get(&node) {
            let value = self.builder.ins().load(
                types::I64,
                MemFlags::trusted(),
                self.pointers.outputs,
                (i * 8) as i32,
            );
            return Ok(Typed::quad(value));
        }
        Err(CodegenError::Invalid(format!(
            "net `{}` is neither a digital input nor output",
            self.module.symbols.node(node).name
        )))
    }

    fn load_var(&mut self, var: VarId) -> Typed {
        let info = self.module.symbols.var(var);
        match info.ty {
            IrType::Real => {
                let slot = self.layout.real_slot(var).expect("layout covers all vars");
                let bank = match self.reads {
                    VarReads::Live => self.pointers.vars_real,
                    VarReads::PreEdge => self.pointers.vars_real_old,
                };
                let value =
                    self.builder
                        .ins()
                        .load(types::F64, MemFlags::trusted(), bank, (slot * 8) as i32);
                Typed::real(value)
            }
            ty => {
                let slot = self.layout.int_slot(var).expect("layout covers all vars");
                let bank = match self.reads {
                    VarReads::Live => self.pointers.vars_int,
                    VarReads::PreEdge => self.pointers.vars_int_old,
                };
                let value =
                    self.builder
                        .ins()
                        .load(types::I64, MemFlags::trusted(), bank, (slot * 8) as i32);
                match ty {
                    IrType::Quad => Typed::quad(value),
                    _ => Typed::int(value),
                }
            }
        }
    }

    fn emit_sim(&mut self, query: &SimQuery) -> Result<Typed, CodegenError> {
        match query {
            SimQuery::Abstime => {
                let value = self.builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    self.pointers.sim,
                    8, // SimCtx.abstime
                );
                Ok(Typed::real(value))
            }
            SimQuery::Temperature => {
                let value = self.builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    self.pointers.sim,
                    0, // SimCtx.temperature
                );
                Ok(Typed::real(value))
            }
            other => Err(CodegenError::unsupported(format!(
                "simulator query {other:?} in a digital body"
            ))),
        }
    }

    fn emit_unary(&mut self, op: IrUnOp, x: Typed) -> Result<Typed, CodegenError> {
        match (op, x.ty) {
            (IrUnOp::Neg, DigTy::Real) => Ok(Typed::real(self.builder_fneg(x.value))),
            (IrUnOp::Neg, DigTy::Int) => Ok(Typed::int(self.builder_ineg(x.value))),
            (IrUnOp::Not | IrUnOp::BitNot, DigTy::Quad) => {
                let x = self.normalize_z(x.value);
                Ok(Typed::quad(self.quad_not(x)))
            }
            (IrUnOp::Not, DigTy::Int) => {
                let zero = self.builder_i64(0);
                let flag = self.builder.ins().icmp(IntCC::Equal, x.value, zero);
                Ok(Typed::int(self.builder_flag_i64(flag)))
            }
            (IrUnOp::Not, DigTy::Real) => {
                let zero = self.builder_f64(0.0);
                let flag = self.builder.ins().fcmp(FloatCC::Equal, x.value, zero);
                Ok(Typed::int(self.builder_flag_i64(flag)))
            }
            (IrUnOp::BitNot, DigTy::Int) => Ok(Typed::int(self.builder.ins().bnot(x.value))),
            // A reduction over a scalar is the scalar (buses are rejected).
            (IrUnOp::RedAnd | IrUnOp::RedOr | IrUnOp::RedXor, DigTy::Quad | DigTy::Int) => Ok(x),
            (op, ty) => Err(CodegenError::unsupported(format!(
                "unary {op:?} on {ty:?} in digital codegen"
            ))),
        }
    }

    fn emit_binary(&mut self, op: IrBinOp, a: Typed, b: Typed) -> Result<Typed, CodegenError> {
        use IrBinOp::*;
        // Quad logic when either side is 4-state and the operator is logical.
        let quadish = a.ty == DigTy::Quad || b.ty == DigTy::Quad;
        match op {
            And | Or | BitAnd | BitOr | BitXor if quadish => {
                let a = self.coerce(a, DigTy::Quad)?;
                let b = self.coerce(b, DigTy::Quad)?;
                let av = self.normalize_z(a.value);
                let bv = self.normalize_z(b.value);
                let value = match op {
                    And | BitAnd => self.quad_and(av, bv),
                    Or | BitOr => self.quad_or(av, bv),
                    BitXor => self.quad_xor(av, bv),
                    _ => unreachable!(),
                };
                return Ok(Typed::quad(value));
            }
            Eq | Ne if quadish => {
                let a = self.coerce(a, DigTy::Quad)?;
                let b = self.coerce(b, DigTy::Quad)?;
                let av = self.normalize_z(a.value);
                let bv = self.normalize_z(b.value);
                let value = self.quad_eq(av, bv, op == Ne);
                return Ok(Typed::quad(value));
            }
            _ => {}
        }

        // Real arithmetic when either side is real.
        if a.ty == DigTy::Real || b.ty == DigTy::Real {
            let a = self.coerce(a, DigTy::Real)?;
            let b = self.coerce(b, DigTy::Real)?;
            let fcmp = |e: &mut Self, cc: FloatCC| {
                let flag = e.builder.ins().fcmp(cc, a.value, b.value);
                Typed { value: e.builder_flag_i64(flag), ty: DigTy::Int }
            };
            return match op {
                Add => Ok(Typed::real(self.builder.ins().fadd(a.value, b.value))),
                Sub => Ok(Typed::real(self.builder.ins().fsub(a.value, b.value))),
                Mul => Ok(Typed::real(self.builder.ins().fmul(a.value, b.value))),
                Div => Ok(Typed::real(self.builder.ins().fdiv(a.value, b.value))),
                Pow => {
                    let result = self.call_math("pow", &[a.value, b.value])?;
                    Ok(Typed::real(result))
                }
                Eq => Ok(fcmp(self, FloatCC::Equal)),
                Ne => Ok(fcmp(self, FloatCC::NotEqual)),
                Lt => Ok(fcmp(self, FloatCC::LessThan)),
                Le => Ok(fcmp(self, FloatCC::LessThanOrEqual)),
                Gt => Ok(fcmp(self, FloatCC::GreaterThan)),
                Ge => Ok(fcmp(self, FloatCC::GreaterThanOrEqual)),
                other => Err(CodegenError::unsupported(format!(
                    "binary {other:?} on reals in digital codegen"
                ))),
            };
        }

        // Two-state integer path (quads coerce through their 0/1 values;
        // X/Z in arithmetic is rejected by coerce).
        let a = self.coerce(a, DigTy::Int)?;
        let b = self.coerce(b, DigTy::Int)?;
        let icmp = |e: &mut Self, cc: IntCC| {
            let flag = e.builder.ins().icmp(cc, a.value, b.value);
            Typed { value: e.builder_flag_i64(flag), ty: DigTy::Int }
        };
        match op {
            Add => Ok(Typed::int(self.builder.ins().iadd(a.value, b.value))),
            Sub => Ok(Typed::int(self.builder.ins().isub(a.value, b.value))),
            Mul => Ok(Typed::int(self.builder.ins().imul(a.value, b.value))),
            Div => Ok(Typed::int(self.builder.ins().sdiv(a.value, b.value))),
            Rem => Ok(Typed::int(self.builder.ins().srem(a.value, b.value))),
            BitAnd => Ok(Typed::int(self.builder.ins().band(a.value, b.value))),
            BitOr => Ok(Typed::int(self.builder.ins().bor(a.value, b.value))),
            BitXor => Ok(Typed::int(self.builder.ins().bxor(a.value, b.value))),
            Shl => Ok(Typed::int(self.builder.ins().ishl(a.value, b.value))),
            Shr => Ok(Typed::int(self.builder.ins().ushr(a.value, b.value))),
            Eq => Ok(icmp(self, IntCC::Equal)),
            Ne => Ok(icmp(self, IntCC::NotEqual)),
            Lt => Ok(icmp(self, IntCC::SignedLessThan)),
            Le => Ok(icmp(self, IntCC::SignedLessThanOrEqual)),
            Gt => Ok(icmp(self, IntCC::SignedGreaterThan)),
            Ge => Ok(icmp(self, IntCC::SignedGreaterThanOrEqual)),
            And | Or => {
                let zero = self.builder_i64(0);
                let a_true = self.builder.ins().icmp(IntCC::NotEqual, a.value, zero);
                let b_true = self.builder.ins().icmp(IntCC::NotEqual, b.value, zero);
                let combined = if op == And {
                    self.builder.ins().band(a_true, b_true)
                } else {
                    self.builder.ins().bor(a_true, b_true)
                };
                Ok(Typed::int(self.builder_flag_i64(combined)))
            }
            Pow => Err(CodegenError::unsupported("integer `**` in digital codegen")),
        }
    }

    fn emit_math(&mut self, name: &str, args: &[IrExpr]) -> Result<Typed, CodegenError> {
        let values = args
            .iter()
            .map(|a| {
                let v = self.emit_expr(a)?;
                Ok(self.coerce(v, DigTy::Real)?.value)
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let result = self.call_math(name, &values)?;
        Ok(Typed::real(result))
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

    // ── Type plumbing ──

    /// Truthiness flag: quad → `== 1`, int → `!= 0`, real → `!= 0.0`.
    fn truthy(&mut self, v: Typed) -> Result<Value, CodegenError> {
        match v.ty {
            DigTy::Quad => {
                let one = self.builder_i64(1);
                Ok(self.builder.ins().icmp(IntCC::Equal, v.value, one))
            }
            DigTy::Int => {
                let zero = self.builder_i64(0);
                Ok(self.builder.ins().icmp(IntCC::NotEqual, v.value, zero))
            }
            DigTy::Real => {
                let zero = self.builder_f64(0.0);
                Ok(self.builder.ins().fcmp(FloatCC::NotEqual, v.value, zero))
            }
        }
    }

    fn unify(&mut self, a: Typed, b: Typed) -> Result<(Typed, Typed), CodegenError> {
        if a.ty == b.ty {
            return Ok((a, b));
        }
        if a.ty == DigTy::Real || b.ty == DigTy::Real {
            return Ok((self.coerce(a, DigTy::Real)?, self.coerce(b, DigTy::Real)?));
        }
        // Int vs Quad: 0/1 integers lift losslessly into 4-state.
        Ok((self.coerce(a, DigTy::Quad)?, self.coerce(b, DigTy::Quad)?))
    }

    fn coerce(&mut self, v: Typed, ty: DigTy) -> Result<Typed, CodegenError> {
        if v.ty == ty {
            return Ok(v);
        }
        match (v.ty, ty) {
            (DigTy::Int, DigTy::Real) => {
                Ok(Typed::real(self.builder.ins().fcvt_from_sint(types::F64, v.value)))
            }
            (DigTy::Real, DigTy::Int) => {
                Ok(Typed::int(self.builder.ins().fcvt_to_sint(types::I64, v.value)))
            }
            // A 0/1 integer is already a valid quad; other values would be
            // wrong, but Int here means a boolean-producing expression.
            (DigTy::Int, DigTy::Quad) => Ok(Typed::quad(v.value)),
            // Quad → Int: SPEC says Boolean widens to Quad implicitly. A
            // `Bit` net (storage Boolean) is 2-state; its Quad encoding is
            // always 0 or 1. For genuine 4-state nets used in integer
            // context, X/Z collapse to 0 (2-state projection).
            (DigTy::Quad, DigTy::Int) => {
                let x = self.builder_i64(2);
                let z = self.builder_i64(3);
                let zero = self.builder_i64(0);
                let is_x = self.builder.ins().icmp(IntCC::Equal, v.value, x);
                let is_z = self.builder.ins().icmp(IntCC::Equal, v.value, z);
                let not_4state = self.builder.ins().bnot(is_x);
                let not_4state = self.builder.ins().band(not_4state, is_z);
                let _ = not_4state; // suppress unused; the logic below suffices
                // Map X (2) and Z (3) to 0; keep 0 and 1 as-is.
                let x_or_z = self.builder.ins().bor(is_x, is_z);
                let projected = self.builder.ins().select(x_or_z, zero, v.value);
                Ok(Typed::int(projected))
            }
            (DigTy::Quad, DigTy::Real) | (DigTy::Real, DigTy::Quad) => Err(
                CodegenError::unsupported("real ↔ 4-state conversion in digital codegen"),
            ),
            _ => unreachable!("same-type coercion handled above"),
        }
    }

    // ── Quad logic (values normalised so Z reads as X) ──

    /// Map Z (3) to X (2).
    fn normalize_z(&mut self, v: Value) -> Value {
        let three = self.builder_i64(3);
        let two = self.builder_i64(2);
        let is_z = self.builder.ins().icmp(IntCC::Equal, v, three);
        self.builder.ins().select(is_z, two, v)
    }

    fn quad_not(&mut self, v: Value) -> Value {
        // 0→1, 1→0, X→X.
        let zero = self.builder_i64(0);
        let one = self.builder_i64(1);
        let two = self.builder_i64(2);
        let is_zero = self.builder.ins().icmp(IntCC::Equal, v, zero);
        let is_one = self.builder.ins().icmp(IntCC::Equal, v, one);
        let inner = self.builder.ins().select(is_one, zero, two);
        self.builder.ins().select(is_zero, one, inner)
    }

    fn quad_and(&mut self, a: Value, b: Value) -> Value {
        // 0 dominates; 1&1 = 1; else X.
        let zero = self.builder_i64(0);
        let one = self.builder_i64(1);
        let two = self.builder_i64(2);
        let a_zero = self.builder.ins().icmp(IntCC::Equal, a, zero);
        let b_zero = self.builder.ins().icmp(IntCC::Equal, b, zero);
        let any_zero = self.builder.ins().bor(a_zero, b_zero);
        let a_one = self.builder.ins().icmp(IntCC::Equal, a, one);
        let b_one = self.builder.ins().icmp(IntCC::Equal, b, one);
        let both_one = self.builder.ins().band(a_one, b_one);
        let inner = self.builder.ins().select(both_one, one, two);
        self.builder.ins().select(any_zero, zero, inner)
    }

    fn quad_or(&mut self, a: Value, b: Value) -> Value {
        // 1 dominates; 0|0 = 0; else X.
        let zero = self.builder_i64(0);
        let one = self.builder_i64(1);
        let two = self.builder_i64(2);
        let a_one = self.builder.ins().icmp(IntCC::Equal, a, one);
        let b_one = self.builder.ins().icmp(IntCC::Equal, b, one);
        let any_one = self.builder.ins().bor(a_one, b_one);
        let a_zero = self.builder.ins().icmp(IntCC::Equal, a, zero);
        let b_zero = self.builder.ins().icmp(IntCC::Equal, b, zero);
        let both_zero = self.builder.ins().band(a_zero, b_zero);
        let inner = self.builder.ins().select(both_zero, zero, two);
        self.builder.ins().select(any_one, one, inner)
    }

    fn quad_xor(&mut self, a: Value, b: Value) -> Value {
        // X poisons; otherwise a ^ b.
        let two = self.builder_i64(2);
        let a_x = self.builder.ins().icmp(IntCC::Equal, a, two);
        let b_x = self.builder.ins().icmp(IntCC::Equal, b, two);
        let any_x = self.builder.ins().bor(a_x, b_x);
        let xor = self.builder.ins().bxor(a, b);
        self.builder.ins().select(any_x, two, xor)
    }

    fn quad_eq(&mut self, a: Value, b: Value, negate: bool) -> Value {
        // X poisons; otherwise 0/1 comparison result.
        let zero = self.builder_i64(0);
        let one = self.builder_i64(1);
        let two = self.builder_i64(2);
        let a_x = self.builder.ins().icmp(IntCC::Equal, a, two);
        let b_x = self.builder.ins().icmp(IntCC::Equal, b, two);
        let any_x = self.builder.ins().bor(a_x, b_x);
        let cc = if negate { IntCC::NotEqual } else { IntCC::Equal };
        let flag = self.builder.ins().icmp(cc, a, b);
        let result = self.builder.ins().select(flag, one, zero);
        self.builder.ins().select(any_x, two, result)
    }

    // ── Small builder shorthands ──

    fn builder_f64(&mut self, v: f64) -> Value {
        self.builder.ins().f64const(v)
    }

    fn builder_i64(&mut self, v: i64) -> Value {
        self.builder.ins().iconst(types::I64, v)
    }

    fn builder_fneg(&mut self, v: Value) -> Value {
        self.builder.ins().fneg(v)
    }

    fn builder_ineg(&mut self, v: Value) -> Value {
        self.builder.ins().ineg(v)
    }

    fn builder_flag_i64(&mut self, flag: Value) -> Value {
        let one = self.builder_i64(1);
        let zero = self.builder_i64(0);
        self.builder.ins().select(flag, one, zero)
    }
}

// ─── Fused combinational network (Verilator-style whole-cone JIT) ───────────────

/// One member of a fused combinational network: its IR module plus how it binds
/// into the network-wide arrays. `in_net_slots[i]`/`out_net_slots[i]` are the
/// global net ids wired to kernel input/output `i`; the `*_base` fields are the
/// member's offsets into the shared int/real variable banks and the params bank.
pub struct NetworkMemberSpec<'m> {
    pub module: &'m crate::ir::IrModule,
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
    *const crate::jit::SimCtx,
    *const f64,
);

/// A compiled fused combinational network kernel.
pub struct NetworkComb {
    func: NetworkCombFn,
    _jit: JITModule,
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
        sim: *const crate::jit::SimCtx,
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
            if body.stmts.iter().any(|s| matches!(s, IrStmt::ClockedBlock { .. })) {
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
            let mut emitter = DigitalEmitter {
                builder: &mut builder,
                module: m.module,
                layout,
                pointers,
                reads: VarReads::Live,
                math: &math,
                watch_out: None,
            };
            let body = m.module.digital.as_ref().unwrap();
            for stmt in &body.stmts {
                // Skip register power-on decls (handled at init, not in comb).
                if let IrStmt::VarDecl { var, init: Some(_) } = stmt
                    && body.regs.contains(var)
                {
                    continue;
                }
                emitter.emit_stmt(stmt)?;
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
        Ok(NetworkComb { func, _jit: jit })
    }
}
