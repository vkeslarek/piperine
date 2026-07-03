pub mod types;
pub mod components;
pub mod callables;
pub mod impls;

pub use types::{TypeRegistry, TypeDef};
pub use components::{ComponentRegistry, ComponentDef};
pub use callables::{CallableRegistry, CallableDef};

use crate::elab::event::EventRegistry;

pub struct ElabContext {
    pub types: TypeRegistry,
    pub components: ComponentRegistry,
    pub callables: CallableRegistry,
    pub events: EventRegistry,
}

impl Default for ElabContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ElabContext {
    pub fn new() -> Self {
        let mut types = TypeRegistry::new();
        use crate::pom::ValueType;
        use impls::PrimitiveTypeDef;
        let prims = vec![
            ("Real", ValueType::Real),
            ("Natural", ValueType::Natural),
            ("Integer", ValueType::Integer),
            ("Complex", ValueType::Complex),
            ("Boolean", ValueType::Boolean),
            ("Quad", ValueType::Quad),
            ("String", ValueType::Str),
        ];
        for (name, val_type) in prims {
            types.register(PrimitiveTypeDef { name: name.to_string(), val_type });
        }

        Self {
            types,
            components: ComponentRegistry::new(),
            callables: CallableRegistry::new(),
            events: EventRegistry::with_builtins(),
        }
    }
}
