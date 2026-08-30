//! LRM-COVERS: 5.8.4
//!
//! T34 (`lrm-conformance-wave3`, LRMW3-17): §5.8.4 restricts two more
//! constructs under a conditional statement, beyond §4.5.15's own analog
//! operators (T33): an event control statement (`@(cross(...))`,
//! `@(timer(...))`, ...) is illegal inside an `if`/`case` whose
//! controlling expression is not elaboration-constant, and an analog
//! *filter function* (`transition`/`slew`/`absdelay`/`laplace_*`/`zi_*`)
//! is illegal inside a *nested* conditional (an `if`/`case` whose parent
//! statement is itself an `if`/`case`) regardless of constantness at
//! either level.
//!
//! Unlike T33 (which found a real, in-tree production model conflicting
//! with §4.5.15's letter and had to carve out a documented exception),
//! neither restriction here conflicts with `headers/spice/*.va`: those
//! headers never use an event-control statement or a filter function
//! inside any `if`/`case` at all (`tests/spice_headers.rs::
//! every_spice_header_elaborates` stays green with both restrictions
//! enforced in full — see `elab/typecheck.rs::check_conditional_restrictions_stmt`).

use piperine_lang::parse_and_elaborate;

fn check(src: &str) -> Result<(), String> {
    parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).map(|_| ()).map_err(|e| format!("{e:?}"))
}

/// AC1: `@(cross(...))` under a data-dependent `if` fails loud citing
/// §5.8.4.
#[test]
fn event_control_under_a_data_dependent_if_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            real x;
            analog begin
                if (V(a) > 0.0)
                    @(cross(V(a), 0))
                        x = 1.0;
            end
        endmodule
    "#;
    let err = check(src).expect_err("@(cross(...)) under a data-dependent if must fail loud");
    assert!(err.contains("5.8.4"), "expected §5.8.4 cited, got: {err}");
}

/// AC2: a nested conditional around a filter function (`transition`)
/// fails loud, even though both controlling expressions are individually
/// elaboration-constant.
#[test]
fn nested_conditional_around_a_filter_function_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            parameter real p = 1.0;
            parameter real q = 1.0;
            analog begin
                if (p > 0.0) begin
                    if (q > 0.0)
                        V(b) <+ transition(V(a), 0, 1e-9);
                end
            end
        endmodule
    "#;
    let err = check(src).expect_err("a filter function inside a nested conditional must fail loud");
    assert!(err.contains("5.8.4"), "expected §5.8.4 cited, got: {err}");
    assert!(err.contains("transition"), "expected the offending filter function named, got: {err}");
}

/// AC3 (event control): the constant-condition case still compiles.
#[test]
fn event_control_under_a_constant_if_still_compiles() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            parameter real p = 1.0;
            real x;
            analog begin
                if (p > 0.0)
                    @(cross(V(a), 0))
                        x = 1.0;
            end
        endmodule
    "#;
    check(src).expect("a constant conditional around an event control statement must compile");
}

/// AC3 (filter function): a single (non-nested), constant conditional
/// around a filter function still compiles.
#[test]
fn single_constant_conditional_around_a_filter_function_still_compiles() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            parameter real p = 1.0;
            analog begin
                if (p > 0.0)
                    V(b) <+ transition(V(a), 0, 1e-9);
            end
        endmodule
    "#;
    check(src).expect("a single constant conditional around a filter function must compile");
}

/// A single (non-nested), *non-constant* conditional around a filter
/// function is also illegal (§5.8.4's first sentence, mirroring §4.5.15's
/// own analog-operator rule) — proves the check is not limited to the
/// nesting case alone.
#[test]
fn filter_function_under_a_single_nonconstant_conditional_fails_loud() {
    let src = r#"
        module m(inout electrical a, inout electrical b);
            analog begin
                if (V(a) > 0.0)
                    V(b) <+ transition(V(a), 0, 1e-9);
            end
        endmodule
    "#;
    let err = check(src).expect_err("a filter function under a non-constant if must fail loud");
    assert!(err.contains("5.8.4"), "expected §5.8.4 cited, got: {err}");
}
