//! Unified port model (Section 3 of SOLVER_COSIMULATION.md).
//!
//! A `Port` represents any connection point in the simulation, regardless
//! of discipline.  The compiler produces port names; the elaborator resolves
//! them to `Port::Analog` or `Port::Digital`; the solver consumes them.

use crate::analog::AnalogReference;
use crate::digital::DigitalNet;

/// Represents a resolved port in the simulation.
/// The discipline determines which variant is used.
///
/// This enum is the SINGLE type used across the compiler, elaborator, and solver
/// to refer to a named signal. The compiler produces Port values. The elaborator
/// resolves them to their variant. The solver consumes them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Port {
    /// An analog (continuous) port. Participates in the Jacobian matrix.
    /// The AnalogReference contains the matrix index used by the NR solver.
    ///
    /// Example: `inout electrical vdd;` → Port::Analog(AnalogReference { ... })
    Analog(AnalogReference),

    /// A digital (discrete) port. Participates in the event queue.
    /// The DigitalNet indexes into the DigitalState.nets[] array.
    ///
    /// Example: `input logic clk;` → Port::Digital(DigitalNet(42))
    Digital(DigitalNet),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog::{AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    #[test]
    fn test_port_enum_variants() {
        let analog_ref = AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(1))), 1);
        let port_a = Port::Analog(analog_ref.clone());
        let port_d = Port::Digital(DigitalNet(42));

        match port_a {
            Port::Analog(r) => assert_eq!(r, analog_ref),
            _ => panic!("Expected Analog port"),
        }

        match port_d {
            Port::Digital(net) => assert_eq!(net, DigitalNet(42)),
            _ => panic!("Expected Digital port"),
        }
    }
}
