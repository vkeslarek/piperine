//! Template values (§8.4, GN-21): `Name { params }` is a value of the type of its module wherever a value goes. Its
//! params are checked like a placement's block, a required param it leaves out is `E0319` (§8.4.2), and it is a const
//! expression, so a param value that reads a port, a signal or a probe is `E0362` (§8.4.6).

use crate::msl::Diagnostic;
use crate::pom::{DeclId, DeclKind, ExprId, FieldInit, Resolution};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// A template `Name { params }` (§8.4.1): the type is the module it names, so a template of another module where a
    /// module type is expected is a mismatch (`E0180`) naming both.
    pub(super) fn template_value(&mut self, expr: ExprId, module: DeclId, fields: &[FieldInit]) -> Option<Ty> {
        self.param_block(fields, None);
        self.require_bound_params(expr, module, fields);
        self.require_const_params(fields);
        self.exprs[expr.index()] = Some(Ty::Module(module));
        Some(Ty::Module(module))
    }

    /// Every param of `module` without a default is bound in the template (§8.4.2), else `E0319` naming the first one.
    fn require_bound_params(&mut self, expr: ExprId, module: DeclId, fields: &[FieldInit]) {
        let data = self.design.decl_data(module);
        let module_name = data.name.as_ref().map_or("", |name| name.text.as_str()).to_string();
        let missing = data.children.iter().find_map(|child| {
            let child_data = self.design.decl_data(*child);
            let DeclKind::Param(param) = &child_data.kind else { return None };
            let bound = fields.iter().any(|field| field.res == Resolution::Decl(*child));
            (param.default.is_none() && !bound).then(|| child_data.name.as_ref().map_or("", |name| name.text.as_str()).to_string())
        });
        let Some(name) = missing else { return };
        let (file, span) = (self.file_of_expr(expr), self.design.expr_node(expr).span);
        let message = format!("the template of `{module_name}` leaves the required param `{name}` unbound (§8.4.2)");
        self.report(file, Diagnostic::new("E0319", message, span, "required param not bound"));
    }

    /// A template is evaluated at elaboration (§8.4.6): a param value that reads a port, a signal or a probe is `E0362`.
    fn require_const_params(&mut self, fields: &[FieldInit]) {
        for field in fields {
            let Some(read) = self.runtime_read(field.value) else { continue };
            let (file, span) = (self.file_of_expr(field.value), self.design.expr_node(field.value).span);
            let message = format!("a template is a const expression and this param value reads {read} (§8.4.6)");
            self.report(file, Diagnostic::new("E0362", message, span, "not a const expression"));
        }
    }
}
