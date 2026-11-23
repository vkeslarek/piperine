//! Scope: the elaboration of the forms of `analog-operators`: `ddx` (AO-01), the delay, noise,
//! session parameter and trace forms as their lowering tasks land. Each source is a module `M`
//! placed by `Top`; the expectation is the IR the form lowers to, or the code and the form a
//! refusal names.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

/// The design `M` placed once, whose `analog` block is `body`.
fn design(body: &str) -> String {
    format!("module M(a: Electrical, o: Electrical) {{ k: f64 = 2.0; }}\nanalog for M {{ {body} }}\nmodule Top() {{ signal a: Electrical; signal o: Electrical; let m = M {{ }}(a, o); }}")
}

#[test]
fn ddx_lowers_to_its_derivative_and_is_recorded() {
    // `ddx(v(a) * v(a), v(a))` is `v(a) + v(a)` (the product rule, a factor 1 folded): the body holds
    // the derivative as an ordinary expression, and records the `ddx` and the probe it names.
    let ir = Elab::ir(&design("v(o, gnd) +> ddx(v(a, gnd) * v(a, gnd), v(a, gnd));"), "Top");
    let body = ir.bodies().find(|b| b.name() == "M").expect("the body M");
    assert_eq!(body.partial_derivatives().len(), 1);
    assert_eq!(body.partial_derivatives()[0].name, "v(a, gnd)");
    let text = ir.to_string();
    assert!(text.contains("ddx0 \"v(a, gnd)\""), "{text}");
    assert!(text.contains("(probe(a, gnd) + probe(a, gnd))"), "{text}");
    // an expression that reads no probe is the literal zero
    let zero = Elab::ir(&design("v(o, gnd) +> ddx(k * 2.0, v(a, gnd));"), "Top");
    assert!(zero.to_string().contains("potential +> direct 0.0"), "{zero}");
}

#[test]
fn ddx_through_a_fn_call_is_p0001_naming_the_form() {
    // `ddx` is symbolic over the lowered expression; a call of a `fn` has no derived function yet
    // (the elaborator adds the functions after the body), so it is refused by name.
    let src = format!("fn square(x: f64) -> f64 {{ x * x }}\n{}", design("v(o, gnd) +> ddx(square(v(a, gnd)), v(a, gnd));"));
    let err = Elab::run(&src, "Top").expect_err("a call inside ddx");
    assert_eq!(Elab::codes(&err), ["P0001"], "{:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    assert!(err[0].diagnostic.message.contains("a call of a `fn` inside a `ddx`"), "{}", err[0].diagnostic.message);
}
