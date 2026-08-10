//! IR cells — the ONLY state concept (ideal.md §1.4).
//!
//! `cell <name> : <type>` has ONE declared writer and ONE init. The type
//! is `real | bits<N> | [T; D]`; depth lives in the type, never in a
//! separate modifier. A scalar cell is a register; an array cell
//! (`[T; D]`) is a memory (passes reason about it as a memory — port
//! count, read/write conflict, aliasing — never scalarizing by default).
//!
//! Memory ACCESS is separate ops (`CellRead`/`CellWrite` in T5), not
//! ports baked into the declaration (the RTLIL split: `$memrd`/`$memwr`/
//! `$meminit` are separate from the `Memory` object).
//!
//! Params are cells whose single writer is the host (MD-18 restamp
//! preserved). NOT cells: `ddt` companion history, state behind
//! `idt`/`transition`/`delay`/`slew` — private to their intrinsic.
//!
//! **One declared writer is load-bearing.** A0d happened because a
//! module-level var had two claimed owners (analog var bank + digital var
//! bank) and the D2A bridge blitted one over the other. Here, two writers
//! do not typecheck — `writer` is a FIELD, so violating single-writer is
//! a construction error, not a review finding.

use super::{BlockId, ConstId, Ty};

/// The only state concept in the IR. One declared writer, one init; depth
/// (if any) lives in [`Ty::Array`].
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// The cell's type — `Real`, `Bits(N)`, or `Array(T, D)`. A scalar
    /// cell is a register; an array cell is a memory.
    pub ty: Ty,
    /// The cell's initial value, drawn from the constant pool. The host
    /// is the writer for param cells (MD-18 restamp), so the init may be
    /// re-stamped across the simulation for a param.
    pub init: ConstId,
    /// The SINGLE block that may write this cell. Two writers do not
    /// typecheck — this is a field, not a convention (A0d's defence).
    pub writer: BlockId,
}

impl Cell {
    /// Construct a cell, requiring its single declared writer up front.
    /// Two writers cannot be claimed: there is no constructor that omits
    /// `writer`, so the question is unaskable.
    pub fn new(ty: Ty, init: ConstId, writer: BlockId) -> Self {
        Self { ty, init, writer }
    }

    /// `true` if this cell is a memory (array-typed). The check is on the
    /// type, not on a separate `depth` modifier — there is no such field.
    pub fn is_memory(&self) -> bool {
        matches!(self.ty, Ty::Array(..))
    }

    /// `true` if this cell is a scalar register (real or bits).
    pub fn is_register(&self) -> bool {
        matches!(self.ty, Ty::Real | Ty::Bits(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Module;

    #[test]
    fn a_scalar_cell_is_a_register() {
        let mut m = Module::new();
        let init = m.push_const(crate::core::Const::Real(0.0));
        let cell = Cell::new(Ty::Real, init, BlockId(0));
        let id = m.push_cell(cell);
        assert!(m.cell_at(id).is_register());
        assert!(!m.cell_at(id).is_memory());
    }

    #[test]
    fn an_array_cell_is_a_memory() {
        let mut m = Module::new();
        // A register file: 1024 words of 32 bits.
        let init = m.push_const(crate::core::Const::Bits(0));
        let cell = Cell::new(Ty::Array(Box::new(Ty::Bits(32)), 1024), init, BlockId(0));
        let id = m.push_cell(cell);
        assert!(m.cell_at(id).is_memory());
        assert!(!m.cell_at(id).is_register());
    }

    #[test]
    fn a_bits_cell_is_a_scalar_register_not_a_memory() {
        let mut m = Module::new();
        let init = m.push_const(crate::core::Const::Bits(0));
        let cell = Cell::new(Ty::Bits(8), init, BlockId(0));
        let id = m.push_cell(cell);
        assert!(m.cell_at(id).is_register());
        assert!(!m.cell_at(id).is_memory());
    }

    #[test]
    fn writer_is_a_field_required_at_construction_no_omitted_writer_constructor() {
        // A0d's defence: there is no `Cell::new_without_writer` and no
        // `Cell { writer: None }` — the field is a `BlockId`, not an
        // `Option<BlockId>`. This test compiles only because the
        // constructor takes the writer; if a no-writer constructor existed
        // it would be a defect.
        let mut m = Module::new();
        let init = m.push_const(crate::core::Const::Real(0.0));
        let cell = Cell::new(Ty::Real, init, BlockId(7));
        assert_eq!(cell.writer, BlockId(7));
    }

    #[test]
    fn cell_has_no_depth_modifier_depth_lives_in_the_type() {
        // The struct has fields: ty, init, writer. No `depth`, no `size`,
        // no `rank`. Array depth is in Ty::Array(_, depth).
        let mut m = Module::new();
        let init = m.push_const(crate::core::Const::Bits(0));
        let cell = Cell::new(Ty::Array(Box::new(Ty::Bits(8)), 16), init, BlockId(0));
        let id = m.push_cell(cell);
        // The only way depth shows up is by inspecting the type:
        match &m.cell_at(id).ty {
            Ty::Array(_, depth) => assert_eq!(*depth, 16),
            _ => panic!("expected an array cell"),
        }
    }

    #[test]
    fn cell_writer_and_init_round_trip_through_the_arena() {
        let mut m = Module::new();
        let init = m.push_const(crate::core::Const::Real(1.5));
        let id = m.push_cell(Cell::new(Ty::Real, init, BlockId(3)));
        let cell = m.cell_at(id);
        assert_eq!(cell.init, init);
        assert_eq!(cell.writer, BlockId(3));
        assert_eq!(cell.ty, Ty::Real);
    }

    #[test]
    fn cell_ids_are_distinct_and_allocated_in_order() {
        let mut m = Module::new();
        let init = m.push_const(crate::core::Const::Real(0.0));
        let a = m.push_cell(Cell::new(Ty::Real, init, BlockId(0)));
        let b = m.push_cell(Cell::new(Ty::Bits(1), init, BlockId(1)));
        assert_ne!(a, b);
        assert!(a < b);
    }
}
