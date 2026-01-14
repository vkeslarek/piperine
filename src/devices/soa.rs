use crate::circuit::state::CircuitState;
use crate::devices::Component;
use crate::solver::Context;
use std::collections::{HashMap, HashSet};
use std::path::Components;

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum SoaViolationSeverity {
    LOW,
    HIGH,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct SoaViolation {
    pub id: String,
    pub component: String,
    pub message: String,
    pub severity: SoaViolationSeverity,
}

impl SoaViolation {
    pub fn new(
        id: impl Into<String>,
        component: impl Into<String>,
        message: impl Into<String>,
        severity: SoaViolationSeverity,
    ) -> Self {
        Self {
            id: id.into(),
            component: component.into(),
            message: message.into(),
            severity,
        }
    }
}

pub struct SoaViolations {
    violations: HashSet<SoaViolation>,
}

impl SoaViolations {
    pub fn new() -> Self {
        Self {
            violations: HashSet::new(),
        }
    }

    pub fn add_all(&mut self, violations: Vec<SoaViolation>) {
        violations.into_iter().for_each(|violation| {
            self.violations.insert(violation);
        });
    }

    pub fn as_vec(self) -> Vec<SoaViolation> {
        self.violations.into_iter().collect()
    }
}

pub trait SoaCheck: Component {
    fn soa_check(&self, circuit_state: CircuitState<f64>, context: &Context) -> Vec<SoaViolation>;
}
