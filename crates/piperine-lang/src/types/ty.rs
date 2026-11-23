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
    /// `f32` (`Float<32>`): IEEE 754 binary32.
    F32,
    /// `f16` (`Float<16>`): IEEE 754 binary16.
    F16,
    /// `f128` (`Float<128>`): IEEE 754 binary128.
    F128,
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
    /// `UInt<N>` (`uN`), `N` in 1..=2^32 - 1.
    UInt(u32),
    /// `Int<N>` (`iN`), `N` in 1..=2^32 - 1.
    Int(u32),
    /// `UInt<N>` or `Int<N>` whose width names const parameters of the generic item being checked (`UInt<N>` in
    /// `impl<const N: usize> Logic<N>`): an integer of a width the instantiation gives. Substitution turns it into
    /// `UInt(n)` or `Int(n)` once the width is a number.
    GenericInteger { signed: bool, width: ConstTerm },
    /// An enum of unit variants deriving `Repr`, by its declaration.
    Enum(Nominal),
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
    Struct(Nominal),
    /// `[T; N]`: `N` values of one type (§1.2.5).
    Array(Box<Ty>, Length),
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

/// A struct or an enum by its declaration and the generic arguments it is written with: `Pair<f64>`
/// is `Nominal { decl: Pair, arguments: [f64] }`. The arguments of a type that is not generic are
/// empty, and two uses are one type exactly when their `Nominal`s are equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Nominal {
    /// The declaration of the struct or enum.
    pub decl: DeclId,
    /// The arguments for its generic parameters, in order; one for each parameter once the type
    /// is written out in full.
    pub arguments: Vec<GenericValue>,
}

impl Nominal {
    /// The generic arguments as written, `<f64, 4>`, or nothing for a type that has none.
    pub fn arguments_name(&self, design: &Design) -> String {
        if self.arguments.is_empty() {
            return String::new();
        }
        let names: Vec<String> = self.arguments.iter().map(|argument| argument.name(design)).collect();
        format!("<{}>", names.join(", "))
    }

    /// The type `decl` names with no generic argument.
    pub fn plain(decl: DeclId) -> Nominal {
        Nominal { decl, arguments: Vec::new() }
    }
}

impl Ty {
    /// Whether a value of this type may stand where `expected` is wanted: equal types, where an array
    /// length built from const parameters agrees with any other length until the item is instantiated
    /// ([`Length::fits`]); the arguments of a generic type are compared the same way.
    pub fn fits(&self, expected: &Ty) -> bool {
        match (self, expected) {
            (Ty::Array(found, found_length), Ty::Array(wanted, wanted_length)) => found.fits(wanted) && found_length.fits(wanted_length),
            (Ty::Option(found), Ty::Option(wanted)) => found.fits(wanted),
            (Ty::GenericInteger { signed: found, width: found_width }, Ty::GenericInteger { signed: wanted, width: wanted_width }) => found == wanted && Length::Generic(found_width.clone()).fits(&Length::Generic(wanted_width.clone())),
            (Ty::GenericInteger { signed, .. }, Ty::UInt(_)) | (Ty::UInt(_), Ty::GenericInteger { signed, .. }) if !signed => true,
            (Ty::GenericInteger { signed, .. }, Ty::Int(_)) | (Ty::Int(_), Ty::GenericInteger { signed, .. }) if *signed => true,
            (Ty::Tuple(found), Ty::Tuple(wanted)) => found.len() == wanted.len() && found.iter().zip(wanted).all(|(found, wanted)| found.fits(wanted)),
            (Ty::Fn(found, found_result), Ty::Fn(wanted, wanted_result)) => found.len() == wanted.len() && found.iter().zip(wanted).all(|(found, wanted)| found.fits(wanted)) && found_result.fits(wanted_result),
            (Ty::Struct(found), Ty::Struct(wanted)) | (Ty::Enum(found), Ty::Enum(wanted)) => {
                found.decl == wanted.decl
                    && found.arguments.len() == wanted.arguments.len()
                    && found.arguments.iter().zip(&wanted.arguments).all(|pair| match pair {
                        (GenericValue::Type(found), GenericValue::Type(wanted)) => found.fits(wanted),
                        (found, wanted) => found == wanted,
                    })
            }
            _ => self == expected,
        }
    }

