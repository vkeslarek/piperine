//! The running engine of the digital crate (feature `solver-abi-digital`): the fixture devices, look-ahead
//! and `advance`, the commit queue, the discrete functions, reactions, the microstep loop and the settle
//! phase, host writes, relaxation and checkpoint/restore, with the regressions of the first verification
//! round. Each former target is a module of this one.

mod advance;
mod checkpoint;
mod devices;
mod functions;
mod host;
mod instant;
mod queue;
mod reactions;
mod relax;
mod round1;
