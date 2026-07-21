//! The plugin-device seam (Plugin plan D4): `CircuitCompiler` delegates
//! `@device`-annotated instances to a [`DeviceProvider`] instead of compiling
//! them from PHDL. The provider (a plugin host) returns a solver `Element`;
//! the solver never learns it came from a plugin (SPEC Part VI §7).
//!
//! Dependency direction: this trait lives in codegen so codegen never
//! depends on the plugin crate — the plugin host implements it.

use piperine_lang::parse::ast::Direction;
use piperine_lang::pom::module::Attribute;
use piperine_lang::pom::{Instance, Module};
use piperine_lang::Value;
use piperine_solver::abi::AnalogReference;
use piperine_solver::abi::DigitalNet;
use piperine_solver::abi::Element;

use crate::error::CodegenError;

use super::builder::InstanceBuilder;

/// How one port of a `@device` module binds into the built circuit.
#[derive(Debug, Clone)]
pub enum PortBinding {
    /// An analog terminal: the netlist reference the device stamps/reads.
    Analog(AnalogReference),
    /// A digital net in the scheduler's namespace.
    Digital(DigitalNet),
}

/// One resolved port of a plugin device.
#[derive(Debug, Clone)]
pub struct PluginPort {
    /// The plugin-facing logical name (`@port(name = "A0")`), or the PHDL
    /// port name when no `@port` attribute names it.
    pub logical: String,
    /// The PHDL port name.
    pub phdl_name: String,
    pub direction: Direction,
    pub binding: PortBinding,
}

/// Everything a device factory needs to construct one plugin device
/// (SPEC Part VI §7.3).
pub struct PluginDeviceSpec {
    /// The plugin named by `@device(plugin = …)`.
    pub plugin: String,
    /// The type id named by `@device(type = …)`.
    pub type_id: String,
    /// The instance label in the parent module.
    pub instance_label: String,
    /// The validated `@device`/`@port` (and any other) attributes on the
    /// module and the instance.
    pub attributes: Vec<Attribute>,
    /// Resolved port bindings, in PHDL port order.
    pub ports: Vec<PluginPort>,
    /// Instance parameter overrides (`{ .name = value }`), unevaluated.
    pub params: Vec<(String, Value)>,
}

/// The bridge between the plugin world and the circuit builder. Errors are
/// plain strings; the compiler wraps them as fail-loud `CodegenError`s.
pub trait DeviceProvider {
    fn build(&self, spec: PluginDeviceSpec) -> Result<Box<dyn Element>, String>;
}

impl<'c, 'p> InstanceBuilder<'c, 'p> {
    /// Build a `@device`-annotated instance through the wired
    /// [`DeviceProvider`]: resolve each port into an analog netlist
    /// reference or a digital scheduler net (per its `@port(kind = …)` or
    /// its discipline), hand the spec to the provider, and inject the
    /// returned `Element` as-is (SPEC Part VI §7).
    pub(super) fn add_plugin_instance(
        &mut self,
        instance: &Instance,
        child: &'p Module,
        dev_attr: &Attribute,
    ) -> Result<(), CodegenError> {
        let provider = self.compiler.provider.ok_or_else(|| {
            CodegenError::unsupported(format!(
                "instance `{}` is a plugin device (`@device`) but no plugin host is wired",
                instance.name()
            ))
        })?;
        let str_field = |attr: &Attribute, field: &str| -> Option<String> {
            match attr.field(field) {
                Some(Value::Str(s)) => Some(s.clone()),
                _ => None,
            }
        };
        let plugin = str_field(dev_attr, "plugin").ok_or_else(|| {
            CodegenError::Invalid(format!("instance `{}`: @device needs a `plugin` string", instance.name()))
        })?;
        let type_id = str_field(dev_attr, "type").ok_or_else(|| {
            CodegenError::Invalid(format!("instance `{}`: @device needs a `type` string", instance.name()))
        })?;
        if instance.ports().len() != child.ports().len() {
            return Err(CodegenError::Invalid(format!(
                "instance `{}` connects {} nets, module `{}` has {} ports",
                instance.name(), instance.ports().len(), child.name(), child.ports().len()
            )));
        }
        let connections = self.resolve_connections(instance)?;

        let mut ports = Vec::with_capacity(child.ports().len());
        for (i, port) in child.ports().iter().enumerate() {
            let port_attr = port.attributes().iter().find(|a| a.schema() == "port");
            let logical = port_attr
                .and_then(|a| str_field(a, "name"))
                .unwrap_or_else(|| port.name().to_string());
            // `@port(kind = …)` wins; otherwise the port's discipline decides
            // (digital storage disciplines → scheduler net, else MNA node).
            let kind = port_attr
                .and_then(|a| str_field(a, "kind"))
                .unwrap_or_else(|| {
                    match port.ty.discipline_name() {
                        "Bit" | "Logic" | "DDiscrete" => "digital".to_string(),
                        _ => "analog".to_string(),
                    }
                });
            let parent = connections[i];
            let binding = match kind.as_str() {
                "digital" => {
                    let next = self.digital_nets.len();
                    PortBinding::Digital(
                        *self.digital_nets.entry(parent).or_insert(DigitalNet(next)),
                    )
                }
                "analog" => {
                    let node = self.node_identifier(parent);
                    PortBinding::Analog(self.netlist.connect_node(node))
                }
                other => {
                    return Err(CodegenError::Invalid(format!(
                        "instance `{}` port `{}`: unknown @port kind `{other}` (analog|digital)",
                        instance.name(), port.name()
                    )));
                }
            };
            ports.push(PluginPort {
                logical,
                phdl_name: port.name().to_string(),
                direction: port.direction.clone(),
                binding,
            });
        }

        let mut attributes: Vec<Attribute> = child.attributes().to_vec();
        attributes.extend(instance.attributes().iter().cloned());
        let spec = PluginDeviceSpec {
            plugin,
            type_id: type_id.clone(),
            instance_label: instance.name().to_string(),
            attributes,
            ports,
            params: instance.params().iter().map(|(n, v)| (n.clone(), v.clone())).collect(),
        };
        let device = provider.build(spec).map_err(|e| {
            CodegenError::Invalid(format!(
                "plugin device `{type_id}` (instance `{}`): {e}",
                instance.name()
            ))
        })?;
        self.devices.push(device);
        Ok(())
    }
}
