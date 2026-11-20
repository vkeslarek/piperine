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

/// A design of `Top` whose `analog` block contributes `call` to its node.
fn analog_calling(items: &str, call: &str) -> String {
    format!("{items}\nmodule Top() {{ signal o: Electrical; signal a: Electrical; }}\nanalog for Top {{ i(o) +> {call}; }}")
}

#[test]
fn a_recursion_reached_from_analog_is_unrolled() {
    // §3 rule 12 (CV-07 AC 2): the depth is decided by constants, so the recursion is expanded where
    // it is called; the IR holds no function that calls itself (its own check passes).
    let items = "fn tri(n: u8, x: f64) -> f64 { if n == 0 { 0.0 } else { x + tri(n - 1, x) } }";
    let ir = Elab::ir(&analog_calling(items, "tri(3, v(a))"), "Top");
    let text = ir.print_structural();
    assert!(ir.functions().all(|f| f.name() != "tri"), "{text}");
    // `tri(3)` expands `tri(3)`, `tri(2)`, `tri(1)` and `tri(0)`, whose `n == 0` folds to the base case.
    assert_eq!(text.matches("let tri.n").count(), 4, "{text}");
    assert!(text.contains("(1u8 - 1u8); let tri.x = tri.x; 0.0 }"), "the last level is the base case:\n{text}");

    // Through a function that is not itself recursive.
    let items = "fn tri(n: u8, x: f64) -> f64 { if n == 0 { 0.0 } else { x + tri(n - 1, x) } }\nfn twice(x: f64) -> f64 { tri(2, x) + tri(2, x) }";
    let ir = Elab::ir(&analog_calling(items, "twice(v(a))"), "Top");
    assert!(ir.functions().all(|f| f.name() != "tri" && f.name() != "twice"), "{}", ir.print_structural());

    // A recursion no analog block reaches stays a recursion of the IR (D12).
    let items = "fn tri(n: u8, x: f64) -> f64 { if n == 0 { 0.0 } else { x + tri(n - 1, x) } }";
    let src = format!("{items}\nmodule Top(in clk: Bit, out q: Bit<u8>) {{ }}\ndigital for Top {{ on clk.rise() {{ q <- tri(3, 1.0) as u8; }} }}");
    let ir = Elab::ir(&src, "Top");
    assert!(ir.functions().any(|f| f.name() == "tri"));
}

#[test]
fn an_unfoldable_recursion_from_analog_is_e0126() {
    // §3 rule 12 (CV-07 AC 3): the depth reads a probe, so the decision does not fold; the error names
    // the function.
    let items = "fn tri(n: u8, x: f64) -> f64 { if n == 0 { 0.0 } else { x + tri(n - 1, x) } }";
    let refused = Elab::run(&analog_calling(items, "tri(v(a) as u8, v(a))"), "Top").expect_err("a depth from a probe");
    assert_eq!(Elab::codes(&refused), ["E0126"]);
    assert!(refused[0].diagnostic.message.contains("`tri`"), "{:?}", refused[0].diagnostic.message);

    // A recursion with no decision at all runs past 1 024 calls.
    let items = "fn forever(x: f64) -> f64 { forever(x) }";
    let refused = Elab::run(&analog_calling(items, "forever(v(a))"), "Top").expect_err("no end");
    assert_eq!(Elab::codes(&refused), ["E0126"]);
    assert!(refused[0].diagnostic.message.contains("`forever`"), "{:?}", refused[0].diagnostic.message);
}

/// The folded value of the `f64` param `name` of `Top`.
fn default_of(ir: &Ir, name: &str) -> piperine_ir::Constant {
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    let param = top.params().iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no param {name}"));
    param.default.unwrap_or_else(|| panic!("no default for {name}"))
}

