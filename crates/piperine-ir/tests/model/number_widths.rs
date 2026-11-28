//! Scope: the number widths of the IR (composite-values phase D, CV-29 to CV-32, CV-34): the types
//! `f32`, `f16`, `f128` and integers of any width from 1 to 2^32 - 1, the constants that carry
//! them (`Constant::FloatBits`, `Constant::Wide`, `Expr::FloatBits`, `Expr::Wide`), what
//! `Ir::check` demands of them, and how they print and serialize. Each form is built by hand,
//! checked (valid and refused), printed and round-tripped; the oracle is the spec's own rule
//! (§1 rule 5: a width is any `N >= 1`; Q4 of the user: IEEE 754 bit patterns, two's complement
//! limbs).

use piperine_ir_fixtures::digital;
use crate::shape;

use digital::ValueAliases;
use piperine_ir::{BinaryOp, Expr, ExprId, Ir, IrBuilder, Local, Method, ValueType, WideData, WideId};
use shape::{Entries, Shape, Written};

/// Builders of designs that hold one number-width form.
struct Widths;

impl Widths {
    /// A design whose locals have the types `write` returns after adding its expressions: the
    /// returned expression (`f64`) is what the body contributes.
    fn ir(write: impl FnOnce(&mut IrBuilder) -> (Vec<Local>, ExprId)) -> Ir {
        Shape::ir(|b| {
            let (locals, value) = write(b);
            Written { locals, stmts: vec![], value }
        })
    }

    /// An expression of `kind` and type `ty` at line `line`.
    fn expr(b: &mut IrBuilder, kind: Expr, ty: ValueType, line: u32) -> ExprId {
        Shape::expr(b, kind, ty, line)
    }

    /// `(lhs + rhs) as f64` over two literals of one type `ty` built by `literal`.
    fn sum_as_f64(b: &mut IrBuilder, ty: ValueType, line: u32, literal: impl Fn(&mut IrBuilder, u32) -> ExprId) -> ExprId {
        let (lhs, rhs) = (literal(b, line), literal(b, line));
        let sum = Widths::expr(b, Expr::Binary { op: BinaryOp::Add, lhs, rhs }, ty, line);
        Widths::expr(b, Expr::Cast { expr: sum, to: ValueType::F64 }, ValueType::F64, line)
    }

    /// The wide constant of `limbs`.
    fn wide(b: &mut IrBuilder, limbs: &[u64]) -> WideId {
        b.wide(WideData { limbs: limbs.to_vec() })
    }
}

