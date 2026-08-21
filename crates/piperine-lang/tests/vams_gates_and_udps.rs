//! LRM-COVERS: 6.4, 8.5.3.5
//! LRMC-C7-13 (T39, lowering half): gate primitives (Annex A.4.2) and UDP
//! declarations/instances (Annex A.5) reach the POM — `Module::gates`
//! (a gate is not a module instance, no port declarations to resolve
//! against) and `Design::udp`/`Module::instances` (a UDP instantiation is
//! syntactically identical to a module instantiation, resolved against
//! the UDP's own port order — LRMC-C5-07's paramset-resolution precedent,
//! applied to a different indirection). Codegen dispatch (gate/UDP truth
//! tables actually driving digital simulation) is T40's job.

use piperine_lang::{parse_and_elaborate, SourceMap};

#[test]
fn digital_gate_primitives_reach_the_pom_with_terminals_and_delay_intact() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        module dut (input logic a, input logic b, output logic y1, output logic y2, output logic y3);
            and #2 g1 (y1, a, b);
            nand g2 (y2, a, b);
            not g3 (y3, a);
        endmodule
    ";
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("gate primitives elaborate");
    let dut = design.module("dut").expect("dut elaborated");
    assert_eq!(dut.gates.len(), 3);

    let g1 = &dut.gates[0];
    assert_eq!(g1.kind, piperine_lang::ast::GateKind::And);
    assert_eq!(g1.name.as_deref(), Some("g1"));
    assert_eq!(g1.terminals.len(), 3, "y1, a, b");
    assert_eq!(g1.delay, Some(2.0));

    let g2 = &dut.gates[1];
    assert_eq!(g2.kind, piperine_lang::ast::GateKind::Nand);
    assert_eq!(g2.delay, None);

    let g3 = &dut.gates[2];
    assert_eq!(g3.kind, piperine_lang::ast::GateKind::Not);
    assert_eq!(g3.terminals.len(), 2, "y3, a");
}

#[test]
fn a_gate_statement_with_multiple_instances_shares_kind_and_delay() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        module dut (input logic a, input logic b, output logic y1, output logic y2);
            buf #3 g1 (y1, a), g2 (y2, b);
        endmodule
    ";
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let dut = design.module("dut").expect("dut elaborated");
    assert_eq!(dut.gates.len(), 2);
    assert!(dut.gates.iter().all(|g| g.kind == piperine_lang::ast::GateKind::Buf && g.delay == Some(3.0)));
    assert_eq!(dut.gates[0].name.as_deref(), Some("g1"));
    assert_eq!(dut.gates[1].name.as_deref(), Some("g2"));
}

#[test]
fn switch_level_gate_primitives_fail_loud_naming_the_gap() {
    for kw in ["nmos", "pmos", "cmos", "tran", "tranif1", "pullup"] {
        let src = format!(
            "discipline logic;
                domain discrete;
            enddiscipline

            module dut (input logic a, input logic b, output logic y);
                {kw} g1 (y, a, b);
            endmodule"
        );
        let err = parse_and_elaborate(&src, &SourceMap::dummy())
            .expect_err(&format!("{kw} must fail loud, not silently drop"));
        let msg = format!("{err}").to_lowercase();
        assert!(
            msg.contains("switch-level") || msg.contains("not supported"),
            "{kw}: error should name the gap, got: {msg}"
        );
    }
}

// ── UDP (primitive), Annex A.5 ───────────────────────────────────────────

const LOGIC_DISC: &str = "
    discipline logic;
        domain discrete;
    enddiscipline
";

const AND_UDP: &str = "
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
";

#[test]
fn udp_declaration_reaches_the_design_with_its_table_intact() {
    let src = format!(
        "{LOGIC_DISC}{AND_UDP}
        module dut (input logic a, input logic b, output logic y);
            my_and u1 (y, a, b);
        endmodule"
    );
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("UDP instantiation elaborates");
    let udp = design.udp("my_and").expect("my_and registered in Design::udps");
    assert!(!udp.is_sequential);
    assert_eq!(udp.table.len(), 4);
    assert_eq!(udp.table[3].next_state, "1");
}

