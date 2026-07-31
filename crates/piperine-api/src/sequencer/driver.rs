//! `Sequencer<P>` (T19/T23/T24): the coroutine driver whose
//! `next_breakpoints` is exactly its program's pending time wake, resumed
//! once per accepted landing, plus the digital edge half of `wait` (T23)
//! and the `Wake`/`SequencerProgram` shapes both build on.
//!
//! **Why the resume site is safe (D11, `design.md` §C4).** The resume call
//! lives in [`HostElement::accept_timestep`]. The solver reaches that hook
//! from exactly one call site: `CircuitInstance::accept_and_run_digital` →
//! `seed_digital_from_accept_hooks` (`piperine-solver/src/core/circuit.rs`),
//! which is invoked **only** from `TransientDriver::accept_step`
//! (`piperine-solver/src/analyses/transient.rs`) — itself only reached
//! once `assess_step` has already decided to accept the candidate. A
//! rejected step (`reject_step`/`reject_lte_step`) never calls
//! `accept_step`, so `accept_timestep` is never called for it, so the
//! sequencer is never resumed on a step later rejected — for free, from
//! the existing ABI ordering. `on_digital_settled` (T23) is reached from
//! the same `accept_and_run_digital` call, so D11 holds for edge waits
//! too. **No new solver hook beyond `on_digital_settled` is added**; that
//! one lives in `piperine-solver` (T23 is the one task that touches it).
//!
//! `SimHooks` (`crate::hooks::SimHooks`) is not on this path: this module
//! never imports or references it — the sequencer's resume is driven
//! entirely through `HostElement`/`Element`, not the session lifecycle
//! hooks `Session` wires around a build/solve.

use piperine_solver::abi::{AnalogReference, DigitalNet, ElementCapabilities, LimitingReport, LogicValue};

use piperine_signal::events::CrossDir;

use crate::device::host_element::HostElement;
use crate::device::stamp::StateView;

use super::control::SequencerControl;
use super::crossing::CrossingWait;
use super::wait::{EdgeDirection, EdgeWait};

/// What a [`Sequencer`] is waiting for — `ideal.md` §4.1's "one `wait`,
/// three wake sources" table: `Advance` (T19), `WaitEdge` (T23), `Crossing`
/// (T24).
#[derive(Debug, Clone, PartialEq)]
pub enum Wake {
    /// Wake at absolute time `t + dt` from the instant this was returned
    /// (`dt < 0.0` is a program error — panics rather than traveling
    /// backward).
    Advance(f64),
    /// `t.wait(edge(net, direction))`: resume the first time `net` settles
    /// through the watched transition (T23) — never a time breakpoint,
    /// dispatched from [`HostElement::on_digital_settled`] instead of
    /// `next_breakpoints`.
    WaitEdge(DigitalNet, EdgeDirection),
    /// `t.wait(above(net, level))`: resume once an accepted point brackets
    /// `net` crossing `level` in `direction` (T24) — tier 2 (predict +
    /// bound, `piperine_signal::events`), never a time breakpoint. Read
    /// `SequencerControl::last_crossing` from `resume` for the bound.
    WaitCrossing(AnalogReference, f64, CrossDir),
    /// The coroutine has nothing further to do.
    Done,
}

/// A hand-written coroutine body (`ideal.md` §6): Rust has no stable
/// generators, so a `Sequencer`'s program is an explicit state machine
/// rather than a function with `yield` in it. `start`/`resume` return the
/// *next* wake — the driver ([`Sequencer`]) turns that into a breakpoint
/// and calls back in exactly once that instant is accepted.
pub trait SequencerProgram: Send + Sync {
    /// The first wake, before any step has run.
    fn start(&mut self, ctl: &mut SequencerControl) -> Wake;
    /// Called once, exactly when the wake most recently returned lands on
    /// an accepted step, at the (possibly clamped) time it landed.
    fn resume(&mut self, t: f64, ctl: &mut SequencerControl) -> Wake;
    /// Notified once if a pending wake would land past the run's stop
    /// time: the run ends here, and no further resume happens.
    fn on_run_end(&mut self, _t: f64, _ctl: &mut SequencerControl) {}
}

/// Advance(0) is legal (the coroutine may want to re-check something at
/// the same instant) but bounded: this many consecutive zero-advance
/// resumes at one instant is treated as a program bug, never a hang.
const MAX_ZERO_ADVANCE_RESUMES: usize = 1_000;

