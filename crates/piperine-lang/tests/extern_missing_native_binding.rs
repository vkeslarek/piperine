//! `extern` declaration with a missing native registry binding — distinct
//! fail-loud (declared-language-surface T13, DLS-05).
//!
//! The mechanism landed alongside T11 (`elab/resolve.rs::resolve_declared_call`):
//! a call resolving to an `extern fn`/`extern task` candidate whose name has
//! no matching entry in `math.rs`'s `MATH_FNS` table fails loud with
//! `ElabErrorKind::ExternMissingBinding` — a distinct enum variant from
//! `ElabErrorKind::Other`, the variant used for DLS-04's "no declaration at
//! all" case (`resolve_path_call`'s error, and `UnknownAttrSchema` for
//! attribute schemas). This task's job is proving the distinction with a
//! dedicated test that asserts on the variant, not just "an error occurred."

use piperine_lang::parse_str;
use piperine_lang::pom::ElabErrorKind;
use piperine_lang::SourceMap;

/// DLS-05: an `extern fn` declaration that *does* resolve (found in
/// `CallableRegistry` — not a DLS-04 "no declaration" case) but has no
/// matching `math.rs` entry fails loud with `ExternMissingBinding`, naming
/// the declaration and the missing binding.
#[test]
fn extern_fn_with_no_native_binding_fails_loud_as_extern_missing_binding() {
    let src = "
        extern fn totally_unbacked_native_fn(x: Real) -> Real;
        mod Top() {}
        digital Top {
            var y: Real = totally_unbacked_native_fn(1.0);
        }
    ";
    let err = parse_str(src)
        .expect("parse failed")
        .elaborate(&SourceMap::dummy())
        .expect_err("an extern fn with no math.rs backing must fail loud");

    assert!(
        matches!(err.kind, ElabErrorKind::ExternMissingBinding { .. }),
        "expected ElabErrorKind::ExternMissingBinding, got: {:?}",
        err.kind
    );
    let msg = err.to_string();
    assert!(msg.contains("totally_unbacked_native_fn"), "error should name the extern declaration: {msg}");
}

/// DLS-05 vs DLS-04 distinction: an undeclared name (no `CallableRegistry`
/// entry at all) does *not* produce `ExternMissingBinding` — it is either
/// left untouched (bare-identifier calls, per T11's per-category scope) or,
/// for `Type::method(...)` calls, a plain `Other`-kind "no declaration"
/// error. Either way, the two failure modes never share a variant.
#[test]
fn undeclared_path_call_is_not_extern_missing_binding() {
    let src = "
        mod Top() {}
        digital Top {
            NoSuchType::no_such_method(1.0);
        }
    ";
    let err = parse_str(src)
        .expect("parse failed")
        .elaborate(&SourceMap::dummy())
        .expect_err("a call with zero declaration anywhere must fail loud");

    assert!(
        !matches!(err.kind, ElabErrorKind::ExternMissingBinding { .. }),
        "an undeclared call (DLS-04) must not be reported as ExternMissingBinding (DLS-05): {:?}",
        err.kind
    );
}
