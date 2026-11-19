//! The IR each `analog-structure` form lowers to, and its refusals: contributions under control
//! flow and the kind they give a branch (AS-01, AS-02), then, as the feature's tasks land,
//! key params and body variants, derivative sites and guarded sites. Every source is a few lines
//! of MSL elaborated over the real standard library; every expectation is the spec rule cited
//! at its assertion (`docs/spec/part_iv_behavior.md` §11).

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{BranchKind, Ir, QuantityRole};
use support::Elab;

/// A source of one device `M(p, n, in en: Bit)` with `analog` as its `analog` block, placed in `Top`.
struct Device;

impl Device {
    /// The source text of the device and its top.
    fn source(analog: &str) -> String {
        format!("module M(p: Electrical, n: Electrical, in en: Bit) {{ k: f64 = 1.0; }}\nanalog for M {{ {analog} }}\nmodule Top() {{ signal s: Electrical; signal e: Bit; let m = M {{ }}(s, gnd, e); }}")
    }

    /// The IR of `analog`, which must elaborate.
    fn ir(analog: &str) -> Ir {
        Elab::ir(&Device::source(analog), "Top")
    }

    /// The kinds of the branches of the body `M`, in branch order.
    fn kinds(ir: &Ir) -> Vec<BranchKind> {
        let body = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        body.branches().iter().map(|b| b.kind).collect()
    }
}

#[test]
fn a_contribution_in_a_nested_block_lowers_as_at_the_top() {
    // AS-01 AC 3: a contribution in a plain block nested in the `analog` block lowers as if it were
    // written at the block's top level: the same branch, quantity and addends, no enclosing condition.
    let top = Device::ir("i(p, n) +> v(p, n) / 1k;");
    for nested in ["{ i(p, n) +> v(p, n) / 1k; }", "{ { i(p, n) +> v(p, n) / 1k; } }", "let x = { i(p, n) +> v(p, n) / 1k; 2.0 };"] {
        let ir = Device::ir(nested);
        let (want, got) = (top.bodies().find(|b| b.name() == "M").expect("M").contributions(), ir.bodies().find(|b| b.name() == "M").expect("M").contributions());
        assert_eq!(got.len(), 1, "{nested}");
        assert_eq!((got[0].branch, got[0].quantity, got[0].addends.len()), (want[0].branch, want[0].quantity, want[0].addends.len()), "{nested}");
        assert!(got[0].conditions.is_empty(), "{nested}: a plain block is no condition");
        assert_eq!(Device::kinds(&ir), [BranchKind::Flow], "{nested}");
    }
}

#[test]
fn a_contribution_in_a_closure_is_refused_at_parse() {
    // AS-01 AC 6 (Appendix B.2 rule 3): a contribution in the body of a closure stays refused at parse,
    // before elaboration: the design carries the diagnostic.
    let design = Elab::design(&Device::source("let f = |x: f64| { i(p, n) +> x; };"));
    assert!(!design.diagnostics().is_empty(), "the front end refuses a contribution in a closure");
}

#[test]
fn a_selector_on_a_carrier_or_the_environment_keeps_the_flow_unknown() {
    // AS-02 AC 6 (§11 rule 3): a branch whose potential contribution a discrete carrier or the
    // environment of the solve selects keeps its flow unknown in every solve: it is a switch
    // branch (its private flow pin and equation row stay for the life of the instance). A
    // potential contribution under no condition is a potential branch; a flow-only branch has no
    // unknown whatever selects it.
    for (selector, kinds) in [
        ("if en.known_or(false) { v(p, n) +> 1.0; }", [BranchKind::Switch]),
        ("if now() > 1u { v(p, n) +> 1.0; }", [BranchKind::Switch]),
        ("if analysis(Static) { v(p, n) +> 1.0; } else { i(p, n) +> 2.0; }", [BranchKind::Switch]),
        ("v(p, n) +> 1.0;", [BranchKind::Potential]),
        ("if en.known_or(false) { i(p, n) +> 1.0; }", [BranchKind::Flow]),
    ] {
        let ir = Device::ir(selector);
        assert_eq!(Device::kinds(&ir), kinds, "{selector}");
    }
    // The contributions keep their place: under the arm of the condition, not at the top.
    let ir = Device::ir("if en.known_or(false) { v(p, n) +> 1.0; } else { i(p, n) +> 2.0; }");
    let body = ir.bodies().find(|b| b.name() == "M").expect("M");
    let found: Vec<(QuantityRole, bool)> = body.contributions().iter().map(|c| (c.quantity, c.conditions[0].holds)).collect();
    assert_eq!(found, [(QuantityRole::Potential, true), (QuantityRole::Flow, false)]);
}

