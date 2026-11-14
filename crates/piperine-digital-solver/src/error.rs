//! The two loud failures of the digital scheduler. `Display` is the message
//! text the owner wraps into its own error type.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum DigitalError {
    /// More delta cycles than the cap at one evaluation time.
    DeltaCycleLimit { limit: usize, time: f64 },
    /// The DAG-ordered evaluation's back-edge loop did not settle.
    DagNotConverged { iterations: usize, time: f64 },
}

impl fmt::Display for DigitalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DigitalError::DeltaCycleLimit { limit, time } => write!(
                f,
                "Delta cycle limit ({limit}) exceeded at t={time}. Possible combinational loop."
            ),
            DigitalError::DagNotConverged { iterations, time } => write!(
                f,
                "DAG digital eval: back-edge loop did not converge in {iterations} iters at t={time}"
            ),
        }
    }
}

impl std::error::Error for DigitalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_scheduler_message_text() {
        let e = DigitalError::DeltaCycleLimit { limit: 1000, time: 1.5 };
        assert_eq!(e.to_string(), "Delta cycle limit (1000) exceeded at t=1.5. Possible combinational loop.");
        let e = DigitalError::DagNotConverged { iterations: 7, time: 2.0 };
        assert_eq!(e.to_string(), "DAG digital eval: back-edge loop did not converge in 7 iters at t=2");
    }
}
