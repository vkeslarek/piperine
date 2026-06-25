use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::tf::TransferFunctionAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::circuit::Circuit;
use crate::circuit::netlist::Netlist;
use crate::osdi::runtime::OsdiRuntime;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::Context;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::tf::TransferFunctionSolver;
use crate::solver::transient::TransientSolver;

pub struct CircuitInstance {
    pub title: String,
    pub runtimes: Vec<OsdiRuntime>,
    pub netlist: Netlist,
}

impl CircuitInstance {
    pub fn instantiate(circuit: &Circuit) -> crate::result::Result<Self> {
        let mut netlist = Netlist::new();
        let runtimes = circuit
            .components()
            .values()
            .map(|component| OsdiRuntime::allocate_osdi(component.lib.clone(), component.descriptor_idx, component.name.clone(), &component.terminals, &component.params, &component.str_params, &mut netlist))
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

    pub fn dc(&mut self, context: Context) -> crate::result::Result<DcSolver<'_>> {
        DcSolver::new(self, context)
    }

    pub fn noise(
        &mut self,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<NoiseSolver<'_>> {
        NoiseSolver::new(self, options, context)
    }

    pub fn transfer_function(
        &mut self,
        options: TransferFunctionAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<TransferFunctionSolver<'_>> {
        TransferFunctionSolver::new(self, options, context)
    }

    pub fn transient(
        &mut self,
        transient_options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<TransientSolver<'_>> {
        TransientSolver::new(self, transient_options, context)
    }

    pub fn all_runtimes(&self) -> &[OsdiRuntime] {
        &self.runtimes
    }

    pub fn all_runtimes_mut(&mut self) -> &mut [OsdiRuntime] {
        &mut self.runtimes
    }
}
