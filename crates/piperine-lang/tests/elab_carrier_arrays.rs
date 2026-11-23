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
