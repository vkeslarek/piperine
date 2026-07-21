pub mod types;
pub mod components;
pub mod callables;
pub mod impls;
pub mod schemas;
pub mod operators;
pub mod impl_methods;

pub use types::{TypeRegistry, TypeDefKind};
pub use components::{ComponentRegistry, ComponentDef};
pub use callables::{CallableRegistry, CallableDef, ExternFnDecl};
pub use schemas::{AttrField, SchemaRegistry, SchemaShape};
pub use operators::{OperatorRegistry, ExternOperatorDecl};
pub use impl_methods::ImplMethodTable;

use crate::elab::event::EventRegistry;
use crate::value::Value;

pub struct ElabContext {
    pub types: TypeRegistry,
    pub components: ComponentRegistry,
    pub callables: CallableRegistry,
    pub events: EventRegistry,
    pub schemas: SchemaRegistry,
    /// Runtime operators (`ddt`, `delay`, `slew`, …) — declared-language-
    /// surface T10 groundwork; real `extern operator` declarations land in
    /// T22.
    pub operators: OperatorRegistry,
    /// Native methods declared via `extern impl TypeName { fn ...; }`,
    /// keyed by `(type_name, method_name)` — T10 groundwork; the first real
    /// consumer is the cast migration (T17).
    pub impl_methods: ImplMethodTable,
}

impl Default for ElabContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ElabContext {
    pub fn new() -> Self {
        // The seven primitives (`Real`, `Natural`, `Integer`, `Complex`,
        // `Boolean`, `Quad`, `String`) are no longer hardcoded here — they
        // register via `extern type` declarations parsed from
        // `headers/types.phdl` (declared-language-surface T16, DLS-17),
        // routed through the ordinary `Register` pass exactly like any
        // other top-level item.
        let types = TypeRegistry::new();

        let mut schemas = SchemaRegistry::new();
        // `@rfport(num, z0)` — stdlib-reserved attribute marking a node/wire
        // as an `.sp` S-parameter port (SP-01). Registered unconditionally
        // (not gated by plugin loading, unlike the plugin system's own
        // `@device`/`@port`) so `.sp` port declarations work in any project.
        schemas.register_declared(
            "rfport",
            vec![
                AttrField { name: "num".into(), ty: "Integer".into(), required: true, default: None, decl_span: None },
                AttrField {
                    name: "z0".into(),
                    ty: "Real".into(),
                    required: false,
                    default: Some(Value::Real(50.0)),
                    decl_span: None,
                },
            ],
            None,
        );

        Self {
            types,
            components: ComponentRegistry::new(),
            callables: CallableRegistry::new(),
            events: EventRegistry::with_builtins(),
            schemas,
            operators: OperatorRegistry::new(),
            impl_methods: ImplMethodTable::new(),
        }
    }

    /// Entry point for the `ResolveCalls` elaboration pass
    /// (declared-language-surface T11) — needs the whole `ElabContext`
    /// (not just `CallableRegistry`) since resolution also consults
    /// `impl_methods` for `Type::method(...)` calls.
    pub fn resolve_calls(&self, design: &mut crate::pom::Design) -> Result<(), crate::pom::ElabError> {
        crate::elab::resolve::resolve_calls(design, self)
    }
}
