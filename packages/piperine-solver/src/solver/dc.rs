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
    use crate::circuit::builder;
    use crate::circuit::instance::CircuitInstance;
    use crate::circuit::netlist::GND;
    use crate::math::unit::UnitExt;
    use crate::solver::Context;

    #[test]
    fn test_dc_resistive_divider() {
        let mut circuit: CircuitInstance = builder("DC Divider", |builder| {
            builder.voltage_source("V1", "in", GND, 10.0.V());

            builder.resistor("R1", "in", "out", 1.0.kOhms());
            builder.resistor("R2", "out", GND, 1.0.kOhms());
        })
        .into();

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

        let v_out = result.get_node("out").unwrap();

        println!("DC Divider: V_out = {:.4} V", v_out);
        assert!((v_out - 5.0).abs() < 1e-6, "Divider failed: Expected 5.0V");
    }

    #[test]
    fn test_dc_diode_bias() {
        let mut circuit: CircuitInstance = builder("Diode DC Bias", |builder| {
            builder.voltage_source("V1", "in", GND, 5.0.V());
            builder.resistor("R1", "in", "anode", 1.0.kOhms());
            builder.diode("D1", "anode", GND);
        })
        .into();

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
        let mut circuit: CircuitInstance = builder("Floating Node (Series Caps)", |builder| {
            builder.voltage_source("V1", "in", GND, 10.0.V());

            builder.capacitor("C1", "in", "mid", 1.0.uF());
            builder.capacitor("C2", "mid", GND, 1.0.uF());
        })
        .into();

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
