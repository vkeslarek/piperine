//! Program-level compilation: walk the top module's instances and build a
//! ready-to-simulate `CircuitInstance`.
//!
//! The top module is structural — a netlist of instances. Each instantiated
//! module compiles once ([`CompiledModule`], cached) and wraps per-instance
//! into a [`PiperineDevice`].

use std::collections::HashMap;
use std::sync::Arc;

use piperine_solver::analog::{Netlist, NodeIdentifier};
use piperine_solver::circuit::CircuitInstance;
use piperine_solver::device::Device;
use piperine_solver::digital::DigitalNet;
use piperine_solver::topology::DigitalState;

use crate::ir::{IrInstance, IrModule, IrProgram, NodeId, ParamId};
use crate::jit::CodegenError;

use super::{AnalogInstance, CompiledModule, DigitalInstance, PiperineDevice};

/// Compiles an [`IrProgram`] into solver circuits. Kernels are cached per
/// module name, so instantiating a module many times compiles it once.
pub struct CircuitCompiler<'p> {
    program: &'p IrProgram,
    kernels: HashMap<String, Arc<CompiledModule>>,
}

impl<'p> CircuitCompiler<'p> {
    pub fn new(program: &'p IrProgram) -> Self {
        Self { program, kernels: HashMap::new() }
    }

    /// The compiled kernels for `module`, compiling on first use.
    pub fn compiled(&mut self, name: &str) -> Result<Arc<CompiledModule>, CodegenError> {
        if let Some(compiled) = self.kernels.get(name) {
            return Ok(compiled.clone());
        }
        let module = self
            .program
            .module(name)
            .ok_or_else(|| CodegenError::ModuleNotFound(name.to_string()))?;
        let compiled = Arc::new(CompiledModule::compile(module)?);
        self.kernels.insert(name.to_string(), compiled.clone());
        Ok(compiled)
    }

    /// Build the circuit rooted at module `top`. The top may have both
    /// child instances and its own behavior bodies (SPEC §7.3, B.1, B.10):
    /// the parent's `analog`/`digital` blocks contribute to the children's
    /// port nodes (KCL accumulation — parasitic load, coupling, trim).
    pub fn build_circuit(&mut self, top: &str) -> Result<CircuitInstance, CodegenError> {
        let top_module = self
            .program
            .module(top)
            .ok_or_else(|| CodegenError::ModuleNotFound(top.to_string()))?;

        let top_params = Self::param_values(top_module, &[])?;
        let mut builder = InstanceBuilder::new(self, top_module, top_params);
        for (index, instance) in top_module.instances.iter().enumerate() {
            builder.add_instance(index, instance)?;
        }
        // SPEC §7.3, B.1, B.10: if the top has its own behavior bodies AND
        // child instances, compile the top's behavior into a device that
        // stamps contributions (parasitic loads, coupling) at the child
        // instance nodes. A leaf top (behavior but no instances) produces
        // an empty circuit.
        if !top_module.instances.is_empty()
            && (top_module.analog.is_some() || top_module.digital.is_some())
        {
            builder.add_top_behavior_device()?;
        }
        Ok(builder.finish(top))
    }

    /// Evaluate a module's parameter values: defaults in id order (later
    /// defaults may reference earlier parameters), then `overrides`.
    fn param_values(
        module: &IrModule,
        overrides: &[(ParamId, f64)],
    ) -> Result<Vec<f64>, CodegenError> {
        let mut values: Vec<Option<f64>> = vec![None; module.symbols.num_params()];
        for (id, info) in module.symbols.params() {
            if let Some((_, v)) = overrides.iter().find(|(o, _)| *o == id) {
                values[id.0 as usize] = Some(*v);
                continue;
            }
            let default = info.default.as_ref().ok_or_else(|| {
                CodegenError::ConstEval(format!(
                    "parameter `{}` of `{}` has no default and no override",
                    info.name, module.name
                ))
            })?;
            let value = default
                .eval_const(&|p| values.get(p.0 as usize).copied().flatten())
                .map_err(CodegenError::ConstEval)?;
            values[id.0 as usize] = Some(value);
        }
        Ok(values.into_iter().map(|v| v.expect("all params filled")).collect())
    }
}

/// Accumulates devices, the analog netlist, and the digital net map while
/// walking the top module's instances.
struct InstanceBuilder<'c, 'p> {
    compiler: &'c mut CircuitCompiler<'p>,
    top: &'c IrModule,
    top_params: Vec<f64>,
    netlist: Netlist,
    devices: Vec<Box<dyn Device>>,
    digital_nets: HashMap<NodeId, DigitalNet>,
    /// Fresh ids for module-internal analog nodes (top node ids come first).
    next_anon: usize,
}

impl<'c, 'p> InstanceBuilder<'c, 'p> {
    fn new(compiler: &'c mut CircuitCompiler<'p>, top: &'c IrModule, top_params: Vec<f64>) -> Self {
        let next_anon = top.symbols.nodes().count();
        Self {
            compiler,
            top,
            top_params,
            netlist: Netlist::new(),
            devices: Vec::new(),
            digital_nets: HashMap::new(),
            next_anon,
        }
    }

