use crate::devices::resistor::Resistor;
use crate::devices::soa::{SoaCheck, SoaCheckState, SoaViolation, SoaViolationSeverity};
use crate::solver::Context;

impl SoaCheck for Resistor {
    fn soa_check(&self, circuit_state: &SoaCheckState, context: &Context) -> Vec<SoaViolation> {
        let mut soa_violations = Vec::new();

        if let Some(bv_max) = self.model.bv_max {
            let v_plus = circuit_state
                .latest()
                .and_then(|val| val.get(&self.node_plus).cloned())
                .unwrap_or(0.0);
            let v_minus = circuit_state
                .latest()
                .and_then(|val| val.get(&self.node_minus).cloned())
                .unwrap_or(0.0);

            if (v_plus - v_minus).abs() >= bv_max {
                soa_violations.push(SoaViolation::new(
                    "BVMAX_EXCEEDED",
                    self.name.clone(),
                    "Maximum breakdown voltage of the Resistor reached!",
                    SoaViolationSeverity::HIGH,
                ));
            }
        }

        soa_violations
    }
}
