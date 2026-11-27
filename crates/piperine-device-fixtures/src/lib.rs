//! The hand-written ABI devices the test suites of the back end share, one type per device. A dev-only
//! crate: no product crate depends on it. `examples` holds the worked examples of abi.md section 11
//! (`Resistor`, `Diode`, `Capacitor`, `FlipFlop`, `Comparator`, `DigitalToAnalog`) and the `ShapeParts` every
//! shape is built from; `analog` the analog assembly's own devices; `digital` the digital engine's, with
//! their call counters; `circuit` the analog device library of the solver's ABI tests; `mixed` the
//! mixed-signal devices of the protocol tests; `analyses` the doubles of the small-signal unit tests;
//! `partials` the `PartialsCheck` that holds every device's declared partials to a finite difference.

#![deny(missing_docs)]

pub mod analog;
pub mod analyses;
pub mod assembly;
pub mod circuit;
pub mod digital;
pub mod examples;
pub mod mixed;
pub mod partials;
