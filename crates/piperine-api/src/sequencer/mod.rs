//! The sequencer coroutine driver (T19, DVT-25/26): [`Sequencer<P>`] is a
//! [`HostElement`](crate::device::HostElement) whose `next_breakpoints`
//! comes from a pending wake advanced by a hand-written state machine
//! (a [`SequencerProgram`]) — Rust has no stable generators, so `resume` is
//! an explicit state machine rather than a language-level coroutine
//! (`ideal.md` §6).
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
//! the existing ABI ordering. **No new solver hook is added by this
//! task** (T23 is the one task that touches the solver, for a different
//! seam: the digital subscriber).
//!
//! `SimHooks` (`crate::hooks::SimHooks`) is not on this path: this module
//! never imports or references it — the sequencer's resume is driven
//! entirely through `HostElement`/`Element`, not the session lifecycle
//! hooks `Session` wires around a build/solve.

pub mod drive;
pub mod findings;
pub mod install;

use piperine_solver::abi::{DigitalNet, ElementCapabilities, LimitingReport, LogicValue};

use crate::device::host_element::HostElement;
use crate::device::stamp::StateView;

pub use findings::Findings;

/// The mutable handle a [`SequencerProgram`] drives side effects through —
/// the Rust mirror of `t` in `ideal.md`'s `def run(self, t):` examples,
/// minus the drive/wait verbs themselves (those are free functions in
/// [`drive`] taking `&mut SequencerControl` explicitly, since a program's
/// analog drive sites and install sites are its own struct fields, not
/// something `Sequencer` owns on its behalf).
#[derive(Default)]
pub struct SequencerControl {
    digital_drives: Vec<(DigitalNet, LogicValue, f64)>,
    findings: Findings,
}

impl SequencerControl {
    fn new() -> Self {
        Self::default()
    }

    /// Queue a digital drive: forwarded to the real `EventSink` once this
    /// accept's `accept_timestep` returns (never touches the MNA — T21,
    /// DVT-33).
    pub fn drive_digital(&mut self, net: DigitalNet, value: LogicValue, delay: f64) {
        self.digital_drives.push((net, value, delay));
    }

    /// The findings buffer `expect`/`warn` (T22) and internal-net drive
    /// notices (T21) record into.
    pub fn findings(&mut self) -> &mut Findings {
        &mut self.findings
    }

    fn take_digital_drives(&mut self) -> Vec<(DigitalNet, LogicValue, f64)> {
        std::mem::take(&mut self.digital_drives)
    }
}

/// What a [`Sequencer`] is waiting for. `t.advance(dt)` is the only form
/// this task adds; `Edge`/`Crossing` wake sources arrive in T23/T24
/// (`ideal.md` §4.1's "one `wait`, three wake sources" table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wake {
    /// Wake at absolute time `t + dt` from the instant this was returned
    /// (`dt < 0.0` is a program error — panics rather than traveling
    /// backward).
    Advance(f64),
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
        };
        me.apply_wake(start_time, wake);
        me
    }

    /// The pending time wake's absolute instant, if any.
    pub fn pending_breakpoint(&self) -> Option<f64> {
        self.pending
    }

    /// True once the coroutine returned `Wake::Done` or the run ended.
    pub fn finished(&self) -> bool {
        self.pending.is_none()
    }

    /// Findings recorded so far (T21's internal-net-drive notices, T22's
    /// `expect`/`warn`) — the same buffer [`HostElement::validation_reports`]
    /// (wired in T22) drains through `dv-core`'s channel.
    pub fn findings(&self) -> &Findings {
        &self.control.findings
    }

    fn apply_wake(&mut self, from: f64, wake: Wake) {
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
                    self.pending = None;
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
            Wake::Done => {
                self.zero_advance_count = 0;
                self.pending = None;
            }
        }
    }
}

impl<P: SequencerProgram> HostElement for Sequencer<P> {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::empty()
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

    fn accept_timestep(&mut self, _state: &StateView<'_>, t: f64) {
        if let Some(pending) = self.pending {
            let tol = 1e-9 * pending.abs().max(f64::MIN_POSITIVE);
            if (t - pending).abs() <= tol {
                self.pending = None; // consumed before resume: a panic inside resume leaves no stale pending
                let wake = self.program.resume(pending, &mut self.control);
                self.apply_wake(pending, wake);
            }
        }
    }

    fn drain_digital_drives(&mut self) -> Vec<(DigitalNet, LogicValue, f64)> {
        self.control.take_digital_drives()
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
        let src = include_str!("mod.rs");
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
