//! Program-level compilation: walk the top module's instances and build a
//! ready-to-simulate `CircuitInstance`.
//!
//! The top module is structural — a netlist of instances. Each instantiated
//! module compiles once ([`CompiledModule`], cached) and wraps per-instance
//! into a [`PiperineDevice`]. Instance structure (connections, param
//! overrides) is read straight from the POM `Design`/`Module`/`Instance` —
//! there is no `IrModule`/`IrInstance`/`IrProgram` structural twin; only a
//! module's *own* resolved body ([`crate::lower::pom::LoweredBody`]) is
//! precomputed, by `crate::lower::pom::lower_bodies`.

use std::collections::HashMap;
use std::sync::Arc;

use piperine_lang::pom::{Design, Instance, Module};

use piperine_solver::analog::{Netlist, NodeIdentifier};
use piperine_solver::core::circuit::CircuitInstance;
use piperine_solver::core::device::Device;
use piperine_solver::digital::DigitalNet;
use piperine_solver::digital::scheduler::DigitalState;

use crate::ir::{NodeId, ParamId};
use crate::lower::pom::LoweredBody;
use crate::jit::CodegenError;

use super::{AnalogInstance, CompiledModule, DigitalInstance, PiperineDevice};

/// The ground-node aliases every net namespace accepts (SPEC: gnd-family).
const GROUND_NAMES: &[&str] = &["gnd", "GND", "vss", "VSS", "0"];

/// Everything a caller outside the solver needs to *address* a built
/// circuit by name — the top module's net names, and each instance's
/// compiled kernel/params/terminals for reading a device-internal quantity
/// (e.g. a branch current) that isn't already a solver-level result. Built
/// alongside the circuit by [`CircuitCompiler::build_circuit_mapped`];
/// `build_circuit` discards this and is a thin wrapper over it.
pub struct CircuitBuildInfo {
    /// Top-module net name → solver node (`"gnd"` included, mapping to
    /// [`NodeIdentifier::Gnd`]).
    pub nets: HashMap<String, NodeIdentifier>,
    /// Top-module digital net name → index into
    /// `CircuitInstance::digital_state.nets` — how a bench reads a `Bit`
    /// net's logic value off a result.
    pub digital_nets: HashMap<String, usize>,
    /// One entry per top-level instance, in declaration order.
    pub instances: Vec<BuiltInstanceInfo>,
}

/// A single instantiated device as built into the circuit: its compiled
/// kernel plus everything [`crate::jit::analog::AnalogKernel::eval_residual`]
/// needs to recompute a terminal current outside the solver's own MNA
/// stamping (used to read `.i(a, b)` on a two-terminal device with no force
/// row).
pub struct BuiltInstanceInfo {
    pub label: String,
    pub module: String,
    pub kernel: Arc<crate::jit::analog::AnalogKernel>,
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
}

impl<'p> CircuitCompiler<'p> {
    pub fn new(design: &'p Design, bodies: &'p HashMap<String, LoweredBody>) -> Self {
        Self { design, bodies, kernels: HashMap::new() }
    }

    fn module(&self, name: &str) -> Result<&'p Module, CodegenError> {
        self.design.module(name).ok_or_else(|| CodegenError::ModuleNotFound(name.to_string()))
    }

    fn body(&self, name: &str) -> Result<&'p LoweredBody, CodegenError> {
        self.bodies.get(name).ok_or_else(|| CodegenError::ModuleNotFound(name.to_string()))
    }

    /// The compiled kernels for `module`, compiling on first use.
    pub fn compiled(&mut self, name: &str) -> Result<Arc<CompiledModule>, CodegenError> {
        if let Some(compiled) = self.kernels.get(name) {
            return Ok(compiled.clone());
        }
        let body = self.body(name)?;
        let compiled = Arc::new(CompiledModule::compile(body)?);
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
    /// (a `bench` runner) needs to read a named quantity back out of the
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
        Ok(builder.finish(top))
    }

    /// Evaluate a module's parameter values: defaults in id order (later
    /// defaults may reference earlier parameters), then `overrides`.
    fn param_values(
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
            let value = crate::ir::pom_eval_const(default, &resolve)
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
    fn resolve_node(body: &LoweredBody, name: &str) -> Option<NodeId> {
        if GROUND_NAMES.contains(&name) {
            return Some(NodeId::GROUND);
        }
        body.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
    }
}

