//! Read-only questions of the design the lowerings of every concern ask: the expressions of a body, the lets a condition reads, what a frame binds and what a generation made.

use crate::elab::consts::{Bindings};
use crate::pom::{self, BlockNode, CallClass, DeclId, DeclKind, ExprId, ExprKind, GenerationData, PatKind, Resolution, StmtKind};
use crate::types::{Callee};
use super::Elaboration;

impl<'d, 't> Elaboration<'d, 't> {
    /// `expr` without the parentheses around it.
    pub(in crate::elab) fn peel_parens(&self, mut expr: ExprId) -> ExprId {
        while let ExprKind::Paren(inner) = &self.state.design.expr_node(expr).kind {
            expr = *inner;
        }
        expr
    }

    /// Whether `function` takes a handle: a parameter the checker found to be one.
    pub(in crate::elab) fn takes_handle(&self, function: DeclId) -> bool {
        self.state.design.decl_data(function).children.iter().any(|child| matches!(self.state.design.decl_data(*child).kind, DeclKind::FnParam(_)) && self.state.types.handle(*child))
    }

    /// What the assignments of the `let` `local` read, in the block that declares it: each value assigned and each condition around
    /// an assignment (an `if`, a `match`, a loop's condition or range, and every condition inside a loop around it), whose params
    /// a choice by `local` makes structural as it does those of the initializer (`structural.rs`).
    pub(in crate::elab) fn assignment_reads(&self, local: DeclId) -> Vec<ExprId> {
        let Some((stmts, from)) = self.block_declaring(local) else { return Vec::new() };
        let mut inside = Vec::new();
        for stmt in &stmts[from..] {
            for child in crate::pom::BodyWalk::new(self.state.design).statement_children(*stmt) {
                crate::pom::BodyWalk::new(self.state.design).descendants_depth_first(child, &mut inside);
            }
        }
        let within = |outer: crate::msl::Span, inner: crate::msl::Span| outer.start <= inner.start && inner.end <= outer.end;
        let mut reads = Vec::new();
        for assignment in inside.iter().copied() {
            let ExprKind::Assign { lhs, rhs, .. } = &self.state.design.expr_node(assignment).kind else { continue };
            if !self.read_bindings(*lhs).contains(&local) {
                continue;
            }
            reads.push(*rhs);
            let at = self.state.design.expr_node(assignment).span;
            for around in inside.iter().copied().filter(|expr| within(self.state.design.expr_node(*expr).span, at)) {
                match &self.state.design.expr_node(around).kind {
                    ExprKind::While { .. } | ExprKind::Loop { .. } | ExprKind::For { .. } => {
                        let span = self.state.design.expr_node(around).span;
                        reads.extend(inside.iter().filter(|expr| within(span, self.state.design.expr_node(**expr).span)).flat_map(|expr| Self::conditions_of(&self.state.design.expr_node(*expr).kind)));
                    }
                    kind => reads.extend(Self::conditions_of(kind)),
                }
            }
        }
        reads
    }

