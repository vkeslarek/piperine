//! Scope: the analog block of `piperine_lang::elab` (ELAB-04): value code and functions lowered
//! to the IR node of the same form (each function once), probes and environment reads, sites
//! numbered in source order, and, with T7, branches and the split of a contribution.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use std::path::PathBuf;

use support::Elab;

/// One function block of a golden text of `piperine-ir`: the `fn` line and its indented lines.
struct Golden;

impl Golden {
    fn function(file: &str, name: &str) -> String {
        let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "piperine-ir", "tests", "golden", file].iter().collect();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut block = Vec::new();
        for line in text.lines().skip_while(|l| !l.starts_with(&format!("fn {name}("))) {
            if !block.is_empty() && !line.starts_with("  ") {
                break;
            }
            block.push(line);
        }
        assert!(!block.is_empty(), "no `fn {name}` in {file}");
        block.join("\n") + "\n"
    }
}

#[test]
fn value_code_lowers_to_the_same_forms_and_functions_once() {
    // Two bodies call `pnjlim` (std, §22.2): one function, however many callers.
    let src = "
module A() { }
analog for A { let y = pnjlim(1.0, 0.5, 0.026, 0.6); }
module B() { k: f64 = 2.0; }
analog for B { let z = pnjlim(k, 0.5, 0.026, 0.6) + 1.0; let w = if z > 1.0 { z } else { -z }; }
module Top() { let a = A { }(); let b = B { }(); }";
    let ir = Elab::ir(src, "Top");
    assert_eq!(ir.functions().filter(|f| f.name() == "pnjlim").count(), 1, "pnjlim is one FunctionId");
    assert_eq!(ir.functions().count(), 1);

    // The function is the form the hand-built golden of `piperine-ir` pins, line for line:
    // `if`/`else`, blocks with `let`s, `abs`/`ln` as `Math`, unary `-`, every local.
    let text = ir.print_structural();
    let golden = Golden::function("diode.txt", "pnjlim");
    assert!(text.contains(&golden), "the lowered pnjlim differs from the golden:\n{golden}\nin\n{text}");

    // The calls, the `if` expression and the locals of the bodies.
    assert!(text.contains("let y = pnjlim(1.0, 0.5, 0.026, 0.6)"), "{text}");
    assert!(text.contains("let z = (pnjlim(k, 0.5, 0.026, 0.6) + 1.0)"), "{text}");
    assert!(text.contains("let w = if (z > 1.0) { z } else { (-z) }"), "{text}");
}

#[test]
fn a_lowered_expression_carries_its_type_and_the_origin_of_its_construct() {
    let src = "module A() { }\nanalog for A { let y = pnjlim(1.0, 0.5, 0.026, 0.6); }\nmodule Top() { let a = A { }(); }";
    let ir = Elab::ir(src, "Top");
    let body = ir.bodies().find(|b| b.name() == "A").expect("body A");
    let first = body.analog().expect("an analog block")[0];
    let piperine_ir::Stmt::Let { init, .. } = ir.stmt(first) else { panic!("a `let`") };
    let call_col = src.lines().nth(1).unwrap().find("pnjlim").unwrap() as u32 + 1;
    let origin = ir.expr_origin(*init);
    assert_eq!((origin.file.as_str(), origin.line, origin.col), ("top.msl", 2, call_col), "the start of the call");
    assert_eq!(ir.expr_type(*init), &piperine_ir::ValueType::F64);
}

const PROBE_MODULE: &str = "
module M(a: Electrical, k: Electrical) { }
analog for M {
    let x = v(a);
    let y = v(a, k);
    let g = v(k, gnd);
    let e = temperature() + gmin() * source_scale();
}
module Top() { signal s: Electrical; let m = M { }(s, gnd); }";

#[test]
fn probes_and_environment_lower_to_their_nodes() {
    let ir = Elab::ir(PROBE_MODULE, "Top");
    let text = ir.print_structural();
    assert!(text.contains("let x = probe(a, gnd)"), "`v(a)` is a probe to ground: {text}");
    assert!(text.contains("let y = probe(a, k)"), "`v(a, k)` is between two body nodes: {text}");
    assert!(text.contains("let g = probe(k, gnd)"), "{text}");
    assert!(text.contains("let e = (temperature() + (gmin() * source_scale()))"), "the environment reads: {text}");

    // The terminals are the body's node indices (ports in order), not names.
    let body = ir.bodies().find(|b| b.name() == "M").expect("body M");
    let stmts = body.analog().expect("an analog block");
    let probe_of = |i: usize| match ir.stmt(stmts[i]) {
        piperine_ir::Stmt::Let { init, .. } => ir.expr(*init).clone(),
        other => panic!("a `let`, got {other:?}"),
    };
    assert_eq!(probe_of(0), piperine_ir::Expr::Probe { plus: piperine_ir::Terminal::Node(0), minus: piperine_ir::Terminal::Ground });
    assert_eq!(probe_of(1), piperine_ir::Expr::Probe { plus: piperine_ir::Terminal::Node(0), minus: piperine_ir::Terminal::Node(1) });
}

#[test]
fn sites_are_numbered_in_source_order_and_named() {
    let src = "
module D(a: Electrical, k: Electrical) { }
analog for D {
    let vcrit = 0.6;
    let vd = limit(v(a, k), |new, old| pnjlim(new, old, 0.026, vcrit), vcrit);
    let w = limit(v(a), |n, o| n);
    let hold = limit(v(k), |n, o| o) + 1.0;
    let s = ac_stim(1.0);
    let u = ac_stim(2.0, 0.5);
}
module Top() { let d = D { }(gnd, gnd); }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    // The sites, in source order: the `let` names a `limit`; an `ac_stim` is the analysis `ac`.
    assert!(text.contains("  limit L0 \"vd\"\n  limit L1 \"w\"\n  limit L2 \"limit2\"\n  stim S0 \"ac\"\n  stim S1 \"ac\"\n"), "{text}");
    // The diode's form, closure and seed included.
    assert!(text.contains("let vd = limit L0 (probe(a, k), |new, old| pnjlim(new, old, 0.026, vcrit), seed vcrit)"), "{text}");
    assert!(text.contains("let w = limit L1 (probe(a, gnd), |n, o| n)"), "no seed, no `seed`: {text}");
    assert!(text.contains("let s = ac_stim S0 (1.0)") && text.contains("let u = ac_stim S1 (2.0, phase 0.5)"), "{text}");
}

#[test]
fn a_site_under_an_if_is_refused_not_mis_lowered() {
    let src = "module D(a: Electrical) { }\nanalog for D { let x = if v(a) > 0.0 { limit(v(a), |n, o| n) } else { 0.0 }; }\nmodule Top() { let d = D { }(gnd); }";
    let err = Elab::run(src, "Top").expect_err("a site is unconditional");
    assert_eq!(Elab::codes(&err), ["P0001"]);
}
