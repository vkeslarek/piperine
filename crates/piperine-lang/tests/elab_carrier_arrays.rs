//! Arrays of carriers and of nodes elaborated (composite-values CV-23 to CV-26; docs/spec part II §5.1.8): the
//! declaration of an array of discrete carriers as one sparse carrier with a count and an initial value of either form,
//! an array of nodes as consecutive nodes under one name, element reads, history and events, node arrays in `analog`,
//! element and field commits and drives, and connections by window. Each test states the rule it proves.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{Constant, Ir, ValueType};
use support::Elab;

/// What a test reads of an elaborated body.
struct Read<'a> {
    ir: &'a Ir,
}

impl Read<'_> {
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
               signal flags: [Bit; 4];\n\
               signal grid: [[Bit; 2]; 3] = [[true; 2]; 3];\n\
               signal bl: [Electrical; 4];\n}";
    let ir = Elab::ir(src, "Top");
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

/// The structural text of body `Top` of `src`, which elaborates and checks.
fn structure_of(src: &str) -> String {
    Elab::ir(src, "Top").print_structural()
}

#[test]
fn element_reads_and_events_lower_per_element() {
    // CV-24: `x[i]` is one element read (constant or run-time index), the whole `x` an array of the values,
    // `x[k].prev()` the element's previous value, `x[i].changed()` the element's change as a `bool`, and
    // `on x[k].rise()` an event on that element only, guarded by its edge
    let src = "module Top(in clk: Bit, in at: Bit<u2>, out q: Bit<u8> = 0) {\n\
               signal mem: [Bit<u8>; 4] = [0; 4];\n\
               signal lines: [Bit; 4];\n\
               signal grid: [[Bit<u8>; 2]; 3] = [[0; 2]; 3];\n}\n\
               digital for Top {\n\
               on clk.rise() { let a = mem[1]; let b = mem[at]; let p = mem[2].prev(); let c = lines[at].changed(); let g = grid[2][1]; q <- a + b + p; }\n\
               on lines[2].rise() { }\n\
               on mem[3].changed() { }\n}";
    let text = structure_of(src);
    for needle in ["mem[", ".prev", "lines["] {
        assert!(text.contains(needle), "the structure names `{needle}`:\n{text}");
    }
    let ir = Elab::ir(src, "Top");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    // the events are the elements' own: `lines[2]` (a rise) and `mem[3]`, never the whole carriers
    let events: Vec<_> = body.events().iter().map(|e| e.source.clone()).collect();
    let lines = body.carriers().iter().position(|c| c.name == "lines").expect("lines") as u32;
    let mem = body.carriers().iter().position(|c| c.name == "mem").expect("mem") as u32;
    assert!(events.contains(&piperine_ir::EventSource::ElementChanged { carrier: lines, element: 2 }), "{events:?}");
    assert!(events.contains(&piperine_ir::EventSource::ElementChanged { carrier: mem, element: 3 }), "{events:?}");
    let rise = body.events().iter().find(|e| e.source == piperine_ir::EventSource::ElementChanged { carrier: lines, element: 2 }).expect("the rise event");
    assert!(rise.guard.is_some(), "a rise carries its edge guard (!prev && now)");
    // §1 rule 11: a constant element past the end is E0104 naming it; an event head that does not fold is the E0180 mismatch naming it
    let past = Elab::run("module Top() { signal m: [Bit<u8>; 4] = [0; 4]; }\ndigital for Top { on m[0].changed() { let a = m[4]; } }", "Top").expect_err("past the end");
    assert!(past.iter().any(|d| d.diagnostic.code == "E0104" && d.diagnostic.message.contains('4')), "{:?}", support::Elab::codes(&past));
    let folding = Elab::run("module Top(in at: Bit<u2>) { signal l: [Bit; 4]; }\ndigital for Top { on l[at].rise() { } }", "Top").expect_err("no fold");
    assert!(folding.iter().any(|d| d.diagnostic.code == "E0180" && d.diagnostic.message.contains("`l[at]`")), "{:?}", support::Elab::codes(&folding));
}

#[test]
fn an_element_binds_a_handle_parameter() {
    // GN-11, QC3: `f(lines[k])` binds the handle to that one carrier, so `.prev()` and a read inside `f` are the
    // element's
    let src = "fn was(a: Bit) -> bool { a.prev() }\n\
               module Top(in clk: Bit, out q: Bit = false) { signal lines: [Bit; 4]; }\n\
               digital for Top { on clk.rise() { q <- was(lines[2]); } }";
    let text = structure_of(src);
    assert!(text.contains("lines") && text.contains(".prev"), "the handle reads the element's previous value:\n{text}");
}

#[test]
fn node_array_contributions_unroll_to_their_nodes() {
    // CV-26 AC 1: a loop of constant trip count that contributes to `bl[k]` and `bl[k+1]` is unrolled with `k` folded:
    // three branches between consecutive nodes of the array, `bl[0]` to `bl[3]`, a probe of one element reads that node
    let src = "module Top() {\n\
               signal bl: [Electrical; 4];\n}\n\
               analog for Top { for k in 0..3 { i(bl[k], bl[k + 1]) +> v(bl[k], bl[k + 1]) / 1000.0; } let tap = v(bl[2]); }";
    let ir = Elab::ir(src, "Top");
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
    let found = Elab::run("module Top() { signal bl: [Electrical; 4]; }\nanalog for Top { let k = 2; let x = v(bl[k]); }", "Top").expect_err("no fold");
    assert!(found.iter().any(|d| d.diagnostic.code == "E0180" && d.diagnostic.message.contains("`k`")), "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    let past = Elab::run("module Top() { signal bl: [Electrical; 4]; }\nanalog for Top { let x = v(bl[4]); }", "Top").expect_err("past the end");
    assert!(past.iter().any(|d| d.diagnostic.code == "E0104" && d.diagnostic.message.contains('4')), "{:?}", Elab::codes(&past));
}
