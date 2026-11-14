//! The two-phase delta-cycle scheduler: fixed-point evaluation over the
//! DAG topology with delta/time event queues (methods on `DigitalState`).
//!
//! Both methods below drain `self.event_queue` (a
//! `BinaryHeap<Reverse<DigitalEvent>>`) via successive `pop()`, which
//! yields events in ascending `DigitalEvent::Ord` — `(time, seq)`.
use std::collections::HashSet;
use std::cmp::Reverse;
use piperine_digital_solver::events::{LogicValue, DigitalNet, DigitalEvent};
use piperine_digital_solver::interface::{EvalCtx, QueueSink};
use crate::core::element::{Element, ElementCapabilities};
use crate::digital::state::DigitalState;
use crate::digital::topology::DigitalTopology;
use piperine_digital_solver::drivers::{default_strength_for, DriverId, DriverKind};
use piperine_digital_solver::limits::DigitalLimits;

/// Applies one drained `event` to `nets`/`driver_table` (T54/LRMW2-46):
/// registers the event's source as `net`'s driver per §7.9 (every event's
/// `source` is already the emitting device's stable index — the
/// "elaborated instance graph" this table keys off), then resolves the
/// net through [`piperine_digital_solver::drivers::DriverTable::resolved`] instead
/// of overwriting with the raw event value. For the common case (a net
/// with exactly one live driver) this folds back to the same value as a
/// direct overwrite — `Strength::resolve` of a single entry against the
/// undriven default returns that entry unchanged — so single-driver
/// nets are bit-for-bit unaffected; a multiply-driven net (e.g. a
/// tristate bus) now resolves per Table 7-3 instead of "whichever event
/// applied last wins" (register-write-order noise, not LRM semantics).
/// Returns the resolved value so callers can compare/mark `changed` the
/// same way they compared the raw event value before.
fn apply_driven_event(state: &mut DigitalState, event: &DigitalEvent) -> LogicValue {
    state.driver_table.set_driver(
        event.net,
        DriverId(event.source),
        // SPEC_DEVIATION (T54, disclosed in drivers.rs's module doc):
        // ordinary-vs-connect-module classification is not threaded
        // through to this layer yet, so every scheduler-level driver is
        // recorded `Ordinary`. `DriverTable::resolved_by_kind` (the
        // segregation primitive) is exercised directly against
        // hand-built mixed-kind tables until that plumbing lands.
        DriverKind::Ordinary,
        event.value,
        // LRMW3-09: an authored strength (a continuous assign's `(strong1,
        // weak0)` spec, threaded through `EventSink::emit_with_strength`)
        // wins over the value-derived Table 7-2 default.
        event.strength.unwrap_or_else(|| default_strength_for(event.value)),
        0.0,
    );
    state.driver_table.resolved(event.net).0
}

impl DigitalState {
    /// Simple fixed-point evaluation (fallback, no topology required).
    ///
    /// `limits` controls the delta-cycle cap and the time-equality epsilon.
    /// `analog_slice` is the latest analog solution handed to elements that
    /// declared [`ElementCapabilities::SAMPLES_ANALOG`]; pass `&[]` when no
    /// element in the circuit samples analog voltages (the common case for
    /// pure-digital circuits).
    /// Returns `Err` if the cap is reached — the scheduler used to log a
    /// warning and continue; that is no longer acceptable for a production
    /// simulation. A successful run leaves the network settled at `t`.
    pub fn evaluate_until_stable(
        &mut self,
        t: f64,
        devices: &mut [Box<dyn Element>],
        limits: DigitalLimits,
        analog_slice: &[f64],
    ) -> crate::core::result::Result<()> {
        let epsilon = limits.time_epsilon;
        let max_delta_cycles = limits.max_delta_cycles;
        let mut delta_count = 0;
        let mut seq: u64 = 0;

        loop {
            let mut events_now = Vec::new();
            while let Some(Reverse(event)) = self.event_queue.peek() {
                if event.time <= t + epsilon {
                    events_now.push(self.event_queue.pop().unwrap().0);
                } else {
                    break;
                }
            }

            if events_now.is_empty() { break; }

            let mut changed = HashSet::new();
            for event in &events_now {
                let resolved = apply_driven_event(self, event);
                if self.nets[event.net.0] != resolved {
                    self.nets[event.net.0] = resolved;
                    changed.insert(event.net);
                }
            }

            // Two-phase delta cycle (see `evaluate_dag_ordered`): commit every
            // device's register writes from the pre-settle net snapshot
            // before any device's comb output is recomputed, so a register
            // chain samples the same pre-edge values instead of racing.
            let ctx = EvalCtx { time: t, nets: &self.nets, analog: analog_slice };
            let mut fired = vec![false; devices.len()];
            for (i, device) in devices.iter_mut().enumerate() {
                if device.has_input_on(&changed) {
                    fired[i] = device.seq_phase(&ctx);
                }
            }
            for (i, device) in devices.iter_mut().enumerate() {
                if fired[i] || device.has_input_on(&changed) {
                    let mut sink = QueueSink::new(&mut self.event_queue, t, i, &mut seq);
                    device.comb_phase(&ctx, &mut sink);
                }
            }

            delta_count += 1;
            if delta_count >= max_delta_cycles {
                return Err(piperine_analog_solver::error::Error::simple(
                    piperine_analog_solver::error::SolverDomain::Digital,
                    format!(
                        "Delta cycle limit ({}) exceeded at t={}. Possible combinational loop.",
                        max_delta_cycles, t
                    ),
                ));
            }

            let has_more_at_t = self.event_queue.peek()
                .map(|Reverse(e)| e.time <= t + epsilon)
                .unwrap_or(false);

            if !has_more_at_t { break; }
        }
        Ok(())
    }

