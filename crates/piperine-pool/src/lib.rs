pub mod ipc;
pub mod pool;
pub mod engine;

pub use engine::{NgspiceEngine, EngineError};
pub use pool::WorkerPool;

/// Re-export worker_main for the binary crate.
pub fn worker_main() -> std::io::Result<()> {
    piperine_ngspice::worker::worker_main()
}
