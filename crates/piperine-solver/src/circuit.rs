use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::tf::TransferFunctionAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::analog::{NodeIdentifier, Netlist};
use crate::device::Device;
use crate::topology::{DigitalState, DigitalTopology};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::osdi::device::OsdiDevice;
use crate::solver::Context;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::tf::TransferFunctionSolver;
use crate::solver::transient::TransientSolver;

// ---------------------------------------------------------------------------
// Circuit — builder for assembling OSDI components before instantiation
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

    pub fn components(&self) -> &HashMap<String, OsdiDevice> { &self.components }
    pub fn components_mut(&mut self) -> &mut HashMap<String, OsdiDevice> { &mut self.components }

    pub fn builder<F: FnOnce(&mut Circuit)>(title: impl Into<String>, f: F) -> Circuit {
        let mut c = Circuit::new(title);
        f(&mut c);
        c
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
    /// All devices — both analog and digital. Each device may implement either
    /// or both sides; the `Device` trait default impls handle the no-op cases.
    pub devices: Vec<Box<dyn Device>>,
    pub digital_topology: Option<DigitalTopology>,
    pub digital_state: DigitalState,
    pub netlist: Netlist,
}

impl CircuitInstance {
    /// Instantiate an OSDI-component circuit.
    pub fn instantiate(circuit: &Circuit) -> crate::result::Result<Self> {
        let mut netlist = Netlist::new();
        let ctx = Context::default();
        let devices = circuit
            .components()
            .values()
            .map(|spec| Box::new(OsdiDevice::from_spec(spec, &mut netlist, &ctx)) as Box<dyn Device>)
            .collect();
        Ok(Self {
            title: circuit.title.clone(),
            devices,
            digital_topology: None,
            digital_state: DigitalState::new(0),
            netlist,
        })
    }

    /// Build a `CircuitInstance` from pre-built devices and a netlist.
    ///
    /// Used by higher-level crates (e.g. `piperine-lang`) that compile
    /// PHDL modules into devices before handing them to the solver.
    pub fn from_devices_and_netlist(
        title: impl Into<String>,
        devices: Vec<Box<dyn Device>>,
        netlist: Netlist,
    ) -> Self {
        Self {
            title: title.into(),
            devices,
            digital_topology: None,
            digital_state: DigitalState::new(0),
            netlist,
        }
    }

    pub fn update_all(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) {
        self.devices.iter_mut().for_each(|d| d.update(state, context));
    }

    pub fn netlist(&self) -> &Netlist { &self.netlist }

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

    pub fn all_devices(&self) -> &[Box<dyn Device>] { &self.devices }
    pub fn all_devices_mut(&mut self) -> &mut [Box<dyn Device>] { &mut self.devices }

    pub fn rebuild_digital_topology(&mut self) {
        self.digital_topology = Some(DigitalTopology::build(&self.devices));
    }

    pub fn run_digital_at(&mut self, t: f64) {
        match &self.digital_topology {
            Some(topo) => self.digital_state.evaluate_dag_ordered(t, &mut self.devices, topo),
            None => self.digital_state.evaluate_until_stable(t, &mut self.devices),
        }
    }

    /// Initialize all digital devices and seed the `DigitalState` with t=0 events.
    ///
    /// Must be called once before the first [`run_digital_at`] call.  Collects
    /// initial events from every device's `digital_init`, schedules them into
    /// `digital_state`, then runs propagation at t=0 so all downstream logic
    /// reflects its power-on state.
    pub fn init_digital(&mut self) {
        use std::cmp::Reverse;
        use crate::digital::DigitalEvent;

        let mut seed_queue = std::collections::BinaryHeap::<Reverse<DigitalEvent>>::new();
        for device in &mut self.devices {
            device.digital_init(&mut seed_queue);
        }
        for Reverse(event) in seed_queue {
            self.digital_state.schedule(event);
        }
        self.run_digital_at(0.0);
    }
}
