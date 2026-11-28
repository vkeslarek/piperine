//! Scope: the number widths lowered and folded by `piperine_lang::elab` (composite-values phase D,
//! CV-29 to CV-32, CV-36; docs/spec Part I §1, Appendix A.8 rule 8, Appendix H). Each test
//! elaborates a design of `Top` and reads the IR it gives. The oracles are the rules cited at each
//! assertion: Rust's own `f32`, the bit patterns of the binary16 and binary128 neighbours of a
//! decimal (written out by hand below), and `piperine-numbers` called directly for each operation a
//! const context folds (the library is what the generated code embeds, so a fold that differs from
//! it is the evaluator's). Run-time values are proven from source in `piperine-parity`'s
//! `source_number_widths`.


use std::ops::{Add, Div, Mul, Neg, Sub};

use piperine_ir::{Constant, Expr, Ir, ValueType};
use piperine_numbers::convert::NumberConversion;
use piperine_numbers::decimal::DecimalText;
use piperine_numbers::format::BinaryFormat;
use piperine_numbers::half::Float16;
use piperine_numbers::quad::Float128;
use piperine_numbers::wide::{WideInt, WideUInt};
use piperine_lang_fixtures::elaboration::SourceElaboration;
use piperine_lang_fixtures::elaboration_forms::{ElaborationForms, IrReading};

/// What the tests ask of an elaborated design.
struct Reading;

impl Reading {
    /// The `FloatBits` literals of `ir`, with the type of each.
    fn float_literals(ir: &Ir) -> Vec<(ValueType, u128)> {
        ir.expression_types().zip(ir.expressions()).filter_map(|(ty, e)| if let Expr::FloatBits(bits) = e { Some((ty.clone(), *bits)) } else { None }).collect()
    }

    /// The limbs of every wide constant of `ir`.
    fn wide_limbs(ir: &Ir) -> Vec<Vec<u64>> {
        ir.wides().iter().map(|wide| wide.limbs.clone()).collect()
    }
}

