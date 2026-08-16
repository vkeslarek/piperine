//! `@device` gains a host binding mode (T17, DVT-20/21): [`HostDeviceRegistry`]
//! is a [`DeviceProvider`] that tries a wired plugin binding first and a
//! host registration second, failing loud naming both when neither is
//! found — the same `@device` declaration, unchanged, serves either
//! mode (`ideal.md` §8.7, `design.md` §C3). No new macro (MD-13 rule 5): a
//! Rust host device registers with [`HostDeviceRegistry::register`], an
//! explicit call, never `#[pip::device]`'s plugin-binary `inventory`
//! mechanism.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use piperine_codegen::device::{DeviceProvider, PluginDeviceSpec, PortBinding};
use piperine_solver::abi::Element;

/// Everything a host factory needs to construct one host device — the
/// host-binding twin of [`PluginDeviceSpec`], built from it before the
/// original is (possibly) consumed trying the plugin path first.
pub struct HostDeviceSpec {
    /// The `@device(type = …)` type id.
    pub type_id: String,
    /// The instance label in the parent module.
    pub instance_label: String,
    /// Resolved port bindings, in declaration port order.
    pub ports: Vec<PortBinding>,
    /// Instance parameter overrides, unevaluated.
    pub params: Vec<(String, piperine_lang::Value)>,
}

impl HostDeviceSpec {
    fn from_plugin_spec(spec: &PluginDeviceSpec) -> Self {
        Self {
            type_id: spec.type_id.clone(),
            instance_label: spec.instance_label.clone(),
            ports: spec.ports.iter().map(|p| p.binding.clone()).collect(),
            params: spec.params.clone(),
        }
    }
}

type HostFactory = dyn Fn(HostDeviceSpec) -> Result<Box<dyn Element>, String>;

/// The `@device` resolution seam: plugin binding first, host registration
/// second, loud naming both when neither has the requested type id.
#[derive(Default)]
pub struct HostDeviceRegistry {
    plugin: Option<Rc<dyn DeviceProvider>>,
    hosts: RefCell<HashMap<String, Rc<HostFactory>>>,
    /// Type ids registered via [`Self::register_sequencer`] (T25, D13): a
    /// session built against a registry with any entry here refuses AC
    /// loud rather than silently freezing the coroutine — see
    /// [`Self::has_sequencer`].
    sequencer_ids: RefCell<HashSet<String>>,
}

impl HostDeviceRegistry {
    /// A registry with no plugin binding wired — host registrations only.
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry that tries `plugin` first, falling back to any host
    /// registration — the shape a session wires when both binding modes
    /// coexist (`ideal.md` §8.7's promotion path: a device can move from
    /// this registry into `plugin` with no declaration change).
    pub fn with_plugin(plugin: Rc<dyn DeviceProvider>) -> Self {
        Self { plugin: Some(plugin), hosts: RefCell::new(HashMap::new()), sequencer_ids: RefCell::new(HashSet::new()) }
    }

    /// Register a host device factory for `@device(type = "type_id")`, an
    /// explicit call — no macro, no `inventory` collection (MD-13 rule 5).
    pub fn register(
        &self,
        type_id: impl Into<String>,
        factory: impl Fn(HostDeviceSpec) -> Result<Box<dyn Element>, String> + 'static,
    ) {
        self.hosts.borrow_mut().insert(type_id.into(), Rc::new(factory));
    }

    /// [`Self::register`], additionally marking `type_id` as
    /// sequencer-bearing (T25, D13) — a `Session` built with this registry
    /// as its [`Self::has_sequencer`]-checked provider refuses AC loud
    /// rather than silently freezing the coroutine.
    pub fn register_sequencer(
        &self,
        type_id: impl Into<String>,
        factory: impl Fn(HostDeviceSpec) -> Result<Box<dyn Element>, String> + 'static,
    ) {
        let type_id = type_id.into();
        self.sequencer_ids.borrow_mut().insert(type_id.clone());
        self.register(type_id, factory);
    }

