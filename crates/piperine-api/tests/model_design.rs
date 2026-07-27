//! Scope: `piperine_api::model::Design` (CLA-17) — the navigation root: load,
//! `top` (including the inference rule), `module`, `modules`, `const_`, and
//! `select` with its two fail-loud paths. Plus the MD-25 proof: the hierarchy
//! the model exposes is the **authored** one, and codegen's flattened side
//! artifact is unreachable through it.

use piperine_api::model::Design;
use piperine_api::Error;
use piperine_lang::Value;

/// Three levels — `Top` → `Mid` → `Leaf` — so the authored hierarchy and the
/// flattened side artifact genuinely differ: flattening splices `Mid`'s two
/// leaf instances into `Top`, which authored exactly one instance.
const HIER: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

const VDD: Real = 3.3;

mod Leaf(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog Leaf { I(p, n) <+ V(p, n) / r; }

mod Mid(inout a: Electrical, inout b: Electrical) {
    wire tap : Electrical;
    la : Leaf(.p = a, .n = tap) { .r = 100.0 };
    lb : Leaf(.p = tap, .n = b) { .r = 200.0 };
}

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    m1 : Mid(.a = vin, .b = gnd) {};
}
";

/// A design with two uninstantiated modules — no unambiguous root, so `top`
/// stays unset (the inference rule's negative case).
const AMBIGUOUS_TOP: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod A(inout p: Electrical) {}
mod B(inout p: Electrical) {}
";

#[test]
fn load_str_elaborates_and_infers_the_unique_top() {
    let design = Design::load_str(HIER).expect("HIER elaborates");
    let top = design.top().expect("a unique uninstantiated module is inferred as top");
    assert_eq!(top.name().expect("top resolves"), "Top", "`Top` is the only module nothing instantiates");
}

#[test]
fn top_is_unset_when_the_root_is_ambiguous() {
    let design = Design::load_str(AMBIGUOUS_TOP).expect("fixture elaborates");
    assert!(design.top().is_none(), "two candidate roots must leave the top unset, not guess one");
}

#[test]
fn load_str_surfaces_an_elaboration_failure_loudly() {
    let err = Design::load_str("mod Broken( {").expect_err("garbage must not elaborate");
    assert!(
        matches!(err, Error::Model(_)),
        "a parse/elaboration diagnostic surfaces as Error::Model, got {err:?}"
    );
}

#[test]
fn load_surfaces_an_unreadable_path_loudly() {
    let err = Design::load("/nonexistent/piperine/model_design_fixture.phdl")
        .expect_err("a missing file must fail loud");
    let msg = format!("{err}");
    assert!(
        msg.contains("failed to read `/nonexistent/piperine/model_design_fixture.phdl`"),
        "the diagnostic names the unreadable path, got {msg}"
    );
}

#[test]
fn module_resolves_by_name_and_fails_loud_when_absent() {
    let design = Design::load_str(HIER).expect("HIER elaborates");
    assert_eq!(design.module("Mid").expect("Mid present").name().expect("resolves"), "Mid");

    let err = design.module("Nope").expect_err("an unknown module must fail loud");
    assert_eq!(format!("{err}"), "module `Nope` not found", "the diagnostic names the module");
}

#[test]
fn modules_enumerates_every_authored_module() {
    let design = Design::load_str(HIER).expect("HIER elaborates");
    let mut names: Vec<String> =
        design.modules().iter().map(|m| m.name().expect("resolves").to_string()).collect();
    names.sort();
    assert_eq!(names, vec!["Leaf", "Mid", "Top"], "all three authored modules, no more and no fewer");
}

#[test]
fn const_reads_a_global_and_reports_none_for_an_unknown_name() {
    let design = Design::load_str(HIER).expect("HIER elaborates");
    assert_eq!(design.const_("VDD"), Some(Value::Real(3.3)), "the authored constant's folded value");
    assert_eq!(design.const_("NOT_A_CONST"), None, "an unknown name reads None, not an error");
}

#[test]
fn select_resolves_a_path_to_typed_nodes() {
    let design = Design::load_str(HIER).expect("HIER elaborates");
    let selection = design.select("/m1").expect("`/m1` resolves against the inferred top");
    assert_eq!(selection.len(), 1, "one matched node");
    assert!(!selection.is_empty());
    assert_eq!(selection.nodes()[0].kind(), "instance", "the node's kind discriminator");
    assert_eq!(selection.nodes()[0].name(), "m1", "the matched instance label");
}

#[test]
fn select_fails_loud_on_a_path_that_matches_nothing() {
    let design = Design::load_str(HIER).expect("HIER elaborates");
    let err = design.select("/no_such_instance").expect_err("zero matches must fail loud");
    assert!(
        matches!(err, Error::NotFound(_)),
        "zero matches is Error::NotFound so a host can map it to a lookup failure, got {err:?}"
    );
    assert_eq!(format!("{err}"), "selector `/no_such_instance` resolved to no nodes");
}

#[test]
fn select_fails_loud_on_a_malformed_path() {
    let design = Design::load_str(HIER).expect("HIER elaborates");
    let err = design.select("///").expect_err("a malformed selector must fail loud");
    assert!(
        matches!(err, Error::Model(_)),
        "a malformed path is Error::Model, distinct from a zero-match NotFound, got {err:?}"
    );
}

/// MD-25 (spec AC6 / the feature's hard constraint): what the model exposes is
/// the authored hierarchy. The fixture's flattened form differs measurably —
/// `Top` gains `Mid`'s two leaf instances — and the model reports the authored
/// single instance instead, at every level. There is no accessor on `Design` or
/// `Module` that reaches the flat map, so an attempt to read it does not
/// compile.
#[test]
fn the_model_exposes_the_authored_hierarchy_never_the_flattened_form() {
    let design = Design::load_str(HIER).expect("HIER elaborates");

    // The discriminator is real: elaboration DID record a flattened `Top` and
    // it is not the authored one.
    let flat_top = design.pom().flat_module("Top").expect("the flatten pass recorded Top");
    assert_eq!(
        flat_top.instances().len(),
        2,
        "the flattened Top splices Mid's two leaves — this is what the model must NOT show"
    );

    // What the model shows: Top's one authored instance, descending into Mid.
    let top = design.module("Top").expect("Top present");
    let top_instances = top.instances().expect("Top's instances");
    assert_eq!(top_instances.len(), 1, "the authored Top instantiates exactly one submodule");
    assert_eq!(top_instances[0].name(), "m1");
    assert_eq!(top_instances[0].module(), "Mid", "the instance names its submodule, uncollapsed");

    // ...and the tree stays walkable one level further down.
    let mid = design.module(top_instances[0].module()).expect("Mid reachable from the instance");
    let mid_instances = mid.instances().expect("Mid's instances");
    let mut leaves: Vec<&str> = mid_instances.iter().map(|i| i.name()).collect();
    leaves.sort();
    assert_eq!(leaves, vec!["la", "lb"], "Mid's own instances, at Mid — not lifted into Top");
    assert!(
        mid_instances.iter().all(|i| i.module() == "Leaf"),
        "each sub-instance still names the module it instantiates"
    );
}
