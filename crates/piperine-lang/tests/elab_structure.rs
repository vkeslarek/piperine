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
const SWITCH: &str = "module Sw(a: Electrical, b: Electrical) { short: f64 = 1.0; }\nanalog for Sw { if short > 0.5 { v(a, b) +> 1.0; } else { i(a, b) +> 0.0; } }\n";

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
fn a_match_on_a_param_picks_its_arm_and_makes_the_param_structural() {
    // AS-02 AC 3 (§11 rule 3: the selector is the conjunction of the conditions of the `if` AND
    // `match` arms): a `match` on a param fixes the kind like an `if`, the param is structural and
    // only the selected arm is lowered. `true` selects the potential arm, `false` the flow arm.
    let mode = "module Sw(a: Electrical, b: Electrical) { mode: bool = true; }\nanalog for Sw { match mode { true => { v(a, b) +> 1.0; } _ => { i(a, b) +> 0.0; } } }\n";
    for (value, kind) in [(true, BranchKind::Potential), (false, BranchKind::Flow)] {
        let src = format!("{mode}module Top() {{ signal s: Electrical; let m = Sw {{ mode: {value} }}(s, gnd); }}");
        let ir = Elab::ir(&src, "Top");
        let variant = ir.bodies().find(|b| b.name().starts_with("Sw~mode=")).expect("a body of the variant");
        assert_eq!(variant.branches().iter().map(|b| b.kind).collect::<Vec<_>>(), [kind], "mode = {value}");
        assert!(variant.params()[0].structural, "`mode` is structural");
        assert_eq!(variant.contributions().len(), 1, "only the selected arm is lowered");
        assert!(variant.contributions()[0].conditions.is_empty(), "the folded match is no condition");
    }
}

