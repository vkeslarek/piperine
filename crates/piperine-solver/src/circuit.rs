use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::tf::TransferFunctionAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::analog::device::AnalogDevice;
use crate::analog::netlist::{NodeIdentifier, Netlist};
use crate::analog::osdi::device::OsdiDevice;
use crate::digital::state::{DigitalDevice, DigitalState, DigitalTopology};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::Context;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::tf::TransferFunctionSolver;
use crate::solver::transient::TransientSolver;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Circuit — builder for assembling components before instantiation
// ---------------------------------------------------------------------------

pub struct Circuit {
    pub title: String,
    pub components: HashMap<String, OsdiDevice>,
    pub node_counter: AtomicUsize,
}

impl Circuit {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            components: HashMap::new(),
            node_counter: AtomicUsize::new(0),
        }
    }

    pub fn port(&self) -> NodeIdentifier {
        NodeIdentifier::Anonymous(self.node_counter.fetch_add(1, Ordering::Relaxed))
    }

    pub fn components(&self) -> &HashMap<String, OsdiDevice> {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut HashMap<String, OsdiDevice> {
        &mut self.components
    }

    pub fn builder<F: FnOnce(&mut Circuit)>(title: impl Into<String>, builder_fn: F) -> Circuit {
        let mut circuit = Circuit::new(title);
        builder_fn(&mut circuit);
        circuit
    }
}

impl Into<CircuitInstance> for Circuit {
    fn into(self) -> CircuitInstance {
        CircuitInstance::instantiate(&self).expect("Failed to instantiate circuit")
    }
}

// ---------------------------------------------------------------------------
// CircuitInstance — the instantiated, ready-to-simulate circuit
// ---------------------------------------------------------------------------

pub struct CircuitInstance {
    pub title: String,
    pub runtimes: Vec<Box<dyn crate::analog::runtime::AnalogRuntime>>,
    pub digital_runtimes: Vec<Box<dyn DigitalDevice>>,
    pub digital_topology: Option<DigitalTopology>,
    pub digital_state: DigitalState,
    pub netlist: Netlist,
}

impl CircuitInstance {
    pub fn instantiate(circuit: &Circuit) -> crate::result::Result<Self> {
        let mut netlist = Netlist::new();
        let runtimes = circuit
            .components()
            .values()
            .map(|component| {
                let paras = crate::analog::device::SimParams {
                    ini_lim: false,
                    gmin: 1e-12,
                    gdev: 1e-12,
                    tnom: 300.15,
                    simulator_version: 1.0,
                    source_scale_factor: 1.0,
                    epsmin: 1e-12,
                    reltol: 1e-3,
                    vntol: 1e-6,
                    abstol: 1e-12,
                };

                // Build a fresh OsdiDevice for the runtime (lib is Arc, cheap clone)
                let device = OsdiDevice {
                    name: component.name.clone(),
                    lib: component.lib.clone(),
                    descriptor_idx: component.descriptor_idx,
                    terminals: component.terminals.clone(),
                    params: component.params.clone(),
                    str_params: component.str_params.clone(),
                };
                let node_refs = device.allocate_nodes(&device.name, &device.terminals, &mut netlist);

                Box::new(crate::analog::runtime::DeviceRuntime::new(
                    device,
                    component.name.clone(),
                    node_refs,
                    &component.params,
                    &component.str_params,
                    &paras
                )) as Box<dyn crate::analog::runtime::AnalogRuntime>
            })
            .collect();

        Ok(Self {
            title: circuit.title.clone(),
            runtimes,
            digital_runtimes: Vec::new(),
            digital_topology: None,
            digital_state: DigitalState::new(0),
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

    pub fn all_runtimes(&self) -> &[Box<dyn crate::analog::runtime::AnalogRuntime>] {
        &self.runtimes
    }

    pub fn all_runtimes_mut(&mut self) -> &mut [Box<dyn crate::analog::runtime::AnalogRuntime>] {
        &mut self.runtimes
    }

    /// Build (or rebuild) the DAG topology from the current `digital_runtimes`.
    /// Call after all digital devices have been added and before starting simulation.
    pub fn rebuild_digital_topology(&mut self) {
        self.digital_topology = Some(DigitalTopology::build(&self.digital_runtimes));
    }
}
