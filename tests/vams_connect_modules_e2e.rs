//! LRM-COVERS: 7.4.4.1, 7.4.4.3
//! T52 (VAMS-20) — "mixed analog/digital fixture simulates end-to-end"
//! done-when: a single `.vams` design containing an analog subcircuit
//! (an ideal source, a resistor, and an explicitly-instantiated
//! `connectmodule` bridge — LRM ch. 7's own module kind, elaborated
//! uniformly with `module`/`macromodule` per `crates/piperine-lang/src/
//! elab/lower/connect.rs`'s survey finding) *and* a digital register
//! (the same clocked-counter pattern `tests/vams_vertical_slice.rs`'s
//! T33 already proved), compiled into **one** circuit and simulated
//! together on the unmodified native solver.
//!
//! Placement note (MD-28): this is a cross-crate host-surface proof
//! (`piperine_lang` → `piperine_codegen` → `piperine_solver`), so it lives
//! in root `tests/`, alongside `vams_vertical_slice.rs` — not inside
//! `piperine-lang`'s own `tests/`, which cannot reach codegen/solver.
//! `crates/piperine-lang/tests/vams_connect_modules.rs` covers the
//! elaboration-only claims (T52's stated `Where`); this file is the
//! necessary scope-widening for the "simulates end-to-end" claim, in the
//! same spirit T46's own progress-log entry documented for its CLI fixes.
//!
//! **Scope note on "mixed"**: the first fixture below bridges two
//! same-hierarchy `electrical` nets (an identity/zero-volt bridge)
//! through an *explicitly instantiated* `connectmodule`, proving a
//! connectmodule-kind instance coexists in the same compiled circuit as
//! genuine analog *and* digital content, simulating together without any
//! special-casing anywhere in the pipeline.
//!
//! **T37 (LRMC-C7-01) closes the gap this note used to name**: the
//! second fixture/test below (`auto_inserted_connect_module_bridges_...`)
//! is a genuine `electrical<->logic` crossing with *no* explicit
//! connectmodule instance written anywhere — `InsertConnectModules`
//! (T8/`connect.rs`) auto-splices the bridge into `Design::flat_modules`,
//! and `CircuitCompiler` (which always builds from the flat form, never
//! `Design::modules` — MD-25) compiles and simulates it exactly like any
//! authored instance. The `passes.rs` doc comment that used to claim
//! `InsertConnectModules` "is a no-op over every design in today's
//! corpus" was already stale before this task (T8 fixed the underlying
//! veto) — corrected alongside this test, not left to drift further.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// T37 (LRMC-C7-01): a genuine `electrical<->logic` crossing with *no*
/// explicit connectmodule instance — `d1`'s `logic` port `p` is
/// connected straight to `top`'s `electrical` net `vmid`.
/// `InsertConnectModules` (`connect.rs`, driven by `connectrules basic_
/// rules`'s `connect logic, electrical;` eligibility rule) auto-splices
/// a `bridge` instance into `Design::flat_modules`; `CircuitCompiler`
/// always builds from the flat form, so the auto-inserted bridge reaches
/// the compiled circuit and DC-solves exactly like an authored instance.
const AUTO_INSERTED_FIXTURE: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    connectrules basic_rules;
        connect logic, electrical;
    endconnectrules

    connectmodule bridge (inout logic a, inout electrical b);
        analog begin
            V(b) <+ 1.0;
        end
    endmodule

    module digital_leaf (inout logic p);
    endmodule

    module top ();
        electrical vmid;
        digital_leaf d1 (vmid);
    endmodule
"#;

#[test]
fn auto_inserted_connect_module_bridges_a_genuine_discipline_crossing_end_to_end() {
    let design = parse_and_elaborate(AUTO_INSERTED_FIXTURE, &piperine_lang::SourceMap::dummy())
        .expect("a genuine electrical<->logic crossing with an eligible connectrules pair elaborates");

    // MD-25: the authored form never gained the bridge; only the derived
    // flat form did.
    let top_authored = design.module("top").expect("top elaborated");
    assert_eq!(top_authored.instances.len(), 1, "authored: only `d1`");
    let top_flat = design.flat_module("top").expect("top's flattened form");
    assert_eq!(top_flat.instances.len(), 2, "authored `d1` + auto-inserted bridge");
    assert!(top_flat.instances.iter().any(|i| i.module == "bridge"));

    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler
        .build_circuit_mapped("top")
        .expect("CircuitCompiler compiles a circuit containing the auto-inserted connect module");

    let dc_result = circuit.dc(Context::default()).expect("dc analysis starts").solve().expect("dc converges");
    let vmid = info.nets.get("vmid").expect("net `vmid` (the auto-inserted bridge's electrical side)");
    let v_mid = dc_result.get_node(vmid).expect("v(vmid)");
    assert!((v_mid - 1.0).abs() < 1e-6, "the auto-inserted bridge drives v(vmid) to 1.0V, got {v_mid}");
}

