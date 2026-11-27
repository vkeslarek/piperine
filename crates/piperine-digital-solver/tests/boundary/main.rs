//! The boundary of the digital crate (feature `solver-abi-digital`): the crate depends on the ABI crate
//! only and names nothing of the analog side, the byte store holds bytes per net that start at zero, and
//! integer time has its tick count and session resolution. Each former target is a module of this one.

mod boundary;
mod store;
mod ticks;
