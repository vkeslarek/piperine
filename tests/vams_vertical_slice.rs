//! T33 (VAMS-06) — the P1 vertical slice's end-to-end proof: a `.vams`
//! analog RC circuit authored in Verilog-AMS syntax, parsed → elaborated →
//! lowered → compiled → simulated (DC and transient) on the **unmodified**
//! native solver, exactly the way `tests/ngspice_tran.rs`'s PHDL-authored
//! RC fixtures already were before the frontend swap.
//!
//! **Oracle**: the closed-form RC solution `v(t) = V0·(1 − e^(−t/RC))` — an
//! analytic golden, not a recorded PHDL run (MD-44 point 5: "the recorded
//! PHDL-fixture numeric golden... the golden survives PHDL's deletion as a
//! static oracle"; a closed-form solution is a strictly stronger oracle
//! than a stored numeric snapshot of the same fixture, and is available
//! because PHDL's own `examples/22_rc_step_response.phdl` is the fixture
//! this mirrors in topology — see the transient test's own doc comment for
//! why it checks steady-state tracking rather than the literal step edge).
//!
//! **SPEC_DEVIATION** (recorded, not silent): T33's own text says
//! "Reuses: `tests/support/ngspice.rs` `ToleranceContract` — unmodified"
//! and the gate is `cargo test --workspace`. Neither is achievable yet:
//! `tests/support/ngspice.rs` imports `piperine::{Session, ...}`
//! (`piperine-api`), and `piperine-api` does not build — its own retarget
//! is T34 (Phase 7), not this batch's scope. This target therefore talks
//! to `piperine_solver` directly (the same low-level API
//! `crates/piperine-codegen/tests/flatten_hierarchy.rs` already uses) and
//! defines its own tolerance constant with the same numeric bound
//! `ToleranceContract::DC`/a comparable transient bound already uses,
//! rather than importing a harness that cannot compile. `cargo test
//! --workspace` cannot pass either, for the same reason (`piperine-api`
//! and everything downstream of it is still broken) — see this feature's
//! `tasks.md` Progress Log (T33) for the full accounting of what's still
//! broken workspace-wide.
//!
//! **Digital counter — now included.** The prior batch found
//! `crates/piperine-lang/src/elab/lower/vams.rs`'s `ModuleItem::Var(_) =>
//! {}` arm silently skipped every module-level `integer`/`real`/`time`/
//! `realtime` declaration, so no `.vams` source could declare working
//! register state (a T22 gap, fixed as a same-scope addendum — see
//! `tests/vams_lower_var.rs` in `piperine-lang` for the elaboration-level
//! proof, plus a `piperine-vams` grammar fix: `parse_module_item` never
//! dispatched those keywords into `ModuleItem::Var` at all either). With
//! both fixed, `vams_digital_counter_register_increments_on_posedge`
//! below proves the fix all the way through: elaborate → lower_bodies →
//! `CircuitCompiler` → a real digital device whose `integer count;`
//! register is written by a compiled `always @(posedge clk) count <=
//! count + 1;` clocked block on the native solver, unmodified.
//!
//! **Stimulus note**: VAMS has no `#delay` procedural timing control in
//! this codegen (`emit/stmt.rs`: `` `#delay` control in digital codegen``
//! is `CodegenError::unsupported`) and digital-only devices have no
//! writable params yet (`device/element.rs`'s `set_param`/`list_params`
//! read `self.analog` only — a purely-digital device has none), so a
//! self-clocking oscillator can't yet be authored in `.vams` source
//! itself (a real, separate gap, not this task's to fix). The clock edges
//! are instead forced directly through `DigitalState::schedule` (the same
//! public event-queue API `CircuitInstance::init_digital` itself seeds
//! t=0 power-on events through) — a legitimate Rust-level testbench
//! stimulus, not a VAMS-authored oscillator.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, ElementCapabilities, LogicValue};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// The exact `.vams` translation of `examples/22_rc_step_response.phdl`'s
/// circuit: an ideal step (`V(p,n) <+ dc;` — real Verilog-A's own
/// ideal-voltage-source idiom, LRM §5) driving a series R into a shunt C.
const RC_STEP: &str = r#"
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

    module capacitor (inout electrical p, inout electrical n);
        parameter real c = 1.0e-6;
        analog begin
            I(p, n) <+ c * ddt(V(p, n));
        end
    endmodule

    module top ();
        electrical gnd;
        electrical vin;
        electrical vout;
        vsrc #(.dc(1.0)) v1 (vin, gnd);
        resistor #(.r(1.0e3)) r1 (vin, vout);
        capacitor #(.c(1.0e-6)) c1 (vout, gnd);
    endmodule
