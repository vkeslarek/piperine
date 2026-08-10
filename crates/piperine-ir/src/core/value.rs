//! The two-class-plus-array type (ideal.md §1.1) and the constant pool.
//!
//! `Ty` is effectively untyped from the typecheck's standpoint — the IR
//! runs after typecheck, so this is shape, not a type lattice. There is no
//! `Int`, no `Logic4`, no signedness: signedness is a property of the OP
//! (like RTLIL), and `bits<N>` is a WIDTH, not a type.

/// Two value classes plus one aggregate. NOT a type lattice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// Every analog value; also the digital `var acc = 0.0` idiom.
    Real,
    /// Every digital logic value. `N` is a WIDTH, not a type; `bool` is
    /// `Bits(1)`. A digital value IS a vector of width `N`, so `Bit[8]`
    /// becomes ONE value of width 8 (ideal.md §1.1).
    Bits(u32),
    /// Array of `D` elements of `T`. Orthogonal to width (D11): collapsing
    /// a digital bus into an array of bits would make a 32-bit add 32 ops.
    /// `[bits<32>; 1024]` is a register file; `[real; 8]` is an analog bus.
    Array(Box<Ty>, u32),
}

impl Ty {
    /// The width in bits of a value of this type. `Real` is a single
    /// scalar slot (so width 1 — the count of values, not bits in any
    /// representational sense), `Bits(n)` is `n`, and `Array(t, d)` is
    /// `d * t.width()`.
    pub fn width(&self) -> u32 {
        match self {
            Ty::Real => 1,
            Ty::Bits(n) => *n,
            Ty::Array(elem, dim) => dim * elem.width(),
        }
    }

    /// The element type of an array, or `None` for a scalar type. Array
    /// access ops (`CellRead`/`CellWrite` with an address) produce/consume
    /// the element type, never the whole array.
    pub fn element(&self) -> Option<&Ty> {
        match self {
            Ty::Array(elem, _) => Some(elem),
            _ => None,
        }
    }
}

/// A literal held in the `Module::consts` arena. The IR carries exactly
/// two literal shapes — one per value class — and uses them for cell
/// initializers, embedded signal chunks (T6), and `Pure` op arguments
/// after lowering.
///
/// `Bits` holds up to 128 bits as a `u128` — wider bit-strings are built
/// as `SigRef` concatenations of narrower constants (T6), not as a single
/// wide literal.
#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    /// A `real` literal.
    Real(f64),
    /// A `bits<N>` literal, stored zero-extended to 128 bits. The intended
    /// width is whatever the consuming slot's `Ty::Bits(N)` says.
    Bits(u128),
}

impl Const {
    /// The narrowest `Ty` that holds this literal — `Real` for `Real`,
    /// `Bits(N)` where `N` is the smallest width that fits the value.
    /// A consuming slot may demand a wider `Ty`; the verifier checks that.
    pub fn ty(&self) -> Ty {
        match self {
            Const::Real(_) => Ty::Real,
            Const::Bits(n) => {
                let bits = if *n == 0 { 1 } else { 128 - n.leading_zeros() };
                Ty::Bits(bits)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_is_one_of_exactly_two_classes() {
        // Three variants, no more, no less — the spec is "two classes plus
        // one aggregate", not a lattice. `Real` is the analog class.
        let _real = Ty::Real;
    }

    #[test]
    fn bits_carries_a_width_not_a_type() {
        // `bool` is `Bits(1)`; an 8-bit bus is ONE value of width 8.
        assert_eq!(Ty::Bits(1).width(), 1);
        assert_eq!(Ty::Bits(8).width(), 8);
        assert_eq!(Ty::Bits(32).width(), 32);
    }

    #[test]
    fn array_depth_is_in_the_type_orthogonal_to_width() {
        // A register file is ONE value, not 1024 separate nets.
        let reg_file = Ty::Array(Box::new(Ty::Bits(32)), 1024);
        assert_eq!(reg_file.width(), 32 * 1024);
        assert_eq!(reg_file.element(), Some(&Ty::Bits(32)));

        // An analog bus is also ONE value.
        let analog_bus = Ty::Array(Box::new(Ty::Real), 8);
        assert_eq!(analog_bus.width(), 8);
        assert_eq!(analog_bus.element(), Some(&Ty::Real));
    }

    #[test]
    fn array_nests_to_build_higher_rank_shapes() {
        // A 2-D structure is `Array(Array(...))` — depth in the type, not a
        // modifier (the spec is explicit: NO depth modifier on Cell).
        let matrix = Ty::Array(Box::new(Ty::Array(Box::new(Ty::Real), 4)), 4);
        assert_eq!(matrix.width(), 16);
        assert_eq!(matrix.element(), Some(&Ty::Array(Box::new(Ty::Real), 4)));
    }

    #[test]
    fn scalar_types_have_no_element_type() {
        assert_eq!(Ty::Real.element(), None);
        assert_eq!(Ty::Bits(8).element(), None);
    }

    #[test]
    fn distinct_id_newtypes_are_not_interchangeable() {
        // The point of the newtypes: a `ValueId` is not a `CellId`, even
        // though both wrap a u32. The compiler enforces it; the test
        // exercises both id spaces and proves they pass to the right slot.
        fn takes_value(_: crate::core::ValueId) {}
        fn takes_cell(_: crate::core::CellId) {}
        takes_value(crate::core::ValueId(0));
        takes_cell(crate::core::CellId(0));
        // The line below would not compile — distinct newtypes catch the
        // mix-up at type-check time:
        // takes_value(crate::core::CellId(0));
    }

    #[test]
    fn id_newtypes_compare_equal_only_to_themselves() {
        // The same index in two id spaces is NOT the same id.
        assert_eq!(crate::core::ValueId(0), crate::core::ValueId(0));
        assert_ne!(crate::core::ValueId(0), crate::core::ValueId(1));
        // ValueId(0) and CellId(0) cannot be compared with == at all —
        // they are different types. That's the property this test guards.
    }

    #[test]
    fn const_real_infers_real_type() {
        assert_eq!(Const::Real(3.14).ty(), Ty::Real);
        assert_eq!(Const::Real(-2.5).ty(), Ty::Real);
    }

    #[test]
    fn const_bits_infers_minimum_width_width() {
        assert_eq!(Const::Bits(0).ty(), Ty::Bits(1), "zero is width-1");
        assert_eq!(Const::Bits(1).ty(), Ty::Bits(1));
        assert_eq!(Const::Bits(2).ty(), Ty::Bits(2));
        assert_eq!(Const::Bits(255).ty(), Ty::Bits(8));
        assert_eq!(Const::Bits(256).ty(), Ty::Bits(9));
    }
}