#[test]
fn number_width_types_and_constants_are_checked_printed_and_serialized() {
    // CV-29 to CV-32: every new type typed by its operators, `as` and methods, a literal of each
    // checked, then printed and serialized and read back unchanged.
    let ir = Widths::ir(|b| {
        let one_and_a_half = Widths::sum_as_f64(b, ValueType::F32, 3, |b, line| Widths::expr(b, Expr::FloatBits(0x3fc0_0000), ValueType::F32, line));
        let half_float = Widths::sum_as_f64(b, ValueType::F16, 4, |b, line| Widths::expr(b, Expr::FloatBits(0x3c00), ValueType::F16, line));
        let quad = Widths::sum_as_f64(b, ValueType::F128, 5, |b, line| Widths::expr(b, Expr::FloatBits(0x3fff_0000_0000_0000_0000_0000_0000_0000), ValueType::F128, line));
        let wide = Widths::wide(b, &[7, 0, 0, 0]);
        let wide_value = Widths::sum_as_f64(b, ValueAliases::u(200), 6, |b, line| Widths::expr(b, Expr::Wide(wide), ValueAliases::u(200), line));
        // `x.floor()` of an `f32`, `a < b` of two `f16`, `n.min(m)` of two wide integers
        let lit32 = Widths::expr(b, Expr::FloatBits(0x3fc0_0000), ValueType::F32, 7);
        let floor = Widths::expr(b, Expr::Method { method: Method::Floor, receiver: lit32, args: vec![] }, ValueType::F32, 7);
        let floor_f64 = Widths::expr(b, Expr::Cast { expr: floor, to: ValueType::F64 }, ValueType::F64, 7);
        let (low, high) = (Widths::expr(b, Expr::FloatBits(0x3c00), ValueType::F16, 8), Widths::expr(b, Expr::FloatBits(0x4000), ValueType::F16, 8));
        let less = Widths::expr(b, Expr::Binary { op: BinaryOp::Lt, lhs: low, rhs: high }, ValueType::Bool, 8);
        let (n, m) = (Widths::expr(b, Expr::Wide(wide), ValueAliases::u(200), 9), Widths::expr(b, Expr::Wide(wide), ValueAliases::u(200), 9));
        let minimum = Widths::expr(b, Expr::Method { method: Method::Min, receiver: n, args: vec![m] }, ValueAliases::u(200), 9);
        let minimum_f64 = Widths::expr(b, Expr::Cast { expr: minimum, to: ValueType::F64 }, ValueType::F64, 9);
        let zero = Shape::dx(b, 10).real(0.0);
        let picked = Widths::expr(b, Expr::If { cond: less, then: minimum_f64, otherwise: zero }, ValueType::F64, 10);
        let total = [one_and_a_half, half_float, quad, wide_value, floor_f64, picked].into_iter().reduce(|lhs, rhs| Widths::expr(b, Expr::Binary { op: BinaryOp::Add, lhs, rhs }, ValueType::F64, 11)).expect("six terms");
        let locals = [ValueType::F32, ValueType::F16, ValueType::F128, ValueAliases::u(200), ValueAliases::i(100), ValueAliases::u(4_294_967_295), ValueAliases::i(65)].into_iter().enumerate().map(|(i, ty)| Shape::local(&format!("v{i}"), ty)).collect();
        (locals, total)
    });
    assert_eq!(ir.check(), vec![], "a design of the number widths checks (§1 rule 5)");
    assert_eq!(ir.wides().len(), 1);
    assert_eq!(ir.wide(WideId(0)).limbs, [7, 0, 0, 0]);

    let text = ir.to_string();
    for shape in [
        "local v0: f32",
        "local v1: f16",
        "local v2: f128",
        "local v3: u200",
        "local v4: i100",
        "local v5: u4294967295",
        "1.5f32",
        "f16(0x3c00)",
        "f128(0x3fff0000000000000000000000000000)",
        "wide(0x0000000000000000000000000000000000000000000000000000000000000007)",
    ] {
        assert!(text.contains(shape), "the printed IR holds `{shape}`:\n{text}");
    }

    let json = serde_json::to_string(&ir).expect("serialize");
    let back: Ir = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, ir, "serialize then read is the identity");
    assert_eq!(serde_json::to_string(&back).expect("serialize"), json);
    assert_eq!(back.to_string(), text, "the read-back design prints as the original");
}

#[test]
fn a_design_without_number_widths_serializes_without_the_arena() {
    // An IR that holds none of the new forms serializes byte-identical to before: the arena of
    // wide constants is omitted when empty (`serde(default)`), so no stored IR changes.
    let ir = Widths::ir(|b| (vec![], Shape::dx(b, 3).real(1.0)));
    let json = serde_json::to_string(&ir).expect("serialize");
    assert!(!json.contains("wides"), "{json}");
    assert!(!ir.to_string().contains("wide"), "{ir}");
}