    /// The pairs of lengths [`Ty::fits`] accepted without deciding them: where `self` and `expected`
    /// have arrays whose lengths are not the same number and not the same parameter, because one is
    /// built from const parameters. The instantiation compares each pair by value.
    pub fn deferred_lengths(&self, expected: &Ty) -> Vec<(Length, Length)> {
        match (self, expected) {
            (Ty::Array(found, found_length), Ty::Array(wanted, wanted_length)) => {
                let mut pairs = found.deferred_lengths(wanted);
                if found_length != wanted_length {
                    pairs.push((found_length.clone(), wanted_length.clone()));
                }
                pairs
            }
            (Ty::Option(found), Ty::Option(wanted)) => found.deferred_lengths(wanted),
            (Ty::Tuple(found), Ty::Tuple(wanted)) => found.iter().zip(wanted).flat_map(|(found, wanted)| found.deferred_lengths(wanted)).collect(),
            _ => Vec::new(),
        }
    }

    /// Every instance of a generic struct or enum the type holds, each with all its arguments given (no parameter in
    /// them), an instance's own arguments before it: what the elaborator gives an arena entry (GN-06).
    pub fn generic_instances(&self, out: &mut Vec<Nominal>) {
        match self {
            Ty::Struct(nominal) | Ty::Enum(nominal) => {
                nominal.arguments.iter().for_each(|argument| {
                    if let GenericValue::Type(ty) = argument {
                        ty.generic_instances(out);
                    }
                });
                if !nominal.arguments.is_empty() && !self.has_param() && !out.contains(nominal) {
                    out.push(nominal.clone());
                }
            }
            Ty::Option(inner) | Ty::Array(inner, _) => inner.generic_instances(out),
            Ty::Tuple(items) => items.iter().for_each(|item| item.generic_instances(out)),
            Ty::Fn(params, ret) => {
                params.iter().for_each(|param| param.generic_instances(out));
                ret.generic_instances(out);
            }
            _ => {}
        }
    }

    /// Whether a type parameter is mentioned anywhere in the type, an argument and a length included.
    pub fn has_param(&self) -> bool {
        match self {
            Ty::Param(_) | Ty::GenericInteger { .. } => true,
            Ty::Fn(params, ret) => params.iter().any(Ty::has_param) || ret.has_param(),
            Ty::Option(inner) | Ty::Range { element: inner, .. } | Ty::Iterator { element: inner, .. } | Ty::Carrier { value: inner, .. } => inner.has_param(),
            Ty::Tuple(items) => items.iter().any(Ty::has_param),
            Ty::Array(element, length) => element.has_param() || matches!(length, Length::Generic(_)),
            Ty::Struct(nominal) | Ty::Enum(nominal) => nominal.arguments.iter().any(|argument| match argument {
                GenericValue::Type(ty) => ty.has_param(),
                GenericValue::Const(ConstTerm::Param(_) | ConstTerm::Width(_) | ConstTerm::Expr(_)) => true,
                _ => false,
            }),
            _ => false,
        }
    }

    /// The struct `decl`, not generic.
    pub fn structure(decl: DeclId) -> Ty {
        Ty::Struct(Nominal::plain(decl))
    }

    /// The enum `decl`, not generic.
    pub fn enumeration(decl: DeclId) -> Ty {
        Ty::Enum(Nominal::plain(decl))
    }
}

/// The number of elements of an array type (§1.2.5): a number, or a const parameter or expression of
/// the generic item being checked, which the elaborator folds per instantiation (§3.8.3). A
/// generic length is legal only inside a generic item (D14).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Length {
    /// A length known now.
    Fixed(u64),
    /// A length that names const parameters.
    Generic(ConstTerm),
}