#[test]
fn number_widths_lower_to_their_ir_types_and_constants() {
    // CV-29 AC 1, A.8 rule 8: a float literal is read in the format of its position, once, from its
    // text: `0.1f32` is Rust's `0.1f32`; the nearest binary16 to 0.1 is 0x2e66 (0.0999755859375);
    // the nearest binary128 to 0.1 is 0x3ffb999999999999999999999999999a (a round-up of the
    // repeating 1001 pattern), and neither is the rounding of the `f64` 0.1 (double rounding).
    let items = "fn single() -> f32 { 0.1f32 }\nfn half() -> f16 { 0.1 }\nfn quad() -> f128 { 0.1 }";
    let ir = SourceElaboration::ir(&ElaborationForms::reaching(items, &["single()", "half()", "quad()"]), "Top");
    let literals = Reading::float_literals(&ir);
    assert!(literals.contains(&(ValueType::F32, u128::from(0.1f32.to_bits()))), "{literals:?}");
    assert!(literals.contains(&(ValueType::F16, 0x2e66)), "{literals:?}");
    assert!(literals.contains(&(ValueType::F128, 0x3ffb_9999_9999_9999_9999_9999_9999_999a)), "{literals:?}");
    for (name, ty) in [("single", ValueType::F32), ("half", ValueType::F16), ("quad", ValueType::F128)] {
        let function = ir.functions().find(|f| f.name() == name).expect(name);
        assert_eq!(*function.ret(), ty, "{name}");
    }

    // a double-rounding literal: 1 + 2^-11 + 2^-40 is above the f16 midpoint between 1 and 1 + 2^-10; rounding it through the f64
    // first loses nothing here, but 1 + 2^-11 exactly (a tie) rounds to even (1.0) and one more digit makes it round up
    let tie = SourceElaboration::ir(&ElaborationForms::reaching("fn tie() -> f16 { 1.00048828125 }\nfn above() -> f16 { 1.000488281250000000001 }", &["tie()", "above()"]), "Top");
    let bits = Reading::float_literals(&tie);
    assert!(bits.contains(&(ValueType::F16, 0x3c00)), "the exact midpoint rounds to even: {bits:?}");
    assert!(bits.contains(&(ValueType::F16, 0x3c01)), "a digit past the midpoint, which an f64 cannot hold, rounds up: {bits:?}");

    // an `f128` literal above `f64::MAX` is read from its text in its own format, never through the `f64` (infinite there):
    // the nearest binary128 to 1e400, and the largest finite binary128 written as a literal (both from an independent exact-decimal computation)
    let beyond = SourceElaboration::ir(&ElaborationForms::reaching("fn huge() -> f128 { 1e400 }\nfn largest() -> f128 { 1.18973149535723176508575932662800702e4932 }", &["huge()", "largest()"]), "Top");
    let literals = Reading::float_literals(&beyond);
    assert!(literals.contains(&(ValueType::F128, 0x452f_b4ec_7f91_973f_f3cb_1ccf_26fb_c178)), "{literals:?}");
    assert!(literals.contains(&(ValueType::F128, 0x7ffe_ffff_ffff_ffff_ffff_ffff_ffff_ffff)), "{literals:?}");

    // CV-32: an integer wider than 64 bits is a wide constant of the type's limbs, and the types are the IR's
    let wide = SourceElaboration::ir(&ElaborationForms::reaching("fn big() -> UInt<200> { 0x1_0000_0000_0000_0000_0000 }\nfn minus() -> Int<100> { -5 }", &["big()", "minus()"]), "Top");
    assert!(Reading::wide_limbs(&wide).contains(&vec![0, 0x1_0000, 0, 0]), "{:?}", Reading::wide_limbs(&wide));
    let big = wide.functions().find(|f| f.name() == "big").expect("big");
    assert_eq!(*big.ret(), ValueType::UInt(200));
    let minus = wide.functions().find(|f| f.name() == "minus").expect("minus");
    assert_eq!(*minus.ret(), ValueType::Int(100));
    assert!(Reading::wide_limbs(&wide).contains(&vec![u64::MAX - 4, u64::MAX]), "-5 as i100 is sign-extended to its limbs: {:?}", Reading::wide_limbs(&wide));

    // a param of a new type holds its default as the IR's constant of that type
    let params = SourceElaboration::ir(&ElaborationForms::with_params("", "a: f32 = 1.5; b: UInt<70> = 3;"), "Top");
    assert_eq!(IrReading::default_of(&params, "a"), Constant::FloatBits(u128::from(1.5f32.to_bits())));
    assert!(matches!(IrReading::default_of(&params, "b"), Constant::Wide(_)), "{:?}", IrReading::default_of(&params, "b"));
}

/// A float format of the number widths and the library calls that give what a const context must fold.
trait Sample: Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self> + PartialOrd {
    /// The source spelling of the type.
    const NAME: &'static str;
    /// The interchange format.
    const FORMAT: BinaryFormat;
    /// The value of a bit pattern.
    fn from_pattern(pattern: u128) -> Self;
    /// The bit pattern.
    fn pattern(self) -> u128;
    /// The nearest value to the decimal `text` (A.8 rule 8).
    fn decimal(text: &str) -> Self {
        Self::from_pattern(DecimalText::nearest(text, Self::FORMAT).expect("a decimal"))
    }
    /// `self.min(other)`.
    fn least(self, other: Self) -> Self;
    /// `self.floor()`.
    fn floor(self) -> Self;
    /// `self.sin()`.
    fn sin(self) -> Self;
    /// `self.sqrt()`.
    fn sqrt(self) -> Self;
    /// `self.powi(n)`.
    fn powi(self, n: i32) -> Self;
    /// `self.clamp(lo, hi)`.
    fn clamped(self, lo: Self, hi: Self) -> Self;
    /// `self.atan2(x)`.
    fn atan2(self, x: Self) -> Self;
}

