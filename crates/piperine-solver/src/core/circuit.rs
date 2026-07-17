use crate::analysis::noise::NoiseAnalysisOptions;
use crate::analysis::tf::TransferFunctionAnalysisOptions;
use crate::analysis::transient::TransientAnalysisOptions;
use crate::analog::Netlist;
use crate::core::element::{Element, ElementCapabilities};
use crate::digital::{DigitalState, DigitalTopology};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::solver::Context;
use crate::solver::ac::AcSolver;
use crate::solver::dc::DcSolver;
use crate::solver::noise::NoiseSolver;
use crate::solver::tf::TransferFunctionSolver;
use crate::solver::transient::TransientSolver;


// ---------------------------------------------------------------------------
// SignalBridge — analog↔digital bridge, extracted from CircuitInstance
// ---------------------------------------------------------------------------

/// Internal component owned by `CircuitInstance`. Handles the analog→digital
/// bridge: builds the solution buffer, seeds the digital event queue from
/// analog accept hooks, and runs the digital scheduler.
pub struct SignalBridge {
    // stateless today; future home for bridge-specific config
}

impl SignalBridge {
    /// Build a 1-row circular buffer from the solution slice.
    pub fn build_accept_state(&self, solution: &[f64]) -> CircularArrayBuffer2<f64> {
        let mut state = CircularArrayBuffer2::new(1, solution.len());
        let row = ndarray::Array1::from_vec(solution.to_vec());
        state.push(&row.view());
        state
    }

    /// Run analog accept hooks at time `t` and seed the digital event queue.
    /// The caller must call `run_digital_at` afterward.
    pub fn settle(
        &mut self,
        devices: &mut [Box<dyn Element>],
        digital_state: &mut DigitalState,
        state: &CircularArrayBuffer2<f64>,
        t: f64,
    ) {
        use std::cmp::Reverse;
        let before = digital_state.nets.clone();
        let mut seed_queue = std::collections::BinaryHeap::new();
        let mut seq = 0u64;
        for (i, device) in devices.iter_mut().enumerate() {
            let mut sink =
                crate::digital::interface::QueueSink::new(&mut seed_queue, t, i, &mut seq);
            device.accept_timestep(state, t, &before, &mut sink);
        }
        for Reverse(event) in seed_queue {
            digital_state.schedule(event);
        }
    }
}

// ---------------------------------------------------------------------------
// CircuitInstance — the instantiated, ready-to-simulate circuit
// ---------------------------------------------------------------------------

pub struct CircuitInstance {
    pub title: String,
    /// All devices — both analog and digital. Each device may implement either
    /// or both sides; the `Element` trait default impls handle the no-op cases.
    pub devices: Vec<Box<dyn Element>>,
    pub digital_topology: Option<DigitalTopology>,
    pub digital_state: DigitalState,
    pub netlist: Netlist,
    bridge: SignalBridge,
    is_set_up: bool,
}

impl CircuitInstance {
    /// Build a `CircuitInstance` from pre-built devices and a netlist.
    /// PHDL modules into devices before handing them to the solver.
    pub fn from_devices_and_netlist(
        title: impl Into<String>,
        devices: Vec<Box<dyn Element>>,
        netlist: Netlist,
    ) -> Self {
        Self {
            title: title.into(),
            devices,
            digital_topology: None,
            digital_state: DigitalState::new(0),
            netlist,
            bridge: SignalBridge {},
            is_set_up: false,
        }
    }

    pub(crate) fn setup_all(&mut self, ctx: &Context) -> crate::result::Result<()> {
        if self.is_set_up { return Ok(()); }
        for d in self.devices.iter_mut() { d.setup(ctx)?; }
        self.is_set_up = true;
        Ok(())
    }

    pub fn update_all(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) {
        self.devices.iter_mut().for_each(|d| d.update(state, context));
    }

    /// Solver-level restamp path (MD-18): set a parameter on the built
    /// element labeled `label` — no re-elaboration, no re-compilation. The
    /// element reports how much solve state the change invalidates
    /// (numeric-only changes are [`Invalidation::Restamp`]); a sweep loop
    /// re-runs the analysis on the same compiled circuit. Unknown labels
    /// and parameter errors are loud.
    pub fn set_element_param(
        &mut self,
        label: &str,
        param: &str,
        value: crate::core::introspect::Value,
    ) -> crate::result::Result<crate::core::introspect::Invalidation> {
        let device = self.devices.iter_mut().find(|d| d.name() == label).ok_or_else(|| {
            crate::error::Error::simple(
                crate::error::SolverDomain::Element,
                format!("no element labeled `{label}`"),
            )
        })?;
        let outcome = device.set_param(param, value);
        outcome.map_err(|e| {
            // Unknown-parameter writes list what the element does declare —
            // the caller sees the valid names instead of a bare rejection.
            let detail = match &e {
                crate::core::introspect::ParamError::Unknown(_) => {
                    let names: Vec<String> =
                        device.list_params().into_iter().map(|p| p.name).collect();
                    if names.is_empty() {
                        format!("`{label}`: {e}; element declares no writable parameters")
                    } else {
                        format!("`{label}`: {e}; available parameters: {}", names.join(", "))
                    }
                }
                _ => format!("`{label}`: {e}"),
            };
            crate::error::Error::simple(crate::error::SolverDomain::Element, detail)
        })
    }