    /// The bindings `expr` names, anywhere in it.
    pub(in crate::elab) fn read_bindings(&self, expr: ExprId) -> Vec<DeclId> {
        let node = self.state.design.expr_node(expr);
        match &node.kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if matches!(self.state.design.decl_data(decl).kind, crate::pom::DeclKind::Binding(_)) => vec![decl],
                _ => Vec::new(),
            },
            kind => crate::pom::BodyWalk::new(self.state.design).expression_children(kind).into_iter().flat_map(|child| self.read_bindings(child)).collect(),
        }
    }

    /// The initializer of the `let` that binds `local` with a name, in a statement or a module's body.
    pub(in crate::elab) fn let_initializer(&self, local: DeclId) -> Option<ExprId> {
        if let Some(value) = self.body_let_value(local) {
            return Some(value);
        }
        (0..self.state.design.stmt_count()).find_map(|index| match &self.state.design.stmt_node(crate::pom::StmtId(index as u32)).kind {
            StmtKind::Let { pat, init: Some(init), .. } if matches!(&self.state.design.pat_node(*pat).kind, PatKind::Ident { binding, sub: None, .. } if *binding == local) => Some(*init),
            _ => None,
        })
    }

    /// The quantity the call of `callee` reads or contributes: a nature's access alias (`i`, `v`)
    /// or the built-in `flow` and `potential`; `None` for anything else.
    pub(in crate::elab) fn quantity_role(&self, callee: ExprId) -> Option<pom::QuantityRole> {
        let ExprKind::Path(path) = &self.state.design.expr_node(callee).kind else { return None };
        match self.state.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
            Resolution::Decl(q) => match &self.state.design.decl_data(q).kind {
                DeclKind::Quantity(data) => Some(data.role),
                _ => None,
            },
            Resolution::Builtin(id) if id.info().name == "flow" => Some(pom::QuantityRole::Flow),
            Resolution::Builtin(id) if id.info().name == "potential" => Some(pom::QuantityRole::Potential),
            // an access the resolver left ambiguous takes the role the checker read from the node (`v(y)`, `let y = x;`)
            Resolution::Deferred(_) => self.state.types.access_role(callee),
            _ => None,
        }
    }

    /// Every expression owned by `module` or by one of its behaviors.
    pub(in crate::elab) fn module_exprs(&self, module: DeclId) -> Vec<ExprId> {
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return Vec::new() };
        let behaviors = &data.behaviors;
        let inside = |mut owner: DeclId| loop {
            if owner == module || behaviors.contains(&owner) {
                return true;
            }
            match self.state.design.decl_data(owner).parent {
                Some(parent) => owner = parent,
                None => return false,
            }
        };
        (0..self.state.design.expr_count() as u32).map(ExprId).filter(|e| inside(self.state.design.expr_node(*e).owner)).collect()
    }

    /// Every expression of `body`, nested ones included.
    pub(in crate::elab) fn body_exprs(&self, body: &BlockNode) -> Vec<ExprId> {
        let mut inside = Vec::new();
        for expr in crate::pom::BodyWalk::new(self.state.design).block_expressions(body) {
            crate::pom::BodyWalk::new(self.state.design).descendants_depth_first(expr, &mut inside);
        }
        inside
    }

    /// The statements of the block that declares the `let` `local` with a name, and the index of its `let` among them: the body
    /// of a function, an `analog` block or a reaction, or a block of an expression of the same owner.
    pub(in crate::elab) fn block_declaring(&self, local: DeclId) -> Option<(Vec<crate::pom::StmtId>, usize)> {
        let declares = |stmt: &crate::pom::StmtId| matches!(&self.state.design.stmt_node(*stmt).kind, StmtKind::Let { pat, .. } if matches!(&self.state.design.pat_node(*pat).kind, PatKind::Ident { binding, sub: None, .. } if *binding == local));
        let statement = (0..self.state.design.stmt_count() as u32).map(crate::pom::StmtId).find(|stmt| declares(stmt))?;
        let owner = self.state.design.stmt_node(statement).owner;
        let mut blocks: Vec<Vec<crate::pom::StmtId>> = match &self.state.design.decl_data(owner).kind {
            crate::pom::DeclKind::Fn(data) => data.body.iter().map(|body| body.stmts.clone()).collect(),
            crate::pom::DeclKind::Behavior(data) => vec![data.body.clone()],
            crate::pom::DeclKind::Reaction(data) => vec![data.body.stmts.clone()],
            _ => Vec::new(),
        };
        for index in 0..self.state.design.expr_count() as u32 {
            let node = self.state.design.expr_node(ExprId(index));
            if node.owner != owner {
                continue;
            }
            match &node.kind {
                ExprKind::Block(block) | ExprKind::If { then: block, .. } | ExprKind::While { body: block, .. } | ExprKind::Loop { body: block } | ExprKind::For { body: block, .. } => blocks.push(block.stmts.clone()),
                _ => {}
            }
        }
        blocks.into_iter().find_map(|stmts| stmts.iter().position(declares).map(|from| (stmts, from)))
    }

    /// The conditions an expression of kind `kind` decides by: an `if`'s, a `match`'s scrutinee and guards, a `while`'s, a `for`'s
    /// range.
    pub(in crate::elab) fn conditions_of(kind: &ExprKind) -> Vec<ExprId> {
        match kind {
            ExprKind::If { cond, .. } | ExprKind::While { cond, .. } => vec![*cond],
            ExprKind::For { iter, .. } => vec![*iter],
            ExprKind::Match { scrutinee, arms } => std::iter::once(*scrutinee).chain(arms.iter().filter_map(|arm| arm.guard)).collect(),
            _ => Vec::new(),
        }
    }

    /// The `impl Resolve` of `nature` the first drive of the resolved carrier `decl` picked. A carrier of a nature parameter
    /// bound by `Resolve<X, V>` picked the bound (the checker saw no concrete nature): the instance's nature answers with its
    /// `impl` whose drive type is `X`. `None` when no drive picked one.
    pub(in crate::elab) fn concrete_implementation(&self, decl: DeclId, nature: DeclId) -> Option<DeclId> {
        let picked = self.state.types.carrier_drive_impls(decl).first().copied()?;
        if !matches!(self.state.design.decl_data(picked).kind, DeclKind::GenericParam(_)) {
            return Some(picked);
        }
        let driven = &self.state.types.resolve_impls(picked).first()?.driven;
        self.state.types.resolve_impls(nature).iter().find(|candidate| candidate.driven == *driven).map(|candidate| candidate.implementation)
    }

    /// The values the params a frame's initial values may read take: the body's key params for the body's
    /// own module (frame 0, `body_keys`), the constants the placement bound for a hoisted module.
    pub(in crate::elab) fn frame_bindings(&self, frame: usize) -> Bindings {
        if frame == 0 {
            return self.state.structure.body_keys.clone();
        }
        let mut bindings = Bindings::new();
        for (param, value) in &self.state.structure.frames[frame].params {
            self.bind_param(&mut bindings, *param, *value);
        }
        bindings
    }

    /// The ordinals of the iteration `ordinals` the code being lowered sits in, for the generation `for`s around `decl` it is
    /// inside (a prefix of them, outermost first).
    pub(in crate::elab) fn implicit_ordinals(&self, decl: DeclId, ordinals: &[(DeclId, u32)]) -> Vec<u64> {
        self.generation_loops(decl).iter().map_while(|generation| ordinals.iter().find(|(held, _)| held == generation).map(|(_, ordinal)| u64::from(*ordinal))).collect()
    }

    /// The module `call` places when that module is hoisted: one with a result or with a value input. A call through a param of
    /// a generic type bounded by `Fn(..)` (`s(5.0)` with `s: C`) places the module of the template the param holds in the frame
    /// being lowered.
    pub(in crate::elab) fn hoisted_callee(&self, call: ExprId) -> Option<DeclId> {
        self.placed_module(call, self.state.consts.frame_params()).filter(|m| self.is_hoisted(*m))
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier
    /// pattern, the matching component of a tuple pattern over a tuple expression.
    pub(in crate::elab) fn body_let_value(&self, binding: DeclId) -> Option<ExprId> {
        crate::elab::consts::ConstEval::body_let_value(self.state.design, binding)
    }

    /// The generation `for`s around `decl`, outermost first.
    pub(in crate::elab) fn generation_loops(&self, decl: DeclId) -> Vec<DeclId> {
        let mut loops = Vec::new();
        let mut at = self.state.design.decl_data(decl).parent;
        while let Some(current) = at {
            let data = self.state.design.decl_data(current);
            if matches!(data.kind, DeclKind::Generation(GenerationData::For { .. })) {
                loops.push(current);
            }
            if matches!(data.kind, DeclKind::Module(_)) {
                break;
            }
            at = data.parent;
        }
        loops.reverse();
        loops
    }

    /// The module the placement `call` places: the one the checker names, else (a call of a callable param, `cell(a)` with
    /// `cell: C` and `C: Fn(..)`) the module of the template the param holds in `keys`. `None` for a call that places nothing, or a
    /// param no template is bound to.
    pub(in crate::elab) fn placed_module(&self, call: ExprId, keys: &Bindings) -> Option<DeclId> {
        if let Some(Callee::Module(module)) = self.state.types.callee(call) {
            return Some(module);
        }
        let ExprKind::Call { class: CallClass::Instance { module: None, via: Some(via) }, .. } = &self.state.design.expr_node(call).kind else { return None };
        if !self.is_module_param(*via) {
            return None;
        }
        keys.get(*via).and_then(|number| self.state.consts.templates().get(number)).map(|template| template.module)
    }

    /// Whether a placement of `module` is hoisted into the body that places it (Appendix C.1.1): it
    /// declares a result or takes a value input. Every other module keeps its own body.
    pub(in crate::elab) fn is_hoisted(&self, module: DeclId) -> bool {
        let data = self.state.design.decl_data(module);
        matches!(&data.kind, DeclKind::Module(m) if m.ret.is_some()) || data.children.iter().any(|c| self.is_value_input(*c))
    }

    /// Whether `param` is declared with a module type, or with a type parameter bounded by `Fn(..)`: its value is a template.
    pub(in crate::elab) fn is_module_param(&self, param: DeclId) -> bool {
        self.template_array_length(param).is_none()
            && matches!(&self.state.design.decl_data(param).kind, DeclKind::Param(p) if matches!(self.state.design.type_resolution(p.ty), Resolution::Decl(m) if matches!(self.state.design.decl_data(m).kind, DeclKind::Module(_)) || crate::types::GenericSignature::is_callable(self.state.design, m)))
    }

    /// Whether `decl` is a port of a value type: a value input.
    pub(in crate::elab) fn is_value_input(&self, decl: DeclId) -> bool {
        crate::elab::structure::structural::ParamReads::new(self.state.design, self.state.types).is_value_input(decl)
    }

    /// The length of the array of templates `param` is declared as, `None` for a param of any other type.
    pub(in crate::elab) fn template_array_length(&self, param: DeclId) -> Option<usize> {
        let DeclKind::Param(p) = &self.state.design.decl_data(param).kind else { return None };
        match self.typed_written(p.ty)? {
            crate::types::Ty::Array(element, crate::types::Length::Fixed(length)) if matches!(*element, crate::types::Ty::Module(_)) => usize::try_from(length).ok(),
            _ => None,
        }
    }
}
