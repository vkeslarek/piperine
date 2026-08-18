//! dv-gradients T13 (DVG-16) — `SamplePlan`: walk the authored instance
//! tree, classify each `tol`-carrying param as process (`global`) or
//! mismatch, and produce draws where a `global` parameter gets ONE draw
//! shared by every instance of the same `(module, param)` group while a
//! plain (mismatch) parameter gets an INDEPENDENT draw per instance.
//!
//! Also covers T14 (DVG-17): `(seed, index)` reproducibility across
//! independent constructions, a named failing sample replayed as an
//! ordinary session reproducing its failure exactly, and a nominal
//! analysis after sampling returning to declared values via
//! `SamplePlan::reset` (no residue).
//!
//! The walk reads the authored POM (`design.module`) — never
//! `design.flat_module` (MD-25, the UNBREAKABLE rule). The flat form is a
//! codegen concern the sampler must never read; the mismatch/process
//! distinction is a structural question that lives on the authored
//! declarations.
//!
//! **Migrated** (random-variable-catalogue): the fixtures run against the
//! live VAMS `tol gauss(…)`/`tol uniform(…)` parameter-distribution
//! surface — `tol`/`global` are contextual keywords and the keyed args
//! (`sigma`, `sigma_rel`, `rel`) are const-evaluated in module scope.

use piperine_api::{SamplePlan, SampleScope, Session, SolverConfig};
use piperine_api::prelude::CheckPosture;
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// A two-instance fixture with BOTH a `global` (process) and a mismatch
/// `tol` on the same module — the structural test for the global vs
/// mismatch distinction in one run. Two `R` instances share their `r_g`
/// draw (process) and take independent `r_m` draws (mismatch).
const TWO_INSTANCE: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r_g = 1e3 tol gauss(sigma = 10.0) global;
    parameter real r_m = 1e3 tol gauss(sigma = 10.0);
    analog begin
        I(p, n) <+ V(p, n) / (r_g + r_m);
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    R r2 (.p(out), .n(gnd));
endmodule
"#;

/// A hierarchical fixture: `Top → Mid → Leaf`, two leaves per mid, two
/// mids — proves the authored-tree walk descends through sub-instances
/// and produces path-prefixed labels (`m1.l1`, `m1.l2`, …). A flat-walk
/// would miss the path prefix; the assertions catch that.
const HIERARCHICAL: &str = r#"`include "disciplines.vams"
module Leaf (inout electrical p, inout electrical n);
    parameter real r = 1e3 tol gauss(sigma = 5.0) global;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Mid (inout electrical p, inout electrical n);
    electrical mid;
    Leaf l1 (.p(p), .n(mid));
    Leaf l2 (.p(mid), .n(n));
endmodule
module Top ();
    electrical gnd, in, out;
    Mid m1 (.p(in), .n(out));
    Mid m2 (.p(out), .n(gnd));
endmodule
"#;

