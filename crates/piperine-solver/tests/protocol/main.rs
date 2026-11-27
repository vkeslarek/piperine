//! The owner protocol of the solver over the real sub-solvers and scripted ones (feature
//! `solver-abi-core`, `solver-abi-integration`): the transient loop of a circuit of devices, crossings and
//! boundaries, the epoch, the host's writes and waits, the operating point, the seam between the sides and
//! the sink. Each former target is a module of this one.

mod protocol_crossings;
mod protocol_epoch;
mod protocol_host;
mod protocol_operating_point;
mod protocol_real;
mod protocol_seam;
mod protocol_sink;
mod protocol_transient;
