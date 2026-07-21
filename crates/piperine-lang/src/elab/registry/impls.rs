use crate::parse::ast::{ModuleDeclaration, FnDecl};
use super::components::ComponentDef;
use super::callables::CallableDef;
use crate::pom::{Module, ElabError};
use crate::elab::const_eval::ConstEnv;
use std::collections::HashMap;

// Module Def
impl ComponentDef for ModuleDeclaration {
    fn name(&self) -> &str { &self.name }
    fn is_generic(&self) -> bool { !self.const_params.is_empty() || !self.type_params.is_empty() }
    fn instantiate(&self, instantiator: &mut dyn crate::elab::registry::components::Instantiator, const_args: &[u64], _env: &mut ConstEnv, type_subst: &HashMap<String, String>) -> Result<Module, ElabError> {
        if self.const_params.len() != const_args.len() {
            return Err(ElabError::from(crate::pom::ElabErrorKind::Other(format!(
                "module `{}` expects {} const params, got {}",
                self.name,
                self.const_params.len(),
                const_args.len()
            ))));
        }
        let mut new_env = crate::elab::const_eval::ConstEnv::new();
        for (param_name, val) in self.const_params.iter().zip(const_args.iter()) {
            new_env.define(param_name.clone(), crate::value::Value::Nat(*val));
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
    fn decl_span(&self) -> Option<miette::SourceSpan> { self.span }
}
