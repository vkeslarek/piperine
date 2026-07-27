//! dv-core T4 (DVC-02) — `tol` elaboration: the distribution resolves
//! against `headers/statistics.phdl`, its arguments evaluate as elaboration
//! constants in module param scope, and the POM `Param` exposes it through
//! `Param.distribution()`. `tol` on a non-`Real` param and an undeclared
//! distribution name both fail loud.

use piperine_lang::value::Value;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &SourceMap::dummy())
}

fn real_arg(args: &[(String, Value)], name: &str) -> f64 {
    let Some((_, Value::Real(v))) = args.iter().find(|(n, _)| n == name) else {
        panic!("argument `{name}` must be an evaluated Real: {args:?}");
    };
    *v
}

/// DVC-02 AC1: a `tol` clause elaborates and the POM `Param` exposes the
/// distribution — kind, evaluated args, and the mismatch (no `global`)
/// marker.
#[test]
fn distribution_is_exposed_on_the_pom_param() {
    let design = elaborate(
        "mod M ( inout p : Electrical ) {
            param vto : Real = 0.7 tol gauss(sigma = 0.005);
            param r : Real = 1k;
        }",
    )
    .expect("a tol clause on a Real param must elaborate");
    let dist = design
        .module("M")
        .and_then(|m| m.param("vto"))
        .and_then(|p| p.distribution())
        .expect("vto must carry a distribution");
    assert_eq!(dist.kind, "gauss");
    assert!(!dist.global, "no `global` keyword means mismatch variation");
    assert_eq!(real_arg(&dist.args, "sigma"), 0.005);

    // A param without a `tol` clause exposes no distribution.
    let plain = design
        .module("M")
        .and_then(|m| m.param("r"))
        .expect("r is a param");
    assert!(plain.distribution().is_none(), "no tol clause means no distribution");
}

/// DVC-02 AC1: the `global` marker elaborates as process variation.
#[test]
fn global_marker_means_process_variation() {
    let design = elaborate(
        "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol uniform(rel = 0.01) global; }",
    )
    .expect("a global tol clause must elaborate");
    let dist = design
        .module("M")
        .and_then(|m| m.param("vto"))
        .and_then(|p| p.distribution())
        .expect("vto must carry a distribution");
    assert_eq!(dist.kind, "uniform");
    assert!(dist.global, "`global` marks process variation");
    assert_eq!(real_arg(&dist.args, "rel"), 0.01);
}

/// DVC-02 AC5: distribution arguments referencing sibling params evaluate
/// as elaboration constants in module param scope.
#[test]
fn distribution_args_evaluate_against_sibling_params() {
    let design = elaborate(
        "mod M ( inout p : Electrical ) {
            param w : Real = 1e-6;
            param l : Real = 2e-6;
            param vto : Real = 0.7 tol gauss(sigma = 0.005 / sqrt(w * l));
        }",
    )
    .expect("sibling-param references in tol args must elaborate");
    let dist = design
        .module("M")
        .and_then(|m| m.param("vto"))
        .and_then(|p| p.distribution())
        .expect("vto must carry a distribution");
    let expected = 0.005 / (1e-6_f64 * 2e-6_f64).sqrt();
    assert_eq!(real_arg(&dist.args, "sigma"), expected);
}

/// DVC-02 AC3: `tol` on an `Integer`/`Natural` param fails loud.
#[test]
fn tol_on_a_non_real_param_fails_loud() {
    for ty in ["Integer", "Natural"] {
        let src = format!(
            "mod M ( inout p : Electrical ) {{ param n : {ty} = 1 tol gauss(sigma = 0.005); }}"
        );
        let err = elaborate(&src).expect_err("tol on a discrete param must fail loud");
        let msg = err.to_string();
        assert!(msg.contains("n"), "error should name the param: {msg}");
        assert!(msg.contains("Real"), "error should state the Real requirement: {msg}");
    }
}

/// DVC-02 AC2: an undeclared distribution name fails loud, naming it —
/// proves T2's header wiring reaches elaboration.
#[test]
fn undeclared_distribution_fails_loud_naming_it() {
    let err = elaborate(
        "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol normal(sigma = 0.005); }",
    )
    .expect_err("an undeclared distribution must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("normal"), "error should name the undeclared distribution: {msg}");
}

/// DVC-02: a declared distribution with the wrong keyed-argument spelling
/// fails loud, naming the declared spellings.
#[test]
fn wrong_argument_spelling_fails_loud() {
    let err = elaborate(
        "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol gauss(stddev = 0.005); }",
    )
    .expect_err("an undeclared argument spelling must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("gauss"), "error should name the distribution: {msg}");
    assert!(msg.contains("sigma"), "error should list the declared spellings: {msg}");
}
