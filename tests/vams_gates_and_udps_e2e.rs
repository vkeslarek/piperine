//! LRMC-C7-13 (T40, dispatch half) — a gate-level testbench with UDPs
//! simulates and produces the LRM-specified values: `and`/`nand`/`buf`
//! gate primitives (Annex A.4.2 Table 7-4/7-9) and a combinational UDP
//! (Annex A.5.3), driven end-to-end through the real digital scheduler
//! (`GateElement`/`UdpElement`, `piperine-codegen::device::prim`).
//!
//! Placement note (MD-28): cross-crate host-surface proof
//! (`piperine_lang` → `piperine_codegen` → `piperine_solver`), root
//! `tests/`, mirroring `vams_connect_modules_e2e.rs`'s own placement note.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const GATE_FIXTURE: &str = "
    discipline logic;
        domain discrete;
    enddiscipline

    module dut (input logic a, input logic b, output logic y_and, output logic y_nand, output logic y_buf);
        and g1 (y_and, a, b);
        nand g2 (y_nand, a, b);
        buf g3 (y_buf, a);
    endmodule
";

#[test]
fn gate_primitives_simulate_end_to_end_and_produce_lrm_truth_table_values() {
    let design = parse_and_elaborate(GATE_FIXTURE, &piperine_lang::SourceMap::dummy()).expect("gates elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("dut").expect("gate instances compile into the circuit");

    let a = DigitalNet(*info.digital_nets.get("a").expect("net `a`"));
    let b = DigitalNet(*info.digital_nets.get("b").expect("net `b`"));

    // Force a=1, b=1 at t=0 (both events, region Active — the everyday
    // "ordinary write" region every pre-T38 source stamps).
    circuit.digital_state.schedule(DigitalEvent {
        time: 0.0,
        net: a,
        value: LogicValue::One,
        source: usize::MAX,
        seq: 0,
        region: Region::Active,
    });
    circuit.digital_state.schedule(DigitalEvent {
        time: 0.0,
        net: b,
        value: LogicValue::One,
        source: usize::MAX,
        seq: 1,
        region: Region::Active,
    });

    let opts = TransientAnalysisOptions::new(1.0, 0.1);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    tran.solve().expect("transient run completes");

    let y_and = DigitalNet(*info.digital_nets.get("y_and").expect("net `y_and`"));
    let y_nand = DigitalNet(*info.digital_nets.get("y_nand").expect("net `y_nand`"));
    let y_buf = DigitalNet(*info.digital_nets.get("y_buf").expect("net `y_buf`"));

    // Table 7-4: and(1,1) = 1, nand(1,1) = 0. Table 7-9: buf(1) = 1.
    assert_eq!(circuit.digital_state.nets[y_and.0], LogicValue::One, "and(1,1) = 1");
    assert_eq!(circuit.digital_state.nets[y_nand.0], LogicValue::Zero, "nand(1,1) = 0");
    assert_eq!(circuit.digital_state.nets[y_buf.0], LogicValue::One, "buf(1) = 1");
}

const UDP_FIXTURE: &str = "
    primitive my_and (q, a, b);
        output q;
        input a, b;
        table
            0 0 : 0;
            0 1 : 0;
            1 0 : 0;
            1 1 : 1;
        endtable
    endprimitive

    discipline logic;
        domain discrete;
    enddiscipline

    module dut (input logic a, input logic b, output logic y);
        my_and u1 (y, a, b);
    endmodule
";

#[test]
fn udp_instance_simulates_end_to_end_and_produces_its_own_table_value() {
    let design = parse_and_elaborate(UDP_FIXTURE, &piperine_lang::SourceMap::dummy()).expect("UDP elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("dut").expect("UDP instance compiles into the circuit");

    let a = DigitalNet(*info.digital_nets.get("a").expect("net `a`"));
    let b = DigitalNet(*info.digital_nets.get("b").expect("net `b`"));
    circuit.digital_state.schedule(DigitalEvent {
        time: 0.0,
        net: a,
        value: LogicValue::One,
        source: usize::MAX,
        seq: 0,
        region: Region::Active,
    });
    circuit.digital_state.schedule(DigitalEvent {
        time: 0.0,
        net: b,
        value: LogicValue::Zero,
        source: usize::MAX,
        seq: 1,
        region: Region::Active,
    });

    let opts = TransientAnalysisOptions::new(1.0, 0.1);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    tran.solve().expect("transient run completes");

    let y = DigitalNet(*info.digital_nets.get("y").expect("net `y`"));
    // The UDP's own row "1 0 : 0" — same truth as `and`, but driven
    // through UdpTable::eval_combinational, not GatePrimitive.
    assert_eq!(circuit.digital_state.nets[y.0], LogicValue::Zero, "my_and(1,0) = 0 (UDP's own table row)");
}