#[test]
fn phase_a_forms_fold_in_const_contexts() {
    // CV-36 AC 1: every form of phase A evaluates at elaboration to the value it has at run time, in
    // a `const`, a `const fn`, a param default and a `where`. The expected values are the same
    // computations in Rust.
    let gcd = |mut x: u32, mut y: u32| {
        while y != 0 {
            (x, y) = (y, x % y);
        }
        x
    };
    let items = "\
        const fn gcd(a: u32, b: u32) -> u32 { let mut x = a; let mut y = b; while y != 0 { let t = x % y; x = y; y = t; } x }\n\
        const fn fact(n: u32) -> u32 { if n == 0 { 1 } else { n * fact(n - 1) } }\n\
        const fn first_square_over(limit: u32) -> u32 { let mut k = 0u32; loop { k += 1; if k * k > limit { break k; } } }\n\
        const fn weigh(x: u32) -> f64 { match x { 0 => 0.0, 1..=3 => 1.5, n if n % 2 == 0 => 2.5, _ => 3.5 } }\n\
        const fn early(n: u32) -> u32 { for k in 0..n { if k * k >= 30 { return k; } } n }\n\
        const fn through_closure(n: u32) -> u32 { let double = |x: u32| x * 2; let mut s = 0u32; for k in 1..=n { if k == 3 { continue; } s += double(k); } s }\n\
        const fn pick(v: u32) -> u32 { if let 0..=2 = v { return 10; } let 3 = v else { return 99; }; let mut n = 0u32; while let 0..=4 = n { n += 2; } 30 + n }\n\
        const GCD: u32 = gcd(84, 36);\n\
        const FACT: u32 = fact(5);\n";
    let src = format!(
        "{items}module Top() {{\n\
            a: f64 = GCD as f64;\n\
            b: f64 = FACT as f64;\n\
            c: f64 = first_square_over(40) as f64;\n\
            d: f64 = weigh(0) + weigh(2) + weigh(8) + weigh(9);\n\
            e: f64 = early(100) as f64;\n\
            f: f64 = through_closure(5) as f64 where f > 0.0;\n\
            h: f64 = (pick(1) + pick(3) + pick(7)) as f64;\n\
        }}\nmodule Unused() {{ }}"
    );
    let ir = Elab::ir(&src, "Top");
    let real = |name: &str| match default_of(&ir, name) {
        piperine_ir::Constant::Real(r) => r.get(),
        other => panic!("{name} is {other:?}"),
    };
    assert_eq!(real("a"), f64::from(gcd(84, 36)), "a `while` over mutable locals (§1.8.8)");
    assert_eq!(real("b"), 120.0, "a recursion (§3 rule 12)");
    assert_eq!(real("c"), 7.0, "`loop` with `break value` (§1.8.9)");
    let weigh = |x: u32| match x {
        0 => 0.0,
        1..=3 => 1.5,
        n if n % 2 == 0 => 2.5,
        _ => 3.5,
    };
    assert_eq!(real("d"), weigh(0) + weigh(2) + weigh(8) + weigh(9), "range patterns, a guard and a binder (§1.8.4)");
    assert_eq!(real("e"), 6.0, "`return` from inside a `for` (§1.8.12)");
    assert_eq!(real("f"), f64::from((1..=5u32).filter(|k| *k != 3).map(|k| k * 2).sum::<u32>()), "a closure, `continue` and `..=` (§1.8.10)");

    assert_eq!(real("h"), 10.0 + 36.0 + 99.0, "`if let`, `let .. else` and `while let` with range patterns (§1.7.5, §1.8.4)");

    // The `where` of a param folds the same forms: a condition over a `const fn` that loops.
    let src = format!("{items}module Top() {{ g: f64 = 1.0 where gcd(12, 18) == 6 && fact(4) == 24; }}");
    Elab::ir(&src, "Top");
    let src = format!("{items}module Top() {{ g: f64 = 1.0 where gcd(12, 18) == 7; }}");
    let refused = Elab::run(&src, "Top").expect_err("the `where` is false");
    assert_eq!(Elab::codes(&refused), ["E0317"]);
}

#[test]
fn a_panic_in_a_const_evaluation_names_the_expression() {
    // §1 rule 11 and Q5 (CV-36 AC 2): a division by zero reached through a loop, a loop past 2^24
    // iterations and a call nested past 1 024 are elaboration errors, `E0104`, at the expression.
    let division = "const fn bad() -> u32 { let mut s = 0u32; loop { s += 1; if s > 3 { break; } } 10 / (s - 4) }\nconst X: u32 = bad();\nmodule Top() { a: f64 = X as f64; }";
    let refused = Elab::run(division, "Top").expect_err("a division by zero");
    assert_eq!(Elab::codes(&refused), ["E0104"]);
    let message = &refused[0].diagnostic.message;
    assert!(message.contains("division by zero"), "{message}");
    let span = refused[0].diagnostic.span();
    assert_eq!(&division[span.start..span.end], "10 / (s - 4)", "the diagnostic sits on the expression that panics");

    let endless = "const fn spin() -> u32 { let mut s = 0u32; loop { s += 1; } }\nconst X: u32 = spin();\nmodule Top() { a: f64 = X as f64; }";
    let refused = Elab::run(endless, "Top").expect_err("a loop with no end");
    assert_eq!(Elab::codes(&refused), ["E0104"]);
    assert!(refused[0].diagnostic.message.contains("2^24"), "{:?}", refused[0].diagnostic.message);

    let deep = "const fn down(n: u32) -> u32 { if n == 0 { 0 } else { down(n - 1) } }\nconst X: u32 = down(2000);\nmodule Top() { a: f64 = X as f64; }";
    let refused = Elab::run(deep, "Top").expect_err("a call nested too deep");
    assert_eq!(Elab::codes(&refused), ["E0104"]);
    assert!(refused[0].diagnostic.message.contains("1 024"), "{:?}", refused[0].diagnostic.message);
    // the same recursion within the bound is the value
    let within = "const fn down(n: u32) -> u32 { if n == 0 { 7 } else { down(n - 1) } }\nconst X: u32 = down(1000);\nmodule Top() { a: f64 = X as f64; }";
    assert_eq!(default_of(&Elab::ir(within, "Top"), "a"), piperine_ir::Constant::Real(piperine_ir::Real::new(7.0)));
}
