//! The digital engine of the device ABI: byte nets (`net`, `store`), a commit
//! queue (`queue`), a levelized function order (`dag`, `plan`), the instant
//! (`instant`, `run`) and the time advance (`advance`, `tick`) of the engine
//! (`engine`), the values an instant passes through (`passing`), what the engine observes (`observe`), its caps (`limits`) and
//! its loud failures (`error`). It names no analog concept and depends on no
//! crate of the workspace but `piperine-abi`.

pub mod advance;
pub mod array;
pub mod dag;
pub mod engine;
pub mod error;
pub mod instant;
pub(crate) mod journal;
pub mod landing;
pub mod limits;
pub mod net;
pub mod observe;
pub(crate) mod passing;
pub mod plan;
pub mod queue;
pub mod run;
pub mod settle;
pub mod store;
pub mod tick;
