//! Elaboration-constant-evaluator function calls (`elab::const_eval::
//! ConstEnv::eval`'s `Expr::Call` arm) — the second T43 `urc.phdl`
//! migration blocker: `pow(k, i)`-style per-segment overrides
//! (`headers/spice/urc.phdl`'s `urc[N]`/`urc_taper_5_k2`/
//! `urc_isperl_5_k2`) failed with `NotConst` before this fix because no
//! call form was supported at all.
//!
//! Scope boundary (documented, not silently under-delivered): only the
//! built-in math functions (`crate::math::MATH_FNS`, e.g. `pow`) are
//! evaluable in constant context, reusing the same libm wrappers the JIT
//! links against. A call to a user-declared `analog function`/`function`
//! is not supported here and fails loud naming the call — evaluating a
//! user function body would need its own recursive interpreter (see
//! `const_eval.rs`'s `eval_call` doc comment for the full boundary
//! rationale).

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

#[test]
fn pow_of_two_literals_evaluates_as_a_param_default() {
    // urc_taper_5_k2's exact shape: `1000.0 * pow(2.0, 0.0) / 31.0`.
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1000.0 * pow(2.0, 3.0) / 31.0;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let expected = 1000.0 * 2.0_f64.powf(3.0) / 31.0;
    assert_eq!(m.params()[0].default, Some(piperine_lang::pom::Value::Real(expected)));
}

#[test]
fn pow_of_a_forwarded_param_and_a_generate_style_index_evaluates_in_an_instance_override() {
    // urc[N]'s exact shape: `.r = r * pow(k, i)` inside a sub-instance
    // override, where both `r`/`k` are the enclosing module's own params
    // (bug 1) and the exponent is a plain integer literal here (bug 2 in
    // isolation — genvar-index interaction is T25's own scope).
    let d = elab(
        r#"
        module leaf (inout electrical p, inout electrical n);
            parameter real r = 1.0;
        endmodule

        module mid (inout electrical p, inout electrical n);
            parameter real r = 100.0;
            parameter real k = 1.5;
            leaf #(.r(r * pow(k, 2.0))) l1 (p, n);
        endmodule
        "#,
    );
    let m = d.module("mid").expect("mid elaborated");
    let l1 = m.instances().iter().find(|i| i.name() == "l1").expect("l1");
    let expected = 100.0 * 1.5_f64.powf(2.0);
    assert_eq!(l1.params, vec![("r".to_string(), piperine_lang::pom::Value::Real(expected))]);
}

#[test]
fn a_call_to_an_undeclared_or_user_function_fails_loud_naming_it() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = my_custom_fn(1.0);
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("my_custom_fn"), "names the unsupported call: {msg}");
}
