//! The hand-written ABI devices the test suites of the back end share, one type per device. A dev-only
//! crate: no product crate depends on it, and no device is declared twice (`repeated_type_names` holds this
//! crate too).
//!
//! - `circuit`: the analog device library of the solver's ABI tests (resistor, capacitor, diode, sources,
//!   inductor, noisy resistor), the same equations as the Verilog-A models.
//! - `digital`: the digital engine's devices, with their per-thread call counters; `mixed`: the mixed-signal
//!   devices of the protocol tests (comparator, converters, clock, tracers).
//! - `analog`: the analog assembly's own devices, built for one rule each, and `assembly`: the netlists and
//!   solves it tests with; `analyses`: the doubles of the small-signal unit tests.
//! - `examples`: the worked examples of abi.md section 11 that exist nowhere else (the limiting diode, the
//!   DAC) and the `ShapeParts` every shape is built from.
//! - `binding` binds placed devices into a digital plan, `circuits` places the `circuit` devices on a circuit
//!   builder, `scripted` holds the scripted stand-ins of the two sub-solvers.
//! - `solver_devices`: the devices of the solver's own integration targets, one module per target.
//! - `partials`: the `PartialsCheck` that holds every device's declared partials to a finite difference;
//!   `tests/partials.rs` lists every device once.

#![deny(missing_docs)]

pub mod analog;
pub mod analyses;
pub mod assembly;
pub mod binding;
pub mod circuit;
pub mod circuits;
pub mod digital;
pub mod examples;
pub mod mixed;
pub mod partials;
pub mod scripted;
pub mod solver_devices;