impl Length {
    /// Whether a value of length `self` may stand where one of length `expected` is wanted. Two
    /// numbers agree when equal and a parameter with itself; every other pair involves a length
    /// built from const parameters, which is compared by value when the item is instantiated
    /// (Q1 (c), §3.8.9), so the definition accepts it.
    pub fn fits(&self, expected: &Length) -> bool {
        match (self, expected) {
            (Length::Fixed(found), Length::Fixed(wanted)) => found == wanted,
            (Length::Generic(ConstTerm::Param(found)), Length::Generic(ConstTerm::Param(wanted))) => found == wanted,
            (Length::Generic(ConstTerm::Width(found)), Length::Generic(ConstTerm::Width(wanted))) => found == wanted,
            _ => true,
        }
    }

    /// The length as a number, `None` for a generic one.
    pub fn fixed(&self) -> Option<u64> {
        match self {
            Length::Fixed(n) => Some(*n),
            Length::Generic(_) => None,
        }
    }

    /// The length as a model author writes it, for messages.
    pub fn name(&self, design: &Design) -> String {
        match self {
            Length::Fixed(n) => n.to_string(),
            Length::Generic(ConstTerm::Value(n)) => n.to_string(),
            Length::Generic(ConstTerm::Bool(b)) => b.to_string(),
            Length::Generic(ConstTerm::Param(param)) => design.decl_data(*param).name.as_ref().map_or_else(|| "N".to_string(), |n| n.text.clone()),
            Length::Generic(ConstTerm::Expr(_)) => "{..}".to_string(),
            Length::Generic(ConstTerm::Width(param)) => format!("{}::WIDTH", design.decl_data(*param).name.as_ref().map_or("T", |n| n.text.as_str())),
        }
    }
}

impl Ty {
    /// The array of `length` elements of `element`.
    pub fn array(element: Ty, length: u64) -> Ty {
        Ty::Array(Box::new(element), Length::Fixed(length))
    }
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

impl GenericValue {
    /// The argument as a model author writes it, for messages.
    pub fn name(&self, design: &Design) -> String {
        match self {
            GenericValue::Type(ty) => ty.source_name(design),
            GenericValue::Const(term) => Length::Generic(term.clone()).name(design),
            GenericValue::Function(function) => design.decl_data(*function).name.as_ref().map_or_else(|| "fn".to_string(), |n| n.text.clone()),
            GenericValue::Closure(_) => "closure".to_string(),
        }
    }
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
    /// `T::WIDTH` of a type parameter bound by `Repr` (§2): the number of levels of whatever `T` is
    /// instantiated with.
    Width(DeclId),
}

impl Ty {
    /// Whether this is a carrier, a node, or an array of them: what an array of carriers is made of (CV-23).
    pub fn is_carrier_element(&self) -> bool {
        match self {
            Ty::Carrier { .. } | Ty::Node(_) => true,
            Ty::Array(element, _) => element.is_carrier_element(),
            _ => false,
        }
    }

    /// Whether this is an array one of whose levels has the fixed length 0: it holds no element.
    pub fn has_fixed_zero_length(&self) -> bool {
        match self {
            Ty::Array(element, length) => length.fixed() == Some(0) || element.has_fixed_zero_length(),
            _ => false,
        }
    }

    /// The type an array of arrays of `T` is made of: `T`; any other type is itself.
    pub fn innermost(&self) -> &Ty {
        match self {
            Ty::Array(element, _) => element.innermost(),
            other => other,
        }
    }

    /// The value type a read of this carrier type gives: a carrier's value, an array of carriers' array of values, a node
    /// array's itself; any other type is its own read.
    pub fn read_value(&self) -> Ty {
        match self {
            Ty::Carrier { value, .. } => (**value).clone(),
            Ty::Array(element, length) if matches!(**element, Ty::Carrier { .. }) || matches!(**element, Ty::Array(..)) && element.is_carrier_element() => {
                Ty::Array(Box::new(element.read_value()), length.clone())
            }
            other => other.clone(),
        }
    }

