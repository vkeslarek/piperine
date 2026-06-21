use std::collections::HashMap;
use crate::hardware::HardwareDefinition;

/// Registry of all known hardware element types.
///
/// Populated at startup by plugins via `Plugin::register_hardware()`.
/// The elaborator looks up instances by module name.
#[derive(Default)]
pub struct HardwareRegistry {
    definitions: HashMap<String, Box<dyn HardwareDefinition>>,
}

impl HardwareRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, definition: Box<dyn HardwareDefinition>) {
        self.definitions.insert(definition.name().to_string(), definition);
    }

    pub fn get(&self, name: &str) -> Option<&dyn HardwareDefinition> {
        self.definitions.get(name).map(|b| b.as_ref())
    }
}
