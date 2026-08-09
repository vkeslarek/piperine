//! `BehaviorKind` is a public trait: a new kind written outside the
//! crate's own catalog file (`behavior/kinds.rs`) composes exactly like a
//! shipped one — no plugin, no build ceremony (`ideal.md` §3.7/§12.2).
//!
//! This file is itself the proof: it is an ordinary integration test crate
//! linking against `piperine_ir`, so a `BehaviorKind` impl written here
//! is, mechanically, "outside the crate."

use piperine_ir::behavior::{BehaviorKind, Crossing};

/// A toy external kind: `x^3`. Not in `kinds.rs`, not blessed in any way —
/// just an ordinary trait impl.
struct Cube;

impl BehaviorKind for Cube {
    fn value(&self, inputs: &[f64]) -> f64 {
        inputs[0].powi(3)
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        out[0] = 3.0 * inputs[0].powi(2);
    }
    fn crossings(&self, _inputs: &[f64]) -> Option<Crossing> {
        None
    }
}

#[test]
fn a_kind_defined_outside_the_catalog_composes_like_a_shipped_one() {
    let cube = Cube;
    assert_eq!(cube.value(&[2.0]), 8.0);
    let mut j = [0.0];
    cube.jacobian(&[2.0], &mut j);
    assert_eq!(j[0], 12.0);

    // Composes with a shipped kind exactly as a shipped kind would with
    // another: Behavior.scaled(Cube, 2.0).
    let scaled = piperine_ir::behavior::kinds::Scaled { inner: Box::new(Cube), k: 2.0 };
    assert_eq!(scaled.value(&[2.0]), 16.0);
    let mut j2 = [0.0];
    scaled.jacobian(&[2.0], &mut j2);
    assert_eq!(j2[0], 24.0);
}
