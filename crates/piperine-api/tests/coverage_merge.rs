//! `cover` merge and closure data (dv-tester T34, DVT-43): counters merge
//! (sum) across runs and seeds on the host rather than replacing, merged
//! data identifies empty bins by name, coverage persists across runs
//! (unlike a margin — scoped to one analysis), a run with zero accepted
//! points reports every bin as not-exercised (never a vacuous 100%), and
//! a sweep filling a known subset names the empty bins with a second run
//! merging correctly.
//!
//! **Migrated** (constraint-statements): the fixture runs against the
//! `constraint begin … end` producer's `cover … bins […]` statement.

use piperine_api::CoverageDatabase;
use piperine_codegen::CircuitCompiler;
use piperine_solver::prelude::{CircuitInstance, Context};

/// Self-contained inline natures/discipline (elaborates under
/// `SourceMap::dummy()` regardless of the runner's CWD).
const DISCIPLINE: &str = "nature Voltage; units = \"V\"; access = V; abstol = 1e-6; endnature\nnature Current; units = \"A\"; access = I; abstol = 1e-12; endnature\ndiscipline Electrical; potential Voltage; flow Current; enddiscipline";

/// A source-driven resistor with a `cover` point over its own voltage,
/// baked to a literal DC level: `V(p,n) bins [0.0:1.0:3.0]` is 4 bins
/// (0,1,2,3).
fn circuit_at(v: f64) -> CircuitInstance {
    let src = format!(
        "{DISCIPLINE}
module Vsrc (inout Electrical p, inout Electrical n);
    parameter real dc = {v};
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module Dut (inout Electrical p, inout Electrical n);
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        cover c1 : V(p, n) bins [0.0:1.0:3.0];
    end
endmodule
module Top ();
    Electrical gnd, in;
    Vsrc v1 (.p(in), .n(gnd));
    Dut d1 (.p(in), .n(gnd));
endmodule
"
    );
    let design = piperine_lang::parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");
    circuit.dc(Context::default()).expect("dc setup").solve().expect("dc solves");
    circuit
}

/// DVT-43: a run's coverage reports come back through the solver's
/// `coverage_reports()` channel, correctly attributing the hit to the
/// declared bin (`V = 0.4` hits bin 0).
#[test]
fn a_solved_circuits_coverage_report_reflects_its_operating_point() {
    let circuit = circuit_at(0.4);
    let (points, crosses) = circuit.coverage_reports();
    assert_eq!(crosses.len(), 0);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].label, "c1");
    assert_eq!(points[0].instance_path, "d1");
    assert_eq!(points[0].counts, vec![1, 0, 0, 0], "V=0.4 hits bin 0");
}

/// DVT-43: counters **merge (sum)** across runs — never replace. Two runs
/// at the same operating point double the hit count for that bin.
#[test]
fn counters_merge_summing_not_replacing_across_runs() {
    let mut db = CoverageDatabase::new();
    let (p1, c1) = circuit_at(0.4).coverage_reports();
    db.merge_run(&p1, &c1);
    let (p2, c2) = circuit_at(0.4).coverage_reports();
    db.merge_run(&p2, &c2);

    let entry = db.points().get("d1.c1").expect("the point is keyed by instance_path.label");
    assert_eq!(entry.counts, vec![2, 0, 0, 0], "two runs at the same point sum, never replace");
}

/// DVT-43: a sweep filling a known subset of bins names the untouched
/// ones; a second run (a different subset) merges rather than replacing —
/// the union, not the latest run alone, determines what's still empty.
#[test]
fn a_sweep_names_empty_bins_and_a_second_run_merges() {
    let mut db = CoverageDatabase::new();
    // First "run": a sweep hitting bins 0 and 1 only.
    for v in [0.0, 1.0] {
        let (p, c) = circuit_at(v).coverage_reports();
        db.merge_run(&p, &c);
    }
    assert_eq!(
        db.empty_bins(),
        vec!["d1.c1[2]".to_string(), "d1.c1[3]".to_string()],
        "bins 2 and 3 are still not-exercised after the first sweep"
    );

    // Second run: fills bin 2, leaving only bin 3 empty — proving the
    // second run's report merges into the same database rather than
    // starting a fresh one.
    let (p, c) = circuit_at(2.0).coverage_reports();
    db.merge_run(&p, &c);
    assert_eq!(db.empty_bins(), vec!["d1.c1[3]".to_string()], "the second run's hit merges in, narrowing the gap");

    let entry = db.points().get("d1.c1").unwrap();
    assert_eq!(entry.counts, vec![1, 1, 1, 0], "bins 0/1/2 each hit once across the two runs, bin 3 untouched");
}

/// DVT-43 edge case: a run with zero accepted points (a module compiled
/// but never solved into an accepted state that reaches `evaluate_
/// constraints`) reports every bin as not-exercised — never a vacuous
/// 100%. Modeled here by merging an explicit all-zero report (the shape a
/// zero-accepted-point run's `coverage_reports()` would actually produce
/// — the tracker starts at all-zero and only advances on an accepted
/// evaluation point).
#[test]
fn a_run_with_zero_accepted_points_reports_every_bin_as_not_exercised() {
    let mut db = CoverageDatabase::new();
    let zero_run = vec![piperine_solver::abi::ReportedCoverPoint {
        label: "c1".to_string(),
        counts: vec![0, 0, 0, 0],
        instance_path: "d1".to_string(),
    }];
    db.merge_run(&zero_run, &[]);

    assert_eq!(
        db.empty_bins(),
        vec!["d1.c1[0]".to_string(), "d1.c1[1]".to_string(), "d1.c1[2]".to_string(), "d1.c1[3]".to_string()],
        "zero accepted points: every declared bin is named not-exercised, never a vacuous pass"
    );
    let entry = db.points().get("d1.c1").unwrap();
    assert!(!entry.fully_covered(), "an all-zero entry is never reported as fully covered");
}

/// DVT-43: coverage persists across runs — unlike a margin (scoped to one
/// analysis, reset every run, DVC-18), the database a caller keeps
/// outlives any single `Session`/`CircuitInstance`, accumulating whatever
/// is merged into it for as long as the caller holds it (a whole
/// regression, not one analysis).
#[test]
fn coverage_persists_across_independently_compiled_sessions() {
    let mut db = CoverageDatabase::new();
    // Two *independently compiled* sessions (a fresh Session::builder
    // each time, as a regression's separate test runs would do) still
    // merge into the same long-lived database.
    let (p1, c1) = circuit_at(0.4).coverage_reports();
    db.merge_run(&p1, &c1);
    drop(p1);
    drop(c1);

    let (p2, c2) = circuit_at(1.4).coverage_reports();
    db.merge_run(&p2, &c2);

    let entry = db.points().get("d1.c1").unwrap();
    assert_eq!(entry.counts, vec![1, 1, 0, 0], "both independently-compiled runs' hits accumulate");
}
