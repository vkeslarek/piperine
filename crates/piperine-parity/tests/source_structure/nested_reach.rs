//! Items of a cell read three and four levels below the reader run from MSL source on the Rust backend (feature `nested-reach`,
//! part III §8.2.8, §8.4.8; GAPS L81 and L82): each model is written in `tests/nested_reach/`, elaborated by the front end,
//! compiled on the transpiled backend and run on the solver. Every expected value is computed here by Rust from the rule the test
//! cites, never read back from the model.
#![cfg(unix)]

use piperine_parity::source::network::UnitBranchNetwork;
use piperine_parity::source::run;

use piperine_api::{Field, Record};
use run::SourceRun;

/// The model files of `tests/nested_reach/`.
const RUN: SourceRun = SourceRun::in_directory("tests/nested_reach");

/// The tolerance of an operating point of sources and unit gains.
const TOLERANCE: f64 = 1e-9;

/// The value of the cell of the model `reach.msl`, at conductance `k` and driven at `drive` volts.
struct CellOracle {
    k: f64,
    drive: f64,
}

impl CellOracle {
    /// The storage node of the cell: the middle of a divider of the conductances `k` and `1` driven at `drive`, which is also what
    /// the `let` over the solve (`vs = v(sn)`) reads.
    fn storage(&self) -> f64 {
        self.drive * self.k / (self.k + 1.0)
    }

    /// The five values a rig reads, in the order of its nodes: `v(sn)`, `vs`, `q` (its initial value), `k`, `twice` (`2 k`).
    fn reads(&self) -> [f64; 5] {
        [self.storage(), self.storage(), 2.5, self.k, 2.0 * self.k]
    }
}

/// The reads of a nested-reach rig, checked against the cell it holds.
struct NestedReads;

impl NestedReads {
    /// The five reads of the rig `top`, whose cell is at conductance `k`, against the cell's own values.
    fn assert_reads(top: &str, k: f64) {
        let seen = RUN.operating_point("reach.msl", top, &["sn_read", "vs_read", "q_read", "k_read", "twice_read"]);
        for (value, wanted) in seen.iter().zip(CellOracle { k, drive: 1.0 }.reads()) {
            assert!((value - wanted).abs() < TOLERANCE, "{top}: {value} is not {wanted}: {seen:?}");
        }
    }

    /// The `seen` records of a bench of `drive.msl`, the value of each as a flag.
    fn seen_flags(top: &str) -> Vec<bool> {
        let records: Vec<Record> = RUN.records_of("drive.msl", top).into_iter().filter(|record| record.stream == "seen").collect();
        records
            .iter()
            .map(|record| match record.fields.iter().find(|(name, _)| name == "value") {
                Some((_, Field::Bool(value))) => *value,
                other => panic!("`value` is a bool: {other:?}"),
            })
            .collect()
    }
}

#[test]
fn a_cell_three_and_four_levels_down_by_name_reads_its_node_lets_carrier_and_params() {
    // §8.2.8 (GAPS L81): `p.mid.c0` and `p.top.mid.c0`, a cell of `k = 1` driven at 1 V, read `sn` = 1/2 (the divider of 1 S and
    // 1 S), `vs` = `v(sn)` = 1/2, `q` = 2.5, `k` = 1 and `twice` = 2
    NestedReads::assert_reads("RigName3", 1.0);
    NestedReads::assert_reads("RigName4", 1.0);
}

#[test]
fn a_cell_three_and_four_levels_down_through_templates_reads_its_node_lets_carrier_and_params() {
    // §8.4.8: the cell the template names (`k = 2` three levels down, `k = 3` four), read through `Block` and `Wing`: `sn` = k / (k + 1)
    NestedReads::assert_reads("RigTemplate3", 2.0);
    NestedReads::assert_reads("RigTemplate4", 3.0);
}

#[test]
fn a_cell_through_generic_templates_three_and_four_levels_down_reads_its_node_lets_carrier_and_params() {
    // §8.4.8: a type parameter (`C: Fn(Electrical)`) of each wrapper handed down to the wrapper below: the cell the template names
    // (`k = 2`, `k = 3`, and `k = 5` in a generation) is read through `GBlock`, `GWing` and `GPlane`
    NestedReads::assert_reads("RigGeneric3", 2.0);
    NestedReads::assert_reads("RigGeneric4", 3.0);
    NestedReads::assert_reads("RigGenericRow4", 5.0);
}

#[test]
fn a_cell_of_a_generation_three_and_four_levels_down_reads_the_iteration_its_index_names() {
    // §8.2.23, §8.4.8: `cells[1]` of a generation reached through a template (`k = 4`, `k = 5`) and by name (`k = 1 + 1`)
    NestedReads::assert_reads("RigRow3", 4.0);
    NestedReads::assert_reads("RigRow4", 5.0);
    NestedReads::assert_reads("RigRows3", 2.0);
    NestedReads::assert_reads("RigRows4", 2.0);
}

