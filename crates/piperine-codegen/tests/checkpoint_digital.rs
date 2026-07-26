//! Digital register checkpoint/restore (ABI-05): a digital device's
//! non-accept-gated register state — `vars_int`, `vars_real`, `prev_watch` —
//! round-trips through `Element::checkpoint_state`/`restore_state`, so a
//! rejected settle rewinds the registers a `seq_phase` committed during the
//! failed attempt.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::{ElementCapabilities, ElementCheckpoint};
use piperine_solver::prelude::CircuitInstance;

fn from_ir(design: &piperine_lang::pom::Design, bodies: &HashMap<String, LoweredBody>, top: &str) -> CircuitInstance {
    let mut c = CircuitCompiler::new(design, bodies);
    c.build_circuit(top).expect("circuit compiles")
}

/// A clocked register (`@posedge(clk) Q <- D`) — owns `vars_int` (the Q
/// register) and `prev_watch` (clock-edge detection memory).
fn register_circuit() -> CircuitInstance {
    let src = "
        discipline Bit { storage Boolean; }
        mod Reg (input clk: Bit, input D: Bit, output Q: Bit) {}
        digital Reg { @ posedge(clk) { Q <- D; } }
        mod Top (input clk: Bit, input D: Bit, output Q: Bit) { Reg(clk, D, Q); }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    from_ir(&elab, &bodies, "Top")
}

/// Spec ABI-05: a stateful digital device declares `SUPPORTS_ROLLBACK`.
#[test]
fn digital_register_device_declares_supports_rollback() {
    let ci = register_circuit();
    let dev = &ci.all_devices()[0];
    assert!(
        dev.capabilities().contains(ElementCapabilities::SUPPORTS_ROLLBACK),
        "stateful digital device must declare SUPPORTS_ROLLBACK"
    );
}

/// Spec ABI-05: a stateful digital device checkpoints `Some`, and the int
/// carrier carries the registers + watch memory.
#[test]
fn digital_device_checkpoints_registers() {
    let ci = register_circuit();
    let dev = &ci.all_devices()[0];
    let ckpt = dev.checkpoint_state().expect("digital device checkpoints");
    // int_state = vars_int ++ prev_watch. A register with a clock has at
    // least one register slot + watch terms, so the carrier is non-empty.
    assert!(!ckpt.int_state.is_empty(), "checkpoint carries register + watch state");
}

/// Spec ABI-05: after a rejected settle dirties the registers, `restore_state`
/// rewinds `vars_int`/`vars_real`/`prev_watch` to the pre-settle checkpoint.
#[test]
fn digital_registers_round_trip_through_checkpoint_restore() {
    let mut ci = register_circuit();
    let dev = &mut ci.all_devices_mut()[0];

    // Capture the pre-settle register state.
    let ckpt0 = dev.checkpoint_state().expect("digital device checkpoints");
    let hidden0 = dev
        .digital_hidden_snapshot()
        .expect("digital device exposes registers");
    assert!(!hidden0.0.is_empty(), "vars_int/prev_watch present");

    // Simulate a rejected settle dirtying the int registers: overwrite every
    // int slot with a sentinel distinct from the seeded (X) values.
    let dirty_int = vec![777_i64; ckpt0.int_state.len()];
    let dirty = ElementCheckpoint {
        int_state: dirty_int,
        real_state: ckpt0.real_state.clone(),
    };
    dev.restore_state(&dirty);
    let dirty_hidden = dev.digital_hidden_snapshot().expect("registers");
    assert!(
        dirty_hidden.0.iter().all(|&v| v == 777),
        "dirty restore overwrote the int registers"
    );

    // Restore the pre-settle checkpoint — registers must rewind exactly.
    dev.restore_state(&ckpt0);
    let restored_hidden = dev.digital_hidden_snapshot().expect("registers");
    assert_eq!(
        restored_hidden, hidden0,
        "registers + watch memory restored to pre-settle values"
    );
}

/// Spec ABI-05 (combined): a mixed-signal device (analog limiter + digital
/// registers) checkpoints and restores BOTH — the limiter slice and the
/// digital slice are independent within one `ElementCheckpoint`.
#[test]
fn combined_limiter_and_digital_round_trip() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        discipline Bit { storage Boolean; }
        mod M (inout d: Electrical, inout s: Electrical,
               input clk: Bit, output Q: Bit) {
            param vto: Real = 1.0;
        }
        analog M { I(d, s) <+ $limit(\"limvds\", V(d, s), 0.0, vto, 0.0); }
        digital M { @ posedge(clk) { Q <- 1; } }
        mod Top (inout a: Electrical, inout b: Electrical,
                 input clk: Bit, output Q: Bit) { M(a, b, clk, Q); }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    let mut ci = from_ir(&elab, &bodies, "Top");
    let dev = &mut ci.all_devices_mut()[0];

    // Sanity: the mixed device declares rollback and carries both halves.
    assert!(dev.capabilities().contains(ElementCapabilities::SUPPORTS_ROLLBACK));
    let ckpt0 = dev.checkpoint_state().expect("mixed device checkpoints");
    assert!(!ckpt0.int_state.is_empty(), "digital registers present");
    // Limiter slice is the leading real_state: [active, seed, vold] (≥ 3).
    assert!(ckpt0.real_state.len() >= 3, "limiter slice present");

    let limiter0: Vec<f64> = ckpt0.real_state.iter().take(3).copied().collect();
    let hidden0 = dev.digital_hidden_snapshot().expect("registers");

    // Dirty both halves: flip the limiter active flag and overwrite registers.
    let mut dirty_real = ckpt0.real_state.clone();
    dirty_real[0] = 1.0;
    let dirty = ElementCheckpoint {
        int_state: vec![777; ckpt0.int_state.len()],
        real_state: dirty_real,
    };
    dev.restore_state(&dirty);
    // Observe the limiter active flag via a fresh checkpoint (the report cache
    // is load-populated; the active flag itself is packed into real_state[0]).
    let dirty_ckpt = dev.checkpoint_state().expect("re-checkpoint after dirty");
    assert_eq!(
        *dirty_ckpt.real_state.first().unwrap(),
        1.0,
        "limiter active flag flipped by dirty restore"
    );
    let dirty_hidden = dev.digital_hidden_snapshot().expect("registers");
    assert!(dirty_hidden.0.iter().all(|&v| v == 777), "registers overwritten");

    // Restore the pre-attempt checkpoint — both halves rewind independently.
    dev.restore_state(&ckpt0);
    let restored_limiter: Vec<f64> = {
        let live = dev.checkpoint_state().expect("re-checkpoint");
        live.real_state.iter().take(3).copied().collect()
    };
    assert_eq!(restored_limiter, limiter0, "limiter slice restored");
    assert_eq!(
        dev.digital_hidden_snapshot().unwrap(),
        hidden0,
        "digital registers restored"
    );
}
