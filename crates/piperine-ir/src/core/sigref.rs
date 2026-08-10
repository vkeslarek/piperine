//! Signal references as chunk lists (ideal.md §3 — the RTLIL SigSpec model).
//!
//! A signal reference is a CONCATENATION OF CHUNKS, not a computation.
//! `x[31:25]`, `concat(a, b)`, `{4'b0, y}` are representation, not ops.
//! This deletes `slice`/`concat`/`extract`/`insert` as ops: they are not
//! in the `Op` enum at all (T5).
//!
//! T5 forward-declares the types (so `Trigger::Edge` can name a `SigRef`);
//! T6 adds the `width` accessor and the `concat` constructor plus their
//! tests.

use super::{ConstId, ValueId};

/// One piece of a [`SigRef`]: either a contiguous slice of a typed value
/// (`x[hi:lo]`) or an embedded constant (`4'b0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk {
    /// `value[hi:lo]` — a bit-run of length `hi - lo` from `value`. The
    /// source value's `Ty` is in the `Module::values` arena; the verifier
    /// (T7) checks `hi <= source_width`.
    Slice { value: ValueId, lo: u32, hi: u32 },
    /// An embedded constant from the pool.
    Const(ConstId),
}

/// A signal reference — a list of chunks concatenated high-to-low (RTLIL
/// `SigSpec` order). `concat(a, b)` is `SigRef { chunks: a.chunks ++ b.chunks }`,
/// a constructor on this type (T6), NOT an op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigRef(pub Vec<Chunk>);
