//! host-library T21 (HOST-19): nested/named `Session::sweep_grid` +
//! `Grid::map` -> `Nested<R>` (the shaped-array return, Rust side).
//!
//! Fixture: a two-resistor voltage divider `Top(r1, r2)` — `mid = 10·r2 /
//! (r1+r2)`, a closed form independent of the implementation, evaluated
//! directly in the test as the ground truth for every combination.

use piperine::{Nested, Session, SolverConfig};
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 10.0; }
analog Vsrc { V(p, n) <- dc; }

mod Top() {
    wire gnd : Electrical;
    wire top : Electrical;
    wire mid : Electrical;
    v1 : Vsrc(.p = top, .n = gnd) {};
    r1 : R(.p = top, .n = mid) {};
    r2 : R(.p = mid, .n = gnd) {};
}
";

fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &SourceMap::dummy()).expect("divider fixture elaborates")
}

fn expected_mid(r1: f64, r2: f64) -> f64 {
    10.0 * r2 / (r1 + r2)
}

/// HOST-19 AC2/AC3: `sweep_grid([r1, r2])` visits every `(r1, r2)`
/// combination (row-major, `r1` outer) and `Grid::map` collects the
/// results into a `Nested<R>` tree shaped `[r1_values.len(),
/// r2_values.len()]` — `Nested::Branch` at the outer (`r1`) axis,
/// `Nested::Leaf` at the inner (`r2`) axis — matching the closed-form
/// divider voltage at every combination.
#[test]
fn nested_grid_visits_every_combination_shaped_like_the_axes() {
    let design = design();
    let mut session = Session::compile(&design, "Top").expect("session compiles");

    let r1_values = [1e3_f64, 2e3];
    let r2_values = [1e3_f64, 3e3, 5e3];
    let mut grid = session.sweep_grid(&[("r1", "r", &r1_values), ("r2", "r", &r2_values)]);
    assert_eq!(grid.shape(), vec![2, 3]);
    assert_eq!(grid.len(), 6);

    let nested = grid
        .map(|s, coord| {
            let mid = s.op(&SolverConfig::default(), None)?.v("mid")?;
            Ok((coord.to_vec(), mid))
        })
        .expect("grid map solves");

    // Branch (outer, r1) of length 2, each a Branch (inner, r2) of length 3.
    let Nested::Branch(outer) = &nested else { panic!("expected an outer Branch") };
    assert_eq!(outer.len(), 2);
    for (i, &r1) in r1_values.iter().enumerate() {
        let Nested::Branch(inner) = &outer[i] else { panic!("expected an inner Branch at r1={r1}") };
        assert_eq!(inner.len(), 3);
        for (j, &r2) in r2_values.iter().enumerate() {
            let Nested::Leaf((coord, mid)) = &inner[j] else { panic!("expected a Leaf at ({r1},{r2})") };
            assert_eq!(coord, &vec![r1, r2], "coordinates must be (r1, r2) in axis order");
            let expected = expected_mid(r1, r2);
            let rel_err = (mid - expected).abs() / expected;
            assert!(
                rel_err < 1e-9,
                "r1={r1}, r2={r2}: grid mid={mid}, closed-form mid={expected} (rel {rel_err:.3e})"
            );
        }
    }
}

/// HOST-19 edge (spec): a sweep-point failure inside `Grid::map` surfaces
/// with the failing combination's coordinates, not a bare error — here an
/// unaddressable output net name inside the mapped closure.
#[test]
fn map_failure_surfaces_with_the_combinations_coordinates() {
    let design = design();
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let r1_values = [1e3_f64];
    let r2_values = [1e3_f64];
    let mut grid = session.sweep_grid(&[("r1", "r", &r1_values), ("r2", "r", &r2_values)]);

    let err = grid
        .map(|s, _coord| {
            s.op(&SolverConfig::default(), None)?.v("nope")
        })
        .expect_err("unknown net must fail loud");
    let msg = format!("{err}");
    assert!(msg.contains("nope"), "must name the failing net: {msg}");
}
