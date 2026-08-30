//! LRM-COVERS: 6.9.3, 7.8.5.1, 7.8.6
//!
//! lrm-conformance-wave3 (Phase 6, T40): §6.9.3 "Elaboration and
//! connectmodules" — "Automatic insertion of connect modules is a
//! post-elaboration operation, as first the disciplines of the various
//! nets need to be resolved... after elaboration of the generate
//! constructs... and after the paramset selection." Cross-declared: this
//! is exactly the order `tests/vams_connect_modules_e2e.rs`'s
//! `auto_inserted_connect_module_bridges_a_genuine_discipline_crossing_
//! end_to_end` already proves (a connect module is only inserted once
//! discipline resolution runs, which happens after the design is fully
//! elaborated).
//!
//! §7.8.5.1 (Port names for Verilog built-in primitives) is a **declared
//! limitation**, not implemented (T40's own "implemented or declared a
//! limitation" choice, recorded in this task's commit body): Piperine's
//! built-in gate primitives (`and`/`nand`/`or`/.../`nmos`/`pmos`/...) are
//! lowered into `pom::GateInstance` (a POM concept entirely separate
//! from an ordinary module `Instance`), and the discipline-resolution/
//! connect-module-auto-insertion machinery
//! (`elab::lower::disciplines.rs`) never looks at `Module::gates` at
//! all — a gate's terminals are treated as plain digital ports
//! regardless of what discipline their connected net was declared with,
//! so no connect module is EVER auto-inserted for a built-in primitive,
//! and the LRM's predictable `out`/`in1`/`in2`/`source`/`drain`/`gate`
//! naming convention (which only matters for naming an auto-inserted
//! connect module) has nothing to attach to. `gate_terminal_discipline_
//! is_ignored_declared_limitation` pins this: a gate instantiated with
//! `electrical`-declared terminals still elaborates and compiles as an
//! ordinary digital gate, with no connect module and no error — the
//! honest current behavior, not a claim that §7.8.5.1's naming
//! convention is implemented.
//!
//! §7.8.6 "Supply sensitive connect module examples" — the LRM's own
//! `dig_inv`/`analog_inv` pair (a connect module comparing an input
//! against a supply-derived threshold) is reproduced in simplified form
//! (a plain `V(vdd)` port read rather than the LRM's own `$simparam`-
//! style hierarchical-string supply reference, which routes through
//! §9.20's `$analog_node_alias`/`$analog_port_alias` — already a
//! disclosed MD-18 rejection elsewhere in this codebase, not this
//! task's scope to revisit): `a_supply_sensitive_connectmodule_
//! elaborates_and_simulates` proves the module elaborates AND produces
//! the threshold-comparison's correct digital value.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

#[test]
fn gate_terminal_discipline_is_ignored_declared_limitation() {
    let src = "module top ();\nelectrical a, b, c;\nnand n1 (a, b, c);\nendmodule\n";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = design.module("top").expect("top elaborated");
    assert_eq!(m.gates.len(), 1, "the nand instance is a GateInstance, not an ordinary Instance");
    assert_eq!(m.instances().len(), 0, "gates never appear in the ordinary Instance list");

    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = piperine_codegen::CircuitCompiler::new(&design, &bodies);
    // No error and no auto-inserted connect module — the current,
    // disclosed behavior (§7.8.5.1's naming convention is unimplemented,
    // not incorrectly implemented).
    compiler.build_circuit_mapped("top").expect("compiles despite the electrical-declared terminals");
}

const SUPPLY_SENSITIVE_SRC: &str = "
    discipline logic;
        domain discrete;
    enddiscipline

    module Vsrc (inout electrical p, inout electrical n);
        parameter real dc = 5.0;
        analog V(p, n) <+ dc;
    endmodule

    connectmodule sensitive_bridge (inout electrical in, inout electrical vdd, inout logic out);
        assign out = (V(in) > V(vdd)/2) ? 1'b0 : 1'b1;
    endmodule

    module Top ();
        electrical gnd, vin, vsupply;
        logic outnet;
        Vsrc vs1 (.p(vin), .n(gnd));
        Vsrc vs2 (.p(vsupply), .n(gnd));
        sensitive_bridge sb (.in(vin), .vdd(vsupply), .out(outnet));
    endmodule
";

#[test]
fn a_supply_sensitive_connectmodule_elaborates_and_simulates() {
    let design = piperine_lang::parse_and_elaborate(SUPPLY_SENSITIVE_SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let out = op.v("outnet").expect("v(outnet)");
    // vin = 5.0 > vsupply/2 = 2.5, so `out` takes the "high" branch (0).
    assert!((out - 0.0).abs() < 1e-9, "V(in) > V(vdd)/2, so out must be logic-low (0), got {out}");
}
