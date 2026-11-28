//! Types as written: the type a type node names, lowered to a [`Ty`] (§1, §3.8).

use crate::refusal::forms::ValueForm;
use crate::refusal::RefusedForm;
use crate::msl::{Diagnostic, Span};
use crate::pom::{BuiltinId, DeclId, DeclKind, FileId, GenericArgNode, GenericArgsNode, PathId, Resolution, TypeId, TypeKind};

use super::ExpressionTyping;
use crate::types::analog::nature_rules::NatureRules;
use crate::types::{ConstTerm, Ty};

impl ExpressionTyping<'_, '_> {
    /// The type a type node writes, or a `P0001` and `None` for one outside the subset.
    pub(in crate::types) fn lower_type(&mut self, ty: TypeId) -> Option<Ty> {
        let lowered = self.lower_type_node(ty);
        if let Some(t) = &lowered {
            self.state.result.written.insert(ty, t.clone());
        }
        lowered
    }

    /// Whether the path of a struct or enum type writes no generic argument where the declaration `decl`
    /// has no generic parameter: `Pair<4>` over a plain `struct Pair` is `P0001` (it would be read as `Pair`,
    /// silently ignoring `4`). A generic declaration is read by `generic_nominal`.
    fn writes_no_arguments_to_a_plain_type(&mut self, path: PathId, decl: DeclId, file: FileId, span: Span) -> bool {
        let written = self.state.design.path_node(path).segments.last().is_some_and(|segment| segment.args.is_some());
        if !written || self.state.design.decl_data(decl).generics_span.is_some() {
            return true;
        }
        let name = self.state.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        self.refuse_with(file, span, ValueForm::GenericArgumentsOnWhichHasNoGeneric, [name.to_string()]);
        false
    }

    /// The type a built-in name written as a type stands for: a primitive, `Option` or `Event`; any other built-in is
    /// refused.
    fn lower_builtin_type(&mut self, id: BuiltinId, path: PathId, file: FileId, span: Span) -> Option<Ty> {
        match (id.info().name, id.width()) {
            ("fN", Some(w)) => Ty::float(u32::from(w)),
            ("bool", _) => Some(Ty::Bool),
            // §1.2.9: `str` is a value type, the type of a string.
            ("str", _) => Some(Ty::Str),
            ("uN", Some(w)) => self.expressions().integer_type(file, span, Ty::UInt, u128::from(w), &id.name()),
            ("iN", Some(w)) => self.expressions().integer_type(file, span, Ty::Int, u128::from(w), &id.name()),
            // §1 Resolution: `usize` and `isize` are `UInt<64>` and `Int<64>` everywhere.
            ("usize", _) => Some(Ty::UInt(64)),
            ("isize", _) => Some(Ty::Int(64)),
            (family @ ("UInt" | "Int" | "Float"), _) => self.sized_type(file, span, path, family),
            ("Option", _) => self.expressions().option_type(file, span, path),
            ("Event", _) => Some(Ty::Event),
            _ => {
                self.refuse_with(file, span, ValueForm::Type, [id.name().to_string()]);
                None
            }
        }
    }

