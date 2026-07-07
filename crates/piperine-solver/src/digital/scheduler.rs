use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Reverse;
use crate::digital::{LogicValue, DigitalNet, DigitalEvent};
use crate::digital::interface::{EvalCtx, QueueSink};
use crate::core::device::Device;

// ---------------------------------------------------------------------------
// DigitalTopology — DAG order + back edges for a fixed set of devices
// ---------------------------------------------------------------------------

pub struct DigitalTopology {
    /// Device indices (into the original `digital_runtimes` vec) in topological order.
    pub topo_order: Vec<usize>,
    /// Back edges as (src_topo_pos, dst_topo_pos) where src > dst.
    /// When the device at src_topo_pos changes its outputs, restart from dst_topo_pos.
    pub back_edges: Vec<(usize, usize)>,
}

impl DigitalTopology {
    pub fn build(devices: &[Box<dyn Device>]) -> Self {
        let n = devices.len();
        if n == 0 {
            return Self { topo_order: Vec::new(), back_edges: Vec::new() };
        }

        // net → device index that produces it
        let mut output_to_dev: HashMap<DigitalNet, usize> = HashMap::new();
        for (i, dev) in devices.iter().enumerate() {
            if let Some(d) = dev.as_digital_ref() {
                for &net in d.boundary().outputs {
                    output_to_dev.insert(net, i);
                }
            }
        }

        // adj[i] = devices that consume at least one of device i's outputs
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
        for (j, dev) in devices.iter().enumerate() {
            if let Some(d) = dev.as_digital_ref() {
                for &net in d.boundary().inputs {
                    if let Some(&i) = output_to_dev.get(&net) {
                        if i != j && !adj[i].contains(&j) {
                            adj[i].push(j);
                        }
                    }
                }
            }
        }

        // Iterative DFS topo sort with back-edge detection.
        // color: 0=unvisited, 1=on-stack, 2=done
        let mut color = vec![0u8; n];
        let mut topo_rev: Vec<usize> = Vec::with_capacity(n);
        let mut raw_back: Vec<(usize, usize)> = Vec::new(); // (src_dev, dst_dev)

        for start in 0..n {
            if color[start] != 0 { continue; }
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            color[start] = 1;
            while let Some((v, ai)) = stack.last_mut() {
                let v = *v;
                if *ai < adj[v].len() {
                    let u = adj[v][*ai];
                    *ai += 1;
                    match color[u] {
                        0 => { color[u] = 1; stack.push((u, 0)); }
                        1 => { raw_back.push((v, u)); } // back edge
                        _ => {}
                    }
                } else {
                    color[v] = 2;
                    topo_rev.push(v);
                    stack.pop();
                }
            }
        }

        let topo_order: Vec<usize> = topo_rev.into_iter().rev().collect();

        let mut dev_to_pos = vec![0usize; n];
        for (pos, &dev) in topo_order.iter().enumerate() {
            dev_to_pos[dev] = pos;
        }

        let back_edges = raw_back.iter()
            .map(|&(src, dst)| (dev_to_pos[src], dev_to_pos[dst]))
            .collect();

        Self { topo_order, back_edges }
    }
}

// ---------------------------------------------------------------------------
// DigitalState
// ---------------------------------------------------------------------------

pub struct DigitalState {
    pub nets: Vec<LogicValue>,
    pub event_queue: BinaryHeap<Reverse<DigitalEvent>>,
    checkpoint: Option<(Vec<LogicValue>, BinaryHeap<Reverse<DigitalEvent>>)>,
}

impl DigitalState {
    pub fn new(num_nets: usize) -> Self {
        Self {
            nets: vec![LogicValue::X; num_nets],
            event_queue: BinaryHeap::new(),
            checkpoint: None,
        }
    }

    pub fn schedule(&mut self, event: DigitalEvent) {
        self.event_queue.push(Reverse(event));
    }

    pub fn peek_next_event_time(&self) -> f64 {
        self.event_queue.peek().map(|Reverse(e)| e.time).unwrap_or(f64::INFINITY)
    }

    pub fn checkpoint(&mut self) {
        self.checkpoint = Some((self.nets.clone(), self.event_queue.clone()));
    }

    pub fn rollback(&mut self) {
        if let Some((nets, queue)) = self.checkpoint.take() {
            self.nets = nets;
            self.event_queue = queue;
        }
    }

    pub fn commit(&mut self) {
        self.checkpoint = None;
    }

    /// Simple fixed-point evaluation (fallback, no topology required).
    pub fn evaluate_until_stable(&mut self, t: f64, devices: &mut [Box<dyn Device>]) {
        let epsilon = 1e-12;
        let max_delta_cycles = 1000;
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
                if self.nets[event.net.0] != event.value {
                    self.nets[event.net.0] = event.value;
                    changed.insert(event.net);
                }
            }

