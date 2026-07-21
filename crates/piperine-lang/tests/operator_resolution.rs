//! Declared-language-surface DLS-20: `resolve_operator_call`'s positive
//! resolution path. The Verifier's discrimination-sensor round 1 (post-T27)
//! found that this code path was dead from a test-coverage perspective —
//! commenting out `resolve.rs::resolve_operator_call`'s entire body still
//! let the whole 666-test workspace pass. Root cause: `ExternOperatorDecl`
//! was registered without `param_types`, so `validate_call` always
//! succeeded permissively (the `CallableDef` default), regardless of arity.
//!
//! Fixed post-T27 by giving `ExternOperatorDecl` real `param_types` (same
//! shape `ExternFnDecl` already had), so operator arity/type mismatches
//! now fail loud at elaboration. The tests below prove both directions:
//!
//! - Positive: a correctly-typed operator call elaborates cleanly through
//!   the new lookup path.
//! - Negative (arity mismatch): an N-arg call against a 1-arg `extern
//!   operator` declaration fails loud, naming the operator and its
//!   declared signature. This is the test that kills the Verifier's
//!   surviving mutant — replacing `resolve_operator_call`'s body with
//!   `Ok(())` lets the wrong-arity call elaborate silently.
//!
//! Scope (DLS-20 + EC2): only the eight `Expr::Call`-shaped operators
//! (`ddt`/`idt`/`ddx`/`delay`/`transition`/`slew`/`white_noise`/
//! `flicker_noise`) reach this path. The three `EventSpec::Named`-shaped
//! operators (`cross`/`above`/`timer`) live in a separate grammar
//! construct (`@ above(x) { ... }`) resolved by `elab/event.rs`'s
//! `EventRegistry`; `$limit` lexes as `Expr::SysCall` and has no
//! `piperine-lang`-level existence check today (T20/T22's documented
//! scope note). All three are declared in `headers/operators.phdl` for
//! textual/LSP visibility only.

use piperine_lang::{SourceMap, parse_and_elaborate};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(src, &SourceMap::dummy())
}

/// DLS-20 positive: `ddt(...)` with a literal Real argument (so argument
/// type inference succeeds) resolves cleanly through `OperatorRegistry`'s
/// single-candidate path. Codegen still emits the same `ddt` companion
/// model downstream (unchanged by this feature — DLS-20 only moves the
/// name's existence check upstream).
#[test]
fn ddt_call_with_correct_arity_and_type_elaborates() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Top ( inout p : Electrical ) { }
        analog Top {
            I(p) <+ ddt(1.0);
        }
    ";
    let design = elaborate(src).expect("a correctly-typed ddt call must elaborate");
    assert!(design.module("Top").is_some());
}

/// DLS-20 negative (kills the Verifier's surviving mutant): `ddt(1.0, 2.0)`
/// has arity 2 against the declared `extern operator ddt(x: Real) -> Real;`
/// (arity 1, `headers/operators.phdl:18`). With the fix in
/// `ExternOperatorDecl::param_types`, `validate_call` now reports the
/// mismatch; without it (or if `resolve_operator_call`'s body is replaced
/// with `Ok(())`), this call would elaborate silently.
#[test]
fn ddt_call_with_wrong_arity_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Top ( inout p : Electrical ) { }
        analog Top {
            I(p) <+ ddt(1.0, 2.0);
        }
    ";
    let err = elaborate(src).expect_err("a wrong-arity ddt call must fail loud");
    let msg = err.to_string();
    assert!(
        msg.contains("ddt") && msg.contains("Real"),
        "error must name the operator and its declared signature: {msg}"
    );
}

/// DLS-20 negative (type mismatch): `ddt(b)` where `b: Boolean` has the
/// right arity but the wrong argument type — Boolean where `extern
/// operator ddt(x: Real) -> Real;` requires Real. Same `validate_call`
/// path, different fault shape (type instead of arity).
///
/// Uses a local var because PHDL has no `Boolean` literal syntax (`true`/
/// `false` lex as identifiers, never `Literal::Bool` — verified by
/// `cast_impl_methods.rs:38-41`'s note: the parser never constructs that
/// variant). A local `var b: Boolean = 0q0;` is the minimum PHDL whose
/// `b` reference has a known type via `Behavior::var_types`.
#[test]
fn ddt_call_with_wrong_arg_type_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Top ( inout p : Electrical ) { }
        analog Top {
            var b: Boolean = 0q0;
            I(p) <+ ddt(b);
        }
    ";
    let err = elaborate(src).expect_err("a wrong-type ddt call must fail loud");
    let msg = err.to_string();
    assert!(
        msg.contains("ddt") && msg.contains("Boolean"),
        "error must name the operator and the wrong type: {msg}"
    );
}

/// DLS-20 multi-arg path: `white_noise(pwr)` is a one-arg operator (per
/// `headers/operators.phdl:24`) and elaborates cleanly with a literal
/// argument — exercises a second operator name to prove the registry
/// covers more than just `ddt`.
#[test]
fn white_noise_call_with_correct_arity_elaborates() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Top ( inout p : Electrical ) { }
        analog Top {
            I(p) <+ white_noise(1.0e-21);
        }
    ";
    let design = elaborate(src).expect("a correctly-typed white_noise call must elaborate");
    assert!(design.module("Top").is_some());
}

/// DLS-20 multi-arg negative: `flicker_noise(1.0)` has arity 1 against
/// `extern operator flicker_noise(pwr: Real, exponent: Real) -> Real;`
/// (arity 2). Proves the same `validate_call` path also catches arity
/// mismatch for the multi-arg operators, not just `ddt`.
#[test]
fn flicker_noise_call_with_wrong_arity_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Top ( inout p : Electrical ) { }
        analog Top {
            I(p) <+ flicker_noise(1.0e-21);
        }
    ";
    let err = elaborate(src).expect_err("a wrong-arity flicker_noise call must fail loud");
    let msg = err.to_string();
    assert!(
        msg.contains("flicker_noise"),
        "error must name the operator: {msg}"
    );
}
