//! Limiter checkpoint/restore (ABI-04): a `$limit` device's mutable
//! non-accept-gated state — the `active` flag, the vcrit seeds, and the vold
//! slots — round-trips through `Element::checkpoint_state`/`restore_state`, so
//! a rejected transient/DC attempt rewinds the limiter to the last accepted
//! state instead of stalling on dirty rejected-attempt values.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::{
    CircularArrayBuffer2, DcAnalysisState, ElementCapabilities, ElementCheckpoint, LimitReason,
};
use piperine_solver::prelude::{CircuitInstance, Context};

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
    // The active flag is the first real_state entry of the limiter checkpoint.
    let active0 = ckpt0.real_state.first().copied().unwrap_or(0.0);

    // Simulate a rejected attempt dirtying the limiter: flip `active` and
    // overwrite the vold slot. Built from ckpt0's layout so the seed is kept.
    let mut dirty_real = ckpt0.real_state.clone();
    dirty_real[0] = 1.0; // active = true
    if let Some(last) = dirty_real.last_mut() {
        *last = 99.0; // vold slot changed
    }
    let dirty = ElementCheckpoint { int_state: Vec::new(), real_state: dirty_real };
    dev.restore_state(&dirty);
    // Observe via a fresh checkpoint (the active flag is not load-cached here).
    let dirty_ckpt = dev.checkpoint_state().expect("re-checkpoint after dirty");
    assert_eq!(
        *dirty_ckpt.real_state.first().unwrap(),
        1.0,
        "dirty restore flipped the active flag"
    );
    assert_eq!(
        dirty_ckpt.real_state.last(),
        Some(&99.0),
        "dirty restore overwrote the vold slot"
    );

    // Restore the pre-attempt checkpoint — the limiter must rewind exactly.
    dev.restore_state(&ckpt0);
    let restored_ckpt = dev.checkpoint_state().expect("re-checkpoint after restore");
    assert_eq!(
        restored_ckpt.real_state.first().copied().unwrap_or(0.0),
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

// ── T8: PiperineDevice produces a LimitingReport (ABI-09/12) ───────────────
//
// When the `$limit` limiter clamps during a load, `limiting_report()` returns
// a structured report naming the clamped node, the proposed vs limited value,
// the limiter name, and the reason. Since phdl-introspection-attributes PIA-15,
// the name/reason come from the call-site `$limit` kind via the kernel's
// per-slot catalog (here `limvds` → `"limvds"`/`VdsStep`), not the former
// hardcoded `"pnjlim"`/`VoltageStep`.

/// Spec ABI-09/12 + PIA-15/16: a `$limit` device driven through a load that
/// clamps produces a `LimitingReport` with the documented fields. The report
/// is read AFTER `load_dc` (the load caches it; the solver reads it in
/// `apply_limiting_reports`).
#[test]
fn piperine_device_produces_limiting_report_when_clamping() {
    let mut ci = limiter_circuit();
    let dev = &mut ci.all_devices_mut()[0];

    // The limiter (limvds, vto=1.0) seeds its vold slot to vcrit near turn-on.
    // A load at a large Vds (5 V) clamps well below the raw proposal, so
    // `Limiter::update` flags active and the report is cached.
    // Node layout: Top instantiates L(a, b) → d=node a (idx 0), s=node b (idx 1).
    let mut buf = CircularArrayBuffer2::new(1, 2);
    let guess = ndarray::arr1(&[5.0, 0.0]);
    buf.push(&guess.view());
    let dc_state = DcAnalysisState::new(&buf, &[], 1.0);
    let _ = dev.load_dc(&dc_state, &Context::default());

    let report = dev
        .limiting_report()
        .expect("limiter produced a report while clamping");
    // PIA-15: the name is the call-site kind (`limvds`), not hardcoded "pnjlim".
    assert_eq!(report.limiter_name, "limvds");
    // PIA-16: limvds infers VdsStep (not the default VoltageStep).
    assert_eq!(report.reason, LimitReason::VdsStep);
    assert!(
        report.net.idx().is_some(),
        "report names a real MNA unknown"
    );
    // The limiter moved the clamped node off its raw value to reduce the
    // branch voltage — the limited node voltage differs from the proposed.
    assert_ne!(
        report.limited_value, report.proposed,
        "limited ({}) must differ from proposed ({}) while clamping",
        report.limited_value,
        report.proposed
    );
}

/// Spec ABI-11: a device whose limiter has NOT clamped reports `None` — the
/// gate stays open and the host sees no false diagnostic.
#[test]
fn piperine_device_reports_none_when_limiter_idle() {
    let mut ci = limiter_circuit();
    let dev = &mut ci.all_devices_mut()[0];

    // A load at Vds = 0: the limiter (limvds) does not clamp (0 V is within
    // the seed), so no report is cached.
    let mut buf = CircularArrayBuffer2::new(1, 2);
    let guess = ndarray::arr1(&[0.0, 0.0]);
    buf.push(&guess.view());
    let dc_state = DcAnalysisState::new(&buf, &[], 1.0);
    let _ = dev.load_dc(&dc_state, &Context::default());
    assert!(
        dev.limiting_report().is_none(),
        "idle limiter reports None"
    );
}