/// Accumulates devices, the analog netlist, and the digital net map while
/// walking the top module's instances.
struct InstanceBuilder<'c, 'p> {
    compiler: &'c mut CircuitCompiler<'p>,
    top: &'p Module,
    top_body: &'p LoweredBody,
    top_params: Vec<f64>,
    netlist: Netlist,
    devices: Vec<Box<dyn Device>>,
    digital_nets: HashMap<NodeId, DigitalNet>,
    /// Fresh ids for module-internal analog nodes (top node ids come first).
    next_anon: usize,
    build_info: CircuitBuildInfo,
}

impl<'c, 'p> InstanceBuilder<'c, 'p> {
    fn new(
        compiler: &'c mut CircuitCompiler<'p>,
        top: &'p Module,
        top_body: &'p LoweredBody,
        top_params: Vec<f64>,
    ) -> Self {
        let next_anon = top_body.symbols.nodes().count();
        let nets = top_body
            .symbols
            .nodes()
            .map(|(id, info)| {
                let node = if id.is_ground() { NodeIdentifier::Gnd } else { NodeIdentifier::Anonymous(id.0 as usize) };
                (info.name.clone(), node)
            })
            .collect();
        Self {
            compiler,
            top,
            top_body,
            top_params,
            netlist: Netlist::new(),
            devices: Vec::new(),
            digital_nets: HashMap::new(),
            next_anon,
            build_info: CircuitBuildInfo {
                nets,
                digital_nets: HashMap::new(),
                instances: Vec::new(),
            },
        }
    }

    /// Resolve one instance's port bindings (parent-scope net names) into
    /// this module's `NodeId`s — the structural work `lower_bodies` used to
    /// do once for every module's `IrInstance.connections`; now done here,
    /// once per instantiation, directly from the POM.
    fn resolve_connections(&self, instance: &Instance) -> Result<Vec<NodeId>, CodegenError> {
        instance
            .ports()
            .iter()
            .map(|r| {
                let name = r.to_string();
                CircuitCompiler::resolve_node(self.top_body, &name)
                    .or_else(|| CircuitCompiler::resolve_node(self.top_body, r.net()))
                    .ok_or_else(|| {
                        CodegenError::Invalid(format!(
                            "instance `{}`: unresolved net `{name}`",
                            instance.name()
                        ))
                    })
            })
            .collect()
    }

    /// Resolve one instance's param overrides against the child module's
    /// `ParamId`s (by name) and evaluate each override in the parent scope.
    fn resolve_overrides(
        &self,
        instance: &Instance,
        child_body: &LoweredBody,
    ) -> Result<Vec<(ParamId, f64)>, CodegenError> {
        instance
            .params()
            .iter()
            .map(|(pname, pval)| {
                let id = child_body
                    .symbols
                    .params()
                    .find(|(_, info)| &info.name == pname)
                    .map(|(id, _)| id)
                    .ok_or_else(|| {
                        CodegenError::Invalid(format!(
                            "instance `{}`: unknown parameter override `{pname}`",
                            instance.name()
                        ))
                    })?;
                let expr = crate::lower::pom::structure::value_to_pom_expr(pval);
                let top = &self.top_params;
                let top_body = &self.top_body;
                let resolve = |name: &str| -> Option<f64> {
                    top_body.symbols.params()
                        .find(|(_, p)| p.name == name)
                        .and_then(|(id, _)| top.get(id.0 as usize).copied())
                };
                let value = crate::ir::pom_eval_const(&expr, &resolve)
                    .map_err(CodegenError::ConstEval)?;
                Ok((id, value))
            })
            .collect()
    }

