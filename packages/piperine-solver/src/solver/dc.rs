use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::CircuitReference;
use crate::devices::soa::SoaViolations;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::{init_solver_configuration, Context};
use log::debug;
use ndarray::{ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

pub struct DcSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    pub soa_violations: SoaViolations,
}

impl<'a> NonLinearSystem<CircuitReference, f64> for DcSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha: f64,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut all_stamps = Vec::new();

        self.circuit.update_all(state, &self.context);
        for dc in self.circuit.dc_runtimes().iter() {
            all_stamps.extend(dc.load_dc(state, &self.context));
        }

        Ok(all_stamps)
    }

    fn converged(&self, state: &CircularArrayBuffer2<f64>, new_guess: &ArrayView1<f64>) -> bool {
        let netlist = self.circuit.netlist();
        self.context
            .has_converged(state.view(0), new_guess, netlist)
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        mut current_guess: ArrayViewMut1<f64>,
    ) {
        let last_guess = match state.latest() {
            Some(guess) => guess,
            None => return,
        };

        let diff_norm_sq: f64 = current_guess
            .iter()
            .zip(last_guess.iter())
            .fold(0.0, |acc, (curr, prev)| acc + (curr - prev).powi(2));

        let diff_norm = diff_norm_sq.sqrt();

        if diff_norm >= self.context.dc_damp_tolerance {
            for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }

            debug!(
                "Damping applied: Step norm {:.2e} > Tolerance {:.2e}",
                diff_norm, self.context.dc_damp_tolerance
            );
        }
    }

    fn convergence_success_callback(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _: &ArrayView1<f64>,
    ) {
        for soa_comp in self.circuit.soa_runtimes() {
            self.soa_violations
                .add_all(soa_comp.soa_check(state, &self.context));
        }
    }
}

pub struct DcSolver<'a> {
    pub system: DcSystem<'a>,
    pub solver: NewtonRaphsonSolver<CircuitReference, f64, FaerSparseLinearSystem<f64>>,
}

impl<'a> DcSolver<'a> {
    pub fn new(circuit: &'a mut CircuitInstance, context: Context) -> crate::result::Result<Self> {
        init_solver_configuration();
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = DcSystem {
            circuit,
            context,
            soa_violations: SoaViolations::new(),
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 1)?;

        Ok(Self { system, solver })
    }

    pub fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        let max_iter = self.system.context.max_iter;
        let raw_solution = self.solver.solve(&mut self.system, 0.0, max_iter)?;

        let mut values = HashMap::new();
        let netlist = self.system.circuit.netlist();

        for reference in netlist.all_references() {
            if let Some(reference_idx) = reference.idx() {
                values.insert(
                    reference.variable().clone(),
                    raw_solution[reference_idx].clone(),
                );
            }
        }

