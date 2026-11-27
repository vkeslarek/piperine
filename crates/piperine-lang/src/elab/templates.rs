//! Template values: a param of module type holds a template `Name { params }` (§8.1.1, §8.4.4;
//! PI-04). The param is structural, so its value is a constant of the elaboration: the template
//! is folded to the module it names and the constants of its param block, interned, and the
//! param holds the template's number as a `Constant::Int`. Two equal templates are one number, so
//! they select one body variant (`structural.rs`); a placement through the param (`cell(c)`)
//! places the template's module with the template's values (`children.rs`).

use piperine_ir::{AggregateData, Constant, ValueType};

use super::consts::{Bindings, ConstFault, ConstTree};
use super::elaborator::Elaboration;
use super::instances::ConcreteValue;
use crate::refusal::RefusedForm;
use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtKind, StructClass};
use crate::types::Callee;

/// A template folded: the module it names and the value each param of its block gives.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::elab) struct TemplateValue {
    /// The module the template places.
    pub(super) module: DeclId,
    /// The generic arguments of the module, in the order of its parameters (§8.4.3); empty for a module that is not generic.
    pub(super) arguments: Vec<ConcreteValue>,
    /// The params the block binds: the param, its folded value and the expression that gave it. The value is held taken apart
    /// (`ConstTree`), so it belongs to no arena (the evaluator's and the IR's composites are numbered separately) and two
    /// templates with equal values are equal (§8.4: two equal templates are one value).
    pub(super) fields: Vec<(DeclId, ConstTree, ExprId)>,
}

/// What holds the elements of an array of templates.
enum TemplateItems {
    /// The expressions of the elements.
    Written(Vec<ExprId>),
    /// The param declared as the array: its elements are in the bindings.
    Param(DeclId),
}

/// The templates interned so far, by number.
#[derive(Debug, Default)]
pub(in crate::elab) struct Templates {
    values: Vec<TemplateValue>,
}

impl Templates {
    /// The number of `value`: the one it was given before, else the next.
    pub(in crate::elab) fn intern(&mut self, value: TemplateValue) -> Constant {
        let number = match self.values.iter().position(|known| known.module == value.module && known.arguments == value.arguments && known.fields.iter().map(|f| (f.0, &f.1)).eq(value.fields.iter().map(|f| (f.0, &f.1)))) {
            Some(number) => number,
            None => {
                self.values.push(value);
                self.values.len() - 1
            }
        };
        Constant::Int(number as i128)
    }

