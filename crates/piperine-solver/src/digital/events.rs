use std::cmp::Ordering;

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
}

impl PartialEq for DigitalEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time &&
        self.seq == other.seq &&
        self.net == other.net &&
        self.value == other.value &&
        self.source == other.source
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
    use crate::digital::DigitalState;
    use crate::digital::interface::{DigitalPorts, EvalCtx, EventSink};
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use std::cmp::Reverse;

    #[allow(dead_code)]
    struct MockInverter {
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
            seq: 0,
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
            seq: 0,
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

        state.schedule(DigitalEvent { time: 5.0, net: DigitalNet(0), value: LogicValue::One, source: 0, seq: 2 });
        state.schedule(DigitalEvent { time: 3.0, net: DigitalNet(0), value: LogicValue::Zero, source: 0, seq: 0 });
        state.schedule(DigitalEvent { time: 5.0, net: DigitalNet(0), value: LogicValue::Z, source: 0, seq: 1 });

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
