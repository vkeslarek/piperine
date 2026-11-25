//! Items of a cell read three and four levels below the reader run from MSL source on the Rust backend (feature `nested-reach`,
//! part III §8.2.8, §8.4.8; GAPS L81 and L82): each model is written in `tests/nested_reach/`, elaborated by the front end,
//! compiled on the transpiled backend and run on the solver. Every expected value is computed here by Rust from the rule the test
//! cites, never read back from the model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/run.rs"]
mod run;

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

/// The voltage of each of `count` nodes joined by 1 S branches (`None` is ground) and fed `amps` at node `fed`: Kirchhoff's current
/// law `G v = i` over the branches, solved by Gaussian elimination.
fn unit_branches(count: usize, branches: &[(usize, Option<usize>)], fed: usize, amps: f64) -> Vec<f64> {
    let mut g = vec![vec![0.0; count + 1]; count];
    for (from, to) in branches {
        g[*from][*from] += 1.0;
        if let Some(to) = to {
            g[*to][*to] += 1.0;
            g[*from][*to] -= 1.0;
            g[*to][*from] -= 1.0;
        }
    }
    g[fed][count] = amps;
    for pivot in 0..count {
        for row in pivot + 1..count {
            let factor = g[row][pivot] / g[pivot][pivot];
            for column in pivot..=count {
                g[row][column] -= factor * g[pivot][column];
            }
        }
    }
    let mut v = vec![0.0; count];
    for row in (0..count).rev() {
        let known: f64 = (row + 1..count).map(|column| g[row][column] * v[column]).sum();
        v[row] = (g[row][count] - known) / g[row][row];
    }
    v
}

/// The five reads of the rig `top`, whose cell is at conductance `k`, against the cell's own values.
fn assert_reads(top: &str, k: f64) {
    let seen = RUN.operating_point("reach.msl", top, &["sn_read", "vs_read", "q_read", "k_read", "twice_read"]);
    for (value, wanted) in seen.iter().zip(CellOracle { k, drive: 1.0 }.reads()) {
        assert!((value - wanted).abs() < TOLERANCE, "{top}: {value} is not {wanted}: {seen:?}");
    }
}

#[test]
fn a_cell_three_and_four_levels_down_by_name_reads_its_node_lets_carrier_and_params() {
    // §8.2.8 (GAPS L81): `p.mid.c0` and `p.top.mid.c0`, a cell of `k = 1` driven at 1 V, read `sn` = 1/2 (the divider of 1 S and
    // 1 S), `vs` = `v(sn)` = 1/2, `q` = 2.5, `k` = 1 and `twice` = 2
    assert_reads("RigName3", 1.0);
    assert_reads("RigName4", 1.0);
}

#[test]
fn a_cell_three_and_four_levels_down_through_templates_reads_its_node_lets_carrier_and_params() {
    // §8.4.8: the cell the template names (`k = 2` three levels down, `k = 3` four), read through `Block` and `Wing`: `sn` = k / (k + 1)
    assert_reads("RigTemplate3", 2.0);
    assert_reads("RigTemplate4", 3.0);
}

#[test]
fn a_cell_of_a_generation_three_and_four_levels_down_reads_the_iteration_its_index_names() {
    // §8.2.23, §8.4.8: `cells[1]` of a generation reached through a template (`k = 4`, `k = 5`) and by name (`k = 1 + 1`)
    assert_reads("RigRow3", 4.0);
    assert_reads("RigRow4", 5.0);
    assert_reads("RigRows3", 2.0);
    assert_reads("RigRows4", 2.0);
}

#[test]
fn a_cycle_of_node_connections_three_and_four_levels_deep_runs_as_hardware() {
    // §8.2.14, §8.3.5 (GAPS L82): two wrappers wired to each other's cell node (three levels by name, four through templates) settle
    // where Kirchhoff's law over the cells' 1 S branches puts them, fed 1 A at `p`'s node: nodes 0 = p's, 1 = q's, the branch of each
    // cell from the other wrapper's node to its own and a 1 S leak at each; a wrapper wired to its own cell's node (four levels) sees
    // only the leak
    let pair = unit_branches(2, &[(1, Some(0)), (0, None), (0, Some(1)), (1, None)], 0, 1.0);
    for (top, names) in [("RigPair3", ["p.mid.c0.sn", "q.mid.c0.sn"]), ("RigPair4", ["p.block.bank.c0.sn", "q.block.bank.c0.sn"])] {
        let seen = RUN.operating_point("reach.msl", top, &names);
        for (value, wanted) in seen.iter().zip(&pair) {
            assert!((value - wanted).abs() < TOLERANCE, "{top}: {value} is not {wanted}: {seen:?}");
        }
    }
    let own = unit_branches(1, &[(0, Some(0)), (0, None)], 0, 1.0);
    let seen = RUN.operating_point("reach.msl", "RigOwn4", &["p.top.mid.c0.sn"]);
    assert!((seen[0] - own[0]).abs() < TOLERANCE, "RigOwn4: {} is not {}", seen[0], own[0]);
}

#[test]
fn a_cycle_of_node_connections_through_a_generation_four_levels_deep_runs_as_hardware() {
    // §8.2.14, §8.2.23 (GAPS L82): each wrapper places two cells (`cells[0]`, `cells[1]`) on the node it is wired to, the other
    // wrapper's `cells[1]` node: nodes 0 = p's `cells[1]`, 1 = q's `cells[1]`, 2 = p's `cells[0]`, 3 = q's `cells[0]`, each cell a
    // 1 S branch from the wired node to its own and a 1 S leak, fed 1 A at node 0
    let branches = [(1, Some(2)), (2, None), (1, Some(0)), (0, None), (0, Some(3)), (3, None), (0, Some(1)), (1, None)];
    let wanted = unit_branches(4, &branches, 0, 1.0);
    let seen = RUN.operating_point("reach.msl", "RigPairGeneration4", &["p.grid.row.cells[1].sn", "q.grid.row.cells[1].sn", "p.grid.row.cells[0].sn", "q.grid.row.cells[0].sn"]);
    for (value, wanted) in seen.iter().zip(&wanted) {
        assert!((value - wanted).abs() < TOLERANCE, "{value} is not {wanted}: {seen:?}");
    }
}