    fn add_instance(&mut self, device_id: usize, instance: &Instance) -> Result<(), CodegenError> {
        let child = self.compiler.module(instance.module_name())?;
        if !child.instances().is_empty() {
            return Err(CodegenError::unsupported(format!(
                "nested hierarchy: `{}` instantiates further modules — flatten during elaboration",
                child.name()
            )));
        }
        if instance.ports().len() != child.ports().len() {
            return Err(CodegenError::Invalid(format!(
                "instance `{}` connects {} nets, module `{}` has {} ports",
                instance.name(),
                instance.ports().len(),
                child.name(),
                child.ports().len()
            )));
        }
        let connections = self.resolve_connections(instance)?;
        let child_body = self.compiler.body(instance.module_name())?;
        let overrides = self.resolve_overrides(instance, child_body)?;
        let compiled = self.compiler.compiled(instance.module_name()).map_err(|e| {
            CodegenError::Invalid(format!(
                "instance `{}` (module `{}`): {e}",
                instance.name(), instance.module_name()
            ))
        })?;

        let params = CircuitCompiler::param_values(child_body, &overrides)?;
        let param_given_mask = overrides
            .iter()
            .fold(0u64, |mask, (id, _)| mask | (1 << id.0.min(63)));

        let analog_terminals: Option<Vec<NodeIdentifier>> = compiled.analog().map(|kernel| {
            // Terminal identifiers: connected parent nodes for ports,
            // fresh anonymous nodes for module-internal terminals.
            kernel
                .terminals()
                .iter()
                .enumerate()
                .map(|(i, _)| match connections.get(i) {
                    Some(&parent) => self.node_identifier(parent),
                    None => {
                        let id = NodeIdentifier::Anonymous(self.next_anon);
                        self.next_anon += 1;
                        id
                    }
                })
                .collect()
        });
        if let (Some(kernel), Some(terminals)) = (compiled.analog(), &analog_terminals) {
            self.build_info.instances.push(BuiltInstanceInfo {
                label: instance.name().to_string(),
                module: instance.module_name().to_string(),
                kernel: kernel.clone(),
                params: params.clone(),
                terminals: terminals.clone(),
                num_forces: kernel.num_forces(),
            });
        }
        let analog = match (compiled.analog(), analog_terminals) {
            (Some(kernel), Some(terminals)) => Some(AnalogInstance::new(
                instance.name(),
                kernel.clone(),
                &terminals,
                params.clone(),
                param_given_mask,
                &mut self.netlist,
            )?),
            _ => None,
        };

        let digital = compiled
            .digital()
            .map(|kernel| {
                let map_nets = |nodes: &[NodeId],
                                nets: &mut HashMap<NodeId, DigitalNet>|
                 -> Result<Vec<DigitalNet>, CodegenError> {
                    nodes
                        .iter()
                        .map(|node| {
                            let port_index = child_body
                                .ports
                                .iter()
                                .position(|p| p.node == *node)
                                .ok_or_else(|| {
                                    CodegenError::unsupported(format!(
                                        "digital net `{}` of `{}` is not a port",
                                        child_body.symbols.node(*node).name,
                                        child.name()
                                    ))
                                })?;
                            let parent = connections[port_index];
                            let next = nets.len();
                            Ok(*nets.entry(parent).or_insert(DigitalNet(next)))
                        })
                        .collect()
                };
                let in_nets = map_nets(kernel.inputs(), &mut self.digital_nets)?;
                let out_nets = map_nets(kernel.outputs(), &mut self.digital_nets)?;
                DigitalInstance::new(kernel.clone(), device_id, in_nets, out_nets, params.clone())
            })
            .transpose()?;

        let mut device = PiperineDevice::new(
            instance.name().to_string(),
            analog,
            digital,
        );

        // For digital-only devices with analog input ports (e.g. a
        // Comparator: `input vp : Electrical, input vn : Electrical,
        // output out : Bit`): wire the analog port terminals into the
        // netlist so the A2D bridge can read their voltages.
        if device.analog.is_none()
            && let Some(digital) = device.digital()
                && digital.kernel().layout().num_analog() > 0 {
                    let mut refs = Vec::new();
                    let mut node_ids = Vec::new();
                    for (port_idx, port) in child_body.ports.iter().enumerate() {
                        // Only ports the digital kernel actually reads as
                        // analog (Electrical) terminals get a netlist node —
                        // a digital-typed port (e.g. `output y : Bit`) must
                        // never allocate an MNA unknown, or it leaves an
                        // unstamped row (`SymbolicSingular`).
                        if digital.kernel().layout().analog_index(port.node).is_none() {
                            refs.push(None);
                            node_ids.push(port.node);
                            continue;
                        }
                        let parent = connections.get(port_idx).copied().unwrap_or(NodeId::GROUND);
                        let node_id = self.node_identifier(parent);
                        let reference = self.netlist.connect_node(node_id);
                        refs.push(reference.idx().map(|_| reference));
                        node_ids.push(port.node);
                    }
                    device.set_analog_terminals(refs, node_ids);
                }

        self.devices.push(Box::new(device));
        Ok(())
    }

