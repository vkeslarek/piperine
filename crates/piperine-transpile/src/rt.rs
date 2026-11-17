//! The runtime source: the few types generated code needs and Rust lacks. Each file is a module
//! of this crate, so its unit tests run under `cargo test` with no `rustc` call, and is embedded
//! as text to be written into every cache entry as `rt.rs` (and, for a design with digital
//! forms, the files beside it).

pub mod flag;
pub mod int;
pub mod real;

/// The text of `rt/real.rs`, as generated code compiles it.
pub const REAL: &str = include_str!("rt/real.rs");
/// The text of `rt/int.rs`, as generated code compiles it.
pub const INT: &str = include_str!("rt/int.rs");
/// The text of `rt/flag.rs`, as generated code compiles it.
pub const FLAG: &str = include_str!("rt/flag.rs");

/// The `rt.rs` of a design with digital forms: the three files as its modules, their names
/// re-exported, so generated code spells `rt::Real`, `rt::UInt`, `rt::Flagged` whatever file
/// each is in.
const ROOT: &str = "\
//! The runtime of a design with digital forms: reals and their derivatives (real.rs), integers
//! that wrap at their width (int.rs), values that may be unknown and the commits of a reaction
//! (flag.rs).

pub mod flag;
pub mod int;
pub mod real;

pub use flag::{Commits, Flagged};
pub use int::{Int, UInt, Unwrap};
pub use real::{Dual, Real};
";

/// The files a design's cache entry holds as its runtime.
#[derive(Debug)]
pub struct Runtime;

impl Runtime {
    /// The runtime files of a design, `(path under the entry, text)`, the root `rt.rs` first. A
    /// design with no digital form gets `real.rs` alone as `rt.rs`, so its entry and its cache
    /// key are what an analog-only build has always had.
    pub fn files(digital: bool) -> Vec<(&'static str, &'static str)> {
        if digital {
            vec![("rt.rs", ROOT), ("rt/real.rs", REAL), ("rt/int.rs", INT), ("rt/flag.rs", FLAG)]
        } else {
            vec![("rt.rs", REAL)]
        }
    }
}
