//! The sequencer coroutine driver layer (Phase 4, DVT-25..29/31..36/44):
//! [`Sequencer<P>`] (`driver.rs`) is a
//! [`HostElement`](crate::device::HostElement) whose `next_breakpoints`
//! comes from a pending wake advanced by a hand-written state machine (a
//! [`SequencerProgram`]) — Rust has no stable generators, so `resume` is an
//! explicit state machine rather than a language-level coroutine
//! (`ideal.md` §6). See `driver.rs`'s own header for the D11 resume-site
//! safety argument and `on_digital_settled`'s settled-only contract.
//!
//! - [`control::SequencerControl`] — the mutable handle a program drives
//!   side effects through (`t` in `ideal.md`'s examples): digital drive
//!   queuing, `expect`/`warn`.
//! - [`driver::{Sequencer, SequencerProgram, Wake}`] — the coroutine
//!   driver itself.
//! - [`findings::Findings`] — the append-only buffer `expect`/`warn` and
//!   internal-net-drive notices record into, drained into `dv-core`'s
//!   channel through `HostElement::validation_reports`.
//! - [`install::InstallSite`] — install-relative `Wave` semantics (T20).
//! - [`drive`] — `drive_voltage`/`drive_current`/`drive_digital` free
//!   functions, declared-impedance and reach-reporting (T21).
//! - [`wait::{EdgeDirection, EdgeWait}`] — the digital half of `wait` (T23),
//!   built on the solver's post-settle dispatch.

pub mod control;
pub mod crossing;
pub mod drive;
pub mod driver;
pub mod findings;
pub mod install;
pub mod wait;

pub use control::SequencerControl;
pub use driver::{Sequencer, SequencerProgram, Wake};
pub use findings::Findings;
pub use wait::EdgeDirection;
