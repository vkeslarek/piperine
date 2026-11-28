//! One instant: the microstep loop (abi.md §5, §6; `docs/spec/part_v_execution.md`
//! §15). Each microstep applies the commits due at it (C2 order), runs the
//! functions to a fixed point (D), runs the reactions of the events that fired
//! (R) and goes on while a commit to a non-`QUIET` net is due at the next
//! microstep, up to the session's bound.

use std::collections::BTreeSet;

use crate::engine::{Digital, Horizon, Mode, Outcome};
use crate::landing::{LandingCause, LandingTest};
use crate::net::{NetId, NetKind};
use crate::passing::PassingValues;
use crate::queue::{Instant, Writer};
use crate::run::{Ambient, Firing, FunctionRun, Phase, ReactionRun};
use crate::tick::Tick;
use crate::error::DigitalError;

/// How a pass over an instant ended.
pub(crate) enum Stepped {
    Done(Outcome),
    /// A look-ahead stopped: the instant is a landing, for `cause`.
    Landing { cause: LandingCause },
}

/// What the settle phase of an instant reported (`Digital::settle_phase`).
struct SettlePhase {
    /// A reaction of the phase returned `FINISH`.
    finish: bool,
    /// The nets that fired in the phase.
    fired: BTreeSet<NetId>,
    /// The analog-read nets the phase's functions changed.
    boundary: BTreeSet<NetId>,
}

/// One discrete net as the commits due at a microstep left it.
struct Settled {
    net: NetId,
    /// The element of an array net the commits are to.
    element: Option<u64>,
    /// The whole net: its value before the microstep, each commit's bytes laid over it in order.
    bytes: Vec<u8>,
    /// A commit came from the host.
    host: bool,
    /// The settled bytes differ from the value before the microstep.
    changed: bool,
}

/// What the microsteps of an instant reached so far.
#[derive(Default)]
struct Reached {
    /// The nets the analog side reads that changed.
    boundary: BTreeSet<NetId>,
    /// The nets whose event fired.
    fired: BTreeSet<NetId>,
}