    /// Whether this is `UInt<N>` or `Int<N>`.
    pub fn is_integer(&self) -> bool {
        matches!(self, Ty::UInt(_) | Ty::Int(_) | Ty::GenericInteger { .. })
    }

    /// The float type of `width` bits, `None` unless `width` is 16, 32, 64 or 128.
    pub fn float(width: u32) -> Option<Ty> {
        match width {
            16 => Some(Ty::F16),
            32 => Some(Ty::F32),
            64 => Some(Ty::F64),
            128 => Some(Ty::F128),
            _ => None,
        }
    }

    /// Whether this is a float of any width: `f64`, `f32`, `f16` or `f128`.
    pub fn is_float(&self) -> bool {
        matches!(self, Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128)
    }

    /// Whether this is a float of a width other than `f64`.
    pub fn is_other_float(&self) -> bool {
        matches!(self, Ty::F32 | Ty::F16 | Ty::F128)
    }

    /// Whether this is a float of any width or an integer type: what `as` converts among (§1.4.5).
    pub fn is_numeric(&self) -> bool {
        self.is_float() || self.is_integer()
    }

    /// Whether this is a type made of parts: a struct, an enum, a tuple, an array or an `Option`.
    pub fn is_composite(&self) -> bool {
        matches!(self, Ty::Struct(_) | Ty::Enum(_) | Ty::Tuple(_) | Ty::Array(..) | Ty::Option(_))
    }

    /// Whether the type implements `Repr` (part I §2): the types a carrier may hold.
    pub fn has_repr(&self) -> bool {
        matches!(self, Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::GenericInteger { .. } | Ty::Enum(_))
    }

    /// Whether a value of this type can be bound, stored and passed (§1.2): a type a carrier, a
    /// node, a module or an event is not.
    pub fn is_value(&self) -> bool {
        match self {
            Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 | Ty::Bool | Ty::Unit | Ty::UInt(_) | Ty::Int(_) | Ty::GenericInteger { .. } | Ty::Enum(_) | Ty::Struct(_) | Ty::Str => true,
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
            // a literal's value is a `u128`: a type wider than 128 bits holds every literal
            Ty::UInt(n) => Some(u128::MAX >> (128 - n.min(128))),
            Ty::Int(n) if n > 128 => Some(u128::MAX),
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
            Ty::F32 => Some(32),
            Ty::F16 => Some(16),
            Ty::F128 => Some(128),
            Ty::UInt(n) | Ty::Int(n) => Some(*n),
            Ty::Enum(Nominal { decl, .. }) => {
                let variants = design.decl_data(*decl).children.iter().filter(|c| matches!(design.decl_data(**c).kind, crate::pom::DeclKind::Variant(_))).count();
                Some(usize::BITS - variants.saturating_sub(1).leading_zeros())
            }
            _ => None,
        }
    }

    /// The type as a path is written where it is an argument: a struct or an enum bare, with its
    /// own arguments (`Pair<f64>`); any other type as [`Ty::name`] gives it.
    pub fn source_name(&self, design: &Design) -> String {
        match self {
            Ty::Struct(nominal) | Ty::Enum(nominal) => match &design.decl_data(nominal.decl).name {
                Some(n) => format!("{}{}", n.text, nominal.arguments_name(design)),
                None => self.name(design),
            },
            other => other.name(design),
        }
    }

    /// The type as a model author writes it, for messages.
    pub fn name(&self, design: &Design) -> String {
        match self {
            Ty::F64 => "f64".to_string(),
            Ty::F32 => "f32".to_string(),
            Ty::F16 => "f16".to_string(),
            Ty::F128 => "f128".to_string(),
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
            Ty::GenericInteger { signed, width } => format!("{}<{}>", if *signed { "Int" } else { "UInt" }, Length::Generic(width.clone()).name(design)),
            Ty::Enum(_) => self.source_name(design),
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
            Ty::Struct(nominal) => match &design.decl_data(nominal.decl).name {
                Some(_) => format!("struct `{}`", self.source_name(design)),
                None => "struct".to_string(),
            },
            Ty::Array(element, length) => format!("[{}; {}]", element.name(design), length.name(design)),
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
