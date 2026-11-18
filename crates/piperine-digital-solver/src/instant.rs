//! One instant: the microstep loop (abi.md §5, §6; `docs/spec/part_v_execution.md`
//! §15). Each microstep applies the commits due at it (C2 order), runs the
//! functions to a fixed point (D), runs the reactions of the events that fired
//! (R) and goes on while a commit to a non-`QUIET` net is due at the next
//! microstep, up to the session's bound.

use std::collections::BTreeSet;

use crate::engine::{Digital, Horizon, Mode, Outcome};
use crate::net::{NetId, NetKind};
use crate::queue::{Instant, Writer};
use crate::run::{check_canonical, fire_net, fired_reactions, run_functions, run_reactions, write_net};
use piperine_abi::layout::Merge;
use piperine_abi::raw::{commit_flag, reaction_flag};
use crate::tick::Tick;
use crate::error::DigitalError;

/// How a pass over an instant ended.
pub(crate) enum Stepped {
    Done(Outcome),
    /// A look-ahead stopped: the instant is a landing; `time_point` is false when only an
    /// `IRREVERSIBLE` reaction made it one.
    Landing { time_point: bool },
}

/// One discrete net as the commits due at a microstep left it.
struct Settled {
    net: NetId,
    /// The whole net: its value before the microstep, each commit's bytes laid over it in order.
    bytes: Vec<u8>,
    /// The writer of the last commit, whose bytes the net holds.
    last: Writer,
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
    /// A `MERGE` commit (abi.md C7, part I §3.3.3) applies merged, level by level, with the value
    /// the net holds at that point of the C2 order: what a plain commit would leave, merged with
    /// what the net holds, so the levels the commit does not write are kept as they are.
    /// Returns each written net, in the order of its first commit; a merge the layout cannot do
    /// is the error.
    fn apply_due(&mut self, here: Instant) -> Result<Vec<Settled>, DigitalError> {
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
                    settled.push(Settled { net: p.net, bytes, last: p.writer, host: false, changed: false });
                    settled.len() - 1
                }
            };
            let offset = p.offset as usize;
            if p.flags & commit_flag::MERGE == 0 {
                let s = &mut settled[at];
                s.bytes[offset..offset + p.bytes.len()].copy_from_slice(&p.bytes);
            } else {
                let Some(layout) = self.plan.net_layout(p.net) else {
                    return Err(DigitalError::Unsupported { what: "a merge commit to a net with no layout" });
                };
                let mut laid = settled[at].bytes.clone();
                laid[offset..offset + p.bytes.len()].copy_from_slice(&p.bytes);
                Merge::of(layout).merge_into(&mut settled[at].bytes, &laid).map_err(|error| DigitalError::Merge {
                    net: self.plan.nets[p.net.0 as usize].name.clone(),
                    writer: self.writer_name(p.writer),
                    error,
                })?;
            }
            let s = &mut settled[at];
            s.last = p.writer;
            s.host |= p.writer == Writer::Host;
        }
        for s in &mut settled {
            s.changed = write_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, s.net, 0, &s.bytes, here);
        }
        Ok(settled)
    }

    /// Who made a commit, for an error: the instance's path, or the host.
    fn writer_name(&self, writer: Writer) -> String {
        match writer {
            Writer::Host => "host".to_string(),
            Writer::Reaction { instance, .. } => self.plan.instances[instance as usize].path.clone(),
        }
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
            for s in self.apply_due(here)? {
                out.host_param_write |= s.changed && s.host && self.plan.analysis().is_param_net(s.net);
                check_canonical(&self.plan, &self.store, s.net, at, &self.writer_name(s.last))?;
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
                let settle = self.plan.analysis().settle_nets();
                let firing = fired_reactions(&self.plan.instances, settle, &self.store);
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
                for s in self.apply_due(next)? {
                    check_canonical(&self.plan, &self.store, s.net, at, &self.writer_name(s.last))?;
                }
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

        if mode.settle {
            let here = (at, n + 1);
            self.store.begin_microstep();
            self.call = 0;
            for net in self.plan.analysis().settle_nets().to_vec() {
                fire_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, net, here);
                fired.insert(net);
            }
            let mut ambient = self.ambient(at);
            ambient.is_static = mode.is_static;
            ambient.horizon = self.horizon(at, limit.stop);
            ambient.crossing_horizon = self.horizon(at, limit.crossing);
            let ran = run_reactions(
                &mut self.plan,
                &self.store,
                samples,
                &ambient,
                &mut self.queue,
                &mut self.obs,
                here,
                &mut self.call,
            )?;
            out.finish |= ran.finish;
            self.obs.flush();
        }

        out.boundary = boundary.into_iter().map(|n| (n, self.store.value(n).to_vec())).collect();
        out.fired = fired.into_iter().collect();
        Ok(Stepped::Done(out))
    }
}
