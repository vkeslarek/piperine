//! LRM-COVERS: 8.5.3.5
//! T67/LRMW2-64: the bidirectional pass-switch family (`tran`/`rtran`/
//! `tranif0`/`tranif1`/`rtranif0`/`rtranif1`, §8.5.3.5) — previously
//! rejected loud at elaboration (`elab::lower::vams::is_switch_level_gate`)
//! for every switch-level gate keyword. Modeled unidirectionally
//! (`source → drain`, the LRM's own §9.4.1 terminal naming), reusing the
//! existing tristate-buffer primitives (`GatePrimitive::Buf`/`Bufif0`/
//! `Bufif1`, Table 7-9/7-10) — `kernel::digital::gates`'s module doc
//! discloses the gap against the LRM's full bidirectional relaxation
//! model, and the strength-reduction half of Table 7-3 for the `r`-forms
//! (no per-event strength vehicle exists in `DigitalEvent` today — a
//! separate, larger ABI-extension task, disclosed rather than faked).
//!
//! Mirrors `tests/vams_gates_and_udps_e2e.rs`'s proven end-to-end harness
//! (real `CircuitCompiler` + digital scheduler, not a synthetic
//! `GatePrimitive` call).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const FIXTURE: &str = "
    discipline logic;
        domain discrete;
    enddiscipline

    module dut (
        input logic src, input logic gate_ctrl, output logic drain,
        input logic src0, input logic gate_ctrl0, output logic drain0,
        input logic rsrc, input logic rgate, output logic rdrain,
        input logic tsrc, output logic tdrain
    );
        tranif1 t1 (src, drain, gate_ctrl);
        tranif0 t0 (src0, drain0, gate_ctrl0);
        rtranif1 rt1 (rsrc, rdrain, rgate);
        tran tr (tsrc, tdrain);
    endmodule
";

struct Circuit {
    instance: piperine_solver::abi::CircuitInstance,
    nets: std::collections::HashMap<String, usize>,
}

fn build() -> Circuit {
    let design = parse_and_elaborate(FIXTURE, &piperine_lang::SourceMap::dummy()).expect("switch primitives elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (circuit, info) = compiler.build_circuit_mapped("dut").expect("switch instances compile into the circuit");
    Circuit { instance: circuit, nets: info.digital_nets }
}

fn net(c: &Circuit, name: &str) -> DigitalNet {
    DigitalNet(*c.nets.get(name).unwrap_or_else(|| panic!("net `{name}`")))
}

fn drive(c: &mut Circuit, assignments: &[(&str, LogicValue)]) {
    for (i, &(name, value)) in assignments.iter().enumerate() {
        c.instance.digital_state.schedule(DigitalEvent {
            time: 0.0, net: net(c, name), value, source: usize::MAX, seq: i as u64, region: Region::Active,
        });
    }
    let opts = TransientAnalysisOptions::new(1.0, 0.1);
    let mut tran = c.instance.transient(opts, Context::default()).unwrap();
    tran.solve().expect("transient run completes");
}

/// Done-when: "A `tranif1` conducts when enabled ... with strength
/// reduction on the r-forms per Table 7-3" (the strength-reduction half is
/// a disclosed, separate gap — see the module doc; conduct/isolate is
/// this test's claim). `gate_ctrl = 1` conducts: `drain` mirrors `src`
/// (Table 7-10's `bufif1` truth table, reused verbatim).
#[test]
fn tranif1_conducts_when_enabled() {
    let mut c = build();
    drive(&mut c, &[("src", LogicValue::One), ("gate_ctrl", LogicValue::One)]);
    assert_eq!(c.instance.digital_state.nets[net(&c, "drain").0], LogicValue::One, "tranif1: gate=1 conducts, drain mirrors src=1");
}

/// Done-when: "...and isolates when not." `gate_ctrl = 0` isolates:
/// `drain` goes high-impedance regardless of `src`'s value.
#[test]
fn tranif1_isolates_when_disabled() {
    let mut c = build();
    drive(&mut c, &[("src", LogicValue::One), ("gate_ctrl", LogicValue::Zero)]);
    assert_eq!(c.instance.digital_state.nets[net(&c, "drain").0], LogicValue::Z, "tranif1: gate=0 isolates, drain floats to Z");
}

/// `tranif0` is `tranif1`'s complement (Table 7-10's `bufif0`): conducts
/// while its gate terminal is 0, isolates while it is 1 — the opposite
/// polarity from `tranif1`, asserted here so the two primitives aren't
/// accidentally wired to the same truth table.
#[test]
fn tranif0_conducts_on_zero_and_isolates_on_one() {
    let mut c = build();
    drive(&mut c, &[("src0", LogicValue::Zero), ("gate_ctrl0", LogicValue::Zero)]);
    assert_eq!(c.instance.digital_state.nets[net(&c, "drain0").0], LogicValue::Zero, "tranif0: gate=0 conducts, drain mirrors src0=0");

    let mut c2 = build();
    drive(&mut c2, &[("src0", LogicValue::Zero), ("gate_ctrl0", LogicValue::One)]);
    assert_eq!(c2.instance.digital_state.nets[net(&c2, "drain0").0], LogicValue::Z, "tranif0: gate=1 isolates, drain floats to Z");
}

/// `rtranif1` (the resistive form) still conducts/isolates correctly —
/// the disclosed gap is strength only (unmodeled, not silently faked),
/// asserted here so the `r`-forms aren't rejected loud or silently
/// dropped the way they were pre-T67.
#[test]
fn rtranif1_conducts_and_isolates_like_its_non_resistive_counterpart() {
    let mut c = build();
    drive(&mut c, &[("rsrc", LogicValue::One), ("rgate", LogicValue::One)]);
    assert_eq!(c.instance.digital_state.nets[net(&c, "rdrain").0], LogicValue::One, "rtranif1: gate=1 conducts, rdrain mirrors rsrc=1");

    let mut c2 = build();
    drive(&mut c2, &[("rsrc", LogicValue::One), ("rgate", LogicValue::Zero)]);
    assert_eq!(c2.instance.digital_state.nets[net(&c2, "rdrain").0], LogicValue::Z, "rtranif1: gate=0 isolates, rdrain floats to Z");
}

/// `tran` (no gate terminal) always conducts — `tdrain` mirrors `tsrc`
/// unconditionally.
#[test]
fn tran_always_conducts_with_no_gate_terminal() {
    let mut c = build();
    drive(&mut c, &[("tsrc", LogicValue::One)]);
    assert_eq!(c.instance.digital_state.nets[net(&c, "tdrain").0], LogicValue::One, "tran: unconditional pass, tdrain mirrors tsrc=1");

    let mut c2 = build();
    drive(&mut c2, &[("tsrc", LogicValue::Zero)]);
    assert_eq!(c2.instance.digital_state.nets[net(&c2, "tdrain").0], LogicValue::Zero, "tran: unconditional pass, tdrain mirrors tsrc=0");
}
