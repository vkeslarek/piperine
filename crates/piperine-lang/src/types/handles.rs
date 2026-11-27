//! Handles (generics GN-11, GN-12, part I §3.1.5): a function parameter of a nature type, and `self` of an
//! `impl` on a nature, stands for the carrier the caller passes. A handle is read (a discrete one gives its
//! value), probed (`v(a)`, `temp(self)`), passed on to another function and has carrier methods called on it
//! (`a.prev()`); it is never returned or stored (`E0133`, `handles` rules below).
//!
//! The type of a handle is the type of the carrier: `Ty::Node` for a continuous nature, `Ty::Carrier` for a
//! discrete one. [`Types::handle`](super::Types::handle) says which parameters are handles.

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtKind};

use super::check::Checker;
use super::Bound;
use super::Ty;

impl Checker<'_> {
    /// What a read of a handle of type `ty` is: the value of a discrete carrier, the node itself.
    pub(super) fn handle_read_type(ty: &Ty) -> Ty {
        match ty {
            Ty::Carrier { value, .. } => (**value).clone(),
            other => other.clone(),
        }
    }

    /// Whether a parameter of type `ty` is a handle: the type is a node or a carrier of a nature.
    pub(super) fn is_handle_type(ty: &Ty) -> bool {
        matches!(ty, Ty::Node(_) | Ty::Carrier { .. })
    }

    /// Whether `ty` is a type parameter bound by `Nature`: what a function generic over a nature takes (`fn f<N: Nature>(a: N)`).
    pub(super) fn is_nature_parameter(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Param(param) if self.bounds_of(*param).contains(&Bound::Nature))
    }

    /// Records the parameter `param` of type `ty` as a handle when `ty` is one: a node or a carrier of a nature, or a
    /// type parameter bound by `Nature`.
    pub(super) fn note_handle(&mut self, param: DeclId, ty: &Ty) {
        if Self::is_handle_type(ty) || self.is_nature_parameter(ty) || self.is_bundle_type(ty) {
            self.handles.insert(param);
        }
    }

    /// The type of the argument `expr` for a parameter generic over a nature: the carrier it names (a discrete one is
    /// the carrier, not the value it reads, so the call's nature is inferred from it), else the type it reads as.
    pub(super) fn infer_carrier(&mut self, expr: ExprId) -> Option<Ty> {
        let read = self.infer(expr)?;
        match self.carrier_named(expr) {
            Some(carrier @ Ty::Carrier { .. }) => {
                self.exprs[expr.index()] = Some(Self::handle_read_type(&carrier));
                Some(carrier)
            }
            _ => Some(read),
        }
    }

    /// The handle parameter `expr` names, through parentheses.
    pub(super) fn handle_named(&self, expr: ExprId) -> Option<DeclId> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.handle_named(*inner),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) if self.handles.contains(&decl) => Some(decl),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether the handle `decl` is a node (a continuous carrier): a read of it is the node itself, which no value holds.
    /// A discrete carrier's read is its value (§3.1.6), so using it as a value is no handle kept.
    fn is_node_handle(&self, decl: DeclId) -> bool {
        !matches!(self.locals.get(&decl), Some(Ty::Carrier { .. }))
    }

    /// The expression of a handle that `expr` evaluates to: the handle's own path, or the one a block's tail, an
    /// `if` arm or a `match` arm yields (§3.1.5: storing a handle through a value position is storing it).
    pub(super) fn handle_held(&self, expr: ExprId) -> Option<ExprId> {
        if self.handle_named(expr).is_some_and(|decl| self.is_node_handle(decl)) {
            return Some(expr);
        }
        match &self.design.expr_node(expr).kind {
            ExprKind::Block(block) => self.handle_in_block(block),
            ExprKind::If { then, else_, .. } => self.handle_in_block(then).or_else(|| else_.and_then(|other| self.handle_held(other))),
            ExprKind::Match { arms, .. } => arms.iter().find_map(|arm| self.handle_held(arm.body)),
            _ => None,
        }
    }

    /// The handle a block's tail expression yields, if it has a tail.
    fn handle_in_block(&self, block: &crate::pom::BlockNode) -> Option<ExprId> {
        match block.stmts.last().map(|stmt| &self.design.stmt_node(*stmt).kind) {
            Some(StmtKind::Expr { expr, semi: false }) => self.handle_held(*expr),
            _ => None,
        }
    }

    /// What a read of the handle `decl` is: the value of a discrete carrier (§3.1.6), the node itself for a
    /// continuous one. `None` when `decl` is not a handle.
    pub(super) fn handle_read(&self, decl: DeclId) -> Option<Ty> {
        if !self.handles.contains(&decl) {
            return None;
        }
        match self.locals.get(&decl)? {
            Ty::Carrier { value, .. } => Some((**value).clone()),
            node => Some(node.clone()),
        }
    }

    /// The carrier type of the handle `decl`, for the carrier methods and the arguments that pass it on.
    pub(super) fn handle_type(&self, decl: DeclId) -> Option<Ty> {
        self.handles.contains(&decl).then(|| self.locals.get(&decl).cloned()).flatten()
    }

    /// Whether every operand of a probe is a handle, which is what a function's probe may read (§3 rule 7).
    pub(super) fn probes_only_handles(&self, operands: &[ExprId]) -> bool {
        !operands.is_empty() && operands.iter().all(|operand| self.handle_named(*operand).is_some() || self.bundle_handle_root(*operand).is_some())
    }

    /// An argument for a parameter of a nature type: the carrier it names (a port, a signal or a handle of the
    /// same nature and value) is passed on as it is (§3.1.5); anything else is the plain mismatch.
    pub(super) fn handle_argument(&mut self, expr: ExprId, expected: &Ty) {
        let passed = match &self.design.expr_node(expr).kind {
            ExprKind::Path(_) | ExprKind::Paren(_) | ExprKind::Index { .. } => self.carrier_named(expr),
            _ => None,
        };
        match passed {
            // the argument is read as the value the carrier holds; the elaborator binds the carrier itself (`elab/handles.rs`)
            Some(found) if found == *expected => {
                // an element `x[k]`: its base and its index are typed like any read
                if matches!(self.design.expr_node(expr).kind, ExprKind::Index { .. }) {
                    self.infer(expr);
                }
                self.exprs[expr.index()] = Some(Self::handle_read_type(&found));
            }
            _ => self.check(expr, expected),
        }
    }

    /// The type of the port, signal or handle `expr` names; `None` for any other expression.
    pub(super) fn carrier_named(&mut self, expr: ExprId) -> Option<Ty> {
        match self.declared_port(expr) {
            Some((_, ty)) => Some(ty),
            None => self.handle_named(expr).and_then(|decl| self.handle_type(decl)),
        }
    }

    /// `E0133` for every use of a handle in `function` that keeps it past a call (§3.1.5, D12): the result type of
    /// the function is a handle, a `return` names one, a `let`, a struct field, a tuple or array element or a closure
    /// holds one, or an assignment or a commit writes through one. Passing it as an argument is the use it is for.
    pub(super) fn check_handle_uses(&mut self, function: DeclId) {
        if self.handles.is_empty() {
            return;
        }
        let file = self.design.decl_data(function).file;
        let mut found: Vec<(Span, ExprId, &'static str)> = Vec::new();
        let mut written: Vec<ExprId> = Vec::new();
        for index in 0..self.design.expr_count() as u32 {
            let expr = ExprId(index);
            let node = self.design.expr_node(expr);
            if node.owner != function {
                continue;
            }
            match &node.kind {
                ExprKind::Struct { fields, .. } => found.extend(fields.iter().filter(|field| self.handle_held(field.value).is_some()).map(|field| (field.span, field.value, "stored in a struct field"))),
                ExprKind::Tuple(items) => found.extend(items.iter().filter(|item| self.handle_held(**item).is_some()).map(|item| (self.design.expr_node(*item).span, *item, "stored in a tuple element"))),
                ExprKind::Array(items) => found.extend(items.iter().filter(|item| self.handle_held(**item).is_some()).map(|item| (self.design.expr_node(*item).span, *item, "stored in an array element"))),
                ExprKind::ArrayRepeat { elem, .. } if self.handle_held(*elem).is_some() => found.push((self.design.expr_node(*elem).span, *elem, "stored in an array element")),
                ExprKind::Return(Some(value)) if self.handle_held(*value).is_some() => found.push((self.design.expr_node(*value).span, *value, "returned")),
                ExprKind::Assign { lhs, .. } if self.handle_named(*lhs).is_some() => found.push((self.design.expr_node(*lhs).span, *lhs, "written through")),
                ExprKind::Assign { lhs, .. } if self.bundle_handle_root(*lhs).is_some() => written.push(*lhs),
                ExprKind::Closure { body, .. } => {
                    let mut inside = Vec::new();
                    crate::pom::BodyWalk::new(self.design).descendants_depth_first(*body, &mut inside);
                    found.extend(inside.into_iter().filter(|inner| self.handle_held(*inner).is_some()).map(|inner| (self.design.expr_node(inner).span, inner, "captured by a closure")));
                }
                _ => {}
            }
        }
        for index in 0..self.design.stmt_count() as u32 {
            let node = self.design.stmt_node(crate::pom::StmtId(index));
            if node.owner != function {
                continue;
            }
            match &node.kind {
                StmtKind::Let { init: Some(init), .. } if self.handle_held(*init).is_some() => found.push((self.design.expr_node(*init).span, *init, "stored in a `let`")),
                StmtKind::Commit { place, .. } if self.handle_named(*place).is_some() => found.push((self.design.expr_node(*place).span, *place, "written through")),
                StmtKind::Commit { place, .. } if self.bundle_handle_root(*place).is_some() => written.push(*place),
                _ => {}
            }
        }
        for place in written {
            let (span, field) = (self.design.expr_node(place).span, self.field_chain_text(place));
            let message = format!("the field `{field}` of a bundle handle is written through: every field is read-only through a handle, whatever the view (§7 rule 13)");
            self.coded(file, "E0133", span, message, "a handle written through");
        }
        for (span, expr, how) in found {
            let expr = self.handle_held(expr).unwrap_or(expr);
            let name = self.handle_named(expr).and_then(|decl| self.design.decl_data(decl).name.as_ref().map(|name| name.text.clone())).unwrap_or_default();
            let message = format!("the handle `{name}` is {how}: a handle is read, probed, passed on and has carrier methods called on it, nothing else (§3.1.5)");
            self.coded(file, "E0133", span, message, "a handle kept past its call");
        }
    }

    /// Records `expr`, of the type `ty`, when it is an `if` or a `match` whose result is a node: a choice of hardware, judged once
    /// the whole design is typed (`judge_node_choices`), when every assignment is known. It is typed as the node meanwhile.
    pub(super) fn record_node_choice(&mut self, expr: ExprId, ty: Option<&Ty>) {
        if matches!(ty, Some(Ty::Node(_) | Ty::AnyNode)) && matches!(self.design.expr_node(expr).kind, ExprKind::If { .. } | ExprKind::Match { .. }) {
            self.node_choices.push(expr);
        }
    }

    /// `E0133` for every recorded choice of a node whose value depends on the run (§1.4.7, user decision of 2026-11-24): its
    /// condition, scrutinee or a guard reads a probe, a port, a signal, a core primitive of the run, or a variable whose value
    /// does. A choice over params, constants, generation indexes and `let`s of those is made at elaboration.
    pub(super) fn judge_node_choices(&mut self) {
        for expr in std::mem::take(&mut self.node_choices) {
            let (form, read) = match self.design.expr_node(expr).kind.clone() {
                ExprKind::If { cond, .. } => ("an `if`", self.run_read(cond, &mut Vec::new())),
                ExprKind::Match { scrutinee, arms } => {
                    let guards = arms.iter().filter_map(|arm| arm.guard).find_map(|guard| self.run_read(guard, &mut Vec::new()));
                    ("a `match`", self.run_read(scrutinee, &mut Vec::new()).or(guards))
                }
                _ => continue,
            };
            let Some(read) = read else { continue };
            let (file, span) = (self.file_of_expr(expr), self.design.expr_node(expr).span);
            let message = format!(
                "{form} whose result is a node chooses it by {read}: a node is hardware, chosen at elaboration or named (§1.4.7)"
            );
            self.coded(file, "E0133", span, message, "a node chosen at run time");
            self.chosen_nodes.insert(expr);
        }
    }

    /// What `expr` reads whose value depends on the run: a probe, a port or a signal (`runtime_read`), a core primitive of the
    /// run (`now()`, `temperature()`, §14), or a variable whose value does (`run_variable`); `followed` holds the variables judged
    /// already, each once. `None` for an expression over params, constants, generation indexes and
    /// `let`s of those; the elaborator folds it, and a call that reads the run inside a function is refused there with the same
    /// code.
    pub(super) fn run_read(&self, expr: ExprId, followed: &mut Vec<DeclId>) -> Option<String> {
        if let Some(read) = self.runtime_read(expr) {
            return Some(read);
        }
        let node = self.design.expr_node(expr);
        match &node.kind {
            ExprKind::Call { callee, .. } if let Some(primitive) = self.run_primitive(*callee) => Some(format!("`{primitive}()`")),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => self.run_variable(decl, followed).map(|read| {
                    format!("the variable `{}`, whose value depends on the run: it reads {read}", self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()))
                }),
                _ => None,
            },
            kind => crate::pom::BodyWalk::new(self.design).expression_children(kind).into_iter().find_map(|child| self.run_read(child, followed)),
        }
    }

    /// The name of the core primitive the callee `callee` names (`now`), a value only the run gives (§14).
    fn run_primitive(&self, callee: ExprId) -> Option<String> {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Builtin(id)) if super::catalogue::Core::signature(id).is_some() => Some(id.name()),
            _ => None,
        }
    }

    /// What makes the value of the variable `decl` depend on the run, `None` when nothing does: a function parameter, a binding
    /// with no initializer known here (a pattern, a loop's variable), an initializer, an assigned value or a condition around an
    /// assignment that reads the run, or an assignment in a closure (coordinator ruling, finding 26: a `let` is a constant when its
    /// initializer, every value assigned to it and every condition around those fold at elaboration). A generation index is no
    /// variable of the run; a chain of `let`s deeper than 64 is taken as the run's.
    fn run_variable(&self, decl: DeclId, followed: &mut Vec<DeclId>) -> Option<String> {
        match &self.design.decl_data(decl).kind {
            DeclKind::FnParam(_) => Some("a parameter its caller binds".to_string()),
            DeclKind::Binding(_) => {
                let parent = self.design.decl_data(decl).parent;
                if parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Generation(_))) || followed.contains(&decl) {
                    return None;
                }
                if followed.len() >= 64 {
                    return Some("a chain of `let`s deeper than 64".to_string());
                }
                let body_value = parent.and_then(|parent| match &self.design.decl_data(parent).kind {
                    DeclKind::Let(data) => Some(data.value),
                    _ => None,
                });
                let Some(init) = self.let_values.get(&decl).copied().or(body_value) else {
                    return Some("a value only a pattern or a loop binds".to_string());
                };
                // judged once: a variable met again is being judged or was found to depend on nothing of the run (one that does
                // ends the search), so following it again adds nothing and the judgement ends
                followed.push(decl);
                self.run_read(init, followed).or_else(|| self.places.writes_of(decl).iter().find_map(|write| self.write_reads_the_run(decl, *write, followed)))
            }
            _ => None,
        }
    }

    /// What makes the assignment `write` of `decl` depend on the run: an assignment in a closure, the value it writes, or a condition
    /// around it inside the body that declares `decl` (an `if`, a `match`, a loop's condition or range, and every condition inside
    /// a loop around it, which may end the loop early).
    fn write_reads_the_run(&self, decl: DeclId, write: super::assign::Write, followed: &mut Vec<DeclId>) -> Option<String> {
        if write.in_closure {
            return Some("an assignment inside a closure".to_string());
        }
        if let Some(read) = self.run_read(write.value, followed) {
            return Some(read);
        }
        let at = self.design.expr_node(write.assignment).clone();
        let declared = self.design.decl_data(decl).span;
        let around = |span: Span| span.start <= at.span.start && at.span.end <= span.end && !(span.start <= declared.start && declared.end <= span.end);
        let mut conditions = Vec::new();
        for index in 0..self.design.expr_count() as u32 {
            let node = self.design.expr_node(ExprId(index));
            if node.owner != at.owner || !around(node.span) {
                continue;
            }
            match &node.kind {
                ExprKind::While { .. } | ExprKind::Loop { .. } | ExprKind::For { .. } => conditions.extend(self.conditions_within(at.owner, node.span)),
                _ => conditions.extend(self.conditions_of(&node.kind)),
            }
        }
        conditions.into_iter().find_map(|condition| self.run_read(condition, followed)).map(|read| format!("{read} in a condition around an assignment"))
    }

    /// The conditions an expression of kind `kind` decides by: an `if`'s, a `match`'s scrutinee and guards, a `while`'s, a `for`'s
    /// range.
    fn conditions_of(&self, kind: &ExprKind) -> Vec<ExprId> {
        match kind {
            ExprKind::If { cond, .. } | ExprKind::While { cond, .. } => vec![*cond],
            ExprKind::For { iter, .. } => vec![*iter],
            ExprKind::Match { scrutinee, arms } => std::iter::once(*scrutinee).chain(arms.iter().filter_map(|arm| arm.guard)).collect(),
            _ => Vec::new(),
        }
    }

    /// Every condition of an expression of `owner` written within `span`.
    fn conditions_within(&self, owner: DeclId, span: Span) -> Vec<ExprId> {
        (0..self.design.expr_count() as u32)
            .map(ExprId)
            .filter(|expr| {
                let node = self.design.expr_node(*expr);
                node.owner == owner && span.start <= node.span.start && node.span.end <= span.end
            })
            .flat_map(|expr| self.conditions_of(&self.design.expr_node(expr).kind))
            .collect()
    }

    /// Whether the node operand `operand` was refused already: a node chosen by an expression (through parentheses), or a `let`
    /// whose value was refused.
    pub(super) fn operand_refused(&self, operand: ExprId) -> bool {
        match &self.design.expr_node(operand).kind {
            ExprKind::Paren(inner) => self.operand_refused(*inner),
            _ => self.chosen_nodes.contains(&operand) || self.reads_poisoned(operand),
        }
    }

    /// `E0133` for a function whose result type is a handle (a node or a carrier of a nature).
    pub(super) fn check_handle_result(&mut self, function: DeclId, ret: Option<&Ty>) {
        let Some(ret) = ret.filter(|ty| Self::is_handle_type(ty)) else { return };
        let DeclKind::Fn(data) = &self.design.decl_data(function).kind else { return };
        let span = data.ret.map_or(self.design.decl_data(function).span, |ty| self.design.type_node(ty).span);
        let file = self.design.decl_data(function).file;
        let message = format!("a function returns `{}`, a handle: a handle is never returned (§3.1.5)", ret.name(self.design));
        self.coded(file, "E0133", span, message, "a handle returned");
    }
}
