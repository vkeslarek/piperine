use crate::parse::ast::{DisciplineDecl, EnumDecl, BundleDecl, Type as AstType};
use crate::pom::{TypeRef, ValueType, NetType, ElabError};
use crate::elab::const_eval::ConstEnv;
use std::collections::HashMap;

pub trait TypeDef: Send + Sync {
    fn name(&self) -> &str;
    fn as_discipline(&self) -> Option<&DisciplineDecl> { None }
    fn as_enum(&self) -> Option<&EnumDecl> { None }
    fn as_bundle(&self) -> Option<&BundleDecl> { None }
    fn resolve(&self, ty: &AstType, env: &ConstEnv, type_subst: &HashMap<String, String>) -> Result<TypeRef, ElabError>;
}

pub struct TypeRegistry {
    types: HashMap<String, Box<dyn TypeDef>>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self { types: HashMap::new() }
    }

    pub fn register<T: TypeDef + 'static>(&mut self, def: T) {
        self.types.insert(def.name().to_string(), Box::new(def));
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn TypeDef> {
        self.types.get(name).map(|b| b.as_ref())
    }
}
