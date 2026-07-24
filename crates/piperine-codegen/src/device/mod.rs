//! The solver boundary: compiled kernels wrapped as [`piperine_solver::prelude::Element`]s.
//!
//! - [`CompiledModule`] — the per-module compilation artifact (analog and/or
//!   digital kernel), shared across instances.
//! - [`PiperineDevice`] — one instance: parameter values, operator state,
//!   register banks, netlist references. Implements the solver `Element`
//!   trait for both domains.
//! - [`CircuitCompiler`] — walks an [`crate::resolve::IrProgram`]'s top module and
//!   builds a ready-to-simulate `CircuitInstance`.

mod analog;
mod builder;
mod circuit;
mod digital;
mod fusion;
mod plugin;

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use num_complex::Complex64;

use piperine_solver::abi::AnalogReference;
use piperine_solver::abi::AcAnalysisContext;
use piperine_solver::abi::{DcAnalysisResult, DcAnalysisState};
use piperine_solver::abi::Noise;
use piperine_solver::abi::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::abi::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
use piperine_solver::abi::{
    Bounds, Direction, Domain, Invalidation, ParamDescriptor, ParamError, ParamScope, Value, ValueKind,
};
use piperine_solver::abi::{TerminalDescriptor, TerminalKind};
use piperine_solver::abi::DigitalEvent;
use piperine_solver::abi::{DigitalPorts, EvalCtx, EventSink};
use piperine_solver::abi::CircularArrayBuffer2;
use piperine_solver::abi::Stamp;
use piperine_solver::abi::Context;

use crate::resolve::{Analysis, NodeId};
use crate::resolve::pom::LoweredBody;
use crate::kernel::analog::AnalogKernel;
use crate::kernel::digital::DigitalKernel;
use crate::error::CodegenError;

pub use analog::AnalogInstance;
pub use circuit::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler};
pub use plugin::{DeviceProvider, PluginDeviceSpec, PluginPort, PortBinding};
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
    /// Compile every behavior body of `module`, including `.disto` kernels.
    pub fn compile(module: &LoweredBody) -> Result<Self, CodegenError> {
        Self::compile_with_options(module, true)
    }

    /// Compile every behavior body of `module`. `compile_disto` gates the
    /// `.disto` 2nd/3rd-derivative kernels (see
    /// [`AnalogKernel::compile_with_options`]) — callers that will never
    /// run `.disto` on this circuit pass `false` to skip that compile cost.
    pub fn compile_with_options(module: &LoweredBody, compile_disto: bool) -> Result<Self, CodegenError> {
        let analog = module
            .analog
            .as_ref()
            .map(|_| AnalogKernel::compile_with_options(module, compile_disto).map(Arc::new))
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
    /// Author-declared introspection metadata resolved from POM attributes
    /// (phdl-introspection-attributes). The `Introspect` bridge prefers a
    /// sidecar field over the codegen-derived default and falls back when
    /// absent (PIA-02/08/12). Empty for a module with no introspection attrs.
    meta: piperine_lang::pom::IntrospectionMeta,
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
        meta: piperine_lang::pom::IntrospectionMeta,
    ) -> Self {
        Self {
            label: label.into(),
            analog,
            digital,
            meta,
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

    /// The display name for the var identified by `kernel_name`: the author-
    /// declared `@name(value)` when present, else the kernel var id. ONE
    /// `@name` feeds both the opvar-query catalog and the observable catalog
    /// (PIA-07 — the inconsistency is dissolved at the source). Absent
    /// `@name` → kernel id unchanged (PIA-08, no regression).
    fn var_display_name(&self, kernel_name: &str) -> String {
        self.meta
            .vars
            .get(kernel_name)
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| kernel_name.to_string())
    }

    /// Map a validated `@kind(value)` canonical string (lowercased
    /// `ObservableKind` variant name, see `pom::introspection::VAR_KINDS`)
    /// onto the solver enum. The `_` arm is unreachable post-resolution
    /// (the lang resolver rejects values outside `VAR_KINDS`); `Var` is the
    /// defensive fallback so a future enum drift never silently panics.
    fn observable_kind_from_str(s: &str) -> piperine_solver::abi::ObservableKind {
        use piperine_solver::abi::ObservableKind;
        match s {
            "branchcurrent" => ObservableKind::BranchCurrent,
            "charge" => ObservableKind::Charge,
            "flux" => ObservableKind::Flux,
            "state" => ObservableKind::State,
            _ => ObservableKind::Var,
        }
    }
}

