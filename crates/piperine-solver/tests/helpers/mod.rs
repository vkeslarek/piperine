use piperine_solver::core::element::{Element, ElementCapabilities};
use piperine_solver::digital::{LogicValue, DigitalNet};
use piperine_solver::digital::interface::{DigitalPorts, EvalCtx, EventSink};

// ---------------------------------------------------------------------------
// D2ADevice
// ---------------------------------------------------------------------------

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

impl Element for D2ADevice {
    fn name(&self) -> &str { "d2a" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts {
            inputs: std::slice::from_ref(&self.input_net),
            outputs: &[],
        }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {
        let new_val = ctx.nets[self.input_net.0];
        if new_val != self.current_value {
            let current_v = self.voltage_at(ctx.time);
            self.v_from = current_v;
            self.transition_start_time = ctx.time;
            self.target_voltage = match new_val {
                LogicValue::One  => self.v_high,
                LogicValue::Zero => self.v_low,
                _                => self.v_from,
            };
            self.current_value = new_val;
        }
    }
}



// ---------------------------------------------------------------------------
// A2DState
// ---------------------------------------------------------------------------

/// State for an analog-to-digital comparator with optional hysteresis.
pub struct A2DState {
    pub threshold: f64,
    pub hysteresis: f64,
    pub last_value: LogicValue,
}

impl Default for A2DState {
    fn default() -> Self {
        Self { threshold: 0.9, hysteresis: 0.0, last_value: LogicValue::X }
    }
}

impl A2DState {
    pub fn new(threshold: f64, hysteresis: f64) -> Self {
        Self { threshold, hysteresis, last_value: LogicValue::X }
    }

    /// Returns `Some((event_time, new_value))` if a threshold crossing occurred
    /// between `(t_prev, v_prev)` and `(t_now, v_now)`.
    pub fn check_crossing(
        &mut self,
        v_prev: f64,
        v_now: f64,
        t_prev: f64,
        t_now: f64,
    ) -> Option<(f64, LogicValue)> {
        let thresh_high = self.threshold + self.hysteresis / 2.0;
        let thresh_low  = self.threshold - self.hysteresis / 2.0;

        let (new_val, eff_thresh) = if v_prev < thresh_high && v_now >= thresh_high {
            (LogicValue::One, thresh_high)
        } else if v_prev >= thresh_low && v_now < thresh_low {
            (LogicValue::Zero, thresh_low)
        } else {
            return None;
        };

        if new_val == self.last_value { return None; }
        self.last_value = new_val;

        let dv = v_now - v_prev;
        let fraction = if dv.abs() > 1e-30 { (eff_thresh - v_prev) / dv } else { 0.0 };
        let t_cross = t_prev + fraction * (t_now - t_prev);
        let event_time = if t_cross > t_now { t_cross } else { t_now };

        Some((event_time, new_val))
    }
}
