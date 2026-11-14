//! Digital run state: the net values, the event queue, and the
//! checkpoint/rollback snapshots the analog accept gate relies on.
use std::collections::BinaryHeap;
use std::cmp::Reverse;
use piperine_digital_solver::events::{LogicValue, DigitalNet, DigitalEvent};
use piperine_digital_solver::drivers::{DriverTable};

/// Frozen scheduler snapshot for checkpoint/rollback.
#[derive(Clone)]
struct Checkpoint {
    nets: Vec<LogicValue>,
    queue: BinaryHeap<Reverse<DigitalEvent>>,
    labels: Vec<String>,
    driver_table: DriverTable,
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
    checkpoint: Option<Checkpoint>,
    /// LRM §7.9 per-net driver records (T34/LRMC-C7-05) — rolled back
    /// alongside `nets`/`event_queue` so a rejected transient step can't
    /// leave a stale driver contribution behind.
    pub driver_table: DriverTable,
}

impl DigitalState {
    pub fn new(num_nets: usize) -> Self {
        Self {
            nets: vec![LogicValue::X; num_nets],
            event_queue: BinaryHeap::new(),
            labels: Vec::new(),
            checkpoint: None,
            driver_table: DriverTable::new(),
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
            driver_table: DriverTable::new(),
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
        self.checkpoint = Some(Checkpoint {
            nets: self.nets.clone(),
            queue: self.event_queue.clone(),
            labels: self.labels.clone(),
            driver_table: self.driver_table.clone(),
        });
    }

    pub fn rollback(&mut self) {
        if let Some(chk) = self.checkpoint.take() {
            self.nets = chk.nets;
            self.event_queue = chk.queue;
            self.labels = chk.labels;
            self.driver_table = chk.driver_table;
        }
    }

    pub fn commit(&mut self) {
        self.checkpoint = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::element::{AnalogDevice, Element, ElementCapabilities, Introspect};
    use piperine_digital_solver::interface::DigitalDevice;
    use piperine_digital_solver::interface::{DigitalPorts, EvalCtx, EventSink};

    struct MockInverter {
        // Set at construction so each mock is distinguishable in a failure
        // dump; these tests assert on the event queue, never on the id.
        #[allow(dead_code)]
        id: usize,
        input: DigitalNet,
        output: DigitalNet,
        delay: f64,
    }

    impl AnalogDevice for MockInverter {}

    impl DigitalDevice for MockInverter {
        fn boundary(&self) -> DigitalPorts<'_> {
            DigitalPorts {
                inputs: std::slice::from_ref(&self.input),
                outputs: std::slice::from_ref(&self.output),
            }
        }

        fn init(&mut self, _sink: &mut dyn EventSink) {}

        fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
            let out_val = match ctx.nets[self.input.0] {
                LogicValue::Zero => LogicValue::One,
                LogicValue::One => LogicValue::Zero,
                _ => LogicValue::X,
            };
            sink.emit(self.output, out_val, self.delay);
        }
    }

    impl Introspect for MockInverter {}

    impl Element for MockInverter {
        fn name(&self) -> &str { "mock_inverter" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    }

    #[test]
    fn test_evaluate_until_stable_chain() {
        let mut state = DigitalState::new(4);

        state.nets[0] = LogicValue::One;

        state.schedule(DigitalEvent {
            time: 1.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 99,
            seq: 0, strength: None,
        });

        let mut devices: Vec<Box<dyn Element>> = vec![
            Box::new(MockInverter { id: 0, input: DigitalNet(0), output: DigitalNet(1), delay: 0.0 }),
            Box::new(MockInverter { id: 1, input: DigitalNet(1), output: DigitalNet(2), delay: 0.0 }),
            Box::new(MockInverter { id: 2, input: DigitalNet(2), output: DigitalNet(3), delay: 0.0 }),
        ];
        state.evaluate_until_stable(1.0, &mut devices, Default::default(), &[]).unwrap();

        assert_eq!(state.nets[0], LogicValue::Zero);
        assert_eq!(state.nets[1], LogicValue::One);
        assert_eq!(state.nets[2], LogicValue::Zero);
        assert_eq!(state.nets[3], LogicValue::One);
    }

