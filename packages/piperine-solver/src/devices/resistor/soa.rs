use crate::devices::resistor::Resistor;
use crate::devices::soa::{SoaCheck, SoaCheckState, SoaViolation, SoaViolationSeverity};
use crate::math::linear::AsIndexGetExt;
use crate::solver::Context;

impl SoaCheck for Resistor {
    fn soa_check(&self, circuit_state: &SoaCheckState, _context: &Context) -> Vec<SoaViolation> {
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

#[cfg(test)]
mod test {
    use crate::circuit::builder::builder;
    use crate::devices::resistor::model::ResistorModel;

    #[test]
    fn test_dc_resistor_soa_violation() {
        use crate::circuit::Circuit;
        use crate::circuit::netlist::GND;
        use crate::devices::soa::SoaViolationSeverity;
        use crate::math::unit::UnitExt;
        use crate::solver::Context;

        let mut circuit: Circuit = builder("Resistor SOA Test", |builder| {
            let resistor_model = builder.model(
                "SOA_MODEL",
                ResistorModel {
                    bv_max: Some(50.0),
                    ..Default::default()
                },
            );
            builder
                .resistor("R1", "high_volt", GND, 10.0.kOhms())
                .with_model(resistor_model);

            builder.voltage_source("V1", "high_volt", GND, 100.0.V());
        })
        .into();

        let result = circuit
            .dc(Context::default())
            .expect("Solver failed")
            .solve()
            .expect("Simulation failed to converge");

        let violations = result.soa_violations();

        println!("Detected {} SOA violations", violations.len());
        for v in violations {
            println!("[{:?}] Device {}: {}", v.severity, v.component, v.message);
        }

        assert!(!violations.is_empty(), "SOA violation was not detected!");

        let r1_violation = violations
            .iter()
            .find(|v| v.component == "R1" && v.id == "BVMAX_EXCEEDED")
            .expect("Specific BVMAX violation for R1 not found");

        assert_eq!(r1_violation.severity, SoaViolationSeverity::HIGH);
    }
}
