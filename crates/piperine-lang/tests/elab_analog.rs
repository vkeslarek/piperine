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

/// The block of `text` that starts with the line `head` and goes on over its indented lines.
struct Block;

impl Block {
    fn of(text: &str, head: &str) -> String {
        let mut block = Vec::new();
        for line in text.lines().skip_while(|l| *l != head && !l.starts_with(&format!("{head}("))) {
            if !block.is_empty() && !line.starts_with("  ") {
                break;
            }
            block.push(line);
        }
        assert!(!block.is_empty(), "no `{head}` in\n{text}");
        block.join("\n") + "\n"
    }

    /// The `+>` lines of the analog block of body `name`, in order.
    fn contributions(text: &str, name: &str) -> Vec<String> {
        Block::of(text, &format!("body {name}")).lines().filter(|l| l.contains(" +> ")).map(|l| l.trim().to_string()).collect()
    }

    /// The `branch` lines of body `name`, in order.
    fn branches(text: &str, name: &str) -> Vec<String> {
        Block::of(text, &format!("body {name}")).lines().filter(|l| l.trim_start().starts_with("branch ")).map(|l| l.trim().to_string()).collect()
    }
}

#[test]
fn a_branch_is_one_unordered_pair_with_its_kind() {
    // (a, k) twice, once reversed; (b, k) potential; `i(a)` and `i(a, gnd)` are one pair.
    let src = "
module M(a: Electrical, k: Electrical, b: Electrical) { }
analog for M {
    i(a, k) +> v(a, k);
    i(k, a) +> 1.0;
    v(b, k) +> 2.0;
    i(a) +> 3.0;
    i(a, gnd) +> 4.0;
    i(gnd, a) +> 5.0;
}
module Top() { signal s: Electrical; signal t: Electrical; let m = M { }(s, gnd, t); }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    assert_eq!(Block::branches(&text, "M"), ["branch b0 (a, k) flow", "branch b1 (b, k) potential", "branch b2 (a, gnd) flow"], "{text}");
    assert_eq!(
        Block::contributions(&text, "M"),
        ["b0.flow +> direct probe(a, k)", "b0.flow +> neg direct 1.0", "b1.potential +> direct 2.0", "b2.flow +> direct 3.0", "b2.flow +> direct 4.0", "b2.flow +> neg direct 5.0"],
        "{text}"
    );
}

#[test]
fn a_sum_splits_into_direct_and_ddt_addends() {
    // The diode's contribution, as the spec's independent test writes it.
    let src = "
module D(a: Electrical, k: Electrical) { cj0: f64 = 0.0; }
analog for D {
    let vd = v(a, k);
    let id = 1e-14 * ((vd / 0.026).exp() - 1.0);
    i(a, k) +> id + gmin() * vd + ddt(cj0 * vd);
}
module S(p: Electrical, q: Electrical, r: Electrical, s: Electrical) { }
analog for S {
    let a = v(p);
    let b = v(q);
    let x = v(r);
    let y = v(s);
    i(p, q) +> a - (b + ddt(x));
    i(p, r) +> -ddt(x);
    i(p, s) +> ddt(x) + a - b + ddt(y);
    i(q, r) +> (ddt(x)) - a + b;
    i(q, s) +> -(a + ddt(y));
    i(r, s) +> a - (b + x) + ddt(y);
}
module Top() { signal n1: Electrical; signal n2: Electrical; signal n3: Electrical; let d = D { }(n1, gnd); let s = S { }(n1, n2, n3, gnd); }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    assert_eq!(Block::contributions(&text, "D"), ["b0.flow +> direct (id + (gmin() * vd)), ddt (cj0 * vd)"], "{text}");
    assert_eq!(
        Block::contributions(&text, "S"),
        [
            // The sign of `-` crosses the parentheses: `a - b` direct, then `-ddt(x)`.
            "b0.flow +> direct (a - b), neg ddt x",
            "b1.flow +> neg ddt x",
            // Each maximal run between two `ddt` leaves is one direct addend, in source order.
            "b2.flow +> ddt x, direct (a - b), ddt y",
            // A run that starts negative is a negated addend over the run with its signs flipped.
            "b3.flow +> ddt x, neg direct (a - b)",
            "b4.flow +> neg direct a, neg ddt y",
            // A sub-sum with no `ddt` in it is one leaf, kept as written.
            "b5.flow +> direct (a - (b + x)), ddt y",
        ],
        "{text}"
    );
}

