//! Backend loaders — one per ABI tier (SPEC Part VI §6). Native is the
//! first delivery (Plugin plan D7); WASM and process land as later phases.

pub mod native;
pub mod process;
pub mod wasm;
pub mod wire_hosted;
