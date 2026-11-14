//! Digital events and logic values: `LogicValue`, the `DigitalNet`
//! handle, and the time-ordered `DigitalEvent` the scheduler queues.

use std::cmp::Ordering;

use crate::drivers::Strength;

// ---------------------------------------------------------------------------
// LogicValue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogicValue {
    Zero = 0,
    One = 1,
    X = 2,
    Z = 3,
}

impl LogicValue {
    /// Resolves two driving logic values onto the same net.
    pub fn resolve(a: LogicValue, b: LogicValue) -> LogicValue {
        match (a, b) {
            (LogicValue::Z, other) | (other, LogicValue::Z) => other,
            (LogicValue::Zero, LogicValue::Zero) => LogicValue::Zero,
            (LogicValue::One, LogicValue::One) => LogicValue::One,
            _ => LogicValue::X,
        }
    }
}

// ---------------------------------------------------------------------------
// DigitalNet / DigitalEvent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DigitalNet(pub usize);

#[derive(Debug, Clone)]
pub struct DigitalEvent {
    pub time: f64,
    pub net: DigitalNet,
    pub value: LogicValue,
    pub source: usize,
    pub seq: u64,
    /// LRM §7.9 Table 7-2 drive strength (LRMW3-09, A.2.2.2) explicitly
    /// authored by the emitting model (e.g. a continuous assign carrying
    /// `assign (strong1, weak0) w = r;`) — `None` for every event source
    /// that predates this field (the overwhelming common case: an
    /// ordinary write with no authored strength), which
    /// [`super::scheduler::apply_driven_event`] falls back to
    /// [`super::drivers::default_strength_for`] for, exactly as before
    /// this field existed. Additive per this struct's own ABI-stability
    /// contract (`interface.rs`'s module doc) — a model compiled against
    /// an older version never needs to know this field exists.
    pub strength: Option<Strength>,
}

impl PartialEq for DigitalEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time &&
        self.seq == other.seq &&
        self.net == other.net &&
        self.value == other.value &&
        self.source == other.source &&
        self.strength == other.strength
    }
}

impl Eq for DigitalEvent {}

impl PartialOrd for DigitalEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DigitalEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_cmp = self.time.total_cmp(&other.time);
        if time_cmp != Ordering::Equal {
            return time_cmp;
        }
        self.seq.cmp(&other.seq)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::{DigitalDevice, DigitalParticipant, DigitalPorts, EvalCtx, EventSink};
    use crate::state::DigitalState;
    use std::cmp::Reverse;

    struct MockInverter {
        // Set at construction so each mock is distinguishable in a failure
        // dump; these tests assert on the event queue, never on the id.
        #[allow(dead_code)]
        id: usize,
        input: DigitalNet,
        output: DigitalNet,
        delay: f64,
    }

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

    impl DigitalParticipant for MockInverter {
        fn is_digital(&self) -> bool { true }
    }

    #[test]
    fn test_logic_resolution() {
        assert_eq!(LogicValue::resolve(LogicValue::Zero, LogicValue::One), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::Z, LogicValue::One), LogicValue::One);
        assert_eq!(LogicValue::resolve(LogicValue::X, LogicValue::Zero), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::One, LogicValue::One), LogicValue::One);
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

        let mut devices: Vec<Box<MockInverter>> = vec![
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
        let mut devices: Vec<Box<MockInverter>> =
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
        state.evaluate_until_stable(1.0, &mut [] as &mut [Box<MockInverter>], Default::default(), &[]).unwrap();
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
