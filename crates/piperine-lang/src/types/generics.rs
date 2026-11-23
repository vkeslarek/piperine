//! The generic parameter list of an item as data (part I §3.8, GN-01, GN-06): each parameter's
//! kind, its bounds and its default, read from the POM, and the rules of a list the checker holds
//! it to: defaults come last (§3.8.4), a const parameter is an integer or a `bool` (§3.8.3), a
//! bound names a trait (§3.8.6, `E0161`), and a use writes as many arguments as the list takes
//! (§3.8.7, `E0187`).

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, Design, ExprId, FileId, GenericArgsNode, GenericParamData, PathId, Resolution, TypeId, TypeKind};

use super::check::Checker;

/// What a generic parameter stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterKind {
    /// A value type (§3.8.2).
    Type,
    /// A nature: a type parameter bound by `Nature` (§3.8.2, §4 rule 11).
    Nature,
    /// A constant of the integer type or of `bool` that `ConstType` names (§3.8.3).
    Const(ConstType),
}

/// The type of a const parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstType {
    /// An integer type, by its spelling (`u32`, `usize`, `i8`).
    Integer(String),
    /// `bool`.
    Bool,
}

/// One bound of a type parameter (§3.8.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    /// A trait of the design.
    Trait(DeclId),
    /// A trait of Appendix H.8, by its spelling (`PartialEq`, `Default`).
    Library(&'static str),
    /// `Repr` (§2).
    Repr,
    /// `Nature` (§4).
    Nature,
    /// `Fn(A, B) -> R`: the inputs and the output as written, lowered where a callable is checked.
    Callable {
        /// The argument types.
        inputs: Vec<TypeId>,
        /// The result type; `None` for a callable that returns `()`.
        output: Option<TypeId>,
    },
}

/// The default of a parameter, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterDefault {
    /// `T = bool`.
    Type(TypeId),
    /// `const N: u32 = 4`: a literal or a block.
    Const(ExprId),
}

/// One parameter of a generic list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParameter {
    /// The declaration of the parameter, which a `Ty::Param` names.
    pub decl: DeclId,
    /// The name as written.
    pub name: String,
    /// What the parameter stands for.
    pub kind: ParameterKind,
    /// The bounds of a type parameter; empty for a const parameter.
    pub bounds: Vec<Bound>,
    /// The default, when the parameter has one.
    pub default: Option<ParameterDefault>,
}

/// The generic parameter list of one item, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericSignature {
    parameters: Vec<GenericParameter>,
}

/// The H.8 traits a bound may name, by spelling.
const LIBRARY_TRAITS: [&str; 7] = ["PartialEq", "Eq", "PartialOrd", "Ord", "Default", "Debug", "Hash"];

impl GenericSignature {
    /// The list of `item` as written; empty for an item with no `<..>`. Reads the POM only: the
    /// rules of the list are held by [`Checker::generic_signature`].
    pub fn of(design: &Design, item: DeclId) -> GenericSignature {
        let parameters = design
            .decl_data(item)
            .children
            .iter()
            .filter_map(|child| {
                let data = design.decl_data(*child);
                let DeclKind::GenericParam(parameter) = &data.kind else { return None };
                let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
                Some(match parameter {
                    GenericParamData::Type { bounds, default } => {
                        let bounds: Vec<Bound> = bounds.iter().filter_map(|path| Self::bound(design, *path)).collect();
                        let kind = if bounds.contains(&Bound::Nature) { ParameterKind::Nature } else { ParameterKind::Type };
                        GenericParameter { decl: *child, name, kind, bounds, default: default.map(ParameterDefault::Type) }
                    }
                    GenericParamData::Const { ty, default } => {
                        let kind = Self::const_type(design, *ty).map_or(ParameterKind::Const(ConstType::Integer(String::new())), ParameterKind::Const);
                        GenericParameter { decl: *child, name, kind, bounds: Vec::new(), default: default.map(ParameterDefault::Const) }
                    }
                })
            })
            .collect();
        GenericSignature { parameters }
    }

    /// The parameters, in source order.
    pub fn parameters(&self) -> &[GenericParameter] {
        &self.parameters
    }

    /// How many arguments a use must write at least: the parameters without a default.
    pub fn required(&self) -> usize {
        self.parameters.iter().filter(|parameter| parameter.default.is_none()).count()
    }

