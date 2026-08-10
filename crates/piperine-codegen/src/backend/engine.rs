//! The shared IR-to-Cranelift compiling engine (T18) — `Engine`, the
//! `BlockEnv` ABI, `CellLayout`/`PortMap` pre-lowering inspection, and the
//! per-op emission context (`OpCtx`). Moved out of `backend/mod.rs` (MD-34:
//! a `mod.rs` declares its modules and states the layer's contract; it does
//! not hold implementation).

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Signature, Value};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module as ClifModule};

use piperine_ir::catalog::Catalog;
use piperine_ir::core::{CellId, Module, NetId, Op, Pattern, PureOp, Ty, ValueId};

/// What the backend could not lower. Fail loud (CLAUDE.md: never a silent
/// `0.0`/no-op) — every variant names the offending entity.
#[derive(Debug, Clone, PartialEq)]
pub enum BackendError {
    /// An op variant this backend does not (yet) compile.
    UnsupportedOp(&'static str),
    /// A `Call` to an intrinsic with no JIT-linkable implementation.
    UnknownIntrinsic(&'static str),
    /// A Cranelift/`cranelift_module` failure, wrapped with its message.
    Module(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::UnsupportedOp(kind) => write!(f, "backend: unsupported op `{kind}`"),
            BackendError::UnknownIntrinsic(name) => {
                write!(f, "backend: intrinsic `{name}` has no JIT-linkable implementation")
            }
            BackendError::Module(msg) => write!(f, "backend: module error: {msg}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// The runtime environment every compiled block function reads/writes
/// through — ten raw pointers, `#[repr(C)]` so the byte offsets below
/// (`ENV_FIELD_COUNT` fields, 8 bytes each) are stable across the JIT ABI.
/// One instance is built per evaluation step by the caller (mirrors
/// `Interpreter::set_node_values`/`set_signal`/`seed_value`/`cell_value`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlockEnv {
    /// Across value per net, indexed by `NetId` (Node nets only).
    pub net_across: *const f64,
    /// Through value per net, indexed by `NetId` (Node nets only).
    pub net_through: *const f64,
    /// Signal word per net, indexed by `NetId` (Signal nets only).
    pub net_signal: *const u64,
    /// Externally-seeded real values, indexed by `ValueId` (leaves with no
    /// producing op in the compiled block — a "param", a Timer input, …).
    pub seed_real: *const f64,
    /// Externally-seeded bits values, indexed by `ValueId`.
    pub seed_bits: *const u64,
    /// Real-typed cell storage, flattened per [`CellLayout::real_base`].
    pub cell_real: *mut f64,
    /// Bits-typed cell storage, flattened per [`CellLayout::bits_base`].
    pub cell_bits: *mut u64,
    /// Accumulated `contribute_through` output, one slot per unique
    /// `(a, b)` pair the block contributes to (see [`PortMap::contrib`]).
    /// The caller zero-initialises this before each evaluation — the
    /// compiled function only ever accumulates (`+=`), matching the
    /// interpreter's `HashMap`-accumulate semantics.
    pub contrib_out: *mut f64,
    /// `force_across` output, one slot per unique `(a, b)` pair (see
    /// [`PortMap::force`]). Overwritten (last write wins), matching the
    /// interpreter.
    pub force_out: *mut f64,
    /// `drive` output, one slot per unique net (see [`PortMap::drive`]).
    /// Overwritten, matching the interpreter.
    pub drive_out: *mut u64,
}

/// Field count of [`BlockEnv`] — every field is one 8-byte pointer, so the
/// byte offset of field `i` is `i * 8`.
const ENV_FIELDS: &[i32] = &[0, 8, 16, 24, 32, 40, 48, 56, 64, 72];

/// Field index into [`ENV_FIELDS`] — named so op-emission code reads like
/// "load `net_across`" rather than a bare integer.
#[derive(Debug, Clone, Copy)]
enum EnvField {
    NetAcross = 0,
    NetThrough = 1,
    NetSignal = 2,
    SeedReal = 3,
    SeedBits = 4,
    CellReal = 5,
    CellBits = 6,
    ContribOut = 7,
    ForceOut = 8,
    DriveOut = 9,
}

/// Where each declared [`piperine_ir::core::Cell`] lives in the flattened
/// `cell_real`/`cell_bits` arrays. Built once per module (T22 — "port count
/// inspectable pre-lowering": this IS that inspection, computed before any
/// Cranelift function is built).
pub struct CellLayout {
    real_base: HashMap<CellId, u32>,
    bits_base: HashMap<CellId, u32>,
    real_len: u32,
    bits_len: u32,
}

impl CellLayout {
    /// Compute the layout for every declared cell in `module`. A scalar
    /// cell occupies one slot; an array cell (a memory) occupies `dim`
    /// contiguous slots, addressed at `base + addr`.
    pub fn build(module: &Module) -> Self {
        let mut real_base = HashMap::new();
        let mut bits_base = HashMap::new();
        let mut real_len = 0u32;
        let mut bits_len = 0u32;
        for (id, cell) in module.cells() {
            let (elem_ty, count) = match &cell.ty {
                Ty::Array(elem, d) => (elem.as_ref(), *d),
                other => (other, 1u32),
            };
            match elem_ty {
                Ty::Real => {
                    real_base.insert(id, real_len);
                    real_len += count;
                }
                Ty::Bits(_) => {
                    bits_base.insert(id, bits_len);
                    bits_len += count;
                }
                Ty::Array(..) => {
                    // Nested arrays are out of this backend's scope
                    // (no fixture needs them); the cell simply owns no
                    // slots rather than silently aliasing another cell's.
                }
            }
        }
        Self { real_base, bits_base, real_len, bits_len }
    }

    /// Total real-cell slots — the caller allocates `cell_real` this long.
    pub fn real_len(&self) -> usize {
        self.real_len as usize
    }

    /// Total bits-cell slots — the caller allocates `cell_bits` this long.
    pub fn bits_len(&self) -> usize {
        self.bits_len as usize
    }

    /// The base slot of a real cell, or `None` if `cell` is not real-typed.
    pub fn real_base_of(&self, cell: CellId) -> Option<u32> {
        self.real_base.get(&cell).copied()
    }

    /// The base slot of a bits cell, or `None` if `cell` is not bits-typed.
    pub fn bits_base_of(&self, cell: CellId) -> Option<u32> {
        self.bits_base.get(&cell).copied()
    }
}

/// The unique `contribute_through`/`force_across`/`drive` sites a block
/// touches, in first-seen order — the slot layout for `contrib_out`/
/// `force_out`/`drive_out`. Also doubles as T22's "port count inspectable
/// pre-lowering": `contrib.len()`/`force.len()`/`drive.len()` are known
/// before any Cranelift function is built.
#[derive(Debug, Default, Clone)]
pub struct PortMap {
    pub contrib: Vec<(NetId, NetId)>,
    pub force: Vec<(NetId, NetId)>,
    pub drive: Vec<NetId>,
}

impl PortMap {
    /// Scan `ops` (recursing into `If`/`Match` arms) and record every
    /// unique net-write site. Explicit arm per [`Op`] variant — no
    /// catch-all (the A0c defence; registered as a dispatch site in
    /// `crates/piperine-ir/tests/policy_gates.rs`).
    pub fn scan(ops: &[Op]) -> Self {
        let mut pm = Self::default();
        pm.walk(ops);
        pm
    }

    fn walk(&mut self, ops: &[Op]) {
        for op in ops {
            match op {
                Op::Pure { .. }
                | Op::Call { .. }
                | Op::ReadAcross { .. }
                | Op::ReadThrough { .. }
                | Op::ReadSignal { .. }
                | Op::CellRead { .. } => {}
                Op::ContributeThrough { a, b, .. } => {
                    Self::slot_or_push(&mut self.contrib, (*a, *b));
                }
                Op::ForceAcross { a, b, .. } => {
                    Self::slot_or_push(&mut self.force, (*a, *b));
                }
                Op::Drive { net, .. } => {
                    Self::slot_or_push(&mut self.drive, *net);
                }
                Op::CellWrite { .. } => {}
                Op::If { then_ops, else_ops, .. } => {
                    self.walk(then_ops);
                    self.walk(else_ops);
                }
                Op::Match { arms, .. } => {
                    for (_, body) in arms {
                        self.walk(body);
                    }
                }
            }
        }
    }

    pub fn contrib_slot(&self, a: NetId, b: NetId) -> usize {
        self.contrib.iter().position(|&p| p == (a, b)).expect("contrib site must have been scanned")
    }

    pub fn force_slot(&self, a: NetId, b: NetId) -> usize {
        self.force.iter().position(|&p| p == (a, b)).expect("force site must have been scanned")
    }

    pub fn drive_slot(&self, net: NetId) -> usize {
        self.drive.iter().position(|&n| n == net).expect("drive site must have been scanned")
    }

    /// The index of `item` in `v`, first-seen order — appends `item` if
    /// this is its first occurrence. An associated function (MD-13 rule
    /// 2: no loose module-level functions) rather than a free helper.
    fn slot_or_push<T: PartialEq + Copy>(v: &mut Vec<T>, item: T) -> usize {
        if let Some(i) = v.iter().position(|&x| x == item) {
            i
        } else {
            v.push(item);
            v.len() - 1
        }
    }
}

/// The shared IR→Cranelift engine: owns one `JITModule`, the imported
/// libm symbols, and compiles one [`Op`] sequence (a block's body) at a
/// time into a native `extern "C" fn(*const BlockEnv)`.
pub struct Engine {
    jit: JITModule,
    math_ids: HashMap<&'static str, FuncId>,
    fb_ctx: FunctionBuilderContext,
}

impl Engine {
    pub fn new() -> Result<Self, BackendError> {
        let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| BackendError::Module(e.to_string()))?;
        for f in piperine_lang::math::MATH_FNS {
            jit_builder.symbol(f.name, f.symbol);
        }
        let mut jit = JITModule::new(jit_builder);

        let mut math_ids = HashMap::new();
        for f in piperine_lang::math::MATH_FNS {
            let mut sig = jit.make_signature();
            for _ in 0..f.arity {
                sig.params.push(AbiParam::new(types::F64));
            }
            sig.returns.push(AbiParam::new(types::F64));
            let id = jit
                .declare_function(f.name, Linkage::Import, &sig)
                .map_err(|e| BackendError::Module(e.to_string()))?;
            math_ids.insert(f.name, id);
        }

        Ok(Self { jit, math_ids, fb_ctx: FunctionBuilderContext::new() })
    }

    /// Compile `ops` (one block's body) into a native function taking one
    /// `*const BlockEnv` argument and returning nothing (all outputs are
    /// side effects through the env's pointers). Returns the raw function
    /// pointer plus the [`PortMap`] the caller uses to size `contrib_out`/
    /// `force_out`/`drive_out` and read them back by slot.
    pub fn compile_block(
        &mut self,
        module: &Module,
        ops: &[Op],
        layout: &CellLayout,
    ) -> Result<(CompiledFn, PortMap), BackendError> {
        self.compile_block_with_constants(module, ops, layout, &HashMap::new())
    }

    /// [`Self::compile_block`], additionally resolving `constants` —
    /// `piperine_ir::diff::Differentiator::constants()`'s side-channel for
    /// its `zero_id`/`one_id`/`real_const(c)` anchors. Each anchor's
    /// self-referential defining op (`Pure::Add { args: [out, out], out }`
    /// — see `Differentiator`'s field doc) is deliberately unevaluable by
    /// walking the op graph alone, so this backend is handed the actual
    /// value alongside the graph rather than re-deriving it (T19: no
    /// second differentiation implementation, and no guessing either).
    pub fn compile_block_with_constants(
        &mut self,
        module: &Module,
        ops: &[Op],
        layout: &CellLayout,
        constants: &HashMap<ValueId, f64>,
    ) -> Result<(CompiledFn, PortMap), BackendError> {
        let ports = PortMap::scan(ops);

        let mut sig = Signature::new(cranelift_codegen::isa::CallConv::SystemV);
        sig.params.push(AbiParam::new(types::I64));
        let func_id = self
            .jit
            .declare_anonymous_function(&sig)
            .map_err(|e| BackendError::Module(e.to_string()))?;

        let mut ctx = self.jit.make_context();
        ctx.func.signature = sig;

        // Import every libm symbol into THIS function up front (before the
        // `FunctionBuilder` borrows `ctx.func`) — `declare_func_in_func`
        // needs the `JITModule`, which `OpCtx` does not hold.
        let mut math_refs = HashMap::new();
        for (&name, &id) in &self.math_ids {
            let fref = self.jit.declare_func_in_func(id, &mut ctx.func);
            math_refs.insert(name, fref);
        }

        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut self.fb_ctx);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);
            let env_ptr = builder.block_params(entry)[0];

            let mut fields = [Value::from_u32(0); ENV_FIELDS.len()];
            for (i, off) in ENV_FIELDS.iter().enumerate() {
                fields[i] = builder.ins().load(types::I64, MemFlags::trusted(), env_ptr, *off);
            }

            let mut op_ctx = OpCtx {
                builder,
                module,
                layout,
                ports: &ports,
                math_refs: &math_refs,
                constants,
                vars: HashMap::new(),
                next_var: 0,
                env: fields,
            };
            op_ctx.declare_seeds(ops)?;
            for op in ops {
                op_ctx.compile_op(op)?;
            }
            op_ctx.builder.ins().return_(&[]);
            op_ctx.builder.finalize();
        }

        self.jit
            .define_function(func_id, &mut ctx)
            .map_err(|e| BackendError::Module(e.to_string()))?;
        self.jit.clear_context(&mut ctx);
        self.jit
            .finalize_definitions()
            .map_err(|e| BackendError::Module(e.to_string()))?;
        let ptr = self.jit.get_finalized_function(func_id);
        Ok((CompiledFn(ptr as *const u8), ports))
    }

