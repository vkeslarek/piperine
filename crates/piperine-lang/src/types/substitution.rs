//! Substitution (part I §3.8.9, GN-01, GN-06): what a generic item's parameters are replaced by,
//! and the checker's reading of a written use of a generic type, `Pair<f64>`, `Maybe<T>`,
//! `Delay<u8, 4>`: each argument lowered by the kind of its parameter, an omitted one taken from
//! the parameter's default, and the use recorded as the `Nominal` of its type.

use std::collections::BTreeMap;

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FileId, GenericArgNode, GenericArgsNode, LitKind, PathId, Resolution, StmtKind, TypeId, TypeKind, UnOp};

use super::check::Checker;
use super::bounds::BoundCheck;
use super::generics::{GenericParameter, GenericSignature, ParameterDefault, ParameterKind};
use super::{ConstTerm, GenericValue, Length, Nominal, Ty};

/// The values the parameters of a generic item are replaced by, by parameter declaration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Substitution {
    values: BTreeMap<DeclId, GenericValue>,
}

impl Substitution {
    /// The empty substitution: every type is itself.
    pub fn new() -> Substitution {
        Substitution::default()
    }

    /// The substitution that gives the parameters of `signature` the `arguments`, in order; a
    /// parameter with no argument is left unbound.
    pub fn of(signature: &GenericSignature, arguments: &[GenericValue]) -> Substitution {
        let mut substitution = Substitution::new();
        for (parameter, argument) in signature.parameters().iter().zip(arguments) {
            substitution.bind(parameter.decl, argument.clone());
        }
        substitution
    }

    /// Replaces `parameter` by `value`.
    pub fn bind(&mut self, parameter: DeclId, value: GenericValue) {
        self.values.insert(parameter, value);
    }

    /// What `parameter` is replaced by.
    pub fn get(&self, parameter: DeclId) -> Option<&GenericValue> {
        self.values.get(&parameter)
    }

    /// Every parameter and what it is replaced by.
    pub fn entries(&self) -> impl Iterator<Item = (DeclId, &GenericValue)> {
        self.values.iter().map(|(parameter, value)| (*parameter, value))
    }

