//! The statements of the code walk, one method per statement form: a `let` binds a local of its
//! init's type, an assignment writes a `mutable` local, a `bound_step` and a contribution sit in the
//! `analog` block, a commit writes its carrier's type, and a trace or an emit writes its stream's
//! field types.

use super::TypeCheck;
use super::walk::{CodeWalk, WalkPosition, WalkStep};
use crate::ids::{ExprId, StmtId};
use crate::model::{Addend, BinaryOp, BranchKind, EmitField, Origin, Place, QuantityRole, Stmt, ValueType};
use crate::typing::Typing;

impl TypeCheck<'_> {
    /// The visit of statement `id` at `position` (`top` at the top of an `analog` block): its
    /// checks, then the steps of its expressions.
    pub(super) fn statement(&mut self, walk: &mut CodeWalk, id: StmtId, position: WalkPosition, top: bool) {
        let Some(stmt) = self.reader.ir.stmts.get(id.index()) else { return };
        if !walk.statements_done.insert((id, position, top, walk.scope)) {
            return;
        }
        let origin = self.reader.statement_origin(stmt);
        match stmt {
            Stmt::Let { local, init } => self.let_statement(walk, &origin, *local, *init, position),
            Stmt::Expr(e) => walk.pending.push(WalkStep::Expression(*e, position)),
            Stmt::Assign { place, op, value } => {
                self.assignment(walk, &origin, place, *op, *value);
                walk.pending.push(WalkStep::Expression(*value, position));
                walk.pending.extend(place.index_operands().map(|index| WalkStep::Expression(index, position)));
            }
            Stmt::BoundStep { value, when, .. } => self.bound_step(walk, &origin, *value, *when, position, top),
            Stmt::Contribute { branch, quantity, addends } => self.contribution(walk, &origin, *branch, *quantity, addends, position),
            Stmt::Commit { .. } => self.commit(walk, &origin, stmt, position),
            Stmt::Trace { site, fields } => self.trace(walk, &origin, *site, fields, position),
            Stmt::Emit { stream, fields } => self.emit(walk, &origin, *stream, fields, position),
        }
    }

    /// `let local = init`: `init` has the local's type, and the local is bound once it is evaluated.
    fn let_statement(&mut self, walk: &mut CodeWalk, origin: &Origin, local: u32, init: ExprId, position: WalkPosition) {
        match walk.owner.locals.get(local as usize).cloned() {
            Some(expected) => self.require_type(walk, init, &expected),
            None => self.findings.report(walk.owner.item.clone(), origin.clone(), format!("let binds local {local}, out of range ({} locals)", walk.owner.locals.len())),
        }
        walk.pending.push(WalkStep::Expression(init, position));
        walk.pending.push(WalkStep::Bind(local));
    }

    /// An assignment `place op= value` (§1.5.12): the target is a place of a `mutable` local, the
    /// value has the target's type, and `op`, when given, is a compound operator the type has.
    fn assignment(&mut self, walk: &CodeWalk, origin: &Origin, place: &Place, op: Option<BinaryOp>, value: ExprId) {
        let Some(target) = walk.owner.locals.get(place.local as usize).cloned() else {
            self.wrong(walk, origin.clone(), format!("assignment to local {}, out of range ({} locals)", place.local, walk.owner.locals.len()));
            return;
        };
        if !walk.owner.mutable.get(place.local as usize).copied().unwrap_or(false) {
            self.wrong(walk, origin.clone(), format!("assignment to local {}, which is not `mutable`", place.local));
        }
        let Some(target) = self.place_type(walk, origin, place, target) else { return };
        match op {
            None => self.want(walk, value, &target, "the value of an assignment"),
            Some(op) => {
                let result = self.reader.type_of(value).and_then(|v| Typing::compound(op, &target, v));
                if self.reader.type_of(value).is_some() && result.as_ref() != Some(&target) {
                    self.wrong(walk, origin.clone(), format!("`{op:?}` does not apply to a {target} target and this value"));
                }
            }
        }
    }

    /// A `bound_step` sits at the top of an `analog` block; its step is an `f64`, its `when` a `bool`.
    fn bound_step(&mut self, walk: &mut CodeWalk, origin: &Origin, value: ExprId, when: Option<ExprId>, position: WalkPosition, top: bool) {
        if !top || !walk.owner.analog {
            self.findings.report(walk.owner.item.clone(), origin.clone(), "`bound_step` outside the top level of an analog block".to_string());
        }
        self.want(walk, value, &ValueType::F64, "the step of a `bound_step`");
        walk.pending.push(WalkStep::Expression(value, position));
        if let Some(when) = when {
            self.want(walk, when, &ValueType::Bool, "the `when` of a `bound_step`");
            walk.pending.push(WalkStep::Expression(when, position));
        }
    }

    /// A contribution may stand under an `if` or in a nested block, anywhere inside the analog block
    /// but a closure or a loop (analog-structure, mechanism 1); its quantity is one its branch takes,
    /// and each addend is an `f64`.
    fn contribution(&mut self, walk: &mut CodeWalk, origin: &Origin, branch: u32, quantity: QuantityRole, addends: &[Addend], position: WalkPosition) {
        if !walk.owner.analog {
            self.findings.report(walk.owner.item.clone(), origin.clone(), "contribution outside the analog block of a body".to_string());
        } else if position.closure {
            self.findings.report(walk.owner.item.clone(), origin.clone(), "contribution inside a closure".to_string());
        } else if position.enclosing_loop.is_some() {
            // §11 rule 12: the elaborator unrolls a loop that contributes, so none reaches the IR.
            self.findings.report(walk.owner.item.clone(), origin.clone(), "contribution inside a loop".to_string());
        }
        match walk.owner.branches.get(branch as usize).copied() {
            None if walk.owner.analog => {
                self.findings.report(walk.owner.item.clone(), origin.clone(), format!("contribution to branch {branch}, out of range"));
            }
            Some(kind) => {
                let flow = quantity == QuantityRole::Flow;
                let matches = match kind {
                    BranchKind::Flow => flow,
                    BranchKind::Potential => !flow,
                    BranchKind::Switch => true,
                };
                if !matches {
                    self.findings.report(walk.owner.item.clone(), origin.clone(), format!("contribution quantity does not match the kind of branch {branch}"));
                }
            }
            None => {}
        }
        for a in addends {
            self.require_type(walk, a.value, &ValueType::F64);
            walk.pending.push(WalkStep::Expression(a.value, position));
        }
    }

    /// A commit writes a value of the type its target names inside its carrier, after an `f64` delay.
    fn commit(&mut self, walk: &mut CodeWalk, origin: &Origin, commit: &Stmt, position: WalkPosition) {
        let Stmt::Commit { carrier, value, after, target } = commit else { return };
        let (carrier, value, after, target) = (*carrier, *value, *after, target.as_ref());
        match walk.owner.carriers.get(carrier as usize).cloned() {
            Some(held) => {
                let ty = walk.owner.drive_types.get(carrier as usize).cloned().flatten().unwrap_or(held);
                if let Some(written) = self.commit_type(walk, origin, carrier, &ty, target) {
                    self.carried(walk, value, &written, "the value of a commit");
                }
            }
            None => self.findings.report(walk.owner.item.clone(), origin.clone(), format!("commit to carrier {carrier}, out of range ({} carriers)", walk.owner.carriers.len())),
        }
        walk.pending.push(WalkStep::Expression(value, position));
        if let Some(target) = target {
            walk.pending.extend(target.index_operands().map(|index| WalkStep::Expression(index, position)));
        }
        if let Some(after) = after {
            self.carried(walk, after, &ValueType::F64, "the delay of a commit");
            walk.pending.push(WalkStep::Expression(after, position));
        }
    }

    /// A trace is written where the analog block executes it: not in a function, a closure, a loop
    /// or a reaction. Its site names a stream of the body, and each field has the stream's type.
    fn trace(&mut self, walk: &mut CodeWalk, origin: &Origin, site: u32, fields: &[EmitField], position: WalkPosition) {
        if !walk.owner.analog {
            self.findings.report(walk.owner.item.clone(), origin.clone(), "a trace outside the analog block of a body".to_string());
        } else if position.closure {
            self.findings.report(walk.owner.item.clone(), origin.clone(), "a trace inside a closure".to_string());
        }
        let Some(stream) = walk.owner.traces.get(site as usize).copied() else {
            self.findings.report(walk.owner.item.clone(), origin.clone(), format!("trace of site {site}, out of range ({} sites)", walk.owner.traces.len()));
            return;
        };
        let Some(types) = walk.owner.streams.get(stream as usize).cloned() else {
            self.findings.report(walk.owner.item.clone(), origin.clone(), format!("trace site {site} names stream {stream}, out of range ({} streams)", walk.owner.streams.len()));
            return;
        };
        for f in fields {
            let EmitField::Value { field, value } = f else { continue };
            match types.get(*field as usize) {
                Some(ty) => self.carried(walk, *value, ty, "the value of a trace field"),
                None => self.findings.report(walk.owner.item.clone(), origin.clone(), format!("trace writes field {field} of stream {stream}, out of range ({} fields)", types.len())),
            }
            walk.pending.push(WalkStep::Expression(*value, position));
        }
    }

    /// An emit writes the fields of a stream of the body, each of the stream's type. An emit outside
    /// a reaction is named by the class check; its stream is a body's.
    fn emit(&mut self, walk: &mut CodeWalk, origin: &Origin, stream: u32, fields: &[EmitField], position: WalkPosition) {
        let types = if walk.owner.in_body { walk.owner.streams.get(stream as usize).cloned() } else { None };
        if walk.owner.in_body && types.is_none() {
            self.findings.report(walk.owner.item.clone(), origin.clone(), format!("emit to stream {stream}, out of range ({} streams)", walk.owner.streams.len()));
        }
        for f in fields {
            let EmitField::Value { field, value } = f else { continue };
            if let Some(types) = &types {
                match types.get(*field as usize) {
                    Some(ty) => self.carried(walk, *value, ty, "the value of an emit field"),
                    None => self.findings.report(walk.owner.item.clone(), origin.clone(), format!("emit writes field {field} of stream {stream}, out of range ({} fields)", types.len())),
                }
            }
            walk.pending.push(WalkStep::Expression(*value, position));
        }
    }
}
