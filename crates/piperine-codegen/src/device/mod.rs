//! The solver boundary: compiled kernels wrapped as [`piperine_solver::prelude::Element`]s.
//!
//! - [`CompiledModule`] — the per-module compilation artifact (analog and/or
//!   digital kernel), shared across instances.
//! - [`PiperineDevice`] — one instance: parameter values, operator state,
//!   register banks, netlist references. Implements the solver `Element`
//!   trait for both domains.
//! - [`CircuitCompiler`] — walks an [`crate::ir::IrProgram`]'s top module and
//!   builds a ready-to-simulate `CircuitInstance`.

mod analog;
mod circuit;
mod digital;
mod provider;

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use num_complex::Complex64;

use piperine_solver::abi::AnalogReference;
use piperine_solver::abi::AcAnalysisContext;
use piperine_solver::abi::{DcAnalysisResult, DcAnalysisState};
use piperine_solver::abi::Noise;
use piperine_solver::abi::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::abi::{Element, ElementCapabilities};
use piperine_solver::abi::{
    Bounds, Invalidation, ParamDescriptor, ParamError, ParamScope, Value, ValueKind,
};
use piperine_solver::abi::DigitalEvent;
use piperine_solver::abi::{DigitalPorts, EvalCtx, EventSink};
use piperine_solver::abi::CircularArrayBuffer2;
use piperine_solver::abi::Stamp;
use piperine_solver::abi::Context;

use crate::ir::{Analysis, NodeId};
use crate::lower::pom::LoweredBody;
use crate::jit::analog::AnalogKernel;
use crate::jit::digital::DigitalKernel;
use crate::jit::CodegenError;

pub use analog::AnalogInstance;
pub use circuit::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler};
pub use provider::{DeviceProvider, PluginDeviceSpec, PluginPort, PortBinding};
pub use digital::DigitalInstance;

/// The compiled artifact for one module: the JIT kernels, shared (`Arc`)
/// across every instance of the module.
#[derive(Clone)]
pub struct CompiledModule {
    name: String,
    analog: Option<Arc<AnalogKernel>>,
    digital: Option<Arc<DigitalKernel>>,
}

impl CompiledModule {
    /// Compile every behavior body of `module`.
    pub fn compile(module: &LoweredBody) -> Result<Self, CodegenError> {
        let analog = module
            .analog
            .as_ref()
            .map(|_| AnalogKernel::compile(module).map(Arc::new))
            .transpose()?;
        let digital = module
            .digital
            .as_ref()
            .map(|_| DigitalKernel::compile(module).map(Arc::new))
            .transpose()?;
        Ok(Self { name: module.name.clone(), analog, digital })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn analog(&self) -> Option<&Arc<AnalogKernel>> {
        self.analog.as_ref()
    }

    pub fn digital(&self) -> Option<&Arc<DigitalKernel>> {
        self.digital.as_ref()
    }
}

/// One device instance: the mixed-signal `Element` the solver drives.
pub struct PiperineDevice {
    label: String,
    analog: Option<AnalogInstance>,
    digital: Option<DigitalInstance>,
    /// Analog terminal netlist references for digital-only devices (devices
    /// with analog input ports but no analog body). Used by the A2D bridge
    /// to read analog voltages when there's no `AnalogInstance` to track
    /// them. Each entry corresponds to a terminal in the module's port
    /// order; `None` = ground or digital-only port.
    analog_terminal_refs: Vec<Option<AnalogReference>>,
    /// Terminal NodeIds in port order (for mapping to the digital layout's
    /// `analog_index`).
    analog_terminal_node_ids: Vec<NodeId>,
    /// Cached analog voltages (from `accept_timestep`), used by the A2D
    /// bridge when the solver passes `&[]` to `eval_discrete`.
    last_analog_voltages: Vec<f64>,
}

impl PiperineDevice {
    pub fn new(
        label: impl Into<String>,
        analog: Option<AnalogInstance>,
        digital: Option<DigitalInstance>,
    ) -> Self {
        Self {
            label: label.into(),
            analog,
            digital,
            analog_terminal_refs: Vec::new(),
            analog_terminal_node_ids: Vec::new(),
            last_analog_voltages: Vec::new(),
        }
    }

    /// Set the analog terminal references for a digital-only device.
    /// Called by the circuit compiler when the device has analog input
    /// ports but no analog body.
    pub fn set_analog_terminals(
        &mut self,
        refs: Vec<Option<AnalogReference>>,
        node_ids: Vec<NodeId>,
    ) {
        self.last_analog_voltages = vec![0.0; refs.len()];
        self.analog_terminal_refs = refs;
        self.analog_terminal_node_ids = node_ids;
    }