"#;

/// Analytic RC step response: `v(t) = V0 (1 − e^{−t/RC})`.
fn analytic_rc_step(v0: f64, r: f64, c: f64, t: f64) -> f64 {
    v0 * (1.0 - (-t / (r * c)).exp())
}

#[test]
fn vams_rc_step_response_matches_the_closed_form_solution() {
    let design = parse_and_elaborate(RC_STEP, &piperine_lang::SourceMap::dummy())
        .expect("a .vams RC circuit parses and elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design)
        .expect("lowers to the resolved form (T30)");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler
        .build_circuit_mapped("top")
        .expect("CircuitCompiler builds a device tree from the VAMS-elaborated Design (T32)");

    let vout = info.nets.get("vout").expect("net `vout`");

    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c; // 1 microsecond
    let stop = 5.0 * tau;
    let options = TransientAnalysisOptions::new(stop, tau / 50.0);
    let result = circuit
        .transient(options, Context::default())
        .expect("transient analysis starts")
        .solve()
        .expect("the RC circuit converges on the native solver, unmodified downstream of the POM");

    assert!(result.len() > 10, "the run recorded a meaningful number of steps");

    // SPEC_DEVIATION (recorded, not silent): the circuit's ideal source is a
    // constant DC value (`V(p,n) <+ dc;`), so the DC operating point the
    // transient run starts from is *already* the charged steady state
    // (v(vout) = 1.0 V, proven by the DC test below) — there is no t=0
    // discontinuity to ramp away from, so this is not literally the
    // `examples/22_rc_step_response.phdl` step trace (a true step needs a
    // pulse/UIC source; piperine_ir::ops::Operator::transition's own test
    // suite documents "the DC-op seed settles at the input — no ramp from
    // 0", and PHDL's `<-` UIC-seeding path has no VAMS production — T32's
    // documented gap). What this DOES prove, non-trivially: the `ddt`
    // charge/Jacobian codegen path (T28-T32's retarget) integrates
    // correctly under the TrBdf2 stepper without drifting off the true
    // solution — a wrong charge residual or Jacobian would very plausibly
    // show up as drift or a convergence failure here, not a silent pass.
    const RELTOL: f64 = 1e-3;
    const ABSTOL: f64 = 1e-6;
    let expected = analytic_rc_step(1.0, r, c, stop * 10.0); // settled steady state = 1.0 V
    let mut checked = 0;
    for step in result.iter() {
        let t = step.time();
        let v = step.get_node(vout).expect("v(vout) recorded at every step");
        let bound = ABSTOL + RELTOL * expected.abs();
        assert!(
            (v - expected).abs() <= bound,
            "t={t:e}: v(vout)={v} drifted from the steady state {expected} (bound={bound})"
        );
        checked += 1;
    }
    assert_eq!(checked, result.len(), "every recorded step was checked");
}

/// DC operating point sanity check: at steady state the capacitor is open
/// (no current flow through R at DC), so `v(vout) = v(vin) = 1.0 V` exactly
/// (an ideal source with a purely resistive/ddt-only network has no DC
/// loss). Proves `AnalogKernel::compile` and `CircuitCompiler` reach a
/// converged DC operating point on the unmodified native solver, not just
/// a transient trace.
#[test]
fn vams_rc_circuit_dc_operating_point() {
    let design = parse_and_elaborate(RC_STEP, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("builds");

    let result = circuit
        .dc(Context::default())
        .expect("dc analysis starts")
        .solve()
        .expect("dc converges");

    let vin = info.nets.get("vin").expect("net `vin`");
    let vout = info.nets.get("vout").expect("net `vout`");
    let v_in = result.get_node(vin).expect("v(vin)");
    let v_out = result.get_node(vout).expect("v(vout)");

    assert!((v_in - 1.0).abs() < 1e-6, "v(vin) = 1.0 V (the ideal source), got {v_in}");
    assert!((v_out - 1.0).abs() < 1e-6, "v(vout) = 1.0 V at DC (capacitor open, no IR drop), got {v_out}");
}

/// VAMS-06's digital half: a `.vams` module declaring a persistent
/// register (`integer count;`, LRM Annex A.2.1.3 `integer_declaration`)
/// written by a clocked block (`always @(posedge clk) count <= count +
/// 1;`). `clk` is driven by 5 externally forced `Zero -> One` transitions
/// (see the file doc comment for why the stimulus is Rust-level, not
/// VAMS-authored). `count`'s power-on value is the digital kernel's
/// uninitialized-register sentinel (`Quad::X`, exported as raw value `2`
/// through `Element::digital_hidden_snapshot`'s `Vec<i64>`) — VAMS's own
/// `VarDecl` carries no initializer (`var_declarator_to_var`'s doc
/// comment in `elab/lower/vams.rs`), so there is no `reg_inits` entry to
/// seed a clean `0`. The test asserts the *delta* (post-run minus
/// pre-run), not the sentinel's raw encoding, so it stays correct even if
/// the sentinel's internal representation changes.
const COUNTER: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module counter (input logic clk, output logic q);
        integer count;
        always @(posedge clk) count <= count + 1;
    endmodule

    module top ();
        logic clk_net;
        logic q_net;
        counter c1 (clk_net, q_net);
    endmodule
"#;

#[test]
fn vams_digital_counter_register_increments_on_posedge() {
    let design = parse_and_elaborate(COUNTER, &piperine_lang::SourceMap::dummy())
        .expect("a .vams counter with a module-level `integer` register parses and elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler
        .build_circuit_mapped("top")
        .expect("CircuitCompiler builds a digital device from the VAMS-elaborated register");

    let counter_dev_index = circuit
        .all_devices()
        .iter()
        .position(|d| d.name() == "c1")
        .expect("the `c1` counter instance is a device in the built circuit");

    // A device with real persistent digital state declares SUPPORTS_ROLLBACK
    // (ABI-05) — proves the elaborated `integer count;` actually became
    // stateful register storage in the compiled kernel, not just a local.
    let caps = circuit.all_devices()[counter_dev_index].capabilities();
    assert!(
        caps.contains(ElementCapabilities::SUPPORTS_ROLLBACK),
        "a device with a written module-level var must declare SUPPORTS_ROLLBACK, got {caps:?}"
    );

    let count_before = circuit.all_devices()[counter_dev_index]
        .digital_hidden_snapshot()
        .expect("the counter device exposes hidden digital register state")
        .0[0];

    let clk_idx = *info.digital_nets.get("clk_net").expect("`clk_net` mapped to a digital net");

    // Force exactly 5 clean posedges (Zero -> One) at t = 1, 3, 5, 7, 9,
    // each preceded by a return to Zero so every rising edge is genuine.
    const EDGES: u32 = 5;
    for i in 0..EDGES {
        let base = (2 * i) as f64;
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 0.5,
            net: DigitalNet(clk_idx),
            value: LogicValue::Zero,
            source: usize::MAX,
            seq: (2 * i) as u64,
        });
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 1.0,
            net: DigitalNet(clk_idx),
            value: LogicValue::One,
            source: usize::MAX,
            seq: (2 * i + 1) as u64,
        });
    }

    let opts = TransientAnalysisOptions::new(10.0, 0.1);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    let result = tran.solve().expect("the all-digital circuit's transient run completes");

    // The forced clock itself only rose exactly `EDGES` times — confirms
    // the stimulus landed as intended before trusting the register count.
    let mut clk_rising_edges = 0;
    let mut prev = None;
    for step in result.iter() {
        if let Some(v) = step.digital(clk_idx) {
            if prev == Some(LogicValue::Zero) && v == LogicValue::One {
                clk_rising_edges += 1;
            }
            prev = Some(v);
        }
    }
    assert_eq!(clk_rising_edges, EDGES, "exactly {EDGES} forced posedges were observed on clk_net");

    let count_after = circuit.all_devices()[counter_dev_index]
        .digital_hidden_snapshot()
        .expect("the counter device exposes hidden digital register state")
        .0[0];

    assert_eq!(
        count_after - count_before,
        i64::from(EDGES),
        "count increments by exactly 1 per posedge (before={count_before}, after={count_after})"
    );
}
