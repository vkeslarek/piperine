//! LRM-COVERS: 1.3.1.1, 3.6.4
//! Wave-2 (T69) re-reading, no new tests: §1.3.1.1 "Reference nodes"
//! states "any net of continuous discipline can be declared to be ground
//! [and] the node associated with the net shall be the global reference
//! node in the circuit" — exactly what
//! `ground_declared_net_becomes_the_solver_reference_node` proves at the
//! solver (the DC solve only succeeds because `ground foo;` actually wired
//! `foo` in as the MNA reference).
//!
//! LRM §3.6.4: `ground foo;` makes `foo` the MNA reference node. Before
//! this fix the declaration was a pure no-op at elaboration (SPEC_DEVIATION.md
//! finding 1.4) — `foo` never became [`piperine_codegen::resolve::NodeId::GROUND`],
//! it stayed an ordinary net with no path to a reference. This is proven at
//! the SOLVER (not just POM inspection): a circuit with a source branch
//! between two nets, neither of which is named `gnd`/`GND`/`vss`/`VSS`/`0`
//! (the crate-global fallback list), only solves if `ground foo;` actually
//! wired `foo` in as the reference — with no reference node anywhere in
//! the netlist, the MNA system is underdetermined (every node voltage is
//! only defined up to an additive constant) and the DC solve fails.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

fn source_map() -> piperine_lang::SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    piperine_lang::SourceMap::new(headers)
}

/// `vdd`/`foo` — neither name is in the crate-global ground fallback list
/// (`gnd`/`GND`/`vss`/`VSS`/`0`). The ONLY thing that can make this DC
/// solve is `ground foo;` actually resolving `foo` to the reference node.
const SRC: &str = "\
`include \"disciplines.vams\"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 0.0;
    analog V(p, n) <+ dc;
endmodule
module Top ();
    electrical vdd, foo;
    ground foo;
    Vsrc #(.dc(5.0)) v1 (.p(vdd), .n(foo));
endmodule
";

#[test]
fn ground_declared_net_becomes_the_solver_reference_node() {
    let design = parse_and_elaborate(SRC, &source_map()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    circuit.init_digital().expect("digital init");

    let vdd = info.nets.get("vdd").expect("net `vdd` mapped");

    let result = circuit.dc(Context::default()).expect("dc solver starts").solve().expect("DC operating point solves");
    let v_vdd = result.get_node(vdd).expect("v(vdd) present in the DC solution");
    assert!(
        (v_vdd - 5.0).abs() < 1e-9,
        "with `foo` as the MNA reference, v(vdd) must read 5.0 V (the ideal \
         source's own value relative to that reference), got {v_vdd}"
    );
}
