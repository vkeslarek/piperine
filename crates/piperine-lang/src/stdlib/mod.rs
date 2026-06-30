// The stdlib is defined as .phdl source files embedded in the binary.
// Resolution and injection are handled by `crate::resolve::Resolver`:
//   - `Resolver::prelude_items()` returns the stdlib items for auto-injection.
//   - `use piperine::capabilities;` / `use piperine::collections;` resolve here.
//
// This module is kept so the .phdl files are reachable via `include_str!` paths
// in `src/resolve/mod.rs` (which uses `../stdlib/capabilities.phdl`).
