//! Discrete carriers (DIG-02, DIG-04): the type of a `signal` or a port of the nature `Bit<T>`,
//! the types a carrier may hold (`E0112`), initial values (`E0210`), the flag reads `known`,
//! `known_or`, `is_known`, `high`, and the history read `prev` (part II §5, part I §3.3).
//!
//! A carrier has the type `Ty::Carrier { value }`; reading it gives `value`. The checker reads
//! the four flag reads, the two edges of `Bit` and `prev` as built-ins only when the name
//! resolved to the stdlib's own item (§3.3.7): a project item of the same name is `P0001`.

use crate::msl::Diagnostic;
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FileId, GenericArgNode, GenericArgsNode, GenericParamData, NatureKind, PathId, Resolution, TypeId};

use super::check::Checker;
use super::{Builtin, Ty};

/// The methods of a value or a carrier that are flag reads, by name.
pub(super) fn flag_builtin(name: &str) -> Option<Builtin> {
    match name {
        "known" => Some(Builtin::Known),
        "known_or" => Some(Builtin::KnownOr),
        "is_known" => Some(Builtin::IsKnown),
        "level" => Some(Builtin::Level),
        "high" => Some(Builtin::High),
        _ => None,
    }
}

impl Checker<'_> {
    /// `Bit<T>` written at `path`: a carrier of the nature `nature` holding `T` (`bool` when the
    /// argument is left out, the default of the nature's generic).
    pub(super) fn bit_type(&mut self, nature: DeclId, file: FileId, span: crate::msl::Span, path: PathId) -> Option<Ty> {
        let args = self.design.path_node(path).segments.last().and_then(|s| s.args.clone());
        let value = match &args {
            Some(GenericArgsNode::Angle { args, .. }) if args.len() == 1 => match &args[0] {
                GenericArgNode::Type(t) => self.carried_type(*t)?,
                GenericArgNode::Const(_) => {
                    self.unsupported(file, span, "`Bit` of a const argument");
                    return None;
                }
            },
            None => {
                let default = self.design.decl_data(nature).children.iter().find_map(|c| match &self.design.decl_data(*c).kind {
                    DeclKind::GenericParam(GenericParamData::Type { default: Some(t), .. }) => Some(*t),
                    _ => None,
                })?;
                self.carried_type(default)?
            }
            Some(_) => {
                self.unsupported(file, span, "`Bit` with these generic arguments");
                return None;
            }
        };
        Some(Ty::Carrier { nature, value: Box::new(value) })
    }

    /// The type a carrier holds, `T` of `Bit<T>`: a type with `Repr` (part I §2), else `E0112`.
    /// An enum that does not derive `Repr` is `E0112` as well; a single-variant enum has a tag of
    /// zero levels and is `P0001`.
    fn carried_type(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.design.type_node(ty).clone();
        let file = self.design.decl_data(node.owner).file;
        if let Resolution::Decl(d) = self.design.type_resolution(ty)
            && matches!(self.design.decl_data(d).kind, DeclKind::Enum(_))
            && !self.derives(d, "Repr")
        {
            let name = self.design.decl_data(d).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            self.no_repr(file, node.span, &format!("enum `{name}`"));
            return None;
        }
        let value = self.lower_type(ty)?;
        match &value {
            Ty::Event => {
                let message = "`Event` has no `Repr`: it is never carried, committed or stored (§13 rule 8)".to_string();
                self.coded(file, "E0441", node.span, message, "an `Event` is not carried");
                None
            }
            Ty::Enum(e) if self.design.decl_data(*e).children.iter().filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::Variant(_))).count() == 1 => {
                let name = self.design.decl_data(*e).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.unsupported(file, node.span, &format!("a carrier of the single-variant enum `{name}` (its tag has zero levels)"));
                None
            }
            v if v.has_repr() => Some(value),
            other => {
                let name = other.name(self.design);
                self.no_repr(file, node.span, &name);
                None
            }
        }
    }

    fn no_repr(&mut self, file: FileId, span: crate::msl::Span, what: &str) {
        let message = format!("`{what}` has no `Repr`: a carrier holds only a type that implements it (part I §2)");
        self.coded(file, "E0112", span, message, "no `Repr`");
    }

    /// The type of a port or a signal: a node of a conservative nature, a carrier of `Bit<T>`,
    /// or `P0001` and `None`.
    pub(super) fn port_type(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.design.type_node(ty).clone();
        let file = self.design.decl_data(node.owner).file;
        match self.lower_type(ty)? {
            Ty::Node(nature) => {
                let DeclKind::Nature(data) = &self.design.decl_data(nature).kind else { return None };
                let name = self.design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                match data.kind {
                    NatureKind::Conservative => Some(Ty::Node(nature)),
                    NatureKind::Discrete => {
                        self.unsupported(file, node.span, &format!("discrete nature `{name}`"));
                        None
                    }
                    NatureKind::Resolved => {
                        self.unsupported(file, node.span, &format!("resolved nature `{name}`"));
                        None
                    }
                    NatureKind::SignalFlow => {
                        self.unsupported(file, node.span, &format!("signal-flow nature `{name}`"));
                        None
                    }
                    NatureKind::Invalid => None,
                }
            }
            carrier @ Ty::Carrier { .. } => Some(carrier),
            // A value input: a port of a value type (§8.1.4), the expression the caller wrote.
            value @ (Ty::F64 | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::Enum(_) | Ty::Option(_)) => Some(value),
            other => {
                self.unsupported(file, node.span, &format!("value input port of type `{}`", other.name(self.design)));
                None
            }
        }
    }

    /// The type a read of a port or a signal declaration has: a node is a node, a carrier gives
    /// the value it holds.
    pub(super) fn read_type(&mut self, decl: DeclId, ty: TypeId) -> Option<Ty> {
        match self.port_type(ty)? {
            carrier @ Ty::Carrier { .. } => {
                self.carriers.insert(decl, carrier.clone());
                let Ty::Carrier { value, .. } = carrier else { return None };
                Some(*value)
            }
            node => Some(node),
        }
    }

    /// The port or signal `expr` reads, through parentheses, with the type of its declaration.
    pub(super) fn declared_port(&mut self, expr: ExprId) -> Option<(DeclId, Ty)> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.declared_port(*inner),
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
                let ty = match &self.design.decl_data(decl).kind {
                    DeclKind::Port(p) => p.ty,
                    DeclKind::Signal(s) => s.ty,
                    _ => return None,
                };
                self.port_type(ty).map(|t| (decl, t))
            }
            // `c.state`: a `pub` port or signal of a placed child (§8.2.8).
            ExprKind::Field { res: Resolution::Decl(decl), .. } => {
                let ty = match &self.design.decl_data(*decl).kind {
                    DeclKind::Port(p) => p.ty,
                    DeclKind::Signal(s) => s.ty,
                    _ => return None,
                };
                self.port_type(ty).map(|t| (*decl, t))
            }
            _ => None,
        }
    }

    /// The initial value of a carrier or a node reads no carrier and no probe (`E0210`, §5.1.5).
    /// The nodes a probe names are part of the probe, not reads of their own.
    pub(super) fn initial_reads(&mut self, init: ExprId) {
        let node = self.design.expr_node(init).clone();
        let file = self.file_of_expr(init);
        let what = match &node.kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Port(_) | DeclKind::Signal(_)) => Some("a carrier"),
                _ => None,
            },
            ExprKind::Call { class: crate::pom::CallClass::Probe, .. } => Some("a probe"),
            _ => None,
        };
        if let Some(what) = what {
            let message = format!("an initial value reads {what}: it is committed at (0, 0), before anything exists over time (§5.1.5)");
            self.report(file, Diagnostic::new("E0210", message, node.span, format!("{what} in an initial value")));
        }
        if what != Some("a probe") {
            for child in crate::pom::expr_children(&node.kind, self.design) {
                self.initial_reads(child);
            }
        }
    }

    /// Whether `f` is the stdlib's own item behind a built-in method (§3.3.7): a method of the
    /// trait `Known`, or of the `impl Bit`.
    pub(super) fn stdlib_builtin(&self, f: DeclId) -> Option<Builtin> {
        let data = self.design.decl_data(f);
        if !matches!(data.kind, DeclKind::Fn(_)) || !self.is_std(f) {
            return None;
        }
        let name = data.name.as_ref()?.text.as_str();
        let parent = data.parent.map(|p| self.design.decl_data(p))?;
        match (&parent.kind, name) {
            (DeclKind::Trait(_), "known" | "known_or" | "is_known") if parent.name.as_ref().is_some_and(|n| n.text == "Known") => flag_builtin(name),
            (DeclKind::Impl(_), "high") => Some(Builtin::High),
            (DeclKind::Impl(_), "rise") => Some(Builtin::Rise),
            (DeclKind::Impl(_), "fall") => Some(Builtin::Fall),
            _ => None,
        }
    }

    /// The flag read `builtin` applied to a receiver of type `recv_ty`: its result type, or
    /// `None` when the receiver's type has no such read (`high` is the read of a `Bit`).
    pub(super) fn flag_read(&mut self, expr: ExprId, builtin: Builtin, recv_ty: &Ty, args: &[Arg]) -> Option<Option<Ty>> {
        if !recv_ty.has_repr() || (builtin == Builtin::High && *recv_ty != Ty::Bool) {
            return None;
        }
        let (params, result) = match builtin {
            Builtin::Known => (vec![], Ty::Option(Box::new(recv_ty.clone()))),
            Builtin::KnownOr => (vec![recv_ty.clone()], recv_ty.clone()),
            Builtin::IsKnown | Builtin::High => (vec![], Ty::Bool),
            // §22.1: level `k` of the value, `k: u32`; known when `k` is (§3.3.14).
            Builtin::Level => (vec![Ty::UInt(32)], Ty::Option(Box::new(Ty::Bool))),
            Builtin::Changed | Builtin::Prev | Builtin::Rise | Builtin::Fall | Builtin::Inflow => return None,
        };
        if self.apply(expr, &params, args).is_none() {
            return Some(None);
        }
        self.builtins.insert(expr, builtin);
        Some(Some(result))
    }

    /// `T::WIDTH` written with a concrete `Repr` type `T` as its first segment: the level count
    /// of `T`, a const `u32` (§2, US-02 AC 3). `None` when `expr` is not such a path, so the caller
    /// reports it as it reports any other built-in name.
    pub(super) fn width_read(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let segments = self.design.path_node(*path).segments.clone();
        let [owner, member] = segments.as_slice() else { return None };
        if member.name.text != "WIDTH" || !matches!(member.res, Resolution::Deferred(_) | Resolution::Builtin(_)) {
            return None;
        }
        let file = self.file_of_expr(expr);
        let owner_type = match owner.res {
            Resolution::Builtin(id) => match (id.info().name, id.width()) {
                ("fN", Some(64)) => Ty::F64,
                ("bool", _) => Ty::Bool,
                ("uN", Some(w)) => self.integer_type(file, node.span, Ty::UInt, u128::from(w), &id.name())?,
                ("iN", Some(w)) => self.integer_type(file, node.span, Ty::Int, u128::from(w), &id.name())?,
                ("usize", _) => Ty::UInt(64),
                ("isize", _) => Ty::Int(64),
                _ => return None,
            },
            Resolution::Decl(decl) if matches!(self.design.decl_data(decl).kind, DeclKind::Enum(_)) => self.enum_type(decl, file, node.span)?,
            _ => return None,
        };
        let levels = owner_type.levels(self.design)?;
        self.widths.insert(expr, levels);
        Some(Ty::UInt(32))
    }

    /// `c.prev()` on a discrete carrier: the value before its most recent change, of the
    /// carrier's value type (§5.3). On a node it is `E0230`.
    pub(super) fn prev_read(&mut self, expr: ExprId, recv: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        match self.declared_port(recv) {
            Some((_, Ty::Carrier { value, .. })) => {
                self.apply(expr, &[], args)?;
                self.builtins.insert(expr, Builtin::Prev);
                self.infer(recv);
                Some(*value)
            }
            Some((_, Ty::Node(_))) => {
                self.coded(file, "E0230", span, "`prev()` applies to a discrete carrier: a node has no past (§5.3.2)".to_string(), "`prev()` on a node");
                None
            }
            _ => {
                self.unsupported(file, span, "method `prev` on this receiver");
                None
            }
        }
    }
}