#[test]
fn a_cycle_of_node_connections_three_and_four_levels_deep_runs_as_hardware() {
    // §8.2.14, §8.3.5 (GAPS L82): two wrappers wired to each other's cell node (three levels by name, four through templates) settle
    // where Kirchhoff's law over the cells' 1 S branches puts them, fed 1 A at `p`'s node: nodes 0 = p's, 1 = q's, the branch of each
    // cell from the other wrapper's node to its own and a 1 S leak at each; a wrapper wired to its own cell's node (four levels) sees
    // only the leak
    let pair = UnitBranchNetwork::voltages(2, &[(1, Some(0)), (0, None), (0, Some(1)), (1, None)], 0, 1.0);
    for (top, names) in [("RigPair3", ["p.mid.c0.sn", "q.mid.c0.sn"]), ("RigPair4", ["p.block.bank.c0.sn", "q.block.bank.c0.sn"])] {
        let seen = RUN.operating_point("reach.msl", top, &names);
        for (value, wanted) in seen.iter().zip(&pair) {
            assert!((value - wanted).abs() < TOLERANCE, "{top}: {value} is not {wanted}: {seen:?}");
        }
    }
    let own = UnitBranchNetwork::voltages(1, &[(0, Some(0)), (0, None)], 0, 1.0);
    let seen = RUN.operating_point("reach.msl", "RigOwn4", &["p.top.mid.c0.sn"]);
    assert!((seen[0] - own[0]).abs() < TOLERANCE, "RigOwn4: {} is not {}", seen[0], own[0]);
}

#[test]
fn a_cycle_of_node_connections_through_a_generation_four_levels_deep_runs_as_hardware() {
    // §8.2.14, §8.2.23 (GAPS L82): each wrapper places two cells (`cells[0]`, `cells[1]`) on the node it is wired to, the other
    // wrapper's `cells[1]` node: nodes 0 = p's `cells[1]`, 1 = q's `cells[1]`, 2 = p's `cells[0]`, 3 = q's `cells[0]`, each cell a
    // 1 S branch from the wired node to its own and a 1 S leak, fed 1 A at node 0
    let branches = [(1, Some(2)), (2, None), (1, Some(0)), (0, None), (0, Some(3)), (3, None), (0, Some(1)), (1, None)];
    let wanted = UnitBranchNetwork::voltages(4, &branches, 0, 1.0);
    let seen = RUN.operating_point("reach.msl", "RigPairGeneration4", &["p.grid.row.cells[1].sn", "q.grid.row.cells[1].sn", "p.grid.row.cells[0].sn", "q.grid.row.cells[0].sn"]);
    for (value, wanted) in seen.iter().zip(&wanted) {
        assert!((value - wanted).abs() < TOLERANCE, "{value} is not {wanted}: {seen:?}");
    }
}

/// The three `let`s of the cell of `mixed.msl` over a param and the solve, at conductance `k`, gain `g`, driven at 1 V: `deep = k vs`,
/// `gated = vs` when `k > 0.5` else `0`, `gi = g v(sn)`, with `vs = v(sn)` the storage node of the divider (`CellOracle`).
struct MixOracle {
    k: f64,
    g: f64,
}

impl MixOracle {
    /// The reads of a rig, in the order of its nodes: `deep`, `gated`, `gi`.
    fn reads(&self) -> [f64; 3] {
        let storage = CellOracle { k: self.k, drive: 1.0 }.storage();
        [self.k * storage, if self.k > 0.5 { storage } else { 0.0 }, self.g * storage]
    }
}

#[test]
fn a_let_over_a_param_and_the_solve_one_three_and_four_levels_down_reads_the_placements_params() {
    // §8.2.25 (verification finding 10): the reader evaluates the cell's `let` in its own context with each param at the value the
    // placement gives, else the default: one level (`k = 2`, `g = 1.5`), three by name (`k` the default 1, `g = 2` from the
    // placement), three through a template (`k = 0.25`, under the threshold, so `gated` is 0), four in generation (`k = 0.25 + j`)
    for (top, k, g) in [("MixRig1", 2.0, 1.5), ("MixRig3", 1.0, 2.0), ("MixRigTemplate3", 0.25, 4.0), ("MixRigRows4", 1.25, 3.0), ("MixRigRows4First", 0.25, 3.0)] {
        let seen = RUN.operating_point("mixed.msl", top, &["deep_read", "gated_read", "gi_read"]);
        for (value, wanted) in seen.iter().zip(MixOracle { k, g }.reads()) {
            assert!((value - wanted).abs() < TOLERANCE, "{top}: {value} is not {wanted}: {seen:?}");
        }
    }
}