    /// Whether nothing is replaced.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// `ty` with every bound parameter replaced, through every part of the type (a generic
    /// argument and an array length included).
    pub fn apply(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Param(parameter) => match self.get(*parameter) {
                Some(GenericValue::Type(replacement)) => replacement.clone(),
                _ => ty.clone(),
            },
            Ty::Fn(params, ret) => Ty::Fn(params.iter().map(|p| self.apply(p)).collect(), Box::new(self.apply(ret))),
            Ty::Option(inner) => Ty::Option(Box::new(self.apply(inner))),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|item| self.apply(item)).collect()),
            Ty::Carrier { nature, value } => Ty::Carrier { nature: *nature, value: Box::new(self.apply(value)) },
            Ty::Struct(nominal) => Ty::Struct(self.apply_nominal(nominal)),
            Ty::Enum(nominal) => Ty::Enum(self.apply_nominal(nominal)),
            Ty::Array(element, length) => Ty::Array(Box::new(self.apply(element)), self.apply_length(length)),
            Ty::Range { element, iterable } => Ty::Range { element: Box::new(self.apply(element)), iterable: *iterable },
            Ty::Iterator { element, reversible } => Ty::Iterator { element: Box::new(self.apply(element)), reversible: *reversible },
            Ty::GenericInteger { signed, width } => match self.apply_term(width) {
                ConstTerm::Value(n @ 1..=4_294_967_295) if *signed => Ty::Int(n as u32),
                ConstTerm::Value(n @ 1..=4_294_967_295) => Ty::UInt(n as u32),
                replaced => Ty::GenericInteger { signed: *signed, width: replaced },
            },
            Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 | Ty::Bool | Ty::Unit | Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::UInt(_) | Ty::Int(_) | Ty::Event | Ty::Str | Ty::Never => ty.clone(),
        }
    }

    /// `value` with every bound parameter replaced.
    pub fn apply_value(&self, value: &GenericValue) -> GenericValue {
        match value {
            GenericValue::Type(ty) => GenericValue::Type(self.apply(ty)),
            GenericValue::Const(term) => GenericValue::Const(self.apply_term(term)),
            GenericValue::Function(_) | GenericValue::Closure(_) => value.clone(),
        }
    }

    /// The const argument `term` with a const parameter replaced by the const it is given.
    pub fn apply_term(&self, term: &ConstTerm) -> ConstTerm {
        match term {
            ConstTerm::Param(parameter) => match self.get(*parameter) {
                Some(GenericValue::Const(replacement)) => replacement.clone(),
                _ => term.clone(),
            },
            // `T::WIDTH` follows `T` to the parameter it is replaced by; a concrete type's width is read
            // where the instantiation is made (`Ty::levels`)
            ConstTerm::Width(parameter) => match self.get(*parameter) {
                Some(GenericValue::Type(Ty::Param(replacement))) => ConstTerm::Width(*replacement),
                _ => term.clone(),
            },
            ConstTerm::Value(_) | ConstTerm::Bool(_) | ConstTerm::Expr(_) => term.clone(),
        }
    }

    /// `length` with its const parameter replaced; a generic length that becomes a number is fixed.
    pub fn apply_length(&self, length: &Length) -> Length {
        match length {
            Length::Fixed(_) => length.clone(),
            Length::Generic(term) => match self.apply_term(term) {
                ConstTerm::Value(n) => u64::try_from(n).map_or_else(|_| length.clone(), Length::Fixed),
                replaced => Length::Generic(replaced),
            },
        }
    }

    /// The substitution that replaces first as `self` does and then as `outer` does: a parameter
    /// `self` gives a type that names an outer parameter ends as what `outer` gives that one.
    pub fn compose(&self, outer: &Substitution) -> Substitution {
        let mut composed = Substitution::new();
        for (parameter, value) in &self.values {
            composed.bind(*parameter, outer.apply_value(value));
        }
        for (parameter, value) in &outer.values {
            composed.values.entry(*parameter).or_insert_with(|| value.clone());
        }
        composed
    }

    /// The type and the arguments of a nominal type with every bound parameter replaced.
    fn apply_nominal(&self, nominal: &Nominal) -> Nominal {
        Nominal { decl: nominal.decl, arguments: nominal.arguments.iter().map(|argument| self.apply_value(argument)).collect() }
    }
}

