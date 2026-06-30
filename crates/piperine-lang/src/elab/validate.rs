use crate::parse::ast::{BehaviorKind, BehaviorStmt, BindOp, EventSpec, ModStmt, Stmt};
use crate::elab::event::EventRegistry;
use crate::elab::ir::ElabError;

pub struct Validator<'a> {
    event_registry: &'a EventRegistry,
}

impl<'a> Validator<'a> {
    pub fn new(event_registry: &'a EventRegistry) -> Self {
        Self { event_registry }
    }

    /// Validate mod body: no `<+` (Contrib) or `<-` (Force) allowed.
    pub fn validate_mod_body(&self, stmts: &[ModStmt]) -> Result<(), ElabError> {
        for stmt in stmts {
            match stmt {
                ModStmt::StructuralFor { body, .. } => self.validate_mod_body(body)?,
                ModStmt::StructuralIf { then_body, else_body, .. } => {
                    self.validate_mod_body(then_body)?;
                    if let Some(eb) = else_body {
                        self.validate_mod_body(eb)?;
                    }
                }
                // Instance and Connection are fine.
                ModStmt::ParamDecl { .. }
                | ModStmt::WireDecl { .. }
                | ModStmt::VarDecl { .. }
                | ModStmt::Instance { .. }
                | ModStmt::Connection { .. } => {}
            }
        }
        Ok(())
    }

    /// Validate a behavior block: contribution/force/event domain rules.
    pub fn validate_behavior(
        &self,
        kind: BehaviorKind,
        stmts: &[BehaviorStmt],
    ) -> Result<(), ElabError> {
        for stmt in stmts {
            self.validate_behavior_stmt(kind.clone(), stmt)?;
        }
        Ok(())
    }

    fn validate_behavior_stmt(
        &self,
        kind: BehaviorKind,
        stmt: &BehaviorStmt,
    ) -> Result<(), ElabError> {
        match stmt {
            BehaviorStmt::Bind { op: BindOp::Contrib, .. } => {
                if kind == BehaviorKind::Digital {
                    return Err(ElabError::ContribInDigital);
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
                // Event body uses Block stmts — validate recursively via body.stmts.
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

    fn validate_event_spec(&self, kind: BehaviorKind, spec: &EventSpec) -> Result<(), ElabError> {
        match spec {
            EventSpec::Named { name, .. } => {
                let ev = self
                    .event_registry
                    .lookup(name)
                    .ok_or_else(|| ElabError::UnknownEvent(name.clone()))?;

                if kind == BehaviorKind::Digital && ev.is_analog_crossing() {
                    return Err(ElabError::AnalogEventInDigital(name.clone()));
                }
                if kind == BehaviorKind::Analog && ev.is_digital_edge() {
                    return Err(ElabError::DigitalEventInAnalog(name.clone()));
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

    /// Validate ordinary function block statements (no behavior-specific ops expected,
    /// but we still recurse for completeness).
    fn validate_stmt_in_behavior(
        &self,
        kind: BehaviorKind,
        stmt: &Stmt,
    ) -> Result<(), ElabError> {
        match stmt {
            Stmt::Bind { op: BindOp::Contrib, .. } => {
                if kind == BehaviorKind::Digital {
                    return Err(ElabError::ContribInDigital);
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