#[test]
fn a_given_value_over_a_param_of_the_reader_one_and_three_levels_down_reads_the_cell_it_sets() {
    // §8.2.25 (verification finding 11): the reader's `m = 1.5` sets the cell's `k` through the placement (`k: m * 2.0`, one level;
    // handed down through each wrapper's param three levels by name, and halved on the way in generation): the reader reads the
    // cell's node `sn`, `vs = v(sn)` and `deep = k vs` of the divider at that `k`
    let m = 1.5;
    for (top, k) in [("MixParamRig1", m * 2.0), ("MixParamRig3", m * 2.0), ("MixParamRigRows3", m * 0.5 * 2.0)] {
        let storage = CellOracle { k, drive: 1.0 }.storage();
        let seen = RUN.operating_point("mixed.msl", top, &["sn_read", "vs_read", "deep_read"]);
        for (value, wanted) in seen.iter().zip([storage, storage, k * storage]) {
            assert!((value - wanted).abs() < TOLERANCE, "{top}: {value} is not {wanted}: {seen:?}");
        }
    }
}

#[test]
fn a_cell_in_generation_whose_param_reads_a_param_and_the_loop_variable_runs_at_each_iteration() {
    // §8.2.23, §8.1.16: `k: m3 + (j as f64)` with `m3 = m * 0.5` handed down from the reader's `m = 1.5`: `cells[1]` runs at
    // `k = 1.75` (its node, `vs` and `deep = k vs` read through three levels) and `cells[0]` at `k = 0.75`
    let m = 1.5;
    let (k1, k0) = (m * 0.5 + 1.0, m * 0.5);
    let (s1, s0) = (CellOracle { k: k1, drive: 1.0 }.storage(), CellOracle { k: k0, drive: 1.0 }.storage());
    let seen = RUN.operating_point("mixed.msl", "MixLoopRig3", &["sn_read", "vs_read", "deep_read", "first_read"]);
    for (value, wanted) in seen.iter().zip([s1, s1, k1 * s1, s0]) {
        assert!((value - wanted).abs() < TOLERANCE, "{value} is not {wanted}: {seen:?}");
    }
}

#[test]
fn the_params_of_a_cell_in_generation_over_a_param_and_the_loop_variable_read_three_levels_down() {
    // §8.2.8, §8.2.23: `k: m3 + (j as f64)` with `m3 = m * 0.5`, `m = 1.5`: the reader reads `cells[1].k = 0.75 + 1`, its
    // `twice = 2 k`, and `cells[0].k = 0.75`
    let m = 1.5;
    let (k1, k0) = (m * 0.5 + 1.0, m * 0.5);
    let seen = RUN.operating_point("mixed.msl", "MixLoopRigParams3", &["k_read", "twice_read", "first_k_read"]);
    for (value, wanted) in seen.iter().zip([k1, 2.0 * k1, k0]) {
        assert!((value - wanted).abs() < TOLERANCE, "{value} is not {wanted}: {seen:?}");
    }
}

#[test]
fn a_body_reads_the_params_of_its_own_cells_in_generation_one_level_down() {
    // §8.2.8, §8.2.23 (verification finding 13): `k: m + (j as f64)` at `m = 0.25`: `c[1].k = 1.25`, `c[0].k = 0.25`, and the
    // condition `c[1].k > 1.0` holds, so the gated read is the node of `c[1]`, the divider at `k = 1.25`
    let m = 0.25;
    let (k1, k0) = (m + 1.0, m);
    let gated = if k1 > 1.0 { CellOracle { k: k1, drive: 1.0 }.storage() } else { 0.0 };
    let seen = RUN.operating_point("mixed.msl", "MixOwnRig1", &["k_read", "first_k_read", "gated_read"]);
    for (value, wanted) in seen.iter().zip([k1, k0, gated]) {
        assert!((value - wanted).abs() < TOLERANCE, "{value} is not {wanted}: {seen:?}");
    }
}

#[test]
fn a_parent_drives_a_resolved_carrier_of_a_cell_three_levels_down() {
    // §5.2.2, §22.3 (`AndWire`): the cell drives its `line` high, the top drives it low through `p.mid.c0.line`: the net holds the
    // AND of the two drives, false (the cell's own drive alone leaves it high, `QuietBench`)
    assert_eq!(NestedReads::seen_flags("DriveBench").last(), Some(&(true && false)), "the top's drive is held beside the cell's");
    assert_eq!(NestedReads::seen_flags("QuietBench").last(), Some(&true), "the cell's drive alone is high");
}
