//! Template values (§8.4, GN-21): `Name { params }` is a value of the type of its module wherever a value goes. Its
//! params are checked like a placement's block, a required param it leaves out is `E0319` (§8.4.2), and it is a const
//! expression, so a param value that reads a port, a signal or a probe is `E0362` (§8.4.6).

use crate::msl::Diagnostic;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FieldInit, Resolution};

use super::check::Checker;
use super::{Nominal, ParamRequirement, Ty};

impl Checker<'_> {
    /// A template `Name { params }` (§8.4.1): the type is the module it names with the generic arguments it names or lets be
    /// inferred, `Name<G>` (§8.4.3), so a template of another module, or of the same module with other arguments, where a module
    /// type is expected is a mismatch (`E0180`) naming both. An argument left out is taken from the `expected` type first.
    pub(super) fn template_value(&mut self, expr: ExprId, module: DeclId, fields: &[FieldInit], expected: Option<&Ty>) -> Option<Ty> {
        self.param_block(fields, None);
        self.require_bound_params(expr, module, fields);
        self.require_const_params(fields);
        // a template of a generic module names its arguments or lets them be inferred (§8.4.3); they travel with the template
        let arguments = if self.state.design.decl_data(module).generics_span.is_some() {
            let expected = match expected {
                Some(Ty::Module(wanted)) if wanted.decl == module => Some(wanted),
                _ => None,
            };
            let substitution = self.generics().template_instantiation(expr, module, fields, expected)?;
            self.generics().generic_signature(module).parameters().iter().filter_map(|parameter| substitution.get(parameter.decl).cloned()).collect()
        } else {
            Vec::new()
        };
        // the template's module is placed at elaboration, through a callable param whose module the checker never names
        self.reach(module);
        let ty = Ty::Module(Nominal { decl: module, arguments });
        self.state.result.exprs[expr.index()] = Some(ty.clone());
        Some(ty)
    }

    /// The module a bare name `expr` names (`Cell`, read as the template `Cell {}`, §8.4.1 amendment), `None` for any other
    /// expression.
    pub(super) fn bare_module(&self, expr: ExprId) -> Option<DeclId> {
        let ExprKind::Path(path) = &self.state.design.expr_node(expr).kind else { return None };
        match self.state.design.path_node(*path).segments.last().map(|segment| segment.res) {
            Some(Resolution::Decl(decl)) if matches!(self.state.design.decl_data(decl).kind, DeclKind::Module(_)) => Some(decl),
            _ => None,
        }
    }

    /// Every required param of `module` (§8.1.17, `ParamRequirement`) is bound in the template (§8.4.2), else `E0319` naming
    /// the first one; an `Option<T>` param left out is `None`, as at a placement.
    fn require_bound_params(&mut self, expr: ExprId, module: DeclId, fields: &[FieldInit]) {
        let data = self.state.design.decl_data(module).clone();
        let module_name = data.name.as_ref().map_or("", |name| name.text.as_str()).to_string();
        let mut missing = None;
        for child in &data.children {
            let child_data = self.state.design.decl_data(*child).clone();
            let DeclKind::Param(param) = &child_data.kind else { continue };
            if fields.iter().any(|field| field.res == Resolution::Decl(*child)) {
                continue;
            }
            let ty = self.expressions().lower_type_quietly(param.ty);
            if ParamRequirement::new(param, ty.as_ref()).is_required() {
                missing = child_data.name.as_ref().map(|name| name.text.clone());
                break;
            }
        }
        let Some(name) = missing else { return };
        let (file, span) = (self.file_of_expr(expr), self.state.design.expr_node(expr).span);
        let message = format!("the template of `{module_name}` leaves the required param `{name}` unbound (§8.4.2)");
        self.report(file, Diagnostic::new("E0319", message, span, "required param not bound"));
    }

    /// A template is evaluated at elaboration (§8.4.6): a param value that reads a port, a signal or a probe is `E0362`.
    fn require_const_params(&mut self, fields: &[FieldInit]) {
        for field in fields {
            let Some(read) = self.runtime_read(field.value) else { continue };
            let (file, span) = (self.file_of_expr(field.value), self.state.design.expr_node(field.value).span);
            let message = format!("a template is a const expression and this param value reads {read} (§8.4.6)");
            self.report(file, Diagnostic::new("E0362", message, span, "not a const expression"));
        }
    }
}
