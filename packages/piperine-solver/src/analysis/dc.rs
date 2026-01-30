use crate::circuit::netlist::{
    BranchIdentifier, CircuitReference, CircuitVariable, Netlist, NodeIdentifier,
};
use crate::devices::soa::SoaViolation;
use crate::devices::Component;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::solver::Context;
use std::collections::HashMap;
use std::sync::Arc;

pub type DcAnalysisState = CircularArrayBuffer2<f64>;

pub trait DcAnalysis: Component {
    fn update_dc(
        &mut self,
        _dc_circuit_state: &DcAnalysisState,
        _context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_dc(
        &self,
        dc_circuit_state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn initial_dc_values(&self, _context: &Context) -> Vec<InitialValue<CircuitReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    values: HashMap<Arc<CircuitVariable>, f64>,
    soa_violations: Vec<SoaViolation>,
}

impl DcAnalysisResult {
    pub fn new(
        values: HashMap<Arc<CircuitVariable>, f64>,
        soa_violations: Vec<SoaViolation>,
    ) -> Self {
        Self {
            values,
            soa_violations,
        }
    }
    pub fn get(&self, variable: impl Into<Arc<CircuitVariable>>) -> Option<f64> {
        self.values.get(&variable.into()).cloned()
    }

    pub fn get_node(&self, node_identifier: impl Into<NodeIdentifier>) -> Option<f64> {
        self.get(CircuitVariable::Node(node_identifier.into()))
    }

    pub fn get_branch(&self, branch_identifier: impl Into<BranchIdentifier>) -> Option<f64> {
        self.get(CircuitVariable::Branch(branch_identifier.into()))
    }

    pub fn values(&self) -> &HashMap<Arc<CircuitVariable>, f64> {
        &self.values
    }

    pub fn soa_violations(&self) -> &Vec<SoaViolation> {
        &self.soa_violations
    }

    /// This method is useful because many analysis types use DC as a starting point
    pub fn as_iv(&self, netlist: &Netlist) -> Vec<InitialValue<CircuitReference, f64>> {
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
