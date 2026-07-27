//! The analog side of a device instance: MNA stamping around an
//! [`AnalogKernel`](crate::kernel::analog::AnalogKernel), including the
//! reactive companion model, ideal-source branch rows, runtime operators
//! (`delay`/`slew`/`transition`), and noise.
//!
//! `instance.rs` holds `AnalogInstance` and the two contracts the capability
//! modules implement against; each capability lives in the file named for it.

mod events;
mod forces;
mod instance;
mod limits;
mod operators;

pub use instance::AnalogInstance;
use instance::{LoadCtx, Stamps};
