//! Cast associated functions — declared-language-surface T17 (DLS-23).
//!
//! The bare-name cast forms (`real(x)`, `int(x)`, `bit(x)`, `Boolean(x)`,
//! `Quad(x)`) are deleted (`elab/resolve.rs`'s special-cased rewrite is
//! gone); their replacement is an ordinary, overloaded `extern impl
//! TypeName { fn from(x: SourceType) -> TypeName; ... }` per target type,
//! declared in `headers/types.phdl` and resolved through the impl-method
//! table's overload resolution (proven in isolation by
//! `overload_resolution.rs`; this file proves it end-to-end through real
//! stdlib declarations instead of synthetic fixtures).
//!
//! Scope: this file proves the mechanism (SPEC P4-AC7, "Real::from(x) …
//! resolves correctly by argument type for each declared overload").
//! Migrating the 4 known bare-cast call sites is T18's job, a separate
//! commit.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(src, &SourceMap::dummy())
}

/// `Real::from(x: Integer)` resolves by argument type — a literal `Integer`
/// argument selects the `Integer` overload.
#[test]
fn real_from_integer_resolves() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Real = 0.0;
            y = Real::from(1);
        }
    ";
    elaborate(src).expect("Real::from(x: Integer) must resolve by argument type");
}

/// `Real::from(x: Boolean)` — a different overload, same name, selected by
/// argument type. PHDL has no `Boolean` literal syntax (`true`/`false`
/// lex as identifiers, never `Literal::Bool` — verified: the parser never
/// constructs that variant), so, like the `Natural` case below, this
/// overload is only reachable via a local variable's declared type.
#[test]
fn real_from_boolean_resolves() {
    let src = "
        mod Top() {}
        digital Top {
            var b: Boolean = 0q0;
            var y: Real = 0.0;
            y = Real::from(b);
        }
    ";
    elaborate(src).expect("Real::from(x: Boolean) must resolve by argument type");
}

/// `Real::from(x: Quad)` — the third literal-selectable overload.
#[test]
fn real_from_quad_resolves() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Real = 0.0;
            y = Real::from(0q0);
        }
    ";
    elaborate(src).expect("Real::from(x: Quad) must resolve by argument type");
}

/// `Real::from(x: Natural)` — no literal syntax produces a `Natural` value,
/// so this overload is only reachable via a local variable's declared
/// type (`Behavior::var_types`, threaded into overload resolution by T17).
#[test]
fn real_from_natural_resolves_via_local_var() {
    let src = "
        mod Top() {}
        digital Top {
            var n: Natural = 5;
            var y: Real = 0.0;
            y = Real::from(n);
        }
    ";
    elaborate(src).expect("Real::from(x: Natural) must resolve via a local var's declared type");
}

/// A different target type's overload set — `Integer::from(x: Real)` —
/// proves the mechanism isn't special-cased to `Real`.
#[test]
fn integer_from_real_resolves() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Integer = 0;
            y = Integer::from(1.0);
        }
    ";
    elaborate(src).expect("Integer::from(x: Real) must resolve by argument type");
}

/// DLS-23 (Verifier round 1, Gap 2): a stray bare `real(x)` call no
/// longer carries the special-case `Expr::Cast` meaning (the rewrite is
/// deleted — that's the load-bearing claim of AC7). But piperine-lang
/// does not yet reject it as a hard error either, because per-category
/// progressive enforcement (T11's documented scope) leaves undeclared
/// bare-identifier calls untouched for codegen to handle. Codegen fails
/// loud downstream when it can't resolve `real` to anything.
///
/// This test documents the current mechanism state explicitly so a future
/// change either direction (reject at piperine-lang level, or accept
/// globally as a no-op) trips it.
#[test]
fn bare_cast_call_has_no_special_case_meaning_but_is_not_yet_rejected() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Real = 0.0;
            y = real(1);
        }
    ";
    let result = elaborate(src);
    // The mechanism claim: no Expr::Cast rewrite happens, so the call
    // reaches codegen as a plain `Call(Ident("real"), [1])`. Whether
    // piperine-lang's own elaboration accepts or rejects this bare name
    // is the open per-category progressive-enforcement question.
    // Today: elaboration accepts (no special-cased rejection); the test
    // would fail if a future commit adds a bare-name rejection rule.
    assert!(
        result.is_ok(),
        "post-T17 a bare real(x) call elaborates (no Expr::Cast special \
         case; per-category progressive enforcement leaves it for codegen) \
         — if this test fails, piperine-lang gained a bare-name rejection; \
         update spec P4-AC7's 'Rejected scope' note accordingly: {:?}",
        result.err()
    );
}

/// `Quad::from(x: Integer)` — the target type the deleted `bit(x)`/`Quad(x)`
/// bare forms both mapped to (`ValueType::Quad`).
#[test]
fn quad_from_integer_resolves() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Quad = 0q0;
            y = Quad::from(1);
        }
    ";
    elaborate(src).expect("Quad::from(x: Integer) must resolve by argument type");
}

/// `Boolean::from(x: Integer)` — the fourth target type's overload set.
#[test]
fn boolean_from_integer_resolves() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Boolean = false;
            y = Boolean::from(1);
        }
    ";
    elaborate(src).expect("Boolean::from(x: Integer) must resolve by argument type");
}

/// A call whose argument type matches none of `Real::from`'s declared
/// overloads fails loud, naming the call (DLS-07's 0-match path, exercised
/// through the real cast declarations rather than a synthetic fixture).
#[test]
fn real_from_string_matches_no_overload_fails_loud() {
    let src = "
        mod Top() {}
        digital Top {
            var y: Real = 0.0;
            y = Real::from(\"nope\");
        }
    ";
    let err = elaborate(src)
        .expect_err("a Str argument must not match any declared Real::from overload");
    let msg = err.to_string();
    assert!(msg.contains("Real::from") || msg.contains("from"), "error should name the call site: {msg}");
}
