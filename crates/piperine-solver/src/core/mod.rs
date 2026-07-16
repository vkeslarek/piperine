//! The solver's contracts: the `Element` ABI every simulated participant
//! implements (`element.rs`), the instantiated circuit that owns them
//! (`circuit.rs`), the unified `Net` naming layer (`net.rs`), typed ports
//! (`port.rs`), and OSDI-style introspection metadata (`introspect.rs`).

pub mod circuit;
pub mod element;
pub mod introspect;
pub mod net;
pub mod port;
