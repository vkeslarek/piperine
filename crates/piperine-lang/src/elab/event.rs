use crate::parse::ast::{BehaviorKind, BehaviorStmt, BindOp, EventSpec, Expr, ModuleStatement, Stmt};
use crate::pom::{ElabError, ElabErrorKind};
use std::collections::HashMap;

/// A named event-kind specifier used in event control blocks. Implementors
/// describe digital edges, analog crossings, or level-sensitive triggers and
/// can validate the argument expression supplied in the event specification.
pub trait EventKind: Send + Sync {
    /// Returns the keyword name for this event kind (e.g. `"posedge"`).
    fn name(&self) -> &str;
    /// Returns `true` if this is a digital edge event (`posedge`, `negedge`,
    /// `change`). Defaults to `false`.
    fn is_digital_edge(&self) -> bool { false }
    /// Returns `true` if this is an analog crossing event (`cross`, `above`).
    /// Defaults to `false`.
    fn is_analog_crossing(&self) -> bool { false }
    /// Returns `true` if this event is level-sensitive. Defaults to `false`.
    fn is_level(&self) -> bool { false }
    /// Validates the argument expression supplied with the event spec.
    /// Returns `Ok(())` by default.
    fn validate_arg(&self, _arg: &Expr) -> Result<(), String> { Ok(()) }
}

/// Digital edge event: triggers on a low-to-high transition (`posedge`).
pub struct RisingEdge;
/// Digital edge event: triggers on a high-to-low transition (`negedge`).
pub struct FallingEdge;
/// Digital edge event: triggers on any change of the watched net (`change`).
pub struct AnyChange;
/// Analog crossing event: triggers when the watched expression crosses zero
/// (`cross`).
pub struct AnalogCross;
/// Analog crossing event: triggers when the watched expression is above a
/// threshold (`above`).
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

/// Periodic analog event: `@ timer(period)`. Analog-only — the digital
/// kernel has no time-driven events, so a digital `timer` is rejected like
/// any analog event in a digital block.
pub struct AnalogTimer;

impl EventKind for AnalogTimer {
    fn name(&self) -> &str { "timer" }
    fn is_analog_crossing(&self) -> bool { true }
}

pub struct EventRegistry {
    events: HashMap<String, Box<dyn EventKind>>,
}

impl EventRegistry {
    /// Creates a new `EventRegistry` pre-registered with the built-in event
    /// kinds: `RisingEdge`, `FallingEdge`, `AnyChange`, `AnalogCross`,
    /// `AnalogAbove`, and `AnalogTimer`.
    pub fn with_builtins() -> Self {
        let mut r = Self { events: HashMap::new() };
        r.register(RisingEdge);
        r.register(FallingEdge);
        r.register(AnyChange);
        r.register(AnalogCross);
        r.register(AnalogAbove);
        r.register(AnalogTimer);
        r
    }