    /// T60 (LRMC-C10-12, model half): a nonzero `#delay` produces an
    /// output event at exactly `input_time + delay` — not before, not at
    /// some other offset. `evaluate_until_stable` is the scheduler
    /// primitive every gate-primitive/UDP dispatch (`piperine-codegen`'s
    /// `device/prim.rs::GateElement`, which threads its own `#delay`
    /// straight into `EventSink::emit`'s `delay` argument) and every
    /// module-path delay ultimately runs on, so proving it here proves it
    /// for every caller without needing `piperine-codegen`'s own types
    /// (which this crate must not depend on).
    #[test]
    fn nonzero_gate_delay_produces_a_correctly_timed_event() {
        let mut state = DigitalState::new(2);
        let input = DigitalNet(0);
        let output = DigitalNet(1);
        let delay = 2.0e-6;
        let mut devices: Vec<Box<dyn Element>> =
            vec![Box::new(MockInverter { id: 0, input, output, delay })];

        let t_input = 1.0e-6;
        state.schedule(DigitalEvent {
            time: t_input,
            net: input,
            value: LogicValue::One,
            source: 0,
            seq: 0,
            strength: None,
        });
        state.evaluate_until_stable(t_input, &mut devices, Default::default(), &[]).unwrap();
        assert_eq!(state.nets[output.0], LogicValue::X, "output must not flip before the delay elapses");

        // Just short of the delay: still unchanged.
        let just_before = t_input + delay - 1.0e-9;
        state.evaluate_until_stable(just_before, &mut devices, Default::default(), &[]).unwrap();
        assert_eq!(state.nets[output.0], LogicValue::X, "output must not flip early");

        // Exactly at input_time + delay: the queued event drains and applies.
        state.evaluate_until_stable(t_input + delay, &mut devices, Default::default(), &[]).unwrap();
        assert_eq!(state.nets[output.0], LogicValue::Zero, "inverter of One flips exactly at input_time + delay");
    }

    #[test]
    fn test_checkpoint_rollback() {
        let mut state = DigitalState::new(1);
        state.nets[0] = LogicValue::Zero;

        state.checkpoint();

        state.nets[0] = LogicValue::One;
        state.schedule(DigitalEvent {
            time: 5.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 0,
            seq: 0, strength: None,
        });

        assert_eq!(state.nets[0], LogicValue::One);
        assert_eq!(state.peek_next_event_time(), 5.0);

        state.rollback();

        assert_eq!(state.nets[0], LogicValue::Zero);
        assert_eq!(state.peek_next_event_time(), f64::INFINITY);
    }

    #[test]
    fn test_event_ordering() {
        let mut state = DigitalState::new(1);

        state.schedule(DigitalEvent { time: 5.0, net: DigitalNet(0), value: LogicValue::One, source: 0, seq: 2, strength: None });
        state.schedule(DigitalEvent { time: 3.0, net: DigitalNet(0), value: LogicValue::Zero, source: 0, seq: 0, strength: None });
        state.schedule(DigitalEvent { time: 5.0, net: DigitalNet(0), value: LogicValue::Z, source: 0, seq: 1, strength: None });

        let mut extracted = Vec::new();
        while let Some(Reverse(e)) = state.event_queue.pop() {
            extracted.push((e.time, e.value));
        }

        assert_eq!(extracted, vec![
            (3.0, LogicValue::Zero),
            (5.0, LogicValue::Z),
            (5.0, LogicValue::One),
        ]);
    }

    /// T54/LRMW2-46's half of the same boundary: two *distinct* sources
    /// driving one net at the same timestamp are drivers, not a region
    /// race — strong-0 against strong-1 resolves to `X` per Table 7-3, no
    /// matter which region each carries. This is the assertion that makes
    /// the same-`source` requirement above falsifiable rather than a
    /// stylistic choice.
    #[test]
    fn two_distinct_drivers_contend_per_table_7_3() {
        let mut state = DigitalState::new(1);
        state.schedule(DigitalEvent {
            time: 1.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 0,
            seq: 0,
            strength: None,
        });
        state.schedule(DigitalEvent {
            time: 1.0,
            net: DigitalNet(0),
            value: LogicValue::One,
            source: 1,
            seq: 1,
            strength: None,
        });
        state.evaluate_until_stable(1.0, &mut [], Default::default(), &[]).unwrap();
        assert_eq!(
            state.nets[0],
            LogicValue::X,
            "two equal-strength opposing drivers contend to X (Table 7-3), \
             regardless of the region each write arrived in"
        );
    }

    #[test]
    fn digital_state_carries_source_labels_or_anonymous_fallback() {
        let mut state = DigitalState::new(3);

        // No labels attached — defaults to d{idx}.
        assert_eq!(state.label_or_default(DigitalNet(0)), "d0");
        assert_eq!(state.label_or_default(DigitalNet(2)), "d2");

        // Attach a hierarchical source label and verify it survives a checkpoint
        // round-trip — used by result mappers and diagnostics.
        state.set_label(DigitalNet(1), "top.u1.clk");
        assert_eq!(state.label_or_default(DigitalNet(1)), "top.u1.clk");

        state.checkpoint();
        state.set_label(DigitalNet(1), "scratch");
        assert_eq!(state.label_or_default(DigitalNet(1)), "scratch");
        state.rollback();
        assert_eq!(state.label_or_default(DigitalNet(1)), "top.u1.clk");
    }
}
