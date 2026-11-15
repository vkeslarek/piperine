//! The digital engine of the device ABI: byte nets (`net`, `store`), a commit
//! queue (`queue`), a levelized function order (`dag`, `plan`), the instant
//! (`instant`, `run`) and the time advance (`advance`, `tick`) of the engine
//! (`engine`), what the engine observes (`observe`), its caps (`limits`) and
//! its loud failures (`error`). It names no analog concept and depends on no
//! crate of the workspace but `piperine-abi`.

pub mod advance;
pub mod dag;
pub mod engine;
pub mod error;
pub mod instant;
pub mod limits;
pub mod net;
pub mod observe;
pub mod plan;
pub mod queue;
pub mod run;
pub mod store;
pub mod tick;