    /// True once any type registered through [`Self::register_sequencer`]
    /// exists in this registry.
    pub fn has_sequencer(&self) -> bool {
        !self.sequencer_ids.borrow().is_empty()
    }
}

impl DeviceProvider for HostDeviceRegistry {
    fn build(&self, spec: PluginDeviceSpec) -> Result<Box<dyn Element>, String> {
        let type_id = spec.type_id.clone();
        // Built from `&spec` before any attempt consumes it, so the host
        // path is always available regardless of which binding is tried
        // first (`PluginDeviceSpec` does not derive `Clone`).
        let host_spec = HostDeviceSpec::from_plugin_spec(&spec);
        let host_factory = self.hosts.borrow().get(&type_id).cloned();

        if let Some(plugin) = &self.plugin {
            match plugin.build(spec) {
                Ok(element) => return Ok(element),
                Err(plugin_err) => {
                    return match host_factory {
                        Some(factory) => factory(host_spec),
                        None => Err(format!(
                            "@device `{type_id}`: no plugin binding ({plugin_err}) and no host \
                             registration — register a plugin device of this type, or call \
                             `HostDeviceRegistry::register(\"{type_id}\", ...)`"
                        )),
                    };
                }
            }
        }

        match host_factory {
            Some(factory) => factory(host_spec),
            None => Err(format!(
                "@device `{type_id}`: no plugin host wired and no host registration — call \
                 `HostDeviceRegistry::register(\"{type_id}\", ...)`, or wire a plugin host with \
                 `HostDeviceRegistry::with_plugin`"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::adapter::HostAdapter;
    use crate::device::host_element::HostElement;
    use crate::device::stamp::{Ctx, StampSink};
    use crate::{NetRef, Session, SolverConfig};
    use piperine_lang::{Design, SourceMap};
    use piperine_solver::abi::{AnalogReference, ElementCapabilities};
    use std::rc::Rc;

    const DIVIDER_WITH_HOST_DEVICE: &str = "\
nature Voltage;
  units = \"V\";
  access = V;
  abstol = 1e-6;
endnature
nature Current;
  units = \"A\";
  access = I;
  abstol = 1e-12;
endnature
discipline electrical;
  potential Voltage;
  flow Current;
enddiscipline

module Vsrc(p, n);
  inout electrical p, n;
  parameter real voltage = 0.0;
  analog V(p, n) <+ voltage;
endmodule

(* device = \"none\", type = \"host_resistor\" *)
module HostR(p, n);
  inout electrical p, n;
  parameter real r = 1e3;
endmodule

module Divider();
  electrical gnd, vin, mid;
  Vsrc #(.voltage(5.0)) src (vin, gnd);
  HostR #(.r(3e3)) r_top (vin, mid);
  HostR #(.r(2e3)) r_bot (mid, gnd);
endmodule
";

    struct HostR {
        p: AnalogReference,
        n: AnalogReference,
        g: f64,
    }
    impl HostElement for HostR {
        fn name(&self) -> &str {
            "host_r"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
        fn load_dc(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
            sink.g(&self.p, &self.p, self.g);
            sink.g(&self.p, &self.n, -self.g);
            sink.g(&self.n, &self.p, -self.g);
            sink.g(&self.n, &self.n, self.g);
        }
    }

    fn host_r_factory(spec: HostDeviceSpec) -> Result<Box<dyn Element>, String> {
        let node = |binding: &PortBinding| -> AnalogReference {
            match binding {
                PortBinding::Analog(r) => r.clone(),
                PortBinding::Digital(_) => panic!("HostR has no digital ports"),
            }
        };
        let p = node(&spec.ports[0]);
        let n = node(&spec.ports[1]);
        let r = spec.params.iter().find(|(k, _)| k == "r").map(|(_, v)| match v {
            piperine_lang::Value::Real(r) => *r,
            _ => 1e3,
        });
        Ok(Box::new(HostAdapter(HostR { p, n, g: 1.0 / r.unwrap_or(1e3) })))
    }

    fn elaborate(src: &str) -> Design {
        piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), piperine_lang::seed_device_port_schemas)
            .expect("elaborates with the @device/@port schemas seeded")
    }

    /// T17's core proof: a host device declared with `@device` builds and
    /// solves like any module — instantiated **more than once**, in a
    /// hierarchy, with parameters, resolved entirely through the host
    /// registry (no plugin present at all).
    #[test]
    fn a_host_registered_device_composes_like_any_module() {
        let design = elaborate(DIVIDER_WITH_HOST_DEVICE);
        let registry = Rc::new(HostDeviceRegistry::new());
        registry.register("host_resistor", host_r_factory);

        let mut session = Session::builder(&design, "Divider")
            .provider(registry)
            .compile()
            .expect("a host-registered @device compiles");
        let v = session
            .op(&SolverConfig::default(), None)
            .expect("op solves")
            .v(NetRef { name: "mid".into() })
            .expect("v(mid)");
        assert!((v - 2.0).abs() < 1e-9, "3k/2k divider from 5V gives 2.0V at mid, got {v}");
    }

    /// Neither binding present: the failure names both (DVT-20/21's "Done
    /// when" bullet).
    #[test]
    fn neither_plugin_nor_host_binding_found_fails_loud_naming_both() {
        let design = elaborate(DIVIDER_WITH_HOST_DEVICE);
        let registry = Rc::new(HostDeviceRegistry::new()); // nothing registered
        let err = match Session::builder(&design, "Divider").provider(registry).compile() {
            Err(e) => e,
            Ok(_) => panic!("no plugin and no host registration must fail loud"),
        };
        let msg = err.to_string();
        assert!(msg.contains("host_resistor"), "names the missing type id: {msg}");
        assert!(
            msg.contains("plugin") && msg.contains("host"),
            "names both binding modes that were tried: {msg}"
        );
    }

    /// The same `@device` declaration serves a plugin binding **and** a
    /// host binding unchanged — only the registration differs.
    #[test]
    fn the_same_device_declaration_serves_either_binding_mode() {
        struct FakePluginHost;
        impl DeviceProvider for FakePluginHost {
            fn build(&self, spec: PluginDeviceSpec) -> Result<Box<dyn Element>, String> {
                if spec.type_id != "host_resistor" {
                    return Err(format!("unknown plugin device type `{}`", spec.type_id));
                }
                let node = |b: &PortBinding| match b {
                    PortBinding::Analog(r) => r.clone(),
                    PortBinding::Digital(_) => panic!("no digital ports"),
                };
                let p = node(&spec.ports[0].binding);
                let n = node(&spec.ports[1].binding);
                let r = spec.params.iter().find(|(k, _)| k == "r").map(|(_, v)| match v {
                    piperine_lang::Value::Real(r) => *r,
                    _ => 1e3,
                });
                Ok(Box::new(HostAdapter(HostR { p, n, g: 1.0 / r.unwrap_or(1e3) })) as Box<dyn Element>)
            }
        }

        let design = elaborate(DIVIDER_WITH_HOST_DEVICE);

        // Bound as a plugin.
        let plugin_registry = Rc::new(HostDeviceRegistry::with_plugin(Rc::new(FakePluginHost)));
        let mut via_plugin = Session::builder(&design, "Divider")
            .provider(plugin_registry)
            .compile()
            .expect("plugin-bound @device compiles");
        let v_plugin = via_plugin
            .op(&SolverConfig::default(), None)
            .expect("op solves")
            .v(NetRef { name: "mid".into() })
            .expect("v(mid)");

        // Bound as a host registration — no declaration change.
        let host_registry = Rc::new(HostDeviceRegistry::new());
        host_registry.register("host_resistor", host_r_factory);
        let mut via_host = Session::builder(&design, "Divider")
            .provider(host_registry)
            .compile()
            .expect("host-bound @device compiles");
        let v_host = via_host
            .op(&SolverConfig::default(), None)
            .expect("op solves")
            .v(NetRef { name: "mid".into() })
            .expect("v(mid)");

        assert!(
            (v_plugin - v_host).abs() < 1e-9,
            "identical device, two binding modes, identical result: plugin={v_plugin} host={v_host}"
        );
    }
}
