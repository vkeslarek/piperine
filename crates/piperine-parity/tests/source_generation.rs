//! Generation in a module body runs from MSL source on the Rust backend (feature `generics` phase C, GN-17, GN-18): each
//! model is written in `tests/generation/`, elaborated by the front end, compiled on the transpiled backend and run on the
//! solver. Every expected value is computed here by Rust from the rule the test cites, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use piperine_lang::types::UnfinishedForm;
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

/// The model `file` of `tests/generation/`, with generation in a module body admitted until its flip.
struct GenerationModels;

impl GenerationModels {
    /// The project of `file`.
    fn project(file: &str) -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generation").join(file);
        Project::new(&path).admitting(&[UnfinishedForm::Generation])
    }

    /// The voltage of each node in `names` of `top` of `file` at its operating point.
    fn operating_point(file: &str, top: &str, names: &[&str]) -> Vec<f64> {
        let mut running = Self::project(file).compile(top).unwrap_or_else(|r| panic!("{r}"));
        let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        names
            .iter()
            .map(|name| {
                let node = running.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("node {name}"));
                solved.get_node(&NodeIdentifier::Anonymous(node.id().index())).expect("v(n)")
            })
            .collect()
    }

    /// The middle node of a 1k feed from 1 V into `load` ohms: Rust's divider.
    fn divided(load: f64) -> f64 {
        load / (1e3 + load)
    }
}

#[test]
fn generated_instances_run_and_read_by_ordinal() {
    // GN-17 AC 1, AC 2 (§8.2.23): `count` resistors of `r` in parallel are `r / count`; the element `k` is the instance
    // `bank.rk[k]` of the host path, and a count of zero is no instance at all (an empty array)
    for (rig, count) in [("FourRig", 4.0_f64), ("OneRig", 1.0)] {
        let seen = GenerationModels::operating_point("bank.msl", rig, &["mid"]);
        let expected = GenerationModels::divided(4e3 / count);
        assert!((seen[0] - expected).abs() < 1e-9, "{rig}: {seen:?} against {expected}");
    }
    let ir = GenerationModels::project("bank.msl").elaborate("FourRig").unwrap_or_else(|r| panic!("{r}"));
    for k in 0..4 {
        let element = ir.instance_at(&format!("FourRig.bank.rk[{k}]")).unwrap_or_else(|| panic!("instance rk[{k}]"));
        assert_eq!(element.name(), format!("rk[{k}]"));
    }
    assert!(ir.instance_at("FourRig.bank.rk[4]").is_none(), "an array of 4 has no element 4");
    let none = GenerationModels::operating_point("bank.msl", "NoneRig", &["mid"]);
    assert!((none[0] - GenerationModels::divided(1e3)).abs() < 1e-9, "no resistor in the bank leaves the 1k load alone: {none:?}");
    let empty = GenerationModels::project("bank.msl").elaborate("NoneRig").unwrap_or_else(|r| panic!("{r}"));
    assert!(empty.instance_at("NoneRig.bank.rk[0]").is_none());
    // the loop variable reaches the param block: element `k` is `1k * (k + 1)`
    let ramp = GenerationModels::operating_point("bank.msl", "RampRig", &["mid"]);
    let conductance: f64 = (0..3).map(|k| 1.0 / (1e3 * (f64::from(k) + 1.0))).sum();
    assert!((ramp[0] - GenerationModels::divided(1.0 / conductance)).abs() < 1e-9, "{ramp:?}");
    let ir = GenerationModels::project("bank.msl").elaborate("RampRig").unwrap_or_else(|r| panic!("{r}"));
    let third = ir.instance_at("RampRig.ramp.rk[2]").expect("rk[2]");
    let parameter = third.params().first().expect("the resistor's r");
    assert_eq!(parameter.to_string().parse::<f64>().ok(), Some(3e3), "{parameter}");
}

