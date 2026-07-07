//! The [`Builder`]: wraps Cranelift's `FunctionBuilder` and provides
//! high-level emission methods (arithmetic, quad logic, name resolution,
//! control flow) that the [`Codegen`](super::trait_::Codegen) trait impls call.

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, FuncRef, InstBuilder, MemFlags, Value};
use cranelift_frontend::FunctionBuilder;

use piperine_lang::parse::ast::{BindOp, BinaryOp, Expr, Literal, Pattern, Stmt, UnaryOp};

use crate::ir::{BinOp, FnId, LoweredBody, NodeId, ParamId, SymbolTable, Type, UnOp, VarId};
use crate::jit::digital::compile::{Pointers, VarReads};
use crate::jit::digital::layout::DigitalLayout;
use crate::jit::{math, CodegenError, SimCtx};

use super::trait_::Codegen;

// ─── Analog CSE infrastructure (copied from jit/emit.rs) ──────────────────────

/// Structural key for common-subexpression elimination in analog emission.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum CseKey {
    /// f64/const bit pattern.
    Const(u64),
    /// A load: `(bank tag, byte offset)`.
    Load(u8, i32),
    /// Unary op `(tag, child)`.
    Op1(u8, u32),
    /// Binary op / comparison `(tag, lhs, rhs)`.
    Op2(u8, u32, u32),
    /// Ternary (select) `(tag, a, b, c)`.
    Op3(u8, u32, u32, u32),
    /// Math builtin call `(name, args)`.
    Call(&'static str, Vec<u32>),
    /// Voltage-limited value for `$limit` slot `i`.
    Limit(u32),
}

/// Byte offsets of [`SimCtx`] fields, as read by JIT code.
struct SimField;

impl SimField {
    const TEMPERATURE: i32 = 0;
    const ABSTIME: i32 = 8;
    const MFACTOR: i32 = 16;
    const GMIN: i32 = 24;
    const STEP: i32 = 32;
    const TFINAL: i32 = 40;
    const PARAM_GIVEN_MASK: i32 = 48;
    const CURRENT_ANALYSIS: i32 = 56;
}

// Load-bank tags.
const BANK_STATE: u8 = 0;
const BANK_VARS: u8 = 1;
const BANK_SIM: u8 = 2;
// Op tags (namespaced across unary/binary/select/cmp).
const T_NEG: u8 = 0;
const T_SELECT: u8 = 1;
const T_NOT: u8 = 2;
const T_FCMP_BASE: u8 = 16;
const T_BIN_BASE: u8 = 40;

/// Distinct CSE tag per binary op (offset past the fcmp/select tags).
fn bin_tag(op: crate::ir::BinOp) -> u8 {
    T_BIN_BASE
        + match op {
            crate::ir::BinOp::Add => 0,
            crate::ir::BinOp::Sub => 1,
            crate::ir::BinOp::Mul => 2,
            crate::ir::BinOp::Div => 3,
            crate::ir::BinOp::Rem => 4,
            crate::ir::BinOp::Eq => 5,
            crate::ir::BinOp::Ne => 6,
            crate::ir::BinOp::Lt => 7,
            crate::ir::BinOp::Le => 8,
            crate::ir::BinOp::Gt => 9,
            crate::ir::BinOp::Ge => 10,
            crate::ir::BinOp::And => 11,
            crate::ir::BinOp::Or => 12,
            crate::ir::BinOp::Pow => 13,
            crate::ir::BinOp::BitAnd => 14,
            crate::ir::BinOp::BitOr => 15,
            crate::ir::BinOp::BitXor => 16,
            crate::ir::BinOp::Shl => 17,
            crate::ir::BinOp::Shr => 18,
        }
}

// ─── Name resolution ──────────────────────────────────────────────────────────

/// Name → resolved id maps, built once from the `SymbolTable`.
pub struct Resolver {
    pub vars: HashMap<String, VarId>,
    pub nodes: HashMap<String, NodeId>,
    pub params: HashMap<String, ParamId>,
    pub fns: HashMap<String, FnId>,
}

impl Resolver {
    pub fn from_symbols(symbols: &SymbolTable) -> Self {
        Self {
            vars: symbols.vars().map(|(id, v)| (v.name.clone(), id)).collect(),
            nodes: symbols.nodes().map(|(id, n)| (n.name.clone(), id)).collect(),
            params: symbols.params().map(|(id, p)| (p.name.clone(), id)).collect(),
            fns: symbols.fns().map(|(id, f)| (f.name.clone(), id)).collect(),
        }
    }
}

// ─── Typed values ─────────────────────────────────────────────────────────────

/// A value plus its digital type.
#[derive(Clone, Copy)]
pub struct Typed {
    pub value: Value,
    pub ty: DigTy,
}

impl Typed {
    pub fn real(value: Value) -> Self {
        Self { value, ty: DigTy::Real }
    }

    pub fn int(value: Value) -> Self {
        Self { value, ty: DigTy::Int }
    }

