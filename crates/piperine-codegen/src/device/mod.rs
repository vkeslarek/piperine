//! The solver boundary: compiled kernels wrapped as [`piperine_solver::device::Device`]s.
//!
//! - [`CompiledModule`] — the per-module compilation artifact (analog and/or
//!   digital kernel), shared across instances.
//! - [`PiperineDevice`] — one instance: parameter values, operator state,
//!   register banks, netlist references. Implements the solver `Device`
//!   trait for both domains.
//! - [`CircuitCompiler`] — walks an [`crate::ir::IrProgram`]'s top module and
//!   builds a ready-to-simulate `CircuitInstance`.

mod analog;
mod circuit;
mod digital;

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use num_complex::Complex64;

use piperine_solver::analog::AnalogReference;
use piperine_solver::analysis::ac::AcAnalysisContext;
use piperine_solver::analysis::dc::{DcAnalysisResult, DcAnalysisState};
use piperine_solver::analysis::noise::Noise;
use piperine_solver::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::core::device::{AnalogDevice, Device, DigitalDevice};
use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};
use piperine_solver::math::circular_array::CircularArrayBuffer2;
use piperine_solver::math::linear::Stamp;
use piperine_solver::solver::Context;

use crate::ir::{Analysis, NodeId};
use crate::lower::pom::LoweredBody;
use crate::jit::analog::AnalogKernel;
use crate::jit::digital::DigitalKernel;
use crate::jit::CodegenError;

pub use analog::AnalogInstance;
pub use circuit::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler};
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
    /// Validate and compile every behavior body of `module`.
    pub fn compile(module: &LoweredBody) -> Result<Self, CodegenError> {
        module
            .validated()
            .map_err(|d| CodegenError::Invalid(format!("{}: {}", module.name, d.message)))?;
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

/// One device instance: the mixed-signal `Device` the solver drives.
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

impl Device for PiperineDevice {
    fn device_name(&self) -> &str {
        &self.label
    }
    fn as_analog(&mut self) -> Option<&mut dyn AnalogDevice> { Some(self) }
    fn as_analog_ref(&self) -> Option<&dyn AnalogDevice> { Some(self) }
    fn as_digital(&mut self) -> Option<&mut dyn DigitalDevice> { Some(self) }
    fn as_digital_ref(&self) -> Option<&dyn DigitalDevice> { Some(self) }
}

impl AnalogDevice for PiperineDevice {
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

    fn load_dc(
        &mut self,
        state: &DcAnalysisState,
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
        states: &TransientAnalysisState,
        tran_ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        match &mut self.analog {
            Some(analog) => analog.load_transient(states, tran_ctx, context),
            None => Vec::new(),
        }
    }

    fn accept_timestep(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        ctx: &Context,
        nets: &[piperine_solver::digital::LogicValue],
        event_queue: &mut std::collections::BinaryHeap<std::cmp::Reverse<piperine_solver::digital::DigitalEvent>>,
    ) {
        if let Some(analog) = &mut self.analog {
            analog.accept_timestep(state, ctx);
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
        
        if self.digital.as_ref().map_or(false, |d| d.kernel().layout().num_analog() > 0) {
            self.eval_discrete(ctx.time, nets, ndarray::ArrayView1::from(&[]), event_queue);
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
}

impl DigitalDevice for PiperineDevice {
    fn digital_input_nets(&self) -> &[DigitalNet] {
        self.digital
            .as_ref()
            .map_or(&[], DigitalInstance::input_nets)
    }

    fn digital_output_nets(&self) -> &[DigitalNet] {
        self.digital
            .as_ref()
            .map_or(&[], DigitalInstance::output_nets)
    }

    fn digital_init(&mut self, event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        if let Some(digital) = &mut self.digital {
            digital.init(event_queue);
        }
    }

    fn eval_discrete(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: ndarray::ArrayView1<f64>,
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let Some(digital) = &mut self.digital else { return };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            analog_voltages.as_slice().unwrap(),
        );
        digital.eval(t, nets, &av, event_queue);

        if let Some(analog) = &mut self.analog {
            let vars = digital.export_vars();
            analog.sync_vars(&vars);
        }
    }

    fn digital_seq_phase(&mut self, t: f64, nets: &[LogicValue], analog_voltages: ndarray::ArrayView1<f64>) -> bool {
        let Some(digital) = &mut self.digital else { return false };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            analog_voltages.as_slice().unwrap(),
        );
        digital.eval_seq_phase(t, nets, &av)
    }

    fn digital_comb_phase(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        analog_voltages: ndarray::ArrayView1<f64>,
        event_queue: &mut std::collections::BinaryHeap<std::cmp::Reverse<piperine_solver::digital::DigitalEvent>>,
    ) {
        let Some(digital) = &mut self.digital else { return };
        let av = Self::analog_voltages_for(
            digital.kernel().layout(),
            self.analog.as_ref(),
            &self.analog_terminal_node_ids,
            &self.last_analog_voltages,
            analog_voltages.as_slice().unwrap(),
        );
        digital.eval_comb_phase(t, nets, &av, event_queue);

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
