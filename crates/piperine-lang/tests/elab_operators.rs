//! Scope: the elaboration of the forms of `analog-operators`: `ddx` (AO-01), the delay, noise,
//! session parameter and trace forms as their lowering tasks land. Each source is a module `M`
//! placed by `Top`; the expectation is the IR the form lowers to, or the code and the form a
//! refusal names.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

/// The sources of the tests.
struct Sources;

impl Sources {
    /// The design `M` placed once, whose `analog` block is `body`.
    fn design(body: &str) -> String {
        format!("module M(a: Electrical, o: Electrical) {{ k: f64 = 2.0; }}\nanalog for M {{ {body} }}\nmodule Top() {{ signal a: Electrical; signal o: Electrical; let m = M {{ }}(a, o); }}")
    }
}

#[test]
fn ddx_lowers_to_its_derivative_and_is_recorded() {
    // `ddx(v(a) * v(a), v(a))` is `v(a) + v(a)` (the product rule, a factor 1 folded): the body holds
    // the derivative as an ordinary expression, and records the `ddx` and the probe it names.
    let ir = Elab::ir(&Sources::design("v(o, gnd) +> ddx(v(a, gnd) * v(a, gnd), v(a, gnd));"), "Top");
    let body = ir.bodies().find(|b| b.name() == "M").expect("the body M");
    assert_eq!(body.partial_derivatives().len(), 1);
    assert_eq!(body.partial_derivatives()[0].name, "v(a, gnd)");
    let text = ir.to_string();
    assert!(text.contains("ddx0 \"v(a, gnd)\""), "{text}");
    assert!(text.contains("(probe(a, gnd) + probe(a, gnd))"), "{text}");
    // an expression that reads no probe is the literal zero
    let zero = Elab::ir(&Sources::design("v(o, gnd) +> ddx(k * 2.0, v(a, gnd));"), "Top");
    assert!(zero.to_string().contains("potential +> direct 0.0"), "{zero}");
}

#[test]
fn ddx_through_a_fn_call_expands_the_call_and_differentiates_its_body() {
    // §14.2.3: `ddx(square(v(a)), v(a))` is the derivative of the body of `square` with `x` read as
    // `v(a)`: `v(a) + v(a)`, as for `v(a) * v(a)`; the body adds no IR function and no local. The argument is lowered
    // once, so the second `v(a)` of the sum is the shared probe `e0`.
    let src = format!("fn square(x: f64) -> f64 {{ x * x }}\n{}", Sources::design("v(o, gnd) +> ddx(square(v(a, gnd)), v(a, gnd));"));
    let ir = Elab::ir(&src, "Top");
    let text = ir.to_string();
    assert!(text.contains("potential +> direct (probe(a, gnd) + e0)") && text.contains("e0: f64 = probe n0 gnd"), "{ir}");
    assert_eq!(ir.functions().count(), 0, "the call is expanded, so no function is added");
}

#[test]
fn a_fn_with_statements_is_run_symbolically_inside_a_ddx() {
    // §14.2.3: an early `return` under an `if`, an assignment and a loop over constant bounds are run, the call
    // standing for one expression: no function is added, and the derivative reads the value of the branch taken.
    let early = format!("fn clipped(x: f64) -> f64 {{ if x > 1.0 {{ return 1.0; }} x }}\n{}", Sources::design("v(o, gnd) +> ddx(clipped(v(a, gnd)), v(a, gnd));"));
    let ir = Elab::ir(&early, "Top");
    assert_eq!(ir.functions().count(), 0, "the call is expanded, so no function is added");
    let looped = format!("fn cubed(x: f64) -> f64 {{ let p = 1.0; for k in 0..3 {{ p = p * x; }} p }}\n{}", Sources::design("v(o, gnd) +> ddx(cubed(v(a, gnd)), v(a, gnd));"));
    assert_eq!(Elab::ir(&looped, "Top").functions().count(), 0);
}