    /// The type a type node writes, by what its path resolves to: a built-in, a nature, a struct, an enum, a type parameter or a
    /// module; or by its shape, a tuple, an array or a function pointer.
    fn lower_type_node(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.state.design.type_node(ty).clone();
        let file = self.state.design.decl_data(node.owner).file;
        match &node.kind {
            TypeKind::Path(path) => match self.state.design.type_resolution(ty) {
                Resolution::Builtin(id) => self.lower_builtin_type(id, *path, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Nature(_)) && NatureRules::of(self.state.design, d).carries_values => self.carriers().discrete_carrier_type(d, file, node.span, *path),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Nature(_)) => Some(Ty::Node(d)),
                // §3.8.5: a generic struct or enum is written with its arguments; its fields are typed where it is used
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && self.state.design.decl_data(d).generics_span.is_some() => self.generics().generic_nominal(*path, d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && !self.writes_no_arguments_to_a_plain_type(*path, d, file, node.span) => None,
                // §3.8.2: a type or nature parameter is a type of the generic body, opaque
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::GenericParam(crate::pom::GenericParamData::Type { .. })) => Some(Ty::Param(d)),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_)) => self.expressions().enum_type(d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Struct(_)) => self.expressions().struct_type(d, file, node.span),
                // A module named as a type is the type of a param (§8.1.1, PI-04): a template of it; a generic one's names its
                // arguments, `G<2>` (§8.4.3)
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Module(_)) && self.state.design.decl_data(d).generics_span.is_some() => self.generics().generic_module_type(*path, d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Module(_)) => self.writes_no_arguments_to_a_plain_type(*path, d, file, node.span).then(|| Ty::module(d)),
                Resolution::Decl(d) => {
                    let what = RefusedForm::declaration_name(&self.state.design.decl_data(d).kind);
                    self.refuse_with(file, node.span, ValueForm::UsedAsType, [what.to_string()]);
                    None
                }
                _ => {
                    self.refuse(file, node.span, ValueForm::ThisType);
                    None
                }
            },
            TypeKind::Tuple(items) if items.is_empty() => Some(Ty::Unit),
            TypeKind::Tuple(items) => self.expressions().tuple_type(items),
            TypeKind::Array { elem, len } => self.expressions().array_type(file, node.span, *elem, *len),
            TypeKind::Flip(_) | TypeKind::Infer => {
                self.refuse(file, node.span, RefusedForm::type_form(&node.kind));
                None
            }
            TypeKind::FnPtr { params, ret } => {
                let params: Option<Vec<Ty>> = params.iter().map(|p| self.expressions().lower_type(*p)).collect();
                let ret = match ret {
                    Some(r) => self.expressions().lower_type(*r),
                    None => Some(Ty::Unit),
                };
                Some(Ty::Fn(params?, Box::new(ret?)))
            }
        }
    }

    /// `UInt<N>`, `Int<N>` or `Float<N>` written with its width argument. The width is an integer
    /// literal; a width computed from a const expression is outside the subset (`P0001`).
    fn sized_type(&mut self, file: FileId, span: Span, path: PathId, family: &str) -> Option<Ty> {
        let args = self.state.design.path_node(path).segments.last().and_then(|s| s.args.clone());
        // a width that names const parameters is an integer of the instantiation's width (`UInt<N>`, `Int<{ N + 1 }>`)
        let term = match &args {
            Some(GenericArgsNode::Angle { args, .. }) if args.len() == 1 => match &args[0] {
                GenericArgNode::Const(expr) => self.generics().const_term(*expr, file),
                GenericArgNode::Type(ty) => self.generics().const_parameter_named_by(*ty).map(ConstTerm::Param),
            },
            _ => None,
        };
        let width = match term {
            Some(ConstTerm::Value(value)) => u128::try_from(value).ok(),
            Some(generic @ (ConstTerm::Param(_) | ConstTerm::Expr(_))) if matches!(family, "UInt" | "Int") => {
                if let ConstTerm::Expr(expr) = &generic {
                    self.generics().defer_constant(*expr);
                }
                return Some(Ty::GenericInteger { signed: family == "Int", width: generic });
            }
            _ => None,
        };
        let Some(width) = width else {
            self.refuse_with(file, span, ValueForm::WithWidthThatIsNotIntegerLiteral, [family.to_string()]);
            return None;
        };
        let written = format!("{family}<{width}>");
        match family {
            "UInt" => self.expressions().integer_type(file, span, Ty::UInt, width, &written),
            "Int" => self.expressions().integer_type(file, span, Ty::Int, width, &written),
            _ => match u32::try_from(width).ok().and_then(Ty::float) {
                Some(float) => Some(float),
                None => {
                    let message = format!("`{written}`: a float width is one of 16, 32, 64 and 128");
                    self.report(file, Diagnostic::new("E0103", message, span, "float width outside {16, 32, 64, 128}"));
                    None
                }
            },
        }
    }

    /// An integer type of width `width`, written `written`: any width from 1 to 2^32 - 1 (§1 rule 5; the
    /// limit is an implementation limit, Appendix C.2). A width below 1 or past the limit is `E0103`.
    pub(in crate::types) fn integer_type(&mut self, file: FileId, span: Span, make: fn(u32) -> Ty, width: u128, written: &str) -> Option<Ty> {
        match width {
            0 => {
                let message = format!("`{written}`: an integer width is a const expression `>= 1`");
                self.report(file, Diagnostic::new("E0103", message, span, "integer width below 1"));
                None
            }
            1..=0xFFFF_FFFF => Some(make(width as u32)),
            _ => {
                let message = format!("`{written}`: an integer width is at most 2^32 - 1 (Appendix C.2)");
                self.report(file, Diagnostic::new("E0103", message, span, "integer width past the implementation limit"));
                None
            }
        }
    }
}
