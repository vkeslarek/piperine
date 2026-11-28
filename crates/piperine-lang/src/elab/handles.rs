//! Handle functions (generics GN-11, GN-12, part I §3.1.5): IR functions are pure values (MD-64) and a handle is
//! hardware, so a function with a handle parameter is no IR function. Each call is inlined where it stands, as the
//! functions that emit are (`records.rs`): its value parameters bound to locals holding the arguments, and each handle
//! parameter bound in `Scope::handles` to the caller's argument, the name of a port or a signal of the frame the call is
//! written in. A probe of the handle lowers to the terminals of that node, a read of a discrete handle to a read of that
//! carrier (its value), and a carrier method on it to the method on that carrier.

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

impl Elaboration<'_, '_> {
    /// Whether `function` takes a handle: a parameter the checker found to be one.
    pub(super) fn takes_handle(&self, function: DeclId) -> bool {
        self.state.design.decl_data(function).children.iter().any(|child| matches!(self.state.design.decl_data(*child).kind, DeclKind::FnParam(_)) && self.state.types.handle(*child))
    }

    /// What the handle parameter of a call is bound to for the argument `arg`: the argument itself (a name in the frame of
    /// the call), or, when it names a handle of the function being inlined, what that one is bound to.
    pub(super) fn handle_argument_target(&self, scope: &Scope, arg: ExprId) -> ExprId {
        let arg = self.peel_parens(arg);
        if let ExprKind::Path(path) = &self.state.design.expr_node(arg).kind
            && let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|segment| segment.res)
            && let Some(bound) = scope.digital.handles.get(&decl)
        {
            return *bound;
        }
        arg
    }
}
