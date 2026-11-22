//! One instant: the microstep loop (abi.md §5, §6; `docs/spec/part_v_execution.md`
//! §15). Each microstep applies the commits due at it (C2 order), runs the
//! functions to a fixed point (D), runs the reactions of the events that fired
//! (R) and goes on while a commit to a non-`QUIET` net is due at the next
//! microstep, up to the session's bound.

use std::collections::BTreeSet;

use crate::engine::{Digital, Horizon, Mode, Outcome};
use crate::net::{NetId, NetKind};
use crate::queue::{Instant, Writer};
use crate::run::{Phase, fire_net, fired_reactions, run_functions, run_reactions, write_net};
use piperine_abi::raw::reaction_flag;
use crate::tick::Tick;
use crate::error::DigitalError;

/// How a pass over an instant ended.
pub(crate) enum Stepped {
    Done(Outcome),
    /// A look-ahead stopped: the instant is a landing; `time_point` is false when only an
    /// `IRREVERSIBLE` reaction made it one.
    Landing { time_point: bool },
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
    /// The whole net: its value before the microstep, each commit's bytes laid over it in order.
    bytes: Vec<u8>,
    /// A commit came from the host.
    host: bool,
    /// The settled bytes differ from the value before the microstep.
    changed: bool,
}

impl Digital {
    /// Applies the commits due at `here`, in C2 order. A commit to an event net fires it. The
    /// commits to one discrete net settle over its value, the later one winning each byte, and
    /// the net is written once (§12.1.1, §12.1.2): `CHANGED`, `PREVIOUS` and the one `Change`
    /// observed compare the settled value with the value before the microstep (§8.2.3, §12.1.18).
    /// Returns each written net, in the order of its first commit.
    fn apply_due(&mut self, here: Instant) -> Vec<Settled> {
        let mut settled: Vec<Settled> = Vec::new();
        for p in self.queue.take_due(here) {
            if self.plan.nets[p.net.0 as usize].kind == NetKind::Event {
                fire_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, p.net, here);
                continue;
            }
            let at = match settled.iter().position(|s| s.net == p.net) {
                Some(i) => i,
                None => {
                    let bytes = self.store.value(p.net).to_vec();
                    settled.push(Settled { net: p.net, bytes, host: false, changed: false });
                    settled.len() - 1
                }
            };
            let offset = p.offset as usize;
            let s = &mut settled[at];
            s.bytes[offset..offset + p.bytes.len()].copy_from_slice(&p.bytes);
            s.host |= p.writer == Writer::Host;
        }
        for s in &mut settled {
            s.changed = write_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, s.net, 0, &s.bytes, here);
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
        let mut boundary: BTreeSet<NetId> = BTreeSet::new();
        let mut fired: BTreeSet<NetId> = BTreeSet::new();
        // An instant suspended for the host's waiters continues where it stopped.
        let mut n = match self.suspended.take() {
            Some((tick, microstep)) if tick == at && !lookahead => microstep,
            _ => 0,
        };
        loop {
            let here = (at, n);
            self.store.begin_microstep();
            self.call = 0;
            for s in self.apply_due(here) {
                out.host_param_write |= s.changed && s.host && self.plan.analysis().is_param_net(s.net);
            }
            let mut ambient = self.ambient(at);
            ambient.is_static = mode.is_static;
            ambient.horizon = self.horizon(at, limit.stop);
            ambient.crossing_horizon = self.horizon(at, limit.crossing);
            run_functions(&self.plan, &mut self.store, &mut self.dirty, here, &ambient, &mut self.obs)?;
            let mut analog_now = false;
            for (i, spec) in self.plan.nets.iter().enumerate() {
                let id = NetId(i as u32);
                if self.store.fired(id) {
                    fired.insert(id);
                }
                if spec.analog_reads && self.store.changed(id) {
                    boundary.insert(id);
                    analog_now = true;
                }
            }
            if lookahead {
                let firing = fired_reactions(&self.plan.instances, Phase::Microstep, &self.store);
                let probing = firing.iter().any(|f| self.plan.analysis().probes(f.instance as u32, f.reaction as u32));
                let irreversible = firing.iter().any(|f| {
                    let shape = self.plan.instances[f.instance].handle.shape();
                    shape.reactions.get(f.reaction).is_some_and(|r| r.flags & reaction_flag::IRREVERSIBLE != 0)
                });
                if analog_now || probing {
                    return Ok(Stepped::Landing { time_point: true });
                }
                let waited = self.waited.iter().any(|w| self.store.fired(*w));
                if irreversible || waited {
                    return Ok(Stepped::Landing { time_point: false });
                }
            }
            let ran = run_reactions(
                &mut self.plan,
                &self.store,
                samples,
                &ambient,
                &mut self.queue,
                &mut self.obs,
                here,
                &mut self.call,
                &mut self.settle_commits,
                Phase::Microstep,
            )?;
            out.finish |= ran.finish;
            self.obs.flush();
            out.microsteps = n + 1;

            // A host waiter's event fired: suspend until the host has answered (abi.md §5 R7).
            let host_events: Vec<NetId> = self.waited.iter().copied().filter(|w| self.store.fired(*w)).collect();
            if !lookahead && !host_events.is_empty() {
                self.suspended = Some((at, n + 1));
                out.boundary = boundary.into_iter().map(|n| (n, self.store.value(n).to_vec())).collect();
                out.fired = fired.into_iter().collect();
                out.host_events = host_events;
                return Ok(Stepped::Done(out));
            }

            // the next microstep runs when a commit to a net that is not QUIET is due at it
            let next = (at, n + 1);
            let live = self
                .queue
                .due_at(next)
                .any(|p| !self.plan.nets[p.net.0 as usize].is_quiet());
            if !live {
                // what is left for this instant applies as it exits
                self.apply_due(next);
                self.obs.flush();
                break;
            }
            if (n + 1) as usize >= self.limits.max_delta_cycles {
                let mut nets: Vec<String> = self
                    .queue
                    .due_at(next)
                    .filter(|p| !self.plan.nets[p.net.0 as usize].is_quiet())
                    .map(|p| self.plan.nets[p.net.0 as usize].name.clone())
                    .collect();
                nets.sort();
                nets.dedup();
                return Err(DigitalError::NotQuiet { code: "E0501", tick: at.0, nets });
            }
            n += 1;
        }

