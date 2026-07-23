//! Limiter checkpoint/restore (ABI-04): a `$limit` device's mutable
//! non-accept-gated state — the `active` flag, the vcrit seeds, and the vold
//! slots — round-trips through `Element::checkpoint_state`/`restore_state`, so
//! a rejected transient/DC attempt rewinds the limiter to the last accepted
//! state instead of stalling on dirty rejected-attempt values.

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

/// `L(d, s)` with a single `$limit` — exercises the limiter checkpoint path.
fn limiter_circuit() -> CircuitInstance {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod L (inout d: Electrical, inout s: Electrical) { param vto: Real = 1.0; }
        analog L { I(d, s) <+ $limit(\"limvds\", V(d, s), 0.0, vto, 0.0); }
        mod Top (inout a: Electrical, inout b: Electrical) { L(a, b); }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    from_ir(&elab, &bodies, "Top")
}

/// A plain resistor — no `$limit`, so the limiter checkpoint is `None`.
fn resistor_circuit() -> CircuitInstance {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    from_ir(&elab, &bodies, "TopR")
}

/// Spec ABI-04: a device with a `$limit` declares `SUPPORTS_ROLLBACK` (it owns
/// mutable limiter state the solver must checkpoint), and a plain resistor does
/// not.
#[test]
fn supports_rollback_flag_tracks_the_limiter() {
    let lim = limiter_circuit();
    let dev = &lim.all_devices()[0];
    assert!(
        dev.capabilities().contains(ElementCapabilities::SUPPORTS_ROLLBACK),
        "$limit device must declare SUPPORTS_ROLLBACK"
    );

    let res = resistor_circuit();
    let dev = &res.all_devices()[0];
    assert!(
        !dev.capabilities().contains(ElementCapabilities::SUPPORTS_ROLLBACK),
        "resistor has no limiter — no SUPPORTS_ROLLBACK"
    );
}

/// Spec ABI-04: a `$limit` device checkpoints `Some`, a resistor `None`.
#[test]
fn limiter_device_checkpoints_some_resistor_none() {
    let lim = limiter_circuit();
    let dev = &lim.all_devices()[0];
    let ckpt = dev.checkpoint_state().expect("$limit device returns a checkpoint");
    // Layout: [active, seed_0, vold_0] for one $limit slot.
    assert!(ckpt.real_state.len() >= 3, "checkpoint packs active+seed+vold");

    let res = resistor_circuit();
    let dev = &res.all_devices()[0];
    assert!(dev.checkpoint_state().is_none(), "resistor has no limiter state");
}

/// Spec ABI-04: after a rejected attempt dirties the limiter (active flag +
/// vold slots), `restore_state` rewinds them to the pre-attempt checkpoint.
#[test]
fn limiter_state_round_trips_through_checkpoint_restore() {
    let mut ci = limiter_circuit();
    let dev = &mut ci.all_devices_mut()[0];

    // Capture the pre-attempt (seeded) state.
    let ckpt0 = dev.checkpoint_state().expect("$limit device checkpoints");
    let vold0 = dev.runtime_banks().0.to_vec();
    assert!(!vold0.is_empty(), "limiter vold slots are seeded at construction");
    let active0 = dev.limiting_active();

    // Simulate a rejected attempt dirtying the limiter: flip `active` and
    // overwrite the vold slot. Built from ckpt0's layout so the seed is kept.
    let mut dirty_real = ckpt0.real_state.clone();
    dirty_real[0] = 1.0; // active = true
    if let Some(last) = dirty_real.last_mut() {
        *last = 99.0; // vold slot changed
    }
    let dirty = ElementCheckpoint { int_state: Vec::new(), real_state: dirty_real };
    dev.restore_state(&dirty);
    assert!(dev.limiting_active(), "dirty restore flipped the active flag");
    assert_eq!(
        dev.runtime_banks().0.last(),
        Some(&99.0),
        "dirty restore overwrote the vold slot"
    );

    // Restore the pre-attempt checkpoint — the limiter must rewind exactly.
    dev.restore_state(&ckpt0);
    assert_eq!(
        dev.limiting_active(),
        active0,
        "active flag restored to pre-attempt"
    );
    assert_eq!(
        dev.runtime_banks().0,
        vold0.as_slice(),
        "vold slots restored to pre-attempt values"
    );
}

/// Spec ABI-04 (idempotence): checkpointing again after a restore captures the
/// restored state, and a second restore reproduces it — the retry after a
/// reject starts from clean limiter state.
#[test]
fn checkpoint_after_restore_recaptures_clean_state() {
    let mut ci = limiter_circuit();
    let dev = &mut ci.all_devices_mut()[0];

    let ckpt0 = dev.checkpoint_state().expect("checkpoint");
    let vold0 = dev.runtime_banks().0.to_vec();

    // Dirty + restore (a rejected attempt + rewind).
    let mut dirty_real = ckpt0.real_state.clone();
    if let Some(last) = dirty_real.last_mut() {
        *last = -42.0;
    }
    dev.restore_state(&ElementCheckpoint { int_state: Vec::new(), real_state: dirty_real });
    dev.restore_state(&ckpt0);

    // A fresh checkpoint must match the original (the retry's checkpoint).
    let ckpt1 = dev.checkpoint_state().expect("checkpoint");
    assert_eq!(ckpt1.real_state, ckpt0.real_state, "re-checkpoint matches pre-attempt");
    assert_eq!(dev.runtime_banks().0, vold0.as_slice(), "vold stable after round-trip");
}
