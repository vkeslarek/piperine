//! Safe circuit assembly: [`CircuitBuilder`] wraps the manual `Netlist` API,
//! and [`UnknownAllocator`] gives elements a pre-freeze seam to claim internal
//! MNA unknowns (auxiliary branch currents, hidden states) before the matrix
//! shape freezes.

use std::collections::HashMap;

use crate::analog::{AnalogReference, BranchIdentifier, Netlist, NodeIdentifier};
use crate::core::circuit::CircuitInstance;
use crate::core::element::{Element, ElementCapabilities};
use crate::digital::{DigitalNet, DigitalState};
use crate::error::{Error, SolverDomain};
use crate::result::Result;

// ── UnknownAllocator ─────────────────────────────────────────────────────────

/// Pre-freeze internal-unknown allocation seam handed to
/// [`AnalogDevice::allocate_unknowns`](crate::core::element::AnalogDevice::allocate_unknowns).
/// Constructed **only** by
/// [`CircuitBuilder::build`] — not exported in `prelude` (hosts never allocate
/// unknowns); exported in `abi` so elements can name the type in signatures.
pub struct UnknownAllocator<'a> {
    netlist: &'a mut Netlist,
    allocated: usize,
}

impl<'a> UnknownAllocator<'a> {
    pub(crate) fn new(netlist: &'a mut Netlist) -> Self {
        Self { netlist, allocated: 0 }
    }

    /// Allocate an auxiliary branch unknown keyed by `(component, name)`.
    /// Returns the [`AnalogReference`] the element should use in its stamps.
    /// Increments the internal counter used by [`CircuitBuilder::build`]'s
    /// `HAS_INTERNAL_UNKNOWNS` check.
    pub fn branch(&mut self, component: &str, name: &str) -> AnalogReference {
        let id = BranchIdentifier::new(component, name);
        self.allocated += 1;
        self.netlist.connect_branch(id)
    }

    /// How many unknowns this element allocated so far (read by `build()`'s check).
    pub fn allocated(&self) -> usize {
        self.allocated
    }
}

// ── CircuitBuilder ────────────────────────────────────────────────────────────

/// Safe, discoverable circuit assembly. Wraps the manual `Netlist` API so
/// hosts never call `Netlist::connect_node` directly.
///
/// Construction boundary (design §6b): this builder is the one construction
/// path — [`build`](Self::build) is the sole intended caller of
/// [`CircuitInstance::from_devices_and_netlist`], and `CircuitInstance` grows
/// no ad-hoc constructor beyond it. After construction, re-entry goes through
/// the analysis drivers (e.g. `TransientSolver::with_initial_state`) and the
/// MD-18 restamp path (`CircuitInstance::set_element_param` + re-run), never
/// through a new constructor.
///
/// ```ignore
/// use piperine_solver::prelude::*;
/// let mut b = CircuitBuilder::new("top");
/// let gnd = b.ground();
/// let vdd = b.node("vdd");
/// b.element(Box::new(my_element));
/// let circuit = b.build()?;
/// ```
pub struct CircuitBuilder {
    title: String,
    netlist: Netlist,
    /// name → reference cache (includes "gnd" → ground)
    nodes: HashMap<String, AnalogReference>,
    elements: Vec<Box<dyn Element>>,
    digital_labels: Vec<Option<String>>,
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
            elements: Vec::new(),
            digital_labels: Vec::new(),
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

    /// Digital net with optional label. Returns `DigitalNet(index)`, sequential.
    pub fn digital_net(&mut self, label: Option<&str>) -> DigitalNet {
        let idx = self.digital_labels.len();
        self.digital_labels.push(label.map(|s| s.to_string()));
        DigitalNet(idx)
    }

    /// Store an element (insertion order preserved).
    pub fn element(&mut self, element: Box<dyn Element>) -> &mut Self {
        self.elements.push(element);
        self
    }

