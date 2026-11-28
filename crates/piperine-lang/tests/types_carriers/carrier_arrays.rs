//! Arrays of carriers in `piperine_lang::types` (feature `composite-values` phase C, CV-23 to CV-25; docs/spec part I
//! §5.1.8, part III §8.1): the type of a `signal` or port that is an array of carriers or of nodes, its initial value,
//! the reads of an element and its history, the targets of commits and definitions, and connection to ports. Each
//! test states the rule it proves; every source is a design of `Top` over the embedded standard library.


use piperine_lang::pom::{DeclKind, Reflect};
use piperine_lang::types::{Length, Ty, Types};
use piperine_lang_fixtures::checked::CheckedSource;


/// A module `Top` checked over the standard library, whatever it says.
struct StandardTopCheck;

impl StandardTopCheck {
    /// The diagnostics of checking `Top` of `src` as `(code, message)`.
    fn found(src: &str) -> Vec<(String, String)> {
        CheckedSource::standard_outcome(src)
    }

    /// The diagnostics hold exactly one of `code` naming `words`.
    fn assert_one(src: &str, code: &str, words: &str) {
        let found = Self::found(src);
        assert!(found.iter().any(|(c, m)| c == code && m.contains(words)), "expected {code} naming `{words}`, got {found:?}\n{src}");
    }

