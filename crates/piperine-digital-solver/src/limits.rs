//! The digital caps the scheduler honors.

/// Numerical caps of the digital scheduler.
#[derive(Debug, Clone, Copy)]
pub struct DigitalLimits {
    /// Maximum delta cycles in a single digital evaluation time. Above this,
    /// the scheduler reports a combinational loop instead of warning.
    pub max_delta_cycles: usize,
    /// Absolute time equality tolerance when comparing two event times.
    pub time_epsilon: f64,
}

impl Default for DigitalLimits {
    fn default() -> Self {
        Self {
            max_delta_cycles: 1000,
            time_epsilon: 1e-12,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digital_limits_defaults_are_the_literals() {
        let limits = DigitalLimits::default();
        assert_eq!(limits.max_delta_cycles, 1000);
        assert_eq!(limits.time_epsilon, 1e-12);
    }
}
