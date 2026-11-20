//! The typing of each `analog-structure` form and its codes (feature `analog-structure`, part III
//! §8.3): feedback through an instance (AS-10), then, as the tasks land, a continuous argument read
//! by a callee's definition (AS-08) and `inst.name` (AS-11). Every source is a few lines checked
//! over the embedded standard library; every expectation is the rule cited at its assertion.

use piperine_lang::pom::{Design, DesignDiagnostic, ExprId};
use piperine_lang::types::{Ty, Types};

/// A module whose result reads only its own carrier, so it goes through an unknown (§8.3.5).
const INTEG: &str = "module Integ(x: f64, ic: f64) -> f64 {\n    signal s: Bit<f64> = 0.0;\n    s.known_or(0.0)\n}\n";
/// A module whose result depends directly on its input (§8.3.5).
const TWICE: &str = "module twice(x: f64) -> f64 { 2.0 * x }\n";

/// The design of `src`, which must parse and resolve.
fn design_of(src: &str) -> Design {
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    design
}

/// The diagnostics `Top` of `src` raises when checked.
fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
    Types::check(&design_of(src), "Top").expect_err("expected diagnostics")
}

#[test]
fn feedback_through_an_unknown_of_a_module_is_accepted() {
    // AS-10 AC 3 (§8.3.5): `let y = Integ(-y / 1m, 1.0);` reads its own result in its arguments, and
    // the result reads only the module's own carrier, so the cycle has an unknown in it: it types,
    // `y` as the declared result type `f64`, with no `P0001`. A nested placement that goes through an
    // unknown keeps the cycle legal even when another result on it depends directly (`twice`).
    for body in ["let y = Integ(-y / 1m, 1.0);", "let y = Integ(twice(y), 1.0);", "let y = (Integ(y, 1.0));"] {
        let src = format!("{TWICE}{INTEG}module Top() {{ {body} }}");
        let design = design_of(&src);
        let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{body}: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
        let reads: Vec<ExprId> = (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && src[design.expr_node(*e).span.range()] == *"y").collect();
        assert!(!reads.is_empty() && reads.iter().all(|e| types.ty(*e) == Some(&Ty::F64)), "{body}: `y` reads as the declared result type f64");
    }
}

#[test]
fn a_cycle_with_a_direct_dependence_at_every_result_is_e0353() {
    // AS-10 AC 2 (§8.3.5, §8.3.6): a cycle in which every result depends directly on its input has no
    // unknown in it: `E0353` at the read, naming the cycle's names (`y`, the placed modules; a core
    // module's result always depends directly).
    for (body, cycle) in [
        ("let y = twice(y);", "y -> twice -> y"),
        ("let y = ddt(y);", "y -> a core module -> y"),
        ("let y = twice(1.0 + twice(y));", "y -> twice -> twice -> y"),
        ("let y = ddt(twice(y));", "y -> a core module -> twice -> y"),
    ] {
        let src = format!("{TWICE}module Top() {{ {body} }}");
        let found = diagnostics(&src);
        let codes: Vec<&str> = found.iter().map(|d| &*d.diagnostic.code).collect();
        assert_eq!(codes, ["E0353"], "{body}");
        assert!(found[0].diagnostic.message.contains(cycle), "{body}: the message names `{cycle}`: {}", found[0].diagnostic.message);
    }
}

/// A module that drives its `out` carrier with a definition reading its value input `x` (§8.2.5).
const HOLD: &str = "module Hold(x: f64, out q: Bit<f64>) { q = x; }\n";