/// The coroutine driver: a [`HostElement`] whose `next_breakpoints` is
/// exactly its program's pending time wake, resumed once per accepted
/// landing (T19).
pub struct Sequencer<P: SequencerProgram> {
    label: String,
    program: P,
    stop_time: f64,
    pending: Option<f64>,
    ended: bool,
    zero_advance_at: f64,
    zero_advance_count: usize,
    control: SequencerControl,
    /// How much of `control.findings` has already been handed to the
    /// solver's poll (T22) — `validation_reports` reports only the delta
    /// since the last poll, never the whole accumulated history again,
    /// since the solver's own `consume_validation_reports` appends every
    /// non-`None` report it reads into the run's accumulated record.
    reported_up_to: usize,
    pending_report: Option<piperine_solver::abi::ValidationReport>,
    /// The one outstanding digital wait, if the coroutine's pending wake
    /// is `Wake::WaitEdge` (T23) — mutually exclusive with `pending` (a
    /// coroutine blocks on exactly one wake source at a time).
    edge_wait: Option<EdgeWait>,
    /// The one outstanding analog crossing wait (T24) — mutually
    /// exclusive with `pending`/`edge_wait` for the same reason.
    crossing_wait: Option<CrossingWait>,
    /// The most recent instant any wake-source hook observed — used only
    /// to report a still-pending wait honestly if the run ends first
    /// (`destroy`, T24's "a crossing that never occurs ends the run
    /// cleanly").
    last_t: f64,
}

impl<P: SequencerProgram> Sequencer<P> {
    /// Build a sequencer whose coroutine starts running from `start_time`
    /// (`program.start()` is called immediately) and never wakes past
    /// `stop_time`.
    pub fn new(label: impl Into<String>, mut program: P, start_time: f64, stop_time: f64) -> Self {
        let mut control = SequencerControl::new();
        let wake = program.start(&mut control);
        let mut me = Self {
            label: label.into(),
            program,
            stop_time,
            pending: None,
            ended: false,
            zero_advance_at: f64::NAN,
            zero_advance_count: 0,
            control,
            reported_up_to: 0,
            pending_report: None,
            edge_wait: None,
            crossing_wait: None,
            last_t: start_time,
        };
        me.apply_wake(start_time, wake);
        me.refresh_pending_report();
        me
    }

    /// Recompute the delta report from whatever `control.findings` gained
    /// since the last poll — called at the end of every `on_digital_settled`
    /// (T22/T23), the last hook the solver calls per accepted step, so a
    /// poll immediately following sees exactly this step's new findings,
    /// never a repeat of an earlier step's.
    fn refresh_pending_report(&mut self) {
        let all = self.control.findings.as_slice();
        self.pending_report = if self.reported_up_to < all.len() {
            let fresh = piperine_solver::abi::ValidationReport::new(all[self.reported_up_to..].to_vec());
            self.reported_up_to = all.len();
            Some(fresh)
        } else {
            None
        };
    }

