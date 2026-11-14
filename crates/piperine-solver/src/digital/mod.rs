//! The owner's half of the digital engine, until the scheduler moves out: the
//! digital run state (`state.rs`), the topology (`topology.rs`) and the
//! two-phase delta-cycle scheduler (`scheduler.rs`). Events, drivers and the
//! device interface live in `piperine-digital-solver`.

pub mod scheduler;
pub mod state;
pub mod topology;
