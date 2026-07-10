use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::tf::TransferFunctionAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::analog::Netlist;
use crate::core::device::Device;
use crate::digital::{DigitalState, DigitalTopology};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::Context;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::tf::TransferFunctionSolver;
use crate::solver::transient::TransientSolver;


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
    /// Build a `CircuitInstance` from pre-built devices and a netlist.
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
        self.devices.iter_mut().for_each(|d| {
            if let Some(a) = d.as_analog() {
                a.update(state, context);
            }
        });
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

    /// Run digital evaluation at time `t`.
    ///
    /// Integration seam for the fused-network JIT (see
    /// `piperine-codegen/docs/DIGITAL_JIT.md`): today this either walks the
    /// ranked DAG calling each device's `eval_discrete`, or falls back to the
    /// event/delta-cycle loop. The follow-up adds a third arm — a single fused
    /// `DigitalEventModel` over the whole combinational cone — that replaces the
    /// per-device loop with one native call, the scheduler unchanged (it still
    /// only sees the event boundary).
    pub fn run_digital_at(&mut self, t: f64) {
        match &self.digital_topology {
            Some(topo) => self.digital_state.evaluate_dag_ordered(t, &mut self.devices, topo),
            None => self.digital_state.evaluate_until_stable(t, &mut self.devices),
        }
    }

    /// Update all devices' cached analog voltages from a solution vector,
    /// then run digital evaluation. Used by the DC solver's mixed-signal
    /// convergence loop: after the analog solve converges, the digital
    /// devices need to see the analog voltages (A2D bridge) and their
    /// register updates need to propagate back (D2A bridge).
    ///
    /// Returns `true` if any digital output net changed value.
    pub fn accept_and_run_digital(&mut self, solution: &[f64], ctx: &Context, t: f64) -> bool {
        use std::cmp::Reverse;
        use ndarray::Array1;
        
        let mut state = CircularArrayBuffer2::new(1, solution.len());
        let row = Array1::from_vec(solution.to_vec());
        state.push(&row.view());

        let before = self.digital_state.nets.clone();
        let mut seed_queue = std::collections::BinaryHeap::new();
        let mut seq = 0u64;

        for (i, device) in self.devices.iter_mut().enumerate() {
            if let Some(a) = device.as_analog() {
                let mut sink =
                    crate::digital::interface::QueueSink::new(&mut seed_queue, ctx.time, i, &mut seq);
                a.accept_timestep(&state, ctx, &before, &mut sink);
            }
        }
        
        for Reverse(event) in seed_queue {
            self.digital_state.schedule(event);
        }

        self.run_digital_at(t);
        let after = &self.digital_state.nets;
        before != *after
    }

    /// Initialize all digital devices and seed the `DigitalState` with t=0 events.
    ///
    /// Must be called once before the first [`run_digital_at`] call.  Collects
    /// initial events from every device's `init`, schedules them into
    /// `digital_state`, then runs propagation at t=0 so all downstream logic
    /// reflects its power-on state.
    pub fn init_digital(&mut self) {
        use std::cmp::Reverse;
        use crate::digital::DigitalEvent;
        use crate::digital::interface::QueueSink;

        let mut seed_queue = std::collections::BinaryHeap::<Reverse<DigitalEvent>>::new();
        let mut seq: u64 = 0;
        for (i, device) in self.devices.iter_mut().enumerate() {
            if let Some(d) = device.as_digital() {
                let mut sink = QueueSink::new(&mut seed_queue, 0.0, i, &mut seq);
                d.init(&mut sink);
            }
        }
        for Reverse(event) in seed_queue {
            self.digital_state.schedule(event);
        }
        self.run_digital_at(0.0);
    }
}
