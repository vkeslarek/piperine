//! LRM-COVERS: 6.5.7.1, 7.2.3
//!
//! T13 (`lrm-conformance-wave3`, LRMW3-05) — spec.md's P1 "Vectors are a
//! real data type" user story, AC7: "WHEN a port is declared with a range
//! (`input [0:width-1] in;`) THEN instantiating it with a vector net
//! SHALL connect bit-for-bit, and a width mismatch SHALL fail loud citing
//! §6.5.7.1."
//!
//! **Scope note (why this lives here, not in `piperine-codegen/tests/` or
//! `piperine-lang/tests/`, MD-28):** the AC's own "bit-for-bit" half needs
//! a real DC solve reading a leaf-side value through the hierarchy — a
//! cross-crate proof (`piperine_lang` elaborates → `piperine_codegen`
//! resolves/compiles → `piperine_solver` solves), the same placement
//! `hierarchical_ref_e2e.rs` uses for the same reason.
//!
//! **What was broken before this task**: `resolve/pom/structure.rs`'s
//! port-registration loop (`build_symbols_and_ports`, "1. Ports")
//! registered exactly one `NodeId` per declared port regardless of
//! `NetType::Array` width — unlike the wire-registration loop a few lines
//! below ("3. Wires"), which already recursed per-element via
//! `register_element_nodes`. `resolve::Port` itself only had room for one
//! `NodeId` (`pub node: NodeId`), and `device::builder::InstanceBuilder`
//! resolved exactly one parent node per declared port position
//! (`resolve_connections`), with no notion of a port position expanding
//! into several bits. A vector port therefore silently collapsed onto a
//! single scalar node — invisible at elaboration time (§6.5.7.1's width
//! check runs on POM `NetRef`/`NetType` data, never on codegen's own
//! `resolve::Port`), only observable by actually solving and reading a
//! leaf-side bit, which is what this file's first test does.
//!
//! **The fix**: `Port` now carries `nodes: Vec<NodeId>` (length 1 for a
//! scalar port — never a special-cased branch), `structure.rs`'s port
//! loop calls the same `register_element_nodes` the wire loop already
//! used, and `device::builder::InstanceBuilder::resolve_connections_by_bit`
//! (used only by the ordinary module-instantiation path in `add_instance`)
//! expands a vector port's single `NetRef` into one per-bit parent-node
//! lookup, using the exact same `"{base}[{i}]"` naming convention
//! (`element_node_name`) the wire path already relies on. Every consumer
//! of a port's node(s) (`emit/analog_expr.rs`'s `$port_connected`,
//! `kernel/analog/compile.rs`'s terminal ordering, the digital net-mapping
//! and A2D-bridge loops in `device/builder.rs`) was widened to iterate
//! `port.nodes` instead of reading a single `port.node` field.
//!
//! §6.5.7.1's width-mismatch fail-loud path was already implemented before
//! this task (`CheckPortSizes`, `crates/piperine-lang/src/elab/lower/
//! passes.rs`) and already has message-content-asserted coverage in
//! `crates/piperine-lang/tests/port_size_matching.rs`; the second test
//! below is a regression proof of that same behavior at an 8-bit/4-bit
//! width, kept green by this task rather than re-derived.
//!
//! **Regression checked, not touched**: the array-of-instances broadcast
//! rule (`name_of_instance [msb:lsb] (...)`, §6.2.2,
//! `elab/lower/flatten.rs::inline_sub_instances`) is an entirely different
//! mechanism (an *instance* range, not a *port* range) and is untouched by
//! this task — its own regression suite,
//! `crates/piperine-lang/tests/vams_array_of_instances.rs`, stays green
//! unmodified (4/4 passing after this task's changes).

use piperine_codegen::{CircuitBuildInfo, CircuitCompiler};
use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::ElabError;
use piperine_solver::prelude::{Context, DcAnalysisResult};

const WIDTH: usize = 8;

