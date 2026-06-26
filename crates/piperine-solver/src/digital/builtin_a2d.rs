use crate::digital::logic::LogicValue;

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
    ///
    /// `event_time` is the interpolated crossing time, clamped to `t_now` if
    /// the crossing falls before the current step end.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a2d_rising_crossing() {
        let mut a2d = A2DState::default();
        let result = a2d.check_crossing(0.0, 1.8, 0.0, 10e-9);
        assert!(result.is_some());
        let (t, v) = result.unwrap();
        assert_eq!(v, LogicValue::One);
        assert_eq!(t, 10e-9);
    }

    #[test]
    fn test_a2d_crossing_tcross() {
        // threshold=1.0, v goes 0.8→1.1; crossing at fraction 2/3 of step
        let mut a2d = A2DState { last_value: LogicValue::Zero, ..A2DState::new(1.0, 0.0) };
        let result = a2d.check_crossing(0.8, 1.1, 0.0, 10e-9);
        assert!(result.is_some());
        let (t, v) = result.unwrap();
        assert_eq!(v, LogicValue::One);
        // t_cross < t_now → clamped to t_now
        assert_eq!(t, 10e-9);
        assert_eq!(a2d.last_value, LogicValue::One);
    }

    #[test]
    fn test_a2d_hysteresis_no_crossing() {
        let mut a2d = A2DState { last_value: LogicValue::Zero, ..A2DState::new(1.0, 0.2) };
        // thresh_high=1.1; v_now=1.05 doesn't reach it
        let result = a2d.check_crossing(0.95, 1.05, 0.0, 10e-9);
        assert!(result.is_none());
        assert_eq!(a2d.last_value, LogicValue::Zero);
    }

    #[test]
    fn test_a2d_no_duplicate_crossing() {
        let mut a2d = A2DState::default();
        a2d.check_crossing(0.0, 1.8, 0.0, 10e-9);
        // already One, second crossing ignored
        let result = a2d.check_crossing(0.8, 1.9, 10e-9, 20e-9);
        assert!(result.is_none());
    }

    #[test]
    fn test_a2d_falling_crossing() {
        let mut a2d = A2DState { last_value: LogicValue::One, ..Default::default() };
        let result = a2d.check_crossing(1.8, 0.0, 0.0, 10e-9);
        assert!(result.is_some());
        let (_, v) = result.unwrap();
        assert_eq!(v, LogicValue::Zero);
    }
}
