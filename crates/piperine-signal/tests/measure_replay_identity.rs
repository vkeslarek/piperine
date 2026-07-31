//! The online/post-run identity (`ideal.md` §3.5/§4.3, DVT-14): the same
//! reduction over the same sample sequence gives identical results
//! whether streamed live (one `(t, v)` at a time, as a real transient
//! would feed it) or replayed post-run from a stored array.
//!
//! This crate must not depend on `piperine-api`'s `Waveform` (purity,
//! `ideal.md` §3.1) — the "post-run" input here is constructed directly as
//! a plain sample array, by value, the same shape `Waveform` would hand a
//! consumer.

use piperine_signal::measure::gate::{Gate, Gated};
use piperine_signal::measure::handle::Measure;
use piperine_signal::measure::kinds::{Average, Peak, Rms};

/// A synthetic "recorded transient": accepted `(t, v, lte)` triples, the
/// same shape a live driver would produce one at a time.
fn synthetic_samples() -> Vec<(f64, f64, f64)> {
    (0..200)
        .map(|i| {
            let t = i as f64 * 0.01;
            let v = (2.0 * std::f64::consts::PI * t).sin() * (1.0 + 0.3 * t);
            let lte = 1e-4 * (1.0 + t);
            (t, v, lte)
        })
        .collect()
}

#[test]
fn peak_online_and_post_run_replay_agree_exactly() {
    let samples = synthetic_samples();

    let mut online = Measure::arm(Peak::new());
    for &(t, v, lte) in &samples {
        online.feed(t, v, lte); // one at a time, as a live transient would
    }

    let mut post_run = Measure::arm(Peak::new());
    for &(t, v, lte) in samples.iter() {
        // Same call, same order — replay from the stored array is not a
        // different code path, just a different iteration source.
        post_run.feed(t, v, lte);
    }

    assert_eq!(online.value(), post_run.value());
    assert_eq!(online.error_bound(), post_run.error_bound());
}

#[test]
fn average_and_rms_online_and_post_run_replay_agree_exactly() {
    let samples = synthetic_samples();

    let mut avg_online = Measure::arm(Average::new());
    let mut rms_online = Measure::arm(Rms::new());
    for &(t, v, lte) in &samples {
        avg_online.feed(t, v, lte);
        rms_online.feed(t, v, lte);
    }

    let stored: Vec<(f64, f64, f64)> = samples.clone();
    let mut avg_replay = Measure::arm(Average::new());
    let mut rms_replay = Measure::arm(Rms::new());
    for &(t, v, lte) in &stored {
        avg_replay.feed(t, v, lte);
        rms_replay.feed(t, v, lte);
    }

    assert_eq!(avg_online.value(), avg_replay.value());
    assert_eq!(rms_online.value(), rms_replay.value());
}

#[test]
fn a_gated_measure_agrees_between_online_and_post_run_replay_too() {
    let samples = synthetic_samples();

    let mut online = Gated::new(Peak::new(), Gate::Time { t0: 0.5, t1: 1.5 });
    for &(t, v, lte) in &samples {
        online.feed(t, v, lte);
    }

    let stored = samples.clone();
    let mut post_run = Gated::new(Peak::new(), Gate::Time { t0: 0.5, t1: 1.5 });
    for &(t, v, lte) in &stored {
        post_run.feed(t, v, lte);
    }

    assert_eq!(online.value(), post_run.value());
    assert_eq!(online.error_bound(), post_run.error_bound());
    assert!(online.ever_opened());
}