#[test]
fn a_contribution_to_a_branch_from_a_node_to_itself_under_an_if_is_e0241() {
    // §11 rule 10: the two nodes differ, under an `if` as at the top.
    let src = Device::source("if v(p) > 0.0 { i(p, p) +> 1.0; }");
    let err = Elab::run(&src, "Top").expect_err("a branch (p, p)");
    assert_eq!(Elab::codes(&err), ["E0241"]);
    let span = err[0].diagnostic.primary.0;
    assert_eq!(&src[span.start..span.end], "i(p, p)");
}

/// What elaboration says of `analog` as the body of `Device`: the codes, and the text the first
/// diagnostic's span covers.
fn refusal(analog: &str) -> (Vec<String>, String, String) {
    let src = Device::source(analog);
    let err = Elab::run(&src, "Top").expect_err("the selector rule refuses it");
    let span = err[0].diagnostic.primary.0;
    (Elab::codes(&err), src[span.start..span.end].to_string(), err[0].diagnostic.message.clone())
}

#[test]
fn a_probe_in_the_selector_of_a_potential_capable_branch_is_e0414() {
    // AS-02 AC 1 (§11 rule 3): a probe selecting a contribution to a branch that has a potential
    // contribution is E0414, naming the branch and the probe, at the selected contribution: on the
    // potential side, on the flow side of the same branch (the G.58 resolution), and through a
    // local the condition reads.
    for analog in [
        "if v(p) > 0.5 { v(p, n) +> 1.0; }",
        "v(p, n) +> 1.0; if v(p) > 0.5 { i(p, n) +> 1.0; }",
        "let hot = v(p) > 0.5; if hot { v(p, n) +> 1.0; }",
        "if en.known_or(false) { v(p, n) +> 1.0; } else if v(p) > 0.5 { i(p, n) +> 1.0; }",
    ] {
        let (codes, at, message) = refusal(analog);
        assert_eq!(codes, ["E0414"], "{analog}");
        assert!(at.trim_end_matches(';').ends_with("+> 1.0"), "{analog}: at the contribution, got `{at}`");
        assert!(message.contains("(p, n)") && message.contains("the probe of (p, gnd)"), "{analog}: {message}");
    }
}

#[test]
fn a_probe_selecting_between_flow_contributions_is_accepted() {
    // §11 rule 3 binds a branch that has a potential contribution: a probe may choose between the
    // flow contributions of a branch that has none, and between the expressions of a potential
    // contribution (§11 rule 5: an `if` on a probe chooses between expressions, never kinds).
    for analog in [
        "if v(p) > 0.5 { i(p, n) +> v(p, n) / 1k; } else { i(p, n) +> v(p, n) / 2k; }",
        "v(p, n) +> if v(p) > 0.5 { 1.0 } else { 2.0 };",
    ] {
        let ir = Device::ir(analog);
        assert!(Device::kinds(&ir).len() == 1, "{analog}");
    }
}

/// A switch `Sw(a, b)` whose kind its param `short` fixes: the params-only selector of §11 rule 3.
const SWITCH: &str = "module Sw(a: Electrical, b: Electrical) { short: f64 = 1.0; }\nanalog for Sw { if short > 0.5 { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; } }\n";

/// The names of the bodies of `ir`, in id order.
fn body_names(ir: &Ir) -> Vec<String> {
    ir.bodies().map(|b| b.name().to_string()).collect()
}

/// The body of `ir` named `name`.
fn body<'a>(ir: &'a Ir, name: &str) -> piperine_ir::Body<'a> {
    ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body {name} among {:?}", body_names(ir)))
}