#[test]
fn a_reversed_pair_negates_its_addends() {
    let src = "
module R(p: Electrical, n: Electrical) { }
analog for R {
    let x = v(p, n);
    let y = v(p);
    i(p, n) +> x;
    i(n, p) +> ddt(x);
    i(n, p) +> y + ddt(x);
    i(n, p) +> -ddt(x);
}
module Top() { signal s: Electrical; let r = R { }(s, gnd); }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    assert_eq!(Block::branches(&text, "R"), ["branch b0 (p, n) flow"], "{text}");
    assert_eq!(
        Block::contributions(&text, "R"),
        ["b0.flow +> direct x", "b0.flow +> neg ddt x", "b0.flow +> neg direct y, neg ddt x", "b0.flow +> ddt x"],
        "`i(n, p) +> e` is `i(p, n) +> -e` (§11 rule 10): {text}"
    );
}

#[test]
fn a_branch_from_a_node_to_itself_is_e0241() {
    let src = "module M(a: Electrical) { }\nanalog for M { i(a, a) +> 1.0; }\nmodule Top() { signal s: Electrical; let m = M { }(s); }";
    let err = Elab::run(src, "Top").expect_err("a branch (a, a)");
    assert_eq!(Elab::codes(&err), ["E0241"]);
    let span = err[0].diagnostic.primary.0;
    assert_eq!(&src[span.start..span.end], "i(a, a)", "at the branch the contribution names");

    let src = "module M() { }\nanalog for M { potential(gnd, gnd) +> 1.0; }\nmodule Top() { let m = M { }(); }";
    let err = Elab::run(src, "Top").expect_err("a branch (gnd, gnd)");
    assert_eq!(Elab::codes(&err), ["E0241"]);
    let span = err[0].diagnostic.primary.0;
    assert_eq!(&src[span.start..span.end], "potential(gnd, gnd)");
}

#[test]
fn potential_and_flow_on_one_branch_is_e0415() {
    let src = "module M(a: Electrical, k: Electrical) { }\nanalog for M { i(a, k) +> 1.0; v(k, a) +> 2.0; }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }";
    let err = Elab::run(src, "Top").expect_err("potential and flow on (a, k)");
    assert_eq!(Elab::codes(&err), ["E0415"]);
    let span = err[0].diagnostic.primary.0;
    assert!(src[span.start..span.end].starts_with("v(k, a) +> 2.0"), "at the second contribution: {:?}", &src[span.start..span.end]);
}

#[test]
fn a_single_addend_is_one_direct_addend() {
    let src = "
module M(p: Electrical, n: Electrical) { r: f64 = 1.0; }
analog for M { i(p, n) +> v(p, n) / r; }
module Top() { signal s: Electrical; let m = M { }(s, gnd); }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    assert_eq!(Block::contributions(&text, "M"), ["b0.flow +> direct (probe(p, n) / r)"], "{text}");
}

