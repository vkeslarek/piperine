//! `Transpiler::compile` end to end: the cache and its racing builds, locating and running `rustc`, loading a library through `piperine_abi_1`, the compiled design as modules and a circuit, the embedded ABI sources, a session that never calls the compiler, and an `Ir` that fails `check()`. Each former target is a module of this one.

mod abi_source;
mod cache;
mod compile;
mod compiled;
mod load;
mod reject;
mod rustc;
mod session;
