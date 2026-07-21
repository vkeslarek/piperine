//! Program-level compilation: walk the top module's instances and build a
//! ready-to-simulate `CircuitInstance`.
//!
//! The top module is structural — a netlist of instances. Each instantiated
//! module compiles once ([`CompiledModule`], cached) and wraps per-instance
//! into a [`super::PiperineDevice`]. Instance structure (connections, param
//! overrides) is read straight from the POM `Design`/`Module`/`Instance` —
//! there is no `IrModule`/`IrInstance`/`IrProgram` structural twin; only a
//! module's *own* resolved body ([`crate::resolve::pom::LoweredBody`]) is
//! precomputed, by `crate::resolve::pom::lower_bodies`.

use std::collections::HashMap;
use std::sync::Arc;

use piperine_lang::pom::{Design, Module};

use piperine_solver::abi::NodeIdentifier;
use piperine_solver::abi::CircuitInstance;

use crate::resolve::{NodeId, ParamId};
use crate::resolve::pom::LoweredBody;
use crate::error::CodegenError;

use super::CompiledModule;
use super::builder::InstanceBuilder;

/// Everything a caller outside the solver needs to *address* a built
/// circuit by name — the top module's net names, and each instance's
/// compiled kernel/params/terminals for reading a device-internal quantity
/// (e.g. a branch current) that isn't already a solver-level result. Built
/// alongside the circuit by [`CircuitCompiler::build_circuit_mapped`];
/// `build_circuit` discards this and is a thin wrapper over it.
#[derive(Clone)]
pub struct CircuitBuildInfo {
    /// Top-module net name → solver node (`"gnd"` included, mapping to
    /// [`NodeIdentifier::Gnd`]).
    pub nets: HashMap<String, NodeIdentifier>,
    /// Top-module digital net name → index into
    /// `CircuitInstance::digital_state.nets` — how a host reads a `Bit`
    /// net's logic value off a result.
    pub digital_nets: HashMap<String, usize>,
    /// One entry per top-level instance, in declaration order.
    pub instances: Vec<BuiltInstanceInfo>,
    /// How many fused combinational cones were compiled into single
    /// `DigitalNetwork` elements during the build (SC-13 instrumentation —
    /// a circuit with comb cones proves fusion is active when this is > 0).
    pub fused_networks: usize,
}

/// A single instantiated device as built into the circuit: its compiled
/// kernel plus everything [`crate::kernel::analog::AnalogKernel::eval_residual`]
/// needs to recompute a terminal current outside the solver's own MNA
/// stamping (used to read `.i(a, b)` on a two-terminal device with no force
/// row).
#[derive(Clone)]
pub struct BuiltInstanceInfo {
    pub label: String,
    pub module: String,
    pub kernel: Arc<crate::kernel::analog::AnalogKernel>,
    pub params: Vec<f64>,
    pub terminals: Vec<NodeIdentifier>,
    /// Number of MNA branch-current unknowns this instance owns
    /// (`BranchIdentifier::new(label, "force{i}")`, i < num_forces) — a
    /// nonzero count means the current is already a solver variable and
    /// should be read via `DcAnalysisResult::get_branch`, not recomputed.
    pub num_forces: usize,
}

/// Compiles a POM [`Design`] into solver circuits. `bodies` is every
/// module's resolved lowering (`lower_bodies`), computed once by the
/// caller and kept alive alongside `design` — both outlive this compiler.
/// Kernels are cached per module name, so instantiating a module many times
/// compiles it once.
pub struct CircuitCompiler<'p> {
    design: &'p Design,
    bodies: &'p HashMap<String, LoweredBody>,
    kernels: HashMap<String, Arc<CompiledModule>>,
    /// Builds `@device`-annotated instances (SPEC Part VI §7). `None` means
    /// no plugin host is wired — a `@device` instance then fails loud.
    pub(super) provider: Option<&'p dyn super::plugin::DeviceProvider>,
    /// Fuse connected pure-combinational digital cones into single
    /// `DigitalNetwork` elements (Verilator-style whole-cone evaluation).
    /// `false` keeps every digital instance on the per-device path — the
    /// bit-exact reference the fused path is validated against.
    pub fuse_digital_cones: bool,
    /// Whether compiled analog kernels include the `.disto` 2nd/3rd-derivative
    /// kernels (see [`crate::kernel::analog::AnalogKernel::compile_with_options`]).
    /// Defaults to `true` (matches the pre-flag behavior); callers that know
    /// `.disto` will never run on this circuit call [`Self::with_disto`]`(false)`
    /// to skip that compile cost.
    compile_disto: bool,
}

impl<'p> CircuitCompiler<'p> {
    pub fn new(design: &'p Design, bodies: &'p HashMap<String, LoweredBody>) -> Self {
        Self { design, bodies, kernels: HashMap::new(), provider: None, fuse_digital_cones: true, compile_disto: true }
    }

