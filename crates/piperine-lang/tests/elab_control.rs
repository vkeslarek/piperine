//! Scope: mutation, loops, exits, patterns and function values lowered by `piperine_lang::elab`
//! (feature `composite-values`, CV-01 to CV-07; docs/spec Part I §1.5 to §1.8, §3). Each test
//! elaborates a design of `Top` and reads the IR it gives: the locals, the statements and the
//! codes of what is refused. Run-time values are proven from source in `piperine-parity`'s
//! `source_control`.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::Ir;
use support::Elab;

/// A design of `Top` with `items` before it, calling each of `calls` from its `analog` block.
fn reaching(items: &str, calls: &[&str]) -> String {
    let lets: Vec<String> = calls.iter().enumerate().map(|(k, call)| format!("let x{k} = {call};")).collect();
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {} }}", lets.join(" "))
}

/// The text of the function named `name` in the structural print.
fn function_text(ir: &Ir, name: &str) -> String {
    let text = ir.print_structural();
    let head = format!("fn {name}(");
    let start = text.find(&head).unwrap_or_else(|| panic!("no `{head}` in:\n{text}"));
    let rest = &text[start..];
    let end = rest[1..].find("\nfn ").or_else(|| rest[1..].find("\ninstance ")).map_or(rest.len(), |at| at + 1);
    rest[..end].to_string()
}

#[test]
fn let_mut_lowers_to_mutable_locals_and_assignments() {
    // §1.5.12: `let mut` is a mutable local, `x = e` and `x op= e` assign it; a plain `let` and a
    // parameter stay immutable. The IR passes its own check.
    let items = "fn work(a: f64) -> f64 { let mut x = a; x = x + 1.0; x += 2.0; x *= 3.0; let y = x; y }";
    let ir = Elab::ir(&reaching(items, &["work(1.0)"]), "Top");
    let function = ir.functions().find(|f| f.name() == "work").expect("work");
    let flags: Vec<(String, bool)> = function.locals().iter().map(|l| (l.name.clone(), l.mutable)).collect();
    assert_eq!(flags, vec![("a".to_string(), false), ("x".to_string(), true), ("y".to_string(), false)]);
    let text = function_text(&ir, "work");
    for printed in ["x = (x + 1", "x += 2", "x *= 3"] {
        assert!(text.contains(printed), "`{printed}` in:\n{text}");
    }

    // §1.7.5: `let x;` is a local first assigned later; it is assignable in the IR though the
    // source wrote no `mut`.
    let items = "fn pick(c: bool) -> u8 { let r: u8; if c { r = 1; } else { r = 2; } r }";
    let ir = Elab::ir(&reaching(items, &["pick(true)"]), "Top");
    let function = ir.functions().find(|f| f.name() == "pick").expect("pick");
    let flags: Vec<(String, bool)> = function.locals().iter().map(|l| (l.name.clone(), l.mutable)).collect();
    assert_eq!(flags, vec![("c".to_string(), false), ("r".to_string(), true)]);
    let text = function_text(&ir, "pick");
    assert!(text.contains("r = 1") && text.contains("r = 2"), "{text}");
}

/// A design whose `Top` has the reaction `body` on a clock `clk`.
fn reacting(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top(in clk: Bit, out q: Bit<u8>) {{ }}\ndigital for Top {{ on clk.rise() {{ {body} }} }}")
}

#[test]
fn loops_lower_to_the_loops_of_the_ir() {
    // §1.8.7 to §1.8.12: every form stays the IR's own loop in a function, where no decision may
    // be unknown; `break value` gives the loop its value's type.
    let items = "fn work(n: u8) -> u8 {\n\
                 let mut s = 0u8; let mut i = 0u8;\n\
                 while i < n { s += i; i += 1; }\n\
                 let first = loop { if s > 3 { break s; } s += 1; };\n\
                 for k in 0..n { if k == 2 { continue; } s += k; }\n\
                 for k in 1..=n { s += k; }\n\
                 first + s }";
    let ir = Elab::ir(&reaching(items, &["work(5)"]), "Top");
    let text = function_text(&ir, "work");
    for printed in ["while (i < n)", "loop {", "break s", "continue", "for k in 0u8..n", "for k in 1u8..=n"] {
        assert!(text.contains(printed), "`{printed}` in:\n{text}");
    }

    // In a reaction, whose values may be unknown, a `for` that leaves early is the `while` that
    // counts a mutable local, taking the next value before the body so `continue` still counts.
    let body = "let mut hit = 0u8; for k in 0..9u8 { if k > 3 { hit = k; break; } } q <- hit;";
    let ir = Elab::ir(&reacting("", body), "Top");
    let text = ir.print_structural();
    assert!(text.contains("while") && !text.contains("for k in"), "{text}");
}
