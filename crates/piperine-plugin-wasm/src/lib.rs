//! Guest-side SDK for Piperine WASM plugins — a thin re-export of the wire
//! protocol (`piperine_lang::pom::wire`, SPEC Part VI §6.1). All the logic
//! lives next to the POM it serializes; this crate exists so a guest cdylib
//! has a dependency whose whole tree compiles to `wasm32-unknown-unknown`
//! (no wasmtime, no host machinery), and to host the gate example.
//!
//! A WASM plugin implements [`WirePlugin`] and hand-writes four thin
//! exports — plain functions, no macros:
//!
//! ```ignore
//! use piperine_plugin_wasm as sdk;
//!
//! struct MyPlugin;
//! impl sdk::WirePlugin for MyPlugin { /* hooks */ }
//!
//! #[unsafe(no_mangle)]
//! pub extern "C" fn pp_abi_version() -> i32 { sdk::wasm_abi_version() }
//! #[unsafe(no_mangle)]
//! pub extern "C" fn pp_alloc(len: i32) -> i32 { sdk::wasm_alloc(len) }
//! #[unsafe(no_mangle)]
//! pub extern "C" fn pp_register() -> i64 { sdk::wasm_register(&MyPlugin) }
//! #[unsafe(no_mangle)]
//! pub extern "C" fn pp_hook(ptr: i32, len: i32) -> i64 { sdk::wasm_hook(&MyPlugin, ptr, len) }
//! ```
//!
//! Compile with `--target wasm32-unknown-unknown`, crate-type `cdylib`.

pub use piperine_lang::pom::wire::*;
pub use piperine_lang::pom::wire;