fn design(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

// ── DVG-16: a `global` param gets ONE draw shared by every instance ────────
//
// The two R instances both read `r_g`. The walk collects two entries
// (r1.r_g, r2.r_g); both belong to the same `(R, r_g)` global group, so
// `draws` returns ONE draw for the group and writes the same value to
// both labels. Verifying the value is equal is the structural
// discrimination — a per-instance draw would (almost surely) differ.
#[test]
fn a_global_parameter_gets_one_draw_shared_across_instances() {
    let plan = SamplePlan::collect(&design(TWO_INSTANCE), "Top").expect("plan collects");

    let global_entries: Vec<_> = plan
        .entries()
        .iter()
        .filter(|e| e.param == "r_g")
        .collect();
    assert_eq!(global_entries.len(), 2, "two R instances, two `r_g` entries");
    assert!(
        global_entries.iter().all(|e| e.scope() == SampleScope::Global),
        "every `r_g` entry is process scope"
    );

    let draws = plan.draws(7, 0);
    let r1_rg = draws
        .iter()
        .find(|d| d.label == "r1" && d.param == "r_g")
        .expect("r1.r_g draw");
    let r2_rg = draws
        .iter()
        .find(|d| d.label == "r2" && d.param == "r_g")
        .expect("r2.r_g draw");
    assert_eq!(
        r1_rg.value, r2_rg.value,
        "global draw is shared: r1.r_g = {} should equal r2.r_g = {}",
        r1_rg.value, r2_rg.value,
    );
}

// ── DVG-16: a mismatch param gets an INDEPENDENT draw per instance ─────────
//
// The two R instances both read `r_m`. Both entries are mismatch scope;
// `draws` returns ONE draw per instance, and they (almost surely) differ.
#[test]
fn a_mismatch_parameter_gets_independent_draws_per_instance() {
    let plan = SamplePlan::collect(&design(TWO_INSTANCE), "Top").expect("plan collects");

    let mismatch_entries: Vec<_> = plan
        .entries()
        .iter()
        .filter(|e| e.param == "r_m")
        .collect();
    assert_eq!(mismatch_entries.len(), 2, "two R instances, two `r_m` entries");
    assert!(
        mismatch_entries.iter().all(|e| e.scope() == SampleScope::Mismatch),
        "every `r_m` entry is mismatch scope"
    );

    let draws = plan.draws(7, 0);
    let r1_rm = draws
        .iter()
        .find(|d| d.label == "r1" && d.param == "r_m")
        .expect("r1.r_m draw");
    let r2_rm = draws
        .iter()
        .find(|d| d.label == "r2" && d.param == "r_m")
        .expect("r2.r_m draw");
    assert_ne!(
        r1_rm.value, r2_rm.value,
        "mismatch draws are independent: r1.r_m = {} should differ from r2.r_m = {}",
        r1_rm.value, r2_rm.value,
    );
}

// ── DVG-16: both behaviors in one run, on one fixture ──────────────────────
//
// The single fixture carries one `global` and one mismatch `tol` on the
// same module, instantiated twice. The plan's draws list contains both
// behaviors in one pass: a shared `r_g` and independent `r_m`s. This is
// the criterion test — both shapes in one run.
#[test]
fn both_behaviors_in_one_run() {
    let plan = SamplePlan::collect(&design(TWO_INSTANCE), "Top").expect("plan collects");
    let draws = plan.draws(11, 3);

    // Four draws total: 2 global (r_g on r1, r2) + 2 mismatch (r_m on r1, r2).
    assert_eq!(draws.len(), 4, "one draw per (label, param) entry");

    // Global pair shares a value.
    let r1_rg = draws.iter().find(|d| d.label == "r1" && d.param == "r_g").expect("r1.r_g");
    let r2_rg = draws.iter().find(|d| d.label == "r2" && d.param == "r_g").expect("r2.r_g");
    assert_eq!(r1_rg.value, r2_rg.value, "global draws agree");

    // Mismatch pair differs.
    let r1_rm = draws.iter().find(|d| d.label == "r1" && d.param == "r_m").expect("r1.r_m");
    let r2_rm = draws.iter().find(|d| d.label == "r2" && d.param == "r_m").expect("r2.r_m");
    assert_ne!(r1_rm.value, r2_rm.value, "mismatch draws differ");

    // The global draw's value is distinct from the mismatch draws' values
    // — no stream collision between the two scopes.
    assert_ne!(
        r1_rg.value, r1_rm.value,
        "the global stream and the mismatch stream are decorrelated"
    );
}

// ── DVG-16: draws are written through the ordinary restamp path ────────────
//
// `plan.apply(session, seed, index)` writes each draw through
// `Session::set` — the same restamp path a host `set` uses. Prove it by
// solving the operating point before and after applying a sample, and
// checking the operating point moved by the amount the draws shift the
// resistor ratio.
//
// Nominal `r_g + r_m = 2 kΩ` per resistor ⇒ `V(out) = 0.5 V`. A sample
// shifts both `r1`'s total and `r2`'s total; the new `V(out)` reflects
// the new ratio.
#[test]
fn draws_apply_through_the_ordinary_restamp_path() {
    let design = design(TWO_INSTANCE);
    let mut session = Session::compile(&design, "Top").expect("session compiles");

    // Nominal operating point: V(out) = 0.5 V.
    let nominal = session.op(&collect_config(), None).expect("nominal op solves");
    let nominal_out = nominal.v("out").expect("v(out)");
    assert!(
        (nominal_out - 0.5).abs() < 1e-9,
        "nominal divider V(out) = 0.5 V, got {nominal_out}"
    );

    // Apply sample (seed, index) = (42, 0).
    let plan = SamplePlan::collect(session.design(), "Top").expect("plan collects");
    plan.apply(&mut session, 42, 0).expect("draws apply through Session::set");

    // The operating point now reflects the drawn resistor values — it
    // moved off 0.5 V. The exact value is a function of the draws; the
    // proof that the restamp path was used is that the op moved AT ALL
    // and that re-applying the same (seed, index) reproduces the same op.
    let sampled = session.op(&collect_config(), None).expect("sampled op solves");
    let sampled_out = sampled.v("out").expect("v(out)");
    assert_ne!(
        nominal_out.to_bits(),
        sampled_out.to_bits(),
        "the sampled op must differ from the nominal — restamp applied the draws"
    );

    // Replay: a fresh session, same (seed, index), reproduces the op
    // bit-identical — proving the apply path is deterministic and the
    // draws route through `Session::set` (no hidden state).
    let mut replay = Session::compile(&design, "Top").expect("replay compiles");
    plan.apply(&mut replay, 42, 0).expect("replay applies");
    let replay_out = replay.op(&collect_config(), None).expect("replay op solves").v("out").expect("v(out)");
    assert_eq!(
        sampled_out.to_bits(),
        replay_out.to_bits(),
        "same (seed, index) on a fresh session reproduces the operating point"
    );
}

// ── DVG-16 / MD-25: the walk reads the AUTHORED tree, descending ──────────
//
// A hierarchical `Top → Mid → Leaf` design, two leaves per mid, two mids.
// The walk produces four entries with PATH-PREFIXED labels (`m1.l1`,
// `m1.l2`, `m2.l1`, `m2.l2`) — matching the labels the flattener emits
// and the host's `Session::set` accepts. A walk that only read the top
// module's direct instances would find zero entries (Mid has no `tol`);
// a walk that read `flat_module` would also find four, but the
// sampler's source never calls `flat_module` — the rule is structural.
//
// Discrimination: the entries' labels are path-prefixed (`m1.l1`), not
// bare leaf labels (`l1`). If the walker collapsed the hierarchy, the
// labels would collide (`l1` appears twice) and the assertion fails.
#[test]
fn the_walk_descends_the_authored_instance_tree() {
    let plan = SamplePlan::collect(&design(HIERARCHICAL), "Top").expect("plan collects");

    let labels: Vec<&str> = plan.entries().iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["m1.l1", "m1.l2", "m2.l1", "m2.l2"],
        "the authored-tree walk produces path-prefixed labels in declaration order"
    );

    // All four leaves are the same module (`Leaf`) and param (`r`); since
    // `r` is `global`, every leaf shares ONE draw — even across mids.
    // Proves the global group is keyed by (module, param), not by parent.
    let draws = plan.draws(7, 0);
    let values: Vec<f64> = draws.iter().map(|d| d.value).collect();
    let (first, rest) = values.split_first().expect("non-empty");
    assert!(
        rest.iter().all(|v| v == first),
        "global draw is shared across every Leaf instance (m1.l1, m1.l2, m2.l1, m2.l2): {values:?}"
    );
}

