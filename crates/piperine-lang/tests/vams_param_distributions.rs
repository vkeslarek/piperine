//! `tol` distribution clauses on parameter declarations
//! (`random-variable-catalogue`): `parameter real r = 1e3 tol
//! gauss(sigma = 10.0) global;` parses, const-evaluates its keyed
//! arguments in module scope, and lands on the POM's `Param::
//! distribution` — the authored carrier the surviving `SamplePlan` walk
//! reads. `tol`/`global` are contextual keywords (Annex B untouched).
//!

use piperine_lang::value::Value;
use piperine_lang::{parse_and_elaborate, Grammar, Keywords, Lexer, SourceMap};

fn grammar(src: &str) -> Grammar {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error for {src:?}: {e}"));
    Grammar::new(toks)
}

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

#[test]
fn a_gauss_clause_with_sigma_and_global_parses() {
    let m = grammar(
        "module m(); parameter real r = 1e3 tol gauss(sigma = 10.0) global; endmodule",
    )
    .parse_module_decl()
    .expect("parses");
    let piperine_lang::ast::ModuleItem::Param(pd) = &m.items[0] else {
        panic!("a parameter declaration")
    };
    let tol = pd.tol.clone().expect("tol clause");
    assert_eq!(tol.kind, "gauss");
    assert_eq!(tol.args.len(), 1);
    assert!(tol.global, "the global marker travels");
}

#[test]
fn sigma_rel_and_rel_forms_parse() {
    for src in [
        "module m(); parameter real r = 1e3 tol gauss(sigma_rel = 0.05); endmodule",
        "module m(); parameter real v = 1.0 tol uniform(rel = 0.01); endmodule",
    ] {
        let m = grammar(src).parse_module_decl().expect("parses");
        let piperine_lang::ast::ModuleItem::Param(pd) = &m.items[0] else {
            panic!("a parameter declaration")
        };
        assert!(pd.tol.is_some(), "the clause parses: {src}");
    }
}

#[test]
fn two_tol_clauses_on_one_declaration_fail_loud() {
    let err = grammar(
        "module m(); parameter real r = 1.0 tol gauss(sigma = 1.0) tol uniform(rel = 0.1); endmodule",
    )
    .parse_module_decl()
    .expect_err("a duplicate tol clause is a typed error");
    assert!(
        err.to_string().contains("at most one"),
        "names the rule: {err}"
    );
}

#[test]
fn the_clause_lands_on_every_name_of_the_declaration() {
    let d = elab(
        "module m(); parameter real a = 1.0, b = 2.0 tol gauss(sigma = 0.1); endmodule",
    );
    let m = d.module("m").expect("elaborated");
    for name in ["a", "b"] {
        let p = m.params().iter().find(|p| p.name == name).expect(name);
        let dist = p.distribution.as_ref().expect("the clause applies to every name");
        assert_eq!(dist.kind, "gauss");
        assert!(!dist.global, "mismatch (per-instance) is the default scope");
    }
}

#[test]
fn keyed_arguments_const_evaluate_in_module_scope() {
    let d = elab(
        "\
module m();
    parameter real w = 2.0;
    parameter real l = 0.5;
    parameter real r = 1e3 tol gauss(sigma = 0.005 / sqrt(w * l)) global;
endmodule",
    );
    let m = d.module("m").expect("elaborated");
    let r = m.params().iter().find(|p| p.name == "r").expect("r");
    let dist = r.distribution.as_ref().expect("tol clause");
    let sigma = dist
        .args
        .iter()
        .find(|(k, _)| k == "sigma")
        .map(|(_, v)| v.clone())
        .expect("sigma arg");
    assert_eq!(sigma, Value::Real(0.005 / (2.0 * 0.5f64).sqrt()));
    assert!(dist.global);
}

#[test]
fn an_unknown_distribution_kind_fails_loud() {
    let msg = elab_err(
        "module m(); parameter real r = 1.0 tol pareto(alpha = 2.0); endmodule",
    );
    assert!(msg.contains("pareto"), "names the unknown kind: {msg}");
    assert!(msg.contains("gauss"), "names what is implemented: {msg}");
}

#[test]
fn an_unresolvable_argument_fails_loud() {
    let msg = elab_err(
        "module m(); parameter real r = 1.0 tol gauss(sigma = nope); endmodule",
    );
    assert!(msg.contains("nope"), "names the unresolved name: {msg}");
}

#[test]
fn tol_and_global_stay_out_of_annex_b() {
    for word in ["tol", "global"] {
        assert!(
            !Keywords::is_reserved(word),
            "`{word}` must remain a contextual keyword"
        );
    }
}

#[test]
fn tol_remains_usable_as_a_parameter_name() {
    // `tol` only means the clause after a parameter's default value — a
    // parameter *named* `tol` still parses.
    let d = elab("module m(); parameter real tol = 1.0; endmodule");
    assert!(d.module("m").expect("elaborated").params().iter().any(|p| p.name == "tol"));
}