    /// Steer the Newton guess with every device's structured limiting
    /// feedback ([`Element::convergence_hint`]): the clamped unknown is set
    /// to the limited value before the convergence test. The DC and
    /// transient systems delegate here each iteration.
    pub fn apply_convergence_hints(&self, mut guess: ndarray::ArrayViewMut1<f64>) {
        use crate::math::linear::AsIndex;
        for dev in &self.devices {
            if let Some(hint) = dev.convergence_hint()
                && let Some(i) = hint.net.as_index()
                && i < guess.len()
            {
                guess[i] = hint.limited_value;
            }
        }
    }

    pub fn netlist(&self) -> &Netlist { &self.netlist }

    /// Every solved signal in the circuit — analog nodes, analog branch
    /// currents, and digital nets — as one unified [`Net`] list. This is the
    /// symmetric naming layer a host, result mapper, or diagnostic uses instead
    /// of walking the analog netlist and the digital net array separately.
    ///
    /// Digital nets carry the label the circuit builder attached via
    /// [`DigitalState::set_label`], or the anonymous `d{idx}` form when none
    /// was provided.
    pub fn nets(&self) -> Vec<crate::core::net::Net> {
        use crate::core::net::Net;
        use crate::digital::DigitalNet;
        let mut nets = self.netlist.nets();
        nets.extend(
            (0..self.digital_state.nets.len())
                .map(|i| Net::digital(i, self.digital_state.label_or_default(DigitalNet(i)))),
        );
        nets
    }

    /// Look up the digital net's stable source-level label, defaulting to
    /// `d{idx}` when no label was attached. Convenience for diagnostics and
    /// result mapping that don't want to construct a full [`Net`].
    pub fn digital_label(&self, net: crate::digital::DigitalNet) -> String {
        self.digital_state.label_or_default(net)
    }

    /// Union of every element's declared [`ElementCapabilities`] — what this
    /// whole circuit participates in. Drivers plan against this (e.g. a
    /// pure-analog circuit skips the mixed-signal loop) instead of scanning the
    /// element list by trial downcast.
    pub fn capabilities(&self) -> ElementCapabilities {
        self.devices
            .iter()
            .fold(ElementCapabilities::empty(), |acc, d| acc | d.capabilities())
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

    pub fn all_devices(&self) -> &[Box<dyn Element>] { &self.devices }
    pub fn all_devices_mut(&mut self) -> &mut [Box<dyn Element>] { &mut self.devices }

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
    pub fn run_digital_at(&mut self, t: f64) -> crate::result::Result<()> {
        self.run_digital_at_with_analog(t, &[])
    }

    /// Run digital evaluation at time `t`, supplying the latest analog
    /// solution to elements that declared [`ElementCapabilities::SAMPLES_ANALOG`].
    /// Pass `&[]` when no element in the circuit samples analog (the common
    /// case for pure-digital circuits).
    pub fn run_digital_at_with_analog(
        &mut self,
        t: f64,
        analog_slice: &[f64],
    ) -> crate::result::Result<()> {
        let limits = crate::solver::convergence::PlanLimits::default();
        match &self.digital_topology {
            Some(topo) => self.digital_state.evaluate_dag_ordered(
                t,
                &mut self.devices,
                topo,
                limits,
                analog_slice,
            ),
            None => self.digital_state.evaluate_until_stable(
                t,
                &mut self.devices,
                limits,
                analog_slice,
            ),
        }
    }

    /// Update all devices' cached analog voltages from a solution vector,
    /// then run digital evaluation at time `t`. Used by the DC solver's
    /// mixed-signal convergence loop: after the analog solve converges, the
    /// digital devices need to see the analog voltages (A2D bridge) and their
    /// register updates need to propagate back (D2A bridge).
    ///
    /// Returns `true` if any digital output net changed value.
    pub fn accept_and_run_digital(&mut self, solution: &[f64], t: f64) -> crate::result::Result<bool> {
        let state = self.bridge.build_accept_state(solution);
        let before = self.digital_state.nets.clone();
        {
            let CircuitInstance { devices, digital_state, bridge, .. } = self;
            bridge.settle(devices, digital_state, &state, t);
        }
        self.run_digital_at_with_analog(t, solution)?;
        Ok(before != self.digital_state.nets)
    }

    /// Initialize all digital devices and seed the `DigitalState` with t=0 events.
    ///
    /// Must be called once before the first [`run_digital_at`] call.  Collects
    /// initial events from every device's `init`, schedules them into
    /// `digital_state`, then runs propagation at t=0 so all downstream logic
    /// reflects its power-on state.
    pub fn init_digital(&mut self) -> crate::result::Result<()> {
        use std::cmp::Reverse;
        use crate::digital::DigitalEvent;
        use crate::digital::interface::QueueSink;

        let mut seed_queue = std::collections::BinaryHeap::<Reverse<DigitalEvent>>::new();
        let mut seq: u64 = 0;
        for (i, device) in self.devices.iter_mut().enumerate() {
            if device.capabilities().contains(ElementCapabilities::DIGITAL) {
                let mut sink = QueueSink::new(&mut seed_queue, 0.0, i, &mut seq);
                device.init(&mut sink);
            }
        }
        for Reverse(event) in seed_queue {
            self.digital_state.schedule(event);
        }
        self.run_digital_at(0.0)
    }
}

impl Drop for CircuitInstance {
    fn drop(&mut self) {
        for d in self.devices.iter_mut() { d.destroy(); }
    }
}