    /// The POM refuses `src` with `code`, and checking it adds no `P0001` (the refusal is never masked).
    fn assert_pom_refusal(src: &str, code: &str) {
        let design = piperine_lang::pom::Design::builder().source("top.msl", src).build();
        let codes: Vec<String> = design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect();
        assert!(codes.iter().any(|c| c == code), "expected {code}, got {codes:?}\n{src}");
        if let Err(found) = Types::check(&design, "Top") {
            assert!(found.iter().all(|d| d.diagnostic.code != "P0001"), "P0001 masks {code}: {:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        }
    }

    /// The source checks clean.
    fn assert_clean(src: &str) {
        let found = Self::found(src);
        assert!(found.is_empty(), "expected a clean check, got {found:?}\n{src}");
    }
}

#[test]
fn arrays_of_carriers_type_as_signals_and_ports() {
    // CV-23: `[Bit<u8>; 4]` as a `signal`, a `[Bit; 8]` port with a direction and `[Electrical; 4]` as a signal are
    // types of their own: an array whose length is part of the type
    let src = "module Top(in a: [Bit; 8], out b: [Bit<u8>; 2] = [0; 2]) { signal m: [Bit<u8>; 4] = [0; 4]; signal bl: [Electrical; 4]; signal g: [[Bit; 2]; 3] = [[false; 2]; 3]; }";
    let design = CheckedSource::standard_design_of(src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let signal = |name: &str| design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Signal(_) | DeclKind::Port(_))).unwrap_or_else(|| panic!("no `{name}`")).id();
    let named = |name: &str| types.carrier(signal(name)).map(|t| t.name(&design)).unwrap_or_default();
    assert_eq!(named("m"), "[Bit<u8>; 4]");
    assert_eq!(named("a"), "[Bit<bool>; 8]");
    assert_eq!(named("g"), "[[Bit<bool>; 2]; 3]", "§1.2.5: arrays nest, `sigtype` allows it");
    let nodes = types.carrier(signal("bl")).expect("the node array is recorded");
    assert!(matches!(nodes, Ty::Array(element, Length::Fixed(4)) if matches!(**element, Ty::Node(_))), "{nodes:?}");
    // §8.1.5: a port of carriers takes a direction, a port of nodes takes none (the direction is the array's)
    StandardTopCheck::assert_one("module Top(a: [Bit; 8]) { }", "E0310", "takes `in` or `out`");
    StandardTopCheck::assert_one("module Top(in a: [Electrical; 2]) { }", "E0310", "takes no direction");
}

#[test]
fn an_array_initial_value_of_another_type_is_e0210() {
    // §5.1.5, §5.1.8: the initial value of an array of carriers is an array of their values, `[x; K]` or
    // `[a, b, c]`; any other type, or a read of a carrier in it, is E0210
    StandardTopCheck::assert_clean("module Top() { signal m: [Bit<u8>; 3] = [1, 2, 3]; signal r: [Bit<u8>; 3] = [7; 3]; }");
    StandardTopCheck::assert_one("module Top() { signal m: [Bit<u8>; 3] = 5; }", "E0210", "");
    // an element of the literal that is the wrong type is that element's own mismatch (E0180), as in any array literal
    StandardTopCheck::assert_one("module Top() { signal m: [Bit<u8>; 3] = [true; 3]; }", "E0180", "expected `u8`, found `bool`");
    StandardTopCheck::assert_one("module Top() { signal m: [Bit<u8>; 3] = [1, 2]; }", "E0210", "");
}

#[test]
fn an_array_of_carriers_as_a_function_parameter_is_refused_by_name() {
    // QC3: §3 rule 7 makes a nature parameter a handle and is silent on an array of them: P0001 naming the form
    StandardTopCheck::assert_one("fn f(a: [Bit; 8]) {}\nmodule Top() { }\nanalog for Top { let h = f; }", "P0001", "an array of carriers as a function parameter");
    StandardTopCheck::assert_one("fn g() { let x: [Bit; 2] = [0; 2]; }\nmodule Top() { }\nanalog for Top { g(); }", "P0001", "an array of carriers as a `let` type");
}

#[test]
fn element_reads_history_and_probes_type_per_element() {
    // CV-24 AC 1, AC 2: a whole read is the array of the elements' values, `x[i]` one element's value (an integer
    // index, constant or not), `x[k].prev()`, `.changed()`, `.rise()`, `.fall()` and the probe `v(bl[k])` apply to
    // that element only
    let src = "module Top(in clk: Bit, in at: Bit<u2>, out q: Bit<u8> = 0) {\n\
               signal mem: [Bit<u8>; 4] = [0; 4];\n\
               signal lines: [Bit; 4] = [false; 4];\n\
               signal bl: [Electrical; 4];\n}\n\
               digital for Top {\n\
               on clk.rise() { let a = mem[1]; let b = mem[at]; let w = mem; let p = mem[2].prev(); let c = lines[3].changed(); q <- a; }\n\
               on lines[2].rise() { }\n\
               on lines[at].fall() { }\n\
               on mem[1].changed() { }\n}\n\
               analog for Top { let x = v(bl[2]); }";
    StandardTopCheck::assert_clean(src);
    // the element types: `mem[1]` is `u8`, the whole `[u8; 4]`, `lines[3].changed()` a `bool`
    let design = CheckedSource::standard_design_of(src);
    let types = Types::check(&design, "Top").expect("the module checks");
    let bindings: Vec<(String, String)> = design
        .decls()
        .into_iter()
        .filter(|d| matches!(d.kind(), DeclKind::Binding(_)))
        .filter_map(|d| Some((d.name()?.to_string(), types.local(d.id())?.name(&design))))
        .collect();
    for (name, want) in [("a", "u8"), ("b", "u8"), ("w", "[u8; 4]"), ("p", "u8"), ("c", "bool")] {
        assert!(bindings.contains(&(name.to_string(), want.to_string())), "`{name}` should be {want}: {bindings:?}");
    }
    // GN-11: an element passed where a nature is wanted binds the handle of that one carrier (QC3)
    StandardTopCheck::assert_clean("fn level(a: Bit) -> bool { a }\nmodule Top(in clk: Bit) { signal l: [Bit; 4] = [false; 4]; }\ndigital for Top { on clk.rise() { let x = level(l[2]); } }");
    // a constant element past the end is E0104 at elaboration (CV-13), proved by `elab_carrier_arrays`
}

#[test]
fn a_carrier_method_on_a_whole_array_is_e0211() {
    // CV-24 AC 3, §5.1.7: `.prev()`, `.changed()`, `.rise()` on a whole array of carriers is E0211 (raised where the
    // POM walks the receiver), and the checker adds no P0001 that would mask it
    for call in ["m.prev()", "m.changed()"] {
        let src = format!("module Top(in clk: Bit) {{ signal m: [Bit<u8>; 4] = [0; 4]; }}\ndigital for Top {{ on clk.rise() {{ let a = {call}; }} }}");
        StandardTopCheck::assert_pom_refusal(&src, "E0211");
    }
    StandardTopCheck::assert_pom_refusal("module Top() { signal l: [Bit; 4] = [false; 4]; }\ndigital for Top { on l.rise() { } }", "E0211");
}

/// The module `Top` of the commit and definition tests: arrays of carriers, a struct-valued carrier and a carrier of
/// a value array.
const TARGETS: &str = "#[derive(Repr)] struct S { n: u8, ok: bool }\n\
    module Top(in clk: Bit, in at: Bit<u2>, out q: Bit<u8> = 0) {\n\
    signal bits: [Bit; 4] = [false; 4];\n\
    signal word: Bit<[bool; 8]> = [false; 8];\n\
    signal st: Bit<S> = S { n: 0, ok: false };\n\
    signal mem: [Bit<S>; 4] = [S { n: 0, ok: false }; 4];\n}\n";

#[test]
fn element_and_field_targets_type_their_levels() {
    // CV-25 AC 1, AC 2, §12.1.9: the value committed has the type of the levels the target names: an element
    // (`bits[3]`, constant or at run time), a field (`st.n`), an element of a value array (`word[2]`) and a field of
    // an element (`mem[at].n`)
    let reaction = |commits: &str| format!("{TARGETS}digital for Top {{ on clk.rise() {{ {commits} }} }}");
    StandardTopCheck::assert_clean(&reaction("bits[3] <- true; bits[at] <- false; word[2] <- true; st.n <- 3; mem[at].n <- 5; mem[1].ok <- true; q <- 1;"));
    // a value of another type than the named levels is E0432, field or element
    StandardTopCheck::assert_one(&reaction("st.n <- true;"), "E0432", "");
    StandardTopCheck::assert_one(&reaction("bits[1] <- 3;"), "E0432", "");
    StandardTopCheck::assert_one(&reaction("word[2] <- 7;"), "E0432", "");
    // §8.2.10: an `in` port is never a target, and a whole array of carriers is no target (name an element)
    StandardTopCheck::assert_one("module Top(in l: [Bit; 2]) { }\ndigital for Top { on l[0].rise() { l[1] <- true; } }", "E0222", "in");
    StandardTopCheck::assert_one(&reaction("bits <- [true; 4];"), "P0001", "name an element");
    // an element of a scalar carrier is no place: indexing a `bool` is the ordinary mismatch
    StandardTopCheck::assert_one("module Top(in clk: Bit, out q: Bit = false) { }\ndigital for Top { on clk.rise() { q[0] <- true; } }", "E0180", "an array");
    // a definition of an element has the element's type (E0337)
    StandardTopCheck::assert_clean(&format!("{TARGETS}"));
    StandardTopCheck::assert_one("module Top(in a: Bit, out b: [Bit; 2]) { b[0] = a; b[1] = 3; }", "E0337", "");
    StandardTopCheck::assert_clean("module Top(in a: Bit, out b: [Bit; 2]) { b[0] = a; b[1] = !a; }");
}

#[test]
fn a_definition_and_commits_of_one_element_are_e0221() {
    // CV-25 AC 3, §5.2.4: one element has one drive. A definition of `bits[0]` and a commit of `bits[0]` are E0221; a
    // commit of `bits[1]` beside it is accepted; two commits of one element are accepted (the later statement wins)
    let module = |initial: &str, body: &str, reaction: &str| format!("module Top(in clk: Bit) {{ signal bits: [Bit; 4]{initial}; {body} }}\ndigital for Top {{ on clk.rise() {{ {reaction} }} }}");
    StandardTopCheck::assert_one(&module(" = [false; 4]", "bits[0] = true;", "bits[0] <- false;"), "E0221", "one element");
    // the initial value of the array is the value of the elements no undelayed definition names (§8.2.21)
    StandardTopCheck::assert_clean(&module(" = [false; 4]", "bits[0] = true;", "bits[1] <- false;"));
    StandardTopCheck::assert_clean(&module(" = [false; 4]", "", "bits[0] <- true; bits[0] <- false;"));
    // a commit at a run-time index may reach any element, so it shares one with a definition
    StandardTopCheck::assert_one("module Top(in clk: Bit, in at: Bit<u2>) { signal bits: [Bit; 4] = [false; 4]; bits[0] = true; }\ndigital for Top { on clk.rise() { bits[at] <- true; } }", "E0221", "");
    // two definitions of one element share a level (E0335); of two elements they do not
    StandardTopCheck::assert_one("module Top(in a: Bit) { signal bits: [Bit; 2] = [false; 2]; bits[0] = a; bits[0] = !a; }", "E0335", "");
    StandardTopCheck::assert_clean("module Top(in a: Bit) { signal bits: [Bit; 2]; bits[0] = a; bits[1] = !a; }");
}

/// Child modules the connection tests place: a four-bit register, a one-bit cell and a ladder of nodes.
const CHILDREN: &str = "module Reg(in d: [Bit; 4], out q: [Bit; 4]) { q[0] = d[0]; q[1] = d[1]; q[2] = d[2]; q[3] = d[3]; }\n\
    module Cell(in d: Bit, out q: Bit) { q = d; }\n\
    module Ladder(a: [Electrical; 4]) { }\n\
    module Tap(p: Electrical) { }\n";

#[test]
fn arrays_and_elements_connect_to_ports_of_their_shape() {
    // CV-23 AC 2: an array to an array port of the same shape, an element `x[k]` to a scalar port, a sub-array `g[r]`
    // of a nested array to an array port, an element of a node array to a node port, a node array to an array of nodes
    // an array handed to an `out` port is driven by the instance and has no initial value; the others start at `false`.
    // Each element needs its driver (§3.3.4): an element no instance holds has no value, so the array is E0187 naming it
    let top = |arrays: &str, body: &str| format!("{CHILDREN}module Top(in x: [Bit; 4]) {{ {arrays} signal bl: [Electrical; 4]; {body} }}");
    let driven = "signal y: [Bit; 4];";
    StandardTopCheck::assert_clean(&top(driven, "let r = Reg(x, y);"));
    StandardTopCheck::assert_one(&top(driven, "let c = Cell(x[1], y[2]);"), "E0187", "`y[0..2], y[3]`");
    StandardTopCheck::assert_clean(&top("signal g: [[Bit; 4]; 2] = [[false; 4]; 2]; signal h: [[Bit; 4]; 2];", "let r = Reg(g[0], h[1]); let s = Reg(g[1], h[0]);"));
    StandardTopCheck::assert_clean(&top("", "let l = Ladder(bl); let t = Tap(bl[3]);"));
    // two elements of one array handed to two `out` ports are two drives of two elements, not two of one
    StandardTopCheck::assert_clean(&top("signal y: [Bit; 2];", "let a = Cell(x[0], y[0]); let b = Cell(x[1], y[1]);"));
    StandardTopCheck::assert_one(&top("signal y: [Bit; 1];", "let a = Cell(x[0], y[0]); let b = Cell(x[1], y[0]);"), "E0220", "second");
}

#[test]
fn an_array_argument_of_another_length_is_e0313_naming_both_lengths() {
    // CV-23 AC 3, §8.1.10: an array argument and its port differ in length: E0313 naming both
    let top = |body: &str| format!("{CHILDREN}module Top() {{ signal short: [Bit; 3] = [false; 3]; signal q: [Bit; 4] = [false; 4]; signal one: Bit = false; signal bl: [Electrical; 3]; {body} }}");
    let found = StandardTopCheck::found(&top("let r = Reg(short, q);"));
    assert!(found.iter().any(|(c, m)| c == "E0313" && m.contains("; 3]") && m.contains("; 4]")), "{found:?}");
    // a single carrier at an array port, an array at a scalar port, and nodes of another count
    StandardTopCheck::assert_one(&top("let r = Reg(one, q);"), "E0313", "");
    StandardTopCheck::assert_one(&top("let c = Cell(short, one);"), "E0313", "");
    StandardTopCheck::assert_one(&top("let l = Ladder(bl);"), "E0313", "");
}

#[test]
fn an_initial_value_is_the_value_of_the_elements_no_out_port_holds_and_e0223_once_all_are_handed() {
    // DC4, §5.2.6, §8.2.21: the instance holds the drive of the element handed to it, so the initial value is the value of
    // the elements no `out` port holds; it is given twice (E0223) only once every element is handed
    let top = |decl: &str, body: &str| format!("{CHILDREN}module Top(in d: Bit) {{ {decl}\n{body} }}");
    StandardTopCheck::assert_clean(&top("signal bits: [Bit; 2] = [false; 2];", "let c = Cell(d, bits[1]);"));
    StandardTopCheck::assert_clean(&top("signal bits: [Bit; 4] = [false; 4];", "let c = Cell(d, bits[3]); let e = Cell(d, bits[0]);"));
    StandardTopCheck::assert_one(&top("signal bits: [Bit; 2] = [false; 2];", "let a = Cell(d, bits[0]); let c = Cell(d, bits[1]);"), "E0223", "every element");
    // a run of a nested array hands its elements: the rows left keep the initial value
    StandardTopCheck::assert_clean(&top("signal g: [[Bit; 4]; 2] = [[false; 4]; 2]; signal h: [[Bit; 4]; 2] = [[false; 4]; 2];", "let r = Reg(g[0], h[1]);"));
    StandardTopCheck::assert_one(&top("signal h: [Bit; 4] = [false; 4]; signal q: [Bit; 4] = [false; 4];", "let r = Reg(h, q);"), "E0223", "`q`");
}

#[test]
fn an_array_of_carriers_without_a_default_is_judged_per_element() {
    // §3.3.4, §8.2.20: an element no undelayed definition and no `out` port names has no value and no initial value, so the
    // array is E0187 naming the elements left (CV-23 AC 2), not exempt because one element has a driver
    let top = |declaration: &str, body: &str| format!("{CHILDREN}module Top(in clk: Bit) {{ {declaration} {body} }}\ndigital for Top {{ on clk.rise() {{ }} }}");
    // one element defined: the others are named
    StandardTopCheck::assert_one(&top("signal b: [Bit; 4];", "b[0] = clk;"), "E0187", "`b[1..4]`");
    StandardTopCheck::assert_one(&top("signal b: [Bit; 4];", "b[2] = clk;"), "E0187", "`b[0..2], b[3]`");
    // one element handed to an `out` port: the others are named
    StandardTopCheck::assert_one(&top("signal y: [Bit; 4];", "let c = Cell(clk, y[2]);"), "E0187", "`y[0..2], y[3]`");
    // an element defined and another handed over: the rest is named, and the two together are not the whole
    StandardTopCheck::assert_one(&top("signal y: [Bit; 3];", "y[0] = clk; let c = Cell(clk, y[1]);"), "E0187", "`y[2]`");
    // a delayed definition and a commit specify no value
    StandardTopCheck::assert_one(&top("signal b: [Bit; 2];", "b[0] = clk; b[1] = clk after 1n;"), "E0187", "`b[1]`");
    // every element named, by definitions, by hand-overs or by both: no E0187
    StandardTopCheck::assert_clean(&top("signal b: [Bit; 2];", "b[0] = clk; b[1] = !clk;"));
    StandardTopCheck::assert_clean(&top("signal y: [Bit; 2];", "let a = Cell(clk, y[0]); let c = Cell(clk, y[1]);"));
    StandardTopCheck::assert_clean(&top("signal y: [Bit; 2];", "y[0] = clk; let c = Cell(clk, y[1]);"));
    // a nested array names its elements row-major
    StandardTopCheck::assert_one(&top("signal g: [[Bit; 2]; 2];", "g[1][0] = clk;"), "E0187", "`g[0..2], g[3]`");
}

#[test]
fn an_initial_value_is_the_value_of_the_elements_no_undelayed_definition_names() {
    // §8.2.21 "on the levels defined": the initial value of an array applies to the elements left, so it is E0338 only
    // when every element is defined
    let top = |declaration: &str, body: &str| format!("module Top(in clk: Bit) {{ {declaration} {body} }}\ndigital for Top {{ on clk.rise() {{ }} }}");
    StandardTopCheck::assert_clean(&top("signal b: [Bit; 4] = [false; 4];", "b[0] = clk;"));
    StandardTopCheck::assert_clean(&top("signal b: [Bit; 2] = [false; 2];", "b[0] = clk; b[1] = clk after 1n;"));
    StandardTopCheck::assert_one(&top("signal b: [Bit; 2] = [false; 2];", "b[0] = clk; b[1] = !clk;"), "E0338", "every element");
    // a whole undelayed definition is refused (P0001) and an initial value beside it was E0338 before it: unchanged
    StandardTopCheck::assert_one(&top("signal s: Bit = false;", "s = clk;"), "E0338", "initial value");
}

#[test]
fn a_refused_whole_array_definition_is_the_driver_and_raises_no_e0187() {
    // the P0001 of a definition of a whole array (or of a sub-array) is the one diagnostic: the array counts as driven
    let found = StandardTopCheck::found("module Top(in x: Bit) { signal s: [Bit; 2]; s = x; }");
    let codes: Vec<&str> = found.iter().map(|(code, _)| code.as_str()).collect();
    assert_eq!(codes, ["P0001"], "{found:?}");
    let found = StandardTopCheck::found("module Top(in x: Bit) { signal g: [[Bit; 2]; 2]; g[0] = x; }");
    let codes: Vec<&str> = found.iter().map(|(code, _)| code.as_str()).collect();
    assert_eq!(codes, ["P0001"], "{found:?}");
}
