//! Declared-first, fail-loud call resolution (declared-language-surface
//! T11, DLS-02/03/04). Small, self-contained PHDL fixtures only. At the
//! time T11 landed, no real stdlib header declared any `extern fn`/`extern
//! impl` yet, so the DLS-03 fixtures below locally re-declared
//! `extern fn sin(x: Real) -> Real;` themselves to test in isolation; T19
//! (DLS-18) has since made `sin` a real, globally-declared `extern fn`
//! (`headers/math.phdl`, auto-loaded into every compilation unit's
//! prelude) — re-declaring it locally would now collide (two structurally
//! identical `sin` candidates, an ambiguous-overload error instead of the
//! intended DLS-03 assertion), so the fixtures below call the real,
//! globally-declared `sin` directly instead of re-declaring it.
//!
//! Scope proven here:
//! - A plain `fn` call resolves exactly as before this task (DLS-02).
//! - An `extern fn` call dispatches with its signature validated (DLS-03).
//! - A `Type::method(...)` call with no `impl`/`extern impl` declaration
//!   anywhere fails loud, naming the call site (DLS-04) — the safe,
//!   currently-unused-in-production surface this task enforces immediately.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(src, &SourceMap::dummy())
}

/// DLS-02: a plain-declaration call resolves exactly as before this task —
/// no new behavior, no new failure mode introduced for ordinary `fn` calls.
#[test]
fn plain_fn_call_resolves_unchanged() {
    let src = "
        fn double(x: Real) -> Real { return x * 2.0; }
        mod Top() {}
        digital Top {
            var y: Real = double(3.0);
        }
    ";
    let design = elaborate(src).expect("a plain fn call must still elaborate cleanly");
    assert!(design.module("Top").is_some());
}

/// DLS-03: an `extern fn` call dispatches with its declared signature
/// validated against the call site — a matching call site elaborates
/// cleanly. Uses the real, globally-declared `sin` (`headers/math.phdl`,
/// DLS-18) rather than a local re-declaration (see module doc) — it's
/// backed by `math.rs`'s `MATH_FNS` table, so it also demonstrates a real,
/// existing native binding, not just DLS-05's missing-binding path.
#[test]
fn extern_fn_call_with_matching_signature_and_native_binding_resolves() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Real = sin(1.0);
        }
    ";
    let design = elaborate(src).expect("extern fn call matching its declared signature must resolve");
    assert!(design.module("Top").is_some());
}

/// DLS-03 (negative half): an `extern fn` call whose argument type doesn't
/// match the declared signature is a normal type/arity error — `extern`
/// does not weaken argument checking (spec Edge Cases). Uses the real,
/// globally-declared `sin` (see module doc) — a single candidate, so this
/// still exercises DLS-03's `validate_call` mismatch path directly rather
/// than DLS-07's separate 0-match-overload path.
#[test]
fn extern_fn_call_with_mismatched_signature_fails_loud() {
    let src = "
        mod Top() {}
        digital Top {
            var z: Real = sin(\"nope\");
        }
    ";
    let err = elaborate(src).expect_err("a Boolean argument must not match `sin(x: Real)`'s declared signature");
    let msg = err.to_string();
    assert!(msg.contains("sin"), "error should name the call: {msg}");
}

/// DLS-04: a `Type::method(...)` call to a type/method pair with no
/// declaration anywhere (plain or `extern`) fails loud, naming the
/// identifier and use site.
#[test]
fn path_call_to_undeclared_impl_method_fails_loud() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Real = 1.0;
            Widget::make(y);
        }
    ";
    let err = elaborate(src).expect_err("Widget::make has no declaration anywhere and must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("Widget"), "error should name the type: {msg}");
    assert!(msg.contains("make"), "error should name the method: {msg}");
}

/// DLS-01/03: a `Type::method(...)` call *does* resolve once an `extern
/// impl` declaration provides it — the positive counterpart to the test
/// above, proving the impl-method table (T10) is a real, working consumer
/// now, not just declared-but-unused scaffolding.
#[test]
fn path_call_to_declared_extern_impl_method_resolves() {
    let src = "
        extern type Widget;
        extern impl Widget {
            fn make(x: Real) -> Widget;
        }
        mod Top() {}
        digital Top {
            Widget::make(1.0);
        }
    ";
    let design = elaborate(src).expect("Widget::make must resolve once declared via extern impl");
    assert!(design.module("Top").is_some());
}

/// DLS-07 (first real consumer via the impl-method table): two `extern
/// impl` methods with the same name and different single-param types both
/// resolve correctly by call-site argument type — the cast use case's
/// mechanism, proven end-to-end through elaboration rather than the
/// registry alone (`overload_resolution.rs` already proves the algorithm
/// in isolation).
#[test]
fn path_call_overload_resolves_by_argument_type() {
    let src = "
        extern type Widget;
        extern impl Widget {
            fn make(x: Real) -> Widget;
            fn make(x: Quad) -> Widget;
        }
        mod Top() {}
        digital Top {
            Widget::make(1.0);
            Widget::make(0q0);
        }
    ";
    let design = elaborate(src).expect("overloaded Widget::make must resolve by argument type");
    assert!(design.module("Top").is_some());
}

/// DLS-07 (0-match half, exercised end-to-end): a call whose argument type
/// matches neither overload fails loud, naming the call site.
#[test]
fn path_call_overload_with_no_matching_candidate_fails_loud() {
    let src = "
        extern type Widget;
        extern impl Widget {
            fn make(x: Real) -> Widget;
            fn make(x: Quad) -> Widget;
        }
        mod Top() {}
        digital Top {
            Widget::make(\"nope\");
        }
    ";
    let err = elaborate(src).expect_err("a Str argument must not match either Widget::make overload");
    let msg = err.to_string();
    assert!(msg.contains("Widget::make") || msg.contains("make"), "error should name the call site: {msg}");
}
