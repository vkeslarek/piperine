//! Safe circuit assembly: [`CircuitBuilder`] wraps the manual `Netlist` API
//! and binds the device instances (abi.md §9) into a [`CircuitInstance`].

use std::collections::HashMap;

use piperine_analog_solver::netlist::{AnalogReference, Netlist, NodeIdentifier};
use crate::core::bind::{bind, BindInput, DeviceSpec, ParamValue, Port};
use crate::core::circuit::CircuitInstance;
use crate::core::result::Result;
use piperine_digital_solver::landing::TimePointRule;

// ── CircuitBuilder ────────────────────────────────────────────────────────────

/// Safe, discoverable circuit assembly. Wraps the manual `Netlist` API so
/// hosts never call `Netlist::connect_node` directly.
///
/// Construction boundary (design §6b): this builder is the one construction
/// path — [`build`](Self::build) is the sole intended caller of
/// [`CircuitInstance::from_netlist`], and `CircuitInstance` grows
/// no ad-hoc constructor beyond it. After construction, re-entry goes through
/// the analysis drivers (e.g. `TransientSolver::with_initial_state`) and the
/// MD-18 restamp path (`CircuitInstance::set` + re-run), never through a new
/// constructor.
///
/// ```ignore
/// use piperine_solver::prelude::*;
/// let mut b = CircuitBuilder::new("top");
/// let gnd = b.ground();
/// let vdd = b.node("vdd");
/// b.device(my_module, "x1", &[], &[Port::Node(vdd)]);
/// let circuit = b.build()?;
/// ```
pub struct CircuitBuilder {
    title: String,
    netlist: Netlist,
    /// name → reference cache (includes "gnd" → ground)
    nodes: HashMap<String, AnalogReference>,
    /// The device instances, bound in [`build`](Self::build).
    devices: Vec<DeviceSpec>,
    shared_params: Vec<(String, ParamValue)>,
    session_params: Vec<(String, ParamValue)>,
    resolution: f64,
    /// The bound on the microsteps of one instant of the digital engine.
    microsteps: usize,
    /// Which scheduled instants the digital engine's look-ahead lands on.
    time_point_rule: TimePointRule,
    /// monotonically increasing counter for fresh Anonymous node IDs
    next_anon: usize,
}

/// Names that route to the ground reference regardless of case (netlist convention).
fn is_gnd_name(name: &str) -> bool {
    matches!(name, "gnd" | "GND" | "vss" | "VSS")
}