    /// Compile a single scalar `ValueId` (e.g. a Jacobian entry produced by
    /// `piperine_ir::diff::Differentiator::d`) as its own function that
    /// writes the value to a one-slot `f64` output via the env's
    /// `contrib_out[0]` convention — reused rather than adding a new ABI
    /// shape (T19: "no second differentiation implementation exists", and
    /// likewise no second emission ABI).
    pub fn compile_scalar(
        &mut self,
        module: &Module,
        ops: &[Op],
        output: ValueId,
        layout: &CellLayout,
        constants: &HashMap<ValueId, f64>,
    ) -> Result<(CompiledFn, PortMap, usize), BackendError> {
        // Append a synthetic write of `output` into its own contrib slot
        // by wrapping it in a trivial ContributeThrough against a private
        // net pair the caller never observes otherwise — reuses the exact
        // same accumulate-into-contrib_out path as a residual row, rather
        // than inventing a second output ABI.
        let mut synthetic = ops.to_vec();
        synthetic.push(Op::ContributeThrough {
            a: SCALAR_OUTPUT_NET_A,
            b: SCALAR_OUTPUT_NET_B,
            value: output,
        });
        let (f, ports) = self.compile_block_with_constants(module, &synthetic, layout, constants)?;
        let slot = ports.contrib_slot(SCALAR_OUTPUT_NET_A, SCALAR_OUTPUT_NET_B);
        Ok((f, ports, slot))
    }
}

