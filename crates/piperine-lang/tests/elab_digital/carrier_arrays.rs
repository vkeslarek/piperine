//! Arrays of carriers and of nodes elaborated (composite-values CV-23 to CV-26; docs/spec part II §5.1.8): the
//! declaration of an array of discrete carriers as one sparse carrier with a count and an initial value of either form,
//! an array of nodes as consecutive nodes under one name, element reads, history and events, node arrays in `analog`,
//! element and field commits and drives, and connections by window. Each test states the rule it proves.


use piperine_ir::{Constant, Ir, ValueType};
use piperine_lang::pom::{DeclKind, Reflect};
use piperine_lang_fixtures::elaboration::SourceElaboration;
use piperine_lang_fixtures::refusals::ElaborationRefusals;

/// What a test reads of an elaborated body.
struct Read<'a> {
    ir: &'a Ir,
}

impl Read<'_> {
    /// The structural text of body `Top` of `src`, which elaborates and checks.
    fn structure_of(src: &str) -> String {
        SourceElaboration::ir(src, "Top").print_structural()
    }

    /// The `(name, count, element type, initial value)` of each carrier of body `name`.
    fn carriers(&self, name: &str) -> Vec<(String, Option<u64>, ValueType, Option<Constant>)> {
        let body = self.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`:\n{}", self.ir.print_structural()));
        body.carriers().iter().map(|c| (c.name.clone(), c.count, c.ty.clone(), c.init)).collect()
    }

    /// The names of the nodes of body `name`, and its arrays of nodes as `(name, first, count)`.
    fn nodes(&self, name: &str) -> (Vec<String>, Vec<(String, u32, u32)>) {
        let body = self.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`"));
        (body.nodes().iter().map(|n| n.name.clone()).collect(), body.node_arrays().iter().map(|a| (a.name.clone(), a.first, a.count)).collect())
    }
}

#[test]
fn arrays_of_carriers_and_nodes_declare_one_carrier_and_k_nodes() {
    // CV-23 AC 1, CV-27 AC 1: `signal x: [N; K]` of a discrete nature is one carrier of `K` elements (no carrier per
    // element), `[x; K]` its initial value for every element, `[a, b, c]` one for each; an array of nodes is `K` nodes
    // named `bl[0]` to `bl[K-1]` and one `NodeArray`
    let src = "module Top() {\n\
               signal mem: [Bit<u8>; 1048576] = [7; 1048576];\n\
               signal row: [Bit<u8>; 3] = [1, 2, 3];\n\
               signal flags: [Bit; 4] = [false; 4];\n\
               signal grid: [[Bit; 2]; 3] = [[true; 2]; 3];\n\
               signal bl: [Electrical; 4];\n}";
    let ir = SourceElaboration::ir(src, "Top");
    let read = Read { ir: &ir };
    let carriers = read.carriers("Top");
    let by = |name: &str| carriers.iter().find(|c| c.0 == name).unwrap_or_else(|| panic!("no carrier `{name}`: {carriers:?}")).clone();
    // a million elements are one carrier whose initial value is one element's value (§5.1.8: nothing proportional to K)
    assert_eq!((by("mem").1, by("mem").2, by("mem").3), (Some(1 << 20), ValueType::UInt(8), Some(Constant::Int(7))));
    assert_eq!(by("flags").1, Some(4), "`[Bit; 4]` is one carrier of four elements");
    assert_eq!((by("grid").1, by("grid").3), (Some(6), Some(Constant::Bool(true))), "a nested array is one carrier of the product, row-major");
    // `[a, b, c]` is an array constant of every element's value
    assert!(matches!(by("row").3, Some(Constant::Aggregate(_))) && by("row").1 == Some(3), "{:?}", by("row"));
    assert_eq!(carriers.len(), 4, "no carrier per element: {carriers:?}");
    let (nodes, arrays) = read.nodes("Top");
    assert_eq!(nodes, ["bl[0]", "bl[1]", "bl[2]", "bl[3]"], "K nodes, named by element");
    assert_eq!(arrays, [("bl".to_string(), 0, 4)], "one name for the array");
}

#[test]
fn an_array_of_nodes_is_one_node_per_element() {
    // CV-23 AC 1 (UNBREAKABLE RULE, the POM mirrors the source): `signal bl: [Electrical; 4]` is four nodes `bl[0]` to
    // `bl[3]` in the IR, while the POM keeps the one declaration `bl` as written
    let src = "module Top() { signal bl: [Electrical; 4]; }";
    let design = SourceElaboration::design(src);
    let declared: Vec<_> = design.decls().into_iter().filter(|d| d.name() == Some("bl")).collect();
    assert_eq!(declared.len(), 1, "one declaration in the POM, not one per element");
    assert!(declared.iter().all(|d| matches!(d.kind(), DeclKind::Signal(_))));
    let ir = SourceElaboration::ir(src, "Top");
    let (nodes, arrays) = Read { ir: &ir }.nodes("Top");
    assert_eq!(nodes, ["bl[0]", "bl[1]", "bl[2]", "bl[3]"], "four nodes in the IR");
    assert_eq!(arrays, [("bl".to_string(), 0, 4)]);
}

