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

use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

/// The model `file` of `tests/generation/`, .
struct GenerationModels;

impl GenerationModels {
    /// The project of `file`.
    fn project(file: &str) -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generation").join(file);
        Project::new(&path)
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

/// The bank of lines of `tests/generation/bank_digital.msl`, run for 4 ns.
struct Bank;

impl Bank {
    /// One nanosecond in the ticks the changes are stamped in.
    const NS: i64 = 1_000_000;

    /// The changes of the net `Bench.<name>` during the transient: instant in femtoseconds and bytes, in order.
    fn changes(name: &str) -> Vec<(i64, Vec<u8>)> {
        let built = support::build(&GenerationModels::project("bank_digital.msl"), "Bench");
        let seen = built.run(4e-9, 0.5e-9);
        support::transient(&seen, &format!("Bench.{name}"))
    }
}

#[test]
fn a_generated_bank_reacts_per_element() {
    // GN-19 AC 1, AC 2 (§12.1.2): the reactions of iteration `k` latch element `k` when line `k` rises, at `k + 1` ns: element
    // `k` becomes `k + 10` then and no other element changes (each element's net has its own changes); of the two reactions of one iteration the later commits last
    // (`order[k]` is 2 and was never left at 1); the lines `0` and `2` rise together at 3 ns and the later iteration wins
    for k in 0..3_i64 {
        let latched = Bank::changes(&format!("latched[{k}]"));
        assert_eq!(latched, vec![((k + 1) * Bank::NS, vec![(k + 10) as u8])], "element {k} latches its own value when its line rises");
        let order = Bank::changes(&format!("order[{k}]"));
        assert_eq!(order, vec![((k + 1) * Bank::NS, vec![2])], "element {k} takes the later commit and never held the earlier one");
    }
    let winner = Bank::changes("winner");
    assert_eq!(winner, vec![(Bank::NS, vec![1]), (2 * Bank::NS, vec![2]), (3 * Bank::NS, vec![3])]);
}

#[test]
fn generation_in_a_digital_block_runs_from_every_entry() {
    // GN-19 (design-c "Interim"): with the last unfinished form closed, the plain entries take generation in a `digital` block:
    // the whole-model check, the check from a top, the elaboration and the compiled run; superseding the pin
    // `unfinished_forms_in_force_are_the_ones_phase_c_has_not_closed`, which listed it as refused
    let project = GenerationModels::project("bank_digital.msl");
    let design = project.design().unwrap_or_else(|r| panic!("{r}"));
    piperine_lang::types::Types::check_model(&design).unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    piperine_lang::types::Types::check(&design, "Bench").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    let ir = project.elaborate("Bench").unwrap_or_else(|r| panic!("{r}"));
    assert!(ir.instance_at("Bench").is_some_and(|bench| bench.body().reactions().len() == 15), "five reactions for each of the three iterations");
    project.compile("Bench").unwrap_or_else(|r| panic!("{r}"));
}

/// The bench of `tests/generation/generated_items.msl`, run for 4 ns.
struct GeneratedItems;

impl GeneratedItems {
    /// The changes of the net `Bench.<name>` during the transient: instant in femtoseconds and bytes, in order.
    fn changes(name: &str) -> Vec<(i64, Vec<u8>)> {
        let built = support::build(&GenerationModels::project("generated_items.msl"), "Bench");
        let seen = built.run(4e-9, 0.5e-9);
        support::transient(&seen, &format!("Bench.{name}"))
    }
}

#[test]
fn carriers_definitions_and_hoisted_placements_in_generation_run() {
    // GN-17 AC 1 (§8.2.23): `signal s` in `for k in 0..3` is the array `s`, element `k` starting at `k + 1`; `let d = Double(s)`
    // is one hoisted placement per iteration reading its element; `y[k] = d + k` one definition per iteration. So `y[k]` is
    // `2 * s[k] + k`. `tick` rises at 1 ns and 3 ns; each rise commits `s[1] + 10` to `s[1]` and `s[0] + 1` to `s[0]`, from
    // outside the loop, and `s[2]` is never committed
    let s0 = |rises: u8| 1 + rises;
    let s1 = |rises: u8| 2 + 10 * rises;
    let y = |k: u8, s: u8| 2 * s + k;
    assert_eq!(GeneratedItems::changes("y[0]"), vec![(Bank::NS, vec![y(0, s0(1))]), (3 * Bank::NS, vec![y(0, s0(2))])], "the element 0 follows its carrier");
    assert_eq!(GeneratedItems::changes("y[1]"), vec![(Bank::NS, vec![y(1, s1(1))]), (3 * Bank::NS, vec![y(1, s1(2))])], "the element 1 follows its carrier");
    assert!(GeneratedItems::changes("y[2]").is_empty(), "the element 2 keeps `2 * 3 + 2`, set at (0, 0)");
    assert_eq!(GeneratedItems::changes("s[1]"), vec![(Bank::NS, vec![s1(1)]), (3 * Bank::NS, vec![s1(2)])], "the generated carrier holds its commits");
    // §8.2.8, §8.2.23: `tap.m` read inside the loop is the member of the iteration's instance, `s[k] + 100`; `tap[1].m` read
    // outside names the instance by its ordinal; the element 2 of `s` never changes, so neither does `w[2]`
    let m = |s: u8| s + 100;
    assert_eq!(GeneratedItems::changes("w[0]"), vec![(Bank::NS, vec![m(s0(1))]), (3 * Bank::NS, vec![m(s0(2))])]);
    assert_eq!(GeneratedItems::changes("w[1]"), vec![(Bank::NS, vec![m(s1(1))]), (3 * Bank::NS, vec![m(s1(2))])]);
    assert!(GeneratedItems::changes("w[2]").is_empty(), "the element 2 keeps `3 + 100`");
    assert_eq!(GeneratedItems::changes("far"), vec![(Bank::NS, vec![m(s1(1))]), (3 * Bank::NS, vec![m(s1(2))])]);
}

#[test]
fn a_hoisted_placement_with_contributions_runs_once_per_iteration() {
    // GN-17 AC 1 (§8.2.23): three loads of `k + 1` mS hoisted by a `for` are 6 mS in parallel under the 1k feed
    let seen = GenerationModels::operating_point("generated_items.msl", "LoadRig", &["mid"]);
    let expected = GenerationModels::divided(1.0 / 6e-3);
    assert!((seen[0] - expected).abs() < 1e-9, "{seen:?} against {expected}");
}

/// The bench of `tests/generation/hoisted_generation.msl`, run for 3 ns.
struct HoistedGeneration;

impl HoistedGeneration {
    /// A nanosecond, in femtoseconds.
    const NS: i64 = 1_000_000;

