//! Scope: the `constraint::` selector axis (T49, `lrm-conformance-wave3`,
//! PSW3-01/B1). Before this task `Axis::Constraint` was a live grammar
//! production (`crate::pom::selector::ast::Axis::Constraint`) that dispatched
//! to an **empty arm** — `constraint::` parsed, always matched nothing, and
//! there was no way to tell "no constraints authored" apart from "this axis
//! is unimplemented" (`docs/spec/part_iv_reflection_selector.md` §10 item 2).
//! This file proves the axis now dispatches to `Module::constraints()` for
//! real: it returns the authored `require`/`var`/`target`/`cover` statements
//! of a module's `constraint begin … end` block (MD-49), by name where the
//! statement has one, and an empty (not error) selection is now
//! distinguishable as "this module truly authored none" rather than "the
//! axis does nothing".
//!
//! SPEC-COVERS: part_iv#9

use piperine_lang::pom::node::Node;
use piperine_lang::pom::traits::{Kinded, Named};
use piperine_lang::{parse_and_elaborate, SourceMap};

const DISCIPLINES: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline
"#;

fn with_constraints(body: &str) -> String {
    format!(
        "{DISCIPLINES}\nmodule Top (inout electrical g, s);\n    parameter real vgs_max = 1.8;\n{body}\nendmodule\n"
    )
}

/// `parse_and_elaborate` does not infer a top module (that inference lives
/// in `piperine-api`'s `model::Design::load`, not the lang crate) —
/// `Design::select`'s relative form resolves from `top()`, so tests need to
/// set it explicitly.
fn elaborate_with_top(src: &str) -> piperine_lang::Design {
    let mut design = parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"));
    design.set_top("Top");
    design
}

#[test]
fn constraint_axis_returns_the_authored_require_statements() {
    let src = with_constraints(
        "    constraint begin\n\
         \x20\x20\x20\x20\x20\x20\x20\x20require vgs_ox : abs(V(g, s)) <= vgs_max;\n\
         \x20\x20\x20\x20end",
    );
    let design = elaborate_with_top(&src);

    let sel = design.select("constraint::*").expect("constraint:: axis parses and evaluates");
    assert_eq!(sel.len(), 1, "exactly one authored constraint statement");
    let node = sel.iter().next().expect("one node");
    assert_eq!(node.kind().to_string(), "constraint", "node kind is Kind::Constraint");
    assert_eq!(node.name(), "vgs_ox", "selected node carries the require's authored label");
    assert!(matches!(node, Node::Constraint(_)), "resolves to the Node::Constraint variant");
}

#[test]
fn constraint_axis_returns_every_statement_in_authored_order() {
    let src = with_constraints(
        "    constraint begin\n\
         \x20\x20\x20\x20\x20\x20\x20\x20require vgs_ox : abs(V(g, s)) <= vgs_max;\n\
         \x20\x20\x20\x20\x20\x20\x20\x20require vds_ox : abs(V(g, s)) <= vgs_max;\n\
         \x20\x20\x20\x20end",
    );
    let design = elaborate_with_top(&src);

    let sel = design.select("constraint::*").expect("constraint:: axis parses and evaluates");
    let names: Vec<&str> = sel.iter().map(Named::name).collect();
    assert_eq!(names, vec!["vgs_ox", "vds_ox"], "authored order preserved through the selector");
}

#[test]
fn constraint_axis_is_a_real_empty_selection_when_none_are_authored() {
    // A module with no `constraint begin … end` block: the axis must still
    // dispatch (no error) and correctly report zero matches — this is the
    // "no constraints authored" case the old dead arm could never be told
    // apart from "the axis does nothing".
    let src = with_constraints("    analog V(g, s) <+ 0.0;");
    let design = elaborate_with_top(&src);

    let sel = design.select("constraint::*").expect("constraint:: axis parses and evaluates");
    assert!(sel.is_empty(), "no constraint block authored, so the selection is genuinely empty");
}

#[test]
fn other_inert_axes_fail_loud_rather_than_return_an_empty_selection() {
    // `driver::`/`load::`/`parent::`/`ancestor::` parse but are not yet
    // rebuilt on the flattened connectivity form (design.md, part_iv#10) —
    // per T49's decision (see commit body) they stay `AxisNotImplemented`
    // errors, never a silent empty `Ok(..)`. Pinned here so a future
    // regression that turns the error back into a swallowed empty result
    // is caught by this axis's own test, not just by inspection.
    let src = with_constraints("    analog V(g, s) <+ 0.0;");
    let design = elaborate_with_top(&src);

    for axis in ["driver", "load", "parent", "ancestor"] {
        let path = format!("{axis}::*");
        let err = design
            .select(&path)
            .expect_err(&format!("`{path}` must fail loud, not return an empty selection"));
        assert!(
            err.to_string().contains("not yet implemented"),
            "unexpected error shape for `{path}`: {err}"
        );
    }
}
