//! The [`Builder`]: wraps Cranelift's `FunctionBuilder` and provides
//! high-level emission methods (arithmetic, quad logic, name resolution,
//! control flow) that the [`Codegen`](super::trait_::Codegen) trait impls call.

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, FuncRef, InstBuilder, MemFlags, Value};
use cranelift_frontend::FunctionBuilder;

use piperine_lang::parse::ast::{BindOp, Expr, Pattern, Stmt};

use crate::ir::{BinOp, FnId, LoweredBody, NodeId, ParamId, SimQuery, SymbolTable, Type, UnOp, VarId};
use crate::jit::digital::compile::{Pointers, VarReads};
use crate::jit::digital::layout::DigitalLayout;
use crate::jit::{math, CodegenError};

use super::trait_::Codegen;

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
/// One per compiled function.
pub struct Builder<'a, 'f, 'm> {
    pub builder: &'a mut FunctionBuilder<'f>,
    pub module: &'m LoweredBody,
    pub resolver: &'a Resolver,
    pub layout: &'a DigitalLayout,
    pub pointers: Pointers,
    pub reads: VarReads,
    pub math: &'a HashMap<&'static str, FuncRef>,
    pub watch_out: Option<Value>,
}

impl Builder<'_, '_, '_> {
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
        let value = self.builder.ins().load(
            types::F64,
            MemFlags::trusted(),
            self.pointers.params,
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
        let vp = load_analog(self.builder, plus)?;
        let vm = load_analog(self.builder, minus)?;
        Ok(Typed::real(self.builder.ins().fsub(vp, vm)))
    }

    /// Emit a `$`-syscall (simulator query).
    pub fn syscall(&mut self, name: &str, args: &[Expr]) -> Result<Typed, CodegenError> {
        let _ = args;
        match name {
            "$abstime" => {
                let value = self.builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    self.pointers.sim,
                    8,
                );
                Ok(Typed::real(value))
            }
            "$temperature" => {
                let value = self.builder.ins().load(
                    types::F64,
                    MemFlags::trusted(),
                    self.pointers.sim,
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
        match info.ty {
            Type::Real => {
                let slot = self.layout.real_slot(id).expect("layout covers all vars");
                let value = self.coerce(value, DigTy::Real)?;
                self.builder.ins().store(
                    MemFlags::trusted(),
                    value.value,
                    self.pointers.vars_real,
                    (slot * 8) as i32,
                );
            }
            Type::Quad => {
                let slot = self.layout.int_slot(id).expect("layout covers all vars");
                let value = self.coerce(value, DigTy::Quad)?;
                self.builder.ins().store(
                    MemFlags::trusted(),
                    value.value,
                    self.pointers.vars_int,
                    (slot * 8) as i32,
                );
            }
            Type::Integer | Type::Bool => {
                let slot = self.layout.int_slot(id).expect("layout covers all vars");
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

    /// Store a value to an output net.
    fn store_net(&mut self, id: NodeId, value: Typed) -> Result<(), CodegenError> {
        let index = self.layout.output_index.get(&id).copied().ok_or_else(|| {
            CodegenError::Invalid(format!(
                "assignment to net `{}` which is not a digital output",
                self.module.symbols.node(id).name
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
        let fired = self.builder.ins().load(
            types::I64,
            MemFlags::trusted(),
            self.pointers.fired,
            (index * 8) as i32,
        );
        let zero = self.builder.ins().iconst(types::I64, 0);
        let flag = self.builder.ins().icmp(IntCC::NotEqual, fired, zero);
        self.emit_if_branch(flag, body, &[])
    }

    // ── Loads (copied from DigitalEmitter) ──

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

    pub(crate) fn load_var(&mut self, var: VarId) -> Typed {
        let info = self.module.symbols.var(var);
        match info.ty {
            Type::Real => {
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
                    Type::Quad => Typed::quad(value),
                    _ => Typed::int(value),
                }
            }
        }
    }

    #[allow(dead_code)]
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
