//! The types of the M1 subset, as data.

use crate::pom::{DeclId, Design, ExprId};

/// A type of the subset. There is no inference variable: what is outside the subset is refused by
/// the checker, never given a `Ty`. The one open type is [`Ty::Param`], a generic parameter of the
/// item being checked: a generic body is checked once at its definition with each parameter
/// opaque, so it may use only what the parameter's bounds give (Q1 (c), part I §3 rule 9). `Ty`
/// is ordered so that an instantiation, a list of types and values, can key a map.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ty {
    F64,
    Bool,
    /// `()`.
    Unit,
    /// A node of the conservative nature declared by this `Nature` declaration.
    Node(DeclId),
    /// `gnd`: a node of any nature.
    AnyNode,
    /// `fn(A, B) -> R`.
    Fn(Vec<Ty>, Box<Ty>),
    /// A module, by its declaration.
    Module(DeclId),
    /// `UInt<N>` (`uN`), `N` in 1..=64.
    UInt(u8),
    /// `Int<N>` (`iN`), `N` in 1..=64.
    Int(u8),
    /// An enum of unit variants deriving `Repr`, by its declaration.
    Enum(DeclId),
    /// `Option<T>`.
    Option(Box<Ty>),
    /// `(A, B, ..)` (§1.2.4), and `(A,)` of one field.
    Tuple(Vec<Ty>),
    /// A discrete carrier of the nature `Bit<value>` (a `signal` or a port). Reading one gives
    /// its `value`; the carrier type itself is what a declaration has.
    Carrier { nature: DeclId, value: Box<Ty> },
    /// `Event` (part IV §13): something that fires at instants. It has no `Repr`: it is never
    /// carried, committed or stored.
    Event,
    /// `str` (§1.2.9), the type of a string: a value type with no `Repr`, so it is never carried,
    /// committed or derived. A string is text known at elaboration, kept on the site that reads it.
    Str,
    /// The type of an expression that never completes: `return`, `break`, `continue` and a
    /// `loop` with no `break` (§1.5.16). It fits any required type.
    Never,
    /// A `struct` of named, positional or no fields, by its declaration (§1.3.1): a nominal type.
    Struct(DeclId),
    /// `[T; N]`: `N` values of one type (§1.2.5).
    Array(Box<Ty>, u64),
    /// The value of `a..b`, `a..=b`, `a..`, `..b` or `..=b` over one integer or float type (§1.5.11,
    /// H.6). It cannot be written. `iterable` is true for an integer range with both bounds: only
    /// that range is an iterator (H.6 rule 3).
    Range { element: Box<Ty>, iterable: bool },
    /// The iterator of `iter()`, a range or an adaptor (H.7). It cannot be written. `reversible`
    /// is true when `rev` applies (H.7 rule 3): `iter()` of an array, an integer range, and the
    /// `map`, `enumerate`, `zip` and `rev` of those.
    Iterator { element: Box<Ty>, reversible: bool },
    /// A type or nature parameter of the generic item being checked (`T` of `fn f<T: Repr>`),
    /// by its declaration: opaque, it offers what its bounds give and nothing else.
    Param(DeclId),
}

/// What a generic parameter is given at a use or an instantiation (§3.8): a type for a type or
/// nature parameter, a const for a const parameter, a function name or a closure for a parameter
/// bounded by `Fn(A) -> R`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GenericValue {
    /// A type, or a nature as the type of a node.
    Type(Ty),
    /// A value of a const parameter.
    Const(ConstTerm),
    /// A function named for an `Fn` bound.
    Function(DeclId),
    /// A closure written for an `Fn` bound, by its expression; every closure is its own type.
    Closure(ExprId),
}

/// A const generic argument as the checker knows it: folded where it can be, else a name or an
/// expression the elaborator folds per instantiation (§3.8.3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConstTerm {
    /// An integer value.
    Value(i128),
    /// A `bool` value.
    Bool(bool),
    /// A const parameter of the enclosing generic item, by its declaration.
    Param(DeclId),
    /// An expression over const parameters, folded when the item is instantiated.
    Expr(ExprId),
}

impl Ty {
    /// Whether this is `UInt<N>` or `Int<N>`.
    pub fn is_integer(&self) -> bool {
        matches!(self, Ty::UInt(_) | Ty::Int(_))
    }

    /// Whether this is a type made of parts: a struct, an enum, a tuple, an array or an `Option`.
    pub fn is_composite(&self) -> bool {
        matches!(self, Ty::Struct(_) | Ty::Enum(_) | Ty::Tuple(_) | Ty::Array(..) | Ty::Option(_))
    }

