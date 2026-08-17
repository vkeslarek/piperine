//! `$param_given` accepts the LRM's own bare-identifier spelling as well as
//! the string-literal form, and the two are equivalent on **both** axes that
//! matter.
//!
//! LRM §9.19's own worked example writes the bare form —
//! `if ($param_given(coeff1) && coeff1 != 0.0)` — while this codebase
//! originally matched only `Expr::Str`, so standard-conformant source failed
//! to compile with `unresolved param \`?\``. Piperine is a **superset** of
//! Verilog-AMS, never a subset: LRM-conformant source must always compile.
//!
//! The second axis is the one that would fail silently. `$param_given` does
//! not only produce a value — scanning for it is what marks a parameter
//! **rebuild-class** (`kernel/analog/compile.rs`'s presence mask), because a
//! write that changes a parameter's *presence* changes which analog code
//! exists and so cannot be a cheap restamp (MD-18). Had only the emission
//! learned the bare spelling and not the mask scan, such a parameter would be
//! misclassified as restamp-class: a live `set` would skip the rebuild and
//! quietly produce results from stale compiled code. Hence both spellings are
//! asserted against the classification, not just against the value.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_lang::parse_and_elaborate;

/// Compiles a one-module device whose body queries `popt`'s presence with
/// the given spelling, alongside a plain restamp-class `g`.
fn kernel_querying(presence_arg: &str) -> AnalogKernel {
    let src = format!(
        "module Dut(inout electrical p, inout electrical n);
    parameter real popt = 0.0;
    parameter real g = 1e-3;
    analog begin
        I(p, n) <+ ($param_given({presence_arg}) ? popt : 0.0) * V(p, n) + g * V(p, n);
    end
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("`$param_given({presence_arg})` must elaborate: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    AnalogKernel::compile(&bodies["Dut"]).expect("compile")
}

fn param_index(kernel: &AnalogKernel, name: &str) -> usize {
    kernel.param_names().iter().position(|n| n == name).unwrap_or_else(|| {
        panic!("param `{name}` must exist, got {:?}", kernel.param_names())
    })
}

/// The LRM's bare-identifier spelling compiles at all — the regression that
/// motivated this: it previously failed with `unresolved param `?``.
#[test]
fn the_lrm_bare_identifier_spelling_compiles() {
    let kernel = kernel_querying("popt");
    assert!(
        kernel.param_names().iter().any(|n| n == "popt"),
        "the presence-queried param must still be a parameter of the kernel"
    );
}

/// The string-literal spelling keeps working — this form was already
/// accepted and must not regress.
#[test]
fn the_string_literal_spelling_still_compiles() {
    let kernel = kernel_querying("\"popt\"");
    assert!(kernel.param_names().iter().any(|n| n == "popt"));
}

/// **The silent-failure guard.** A presence-queried parameter is
/// rebuild-class and therefore carries no analytic derivative; a plain one
/// is restamp-class and does. Both spellings must produce the *same*
/// classification — if the bare form were missed by the presence-mask scan,
/// `popt` would wrongly appear in the parameter Jacobian here.
#[test]
fn both_spellings_classify_the_param_as_rebuild_class() {
    for spelling in ["popt", "\"popt\""] {
        let kernel = kernel_querying(spelling);
        let popt = param_index(&kernel, "popt");
        let g = param_index(&kernel, "g");
        let analytic = kernel.param_jacobian_indices();

        assert!(
            !analytic.contains(&popt),
            "`$param_given({spelling})` must mark `popt` rebuild-class, so it carries \
             no analytic derivative — got jacobian indices {analytic:?}"
        );
        assert!(
            analytic.contains(&g),
            "the plain restamp-class `g` must still be differentiable with spelling \
             {spelling} — got {analytic:?}"
        );
    }
}

/// The two spellings are interchangeable: same parameter set, same
/// classification. Asserted as one equality so a future divergence in
/// either direction fails here rather than only in one of the tests above.
#[test]
fn the_two_spellings_are_equivalent() {
    let bare = kernel_querying("popt");
    let quoted = kernel_querying("\"popt\"");

    assert_eq!(
        bare.param_names(),
        quoted.param_names(),
        "both spellings must yield the same parameter set"
    );
    assert_eq!(
        bare.param_jacobian_indices(),
        quoted.param_jacobian_indices(),
        "both spellings must yield the same rebuild/restamp classification"
    );
}
