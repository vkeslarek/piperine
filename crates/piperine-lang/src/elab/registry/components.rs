use crate::parse::ast::ModDecl;
use crate::pom::{Module, ElabError};
use crate::elab::const_eval::ConstEnv;
use std::collections::HashMap;

pub trait Instantiator {
    fn ctx(&self) -> &crate::elab::registry::ElabContext;
    fn elaborate_mod_decl(&mut self, decl: &ModDecl, env: &mut ConstEnv, type_subst: &HashMap<String, String>) -> Result<Module, ElabError>;
}

pub trait ComponentDef: Send + Sync {
    fn name(&self) -> &str;
    fn as_module(&self) -> Option<&ModDecl> { None }
    fn is_generic(&self) -> bool { false }
    fn instantiate(&self, instantiator: &mut dyn Instantiator, env: &mut ConstEnv, type_subst: &HashMap<String, String>) -> Result<Module, ElabError>;
    fn clone_box(&self) -> Box<dyn ComponentDef>;
}

pub struct ComponentRegistry {
    components: HashMap<String, Box<dyn ComponentDef>>,
    mono_cache: HashMap<String, Module>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            mono_cache: HashMap::new(),
        }
    }

    pub fn register<C: ComponentDef + 'static>(&mut self, def: C) {
        self.components.insert(def.name().to_string(), Box::new(def));
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn ComponentDef> {
        self.components.get(name).map(|c| c.as_ref())
    }

    pub fn instantiate(
        &mut self,
        name: &str,
        instantiator: &mut dyn Instantiator,
        env: &mut ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<Module, ElabError> {
        // Evaluate const args and compute monomorphized name if any logic exists...
        // For now, just lookup and instantiate
        let def = self.components.get(name)
            .ok_or_else(|| ElabError::UndefinedModule(name.to_owned()))?
            .clone_box();

        let module = def.instantiate(instantiator, env, type_subst)?;
        
        // Cache it if it's generic (we would need the mangled name here)
        // For now, just return it
        Ok(module)
    }
}