#[test]
fn a_malformed_number_width_form_is_refused() {
    // §1 rule 5: a width below 1 is refused, any width from 1 is not.
    let zero = Widths::ir(|b| (vec![Shape::local("v", ValueAliases::u(0)), Shape::local("w", ValueAliases::i(0))], Shape::dx(b, 3).real(0.0)));
    assert_eq!(Entries::with(&zero.check(), "has a width outside 1..=4294967295").len(), 2);

    // a `FloatBits` literal is typed `f32`, `f16` or `f128`, and holds no bit past the width
    let wrong_type = Widths::ir(|b| {
        let literal = Widths::expr(b, Expr::FloatBits(0), ValueType::F64, 3);
        (vec![], literal)
    });
    assert_eq!(Entries::with(&wrong_type.check(), "a float bit literal must have the type").len(), 1);
    let past_width = Widths::ir(|b| {
        let literal = Widths::expr(b, Expr::FloatBits(0x1_0000), ValueType::F16, 3);
        let value = Widths::expr(b, Expr::Cast { expr: literal, to: ValueType::F64 }, ValueType::F64, 3);
        (vec![], value)
    });
    assert_eq!(Entries::with(&past_width.check(), "has a bit past the width of f16").len(), 1);

    // a `Wide` literal names an entry, has an integer type over 64 bits, and is canonical
    let outside = Widths::ir(|b| {
        let literal = Widths::expr(b, Expr::Wide(WideId(5)), ValueAliases::u(200), 3);
        let value = Widths::expr(b, Expr::Cast { expr: literal, to: ValueType::F64 }, ValueType::F64, 3);
        (vec![], value)
    });
    assert_eq!(Entries::with(&outside.check(), "is outside the arena").len(), 1);
    let narrow = Widths::ir(|b| {
        let wide = Widths::wide(b, &[1, 0]);
        let literal = Widths::expr(b, Expr::Wide(wide), ValueAliases::u(64), 3);
        let value = Widths::expr(b, Expr::Cast { expr: literal, to: ValueType::F64 }, ValueType::F64, 3);
        (vec![], value)
    });
    assert_eq!(Entries::with(&narrow.check(), "a wide literal must have an integer type wider than 64 bits").len(), 1);
    let not_canonical = Widths::ir(|b| {
        // `UInt<70>` holds 6 bits of its second limb: bit 6 of the limb is past the width
        let too_big = Widths::wide(b, &[0, 1 << 6]);
        let wrong_count = Widths::wide(b, &[0, 0, 0]);
        let (a, c) = (Widths::expr(b, Expr::Wide(too_big), ValueAliases::u(70), 3), Widths::expr(b, Expr::Wide(wrong_count), ValueAliases::u(70), 4));
        let sum = Widths::expr(b, Expr::Binary { op: BinaryOp::Add, lhs: a, rhs: c }, ValueAliases::u(70), 5);
        let value = Widths::expr(b, Expr::Cast { expr: sum, to: ValueType::F64 }, ValueType::F64, 5);
        (vec![], value)
    });
    assert_eq!(Entries::with(&not_canonical.check(), "is not a canonical value of u70").len(), 2);
    // an `Int<70>` sign-extends: all ones in the high bits of the top limb is the value -1 and is canonical
    let negative = Widths::ir(|b| {
        let minus_one = Widths::wide(b, &[u64::MAX, u64::MAX]);
        let literal = Widths::expr(b, Expr::Wide(minus_one), ValueAliases::i(70), 3);
        let value = Widths::expr(b, Expr::Cast { expr: literal, to: ValueType::F64 }, ValueType::F64, 3);
        (vec![], value)
    });
    assert_eq!(negative.check(), vec![], "-1 as i70 is two limbs of ones");
}

#[test]
fn a_math_function_is_typed_by_its_operand() {
    // H.2: `sqrt`, `exp`, `ln` and `abs` of a float of any width give a float of that width; a mismatch is refused
    for ty in [ValueType::F32, ValueType::F16, ValueType::F128] {
        let typed = Widths::ir(|b| {
            let operand = Widths::expr(b, Expr::FloatBits(0), ty.clone(), 3);
            let root = Widths::expr(b, Expr::Math { f: piperine_ir::MathFn::Sqrt, args: vec![operand] }, ty.clone(), 3);
            let value = Widths::expr(b, Expr::Cast { expr: root, to: ValueType::F64 }, ValueType::F64, 3);
            (vec![], value)
        });
        assert_eq!(typed.check(), vec![], "sqrt of a {ty} is a {ty}");
        let mismatched = Widths::ir(|b| {
            let operand = Widths::expr(b, Expr::FloatBits(0), ty.clone(), 3);
            let value = Widths::expr(b, Expr::Math { f: piperine_ir::MathFn::Sqrt, args: vec![operand] }, ValueType::F64, 3);
            (vec![], value)
        });
        assert!(!mismatched.check().is_empty(), "sqrt of a {ty} is not an f64");
    }
}