// ── T14 (DVG-17): (seed, index) reproducibility and replay ────────────────

/// A resistive divider with a `require` and a mismatch `tol` on `r1`:
/// nominal `V(out) = dc·1e3/(r1+1e3) = 0.5 V` (dc=1, r1=r=1e3), and
/// `require head : V(out) <= hi` (hi=0.55) is violated whenever `r1`
/// draws low enough to push `V(out)` above 0.55 — giving a fixture with
/// both a passing nominal and reachable failing samples.
const DIVIDER_TOL: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3 tol gauss(sigma_rel = 0.3);
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.55;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

/// `(seed, index)` fully determines a draw: two `SamplePlan`s collected
/// from two INDEPENDENT parses of the same design (simulating "a
/// different run/process" — no shared state beyond the design text)
/// produce byte-identical draws for the same `(seed, index)`.
#[test]
fn seed_index_produce_byte_identical_draws_across_independent_constructions() {
    let plan_a = SamplePlan::collect(&design(TWO_INSTANCE), "Top").expect("plan a collects");
    let plan_b = SamplePlan::collect(&design(TWO_INSTANCE), "Top").expect("plan b collects");

    let draws_a = plan_a.draws(123, 7);
    let draws_b = plan_b.draws(123, 7);
    assert_eq!(draws_a.len(), draws_b.len(), "same entry count");
    for (a, b) in draws_a.iter().zip(&draws_b) {
        assert_eq!(a.label, b.label);
        assert_eq!(a.param, b.param);
        assert_eq!(
            a.value.to_bits(),
            b.value.to_bits(),
            "byte-identical draw for {}.{} from independent plan constructions",
            a.label,
            a.param
        );
    }
}

