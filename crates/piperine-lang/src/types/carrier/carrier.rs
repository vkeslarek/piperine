//! Discrete carriers (DIG-02, DIG-04): the type of a `signal` or a port of a discrete nature (`Bit<T>` or a project's),
//! the types a carrier may hold (`E0112`), initial values (`E0210`) and the history read `prev` (part II §5, part I §3.3).
//!
//! A carrier has the type `Ty::Carrier { nature, value }`; reading it gives `value`. The checker reads
//! the two edges of `Bit` and `prev` as built-ins only when the name
//! resolved to the stdlib's own item (§3.3.7): a project item of the same name is `P0001`.

use crate::refusal::forms::DigitalForm;
use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FileId, GenericArgNode, GenericArgsNode, GenericParamData, NatureFieldNode, PathId, Resolution, TypeId};

use super::CarrierTyping;
use crate::types::generics::generics::GenericSignature;
use crate::types::analog::nature_rules::NatureRules;
use crate::types::{Builtin, Nominal, Ty};

impl CarrierTyping<'_, '_> {
    /// A discrete nature written at `path`, the stdlib's `Bit<T>` or a project's (§4 rule 12): a
    /// carrier of `nature` holding the type its `value` field names. A generic nature takes its
    /// type arguments in order and falls back to each parameter's default; a nature whose `value`
    /// is not one of its parameters holds that type for every argument list.
    pub(in crate::types) fn discrete_carrier_type(&mut self, nature: DeclId, file: FileId, span: crate::msl::Span, path: PathId) -> Option<Ty> {
        let name = self.state.design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let DeclKind::Nature(data) = &self.state.design.decl_data(nature).kind else { return None };
        let value_type = data.fields.iter().find_map(|f| match f {
            NatureFieldNode::Value { ty, .. } => Some(*ty),
            _ => None,
        })?;
        let parameters: Vec<DeclId> = self.state.design.decl_data(nature).children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::GenericParam(_))).collect();
        let arguments = match self.state.design.path_node(path).segments.last().and_then(|s| s.args.clone()) {
            Some(GenericArgsNode::Angle { args, .. }) => args,
            None => Vec::new(),
            Some(_) => {
                self.refuse_with(file, span, DigitalForm::WithTheseGenericArguments, [name.to_string()]);
                return None;
            }
        };
        if arguments.len() > parameters.len() {
            self.refuse_with(file, span, DigitalForm::WithGenericArguments, [name.to_string(), arguments.len().to_string()]);
            return None;
        }
        let held = match self.state.design.type_resolution(value_type) {
            Resolution::Decl(parameter) if parameters.contains(&parameter) => {
                let position = parameters.iter().position(|p| *p == parameter)?;
                match arguments.get(position) {
                    Some(GenericArgNode::Type(t)) => *t,
                    Some(GenericArgNode::Const(_)) => {
                        self.refuse_with(file, span, DigitalForm::OfConstArgument, [name.to_string()]);
                        return None;
                    }
                    None => match &self.state.design.decl_data(parameter).kind {
                        DeclKind::GenericParam(GenericParamData::Type { default: Some(default), .. }) => *default,
                        _ => {
                            self.refuse_with(file, span, DigitalForm::WithoutItsTypeArgument, [name.to_string()]);
                            return None;
                        }
                    },
                }
            }
            _ => value_type,
        };
        let value = self.carried_type(held)?;
        // a resolved nature's `impl`s are judged wherever a carrier of it is (§4 rule 8)
        for implementation in crate::types::analog::nature::ResolveImpls::declarations(&self.state.traits, nature) {
            self.reach(implementation);
            // the body of `resolve` is run by the net's resolver (`elab/resolvers.rs`): it is checked and typed with the model
            for item in self.state.design.decl_data(implementation).children.clone() {
                if matches!(self.state.design.decl_data(item).kind, DeclKind::Fn(_)) {
                    self.reach(item);
                }
            }
        }
        if NatureRules::of(self.state.design, nature).resolved && !self.state.result.resolve_tables.contains_key(&nature) {
            let table = crate::types::analog::nature::ResolveImpls::new(self).of(nature);
            self.state.result.resolve_tables.insert(nature, table);
        }
        Some(Ty::Carrier { nature, value: Box::new(value) })
    }

    /// The type a carrier holds, `T` of `Bit<T>`: a type with `Repr` (part I §2), else `E0112`.
    /// An enum that does not derive `Repr` is `E0112` as well; a single-variant enum has a tag of
    /// zero levels and is `P0001`.
    fn carried_type(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.state.design.type_node(ty).clone();
        let file = self.state.design.decl_data(node.owner).file;
        if let Resolution::Decl(d) = self.state.design.type_resolution(ty)
            && matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_))
            && !self.expressions().derives(d, "Repr")
            && self.state.traits.hand_written_repr(&Ty::enumeration(d)).is_none()
        {
            let name = self.state.design.decl_data(d).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            self.no_repr(file, node.span, &format!("enum `{name}`"));
            return None;
        }
        let value = self.expressions().lower_type(ty)?;
        match &value {
            Ty::Event => {
                let message = "`Event` has no `Repr`: it is never carried, committed or stored (§13 rule 8)".to_string();
                self.coded(file, "E0441", node.span, message, "an `Event` is not carried");
                None
            }
            Ty::Enum(Nominal { decl: e, .. }) if self.state.design.decl_data(*e).children.iter().filter(|c| matches!(self.state.design.decl_data(**c).kind, DeclKind::Variant(_))).count() == 1 => {
                let name = self.state.design.decl_data(*e).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.refuse_with(file, node.span, DigitalForm::CarrierOfSingleVariantEnumItsTag, [name.to_string()]);
                None
            }
            // a type parameter bound by `Repr` is a type with one (`fn f<T: Repr>(a: Bit<T>)`, §3.8.6)
            v if self.trait_typing().repr_levels(v).is_some() || matches!(v, Ty::Param(param) if self.generics().bounds_of(*param).contains(&crate::types::Bound::Repr)) => {
                self.trait_typing().reach_hand_written_repr(v);
                Some(value)
            }
            other => {
                let name = other.name(self.state.design);
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
    pub(in crate::types) fn port_type(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.state.design.type_node(ty).clone();
        let file = self.state.design.decl_data(node.owner).file;
        match self.expressions().lower_type(ty)? {
            Ty::Node(nature) => {
                let rules = NatureRules::of(self.state.design, nature);
                if rules.flow || rules.signal_flow {
                    // A conservative node, or a signal-flow node (§6 rule 2): both are nodes of the
                    // analog solver.
                    Some(Ty::Node(nature))
                } else {
                    None
                }
            }
            carrier @ Ty::Carrier { .. } => Some(carrier),
            // a nature parameter of a generic module (`a: S`, `S: Nature`): a node of the nature the placement gives (GN-13)
            Ty::Param(param) if GenericSignature::resolve_bound(self.state.design, param).is_some() => self.resolved_parameter_carrier(param),
            nature @ Ty::Param(_) if self.modules().is_nature_parameter(&nature) => Some(nature),
            // an array of carriers, or of conservative nodes (CV-23)
            array @ Ty::Array(..) if array.is_carrier_element() => Some(array),
            // A value input: a port of a value type (§8.1.4), the expression the caller wrote.
            value @ (Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::Enum(_) | Ty::Option(_)) => Some(value),
            other => {
                self.checker.refuse_with(file, node.span, DigitalForm::ValueInputPortOfType, [other.name(self.checker.state.design).to_string()]);
                None
            }
        }
    }

    /// The carrier a nature parameter bound by `Resolve<X, V>` stands for in a generic body: a carrier of the parameter that
    /// holds `V` (`X` when the bound is written `Resolve<X>`), resolved like the nature it becomes (§22.3). `None` for a
    /// bound whose types are refused.
    fn resolved_parameter_carrier(&mut self, param: DeclId) -> Option<Ty> {
        let table = crate::types::analog::nature::ResolveImpls::new(self).of(param);
        let entry = table.first()?.clone();
        self.state.result.resolve_tables.entry(param).or_insert(table);
        Some(Ty::Carrier { nature: param, value: Box::new(entry.value) })
    }

    /// The type a read of a port or a signal declaration has: a node is a node, a carrier gives
    /// the value it holds.
    pub(in crate::types) fn read_type(&mut self, decl: DeclId, ty: TypeId) -> Option<Ty> {
        match self.port_type(ty)? {
            carrier @ Ty::Carrier { .. } => {
                let held = self.modules().generated_type(decl, carrier.clone());
                self.state.result.carriers.insert(decl, held);
                let Ty::Carrier { value, .. } = carrier else { return None };
                Some(*value)
            }
            // an array of carriers reads as the array of their values; an array of nodes is the array itself
            array @ Ty::Array(..) if array.is_carrier_element() => {
                let held = self.modules().generated_type(decl, array.clone());
                self.state.result.carriers.insert(decl, held);
                Some(array.read_value())
            }
            node => Some(node),
        }
    }

    /// The port or signal `expr` reads, through parentheses, with the type of its declaration.
    pub(in crate::types) fn declared_port(&mut self, expr: ExprId) -> Option<(DeclId, Ty)> {
        match &self.state.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.declared_port(*inner),
            // one element of an array of carriers or of nodes (CV-24): the carrier the element is, of the array's declaration
            ExprKind::Index { expr: base, .. } => match self.declared_port(*base)? {
                (decl, Ty::Array(element, _)) if element.is_carrier_element() => Some((decl, *element)),
                _ => None,
            },
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
                if self.state.design.is_bundle_owner(decl) || self.state.design.is_bundle_group(decl) {
                    return None;
                }
                let ty = match &self.state.design.decl_data(decl).kind {
                    DeclKind::Port(p) => p.ty,
                    DeclKind::Signal(s) => s.ty,
                    // a handle is the carrier the caller passes (§3.1.5)
                    DeclKind::FnParam(_) => return self.modules().handle_type(decl).filter(|t| !self.modules().is_bundle_type(t)).map(|t| (decl, t)),
                    _ => return None,
                };
                self.port_type(ty).map(|t| (decl, t))
            }
            // a field of a bundle handle: the carrier or node the `struct` field declares (`bundle_handles.rs`)
            ExprKind::Field { .. } if self.modules().bundle_handle_root(expr).is_some() => self.modules().bundle_handle_field(expr).filter(|(_, ty)| !self.modules().is_bundle_type(ty)),
            // a field of a bundle port or signal: the carrier or node its leaf stands for (§7 rule 12), at an element of an array of
            // bundles the element of the leaf (`rx[k].valid`)
            ExprKind::Field { .. } if let Some(place) = self.state.design.bundle_place(expr).filter(|place| self.state.design.leaf_owner(place.decl).is_some()) => self.modules().bundle_field_type(&place),
            // `c.state`: a `pub` port or signal of a placed child (§8.2.8).
            ExprKind::Field { res: Resolution::Decl(decl), .. } => {
                let ty = match &self.state.design.decl_data(*decl).kind {
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
    pub(in crate::types) fn initial_reads(&mut self, init: ExprId) {
        let node = self.state.design.expr_node(init).clone();
        let file = self.file_of_expr(init);
        let what = match &node.kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(d)) if matches!(self.state.design.decl_data(d).kind, DeclKind::Port(_) | DeclKind::Signal(_)) => Some("a carrier"),
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
            for child in crate::pom::BodyWalk::new(self.state.design).expression_children(&node.kind) {
                self.initial_reads(child);
            }
        }
    }

    /// Whether `f` is the stdlib's own item behind a built-in method (§3.3.7): a method of the
    /// `impl Bit` (`rise`, `fall`).
    pub(in crate::types) fn stdlib_builtin(&self, f: DeclId) -> Option<Builtin> {
        let data = self.state.design.decl_data(f);
        if !matches!(data.kind, DeclKind::Fn(_)) || !self.is_std(f) {
            return None;
        }
        let name = data.name.as_ref()?.text.as_str();
        let parent = data.parent.map(|p| self.state.design.decl_data(p))?;
        match (&parent.kind, name) {
            (DeclKind::Impl(_), "rise") => Some(Builtin::Rise),
            (DeclKind::Impl(_), "fall") => Some(Builtin::Fall),
            _ => None,
        }
    }

    /// The type a path's first segment names when it is the owner of an associated item
    /// (`T::WIDTH`, `T::default()`): a built-in scalar type, a struct or an enum. `None` for any
    /// other segment.
    pub(in crate::types) fn path_owner_type(&mut self, file: FileId, span: Span, owner: Resolution) -> Option<Ty> {
        match owner {
            Resolution::Builtin(id) => match (id.info().name, id.width()) {
                ("fN", Some(w)) => Ty::float(u32::from(w)),
                ("bool", _) => Some(Ty::Bool),
                ("str", _) => Some(Ty::Str),
                ("uN", Some(w)) => self.expressions().integer_type(file, span, Ty::UInt, u128::from(w), &id.name()),
                ("iN", Some(w)) => self.expressions().integer_type(file, span, Ty::Int, u128::from(w), &id.name()),
                ("usize", _) => Some(Ty::UInt(64)),
                ("isize", _) => Some(Ty::Int(64)),
                _ => None,
            },
            Resolution::Decl(decl) if self.generics().parameter_owner(owner).is_some() => Some(Ty::Param(decl)),
            Resolution::Decl(decl) if matches!(self.state.design.decl_data(decl).kind, DeclKind::Enum(_)) => self.expressions().enum_type(decl, file, span),
            Resolution::Decl(decl) if matches!(self.state.design.decl_data(decl).kind, DeclKind::Struct(_)) => self.expressions().struct_type(decl, file, span),
            _ => None,
        }
    }

    /// `T::WIDTH` written with a concrete `Repr` type `T` as its first segment: the level count
    /// of `T`, a const `u32` (§2, US-02 AC 3). `None` when `expr` is not such a path, so the caller
    /// reports it as it reports any other built-in name.
    pub(in crate::types) fn width_read(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.state.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let segments = self.state.design.path_node(*path).segments.clone();
        let [owner, member] = segments.as_slice() else { return None };
        if member.name.text != "WIDTH" || !matches!(member.res, Resolution::Deferred(_) | Resolution::Builtin(_)) {
            return None;
        }
        let file = self.file_of_expr(expr);
        let owner_type = self.path_owner_type(file, node.span, owner.res)?;
        if let Ty::Param(param) = owner_type {
            return self.generics().parameter_width(param, file, node.span);
        }
        let levels = self.trait_typing().repr_levels(&owner_type)?;
        self.state.result.widths.insert(expr, levels);
        Some(Ty::UInt(32))
    }

    /// `s.others()` written on a carrier of a one-driver nature is `E0224` naming the carrier: the
    /// method reads the drives of a resolved carrier other than the module's own, and a one-driver
    /// carrier has no other (§5.2.7). Returns whether the rule fired, so the caller stops there; a
    /// resolved carrier is not this rule's (until the resolved carriers are typed it stays `P0001`).
    pub(in crate::types) fn others_on_one_driver(&mut self, expr: ExprId, recv: ExprId, args: &[Arg]) -> bool {
        let Some((decl, Ty::Carrier { nature, .. })) = self.declared_port(recv) else { return false };
        if !NatureRules::of(self.state.design, nature).one_driver {
            return false;
        }
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let name = self.state.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let nature_name = self.state.design.decl_data(nature).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("`{name}.others()`: `{name}` is a carrier of `{nature_name}`, which has no `resolve`, so it has one drive and no other to read (§5.2.7)");
        self.coded(file, "E0224", span, message, "`others()` on a one-driver carrier");
        for arg in args {
            self.expressions().infer(arg.value);
        }
        true
    }

    /// `c.prev()` on a discrete carrier: the value before its most recent change, of the
    /// carrier's value type (§5.3). On a node it is `E0230`.
    pub(in crate::types) fn prev_read(&mut self, expr: ExprId, recv: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        match self.declared_port(recv) {
            Some((_, Ty::Carrier { value, .. })) => {
                self.expressions().apply(expr, &[], args)?;
                self.state.result.builtins.insert(expr, Builtin::Prev);
                self.expressions().infer(recv);
                Some(*value)
            }
            Some((_, Ty::Node(_))) => {
                self.coded(file, "E0230", span, "`prev()` applies to a discrete carrier: a node has no past (§5.3.2)".to_string(), "`prev()` on a node");
                None
            }
            _ => {
                self.refuse(file, span, DigitalForm::MethodPrevOnThisReceiver);
                None
            }
        }
    }

    /// `s.others()` on a resolved carrier (§5.2.7): the resolution of every drive but this module's
    /// own, of the nature's `value` type. The one-driver carrier was `E0224` before this is asked.
    pub(in crate::types) fn others_read(&mut self, expr: ExprId, recv: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        match self.declared_port(recv) {
            Some((_, Ty::Carrier { nature, value })) if NatureRules::of(self.state.design, nature).resolved => {
                self.expressions().apply(expr, &[], args)?;
                self.state.result.builtins.insert(expr, Builtin::Others);
                self.expressions().infer(recv);
                Some(*value)
            }
            _ => {
                self.refuse(file, span, DigitalForm::MethodOthersOnThisReceiver);
                None
            }
        }
    }
}
