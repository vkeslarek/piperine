use crate::circuit::state::CircuitState;
use crate::devices::resistor::Resistor;
use crate::devices::soa::{SoaCheck, SoaViolation, SoaViolationSeverity};
use crate::solver::Context;

impl SoaCheck for Resistor {
    fn soa_check(&self, circuit_state: CircuitState<f64>, context: &Context) -> Vec<SoaViolation> {
        let mut soa_violations = Vec::new();

        if let Some(bv_max) = self.model.bv_max {
            let v_plus = circuit_state
                .get_dependent_value(&self.node_plus, 0)
                .unwrap_or(0.0);
            let v_minus = circuit_state
                .get_dependent_value(&self.node_minus, 0)
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
