use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::Context;
use std::collections::HashSet;

pub type SoaCheckState = CircularArrayBuffer2<f64>;

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

#[derive(Debug, Clone)]
pub struct SoaViolations {
    violations: HashSet<SoaViolation>,
}

impl SoaViolations {
    pub fn new() -> Self {
        Self {
            violations: HashSet::new(),
        }
    }

    pub fn from_vec(soa_violations_vec: Vec<SoaViolation>) -> Self {
        let mut soa_violations = SoaViolations::new();
        soa_violations.add_all(soa_violations_vec);
        soa_violations
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

pub trait SoaCheck {
    fn soa_check(&self, circuit_state: &SoaCheckState, context: &Context) -> Vec<SoaViolation>;
}