    /// Compile the top module's own behavior bodies into a device (SPEC
    /// §7.3, B.1, B.10). The top's analog/digital blocks contribute to
    /// the child instance nodes — parasitic loads, coupling, trim. The
    /// top's NodeIds map directly to netlist nodes.
    fn add_top_behavior_device(&mut self) -> Result<(), CodegenError> {
        let compiled = self.compiler.compiled(self.top.name())?;
        let device_id = self.devices.len();
        let params = self.top_params.clone();
        let param_given_mask = 0u64; // all defaults, no overrides

        let analog = compiled
            .analog()
            .map(|kernel| {
                // Top terminals map directly: NodeId → NodeIdentifier.
                let terminals: Vec<NodeIdentifier> = kernel
                    .terminals()
                    .iter()
                    .map(|&node| self.node_identifier(node))
                    .collect();
                AnalogInstance::new(
                    &format!("{}__top", self.top.name()),
                    kernel.clone(),
                    &terminals,
                    params.clone(),
                    param_given_mask,
                    &mut self.netlist,
                )
            })
            .transpose()?;

        let digital = compiled
            .digital()
            .map(|kernel| {
                let map_nets = |nodes: &[NodeId], nets: &mut HashMap<NodeId, DigitalNet>|
                 -> Result<Vec<DigitalNet>, CodegenError> {
                    nodes
                        .iter()
                        .map(|node| {
                            let next = nets.len();
                            Ok(*nets.entry(*node).or_insert(DigitalNet(next)))
                        })
                        .collect()
                };
                let in_nets = map_nets(kernel.inputs(), &mut self.digital_nets)?;
                let out_nets = map_nets(kernel.outputs(), &mut self.digital_nets)?;
                DigitalInstance::new(kernel.clone(), device_id, in_nets, out_nets, params.clone())
            })
            .transpose()?;

        let mut device = PiperineDevice::new(
            format!("{}__top", self.top.name()),
            analog,
            digital,
        );

        // A2D bridge for digital-only top behavior with analog port reads.
        if device.analog.is_none()
            && let Some(digital) = device.digital()
                && digital.kernel().layout().num_analog() > 0 {
                    let mut refs = Vec::new();
                    let mut node_ids = Vec::new();
                    for port in self.top_body.ports.iter() {
                        if digital.kernel().layout().analog_index(port.node).is_none() {
                            refs.push(None);
                            node_ids.push(port.node);
                            continue;
                        }
                        let node_id = self.node_identifier(port.node);
                        let reference = self.netlist.connect_node(node_id);
                        refs.push(reference.idx().map(|_| reference));
                        node_ids.push(port.node);
                    }
                    device.set_analog_terminals(refs, node_ids);
                }

        self.devices.push(Box::new(device));
        Ok(())
    }

    /// Map a top-module node to a solver identifier. Digital-domain nodes
    /// also pass through here for mixed instances; the analog side sees them
    /// as ordinary nodes.
    fn node_identifier(&self, node: NodeId) -> NodeIdentifier {
        if node.is_ground() {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Anonymous(node.0 as usize)
        }
    }

    fn finish(mut self, title: &str) -> (CircuitInstance, CircuitBuildInfo) {
        let mut circuit = CircuitInstance::from_devices_and_netlist(title, self.devices, self.netlist);
        circuit.digital_state = DigitalState::new(self.digital_nets.len());
        // Name → digital-net index, for bench-side readback.
        for (id, info) in self.top_body.symbols.nodes() {
            if let Some(dn) = self.digital_nets.get(&id) {
                self.build_info.digital_nets.insert(info.name.clone(), dn.0);
            }
        }
        let _ = self.top_params;
        (circuit, self.build_info)
    }
}
