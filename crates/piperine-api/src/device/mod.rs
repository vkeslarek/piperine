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
//! - [`level1`] (T16) — turns a `piperine-ir` `Wave`/`Behavior` into
//!   native stamps and feeds a `Measure` from accepted points, surfacing
//!   the limiter's convergence veto alongside them.
//! - [`registry`] (T17) — the second `@device` binding mode: a host
//!   registration resolved at circuit-build time, tried alongside (never
//!   instead of) the existing plugin binding.

pub mod adapter;
pub mod host_element;
pub mod level1;
pub mod registry;
pub mod stamp;

pub use adapter::HostAdapter;
pub use host_element::HostElement;
pub use level1::{limiting_report, BehaviorStamp, MeasureFeed, WaveStamp};
pub use registry::{HostDeviceRegistry, HostDeviceSpec};
pub use stamp::{Ctx, StampError, StampSink, StateView};
