pub mod ffi;
pub mod callbacks;
pub mod instance;
pub mod protocol;
pub mod worker;

pub use instance::{NgspiceInstance, NgspiceError};
pub use protocol::{MainToWorker, WorkerToMain, PlotData, VectorData};
pub use worker::worker_main;
