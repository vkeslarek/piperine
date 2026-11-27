//! The solver's contracts: the device ABI (`bind.rs`) every simulated participant
//! implements (`element.rs`), the instantiated circuit that owns them
//! (`circuit.rs`), the unified `Net` naming layer (`net.rs`), OSDI-style
//! introspection metadata (`introspect.rs`), and the analysis result
//! containers handed back to hosts (`result.rs`).

pub mod bind;
pub mod builder;
pub mod circuit;
pub mod ground;
pub mod introspect;
pub mod net;
pub mod result;