impl Sample for f32 {
    const NAME: &'static str = "f32";
    const FORMAT: BinaryFormat = BinaryFormat::SINGLE;
    fn from_pattern(pattern: u128) -> f32 { f32::from_bits(pattern as u32) }
    fn pattern(self) -> u128 { u128::from(self.to_bits()) }
    fn least(self, other: f32) -> f32 { self.min(other) }
    fn floor(self) -> f32 { f32::floor(self) }
    fn sin(self) -> f32 { f32::sin(self) }
    fn sqrt(self) -> f32 { f32::sqrt(self) }
    fn powi(self, n: i32) -> f32 { f32::powi(self, n) }
    fn clamped(self, lo: f32, hi: f32) -> f32 { f32::clamp(self, lo, hi) }
    fn atan2(self, x: f32) -> f32 { f32::atan2(self, x) }
}

impl Sample for Float16 {
    const NAME: &'static str = "f16";
    const FORMAT: BinaryFormat = BinaryFormat::HALF;
    fn from_pattern(pattern: u128) -> Float16 { Float16::from_bits(pattern as u16) }
    fn pattern(self) -> u128 { u128::from(self.to_bits()) }
    fn least(self, other: Float16) -> Float16 { self.min(other) }
    fn floor(self) -> Float16 { Float16::floor(self) }
    fn sin(self) -> Float16 { Float16::sin(self) }
    fn sqrt(self) -> Float16 { Float16::sqrt(self) }
    fn powi(self, n: i32) -> Float16 { Float16::powi(self, n) }
    fn clamped(self, lo: Float16, hi: Float16) -> Float16 { self.clamp_checked(lo, hi).expect("ordered bounds") }
    fn atan2(self, x: Float16) -> Float16 { Float16::atan2(self, x) }
}

impl Sample for Float128 {
    const NAME: &'static str = "f128";
    const FORMAT: BinaryFormat = BinaryFormat::QUAD;
    fn from_pattern(pattern: u128) -> Float128 { Float128::from_bits(pattern) }
    fn pattern(self) -> u128 { self.to_bits() }
    fn least(self, other: Float128) -> Float128 { self.min(other) }
    fn floor(self) -> Float128 { Float128::floor(self) }
    fn sin(self) -> Float128 { Float128::sin(self) }
    fn sqrt(self) -> Float128 { Float128::sqrt(self) }
    fn powi(self, n: i32) -> Float128 { Float128::powi(self, n) }
    fn clamped(self, lo: Float128, hi: Float128) -> Float128 { self.clamp_checked(lo, hi).expect("ordered bounds") }
    fn atan2(self, x: Float128) -> Float128 { Float128::atan2(self, x) }
}

/// The params of one design that each fold a const expression, and the constant each must hold.
struct Folds {
    /// The `name: type = expression;` of each param.
    params: Vec<String>,
    /// The constant expected of each, by param index.
    expected: Vec<Constant>,
    /// The limbs expected of each param that holds a wide constant, by param index.
    wide: Vec<Option<Vec<u64>>>,
}

impl Folds {
    fn new() -> Folds {
        Folds { params: Vec::new(), expected: Vec::new(), wide: Vec::new() }
    }

    /// A param `p<k>: ty = expression` expected to hold `constant`.
    fn expect(&mut self, ty: &str, expression: &str, constant: Constant) {
        self.params.push(format!("p{}: {ty} = {expression};", self.params.len()));
        self.expected.push(constant);
        self.wide.push(None);
    }

    /// A param expected to hold the wide constant of `limbs`.
    fn expect_wide(&mut self, ty: &str, expression: &str, limbs: Vec<u64>) {
        self.expect(ty, expression, Constant::Bool(false));
        *self.wide.last_mut().expect("a param") = Some(limbs);
    }