    /// Freeze: run `allocate_unknowns` for every element (ABI-09), assemble
    /// `CircuitInstance`, size + label digital state, rebuild topology, init
    /// digital devices.
    ///
    /// Returns `Err` if any element allocated unknowns without declaring
    /// `HAS_INTERNAL_UNKNOWNS` (fail loud — programming error).
    pub fn build(mut self) -> Result<CircuitInstance> {
        // Step 1: allocate_unknowns per element, check HAS_INTERNAL_UNKNOWNS flag.
        for element in &mut self.elements {
            let mut alloc = UnknownAllocator::new(&mut self.netlist);
            element.allocate_unknowns(&mut alloc);
            if alloc.allocated() > 0
                && !element.capabilities().contains(ElementCapabilities::HAS_INTERNAL_UNKNOWNS)
            {
                return Err(Error::simple(
                    SolverDomain::Element,
                    format!(
                        "element `{}` allocated internal unknowns without declaring \
                         HAS_INTERNAL_UNKNOWNS",
                        element.name()
                    ),
                ));
            }
        }

        // Step 2: assemble CircuitInstance.
        let mut instance = CircuitInstance::from_devices_and_netlist(
            self.title,
            self.elements,
            self.netlist,
        );

        // Step 3: size + label digital state.
        let count = self.digital_labels.len();
        if count > 0 {
            let labels: Vec<String> = self
                .digital_labels
                .into_iter()
                .enumerate()
                .map(|(i, lbl)| lbl.unwrap_or_else(|| format!("d{i}")))
                .collect();
            instance.digital_state = DigitalState::with_labels(count, labels);
        }

        // Step 4: rebuild topology + init digital devices.
        instance.rebuild_digital_topology();
        instance.init_digital()?;

        Ok(instance)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog::AnalogReference;
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use crate::math::linear::Stamp;
    use crate::analyses::dc::DcAnalysisState;
    use crate::analyses::Context;


    // Test double that allocates an internal unknown
    struct TestVsource {
        v: f64,
        n1: AnalogReference,
        n2: AnalogReference,
        branch: Option<AnalogReference>,
    }

    impl AnalogDevice for TestVsource {
        fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
            self.branch = Some(alloc.branch("v", "i"));
        }
        fn load_dc(
            &mut self,
            _state: &DcAnalysisState<'_>,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, f64>> {
            let b = self.branch.as_ref().unwrap().clone();
            vec![
                Stamp::Matrix(self.n1.clone(), b.clone(), 1.0),
                Stamp::Matrix(b.clone(), self.n1.clone(), 1.0),
                Stamp::Matrix(self.n2.clone(), b.clone(), -1.0),
                Stamp::Matrix(b.clone(), self.n2.clone(), -1.0),
                Stamp::Rhs(b, self.v),
            ]
        }
    }

    impl DigitalDevice for TestVsource {}

    impl Introspect for TestVsource {}

    impl Element for TestVsource {
        fn name(&self) -> &str { "v" }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG
                | ElementCapabilities::LOADS_DC
                | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
        }
    }

    // Test double that allocates without declaring HAS_INTERNAL_UNKNOWNS
    struct BadElement;
    impl AnalogDevice for BadElement {
        fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
            let _ = alloc.branch("bad", "x");
        }
    }
    impl DigitalDevice for BadElement {}
    impl Introspect for BadElement {}
    impl Element for BadElement {
        fn name(&self) -> &str { "bad" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
    }

    #[test]
    fn ac1_new_builder_is_empty() {
        // AC1: empty netlist, 0 elements, 0 digital nets, no ground
        let b = CircuitBuilder::new("top");
        assert_eq!(b.elements.len(), 0);
        assert_eq!(b.digital_labels.len(), 0);
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
    fn ac4_digital_net_sequential_indices() {
        // AC4: .digital_net(Some("clk")) → sequential indices, label registered
        let mut b = CircuitBuilder::new("top");
        let d0 = b.digital_net(Some("clk"));
        let d1 = b.digital_net(None);
        assert_eq!(d0, DigitalNet(0));
        assert_eq!(d1, DigitalNet(1));
        assert_eq!(b.digital_labels[0], Some("clk".to_string()));
        assert_eq!(b.digital_labels[1], None);
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
        // Edge: zero elements → valid empty CircuitInstance (no panic)
        let b = CircuitBuilder::new("empty");
        let circuit = b.build().expect("empty build should succeed");
        assert_eq!(circuit.devices.len(), 0);
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

    #[test]
    fn abi09_allocator_grows_matrix() {
        // ABI-09 AC1+AC2: element allocates a branch → matrix grows by 1
        let mut b = CircuitBuilder::new("vsrc");
        let gnd = b.ground();
        let vdd = b.node("vdd");
        let v = TestVsource { v: 1.0, n1: vdd.clone(), n2: gnd.clone(), branch: None };
        b.element(Box::new(v));
        let circuit = b.build().expect("build should succeed");
        // gnd has no index (ground); vdd gets idx=0; branch gets idx=1
        // max_index() returns the highest allocated index (0-based)
        assert!(circuit.netlist().max_index() >= Some(1), "branch row should be allocated");
    }

    #[test]
    fn abi09_missing_flag_returns_err() {
        // ABI-09 AC3: element allocates unknowns without HAS_INTERNAL_UNKNOWNS → Err
        let mut b = CircuitBuilder::new("bad_circuit");
        b.element(Box::new(BadElement));
        let result = b.build();
        assert!(result.is_err(), "should error on missing HAS_INTERNAL_UNKNOWNS");
        // Use .err() to avoid Debug bound on CircuitInstance
        let err = result.err().expect("err expected");
        let msg = err.to_string();
        assert!(msg.contains("bad"), "error should name the offending element");
    }

    #[test]
    fn ac6_build_runs_digital_init() {
        // AC6: .build() runs init_digital — digital net count equals digital_net calls
        let mut b = CircuitBuilder::new("digital_test");
        let _clk = b.digital_net(Some("clk"));
        let _q = b.digital_net(Some("q"));
        let circuit = b.build().expect("build ok");
        assert_eq!(circuit.digital_state.nets.len(), 2);
    }
}