impl Checker<'_> {
    /// The type a written use of the generic struct or enum `decl` is (`Pair<f64>`, `Maybe<T>`):
    /// its list held to §3.8, the arguments counted (`E0187`), each lowered by the kind of its
    /// parameter, an omitted one taken from the default (§3.8.7). `None` when one is refused.
    pub(super) fn generic_nominal(&mut self, path: PathId, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let signature = self.generic_signature(decl);
        let nodes = match self.design.path_node(path).segments.last().and_then(|segment| segment.args.clone()) {
            Some(GenericArgsNode::Angle { args, .. }) => args,
            _ => Vec::new(),
        };
        if !self.generic_argument_count(decl, nodes.len(), file, span) {
            return None;
        }
        let mut arguments: Vec<GenericValue> = Vec::new();
        let mut substitution = Substitution::new();
        for (index, parameter) in signature.parameters().iter().enumerate() {
            let value = match nodes.get(index) {
                Some(node) => self.generic_argument(parameter, node, file)?,
                None => self.default_argument(parameter, &substitution, file)?,
            };
            substitution.bind(parameter.decl, value.clone());
            arguments.push(value);
        }
        self.arguments_satisfy_bounds(&signature, &arguments, decl, file, span)?;
        // the declaration is typed over its own parameters once, for every use (fields, derives, E0186)
        let declared = if matches!(self.design.decl_data(decl).kind, DeclKind::Enum(_)) { self.enum_type(decl, file, span) } else { self.struct_type(decl, file, span) };
        declared?;
        self.arguments_have_repr(decl, &arguments, file, span)?;
        let nominal = Nominal { decl, arguments };
        Some(if matches!(self.design.decl_data(decl).kind, DeclKind::Enum(_)) { Ty::Enum(nominal) } else { Ty::Struct(nominal) })
    }

    /// Every argument satisfies every bound of its parameter (§3.8.7); one that does not is `E0187`
    /// naming the argument and the bound. `None` after a refusal.
    pub(super) fn arguments_satisfy_bounds(&mut self, signature: &GenericSignature, arguments: &[GenericValue], item: DeclId, file: FileId, span: Span) -> Option<()> {
        let item_name = self.design.decl_data(item).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut satisfied = true;
        for (parameter, argument) in signature.parameters().iter().zip(arguments) {
            let GenericValue::Type(ty) = argument else { continue };
            for bound in &parameter.bounds {
                if !BoundCheck::new(self).satisfies(ty, bound) {
                    let message = format!("`{}` does not satisfy the bound `{}` of `{}` in `{item_name}` (§3.8.7)", ty.source_name(self.design), self.bound_name(bound), parameter.name);
                    self.coded(file, "E0187", span, message, "an argument that does not satisfy its bound");
                    satisfied = false;
                }
            }
        }
        satisfied.then_some(())
    }

    /// H.8 rule 6, §2 rule 3: `#[derive(Repr)]` on a generic type holds for the instance whose every type
    /// argument has a `Repr`; one that has none is `E0110` naming the argument. `None` after a refusal.
    fn arguments_have_repr(&mut self, decl: DeclId, arguments: &[GenericValue], file: FileId, span: Span) -> Option<()> {
        if !self.derives(decl, "Repr") {
            return Some(());
        }
        let name = self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut holds = true;
        for argument in arguments {
            let GenericValue::Type(ty) = argument else { continue };
            if !ty.has_param() && self.repr_levels(ty).is_none() {
                self.coded(file, "E0110", span, format!("`{name}` derives `Repr`, and its argument `{}` has no `Repr` (§2 rule 3)", ty.source_name(self.design)), "an argument without `Repr`");
                holds = false;
            }
        }
        holds.then_some(())
    }

    /// The argument `node` for `parameter`: a type for a type or nature parameter, a const for a
    /// const parameter, which a lone name may also spell (`Delay<u8, N>`, §3.8.5).
    pub(super) fn generic_argument(&mut self, parameter: &GenericParameter, node: &GenericArgNode, file: FileId) -> Option<GenericValue> {
        match (&parameter.kind, node) {
            (ParameterKind::Type | ParameterKind::Nature, GenericArgNode::Type(ty)) => self.lower_type(*ty).map(GenericValue::Type),
            (ParameterKind::Const(_), GenericArgNode::Const(expr)) => self.const_term(*expr, file).map(GenericValue::Const),
            (ParameterKind::Const(_), GenericArgNode::Type(ty)) => match self.const_parameter_named_by(*ty) {
                Some(named) => Some(GenericValue::Const(ConstTerm::Param(named))),
                None => {
                    self.coded(file, "E0187", self.design.type_node(*ty).span, format!("`{}` takes a constant here, found a type (§3.8.5)", parameter.name), "a constant");
                    None
                }
            },
            (ParameterKind::Type | ParameterKind::Nature, GenericArgNode::Const(expr)) => {
                let at = self.design.expr_node(*expr).span;
                self.coded(file, "E0187", at, format!("`{}` takes a type here, found a constant (§3.8.5)", parameter.name), "a type");
                None
            }
        }
    }

    /// The argument an omitted `parameter` takes from its default, with the earlier parameters
    /// already replaced (`struct Pair<T, U = T>`); a parameter with no default is not omitted
    /// here, the count was checked.
    pub(super) fn default_argument(&mut self, parameter: &GenericParameter, earlier: &Substitution, file: FileId) -> Option<GenericValue> {
        match parameter.default.as_ref()? {
            ParameterDefault::Type(ty) => {
                let lowered = self.lower_type(*ty)?;
                Some(GenericValue::Type(earlier.apply(&lowered)))
            }
            ParameterDefault::Const(expr) => {
                self.const_term(*expr, file).map(|term| GenericValue::Const(earlier.apply_term(&term)))
            }
        }
    }

    /// The const parameter a type node spells: `N` in `Delay<u8, N>` reads as a type name.
    pub(super) fn const_parameter_named_by(&self, ty: TypeId) -> Option<DeclId> {
        let TypeKind::Path(path) = &self.design.type_node(ty).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
            Some(Resolution::Decl(decl)) if self.is_const_parameter(decl) => Some(decl),
            _ => None,
        }
    }

    /// Whether `decl` is a const generic parameter.
    fn is_const_parameter(&self, decl: DeclId) -> bool {
        matches!(self.design.decl_data(decl).kind, DeclKind::GenericParam(crate::pom::GenericParamData::Const { .. }))
    }

    /// A const generic argument as the checker knows it (§3.8.5): an integer or `bool` literal, a
    /// literal with `-`, the name of a const parameter, or an expression over const parameters
    /// (`{ N + 1 }`) which the elaborator folds per instantiation; a constant expression with no
    /// parameter is folded now. Anything else is `E0188`.
    pub(super) fn const_term(&mut self, expr: ExprId, file: FileId) -> Option<ConstTerm> {
        let node = self.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Lit(lit) => match &lit.kind {
                LitKind::Bool(value) => return Some(ConstTerm::Bool(*value)),
                LitKind::Int(_) => {
                    if let Some(value) = self.fold_integer(expr, u8::MAX) {
                        return i128::try_from(value).ok().map(ConstTerm::Value);
                    }
                }
                _ => {}
            },
            ExprKind::Unary { op: UnOp::Neg, expr: inner } => {
                if let Some(ConstTerm::Value(value)) = self.const_term(*inner, file) {
                    return Some(ConstTerm::Value(-value));
                }
            }
            ExprKind::Paren(inner) => return self.const_term(*inner, file),
            // `{ N + 1 }`: a block of one expression is that expression (§3.8.5)
            ExprKind::Block(block) if let [only] = block.stmts.as_slice() && let StmtKind::Expr { expr: inner, semi: false } = &self.design.stmt_node(*only).kind => return self.const_term(*inner, file),
            ExprKind::Path(_) => {
                if let Some(parameter) = self.generic_parameter_of(expr) {
                    return Some(ConstTerm::Param(parameter));
                }
            }
            _ if self.mentions_generic_parameter(expr) => return Some(ConstTerm::Expr(expr)),
            _ => {}
        }
        if let Some(value) = self.fold_integer(expr, u8::MAX) {
            return i128::try_from(value).ok().map(ConstTerm::Value);
        }
        let what = self.expression_name(expr);
        self.coded(file, "E0188", node.span, format!("a generic argument that is a constant must be one the checker folds, and {what} is not (§3.8.5)"), "not a constant");
        None
    }

    /// The generic parameter the const expression `expr` is the bare name of, if it is one.
    pub(super) fn generic_parameter_of(&self, expr: ExprId) -> Option<DeclId> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::GenericParam(_)) => Some(decl),
                _ => None,
            },
            ExprKind::Paren(inner) => self.generic_parameter_of(*inner),
            _ => None,
        }
    }

    /// Whether the const expression `expr` names a generic parameter anywhere in it.
    pub(super) fn mentions_generic_parameter(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(_) => self.generic_parameter_of(expr).is_some(),
            ExprKind::Paren(inner) | ExprKind::Unary { expr: inner, .. } | ExprKind::Cast { expr: inner, .. } => self.mentions_generic_parameter(*inner),
            ExprKind::Binary { lhs, rhs, .. } => self.mentions_generic_parameter(*lhs) || self.mentions_generic_parameter(*rhs),
            ExprKind::Block(block) => block.stmts.iter().any(|stmt| match &self.design.stmt_node(*stmt).kind {
                StmtKind::Expr { expr, .. } => self.mentions_generic_parameter(*expr),
                _ => false,
            }),
            _ => false,
        }
    }

    /// Whether the const expression `expr` reads a param of a module anywhere in it (`[Bit; n]`, `[Bit; n + 1]`): an
    /// array length the elaborator folds per body variant, which makes the param structural (§8.1.2).
    pub(super) fn reads_module_param(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => matches!(
                self.design.path_node(*path).segments.last().map(|segment| segment.res),
                Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::Param(_))
            ),
            ExprKind::Paren(inner) | ExprKind::Unary { expr: inner, .. } | ExprKind::Cast { expr: inner, .. } => self.reads_module_param(*inner),
            ExprKind::Binary { lhs, rhs, .. } => self.reads_module_param(*lhs) || self.reads_module_param(*rhs),
            _ => false,
        }
    }
}