impl CircuitBuilder {
    /// Create an empty builder with the given circuit title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            netlist: Netlist::new(),
            nodes: HashMap::new(),
            devices: Vec::new(),
            shared_params: Vec::new(),
            session_params: Vec::new(),
            resolution: piperine_digital_solver::tick::Resolution::default().seconds_per_tick,
            microsteps: piperine_digital_solver::limits::DigitalLimits::default().max_delta_cycles,
            time_point_rule: TimePointRule::default(),
            next_anon: 0,
        }
    }

    /// Ground reference. Idempotent — repeated calls return the same reference.
    pub fn ground(&mut self) -> AnalogReference {
        self.nodes
            .entry("gnd".to_string())
            .or_insert_with(|| self.netlist.connect_node(NodeIdentifier::Gnd))
            .clone()
    }

    /// Named analog node. Same name → same reference (idempotent lookup).
    /// Ground-family names (`"gnd"`, `"GND"`, `"vss"`, `"VSS"`) route to ground.
    pub fn node(&mut self, name: &str) -> AnalogReference {
        if is_gnd_name(name) {
            return self.ground();
        }
        if let Some(r) = self.nodes.get(name) {
            return r.clone();
        }
        // NodeIdentifier has no Named variant; use Anonymous with a fresh counter.
        // The name→reference mapping is tracked by the builder's nodes HashMap.
        let id = NodeIdentifier::Anonymous(self.next_anon);
        self.next_anon += 1;
        let r = self.netlist.connect_node(id);
        self.nodes.insert(name.to_string(), r.clone());
        r
    }

    /// Adds an instance of `module` at `path`: its `PORT` pins bind by
    /// position to `ports`, its `PARAM` pins by name to `values` (a pin
    /// without one keeps the prototype's default), its `GLOBAL` pins to the
    /// session and its `PRIVATE` pins to nets the solver allocates (abi.md
    /// §3.1). Bind runs in [`build`](Self::build).
    pub fn device(
        &mut self,
        module: &'static piperine_abi::raw::Module,
        path: &str,
        values: &[(&str, ParamValue)],
        ports: &[Port],
    ) -> &mut Self {
        self.devices.push(DeviceSpec {
            module,
            path: path.to_string(),
            values: values.iter().map(|(n, v)| (n.to_string(), v.clone())).collect(),
            ports: ports.to_vec(),
        });
        self
    }

    /// A shared param net (a model card, abi.md P5): every instance whose
    /// `PARAM` is given `ParamValue::Shared(name)` is bound to it.
    pub fn param_net(&mut self, name: &str, seed: ParamValue) -> &mut Self {
        self.shared_params.push((name.to_string(), seed));
        self
    }

    /// The host's session parameter `name`, read by a `NAMED` global.
    pub fn session_param(&mut self, name: &str, value: ParamValue) -> &mut Self {
        self.session_params.push((name.to_string(), value));
        self
    }

    /// The session's tick length (the `RESOLUTION` global), seconds.
    pub fn resolution(&mut self, seconds_per_tick: f64) -> &mut Self {
        self.resolution = seconds_per_tick;
        self
    }

    /// The bound on the microsteps of one instant: an instant that needs more fails with `E0501`
    /// naming it. The default is the digital engine's own, 1000.
    pub fn microsteps(&mut self, bound: usize) -> &mut Self {
        self.microsteps = bound;
        self
    }

    /// Which scheduled instants the digital engine lands the analog step on (Part V §17.2 rule 2):
    /// only those something observes (the default) or every one the static reach analysis marks.
    pub fn time_point_rule(&mut self, rule: TimePointRule) -> &mut Self {
        self.time_point_rule = rule;
        self
    }

    /// Freeze: bind the device instances (abi.md §9) into a circuit.
    pub fn build(mut self) -> Result<CircuitInstance> {
        let temperature = piperine_analog_solver::tolerances::Tolerances::default().temperature;
        let recipe = crate::core::bind::Recipe {
            specs: std::mem::take(&mut self.devices),
            shared: std::mem::take(&mut self.shared_params),
            session: std::mem::take(&mut self.session_params),
            resolution: self.resolution,
            microsteps: self.microsteps,
            time_point_rule: self.time_point_rule,
            temperature,
        };
        let bound = bind(BindInput {
            specs: recipe.specs.clone(),
            shared: recipe.shared.clone(),
            session: recipe.session.clone(),
            netlist: &mut self.netlist,
            resolution: recipe.resolution,
            microsteps: recipe.microsteps,
            time_point_rule: recipe.time_point_rule,
            temperature: recipe.temperature,
        })?;
        let ground = self.nodes.get("gnd").cloned();
        let mut node_names: Vec<(String, AnalogReference)> =
            self.nodes.drain().filter(|(_, r)| Some(r) != ground.as_ref()).collect();
        node_names.sort_by(|a, b| a.0.cmp(&b.0));
        let mut instance = CircuitInstance::from_netlist(self.title, self.netlist);
        instance.node_names = node_names;
        instance.abi = Some(bound);
        instance.recipe = Some(recipe);
        Ok(instance)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac1_new_builder_is_empty() {
        // AC1: no instances, no nodes, no ground
        let b = CircuitBuilder::new("top");
        assert_eq!(b.devices.len(), 0);
        assert!(b.nodes.is_empty());
    }

    #[test]
    fn ac2_ground_is_idempotent() {
        // AC2: .ground() twice → same reference
        let mut b = CircuitBuilder::new("top");
        let g1 = b.ground();
        let g2 = b.ground();
        assert_eq!(g1, g2);
    }

    #[test]
    fn ac3_node_is_idempotent() {
        // AC3: .node("out") twice → same reference
        let mut b = CircuitBuilder::new("top");
        let r1 = b.node("out");
        let r2 = b.node("out");
        assert_eq!(r1, r2);
    }

    #[test]
    fn ac7_build_without_ground_succeeds() {
        // AC7: .build() without .ground() succeeds (pure digital / resistor-only)
        let b = CircuitBuilder::new("pure_digital");
        let circuit = b.build();
        assert!(circuit.is_ok());
    }

    #[test]
    fn edge_empty_build_succeeds() {
        // Edge: zero instances → valid empty CircuitInstance (no panic)
        let b = CircuitBuilder::new("empty");
        let circuit = b.build().expect("empty build should succeed");
        assert_eq!(circuit.bound().expect("bound").names.instances.len(), 0);
    }

    #[test]
    fn edge_gnd_name_routes_to_ground() {
        // Edge: .node("gnd") routes to the ground reference
        let mut b = CircuitBuilder::new("top");
        let g = b.ground();
        let n = b.node("gnd");
        assert_eq!(g, n);
        let n2 = b.node("GND");
        assert_eq!(g, n2);
    }
}
