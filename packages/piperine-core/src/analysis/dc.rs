use crate::circuit::Circuit;
use crate::devices::Component;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use faer::Col;
use std::collections::{HashMap, VecDeque};

pub struct DcCircuitState {
    pub mapping: HashMap<CircuitReference, usize>,
    pub values: VecDeque<Col<f64>>,
    pub num_symbols: usize,
    pub size: usize,
}

impl DcCircuitState {
    pub fn new(mapping: HashMap<CircuitReference, usize>, num_symbols: usize, size: usize) -> Self {
        Self {
            mapping,
            values: VecDeque::with_capacity(size),
            num_symbols,
            size,
        }
    }

    pub fn update_guess(&mut self, new_values: Col<f64>) {
        self.values.push_front(new_values);

        while self.values.len() > self.size {
            self.values.pop_back();
        }
    }

    pub fn get_diff(&self, new_guess: &Col<f64>) -> Col<f64> {
        self.values
            .get(0)
            .unwrap_or(&Col::zeros(new_guess.shape().0))
            - new_guess
    }

    pub fn get_value(&self, reference: &CircuitReference, lookback: usize) -> Option<f64> {
        let index = self.mapping.get(reference)?;
        let value = self.values.get(lookback)?;

        Some(value[*index])
    }

    pub fn check_convergence(
        &self,
        new_values: &Col<f64>,
        reltol: f64,
        vntol: f64,
        abstol: f64,
    ) -> bool {
        // 1. Get the most recent guess (v_k) to compare against the solution (v_k+1)
        let old_values = match self.values.back() {
            Some(v) => v,
            // If there's no history yet, we haven't converged
            None => return false,
        };

        // 2. Iterate over every entry in the solution vector (num_symbols)
        for i in 0..self.num_symbols {
            let old_v = old_values[i];
            let new_v = new_values[i];
            let diff = (new_v - old_v).abs();

            // 3. Determine if this index is a Voltage or a Current
            // This is important because 1mA is huge if you treat it like 1V,
            // and 1uV is tiny if you treat it like 1A.
            let abstol_to_use = if self.is_index_branch(i) {
                abstol
            } else {
                vntol
            };

            // 4. The SPICE Hybrid Check:
            // Convergence is met if the change is smaller than the relative
            // tolerance AND the absolute floor.
            let limit = reltol * old_v.abs().max(new_v.abs()) + abstol_to_use;

            if diff > limit {
                return false;
            }
        }

        true
    }

    fn is_index_branch(&self, index: usize) -> bool {
        self.mapping
            .iter()
            .any(|(ref_type, &idx)| idx == index && matches!(ref_type, CircuitReference::Branch(_)))
    }
}

pub trait DcAnalysis: Component {
    fn update_dc(
        &mut self,
        dc_circuit_state: &DcCircuitState,
        context: &Context,
    ) -> crate::error::Result<()> {
        Ok(())
    }

    fn load_dc(
        &self,
        dc_circuit_state: &DcCircuitState,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    pub values: Col<f64>,
    pub mapping: HashMap<CircuitReference, usize>,
}

pub trait DcSolver {
    fn build(circuit: Circuit, context: Context) -> crate::error::Result<impl DcSolver>;
    fn solve(&mut self) -> crate::error::Result<DcAnalysisResult>;
}
