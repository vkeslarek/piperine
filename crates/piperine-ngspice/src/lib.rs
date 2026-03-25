pub mod callbacks;
pub mod ffi;
pub mod instance;
pub mod protocol;
pub mod worker;

pub use instance::{NgspiceError, NgspiceInstance};
pub use protocol::{MainToWorker, PlotData, VectorData, WorkerToMain};
pub use worker::worker_main;