    /// Wire a plugin host as the builder for `@device` instances.
    pub fn with_device_provider(mut self, provider: &'p dyn super::plugin::DeviceProvider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Gate whether compiled analog kernels include the `.disto`
    /// 2nd/3rd-derivative kernels (default `true`). Pass `false` when this
    /// circuit will never run `.disto` — a many-branch device otherwise
    /// pays a real Cranelift compile cost for kernels it never uses.
    pub fn with_disto(mut self, enabled: bool) -> Self {
        self.compile_disto = enabled;
        self
    }

    pub(super) fn module(&self, name: &str) -> Result<&'p Module, CodegenError> {
        self.design.module(name).ok_or_else(|| CodegenError::ModuleNotFound(name.to_string()))
    }

    pub(super) fn body(&self, name: &str) -> Result<&'p LoweredBody, CodegenError> {
        self.bodies.get(name).ok_or_else(|| CodegenError::ModuleNotFound(name.to_string()))
    }

    /// The compiled kernels for `module`, compiling on first use.
    pub fn compiled(&mut self, name: &str) -> Result<Arc<CompiledModule>, CodegenError> {
        if let Some(compiled) = self.kernels.get(name) {
            return Ok(compiled.clone());
        }
        let body = self.body(name)?;
        let compiled = Arc::new(CompiledModule::compile_with_options(body, self.compile_disto)?);
        self.kernels.insert(name.to_string(), compiled.clone());
        Ok(compiled)
    }

    /// Build the circuit rooted at module `top`. The top may have both
    /// child instances and its own behavior bodies (SPEC §7.3, B.1, B.10):
    /// the parent's `analog`/`digital` blocks contribute to the children's
    /// port nodes (KCL accumulation — parasitic load, coupling, trim).
    pub fn build_circuit(&mut self, top: &str) -> Result<CircuitInstance, CodegenError> {
        self.build_circuit_mapped(top).map(|(circuit, _)| circuit)
    }

    /// Like [`Self::build_circuit`], but also returns a [`CircuitBuildInfo`]
    /// mapping the top module's net names and each instance's compiled
    /// kernel/params/terminals — everything a caller outside the solver
    /// (a host) needs to read a named quantity back out of the
    /// built circuit.
    pub fn build_circuit_mapped(
        &mut self,
        top: &str,
    ) -> Result<(CircuitInstance, CircuitBuildInfo), CodegenError> {
        let top_module = self.module(top)?;
        let top_body = self.body(top)?;

        let top_params = Self::param_values(top_body, &[])?;
        let mut builder = InstanceBuilder::new(self, top_module, top_body, top_params);
        for (index, instance) in top_module.instances().iter().enumerate() {
            builder.add_instance(index, instance)?;
        }
        // SPEC §7.3, B.1, B.10: if the top has its own behavior bodies AND
        // child instances, compile the top's behavior into a device that
        // stamps contributions (parasitic loads, coupling) at the child
        // instance nodes. A leaf top (behavior but no instances) produces
        // an empty circuit.
        if !top_module.instances().is_empty()
            && (top_body.analog.is_some() || top_body.digital.is_some())
        {
            builder.add_top_behavior_device()?;
        }
        builder.finish(top)
    }

    /// Evaluate a module's parameter values: defaults in id order (later
    /// defaults may reference earlier parameters), then `overrides`.
    pub(super) fn param_values(
        body: &LoweredBody,
        overrides: &[(ParamId, f64)],
    ) -> Result<Vec<f64>, CodegenError> {
        let mut values: Vec<Option<f64>> = vec![None; body.symbols.num_params()];
        for (id, info) in body.symbols.params() {
            if let Some((_, v)) = overrides.iter().find(|(o, _)| *o == id) {
                values[id.0 as usize] = Some(*v);
                continue;
            }
            let default = info.default.as_ref().ok_or_else(|| {
                CodegenError::ConstEval(format!(
                    "parameter `{}` of `{}` has no default and no override",
                    info.name, body.name
                ))
            })?;
            let resolve = |name: &str| -> Option<f64> {
                body.symbols.params()
                    .find(|(_, p)| p.name == name)
                    .and_then(|(id, _)| values.get(id.0 as usize).copied().flatten())
            };
            let value = crate::resolve::pom_eval_const(default, &resolve)
                .map_err(CodegenError::ConstEval)?;
            values[id.0 as usize] = Some(value);
        }
        Ok(values.into_iter().map(|v| v.expect("all params filled")).collect())
    }

    /// Resolve a net name in `body`'s own node namespace: ground aliases,
    /// then the module's node table. Instance connections and named-port
    /// access (`load.p`) both funnel through this at circuit-build time —
    /// the same resolution `lower_bodies` used to do for the (now deleted)
    /// `IrInstance.connections` structural twin.
    pub(super) fn resolve_node(body: &LoweredBody, name: &str) -> Option<NodeId> {
        if piperine_lang::pom::is_ground(name) {
            return Some(NodeId::GROUND);
        }
        body.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
    }
}
