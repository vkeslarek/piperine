use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::{BranchIdentifier, CircuitReference, Netlist, NodeIdentifier};
use crate::component::{Component, Context};
use crate::model::vsrc::VoltageSourceModel;
use crate::solver::Stamp;
use crate::state::CircuitStates;
use num_complex::Complex;
use piperine_macros::stamps;
use std::sync::Arc;

pub struct VoltageSourceParameters {
    pub name: String,
    pub model: Arc<VoltageSourceModel>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub voltage: f64,
}

pub struct VoltageSource {
    pub name: String,
    pub model: Arc<VoltageSourceModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference,
    pub voltage: f64,
}

impl VoltageSource {
    pub fn new(
        netlist: &mut Netlist,
        parameters: VoltageSourceParameters,
    ) -> crate::error::Result<Self> {
        Ok(Self {
            name: parameters.name.clone(),
            model: parameters.model,
            node_plus: netlist.connect_node(parameters.node_plus),
            node_minus: netlist.connect_node(parameters.node_minus),
            branch: netlist.connect_branch(BranchIdentifier {
                component: parameters.name,
                name: None,
            }),
            voltage: parameters.voltage,
        })
    }
}

impl Component for VoltageSource {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        Some(self)
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        Some(self)
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        Some(self)
    }
}

impl DcAnalysis for VoltageSource {
    fn load_dc(&self, context: &Context) -> Vec<Stamp<f64>> {
        stamps!(
           // Row = Branch Index, Col = Node Indices
            KVL(self.branch): {
                self.node_plus  => 1.0,
                self.node_minus => -1.0,
                RHS             => self.voltage
            },
            // Row = Node Index, Col = Branch Index
            KCL(self.node_plus): {
                self.branch     => 1.0
            },
            KCL(self.node_minus): {
                self.branch     => -1.0
            }
        )
    }
}

impl TransientAnalysis for VoltageSource {
    fn load_transient(
        &self,
        _: &CircuitStates,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<f64>> {
        self.load_dc(context)
    }
}

impl AcAnalysis for VoltageSource {
    fn load_ac(
        &self,
        _circuit_states: &CircuitStates,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Complex<f64>>> {
        let one = Complex::new(1.0, 0.0);
        let zero = Complex::new(0.0, 0.0);
        // Usually, AC sources have a specific AC magnitude; defaulting to 1.0
        let ac_volt = Complex::new(1.0, 0.0);

        stamps!(
            KCL(self.node_plus): {
                self.branch => one
            },
            KCL(self.node_minus): {
                self.branch => -one
            },
            Equation(self.branch): {
                self.node_plus  => one,
                self.node_minus => -one,
                RHS             => ac_volt
            }
        )
    }
}