    /// Extract a human-readable message from a caught coroutine panic
    /// payload (T22: "a coroutine exception surfaces as a test failure
    /// carrying the simulation time").
    fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "sequencer coroutine panicked with a non-string payload".to_string()
        }
    }

    /// The pending time wake's absolute instant, if any.
    pub fn pending_breakpoint(&self) -> Option<f64> {
        self.pending
    }

    /// True once the coroutine returned `Wake::Done` or the run ended —
    /// false while any wake source (time, digital edge, or analog
    /// crossing) is outstanding.
    pub fn finished(&self) -> bool {
        self.pending.is_none() && self.edge_wait.is_none() && self.crossing_wait.is_none()
    }

    /// Findings recorded so far (T21's internal-net-drive notices, T22's
    /// `expect`/`warn`) — the same buffer [`HostElement::validation_reports`]
    /// (wired below) drains through `dv-core`'s channel.
    pub fn findings(&self) -> &super::Findings {
        &self.control.findings
    }

    /// The most recent instant any accept-hook observed — diagnostic only
    /// (e.g. a still-outstanding `wait` at the end of a run, T24).
    pub fn last_observed_time(&self) -> f64 {
        self.last_t
    }

    /// Resume the coroutine right now at `t` (the wake source, whichever
    /// it was, is already satisfied by the caller) — the one call site
    /// [`HostElement::accept_timestep`] and
    /// [`HostElement::on_digital_settled`] both funnel through, including
    /// the T22 panic-catch/finding conversion.
    fn resume_now(&mut self, t: f64) {
        // T22: a coroutine exception must not unwind straight through the
        // solver's call stack — caught here, turned into an Error finding
        // carrying the simulation time, and the sequencer stops (no
        // further resume): the analysis itself then stops cleanly through
        // the ordinary Strict-posture Err() path once this finding is
        // polled.
        let result = {
            let program = &mut self.program;
            let control = &mut self.control;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| program.resume(t, control)))
        };
        match result {
            Ok(wake) => self.apply_wake(t, wake),
            Err(payload) => {
                let msg = Self::panic_message(&*payload);
                let label = self.label.clone();
                self.control.findings.push(
                    piperine_solver::abi::ValidationSeverity::Error,
                    "coroutine_exception",
                    format!("sequencer coroutine panicked at t={t:.6e}: {msg}"),
                    0.0,
                    t,
                    label,
                );
                // Stop the sequencer: nothing further pending.
                self.pending = None;
                self.edge_wait = None;
                self.crossing_wait = None;
            }
        }
    }

    fn apply_wake(&mut self, from: f64, wake: Wake) {
        // Exactly one wake source is ever outstanding — clear the other
        // two before setting whichever this wake asks for.
        self.pending = None;
        self.edge_wait = None;
        self.crossing_wait = None;
        match wake {
            Wake::Advance(dt) => {
                assert!(
                    dt >= 0.0,
                    "Sequencer '{}': advance(dt) requires dt >= 0, got {dt}",
                    self.label
                );
                let t = from + dt;
                if t > self.stop_time {
                    if !self.ended {
                        self.ended = true;
                        self.program.on_run_end(self.stop_time, &mut self.control);
                    }
                    return;
                }
                if dt == 0.0 && (t - self.zero_advance_at).abs() < 1e-15 {
                    self.zero_advance_count += 1;
                } else {
                    self.zero_advance_at = t;
                    self.zero_advance_count = usize::from(dt == 0.0);
                }
                if self.zero_advance_count > MAX_ZERO_ADVANCE_RESUMES {
                    panic!(
                        "Sequencer '{}': {} consecutive zero-advance resumes at t={:.3e} — a \
                         coroutine that never advances simulated time is a bug, not a valid \
                         program (advance(0) is legal but bounded)",
                        self.label, self.zero_advance_count, t
                    );
                }
                self.pending = Some(t);
            }
            Wake::WaitEdge(net, direction) => {
                self.zero_advance_count = 0;
                self.edge_wait = Some(EdgeWait::new(net, direction));
            }
            Wake::WaitCrossing(net, level, direction) => {
                self.zero_advance_count = 0;
                self.crossing_wait = Some(CrossingWait::new(net, level, direction));
            }
            Wake::Done => {
                self.zero_advance_count = 0;
            }
        }
    }
}