        if !lookahead {
            self.last_run = Some((at, n));
        }
        if mode.settle {
            let settled = self.settle_phase(at, n, samples, limit, mode)?;
            out.finish |= settled.finish;
            fired.extend(settled.fired);
            boundary.extend(settled.boundary);
        }

        out.boundary = boundary.into_iter().map(|n| (n, self.store.value(n).to_vec())).collect();
        out.fired = fired.into_iter().collect();
        out.settle_boundary = self.settle_commits.boundary(|net| self.plan.nets[net.0 as usize].analog_reads, &self.store);
        Ok(Stepped::Done(out))
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
            fire_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, net, here);
        }
        let mut ambient = self.ambient(at);
        ambient.is_static = mode.is_static;
        ambient.horizon = self.horizon(at, limit.stop);
        ambient.crossing_horizon = self.horizon(at, limit.crossing);
        run_functions(&self.plan, &mut self.store, &mut self.dirty, here, &ambient, &mut self.obs)?;
        for (i, spec) in self.plan.nets.iter().enumerate() {
            let id = NetId(i as u32);
            if self.store.fired(id) {
                phase.fired.insert(id);
            }
            if spec.analog_reads && self.store.changed(id) {
                phase.boundary.insert(id);
            }
        }
        let ran = run_reactions(&mut self.plan, &self.store, samples, &ambient, &mut self.queue, &mut self.obs, here, &mut self.call, &mut self.settle_commits, Phase::Settle)?;
        phase.finish = ran.finish;
        // what the phase left for the instant, its records, applies as the instant exits, as at the
        // end of the loop: a record left queued would be a commit due at this tick forever
        self.apply_due((at, last + 1));
        self.obs.flush();
        Ok(phase)
    }
}