    /// The operations of one float format: every operator, comparison, method and `as` of a const.
    fn floats<T: Sample>(&mut self) {
        let name = T::NAME;
        let (a, b, c) = (T::decimal("2.5"), T::decimal("0.1"), T::decimal("-3.75"));
        let float = |value: T| Constant::FloatBits(value.pattern());
        for (op, want) in [("+", a + b), ("-", a - b), ("*", a * b), ("/", a / b)] {
            self.expect(name, &format!("A {op} B"), float(want));
        }
        for (op, want) in [("<", a < b), (">=", a >= b), ("==", a == b), ("!=", a != b)] {
            self.expect("bool", &format!("A {op} B"), Constant::Bool(want));
        }
        self.expect(name, "-A", float(-a));
        self.expect(name, "A.min(B)", float(a.least(b)));
        self.expect(name, "C.floor()", float(c.floor()));
        self.expect(name, "C.sin()", float(c.sin()));
        self.expect(name, "A.sqrt()", float(a.sqrt()));
        self.expect(name, "A.powi(3)", float(a.powi(3)));
        self.expect(name, "C.clamp(B, A)", float(c.clamped(b, a)));
        self.expect(name, "A.atan2(B)", float(a.atan2(b)));
        self.expect("bool", "A.is_nan()", Constant::Bool(false));
        // `as` rounds once from the source: to `f64`, from `f64`, to and from integers, to the other formats
        let from = |value: T, target: BinaryFormat| NumberConversion::float_to_float(T::FORMAT, value.pattern(), target);
        self.expect("f64", "A as f64", Constant::Real(piperine_ir::Real::new(f64::from_bits(from(a, BinaryFormat::DOUBLE) as u64))));
        self.expect(name, "1.1f64 as T", float(T::from_pattern(NumberConversion::float_to_float(BinaryFormat::DOUBLE, u128::from(1.1f64.to_bits()), T::FORMAT))));
        self.expect("u8", "A as u8", Constant::Int(2));
        self.expect("u8", "C as u8", Constant::Int(0));
        self.expect(name, "(0 - 7i32) as T", float(T::from_pattern(NumberConversion::integer_to_float(true, &[7], T::FORMAT))));
        self.expect("f32", "A as f32", Constant::FloatBits(from(a, BinaryFormat::SINGLE)));
        self.expect("f16", "A as f16", Constant::FloatBits(from(a, BinaryFormat::HALF)));
        self.expect("f128", "A as f128", Constant::FloatBits(from(a, BinaryFormat::QUAD)));
        self.expect_wide("UInt<200>", "A as UInt<200>", vec![2, 0, 0, 0]);
        self.expect_wide("Int<100>", "C as Int<100>", vec![u64::MAX - 2, u64::MAX]);
    }

    /// The source of the consts `A`, `B`, `C` of `ty` and the params.
    fn source(&self, ty: &str) -> String {
        let consts = format!("const A: {ty} = 2.5; const B: {ty} = 0.1; const C: {ty} = -3.75;");
        let params: String = self.params.join(" ").replace(" as T", &format!(" as {ty}"));
        ElaborationForms::with_params(&consts, &params)
    }

    /// Elaborates the source and compares each param's folded default with the expectation.
    fn check(&self, ty: &str) {
        let ir = SourceElaboration::ir(&self.source(ty), "Top");
        let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
        for (index, (expected, limbs)) in self.expected.iter().zip(&self.wide).enumerate() {
            let param = top.params().iter().find(|p| p.name == format!("p{index}")).expect("a param");
            let got = param.default.expect("a default");
            let text = &self.params[index];
            match (limbs, got) {
                (Some(limbs), Constant::Wide(id)) => assert_eq!(&ir.wide(id).limbs, limbs, "{ty}: {text}"),
                (Some(_), other) => panic!("{ty}: {text}: a wide constant is expected, found {other:?}"),
                (None, got) => assert_eq!(got, *expected, "{ty}: {text}"),
            }
        }
    }
}

