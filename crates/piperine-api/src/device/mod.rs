//! The host-language device library's ABI seam (dv-tester Phase 3,
//! `.specs/features/dv-tester/`): a device written in Rust — no PHDL, no
//! JIT, no plugin cdylib — that the solver cannot tell apart from any other
//! [`Element`](piperine_solver::abi::Element).
//!
//! - [`host_element::HostElement`] (T14) — a trait mirroring `Element`'s
//!   defaulted shape (two required methods, the rest defaulted).
//! - [`adapter`] (T14) — the blanket `impl<T: HostElement> Element for
//!   HostAdapter<T>` that presents any `HostElement` to the solver. **No
//!   solver code changes for this**: the impl lives here, over the
//!   solver's own public `Element` trait, which is the whole MD-01 story
//!   (one ABI, no downcast).
//! - [`stamp::{Ctx, StateView, StampSink}`] (T14 groundwork, hardened in
//!   T15) — the borrow-scoped wrappers a `HostElement` reads/writes
//!   through, so no raw matrix pointer ever crosses into device code.

pub mod adapter;
pub mod host_element;
pub mod stamp;

pub use adapter::HostAdapter;
pub use host_element::HostElement;
pub use stamp::{Ctx, StampError, StampSink, StateView};