    pub fn quad(value: Value) -> Self {
        Self { value, ty: DigTy::Quad }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DigTy {
    /// Two-state integer/boolean (`i64`).
    Int,
    /// `f64`.
    Real,
    /// Four-state logic in `i64`: 0, 1, 2 = X, 3 = Z.
    Quad,
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// The codegen builder: wraps Cranelift + provides high-level emission methods.
/// One per compiled function. Dual-context: `new_digital` for digital bodies
/// (quad logic, ABI pointers), `new_analog` for analog bodies (f64 scalar
/// emission with CSE, bank pointers, `$limit`).
pub struct Builder<'a, 'f, 'm> {
    pub builder: &'a mut FunctionBuilder<'f>,
    pub module: &'m LoweredBody,
    pub resolver: &'a Resolver,
    pub math: &'a HashMap<&'static str, FuncRef>,
    pub watch_out: Option<Value>,
    // ── Digital context (Some for digital, None for analog) ──
    pub layout: Option<&'a DigitalLayout>,
    pub pointers: Option<Pointers>,
    pub reads: Option<VarReads>,
    // ── Analog context (Some for analog, None for digital) ──
    /// Precomputed `V(plus) − V(minus)` per branch pair.
    pub branch_voltages: Option<HashMap<(NodeId, NodeId), Value>>,
    /// Parameter values, indexed by `ParamId`.
    pub params: Option<Vec<Value>>,
    /// `*const f64` runtime-state bank pointer.
    pub state_ptr: Option<Value>,
    /// `*const f64` module-level persistent variable bank pointer.
    pub vars_ptr: Option<Value>,
    /// `*const SimCtx`.
    pub sim_ptr: Option<Value>,
    /// Unique `$limit` expressions (POM `Expr`), in slot order.
    pub limits: Option<Vec<Expr>>,
    /// State-bank offset where `$limit` vold slots begin.
    pub limit_base: usize,
    /// Common-subexpression cache for analog emission.
    pub cse: Option<HashMap<CseKey, Value>>,
}

impl<'a, 'f, 'm> Builder<'a, 'f, 'm> {
    /// Construct a digital-context builder (quad logic, ABI pointers).
    pub fn new_digital(
        builder: &'a mut FunctionBuilder<'f>,
        module: &'m LoweredBody,
        resolver: &'a Resolver,
        layout: &'a DigitalLayout,
        pointers: Pointers,
        reads: VarReads,
        math: &'a HashMap<&'static str, FuncRef>,
        watch_out: Option<Value>,
    ) -> Self {
        Self {
            builder,
            module,
            resolver,
            math,
            watch_out,
            layout: Some(layout),
            pointers: Some(pointers),
            reads: Some(reads),
            branch_voltages: None,
            params: None,
            state_ptr: None,
            vars_ptr: None,
            sim_ptr: None,
            limits: None,
            limit_base: 0,
            cse: None,
        }
    }

    /// Construct an analog-context builder (f64 scalar emission, CSE, bank
    /// pointers). `branch_voltages` and `params` are preloaded once; `limits`
    /// are the unique `$limit` expressions in slot order.
    #[allow(clippy::too_many_arguments)]
    pub fn new_analog(
        builder: &'a mut FunctionBuilder<'f>,
        module: &'m LoweredBody,
        resolver: &'a Resolver,
        math: &'a HashMap<&'static str, FuncRef>,
        branch_voltages: HashMap<(NodeId, NodeId), Value>,
        params: Vec<Value>,
        state_ptr: Value,
        vars_ptr: Value,
        sim_ptr: Value,
        limits: Vec<Expr>,
        limit_base: usize,
    ) -> Self {
        Self {
            builder,
            module,
            resolver,
            math,
            watch_out: None,
            layout: None,
            pointers: None,
            reads: None,
            branch_voltages: Some(branch_voltages),
            params: Some(params),
            state_ptr: Some(state_ptr),
            vars_ptr: Some(vars_ptr),
            sim_ptr: Some(sim_ptr),
            limits: Some(limits),
            limit_base,
            cse: Some(HashMap::new()),
        }
    }

    // ── Digital-context accessors ──

    #[allow(dead_code)]
    fn layout(&self) -> &DigitalLayout {
        self.layout.expect("digital context")
    }
    #[allow(dead_code)]
    fn ptrs(&self) -> Pointers {
        self.pointers.expect("digital context")
    }
    #[allow(dead_code)]
    fn reads(&self) -> VarReads {
        self.reads.expect("digital context")
    }

    // ── Analog-context accessors ──

    fn state_ptr(&self) -> Value {
        self.state_ptr.expect("analog context")
    }
    fn vars_ptr(&self) -> Value {
        self.vars_ptr.expect("analog context")
    }
    fn sim_ptr(&self) -> Value {
        self.sim_ptr.expect("analog context")
    }

    // ── Name resolution & POM dispatch ──

    /// Resolve a name and load the corresponding value.
    /// Dispatches: var? param? net? enum value?
    pub fn load_ident(&mut self, name: &str) -> Result<Typed, CodegenError> {
        const GROUND_NAMES: &[&str] = &["gnd", "GND", "vss", "VSS", "0"];
        if GROUND_NAMES.contains(&name) {
            return Ok(Typed::real(self.builder_f64(0.0)));
        }
        if let Some(&id) = self.resolver.vars.get(name) {
            return Ok(self.load_var(id));
        }
        if let Some(&id) = self.resolver.params.get(name) {
            return self.load_param(id);
        }
        if let Some(&id) = self.resolver.nodes.get(name) {
            return self.load_net(id);
        }
        Err(CodegenError::Invalid(format!("unresolved identifier `{name}`")))
    }

    /// Load a parameter by id.
    pub fn load_param(&mut self, id: ParamId) -> Result<Typed, CodegenError> {
        let params = self.pointers.expect("digital context").params;
        let value = self.builder.ins().load(
            types::F64,
            MemFlags::trusted(),
            params,
            (id.0 * 8) as i32,
        );
        let info = self.module.symbols.param(id);
        match info.ty {
            Type::Real => Ok(Typed::real(value)),
            _ => {
                let as_int = self.builder.ins().fcvt_to_sint(types::I64, value);
                Ok(Typed::int(as_int))
            }
        }
    }

    /// Resolve a branch destination `V(p,n)` or `I(p,n)` to (plus_node, minus_node).
    pub fn resolve_branch(&mut self, dest: &Expr) -> Result<(NodeId, NodeId), CodegenError> {
        if let Expr::Call(func, args) = dest
            && let Expr::Ident(_) = func.as_ref()
        {
            let plus_name = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
            let minus_name = ident_from_expr(args.get(1)).unwrap_or_else(|| "0".into());
            let plus = self.resolve_node(&plus_name)?;
            let minus = self.resolve_node(&minus_name)?;
            return Ok((plus, minus));
        }
        Err(CodegenError::Invalid("expected V(p,n) or I(p,n) branch access".into()))
    }

    fn resolve_node(&self, name: &str) -> Result<NodeId, CodegenError> {
        const GROUND_NAMES: &[&str] = &["gnd", "GND", "vss", "VSS", "0"];
        if GROUND_NAMES.contains(&name) {
            return Ok(NodeId::GROUND);
        }
        self.resolver.nodes.get(name).copied().ok_or_else(|| {
            CodegenError::Invalid(format!("unresolved node `{name}`"))
        })
    }

    /// Dispatch a call expression: math function? analog operator? user function? branch access?
    pub fn call_expr(&mut self, func: &Expr, args: &[Expr]) -> Result<Typed, CodegenError> {
        let Expr::Ident(name) = func else {
            return Err(CodegenError::unsupported("non-identifier call target"));
        };
        // Branch access: V(p,n) or I(p,n)
        match name.as_str() {
            "V" | "I" => {
                let plus = ident_from_expr(args.first()).unwrap_or_default();
                let minus = ident_from_expr(args.get(1)).unwrap_or_else(|| "0".into());
                let plus_id = self.resolve_node(&plus)?;
                let minus_id = self.resolve_node(&minus)?;
                return self.load_branch(plus_id, minus_id);
            }
            _ => {}
        }
        // Math function
        if math::math_fn(name).is_some() {
            return self.emit_math(name, args);
        }
        // User function — inline by looking up FnId and expanding
        if let Some(&fn_id) = self.resolver.fns.get(name) {
            let _ = fn_id;
            // For now, error — user function inlining will be handled separately
            return Err(CodegenError::unsupported(format!(
                "user function `{name}` inlining in digital codegen — not yet implemented via POM path"
            )));
        }
        Err(CodegenError::unsupported(format!("unknown call target `{name}`")))
    }

    /// Load an analog branch voltage V(plus) - V(minus) for the A2D bridge.
    fn load_branch(&mut self, plus: NodeId, minus: NodeId) -> Result<Typed, CodegenError> {
        let ptrs = self.pointers.as_ref().expect("digital context");
        let layout = self.layout.expect("digital context");
        let module = self.module;
        let load_analog = |builder: &mut FunctionBuilder, node: NodeId| -> Result<Value, CodegenError> {
            if node.is_ground() {
                Ok(builder.ins().f64const(0.0))
            } else if let Some(idx) = layout.analog_index(node) {
                Ok(builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    ptrs.analog_voltages,
                    (idx * 8) as i32,
                ))
            } else {
                Err(CodegenError::Invalid(format!(
                    "analog node `{}` is not in the analog voltage array",
                    module.symbols.node(node).name
                )))
            }
        };
        let vp = load_analog(self.builder, plus)?;
        let vm = load_analog(self.builder, minus)?;
        Ok(Typed::real(self.builder.ins().fsub(vp, vm)))
    }

    /// Emit a `$`-syscall (simulator query).
    pub fn syscall(&mut self, name: &str, args: &[Expr]) -> Result<Typed, CodegenError> {
        let _ = args;
        let sim = self.pointers.expect("digital context").sim;
        match name {
            "$abstime" => {
                let value = self.builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    sim,
                    8,
                );
                Ok(Typed::real(value))
            }
            "$temperature" => {
                let value = self.builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    sim,
                    0,
                );
                Ok(Typed::real(value))
            }
            _ => Err(CodegenError::unsupported(format!("syscall `{name}` in digital body"))),
        }
    }

    /// Emit an if/else with Cranelift blocks.
    pub fn emit_if_branch(
        &mut self,
        flag: Value,
        then_body: &[Stmt],
        else_body: &[Stmt],
    ) -> Result<(), CodegenError> {
        let then_block = self.builder.create_block();
        let else_block = self.builder.create_block();
        let merge_block = self.builder.create_block();

        self.builder.ins().brif(flag, then_block, &[], else_block, &[]);

        self.builder.switch_to_block(then_block);
        self.builder.seal_block(then_block);
        for stmt in then_body {
            self.emit_stmt(stmt)?;
        }
        self.builder.ins().jump(merge_block, &[]);

        self.builder.switch_to_block(else_block);
        self.builder.seal_block(else_block);
        for stmt in else_body {
            self.emit_stmt(stmt)?;
        }
        self.builder.ins().jump(merge_block, &[]);

        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);
        Ok(())
    }

    /// Emit a statement (dispatch on POM `Stmt`).
    /// This is the statement-level dispatch — moved here because statements are
    /// a fixed set and don't need a trait.
    pub fn emit_stmt(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
        use piperine_lang::parse::ast::Stmt as S;
        match stmt {
            S::Bind { dest, op, src } => {
                let value = src.emit(self)?;
                match op {
                    BindOp::Assign | BindOp::Force => {
                        self.emit_assign(dest, value)?;
                    }
                    BindOp::Contrib => {
                        return Err(CodegenError::unsupported(
                            "analog contribution `<+` in a digital body",
                        ));
                    }
                }
                Ok(())
            }
            S::VarDecl { name, ty: _, default } => {
                if let Some(init) = default {
                    let value = init.emit(self)?;
                    if let Some(&id) = self.resolver.vars.get(name) {
                        self.store_var(id, value)?;
                    }
                }
                Ok(())
            }
            S::If { cond, then_body, else_body } => {
                let c = cond.emit(self)?;
                let flag = self.truthy(c)?;
                let else_stmts: &[Stmt] = match else_body {
                    Some(b) => &b.stmts,
                    None => &[],
                };
                self.emit_if_branch(flag, &then_body.stmts, else_stmts)
            }
            S::Match { expr, arms } => {
                let scrutinee = expr.emit(self)?;
                self.emit_match(scrutinee, arms)
            }
            S::Event { .. } => Err(CodegenError::Invalid(
                "clocked block in combinational context".into(),
            )),
            S::Diagnostic { .. } => Ok(()), // collected, not executed
            S::Return(_) => Ok(()),         // handled by inliner
            S::Expr(_) => Ok(()),
            S::For { .. } => Err(CodegenError::unsupported(
                "`for` loop — must be unrolled at elaboration",
            )),
        }
    }

    /// Assign to a destination (var or net).
    fn emit_assign(&mut self, dest: &Expr, value: Typed) -> Result<(), CodegenError> {
        match dest {
            Expr::Ident(name) => {
                if let Some(&id) = self.resolver.vars.get(name) {
                    self.store_var(id, value)?;
                    return Ok(());
                }
                if let Some(&id) = self.resolver.nodes.get(name) {
                    self.store_net(id, value)?;
                    return Ok(());
                }
                Err(CodegenError::Invalid(format!(
                    "cannot assign to `{name}` — not a var or output net"
                )))
            }
            _ => Err(CodegenError::unsupported("complex assignment target (bus indexing)")),
        }
    }

    /// Store a value to a variable slot.
    fn store_var(&mut self, id: VarId, value: Typed) -> Result<(), CodegenError> {
        let info = self.module.symbols.var(id);
        let layout = self.layout.expect("digital context");
        let ptrs = self.pointers.expect("digital context");
        let (slot, bank, target_ty) = match info.ty {
            Type::Real => {
                let slot = layout.real_slot(id).expect("layout covers all vars");
                (slot, ptrs.vars_real, DigTy::Real)
            }
            Type::Quad => {
                let slot = layout.int_slot(id).expect("layout covers all vars");
                (slot, ptrs.vars_int, DigTy::Quad)
            }
            Type::Integer | Type::Bool => {
                let slot = layout.int_slot(id).expect("layout covers all vars");
                (slot, ptrs.vars_int, DigTy::Int)
            }
        };
        let value = self.coerce(value, target_ty)?;
        self.builder.ins().store(
            MemFlags::trusted(),
            value.value,
            bank,
            (slot * 8) as i32,
        );
        Ok(())
    }

    /// Store a value to an output net.
    fn store_net(&mut self, id: NodeId, value: Typed) -> Result<(), CodegenError> {
        let layout = self.layout.expect("digital context");
        let outputs = self.pointers.expect("digital context").outputs;
        let index = layout.output_index.get(&id).copied().ok_or_else(|| {
            CodegenError::Invalid(format!(
                "assignment to net `{}` which is not a digital output",
                self.module.symbols.node(id).name
            ))
        })?;
        let value = self.coerce(value, DigTy::Quad)?;
        self.builder.ins().store(
            MemFlags::trusted(),
            value.value,
            outputs,
            (index * 8) as i32,
        );
        Ok(())
    }

    /// Emit a match statement.
    fn emit_match(
        &mut self,
        scrutinee: Typed,
        arms: &[piperine_lang::parse::ast::StmtMatchArm],
    ) -> Result<(), CodegenError> {
        match arms {
            [] => Ok(()),
            [arm, rest @ ..] => {
                let flag = self.pattern_flag(scrutinee, &arm.pat)?;
                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                self.builder.ins().brif(flag, then_block, &[], else_block, &[]);

                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                for stmt in &arm.body.stmts {
                    self.emit_stmt(stmt)?;
                }
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                self.emit_match(scrutinee, rest)?;
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);
                Ok(())
            }
        }
    }

    /// The i1 flag for "scrutinee matches pattern".
    fn pattern_flag(&mut self, scrutinee: Typed, pattern: &Pattern) -> Result<Value, CodegenError> {
        match pattern {
            Pattern::Wildcard => Ok(self.builder.ins().iconst(types::I8, 1)),
            Pattern::Literal(val) => {
                let value = Typed::int(self.builder_i64(*val as i64));
                let value = self.coerce(value, scrutinee.ty)?;
                match scrutinee.ty {
                    DigTy::Real => Ok(self.builder.ins().fcmp(FloatCC::Equal, scrutinee.value, value.value)),
                    DigTy::Int | DigTy::Quad => {
                        Ok(self.builder.ins().icmp(IntCC::Equal, scrutinee.value, value.value))
                    }
                }
            }
            Pattern::Path(p) => {
                let name = if p.segments.len() == 1 {
                    &p.segments[0]
                } else {
                    p.segments.last().unwrap()
                };
                Err(CodegenError::unsupported(format!(
                    "enum pattern `{name}` — enum resolution not yet wired"
                )))
            }
            Pattern::BitPattern(s) => match s.as_str() {
                "?" => Ok(self.builder.ins().iconst(types::I8, 1)),
                "0" | "1" => {
                    let target = i64::from(s.as_str() == "1");
                    let scrutinee = self.coerce(scrutinee, DigTy::Quad)?;
                    let target_val = self.builder_i64(target);
                    Ok(self.builder.ins().icmp(IntCC::Equal, scrutinee.value, target_val))
                }
                _ => Err(CodegenError::unsupported(
                    "multi-bit patterns in a digital `match` (bus signals)",
                )),
            },
        }
    }

    /// Emit a guarded clocked block: `if fired[index] { body }`.
    pub fn emit_guarded_block(&mut self, index: usize, body: &[Stmt]) -> Result<(), CodegenError> {
        let fired = self.pointers.expect("digital context").fired;
        let fired_val = self.builder.ins().load(
            types::I64,
            MemFlags::trusted(),
            fired,
            (index * 8) as i32,
        );
        let zero = self.builder.ins().iconst(types::I64, 0);
        let flag = self.builder.ins().icmp(IntCC::NotEqual, fired_val, zero);
        self.emit_if_branch(flag, body, &[])
    }

    // ── Loads (copied from DigitalEmitter) ──

    /// Read a net (digital input or output) as a quad value.
    fn load_net(&mut self, node: NodeId) -> Result<Typed, CodegenError> {
        let layout = self.layout.expect("digital context");
        let ptrs = self.pointers.expect("digital context");
        if let Some(&i) = layout.input_index.get(&node) {
            let value = self.builder.ins().load(
                types::I64,
                MemFlags::trusted(),
                ptrs.inputs,
                (i * 8) as i32,
            );
            return Ok(Typed::quad(value));
        }
        if let Some(&i) = layout.output_index.get(&node) {
            let value = self.builder.ins().load(
                types::I64,
                MemFlags::trusted(),
                ptrs.outputs,
                (i * 8) as i32,
            );
            return Ok(Typed::quad(value));
        }
        Err(CodegenError::Invalid(format!(
            "net `{}` is neither a digital input nor output",
            self.module.symbols.node(node).name
        )))
    }

    pub(crate) fn load_var(&mut self, var: VarId) -> Typed {
        let info = self.module.symbols.var(var);
        let layout = self.layout.expect("digital context");
        let ptrs = self.pointers.expect("digital context");
        let reads = self.reads.expect("digital context");
        match info.ty {
            Type::Real => {
                let slot = layout.real_slot(var).expect("layout covers all vars");
                let bank = match reads {
                    VarReads::Live => ptrs.vars_real,
                    VarReads::PreEdge => ptrs.vars_real_old,
                };
                let value =
                    self.builder
                        .ins()
                        .load(types::F64, MemFlags::trusted(), bank, (slot * 8) as i32);
                Typed::real(value)
            }
            ty => {
                let slot = layout.int_slot(var).expect("layout covers all vars");
                let bank = match reads {
                    VarReads::Live => ptrs.vars_int,
                    VarReads::PreEdge => ptrs.vars_int_old,
                };
                let value =
                    self.builder
                        .ins()
                        .load(types::I64, MemFlags::trusted(), bank, (slot * 8) as i32);
                match ty {
                    Type::Quad => Typed::quad(value),
                    _ => Typed::int(value),
                }
            }
        }
    }

    // ── Expressions ──

    fn emit_math(&mut self, name: &str, args: &[Expr]) -> Result<Typed, CodegenError> {
        let values = args
            .iter()
            .map(|a| {
                let v = a.emit(self)?;
                Ok(self.coerce(v, DigTy::Real)?.value)
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let result = self.call_math(name, &values)?;
        Ok(Typed::real(result))
    }

    pub(crate) fn emit_unary(&mut self, op: UnOp, x: Typed) -> Result<Typed, CodegenError> {
        match (op, x.ty) {
            (UnOp::Neg, DigTy::Real) => Ok(Typed::real(self.builder_fneg(x.value))),
            (UnOp::Neg, DigTy::Int) => Ok(Typed::int(self.builder_ineg(x.value))),
            (UnOp::Not | UnOp::BitNot, DigTy::Quad) => {
                let x = self.normalize_z(x.value);
                Ok(Typed::quad(self.quad_not(x)))
            }
            (UnOp::Not, DigTy::Int) => {
                let zero = self.builder_i64(0);
                let flag = self.builder.ins().icmp(IntCC::Equal, x.value, zero);
                Ok(Typed::int(self.builder_flag_i64(flag)))
            }
            (UnOp::Not, DigTy::Real) => {
                let zero = self.builder_f64(0.0);
                let flag = self.builder.ins().fcmp(FloatCC::Equal, x.value, zero);
                Ok(Typed::int(self.builder_flag_i64(flag)))
            }
            (UnOp::BitNot, DigTy::Int) => Ok(Typed::int(self.builder.ins().bnot(x.value))),
            // A reduction over a scalar is the scalar (buses are rejected).
            (UnOp::RedAnd | UnOp::RedOr | UnOp::RedXor, DigTy::Quad | DigTy::Int) => Ok(x),
            (op, ty) => Err(CodegenError::unsupported(format!(
                "unary {op:?} on {ty:?} in digital codegen"
            ))),
        }
    }

    pub(crate) fn emit_binary(&mut self, op: BinOp, a: Typed, b: Typed) -> Result<Typed, CodegenError> {
        use BinOp::*;
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
    pub(crate) fn truthy(&mut self, v: Typed) -> Result<Value, CodegenError> {
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

    pub(crate) fn unify(&mut self, a: Typed, b: Typed) -> Result<(Typed, Typed), CodegenError> {
        if a.ty == b.ty {
            return Ok((a, b));
        }
        if a.ty == DigTy::Real || b.ty == DigTy::Real {
            return Ok((self.coerce(a, DigTy::Real)?, self.coerce(b, DigTy::Real)?));
        }
        // Int vs Quad: 0/1 integers lift losslessly into 4-state.
        Ok((self.coerce(a, DigTy::Quad)?, self.coerce(b, DigTy::Quad)?))
    }

    pub(crate) fn coerce(&mut self, v: Typed, ty: DigTy) -> Result<Typed, CodegenError> {
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

    pub(crate) fn builder_f64(&mut self, v: f64) -> Value {
        self.builder.ins().f64const(v)
    }

    pub(crate) fn builder_i64(&mut self, v: i64) -> Value {
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

    // ═══════════════════════════════════════════════════════════════════════════
    //  Analog emission (POM Expr → f64 Value, with CSE)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Emit a POM `Expr` as a scalar `f64` Cranelift `Value` in analog context.
    /// This is the analog counterpart of the digital `Codegen::emit` trait —
    /// same role as the former `AnalogEmitter::emit` but dispatching on POM
    /// `Expr` instead of `IrExpr`.
    pub fn emit_analog(&mut self, expr: &Expr) -> Result<Value, CodegenError> {
        match expr {
            Expr::Literal(Literal::Real(v)) => Ok(self.cse_const(*v)),
            Expr::Literal(Literal::Int(v)) => Ok(self.cse_const(*v as f64)),
            Expr::Literal(Literal::Bool(b)) => Ok(self.cse_const(f64::from(*b))),

            // A bare identifier: param, module-level var, or state marker.
            Expr::Ident(name) => {
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

            // Branch access: V(p,n) / I(p,n).
            Expr::Call(func, args) => {
                if let Expr::Ident(name) = func.as_ref() {
                    match name.as_str() {
                        "V" | "I" => return self.emit_analog_branch(args),
                        "__state_load" => return self.emit_state_load(args),
                        _ => {}
                    }
                    // Math builtins (exp, ln, sqrt, sin, …)
                    if math::math_fn(name).is_some() {
                        return self.emit_analog_math_call(name, args);
                    }
                }
                Err(CodegenError::unsupported(format!(
                    "call `{}` in an analog expression (should be inlined)",
                    ident_from_expr(Some(func)).unwrap_or_default()
                )))
            }

            // Syscalls: $temperature, $abstime, $vt, $simparam, $limit, …
            Expr::SysCall(name, args) => self.emit_analog_syscall(name, args),

            Expr::Unary(op, x) => self.emit_analog_unary(op.clone(), x),

            Expr::Binary(lhs, op, rhs) => self.emit_analog_binary(op.clone(), lhs, rhs),

            // Ternary select: If { cond, then, else } → Cranelift select.
            Expr::If { cond, then_body, else_body } => {
                let c = self.emit_analog_truthy(cond)?;
                let t = self.emit_analog_block_value(then_body)?;
                let e = self.emit_analog_block_value(else_body)?;
                Ok(self.cse_op3(T_SELECT, c, t, e, |b| b.ins().select(c, t, e)))
            }

            Expr::Block(b) => self.emit_analog_block_value(b),

            Expr::Cast(_, inner) => self.emit_analog(inner),

            Expr::Field(base, field) => {
                // Flattened bundle field: "base_field" as a combined name.
                if let Expr::Ident(base_name) = base.as_ref() {
                    let combined = format!("{base_name}_{field}");
                    if let Some(&id) = self.resolver.params.get(&combined) {
                        return self.params.as_ref().expect("analog context")
                            .get(id.0 as usize)
                            .copied()
                            .ok_or_else(|| CodegenError::Invalid(format!("param #{} out of range", id.0)));
                    }
                    if let Some(&id) = self.resolver.vars.get(&combined) {
                        return Ok(self.cse_load(BANK_VARS, self.vars_ptr(), (id.0 * 8) as i32));
                    }
                }
                Err(CodegenError::unsupported(format!("unresolved field access in analog: {expr:?}")))
            }

            Expr::Literal(Literal::String(_)) | Expr::Literal(Literal::None) | Expr::Literal(Literal::Quad(_)) => {
                Err(CodegenError::unsupported("non-real literal in an analog expression"))
            }
            Expr::Path(_) => Err(CodegenError::unsupported("path in an analog expression")),
            Expr::Index(_, _) | Expr::Slice(_, _) | Expr::Array(_) | Expr::Tuple(_)
            | Expr::BundleLit { .. } | Expr::MapLit(_) | Expr::Lambda { .. } => {
                Err(CodegenError::unsupported("vector/value-layer expression in an analog contribution"))
            }
        }
    }

    /// Emit a branch voltage V(plus, minus) lookup from precomputed values.
    fn emit_analog_branch(&mut self, args: &[Expr]) -> Result<Value, CodegenError> {
        let plus_name = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
        let minus_name = ident_from_expr(args.get(1)).unwrap_or_else(|| "0".into());
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
            Some(Expr::Literal(Literal::Int(v))) => *v as u32,
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
                Ok(self.cse_op2(bin_tag(crate::ir::BinOp::Mul), temperature, kb_over_q, |b| {
                    b.ins().fmul(temperature, kb_over_q)
                }))
            }
            "simparam" => {
                let sim_key = match args.first() {
                    Some(Expr::Literal(Literal::String(s))) => s.clone(),
                    _ => "?".into(),
                };
                let default = args.get(1);
                match sim_key.as_str() {
                    "gmin" => Ok(self.load_sim_f64(SimField::GMIN)),
                    "temperature" => Ok(self.load_sim_f64(SimField::TEMPERATURE)),
                    "step" => self.sim_field_or_default(SimField::STEP, default),
                    "tfinal" => self.sim_field_or_default(SimField::TFINAL, default),
                    _ => default.map(|d| self.emit_analog(d)).unwrap_or(Ok(self.cse_const(0.0))),
                }
            }
            "param_given" => {
                let pname = match args.first() {
                    Some(Expr::Literal(Literal::String(s))) => s.clone(),
                    _ => "?".into(),
                };
                let id = *self.resolver.params.get(&pname).ok_or_else(|| {
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
                    Some(Expr::Literal(Literal::String(s))) => match s.as_str() {
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
            "limit" => self.emit_analog_limit(name, args),
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
            Some(Expr::Literal(Literal::String(s))) => s.as_str(),
            _ => return Err(CodegenError::unsupported("$limit expects a kind string")),
        };
        if args.len() < 5 {
            return Err(CodegenError::unsupported("$limit expects (kind, vnew, vseed, vte, vcrit)"));
        }
        // Find the slot by structural equality against the limits table.
        let limits = self.limits.as_ref().expect("analog context");
        let slot = limits.iter().position(|l| expr_structural_eq(l, &Expr::SysCall(full_name.to_string(), args.to_vec())))
            .ok_or_else(|| CodegenError::Invalid("$limit expression missing from slot table".into()))?;
        let key = CseKey::Limit(slot as u32);
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return Ok(hit);
        }
        let vnew = self.emit_analog(&args[1])?;
        let vte = self.emit_analog(&args[3])?;
        let vcrit = self.emit_analog(&args[4])?;
        let vold = self.cse_load(BANK_STATE, self.state_ptr(), ((self.limit_base + slot) * 8) as i32);
        let vlim = match kind {
            "pnjlim" => self.emit_pnjlim(vnew, vold, vte, vcrit)?,
            "fetlim" => vnew,
            other => return Err(CodegenError::unsupported(format!("$limit kind `{other}`"))),
        };
        self.cse.as_mut().expect("analog context").insert(key, vlim);
        Ok(vlim)
    }

    /// Branchless ngspice DEVpnjlim (copied from emit.rs).
    fn emit_pnjlim(&mut self, vnew: Value, vold: Value, vte: Value, vcrit: Value) -> Result<Value, CodegenError> {
        let dv = self.builder.ins().fsub(vnew, vold);
        let absdv = self.builder.ins().fabs(dv);
        let two = self.cse_const(2.0);
        let two_vte = self.builder.ins().fmul(two, vte);
        let cond1 = self.builder.ins().fcmp(FloatCC::GreaterThan, vnew, vcrit);
        let cond2 = self.builder.ins().fcmp(FloatCC::GreaterThan, absdv, two_vte);
        let cond = self.builder.ins().band(cond1, cond2);
        let one = self.cse_const(1.0);
        let dv_over_vte = self.builder.ins().fdiv(dv, vte);
        let arg = self.builder.ins().fadd(one, dv_over_vte);
        let ln_arg = self.analog_call_math("ln", &[arg])?;
        let vte_ln = self.builder.ins().fmul(vte, ln_arg);
        let vold_plus = self.builder.ins().fadd(vold, vte_ln);
        let zero = self.cse_const(0.0);
        let arg_pos = self.builder.ins().fcmp(FloatCC::GreaterThan, arg, zero);
        let posval = self.builder.ins().select(arg_pos, vold_plus, vcrit);
        let vnew_over_vte = self.builder.ins().fdiv(vnew, vte);
        let ln_vnew = self.analog_call_math("ln", &[vnew_over_vte])?;
        let negval = self.builder.ins().fmul(vte, ln_vnew);
        let vold_pos = self.builder.ins().fcmp(FloatCC::GreaterThan, vold, zero);
        let limited = self.builder.ins().select(vold_pos, posval, negval);
        Ok(self.builder.ins().select(cond, limited, vnew))
    }

    // ── Unary / binary / math ──

    fn emit_analog_unary(&mut self, op: UnaryOp, x: &Expr) -> Result<Value, CodegenError> {
        match op {
            UnaryOp::Neg => {
                let v = self.emit_analog(x)?;
                Ok(self.cse_op1(T_NEG, v, |b| b.ins().fneg(v)))
            }
            UnaryOp::Not => {
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
        }
    }

    fn emit_analog_binary(&mut self, op: BinaryOp, a: &Expr, b: &Expr) -> Result<Value, CodegenError> {
        let ir_op = lower_binop_pom(op);
        if ir_op == crate::ir::BinOp::Pow {
            let lhs = self.emit_analog(a)?;
            let rhs = self.emit_analog(b)?;
            return self.analog_call_math("pow", &[lhs, rhs]);
        }
        let lhs = self.emit_analog(a)?;
        let rhs = self.emit_analog(b)?;
        let key = CseKey::Op2(bin_tag(ir_op), lhs.as_u32(), rhs.as_u32());
        if let Some(&hit) = self.cse.as_mut().expect("analog context").get(&key) {
            return Ok(hit);
        }
        let cmp = |e: &mut Self, cc: FloatCC| {
            let flag = e.builder.ins().fcmp(cc, lhs, rhs);
            e.bool_to_f64(flag)
        };
        let val = match ir_op {
            crate::ir::BinOp::Add => self.builder.ins().fadd(lhs, rhs),
            crate::ir::BinOp::Sub => self.builder.ins().fsub(lhs, rhs),
            crate::ir::BinOp::Mul => self.builder.ins().fmul(lhs, rhs),
            crate::ir::BinOp::Div => self.builder.ins().fdiv(lhs, rhs),
            crate::ir::BinOp::Rem => {
                let quotient = self.builder.ins().fdiv(lhs, rhs);
                let floored = self.analog_call_math("floor", &[quotient])?;
                let product = self.builder.ins().fmul(floored, rhs);
                self.builder.ins().fsub(lhs, product)
            }
            crate::ir::BinOp::Eq => cmp(self, FloatCC::Equal),
            crate::ir::BinOp::Ne => cmp(self, FloatCC::NotEqual),
            crate::ir::BinOp::Lt => cmp(self, FloatCC::LessThan),
            crate::ir::BinOp::Le => cmp(self, FloatCC::LessThanOrEqual),
            crate::ir::BinOp::Gt => cmp(self, FloatCC::GreaterThan),
            crate::ir::BinOp::Ge => cmp(self, FloatCC::GreaterThanOrEqual),
            crate::ir::BinOp::And | crate::ir::BinOp::Or => {
                let zero = self.cse_const(0.0);
                let a_true = self.builder.ins().fcmp(FloatCC::NotEqual, lhs, zero);
                let b_true = self.builder.ins().fcmp(FloatCC::NotEqual, rhs, zero);
                let combined = if ir_op == crate::ir::BinOp::And {
                    self.builder.ins().band(a_true, b_true)
                } else {
                    self.builder.ins().bor(a_true, b_true)
                };
                self.bool_to_f64(combined)
            }
            crate::ir::BinOp::BitAnd | crate::ir::BinOp::BitOr
            | crate::ir::BinOp::BitXor | crate::ir::BinOp::Shl | crate::ir::BinOp::Shr => {
                return Err(CodegenError::unsupported(format!("bitwise/shift {ir_op:?} in an analog expression")));
            }
            crate::ir::BinOp::Pow => unreachable!("handled above"),
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

    /// Cached f64 constant (analog context).
    pub fn analog_f64const(&mut self, v: f64) -> Value {
        self.cse_const(v)
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

    /// Evaluate a POM `Block` to its expression value (analog context).
    fn emit_analog_block_value(&mut self, block: &piperine_lang::parse::ast::Block) -> Result<Value, CodegenError> {
        if let Some(e) = &block.expr {
            return self.emit_analog(e);
        }
        for s in block.stmts.iter().rev() {
            if let Stmt::Expr(e) = s {
                return self.emit_analog(e);
            }
        }
        Ok(self.cse_const(0.0))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn ident_from_expr(e: Option<&Expr>) -> Option<String> {
    match e? {
        Expr::Ident(s) => Some(s.clone()),
        Expr::Field(base, field) => match base.as_ref() {
            Expr::Ident(base_name) => Some(format!("{base_name}.{field}")),
            _ => None,
        },
        _ => None,
    }
}

/// Map a POM `BinaryOp` to the IR `BinOp` (shared by digital and analog paths).
fn lower_binop_pom(op: BinaryOp) -> crate::ir::BinOp {
    use piperine_lang::parse::ast::BinaryOp as P;
    match op {
        P::Add => crate::ir::BinOp::Add,
        P::Sub => crate::ir::BinOp::Sub,
        P::Mul => crate::ir::BinOp::Mul,
        P::Div => crate::ir::BinOp::Div,
        P::Rem => crate::ir::BinOp::Rem,
        P::Eq => crate::ir::BinOp::Eq,
        P::Neq => crate::ir::BinOp::Ne,
        P::Lt => crate::ir::BinOp::Lt,
        P::Le => crate::ir::BinOp::Le,
        P::Gt => crate::ir::BinOp::Gt,
        P::Ge => crate::ir::BinOp::Ge,
        P::BitAnd => crate::ir::BinOp::BitAnd,
        P::BitOr => crate::ir::BinOp::BitOr,
        P::BitXor => crate::ir::BinOp::BitXor,
        P::And => crate::ir::BinOp::And,
        P::Or => crate::ir::BinOp::Or,
    }
}

/// Structural equality for POM `Expr` (which doesn't derive `PartialEq`).
/// Used for `$limit` slot deduplication.
pub fn expr_structural_eq(a: &Expr, b: &Expr) -> bool {
    use piperine_lang::parse::ast::Literal;
    match (a, b) {
        (Expr::Literal(la), Expr::Literal(lb)) => match (la, lb) {
            (Literal::Real(x), Literal::Real(y)) => x == y,
            (Literal::Int(x), Literal::Int(y)) => x == y,
            (Literal::Bool(x), Literal::Bool(y)) => x == y,
            (Literal::String(x), Literal::String(y)) => x == y,
            (Literal::Quad(x), Literal::Quad(y)) => x == y,
            (Literal::None, Literal::None) => true,
            _ => false,
        },
        (Expr::Ident(x), Expr::Ident(y)) => x == y,
        (Expr::Path(x), Expr::Path(y)) => x.segments == y.segments,
        (Expr::SysCall(na, aa), Expr::SysCall(nb, ab)) => {
            na == nb && aa.len() == ab.len()
                && aa.iter().zip(ab).all(|(x, y)| expr_structural_eq(x, y))
        }
        (Expr::Call(fa, aa), Expr::Call(fb, ab)) => {
            expr_structural_eq(fa, fb) && aa.len() == ab.len()
                && aa.iter().zip(ab).all(|(x, y)| expr_structural_eq(x, y))
        }
        (Expr::Unary(oa, xa), Expr::Unary(ob, xb)) => oa == ob && expr_structural_eq(xa, xb),
        (Expr::Binary(la, oa, ra), Expr::Binary(lb, ob, rb)) => {
            oa == ob && expr_structural_eq(la, lb) && expr_structural_eq(ra, rb)
        }
        (Expr::Cast(ta, xa), Expr::Cast(tb, xb)) => ta == tb && expr_structural_eq(xa, xb),
        (Expr::Field(ba, fa), Expr::Field(bb, fb)) => expr_structural_eq(ba, bb) && fa == fb,
        (Expr::Index(ba, ia), Expr::Index(bb, ib)) => expr_structural_eq(ba, bb) && expr_structural_eq(ia, ib),
        (Expr::If { cond: ca, then_body: ta, else_body: ea },
         Expr::If { cond: cb, then_body: tb, else_body: eb }) => {
            expr_structural_eq(ca, cb)
                && blocks_eq(ta, tb)
                && blocks_eq(ea, eb)
        }
        _ => false,
    }
}

fn blocks_eq(a: &piperine_lang::parse::ast::Block, b: &piperine_lang::parse::ast::Block) -> bool {
    a.stmts.len() == b.stmts.len()
        && a.stmts.iter().zip(&b.stmts).all(|(x, y)| stmts_eq(x, y))
        && match (&a.expr, &b.expr) {
            (Some(x), Some(y)) => expr_structural_eq(x, y),
            (None, None) => true,
            _ => false,
        }
}

fn stmts_eq(a: &piperine_lang::parse::ast::Stmt, b: &piperine_lang::parse::ast::Stmt) -> bool {
    use piperine_lang::parse::ast::Stmt as S;
    match (a, b) {
        (S::Bind { dest: da, op: oa, src: sa }, S::Bind { dest: db, op: ob, src: sb }) => {
            oa == ob && expr_structural_eq(da, db) && expr_structural_eq(sa, sb)
        }
        (S::Expr(ea), S::Expr(eb)) => expr_structural_eq(ea, eb),
        (S::VarDecl { name: na, default: da, .. }, S::VarDecl { name: nb, default: db, .. }) => {
            na == nb && match (da, db) {
                (Some(x), Some(y)) => expr_structural_eq(x, y),
                (None, None) => true,
                _ => false,
            }
        }
        _ => false,
    }
}
