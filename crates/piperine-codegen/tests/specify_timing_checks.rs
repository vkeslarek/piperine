//! LRM-COVERS: A.7.5.1
//! System timing-check limits reach the solver's `TimingCheckTable` at
//! circuit build (T20, LRMW3-07) — every parsed check kind maps to its
//! `TimingCheckKind`, exhaustively, no catch-all.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::{DigitalNet, TimingCheckKind};
use piperine_solver::prelude::CircuitInstance;

fn build(src: &str) -> (CircuitInstance, HashMap<String, usize>) {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies: HashMap<String, LoweredBody> = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit compiles");
    (circuit, info.digital_nets)
}

fn leaf_with_check(check_stmt: &str) -> String {
    format!(
        r#"
discipline Bit;
    domain discrete;
enddiscipline
module Leaf (input Bit clk, input Bit d, output Bit q);
    specify
        {check_stmt}
    endspecify
    always @(posedge clk)
        q = d;
endmodule
module Top (input Bit clk, input Bit d, output Bit q);
    Leaf u1 (clk, d, q);
endmodule
"#
    )
}

#[test]
fn setup_check_lands_on_the_data_net() {
    let (circuit, nets) = build(&leaf_with_check("$setup(d, posedge clk, 1.5e-9);"));
    let d = DigitalNet(nets["d"]);
    let limits = circuit.digital_state.timing_checks.get(d);
    assert_eq!(limits.len(), 1);
    assert_eq!(limits[0].kind, TimingCheckKind::Setup);
    assert_eq!(limits[0].limit, 1.5e-9);
}

#[test]
fn hold_check_lands_on_the_data_net() {
    let (circuit, nets) = build(&leaf_with_check("$hold(posedge clk, d, 0.5e-9);"));
    let d = DigitalNet(nets["d"]);
    let limits = circuit.digital_state.timing_checks.get(d);
    assert_eq!(limits.len(), 1);
    assert_eq!(limits[0].kind, TimingCheckKind::Hold);
    assert_eq!(limits[0].limit, 0.5e-9);
}

#[test]
fn setuphold_check_decomposes_into_setup_and_hold_on_the_data_net() {
    let (circuit, nets) = build(&leaf_with_check("$setuphold(posedge clk, d, 1.5e-9, 0.5e-9);"));
    let d = DigitalNet(nets["d"]);
    let limits = circuit.digital_state.timing_checks.get(d);
    assert_eq!(limits.len(), 2);
    assert_eq!(limits[0].kind, TimingCheckKind::Setup);
    assert_eq!(limits[0].limit, 1.5e-9);
    assert_eq!(limits[1].kind, TimingCheckKind::Hold);
    assert_eq!(limits[1].limit, 0.5e-9);
}

#[test]
fn width_and_period_checks_land_on_the_sole_reference_net() {
    let (circuit, nets) = build(&leaf_with_check("$width(posedge clk, 2e-9);\n$period(posedge clk, 4e-9);"));
    let clk = DigitalNet(nets["clk"]);
    let limits = circuit.digital_state.timing_checks.get(clk);
    assert_eq!(limits.len(), 2);
    assert_eq!(limits[0].kind, TimingCheckKind::Width);
    assert_eq!(limits[0].limit, 2e-9);
    assert_eq!(limits[1].kind, TimingCheckKind::Period);
    assert_eq!(limits[1].limit, 4e-9);
}

#[test]
fn recovery_removal_and_skew_checks_map_to_their_own_kinds() {
    let (circuit, nets) = build(&leaf_with_check(
        "$recovery(posedge clk, d, 1e-9);\n$removal(posedge clk, d, 1e-9);\n$skew(posedge clk, d, 1e-9);",
    ));
    let d = DigitalNet(nets["d"]);
    let limits = circuit.digital_state.timing_checks.get(d);
    assert_eq!(limits.len(), 3);
    assert_eq!(limits[0].kind, TimingCheckKind::Recovery);
    assert_eq!(limits[1].kind, TimingCheckKind::Removal);
    assert_eq!(limits[2].kind, TimingCheckKind::Skew);
}

#[test]
fn a_module_with_no_timing_checks_has_an_empty_table() {
    let src = r#"
discipline Bit;
    domain discrete;
enddiscipline
module Leaf (input Bit clk, input Bit d, output Bit q);
    always @(posedge clk)
        q = d;
endmodule
module Top (input Bit clk, input Bit d, output Bit q);
    Leaf u1 (clk, d, q);
endmodule
"#;
    let (circuit, _nets) = build(src);
    assert!(circuit.digital_state.timing_checks.is_empty());
}