impl<P: SequencerProgram> HostElement for Sequencer<P> {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        // EMITS_VALIDATION (T22, DVT-29): expect/warn record into `control.
        // findings`, and only elements declaring this flag are ever polled
        // for `validation_reports` — see `HostElement::validation_reports`.
        ElementCapabilities::EMITS_VALIDATION
    }

    fn limiting_report(&self) -> Option<LimitingReport> {
        None
    }

    fn next_breakpoints(&self, from: f64, _horizon: f64) -> Vec<f64> {
        match self.pending {
            Some(t) if t > from => vec![t],
            _ => Vec::new(),
        }
    }

    fn accept_timestep(&mut self, state: &StateView<'_>, t: f64) {
        // Note: `refresh_pending_report` is NOT called here — it runs once
        // at the end of `on_digital_settled`, the last hook the solver
        // calls per accepted step (T23), so a resume triggered by any
        // wake source is captured by exactly one poll-facing refresh,
        // never two competing ones.
        self.last_t = t;
        if let Some(pending) = self.pending {
            let tol = 1e-9 * pending.abs().max(f64::MIN_POSITIVE);
            if (t - pending).abs() <= tol {
                self.pending = None; // consumed before resume: a panic inside resume leaves no stale pending
                self.resume_now(pending);
                return;
            }
        }
        // T24: the analog half of `wait` — fed from this accept hook
        // (StateView already carries the accepted solution), unlike the
        // digital edge half which needs the post-settle value.
        if let Some(crossing_wait) = &mut self.crossing_wait {
            let v = state.v(&crossing_wait.net);
            if let Some(pred) = crossing_wait.observe(t, v) {
                self.crossing_wait = None;
                self.control.set_last_crossing(pred);
                self.resume_now(t);
            }
        }
    }

    fn drain_digital_drives(&mut self) -> Vec<(DigitalNet, LogicValue, f64)> {
        self.control.take_digital_drives()
    }

    fn validation_reports(&self) -> Option<piperine_solver::abi::ValidationReport> {
        self.pending_report.clone()
    }

    fn on_digital_settled(&mut self, nets: &[LogicValue], t: f64) {
        // T23/T24: the digital half of `wait` — never a time breakpoint,
        // so unrelated to `self.pending`. Consulted strictly after the
        // solver's own settle dispatch (never on a rejected step, same
        // call chain as `accept_timestep`).
        if let Some(edge_wait) = &mut self.edge_wait
            && edge_wait.observe(nets)
        {
            self.edge_wait = None;
            self.resume_now(t);
        }
        self.refresh_pending_report();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program that always advances by a fixed `dt`, `n` times, then
    /// finishes — the simplest legal coroutine.
    struct FixedAdvances {
        dt: f64,
        remaining: usize,
    }
    impl SequencerProgram for FixedAdvances {
        fn start(&mut self, ctl: &mut SequencerControl) -> Wake {
            self.resume(0.0, ctl)
        }
        fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
            if self.remaining == 0 {
                Wake::Done
            } else {
                self.remaining -= 1;
                Wake::Advance(self.dt)
            }
        }
    }

    #[test]
    fn next_breakpoints_reports_the_pending_wake_then_nothing_after_done() {
        let seq = Sequencer::new("s", FixedAdvances { dt: 1e-6, remaining: 1 }, 0.0, 10e-6);
        assert_eq!(seq.next_breakpoints(0.0, 1e-6), vec![1e-6]);
        assert!(!seq.finished());
    }

    #[test]
    fn advance_negative_dt_panics() {
        struct Bad;
        impl SequencerProgram for Bad {
            fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
                Wake::Advance(-1.0)
            }
            fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
                Wake::Done
            }
        }
        let result = std::panic::catch_unwind(|| Sequencer::new("bad", Bad, 0.0, 1.0));
        assert!(result.is_err(), "a negative advance(dt) must panic, not silently clamp");
    }

    #[test]
    fn advance_past_stop_time_clamps_and_notifies_run_end() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        struct OverrunsStop {
            notified: Arc<AtomicBool>,
            notified_at: Arc<std::sync::Mutex<f64>>,
        }
        impl SequencerProgram for OverrunsStop {
            fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
                Wake::Advance(100e-6) // stop_time is 10e-6 below
            }
            fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
                panic!("must never resume: the wake never lands before stop_time")
            }
            fn on_run_end(&mut self, t: f64, _ctl: &mut SequencerControl) {
                self.notified.store(true, Ordering::SeqCst);
                *self.notified_at.lock().unwrap() = t;
            }
        }

        let notified = Arc::new(AtomicBool::new(false));
        let notified_at = Arc::new(std::sync::Mutex::new(0.0));
        let seq = Sequencer::new(
            "overrun",
            OverrunsStop { notified: notified.clone(), notified_at: notified_at.clone() },
            0.0,
            10e-6,
        );
        assert!(notified.load(Ordering::SeqCst), "on_run_end must fire when advance overruns stop_time");
        assert_eq!(*notified_at.lock().unwrap(), 10e-6, "the run ended at the clamped stop time");
        assert!(seq.finished(), "a clamped-past-stop wake leaves nothing pending");
        assert!(seq.next_breakpoints(0.0, 1e-3).is_empty());
    }

    #[test]
    fn a_non_yielding_coroutine_is_loud_not_a_hang() {
        // A program that keeps asking for the same instant forever: every
        // `resume` returns `Advance(0.0)` again. Bounded by
        // MAX_ZERO_ADVANCE_RESUMES, then a panic — proving this cannot
        // silently spin the accept-hook forever.
        struct NeverAdvances;
        impl SequencerProgram for NeverAdvances {
            fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
                Wake::Advance(0.0)
            }
            fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
                Wake::Advance(0.0)
            }
        }
        let mut seq = Sequencer::new("stuck", NeverAdvances, 0.0, 1.0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let history = piperine_solver::abi::CircularArrayBuffer2::<f64>::new(1, 1);
            for _ in 0..(MAX_ZERO_ADVANCE_RESUMES + 5) {
                let view = StateView::new(&history);
                seq.accept_timestep(&view, 0.0);
            }
        }));
        assert!(result.is_err(), "a coroutine stuck at advance(0) forever must eventually panic loudly");
    }

    #[test]
    fn sim_hooks_is_absent_from_this_module() {
        // Structural check (the task's "assert its absence" bullet): no
        // non-doc-comment line in this module's own source imports or
        // references the `SimHooks` item — the resume path is driven
        // entirely by `HostElement`/`Element`. Doc-comment lines (`//!`,
        // `///`) are excluded since the header *discusses* why `SimHooks`
        // is irrelevant here, which itself names it in prose.
        let src = include_str!("driver.rs");
        // Only the production code above the test module — this test's own
        // source text necessarily mentions `SimHooks` as a plain string.
        let production = src.split("#[cfg(test)]").next().unwrap();
        let offending: Vec<&str> = production
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//!") && !trimmed.starts_with("///") && line.contains("SimHooks")
            })
            .collect();
        assert!(offending.is_empty(), "code (non-doc) lines must never reference SimHooks: {offending:?}");
    }
}