/// Reserved net-pair used only by [`Engine::compile_scalar`] to smuggle a
/// single scalar value out through the existing `contrib_out[0]` slot.
/// These ids are never looked up in `module.net_at` by the compiler (the
/// synthetic op's `a`/`b` fields are read only to compute the `PortMap`
/// slot, never dereferenced against the net arena), so any two distinct
/// `NetId`s work.
const SCALAR_OUTPUT_NET_A: NetId = NetId(u32::MAX);
const SCALAR_OUTPUT_NET_B: NetId = NetId(u32::MAX - 1);

/// A compiled block's raw entry point. `unsafe` to call: the caller must
/// supply a well-formed `BlockEnv` whose arrays are at least as long as the
/// [`PortMap`]/[`CellLayout`] this function was compiled against.
#[derive(Clone, Copy)]
pub struct CompiledFn(*const u8);

impl CompiledFn {
    /// Invoke the compiled function against `env`.
    ///
    /// # Safety
    /// `env`'s pointers must each reference storage at least as large as
    /// this function's [`PortMap`]/[`CellLayout`] requires — the same
    /// contract as `piperine_ir::interp::Interpreter::set_node_values`
    /// keeping the interpreter's `nets` map populated before `step()`.
    pub unsafe fn call(&self, env: *const BlockEnv) {
        let f: extern "C" fn(*const BlockEnv) = unsafe { std::mem::transmute(self.0) };
        f(env);
    }
}

