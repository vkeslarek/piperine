//! `piperine-numbers`: numbers of any width, as embeddable source.
//!
//! MD-63 item 1 binds every file of this crate: no dependency (only `core` and `std`), no
//! `unsafe`, no macro, no panic. The same files are written verbatim into a design's cache entry
//! as `rt/numbers/*.rs` and compiled with the generated code, so a file refers to a sibling only
//! as `super::<file>::`, and an operation the language makes a panic returns `Option` here and
//! the runtime raises it. `tests/purity.rs` enforces these rules mechanically.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod arithmetic;
pub mod format;
pub mod half;
pub mod limbs;
pub mod quad;
pub mod wide;