    /// Registers an `EventKind` under its [`name`](EventKind::name).
    pub fn register<E: EventKind + 'static>(&mut self, event: E) {
        self.events.insert(event.name().to_owned(), Box::new(event));
    }

    /// Looks up a registered `EventKind` by name, returning `None` if
    /// no event with that name has been registered.
    pub fn lookup(&self, name: &str) -> Option<&dyn EventKind> {
        self.events.get(name).map(|e| e.as_ref())
    }

    /// Recursively validates a module body's statements to ensure no
    /// event constructs appear at the structural level. `StructuralFor`
    /// and `StructuralIf` bodies are recursively descended.
    pub fn validate_mod_body(&self, stmts: &[ModuleStatement]) -> Result<(), ElabError> {
        for stmt in stmts {
            match stmt {
                ModuleStatement::StructuralFor { body, .. } => self.validate_mod_body(body)?,
                ModuleStatement::StructuralIf { then_body, else_body, .. } => {
                    self.validate_mod_body(then_body)?;
                    if let Some(eb) = else_body {
                        self.validate_mod_body(eb)?;
                    }
                }
                ModuleStatement::ParamDecl { .. }
                | ModuleStatement::WireDecl { .. }
                | ModuleStatement::VarDecl { .. }
                | ModuleStatement::Instance { .. }
                | ModuleStatement::Connection { .. }
                | ModuleStatement::Assert { .. } => {}
            }
        }
        Ok(())
    }

    /// Validates a slice of `BehaviorStmt`s against `kind` (Analog/Digital).
    /// Delegates each statement to
    /// [`validate_behavior_stmt`](EventRegistry::validate_behavior_stmt).
    pub fn validate_behavior(&self, kind: BehaviorKind, stmts: &[BehaviorStmt]) -> Result<(), ElabError> {
        for stmt in stmts {
            self.validate_behavior_stmt(kind.clone(), stmt)?;
        }
        Ok(())
    }

    /// Validates a single `BehaviorStmt` against the behavior kind.
    /// Flags `Contrib` in digital blocks, and recursively validates
    /// `If`/`Match`/`For`/`Event` sub-bodies.
    fn validate_behavior_stmt(&self, kind: BehaviorKind, stmt: &BehaviorStmt) -> Result<(), ElabError> {
        match stmt {
            BehaviorStmt::Bind { op: BindOp::Contrib, .. } => {
                if kind == BehaviorKind::Digital {
                    return Err(ElabError::from(ElabErrorKind::ContribInDigital));
                }
            }
            BehaviorStmt::Bind { .. } => {}
            BehaviorStmt::If { then_body, else_body, .. } => {
                self.validate_behavior(kind.clone(), then_body)?;
                if let Some(eb) = else_body {
                    self.validate_behavior(kind.clone(), eb)?;
                }
            }
            BehaviorStmt::Match { arms, .. } => {
                for arm in arms {
                    self.validate_behavior(kind.clone(), &arm.body)?;
                }
            }
            BehaviorStmt::For { body, .. } => {
                self.validate_behavior(kind.clone(), body)?;
            }
            BehaviorStmt::Event { spec, body, .. } => {
                self.validate_event_spec(kind.clone(), spec)?;
                for stmt in &body.stmts {
                    self.validate_stmt_in_behavior(kind.clone(), stmt)?;
                }
            }
            BehaviorStmt::VarDecl { .. }
            | BehaviorStmt::Diagnostic { .. }
            | BehaviorStmt::Expr(_) => {}
        }
        Ok(())
    }

    /// Validates an `EventSpec` against the behavior kind. Rejects analog
    /// events in digital blocks and digital edge events in analog blocks.
    /// Recursively validates `Or` composite specs and passes through
    /// `Initial`/`Final`.
    fn validate_event_spec(&self, kind: BehaviorKind, spec: &EventSpec) -> Result<(), ElabError> {
        match spec {
            EventSpec::Named { name, .. } => {
                let ev = self.lookup(name).ok_or_else(|| ElabError::from(ElabErrorKind::UnknownEvent(name.clone())))?;
                if kind == BehaviorKind::Digital && ev.is_analog_crossing() {
                    return Err(ElabError::from(ElabErrorKind::AnalogEventInDigital(name.clone())));
                }
                if kind == BehaviorKind::Analog && ev.is_digital_edge() {
                    return Err(ElabError::from(ElabErrorKind::DigitalEventInAnalog(name.clone())));
                }
            }
            EventSpec::Or(specs) => {
                for s in specs {
                    self.validate_event_spec(kind.clone(), s)?;
                }
            }
            EventSpec::Initial | EventSpec::Final => {}
        }
        Ok(())
    }

    /// Validates a raw `Stmt` (from a function body) inside an event block.
    /// Recursively checks `If`/`Match`/`For` children and rejects `Contrib`
    /// in digital context.
    fn validate_stmt_in_behavior(&self, kind: BehaviorKind, stmt: &Stmt) -> Result<(), ElabError> {
        match stmt {
            Stmt::Bind { op: BindOp::Contrib, .. } => {
                if kind == BehaviorKind::Digital {
                    return Err(ElabError::from(ElabErrorKind::ContribInDigital));
                }
            }
            Stmt::If { then_body, else_body, .. } => {
                for s in &then_body.stmts {
                    self.validate_stmt_in_behavior(kind.clone(), s)?;
                }
                if let Some(eb) = else_body {
                    for s in &eb.stmts {
                        self.validate_stmt_in_behavior(kind.clone(), s)?;
                    }
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    for s in &arm.body.stmts {
                        self.validate_stmt_in_behavior(kind.clone(), s)?;
                    }
                }
            }
            Stmt::For { body, .. } => {
                for s in &body.stmts {
                    self.validate_stmt_in_behavior(kind.clone(), s)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
