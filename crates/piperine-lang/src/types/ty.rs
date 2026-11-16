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
}

impl Ty {
    /// Whether this is `UInt<N>` or `Int<N>`.
    pub fn is_integer(&self) -> bool {
        matches!(self, Ty::UInt(_) | Ty::Int(_))
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
            Ty::Module(module) => match &design.decl_data(*module).name {
                Some(n) => format!("module `{}`", n.text),
                None => "module".to_string(),
            },
        }
    }
}
