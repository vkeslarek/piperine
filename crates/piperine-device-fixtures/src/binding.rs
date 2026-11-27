//! Binding hand-written devices by net name: a list of live instances with `(pin name, net name)` wires
//! becomes a `DigitalPlan`, the way the digital engine's tests place them.

use std::collections::HashMap;

use piperine_abi::caller::Handle;
use piperine_abi::raw::{kind, pin_flag, role};
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::net::{NetId, NetKind, NetSpec};
use piperine_digital_solver::plan::{Binding, DigitalPlan, InstancePlan};
use piperine_digital_solver::tick::Resolution;

/// One device to place: its path, the live instance and `(pin name, net name)` wires.
pub type Placed<'a> = (&'a str, Handle, Vec<(&'a str, &'a str)>);

/// Binds placed devices into a digital plan by net name.
pub struct PlanBinding;

impl PlanBinding {
    /// Binds `devices` by net name. A wired pin goes to the named net, created on first use from the
    /// pin's layout and flags (`QUIET`, `RECORD`, `RELAX`, `STRUCTURAL` of any pin on it, and the
    /// the `init` of any pin that has one). A pin with no wire gets a net of its own, named `<path>.<pin>`; an unwired
    /// `GLOBAL` pin goes to the design-wide net `global:<constant>`; an unwired `OPTIONAL` pin goes to
    /// nothing. A continuous pin binds to the sample slot of its wire's name, slots numbered in the
    /// order the names first appear.
    pub fn bind_by_name(devices: Vec<Placed<'_>>, resolution: Resolution) -> Result<DigitalPlan, DigitalError> {
        PlanBinding::bind_by_name_with(devices, resolution, |_| {})
    }

    /// `bind_by_name`, letting the test adjust the nets (`analog_reads`, say) before bind.
    pub fn bind_by_name_with(
        devices: Vec<Placed<'_>>,
        resolution: Resolution,
        adjust: impl FnOnce(&mut Vec<NetSpec>),
    ) -> Result<DigitalPlan, DigitalError> {
        let mut nets: Vec<NetSpec> = Vec::new();
        let mut by_name: HashMap<String, NetId> = HashMap::new();
        let mut samples: HashMap<String, u32> = HashMap::new();
        let mut instances = Vec::new();
        for (path, handle, wires) in devices {
            let shape = handle.shape();
            let mut pins = Vec::new();
            for p in shape.pins.iter() {
                let name = p.name.as_str();
                let wired = wires.iter().find(|(pin, _)| *pin == name).map(|(_, net)| net.to_string());
                if p.kind == kind::CONTINUOUS {
                    let key = wired.unwrap_or_else(|| format!("{path}.{name}"));
                    let next = samples.len() as u32;
                    pins.push(Binding::Sample(*samples.entry(key).or_insert(next)));
                    continue;
                }
                if wired.is_none() && p.flags & pin_flag::OPTIONAL != 0 {
                    pins.push(Binding::Absent);
                    continue;
                }
                let net = wired.unwrap_or_else(|| match p.role {
                    role::GLOBAL => format!("global:{}", p.global),
                    _ => format!("{path}.{name}"),
                });
                let id = *by_name.entry(net.clone()).or_insert_with(|| {
                    let mut spec = NetSpec::discrete(&net, p.layout.size, p.layout.align);
                    if p.kind == kind::EVENT {
                        spec.kind = NetKind::Event;
                    }
                    nets.push(spec);
                    NetId(nets.len() as u32 - 1)
                });
                let spec = &mut nets[id.0 as usize];
                spec.quiet |= p.flags & pin_flag::QUIET != 0;
                spec.record |= p.flags & pin_flag::RECORD != 0;
                spec.relax |= p.flags & pin_flag::RELAX != 0;
                spec.structural |= p.flags & pin_flag::STRUCTURAL != 0;
                if spec.init.is_empty() && p.flags & pin_flag::HAS_INIT != 0 {
                    spec.init = p.init.as_slice().to_vec();
                }
                pins.push(Binding::Net(id));
            }
            instances.push(InstancePlan { path: path.to_string(), handle, pins });
        }
        adjust(&mut nets);
        DigitalPlan::bind(nets, instances, resolution)
    }

    /// The id of the net called `name` in a bound plan.
    pub fn net_named(plan: &DigitalPlan, name: &str) -> NetId {
        let i = plan.nets.iter().position(|n| n.name == name).unwrap_or_else(|| panic!("no net called {name}"));
        NetId(i as u32)
    }
}