#[test]
fn a_match_on_a_probe_stays_a_run_time_selector() {
    // §11 rule 3 / rule 5: a `match` on a probe chooses between the flow contributions of a branch
    // with no potential contribution at run time; nothing folds and both arms are lowered under
    // their conditions.
    let ir = Device::ir("match v(p) > 0.5 { true => { i(p, n) +> 1.0; } _ => { i(p, n) +> 2.0; } }");
    let body = ir.bodies().find(|b| b.name() == "M").expect("M");
    assert_eq!(body.contributions().len(), 2, "both arms are lowered");
    assert!(body.contributions().iter().all(|c| !c.conditions.is_empty()), "each arm sits under its condition");
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

/// A source whose module `Short(a, b, c)` writes `analog`, placed between `s` and `t` (and a third
/// node `u` for `c`), in `Top`.
fn shorted(analog: &str, ends: &str) -> String {
    format!("module Short(a: Electrical, b: Electrical, c: Electrical) {{ k: f64 = 1.0; }}\nanalog for Short {{ {analog} }}\nmodule Top() {{ signal s: Electrical; signal t: Electrical; signal u: Electrical; let m = Short {{ }}({ends}); }}")
}

#[test]
fn a_fixed_zero_potential_branch_merges_its_nodes() {
    // AS-02 AC 4 (§11 rule 3): `v(a, b) +> 0.0` is a fixed potential branch of the constant zero
    // with no flow read: its nodes merge, so the bound circuit has one node fewer than with
    // `v(a, b) +> 1.0`. A param selector that folds to the zero contribution merges too.
    let merged = Elab::ir(&shorted("v(a, b) +> 0.0;", "s, t, u"), "Top");
    let driven = Elab::ir(&shorted("v(a, b) +> 1.0;", "s, t, u"), "Top");
    assert_eq!(merged.nodes().count() + 1, driven.nodes().count());
    let body = body(&merged, "Short");
    assert_eq!(body.merges().len(), 1);
    assert!(body.branches().is_empty(), "the merged branch is no branch");
    let folded = Elab::ir(&shorted("if k > 0.5 { v(a, b) +> 0.0; } else { v(a, b) +> 1.0; }", "s, t, u"), "Top");
    assert_eq!(folded.nodes().count() + 1, driven.nodes().count(), "the folded arm is the zero");
}

#[test]
fn params_only_selectors_enabling_both_kinds_are_e0415() {
    // §11 rule 2: the selectors depend on `k` only and both are true for `k` = 1, so a potential and
    // a flow contribution are enabled together: E0415 at elaboration, naming the branch.
    let src = shorted("if k > 0.5 { v(a, b) +> 1.0; } if k > 0.5 { i(a, b) +> 1.0; }", "s, t, u");
    let err = Elab::run(&src, "Top").expect_err("both kinds enabled");
    assert_eq!(Elab::codes(&err), ["E0415"]);
    assert!(err[0].diagnostic.message.contains("(a, b)"), "{}", err[0].diagnostic.message);
    // With `k` = 0 neither is enabled in the variant: no contribution, no error.
    let none = src.replace("let m = Short { }", "let m = Short { k: 0.0 }");
    Elab::ir(&none, "Top");
}

#[test]
fn a_zero_potential_branch_whose_flow_is_read_is_not_merged() {
    // §11 rule 3, last bullet: a probed short keeps its flow unknown like any potential source: `i(c)`
    // is driven by the flow of `(a, b)`, so the branch stays and the nodes do not merge.
    let ir = Elab::ir(&shorted("v(a, b) +> 0.0; i(c, gnd) +> i(a, b);", "s, t, u"), "Top");
    assert!(body(&ir, "Short").merges().is_empty());
    assert_eq!(body(&ir, "Short").branches().len(), 2);
    assert_eq!(ir.nodes().count(), 3);
}

#[test]
fn a_merge_with_ground_binds_the_node_to_ground() {
    // `Short(s, gnd, u)`: merging a node with ground removes the node and binds the instance's port
    // to ground.
    let ir = Elab::ir(&shorted("v(a, b) +> 0.0;", "s, gnd, u"), "Top");
    assert_eq!(ir.nodes().count(), 2, "`s` is gone, `t` and `u` are left");
    assert!(ir.node_at("Top.s").is_none());
    assert_eq!(ir.instance_at("Top.m").expect("m").binding(0), Some(piperine_ir::NodeRef::Ground));
}

/// The rows of the contributions of body `M`: (quantity, operators of the addends, negations).
fn rows(ir: &Ir) -> Vec<(QuantityRole, Vec<(piperine_ir::Operator, bool)>)> {
    body(ir, "M").contributions().iter().map(|c| (c.quantity, c.addends.iter().map(|a| (a.operator, a.negated)).collect())).collect()
}

/// A device `M(p, n, in clk: Bit, out r: Bit<f64>)` with `items` in its body and `analog` as its `analog` block.
fn charge_device(items: &str, analog: &str, digital: &str) -> String {
    format!("module M(p: Electrical, n: Electrical, in clk: Bit, out r: Bit<f64> = 0.0) {{ c: f64 = 1p; {items} }}\nanalog for M {{ {analog} }}\ndigital for M {{ {digital} }}\nmodule Top() {{ signal s: Electrical; signal ck: Bit; signal r: Bit<f64>; let m = M {{ }}(s, gnd, ck, r); }}")
}

#[test]
fn a_body_let_of_ddt_read_as_an_addend_gives_the_inline_rows() {
    // AS-07 AC 3 (§8.2.25): a body `let` that binds `ddt(x)` and is read only as an addend is its
    // expression at the read: the same rows as the inline form, a `Ddt` addend and no site.
    let inline = Elab::ir(&charge_device("", "i(p, n) +> ddt(c * v(p, n)) + v(p, n) / 1k;", ""), "Top");
    let through = Elab::ir(&charge_device("let q = ddt(c * v(p, n));", "i(p, n) +> q + v(p, n) / 1k;", ""), "Top");
    assert_eq!(rows(&through), rows(&inline));
    assert!(body(&through, "M").derivatives().is_empty() && body(&inline, "M").derivatives().is_empty());
    assert_eq!(rows(&inline)[0].1, [(piperine_ir::Operator::Ddt, false), (piperine_ir::Operator::Direct, false)]);
    // Read anywhere else (here also as a factor) it is a site, and the contribution reads its rate.
    let site = Elab::ir(&charge_device("let q = ddt(c * v(p, n));", "i(p, n) +> 2.0 * q;", ""), "Top");
    assert_eq!(body(&site, "M").derivatives().len(), 1);
    assert_eq!(rows(&site)[0].1, [(piperine_ir::Operator::Direct, false)]);
}

#[test]
fn a_ddt_read_in_analog_and_in_an_event_is_one_site() {
    // AS-07 AC 4 (§8.3.3): `let dv = ddt(v(p))` read in the `analog` block and in a reaction is one
    // instance: one derivative site whose rate both reads name.
    let src = charge_device("let dv = ddt(v(p));", "i(p, n) +> 2.0 * dv;", "on clk.rise() { r <- dv; }");
    let ir = Elab::ir(&src, "Top");
    assert_eq!(body(&ir, "M").derivatives().len(), 1);
}

#[test]
fn a_ddt_feedback_through_its_own_result_is_e0353() {
    // §8.3.6: a core module's result depends directly on its input, so `let y = ddt(f(y))` is a
    // cycle with no unknown: E0353 naming `y`, before elaboration.
    let design = Elab::design("module Top() { let y = ddt(2.0 * y); }");
    let found = piperine_lang::types::Types::check(&design, "Top").expect_err("a feedback");
    assert_eq!(Elab::codes(&found), ["E0353"]);
}

#[test]
fn a_limit_in_a_closure_is_e0350() {
    // §8.3.4: a core module is not placed in a closure, because its inputs are evaluated at every
    // solution whatever control flow surrounds it: `limit` in the body of a closure is `E0350`, found
    // by the type checker before elaboration lowers anything (so no body is built around it).
    let src = Device::source("let o: Option<f64> = None; let x = o.unwrap_or_else(|| limit(1.0, |n, o| n)); i(p, n) +> x;");
    let err = Elab::run(&src, "Top").expect_err("a limit in a closure");
    assert_eq!(Elab::codes(&err), ["E0350"]);
    assert!(err[0].diagnostic.message.contains("limit"), "{}", err[0].diagnostic.message);
}

#[test]
fn an_override_of_a_key_param_lowers_its_variant() {
    // AS-15 AC 1 (design mechanism 4): the host's before-run binding `Top.m.short = 0` of the key
    // param `short` selects the variant of that value at that instance, whatever its default: the body
    // is `Sw~short=0.0` with a flow branch and the instance holds 0. A binding of a param that is no key
    // is ignored by the elaborator, and a relative path names the same instance.
    use piperine_lang::elab::{Elaborator, Overrides};
    let src = format!("{SWITCH}module Top() {{ signal s: Electrical; let m = Sw {{ }}(s, gnd); }}");
    let design = Elab::design(&src);
    for entry in ["Top.m.short", "m.short"] {
        let ir = Elaborator::new(&design).run_with("Top", &Overrides::new(&[(entry.to_string(), 0.0)])).expect("elaborates");
        let m = ir.instance_at("Top.m").expect("m");
        assert_eq!(m.body().name(), "Sw~short=0.0", "{entry}");
        assert_eq!(m.body().branches()[0].kind, BranchKind::Flow, "{entry}");
        assert_eq!(m.param(0), Some(&piperine_ir::Constant::Real(piperine_ir::Real::new(0.0))), "{entry}");
    }
    let plain = Elaborator::new(&design).run_with("Top", &Overrides::new(&[("Top.m.nothing".to_string(), 0.0)])).expect("elaborates");
    assert_eq!(plain.instance_at("Top.m").expect("m").body().name(), "Sw~short=1.0", "the default variant: the binding names no key param");
    assert_eq!(body_names(&Elaborator::new(&design).run("Top").expect("elaborates")), body_names(&plain));
}

#[test]
fn the_section_18_diode_waits_for_params_and_io() {
    // AS-05 AC 3 and 4 (Q2): the section 18 `Diode` as the spec writes it has `rs: Option<f64>`, an
    // `Option` param that `params-and-io` (L28) owns. Until then it is refused by name, not mis-run:
    // `P0001` naming the param type. The local `DiodeRs` (`tests/ngspice/diode_rs.msl`) proves the
    // mechanism with a params-only `f64` selector.
    let src = "module Diode(a: Electrical, k: Electrical) {\n    is: f64 = 1e-14 where is > 0.0;\n    rs: Option<f64>;\n    signal ai: Electrical;\n}\nanalog for Diode {\n    match rs {\n        Some(r) => { i(a, ai) +> v(a, ai) / r; }\n        None    => { v(a, ai) +> 0.0; }\n    }\n    i(ai, k) +> is * (v(ai, k) / vt()).exp();\n}\nmodule Top() { signal s: Electrical; let d = Diode { rs: Some(100.0) }(s, gnd); }";
    let design = Elab::design(src);
    let found = piperine_lang::types::Types::check(&design, "Top").expect_err("the Option param is refused");
    assert_eq!(Elab::codes(&found), ["P0001"]);
    assert!(found[0].diagnostic.message.contains("Option"), "the refusal names the param type: {}", found[0].diagnostic.message);
}

/// `Lag` of AS-09: a result that reads its own node, hoisted into each body that places it.
const LAG: &str = "module Lag(x: f64) -> f64 { tau: f64 = 1m; signal y: State; potential(y) }\nanalog for Lag { flow(y) +> tau * ddt(potential(y)) + potential(y) - x; }\n";

#[test]
fn two_placements_with_equal_arguments_are_two_instances() {
    // AS-09 AC 3 (§8.3.3): two placements written the same make two instances, each with its own node
    // (`Lag$0.y` and `Lag$1.y`) and its own equation, not one memoized by its arguments.
    let src = format!("{LAG}module Top() {{ signal a: Electrical; signal o: Electrical; }}\nanalog for Top {{ v(o) +> Lag(v(a)) + Lag(v(a)); }}");
    let ir = Elab::ir(&src, "Top");
    let top = body(&ir, "Top");
    let names: Vec<&str> = top.nodes().iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["a", "o", "Lag$0.y", "Lag$1.y"], "one node per placement, after the body's own");
    assert_eq!(top.hoisted().iter().map(|h| h.name.as_str()).collect::<Vec<_>>(), ["Lag$0", "Lag$1"]);
    assert_eq!(top.contributions().len(), 3, "the body's potential and one flow row per instance");
}