#[test]
fn a_generation_if_places_the_taken_arm() {
    // GN-17 AC 1 (§8.2.24): `bypass` selects one arm: 1 ohm in the low rig, 1M in the high one; the other arm's instance
    // does not exist
    let low = GenerationModels::operating_point("bank.msl", "ChoiceRig", &["low_mid", "high_mid"]);
    assert!((low[0] - GenerationModels::divided(1.0)).abs() < 1e-9 && (low[1] - GenerationModels::divided(1e6)).abs() < 1e-9, "{low:?}");
    let ir = GenerationModels::project("bank.msl").elaborate("ChoiceRig").unwrap_or_else(|r| panic!("{r}"));
    assert!(ir.instance_at("ChoiceRig.low.low").is_some() && ir.instance_at("ChoiceRig.low.high").is_none());
    assert!(ir.instance_at("ChoiceRig.high.high").is_some() && ir.instance_at("ChoiceRig.high.low").is_none());
}

#[test]
fn nested_loops_make_a_nested_array() {
    // GN-17 AC 2 (§8.2.23): `rows` by `columns` cells, the cell of row `i` and column `j` is `cell[i][j]`; `rows * columns`
    // resistors of 6k in parallel
    let seen = GenerationModels::operating_point("bank.msl", "GridRig", &["mid"]);
    assert!((seen[0] - GenerationModels::divided(6e3 / 6.0)).abs() < 1e-9, "{seen:?}");
    let ir = GenerationModels::project("bank.msl").elaborate("GridRig").unwrap_or_else(|r| panic!("{r}"));
    for (row, column) in [(0, 0), (0, 2), (1, 1), (1, 2)] {
        assert!(ir.instance_at(&format!("GridRig.grid.cell[{row}][{column}]")).is_some(), "cell[{row}][{column}]");
    }
    assert!(ir.instance_at("GridRig.grid.cell[2][0]").is_none() && ir.instance_at("GridRig.grid.cell[0][3]").is_none());
}

/// The Book's ladder at `sections` sections and the same ladder written out, read side by side.
struct Ladders;

impl Ladders {
    /// The nodes both models name, below the top: the output and every tap.
    fn names(sections: usize) -> Vec<String> {
        std::iter::once("vout".to_string()).chain((0..=sections).map(|k| format!("lad.tap[{k}]"))).collect()
    }

    /// The voltage of each node of `names` of `top` at the operating point.
    fn operating_point(top: &str, names: &[String]) -> Vec<f64> {
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        GenerationModels::operating_point("ladder.msl", top, &names)
    }

    /// The voltage of each node of `names` at every accepted point of the transient of `top` to 5 us, steps of at most 0.1 us:
    /// the time, then the voltages.
    fn transient(top: &str, names: &[String]) -> Vec<(f64, Vec<f64>)> {
        let built = support::build(&GenerationModels::project("ladder.msl"), top);
        let nodes: Vec<NodeIdentifier> = names.iter().map(|name| NodeIdentifier::Anonymous(built.ir.node_at(&format!("{top}.{name}")).expect("a node").id().index())).collect();
        let (_, result) = built.run_with(5e-6, 1e-7);
        result.iter().map(|point| (point.time(), nodes.iter().map(|node| point.get_node(node).expect("v(n)")).collect())).collect()
    }
}

#[test]
fn the_generic_ladder_equals_the_ladder_written_out() {
    // GN-18 AC 2: `Ladder::<N>` and the sections written one by one are one netlist: at `N = 1, 3, 8` every tap is the same
    // number, to the bit, at the operating point (the capacitors open, so every tap is the 0 V the ramp starts at) and at every
    // point of a transient of the ramp (a charging tap is neither 0 nor 1, so the equality is not vacuous)
    for sections in [1usize, 3, 8] {
        let names = Ladders::names(sections);
        let generated = Ladders::operating_point(&format!("Generic{sections}"), &names);
        let written = Ladders::operating_point(&format!("Explicit{sections}"), &names);
        assert_eq!(generated.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), written.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "N = {sections}");
        let generated = Ladders::transient(&format!("Generic{sections}"), &names);
        let written = Ladders::transient(&format!("Explicit{sections}"), &names);
        assert!(generated.len() > 20, "the transient takes steps");
        assert_eq!(generated.len(), written.len(), "N = {sections}: the same accepted points");
        for ((time, left), (other, right)) in generated.iter().zip(&written) {
            assert_eq!(time.to_bits(), other.to_bits(), "N = {sections}: the same instants");
            assert_eq!(left.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), right.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "N = {sections} at {time}");
        }
        let (_, last) = generated.last().expect("a last point");
        assert!(last[0] > 0.0 && last[0] < 1.0, "the output of the {sections}-section ladder is charging at 5 us: {last:?}");
    }
}