/// A named failing sample — found by scanning indices for one whose
/// applied draws violate `head` — replays as an ordinary session (a
/// freshly compiled `Session`, a freshly collected `SamplePlan`, the
/// same `(seed, index)`) and reproduces the exact same failure: the
/// same `requires_ok() == false`, the same `V(out)`, and the same
/// margin value, bit for bit.
#[test]
fn a_named_failing_sample_replays_as_an_ordinary_session_and_reproduces_the_failure() {
    let divider_design = design(DIVIDER_TOL);
    let config = collect_config();
    let seed = 99u64;

    // Nominal sanity: the require is satisfied at the declared values.
    let mut nominal_session = Session::compile(&divider_design, "Top").expect("nominal compiles");
    let nominal_op = nominal_session.op(&config, None).expect("nominal op solves");
    assert!(nominal_op.requires_ok(), "nominal design satisfies `head`");

    // Scan for a failing (seed, index) — deterministic for this seed.
    let plan = SamplePlan::collect(&divider_design, "Top").expect("plan collects");
    let mut found: Option<(usize, f64, f64)> = None;
    for index in 0..200usize {
        let mut session = Session::compile(&divider_design, "Top").expect("session compiles");
        plan.apply(&mut session, seed, index).expect("draws apply");
        let op = session.op(&config, None).expect("op solves");
        if !op.requires_ok() {
            let out_v = op.v("out").expect("v(out)");
            let margin_v = op.margins().get("head").expect("head margin").scalar();
            found = Some((index, out_v, margin_v));
            break;
        }
    }
    let (index, out_v, margin_v) =
        found.expect("a failing sample exists for seed 99 within 200 draws");

    // Replay: an independent session and an independently collected plan,
    // driven by ONLY (seed, index) — no other shared state.
    let mut replay = Session::compile(&divider_design, "Top").expect("replay compiles");
    let replay_plan =
        SamplePlan::collect(&design(DIVIDER_TOL), "Top").expect("replay plan collects");
    replay_plan.apply(&mut replay, seed, index).expect("replay applies");
    let replay_op = replay.op(&config, None).expect("replay op solves");

    assert!(
        !replay_op.requires_ok(),
        "the replayed sample must reproduce the exact failure (seed={seed}, index={index})"
    );
    assert_eq!(
        replay_op.v("out").expect("v(out)").to_bits(),
        out_v.to_bits(),
        "byte-identical V(out) on replay"
    );
    assert_eq!(
        replay_op.margins().get("head").expect("head margin").scalar(),
        margin_v,
        "byte-identical margin value on replay"
    );
}

/// After a sample is applied, `SamplePlan::reset` returns the session to
/// its declared nominal values — a nominal analysis afterwards is
/// byte-identical to a session that was never sampled (DVG-18 AC5, "no
/// residue").
#[test]
fn a_nominal_analysis_after_sampling_returns_to_declared_values() {
    let two_instance_design = design(TWO_INSTANCE);
    let config = collect_config();

    let mut session = Session::compile(&two_instance_design, "Top").expect("session compiles");
    let nominal_before = session.op(&config, None).expect("nominal op solves").v("out").expect("v(out)");

    let plan = SamplePlan::collect(&two_instance_design, "Top").expect("plan collects");
    plan.apply(&mut session, 55, 2).expect("sample applies");
    let sampled = session.op(&config, None).expect("sampled op solves").v("out").expect("v(out)");
    assert_ne!(
        sampled.to_bits(),
        nominal_before.to_bits(),
        "applying a sample must perturb the operating point"
    );

    plan.reset(&mut session).expect("reset applies nominal values");
    let nominal_after = session.op(&config, None).expect("post-reset op solves").v("out").expect("v(out)");
    assert_eq!(
        nominal_after.to_bits(),
        nominal_before.to_bits(),
        "reset returns to the declared nominal — sampling leaves no residue"
    );
}
