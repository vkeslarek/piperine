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
use piperine_solver::device::Device;
use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};
use piperine_solver::math::circular_array::CircularArrayBuffer2;
use piperine_solver::math::linear::Stamp;
use piperine_solver::solver::Context;

use crate::ir::{Analysis, IrModule};
use crate::jit::analog::AnalogKernel;
use crate::jit::digital::DigitalKernel;
use crate::jit::CodegenError;

pub use analog::AnalogInstance;
pub use circuit::CircuitCompiler;
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
    pub fn compile(module: &IrModule) -> Result<Self, CodegenError> {
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
}

impl PiperineDevice {
    pub fn new(
        label: impl Into<String>,
        analog: Option<AnalogInstance>,
        digital: Option<DigitalInstance>,
    ) -> Self {
        Self { label: label.into(), analog, digital }
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

    // ── Analog ──

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

    fn accept_timestep(&mut self, state: &CircularArrayBuffer2<f64>, ctx: &Context) {
        if let Some(analog) = &mut self.analog {
            analog.accept_timestep(state, ctx);
        }
    }

    fn noise_current_psd(
        &mut self,
        dc_point: &DcAnalysisResult,
        _ac_ctx: &AcAnalysisContext,
    ) -> Vec<Noise> {
        match &mut self.analog {
            Some(analog) => analog.noise_current_psd(dc_point),
            None => Vec::new(),
        }
    }

    // ── Digital ──

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
        _analog_voltages: &[f64],
        event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        if let Some(digital) = &mut self.digital {
            digital.eval(t, nets, event_queue);
        }
    }
}

/// Map the IR analysis enum to the `SimCtx.current_analysis` encoding.
fn analysis_code(analysis: Analysis) -> u64 {
    analysis as u64
}
