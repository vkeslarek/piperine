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

/// T23/LRMW2-14 (LRM §8.5.3.3/4): which region an assignment's write lands
/// in — `Blocking` (`=`) writes the live bank immediately (Active region);
/// `NonBlocking` (`<=`) stages the write for the device to commit after the
/// call returns (Nba region). Both read the live banks for their
/// right-hand side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignKind {
    Blocking,
    NonBlocking,
}

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
            // T23/LRMW2-14 (LRM §8.5.3.3/4): `=` writes land in the Active
            // region — live immediately, visible to a later statement in
            // the same block. `<=` evaluates its right-hand side against
            // the same live banks (so it still sees any earlier *blocking*
            // write this block) but the write itself lands in the Nba
            // region — staged, and only committed to the live banks after
            // the whole `comb`/`seq` call returns (`DigitalInstance::run`/
            // `eval_seq_phase`/`eval_comb_phase`), so `a <= b; b <= a;`
            // swaps regardless of statement order.
            S::Assign { target, value, .. } => {
                let v = value.emit(self)?;
                self.emit_assign(target, v, AssignKind::Blocking)
            }
            S::NonBlockingAssign { target, value, .. } => {
                let v = value.emit(self)?;
                self.emit_assign(target, v, AssignKind::NonBlocking)
            }
            S::Contribution { .. } => Err(CodegenError::unsupported(
                "analog contribution `<+` in a digital body",
            )),
            S::IndirectContribution { .. } => Err(CodegenError::unsupported(
                "analog indirect contribution `:` (LRM §5.6.7) in a digital body",
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
            // LRMC-C7-07 (T43): `$display`-family output, dispatched to the
            // `pip_task_*` trampolines (`emit::runtime_io`) via a real
            // Cranelift-emitted call — `self.tasks`, declared once per
            // compiled function (`kernel/digital/compile.rs`). Static
            // format strings only (`argc=0, argv=null`): dynamic
            // `%d`/... argument marshaling is T44's own scope (the
            // `TaskArg` ABI already exists for it — `runtime_io.rs`'s own
            // doc comment). `$strobe`/`$monitor` route through the same
            // `pip_task_display` call as `$display` rather than deferring
            // to the end-of-timestep Monitor region (T38) — a documented
            // scope limit: comb_phase already only re-evaluates on a real
            // input change (no Newton-iteration-style duplication concern
            // digital shares with analog's own accept_timestep gating),
            // but genuinely deferred "settled value" semantics needs the
            // statement to *schedule* a Monitor-region event instead of
            // calling out immediately, a bigger control-flow change than
            // this task's stated `Where`. `$finish`/`$stop` stay a no-op
            // here (`pip_task_finish` exists but ending the analysis is
            // the solver's own concern, out of this task's scope).
            S::TaskCall { name, args, .. } => match name.trim_start_matches('$') {
                n @ ("display" | "write" | "strobe" | "monitor" | "warning" | "warn" | "error" | "fatal"
                | "info") => {
                    let fmt = match args.first() {
                        Some(Expr::Str { value, .. }) => value.clone(),
                        _ => return Ok(()),
                    };
                    if fmt.is_empty() {
                        return Ok(());
                    }
                    let trampoline = if n == "fatal" || n == "error" { "pip_task_fatal" } else if n == "write" {
                        "pip_task_write"
                    } else {
                        "pip_task_display"
                    };
                    let tasks = self.tasks.expect("digital context has a tasks table");
                    let func = tasks[trampoline];
                    let fmt_id = crate::emit::runtime_io::FormatPool::intern(&fmt);
                    let ctx_ptr = self.pointers.expect("digital context").sim;
                    let fmt_id_val = self.builder.ins().iconst(types::I32, fmt_id as i64);
                    let argc = self.builder.ins().iconst(types::I32, 0);
                    let argv = self.builder.ins().iconst(types::I64, 0);
                    self.builder.ins().call(func, &[ctx_ptr, fmt_id_val, argc, argv]);
                    Ok(())
                }
                "finish" | "stop" => Ok(()),
                // T13/LRMW2-09: shared with the analog arm
                // (`flatten/analog.rs`) through `CodegenError::unknown_task`
                // so the two lowering paths cannot silently drift apart.
                other => Err(CodegenError::unknown_task(other)),
            },
            // §5.9/§5.9.3 (LRMW2-20, T32): "digital loops follow the same
            // rule" as analog — elaboration-constant bounds unroll
            // (`crate::flatten::analog::unroll_loop`, shared with the
            // analog path so the two can't drift apart, same precedent as
            // `CodegenError::unknown_task` above); `forever` still has no
            // bound to unroll and stays a loud, named error.
            stmt @ (S::For { .. } | S::While { .. } | S::Repeat { .. }) => {
                let unrolled = crate::flatten::analog::unroll_loop(stmt)?;
                for s in &unrolled {
                    self.emit_stmt(s)?;
                }
                Ok(())
            }
            S::Forever { .. } => Err(CodegenError::unsupported(
                "digital `forever` statement has no elaboration-constant bound to unroll (LRM §5.9.3)",
            )),
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
            // §8.5.3.2 (T53, LRMW2-47): procedural continuous assignment /
            // force-release. This codegen's digital kernel re-evaluates
            // every comb-region statement in source order each pass
            // (`resolve/pom/body.rs`'s own "behaviors append, evaluated in
            // order" precedent) — `assign`/`force` are emitted as an
            // ordinary immediate (Active-region) write, exactly like `=`,
            // so a write from `assign`/`force` observably "sticks" for as
            // long as no *later*-evaluated statement also writes the same
            // target, matching the LRM's "overrides other drivers" intent
            // for the common case (a testbench statically driving one
            // signal) without a dedicated per-net "forced" override flag
            // that would suppress every other writer unconditionally (a
            // disclosed scope narrowing — LRM §8.5.3.2's *continuous*
            // re-tracking against a *concurrently racing* driver on the
            // same net is not modeled). `deassign`/`release` themselves
            // have no value effect (SIMPLIFICATION.md P5's own "harmless
            // no-op" shape) — the same narrowing means there is no active
            // override state for them to actively clear.
            S::ProceduralAssign { target, value, .. } | S::Force { target, value, .. } => {
                let v = value.emit(self)?;
                self.emit_assign(target, v, AssignKind::Blocking)
            }
            S::ProceduralDeassign { .. } | S::Release { .. } => Ok(()),
        }
    }

    /// Assign to a destination (var or net).
    fn emit_assign(&mut self, dest: &Expr, value: Typed, kind: AssignKind) -> Result<(), CodegenError> {
        match dest {
            Expr::Ident { name, .. } => {
                if let Some(&id) = self.resolver.vars.get(name) {
                    self.store_var(id, value, kind)?;
                    return Ok(());
                }
                if let Some(&id) = self.resolver.nodes.get(name) {
                    self.store_net(id, value, kind)?;
                    return Ok(());
                }
                Err(CodegenError::Invalid(format!(
                    "cannot assign to `{name}` — not a var or output net"
                )))
            }
            _ => Err(CodegenError::unsupported("complex assignment target (bus indexing)")),
        }
    }

    /// Store a value to a variable slot. A blocking (`=`) write lands live
    /// immediately; a non-blocking (`<=`) write stages into
    /// `vars_int_old`/`vars_real_old` (T23/LRMW2-14 — see `DigitalAbi`'s doc
    /// comment) for the device to commit after the call returns.
    fn store_var(&mut self, id: VarId, value: Typed, kind: AssignKind) -> Result<(), CodegenError> {
        let info = self.module.symbols.var(id);
        let layout = self.layout.expect("digital context");
        let ptrs = self.pointers.expect("digital context");
        let (slot, live_bank, nba_bank, target_ty) = match info.ty {
            Type::Real => {
                let slot = layout.real_slot(id).expect("layout covers all vars");
                (slot, ptrs.vars_real, ptrs.vars_real_old, DigTy::Real)
            }
            Type::Quad => {
                let slot = layout.int_slot(id).expect("layout covers all vars");
                (slot, ptrs.vars_int, ptrs.vars_int_old, DigTy::Quad)
            }
            Type::Integer | Type::Bool => {
                let slot = layout.int_slot(id).expect("layout covers all vars");
                (slot, ptrs.vars_int, ptrs.vars_int_old, DigTy::Int)
            }
        };
        let bank = match kind {
            AssignKind::Blocking => live_bank,
            AssignKind::NonBlocking => nba_bank,
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

    /// Store a value to an output net. A blocking (`=`) write lands live
    /// immediately; a non-blocking (`<=`) write stages into `outputs_nba`
    /// (T23/LRMW2-14) for the device to commit after the call returns.
    fn store_net(&mut self, id: NodeId, value: Typed, kind: AssignKind) -> Result<(), CodegenError> {
        let layout = self.layout.expect("digital context");
        let ptrs = self.pointers.expect("digital context");
        let index = layout.output_index.get(&id).copied().ok_or_else(|| {
            CodegenError::Invalid(format!(
                "assignment to net `{}` which is not a digital output",
                self.module.symbols.node(id).name
            ))
        })?;
        let bank = match kind {
            AssignKind::Blocking => ptrs.outputs,
            AssignKind::NonBlocking => ptrs.outputs_nba,
        };
        let value = self.coerce(value, DigTy::Quad)?;
        self.builder.ins().store(
            MemFlags::trusted(),
            value.value,
            bank,
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
