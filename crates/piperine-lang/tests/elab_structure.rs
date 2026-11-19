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
