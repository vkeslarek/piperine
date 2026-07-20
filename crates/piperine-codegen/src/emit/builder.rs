//! The [`Builder`]: wraps Cranelift's `FunctionBuilder` and provides
//! high-level emission methods (arithmetic, quad logic, name resolution,
//! control flow) that the [`Codegen`](super::digital_expr::Codegen) trait impls call.

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, FuncRef, InstBuilder, MemFlags, TrapCode, Value};
use cranelift_frontend::FunctionBuilder;

use piperine_lang::parse::ast::{BindOp, BinaryOp, Expr, Pattern, Stmt};

use crate::resolve::{BinOp, LoweredBody, NodeId, ParamId, Type, UnOp, VarId};
use crate::jit::digital::compile::{Pointers, VarReads};
use crate::jit::digital::layout::DigitalLayout;
use crate::error::CodegenError;
use crate::jit::math;

use super::cse::CseKey;
use super::digital_expr::Codegen;
use super::resolver::{DigTy, Resolver, Typed};

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
    /// Marker tapes: a `__marker(id)` leaf evaluates `tapes[marker].exprs[id]`
    /// once and memoizes in `values`. Installed per kernel/branch via
    /// [`Builder::set_tape`] (and the named wrappers below): the value tape
    /// (`__temp`), the Jacobian branch's derivative tape (`__dtemp`), and
    /// the `.disto` higher-order tapes (`__ddtemp`, `__dtemp_inner`,
    /// `__dtemp1..3`, `__ddtemp12/13/23`, `__dddtemp123`).
    tapes: HashMap<&'static str, Tape>,
}

/// One expression tape (`__temp`, `__dtemp`, …): entries evaluated lazily,
/// memoized per id in `values`.
struct Tape {
    exprs: Vec<Expr>,
    values: Vec<Option<Value>>,
}

impl<'a, 'f, 'm> Builder<'a, 'f, 'm> {
    /// Construct a digital-context builder (quad logic, ABI pointers).
    #[allow(clippy::too_many_arguments)]
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
            tapes: HashMap::new(),
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
            tapes: HashMap::new(),
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

    pub(crate) fn state_ptr(&self) -> Value {
        self.state_ptr.expect("analog context")
    }
    pub(crate) fn vars_ptr(&self) -> Value {
        self.vars_ptr.expect("analog context")
    }
    pub(crate) fn sim_ptr(&self) -> Value {
        self.sim_ptr.expect("analog context")
    }

    /// Install the value tape (`var` definitions). Call once per function
    /// before emitting contributions.
    pub(crate) fn set_value_tape(&mut self, temps: Vec<Expr>) {
        self.set_tape("__temp", temps);
    }

    /// Install the derivative tape for the current Jacobian branch and clear
    /// its memo cache. Call once per branch.
    pub(crate) fn set_deriv_tape(&mut self, dtemps: Vec<Expr>) {
        self.set_tape("__dtemp", dtemps);
    }

    /// Install the second-derivative tape for the current `.disto` branch
    /// pair (`d²(temps[id])/dV(a,b)dV(c,d)`) and clear its memo cache. Call
    /// once per branch pair, alongside [`Builder::set_deriv_tape`] for the
    /// inner branch (DISTO-03).
    pub(crate) fn set_ddtemp_tape(&mut self, ddtemps: Vec<Expr>) {
        self.set_tape("__ddtemp", ddtemps);
    }

    /// Install the second first-derivative tape for the current `.disto`
    /// branch pair (`d(temps[id])/dV(a,b)` — the pair's `(a,b)` branch) and
    /// clear its memo cache (DISTO-03).
    pub(crate) fn set_deriv_tape2(&mut self, dtemps: Vec<Expr>) {
        self.set_tape("__dtemp_inner", dtemps);
    }

    /// Install the tape behind `marker` and clear its memo cache: every
    /// `marker(id)` leaf emitted from here on evaluates `exprs[id]`.
    pub(crate) fn set_tape(&mut self, marker: &'static str, exprs: Vec<Expr>) {
        let values = vec![None; exprs.len()];
        self.tapes.insert(marker, Tape { exprs, values });
    }

    /// Whether `name` is a tape-marker call (`__temp`, `__dtemp`,
    /// `__ddtemp12`, …) rather than another `__`-intrinsic.
    pub(crate) fn is_tape_marker(name: &str) -> bool {
        let Some(rest) = name.strip_prefix("__") else { return false; };
        rest.trim_start_matches('d').starts_with("temp")
    }