            // Two-phase delta cycle (see `evaluate_dag_ordered`): commit every
            // device's register writes from the pre-settle net snapshot
            // before any device's comb output is recomputed, so a register
            // chain samples the same pre-edge values instead of racing.
            let ctx = EvalCtx { time: t, nets: &self.nets, analog: &[] };
            let mut fired = vec![false; devices.len()];
            for (i, device) in devices.iter_mut().enumerate() {
                if let Some(d) = device.as_digital() {
                    if d.has_input_on(&changed) {
                        fired[i] = d.seq_phase(&ctx);
                    }
                }
            }
            for (i, device) in devices.iter_mut().enumerate() {
                if let Some(d) = device.as_digital() {
                    if fired[i] || d.has_input_on(&changed) {
                        let mut sink = QueueSink::new(&mut self.event_queue, t, i, &mut seq);
                        d.comb_phase(&ctx, &mut sink);
                    }
                }
            }

            delta_count += 1;
            if delta_count >= max_delta_cycles {
                log::warn!("Delta cycle limit ({}) exceeded at t={}. Possible combinational loop.", max_delta_cycles, t);
                break;
            }

            let has_more_at_t = self.event_queue.peek()
                .map(|Reverse(e)| e.time <= t + epsilon)
                .unwrap_or(false);

            if !has_more_at_t { break; }
        }
    }

    /// DAG-ordered evaluation (Verilator-style).
    pub fn evaluate_dag_ordered(
        &mut self,
        t: f64,
        devices: &mut [Box<dyn Device>],
        topology: &DigitalTopology,
    ) {
        let epsilon = 1e-12;
        let n = topology.topo_order.len();
        if n == 0 { return; }

        // Drain events at t into initial changed set
        let mut all_changed: HashSet<DigitalNet> = HashSet::new();
        loop {
            match self.event_queue.peek() {
                Some(Reverse(e)) if e.time <= t + epsilon => {
                    let ev = self.event_queue.pop().unwrap().0;
                    if self.nets[ev.net.0] != ev.value {
                        self.nets[ev.net.0] = ev.value;
                        all_changed.insert(ev.net);
                    }
                }
                _ => break,
            }
        }
        if all_changed.is_empty() { return; }

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
            let ctx = EvalCtx { time: t, nets: &self.nets, analog: &[] };
            for &dev_idx in &topology.topo_order {
                let device = &mut devices[dev_idx];
                if let Some(d) = device.as_digital() {
                    if d.has_input_on(&all_changed) {
                        seq_fired[dev_idx] = d.seq_phase(&ctx);
                    }
                }
            }
        }

        const MAX_ITERS: usize = 1000;
        let mut restart_from: usize = 0;

        // Reused across devices — allocating these per device per delta
        // cycle dominated chain propagation.
        let mut output_changed_at = vec![false; n];
        let mut prev_outs: Vec<LogicValue> = Vec::new();
        let mut local_q: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();

        'outer: for iter in 0..MAX_ITERS {
            output_changed_at.iter_mut().for_each(|c| *c = false);

            for (offset, &dev_idx) in topology.topo_order[restart_from..].iter().enumerate() {
                let topo_pos = restart_from + offset;
                let device = &mut devices[dev_idx];

                if let Some(d) = device.as_digital() {
                    if !d.has_input_on(&all_changed) && !seq_fired[dev_idx] {
                        continue;
                    }

                    prev_outs.clear();
                    prev_outs.extend(d.boundary().outputs.iter().map(|n| self.nets[n.0]));

                    local_q.clear();
                    {
                        // Scope ctx so the immutable borrow of self.nets ends
                        // before we mutate it below when draining local_q.
                        let ctx = EvalCtx { time: t, nets: &self.nets, analog: &[] };
                        let mut sink = QueueSink::new(&mut local_q, t, dev_idx, &mut seq);
                        d.comb_phase(&ctx, &mut sink);
                    }

                    while let Some(Reverse(ev)) = local_q.pop() {
                        if ev.time <= t + epsilon {
                            if self.nets[ev.net.0] != ev.value {
                                self.nets[ev.net.0] = ev.value;
                                all_changed.insert(ev.net);
                            }
                        } else {
                            self.event_queue.push(Reverse(ev));
                        }
                    }

                    let outputs = d.boundary().outputs;
                    let outputs_changed = if outputs.is_empty() {
                        true
                    } else {
                        outputs.iter().enumerate().any(|(i, n)| {
                            prev_outs.get(i).is_some_and(|&old| self.nets[n.0] != old)
                        })
                    };
                    output_changed_at[topo_pos] = outputs_changed;
                }
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
                    if iter + 1 == MAX_ITERS {
                        log::warn!(
                            "DAG digital eval: back-edge loop did not converge in {} iters at t={}",
                            MAX_ITERS, t
                        );
                    }
                }
            }
        }
    }
}
