//! LRM-COVERS: A.5, A.5.3
//! T49 (VAMS-19) — user-defined primitive (UDP) state-table evaluation,
//! exercised through `piperine_codegen`'s public surface
//! (`piperine_codegen::kernel::digital::UdpTable`). Mirrors T49's "Done
//! when": combinational and sequential UDPs evaluate against their state
//! tables per LRM/1364.

use piperine_codegen::kernel::digital::UdpTable;
use piperine_solver::abi::LogicValue::{One, X, Zero};
use piperine_lang::ast::{PrimitiveDecl, PreprocessedSpan, UdpTableRow};

fn span() -> PreprocessedSpan {
    PreprocessedSpan::new(0, 0)
}

fn row(inputs: &[&str], current_state: Option<&str>, next_state: &str) -> UdpTableRow {
    UdpTableRow {
        span: span(),
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
        current_state: current_state.map(str::to_string),
        next_state: next_state.to_string(),
    }
}

fn primitive(name: &str, is_sequential: bool, table: Vec<UdpTableRow>) -> PrimitiveDecl {
    PrimitiveDecl { span: span(), attrs: vec![], name: name.to_string(), ports: vec![], table, is_sequential }
}

/// A 2-input `or`-equivalent combinational UDP (LRM Annex A.5.3 shape),
/// mirroring the LRM's own `MUX` / basic-gate style examples.
fn or_udp() -> PrimitiveDecl {
    primitive(
        "udp_or",
        false,
        vec![row(&["1", "?"], None, "1"), row(&["?", "1"], None, "1"), row(&["0", "0"], None, "0")],
    )
}

#[test]
fn combinational_udp_evaluates_against_its_state_table() {
    let decl = or_udp();
    let table = UdpTable::new(&decl);
    assert_eq!(table.eval_combinational(&[Zero, Zero]).unwrap(), Zero);
    assert_eq!(table.eval_combinational(&[One, Zero]).unwrap(), One);
    assert_eq!(table.eval_combinational(&[Zero, One]).unwrap(), One);
    assert_eq!(table.eval_combinational(&[One, One]).unwrap(), One);
}

#[test]
fn combinational_udp_unlisted_combination_yields_x() {
    let decl = or_udp();
    let table = UdpTable::new(&decl);
    assert_eq!(table.eval_combinational(&[X, X]).unwrap(), X);
}

/// A positive-edge D-latch-style sequential UDP: `clk` (edge), `d`
/// (level); holds state otherwise.
fn dff_udp() -> PrimitiveDecl {
    primitive(
        "udp_dff",
        true,
        vec![row(&["r", "0"], Some("?"), "0"), row(&["r", "1"], Some("?"), "1"), row(&["?", "*"], Some("?"), "-")],
    )
}

#[test]
fn sequential_udp_evaluates_against_its_state_table() {
    let decl = dff_udp();
    let table = UdpTable::new(&decl);
    // Rising clk edge with d=1 clocks the new value in.
    assert_eq!(table.eval_sequential(&[Zero, Zero], &[One, One], Zero).unwrap(), One);
    // No clock edge: state holds regardless of d changing.
    assert_eq!(table.eval_sequential(&[Zero, Zero], &[Zero, One], One).unwrap(), One);
}
