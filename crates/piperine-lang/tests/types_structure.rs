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
