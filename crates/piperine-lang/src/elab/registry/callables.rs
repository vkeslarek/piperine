use crate::pom::ElabError;
use crate::parse::ast::Expr;
use std::collections::HashMap;

pub trait CallableDef: Send + Sync {
    fn name(&self) -> &str;
    fn validate_call(&self, _args: &[Expr]) -> Result<(), ElabError> { Ok(()) }
    fn is_capability(&self) -> bool { false }
}

pub struct CallableRegistry {
    callables: HashMap<String, Box<dyn CallableDef>>,
}

impl Default for CallableRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CallableRegistry {
    pub fn new() -> Self {
        Self { callables: HashMap::new() }
    }

    pub fn register<C: CallableDef + 'static>(&mut self, def: C) {
        self.callables.insert(def.name().to_string(), Box::new(def));
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn CallableDef> {
        self.callables.get(name).map(|c| c.as_ref())
    }

    /// Walk a program and resolve calls.
    pub fn resolve_calls(&self, design: &mut crate::pom::Design) -> Result<(), ElabError> {
        crate::elab::resolve::resolve_calls(design)
    }
}
