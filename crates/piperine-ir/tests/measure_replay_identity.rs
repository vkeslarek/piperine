//! The online/post-run identity (`ideal.md` §3.5/§4.3, DVT-14): the same
//! reduction over the same sample sequence gives identical results
//! whether streamed live (one `(t, v)` at a time, as a real transient
//! would feed it) or replayed post-run from a stored array.
//!
//! This crate must not depend on `piperine-api`'s `Waveform` (purity,
//! `ideal.md` §3.1) — the "post-run" input here is constructed directly as
//! a plain sample array, by value, the same shape `Waveform` would hand a
//! consumer.
//!
//! // SPEC_DEVIATION-adjacent honesty note (fix round, DVT-14): `Measure`
//! // is architected as a single streaming accumulator with one entry
//! // point, `feed` (`ideal.md` §3.5: "the same code reduces a stored
//! // Waveform post-run and a live transient online"). Because there is
//! // only one code path by design, a test that merely calls `feed` in a
//! // loop twice cannot fail — it is not proving two independently-coded
//! // reductions agree, only that `feed` is a deterministic function of
//! // its arguments (true of any pure function with no hidden state).
//! // What *is* a genuine, distinguishing property — and what these tests
//! // actually assert — is that a live "online" caller who polls
//! // `value()`/`error_bound()` after every sample (as a dashboard would)
//! // gets the exact same final result as a "post-run" caller who reads
//! // only once at the end: intermediate reads must not perturb the
//! // accumulator's state. A buggy `value()`/`error_bound()` with
//! // observer side effects (e.g. an accidental reset, or caching that
//! // goes stale) would fail this, even though it would pass a bare
//! // double-`feed`-loop comparison.

use piperine_ir::measure::gate::{Gate, Gated};
use piperine_ir::measure::handle::Measure;
use piperine_ir::measure::kinds::{Average, Peak, Rms};

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
        // A live caller polls the running value/bound after every sample
        // (a progress dashboard) — those reads must not perturb the
        // accumulator, or this run's final result would differ from a
        // caller who never reads until the end (post_run, below).
        let _ = online.value();
        let _ = online.error_bound();
    }

    let mut post_run = Measure::arm(Peak::new());
    for &(t, v, lte) in samples.iter() {
        // Same call, same order, but no intermediate reads — a genuinely
        // different calling pattern than `online`'s poll-every-sample loop
        // above, not just a re-run of the identical code.
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
        // Poll every sample, as a live dashboard would — see the header
        // note: this is the property that actually distinguishes "online"
        // from "post-run" here, not merely re-calling `feed`.
        let _ = avg_online.value();
        let _ = rms_online.value();
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
        let _ = online.value();
        let _ = online.error_bound();
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
