//! The device assembly of the analog crate (feature `solver-abi-analog`): the terms an `Assembly` stamps
//! on hand-written fixture devices, one rule at a time: evaluation and the recorded calls, flow terms,
//! `DDT` companions, `DELAY`, `LIMIT`, params, noise, small signal and the watches. Each former target is a
//! module of this one.

mod assembly_ddt;
mod assembly_delay;
mod assembly_eval;
mod assembly_flow;
mod assembly_limit;
mod assembly_noise;
mod assembly_param;
mod assembly_small_signal;
mod assembly_watch;