#[test]
fn a_selector_through_a_value_input_is_judged_per_placement() {
    // AS-02 AC 2 (§11 rule 3, §8.3.9): the selector of `Gate` reads its value input `s`. It is judged
    // per instance, after substitution: a placement that binds `s` to a probe gets E0414 (the branch
    // has a potential contribution under a selector that reads a probe), one that binds it to a param
    // does not.
    let gate = "module Gate(a: Electrical, s: f64) {}\nanalog for Gate { if s > 0.5 { v(a) +> 1.0; } else { i(a, gnd) +> 0.0; } }\n";
    let probed = format!("{gate}module Top() {{ signal a: Electrical; signal b: Electrical; let g = Gate(a, v(b)); }}");
    let err = Elab::run(&probed, "Top").expect_err("a probe selects a potential branch");
    assert_eq!(Elab::codes(&err), ["E0414", "E0414"], "one at each contribution under the selector");
    let bound = format!("{gate}module Top() {{ k: f64 = 1.0; signal a: Electrical; let g = Gate(a, k); }}");
    let ir = Elab::ir(&bound, "Top");
    assert_eq!(body(&ir, "Top").branches().iter().map(|b| b.kind).collect::<Vec<_>>(), [BranchKind::Switch], "a param selects at run time");
}

#[test]
fn the_stdlib_idt_waits_for_generics() {
    // AS-05 AC 1 (Q2): the stdlib `idt` of section 22.2.8 is generic (`idt<S: Nature = State>`), which
    // `generics` (L30) owns; until then a placement of it is refused by name, never run as a different
    // module. The local `Integ` (`tests/structure/modules.msl`) proves the mechanism.
    let src = "module Top() { signal o: Electrical; }\nanalog for Top { v(o) +> idt(1.0, 0.0); }";
    let design = Elab::design(src);
    let found = piperine_lang::types::Types::check(&design, "Top").expect_err("a generic module is refused");
    assert_eq!(Elab::codes(&found), ["P0001"]);
    assert!(found[0].diagnostic.message.contains("generic"), "the refusal names the form: {}", found[0].diagnostic.message);
}

