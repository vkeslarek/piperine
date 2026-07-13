use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Reverse;
use crate::digital::{LogicValue, DigitalNet, DigitalEvent};
use crate::digital::interface::{EvalCtx, QueueSink};
use crate::core::element::{Element, ElementCapabilities};

// ---------------------------------------------------------------------------
// DigitalTopology — DAG order + back edges for a fixed set of devices
// ---------------------------------------------------------------------------

pub struct DigitalTopology {
    /// Element indices (into the original `digital_runtimes` vec) in topological order.
    pub topo_order: Vec<usize>,
    /// Back edges as (src_topo_pos, dst_topo_pos) where src > dst.
    /// When the device at src_topo_pos changes its outputs, restart from dst_topo_pos.
    pub back_edges: Vec<(usize, usize)>,
}

impl DigitalTopology {
    pub fn build(devices: &[Box<dyn Element>]) -> Self {
        let n = devices.len();
        if n == 0 {
            return Self { topo_order: Vec::new(), back_edges: Vec::new() };
        }

        // net → element index that produces it. A pure-analog element drives no
        // nets (its `boundary()` is empty), so it never appears here.
        let mut output_to_dev: HashMap<DigitalNet, usize> = HashMap::new();
        for (i, dev) in devices.iter().enumerate() {
            for &net in dev.boundary().outputs {
                output_to_dev.insert(net, i);
            }
        }

        // adj[i] = elements that consume at least one of element i's outputs
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
        for (j, dev) in devices.iter().enumerate() {
            for &net in dev.boundary().inputs {
                if let Some(&i) = output_to_dev.get(&net)
                    && i != j && !adj[i].contains(&j) {
                        adj[i].push(j);
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
    /// Stable source-level labels for each digital net, parallel to `nets`.
    /// Empty when the circuit builder does not attach labels — the public
    /// lookup then falls back to the anonymous `d{idx}` form.
    labels: Vec<String>,
    checkpoint: Option<(Vec<LogicValue>, BinaryHeap<Reverse<DigitalEvent>>, Vec<String>)>,
}

impl DigitalState {
    pub fn new(num_nets: usize) -> Self {
        Self {
            nets: vec![LogicValue::X; num_nets],
            event_queue: BinaryHeap::new(),
            labels: Vec::new(),
            checkpoint: None,
        }
    }

    /// Build with explicit labels (one per net, in dense order).
    pub fn with_labels(num_nets: usize, labels: Vec<String>) -> Self {
        assert_eq!(
            labels.len(),
            num_nets,
            "with_labels: label count must match net count"
        );
        Self {
            nets: vec![LogicValue::X; num_nets],
            event_queue: BinaryHeap::new(),
            labels,
            checkpoint: None,
        }
    }

    /// Attach a label to a single digital net. Existing nets beyond the
    /// supplied index keep their labels (or `d{i}` if none was set).
    pub fn set_label(&mut self, net: DigitalNet, label: impl Into<String>) {
        let idx = net.0;
        if self.labels.len() <= idx {
            self.labels
                .resize(idx + 1, format!("d{}", self.labels.len()));
        }
        self.labels[idx] = label.into();
    }

    /// Look up the stable label for a digital net. Returns the anonymous
    /// `d{idx}` form when no source-level label was attached.
    pub fn label_or_default(&self, net: DigitalNet) -> String {
        match self.labels.get(net.0) {
            Some(s) if !s.is_empty() => s.clone(),
            _ => format!("d{}", net.0),
        }
    }

    pub fn schedule(&mut self, event: DigitalEvent) {
        self.event_queue.push(Reverse(event));
    }

    pub fn peek_next_event_time(&self) -> f64 {
        self.event_queue.peek().map(|Reverse(e)| e.time).unwrap_or(f64::INFINITY)
    }

    pub fn checkpoint(&mut self) {
        self.checkpoint = Some((
            self.nets.clone(),
            self.event_queue.clone(),
            self.labels.clone(),
        ));
    }

    pub fn rollback(&mut self) {
        if let Some((nets, queue, labels)) = self.checkpoint.take() {
            self.nets = nets;
            self.event_queue = queue;
            self.labels = labels;
        }
    }

    pub fn commit(&mut self) {
        self.checkpoint = None;
    }

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
        limits: crate::solver::convergence::PlanLimits,
        analog_slice: &[f64],
    ) -> crate::result::Result<()> {
        let epsilon = limits.digital_time_epsilon;
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
                if self.nets[event.net.0] != event.value {
                    self.nets[event.net.0] = event.value;
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
                return Err(crate::error::Error::simple(
                    crate::error::SolverDomain::Digital,
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
    /// cap is shared with [`evaluate_until_stable`] through `limits`.
    pub fn evaluate_dag_ordered(
        &mut self,
        t: f64,
        devices: &mut [Box<dyn Element>],
        topology: &DigitalTopology,
        limits: crate::solver::convergence::PlanLimits,
        analog_slice: &[f64],
    ) -> crate::result::Result<()> {
        let epsilon = limits.digital_time_epsilon;
        let max_iters = limits.max_delta_cycles;
        let n = topology.topo_order.len();
        if n == 0 { return Ok(()); }

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
        let mut local_q: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();

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
                        if self.nets[ev.net.0] != ev.value {
                            self.nets[ev.net.0] = ev.value;
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
                        return Err(crate::error::Error::simple(
                            crate::error::SolverDomain::Digital,
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
