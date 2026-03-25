pub mod engine;
pub mod ipc;
pub mod pool;

pub use engine::{EngineError, NgspiceEngine};
pub use pool::WorkerPool;

/// Re-export worker_main for the binary crate.
pub fn worker_main() -> std::io::Result<()> {
    piperine_ngspice::worker::worker_main()
}
