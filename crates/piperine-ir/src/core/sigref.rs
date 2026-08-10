//! Signal references as chunk lists (ideal.md §3 — the RTLIL SigSpec model).
//!
//! A signal reference is a CONCATENATION OF CHUNKS, not a computation.
//! `x[31:25]`, `concat(a, b)`, `{4'b0, y}` are representation, not ops.
//! This deletes `slice`/`concat`/`extract`/`insert` as ops: they are not
//! in the `Op` enum at all (T5).
//!
//! The width of a `SigRef` is the SUM of its chunks' widths. A `Chunk`'s
//! width comes from one of two places: a `Slice`'s explicit `hi - lo`, or
//! a `Const`'s inferred width via the constant pool. T6 adds the `width`
//! accessor (the owner is the type — MD-13 rule 2 — but it needs the
//! `Module` to look up source widths), the `concat` constructor (a method
//! that appends chunks), and tests proving the chunk-list model.

use super::{Const, ConstId, Module, ValueId};

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

impl Chunk {
    /// The width of this chunk in bits, given the module that owns the
    /// source value or constant. A `Slice`'s width is `hi - lo`; a
    /// `Const`'s width is whatever the constant's inferred `Ty` carries.
    pub fn width(&self, module: &Module) -> u32 {
        match self {
            Chunk::Slice { lo, hi, .. } => {
                hi.saturating_sub(*lo).max(1)
            }
            Chunk::Const(cid) => module.const_at(*cid).ty().width(),
        }
    }
}

/// A signal reference — a list of chunks concatenated high-to-low (RTLIL
/// `SigSpec` order). `concat(a, b)` is `SigRef { chunks: a.chunks ++ b.chunks }`,
/// a constructor on this type, NOT an op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigRef(pub Vec<Chunk>);

impl SigRef {
    /// Build a single-chunk full-width reference: `x` with no slice. The
    /// slice spans `[0, x.width)` so the chunk carries the full source
    /// width and the verifier's `hi <= source_width` check passes.
    pub fn whole(module: &Module, value: ValueId) -> Self {
        let w = module.value_ty(value).width();
        Self(vec![Chunk::Slice { value, lo: 0, hi: w }])
    }

    /// Build a single-constant reference: `{4'b0}` becomes a one-chunk
    /// SigRef.
    pub fn from_const(c: ConstId) -> Self {
        Self(vec![Chunk::Const(c)])
    }

    /// Concatenate two SigRefs: `concat(a, b)`. The chunks are appended
    /// in order — this is REPRESENTATION, not an op.
    pub fn concat(self, other: Self) -> Self {
        let mut chunks = self.0;
        chunks.extend(other.0);
        Self(chunks)
    }

    /// The total width of this SigRef — the sum of its chunks' widths.
    /// Needs the module to look up source widths for `Slice` and `Const`
    /// chunks (the type alone doesn't carry enough).
    pub fn width(&self, module: &Module) -> u32 {
        self.0.iter().map(|c| c.width(module)).sum()
    }

    /// The chunk count — useful for tests and for the verifier's
    /// "every chunk is well-formed" check.
    pub fn chunk_count(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Ty;

    /// The load-bearing test: a 32-bit ref built from THREE chunks reports
    /// width 32 (LOV-27). Three chunks of widths summing to 32 — the IR
    /// sees ONE value of width 32, not 32 scalar nets.
    #[test]
    fn a_32_bit_ref_built_from_three_chunks_reports_width_32() {
        let mut m = Module::new();
        let a = m.push_value(Ty::Bits(8));
        let b = m.push_value(Ty::Bits(16));
        let c = m.push_value(Ty::Bits(8));

        let sr = SigRef(vec![
            Chunk::Slice { value: a, lo: 0, hi: 8 },
            Chunk::Slice { value: b, lo: 0, hi: 16 },
            Chunk::Slice { value: c, lo: 0, hi: 8 },
        ]);
        assert_eq!(sr.chunk_count(), 3);
        assert_eq!(sr.width(&m), 32);
    }

    /// Constants embed: `{4'b0, y[27:0]}` is a two-chunk SigRef whose
    /// width includes the embedded constant's width.
    #[test]
    fn constants_embed_in_the_chunk_list() {
        let mut m = Module::new();
        let zero = m.push_const(Const::Bits(0)); // ty infers Bits(1)
        let y = m.push_value(Ty::Bits(27));

        let sr = SigRef(vec![
            Chunk::Const(zero),
            Chunk::Slice { value: y, lo: 0, hi: 27 },
        ]);
        // 1 (from the constant) + 27 (from the slice) = 28.
        assert_eq!(sr.width(&m), 28);
        assert_eq!(sr.chunk_count(), 2);
    }

    /// A single-chunk full-width reference works (LOV-27): the common
    /// case where the SigRef names a whole value, no slicing.
    #[test]
    fn a_single_chunk_full_width_reference_works() {
        let mut m = Module::new();
        let x = m.push_value(Ty::Bits(32));
        let sr = SigRef::whole(&m, x);
        assert_eq!(sr.chunk_count(), 1);
        assert_eq!(sr.width(&m), 32);
    }

    /// `concat(a, b)` builds a SigRef from `a.chunks ++ b.chunks` — a
    /// constructor on the type, NOT an op. The result's width is the sum
    /// of the parts.
    #[test]
    fn concat_appends_chunks_and_widths_sum() {
        let mut m = Module::new();
        let a = m.push_value(Ty::Bits(8));
        let b = m.push_value(Ty::Bits(8));

        let sa = SigRef::whole(&m, a);
        let sb = SigRef::whole(&m, b);
        let cat = sa.concat(sb);

        assert_eq!(cat.chunk_count(), 2);
        assert_eq!(cat.width(&m), 16);
    }

    /// A sub-slice is REPRESENTATION, not an op: `x[31:24]` is a single
    /// chunk with `lo: 24, hi: 32`, not the result of an `extract` op.
    #[test]
    fn a_sub_slice_is_representation_not_an_op() {
        let mut m = Module::new();
        let x = m.push_value(Ty::Bits(32));

        let top_byte = SigRef(vec![Chunk::Slice { value: x, lo: 24, hi: 32 }]);
        assert_eq!(top_byte.width(&m), 8);
        assert_eq!(top_byte.chunk_count(), 1);
    }

    /// A real-typed value can also be referenced (a one-bit chunk per the
    /// `Ty::width()` convention) — the chunk-list model is type-generic.
    #[test]
    fn a_real_value_can_be_referenced_as_a_one_bit_chunk() {
        let mut m = Module::new();
        let r = m.push_value(Ty::Real);
        let sr = SigRef::whole(&m, r);
        assert_eq!(sr.width(&m), 1);
    }
}