    /// Evaluate every entry of the named tapes in increasing id order, so
    /// subsequent emissions read memoized values instead of recursing
    /// through marker chains — emission recursion depth stays at one
    /// entry's own tree depth (long temp chains would otherwise overflow
    /// the stack on the `.disto` multi-tape kernels). Entries reference
    /// only earlier ids of their own or lower tapes, so one ordered pass
    /// resolves everything.
    pub(crate) fn force_tapes(&mut self, markers: &[&'static str]) -> Result<(), CodegenError> {
        let len = markers
            .iter()
            .filter_map(|m| self.tapes.get(m))
            .map(|t| t.exprs.len())
            .max()
            .unwrap_or(0);
        for id in 0..len {
            for marker in markers {
                let has = self.tapes.get(marker).is_some_and(|t| id < t.exprs.len());
                if has {
                    self.emit_tape(marker, id)?;
                }
            }
        }
        Ok(())
    }

    /// Emit `__marker(id)`: entry `id` of the tape installed behind
    /// `marker`, evaluated once and memoized. Tapes only reference earlier
    /// ids of their own/lower-order tapes, so no cycle.
    pub(crate) fn emit_tape(&mut self, marker: &str, id: usize) -> Result<Value, CodegenError> {
        let expr = {
            let tape = self
                .tapes
                .get(marker)
                .ok_or_else(|| CodegenError::Invalid(format!("no tape installed for `{marker}`")))?;
            if let Some(Some(v)) = tape.values.get(id) {
                return Ok(*v);
            }
            tape.exprs
                .get(id)
                .cloned()
                .ok_or_else(|| CodegenError::Invalid(format!("{marker}({id}) out of range")))?
        };
        let v = self.emit_analog(&expr)?;
        self.tapes
            .get_mut(marker)
            .expect("tape checked above")
            .values[id] = Some(v);
        Ok(v)
    }

    // ── Name resolution & POM dispatch ──

    /// Resolve a name and load the corresponding value.
    /// Dispatches: var? param? net? enum value?
    pub fn load_ident(&mut self, name: &str) -> Result<Typed, CodegenError> {
        if piperine_lang::pom::is_ground(name) {
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

    pub(crate) fn resolve_node(&self, name: &str) -> Result<NodeId, CodegenError> {
        if piperine_lang::pom::is_ground(name) {
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
                let merge_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                self.builder.ins().brif(flag, then_block, &[], else_block, &[]);

                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                for stmt in &arm.body.stmts {
                    self.emit_stmt(stmt)?;
                }
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                if rest.is_empty() {
                    // Exhaustiveness is checked at elaboration time. If we
                    // reach here at runtime (e.g. an X/Z 4-state value not
                    // covered), trap loudly rather than silently falling through.
                    self.builder.ins().trap(TrapCode::unwrap_user(5));
                } else {
                    self.emit_match(scrutinee, rest)?;
                    self.builder.ins().jump(merge_block, &[]);
                }

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
    pub(crate) fn load_net(&mut self, node: NodeId) -> Result<Typed, CodegenError> {
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

}

// ─── Helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn ident_from_expr(e: Option<&Expr>) -> Option<String> {
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
pub(crate) fn lower_binop_pom(op: BinaryOp) -> crate::resolve::BinOp {
    use piperine_lang::parse::ast::BinaryOp as P;
    match op {
        P::Add => crate::resolve::BinOp::Add,
        P::Sub => crate::resolve::BinOp::Sub,
        P::Mul => crate::resolve::BinOp::Mul,
        P::Div => crate::resolve::BinOp::Div,
        P::Rem => crate::resolve::BinOp::Rem,
        P::Eq => crate::resolve::BinOp::Eq,
        P::Neq => crate::resolve::BinOp::Ne,
        P::Lt => crate::resolve::BinOp::Lt,
        P::Le => crate::resolve::BinOp::Le,
        P::Gt => crate::resolve::BinOp::Gt,
        P::Ge => crate::resolve::BinOp::Ge,
        P::BitAnd => crate::resolve::BinOp::BitAnd,
        P::BitOr => crate::resolve::BinOp::BitOr,
        P::BitXor => crate::resolve::BinOp::BitXor,
        P::And => crate::resolve::BinOp::And,
        P::Or => crate::resolve::BinOp::Or,
    }
}
