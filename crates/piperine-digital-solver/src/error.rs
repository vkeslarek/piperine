//! The two loud failures of the digital scheduler. `Display` is the message
//! text the owner wraps into its own error type.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum DigitalError {
    /// More delta cycles than the cap at one evaluation time.
    DeltaCycleLimit { limit: usize, time: f64 },
    /// The DAG-ordered evaluation's back-edge loop did not settle.
    DagNotConverged { iterations: usize, time: f64 },
    /// E0502: a time that is not a representable count of ticks.
    TickRange { code: &'static str, seconds: f64 },
    /// E0435: a commit made with a NaN delay.
    NanDelay { code: &'static str, instance: String, statement: u32 },
    /// E0501: an instant that did not go quiet; the nets still changing.
    NotQuiet { code: &'static str, tick: i64, nets: Vec<String> },
    /// A device returned `FAULT` (or `RESHAPE` outside the parameter cone), or
    /// the engine caught a commit that breaks its statement's promise.
    Fault { code: u32, message: String, tick: i64, instance: String },
    /// A bind rule failed; `rule` is abi.md's name for it, `what` the pins or
    /// functions that break it.
    Bind { rule: &'static str, what: Vec<String> },
    /// A construct the engine does not run yet.
    Unsupported { what: &'static str },
    /// A host write the engine refuses; nothing changed.
    HostWrite { net: String, reason: &'static str },
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
            DigitalError::TickRange { code, seconds } => {
                write!(f, "{code}: {seconds} s is outside the range of the tick count")
            }
            DigitalError::NanDelay { code, instance, statement } => {
                write!(f, "{code}: NaN delay in statement {statement} of `{instance}`")
            }
            DigitalError::NotQuiet { code, tick, nets } => {
                write!(f, "{code}: the instant at tick {tick} did not go quiet; still changing: {}", nets.join(", "))
            }
            DigitalError::Fault { code, message, tick, instance } => {
                write!(f, "fault {code} in `{instance}` at tick {tick}: {message}")
            }
            DigitalError::Bind { rule, what } => write!(f, "bind rule {rule} broken: {}", what.join("; ")),
            DigitalError::Unsupported { what } => write!(f, "unsupported: {what}"),
            DigitalError::HostWrite { net, reason } => write!(f, "host write to net `{net}` refused: {reason}"),
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