// SAFETY: the pointer is into JIT-compiled, read-only executable memory
// owned by the `Engine`'s `JITModule`; sharing it across threads for
// read-only invocation is sound as long as the `Engine` (and its
// `OwnedJit`, when wrapped) outlives every call.
unsafe impl Send for CompiledFn {}
unsafe impl Sync for CompiledFn {}

/// Per-compile-call emission context: one [`FunctionBuilder`], the module
/// being compiled, the loaded env-field pointers, and the Cranelift
/// [`Variable`] assigned to each [`ValueId`] this block touches.
struct OpCtx<'a, 'b, 'f> {
    builder: FunctionBuilder<'f>,
    module: &'a Module,
    layout: &'a CellLayout,
    ports: &'b PortMap,
    math_refs: &'b HashMap<&'static str, cranelift_codegen::ir::FuncRef>,
    /// `piperine_ir::diff::Differentiator::constants()`'s side-channel —
    /// see `compile_block_with_constants`'s doc.
    constants: &'b HashMap<ValueId, f64>,
    vars: HashMap<ValueId, Variable>,
    next_var: u32,
    env: [Value; 10],
}

impl<'a, 'b, 'f> OpCtx<'a, 'b, 'f> {
    fn env_field(&self, f: EnvField) -> Value {
        self.env[f as usize]
    }