/// `leaf`'s vector input port `bin[0:7]` is wired straight through
/// (`V(bout[i], gnd) <+ V(bin[i], gnd);`) to its own vector output port
/// `bout[0:7]`; `top` drives each `bin` bit to a distinct voltage
/// (`0.1 * (i+1)`, i.e. never the same value twice, and never zero) and
/// instantiates `leaf` connecting its own top-level `bin`/`bout` vector
/// nets whole (`leaf l1 (gnd, bin, bout);` — no per-bit connection
/// syntax). If the fix collapsed every bit onto one node, permuted bit
/// order, or left a bit unconnected/floating, at least one `bout[i]`
/// would fail to read back its own `bin[i]`'s distinct value.
fn build_and_solve() -> (CircuitBuildInfo, DcAnalysisResult) {
    let assigns: String = (0..WIDTH)
        .map(|i| format!("        V(bin[{i}], gnd) <+ {:.2};\n", 0.1 * (i as f64 + 1.0)))
        .collect();
    let forwards: String = (0..WIDTH)
        .map(|i| format!("        V(bout[{i}], gnd) <+ V(bin[{i}], gnd);\n"))
        .collect();
    let src = format!(
        r#"
        module leaf (gnd, bin, bout);
            inout electrical gnd;
            input electrical [0:{msb}] bin;
            output electrical [0:{msb}] bout;
            analog begin
{forwards}
            end
        endmodule

        module top ();
            electrical gnd;
            electrical [0:{msb}] bin;
            electrical [0:{msb}] bout;
            analog begin
{assigns}
            end
            leaf l1 (gnd, bin, bout);
        endmodule
        "#,
        msb = WIDTH - 1,
    );
    let mut design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("8-bit vector port fixture must elaborate: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    (info, dc)
}

/// AC7's "connect bit for bit" half: every one of the 8 bits reads back
/// through the hierarchy (`top.bin[i]` -> `leaf`'s `bin[i]` port bit ->
/// `leaf`'s `bout[i]` port bit -> `top.bout[i]`) at exactly the distinct
/// voltage `top` drove that bit to — asserted by a real DC solve and a
/// leaf-side (forwarded) readback, not by counting nodes at elaboration
/// time.
#[test]
fn eight_bit_vector_port_connects_bit_for_bit_through_the_hierarchy() {
    let (info, dc) = build_and_solve();
    for i in 0..WIDTH {
        let expected = 0.1 * (i as f64 + 1.0);
        let in_net = info.nets.get(&format!("bin[{i}]")).unwrap_or_else(|| panic!("net `bin[{i}]` not found"));
        let out_net = info.nets.get(&format!("bout[{i}]")).unwrap_or_else(|| panic!("net `bout[{i}]` not found"));
        let in_v = dc.get_node(in_net).unwrap_or_else(|| panic!("V(bin[{i}]) at the operating point"));
        let out_v = dc.get_node(out_net).unwrap_or_else(|| panic!("V(bout[{i}]) at the operating point"));
        assert!((in_v - expected).abs() < 1e-9, "bin[{i}] should be driven to {expected}V, got {in_v}");
        assert!(
            (out_v - expected).abs() < 1e-9,
            "bout[{i}] (forwarded through leaf's own bin[{i}]->bout[{i}]) should read back {expected}V \
             (bin[{i}]'s own distinct value), got {out_v} — a collapsed/misordered/floating bit-for-bit \
             wiring would show up here"
        );
    }
}

/// §6.5.7.1 regression, at the width this task's fix operates on: a 4-bit
/// net connected to an 8-bit port still fails loud citing the clause —
/// `CheckPortSizes` (`crates/piperine-lang/src/elab/lower/passes.rs`) is
/// untouched by this task, and this proves it still runs and still names
/// both widths and the clause number in its diagnostic.
#[test]
fn four_bit_net_on_an_eight_bit_port_fails_loud_citing_6_5_7_1() {
    let src = format!(
        "module leaf (gnd, p);
            inout electrical gnd;
            input electrical [0:{msb}] p;
        endmodule
        module top ();
            electrical gnd;
            electrical [0:3] bus4;
            leaf l1 (gnd, bus4);
        endmodule",
        msb = WIDTH - 1,
    );
    let report = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .expect_err("a 4-bit net bound to an 8-bit port must fail elaboration (LRM §6.5.7.1)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    let msg = format!("{err}");
    assert!(msg.contains("6.5.7.1"), "expected the error to cite LRM §6.5.7.1, got: {msg}");
    assert!(msg.contains("width 4") && msg.contains("width 8"), "expected both widths named, got: {msg}");
}