#[test]
fn a_fn_form_with_no_finite_symbolic_reading_is_p0001_naming_it_inside_a_ddx() {
    // GAPS L74: a loop with no constant bound, a `break`, an `if let`, a `let` of a pattern and an expression
    // statement made for its effect have no finite symbolic reading, so each is `P0001` naming the form.
    let forms = [
        ("fn f(x: f64) -> f64 { let s = x; let n = 0; while n < 3 { n += 1; } s }", "a `while` or `loop`"),
        ("fn f(x: f64) -> f64 { let s = x; for k in 0..3 { if k == 1 { break; } s += x; } s }", "a `break`, a `continue` or a nested `while`"),
        ("fn f(x: f64) -> f64 { let s = x; for k in 0..(x as u8) { s += x; } s }", "a `for` whose bounds do not fold"),
        ("fn f(x: f64) -> f64 { let o = Some(x); if let Some(g) = o { g * x } else { x } }", "an `if let`"),
        ("fn f(x: f64) -> f64 { let (a, b) = (x, x); a + b }", "a `let` of a pattern that is no name"),
    ];
    for (function, form) in forms {
        let src = format!("{function}\n{}", Sources::design("v(o, gnd) +> ddx(f(v(a, gnd)), v(a, gnd));"));
        let err = Elab::run(&src, "Top").map(|_| ()).expect_err(form);
        assert_eq!(Elab::codes(&err), ["P0001"], "{form}: {:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert!(err[0].diagnostic.message.contains(form), "{form}: {}", err[0].diagnostic.message);
    }
}

#[test]
fn ddx_against_the_flow_of_a_branch_with_no_potential_contribution_is_the_literal_zero() {
    // `i(a, gnd)` of a branch the module only gives a flow contribution is no unknown: the derivative
    // against it is 0 (§14.2.3), the same literal as for an expression that reads no probe.
    let ir = Elab::ir(&Sources::design("i(a, gnd) +> v(a, gnd) / 1k; v(o, gnd) +> ddx(3.0 * v(a, gnd), i(a, gnd));"), "Top");
    assert!(ir.to_string().contains("potential +> direct 0.0"), "{ir}");
}

#[test]
fn an_ac_stim_read_by_a_noise_density_a_delay_input_or_a_ddx_value_is_a_site_of_the_body() {
    // §14.3.1: `ac_stim` is allowed where `ddx` is, a value input among them. Each form elaborates to a
    // well-formed body (no `P0004`) whose `ac_stim` site is numbered in the body, whatever primitive reads it.
    let forms = [
        "v(o, gnd) +> noise(ac_stim(1.0) + 1e-18, \"n\");",
        "v(o, gnd) +> delay(ac_stim(1.0) + v(a, gnd), 1n);",
        "v(o, gnd) +> ddx(ac_stim(1.0) * v(a, gnd), v(a, gnd));",
        "v(o, gnd) +> ddx(ac_stim(1.0), v(a, gnd));",
        "v(o, gnd) +> k * ddt(ac_stim(1.0) + v(a, gnd)) * v(a, gnd);",
        "if ac_stim(1.0) > 0.5 { v(o, gnd) +> 1.0; } else { v(o, gnd) +> 2.0; }",
    ];
    for form in forms {
        let ir = Elab::ir(&Sources::design(form), "Top");
        let body = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert_eq!(body.stims().len(), 1, "{form}");
    }
}

#[test]
fn ddx_against_a_flow_with_no_potential_contribution_differentiates_the_expression() {
    // AO-01 AC 11 (§14.2.3): `ddx(5 i(a), i(a))` is the derivative of the expression, 5, not a literal chosen
    // for the branch; a derivative that still reads the value of that flow (`ddx(i * i, i)` is `2 i`) is `P0001`
    // naming it, since no solve reads such a flow yet.
    let five = Elab::ir(&Sources::design("i(a, gnd) +> v(a, gnd) / 1k; v(o, gnd) +> ddx(5.0 * i(a, gnd), i(a, gnd));"), "Top");
    assert!(five.to_string().contains("potential +> direct 5.0"), "{five}");
    let err = Elab::run(&Sources::design("i(a, gnd) +> v(a, gnd) / 1k; v(o, gnd) +> ddx(i(a, gnd) * i(a, gnd), i(a, gnd));"), "Top").expect_err("a derivative that reads the flow");
    assert_eq!(Elab::codes(&err), ["P0001"], "{:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    assert!(err[0].diagnostic.message.contains("reads the value of the flow of a branch with no potential contribution"), "{}", err[0].diagnostic.message);
}

#[test]
fn a_delay_without_max_whose_td_varies_is_e0452() {
    // AO-02 AC 5 (§14.6.2): without `max`, `td` is constant during an analysis: params, constants,
    // `temperature()`. A `td` that reads a probe or `now()` is `E0452` naming the placement (the
    // module); with a `max` any `td` is allowed.
    for td in ["v(a, gnd) * 1n", "now()", "freq()"] {
        let err = Elab::run(&Sources::design(&format!("v(o, gnd) +> delay(v(a, gnd), {td});")), "Top").expect_err(td);
        assert_eq!(Elab::codes(&err), ["E0452"], "{td}: {:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert!(err[0].diagnostic.message.contains("module `M`"), "{td}: {}", err[0].diagnostic.message);
    }
    for td in ["1n", "k * 1n", "temperature().unwrap_or(1.0) * 1p"] {
        let ir = Elab::ir(&Sources::design(&format!("v(o, gnd) +> delay(v(a, gnd), {td});")), "Top");
        let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert_eq!(m.delays().len(), 1, "{td}");
        assert!(m.delays()[0].maximum.is_none(), "{td}");
    }
    for max in ["1m", "Some(1m)"] {
        let ir = Elab::ir(&Sources::design(&format!("v(o, gnd) +> delay(v(a, gnd), v(a, gnd) * 1n, {max});")), "Top");
        let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
        assert!(m.delays()[0].maximum.is_some(), "{max}");
    }
    // a `max` that reads a probe is not on the parameter cone: P0001 naming it
    let err = Elab::run(&Sources::design("v(o, gnd) +> delay(v(a, gnd), 1n, v(a, gnd));"), "Top").expect_err("a max on a probe");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("a `delay` max that is not a constant or a param"), "{}", err[0].diagnostic.message);
}

#[test]
fn one_delay_call_is_one_site_however_often_it_is_read() {
    // §8.3.3: a body `let` that binds a call is one instance: one site `delay0`, read once by the
    // `let` and used twice through it.
    let ir = Elab::ir(&Sources::design("let d = delay(v(a, gnd), 1n); v(o, gnd) +> d + d;"), "Top");
    let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
    assert_eq!(m.delays().len(), 1);
    let text = ir.to_string();
    assert_eq!(text.matches("delay0.value").count(), 1, "{text}");
    assert!(text.contains("(d + d)"), "{text}");
}

#[test]
fn noise_table_waits_for_its_dependencies() {
    // AO-04 AC 3 (D17): `noise_table` is a module generic over `const N` with an array param. The generic module is
    // placed (generics T71) and its array param is bound at the hoisted placement (T110a); what waits is the table of
    // `table_model` that reads that param (generics T112); `GAPS.md` L58 holds it with the generics group (L45 closed with the
    // noise sources).
    let src = "module M(a: Electrical) { }\nanalog for M { i(a, gnd) +> noise_table::<2>(\"t\") { table: [(1k, 1e-18), (10k, 4e-18)] }; }\nmodule Top() { signal a: Electrical; let m = M(a); }";
    let err = Elab::run(src, "Top").expect_err("noise_table is refused");
    assert_eq!(Elab::codes(&err), ["P0001"], "{:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    let message = &err[0].diagnostic.message;
    assert!(message.contains("the table of a `table_model` that reads a param"), "the refusal names the construct that waits: {message}");
}

#[test]
fn simparam_lowers_to_one_session_parameter_per_name() {
    // AO-07 (§14.4, user decision 2026-11-23): `simparam("gmin")` read twice and `simparam("vth0")` are two
    // parameters of the body, one per name; each read is an `Option<f64>` unpacked where it stands.
    let ir = Elab::ir(&Sources::design("v(o, gnd) +> simparam(\"gmin\").unwrap_or(1e-12) + simparam(\"gmin\").unwrap_or(2e-12) + simparam(\"vth0\").unwrap_or(0.5);"), "Top");
    let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
    let names: Vec<&str> = m.session_parameters().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["gmin", "vth0"]);
    let text = ir.to_string();
    assert!(text.contains("simparam(\"gmin\")") && text.contains("simparam(\"vth0\")"), "{text}");
    // `match` over the option takes both arms
    let matched = Elab::ir(&Sources::design("v(o, gnd) +> match simparam(\"gain\") { Some(g) => g, None => 1.0 };"), "Top");
    assert_eq!(matched.bodies().find(|b| b.name() == "M").expect("M").session_parameters().len(), 1);
}

#[test]
fn an_emit_in_analog_lowers_to_a_trace_site() {
    // AO-09 (§19 rule 4): `emit!("conv", v = v(a))` in `analog` is a trace: one site of the body per
    // statement, its stream `conv` with the field `v`, written where it stands (here under an `if`).
    let ir = Elab::ir(&Sources::design("emit!(\"conv\", v = v(a, gnd), who = \"M\"); if v(a, gnd) > 0.5 { emit!(\"conv\", v = 1.0); } v(o, gnd) +> 0.0;"), "Top");
    let m = ir.bodies().find(|b| b.name() == "M").expect("the body M");
    assert_eq!(m.traces().len(), 2, "one site per statement");
    assert!(m.traces().iter().all(|t| t.stream == 0), "both write the stream `conv`");
    let text = ir.to_string();
    assert!(text.contains("trace0 \"conv\"") && text.contains("trace1 \"conv\""), "{text}");
    assert!(text.contains("trace0 {f0 = probe(a, gnd), who = \"M\"}"), "{text}");
}

#[test]
fn a_field_type_conflict_across_trace_sites_is_e0605() {
    // AO-09 AC 5 (§19 rule 12): two sites of one stream give a field one type: `v` is an `f64` at the
    // first and a `bool` at the second.
    let err = Elab::run(&Sources::design("emit!(\"conv\", v = v(a, gnd)); emit!(\"conv\", v = true);"), "Top").expect_err("a field of two types");
    assert!(Elab::codes(&err).iter().any(|c| c == "E0605"), "{:?}", err.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
}

#[test]
fn a_definition_or_an_at_argument_reaching_freq_through_a_fn_is_refused_by_the_checker() {
    // §8.2.5 and §13 rule 11 (GAPS §4.7): a definition and an `at` argument are discrete expressions, so one that
    // reads `freq()` through a `fn` is `E0336` and `E0442`, errors of the language, not a gap; the IR's
    // ambient-reads check is the backstop behind them.
    let definition = "fn level() -> f64 { 1.0 + freq() }\nmodule R(out q: f64 = 0.0) { q = level(); }\nmodule Top() { signal q: f64; let r = R(q); }";
    let err = Elab::run(definition, "Top").expect_err("a definition reading freq");
    assert!(Elab::codes(&err).iter().any(|c| c == "E0336"), "{:?}", Elab::codes(&err));
    let at = "fn when() -> f64 { 1.0e-9 + freq() }\nmodule R(out q: Bit<f64> = 0.0) { }\ndigital for R { on at(when()) { q <- 1.0; } }\nmodule Top() { signal q: Bit<f64>; let r = R(q); }";
    let err = Elab::run(at, "Top").expect_err("an at argument reading freq");
    assert!(Elab::codes(&err).iter().any(|c| c == "E0442"), "{:?}", Elab::codes(&err));
}
