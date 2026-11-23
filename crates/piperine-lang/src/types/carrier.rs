//! Discrete carriers (DIG-02, DIG-04): the type of a `signal` or a port of a discrete nature (`Bit<T>` or a project's),
//! the types a carrier may hold (`E0112`), initial values (`E0210`) and the history read `prev` (part II §5, part I §3.3).
//!
//! A carrier has the type `Ty::Carrier { nature, value }`; reading it gives `value`. The checker reads
//! the two edges of `Bit` and `prev` as built-ins only when the name
//! resolved to the stdlib's own item (§3.3.7): a project item of the same name is `P0001`.

use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FileId, GenericArgNode, GenericArgsNode, GenericParamData, NatureFieldNode, PathId, Resolution, TypeId};

use super::check::Checker;
use super::nature_rules::NatureRules;
use super::{Builtin, Nominal, Ty};

impl Checker<'_> {
    /// A discrete nature written at `path`, the stdlib's `Bit<T>` or a project's (§4 rule 12): a
    /// carrier of `nature` holding the type its `value` field names. A generic nature takes its
    /// type arguments in order and falls back to each parameter's default; a nature whose `value`
    /// is not one of its parameters holds that type for every argument list.
    pub(super) fn discrete_carrier_type(&mut self, nature: DeclId, file: FileId, span: crate::msl::Span, path: PathId) -> Option<Ty> {
        let name = self.design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let DeclKind::Nature(data) = &self.design.decl_data(nature).kind else { return None };
        let value_type = data.fields.iter().find_map(|f| match f {
            NatureFieldNode::Value { ty, .. } => Some(*ty),
            _ => None,
        })?;
        let parameters: Vec<DeclId> = self.design.decl_data(nature).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::GenericParam(_))).collect();
        let arguments = match self.design.path_node(path).segments.last().and_then(|s| s.args.clone()) {
            Some(GenericArgsNode::Angle { args, .. }) => args,
            None => Vec::new(),
            Some(_) => {
                self.unsupported(file, span, &format!("`{name}` with these generic arguments"));
                return None;
            }
        };
        if arguments.len() > parameters.len() {
            self.unsupported(file, span, &format!("`{name}` with {} generic arguments", arguments.len()));
            return None;
        }
        let held = match self.design.type_resolution(value_type) {
            Resolution::Decl(parameter) if parameters.contains(&parameter) => {
                let position = parameters.iter().position(|p| *p == parameter)?;
                match arguments.get(position) {
                    Some(GenericArgNode::Type(t)) => *t,
                    Some(GenericArgNode::Const(_)) => {
                        self.unsupported(file, span, &format!("`{name}` of a const argument"));
                        return None;
                    }
                    None => match &self.design.decl_data(parameter).kind {
                        DeclKind::GenericParam(GenericParamData::Type { default: Some(default), .. }) => *default,
                        _ => {
                            self.unsupported(file, span, &format!("`{name}` without its type argument"));
                            return None;
                        }
                    },
                }
            }
            _ => value_type,
        };
        let value = self.carried_type(held)?;
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
            && self.impls.hand_written_repr(&Ty::enumeration(d)).is_none()
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
            Ty::Enum(Nominal { decl: e, .. }) if self.design.decl_data(*e).children.iter().filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::Variant(_))).count() == 1 => {
                let name = self.design.decl_data(*e).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.unsupported(file, node.span, &format!("a carrier of the single-variant enum `{name}` (its tag has zero levels)"));
                None
            }
            v if self.repr_levels(v).is_some() => {
                self.reach_hand_written_repr(v);
                Some(value)
            }
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
                let name = self.design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                let rules = NatureRules::of(self.design, nature);
                if rules.flow || rules.signal_flow {
                    // A conservative node, or a signal-flow node (§6 rule 2): both are nodes of the
                    // analog solver.
                    Some(Ty::Node(nature))
                } else if rules.resolved {
                    self.unsupported(file, node.span, &format!("resolved nature `{name}`"));
                    None
                } else {
                    None
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
    /// `impl Bit` (`rise`, `fall`).
    pub(super) fn stdlib_builtin(&self, f: DeclId) -> Option<Builtin> {
        let data = self.design.decl_data(f);
        if !matches!(data.kind, DeclKind::Fn(_)) || !self.is_std(f) {
            return None;
        }
        let name = data.name.as_ref()?.text.as_str();
        let parent = data.parent.map(|p| self.design.decl_data(p))?;
        match (&parent.kind, name) {
            (DeclKind::Impl(_), "rise") => Some(Builtin::Rise),
            (DeclKind::Impl(_), "fall") => Some(Builtin::Fall),
            _ => None,
        }
    }

    /// The type a path's first segment names when it is the owner of an associated item
    /// (`T::WIDTH`, `T::default()`): a built-in scalar type, a struct or an enum. `None` for any
    /// other segment.
    pub(super) fn path_owner_type(&mut self, file: FileId, span: Span, owner: Resolution) -> Option<Ty> {
        match owner {
            Resolution::Builtin(id) => match (id.info().name, id.width()) {
                ("fN", Some(64)) => Some(Ty::F64),
                ("bool", _) => Some(Ty::Bool),
                ("str", _) => Some(Ty::Str),
                ("uN", Some(w)) => self.integer_type(file, span, Ty::UInt, u128::from(w), &id.name()),
                ("iN", Some(w)) => self.integer_type(file, span, Ty::Int, u128::from(w), &id.name()),
                ("usize", _) => Some(Ty::UInt(64)),
                ("isize", _) => Some(Ty::Int(64)),
                _ => None,
            },
            Resolution::Decl(decl) if self.parameter_owner(owner).is_some() => Some(Ty::Param(decl)),
            Resolution::Decl(decl) if matches!(self.design.decl_data(decl).kind, DeclKind::Enum(_)) => self.enum_type(decl, file, span),
            Resolution::Decl(decl) if matches!(self.design.decl_data(decl).kind, DeclKind::Struct(_)) => self.struct_type(decl, file, span),
            _ => None,
        }
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
        let owner_type = self.path_owner_type(file, node.span, owner.res)?;
        if let Ty::Param(param) = owner_type {
            return self.parameter_width(param, file, node.span);
        }
        let levels = self.repr_levels(&owner_type)?;
        self.widths.insert(expr, levels);
        Some(Ty::UInt(32))
    }

    /// `s.others()` written on a carrier of a one-driver nature is `E0224` naming the carrier: the
    /// method reads the drives of a resolved carrier other than the module's own, and a one-driver
    /// carrier has no other (§5.2.7). Returns whether the rule fired, so the caller stops there; a
    /// resolved carrier is not this rule's (until the resolved carriers are typed it stays `P0001`).
    pub(super) fn others_on_one_driver(&mut self, expr: ExprId, recv: ExprId, args: &[Arg]) -> bool {
        let Some((decl, Ty::Carrier { nature, .. })) = self.declared_port(recv) else { return false };
        if !NatureRules::of(self.design, nature).one_driver {
            return false;
        }
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let name = self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let nature_name = self.design.decl_data(nature).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("`{name}.others()`: `{name}` is a carrier of `{nature_name}`, which has no `resolve`, so it has one drive and no other to read (§5.2.7)");
        self.coded(file, "E0224", span, message, "`others()` on a one-driver carrier");
        for arg in args {
            self.infer(arg.value);
        }
        true
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