    /// The template `number` names, `None` for a number that was never handed out.
    pub(super) fn get(&self, number: Constant) -> Option<&TemplateValue> {
        match number {
            Constant::Int(n) => usize::try_from(n).ok().and_then(|n| self.values.get(n)),
            _ => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// The constants the params of `template` are bound to, each added to the IR's arena: what a placement through the template
    /// binds (`children.rs`, `hoist.rs`). A value the placement's `Option` param takes from a bare `T` is `Some` of it (§3.2.3).
    pub(super) fn template_constants(&mut self, template: &TemplateValue) -> Result<Vec<(DeclId, Constant, ExprId)>, ConstFault> {
        let mut constants = Vec::with_capacity(template.fields.len());
        for (param, tree, expr) in &template.fields {
            let value = self.template_field_constant(*expr, tree)?;
            constants.push((*param, value, *expr));
        }
        Ok(constants)
    }

    /// The tree `tree`, the value of the expression `expr` of a template's block, as a constant of the IR's arena. The value of a
    /// bare `T` bound to an `Option<T>` param is held as its `Some` (`fold_param_value`, `template_literal`), so it has that type.
    fn template_field_constant(&mut self, expr: ExprId, tree: &ConstTree) -> Result<Constant, ConstFault> {
        // an array of templates: the aggregate of the numbers of its elements (`fold_template_array`)
        if let Some(crate::types::Ty::Array(element, _)) = self.typed(expr)
            && matches!(*element, crate::types::Ty::Module(_))
        {
            return self.import_template_numbers(expr, tree);
        }
        let held = self.typed(expr).and_then(|ty| self.stored_type(&ty));
        let ty = match held {
            Some(held) if self.types.implicit_some(expr) => Some(ValueType::Option(Box::new(held))),
            other => other,
        };
        match (ty, tree) {
            (Some(ty), _) => self.import_tree(expr, tree, &ty),
            (None, ConstTree::Scalar(value)) => Ok(*value),
            (None, ConstTree::Composite { .. }) => Err(ConstFault::refused(expr, RefusedForm::Text("a composite value of a type that has no value type".to_string()))),
        }
    }

    /// The array of template numbers `tree` holds, as the aggregate `fold_template_array` makes.
    fn import_template_numbers(&mut self, expr: ExprId, tree: &ConstTree) -> Result<Constant, ConstFault> {
        let ConstTree::Composite { parts, .. } = tree else { return Err(ConstFault::refused(expr, RefusedForm::Text("an array of templates that is not an array".to_string()))) };
        let mut numbers = Vec::with_capacity(parts.len());
        for part in parts {
            let ConstTree::Scalar(number) = part else { return Err(ConstFault::refused(expr, RefusedForm::Text("an array of templates inside an array of templates".to_string()))) };
            numbers.push(*number);
        }
        let ty = ValueType::Array(Box::new(ValueType::UInt(32)), numbers.len() as u64);
        Ok(Constant::Aggregate(self.ir.aggregate(AggregateData { ty, variant: None, parts: numbers })))
    }

    /// The template at `index` of the array of templates `array`, folded (`Constant::Int` of its number).
    fn fold_template_element(&mut self, expr: ExprId, array: ExprId, index: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let unsupported = |what: &str| ConstFault::refused(expr, RefusedForm::Text(what.to_string()));
        let items = self.template_items(array, 0).ok_or_else(|| unsupported("an array of templates that is not written out, held by a `let`, a `const` or a `param`"))?;
        let Constant::Int(at) = self.consts.value(index, bindings)? else { return Err(unsupported("an index that is not an integer")) };
        let length = match &items {
            TemplateItems::Written(items) => items.len(),
            TemplateItems::Param(param) => self.template_array_length(*param).unwrap_or(0),
        };
        let at = usize::try_from(at).ok().filter(|at| *at < length).ok_or_else(|| ConstFault::Panic { expr, what: format!("the element {at} of an array of {length} templates (§1 rule 11)") })?;
        match items {
            TemplateItems::Written(items) => self.fold_param_value(items[at], bindings),
            TemplateItems::Param(param) => self.template_numbers(expr, param, bindings).map(|numbers| numbers[at]),
        }
    }

    /// The number of the template each element of the array param `param` holds, as `bindings` binds it.
    fn template_numbers(&self, expr: ExprId, param: DeclId, bindings: &Bindings) -> Result<Vec<Constant>, ConstFault> {
        let Some(held) = bindings.get(param) else { return Err(ConstFault::UnboundParam { expr, param }) };
        let Constant::Aggregate(id) = held else { return Err(ConstFault::refused(expr, RefusedForm::Text("an array of templates that is not an array".to_string()))) };
        Ok(self.ir.aggregate_at(id).parts.clone())
    }

    /// The array of templates `expr` folded: the aggregate of the numbers of its templates, a param's value as it is bound.
    fn fold_template_array(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        // a choice of arrays folds its condition, then the array of the arm it takes (§8.4.6)
        if let ExprKind::If { cond, then, else_ } = self.design.expr_node(self.peel_parens(expr)).kind.clone() {
            let Constant::Bool(taken) = self.consts.value(cond, bindings)? else { return Err(ConstFault::refused(expr, RefusedForm::Text("a condition that is not a `bool`".to_string()))) };
            let arm = if taken { then.stmts.last().and_then(|last| match &self.design.stmt_node(*last).kind { StmtKind::Expr { expr, semi: false } => Some(*expr), _ => None }) } else { else_ };
            let arm = arm.ok_or_else(|| ConstFault::refused(expr, RefusedForm::Text("a choice of arrays of templates with no array in the arm it takes".to_string())))?;
            return self.fold_template_array(arm, bindings);
        }
        let items = self.template_items(expr, 0).ok_or_else(|| ConstFault::refused(expr, RefusedForm::Text("an array of templates that is not written out, held by a `let`, a `const` or a `param`".to_string())))?;
        let numbers = match items {
            TemplateItems::Param(param) => return bindings.get(param).ok_or(ConstFault::UnboundParam { expr, param }),
            TemplateItems::Written(items) => items.into_iter().map(|item| self.fold_param_value(item, bindings)).collect::<Result<Vec<_>, _>>()?,
        };
        let ty = ValueType::Array(Box::new(ValueType::UInt(32)), numbers.len() as u64);
        Ok(Constant::Aggregate(self.ir.aggregate(AggregateData { ty, variant: None, parts: numbers })))
    }

    /// What holds the elements of the array of templates `array`: an array written out, or held by a `let` or a `const`, else a `param`.
    fn template_items(&mut self, array: ExprId, depth: usize) -> Option<TemplateItems> {
        if depth > 64 {
            return None;
        }
        let array = self.peel_parens(array);
        match self.design.expr_node(array).kind.clone() {
            ExprKind::Array(items) => Some(TemplateItems::Written(items)),
            // `[T { .. }; N]`: `N` equal templates; the length is a const expression the checker leaves untyped (§3.8.3), so it folds
            // as an array length does (over the const arguments and the frame's params)
            ExprKind::ArrayRepeat { elem, len } => {
                let count = self.consts.integer(len)?;
                Some(TemplateItems::Written(vec![elem; usize::try_from(count).ok()?]))
            }
            ExprKind::Path(path) => match self.design.path_node(path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => match self.design.decl_data(decl).kind.clone() {
                    DeclKind::Binding(_) => {
                        let value = self.body_let_value(decl)?;
                        self.template_items(value, depth + 1)
                    }
                    DeclKind::Const(data) => self.template_items(data.value?, depth + 1),
                    DeclKind::Param(_) => self.template_array_length(decl).map(|_| TemplateItems::Param(decl)),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    /// The length of the array of templates `param` is declared as, `None` for a param of any other type.
    pub(super) fn template_array_length(&self, param: DeclId) -> Option<usize> {
        let DeclKind::Param(p) = &self.design.decl_data(param).kind else { return None };
        match self.typed_written(p.ty)? {
            crate::types::Ty::Array(element, crate::types::Length::Fixed(length)) if matches!(*element, crate::types::Ty::Module(_)) => usize::try_from(length).ok(),
            _ => None,
        }
    }

    /// Whether `param` holds a template or an array of them: it fixes structure (§8.4.6).
    pub(super) fn is_template_param(&self, param: DeclId) -> bool {
        self.is_module_param(param) || self.template_array_length(param).is_some()
    }

    /// Whether `param` is declared with a module type, or with a type parameter bounded by `Fn(..)`: its value is a template.
    pub(super) fn is_module_param(&self, param: DeclId) -> bool {
        self.template_array_length(param).is_none()
            && matches!(&self.design.decl_data(param).kind, DeclKind::Param(p) if matches!(self.design.type_resolution(p.ty), Resolution::Decl(m) if matches!(self.design.decl_data(m).kind, DeclKind::Module(_)) || crate::types::GenericSignature::is_callable(self.design, m)))
    }

    /// The module the placement `call` places: the one the checker names, else (a call of a callable param, `cell(a)` with
    /// `cell: C` and `C: Fn(..)`) the module of the template the param holds in `keys`. `None` for a call that places nothing, or a
    /// param no template is bound to.
    pub(super) fn placed_module(&self, call: ExprId, keys: &Bindings) -> Option<DeclId> {
        if let Some(Callee::Module(module)) = self.types.callee(call) {
            return Some(module);
        }
        let ExprKind::Call { class: CallClass::Instance { module: None, via: Some(via) }, .. } = &self.design.expr_node(call).kind else { return None };
        if !self.is_module_param(*via) {
            return None;
        }
        keys.get(*via).and_then(|number| self.consts.templates().get(number)).map(|template| template.module)
    }

    /// The value `expr` gives a param: a template folded and interned (`Constant::Int` of its
    /// number), any other expression as `ConstEval::value` folds it over `bindings`.
    pub(super) fn fold_param_value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        // one element of an array of templates: the array's items, the index folded over `bindings` (§8.4.1, GAPS L88)
        if let ExprKind::Index { expr: array, index } = self.design.expr_node(expr).kind.clone()
            && self.typed(expr).is_some_and(|ty| matches!(ty, crate::types::Ty::Module(_)))
        {
            return self.fold_template_element(expr, array, index, bindings);
        }
        let ExprKind::Struct { fields, class: StructClass::Template(module), .. } = self.design.expr_node(expr).kind.clone() else {
            // an array of templates: the numbers of its elements (a param declared `[Cell; 2]`)
            if let Some(crate::types::Ty::Array(element, _)) = self.typed(expr)
                && matches!(*element, crate::types::Ty::Module(_))
            {
                return self.fold_template_array(expr, bindings);
            }
            // a value of `T` bound to a param of `Option<T>` is `Some` of it (§3.2.3)
            if self.types.implicit_some(expr) {
                return self.implicit_some_constant(expr, bindings);
            }
            // a composite folds to a constant of the IR's arena (`consts/imported.rs`)
            if self.typed(expr).is_some_and(|ty| ty.is_composite() || matches!(ty, crate::types::Ty::Enum(_))) {
                return self.composite_constant(expr, bindings);
            }
            return self.consts.value(expr, bindings);
        };
        let mut folded = Vec::new();
        for field in &fields {
            let Resolution::Decl(param) = field.res else { continue };
            let value = self.fold_param_value(field.value, bindings)?;
            folded.push((param, self.imported_tree(value), field.value));
        }
        let arguments = self.consts.template_arguments(expr, bindings)?;
        Ok(self.consts.intern_template(TemplateValue { module, arguments, fields: folded }))
    }
}