#[test]
fn const_contexts_compute_every_width_as_the_run_does() {
    // CV-36 AC 1, A.8 rule 8, §1.4.5, H.2: a const folds every operator, comparison, method and `as`
    // of each float format to the bits the library the generated code embeds gives (and Rust's own
    // `f32`), in a `const`, a param default and a method call on a const
    let mut single = Folds::new();
    single.floats::<f32>();
    single.check("f32");
    let mut half = Folds::new();
    half.floats::<Float16>();
    half.check("f16");
    let mut quad = Folds::new();
    quad.floats::<Float128>();
    quad.check("f128");

    // integers wider than 64 bits wrap modulo 2^N (§1 rule 6) with the arithmetic of `WideUInt`/`WideInt`
    let mut wide = Folds::new();
    let (a, b) = (WideUInt::<200, 4>::from_limbs(&[0x1234_5678_9abc_def0, 0xfedc_ba98_7654_3210, 0, 0]), WideUInt::<200, 4>::from_limbs(&[7, 0, 0, 0]));
    let limbs = |value: WideUInt<200, 4>| value.limbs().to_vec();
    wide.expect_wide("UInt<200>", "WA + WB", limbs(a + b));
    wide.expect_wide("UInt<200>", "WB - WA", limbs(b - a));
    wide.expect_wide("UInt<200>", "WA * WB", limbs(a * b));
    wide.expect_wide("UInt<200>", "WA << 70u8", limbs(a.shift_left(70)));
    wide.expect_wide("UInt<200>", "WA >> 70u8", limbs(a.shift_right(70)));
    wide.expect_wide("UInt<200>", "!WB", limbs(!b));
    wide.expect_wide("UInt<200>", "WB.pow(3)", limbs(b.pow(3)));
    wide.expect("bool", "WB < WA", Constant::Bool(b < a));
    wide.expect_wide("UInt<200>", "WA / WB", limbs(a.checked_divide(b).expect("a divisor")));
    wide.expect_wide("UInt<200>", "WA % WB", limbs(a.checked_remainder(b).expect("a divisor")));
    let (p, q) = (WideInt::<100, 2>::from_i128(-123_456_789_012_345_678_901_234_567), WideInt::<100, 2>::from_i128(98_765_432_109_876_543));
    let signed = |value: WideInt<100, 2>| value.sign_extended_limbs().to_vec();
    wide.expect_wide("Int<100>", "SP", signed(p));
    wide.expect_wide("Int<100>", "SP / SQ", signed(p.checked_divide(q).expect("a divisor")));
    wide.expect_wide("Int<100>", "SP % SQ", signed(p.checked_remainder(q).expect("a divisor")));
    wide.expect_wide("Int<100>", "SP >> 5u8", signed(p.shift_right(5)));
    wide.expect_wide("Int<100>", "SP.abs()", signed(p.abs()));
    wide.expect_wide("Int<100>", "SP.rem_euclid(SQ)", signed(p.rem_euclid_checked(q).expect("a divisor")));
    wide.expect_wide("Int<100>", "SP.saturating_mul(SQ)", signed(p.saturating_mul(q)));
    wide.expect("f64", "SP as f64", Constant::Real(piperine_ir::Real::new(f64::from_bits(NumberConversion::signed_to_float(p, BinaryFormat::DOUBLE) as u64))));
    wide.expect("u8", "WA as u8", Constant::Int(i128::from(a.low_u64() as u8)));
    wide.expect("f32", "WA as f32", Constant::FloatBits(NumberConversion::unsigned_to_float(a, BinaryFormat::SINGLE)));
    // a literal is at most 128 bits (A.2 rule 8), so the typed consts are the operands
    let items = format!("const WA: UInt<200> = 0x{:032x}; const WB: UInt<200> = 7; const SP: Int<100> = -123456789012345678901234567; const SQ: Int<100> = 98765432109876543;", a.low_u128());
    let ir = SourceElaboration::ir(&ElaborationForms::with_params(&items, &wide.params.join(" ")), "Top");
    let top = ir.bodies().find(|body| body.name() == "Top").expect("Top");
    for (index, (expected, limbs)) in wide.expected.iter().zip(&wide.wide).enumerate() {
        let got = top.params().iter().find(|param| param.name == format!("p{index}")).and_then(|param| param.default).expect("a default");
        match (limbs, got) {
            (Some(limbs), Constant::Wide(id)) => assert_eq!(&ir.wide(id).limbs, limbs, "{}", wide.params[index]),
            (Some(_), other) => panic!("{}: a wide constant is expected, found {other:?}", wide.params[index]),
            (None, got) => assert_eq!(got, *expected, "{}", wide.params[index]),
        }
    }
}

