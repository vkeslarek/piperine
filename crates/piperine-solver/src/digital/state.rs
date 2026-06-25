use std::collections::{BinaryHeap, HashSet};
use std::cmp::Reverse;
use crate::digital::logic::LogicValue;
use crate::digital::net::{DigitalNet, DigitalEvent};

pub trait DigitalDevice {
    fn has_input_on(&self, changed_nets: &HashSet<DigitalNet>) -> bool;
    fn eval(&mut self, current_time: f64, nets: &[LogicValue], event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>);
}

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

    pub fn evaluate_until_stable(&mut self, t: f64, devices: &mut [&mut dyn DigitalDevice]) {
        let epsilon = 1e-12; // Time tolerance for events occurring "at the same time"
        let max_delta_cycles = 1000;
        let mut delta_count = 0;

        loop {
            let mut events_now = Vec::new();
            while let Some(Reverse(event)) = self.event_queue.peek() {
                if event.time <= t + epsilon {
                    events_now.push(self.event_queue.pop().unwrap().0);
                } else {
                    break;
                }
            }

            if events_now.is_empty() {
                break;
            }

            let mut changed = HashSet::new();
            for event in &events_now {
                if self.nets[event.net.0] != event.value {
                    self.nets[event.net.0] = event.value;
                    changed.insert(event.net);
                }
            }

            for device in devices.iter_mut() {
                if device.has_input_on(&changed) {
                    device.eval(t, &self.nets, &mut self.event_queue);
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

            if !has_more_at_t {
                break;
            }
        }
    }
}
