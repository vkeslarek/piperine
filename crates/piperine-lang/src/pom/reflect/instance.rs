//! The placement view over a call expression classified as an instance.

use super::{Decl, Expr, FieldBinding, Module, Port, Reflect};
use crate::pom::body::{ExprKind, FieldInit};
use crate::pom::decl::DeclKind;
use crate::pom::builtin::{BuiltinId, BuiltinKind};
use crate::pom::design::Design;
use crate::pom::resolution::{CallClass, Resolution};
use crate::pom::ids::ExprId;



/// A placement: a view over a call expression classified as an instance. It
/// is not a declaration and not a node of its own.
#[derive(Clone, Copy)]
pub struct Instance<'d> {
    design: &'d Design,
    expr: ExprId,
}


impl<'d> Instance<'d> {
    pub(crate) fn new(design: &'d Design, expr: ExprId) -> Self {
        Instance { design, expr }
    }

    /// The call parts: callee, connections, param block, class.
    fn call(self) -> (ExprId, &'d [crate::pom::body::Arg], Option<&'d Vec<FieldInit>>, CallClass) {
        match &self.design.expr_node(self.expr).kind {
            ExprKind::Call { callee, args, params, class } => (*callee, args, params.as_ref(), *class),
            _ => unreachable!("Instance over a non-call expression"),
        }
    }

    /// The call expression.
    pub fn expr(self) -> Expr<'d> {
        self.design.expr(self.expr)
    }

    /// The name of the `let` whose whole value is this call, when its
    /// pattern is one identifier; none for an anonymous instance.
    pub fn name(self) -> Option<&'d str> {
        let owner = self.design.expr_node(self.expr).owner;
        let DeclKind::Let(l) = &self.design.decl_data(owner).kind else { return None };
        if l.value != self.expr {
            return None;
        }
        match &self.design.pat_node(l.pat).kind {
            crate::pom::body::PatKind::Ident { name, sub: None, .. } => Some(name.text.as_str()),
            _ => None,
        }
    }

    pub fn is_anonymous(self) -> bool {
        self.name().is_none()
    }

    /// The module placed, when a declaration names it.
    pub fn module(self) -> Option<Module<'d>> {
        match self.call().3 {
            CallClass::Instance { module: Some(m), .. } => Decl::new(self.design, m).as_module(),
            _ => None,
        }
    }

    /// The template (param, value input, constant, binding or function) the
    /// call was written through.
    pub fn via(self) -> Option<Decl<'d>> {
        match self.call().3 {
            CallClass::Instance { via: Some(v), .. } => Some(Decl::new(self.design, v)),
            _ => None,
        }
    }

    pub fn via_template(self) -> bool {
        self.via().is_some()
    }

    /// The called expression.
    pub fn callee(self) -> Expr<'d> {
        self.design.expr(self.call().0)
    }

    /// The declaration the placement is written in: the module body or
    /// generation for a `let`, the block or reaction otherwise.
    pub fn declared_in(self) -> Decl<'d> {
        let owner = self.design.expr_node(self.expr).owner;
        let at = Decl::new(self.design, owner);
        match at.kind() {
            DeclKind::Let(_) => at.declared_in().unwrap_or(at),
            _ => at,
        }
    }

    /// The connection arguments, in order.
    pub fn connections(self) -> Vec<Expr<'d>> {
        self.call().1.iter().map(|a| self.design.expr(a.value)).collect()
    }

    /// The argument written for `port`: the connection named after it, else
    /// the unnamed one at its position. None when the instance's module is
    /// unknown, is not the port's module, or no argument is written there.
    pub fn connection(self, port: Port<'d>) -> Option<Expr<'d>> {
        let module = self.module()?;
        if port.declared_in()? != module.decl() {
            return None;
        }
        let args = self.call().1;
        let named = args.iter().find(|a| a.res == Resolution::Decl(port.decl().id()));
        let arg = named.or_else(|| args.get(port.index()).filter(|a| a.name.is_none()))?;
        Some(self.design.expr(arg.value))
    }

    /// The `{ name: value }` param bindings as written.
    pub fn param_bindings(self) -> Vec<FieldBinding<'d>> {
        self.expr().fields()
    }

    /// The core module this call places (`cross(x)`, `ddt(x)`); none for a
    /// user module and for a call the classifier could not place.
    pub fn builtin(self) -> Option<BuiltinId> {
        match self.callee().resolution() {
            Resolution::Builtin(b) if b.info().kind == BuiltinKind::Module => Some(b),
            _ => None,
        }
    }
}


impl<'d> PartialEq for Instance<'d> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.expr == other.expr
    }
}


impl std::fmt::Debug for Instance<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Instance({:?})", self.expr)
    }
}