    pub fn analog(&self) -> Option<&AnalogInstance> {
        self.analog.as_ref()
    }

    pub fn digital(&self) -> Option<&DigitalInstance> {
        self.digital.as_ref()
    }
}

impl Element for PiperineDevice {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        let mut caps = ElementCapabilities::empty();
        // A digital-only device with analog input terminals (the A2D bridge)
        // still participates in the analog lifecycle: `accept_timestep` caches
        // its terminal voltages after every accepted solution.
        if self.analog.is_some() || !self.analog_terminal_refs.is_empty() {
            caps |= ElementCapabilities::ANALOG;
        }
        if let Some(digital) = &self.digital {
            caps |= ElementCapabilities::DIGITAL;
            if digital.kernel().layout().num_analog() > 0 {
                caps |= ElementCapabilities::SAMPLES_ANALOG;
            }
        }
        if let Some(analog) = &self.analog {
            caps |= ElementCapabilities::ANALYTIC_JACOBIAN;
            if analog.kernel().has_reactive() {
                caps |= ElementCapabilities::STAMPS_CHARGE;
            }
        }
        caps
    }

    fn limiting_active(&self) -> bool {
        self.analog
            .as_ref()
            .is_some_and(AnalogInstance::limiting_active)
    }

    fn bound_step_hint(&self) -> f64 {
        self.analog
            .as_ref()
            .map_or(f64::INFINITY, AnalogInstance::bound_step_hint)
    }

    fn initial_conditions(&self) -> Vec<(Option<AnalogReference>, Option<AnalogReference>, f64)> {
        self.analog
            .as_ref()
            .map_or_else(Vec::new, AnalogInstance::initial_conditions)
    }

    fn list_params(&self) -> Vec<ParamDescriptor> {
        let Some(analog) = &self.analog else { return Vec::new() };
        analog
            .param_names()
            .iter()
            .filter_map(|name| {
                analog.param(name).map(|value| ParamDescriptor {
                    name: name.clone(),
                    kind: ValueKind::Real,
                    // The JIT bakes elaborated defaults into the value; the
                    // model default is not carried separately, so the current
                    // value stands in.
                    default: Value::Real(value),
                    unit: None,
                    bounds: Bounds::UNBOUNDED,
                    scope: ParamScope::Instance,
                    invalidation: Invalidation::Restamp,
                })
            })
            .collect()
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        self.analog.as_ref().and_then(|a| a.param(name)).map(Value::Real)
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        let Some(v) = value.as_real() else {
            return Err(ParamError::TypeMismatch { name: name.into(), expected: ValueKind::Real });
        };
        if let Some(analog) = self.analog.as_mut() {
            // Writing a presence-queried, never-given optional param is
            // structural: the given-mask is baked at build, so the value
            // alone cannot surface the guarded behavior. Typed `Rebuild`
            // outcome, value NOT applied (no partial apply) — the host
            // re-elaborates and rebuilds (LIVE-14).
            if analog.set_flips_presence(name) {
                return Ok(Invalidation::Rebuild);
            }
            if analog.set_param(name, v) {
                return Ok(Invalidation::Restamp);
            }
        }
        Err(ParamError::Unknown(name.to_string()))
    }