/// T37 (LRMC-C7-01): a discipline crossing with NO `connectrules` block
/// registered anywhere in the design (unlike the fixture above) is
/// ineligible for auto-insertion and must still fail loud — naming the
/// missing rule, not a stale generic message.
#[test]
fn discipline_crossing_with_no_eligible_connectrule_still_errors_naming_the_missing_rule() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        discipline electrical;
            domain continuous;
        enddiscipline

        module digital_leaf (inout logic p);
        endmodule

        module top ();
            electrical vmid;
            digital_leaf d1 (vmid);
        endmodule
    ";
    let err = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect_err("a discipline crossing with no connectrules coverage must fail loud");
    let msg = format!("{err}");
    assert!(msg.contains("logic") && msg.contains("electrical"), "names the mismatched pair: {msg}");
    assert!(msg.contains("no `connectrules` rule"), "names the missing rule as the reason: {msg}");
}

const MIXED_FIXTURE: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module vsrc (inout electrical p, inout electrical n);
        parameter real dc = 1.0;
        analog begin
            V(p, n) <+ dc;
        end
    endmodule

    module resistor (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog begin
            I(p, n) <+ V(p, n) / r;
        end
    endmodule

    connectmodule wire_bridge (inout electrical a, inout electrical b);
        analog begin
            V(a, b) <+ 0;
        end
    endmodule

    module counter (input logic clk, output logic q);
        integer count;
        always @(posedge clk) count <= count + 1;
    endmodule

    module top ();
        electrical gnd, vin, vmid;
        logic clk_net, q_net;
        vsrc #(.dc(1.0)) v1 (vin, gnd);
        resistor #(.r(1.0e3)) r1 (vin, vmid);
        wire_bridge b1 (vmid, gnd);
        counter c1 (clk_net, q_net);
    endmodule
"#;

#[test]
fn mixed_analog_digital_fixture_with_a_connectmodule_bridge_simulates_end_to_end() {
    let design = parse_and_elaborate(MIXED_FIXTURE, &piperine_lang::SourceMap::dummy())
        .expect("a mixed analog/digital design with an explicit connectmodule instance elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").expect("CircuitCompiler builds one circuit spanning both domains");

    // Analog half: the connectmodule bridges vmid to gnd at 0V, so the
    // resistor sees the full source voltage across it and vmid settles at
    // ground potential (the bridge, not the resistor, sets the DC node).
    let dc_result = circuit.dc(Context::default()).expect("dc analysis starts").solve().expect("dc converges");
    let vin = info.nets.get("vin").expect("net `vin`");
    let vmid = info.nets.get("vmid").expect("net `vmid`");
    let v_in = dc_result.get_node(vin).expect("v(vin)");
    let v_mid = dc_result.get_node(vmid).expect("v(vmid)");
    assert!((v_in - 1.0).abs() < 1e-6, "v(vin) = 1.0 V (the ideal source), got {v_in}");
    assert!(v_mid.abs() < 1e-6, "the connectmodule bridge holds v(vmid) at ground, got {v_mid}");

    // Digital half, in the SAME compiled circuit: force two posedges on
    // clk_net and confirm the counter's register increments — the same
    // proof T33 (`tests/vams_vertical_slice.rs`) established, now
    // alongside live analog content and a connectmodule instance rather
    // than in isolation.
    let counter_dev_index = circuit
        .all_devices()
        .iter()
        .position(|d| d.name() == "c1")
        .expect("the `c1` counter instance is a device in the built circuit");
    let count_before =
        circuit.all_devices()[counter_dev_index].digital_hidden_snapshot().expect("hidden register state").0[0];

    let clk_idx = *info.digital_nets.get("clk_net").expect("`clk_net` mapped to a digital net");
    const EDGES: u32 = 2;
    for i in 0..EDGES {
        let base = (2 * i) as f64;
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 0.5,
            net: DigitalNet(clk_idx),
            value: LogicValue::Zero,
            source: usize::MAX,
            seq: (2 * i) as u64, region: Region::Active,
        });
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 1.0,
            net: DigitalNet(clk_idx),
            value: LogicValue::One,
            source: usize::MAX,
            seq: (2 * i + 1) as u64, region: Region::Active,
        });
    }

    let opts = TransientAnalysisOptions::new(4.0, 0.05);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    let result = tran.solve().expect("the mixed analog/digital circuit's transient run completes");
    assert!(result.len() > 5, "the run recorded a meaningful number of steps");

    let count_after =
        circuit.all_devices()[counter_dev_index].digital_hidden_snapshot().expect("hidden register state").0[0];
    assert_eq!(
        count_after - count_before,
        i64::from(EDGES),
        "the digital register still increments correctly alongside live analog content \
         (before={count_before}, after={count_after})"
    );

    // And the analog node is still tracked correctly through the same
    // transient run that also advanced the digital register.
    let last_vmid = result.iter().last().expect("at least one step").get_node(vmid).expect("v(vmid) recorded");
    assert!(last_vmid.abs() < 1e-3, "v(vmid) stays near ground through the transient run, got {last_vmid}");
}