    /// The bound a path names, `None` for a name that is no trait (`Checker::generic_signature`
    /// reports it).
    fn bound(design: &Design, path: PathId) -> Option<Bound> {
        let segment = design.path_node(path).segments.last()?;
        match segment.res {
            Resolution::Builtin(builtin) => match builtin.info().name {
                "Repr" => Some(Bound::Repr),
                "Nature" => Some(Bound::Nature),
                "Fn" => match &segment.args {
                    Some(GenericArgsNode::Paren { inputs, output, .. }) => Some(Bound::Callable { inputs: inputs.clone(), output: *output }),
                    _ => Some(Bound::Callable { inputs: Vec::new(), output: None }),
                },
                name => LIBRARY_TRAITS.iter().find(|library| **library == name).map(|library| Bound::Library(library)),
            },
            Resolution::Decl(decl) if matches!(design.decl_data(decl).kind, DeclKind::Trait(_)) => Some(Bound::Trait(decl)),
            _ => None,
        }
    }

    /// The type of a const parameter, `None` when it is neither an integer type nor `bool`.
    fn const_type(design: &Design, ty: TypeId) -> Option<ConstType> {
        let TypeKind::Path(path) = &design.type_node(ty).kind else { return None };
        let spelled = &design.path_node(*path).segments.last()?.name.text;
        let integer = |digits: &str| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
        match spelled.as_str() {
            "bool" => Some(ConstType::Bool),
            "usize" | "isize" => Some(ConstType::Integer(spelled.clone())),
            other if (other.starts_with('u') || other.starts_with('i')) && integer(&other[1..]) => Some(ConstType::Integer(spelled.clone())),
            _ => None,
        }
    }
}

impl Checker<'_> {
    /// The generic parameter list of `item`, held to the rules of §3.8: a parameter with a default
    /// is never followed by one without (§3.8.4), a const parameter is an integer or a `bool`
    /// (§3.8.3), and a bound names a trait (§3.8.6, `E0161`); a list that breaks one is `E0187`.
    pub(super) fn generic_signature(&mut self, item: DeclId) -> GenericSignature {
        let signature = GenericSignature::of(self.design, item);
        let mut defaulted: Option<&GenericParameter> = None;
        for parameter in &signature.parameters {
            let data = self.design.decl_data(parameter.decl);
            let (file, span) = (data.file, data.span);
            match (&defaulted, &parameter.default) {
                (Some(earlier), None) => {
                    let message = format!("the generic parameter `{}` has no default but follows `{}`, which has one (§3.8.4)", parameter.name, earlier.name);
                    self.coded(file, "E0187", span, message, "a parameter without a default after one with");
                }
                (None, Some(_)) => defaulted = Some(parameter),
                _ => {}
            }
            self.parameter_rules(parameter, file);
        }
        signature
    }

    /// The per-parameter rules: a const parameter's type (§3.8.3) and the names of the bounds.
    fn parameter_rules(&mut self, parameter: &GenericParameter, file: FileId) {
        let DeclKind::GenericParam(written) = &self.design.decl_data(parameter.decl).kind else { return };
        match written.clone() {
            GenericParamData::Const { ty, .. } => {
                if GenericSignature::const_type(self.design, ty).is_none() {
                    let type_span = self.design.type_node(ty).span;
                    let message = format!("the const parameter `{}` has a type that is neither an integer nor `bool` (§3.8.3)", parameter.name);
                    self.coded(file, "E0187", type_span, message, "an integer type or `bool`");
                }
            }
            GenericParamData::Type { bounds, .. } => {
                for path in bounds {
                    if GenericSignature::bound(self.design, path).is_none() {
                        let node = self.design.path_node(path);
                        let (named, at) = (node.segments.last().map_or_else(String::new, |s| s.name.text.clone()), node.span);
                        self.coded(file, "E0161", at, format!("`{named}` is not a trait, so it cannot bound `{}` (§3.8.6)", parameter.name), "a trait");
                    }
                }
            }
        }
    }

    /// Whether a use of `item` writes `written` generic arguments where its list takes
    /// `required..=all` (§3.8.7); a wrong number is `E0187` at `span`, naming the item.
    pub(super) fn generic_argument_count(&mut self, item: DeclId, written: usize, file: FileId, span: Span) -> bool {
        let signature = GenericSignature::of(self.design, item);
        let (low, high) = (signature.required(), signature.parameters.len());
        if (low..=high).contains(&written) {
            return true;
        }
        let name = self.design.decl_data(item).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let takes = if low == high { format!("{high}") } else { format!("{low} to {high}") };
        self.coded(file, "E0187", span, format!("`{name}` takes {takes} generic arguments, found {written} (§3.8.7)"), "the wrong number of generic arguments");
        false
    }
}
