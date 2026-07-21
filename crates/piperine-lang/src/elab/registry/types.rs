use crate::parse::ast::{DisciplineDecl, EnumDecl, BundleDecl, Type as AstType};
use crate::pom::{TypeRef, NetType, ValueType, ElabError, ElabErrorKind};
use crate::elab::const_eval::ConstEnv;
use std::collections::HashMap;

/// A registered type definition — one of the four kinds the type
/// checker can resolve. Replaces the former `TypeDef` trait with its
/// `as_discipline`/`as_enum`/`as_bundle` downcast methods.
pub enum TypeDefKind {
    Primitive { name: String, val_type: ValueType },
    Discipline(DisciplineDecl),
    Enum(EnumDecl),
    Bundle(BundleDecl),
    /// An `extern type Name;` declaration (SPEC declared-language-surface
    /// DLS-08) — a type whose shape is a native Rust registry entry, but
    /// whose *name* has a textual `decl_span` an LSP can resolve to. Types
    /// are not overloadable (one type per name), unlike callables.
    Extern { name: String, decl_span: Option<miette::SourceSpan> },
}

impl TypeDefKind {
    pub fn name(&self) -> &str {
        match self {
            TypeDefKind::Primitive { name, .. } => name,
            TypeDefKind::Discipline(d) => &d.name,
            TypeDefKind::Enum(e) => &e.name,
            TypeDefKind::Bundle(b) => &b.name,
            TypeDefKind::Extern { name, .. } => name,
        }
    }

    pub fn resolve(&self, _ty: &AstType, _env: &ConstEnv, _type_subst: &HashMap<String, String>) -> Result<TypeRef, ElabError> {
        match self {
            TypeDefKind::Primitive { val_type, .. } => Ok(TypeRef::Value(val_type.clone())),
            TypeDefKind::Discipline(d) => Ok(TypeRef::Net(NetType::Discipline(d.name.clone()))),
            TypeDefKind::Enum(e) => Ok(TypeRef::Value(ValueType::Enum(e.name.clone()))),
            TypeDefKind::Bundle(_) => Err(ElabError::from(ElabErrorKind::Other(
                "Bundles are flattened and do not resolve to a simple TypeRef".into()
            ))),
            // `extern type` dispatches to the native value-type binding
            // table below (declared-language-surface T16, DLS-17) — an
            // `extern type` with no matching native entry is a distinct,
            // DLS-05-style "extern declared but no registry binding"
            // failure, same class as `extern fn`'s missing-`MATH_FNS`-entry
            // case.
            TypeDefKind::Extern { name, .. } => match native_extern_val_type(name) {
                Some(val_type) => Ok(TypeRef::Value(val_type)),
                None => Err(ElabError::from(ElabErrorKind::Other(format!(
                    "extern type `{name}` has no native value-type binding registered yet"
                )))),
            },
        }
    }

    pub fn as_bundle(&self) -> Option<&BundleDecl> {
        match self {
            TypeDefKind::Bundle(b) => Some(b),
            _ => None,
        }
    }
}

/// Native `ValueType` binding for each `extern type`-declared primitive
/// name — the implementation table `TypeDefKind::Extern::resolve` dispatches
/// to, mirroring `math.rs`'s `MATH_FNS` pattern for `extern fn` (declared-
/// language-surface T16, DLS-17). The seven entries are the same primitives
/// `ElabContext::new()` used to hardcode directly as `TypeDefKind::Primitive`
/// before this migration; `headers/types.phdl` is now their sole textual
/// declaration.
fn native_extern_val_type(name: &str) -> Option<ValueType> {
    Some(match name {
        "Real" => ValueType::Real,
        "Natural" => ValueType::Natural,
        "Integer" => ValueType::Integer,
        "Complex" => ValueType::Complex,
        "Boolean" => ValueType::Boolean,
        "Quad" => ValueType::Quad,
        "String" => ValueType::Str,
        _ => return None,
    })
}

pub struct TypeRegistry {
    types: HashMap<String, TypeDefKind>,
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self { types: HashMap::new() }
    }

    pub fn register(&mut self, kind: TypeDefKind) {
        self.types.insert(kind.name().to_string(), kind);
    }

    pub fn lookup(&self, name: &str) -> Option<&TypeDefKind> {
        self.types.get(name)
    }
}
