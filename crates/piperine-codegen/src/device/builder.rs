//! Instance-level circuit assembly: walks a top module's instances and
//! accumulates devices, the analog netlist, and the digital net map into a
//! ready-to-finish [`CircuitInstance`]. [`CircuitCompiler`] (`circuit.rs`)
//! owns the public build API and kernel cache; this module owns the
//! per-build walk.

use std::collections::HashMap;
use std::sync::Arc;

use piperine_lang::pom::{Instance, Module};

use piperine_solver::abi::Netlist;
use piperine_solver::abi::NodeIdentifier;
use piperine_solver::abi::CircuitInstance;
use piperine_solver::abi::Element;
use piperine_solver::abi::DigitalNet;
use piperine_solver::abi::DigitalState;

use crate::resolve::{NodeId, ParamId};
use crate::resolve::pom::LoweredBody;
use crate::error::CodegenError;

use super::{AnalogInstance, DigitalInstance, PiperineDevice};
use super::circuit::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler};
use super::fusion::FusionCandidate;

/// Accumulates devices, the analog netlist, and the digital net map while
/// walking the top module's instances.
pub(super) struct InstanceBuilder<'c, 'p> {
    pub(super) compiler: &'c mut CircuitCompiler<'p>,
    top: &'p Module,
    top_body: &'p LoweredBody,
    top_params: Vec<f64>,
    pub(super) netlist: Netlist,
    pub(super) devices: Vec<Box<dyn Element>>,
    pub(super) digital_nets: HashMap<NodeId, DigitalNet>,
    /// Fresh ids for module-internal analog nodes (top node ids come first).
    next_anon: usize,
    pub(super) build_info: CircuitBuildInfo,
    /// Fusion-eligible digital devices (pure combinational) recorded during
    /// the walk, in device order.
    pub(super) fusion_candidates: Vec<FusionCandidate>,
    /// One `Arc<LoweredBody>` per digital module name, cloned lazily for
    /// fused-network members.
    module_arcs: HashMap<String, Arc<LoweredBody>>,
}

impl<'c, 'p> InstanceBuilder<'c, 'p> {
    pub(super) fn new(
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
            fusion_candidates: Vec::new(),
            module_arcs: HashMap::new(),
            build_info: CircuitBuildInfo {
                nets,
                digital_nets: HashMap::new(),
                instances: Vec::new(),
                fused_networks: 0,
            },
        }
    }

    /// Resolve one instance's port bindings (parent-scope net names) into
    /// this module's `NodeId`s — the structural work `lower_bodies` used to
    /// do once for every module's `IrInstance.connections`; now done here,
    /// once per instantiation, directly from the POM.
    pub(super) fn resolve_connections(&self, instance: &Instance) -> Result<Vec<NodeId>, CodegenError> {
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
                let expr = crate::resolve::pom::structure::value_to_pom_expr(pval);
                let top = &self.top_params;
                let top_body = &self.top_body;
                let resolve = |name: &str| -> Option<f64> {
                    top_body.symbols.params()
                        .find(|(_, p)| p.name == name)
                        .and_then(|(id, _)| top.get(id.0 as usize).copied())
                };
                let value = crate::resolve::pom_eval_const(&expr, &resolve)
                    .map_err(CodegenError::ConstEval)?;
                Ok((id, value))
            })
            .collect()
    }

    pub(super) fn add_instance(&mut self, device_id: usize, instance: &Instance) -> Result<(), CodegenError> {
        let child = self.compiler.module(instance.module_name())?;
        // A `@device`-annotated module's behavior is provided by a plugin,
        // not by PHDL analog/digital blocks (SPEC Part VI §7).
        if let Some(dev_attr) = child
            .attributes()
            .iter()
            .chain(instance.attributes().iter())
            .find(|a| a.schema() == "device")
        {
            let dev_attr = dev_attr.clone();
            return self.add_plugin_instance(instance, child, &dev_attr);
        }
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

        if let Some(c) = FusionCandidate::of(
            self.devices.len(),
            instance.module_name(),
            &device.analog,
            &device.digital,
            params.clone(),
        ) {
            self.fusion_candidates.push(c);
        }
        self.devices.push(Box::new(device));
        Ok(())
    }

    /// Compile the top module's own behavior bodies into a device (SPEC
    /// §7.3, B.1, B.10). The top's analog/digital blocks contribute to
    /// the child instance nodes — parasitic loads, coupling, trim. The
    /// top's NodeIds map directly to netlist nodes.
    pub(super) fn add_top_behavior_device(&mut self) -> Result<(), CodegenError> {
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

        if let Some(c) = FusionCandidate::of(
            self.devices.len(),
            self.top.name(),
            &device.analog,
            &device.digital,
            params.clone(),
        ) {
            self.fusion_candidates.push(c);
        }
        self.devices.push(Box::new(device));
        Ok(())
    }

    /// Map a top-module node to a solver identifier. Digital-domain nodes
    /// also pass through here for mixed instances; the analog side sees them
    /// as ordinary nodes.
    pub(super) fn node_identifier(&self, node: NodeId) -> NodeIdentifier {
        if node.is_ground() {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Anonymous(node.0 as usize)
        }
    }

    /// One shared `Arc<LoweredBody>` per module name (fused-network members
    /// carry it for recompilation into the cone function).
    pub(super) fn module_arc(&mut self, name: &str) -> Result<Arc<LoweredBody>, CodegenError> {
        if let Some(arc) = self.module_arcs.get(name) {
            return Ok(arc.clone());
        }
        let arc = Arc::new(self.compiler.body(name)?.clone());
        self.module_arcs.insert(name.to_string(), arc.clone());
        Ok(arc)
    }

    pub(super) fn finish(mut self, title: &str) -> Result<(CircuitInstance, CircuitBuildInfo), CodegenError> {
        self.fuse_comb_cones()?;
        let mut circuit = CircuitInstance::from_devices_and_netlist(title, self.devices, self.netlist);
        circuit.digital_state = DigitalState::new(self.digital_nets.len());
        // Name → digital-net index, for host-side readback.
        for (id, info) in self.top_body.symbols.nodes() {
            if let Some(dn) = self.digital_nets.get(&id) {
                self.build_info.digital_nets.insert(info.name.clone(), dn.0);
            }
        }
        let _ = self.top_params;
        Ok((circuit, self.build_info))
    }
}
