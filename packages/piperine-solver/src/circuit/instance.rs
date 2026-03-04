use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::noise::{NoiseAnalysisOptions, NoiseSource};
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisOptions};
use crate::circuit::netlist::Netlist;
use crate::circuit::Circuit;
use crate::devices::soa::SoaCheck;
use crate::devices::{AnyRuntime, Component};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::transient::TransientSolver;
use crate::solver::Context;

pub struct CircuitInstance {
    title: String,
    runtimes: Vec<Box<dyn AnyRuntime>>,
    netlist: Netlist,
}

impl CircuitInstance {
    pub fn instantiate(circuit: &Circuit) -> crate::result::Result<Self> {
        let mut netlist = Netlist::new();
        let runtimes = circuit
            .components()
            .values()
            .map(|component| component.runtime(&mut netlist))
            .collect();

        Ok(Self {
            title: circuit.title.clone(),
            runtimes,
            netlist,
        })
    }

    pub fn update_all(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) {
        self.runtimes
            .iter_mut()
            .for_each(|runtime| runtime.update(state, context));
    }

    pub fn netlist(&self) -> &Netlist {
        &self.netlist
    }

    pub fn ac(&mut self, context: Context) -> crate::result::Result<AcSolver<'_>> {
        AcSolver::new(self, context)
    }

    pub fn ac_runtimes(&self) -> Vec<&dyn AcAnalysis> {
        self.runtimes
            .iter()
            .filter_map(|runtime| runtime.as_ac())
            .collect()
    }

    pub fn dc(&mut self, context: Context) -> crate::result::Result<DcSolver<'_>> {
        DcSolver::new(self, context)
    }

    pub fn dc_runtimes(&self) -> Vec<&dyn DcAnalysis> {
        self.runtimes
            .iter()
            .filter_map(|runtime| runtime.as_dc())
            .collect()
    }

    pub fn noise(
        &mut self,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<NoiseSolver<'_>> {
        NoiseSolver::new(self, options, context)
    }

    pub fn noise_runtimes(&self) -> Vec<&dyn NoiseSource> {
        self.runtimes
            .iter()
            .filter_map(|runtime| runtime.as_noise_source())
            .collect()
    }

    pub fn transient(
        &mut self,
        transient_options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<TransientSolver<'_>> {
        TransientSolver::new(self, transient_options, context)
    }

    pub fn transient_runtimes(&self) -> Vec<&dyn TransientAnalysis> {
        self.runtimes
            .iter()
            .filter_map(|runtime| runtime.as_transient())
            .collect()
    }

    pub fn soa_runtimes(&self) -> Vec<&dyn SoaCheck> {
        self.runtimes
            .iter()
            .filter_map(|runtime| runtime.as_soa_check())
            .collect()
    }

    pub fn all_runtimes(&self) -> &[Box<dyn AnyRuntime>] {
        &self.runtimes
    }
}