#[test]
fn an_array_of_length_zero_is_an_empty_array() {
    // §1.2.3 (the length is known and not negative): `[Bit<u8>; 0]` elaborates (no P0004), holds no carrier since it
    // holds no element, reads whole as the empty array, and every element of it is out of range (E0104)
    let src = "module Top(in clk: Bit) { signal none: [Bit<u8>; 0]; signal one: [Bit<u8>; 1] = [3; 1]; }\n\
               digital for Top { on clk.rise() { let all = none; let kept = one[0]; } }";
    let ir = SourceElaboration::ir(src, "Top");
    let carriers = Read { ir: &ir }.carriers("Top");
    assert_eq!(carriers.iter().map(|c| (c.0.as_str(), c.1)).collect::<Vec<_>>(), [("clk", None), ("one", Some(1))], "no carrier for an array of no element");
    for (body, form) in [("let a = none[0];", "a constant element"), ("let a = none[clk as u8];", "a run-time element"), ("none[0] <- 1;", "an element commit"), ("let a = none[0].prev();", "a history read")] {
        let src = format!("module Top(in clk: Bit) {{ signal none: [Bit<u8>; 0]; }}\ndigital for Top {{ on clk.rise() {{ {body} }} }}");
        let found = SourceElaboration::run(&src, "Top").expect_err(form);
        assert!(found.iter().any(|d| d.diagnostic.code == "E0104" && d.diagnostic.message.contains("array of 0 carriers")), "{form}: {:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
    // an empty array connects to an empty array port, in either direction, and an array of no node names no node
    for port in ["in", "out"] {
        let src = format!("module Leaf({port} d: [Bit; 0], in c: Bit) {{ }}\nmodule Top() {{ signal none: [Bit; 0]; signal c: Bit = false; let l = Leaf(none, c); }}");
        SourceElaboration::ir(&src, "Top");
    }
    let ir = SourceElaboration::ir("module Top() { signal bl: [Electrical; 0]; }", "Top");
    assert_eq!(Read { ir: &ir }.nodes("Top"), (Vec::new(), Vec::new()), "no node and no node array");
    // the body keeps only the name of an array of no element, for the host that names it (CV-28)
    let ir = SourceElaboration::ir("module Top() { signal bl: [Electrical; 0]; signal none: [Bit<u8>; 0]; }", "Top");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert_eq!(body.empty_arrays().iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["bl", "none"], "each empty array by name, nodes first");
    // an empty array of a hoisted placement is recorded under the placement's prefix (finding 20)
    let ir = SourceElaboration::ir("module Hoist() -> f64 { signal e: [Bit<u8>; 0]; 1.0 }\nmodule Top() { let p = Hoist(); }", "Top");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert_eq!(body.empty_arrays().iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["p.e"], "the hoisted array under its placement");
    let found = SourceElaboration::run("module Top() { signal bl: [Electrical; 0]; }\nanalog for Top { let x = v(bl[0]); }", "Top").expect_err("no node");
    assert!(found.iter().any(|d| d.diagnostic.code == "E0104"), "{:?}", SourceElaboration::codes(&found));
}

#[test]
fn a_logic_literal_is_a_carriers_initial_value() {
    // §22.1.4 rule 2, §5.1.5: `signal bus: Bit<Logic<4>> = logic("10xx")` starts at the two planes the pattern names, the
    // way `logic` reads in a reaction; an array of them starts every element at it
    let ir = SourceElaboration::ir("module Top() { signal bus: Bit<Logic<4>> = logic(\"10xx\"); signal each: [Bit<Logic<2>>; 3] = [logic(\"1z\"); 3]; }", "Top");
    let carriers = Read { ir: &ir }.carriers("Top");
    let planes = |name: &str| match carriers.iter().find(|c| c.0 == name).and_then(|c| c.3) {
        Some(Constant::Aggregate(id)) => ir.aggregate(id).parts.clone(),
        other => panic!("`{name}` starts at an aggregate: {other:?}"),
    };
    // §22.1.1: a level is `(binary, value)`: `0` is `(1, 0)`, `1` is `(1, 1)`, X is `(0, 0)`, Z is `(0, 1)`; the pattern lists the
    // highest level first, so `10xx` has binary `0b1100` and value `0b1000`
    assert_eq!(planes("bus"), [Constant::Int(0b1100), Constant::Int(0b1000)]);
    assert_eq!(carriers.iter().find(|c| c.0 == "each").map(|c| c.1), Some(Some(3)));
    assert!(matches!(carriers.iter().find(|c| c.0 == "each").and_then(|c| c.3), Some(Constant::Aggregate(_))));
}

#[test]
fn element_reads_and_events_lower_per_element() {
    // CV-24: `x[i]` is one element read (constant or run-time index), the whole `x` an array of the values,
    // `x[k].prev()` the element's previous value, `x[i].changed()` the element's change as a `bool`, and
    // `on x[k].rise()` an event on that element only, guarded by its edge
    let src = "module Top(in clk: Bit, in at: Bit<u2>, out q: Bit<u8> = 0) {\n\
               signal mem: [Bit<u8>; 4] = [0; 4];\n\
               signal lines: [Bit; 4] = [false; 4];\n\
               signal grid: [[Bit<u8>; 2]; 3] = [[0; 2]; 3];\n}\n\
               digital for Top {\n\
               on clk.rise() { let a = mem[1]; let b = mem[at]; let p = mem[2].prev(); let c = lines[at].changed(); let g = grid[2][1]; q <- a + b + p; }\n\
               on lines[2].rise() { }\n\
               on mem[3].changed() { }\n}";
    let text = Read::structure_of(src);
    for needle in ["mem[", ".prev", "lines["] {
        assert!(text.contains(needle), "the structure names `{needle}`:\n{text}");
    }
    let ir = SourceElaboration::ir(src, "Top");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    // the events are the elements' own: `lines[2]` (a rise) and `mem[3]`, never the whole carriers
    let events: Vec<_> = body.events().iter().map(|e| e.source.clone()).collect();
    let lines = body.carriers().iter().position(|c| c.name == "lines").expect("lines") as u32;
    let mem = body.carriers().iter().position(|c| c.name == "mem").expect("mem") as u32;
    assert!(events.contains(&piperine_ir::EventSource::ElementChanged { carrier: lines, element: 2 }), "{events:?}");
    assert!(events.contains(&piperine_ir::EventSource::ElementChanged { carrier: mem, element: 3 }), "{events:?}");
    let rise = body.events().iter().find(|e| e.source == piperine_ir::EventSource::ElementChanged { carrier: lines, element: 2 }).expect("the rise event");
    assert!(rise.guard.is_some(), "a rise carries its edge guard (!prev && now)");
    // §1 rule 11: a constant element past the end is E0104 naming it; an event head that does not fold is E0188 naming it
    let past = SourceElaboration::run("module Top() { signal m: [Bit<u8>; 4] = [0; 4]; }\ndigital for Top { on m[0].changed() { let a = m[4]; } }", "Top").expect_err("past the end");
    assert!(past.iter().any(|d| d.diagnostic.code == "E0104" && d.diagnostic.message.contains('4')), "{:?}", SourceElaboration::codes(&past));
    let folding = SourceElaboration::run("module Top(in at: Bit<u2>) { signal l: [Bit; 4] = [false; 4]; }\ndigital for Top { on l[at].rise() { } }", "Top").expect_err("no fold");
    assert!(folding.iter().any(|d| d.diagnostic.code == "E0188" && d.diagnostic.message.contains("`l[at]`")), "{:?}", SourceElaboration::codes(&folding));
}

#[test]
fn an_array_length_read_from_a_param_makes_it_structural() {
    // CV-36 AC 1 (§8.1.2, QC2): `[Bit; n]` and `[Electrical; n]` read the param `n`, so each body variant folds the
    // length of its own `n`, and `n` is structural (a host `set` of it during a run is `E0803`)
    let src = "module Wide(in d: [Bit; 2]) { n: u32 = 3; signal s: [Bit<u8>; n] = [0; n]; signal bl: [Electrical; n + 1]; }\n\
               module Top() { signal d: [Bit; 2] = [false; 2]; let w = Wide { n: 5 }(d); }";
    let ir = SourceElaboration::ir(src, "Top");
    let read = Read { ir: &ir };
    let variant = ir.bodies().find(|b| b.name().starts_with("Wide~n=")).expect("a body of the variant");
    assert!(variant.params().iter().find(|p| p.name == "n").expect("param n").structural, "`n` is structural");
    let carriers = read.carriers(variant.name());
    assert_eq!(carriers.iter().find(|c| c.0 == "s").expect("s").1, Some(5), "the array is as long as the param says");
    assert_eq!(variant.node_arrays().iter().map(|a| (a.name.as_str(), a.count)).collect::<Vec<_>>(), [("bl", 6)], "`n + 1` nodes");
    // the default is another variant: its own length
    let ir = SourceElaboration::ir("module Wide() { n: u32 = 3; signal s: [Bit<u8>; n] = [0; n]; }\nmodule Top() { let w = Wide(); }", "Top");
    let read = Read { ir: &ir };
    let name = ir.bodies().find(|b| b.name().starts_with("Wide")).expect("Wide").name().to_string();
    assert_eq!(read.carriers(&name).iter().find(|c| c.0 == "s").expect("s").1, Some(3));
}

#[test]
fn an_array_whose_length_reads_a_param_reads_and_writes_whole() {
    // CV-36 AC 1 and CV-24 AC 1 (QC2, QC5): the length folds per body variant wherever the type is read, so the whole
    // read of `in d: [Bit<u8>; n]`, the value array `[1; n]` of a reaction and `[1.0; n]` of `analog` all elaborate,
    // each variant with the length of its own `n`
    let src = "module Wide(in d: [Bit<u8>; n], in clk: Bit, out q: Bit<u8> = 0) { n: u32 = 3; signal bl: [Electrical; n]; }\n\
               digital for Wide { on clk.rise() { let w = d; let ones: [u8; n] = [1; n]; q <- w[0] + ones[1]; } }\n\
               analog for Wide { let zeros: [f64; n] = [1.0; n]; let first = zeros[0]; }\n\
               module Top() { signal d: [Bit<u8>; 5] = [0; 5]; signal clk: Bit = false; signal q: Bit<u8>; signal d3: [Bit<u8>; 3] = [0; 3]; signal q3: Bit<u8>; let w = Wide { n: 5 }(d, clk, q); let v = Wide(d3, clk, q3); }";
    let ir = SourceElaboration::ir(src, "Top");
    assert!(ir.bodies().any(|b| b.name().starts_with("Wide~n=")), "the variant of n = 5 is a body");
    assert_eq!(ir.bodies().filter(|b| b.name().starts_with("Wide")).count(), 2, "the default n = 3 is a body of its own");
    // each variant reads its own folded length, by value: the node array `bl`, and the whole read of the port `d`
    let read = Read { ir: &ir };
    let printed = ir.print_structural();
    for (body, length) in [("Wide~n=5", 5u32), ("Wide~n=3", 3)] {
        let (_, arrays) = read.nodes(body);
        assert_eq!(arrays.iter().find(|a| a.0 == "bl").map(|a| a.2), Some(length), "the node array `bl` of {body}");
        assert!(printed.contains(&format!("local w: [u8; {length}]")) && printed.contains(&format!("local ones: [u8; {length}]")), "the locals of {body} have length {length}:\n{printed}");
    }
    let found = SourceElaboration::run(&src.replace("[1; n]", "[1; q]"), "Top").expect_err("a length that reads no param");
    assert_eq!(SourceElaboration::codes(&found), ["E0180"], "a repeat count that reads nothing is refused by its code");
}

#[test]
fn an_argument_of_another_length_than_a_param_length_is_e0313_at_elaboration() {
    // CV-23 AC 3 (§8.1.10): `Child(in d: [Bit<u8>; n])` with n = 3 given 4 elements is E0313 naming both lengths when
    // the design elaborates, not an error of the bind; the same for an array of nodes; a matching length elaborates
    let child = "module Child(in d: [Bit<u8>; n], e: [Electrical; n + 1]) { n: u32 = 3; }\n";
    let top = |given: u32, nodes: u32| format!("{child}module Top() {{ signal s: [Bit<u8>; {given}] = [0; {given}]; signal bl: [Electrical; {nodes}]; let c = Child(s, bl); }}");
    SourceElaboration::ir(&top(3, 4), "Top");
    for (given, nodes, found_length, wanted_length) in [(4, 4, "[4]", "[3]"), (3, 5, "[5]", "[4]")] {
        let found = SourceElaboration::run(&top(given, nodes), "Top").expect_err("a length that is not the port's");
        let mismatch = found.iter().find(|d| d.diagnostic.code == "E0313").unwrap_or_else(|| panic!("E0313 expected: {:?}", SourceElaboration::codes(&found)));
        assert!(mismatch.diagnostic.message.contains(found_length) && mismatch.diagnostic.message.contains(wanted_length), "both lengths are named: {}", mismatch.diagnostic.message);
    }
    // the port's length follows the placement's own `n`
    SourceElaboration::ir(&format!("{child}module Top() {{ signal s: [Bit<u8>; 5] = [0; 5]; signal bl: [Electrical; 6]; let c = Child {{ n: 5 }}(s, bl); }}"), "Top");
}

#[test]
fn a_negative_array_length_from_a_param_is_e0104_naming_it() {
    // §1.2.3 (a length is known and not negative): `n = -1` makes `[Bit; n]` a panic of the const expression, E0104
    // (§1 rule 11, §3.9.7), naming the length - not P0001, which would call it a missing feature
    for (declared, what) in [("signal s: [Bit; n] = [false; n];", "carriers"), ("signal s: [Electrical; n];", "nodes")] {
        let src = format!("module Top() {{ n: i32 = -1; {declared} }}");
        let found = SourceElaboration::run(&src, "Top").expect_err("a negative length");
        let negative = found.iter().find(|d| d.diagnostic.code == "E0104").unwrap_or_else(|| panic!("E0104 expected: {:?}", SourceElaboration::codes(&found)));
        assert!(negative.diagnostic.message.contains("-1") && negative.diagnostic.message.contains(what), "{}", negative.diagnostic.message);
        assert!(found.iter().all(|d| d.diagnostic.code != "P0001"), "no missing-feature refusal: {:?}", SourceElaboration::codes(&found));
    }
}

#[test]
fn an_element_binds_a_handle_parameter() {
    // GN-11, QC3: `f(lines[k])` binds the handle to that one carrier, so `.prev()` and a read inside `f` are the
    // element's
    let src = "fn was(a: Bit) -> bool { a.prev() }\n\
               module Top(in clk: Bit, out q: Bit = false) { signal lines: [Bit; 4] = [false; 4]; }\n\
               digital for Top { on clk.rise() { q <- was(lines[2]); } }";
    let text = Read::structure_of(src);
    assert!(text.contains("lines") && text.contains(".prev"), "the handle reads the element's previous value:\n{text}");
}

#[test]
fn node_array_contributions_unroll_to_their_nodes() {
    // CV-26 AC 1: a loop of constant trip count that contributes to `bl[k]` and `bl[k+1]` is unrolled with `k` folded:
    // three branches between consecutive nodes of the array, `bl[0]` to `bl[3]`, a probe of one element reads that node
    let src = "module Top() {\n\
               signal bl: [Electrical; 4];\n}\n\
               analog for Top { for k in 0..3 { i(bl[k], bl[k + 1]) +> v(bl[k], bl[k + 1]) / 1000.0; } let tap = v(bl[2]); }";
    let ir = SourceElaboration::ir(src, "Top");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert_eq!(body.nodes().iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["bl[0]", "bl[1]", "bl[2]", "bl[3]"]);
    let branches: Vec<(piperine_ir::Terminal, piperine_ir::Terminal)> = body.branches().iter().map(|b| (b.plus, b.minus)).collect();
    use piperine_ir::Terminal::Node;
    assert_eq!(branches, vec![(Node(0), Node(1)), (Node(1), Node(2)), (Node(2), Node(3))], "one branch per iteration, on its two nodes");
}

#[test]
fn an_analog_node_index_that_does_not_fold_is_refused_naming_it() {
    // CV-26 AC 2, QC4: an index of a node array in `analog` that does not fold after unrolling is refused naming it; a
    // constant past the end is E0104
    let found = SourceElaboration::run("module Top() { signal bl: [Electrical; 4]; }\nanalog for Top { let k = 2; let x = v(bl[k]); }", "Top").expect_err("no fold");
    assert!(found.iter().any(|d| d.diagnostic.code == "E0188" && d.diagnostic.message.contains("`k`")), "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    let past = SourceElaboration::run("module Top() { signal bl: [Electrical; 4]; }\nanalog for Top { let x = v(bl[4]); }", "Top").expect_err("past the end");
    assert!(past.iter().any(|d| d.diagnostic.code == "E0104" && d.diagnostic.message.contains('4')), "{:?}", SourceElaboration::codes(&past));
}

/// The carriers, the commits and the drives of body `Top`, as the targets they name.
struct Writes {
    commits: Vec<(u32, bool, Vec<piperine_ir::Projection>)>,
    drives: Vec<(u32, Option<u64>, Vec<piperine_ir::Projection>)>,
}

impl Writes {
    /// What the reactions and drives of `Top` in `src` write.
    fn of(src: &str) -> Writes {
        let ir = SourceElaboration::ir(src, "Top");
        let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
        let mut commits = Vec::new();
        for index in 0..body.reactions().len() as u32 {
            for statement in body.reaction_statements(index) {
                if let piperine_ir::Stmt::Commit { carrier, target, .. } = ir.stmt(statement) {
                    commits.push((*carrier, target.as_ref().is_some_and(|t| t.element.is_some()), target.as_ref().map_or_else(Vec::new, |t| t.path.clone())));
                }
            }
        }
        let drives = body.drives().iter().map(|d| (d.carrier, d.element, d.path.clone())).collect();
        Writes { commits, drives }
    }
}

#[test]
fn element_and_field_commits_lower_to_their_targets() {
    // CV-25: `bits[3] <- x` and `bits[at] <- y` commit one element (constant or run time), `st.n <- 3` a field,
    // `word[2] <- true` an element of a value array, `mem[at].n <- 5` a field of an element
    let src = "#[derive(Repr)] struct S { n: u8, ok: bool }\n\
               module Top(in clk: Bit, in at: Bit<u2>) {\n\
               signal bits: [Bit; 4] = [false; 4];\n\
               signal word: Bit<[bool; 8]> = [false; 8];\n\
               signal st: Bit<S> = S { n: 0, ok: false };\n\
               signal mem: [Bit<S>; 4] = [S { n: 0, ok: false }; 4];\n}\n\
               digital for Top { on clk.rise() { bits[3] <- true; bits[at] <- false; word[2] <- true; st.n <- 3; mem[at].n <- 5; } }";
    let writes = Writes::of(src);
    let ir = SourceElaboration::ir(src, "Top");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    let at = |name: &str| body.carriers().iter().position(|c| c.name == name).expect(name) as u32;
    assert_eq!(writes.commits.len(), 5, "{:?}", writes.commits);
    // an element of an array carrier names its element and no field; a value-array element or a field names the path
    assert_eq!(writes.commits[0], (at("bits"), true, vec![]));
    assert_eq!(writes.commits[1], (at("bits"), true, vec![]));
    assert_eq!((writes.commits[2].0, writes.commits[2].1, writes.commits[2].2.len()), (at("word"), false, 1), "`word[2]` is one step into the value");
    assert_eq!(writes.commits[3], (at("st"), false, vec![piperine_ir::Projection::Field(0)]));
    assert_eq!((writes.commits[4].0, writes.commits[4].1, writes.commits[4].2.clone()), (at("mem"), true, vec![piperine_ir::Projection::Field(0)]));
}

#[test]
fn an_element_definition_is_one_drive() {
    // CV-25 AC 3: `bits[0] = a;` is the drive of element 0 only, so element 1 may be driven apart; a definition's
    // element folds, else E0188 names it
    let writes = Writes::of("module Top(in a: Bit) { signal bits: [Bit; 2]; bits[0] = a; bits[1] = !a; }");
    assert_eq!(writes.drives, vec![(1, Some(0), vec![]), (1, Some(1), vec![])], "one drive per element, of carrier 1 (`a` is carrier 0)");
    let found = SourceElaboration::run("module Top(in a: Bit, in at: Bit<u2>) { signal bits: [Bit; 4]; bits[at] = a; }", "Top").expect_err("no fold");
    assert!(found.iter().any(|d| d.diagnostic.code == "E0188" && d.diagnostic.message.contains("bits[at]")), "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}

#[test]
fn arrays_and_elements_bind_windows_of_child_ports() {
    // CV-23 AC 2: an array to an array port is one net on both sides (no window); an element `x[k]` to a scalar port
    // binds the window of that element, a sub-array `g[r]` of a nested array the run of its row; a node array gives
    // the child its nodes; the net table names one net per array and one node per element
    let src = "module Reg(in d: [Bit; 4], out q: [Bit; 4]) { q[0] = d[0]; q[1] = d[1]; q[2] = d[2]; q[3] = d[3]; }\n\
               module Cell(in d: Bit, out q: Bit) { q = d; }\n\
               module Ladder(a: [Electrical; 4]) { }\n\
               module Top(in x: [Bit; 4]) {\n\
               signal y: [Bit; 4];\n\
               signal g: [[Bit; 4]; 2] = [[false; 4]; 2];\n\
               signal h: [[Bit; 4]; 2];\n\
               signal z: [Bit; 2];\n\
               signal bl: [Electrical; 4];\n\
               let whole = Reg(x, y);\n\
               let row = Reg(g[1], h[0]);\n\
               let one = Cell(x[2], z[1]);\n\
               let rungs = Ladder(bl);\n\
               let row_rest = Reg(g[0], h[1]);\n\
               let zero = Cell(x[0], z[0]);\n}";
    let ir = SourceElaboration::ir(src, "Top");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    let windows: Vec<Vec<Option<piperine_ir::CarrierWindow>>> = (0..top.children().len()).map(|k| top.child_windows(k).to_vec()).collect();
    let win = |first: u64, length: Option<u64>| Some(piperine_ir::CarrierWindow { first, length });
    assert_eq!(windows[0], Vec::<Option<piperine_ir::CarrierWindow>>::new(), "a whole array to an array port needs no window");
    assert_eq!(windows[1], vec![win(4, Some(4)), win(0, Some(4))], "`g[1]` is the run of elements 4 to 7, `h[0]` the run 0 to 3");
    assert_eq!(windows[2], vec![win(2, None), win(1, None)], "`x[2]` and `z[1]` are one element each");
    // one net per array, named by the array; one node per element
    assert!(ir.nets().any(|n| n.path() == "Top.g") && ir.nets().all(|n| !n.path().starts_with("Top.g[")), "one net for the whole array");
    assert!(ir.nodes().any(|n| n.path() == "Top.bl[2]"), "one node per element");
    // the ladder's four nodes are the parent's four
    let rungs = ir.instance_at("Top.rungs").expect("the ladder");
    assert_eq!(rungs.bindings().len(), 4);
}

/// The `(code, message)` of each diagnostic of elaborating `Top` of `src`.
struct Refusals;

impl Refusals {
    /// Asserts `src` is refused with the one code `code` whose message contains each of `named`.
    fn assert_one(src: &str, code: &str, named: &[&str]) {
        let found = ElaborationRefusals::of(src);
        assert_eq!(found.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), [code], "{src}\n{found:?}");
        for text in named {
            assert!(found[0].1.contains(text), "`{text}` is named: {}", found[0].1);
        }
    }
}

/// A one-bit cell the per-instance tests place: it drives its `out` port.
const CELL: &str = "module Cell(in d: Bit, out q: Bit) { q = d; }\n";

#[test]
fn a_generic_length_array_is_judged_per_element_at_each_instance() {
    // §3.3.4, §8.2.20: `[Bit; N]` with only `b[0]` driven leaves `b[1..N]` with no value: E0187 naming them and the
    // instance `M::<4>`, though the length is known only once the instance is
    let module = |body: &str| format!("module M<const N: usize>(in clk: Bit) {{ signal b: [Bit; N]; {body} }}\n");
    let top = "module Top(in clk: Bit) { let m = M::<4>(clk); }";
    Refusals::assert_one(&format!("{}{top}", module("b[0] = clk;")), "E0187", &["`b[1..4]`", "`M::<4>`"]);
    // the same module at a length one drive covers is clean, and a length of two names the one element left
    Refusals::assert_one(&format!("{}module Top(in clk: Bit) {{ let m = M::<2>(clk); }}", module("b[0] = clk;")), "E0187", &["`b[1]`", "`M::<2>`"]);
    CleanElaboration::elaborates(&format!("{}module Top(in clk: Bit) {{ let m = M::<1>(clk); }}", module("b[0] = clk;")));
    // an element handed to an `out` port of a child names its element too
    Refusals::assert_one(&format!("{CELL}{}{top}", module("let c = Cell(clk, b[0]);")), "E0187", &["`b[1..4]`", "`M::<4>`"]);
}

/// Asserts that a source elaborates with no diagnostic.
struct CleanElaboration;

impl CleanElaboration {
    /// Elaborates `src` and asserts it holds no diagnostic.
    fn elaborates(src: &str) {
        SourceElaboration::ir(src, "Top");
    }
}

#[test]
fn a_param_length_array_is_judged_per_element_at_each_variant() {
    // §3.3.4, §8.1.2: `[Bit; n]` folds per body variant, so `Wide~n=4` with only `b[0]` driven is E0187 naming `b[1..4]`
    let module = "module Wide(in clk: Bit) { n: u32 = 2; signal b: [Bit; n]; b[0] = clk; }\n";
    Refusals::assert_one(&format!("{module}module Top(in clk: Bit) {{ let w = Wide {{ n: 4 }}(clk); }}"), "E0187", &["`b[1..4]`", "`Wide~n=4`"]);
    CleanElaboration::elaborates(&format!("{module}module Top(in clk: Bit) {{ let w = Wide {{ n: 1 }}(clk); }}"));
}

#[test]
fn a_generation_index_names_the_elements_it_reaches() {
    // §8.2.23: `for k in 0..2 { Cell(clk, y[k]) }` over `[Bit; 4]` hands elements 0 and 1: E0187 names `y[2..4]`; a loop
    // over every element is clean
    let top = |range: &str| format!("{CELL}module Top(in clk: Bit) {{ signal y: [Bit; 4]; for k in {range} {{ let c = Cell(clk, y[k]); }} }}");
    Refusals::assert_one(&top("0..2"), "E0187", &["`y[2..4]`"]);
    CleanElaboration::elaborates(&top("0..4"));
    // a definition inside generation drives the elements its iterations name: E0187 names the two it leaves (the refusal it
    // was is gone, generics finding 14), and a loop over every element is clean
    Refusals::assert_one("module Top(in clk: Bit) { signal y: [Bit; 4]; for k in 0..2 { y[k] = clk; } }", "E0187", &["`y[2..4]`", "`Top`"]);
    CleanElaboration::elaborates("module Top(in clk: Bit) { signal y: [Bit; 4]; for k in 0..4 { y[k] = clk; } }");
    // and in a generic module, at its length
    let generic = "module M<const N: usize>(in clk: Bit) { signal y: [Bit; N]; for k in 0..2 { let c = Cell(clk, y[k]); } }\n";
    Refusals::assert_one(&format!("{CELL}{generic}module Top(in clk: Bit) {{ let m = M::<4>(clk); }}"), "E0187", &["`y[2..4]`", "`M::<4>`"]);
    CleanElaboration::elaborates(&format!("{CELL}{generic}module Top(in clk: Bit) {{ let m = M::<2>(clk); }}"));
}

#[test]
fn an_initial_value_over_elements_no_out_port_holds_is_clean_and_over_every_element_is_e0223() {
    // §5.2.6, §8.2.21: the initial value of an array is the value of the elements no `out` port holds, so it is E0223
    // only once every element is handed; a generic length is judged at each instance
    let module = |handed: &str| format!("module M<const N: usize>(in clk: Bit) {{ signal y: [Bit; N] = [false; N]; {handed} }}\n");
    let top = |length: u32| format!("module Top(in clk: Bit) {{ let m = M::<{length}>(clk); }}");
    CleanElaboration::elaborates(&format!("{CELL}{}{}", module("let c = Cell(clk, y[0]);"), top(2)));
    Refusals::assert_one(&format!("{CELL}{}{}", module("let c = Cell(clk, y[0]);"), top(1)), "E0223", &["`y`", "`M::<1>`"]);
}

#[test]
fn an_initial_value_beside_definitions_of_every_element_is_e0338_at_the_instance_that_folds_so() {
    // §8.2.21: an initial value is the value of the elements no undelayed definition names; with a generic length it is
    // E0338 only at the instance whose elements the definitions all name
    let src = |length: u32| format!("module M<const N: usize>(in clk: Bit) {{ signal b: [Bit; N] = [false; N]; b[0] = clk; b[1] = !clk; }}\nmodule Top(in clk: Bit) {{ let m = M::<{length}>(clk); }}");
    Refusals::assert_one(&src(2), "E0338", &["`b`", "`M::<2>`"]);
    CleanElaboration::elaborates(&src(4));
}

#[test]
fn a_nested_array_with_a_generic_inner_length_judges_shared_elements_per_instance() {
    // §8.2.20: `g[1][0]` and `g[0][1]` of `[[Bit; N]; 2]` are different elements once `N` folds: every element defined once is
    // clean at `M::<2>`; a second definition of `g[1][1]` is E0335 naming the element and the instance
    let module = |extra: &str| format!("module M<const N: usize>(in clk: Bit) {{ signal g: [[Bit; N]; 2]; g[1][0] = clk; g[0][1] = clk; g[0][0] = clk; g[1][1] = clk; {extra} }}\n");
    let top = "module Top(in clk: Bit) { let m = M::<2>(clk); }";
    CleanElaboration::elaborates(&format!("{}{top}", module("")));
    Refusals::assert_one(&format!("{}{top}", module("g[1][1] = !clk;")), "E0335", &["`g[1][1]`", "`M::<2>`"]);
    // §5.2, §8.2.20: a definition and a commit of different elements compose; of one element they are E0221 at the instance
    let committed = |element: &str| format!("module M<const N: usize>(in clk: Bit) {{ signal g: [[Bit; N]; 2] = [[false; N]; 2]; g[1][0] = clk; }}\ndigital<N> for M<N> {{ on clk.rise() {{ {element} <- true; }} }}\n{top}");
    CleanElaboration::elaborates(&committed("g[0][1]"));
    Refusals::assert_one(&committed("g[1][0]"), "E0221", &["`g[1][0]`", "`M::<2>`"]);
    // §5.2.6: an element handed to an `out` port and defined here is E0220 at the instance; a different element is clean
    let handed = |element: &str| format!("{CELL}module M<const N: usize>(in clk: Bit) {{ signal g: [[Bit; N]; 2]; let c = Cell(clk, g[1][0]); g[0][0] = clk; g[0][1] = clk; g[1][1] = clk; {element} = !clk; }}\n{top}");
    Refusals::assert_one(&handed("g[1][0]"), "E0220", &["`g[1][0]`", "`M::<2>`"]);
}

#[test]
fn drives_written_in_generation_are_judged_where_generation_is_unrolled() {
    // §8.2.20, §8.2.23: `for k in 0..2 { b[k] = a; }` defines `b[0]` and `b[1]`, so `b[2]` and `b[3]` defined beside it are
    // clean, and `b[1]` defined again is E0335 naming it
    let fixed = |extra: &str| format!("module Top(in clk: Bit) {{ signal b: [Bit; 4]; for k in 0..2 {{ b[k] = clk; }} b[3] = !clk; b[2] = clk; {extra} }}");
    CleanElaboration::elaborates(&fixed(""));
    Refusals::assert_one(&fixed("b[1] = !clk;"), "E0335", &["`b[1]`", "`Top`"]);
    // §8.2.24: two definitions in exclusive arms of an `if` are one drive; one in every iteration of a loop is two
    CleanElaboration::elaborates("module Top(in clk: Bit) { mode: usize = 2; signal y: Bit; if mode > 1 { y = clk; } else { y = !clk; } }");
    CleanElaboration::elaborates("module Top(in clk: Bit) { signal y: [Bit; 2]; for k in 0..2 { if k == 0 { y[k] = clk; } else { y[k] = !clk; } } }");
    Refusals::assert_one("module Top(in clk: Bit) { signal y: Bit; for k in 0..2 { y = clk; } }", "E0335", &["`y`", "`Top`"]);
    // §5.2.6: placements in a loop hand `y[0]` and `y[1]`; `y[2]` and `y[3]` defined here are clean, `y[1]` is E0220
    let handed = |extra: &str| format!("{CELL}module Top(in clk: Bit) {{ signal y: [Bit; 4]; for k in 0..2 {{ let c = Cell(clk, y[k]); }} y[3] = clk; y[2] = clk; {extra} }}");
    CleanElaboration::elaborates(&handed(""));
    Refusals::assert_one(&handed("y[1] = clk;"), "E0220", &["`y[1]`", "`Top`"]);
    // §5.2: a definition in a loop and a commit of another element compose; of the same element they are E0221
    let committed = |element: &str| format!("module Top(in clk: Bit) {{ signal y: [Bit; 2] = [false; 2]; for k in 0..1 {{ y[k] = clk; }} }}\ndigital for Top {{ on clk.rise() {{ {element} <- true; }} }}");
    CleanElaboration::elaborates(&committed("y[1]"));
    Refusals::assert_one(&committed("y[0]"), "E0221", &["`y[0]`", "`Top`"]);
    // §8.2.21, §8.2.24: an initial value beside a definition in an arm not taken stays the carrier's value (clean); in the
    // taken arm it is E0338 at the instance, and with no initial value an untaken definition leaves `y` undriven (E0187)
    let arm = |mode: u32, init: &str| format!("module Top(in clk: Bit) {{ mode: usize = {mode}; signal y: Bit{init}; if mode > 1 {{ y = clk; }} }}");
    CleanElaboration::elaborates(&arm(0, " = false"));
    Refusals::assert_one(&arm(2, " = false"), "E0338", &["`y`", "`Top~mode=2`"]);
    Refusals::assert_one(&arm(0, ""), "E0187", &["`y`", "`Top~mode=0`"]);
}

#[test]
fn a_run_time_element_read_in_a_definition_is_one_read_of_the_array() {
    // §5.1.8, abi.md §3: `q = mem[at]` over 65 536 carriers is one read of `mem` at a run-time element (the array's view),
    // not one constant read per element: the drive's value holds no array literal, and the elaboration takes well under a
    // second for any length
    let src = "module Top(in at: Bit<u16>) { signal mem: [Bit<u8>; 65536] = [7; 65536]; signal q: Bit<u8>; q = mem[at]; }";
    let started = std::time::Instant::now();
    let ir = SourceElaboration::ir(src, "Top");
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "elaborated in {:?}", started.elapsed());
    let printed = ir.print_structural();
    let drive = printed.lines().find(|line| line.trim_start().starts_with("drive q")).expect("the drive of q");
    assert_eq!(drive.trim(), "drive q = mem[at]", "one read at the run-time element");
}

#[test]
fn a_loop_of_definitions_through_elements_is_e0339_naming_the_elements() {
    // §8.2.3: definitions that depend on each other with no reaction and no `after` between are a loop, an error through a
    // one-driver carrier, found at elaboration where elements fold (it was a bind cycle of the digital solver, after rustc,
    // naming internal functions): `b[1] = b[2] + 1; b[2] = b[1];` names both elements and the instance
    Refusals::assert_one("module Top(in clk: Bit<u8>) { signal b: [Bit<u8>; 3]; b[0] = clk; b[1] = b[2] + 1; b[2] = b[1]; }", "E0339", &["`b[1]`, `b[2]`", "`Top`"]);
    // a read at a run-time element may be the definition's own element: a loop
    Refusals::assert_one("module Top(in at: Bit<u8>) { signal b: [Bit<u8>; 3]; b[0] = at; b[1] = b[at] + 1; b[2] = 0; }", "E0339", &["`b[1]`", "`Top`"]);
    // a loop across two arrays, and one through a carrier defined in every iteration of a generation `for`
    Refusals::assert_one("module Top(in clk: Bit<u8>) { signal b: [Bit<u8>; 2]; signal c: [Bit<u8>; 2]; b[0] = clk; b[1] = c[1]; c[0] = clk; c[1] = b[1] + 1; }", "E0339", &["`b[1]`, `c[1]`"]);
    Refusals::assert_one("module Top(in clk: Bit<u8>) { signal y: Bit<u8>; for k in 0..1 { y = y + clk; } }", "E0339", &["`y`", "`Top`"]);
    // a chain through elements is no loop
    CleanElaboration::elaborates("module Top(in clk: Bit<u8>) { signal b: [Bit<u8>; 3]; b[0] = clk; b[1] = b[0] + 1; b[2] = b[1]; }");
}

#[test]
fn an_analog_function_reads_the_carrier_an_element_of_an_array_is_given_to() {
    // §5.1.8, §3.3.6: a cell whose `analog` behavior reads its gate, given `gates[1]` of an array of carriers, elaborates: the
    // gate is a read of the cell's pin, which the placement binds to element 1 of the one carrier `gates` (no diagnostic, one
    // carrier of four elements, the cell's analog function reading it)
    let src = "module Cell(in gate: Bit, p: Electrical) { }\nanalog for Cell { i(p, gnd) +> v(p) / (if gate { 1k } else { 2k }) - 1m; }\nmodule Top() { signal gates: [Bit; 4] = [false, true, false, true]; signal a: Electrical; let c = Cell(gates[1], a); }";
    let ir = SourceElaboration::ir(src, "Top");
    let read = Read { ir: &ir };
    let carriers = read.carriers("Top");
    assert_eq!(carriers.iter().filter(|(name, count, _, _)| name == "gates" && *count == Some(4)).count(), 1, "one carrier of four elements: {carriers:?}");
}