    fn load_dc(
        &mut self,
        state: &DcAnalysisState<'_>,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        match &mut self.analog {
            Some(analog) => analog.load_dc(state, context),
            None => Vec::new(),
        }
    }

    fn load_ac(
        &mut self,
        dc_op: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        match &mut self.analog {
            Some(analog) => analog.load_ac(dc_op, ac_ctx, context),
            None => Vec::new(),
        }
    }

    fn load_transient(
        &mut self,
        states: &TransientAnalysisState<'_>,
        tran_ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        match &mut self.analog {
            Some(analog) => analog.load_transient(states, tran_ctx, context),
            None => Vec::new(),
        }
    }

    fn next_breakpoints(&self, from: piperine_solver::abi::Second, horizon: piperine_solver::abi::Second) -> Vec<piperine_solver::abi::Second> {
        match &self.analog {
            Some(analog) => analog.next_breakpoints(from, horizon),
            None => Vec::new(),
        }
    }

    fn suggest_transient_step(
        &self,
        state: &TransientAnalysisState<'_>,
        time_history: &[f64],
        method: piperine_solver::abi::IntegrationMethod,
        context: &Context,
    ) -> Option<f64> {
        self.analog
            .as_ref()
            .and_then(|a| a.suggest_transient_step(state, time_history, method, context))
    }

    fn accept_timestep(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        t: f64,
        nets: &[piperine_solver::abi::LogicValue],
        sink: &mut dyn EventSink,
    ) {
        if let Some(analog) = &mut self.analog {
            analog.accept_timestep(state, t);
        }

        if self.analog.is_none() && !self.analog_terminal_refs.is_empty() {
            let latest = state.latest();
            for (i, opt_ref) in self.analog_terminal_refs.iter().enumerate() {
                self.last_analog_voltages[i] = opt_ref
                    .as_ref()
                    .and_then(|r| r.idx())
                    .and_then(|idx| latest.map(|s| s[idx]))
                    .unwrap_or(0.0);
            }
        }

        if self.digital.as_ref().is_some_and(|d| d.kernel().layout().num_analog() > 0) {
            let eval_ctx = EvalCtx { time: t, nets, analog: &[] };
            self.evaluate(&eval_ctx, sink);
        }
    }

    fn noise_current_psd(
        &mut self,
        dc_point: &DcAnalysisResult,
        ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> {
        match &mut self.analog {
            Some(analog) => analog.noise_current_psd(dc_point, ac_context),
            None => Vec::new(),
        }
    }

    fn boundary(&self) -> DigitalPorts<'_> {
        match &self.digital {
            Some(d) => DigitalPorts {
                inputs: d.input_nets(),
                outputs: d.output_nets(),
            },
            None => DigitalPorts { inputs: &[], outputs: &[] },
        }
    }

    fn init(&mut self, sink: &mut dyn EventSink) {
        if let Some(digital) = &mut self.digital {
            let mut q: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();
            digital.init(&mut q);
            for Reverse(ev) in q.into_sorted_vec() {
                sink.emit(ev.net, ev.value, ev.time);
            }
        }
    }

    fn seq_phase(&mut self, ctx: &EvalCtx<'_>) -> bool {
        let Some(digital) = &mut self.digital else { return false };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            ctx.analog,
        );
        digital.eval_seq_phase(ctx.time, ctx.nets, &av)
    }

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let Some(digital) = &mut self.digital else { return };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            ctx.analog,
        );
        let mut q: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();
        digital.eval_comb_phase(ctx.time, ctx.nets, &av, &mut q);
        for Reverse(ev) in q.into_sorted_vec() {
            sink.emit(ev.net, ev.value, ev.time - ctx.time);
        }

        if let Some(analog) = &mut self.analog {
            let vars = digital.export_vars();
            analog.sync_vars(&vars);
        }
    }
}

impl PiperineDevice {
    /// A2D bridge: resolve the analog voltages a digital kernel should see
    /// this evaluation. Prefers voltages the solver passed explicitly;
    /// otherwise reads the device's own analog instance (mixed device) or
    /// its cached terminal voltages (digital-only device with analog input
    /// ports), remapped from terminal order into the kernel's compact
    /// `analog_index` order.
    fn analog_voltages_for(
        layout: &crate::jit::digital::DigitalLayout,
        analog: Option<&AnalogInstance>,
        terminal_node_ids: &[NodeId],
        last_analog_voltages: &[f64],
        provided: &[f64],
    ) -> Vec<f64> {
        if !provided.is_empty() {
            return provided.to_vec();
        }
        let num_analog = layout.num_analog();
        let mut compact = vec![0.0; num_analog];
        match analog {
            Some(analog) => {
                let terminal_ids = analog.terminal_node_ids();
                let last_volts = analog.last_volts();
                for (term_idx, &node_id) in terminal_ids.iter().enumerate() {
                    if let Some(compact_idx) = layout.analog_index(node_id)
                        && compact_idx < compact.len() && term_idx < last_volts.len() {
                            compact[compact_idx] = last_volts[term_idx];
                        }
                }
            }
            None => {
                for (term_idx, &node_id) in terminal_node_ids.iter().enumerate() {
                    if let Some(compact_idx) = layout.analog_index(node_id)
                        && compact_idx < compact.len() && term_idx < last_analog_voltages.len() {
                            compact[compact_idx] = last_analog_voltages[term_idx];
                        }
                }
            }
        }
        compact
    }
}

/// Map the IR analysis enum to the `SimCtx.current_analysis` encoding.
fn analysis_code(analysis: Analysis) -> u64 {
    analysis as u64
}
