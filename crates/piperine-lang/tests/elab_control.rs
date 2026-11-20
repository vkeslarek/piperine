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

#[test]
fn a_function_returning_an_option_is_two_ir_functions() {
    // MD-67 item 2 (CV-03): an `Option` is the pair `(is, val)`; a function that returns one is
    // the IR function of each half, and a call reads both.
    let items = "fn halved(x: Option<u8>) -> Option<u8> { let v = x?; Some(v / 2) }";
    let src = reacting(items, "q <- halved(Some(8)).unwrap_or(99);");
    let ir = Elab::ir(&src, "Top");
    let names: Vec<String> = ir.functions().map(|f| f.name().to_string()).collect();
    assert_eq!(names, vec!["halved.is".to_string(), "halved.val".to_string()]);
}

#[test]
fn a_function_value_called_with_an_unknown_is_the_flagged_variant_or_p0001() {
    // §3.3 with §1.5.10: a local bound to a function's name calls the variant for an argument that
    // may be unknown; a closure, whose type is fixed where it is written, cannot take one, and the
    // refusal names the call.
    let named = reacting("fn id(x: u8) -> u8 { x }", "let f = id; let u: u8 = Unknown; q <- f(u).known_or(0);");
    let ir = Elab::ir(&named, "Top");
    assert!(ir.functions().any(|f| f.name() == "id~1"), "{}", ir.print_structural());
    let closure = reacting("", "let f = |x: u8| x; let u: u8 = Unknown; q <- f(u).known_or(0);");
    let refused = Elab::run(&closure, "Top").expect_err("a closure takes no unknown");
    assert_eq!(Elab::codes(&refused), ["P0001"]);
    assert!(refused[0].diagnostic.message.contains("a call through a function value"), "{:?}", refused[0].diagnostic.message);
}

/// `Lag` of AS-09: a result that reads its own node, hoisted into each body that places it.
const LAG: &str = "module Lag(x: f64) -> f64 { tau: f64 = 1m; signal y: State; potential(y) }\nanalog for Lag { flow(y) +> tau * ddt(potential(y)) + potential(y) - x; }\n";

#[test]
fn an_analog_loop_that_contributes_or_places_is_unrolled() {
    // §11 rule 12 (CV-02 AC 2): a loop that contributes, with a constant trip count, is its unrolling;
    // the loop variable is its constant in each iteration. The IR holds no loop.
    let src = "module Top() { signal o: Electrical; signal a: Electrical; }\nanalog for Top { for k in 0..3u8 { i(o) +> (k as f64) * v(a); } }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    assert_eq!(text.matches(" +> ").count(), 3, "{text}");
    assert!(!text.contains("for k in"), "{text}");
    for k in 0..3 {
        assert!(text.contains(&format!("(({k}u8 as f64) * probe(a, gnd))")), "iteration {k} reads its own constant:\n{text}");
    }

    // §8.3.4 (CV-02 AC 2): each iteration's placement is a distinct instance, named by the hoisting
    // rule in unrolled order, and its argument reads that iteration's loop variable.
    let src = format!("{LAG}module Top() {{ signal a: Electrical; signal o: Electrical; }}\nanalog for Top {{ for k in 0..3u8 {{ i(o) +> Lag(v(a) * (k as f64)); }} }}");
    let ir = Elab::ir(&src, "Top");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    let names: Vec<&str> = top.nodes().iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["a", "o", "Lag$0.y", "Lag$1.y", "Lag$2.y"], "one node per iteration");
    assert_eq!(top.hoisted().iter().map(|h| h.name.as_str()).collect::<Vec<_>>(), ["Lag$0", "Lag$1", "Lag$2"]);
    let text = ir.print_structural();
    for k in 0..3 {
        assert!(text.contains(&format!("(probe(a, gnd) * ({k}u8 as f64))")), "instance {k} takes its iteration's argument:\n{text}");
    }

    // A nested loop: the inner trip count reads the outer variable; 0 + 1 + 2 iterations.
    let src = "module Top() { signal o: Electrical; }\nanalog for Top { for j in 0..3u8 { for k in 0..j { i(o) +> 1.0; } } }";
    let ir = Elab::ir(src, "Top");
    assert_eq!(ir.print_structural().matches(" +> ").count(), 3);

    // An empty range contributes nothing (edge case).
    let src = "module Top() { signal o: Electrical; }\nanalog for Top { for k in 2..2u8 { i(o) +> 1.0; } i(o) +> 2.0; }";
    assert_eq!(Elab::ir(src, "Top").print_structural().matches(" +> ").count(), 1);
}

#[test]
fn a_contributing_analog_loop_without_a_constant_trip_count_is_e0416() {
    // §11 rule 12 (CV-02 AC 3): a trip count that reads a probe, and a `while`, are not constants.
    let src = "module Top() { signal o: Electrical; }\nanalog for Top { for k in 0..(v(o) as u8) { i(o) +> 1.0; } }";
    let refused = Elab::run(src, "Top").expect_err("a probe is no trip count");
    assert_eq!(Elab::codes(&refused), ["E0416"]);
    let src = "module Top() { signal o: Electrical; }\nanalog for Top { let mut n = 0u8; while n < 3 { i(o) +> 1.0; n += 1; } }";
    let refused = Elab::run(src, "Top").expect_err("a while has no trip count");
    assert_eq!(Elab::codes(&refused), ["E0416"]);

    // §8.3.4: the same for a placement is E0351.
    let src = format!("{LAG}module Top() {{ signal a: Electrical; signal o: Electrical; }}\nanalog for Top {{ for k in 0..(v(a) as u8) {{ i(o) +> Lag(v(a)); }} }}");
    let refused = Elab::run(&src, "Top").expect_err("a placement in a loop over a probe");
    assert_eq!(Elab::codes(&refused), ["E0416"], "a contribution in the body is named first");
    let src = format!("{LAG}module Top() {{ signal a: Electrical; signal o: Electrical; }}\nanalog for Top {{ for k in 0..(v(a) as u8) {{ let y = Lag(v(a)); }} }}");
    let refused = Elab::run(&src, "Top").expect_err("a placement in a loop over a probe");
    assert_eq!(Elab::codes(&refused), ["E0351"]);
}