impl Digital {
    /// The discrete functions of the engine as a run over its plan, store, dirty set and observations.
    fn function_run(&mut self) -> FunctionRun<'_> {
        FunctionRun::new(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs)
    }

    /// The reactions of the engine's microstep `here`, reading `samples` and `ambient`.
    fn reaction_run<'a>(&'a mut self, samples: &'a [f64], ambient: &'a Ambient, here: Instant) -> ReactionRun<'a> {
        ReactionRun {
            plan: &mut self.plan,
            store: &self.store,
            samples,
            ambient,
            at: here,
            queue: &mut self.queue,
            observer: &mut self.obs,
            call: &mut self.call,
            settle_commits: &mut self.settle_commits,
        }
    }

    /// Applies the commits due at `here`, in C2 order. A commit to an event net fires it. The
    /// commits to one discrete net settle over its value, the later one winning each byte, and
    /// the net is written once (§12.1.1, §12.1.2): `CHANGED`, `PREVIOUS` and the one `Change`
    /// observed compare the settled value with the value before the microstep (§8.2.3, §12.1.18).
    /// Returns each written net, in the order of its first commit.
    fn apply_due(&mut self, here: Instant) -> Vec<Settled> {
        let mut settled: Vec<Settled> = Vec::new();
        for p in self.queue.take_due(here) {
            if self.plan.nets[p.net.0 as usize].kind == NetKind::Event {
                self.function_run().fire_net(p.net, here);
                continue;
            }
            let at = match settled.iter().position(|s| s.net == p.net && s.element == p.element) {
                Some(i) => i,
                None => {
                    let bytes = match p.element {
                        Some(element) => self.store.element_value(p.net, element).to_vec(),
                        None => self.store.value(p.net).to_vec(),
                    };
                    settled.push(Settled { net: p.net, element: p.element, bytes, host: false, changed: false });
                    settled.len() - 1
                }
            };
            let offset = p.offset as usize;
            let s = &mut settled[at];
            s.bytes[offset..offset + p.bytes.len()].copy_from_slice(&p.bytes);
            s.host |= p.writer == Writer::Host;
        }
        for s in &mut settled {
            s.changed = match s.element {
                Some(element) => self.function_run().write_element(s.net, element, 0, &s.bytes, here),
                None => self.function_run().write_net(s.net, 0, &s.bytes, here),
            };
        }
        settled
    }

    /// `HORIZON`: the last tick before the earliest pending commit and before `limit`, never
    /// before the instant itself (abi.md §5 R5).
    pub(crate) fn horizon(&self, at: Tick, limit: Tick) -> Tick {
        let next = self.queue.next_due().map_or(limit, |(t, _)| t.min(limit));
        Tick(next.0.saturating_sub(1)).max(at)
    }

    /// Runs the instant `at`: the commits due at (at, 0), then microsteps until quiet. `samples`
    /// are the accepted continuous values reactions read; `limit` bounds each reaction's `HORIZON`:
    /// `limit.crossing` for one a `CROSS` watch can reach, `limit.stop` for the rest.
    pub fn run_instant(
        &mut self,
        at: Tick,
        samples: &[f64],
        limit: Horizon,
        mode: Mode,
    ) -> Result<Outcome, DigitalError> {
        match self.microsteps(at, samples, limit, mode, false)? {
            Stepped::Done(outcome) => Ok(outcome),
            Stepped::Landing { .. } => unreachable!("only a look-ahead stops at a landing"),
        }
    }

    /// The loop of `run_instant`. In a look-ahead the time-point test (abi.md §5 R4) runs after
    /// each microstep's functions and before its reactions: the instant stops, half done, when an
    /// analog-read net changed, or a reaction about to run touches the analog side or is
    /// `IRREVERSIBLE`; the caller restores its checkpoint.
    pub(crate) fn microsteps(
        &mut self,
        at: Tick,
        samples: &[f64],
        limit: Horizon,
        mode: Mode,
        lookahead: bool,
    ) -> Result<Stepped, DigitalError> {
        self.now = at;
        let mut out = Outcome::default();
        let mut reached = Reached::default();
        // a transient instant run for the owner records the values its analog-read nets pass through
        let tracks_passing = !lookahead && !mode.is_static;
        let mut n = self.resume_microstep(at, lookahead);
        loop {
            let here = (at, n);
            let ambient = self.begin_microstep(here, limit, mode, &mut out);
            self.function_run().run(here, &ambient)?;
            let analog_now = self.scan_nets(tracks_passing, &mut reached);
            if let Some(cause) = self.landing_after_functions(lookahead, analog_now) {
                return Ok(Stepped::Landing { cause });
            }
            let ran = self.reaction_run(samples, &ambient, here).run(Phase::Microstep)?;
            out.finish |= ran.finish;
            self.obs.flush();
            out.microsteps = n + 1;

            // A host waiter's event fired: suspend until the host has answered (abi.md §5 R7).
            let host_events: Vec<NetId> = self.waited.iter().copied().filter(|w| self.store.fired(*w)).collect();
            if !lookahead && !host_events.is_empty() {
                return Ok(Stepped::Done(self.suspended_for_host((at, n + 1), host_events, reached, out)));
            }

            // the next microstep runs when a commit to a net that is not QUIET is due at it
            let next = (at, n + 1);
            if !self.live_at(next) {
                // what is left for this instant applies as it exits
                self.apply_due(next);
                self.obs.flush();
                break;
            }
            if (n + 1) as usize >= self.limits.max_delta_cycles {
                return Err(self.not_quiet(at, next));
            }
            n += 1;
        }

        self.last_run = if lookahead { self.last_run } else { Some((at, n)) };
        // what the whole instant passed through, every part of it: a value passed through is read
        // by the owner after the instant, and the settle phase follows that reading
        out.passing = self.passing.read_state(&self.store, &self.plan.nets);
        if mode.settle && self.passing.is_empty() {
            let settled = self.settle_phase(at, n, samples, limit, mode)?;
            out.finish |= settled.finish;
            reached.fired.extend(settled.fired);
            reached.boundary.extend(settled.boundary);
        }
        Ok(self.report(reached, out))
    }

    /// Opens the microstep `here`: the commits due at it apply (a write to a host param is noted in `out`), the
    /// reaction counter starts again, and the ambient values (the horizons bounded by `limit`) are read.
    fn begin_microstep(&mut self, here: (Tick, u32), limit: Horizon, mode: Mode, out: &mut Outcome) -> Ambient {
        let at = here.0;
        self.store.begin_microstep();
        self.call = 0;
        for s in self.apply_due(here) {
            out.host_param_write |= s.changed && s.host && self.plan.analysis().is_param_net(s.net);
        }
        let mut ambient = self.ambient(at);
        ambient.is_static = mode.is_static;
        ambient.horizon = self.horizon(at, limit.stop);
        ambient.crossing_horizon = self.horizon(at, limit.crossing);
        ambient
    }

    /// Whether a commit to a net that is not QUIET is due at `next`: the instant runs another microstep.
    fn live_at(&self, next: (Tick, u32)) -> bool {
        self.queue.due_at(next).any(|p| !self.plan.nets[p.net.0 as usize].is_quiet())
    }

    /// The instant suspended for the host's waiters (abi.md §5 R7): it resumes at the microstep `resume`, and what it
    /// reached so far is reported with the `host_events` that fired.
    fn suspended_for_host(&mut self, resume: (Tick, u32), host_events: Vec<NetId>, reached: Reached, mut out: Outcome) -> Outcome {
        self.suspended = Some(resume);
        out.boundary = self.analog_values(reached.boundary);
        out.fired = reached.fired.into_iter().collect();
        out.host_events = host_events;
        out.passing = self.passing.read_state(&self.store, &self.plan.nets);
        out
    }

    /// Each net of `boundary` with the value the analog side reads.
    fn analog_values(&self, boundary: BTreeSet<NetId>) -> Vec<(NetId, Vec<u8>)> {
        boundary.into_iter().map(|n| (n, self.store.analog_value(n, &self.plan.nets[n.0 as usize]))).collect()
    }

    /// The outcome of an instant that is done: the nets it reached, as the analog side reads them, and the settle commits.
    fn report(&self, reached: Reached, mut out: Outcome) -> Stepped {
        out.boundary = self.analog_values(reached.boundary);
        out.fired = reached.fired.into_iter().collect();
        out.settle_boundary = self.settle_commits.boundary(&self.plan.nets, &self.store);
        Stepped::Done(out)
    }

    /// In a look-ahead, the landing the time-point test (abi.md §5 R4) finds after a microstep's functions, if any.
    fn landing_after_functions(&self, lookahead: bool, analog_now: bool) -> Option<LandingCause> {
        if !lookahead {
            return None;
        }
        let firing = Firing::fired(&self.plan.instances, Phase::Microstep, &self.store);
        LandingTest::new(self.rule, &self.plan, &self.waited).after_functions(&self.store, &firing, analog_now)
    }

    /// The microstep an instant at `at` starts at. An instant suspended for the host's waiters (or reopened) continues
    /// where it stopped, with what its nets passed through so far; a new instant starts the record again.
    fn resume_microstep(&mut self, at: Tick, lookahead: bool) -> u32 {
        let resumed = match self.suspended.take() {
            Some((tick, microstep)) if tick == at && !lookahead => Some(microstep),
            _ => None,
        };
        if resumed.is_none() {
            self.passing = PassingValues::default();
        }
        resumed.unwrap_or(0)
    }

    /// Reads the nets after a microstep's functions: the ones that fired join `fired`, the analog-read ones that changed
    /// join `boundary` (and, when `tracks_passing`, the values they pass through). Whether an analog-read net changed.
    fn scan_nets(&mut self, tracks_passing: bool, reached: &mut Reached) -> bool {
        let mut analog_now = false;
        for (i, spec) in self.plan.nets.iter().enumerate() {
            let id = NetId(i as u32);
            if self.store.fired(id) {
                reached.fired.insert(id);
            }
            if spec.analog_reads && self.store.changed(id) {
                reached.boundary.insert(id);
                analog_now = true;
                // an array net is recorded as the analog side reads it, its windows (`Store::value` holds no array)
                if tracks_passing && self.store.analog_changed(id, spec) {
                    self.passing.changed(id, &self.store.analog_value(id, spec));
                }
            }
        }
        analog_now
    }

    /// The `E0501` of an instant at `at` that is still not quiet at the microstep `next`: the nets due there.
    fn not_quiet(&self, at: Tick, next: (Tick, u32)) -> DigitalError {
        let mut nets: Vec<String> = self
            .queue
            .due_at(next)
            .filter(|p| !self.plan.nets[p.net.0 as usize].is_quiet())
            .map(|p| self.plan.nets[p.net.0 as usize].name.clone())
            .collect();
        nets.sort();
        nets.dedup();
        DigitalError::NotQuiet { code: "E0501", tick: at.0, nets }
    }

    /// The settle phase of the instant `at` (§15 step 4, §19 rule 8), run after the microstep loop
    /// went quiet at microstep `last`: fire the `SETTLE` nets, run the discrete functions of their
    /// cone (§13 rule 1: a guard reads in the microstep, so a guarded `settle` fires here), then
    /// the reactions whose event pins fired. Every reaction that runs is `on_settle` whatever its
    /// head names (§13 rule 14), and the phase's records are stamped `(at, last)`, the last
    /// microstep the instant ran (§19 Resolution).
    fn settle_phase(&mut self, at: Tick, last: u32, samples: &[f64], limit: Horizon, mode: Mode) -> Result<SettlePhase, DigitalError> {
        let here = (at, last);
        self.store.begin_microstep();
        self.call = 0;
        let mut phase = SettlePhase { finish: false, fired: BTreeSet::new(), boundary: BTreeSet::new() };
        for net in self.plan.analysis().settle_nets().to_vec() {
            self.function_run().fire_net(net, here);
        }
        let mut ambient = self.ambient(at);
        ambient.is_static = mode.is_static;
        ambient.horizon = self.horizon(at, limit.stop);
        ambient.crossing_horizon = self.horizon(at, limit.crossing);
        self.function_run().run(here, &ambient)?;
        for (i, spec) in self.plan.nets.iter().enumerate() {
            let id = NetId(i as u32);
            if self.store.fired(id) {
                phase.fired.insert(id);
            }
            if spec.analog_reads && self.store.changed(id) {
                phase.boundary.insert(id);
            }
        }
        let ran = self.reaction_run(samples, &ambient, here).run(Phase::Settle)?;
        phase.finish = ran.finish;
        // what the phase left for the instant, its records, applies as the instant exits, as at the
        // end of the loop: a record left queued would be a commit due at this tick forever
        self.apply_due((at, last + 1));
        self.obs.flush();
        Ok(phase)
    }
}
