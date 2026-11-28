//! Compiled digital and mixed bodies on the solver: the forms the IR amendments add, arrays of carriers, a design whose reactions sit in a body with no carrier, commit semantics, the buck slice in a circuit, the oscillator and the DPWM, the environment, the PID, reactions and drives called through the ABI, and resolved nets. Each former target is a module of this one.

mod amended;
mod carrier_arrays;
mod carrierless;
mod digital_circuit;
mod dpwm;
mod environment;
mod pid;
mod reaction;
mod resolve;
#[path = "../support/compiled.rs"]
mod compiled;
