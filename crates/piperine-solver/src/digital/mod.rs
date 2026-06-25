pub mod logic;
pub mod net;
pub mod state;

#[cfg(test)]
mod tests {
    use super::*;
    use logic::LogicValue;
    use net::{DigitalNet, DigitalEvent};
    use state::{DigitalState, DigitalDevice};
    use std::collections::{HashSet, BinaryHeap};
    use std::cmp::Reverse;

    // Mock inverter for testing
    struct MockInverter {
        id: usize,
        input: DigitalNet,
        output: DigitalNet,
        delay: f64,
    }

    impl DigitalDevice for MockInverter {
        fn has_input_on(&self, changed_nets: &HashSet<DigitalNet>) -> bool {
            changed_nets.contains(&self.input)
        }

        fn eval(&mut self, current_time: f64, nets: &[LogicValue], event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
            let in_val = nets[self.input.0];
            let out_val = match in_val {
                LogicValue::Zero => LogicValue::One,
                LogicValue::One => LogicValue::Zero,
                _ => LogicValue::X,
            };
            event_queue.push(Reverse(DigitalEvent {
                time: current_time + self.delay,
                net: self.output,
                value: out_val,
                source: self.id,
                seq: 0, // Mock sequencer
            }));
        }
    }

    #[test]
    fn test_evaluate_until_stable_chain() {
        // 3 inverters in a chain: net0 -> INV0 -> net1 -> INV1 -> net2 -> INV2 -> net3
        let mut state = DigitalState::new(4);
        
        // Initial state: net0 = 1, others X
        state.nets[0] = LogicValue::One;

        let mut inv0 = MockInverter { id: 0, input: DigitalNet(0), output: DigitalNet(1), delay: 0.0 };
        let mut inv1 = MockInverter { id: 1, input: DigitalNet(1), output: DigitalNet(2), delay: 0.0 };
        let mut inv2 = MockInverter { id: 2, input: DigitalNet(2), output: DigitalNet(3), delay: 0.0 };

        // Schedule an event to trigger the chain: net0 changes 1 -> 0 at t=1.0
        state.schedule(DigitalEvent {
            time: 1.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 99,
            seq: 0,
        });

        // Evaluate at t=1.0
        let mut devices: Vec<&mut dyn DigitalDevice> = vec![&mut inv0, &mut inv1, &mut inv2];
        state.evaluate_until_stable(1.0, &mut devices);

        // After stabilization: 
        // net0 = 0
        // net1 = 1
        // net2 = 0
        // net3 = 1
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
}
