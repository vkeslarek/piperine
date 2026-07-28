//! The solver's contracts: the `Element` ABI every simulated participant
//! implements (`element.rs`), the instantiated circuit that owns them
//! (`circuit.rs`), the unified `Net` naming layer (`net.rs`), OSDI-style
//! introspection metadata (`introspect.rs`), and the analysis result
//! containers handed back to hosts (`result.rs`), and the validation
//! findings channel every element origin speaks through (`validation.rs`),
//! with the margin channel's run reports alongside it (`margin.rs`).

pub mod builder;
pub mod circuit;
pub mod element;
pub mod introspect;
pub mod margin;
pub mod net;
pub mod result;
pub mod validation;
