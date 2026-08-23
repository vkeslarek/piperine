//! LRM-COVERS: 6.5.8
//!
//! LRMW2-53 (`lrm-conformance-wave2`, T48): §6.5.8's own words — "The
//! node shall be treated as having a potential abstol with a value equal
//! to the smallest abstol of all the potential natures of all the
//! disciplines with which it is associated." `Tolerances::has_converged`/
//! `residual_test` used to read one flat `vntol` scalar for every node,
//! with no way to carry a per-node value at all. `Tolerances::
//! node_vntol_overrides` (an `Rc<HashMap<NodeIdentifier, f64>>`) is the
//! carrying mechanism this task adds — this file proves it is
//! *discriminating*, not just stored: the same voltage-step update
//! passes the convergence test under the default `vntol` and fails it
//! once that one node's `vntol` is tightened, observably changing
//! whether the Newton loop would accept the step.

use piperine_solver::prelude::{NodeIdentifier, Netlist, Tolerances};
use std::collections::HashMap;
use std::rc::Rc;

/// A one-node netlist and a voltage-step update just inside the default
/// `vntol` (1e-6 V) but well outside a tightened one (1e-9 V) — a
/// noise-free, deterministic gap (5e-7 V) chosen with headroom on both
/// sides of the default so floating-point rounding can't flip either
/// verdict. Both values sit near zero (not ~1 V) so the *relative* term
/// (`reltol * magnitude`, 1e-3 of the magnitude) stays negligible and the
/// *absolute* `vntol`/override term is what actually decides the test —
/// at magnitude ~1 V the relative term alone (~1e-3 V) would swamp any
/// vntol/override difference (both far below 1e-3), masking the effect
/// this test exists to prove.
fn one_node_step() -> (Netlist, ndarray::Array1<f64>, ndarray::Array1<f64>) {
    let mut netlist = Netlist::new();
    let _reference = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let old_values = ndarray::Array1::from(vec![0.0]);
    let new_values = ndarray::Array1::from(vec![5e-7]); // +5e-7 V
    (netlist, old_values, new_values)
}

/// Default `vntol` (1e-6): a 5e-7 V step is within tolerance — converged.
#[test]
fn default_vntol_accepts_the_step() {
    let (netlist, old, new) = one_node_step();
    let tol = Tolerances::default();
    assert!(
        tol.has_converged(Some(old.view()), &new.view(), &netlist),
        "5e-7 V step must pass under the default 1e-6 V vntol"
    );
}

/// AC15/discrimination: the identical step, with *only* this one node's
/// `vntol` tightened to 1e-9 via `node_vntol_overrides` (the §6.5.8
/// carrying mechanism) — now fails the same convergence test. This is
/// the "observably tightens convergence," not merely stored, proof:
/// nothing else about `Tolerances` or the netlist changed.
#[test]
fn tightened_node_vntol_rejects_the_same_step() {
    let (netlist, old, new) = one_node_step();
    let mut overrides = HashMap::new();
    overrides.insert(NodeIdentifier::Anonymous(0), 1e-9);
    let tol = Tolerances { node_vntol_overrides: Rc::new(overrides), ..Tolerances::default() };
    assert!(
        !tol.has_converged(Some(old.view()), &new.view(), &netlist),
        "the same 5e-7 V step must fail once this node's vntol is tightened to 1e-9 V"
    );
}

/// A node with *no* override still uses the flat default — tightening one
/// node's tolerance must not leak onto an unrelated node (the override
/// map is genuinely per-node, not a global switch in disguise).
#[test]
fn an_unrelated_node_is_unaffected_by_another_nodes_override() {
    let mut netlist = Netlist::new();
    let _n0 = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let _n1 = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let old_values = ndarray::Array1::from(vec![0.0, 0.0]);
    let new_values = ndarray::Array1::from(vec![5e-7, 5e-7]);

    let mut overrides = HashMap::new();
    overrides.insert(NodeIdentifier::Anonymous(0), 1e-9); // tighten only node 0
    let tol = Tolerances { node_vntol_overrides: Rc::new(overrides), ..Tolerances::default() };

    // Node 0 alone (its own netlist, same tightened value) fails, proving
    // the override reaches it; node 1's default-vntol reference passing
    // under the SAME `Tolerances` (checked via `residual_test`'s scale
    // path below) proves the tightening didn't leak.
    assert!(!tol.has_converged(Some(old_values.view()), &new_values.view(), &netlist));

    let residual = vec![0.0, 5e-7]; // node 0's row satisfied, node 1's is the 5e-7 gap
    let scale = vec![0.0, 0.0];
    assert!(
        tol.residual_test(&netlist, &residual, &scale),
        "node 1 (no override) must still pass residual_test under the flat default vntol"
    );
}