    /// DAG-ordered evaluation (Verilator-style).
    ///
    /// `analog_slice` is forwarded to elements that declared
    /// [`ElementCapabilities::SAMPLES_ANALOG`] via `EvalCtx.analog`.
    /// Returns `Err` if the iteration cap is reached instead of warning; the
    /// cap is shared with [`Self::evaluate_until_stable`] through `limits`.
    pub fn evaluate_dag_ordered(
        &mut self,
        t: f64,
        devices: &mut [Box<dyn Element>],
        topology: &DigitalTopology,
        limits: DigitalLimits,
        analog_slice: &[f64],
    ) -> crate::core::result::Result<()> {
        let epsilon = limits.time_epsilon;
        let max_iters = limits.max_delta_cycles;
        let n = topology.topo_order.len();
        if n == 0 { return Ok(()); }

        // Drain events at t into initial changed set
        let mut all_changed: HashSet<DigitalNet> = HashSet::new();
        loop {
            match self.event_queue.peek() {
                Some(Reverse(e)) if e.time <= t + epsilon => {
                    let ev = self.event_queue.pop().unwrap().0;
                    let resolved = apply_driven_event(self, &ev);
                    if self.nets[ev.net.0] != resolved {
                        self.nets[ev.net.0] = resolved;
                        all_changed.insert(ev.net);
                    }
                }
                _ => break,
            }
        }
        if all_changed.is_empty() { return Ok(()); }

        // Phase 1 of the delta cycle (Verilator-style two-phase register
        // commit): every device samples the same pre-settle net snapshot and
        // commits its register writes before ANY device's comb output is
        // recomputed. Without this, a register chain (e.g. a shift
        // register) would race ahead within a single clock edge — the first
        // flop's new Q would already be visible to the second flop's D
        // sampling in the same delta cycle (SPEC §9 requires non-blocking
        // semantics: all registers read pre-edge values).
        let mut seq: u64 = 0;
        let mut seq_fired = vec![false; devices.len()];
        {
            let ctx = EvalCtx { time: t, nets: &self.nets, analog: analog_slice };
            for &dev_idx in &topology.topo_order {
                let device = &mut devices[dev_idx];
                if device.has_input_on(&all_changed) {
                    seq_fired[dev_idx] = device.seq_phase(&ctx);
                }
            }
        }

        let mut restart_from: usize = 0;

        // Reused across devices — allocating these per device per delta
        // cycle dominated chain propagation.
        let mut output_changed_at = vec![false; n];
        let mut prev_outs: Vec<LogicValue> = Vec::new();
        let mut local_q: std::collections::BinaryHeap<Reverse<DigitalEvent>> = std::collections::BinaryHeap::new();

        'outer: for iter in 0..max_iters {
            output_changed_at.iter_mut().for_each(|c| *c = false);

            for (offset, &dev_idx) in topology.topo_order[restart_from..].iter().enumerate() {
                let topo_pos = restart_from + offset;
                let device = &mut devices[dev_idx];

                if !device.capabilities().contains(ElementCapabilities::DIGITAL) {
                    continue;
                }
                if !device.has_input_on(&all_changed) && !seq_fired[dev_idx] {
                    continue;
                }

                prev_outs.clear();
                prev_outs.extend(device.boundary().outputs.iter().map(|n| self.nets[n.0]));

                local_q.clear();
                {
                    // Scope ctx so the immutable borrow of self.nets ends
                    // before we mutate it below when draining local_q.
                    let ctx = EvalCtx { time: t, nets: &self.nets, analog: analog_slice };
                    let mut sink = QueueSink::new(&mut local_q, t, dev_idx, &mut seq);
                    device.comb_phase(&ctx, &mut sink);
                }

                while let Some(Reverse(ev)) = local_q.pop() {
                    if ev.time <= t + epsilon {
                        let resolved = apply_driven_event(self, &ev);
                        if self.nets[ev.net.0] != resolved {
                            self.nets[ev.net.0] = resolved;
                                all_changed.insert(ev.net);
                        }
                    } else {
                        self.event_queue.push(Reverse(ev));
                    }
                }

                let outputs = device.boundary().outputs;
                let outputs_changed = if outputs.is_empty() {
                    true
                } else {
                    outputs.iter().enumerate().any(|(i, n)| {
                        prev_outs.get(i).is_some_and(|&old| self.nets[n.0] != old)
                    })
                };
                output_changed_at[topo_pos] = outputs_changed;
            }

            let mut next_restart: Option<usize> = None;
            for &(src_pos, dst_pos) in &topology.back_edges {
                if output_changed_at[src_pos] {
                    next_restart = Some(match next_restart {
                        None => dst_pos,
                        Some(cur) => cur.min(dst_pos),
                    });
                }
            }

            match next_restart {
                None => break 'outer,
                Some(pos) => {
                    restart_from = pos;
                    if iter + 1 == max_iters {
                        return Err(piperine_analog_solver::error::Error::simple(
                            piperine_analog_solver::error::SolverDomain::Digital,
                            format!(
                                "DAG digital eval: back-edge loop did not converge in {} iters at t={}",
                                max_iters, t
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