    /// The changes of the net `Bench.<name>` during the transient: instant in femtoseconds and bytes, in order.
    fn changes(name: &str) -> Vec<(i64, Vec<u8>)> {
        let built = support::build(&GenerationModels::project("hoisted_generation.msl"), "Bench");
        let seen = built.run(3e-9, 0.5e-9);
        support::transient(&seen, &format!("Bench.{name}"))
    }
}

#[test]
fn a_hoisted_module_with_generation_runs() {
    // GN-17 AC 1 (§8.2.23, §8.3.12): each placement of `Triple` unrolls its own three carriers `s[k] = x * (k + 1)`, and the
    // result is their sum, `6 x`. `a` is 7 at 1 ns and 2 at 2 ns, `b` is 3 at 2 ns: `y` is `6 a` and `z` is `6 b`, each element
    // of a frame named under its placement
    let sum = |x: u8| 6 * x;
    let ns = HoistedGeneration::NS;
    assert_eq!(HoistedGeneration::changes("y"), vec![(ns, vec![sum(7)]), (2 * ns, vec![sum(2)])]);
    assert_eq!(HoistedGeneration::changes("z"), vec![(2 * ns, vec![sum(3)])], "the second placement follows `b` alone");
    for k in 0..3u8 {
        assert_eq!(HoistedGeneration::changes(&format!("first.s[{k}]")), vec![(ns, vec![7 * (k + 1)]), (2 * ns, vec![2 * (k + 1)])], "the element {k} of the first frame");
        assert_eq!(HoistedGeneration::changes(&format!("second.s[{k}]")), vec![(2 * ns, vec![3 * (k + 1)])], "the element {k} of the second frame");
    }
}

#[test]
fn a_template_array_element_placed_in_generation_runs() {
    // GN-17, GAPS L88 (§8.4.1, §8.4.4): `fs[k](a)` in a generation `for` places the element `k` of the array of templates at its
    // own params: `y[k]` is `a * m[k]` with `m` = 1, 2, 5, and `a` goes from 3 to 4 at 1 ns
    let built = support::build(&GenerationModels::project("template_array.msl"), "Bench");
    let seen = built.run(2e-9, 0.5e-9);
    let ns = 1_000_000;
    for (k, m) in [1u8, 2, 5].into_iter().enumerate() {
        assert_eq!(support::transient(&seen, &format!("Bench.y[{k}]")), vec![(ns, vec![4 * m])], "element {k}: a * {m} after `a` becomes 4");
    }
}

#[test]
fn a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override() {
    // GN-17, GAPS L89 (§8.1, §8.4.1, §8.4.4): `fs[k](a)` of a param `fs: [Scale; 2]` places the element `k` at its own params: line
    // `k` is `a * m[k]` with the default `m` = 1, 2, the literal `m` = 5, 6 and the `const` `m` = 3, 7 and the choice `m` = 9, 4; `a` goes from 3 to 4 at 1 ns
    let built = support::build(&GenerationModels::project("template_param_array.msl"), "Bench");
    let seen = built.run(2e-9, 0.5e-9);
    let ns = 1_000_000;
    for (line, multipliers) in [("d", [1u8, 2]), ("l", [5, 6]), ("c", [3, 7]), ("x", [9, 4])] {
        for (k, m) in multipliers.into_iter().enumerate() {
            assert_eq!(support::transient(&seen, &format!("Bench.{line}.y[{k}]")), vec![(ns, vec![4 * m])], "line {line} element {k}: a * {m} after `a` becomes 4");
        }
    }
}

/// The model of `composite_params.msl`: a template carrying composite params, placed by name and through the template.
struct CompositeParams;

impl CompositeParams {
    /// The changes of the net `Bench.<name>` of the run: instant in femtoseconds and bytes, in order.
    fn changes(name: &str) -> Vec<(i64, Vec<u8>)> {
        let built = support::build(&GenerationModels::project("composite_params.msl"), "Bench");
        let seen = built.run(2e-9, 0.5e-9);
        support::transient(&seen, &format!("Bench.{name}"))
    }
}

#[test]
fn a_template_carrying_composite_params_runs_through_a_placement() {
    // §8.3.12, §8.4.1, §3.2.10: with `a` = 3 after 1 ns, `Mix` at its defaults is `a * c[1] + 3 + 4 + 9` and the template `{ c: [5, 6], o: None,
    // os: [None, Some(1)] }` is `a * 6 + 0 + 0 + 1`; `Gain` through `{ c: [7, 8] }` is `a * 8` and by name `a * 2`
    let defaults = |a: u8| a * 2 + 3 + 4 + 9;
    let given = |a: u8| a * 6 + 1;
    let ns = 1_000_000;
    assert_eq!(CompositeParams::changes("by_name"), vec![(ns, vec![defaults(3)])]);
    assert_eq!(CompositeParams::changes("by_template"), vec![(ns, vec![given(3)])]);
    assert_eq!(CompositeParams::changes("gained_name"), vec![(ns, vec![3 * 2])]);
    assert_eq!(CompositeParams::changes("gained_template"), vec![(ns, vec![3 * 8])]);
}

#[test]
fn a_module_with_a_result_through_a_callable_bound_param_runs() {
    // §8.4.4, §8.3.12: `s(a)` with `s: C`, `C: Fn(u8) -> u8` places `Scale` at the params of the template each `Mid` is given: `p` is
    // `a * 5` and `q` is `a * 7`, with `a` going from 3 to 4 at 1 ns
    let built = support::build(&GenerationModels::project("callable_result.msl"), "Bench");
    let seen = built.run(2e-9, 0.5e-9);
    let ns = 1_000_000;
    for (net, m) in [("p", 5u8), ("q", 7)] {
        assert_eq!(support::transient(&seen, &format!("Bench.{net}")), vec![(ns, vec![4 * m])], "{net}: a * {m} after `a` becomes 4");
    }
}