    /// Whether the type implements `Repr` (part I §2): the types a carrier may hold.
    pub fn has_repr(&self) -> bool {
        matches!(self, Ty::F64 | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::Enum(_))
    }

    /// Whether a value of this type can be bound, stored and passed (§1.2): a type a carrier, a
    /// node, a module or an event is not.
    pub fn is_value(&self) -> bool {
        match self {
            Ty::F64 | Ty::Bool | Ty::Unit | Ty::UInt(_) | Ty::Int(_) | Ty::Enum(_) | Ty::Struct(_) | Ty::Str => true,
            Ty::Option(inner) | Ty::Array(inner, _) => inner.is_value(),
            Ty::Range { element, .. } | Ty::Iterator { element, .. } => element.is_value(),
            Ty::Tuple(items) => items.iter().all(Ty::is_value),
            Ty::Fn(..) | Ty::Param(_) => true,
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::Carrier { .. } | Ty::Event | Ty::Never => false,
        }
    }

    /// The largest magnitude an integer literal of this type may have; with `negated`, the
    /// literal is the operand of a unary `-` (`-128` is an `i8`). `None` for a type that is not
    /// an integer type.
    pub fn literal_max(&self, negated: bool) -> Option<u128> {
        match *self {
            Ty::UInt(n) => Some(u128::MAX >> (128 - u32::from(n))),
            Ty::Int(n) if negated => Some(1u128 << (n - 1)),
            Ty::Int(n) => Some((1u128 << (n - 1)) - 1),
            _ => None,
        }
    }

    /// The number of levels of the type's `Repr` (§2): 1 for `bool`, `N` for `uN` and `iN`, 64
    /// for `f64`, the tag width `ceil(log2(V))` for an enum of `V` unit variants (§2 rule 5).
    /// `None` for a type without `Repr`.
    pub fn levels(&self, design: &Design) -> Option<u32> {
        match self {
            Ty::Bool => Some(1),
            Ty::F64 => Some(64),
            Ty::UInt(n) | Ty::Int(n) => Some(u32::from(*n)),
            Ty::Enum(decl) => {
                let variants = design.decl_data(*decl).children.iter().filter(|c| matches!(design.decl_data(**c).kind, crate::pom::DeclKind::Variant(_))).count();
                Some(usize::BITS - variants.saturating_sub(1).leading_zeros())
            }
            _ => None,
        }
    }

    /// The type as a model author writes it, for messages.
    pub fn name(&self, design: &Design) -> String {
        match self {
            Ty::F64 => "f64".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Unit => "()".to_string(),
            Ty::Node(nature) => match &design.decl_data(*nature).name {
                Some(n) => format!("node of nature `{}`", n.text),
                None => "node".to_string(),
            },
            Ty::AnyNode => "node".to_string(),
            Ty::Fn(params, ret) => {
                let params: Vec<String> = params.iter().map(|p| p.name(design)).collect();
                format!("fn({}) -> {}", params.join(", "), ret.name(design))
            }
            Ty::UInt(n) => format!("u{n}"),
            Ty::Int(n) => format!("i{n}"),
            Ty::Enum(e) => match &design.decl_data(*e).name {
                Some(n) => n.text.clone(),
                None => "enum".to_string(),
            },
            Ty::Option(inner) => format!("Option<{}>", inner.name(design)),
            Ty::Tuple(items) => {
                let names: Vec<String> = items.iter().map(|t| t.name(design)).collect();
                if items.len() == 1 { format!("({},)", names[0]) } else { format!("({})", names.join(", ")) }
            }
            Ty::Carrier { nature, value } => {
                let data = design.decl_data(*nature);
                let nature = data.name.as_ref().map_or("Bit", |n| n.text.as_str());
                // A nature that takes no type argument is written bare, as its author wrote it.
                match data.generics_span {
                    Some(_) => format!("{nature}<{}>", value.name(design)),
                    None => nature.to_string(),
                }
            }
            Ty::Event => "Event".to_string(),
            Ty::Str => "str".to_string(),
            Ty::Never => "!".to_string(),
            Ty::Struct(decl) => match &design.decl_data(*decl).name {
                Some(n) => format!("struct `{}`", n.text),
                None => "struct".to_string(),
            },
            Ty::Array(element, length) => format!("[{}; {length}]", element.name(design)),
            Ty::Range { element, .. } => format!("Range<{}>", element.name(design)),
            Ty::Iterator { element, .. } => format!("Iterator<{}>", element.name(design)),
            Ty::Module(module) => match &design.decl_data(*module).name {
                Some(n) => format!("module `{}`", n.text),
                None => "module".to_string(),
            },
            Ty::Param(param) => match &design.decl_data(*param).name {
                Some(n) => n.text.clone(),
                None => "type parameter".to_string(),
            },
        }
    }
}
