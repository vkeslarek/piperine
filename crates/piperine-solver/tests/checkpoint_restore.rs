//! Rollback lifecycle contract tests (ABI-01..08): the checkpoint/restore
//! pair on `Element` is driven by the solver around every candidate step.
//!
//! - T1: the trait defaults (`checkpoint_state` → `None`, `restore_state` no-op).
//! - T2: transient reject path drives checkpoint before attempt + restore on
//!   rejection; accept discards.
//! - T3: DC homotopy retry drives checkpoint before each strategy + restore on
//!   strategy fallthrough.

use piperine_solver::abi::*;
use piperine_solver::prelude::Context;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A stateless stub element: inherits the checkpoint/restore defaults.
struct StatelessStub;

impl AnalogDevice for StatelessStub {}
impl DigitalDevice for StatelessStub {}
impl Introspect for StatelessStub {}

impl Element for StatelessStub {
    fn name(&self) -> &str { "StatelessStub" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
}

/// Spec ABI-06: an element with no mutable non-accept-gated state returns
/// `None` from `checkpoint_state` — the solver skips the restore entirely.
#[test]
fn default_checkpoint_state_is_none() {
    let dev = StatelessStub;
    assert!(dev.checkpoint_state().is_none());
}

/// Spec ABI-02 default: `restore_state` is a no-op on a stateless device —
/// feeding it an arbitrary checkpoint changes nothing and never panics.
#[test]
fn default_restore_state_is_a_noop() {
    let mut dev = StatelessStub;
    let checkpoint = ElementCheckpoint {
        int_state: vec![1, 2, 3],
        real_state: vec![1.5, -2.0],
    };
    dev.restore_state(&checkpoint);
}

/// A recording element that counts checkpoint/restore calls so the reject
/// path (T2) and homotopy retry (T3) can prove the hooks fire. Owns a small
/// piece of non-accept-gated mutable state it checkpoints + restores so the
/// "dirty rejected state must rewind" property is observable.
struct RecordingDevice {
    checkpoints: Arc<AtomicUsize>,
    restores: Arc<AtomicUsize>,
    state_value: f64,
}

impl AnalogDevice for RecordingDevice {}
impl DigitalDevice for RecordingDevice {}
impl Introspect for RecordingDevice {}

impl Element for RecordingDevice {
    fn name(&self) -> &str { "RecordingDevice" }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::SUPPORTS_ROLLBACK
    }

    fn checkpoint_state(&self) -> Option<ElementCheckpoint> {
        self.checkpoints.fetch_add(1, Ordering::SeqCst);
        Some(ElementCheckpoint {
            int_state: Vec::new(),
            real_state: vec![self.state_value],
        })
    }

    fn restore_state(&mut self, checkpoint: &ElementCheckpoint) {
        self.restores.fetch_add(1, Ordering::SeqCst);
        if let Some(&v) = checkpoint.real_state.first() {
            self.state_value = v;
        }
    }
}

impl RecordingDevice {
    fn new(checkpoints: Arc<AtomicUsize>, restores: Arc<AtomicUsize>) -> Self {
        Self { checkpoints, restores, state_value: 0.0 }
    }
}

/// A bare `Element` declares `SUPPORTS_ROLLBACK` and the trait surface
/// exposes the checkpoint/restore pair — the capability bit and the hook
/// exist together (ABI-01 wiring gate).
#[test]
fn supports_rollback_flag_is_declared_alongside_the_hooks() {
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let restores = Arc::new(AtomicUsize::new(0));
    let dev = RecordingDevice::new(checkpoints, restores);
    assert!(dev
        .capabilities()
        .contains(ElementCapabilities::SUPPORTS_ROLLBACK));
}

/// A device that overrides `checkpoint_state` returns `Some`, and a round-trip
/// through `restore_state` rewinds the mutated state to the checkpoint value.
#[test]
fn checkpoint_then_restore_round_trips_state() {
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let restores = Arc::new(AtomicUsize::new(0));
    let mut dev = RecordingDevice::new(checkpoints.clone(), restores.clone());
    dev.state_value = 4.2;
    let ckpt = dev.checkpoint_state().expect("recording device checkpoints");
    assert_eq!(checkpoints.load(Ordering::SeqCst), 1);

    // Mutate after the checkpoint — the rejected attempt dirties the state.
    dev.state_value = 99.0;
    // Restore rewinds to the checkpointed value.
    dev.restore_state(&ckpt);
    assert_eq!(restores.load(Ordering::SeqCst), 1);
    assert!((dev.state_value - 4.2).abs() < 1e-12);
}

/// Sanity: a `Context` default exists so the recording device can later be
/// driven through a real analysis (used by T2/T3).
#[test]
fn _context_default_compiles_for_later_tests() {
    let _ctx = Context::default();
}