#[test]
fn a_const_fn_and_an_array_length_fold_over_the_new_widths() {
    // CV-36 AC 1: a `const fn` over `f16` and over `UInt<100>` runs at elaboration with the run-time arithmetic, and an array
    // length is a wide const read as a count
    let items = "const fn twice(x: f16) -> f16 { x * 2.0 }\nconst fn square(x: UInt<100>) -> UInt<100> { x * x }\nconst H: f16 = 1.5;\nconst W: UInt<100> = 0x1_0000_0000;\nconst LEN: UInt<70> = 3;";
    let ir = SourceElaboration::ir(&ElaborationForms::with_params(items, "a: f16 = twice(H); b: UInt<100> = square(W); c: [f32; LEN as u64] = [1.0, 2.0, 3.0];"), "Top");
    assert_eq!(IrReading::default_of(&ir, "a"), Constant::FloatBits(u128::from((<Float16 as Sample>::decimal("1.5") + <Float16 as Sample>::decimal("1.5")).to_bits())));
    let Constant::Wide(id) = IrReading::default_of(&ir, "b") else { panic!("a wide default") };
    assert_eq!(ir.wide(id).limbs, vec![0, 1], "2^32 squared is 2^64");
    let top = ir.bodies().find(|body| body.name() == "Top").expect("Top");
    let array = top.params().iter().find(|param| param.name == "c").expect("c");
    assert_eq!(array.ty, ValueType::Array(Box::new(ValueType::F32), 3));
}

#[test]
fn a_const_panic_of_a_new_width_names_the_expression() {
    // CV-36 AC 2, §1 rule 11: a wide integer `/` or `%` by zero, a `rem_euclid` by zero and a `clamp` with its bounds crossed (or
    // a NaN bound) are `E0104` at elaboration naming what panicked
    for (params, named) in [
        ("p: UInt<200> = 7 / (3 - 3);", "an integer division by zero"),
        ("p: Int<100> = 7 % (3 - 3);", "an integer remainder by zero"),
        ("p: UInt<70> = 7u70.rem_euclid(3u70 - 3u70);", "a `rem_euclid` by zero"),
        ("p: UInt<70> = 7u70.clamp(9u70, 2u70);", "a `clamp` whose lower bound is above its upper bound"),
        ("p: f32 = ONE.clamp(TWO, ONE);", "a `clamp` whose bounds are crossed or NaN"),
        ("p: f16 = ONE.clamp(TWO, ONE);", "a `clamp` whose bounds are crossed or NaN"),
        ("p: f128 = ONE.clamp(TWO, ONE);", "a `clamp` whose bounds are crossed or NaN"),
        ("p: UInt<70> = 1 << (0 - 1i8);", "a shift by a negative amount"),
    ] {
        let ty = if params.contains("f32") { "f32" } else if params.contains("f16") { "f16" } else { "f128" };
        let items = if params.contains("ONE") { format!("const ONE: {ty} = 1.0; const TWO: {ty} = 2.0;") } else { String::new() };
        let src = ElaborationForms::with_params(&items, params);
        let found = SourceElaboration::run(&src, "Top").expect_err(params);
        assert!(found.iter().any(|d| d.diagnostic.code == "E0104" && d.diagnostic.message.contains(named)), "{params}: {:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
}