#[test]
fn the_device_bodies_equal_the_structural_goldens() {
    // The five devices of the M1 library, placed once each, printed in structural form: each
    // body (and each function the diode calls) is the block `piperine-ir` pins byte for byte.
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "ngspice", "devices.msl"].iter().collect();
    let devices = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let top = "mod devices;\nuse devices::*;\nmodule Top() {\n    signal a: Electrical;\n    let r = Resistor { r: 1k }(a, gnd);\n    let c = Capacitor { c: 1e-12 }(a, gnd);\n    let v = Vsource { dc: 1.0 }(a, gnd);\n    let i = Isource { }(a, gnd);\n    let d = Diode { }(a, gnd);\n}\n";
    let design = piperine_lang::pom::Design::builder().source("top.msl", top).source("devices.msl", devices).build();
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let ir = piperine_lang::elab::Elaborator::new(&design).run("Top").unwrap_or_else(|d| panic!("{d:?}"));
    assert!(ir.check().is_empty(), "{:?}", ir.check());
    let text = ir.print_structural();
    for (file, heads) in [
        ("resistor.txt", &["nature Electrical", "body Resistor"][..]),
        ("capacitor.txt", &["body Capacitor"][..]),
        ("vsource.txt", &["body Vsource"][..]),
        ("isource.txt", &["body Isource"][..]),
        ("diode.txt", &["body Diode", "fn pnjlim", "fn vt"][..]),
    ] {
        let golden_path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "piperine-ir", "tests", "golden", file].iter().collect();
        let golden = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| panic!("{}: {e}", golden_path.display()));
        for head in heads {
            assert_eq!(Block::of(&text, head), Block::of(&golden, head), "`{head}` of {file}");
        }
    }
}

#[test]
fn a_ddt_off_the_top_of_a_flow_contribution_is_refused() {
    // Each form is refused at its `ddt` (T8 words the message): never lowered as if direct.
    for (body, ddt) in [
        ("i(p, n) +> 2.0 * ddt(v(p, n));", "ddt(v(p, n))"),
        // `v(p, n) +> ddt(v(p))` is lowered from msl-digital T35 on (amendment A5, a positive case of
        // `elab_mixed`); a `ddt` under a product of a potential contribution is still refused.
        ("v(p, n) +> 3.0 * ddt(v(p));", "ddt(v(p))"),
        ("let q = ddt(v(p)); i(p, n) +> q;", "ddt(v(p))"),
        ("i(p, n) +> ddt(v(p), 1e-6);", "ddt(v(p), 1e-6)"),
    ] {
        let src = format!("module M(p: Electrical, n: Electrical) {{ }}\nanalog for M {{ {body} }}\nmodule Top() {{ signal s: Electrical; let m = M {{ }}(s, gnd); }}");
        let err = Elab::run(&src, "Top").expect_err(body);
        assert_eq!(Elab::codes(&err), ["P0001"], "{body}");
        let span = err[0].diagnostic.primary.0;
        assert_eq!(&src[span.start..span.end], ddt, "{body}");
    }
}

#[test]
fn two_analog_blocks_keep_their_locals_apart() {
    // FIX-08 AC 2 (§10 rule 7): two `analog` blocks of one module are one body in source order,
    // and the `x` of each is its own local.
    let src = "module A() { k: f64 = 2.0; }\nanalog for A { let x = k + 1.0; }\nanalog for A { let x = k * 3.0; }\nmodule Top() { let a = A { }(); }";
    let ir = Elab::ir(src, "Top");
    let body = ir.bodies().find(|b| b.name() == "A").expect("body A");
    let stmts = body.analog().expect("an analog block");
    assert_eq!(stmts.len(), 2, "both blocks are in the one body");
    let locals: Vec<u32> = stmts
        .iter()
        .map(|s| match ir.stmt(*s) {
            piperine_ir::Stmt::Let { local, .. } => *local,
            other => panic!("a `let`, got {other:?}"),
        })
        .collect();
    assert_ne!(locals[0], locals[1], "the two `x` are two locals");
    let text = ir.print_structural();
    let (first, second) = (text.find("let x = (k + 1.0)").expect("the first block's let"), text.find("let x = (k * 3.0)").expect("the second block's let"));
    assert!(first < second, "source order: {text}");
}
