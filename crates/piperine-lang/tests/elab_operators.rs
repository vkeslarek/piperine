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

#[test]
fn a_delay_without_max_whose_td_varies_is_e0452() {
    // AO-02 AC 5 (§14.6.2): without `max`, `td` is constant during an analysis: params, constants,
    // `temperature()`. A `td` that reads a probe or `now()` is `E0452` naming the placement (the
    // module); with a `max` any `td` is allowed.
    for td in ["v(a, gnd) * 1n", "now()", "freq()"] {
        let err = Elab::run(&design(&format!("v(o, gnd) +> delay(v(a, gnd), {td});")), "Top").expect_err(td);
        assert_eq!(Elab::codes(&err), ["E0452"], "{td}: {:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert!(err[0].diagnostic.message.contains("module `M`"), "{td}: {}", err[0].diagnostic.message);
    }
    for td in ["1n", "k * 1n", "temperature() * 1p"] {
        let ir = Elab::ir(&design(&format!("v(o, gnd) +> delay(v(a, gnd), {td});")), "Top");
        let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert_eq!(m.delays().len(), 1, "{td}");
        assert!(m.delays()[0].maximum.is_none(), "{td}");
    }
    for max in ["1m", "Some(1m)"] {
        let ir = Elab::ir(&design(&format!("v(o, gnd) +> delay(v(a, gnd), v(a, gnd) * 1n, {max});")), "Top");
        let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert!(m.delays()[0].maximum.is_some(), "{max}");
    }
    // a `max` that reads a probe is not on the parameter cone: P0001 naming it
    let err = Elab::run(&design("v(o, gnd) +> delay(v(a, gnd), 1n, v(a, gnd));"), "Top").expect_err("a max on a probe");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("a `delay` max that is not a constant or a param"), "{}", err[0].diagnostic.message);
}

#[test]
fn one_delay_call_is_one_site_however_often_it_is_read() {
    // §8.3.3: a body `let` that binds a call is one instance: one site `delay0`, read once by the
    // `let` and used twice through it.
    let ir = Elab::ir(&design("let d = delay(v(a, gnd), 1n); v(o, gnd) +> d + d;"), "Top");
    let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
    assert_eq!(m.delays().len(), 1);
    let text = ir.to_string();
    assert_eq!(text.matches("delay0.value").count(), 1, "{text}");
    assert!(text.contains("(d + d)"), "{text}");
}
