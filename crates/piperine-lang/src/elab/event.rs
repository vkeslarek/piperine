use crate::parse::ast::Expr;
use std::collections::HashMap;

pub trait EventKind: Send + Sync {
    fn name(&self) -> &str;
    fn is_digital_edge(&self) -> bool { false }
    fn is_analog_crossing(&self) -> bool { false }
    fn is_level(&self) -> bool { false }
    fn validate_arg(&self, _arg: &Expr) -> Result<(), String> { Ok(()) }
}

pub struct RisingEdge;
pub struct FallingEdge;
pub struct AnyChange;
pub struct AnalogCross;
pub struct AnalogAbove;

impl EventKind for RisingEdge {
    fn name(&self) -> &str { "posedge" }
    fn is_digital_edge(&self) -> bool { true }
}

impl EventKind for FallingEdge {
    fn name(&self) -> &str { "negedge" }
    fn is_digital_edge(&self) -> bool { true }
}

impl EventKind for AnyChange {
    fn name(&self) -> &str { "change" }
    fn is_digital_edge(&self) -> bool { true }
}

impl EventKind for AnalogCross {
    fn name(&self) -> &str { "cross" }
    fn is_analog_crossing(&self) -> bool { true }
}

impl EventKind for AnalogAbove {
    fn name(&self) -> &str { "above" }
    fn is_analog_crossing(&self) -> bool { true }
}

pub struct EventRegistry {
    events: HashMap<String, Box<dyn EventKind>>,
}

impl EventRegistry {
    pub fn with_builtins() -> Self {
        let mut r = Self { events: HashMap::new() };
        r.register(RisingEdge);
        r.register(FallingEdge);
        r.register(AnyChange);
        r.register(AnalogCross);
        r.register(AnalogAbove);
        r
    }

    pub fn register<E: EventKind + 'static>(&mut self, event: E) {
        self.events.insert(event.name().to_owned(), Box::new(event));
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn EventKind> {
        self.events.get(name).map(|e| e.as_ref())
    }
}
