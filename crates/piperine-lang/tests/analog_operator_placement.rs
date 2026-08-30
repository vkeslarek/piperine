//! LRM-COVERS: 4.5.15
//!
//! T33 (`lrm-conformance-wave3`, LRMW3-17): §4.5.15 restricts where an
//! analog operator (`ddt`, `idt`, `transition`, `laplace_*`, ...) may
//! appear — it "maintain[s] ... internal state" that must be evaluated
//! every simulator iteration to stay valid. This file covers three of the
//! four restrictions `elab/typecheck.rs::check_analog_operator_placement_stmt`
//! enforces: an event-triggered statement, a `repeat`/`while`/non-genvar
//! `for` loop, and a user-defined function body. `ddx` (§4.5.15's own
//! stated exception — "does not require an internal state to be
//! maintained") is exempt from all three.
//!
//! **Disclosed scope (SPEC_DEVIATION, documented at length in
//! `typecheck.rs` itself)**: the LRM's first bullet — an analog operator
//! under an `if`/`case`/`?:` whose controlling expression is not
//! elaboration-constant — is **not** enforced. `headers/spice/mos.va`'s
//! own mode-dependent transcapacitance charge equations
//! (`if (mode == 1.0) begin I(g,sp) <+ ... + ddt(V(g,sp)); ... end`,
//! `mode` a `real` derived from `vds`, genuinely non-constant) use exactly
//! this pattern — the universal, industry-standard drain/source symmetry
//! idiom every real SPICE-derived MOSFET compact model needs.
//! `tests/spice_headers.rs::every_spice_header_elaborates` proves this
//! elaborates today; enforcing the letter of that bullet breaks a frozen,
//! real production model, not a contrived fixture — strong evidence that
//! real Verilog-A compilers do not enforce it as a hard error either.
//! `limexp` is separately exempted from every bullet this file *does*
//! enforce for the identical reason (`diode.va`/`mos.va`/`urc.va`'s
//! "guard the exponential near a convergence-sensitive threshold" idiom).

use piperine_lang::parse_and_elaborate;

fn check(src: &str) -> Result<(), String> {
    parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).map(|_| ()).map_err(|e| format!("{e:?}"))
}

/// AC2 (event-triggered statement): an analog operator inside `@(...)`
/// fails loud citing §4.5.15.
#[test]
fn analog_operator_inside_an_event_triggered_statement_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            analog begin
                @(cross(V(a), 0))
                    V(b) <+ ddt(V(a));
            end
        endmodule
    "#;
    let err = check(src).expect_err("ddt inside @(cross(...)) must fail loud");
    assert!(err.contains("4.5.15"), "expected §4.5.15 cited, got: {err}");
    assert!(err.contains("ddt"), "expected the offending operator named, got: {err}");
}

/// AC2 (loop): an analog operator inside a `while` loop fails loud.
#[test]
fn analog_operator_inside_a_while_loop_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            integer i;
            analog begin
                i = 0;
                while (i < 3) begin
                    V(b) <+ ddt(V(a));
                    i = i + 1;
                end
            end
        endmodule
    "#;
    let err = check(src).expect_err("ddt inside a while loop must fail loud");
    assert!(err.contains("4.5.15"), "expected §4.5.15 cited, got: {err}");
}

/// AC2 (loop): an analog operator inside a `repeat` loop fails loud.
#[test]
fn analog_operator_inside_a_repeat_loop_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            analog begin
                repeat (3)
                    V(b) <+ ddt(V(a));
            end
        endmodule
    "#;
    let err = check(src).expect_err("ddt inside a repeat loop must fail loud");
    assert!(err.contains("4.5.15"), "expected §4.5.15 cited, got: {err}");
}

/// AC2 (loop): an analog operator inside a non-genvar `for` loop fails
/// loud, but a genvar `for` loop (elaboration-unrolled, never a runtime
/// loop) is exempt.
#[test]
fn analog_operator_inside_a_non_genvar_for_loop_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            integer i;
            analog begin
                for (i = 0; i < 3; i = i + 1)
                    V(b) <+ ddt(V(a));
            end
        endmodule
    "#;
    let err = check(src).expect_err("ddt inside a non-genvar for loop must fail loud");
    assert!(err.contains("4.5.15"), "expected §4.5.15 cited, got: {err}");
}

#[test]
fn analog_operator_inside_a_genvar_for_loop_still_compiles() {
    let src = r#"
        module m();
            genvar i;
            electrical bus[0:2];
            analog begin
                for (i = 0; i < 3; i = i + 1)
                    V(bus[i]) <+ ddt(V(bus[i]));
            end
        endmodule
    "#;
    check(src).expect("a genvar for loop is elaboration-unrolled, not a runtime loop");
}

/// AC4 (user-defined function): an analog operator inside a
/// `analog function`'s body fails loud, even though the *call site* is a
/// perfectly ordinary, unconditional analog contribution.
#[test]
fn analog_operator_inside_a_user_defined_function_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            analog function real f;
                input x;
                real x;
                begin
                    f = ddt(x);
                end
            endfunction
            analog V(b) <+ f(V(a));
        endmodule
    "#;
    let err = check(src).expect_err("ddt inside a user-defined function body must fail loud");
    assert!(err.contains("4.5.15"), "expected §4.5.15 cited, got: {err}");
}

/// AC3: `ddx` is exempt from every restriction this file enforces — a
/// loop, event-trigger, and user function all still compile with `ddx`
/// inside.
#[test]
fn ddx_is_exempt_from_every_placement_restriction() {
    let src_loop = r#"
        module m(inout electrical a, inout electrical b);
            integer i;
            analog begin
                i = 0;
                while (i < 3) begin
                    V(b) <+ ddx(V(a), V(a));
                    i = i + 1;
                end
            end
        endmodule
    "#;
    check(src_loop).expect("ddx inside a loop must still compile");

    let src_event = r#"
        module m(inout electrical a, inout electrical b);
            analog begin
                @(cross(V(a), 0))
                    V(b) <+ ddx(V(a), V(a));
            end
        endmodule
    "#;
    check(src_event).expect("ddx inside an event-triggered statement must still compile");

    let src_fn = r#"
        module m(inout electrical a, inout electrical b);
            analog function real f;
                input x;
                real x;
                begin
                    f = ddx(x, x);
                end
            endfunction
            analog V(b) <+ f(V(a));
        endmodule
    "#;
    check(src_fn).expect("ddx inside a user-defined function must still compile");
}

/// The legal case (Done-when bullet 2): a construct that is NOT one of
/// the four restricted placements — an ordinary, unconditional
/// contribution — still compiles with `ddt` inside it. Proves the check
/// is not over-broad (it does not reject `ddt` everywhere).
#[test]
fn analog_operator_in_an_ordinary_unconditional_contribution_still_compiles() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            analog V(b) <+ ddt(V(a));
        endmodule
    "#;
    check(src).expect("an ordinary unconditional ddt contribution must compile");
}
