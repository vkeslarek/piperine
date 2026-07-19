pub mod types;
pub mod components;
pub mod callables;
pub mod impls;
pub mod schemas;

pub use types::{TypeRegistry, TypeDefKind};
pub use components::{ComponentRegistry, ComponentDef};
pub use callables::{CallableRegistry, CallableDef};
pub use schemas::{AttrField, SchemaRegistry, SchemaShape};

use crate::elab::event::EventRegistry;
use crate::value::Value;

pub struct ElabContext {
    pub types: TypeRegistry,
    pub components: ComponentRegistry,
    pub callables: CallableRegistry,
    pub events: EventRegistry,
    pub schemas: SchemaRegistry,
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
        use self::types::TypeDefKind;
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
            types.register(TypeDefKind::Primitive { name: name.to_string(), val_type });
        }

        let mut schemas = SchemaRegistry::new();
        // `@rfport(num, z0)` — stdlib-reserved attribute marking a node/wire
        // as an `.sp` S-parameter port (SP-01). Registered unconditionally
        // (not gated by plugin loading, unlike the plugin system's own
        // `@device`/`@port`) so `.sp` port declarations work in any project.
        schemas.register_declared(
            "rfport",
            vec![
                AttrField { name: "num".into(), ty: "Integer".into(), required: true, default: None },
                AttrField {
                    name: "z0".into(),
                    ty: "Real".into(),
                    required: false,
                    default: Some(Value::Real(50.0)),
                },
            ],
        );

        Self {
            types,
            components: ComponentRegistry::new(),
            callables: CallableRegistry::new(),
            events: EventRegistry::with_builtins(),
            schemas,
        }
    }
}
