//! One instant: the microstep loop (abi.md §5, §6; `docs/spec/part_v_execution.md`
//! §15). Each microstep applies the commits due at it (C2 order), runs the
//! functions to a fixed point (D), runs the reactions of the events that fired
//! (R) and goes on while a commit to a non-`QUIET` net is due at the next
//! microstep, up to the session's bound.

use std::collections::BTreeSet;

use super::engine::{Digital, Horizon, Mode, Outcome};
use super::net::{NetId, NetKind};
use super::queue::Writer;
use super::run::{Ambient, check_canonical, fire_net, fired_reactions, run_functions, run_reactions, write_net};
use piperine_abi::raw::reaction_flag;
use super::tick::Tick;
use crate::error::DigitalError;

/// How a pass over an instant ended.
pub(crate) enum Stepped {
    Done(Outcome),
    /// A look-ahead stopped: the instant is a landing; `time_point` is false when only an
    /// `IRREVERSIBLE` reaction made it one.
    Landing { time_point: bool },
}

impl Digital {
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
        let res = self.plan.resolution.seconds_per_tick;
        self.now = at;
        let mut out = Outcome::default();
        let mut boundary: BTreeSet<NetId> = BTreeSet::new();
        let mut fired: BTreeSet<NetId> = BTreeSet::new();
        let mut n = 0u32;
        loop {
            let here = (at, n);
            self.store.begin_microstep();
            self.call = 0;
            let mut written: Vec<(NetId, String)> = Vec::new();
            for p in self.queue.take_due(here) {
                if self.plan.nets[p.net.0 as usize].kind == NetKind::Event {
                    fire_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, p.net, here);
                } else {
                    let changed = write_net(
                        &self.plan,
                        &mut self.store,
                        &mut self.dirty,
                        &mut self.obs,
                        p.net,
                        p.offset,
                        &p.bytes,
                        here,
                    );
                    out.host_param_write |= changed && p.writer == Writer::Host && self.plan.analysis().is_param_net(p.net);
                    written.push((p.net, self.writer_name(p.writer)));
                }
            }
            for (net, who) in &written {
                check_canonical(&self.plan, &self.store, *net, at, who)?;
            }
            let mut ambient = Ambient::at(at, res);
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
                if irreversible {
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

            // the next microstep runs when a commit to a net that is not QUIET is due at it
            let next = (at, n + 1);
            let live = self
                .queue
                .due_at(next)
                .any(|p| !self.plan.nets[p.net.0 as usize].is_quiet());
            if !live {
                // what is left for this instant applies as it exits
                for p in self.queue.take_due(next) {
                    write_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, p.net, p.offset, &p.bytes, next);
                    check_canonical(&self.plan, &self.store, p.net, at, &self.writer_name(p.writer))?;
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
            let mut ambient = Ambient::at(at, res);
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