    /// Declare (and define) a `Variable` for every `ValueId` this op
    /// sequence reads but never writes — the "seed" leaves (params, Timer
    /// inputs, …), mirroring `Interpreter::seed_value`. Recurses into
    /// `If`/`Match` arms.
    fn declare_seeds(&mut self, ops: &[Op]) -> Result<(), BackendError> {
        let mut written = std::collections::HashSet::new();
        let mut read = std::collections::HashSet::new();
        Self::collect_reads_writes(ops, &mut read, &mut written);
        for v in read {
            if written.contains(&v) {
                continue;
            }
            let var = self.variable_for(v);
            let ty = self.module.value_ty(v).clone();
            let value = match ty {
                Ty::Real => {
                    let ptr = self.env_field(EnvField::SeedReal);
                    self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, (v.0 * 8) as i32)
                }
                Ty::Bits(_) => {
                    let ptr = self.env_field(EnvField::SeedBits);
                    self.builder.ins().load(types::I64, MemFlags::trusted(), ptr, (v.0 * 8) as i32)
                }
                Ty::Array(..) => return Err(BackendError::UnsupportedOp("array-typed seed value")),
            };
            self.builder.def_var(var, value);
        }
        Ok(())
    }

    fn variable_for(&mut self, v: ValueId) -> Variable {
        if let Some(&var) = self.vars.get(&v) {
            return var;
        }
        let var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        let cty = match self.module.value_ty(v) {
            Ty::Real => types::F64,
            Ty::Bits(_) => types::I64,
            Ty::Array(..) => types::I64,
        };
        self.builder.declare_var(var, cty);
        self.vars.insert(v, var);
        var
    }

    fn use_value(&mut self, v: ValueId) -> Value {
        let var = self.variable_for(v);
        self.builder.use_var(var)
    }

    fn def_value(&mut self, v: ValueId, value: Value) {
        let var = self.variable_for(v);
        self.builder.def_var(var, value);
    }

    /// Compile one op. Explicit arm per [`Op`] variant — no catch-all (the
    /// A0c defence; registered as a dispatch site).
    fn compile_op(&mut self, op: &Op) -> Result<(), BackendError> {
        match op {
            Op::Pure { op: pop, args, out } => {
                // A `Differentiator` constant anchor (`zero_id`/`one_id`/
                // `real_const(c)`): its defining op is self-referential
                // (`out` reads itself), unevaluable by walking args — the
                // real value comes from the side-channel instead. Emitting
                // it as a literal ALSO short-circuits the would-be
                // read-before-def on `out`'s own Variable.
                if let Some(&c) = self.constants.get(out) {
                    let lit = self.builder.ins().f64const(c);
                    self.def_value(*out, lit);
                    return Ok(());
                }
                let is_real = matches!(self.module.value_ty(*out), Ty::Real);
                let argv: Vec<Value> = args.iter().map(|a| self.use_value(*a)).collect();
                let result = self.compile_pure(*pop, &argv, is_real, *out)?;
                self.def_value(*out, result);
            }
            Op::Call { intrinsic, args, out } => {
                let entry = Catalog::entries().nth(intrinsic.0 as usize);
                let name = entry.map(|e| e.name()).unwrap_or("");
                let Some(&func_ref) = self.math_refs.get(name) else {
                    return Err(BackendError::UnknownIntrinsic(Self::leak_name(name)));
                };
                let argv: Vec<Value> = args.iter().map(|a| self.use_value(*a)).collect();
                let call = self.builder.ins().call(func_ref, &argv);
                let result = self.builder.inst_results(call)[0];
                self.def_value(*out, result);
            }
            Op::ReadAcross { a, b, out } => {
                let ptr = self.env_field(EnvField::NetAcross);
                let va = self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, (a.0 * 8) as i32);
                let vb = self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, (b.0 * 8) as i32);
                let result = self.builder.ins().fsub(va, vb);
                self.def_value(*out, result);
            }
            Op::ReadThrough { a, b, out } => {
                let ptr = self.env_field(EnvField::NetThrough);
                let ia = self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, (a.0 * 8) as i32);
                let ib = self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, (b.0 * 8) as i32);
                let result = self.builder.ins().fsub(ia, ib);
                self.def_value(*out, result);
            }
            Op::ContributeThrough { a, b, value } => {
                let slot = self.ports.contrib_slot(*a, *b);
                let ptr = self.env_field(EnvField::ContribOut);
                let off = (slot * 8) as i32;
                let v = self.use_value(*value);
                let cur = self.builder.ins().load(types::F64, MemFlags::trusted(), ptr, off);
                let sum = self.builder.ins().fadd(cur, v);
                self.builder.ins().store(MemFlags::trusted(), sum, ptr, off);
            }
            Op::ForceAcross { a, b, value } => {
                let slot = self.ports.force_slot(*a, *b);
                let ptr = self.env_field(EnvField::ForceOut);
                let v = self.use_value(*value);
                self.builder.ins().store(MemFlags::trusted(), v, ptr, (slot * 8) as i32);
            }
            Op::ReadSignal { net, out } => {
                let ptr = self.env_field(EnvField::NetSignal);
                let v = self.builder.ins().load(types::I64, MemFlags::trusted(), ptr, (net.0 * 8) as i32);
                self.def_value(*out, v);
            }
            Op::Drive { net, value } => {
                let slot = self.ports.drive_slot(*net);
                let ptr = self.env_field(EnvField::DriveOut);
                let v = self.use_value(*value);
                self.builder.ins().store(MemFlags::trusted(), v, ptr, (slot * 8) as i32);
            }
            Op::CellRead { cell, addr, out } => {
                let (ptr_field, base, cty) = self.cell_slot(*cell)?;
                let ptr = self.env_field(ptr_field);
                let value = match addr {
                    None => self.builder.ins().load(cty, MemFlags::trusted(), ptr, (base * 8) as i32),
                    Some(a) => {
                        let addr_v = self.use_value(*a);
                        let byte_off = self.builder.ins().imul_imm(addr_v, 8);
                        let base_bytes = self.builder.ins().iconst(types::I64, (base * 8) as i64);
                        let off = self.builder.ins().iadd(base_bytes, byte_off);
                        let addr_ptr = self.builder.ins().iadd(ptr, off);
                        self.builder.ins().load(cty, MemFlags::trusted(), addr_ptr, 0)
                    }
                };
                self.def_value(*out, value);
            }
            Op::CellWrite { cell, addr, value } => {
                let (ptr_field, base, _cty) = self.cell_slot(*cell)?;
                let ptr = self.env_field(ptr_field);
                let v = self.use_value(*value);
                match addr {
                    None => {
                        self.builder.ins().store(MemFlags::trusted(), v, ptr, (base * 8) as i32);
                    }
                    Some(a) => {
                        let addr_v = self.use_value(*a);
                        let byte_off = self.builder.ins().imul_imm(addr_v, 8);
                        let base_bytes = self.builder.ins().iconst(types::I64, (base * 8) as i64);
                        let off = self.builder.ins().iadd(base_bytes, byte_off);
                        let addr_ptr = self.builder.ins().iadd(ptr, off);
                        self.builder.ins().store(MemFlags::trusted(), v, addr_ptr, 0);
                    }
                }
            }
            Op::If { cond, then_ops, else_ops } => {
                self.compile_if(*cond, then_ops, else_ops)?;
            }
            Op::Match { scrutinee, arms } => {
                self.compile_match(*scrutinee, arms)?;
            }
        }
        Ok(())
    }

    /// The cell's storage field, base slot, and Cranelift storage type.
    fn cell_slot(&self, cell: CellId) -> Result<(EnvField, u32, cranelift_codegen::ir::Type), BackendError> {
        if let Some(base) = self.layout.real_base_of(cell) {
            return Ok((EnvField::CellReal, base, types::F64));
        }
        if let Some(base) = self.layout.bits_base_of(cell) {
            return Ok((EnvField::CellBits, base, types::I64));
        }
        Err(BackendError::UnsupportedOp("cell with unlayoutable (nested-array) type"))
    }

    fn compile_pure(&mut self, op: PureOp, args: &[Value], is_real: bool, out: ValueId) -> Result<Value, BackendError> {
        let ins = self.builder.ins();
        Ok(match (op, is_real) {
            (PureOp::Add, true) => ins.fadd(args[0], args[1]),
            (PureOp::Sub, true) => ins.fsub(args[0], args[1]),
            (PureOp::Mul, true) => ins.fmul(args[0], args[1]),
            (PureOp::Div, true) => ins.fdiv(args[0], args[1]),
            (PureOp::Neg, true) => ins.fneg(args[0]),
            (PureOp::Add, false) => {
                let r = ins.iadd(args[0], args[1]);
                self.mask_bits(r, out)
            }
            (PureOp::Sub, false) => {
                let r = ins.isub(args[0], args[1]);
                self.mask_bits(r, out)
            }
            (PureOp::Mul, false) => {
                let r = ins.imul(args[0], args[1]);
                self.mask_bits(r, out)
            }
            (PureOp::Div, false) => {
                let r = ins.udiv(args[0], args[1]);
                self.mask_bits(r, out)
            }
            (PureOp::Neg, false) => {
                let r = ins.ineg(args[0]);
                self.mask_bits(r, out)
            }
            (PureOp::Eq, true) => self.bool_to_bits(FloatCmp::Eq, args, true),
            (PureOp::Ne, true) => self.bool_to_bits(FloatCmp::Ne, args, true),
            (PureOp::Lt, true) => self.bool_to_bits(FloatCmp::Lt, args, true),
            (PureOp::Le, true) => self.bool_to_bits(FloatCmp::Le, args, true),
            (PureOp::Gt, true) => self.bool_to_bits(FloatCmp::Gt, args, true),
            (PureOp::Ge, true) => self.bool_to_bits(FloatCmp::Ge, args, true),
            (PureOp::Eq, false) => self.bool_to_bits(FloatCmp::Eq, args, false),
            (PureOp::Ne, false) => self.bool_to_bits(FloatCmp::Ne, args, false),
            (PureOp::Lt, false) => self.bool_to_bits(FloatCmp::Lt, args, false),
            (PureOp::Le, false) => self.bool_to_bits(FloatCmp::Le, args, false),
            (PureOp::Gt, false) => self.bool_to_bits(FloatCmp::Gt, args, false),
            (PureOp::Ge, false) => self.bool_to_bits(FloatCmp::Ge, args, false),
            (PureOp::Select, _) => {
                let cond = self.builder.ins().icmp_imm(IntCC::NotEqual, args[0], 0);
                self.builder.ins().select(cond, args[1], args[2])
            }
        })
    }

    fn mask_bits(&mut self, v: Value, out: ValueId) -> Value {
        let width = self.module.value_ty(out).width();
        if width >= 64 {
            return v;
        }
        let mask = (1i64 << width) - 1;
        self.builder.ins().band_imm(v, mask)
    }

    fn bool_to_bits(&mut self, cmp: FloatCmp, args: &[Value], real: bool) -> Value {
        let flag = if real {
            let cc = cmp.float_cc();
            self.builder.ins().fcmp(cc, args[0], args[1])
        } else {
            let cc = cmp.int_cc();
            self.builder.ins().icmp(cc, args[0], args[1])
        };
        let one = self.builder.ins().iconst(types::I64, 1);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().select(flag, one, zero)
    }

    fn compile_if(&mut self, cond: ValueId, then_ops: &[Op], else_ops: &[Op]) -> Result<(), BackendError> {
        let c = self.use_value(cond);
        let truthy = self.builder.ins().icmp_imm(IntCC::NotEqual, c, 0);
        let then_block = self.builder.create_block();
        let else_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.ins().brif(truthy, then_block, &[], else_block, &[]);

        self.builder.switch_to_block(then_block);
        self.builder.seal_block(then_block);
        for op in then_ops {
            self.compile_op(op)?;
        }
        self.builder.ins().jump(merge_block, &[]);

        self.builder.switch_to_block(else_block);
        self.builder.seal_block(else_block);
        for op in else_ops {
            self.compile_op(op)?;
        }
        self.builder.ins().jump(merge_block, &[]);

        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);
        Ok(())
    }

    fn compile_match(&mut self, scrutinee: ValueId, arms: &[(Pattern, Vec<Op>)]) -> Result<(), BackendError> {
        let s = self.use_value(scrutinee);
        let bit = self.builder.ins().band_imm(s, 1);
        let merge_block = self.builder.create_block();

        // One "test" block per arm, in order; the last arm's test block
        // falls through unconditionally (it MUST match — the interpreter's
        // `eval_op` has the same "first matching arm wins, and callers are
        // expected to cover the space" contract via Wildcard).
        let mut cur_test = self.builder.current_block().expect("in a block");
        for (i, (pat, body)) in arms.iter().enumerate() {
            let is_last = i + 1 == arms.len();
            let arm_block = self.builder.create_block();
            let next_test = if is_last { None } else { Some(self.builder.create_block()) };

            self.builder.switch_to_block(cur_test);
            match pat {
                Pattern::Wildcard => {
                    self.builder.ins().jump(arm_block, &[]);
                }
                Pattern::Zero | Pattern::One => {
                    let want = if matches!(pat, Pattern::One) { 1 } else { 0 };
                    let matches = self.builder.ins().icmp_imm(IntCC::Equal, bit, want);
                    match next_test {
                        Some(nt) => {
                            self.builder.ins().brif(matches, arm_block, &[], nt, &[]);
                        }
                        None => {
                            self.builder.ins().jump(arm_block, &[]);
                        }
                    }
                }
            }
            self.builder.seal_block(cur_test);

            self.builder.switch_to_block(arm_block);
            self.builder.seal_block(arm_block);
            for op in body {
                self.compile_op(op)?;
            }
            self.builder.ins().jump(merge_block, &[]);

            if let Some(nt) = next_test {
                cur_test = nt;
            }
        }

        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);
        Ok(())
    }

    /// Collect the `ValueId`s an op sequence reads and writes, recursing
    /// into `If`/`Match` arms. Explicit arm per [`Op`] variant — no
    /// catch-all. An associated function (MD-13 rule 2: no loose
    /// module-level functions) rather than a free helper.
    fn collect_reads_writes(
        ops: &[Op],
        read: &mut std::collections::HashSet<ValueId>,
        written: &mut std::collections::HashSet<ValueId>,
    ) {
        for op in ops {
            for r in op.reads() {
                read.insert(r);
            }
            if let Some(w) = op.writes() {
                written.insert(w);
            }
            match op {
                Op::Pure { .. }
                | Op::Call { .. }
                | Op::ReadAcross { .. }
                | Op::ReadThrough { .. }
                | Op::ContributeThrough { .. }
                | Op::ForceAcross { .. }
                | Op::ReadSignal { .. }
                | Op::Drive { .. }
                | Op::CellRead { .. }
                | Op::CellWrite { .. } => {}
                Op::If { cond, then_ops, else_ops } => {
                    read.insert(*cond);
                    Self::collect_reads_writes(then_ops, read, written);
                    Self::collect_reads_writes(else_ops, read, written);
                }
                Op::Match { scrutinee, arms } => {
                    read.insert(*scrutinee);
                    for (_, body) in arms {
                        Self::collect_reads_writes(body, read, written);
                    }
                }
            }
        }
    }

    /// Leak a `&str` into `'static` — used only for the small, bounded set
    /// of catalog/intrinsic names (at most a few dozen, each leaked at
    /// most once per compiled block); trades a few bytes of permanent
    /// memory for a `&'static str` key `BackendError::UnknownIntrinsic`
    /// needs. An associated function (MD-13 rule 2) rather than a free
    /// helper.
    fn leak_name(s: &str) -> &'static str {
        Box::leak(s.to_string().into_boxed_str())
    }
}