    fn add_instance(&mut self, device_id: usize, instance: &IrInstance) -> Result<(), CodegenError> {
        let child = self
            .compiler
            .program
            .module(&instance.module)
            .ok_or_else(|| CodegenError::ModuleNotFound(instance.module.clone()))?;
        if !child.instances.is_empty() {
            return Err(CodegenError::unsupported(format!(
                "nested hierarchy: `{}` instantiates further modules — flatten during elaboration",
                child.name
            )));
        }
        if instance.connections.len() != child.ports.len() {
            return Err(CodegenError::Invalid(format!(
                "instance `{}` connects {} nets, module `{}` has {} ports",
                instance.label,
                instance.connections.len(),
                child.name,
                child.ports.len()
            )));
        }
        let compiled = self.compiler.compiled(&instance.module).map_err(|e| {
            CodegenError::Invalid(format!(
                "instance `{}` (module `{}`): {e}",
                instance.label, instance.module
            ))
        })?;

        // Parameters: instance overrides evaluated in the parent scope.
        let overrides = instance
            .params
            .iter()
            .map(|(id, expr)| {
                let value = expr
                    .eval_const(&|p| self.top_params.get(p.0 as usize).copied())
                    .map_err(CodegenError::ConstEval)?;
                Ok((*id, value))
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let params = CircuitCompiler::param_values(child, &overrides)?;
        let param_given_mask = overrides
            .iter()
            .fold(0u64, |mask, (id, _)| mask | (1 << id.0.min(63)));

        let analog = compiled
            .analog()
            .map(|kernel| {
                // Terminal identifiers: connected parent nodes for ports,
                // fresh anonymous nodes for module-internal terminals.
                let terminals: Vec<NodeIdentifier> = kernel
                    .terminals()
                    .iter()
                    .enumerate()
                    .map(|(i, _)| match instance.connections.get(i) {
                        Some(parent) => self.node_identifier(*parent),
                        None => {
                            let id = NodeIdentifier::Anonymous(self.next_anon);
                            self.next_anon += 1;
                            id
                        }
                    })
                    .collect();
                AnalogInstance::new(
                    &instance.label,
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
                let map_nets = |nodes: &[NodeId],
                                child: &IrModule,
                                instance: &IrInstance,
                                nets: &mut HashMap<NodeId, DigitalNet>|
                 -> Result<Vec<DigitalNet>, CodegenError> {
                    nodes
                        .iter()
                        .map(|node| {
                            let port_index = child
                                .ports
                                .iter()
                                .position(|p| p.node == *node)
                                .ok_or_else(|| {
                                    CodegenError::unsupported(format!(
                                        "digital net `{}` of `{}` is not a port",
                                        child.symbols.node(*node).name,
                                        child.name
                                    ))
                                })?;
                            let parent = instance.connections[port_index];
                            let next = nets.len();
                            Ok(*nets.entry(parent).or_insert(DigitalNet(next)))
                        })
                        .collect()
                };
                let in_nets = map_nets(kernel.inputs(), child, instance, &mut self.digital_nets)?;
                let out_nets = map_nets(kernel.outputs(), child, instance, &mut self.digital_nets)?;
                DigitalInstance::new(kernel.clone(), device_id, in_nets, out_nets, params.clone())
            })
            .transpose()?;

        let mut device = PiperineDevice::new(
            instance.label.clone(),
            analog,
            digital,
        );

        // For digital-only devices with analog input ports (e.g. a
        // Comparator: `input vp : Electrical, input vn : Electrical,
        // output out : Bit`): wire the analog port terminals into the
        // netlist so the A2D bridge can read their voltages.
        if device.analog.is_none() {
            if let Some(digital) = device.digital() {
                if digital.kernel().layout().num_analog() > 0 {
                    let mut refs = Vec::new();
                    let mut node_ids = Vec::new();
                    for (port_idx, port) in child.ports.iter().enumerate() {
                        let parent = instance.connections.get(port_idx)
                            .copied()
                            .unwrap_or(NodeId::GROUND);
                        let node_id = self.node_identifier(parent);
                        let reference = self.netlist.connect_node(node_id);
                        refs.push(reference.idx().map(|_| reference));
                        node_ids.push(port.node);
                    }
                    device.set_analog_terminals(refs, node_ids);
                }
            }
        }

        self.devices.push(Box::new(device));
        Ok(())
    }

    /// Compile the top module's own behavior bodies into a device (SPEC
    /// §7.3, B.1, B.10). The top's analog/digital blocks contribute to
    /// the child instance nodes — parasitic loads, coupling, trim. The
    /// top's NodeIds map directly to netlist nodes.
    fn add_top_behavior_device(&mut self) -> Result<(), CodegenError> {
        let compiled = self.compiler.compiled(&self.top.name)?;
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
                    &format!("{}__top", self.top.name),
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
            format!("{}__top", self.top.name),
            analog,
            digital,
        );

        // A2D bridge for digital-only top behavior with analog port reads.
        if device.analog.is_none() {
            if let Some(digital) = device.digital() {
                if digital.kernel().layout().num_analog() > 0 {
                    let mut refs = Vec::new();
                    let mut node_ids = Vec::new();
                    for port in self.top.ports.iter() {
                        let node_id = self.node_identifier(port.node);
                        let reference = self.netlist.connect_node(node_id);
                        refs.push(reference.idx().map(|_| reference));
                        node_ids.push(port.node);
                    }
                    device.set_analog_terminals(refs, node_ids);
                }
            }
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

    fn finish(self, title: &str) -> CircuitInstance {
        let mut circuit = CircuitInstance::from_devices_and_netlist(title, self.devices, self.netlist);
        circuit.digital_state = DigitalState::new(self.digital_nets.len());
        let _ = (self.top, self.top_params);
        circuit
    }
}
