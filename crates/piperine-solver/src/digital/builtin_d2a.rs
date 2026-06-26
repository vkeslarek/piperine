use crate::digital::logic::LogicValue;
use crate::digital::net::{DigitalNet, DigitalEvent};
use crate::digital::state::DigitalDevice;
use std::collections::{BinaryHeap, HashSet};
use std::cmp::Reverse;

/// Digital-to-analog bridge.
///
/// When the input net changes, linearly ramps the output voltage from its
/// current level to the new target over `rise_time` seconds.
pub struct D2ADevice {
    pub input_net: DigitalNet,
    pub target_voltage: f64,
    pub transition_start_time: f64,
    pub v_from: f64,
    pub current_value: LogicValue,
    pub v_high: f64,
    pub v_low: f64,
    pub rise_time: f64,
}

impl Default for D2ADevice {
    fn default() -> Self {
        Self {
            input_net: DigitalNet(0),
            target_voltage: 0.0,
            transition_start_time: -1.0,
            v_from: 0.0,
            current_value: LogicValue::X,
            v_high: 1.8,
            v_low: 0.0,
            rise_time: 100e-12,
        }
    }
}

impl D2ADevice {
    pub fn new(input_net: DigitalNet) -> Self {
        Self { input_net, ..Default::default() }
    }

    /// Interpolated analog output voltage at time `t`.
    pub fn voltage_at(&self, t: f64) -> f64 {
        if self.transition_start_time < 0.0 { return self.target_voltage; }
        let elapsed = t - self.transition_start_time;
        if elapsed >= self.rise_time {
            self.target_voltage
        } else if elapsed <= 0.0 {
            self.v_from
        } else {
            self.v_from + (elapsed / self.rise_time) * (self.target_voltage - self.v_from)
        }
    }
}

impl DigitalDevice for D2ADevice {
    fn has_input_on(&self, changed: &HashSet<DigitalNet>) -> bool {
        changed.contains(&self.input_net)
    }

    fn eval(
        &mut self,
        t: f64,
        nets: &[LogicValue],
        _queue: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let new_val = nets[self.input_net.0];
        if new_val != self.current_value {
            let current_v = self.voltage_at(t);
            self.v_from = current_v;
            self.transition_start_time = t;
            self.target_voltage = match new_val {
                LogicValue::One  => self.v_high,
                LogicValue::Zero => self.v_low,
                _                => self.v_from, // hold on X/Z
            };
            self.current_value = new_val;
        }
    }

    fn input_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.input_net) }
    fn output_nets(&self) -> &[DigitalNet] { &[] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_d2a_transitions_on_one() {
        let mut d = D2ADevice::new(DigitalNet(0));
        let mut q = BinaryHeap::new();
        d.eval(10e-9, &[LogicValue::One], &mut q);
        assert_eq!(d.current_value, LogicValue::One);
        assert!((d.target_voltage - 1.8).abs() < 1e-12);
        assert_eq!(d.transition_start_time, 10e-9);
    }

    #[test]
    fn test_d2a_ramp_midpoint() {
        let mut d = D2ADevice::new(DigitalNet(0));
        let mut q = BinaryHeap::new();
        d.eval(0.0, &[LogicValue::One], &mut q);
        let v = d.voltage_at(50e-12);
        assert!((v - 0.9).abs() < 1e-12);
    }

    #[test]
    fn test_d2a_ramp_voltage() {
        let d = D2ADevice {
            target_voltage: 1.8,
            transition_start_time: 0.0,
            v_from: 0.0,
            current_value: LogicValue::One,
            ..Default::default()
        };
        assert!((d.voltage_at(0.0)   - 0.0).abs() < 1e-12);
        assert!((d.voltage_at(50e-12)  - 0.9).abs() < 1e-12);
        assert!((d.voltage_at(100e-12) - 1.8).abs() < 1e-12);
        assert!((d.voltage_at(200e-12) - 1.8).abs() < 1e-12);
    }

    #[test]
    fn test_d2a_interrupted_ramp() {
        let mut d = D2ADevice::new(DigitalNet(0));
        let mut q = BinaryHeap::new();
        d.eval(0.0, &[LogicValue::One], &mut q);
        // interrupt halfway through rise
        d.eval(50e-12, &[LogicValue::Zero], &mut q);
        assert!((d.v_from - 0.9).abs() < 1e-9, "v_from should be midpoint 0.9, got {}", d.v_from);
        assert_eq!(d.target_voltage, 0.0);
    }

    #[test]
    fn test_d2a_eval() {
        let mut d = D2ADevice::new(DigitalNet(0));
        let mut q = BinaryHeap::new();
        d.eval(10e-9, &[LogicValue::One], &mut q);
        assert_eq!(d.current_value, LogicValue::One);
        assert_eq!(d.target_voltage, 1.8);
        assert_eq!(d.v_from, 0.0);
        assert_eq!(d.transition_start_time, 10e-9);

        d.eval(10e-9 + 50e-12, &[LogicValue::Zero], &mut q);
        assert_eq!(d.current_value, LogicValue::Zero);
        assert_eq!(d.target_voltage, 0.0);
        assert!((d.v_from - 0.9).abs() < 1e-9);
        assert_eq!(d.transition_start_time, 10e-9 + 50e-12);
    }
}
