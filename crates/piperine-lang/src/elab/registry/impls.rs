use crate::parse::ast::{DisciplineDecl, EnumDecl, BundleDecl, ModuleDeclaration, FnDecl, Type as AstType};
use crate::pom::{TypeRef, NetType, ValueType, ElabError, ElabErrorKind, Module};
use crate::elab::const_eval::ConstEnv;
use super::types::TypeDef;
use super::components::ComponentDef;
use super::callables::CallableDef;
use std::collections::HashMap;

// Primitive Type Def
pub struct PrimitiveTypeDef {
    pub name: String,
    pub val_type: ValueType,
}

impl TypeDef for PrimitiveTypeDef {
    fn name(&self) -> &str { &self.name }
    fn resolve(&self, _ty: &AstType, _env: &ConstEnv, _type_subst: &HashMap<String, String>) -> Result<TypeRef, ElabError> {
        Ok(TypeRef::Value(self.val_type.clone()))
    }
}

impl TypeDef for DisciplineDecl {
    fn name(&self) -> &str { &self.name }
    fn as_discipline(&self) -> Option<&DisciplineDecl> { Some(self) }
    fn resolve(&self, _ty: &AstType, _env: &ConstEnv, _type_subst: &HashMap<String, String>) -> Result<TypeRef, ElabError> {
        Ok(TypeRef::Net(NetType::Discipline(self.name.clone())))
    }
}

impl TypeDef for EnumDecl {
    fn name(&self) -> &str { &self.name }
    fn as_enum(&self) -> Option<&EnumDecl> { Some(self) }
    fn resolve(&self, _ty: &AstType, _env: &ConstEnv, _type_subst: &HashMap<String, String>) -> Result<TypeRef, ElabError> {
        Ok(TypeRef::Value(ValueType::Enum(self.name.clone())))
    }
}

impl TypeDef for BundleDecl {
    fn name(&self) -> &str { &self.name }
    fn as_bundle(&self) -> Option<&BundleDecl> { Some(self) }
    fn resolve(&self, _ty: &AstType, _env: &ConstEnv, _type_subst: &HashMap<String, String>) -> Result<TypeRef, ElabError> {
        Err(ElabError::from(ElabErrorKind::Other("Bundles are flattened and do not resolve to a simple TypeRef".into())))
    }
}

// Module Def
impl ComponentDef for ModuleDeclaration {
    fn name(&self) -> &str { &self.name }
    fn is_generic(&self) -> bool { !self.const_params.is_empty() || !self.type_params.is_empty() }
    fn instantiate(&self, instantiator: &mut dyn crate::elab::registry::components::Instantiator, const_args: &[u64], env: &mut ConstEnv, type_subst: &HashMap<String, String>) -> Result<crate::pom::Module, ElabError> {
        if self.const_params.len() != const_args.len() {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "module `{}` expects {} const params, got {}",
                self.name,
                self.const_params.len(),
                const_args.len()
            ))));
        }
        let mut new_env = crate::elab::const_eval::ConstEnv::new();
        for (param_name, val) in self.const_params.iter().zip(const_args.iter()) {
            new_env.define(param_name.clone(), crate::elab::const_eval::ConstVal::Nat(*val));
        }
        instantiator.elaborate_mod_decl(self, &mut new_env, type_subst)
    }
    fn clone_box(&self) -> Box<dyn crate::elab::registry::components::ComponentDef> {
        Box::new(self.clone())
    }
}

// Fn Def
impl CallableDef for FnDecl {
    fn name(&self) -> &str { &self.sig.name }
}
