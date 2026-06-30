use crate::analog::{
    BranchIdentifier, AnalogReference, AnalogVariable, Netlist, NodeIdentifier,
};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::solver::Context;
use std::collections::HashMap;
use std::sync::Arc;

pub type DcAnalysisState = CircularArrayBuffer2<f64>;

pub trait DcAnalysis {
    fn load_dc(
        &mut self,
        dc_circuit_state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>>;

    fn initial_dc_values(&mut self, _context: &Context) -> Vec<InitialValue<AnalogReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    values: HashMap<Arc<AnalogVariable>, f64>,
}

impl DcAnalysisResult {
    pub fn new(
        values: HashMap<Arc<AnalogVariable>, f64>,
        ) -> Self {
        Self {
            values,
        }
    }
    pub fn get(&self, variable: impl Into<Arc<AnalogVariable>>) -> Option<f64> {
        self.values.get(&variable.into()).cloned()
    }

    pub fn get_node(&self, node_identifier: &NodeIdentifier) -> Option<f64> {
        self.get(AnalogVariable::Node(node_identifier.clone()))
    }

    pub fn get_branch(&self, branch_identifier: impl Into<BranchIdentifier>) -> Option<f64> {
        self.get(AnalogVariable::Branch(branch_identifier.into()))
    }

    pub fn values(&self) -> &HashMap<Arc<AnalogVariable>, f64> {
        &self.values
    }

    pub fn as_iv(&self, netlist: &Netlist) -> Vec<InitialValue<AnalogReference, f64>> {
        let mut initial_values = Vec::with_capacity(self.values.len());
        for (var, value) in &self.values {
            if let Some(reference) = netlist.reference_for(&var).cloned() {
                initial_values.push(InitialValue {
                    reference,
                    value: *value,
                });
            }
        }

        initial_values
    }
}
