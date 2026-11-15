//! One instant: the microstep loop (abi.md §5, §6; `docs/spec/part_v_execution.md`
//! §15). Each microstep applies the commits due at it (C2 order), runs the
//! functions to a fixed point (D), runs the reactions of the events that fired
//! (R) and goes on while a commit to a non-`QUIET` net is due at the next
//! microstep, up to the session's bound.

use std::collections::BTreeSet;

use super::engine::{Digital, Mode, Outcome};
use super::net::{NetId, NetKind};
use super::queue::Writer;
use super::run::{Ambient, fire_net, run_functions, run_reactions, write_net};
use super::tick::Tick;
use crate::error::DigitalError;

impl Digital {
    /// `HORIZON`: the last tick before the earliest pending commit and before `limit`, never
    /// before the instant itself (abi.md §5 R5).
    pub(crate) fn horizon(&self, at: Tick, limit: Tick) -> Tick {
        let next = self.queue.next_due().map_or(limit, |(t, _)| t.min(limit));
        Tick(next.0.saturating_sub(1)).max(at)
    }

    /// Runs the instant `at`: the commits due at (at, 0), then microsteps until quiet. `samples`
    /// are the accepted continuous values reactions read; `limit` caps `HORIZON`.
    pub fn run_instant(
        &mut self,
        at: Tick,
        samples: &[f64],
        limit: Tick,
        mode: Mode,
    ) -> Result<Outcome, DigitalError> {
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
                }
            }
            let mut ambient = Ambient::at(at, res);
            ambient.is_static = mode.is_static;
            ambient.horizon = self.horizon(at, limit);
            run_functions(&self.plan, &mut self.store, &mut self.dirty, here, &ambient, &mut self.obs)?;
            for (i, spec) in self.plan.nets.iter().enumerate() {
                let id = NetId(i as u32);
                if self.store.fired(id) {
                    fired.insert(id);
                }
                if spec.analog_reads && self.store.changed(id) {
                    boundary.insert(id);
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
            ambient.horizon = self.horizon(at, limit);
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
        Ok(out)
    }
}