#[test]
fn udp_instance_lowers_as_a_module_instance_bound_to_its_own_port_order() {
    let src = format!(
        "{LOGIC_DISC}{AND_UDP}
        module dut (input logic a, input logic b, output logic y);
            my_and u1 (y, a, b);
        endmodule"
    );
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let dut = design.module("dut").expect("dut elaborated");
    assert_eq!(dut.instances.len(), 1, "the UDP instance lands in Module::instances, not a separate list");
    let inst = &dut.instances[0];
    assert_eq!(inst.module, "my_and");
    assert_eq!(inst.ports.len(), 3, "q, a, b — the UDP's own canonical port order");
    assert!(design.udp(&inst.module).is_some(), "downstream dispatch distinguishes a UDP instance this way");
}

#[test]
fn udp_instance_rejects_parameter_overrides() {
    let src = format!(
        "{LOGIC_DISC}{AND_UDP}
        module dut (input logic a, input logic b, output logic y);
            my_and #(.foo(1)) u1 (y, a, b);
        endmodule"
    );
    let err = parse_and_elaborate(&src, &SourceMap::dummy()).expect_err("a UDP takes no #(...) overrides");
    assert!(format!("{err}").contains("does not accept"), "{err}");
}

// ── `timescale scales gate `#delay` (1364-2005, functional) ─────────────

#[test]
fn a_gate_delay_scales_by_the_modules_timescale_unit() {
    let src = "
        `timescale 1ns/1ps
        discipline logic;
            domain discrete;
        enddiscipline

        module dut (input logic a, input logic b, output logic y);
            and #2 g1 (y, a, b);
        endmodule
    ";
    let d = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = d.module("dut").expect("dut");
    assert_eq!(m.timescale.expect("timescale recorded").unit_exponent, -9);
    let gate = &m.gates[0];
    let delay = gate.delay.expect("delay");
    assert!((delay - 2e-9).abs() < 1e-15, "`#2` under `timescale 1ns` is 2 ns, got {delay}");
}

#[test]
fn a_gate_delay_rounds_to_the_precision_grid() {
    // #2.5 ns under 1ns/1ns rounds to the 1ns grid: 3 ns (1364-2005
    // rounds delay values to the precision).
    let src = "
        `timescale 1ns/1ns
        discipline logic;
            domain discrete;
        enddiscipline

        module dut (input logic a, input logic b, output logic y);
            and #2.5 g1 (y, a, b);
        endmodule
    ";
    let d = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = d.module("dut").expect("dut");
    let delay = m.gates[0].delay.expect("delay");
    assert!((delay - 3e-9).abs() < 1e-15, "2.5 ns on a 1 ns grid rounds up, got {delay}");
}

#[test]
fn each_module_takes_the_last_timescale_before_it() {
    let src = "
        `timescale 1ns/1ps
        discipline logic;
            domain discrete;
        enddiscipline

        module fast (input logic a, input logic b, output logic y);
            and #2 g1 (y, a, b);
        endmodule

        `timescale 1us/1ns
        module slow (input logic a, input logic b, output logic y);
            and #2 g1 (y, a, b);
        endmodule
    ";
    let d = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let fast = d.module("fast").expect("fast").gates[0].delay.expect("delay");
    assert!((fast - 2e-9).abs() < 1e-15, "fast: 1ns unit, got {fast}");
    let slow = d.module("slow").expect("slow").gates[0].delay.expect("delay");
    assert!((slow - 2e-6).abs() < 1e-18, "slow: the 1us directive governs it, got {slow}");
}

#[test]
fn without_a_timescale_a_gate_delay_stays_in_raw_seconds() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        module dut (input logic a, input logic b, output logic y);
            and #2 g1 (y, a, b);
        endmodule
    ";
    let d = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = d.module("dut").expect("dut");
    assert_eq!(m.timescale, None, "no directive precedes the module");
    assert_eq!(m.gates[0].delay, Some(2.0), "raw simulation-time units — the pre-existing behavior");
}
