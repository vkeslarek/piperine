//! Statement-level emission: [`Builder::emit_stmt`] dispatches on the POM
//! `Stmt` (`piperine_lang::ast::Stmt`) — a fixed set that doesn't need a
//! trait — plus its helpers (assignment, `case`, guarded/if-else control
//! flow).

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, InstBuilder, MemFlags, TrapCode, Value};

use piperine_lang::ast::{BinOp, CaseArm, CaseKind, Expr, Stmt};
use piperine_lang::lexer::NumberLit;

use crate::error::CodegenError;
use crate::resolve::{NodeId, Type, VarId};

use super::builder::Builder;
use super::resolver::{DigTy, Typed};
use super::digital_expr::Codegen;

impl<'a, 'f, 'm> Builder<'a, 'f, 'm> {
    /// Emit an if/else with Cranelift blocks.
    pub fn emit_if_branch(
        &mut self,
        flag: Value,
        then_body: &Stmt,
        else_body: Option<&Stmt>,
    ) -> Result<(), CodegenError> {
        let then_block = self.builder.create_block();
        let else_block = self.builder.create_block();
        let merge_block = self.builder.create_block();

        self.builder.ins().brif(flag, then_block, &[], else_block, &[]);

        self.builder.switch_to_block(then_block);
        self.builder.seal_block(then_block);
        self.emit_stmt(then_body)?;
        self.builder.ins().jump(merge_block, &[]);

        self.builder.switch_to_block(else_block);
        self.builder.seal_block(else_block);
        if let Some(else_body) = else_body {
            self.emit_stmt(else_body)?;
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
        use Stmt as S;
        match stmt {
            S::Null { .. } => Ok(()),
            // Blocking vs non-blocking assignment: full IEEE-1364 scheduling
            // semantics (a non-blocking RHS read happening-before every
            // blocking write in the same timestep, deferred until the
            // active-region's NBA update) is P3/VAMS-20 scope — today's
            // digital kernel already only covers the subset PHDL's own
            // `BindOp::Assign` did (immediate write), so both forms emit
            // identically here, same as before the frontend swap. Tracked
            // as a known simplification, not a silent behavior change from
            // what this crate already did.
            S::Assign { target, value, .. } | S::NonBlockingAssign { target, value, .. } => {
                let v = value.emit(self)?;
                self.emit_assign(target, v)
            }
            S::Contribution { .. } => Err(CodegenError::unsupported(
                "analog contribution `<+` in a digital body",
            )),
            S::Block { stmts, .. } => {
                for s in stmts {
                    self.emit_stmt(s)?;
                }
                Ok(())
            }
            S::If { cond, then_branch, else_branch, .. } => {
                let c = cond.emit(self)?;
                let flag = self.truthy(c)?;
                self.emit_if_branch(flag, then_branch, else_branch.as_deref())
            }
            S::Case { kind, selector, arms, .. } => {
                if !matches!(kind, CaseKind::Case) {
                    // casex/casez wildcard (X/Z-position) matching and the
                    // analog-only `analog_case` form are P3/VAMS-18/19
                    // scope — fail loud rather than silently treating a
                    // wildcard case as a plain equality case, which would
                    // give a wrong (over-strict) match result.
                    return Err(CodegenError::unsupported(format!(
                        "{kind:?} (wildcard case matching) in digital codegen"
                    )));
                }
                let scrutinee = selector.emit(self)?;
                self.emit_case(scrutinee, arms)
            }
            S::EventControl { .. } => Err(CodegenError::Invalid(
                "nested event control outside a top-level clocked block".into(),
            )),
            // `$display`-family diagnostic system tasks: collected/reported
            // elsewhere (not executed by this JIT path), matching PHDL's
            // own `Diagnostic` no-op. A genuine user-defined `task` call
            // has no lowering (VAMS `task`/`function` decls aren't lowered
            // into the POM yet — MD-45, see `resolve/pom/body.rs`).
            S::TaskCall { name, .. } => match name.trim_start_matches('$') {
                "display" | "write" | "strobe" | "monitor" | "warning" | "warn" | "error"
                | "fatal" | "info" | "finish" | "stop" => Ok(()),
                other => Err(CodegenError::unsupported(format!("task call `{other}`"))),
            },
            S::For { .. } | S::While { .. } | S::Repeat { .. } | S::Forever { .. } => {
                Err(CodegenError::unsupported(
                    "runtime loop statement (for/while/repeat/forever) in digital codegen",
                ))
            }
            S::DelayControl { .. } => {
                Err(CodegenError::unsupported("`#delay` control in digital codegen"))
            }
            S::Disable { .. } => Err(CodegenError::unsupported("`disable` in digital codegen")),
            // `-> e;` (§5.10.4, LRMC-C5-04): `e` is the hidden `integer`
            // var `elab::lower::vams` allocates per declared `event`
            // (T25/T26) — triggering toggles it (`e = 1 - e;`, alternating
            // 0/1) so any `Trigger::AnyChange` watcher (`@(e)`, digital
            // `EdgeKind::Any` already fires on any var write with zero
            // changes needed here, or analog's own `Named` trigger, T26's
            // cross-domain proof) sees a real change. A *toggle*, not an
            // unbounded increment: only ever one bit differs between 0 and
            // 1 in the digital var's own multi-bit net representation, so
            // a single trigger never fans out into more than one net-level
            // change event (an increment across a multi-bit boundary,
            // e.g. 1->2, flips more than one bit at once and was observed
            // to wake `@(e)` more than once per logical trigger). Desugars
            // and recurses through the exact same assignment path.
            S::EventTrigger { span, name } => {
                let target = Expr::Ident { span: *span, name: name.clone() };
                let value = Expr::Binary {
                    span: *span,
                    op: BinOp::Sub,
                    lhs: Box::new(Expr::Number { span: *span, value: NumberLit::Integer(1) }),
                    rhs: Box::new(target.clone()),
                };
                self.emit_stmt(&Stmt::Assign { span: *span, target, value })
            }
        }
    }

    /// Assign to a destination (var or net).
    fn emit_assign(&mut self, dest: &Expr, value: Typed) -> Result<(), CodegenError> {
        match dest {
            Expr::Ident { name, .. } => {
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

    /// Emit a `case` statement (LRM Annex A.6.7.1): the first arm whose
    /// label set contains a value equal to the scrutinee runs; a `default`
    /// arm (`labels: None`) always matches when reached.
    fn emit_case(&mut self, scrutinee: Typed, arms: &[CaseArm]) -> Result<(), CodegenError> {
        match arms {
            [] => Ok(()),
            [arm, rest @ ..] => {
                let flag = self.case_arm_flag(scrutinee, arm)?;
                let then_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                self.builder.ins().brif(flag, then_block, &[], else_block, &[]);

                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                self.emit_stmt(&arm.body)?;
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                if rest.is_empty() {
                    // Exhaustiveness is checked at elaboration time. If we
                    // reach here at runtime (e.g. an X/Z 4-state value not
                    // covered), trap loudly rather than silently falling through.
                    self.builder.ins().trap(TrapCode::unwrap_user(5));
                } else {
                    self.emit_case(scrutinee, rest)?;
                    self.builder.ins().jump(merge_block, &[]);
                }

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);
                Ok(())
            }
        }
    }

    /// The i1 flag for "scrutinee matches this arm's label set" — `None`
    /// (the `default` arm) always matches; otherwise any label equal to
    /// the scrutinee matches (OR'd).
    fn case_arm_flag(&mut self, scrutinee: Typed, arm: &CaseArm) -> Result<Value, CodegenError> {
        let Some(labels) = &arm.labels else {
            return Ok(self.builder.ins().iconst(types::I8, 1));
        };
        let mut flag: Option<Value> = None;
        for label in labels {
            let label_val = label.emit(self)?;
            let label_val = self.coerce(label_val, scrutinee.ty)?;
            let eq = match scrutinee.ty {
                DigTy::Real => self.builder.ins().fcmp(FloatCC::Equal, scrutinee.value, label_val.value),
                DigTy::Int | DigTy::Quad => {
                    self.builder.ins().icmp(IntCC::Equal, scrutinee.value, label_val.value)
                }
            };
            flag = Some(match flag {
                None => eq,
                Some(prev) => self.builder.ins().bor(prev, eq),
            });
        }
        Ok(flag.unwrap_or_else(|| self.builder.ins().iconst(types::I8, 0)))
    }

    /// Emit a guarded clocked block: `if fired[index] { body }`.
    pub fn emit_guarded_block(&mut self, index: usize, body: &Stmt) -> Result<(), CodegenError> {
        let fired = self.pointers.expect("digital context").fired;
        let fired_val = self.builder.ins().load(
            types::I64,
            MemFlags::trusted(),
            fired,
            (index * 8) as i32,
        );
        let zero = self.builder.ins().iconst(types::I64, 0);
        let flag = self.builder.ins().icmp(IntCC::NotEqual, fired_val, zero);
        self.emit_if_branch(flag, body, None)
    }
}
