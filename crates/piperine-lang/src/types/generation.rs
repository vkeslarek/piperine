//! Generation in a module body (generics GN-17, part III §8.2.7, §8.2.23, §8.2.24): a `for` or an `if` over module items,
//! run at elaboration. The checker types the range or the condition as a const expression (`E0340` when it reads a
//! carrier, a node or a probe), refuses a param inside generation (`E0322`) and gives each item declared in a `for` the
//! array type of its element, one level per generation `for` around it, outermost first (§8.2.23). A reader inside the
//! `for` sees the current element: the tables hold the whole array, a read peels the levels its own position is inside.

use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, GenerationData, PatKind, Resolution};

use super::check::{Checker, Context};
use super::{ConstTerm, Length, Ty};

impl Checker<'_> {
    /// The items `items` of a body with the items written inside their generations in place of each generation, nested ones
    /// included, in source order.
    pub(super) fn generated_items(&self, items: &[DeclId]) -> Vec<DeclId> {
        let mut found = Vec::new();
        for item in items.iter().copied() {
            match &self.state.design.decl_data(item).kind {
                DeclKind::Generation(_) => found.extend(self.generated_items(&self.state.design.decl_data(item).children)),
                _ => found.push(item),
            }
        }
        found
    }

    /// The generation `for`s around the declaration `decl`, outermost first.
    pub(super) fn generation_loops(&self, decl: DeclId) -> Vec<DeclId> {
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

    /// The number of iterations of the generation `for` `generation`: a length when both bounds fold, else the range itself,
    /// which the elaborator counts over the instance's params.
    fn loop_length(&mut self, generation: DeclId) -> Length {
        let DeclKind::Generation(GenerationData::For { iter, .. }) = &self.state.design.decl_data(generation).kind else { return Length::Fixed(0) };
        if let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = &self.state.design.expr_node(*iter).kind
            && let (Some(low), Some(high)) = (self.expressions().fold_integer(*lo, 16), self.expressions().fold_integer(*hi, 16))
        {
            return Length::Fixed(u64::try_from(high.saturating_sub(low) + u128::from(*inclusive)).unwrap_or(u64::MAX));
        }
        Length::Generic(ConstTerm::Expr(*iter))
    }

    /// `element` as the array a generation `for` around `decl` makes of it, nested once for each (§8.2.23).
    pub(super) fn generated_type(&mut self, decl: DeclId, element: Ty) -> Ty {
        self.generation_loops(decl).into_iter().rev().fold(element, |inner, generation| Ty::Array(Box::new(inner), self.loop_length(generation)))
    }

    /// The generation `for`s of `decl` the reader `expr` is not inside: the levels of its array the reader sees whole.
    fn outside_loops(&self, decl: DeclId, expr: ExprId) -> Vec<DeclId> {
        let inside = self.generation_loops_at(self.state.design.expr_node(expr).owner);
        self.generation_loops(decl).into_iter().filter(|generation| !inside.contains(generation)).collect()
    }

    /// The generation `for`s a declaration `owner`, or a position inside it, is inside.
    fn generation_loops_at(&self, owner: DeclId) -> Vec<DeclId> {
        let mut loops = Vec::new();
        let mut at = Some(owner);
        while let Some(current) = at {
            let data = self.state.design.decl_data(current);
            if matches!(data.kind, DeclKind::Generation(GenerationData::For { .. })) {
                loops.push(current);
            }
            at = data.parent;
        }
        loops
    }

    /// The value `read` a reader `expr` gets from the generated carrier or node `decl`: the element, wrapped in the levels
    /// of the generation `for`s the reader is not inside.
    pub(super) fn generated_read(&mut self, expr: ExprId, decl: DeclId, read: Ty) -> Ty {
        self.outside_loops(decl, expr).into_iter().rev().fold(read, |inner, generation| Ty::Array(Box::new(inner), self.loop_length(generation)))
    }

    /// The type a reader `expr` gets from the generated local `decl` (a `let`, an instance), whose table entry is the
    /// whole array: the levels of the generation `for`s the reader is inside are peeled.
    pub(super) fn generated_local(&self, expr: ExprId, decl: DeclId, whole: Ty) -> Ty {
        let around = self.generation_loops(decl);
        let inside = self.generation_loops_at(self.state.design.expr_node(expr).owner);
        let mut ty = whole;
        for generation in around {
            if !inside.contains(&generation) {
                break;
            }
            ty = match ty {
                Ty::Array(element, _) => *element,
                other => other,
            };
        }
        ty
    }

    /// The generation `decl` of a module body in `phase`: its header in the declarations, then its items as the module's own.
    pub(super) fn generation(&mut self, decl: DeclId, declarations: bool, mut item: impl FnMut(&mut Self, DeclId)) {
        let data = self.state.design.decl_data(decl).clone();
        let DeclKind::Generation(generation) = &data.kind else { return };
        if declarations {
            self.generation_header(decl, generation);
        }
        for child in data.children.iter().copied() {
            match self.state.design.decl_data(child).kind {
                DeclKind::Param(_) => {
                    if declarations {
                        let child_data = self.state.design.decl_data(child).clone();
                        let name = child_data.name.as_ref().map_or("", |n| n.text.as_str());
                        let message = format!("the param `{name}` is declared inside generation: a param is declared at the top level of the body (§8.1.20)");
                        self.coded(child_data.file, "E0322", child_data.span, message, "param inside generation");
                    }
                }
                _ => item(self, child),
            }
        }
    }

    /// The range of a generation `for`, or the condition of a generation `if`, as a const expression, and the pattern of the
    /// `for` bound to the element of the range.
    fn generation_header(&mut self, decl: DeclId, generation: &GenerationData) {
        let file = self.state.design.decl_data(decl).file;
        match generation {
            GenerationData::For { pat, iter, .. } => {
                let element = self.const_position(*iter, "a generation range").then(|| self.within(Context::Const, |this| this.generation_element(*iter))).flatten();
                match element {
                    Some(element) => {
                        self.irrefutable_pattern(file, *pat, &element, "a generation `for`");
                    }
                    None => self.poison_pattern(*pat),
                }
            }
            GenerationData::If { cond, .. } => {
                if self.const_position(*cond, "a generation condition") {
                    self.within(Context::Const, |this| this.expressions().check(*cond, &Ty::Bool));
                }
            }
        }
    }

    /// The element type of the range `iter` of a generation `for`.
    fn generation_element(&mut self, iter: ExprId) -> Option<Ty> {
        if matches!(self.state.design.expr_node(iter).kind, ExprKind::Range { lo: Some(_), hi: Some(_), .. }) { self.expressions().range_element(iter) } else { self.expressions().iteration_element(iter) }
    }

    /// Whether `expr` reads no carrier, node or probe; `E0340` naming `what` and the read when it does.
    fn const_position(&mut self, expr: ExprId, what: &str) -> bool {
        let Some(read) = self.runtime_read(expr) else { return true };
        let (file, span) = (self.file_of_expr(expr), self.state.design.expr_node(expr).span);
        self.coded(file, "E0340", span, format!("{what} reads {read}: it is a const expression over params, constants and const generics (§8.2.7)"), "not a const expression");
        false
    }

    /// What `expr` reads that only exists over time: a port or a signal by name, or a probe.
    pub(super) fn runtime_read(&self, expr: ExprId) -> Option<String> {
        let node = self.state.design.expr_node(expr);
        match &node.kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if matches!(self.state.design.decl_data(decl).kind, DeclKind::Port(_) | DeclKind::Signal(_)) => {
                    Some(format!("`{}`", self.state.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str())))
                }
                _ => None,
            },
            ExprKind::Call { class: crate::pom::CallClass::Probe, .. } => Some("a probe".to_string()),
            kind => crate::pom::BodyWalk::new(self.state.design).expression_children(kind).into_iter().find_map(|child| self.runtime_read(child)),
        }
    }

    /// The names of the pattern `pat` of a generation `for` whose range did not type: poisoned, so the items that read them add
    /// nothing to the one diagnostic.
    fn poison_pattern(&mut self, pat: crate::pom::PatId) {
        match self.state.design.pat_node(pat).kind.clone() {
            PatKind::Ident { binding, .. } => {
                self.state.values.poison(binding);
            }
            PatKind::Tuple(items) => items.into_iter().for_each(|item| self.poison_pattern(item)),
            _ => {}
        }
    }
}
