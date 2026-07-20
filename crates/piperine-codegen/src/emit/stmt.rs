//! Statement-level emission: [`Builder::emit_stmt`] dispatches on the POM
//! `Stmt` — a fixed set that doesn't need a trait — plus its helpers
//! (assignment, `match`, guarded/if-else control flow).

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, InstBuilder, MemFlags, TrapCode, Value};

use piperine_lang::parse::ast::{BindOp, Expr, Pattern, Stmt};

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
}
