//! The types of the M1 subset, as data.

use crate::pom::{DeclId, Design};

/// A type of the subset. There is no type variable: what is outside the subset is refused by
/// the checker, never given a `Ty`.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}

impl Ty {
    /// Whether this is `UInt<N>` or `Int<N>`.
    pub fn is_integer(&self) -> bool {
        matches!(self, Ty::UInt(_) | Ty::Int(_))
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
            Ty::Tuple(items) => items.iter().all(Ty::is_value),
            Ty::Fn(..) => true,
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
                Some(n) => format!("enum `{}`", n.text),
                None => "enum".to_string(),
            },
            Ty::Option(inner) => format!("Option<{}>", inner.name(design)),
            Ty::Tuple(items) => {
                let names: Vec<String> = items.iter().map(|t| t.name(design)).collect();
                if items.len() == 1 { format!("({},)", names[0]) } else { format!("({})", names.join(", ")) }
            }
            Ty::Carrier { nature, value } => {
                let nature = design.decl_data(*nature).name.as_ref().map_or("Bit", |n| n.text.as_str());
                format!("{nature}<{}>", value.name(design))
            }
            Ty::Event => "Event".to_string(),
            Ty::Str => "str".to_string(),
            Ty::Never => "!".to_string(),
            Ty::Struct(decl) => match &design.decl_data(*decl).name {
                Some(n) => format!("struct `{}`", n.text),
                None => "struct".to_string(),
            },
            Ty::Array(element, length) => format!("[{}; {length}]", element.name(design)),
            Ty::Module(module) => match &design.decl_data(*module).name {
                Some(n) => format!("module `{}`", n.text),
                None => "module".to_string(),
            },
        }
    }
}
