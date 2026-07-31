//! dv-gradients T13 (DVG-16) — `SamplePlan`: walk the authored instance
//! tree, classify each `tol`-carrying param as process (`global`) or
//! mismatch, and produce draws where a `global` parameter gets ONE draw
//! shared by every instance of the same `(module, param)` group while a
//! plain (mismatch) parameter gets an INDEPENDENT draw per instance.
//!
//! The walk reads the authored POM (`design.module`) — never
//! `design.flat_module` (MD-25, the UNBREAKABLE rule). The flat form is a
//! codegen concern the sampler must never read; the mismatch/process
//! distinction is a structural question that lives on the authored
//! declarations.

use piperine_api::{SamplePlan, SampleScope, Session, SolverConfig};
use piperine_api::prelude::CheckPosture;
use piperine_lang::SourceMap;

/// A two-instance fixture with BOTH a `global` (process) and a mismatch
/// `tol` on the same module — the structural test for the global vs
/// mismatch distinction in one run. Two `R` instances share their `r_g`
/// draw (process) and take independent `r_m` draws (mismatch).
const TWO_INSTANCE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) {
    param r_g : Real = 1e3 tol gauss(sigma = 10.0) global;
    param r_m : Real = 1e3 tol gauss(sigma = 10.0);
}
analog R { I(p, n) <+ V(p, n) / (r_g + r_m); }

mod Top() {
    wire gnd  : Electrical;
    wire in   : Electrical;
    wire out  : Electrical;
    v1  : Vsrc(.p = in,  .n = gnd);
    r1  : R   (.p = in,  .n = out);
    r2  : R   (.p = out, .n = gnd);
}
";

/// A hierarchical fixture: `Top → Mid → Leaf`, two leaves per mid, two
/// mids — proves the authored-tree walk descends through sub-instances
/// and produces path-prefixed labels (`m1.l1`, `m1.l2`, …). A flat-walk
/// would miss the path prefix; the assertions catch that.
const HIERARCHICAL_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Leaf(inout p: Electrical, inout n: Electrical) {
    param r : Real = 1e3 tol gauss(sigma = 5.0) global;
}
analog Leaf { I(p, n) <+ V(p, n) / r; }

mod Mid(inout p: Electrical, inout n: Electrical) {
    wire mid : Electrical;
    l1 : Leaf(.p = p,   .n = mid);
    l2 : Leaf(.p = mid, .n = n);
}

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    m1 : Mid(.p = in,  .n = out);
    m2 : Mid(.p = out, .n = gnd);
}
";

fn design(phdl: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(phdl, &SourceMap::dummy()).expect("elaborates")
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
    let plan = SamplePlan::collect(&design(TWO_INSTANCE_PHDL), "Top").expect("plan collects");

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
    let plan = SamplePlan::collect(&design(TWO_INSTANCE_PHDL), "Top").expect("plan collects");

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
    let plan = SamplePlan::collect(&design(TWO_INSTANCE_PHDL), "Top").expect("plan collects");
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
    let design = design(TWO_INSTANCE_PHDL);
    let mut session = Session::compile(&design, "Top").expect("session compiles");

    // Nominal operating point: V(out) = 0.5 V.
    let nominal = session.op(&collect_config(), None).expect("nominal op solves");
    let nominal_out = nominal.v("out").expect("v(out)");
    assert!(
        (nominal_out - 0.5).abs() < 1e-9,
        "nominal divider V(out) = 0.5 V, got {nominal_out}"
    );

    // Apply sample (seed, index) = (42, 0).
    let plan = SamplePlan::collect(&session.design(), "Top").expect("plan collects");
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
    let plan = SamplePlan::collect(&design(HIERARCHICAL_PHDL), "Top").expect("plan collects");

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