/// Which float/int comparison a [`PureOp`] comparison variant performs —
/// named so `bool_to_bits` reads as a lookup, not six inlined cases.
#[derive(Debug, Clone, Copy)]
enum FloatCmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl FloatCmp {
    fn float_cc(self) -> cranelift_codegen::ir::condcodes::FloatCC {
        use cranelift_codegen::ir::condcodes::FloatCC;
        match self {
            FloatCmp::Eq => FloatCC::Equal,
            FloatCmp::Ne => FloatCC::NotEqual,
            FloatCmp::Lt => FloatCC::LessThan,
            FloatCmp::Le => FloatCC::LessThanOrEqual,
            FloatCmp::Gt => FloatCC::GreaterThan,
            FloatCmp::Ge => FloatCC::GreaterThanOrEqual,
        }
    }
    fn int_cc(self) -> IntCC {
        match self {
            FloatCmp::Eq => IntCC::Equal,
            FloatCmp::Ne => IntCC::NotEqual,
            FloatCmp::Lt => IntCC::UnsignedLessThan,
            FloatCmp::Le => IntCC::UnsignedLessThanOrEqual,
            FloatCmp::Gt => IntCC::UnsignedGreaterThan,
            FloatCmp::Ge => IntCC::UnsignedGreaterThanOrEqual,
        }
    }
}