impl AnalogDevice for PiperineDevice {
    /// Compose the per-instance effective temperature (ABI-21): the solver
    /// or a host temperature sweep passes the ambient/nominal temperature
    /// (`t_nominal`); this override adds the instance `dtemp` (SPICE
    /// convention — an instance param defaulting to 0) and caches
    /// `t_effective = t_nominal + dtemp` on the analog instance. Keeping
    /// the composition in the device keeps the solver's
    /// `CircuitInstance::set_temperature` generic (no param-name
    /// knowledge). The kernel's `$temperature()` syscall still reads
    /// `sim.temperature` (set by `sync_sim` from
    /// `Context.tolerances.temperature`), so the model's own
    /// `var t = $temperature() + dtemp` produces the same effective value
    /// at eval time — the cache is the host/opvar-readable surface.
    fn set_temperature(&mut self, t_nominal: f64) {
        if let Some(analog) = self.analog.as_mut() {
            let dtemp = analog.param("dtemp").unwrap_or(0.0);
            analog.set_temperature(t_nominal + dtemp);
        }
    }

    /// Structured limiting feedback (ABI-09/12): the cached report from the
    /// last `load_dc`/`load_transient` — `Some` when the junction limiter is
    /// clamping. The solver both gates Newton convergence on `is_some()` and
    /// steers the guess from `limited_value`.
    fn limiting_report(&self) -> Option<piperine_solver::abi::LimitingReport> {
        self.analog
            .as_ref()
            .and_then(AnalogInstance::limiting_report)
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

    fn load_disto2(
        &mut self,
        dc_op: &DcAnalysisResult,
        context: &Context,
    ) -> Option<piperine_solver::abi::Disto2> {
        match &mut self.analog {
            Some(analog) => analog.load_disto2(dc_op, context),
            None => None,
        }
    }

    fn load_disto3(
        &mut self,
        dc_op: &DcAnalysisResult,
        context: &Context,
    ) -> Option<piperine_solver::abi::Disto3> {
        match &mut self.analog {
            Some(analog) => analog.load_disto3(dc_op, context),
            None => None,
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

    fn next_breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        match &self.analog {
            Some(analog) => analog.next_breakpoints(from, horizon),
            None => Vec::new(),
        }
    }

    fn suggest_transient_step(
        &self,
        state: &TransientAnalysisState<'_>,
        time_history: &[f64],
        context: &Context,
    ) -> Option<f64> {
        self.analog
            .as_ref()
            .and_then(|a| a.suggest_transient_step(state, time_history, context))
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

    fn digital_hidden_snapshot(&self) -> Option<(Vec<i64>, Vec<f64>)> {
        self.digital.as_ref().and_then(|d| d.hidden_snapshot())
    }

    fn digital_hidden_restore(&mut self, state: &(Vec<i64>, Vec<f64>)) {
        if let Some(d) = self.digital.as_mut() {
            d.hidden_restore(state);
        }
    }
}

impl Introspect for PiperineDevice {
    /// Operating-point variables (ABI-30): the analog kernel's compiled
    /// opvar-eval function evaluated against the last accepted terminal
    /// voltages + state/var banks. Empty for a device whose kernel
    /// compiled no opvar path (no analog vars, or no analog body at all).
    /// The surfaced name is the author-declared `@name(value)` when present,
    /// else the kernel var id (PIA-07); the value is always looked up by the
    /// kernel id (renaming the label never breaks the value fetch).
    fn read_opvars(&self) -> Vec<(String, f64)> {
        self.analog.as_ref().map_or_else(Vec::new, |a| {
            a.eval_opvars()
                .into_iter()
                .map(|(name, value)| (self.var_display_name(&name), value))
                .collect()
        })
    }

    /// Declared query catalog (ABI-31 / PIA-05): one `QueryDescriptor` per
    /// opvar, typed [`QueryKind::OperatingVariable`]. The name is the author-
    /// declared `@name(value)` when present (PIA-07); `@unit`/`@description`
    /// annotate the descriptor when declared. Absent attributes → the bare
    /// `QueryDescriptor::opvar(name)` shape (PIA-08, no regression).
    fn list_queries(&self) -> Vec<piperine_solver::abi::QueryDescriptor> {
        let Some(a) = &self.analog else { return Vec::new() };
        let names = a.kernel().opvar_names();
        if names.is_empty() {
            return Vec::new();
        }
        names
            .iter()
            .map(|n| {
                let Some(meta) = self.meta.vars.get(n) else {
                    return piperine_solver::abi::QueryDescriptor::opvar(n.clone());
                };
                piperine_solver::abi::QueryDescriptor {
                    name: meta.name.clone().unwrap_or_else(|| n.clone()),
                    kind: piperine_solver::abi::QueryKind::OperatingVariable,
                    unit: meta.unit.clone(),
                    description: meta.description.clone(),
                }
            })
            .collect()
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
                    // Presence-queried, never-given optional params are
                    // structural to write (the given-mask is baked at build)
                    // — same classification as `set_param` (LIVE-14).
                    invalidation: if analog.set_flips_presence(name) {
                        Invalidation::Rebuild
                    } else {
                        Invalidation::Restamp
                    },
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

    /// Bridge the analog kernel's terminal catalog to the introspection
    /// surface (ABI-27): one [`TerminalDescriptor`] per kernel terminal,
    /// named from the symbol table. Ports are [`TerminalKind::External`];
    /// module-internal nodes referenced by the body (a series-R/thermal
    /// `wire`, an `idt` accumulator's hidden probe, …) are
    /// [`TerminalKind::Internal`]. Digital-domain terminal pairs from the
    /// digital kernel are appended (ABI-28): one descriptor per input +
    /// per output, carrying [`Domain::Digital`] + the matching direction.
    fn list_terminals(&self) -> Vec<TerminalDescriptor> {
        let mut out = Vec::new();
        if let Some(analog) = &self.analog {
            let kernel = analog.kernel();
            let num_ports = kernel.num_ports();
            for (i, _) in kernel.terminals().iter().enumerate() {
                let name = kernel.terminal_name(i);
                let direction = if i < num_ports {
                    Direction::Inout
                } else {
                    Direction::Out
                };
                let mut desc = TerminalDescriptor::new(name, Domain::Analog, direction);
                desc.kind = if i < num_ports {
                    TerminalKind::External
                } else {
                    TerminalKind::Internal
                };
                out.push(desc);
            }
        }
        if let Some(digital) = &self.digital {
            let kernel = digital.kernel();
            for (i, _) in kernel.inputs().iter().enumerate() {
                let mut desc = TerminalDescriptor::new(
                    kernel.input_name(i),
                    Domain::Digital,
                    Direction::In,
                );
                desc.kind = TerminalKind::External;
                out.push(desc);
            }
            for (i, _) in kernel.outputs().iter().enumerate() {
                let mut desc = TerminalDescriptor::new(
                    kernel.output_name(i),
                    Domain::Digital,
                    Direction::Out,
                );
                desc.kind = TerminalKind::External;
                out.push(desc);
            }
        }
        out
    }

    /// Model identity (ABI-46): the kernel's module name as `type_id`, no
    /// version (the language has no version declaration today — empty
    /// string is the documented "unversioned" sentinel). A host uses this
    /// to render family-specific UI without name-matching.
    fn model_descriptor(&self) -> piperine_solver::abi::ModelDescriptor {
        // PIA-01: an author-declared `@model(type, version)` on the module
        // populates the descriptor from the sidecar. PIA-02: absent `@model`
        // falls back to today's module-name echo with empty version — no
        // regression for stdlib models that carry no `@model`.
        if let Some(model) = &self.meta.model {
            return piperine_solver::abi::ModelDescriptor {
                type_id: model.type_id.clone(),
                version: model.version.clone(),
            };
        }
        let type_id = self
            .analog
            .as_ref()
            .map(|a| a.kernel().name().to_string())
            .or_else(|| self.digital.as_ref().map(|d| d.kernel().name().to_string()))
            .unwrap_or_default();
        piperine_solver::abi::ModelDescriptor { type_id, version: String::new() }
    }

    /// Per-slot names for the analog kernel's runtime state bank (ABI-47):
    /// runtime operators (`ddt`, `delay`, `idt`, …) named by kind + slot
    /// id; trailing `$limit` vold slots named `vold[k]`. Empty for a
    /// digital-only device.
    fn list_state_slot_names(&self) -> Vec<String> {
        self.analog
            .as_ref()
            .map(|a| a.kernel().state_slot_names().to_vec())
            .unwrap_or_default()
    }

    /// Named `(plus, minus)` terminal pairs per force branch (ABI-47),
    /// bridged from `AnalogKernel::force_terminals` via the symbol table.
    fn list_force_terminal_pairs(&self) -> Vec<(String, String)> {
        self.analog
            .as_ref()
            .map(|a| a.kernel().force_terminal_name_pairs())
            .unwrap_or_default()
    }

    /// Named `(plus, minus)` terminal pairs per noise source (ABI-47),
    /// bridged from `AnalogKernel::noise_terminals` via the symbol table.
    fn list_noise_terminal_pairs(&self) -> Vec<(String, String)> {
        self.analog
            .as_ref()
            .map(|a| a.kernel().noise_terminal_name_pairs())
            .unwrap_or_default()
    }

    /// Device-declared observables for per-step recording (ABI-32): one
    /// descriptor per kernel state slot (kind = `State`), per module var
    /// slot (kind = `Var`, synthesized `var[k]` name when the kernel
    /// exposes no source-level var names), and per force branch carrying
    /// a series-R current term (kind = `BranchCurrent`, named
    /// `i(<plus>,<minus>)`). The host pairs this catalog with a
    /// [`ProbeSelection`](piperine_solver::abi::ProbeSelection) to record
    /// a subset of these per step. Empty for a digital-only device.
    fn list_observables(&self) -> Vec<piperine_solver::abi::ObservableDescriptor> {
        use piperine_solver::abi::{ObservableDescriptor, ObservableKind};
        let mut out = Vec::new();
        let Some(analog) = &self.analog else { return out };
        let kernel = analog.kernel();
        for name in kernel.state_slot_names() {
            if name.is_empty() {
                continue;
            }
            out.push(ObservableDescriptor {
                name: name.clone(),
                kind: ObservableKind::State,
                cost: 0.2,
            });
        }
        for k in 0..kernel.num_vars() {
            // PIA-06/07: a var carrying `@name`/`@kind` surfaces in the
            // observable catalog by that name and kind (NOT positional
            // `var[k]`). The same `@name` used by `list_queries`/
            // `read_opvars` is read here — one declaration, both catalogs.
            // Absent `@name` keeps today's positional `var[k]` (PIA-08).
            let src_name = kernel.var_names().get(k).map(|s| s.as_str()).unwrap_or("");
            let (name, kind) = self
                .meta
                .vars
                .get(src_name)
                .and_then(|m| m.name.as_ref().map(|label| (label.clone(), m.kind.as_deref())))
                .map(|(label, kind)| {
                    (label, kind.map_or(ObservableKind::Var, Self::observable_kind_from_str))
                })
                .unwrap_or_else(|| (format!("var[{k}]"), ObservableKind::Var));
            out.push(ObservableDescriptor {
                name,
                kind,
                cost: 0.1,
            });
        }
        if kernel.has_force_current() {
            for (plus, minus) in kernel.force_terminal_name_pairs() {
                out.push(ObservableDescriptor {
                    name: format!("i({plus},{minus})"),
                    kind: ObservableKind::BranchCurrent,
                    cost: 0.3,
                });
            }
        }
        out
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
        // ABI-04/ABI-05: a device with a `$limit` limiter or stateful digital
        // registers owns mutable non-accept-gated state the solver must
        // checkpoint/restore around rejected steps.
        if self.analog.as_ref().is_some_and(AnalogInstance::has_limiter)
            || self.digital.as_ref().is_some_and(DigitalInstance::has_registers)
        {
            caps |= ElementCapabilities::SUPPORTS_ROLLBACK;
        }
        // ABI-26: a JIT device declares the analytic derivative orders its
        // kernel compiled — symbolic differentiation always produces one, so
        // every nonlinear JIT device sets the matching bit(s). The `.disto`
        // driver checks these before solving for HD2/HD3 (ABI-24); a fully
        // linear device (resistor) compiles no disto kernels and sets
        // neither. `NUMERIC_JACOBIAN` is never set by the JIT (its Jacobian
        // is always analytic) — that bit is reserved for finite-difference
        // plugins.
        if let Some(analog) = &self.analog {
            let kernel = analog.kernel();
            if kernel.has_disto2() {
                caps |= ElementCapabilities::HAS_DISTO2;
            }
            if kernel.has_disto3() {
                caps |= ElementCapabilities::HAS_DISTO3;
            }
        }
        caps
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

    fn runtime_banks(&self) -> (&[f64], &[f64]) {
        self.analog.as_ref().map(|a| a.runtime_banks()).unwrap_or((&[], &[]))
    }

    /// Checkpoint the device's mutable non-accept-gated state (ABI-04/05):
    /// the limiter (active, seeds, vold) from the analog instance and the
    /// register banks (vars_int, vars_real, prev_watch) from the digital
    /// instance. Returns `None` when the device owns no such state.
    /// Layout: `int_state` carries the digital registers; `real_state` is
    /// `[limiter_pack..., digital_vars_real...]`.
    fn checkpoint_state(&self) -> Option<piperine_solver::abi::ElementCheckpoint> {
        let limiter = self.analog.as_ref().and_then(AnalogInstance::checkpoint_limiter);
        let digital = self
            .digital
            .as_ref()
            .and_then(DigitalInstance::checkpoint_registers);
        match (limiter, digital) {
            (None, None) => None,
            (Some(lim), None) => Some(lim),
            (None, Some((dig_int, dig_real))) => Some(piperine_solver::abi::ElementCheckpoint {
                int_state: dig_int,
                real_state: dig_real,
            }),
            (Some(mut lim), Some((dig_int, dig_real))) => {
                // Limiter's real_state goes first; digital real vars follow.
                lim.real_state.extend(dig_real);
                lim.int_state = dig_int;
                Some(lim)
            }
        }
    }

    /// Restore device state from a checkpoint produced by
    /// [`checkpoint_state`](Self::checkpoint_state) (ABI-04/05): rewinds the
    /// limiter (reading its own leading slice of `real_state`) and the
    /// digital registers (`int_state` + the trailing `real_state`).
    fn restore_state(&mut self, checkpoint: &piperine_solver::abi::ElementCheckpoint) {
        let limiter_len = self
            .analog
            .as_ref()
            .map_or(0, AnalogInstance::limiter_checkpoint_len);
        if let Some(analog) = self.analog.as_mut() {
            analog.restore_limiter(checkpoint);
        }
        if let Some(digital) = self.digital.as_mut() {
            let dig_real = if checkpoint.real_state.len() > limiter_len {
                &checkpoint.real_state[limiter_len..]
            } else {
                &[]
            };
            digital.restore_registers(&checkpoint.int_state, dig_real);
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
        layout: &crate::kernel::digital::DigitalLayout,
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
