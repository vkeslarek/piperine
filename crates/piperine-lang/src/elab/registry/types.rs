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
}

impl TypeDefKind {
    pub fn name(&self) -> &str {
        match self {
            TypeDefKind::Primitive { name, .. } => name,
            TypeDefKind::Discipline(d) => &d.name,
            TypeDefKind::Enum(e) => &e.name,
            TypeDefKind::Bundle(b) => &b.name,
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
        }
    }

    pub fn as_bundle(&self) -> Option<&BundleDecl> {
        match self {
            TypeDefKind::Bundle(b) => Some(b),
            _ => None,
        }
    }
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
