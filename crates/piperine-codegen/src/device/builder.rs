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
    /// One shared `Arc<PrimitiveDecl>` per UDP name (LRMC-C7-13, T40) —
    /// mirrors `module_arcs` so multiple instances of the same UDP share
    /// one parsed table instead of each cloning it.
    udp_arcs: HashMap<String, Arc<piperine_lang::ast::PrimitiveDecl>>,
    /// Canonical-node map (NGP-18 codegen integration, Fix B). A structural
    /// `connection tap[0] = p;` aliases two `NodeId`s — every device that
    /// connects to `tap[0]` actually connects to `p`. The alias map records
    /// `tap0 → p` (and any further chain) so [`Self::node_identifier`]
    /// returns the canonical [`NodeIdentifier`] for both, and the netlist
    /// ends up with one MNA unknown rather than two unconnected ones. The
    /// map is built once from `top.connections()` in [`Self::new`] (before
    /// any `add_instance` runs); `node_identifier` and the
    /// [`CircuitBuildInfo::nets`] readback both consult it.
    node_aliases: HashMap<NodeId, NodeId>,
}

impl<'c, 'p> InstanceBuilder<'c, 'p> {
    pub(super) fn new(
        compiler: &'c mut CircuitCompiler<'p>,
        top: &'p Module,
        top_body: &'p LoweredBody,
        top_params: Vec<f64>,
    ) -> Self {
        let next_anon = top_body.symbols.nodes().count();
        // NGP-18 codegen integration (Fix B): structural `connection
        // tap[0] = p;` aliases two NodeIds. Built before the `nets`
        // readback and any `add_instance` call so every name → NodeId →
        // NodeIdentifier lookup consults the same canonical map.
        let node_aliases = build_node_aliases(top_body, top);
        let nets = top_body
            .symbols
            .nodes()
            .map(|(id, info)| {
                let canonical = canonicalize(id, &node_aliases);
                let node = if canonical.is_ground() {
                    NodeIdentifier::Gnd
                } else {
                    NodeIdentifier::Anonymous(canonical.0 as usize)
                };
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
            udp_arcs: HashMap::new(),
            build_info: CircuitBuildInfo {
                nets,
                digital_nets: HashMap::new(),
                instances: Vec::new(),
                fused_networks: 0,
            },
            node_aliases,
        }
    }

    /// Resolve one instance's port bindings (parent-scope net names) into
    /// this module's `NodeId`s — structural work done here, once per
    /// instantiation, directly from the POM.
    pub(super) fn resolve_connections(&self, instance: &Instance) -> Result<Vec<NodeId>, CodegenError> {
        Self::resolve_net_refs(self.top_body, instance.ports(), instance.name())
    }

    /// Resolves a bare `&[NetRef]` list against `body`'s node symbols —
    /// the same mechanism [`Self::resolve_connections`] uses for an
    /// ordinary instance's ports, generalized (T40/LRMC-C7-13) for a gate
    /// primitive's terminals or a UDP instance's ports, neither of which
    /// carries a `piperine_lang::pom::Instance` to read `.ports()` off of
    /// in the gate case (`GateInstance::terminals`).
    pub(super) fn resolve_net_refs(
        body: &LoweredBody,
        refs: &[piperine_lang::pom::NetRef],
        owner_name: &str,
    ) -> Result<Vec<NodeId>, CodegenError> {
        refs.iter()
            .map(|r| {
                let name = r.to_string();
                CircuitCompiler::resolve_node(body, &name)
                    .or_else(|| CircuitCompiler::resolve_node(body, r.net()))
                    .ok_or_else(|| {
                        CodegenError::Invalid(format!("`{owner_name}`: unresolved net `{name}`"))
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
        // LRMC-C7-13 (T39/T40): a UDP instance is syntactically identical
        // to a module instantiation — resolved against `Design::udps`
        // (T39) before falling through to the ordinary module lookup,
        // the same precedence a paramset instance gets.
        if self.compiler.udp(instance.module_name()).is_some() {
            return self.add_udp_instance(instance);
        }
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
        // §9.19 (LRMC-C7-09, T50): one bit per port, in declared order —
        // set unless that position's connection is the `NetRef::
        // unconnected()` sentinel (T50's own elaboration-time fix for
        // `.p()`/blank positional slots, previously a hard elaboration
        // error).
        let port_connected_mask: u64 = instance
            .ports()
            .iter()
            .enumerate()
            .fold(0u64, |mask, (i, p)| if p.is_unconnected() { mask } else { mask | (1 << i.min(63)) });

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
        let meta = self.compiler.introspection_meta(instance.module_name())?;
        if let (Some(kernel), Some(terminals)) = (compiled.analog(), &analog_terminals) {
            let opvar_display_names = kernel
                .opvar_names()
                .iter()
                .map(|n| meta.vars.get(n).and_then(|m| m.name.clone()).unwrap_or_else(|| n.clone()))
                .collect();
            self.build_info.instances.push(BuiltInstanceInfo {
                label: instance.name().to_string(),
                module: instance.module_name().to_string(),
                kernel: kernel.clone(),
                params: params.clone(),
                terminals: terminals.clone(),
                num_forces: kernel.num_forces(),
                opvar_display_names,
            });
        }
        let analog = match (compiled.analog(), analog_terminals) {
            (Some(kernel), Some(terminals)) => Some(AnalogInstance::new(
                instance.name(),
                kernel.clone(),
                &terminals,
                params.clone(),
                param_given_mask,
                port_connected_mask,
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
            meta,
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

    /// Maps a resolved `NodeId` to its digital-net slot, allocating a
    /// fresh one on first sight — the same pooling `add_instance`'s own
    /// `map_nets` closure does for an ordinary digital kernel's ports,
    /// generalized so a gate/UDP's terminals share the same pool (a gate
    /// wired to the same net as a module's digital port must resolve to
    /// the identical `DigitalNet`).
    fn map_digital_net(&mut self, node: NodeId) -> DigitalNet {
        let next = self.digital_nets.len();
        *self.digital_nets.entry(node).or_insert(DigitalNet(next))
    }

    /// One UDP instance (Annex A.5, T40) — `instance.module_name()` names
    /// a registered `primitive`, not a module; dispatched to
    /// [`super::prim::UdpElement`] instead of the ordinary
    /// kernel-compile path `add_instance` continues into below.
    fn add_udp_instance(&mut self, instance: &Instance) -> Result<(), CodegenError> {
        let udp = self.compiler.udp(instance.module_name()).ok_or_else(|| {
            CodegenError::Invalid(format!("UDP `{}` vanished between lookup and dispatch", instance.module_name()))
        })?;
        let arc = match self.udp_arcs.get(instance.module_name()) {
            Some(arc) => arc.clone(),
            None => {
                let arc = Arc::new(udp.clone());
                self.udp_arcs.insert(instance.module_name().to_string(), arc.clone());
                arc
            }
        };
        let nodes = self.resolve_connections(instance)?;
        if nodes.is_empty() {
            return Err(CodegenError::Invalid(format!(
                "UDP instance `{}`: no port connections (a UDP's first port is always its output)",
                instance.name()
            )));
        }
        let mapped: Vec<DigitalNet> = nodes.iter().map(|&n| self.map_digital_net(n)).collect();
        let element = super::prim::UdpElement::new(arc, instance.name().to_string(), mapped[0], mapped[1..].to_vec());
        self.devices.push(Box::new(element));
        Ok(())
    }

    /// One gate-primitive instance (Annex A.4.2, T40) — `Module::gates`
    /// entries never appear in `instances()`, so this is called from a
    /// separate walk (`circuit.rs::build_circuit_mapped`).
    pub(super) fn add_gate_instance(
        &mut self,
        device_id: usize,
        gate: &piperine_lang::pom::GateInstance,
    ) -> Result<(), CodegenError> {
        let owner = gate.name.as_deref().unwrap_or("<unnamed gate>");
        let nodes = Self::resolve_net_refs(self.top_body, &gate.terminals, owner)?;
        if nodes.is_empty() {
            return Err(CodegenError::Invalid(format!("gate `{owner}`: no terminals")));
        }
        let mapped: Vec<DigitalNet> = nodes.iter().map(|&n| self.map_digital_net(n)).collect();
        let element = super::prim::GateElement::new(gate, device_id, mapped[0], mapped[1..].to_vec())?;
        self.devices.push(Box::new(element));
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
                    u64::MAX, // the top has no "instance" of its own — every port trivially connected
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
            self.compiler.introspection_meta(self.top.name())?,
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
    /// as ordinary nodes. A node aliased to another by a structural
    /// `connection lhs = rhs;` (NGP-18, Fix B) resolves to its canonical
    /// counterpart's identifier — the netlist then sees one MNA unknown,
    /// shared by every device that connects to either name.
    pub(super) fn node_identifier(&self, node: NodeId) -> NodeIdentifier {
        let canonical = canonicalize(node, &self.node_aliases);
        if canonical.is_ground() {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Anonymous(canonical.0 as usize)
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

// ─── Structural-connection unification (NGP-18 codegen integration, Fix B) ───

/// Build a `child → parent` alias map from a module's structural
/// `connection lhs = rhs;` entries. A connection states that two nets are
/// the same node — e.g. `tap[0] = p;` aliases the `tap[0]` NodeId to the
/// `p` NodeId. Without this map, the circuit builder sees two distinct
/// MNA unknowns; devices that connect to `tap[0]` end up on a different
/// unknown than devices that connect to `p`, and a DC solve produces
/// NaN/Inf for the floating side.
///
/// MD-25 binding: unification lives in codegen's resolved/circuit layer
/// (here) — the POM `Design::modules` is never mutated. Only the
/// `NodeId → NodeIdentifier` mapping this builder produces is affected.
///
/// The map is a forest: each entry points to a parent, and
/// [`canonicalize`] walks to the root. Union by NodeId ordering picks
/// the smaller-id root as canonical (deterministic, no graph cycles).
fn build_node_aliases(
    top_body: &LoweredBody,
    top: &Module,
) -> HashMap<NodeId, NodeId> {
    let mut aliases: HashMap<NodeId, NodeId> = HashMap::new();
    for c in top.connections() {
        let Some(lhs) = resolve_alias_target(top_body, c.lhs()) else { continue };
        let Some(rhs) = resolve_alias_target(top_body, c.rhs()) else { continue };
        let lhs_root = union_root(lhs, &aliases);
        let rhs_root = union_root(rhs, &aliases);
        if lhs_root == rhs_root {
            continue;
        }
        // Pick the smaller id as the canonical root — deterministic and
        // stable across builds (so a host's `nets` map is reproducible).
        let (parent, child) = if lhs_root.0 < rhs_root.0 {
            (lhs_root, rhs_root)
        } else {
            (rhs_root, lhs_root)
        };
        aliases.insert(child, parent);
    }
    aliases
}

/// Resolve one side of a structural connection to its target `NodeId`.
/// Ground aliases to [`NodeId::GROUND`]; any other name looks up through
/// the same `resolve_node` path instance connections use, so an indexed
/// ref (`tap[0]`) finds the per-element node Fix A registered.
fn resolve_alias_target(top_body: &LoweredBody, nr: &piperine_lang::pom::NetRef) -> Option<NodeId> {
    if piperine_lang::pom::is_ground(nr.net()) {
        return Some(NodeId::GROUND);
    }
    let full = nr.to_string();
    CircuitCompiler::resolve_node(top_body, &full)
        .or_else(|| CircuitCompiler::resolve_node(top_body, nr.net()))
}

/// Walk the alias forest to the root. A node not in the map is its own
/// root.
fn union_root(mut node: NodeId, aliases: &HashMap<NodeId, NodeId>) -> NodeId {
    while let Some(&parent) = aliases.get(&node) {
        if parent == node {
            break;
        }
        node = parent;
    }
    node
}

/// Canonical form of `node` under the alias map — the root of its
/// union-find tree. Used by [`InstanceBuilder::node_identifier`] and the
/// `nets` readback so every alias of a node resolves to one
/// [`NodeIdentifier`] (one MNA unknown). The alias map is `&` (built
/// once in [`InstanceBuilder::new`], never mutated thereafter), so this
/// is a plain read.
fn canonicalize(node: NodeId, aliases: &HashMap<NodeId, NodeId>) -> NodeId {
    union_root(node, aliases)
}

#[cfg(test)]
mod tests {
    //! Structural-connection unification (NGP-18 codegen integration,
    //! Fix B). A structural connection aliases two NodeIds. The circuit
    //! builder must produce one MNA unknown per alias group — otherwise
    //! the segment at the first tap floats on its own unknown and the DC
    //! solve diverges. These tests cover the alias map's construction
    //! (`build_node_aliases`), its canonical lookup (`canonicalize`), and
    //! the end-to-end build path through
    //! `CircuitCompiler::build_circuit_mapped`.
    //!
    //! The connections come from the POM staging API
    //! (`Design::stage_connection`, SPEC Part VI §8.2) rather than from
    //! source text: Verilog-AMS has no net-alias statement and no array
    //! analog nets, so `tap : Electrical[3]` + `tap[0] = p;` (PHDL, the
    //! shape `urc[N]` used) has no spelling in the current frontend —
    //! staging is the live producer of `Module::connections` that
    //! survives it. Scalar taps stand in for the array's elements; the
    //! alias-group assertions are unchanged.

    use super::{build_node_aliases, canonicalize};
    use crate::resolve::pom::lower_bodies;
    use crate::CircuitCompiler;
    use piperine_lang::pom::staging::ConnectionSpec;
    use piperine_lang::pom::Design;
    use piperine_lang::{parse_and_elaborate, SourceMap};
    use piperine_solver::abi::NodeIdentifier;

    /// Two `res` segments separated by taps; `tap0` aliases the source
    /// node `vin`, `tap2` aliases the load node `vout`.
    const CHAIN_SRC: &str = "
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0;
            analog I(p, n) <+ V(p, n) / r;
        endmodule
        module vsrc_external (inout electrical p, inout electrical n);
            analog V(p, n) <+ 1.0;
        endmodule
        module Top ();
            electrical gnd, vin, vout;
            electrical tap0, tap1, tap2;
            vsrc_external v (vin, gnd);
            res r0 (.p(tap0), .n(tap1));
            res r1 (.p(tap1), .n(tap2));
        endmodule
    ";

    /// Elaborate `CHAIN_SRC` and stage the two structural connections
    /// (`tap0 = vin`, `tap2 = vout`) onto `Top`.
    fn chain_design() -> Design {
        let design = parse_and_elaborate(CHAIN_SRC, &SourceMap::dummy()).expect("elaborates");
        design
            .stage_connection(
                "Top",
                ConnectionSpec { lhs: "tap0".into(), rhs: "vin".into() },
            )
            .expect("stage tap0 = vin");
        design
            .stage_connection(
                "Top",
                ConnectionSpec { lhs: "tap2".into(), rhs: "vout".into() },
            )
            .expect("stage tap2 = vout");
        design.with_overrides_applied("Top").expect("overrides apply")
    }

    /// `build_node_aliases` collapses the alias group `tap0 = vin` so the
    /// tap element and the source node share a canonical NodeId.
    #[test]
    fn structural_connection_unifies_tap0_with_p() {
        let design = chain_design();
        let bodies = lower_bodies(&design).expect("lowers");
        let flat = design.flat_module("Top").expect("Top flat form");
        let top_body = &bodies["Top"];
        let aliases = build_node_aliases(top_body, flat);

        let vin_id = crate::device::circuit::CircuitCompiler::resolve_node(top_body, "vin")
            .expect("vin resolves");
        let tap0_id = crate::device::circuit::CircuitCompiler::resolve_node(top_body, "tap0")
            .expect("tap0 resolves");
        let vout_id = crate::device::circuit::CircuitCompiler::resolve_node(top_body, "vout")
            .expect("vout resolves");
        let tap2_id = crate::device::circuit::CircuitCompiler::resolve_node(top_body, "tap2")
            .expect("tap2 resolves");

        assert_eq!(
            canonicalize(tap0_id, &aliases),
            canonicalize(vin_id, &aliases),
            "tap0 aliases vin — they share a canonical NodeId"
        );
        assert_eq!(
            canonicalize(tap2_id, &aliases),
            canonicalize(vout_id, &aliases),
            "tap2 aliases vout — they share a canonical NodeId"
        );
        assert_ne!(
            canonicalize(tap0_id, &aliases),
            canonicalize(tap2_id, &aliases),
            "the two alias groups stay distinct"
        );
        // The middle tap is not in any structural connection —
        // it canonicalizes to itself.
        let tap1_id = crate::device::circuit::CircuitCompiler::resolve_node(top_body, "tap1")
            .expect("tap1 resolves");
        assert_eq!(
            canonicalize(tap1_id, &aliases),
            tap1_id,
            "tap1 is unaliased — its own canonical node"
        );
    }

    /// End-to-end build: the two structural connections reduce the
    /// per-segment topology to one DC path from `vin` to `vout`, and the
    /// resulting `nets` map shows `tap0` and `vin` sharing one
    /// NodeIdentifier (likewise `tap2` and `vout`).
    #[test]
    fn structural_connection_produces_one_mna_unknown_per_alias_group() {
        let design = chain_design();
        let bodies = lower_bodies(&design).expect("lowers");
        let mut compiler = CircuitCompiler::new(&design, &bodies);
        let (_circuit, info) = compiler.build_circuit_mapped("Top").expect("builds");

        let tap0_node = info.nets.get("tap0").cloned();
        let vin_node = info.nets.get("vin").cloned();
        let tap2_node = info.nets.get("tap2").cloned();
        let vout_node = info.nets.get("vout").cloned();
        let tap1_node = info.nets.get("tap1").cloned();

        assert!(tap0_node.is_some(), "tap0 in nets");
        assert!(tap1_node.is_some(), "tap1 in nets");
        assert!(tap2_node.is_some(), "tap2 in nets");
        assert_eq!(
            tap0_node, vin_node,
            "tap0 and vin share a NodeIdentifier (one MNA unknown)"
        );
        assert_eq!(
            tap2_node, vout_node,
            "tap2 and vout share a NodeIdentifier (one MNA unknown)"
        );
        // The three alias groups — {vin, tap0}, {tap1}, {vout, tap2} —
        // produce three distinct NodeIdentifiers.
        let groups = [
            tap0_node.as_ref().unwrap(),
            tap1_node.as_ref().unwrap(),
            tap2_node.as_ref().unwrap(),
        ];
        let pairs = [
            (groups[0], groups[1]),
            (groups[0], groups[2]),
            (groups[1], groups[2]),
        ];
        for (a, b) in &pairs {
            assert_ne!(a, b, "alias groups produce distinct NodeIdentifiers");
        }
        // Sanity: none of the alias groups is ground.
        assert_ne!(tap0_node.as_ref().unwrap(), &NodeIdentifier::Gnd);
        assert_ne!(tap1_node.as_ref().unwrap(), &NodeIdentifier::Gnd);
        assert_ne!(tap2_node.as_ref().unwrap(), &NodeIdentifier::Gnd);
    }
}