#[test]
fn a_continuous_argument_read_by_a_callee_definition_is_e0336() {
    // AS-08 AC 4 (§8.3.9, §8.2.5): `Hold` reads its value input `x` in the definition `q = x`, which is
    // a discrete expression; a probe passed as `x` is read there as if written in the definition, so
    // `E0336` at the call, naming the input `x`. The same input read through a `let` of the callee is
    // the same read.
    for (items, call) in [
        (HOLD, "Hold(v(a), q)"),
        ("module Hold(x: f64, out q: Bit<f64>) { let y = x * 2.0; q = y; }\n", "Hold(v(a), q)"),
        (HOLD, "Hold(now(), q)"),
    ] {
        let src = format!("{items}module Top(a: Electrical) {{ signal q: Bit<f64>; let h = {call}; }}");
        let found = diagnostics(&src);
        let codes: Vec<&str> = found.iter().map(|d| &*d.diagnostic.code).collect();
        assert_eq!(codes, ["E0336"], "{call}: {:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(&src[found[0].diagnostic.span().range()], call, "the error is at the call");
        assert!(found[0].diagnostic.message.contains("`x`"), "{call}: it names the input: {}", found[0].diagnostic.message);
    }
}

#[test]
fn a_discrete_argument_read_by_a_callee_definition_is_accepted() {
    // AS-08 AC 4, the other side: a literal, a param or a carrier read is discrete, so a definition may
    // read it; and a continuous argument for an input that only an `analog` block reads is no
    // definition's read (the `analog` rule, `E0150`, is not this one).
    for call in ["Hold(2.0, q)", "Hold(k * 2.0, q)", "Hold(c.known_or(0.0), q)"] {
        let src = format!("{HOLD}module Top(a: Electrical, in c: Bit<f64>) {{ k: f64 = 1.0; signal q: Bit<f64>; let h = {call}; }}");
        let design = design_of(&src);
        Types::check(&design, "Top").unwrap_or_else(|d| panic!("{call}: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    }
    let src = "module Probe(x: f64, p: Electrical) { }\nanalog for Probe { v(p) +> x; }\nmodule Top(a: Electrical) { let h = Probe(v(a), a); }";
    Types::check(&design_of(src), "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
}

/// A child with a `pub` node, carrier, param and `let`, and an item of each that is not `pub`.
const CELL: &str = "module Cell(a: Electrical) {\n    pub signal mid: Electrical;\n    pub signal state: Bit<u8> = 0;\n    pub r: f64 = 2.0;\n    pub let e = r * 2.0;\n    hidden: f64 = 1.0;\n    signal inner: Electrical;\n}\n";

#[test]
fn inst_name_types_as_the_pub_item() {
    // AS-11 AC 1 to 4 (§8.2.8): `c.mid` is the child's node (a probe takes it), `c.state` the child's
    // carrier of `Bit<u8>` (read by `known_or`, it is a `u8`), `c.r` the child's param (an `f64`) and
    // `c.e` the child's `let` (the type of its value, an `f64`): each types as the item itself.
    let src = format!("{CELL}module Top(a: Electrical) {{\n    let c = Cell(a);\n    let z = c.r;\n    let w = c.e;\n}}\nanalog for Top {{ v(a) +> v(c.mid) + (c.state.known_or(0) as f64) + z + w; }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    let typed_as = |text: &str| -> Vec<Option<Ty>> { (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| src.get(design.expr_node(*e).span.range()) == Some(text) && matches!(design.expr_node(*e).kind, piperine_lang::pom::ExprKind::Field { .. })).map(|e| types.ty(e).cloned()).collect() };
    assert!(!typed_as("c.r").is_empty(), "the read of `c.r` is in the design");
    assert!(typed_as("c.r").iter().all(|t| *t == Some(Ty::F64)), "the param `r` reads as f64");
    assert!(typed_as("c.e").iter().all(|t| *t == Some(Ty::F64)), "the let `e` reads as f64");
    assert!(typed_as("c.mid").iter().all(|t| matches!(t, Some(Ty::Node(_)))), "the node `mid` reads as a node");
    assert!(typed_as("c.state").iter().all(|t| *t == Some(Ty::UInt(8))), "the carrier `state` reads as its value, a u8");
}

#[test]
fn inst_name_of_an_item_that_is_not_pub_is_e0343() {
    // AS-11 AC 5 (§8.2.8): `hidden` is a param and `inner` a node of `Cell`, neither `pub`: each read
    // is `E0343` naming the item, at the item's name.
    for item in ["hidden", "inner"] {
        let src = format!("{CELL}module Top(a: Electrical) {{ let c = Cell(a); let z = c.{item}; }}");
        let design = Design::builder().source("top.msl", &src).build();
        let found: Vec<_> = design.diagnostics().iter().filter(|d| &*d.diagnostic.code == "E0343").collect();
        assert_eq!(found.len(), 1, "{item}: {:?}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        assert_eq!(&src[found[0].diagnostic.span().range()], item, "the error is at the item's name");
        assert!(found[0].diagnostic.message.contains(&format!("`{item}`")), "{item}: it names the item: {}", found[0].diagnostic.message);
    }
}

#[test]
fn a_commit_to_a_childs_carrier_is_e0222() {
    // AS-11 AC 6 (§8.2.10): a parent never writes a child's drive: `c.state <- 1` in a reaction and the
    // definition `c.state = 1` are each `E0222` at the place, naming the carrier.
    for (item, behavior) in [("", "digital for Top { on at(1m) { c.state <- 1; } }"), ("c.state = 1;", "")] {
        let body = format!("{item}{behavior}");
        let src = format!("{CELL}module Top(a: Electrical) {{ let c = Cell(a); {item} }}\n{behavior}");
        let found = diagnostics(&src);
        let codes: Vec<&str> = found.iter().map(|d| &*d.diagnostic.code).collect();
        assert_eq!(codes, ["E0222"], "{body}: {:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(&src[found[0].diagnostic.span().range()], "c.state", "the error is at the place");
        assert!(found[0].diagnostic.message.contains("carrier of an instance"), "{body}: {}", found[0].diagnostic.message);
    }
}

#[test]
fn inflow_of_a_node_that_is_not_a_port_is_e0243() {
    // AS-13 AC 4 (§6 rule 5): `inflow()` measures the flow entering the module at a port of a nature with
    // `flow`; an internal node `x` is no port: `E0243` at the call.
    let src = "module Top(a: Electrical) { signal x: Electrical; }\nanalog for Top { v(a) +> x.inflow(); }";
    let found = diagnostics(src);
    let codes: Vec<&str> = found.iter().map(|d| &*d.diagnostic.code).collect();
    assert_eq!(codes, ["E0243"], "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    assert_eq!(&src[found[0].diagnostic.span().range()], "x.inflow()", "the error is at the call");
}

#[test]
fn inflow_on_the_left_of_a_contribution_is_e0410() {
    // AS-13 AC 5: `p.inflow()` is a value, not a branch quantity: on the left of `+>` it is `E0410`.
    let src = "module Top(a: Electrical) {}\nanalog for Top { a.inflow() +> 1.0; }";
    let found = diagnostics(src);
    let codes: Vec<&str> = found.iter().map(|d| &*d.diagnostic.code).collect();
    assert_eq!(codes, ["E0410"], "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}

#[test]
fn changed_and_prev_of_a_param_type_as_an_event_and_the_params_type() {
    // AS-14 (§5.3.1): `r.changed()` is an `Event`, an event position takes it, and `r.prev()` is the
    // param's own type: an `f64`, not flagged, so an `analog` block may add it to a probe.
    let src = "module Top(a: Electrical) { r: f64 = 2.0; }\nanalog for Top { v(a) +> r.prev(); }\ndigital for Top { on r.changed() { emit!(\"c\", p = r.prev()); } }";
    let design = design_of(src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    let typed_as = |text: &str| -> Vec<Option<Ty>> { (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| src.get(design.expr_node(*e).span.range()) == Some(text) && types.ty(*e).is_some()).map(|e| types.ty(e).cloned()).collect() };
    assert!(typed_as("r.changed()").iter().all(|t| *t == Some(Ty::Event)), "an event");
    assert!(!typed_as("r.prev()").is_empty() && typed_as("r.prev()").iter().all(|t| *t == Some(Ty::F64)), "the param's type");
}

#[test]
fn an_initial_value_that_reads_a_carrier_is_e0210() {
    // AS-15 AC 3 (§5.1.5): an initial value is over params and constants: a carrier or a probe in it is
    // `E0210` at the read, for a node and for a carrier alike; a param is accepted.
    for (init, read) in [("signal a: Electrical = c.known_or(0.0);", "c"), ("signal b: Bit<f64> = c.known_or(0.0);", "c"), ("signal a: Electrical = v(x);", "v(x)")] {
        let src = format!("module Top(in c: Bit<f64>, x: Electrical) {{ {init} }}");
        let found = diagnostics(&src);
        let codes: Vec<&str> = found.iter().map(|d| &*d.diagnostic.code).collect();
        assert_eq!(codes, ["E0210"], "{init}: {:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert!(src.get(found[0].diagnostic.span().range()).is_some_and(|t| t.contains(read)), "{init}: the error is at the read of `{read}`");
    }
    for init in ["signal a: Electrical = k0;", "signal b: Bit<f64> = k0 * 2.0;"] {
        let src = format!("module Top() {{ k0: f64 = 1.0; {init} }}");
        Types::check(&design_of(&src), "Top").unwrap_or_else(|d| panic!("{init}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    }
}
