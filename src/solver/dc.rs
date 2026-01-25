use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::{Context, init_solver_configuration};
use log::debug;
use ndarray::{ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

pub struct DcSystem<'a> {
    pub circuit: &'a mut Circuit,
}

impl<'a> NonLinearSystem<CircuitReference, f64> for DcSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha: f64,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut all_stamps = Vec::new();

        for (name, comp) in self.circuit.components_mut() {
            let dc = comp.as_dc().ok_or_else(|| {
                crate::error::Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Missing DC implementation",
                )
            })?;

            dc.update_dc(state, context)?;

            all_stamps.extend(dc.load_dc(state, context));
        }
        Ok(all_stamps)
    }

    fn converged(
        &self,
        state: &CircularArrayBuffer2<f64>,
        new_guess: &ArrayView1<f64>,
        context: &Context,
    ) -> bool {
        let netlist = self.circuit.netlist();
        context.has_converged(state.view(0), new_guess, netlist)
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        mut current_guess: ArrayViewMut1<f64>,
        context: &Context,
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

        if diff_norm >= context.dc_damp_tolerance {
            for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }

            debug!(
                "Damping applied: Step norm {:.2e} > Tolerance {:.2e}",
                diff_norm, context.dc_damp_tolerance
            );
        }
    }

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<f64>, _context: &Context) {}
}

pub struct DcSolver<'a> {
    pub system: DcSystem<'a>,
    pub solver: NewtonRaphsonSolver<CircuitReference, f64, FaerSparseLinearSystem<f64>>,
}

impl<'a> DcSolver<'a> {
    pub fn new(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        init_solver_configuration();
        let netlist = circuit.netlist();
        let mut max_idx = 0;
        let mut variables_by_index = Vec::new();

        let mut mapped_vars: Vec<_> = netlist
            .all_references()
            .into_iter()
            .filter(|id| id.idx().is_some())
            .collect();

        mapped_vars.sort_by_key(|id| id.idx().unwrap());

        if let Some(last) = mapped_vars.last() {
            max_idx = last.idx().unwrap();

            for id in mapped_vars {
                variables_by_index.push(id.clone());
            }
        }

        let size = if variables_by_index.is_empty() {
            0
        } else {
            max_idx + 1
        };

        let mut system = DcSystem { circuit };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 2, context)?;

        Ok(Self { system, solver })
    }

    pub fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        let raw_solution = self.solver.solve(&mut self.system, 0.0)?;

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

        Ok(DcAnalysisResult::new(values))
    }
}

#[cfg(test)]
mod test {
    use crate::circuit::Circuit;
    use crate::circuit::netlist::{CircuitVariable, GND};
    use crate::devices::builder::CircuitBuilderExt;
    use crate::math::unit::UnitExt;
    use crate::solver::Context;

    #[test]
    fn test_dc_resistive_divider() {
        let mut circuit = Circuit::new("DC Divider");

        circuit.voltage_source("V1", "in", GND, 10.0.V());

        circuit.resistor("R1", "in", "out", 1.0.kOhms());
        circuit.resistor("R2", "out", GND, 1.0.kOhms());

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

        let v_out = result.get_node("out").unwrap();

        println!("DC Divider: V_out = {:.4} V", v_out);
        assert!((v_out - 5.0).abs() < 1e-6, "Divider failed: Expected 5.0V");
    }

    #[test]
    fn test_dc_diode_bias() {
        let mut circuit = Circuit::new("Diode DC Bias");

        circuit.voltage_source("V1", "in", GND, 5.0.V());
        circuit.resistor("R1", "in", "anode", 1.0.kOhms());
        circuit.diode("D1", "anode", GND);

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
        let v_d = result.get_node("anode").unwrap();

        println!("Diode Forward Voltage: {:.4} V", v_d);

        assert!(
            v_d > 0.6 && v_d < 0.8,
            "Diode voltage outside realistic range"
        );
    }

    #[test]
    fn test_dc_floating_node_crash() {
        let mut circuit = Circuit::new("Floating Node (Series Caps)");

        circuit.voltage_source("V1", "in", GND, 10.0.V());

        circuit.capacitor("C1", "in", "mid", 1.0.uF());
        circuit.capacitor("C2", "mid", GND, 1.0.uF());

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

        let v_mid = result.get_node("mid").unwrap();

        println!("Floating Node Voltage (stabilized by Gmin): {:.4} V", v_mid);

        assert!(
            (v_mid - 5.0).abs() < 1e-3,
            "Gmin failed to stabilize floating node! Expected 5.0V, got {}",
            v_mid
        );
    }
}