#[test]
fn the_water_model_waits_for_generics() {
    // AS-05 AC 2 (Q2): `Water` of Part IV section 13 reads `idt::<Heat>`, a generic instantiation: refused
    // by name until `generics` merges.
    let src = "#[derive(Repr, Clone, Copy, PartialEq)]\nenum Phase { Liquid, Boiling, Vapor }\nnature Heat { potential: Energy { access: joules, units: \"J\", abstol: 1m }, flow: Power { access: pwr, units: \"W\", abstol: 1n } }\nmodule Water(t: Thermal) {\n    mass: f64 = 1.0;\n    hv: f64 = 2.26M;\n    pub signal phase: Bit<Phase> = Phase::Liquid;\n    let ph = phase.known_or(Phase::Liquid);\n    let absorbed = idt::<Heat>(pwr(t), 0.0, reset: ph != Phase::Boiling);\n}\nmodule Top() { signal t: Thermal; let w = Water(t); }";
    let design = Elab::design(src);
    let found = piperine_lang::types::Types::check(&design, "Top").expect_err("a generic instantiation is refused");
    assert!(Elab::codes(&found).iter().all(|c| c == "P0001"), "{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    assert!(found.iter().any(|d| d.diagnostic.message.contains("generic")), "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}

#[test]
fn a_param_value_let_may_read_another_let() {
    // AS-12 AC 2 (§8.1.19): `b` reads `a`, which reads the param `r`: the placement's param value is the
    // chain over `r`, an expression the host's derive plan recomputes (`ChildValue::Expr`), and the
    // instance holds its value for r = 2.0: b = (2.0 * 3.0) + 1.0 = 7.0.
    let src = "module Leaf() { c: f64 = 1.0; }\nmodule Top() { r: f64 = 2.0; let a = r * 3.0; let b = a + 1.0; let leaf = Leaf { c: b }(); }";
    let ir = Elab::ir(src, "Top");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("the body Top");
    assert!(matches!(top.child_params(0), [(0, piperine_ir::ChildValue::Expr(_))]), "the value is an expression over the param: {:?}", top.child_params(0));
    let leaf = ir.instances().find(|i| i.path() == "Top.leaf").expect("the instance Top.leaf");
    assert_eq!(leaf.params(), [piperine_ir::Constant::Real(piperine_ir::Real::new(7.0))], "the instance holds the value for r = 2.0");
}

#[test]
fn a_param_value_let_that_reads_a_probe_is_e0321() {
    // AS-12 AC 3 (§8.1.19): a `let` used as a param value that reads a probe, a carrier or the environment
    // has no value at elaboration: `E0321` at the use, naming the `let`.
    for (let_, reads) in [("let p = v(a);", "a probe"), ("let p = temperature();", "a call that is not a function"), ("let p = k.known_or(0.0);", "a carrier")] {
        let src = format!("module Leaf() {{ c: f64 = 1.0; }}\nmodule Mid(a: Electrical) {{ signal k: Bit<f64>; {let_} let leaf = Leaf {{ c: p }}(); }}\nmodule Top() {{ signal a: Electrical; let m = Mid(a); }}");
        let err = Elab::run(&src, "Top").expect_err("the value has no constant");
        assert_eq!(Elab::codes(&err), ["E0321"], "{let_}: {:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        let message = &err[0].diagnostic.message;
        assert!(message.contains("the `let` `p`"), "{let_}: it names the let: {message}");
        assert!(message.contains(reads), "{let_}: it says what the let reads ({reads}): {message}");
    }
}