#[test]
fn params_only_selectors_fix_the_kind_and_mark_the_params_structural() {
    // AS-02 AC 3 (§11 rule 3, §8.1.2): every selector depends on `short` only, so the kind is fixed
    // at elaboration (`short` = 1 selects the potential arm: `Potential`, `short` = 0 the flow arm:
    // `Flow`), `short` is structural, and the body is the variant of its value.
    for (short, kind) in [(1.0, BranchKind::Potential), (0.0, BranchKind::Flow)] {
        let src = format!("{SWITCH}module Top() {{ signal s: Electrical; let m = Sw {{ short: {short:?} }}(s, gnd); }}");
        let ir = Elab::ir(&src, "Top");
        let variant = ir.bodies().find(|b| b.name().starts_with("Sw~short=")).expect("a body of the variant");
        assert_eq!(variant.branches().iter().map(|b| b.kind).collect::<Vec<_>>(), [kind], "short = {short}");
        assert!(variant.params()[0].structural, "`short` is structural");
        assert_eq!(variant.contributions().len(), 1, "only the selected arm is lowered");
        assert!(variant.contributions()[0].conditions.is_empty(), "the folded condition is no condition");
    }
}

#[test]
fn two_instances_with_different_key_values_get_two_bodies() {
    // One body per distinct value of the key params: two values, two bodies, in request order after
    // every unkeyed body; the same value twice shares one body.
    let src = format!("{SWITCH}module Top() {{ signal s: Electrical; signal t: Electrical; signal u: Electrical;\n let a = Sw {{ short: 1.0 }}(s, gnd); let b = Sw {{ short: 0.0 }}(t, gnd); let c = Sw {{ short: 1.0 }}(u, gnd); }}");
    let ir = Elab::ir(&src, "Top");
    let names = body_names(&ir);
    assert_eq!(names.iter().filter(|n| n.starts_with("Sw~short=")).count(), 2, "{names:?}");
    assert_eq!(names[0], "Top", "the unkeyed body keeps its id: {names:?}");
    let kinds: Vec<BranchKind> = names.iter().filter(|n| n.starts_with("Sw~")).map(|n| body(&ir, n).branches()[0].kind).collect();
    assert_eq!(kinds, [BranchKind::Potential, BranchKind::Flow]);
    let bodies_of: Vec<&str> = ["Top.a", "Top.b", "Top.c"].iter().map(|p| ir.instance_at(p).expect("an instance").body().name()).collect();
    assert_eq!(bodies_of[0], bodies_of[2]);
    assert_ne!(bodies_of[0], bodies_of[1]);
}

#[test]
fn a_parent_param_feeding_a_key_param_is_a_key() {
    // A param read by the value a placement gives a key param of its child is a key of the same
    // class: `Pair.gate` feeds `Sw.short`, so `gate` is structural and `Pair` has a body per value.
    let src = format!("{SWITCH}module Pair(a: Electrical, b: Electrical) {{ gate: f64 = 1.0; other: f64 = 3.0; let sw = Sw {{ short: gate }}(a, b); }}\nmodule Top() {{ signal s: Electrical; let p = Pair {{ gate: 0.0 }}(s, gnd); }}\n");
    let ir = Elab::ir(&src, "Top");
    let pair = ir.bodies().find(|b| b.name().starts_with("Pair~")).expect("a keyed Pair");
    let structural: Vec<(&str, bool)> = pair.params().iter().map(|p| (p.name.as_str(), p.structural)).collect();
    assert_eq!(structural, [("gate", true), ("other", false)]);
    assert!(pair.name().contains("gate=") && !pair.name().contains("other"), "{}", pair.name());
    assert!(body_names(&ir).iter().any(|n| n.starts_with("Sw~short=")));
}

#[test]
fn a_design_without_key_params_keeps_its_body_ids() {
    // No key param: every body is lowered in first-reach order and keeps its plain name, as before
    // the variants existed.
    let ir = Device::ir("i(p, n) +> v(p, n) / 1k;");
    assert_eq!(body_names(&ir), ["Top", "M"]);
    assert!(ir.bodies().all(|b| b.params().iter().all(|p| !p.structural)));
}