        Ok(DcAnalysisResult::new(
            values,
            self.system.soa_violations.clone().as_vec(),
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::circuit::instance::CircuitInstance;
    use crate::circuit::netlist::GND;
    use crate::circuit::Circuit;
    use crate::math::unit::UnitExt;
    use crate::solver::Context;

    #[test]
    fn test_dc_resistive_divider() {
        let mut v_out = GND;

        let mut circuit: CircuitInstance = Circuit::builder("DC Divider", |b| {
            let v_in = b.port();
            v_out = b.port();

            b.voltage_source("V1", v_in.clone(), GND, 10.0.V());
            b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
            b.resistor("R2", v_out.clone(), GND, 1.0.kOhms());
        })
        .into();

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

        let v_out_value = result.get_node(&v_out).unwrap();

        println!("DC Divider: V_out = {:.4} V", v_out_value);
        assert!(
            (v_out_value - 5.0).abs() < 1e-6,
            "Divider failed: Expected 5.0V"
        );
    }

    #[test]
    fn test_dc_diode_bias() {
        let mut anode = GND;

        let mut circuit: CircuitInstance = Circuit::builder("Diode DC Bias", |b| {
            let v_in = b.port();
            anode = b.port();

            b.voltage_source("V1", v_in.clone(), GND, 5.0.V());
            b.resistor("R1", v_in, anode.clone(), 1.0.kOhms());
            b.diode("D1", anode.clone(), GND);
        })
        .into();

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
        let v_d = result.get_node(&anode).unwrap();

        println!("Diode Forward Voltage: {:.4} V", v_d);

        assert!(
            v_d > 0.6 && v_d < 0.8,
            "Diode voltage outside realistic range"
        );
    }

    #[test]
    fn test_dc_floating_node_crash() {
        let mut v_mid = GND;

        let mut circuit: CircuitInstance = Circuit::builder("Floating Node (Series Caps)", |b| {
            let v_in = b.port();
            v_mid = b.port();

            b.voltage_source("V1", v_in.clone(), GND, 10.0.V());
            b.capacitor("C1", v_in, v_mid.clone(), 1.0.uF());
            b.capacitor("C2", v_mid.clone(), GND, 1.0.uF());
        })
        .into();

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

        let v_mid_value = result.get_node(&v_mid).unwrap();

        println!(
            "Floating Node Voltage (stabilized by Gmin): {:.4} V",
            v_mid_value
        );

        assert!(
            (v_mid_value - 5.0).abs() < 1e-3,
            "Gmin failed to stabilize floating node! Expected 5.0V, got {}",
            v_mid_value
        );
    }

    #[test]
    fn test_subcircuit_voltage_dividers() {
        use crate::circuit::netlist::NodeIdentifier;

        // Define a voltage divider subcircuit as a function returning Circuit
        fn voltage_divider(
            input: NodeIdentifier,
            output: NodeIdentifier,
            gnd: NodeIdentifier,
        ) -> Circuit {
            let mut c = Circuit::new("Voltage Divider");
            c.resistor("R1", input, output.clone(), 1.0.kOhms());
            c.resistor("R2", output, gnd, 1.0.kOhms());
            c
        }

        let mut circuit = Circuit::new("Subcircuit Test");
        let v_in = circuit.port();
        let v_out1 = circuit.port();
        let v_out2 = circuit.port();

        circuit.voltage_source("V1", v_in.clone(), GND, 10.0.V());

        // First divider instance - components will be prefixed with "DIV1."
        circuit.subcircuit("DIV1", voltage_divider(v_in.clone(), v_out1.clone(), GND));

        // Second divider instance - components will be prefixed with "DIV2."
        circuit.subcircuit("DIV2", voltage_divider(v_out1.clone(), v_out2.clone(), GND));

        let mut circuit: CircuitInstance = circuit.into();

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

        let v_out1_value = result.get_node(&v_out1).unwrap();
        let v_out2_value = result.get_node(&v_out2).unwrap();

        println!("Subcircuit Test Results:");
        println!("  DIV1 output: {:.4} V", v_out1_value);
        println!("  DIV2 output: {:.4} V", v_out2_value);

        // With 10V input:
        // - DIV1.R1 (1k) from v_in to v_out1
        // - DIV1.R2 (1k) from v_out1 to GND
        // - DIV2.R1 (1k) from v_out1 to v_out2 (loads DIV1 output)
        // - DIV2.R2 (1k) from v_out2 to GND
        //
        // This creates:
        // v_out1 = 10V * (1k || 2k) / (1k + (1k || 2k)) = 10V * (2/3) / (1 + 2/3) = 4V
        // v_out2 = v_out1 * 1k / (1k + 1k) = 4V / 2 = 2V

        // Verify voltages (component names are scoped as DIV1.R1, DIV1.R2, DIV2.R1, DIV2.R2)
        assert!(
            (v_out1_value - 4.0).abs() < 1e-6,
            "First divider failed: Expected 4.0V, got {}",
            v_out1_value
        );
        assert!(
            (v_out2_value - 2.0).abs() < 1e-6,
            "Second divider failed: Expected 2.0V, got {}",
            v_out2_value
        );
    }

    #[test]
    fn test_circuit_new_direct() {
        // Test using Circuit::new() directly instead of builder closure
        let mut circuit = Circuit::new("Direct API Test");

        let v_in = circuit.port();
        let v_out = circuit.port();

        circuit.voltage_source("V1", v_in.clone(), GND, 5.0.V());
        circuit.resistor("R1", v_in, v_out.clone(), 2.0.kOhms());
        circuit.resistor("R2", v_out.clone(), GND, 3.0.kOhms());

        let mut instance: CircuitInstance = circuit.into();
        let result = instance.dc(Context::default()).unwrap().solve().unwrap();

        let v_out_value = result.get_node(&v_out).unwrap();

        println!("Direct API V_out: {:.4} V", v_out_value);

        // 5V * 3k / (2k + 3k) = 3V
        assert!(
            (v_out_value - 3.0).abs() < 1e-6,
            "Expected 3.0V, got {}",
            v_out_value
        );
    }
}
