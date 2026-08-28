//! Program-level compilation: walk the top module's instances and build a
//! ready-to-simulate `CircuitInstance`.
//!
//! The top module is structural — a netlist of instances. Each instantiated
//! module compiles once ([`CompiledModule`], cached) and wraps per-instance
//! into a [`super::PiperineDevice`]. Instance structure (connections, param
//! overrides) is read straight from the POM `Design`/`Module`/`Instance`.
//! The only precomputed input is each module's *own* resolved body
//! ([`crate::resolve::pom::LoweredBody`]), produced by
//! `crate::resolve::pom::lower_bodies`.

use std::collections::HashMap;
use std::sync::Arc;

use piperine_lang::pom::{Design, IntrospectionMeta, Module};

use piperine_solver::abi::NodeIdentifier;
use piperine_solver::abi::CircuitInstance;
use piperine_solver::abi::{DigitalNet, PathDelay};

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
    /// Top-module net name → declared nodeset value (§3.6.3.2, LRMW2-10,
    /// T14) — a host-supplied `OpConfig.nodeset` entry for the same name
    /// still wins over this (see `piperine-api`'s `build_ivs`).
    pub nodesets: HashMap<String, f64>,
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
    /// The host-visible name for each entry of `kernel.opvar_names()`, in
    /// the same order (host-library HOST-08): the `@name(value)` label when
    /// the module var declares one, else the kernel id unchanged — the same
    /// resolution `PiperineDevice::read_opvars`/`var_display_name` applies,
    /// mirrored here so a recorded trace can address an opvar by the same
    /// name a live `OpResult::instance(...).opvar(...)` read would use.
    pub opvar_display_names: Vec<String>,
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
    /// Resolved device-introspection metadata sidecars, one per module name
    /// (phdl-introspection-attributes). Cached like `kernels` so a module
    /// instantiated many times resolves its `@model`/`@name`/`@unit`/
    /// `@description`/`@kind` attributes once. Built from POM attributes by
    /// [`Design::introspection_meta`][piperine_lang::pom::Design::introspection_meta].
    meta: HashMap<String, IntrospectionMeta>,
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
        Self { design, bodies, kernels: HashMap::new(), meta: HashMap::new(), provider: None, fuse_digital_cones: true, compile_disto: true }
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

    /// The flattened (leaf-only) form of module `name`, for codegen's ROOT
    /// lookup. `Design::flat_module` falls back to the authored module when
    /// no flat form is recorded — a leaf's flat form equals its authored
    /// form, so existing two-level circuits are unchanged. A mid-level root
    /// (one that instantiates sub-modules) resolves to its spliced-flat form
    /// here, making the `builder.rs` nested-hierarchy guard unreachable for
    /// valid flattened input. FLAT-01.
    pub(super) fn flat_module(&self, name: &str) -> Result<&'p Module, CodegenError> {
        self.design.flat_module(name).ok_or_else(|| CodegenError::ModuleNotFound(name.to_string()))
    }

    /// A UDP declaration by name (Annex A.5, LRMC-C7-13 T39/T40) — `None`
    /// when `name` isn't a registered `primitive` (the common case: an
    /// ordinary module instantiation).
    pub(super) fn udp(&self, name: &str) -> Option<&'p piperine_lang::ast::PrimitiveDecl> {
        self.design.udp(name)
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

    /// Resolve (and cache) the device-introspection metadata sidecar for
    /// `module_name` — author-declared `@model`/`@name`/`@unit`/`@description`/
    /// `@kind` attributes read off the POM module. A bad attribute (placement
    /// error, `@kind` value outside its target enum, duplicate `@name`) fails
    /// loud here as a [`CodegenError::Invalid`], mirroring how the `@rfport`
    /// resolver surfaces its validation failures at the consumer boundary.
    /// Cached so a module instantiated many times resolves once.
    pub(super) fn introspection_meta(&mut self, module_name: &str) -> Result<IntrospectionMeta, CodegenError> {
        if let Some(m) = self.meta.get(module_name) {
            return Ok(m.clone());
        }
        let m = self
            .design
            .introspection_meta(module_name)
            .map_err(|e| CodegenError::Invalid(e.to_string()))?;
        self.meta.insert(module_name.to_string(), m.clone());
        Ok(m)
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
        // The root comes from the FLAT netlist (`Design::flat_modules`) — a
        // mid-level module's sub-instances have been spliced into its flat
        // form by `FlattenHierarchy`, so the root seen here contains only
        // leaf devices. Leaf children are still read via `module()` (their
        // flat form equals their authored form). FLAT-01.
        let top_module = self.flat_module(top)?;
        let top_body = self.body(top)?;

        let top_params = Self::param_values(top_body, &[])?;
        let mut builder = InstanceBuilder::new(self, top_module, top_body, top_params);
        for (index, instance) in top_module.instances().iter().enumerate() {
            builder.add_instance(index, instance)?;
        }
        // Gate primitives (Annex A.4.2, LRMC-C7-13, T40) — `Module::gates`
        // entries never appear in `instances()` (a gate is not a module
        // instance), so they get their own walk, continuing the device-id
        // sequence.
        for gate in &top_module.gates {
            let device_id = builder.devices.len();
            builder.add_gate_instance(device_id, gate)?;
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
        let (mut circuit, info) = builder.finish(top)?;
        Self::populate_specify_delays(&mut circuit, &info, self.design, top_module);
        Ok((circuit, info))
    }

    /// T19 (LRMW3-06): each `specify` path delay writes a `PathDelay`
    /// record — keyed by destination net, via `CircuitBuildInfo`'s own
    /// name→digital-net-index map (`info.digital_nets`, the same lookup a
    /// host uses to read a digital net's value back) — into the built
    /// circuit's `DelayTable`. Two sources of `specify` blocks are
    /// checked: `top_module`'s own (rare — a pure structural wrapper
    /// normally declares none) and each leaf instance's own module (the
    /// common case — a `specify` block lives on the model with the
    /// behavior it times), translated from the instantiated module's own
    /// port names into `top_module`'s net names through that instance's
    /// positional port connections.
    ///
    /// A destination terminal that never became a digital net (e.g. a
    /// purely analog port sharing the specify block's scope) has nothing
    /// to key a digital delay by and is skipped, not an error — the
    /// terminal name itself was already validated against the module's
    /// port list at POM-lowering time (T18, A.7.3).
    ///
    /// **Disclosed deviation** (design.md C5): `DelayTable` is per
    /// destination *net*, not per source→destination *path* — two paths
    /// into the same net collapse onto whichever is written last. Kept as
    /// the solver's existing data model (T60), not reopened here.
    fn populate_specify_delays(circuit: &mut CircuitInstance, info: &CircuitBuildInfo, design: &Design, top_module: &Module) {
        Self::apply_specify_paths(circuit, info, top_module.specify(), top_module.timescale, |name| Some(name.to_string()));

        for inst in top_module.instances() {
            let Some(callee) = design.module(&inst.module) else { continue };
            let sb = callee.specify();
            if sb.paths.is_empty() {
                continue;
            }
            let translate = |name: &str| -> Option<String> {
                let idx = callee.ports().iter().position(|p| p.name() == name)?;
                inst.ports.get(idx).map(|nr| nr.net().to_string())
            };
            Self::apply_specify_paths(circuit, info, sb, callee.timescale, translate);
        }
    }

    /// Shared body of [`Self::populate_specify_delays`]'s two passes:
    /// const-evaluated (T18) delays, quantized to `timescale` (T19), for
    /// every path in `sb`, keyed by `translate`d destination-terminal
    /// names into `info.digital_nets`.
    fn apply_specify_paths(
        circuit: &mut CircuitInstance,
        info: &CircuitBuildInfo,
        sb: &piperine_lang::pom::SpecifyBlock,
        timescale: Option<piperine_lang::directives::Timescale>,
        translate: impl Fn(&str) -> Option<String>,
    ) {
        for path in &sb.paths {
            let quantized: Vec<f64> = path.delays.iter().map(|&d| quantize_to_timescale(d, timescale)).collect();
            let delay = path_delay_from_values(&quantized);
            for out in &path.outputs {
                let Some(top_name) = translate(out) else { continue };
                if let Some(&idx) = info.digital_nets.get(&top_name) {
                    circuit.digital_state.delay_table.set(DigitalNet(idx), delay);
                }
            }
        }
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
    /// access (`load.p`) both funnel through this at circuit-build time: this
    /// is the single place instance-level net resolution happens.
    pub(super) fn resolve_node(body: &LoweredBody, name: &str) -> Option<NodeId> {
        if body.symbols.is_ground(name) {
            return Some(NodeId::GROUND);
        }
        body.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
    }

    /// Resolve one element of a vector or memory net by its base name and
    /// index path (T4) — outermost index first, e.g. `["3"]` for
    /// `bus[3]`, `[2, 0]` for a memory's `mem[2][0]`. Builds the element
    /// name through the same [`crate::resolve::pom::structure::
    /// element_node_name`] the registration side used, so a name this
    /// resolves always matches one `build_symbols_and_ports` actually
    /// registered.
    #[allow(dead_code)] // consumed by the Phase-2 vector-access tasks (T7/T9)
    pub(super) fn resolve_element_node(body: &LoweredBody, base: &str, indices: &[u64]) -> Option<NodeId> {
        let name = indices
            .iter()
            .fold(base.to_string(), |acc, i| crate::resolve::pom::structure::element_node_name(&acc, *i));
        Self::resolve_node(body, &name)
    }
}

/// Rounds `value` to the nearest multiple of the module's `` `timescale ``
/// precision grid (T19, LRM §8.4.3.3's rule, the same formula
/// `DigitalKernel::quantize_time` applies to runtime event times — kept in
/// sync with it) — a module with no `` `timescale `` directive quantizes
/// nothing.
fn quantize_to_timescale(value: f64, ts: Option<piperine_lang::directives::Timescale>) -> f64 {
    match ts.map(|t| 10f64.powi(t.precision_exponent)) {
        Some(grid) if grid > 0.0 => (value / grid).round() * grid,
        _ => value,
    }
}

/// Builds a solver `PathDelay` triad from a specify path's const-evaluated
/// delay list (A.7.4): 1 value is uniform; 2 values are rise/fall with
/// `turn_off` defaulting to their minimum (1364's own default-delay rule);
/// 3+ values take the first three as rise/fall/turn_off — the wider
/// 6/12-value forms (0-to-1/1-to-0/…/z-transition triads) collapse onto
/// this same triad, a deliberate simplification matching `DelayTable`'s
/// own disclosed per-net (not per-transition-pair) model.
fn path_delay_from_values(delays: &[f64]) -> PathDelay {
    match delays.len() {
        0 => PathDelay::default(),
        1 => PathDelay::uniform(delays[0]),
        2 => PathDelay { rise: delays[0], fall: delays[1], turn_off: delays[0].min(delays[1]) },
        _ => PathDelay { rise: delays[0], fall: delays[1], turn_off: delays[2] },
    }
}

#[cfg(test)]
mod tests {
    //! T4 (LRMW3-01/LRMW3-04): a round trip through the two call sites
    //! `element_node_name` owns the naming for — registration
    //! (`resolve::pom::structure::build_symbols_and_ports`) and lookup
    //! (`resolve_element_node`, here) — every name the registration side
    //! generates for a vector wire must resolve back through the lookup
    //! side to the exact node it was generated for.

    use super::CircuitCompiler;
    use crate::resolve::pom::lower_bodies;
    use piperine_lang::{parse_and_elaborate, SourceMap};

    #[test]
    fn resolve_element_node_round_trips_every_registered_element() {
        let src = "module Mid ();\n electrical tap[2:0];\nendmodule\n";
        let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
        let bodies = lower_bodies(&design).expect("lowers");
        let body = &bodies["Mid"];
        for i in 0..3u64 {
            let by_name = CircuitCompiler::resolve_node(body, &format!("tap[{i}]"));
            let by_index = CircuitCompiler::resolve_element_node(body, "tap", &[i]);
            assert!(by_name.is_some(), "tap[{i}] must resolve by name");
            assert_eq!(by_index, by_name, "resolve_element_node(tap, [{i}]) must match resolve_node(\"tap[{i}]\")");
        }
    }
}
