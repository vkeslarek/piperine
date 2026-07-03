use crate::parse::ast::ModuleDeclaration;
use crate::pom::{Module, ElabError, ElabErrorKind};
use crate::elab::const_eval::ConstEnv;
use std::collections::HashMap;

pub trait Instantiator {
    fn ctx(&self) -> &crate::elab::registry::ElabContext;
    fn elaborate_mod_decl(&mut self, decl: &ModuleDeclaration, env: &mut ConstEnv, type_subst: &HashMap<String, String>) -> Result<Module, ElabError>;
}

pub trait ComponentDef: Send + Sync {
    fn name(&self) -> &str;
    fn as_module(&self) -> Option<&ModuleDeclaration> { None }
    fn is_generic(&self) -> bool { false }
    fn instantiate(&self, instantiator: &mut dyn Instantiator, const_args: &[u64], env: &mut ConstEnv, type_subst: &HashMap<String, String>) -> Result<Module, ElabError>;
    fn clone_box(&self) -> Box<dyn ComponentDef>;
}

pub struct ComponentRegistry {
    components: HashMap<String, Box<dyn ComponentDef>>,
    mono_cache: std::cell::RefCell<HashMap<String, Module>>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            mono_cache: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn register<C: ComponentDef + 'static>(&mut self, def: C) {
        self.components.insert(def.name().to_string(), Box::new(def));
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn ComponentDef> {
        self.components.get(name).map(|c| c.as_ref())
    }
    
    pub fn insert_mono_cache(&self, name: String, module: Module) {
        self.mono_cache.borrow_mut().insert(name, module);
    }
    
    pub fn get_monomorphized(&self, name: &str) -> Option<Module> {
        self.mono_cache.borrow().get(name).cloned()
    }
    
    pub fn drain_mono_cache(&self) -> Vec<Module> {
        let mut cache = self.mono_cache.borrow_mut();
        cache.drain().map(|(_, m)| m).collect()
    }
}
