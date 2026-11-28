//! The number widths in the transpiler (composite-values phase D, CV-29 to CV-32; MD-63 item 1):
//! the runtime a design gets (`Runtime::files_for`), and the generated code of `f32`, `f16`,
//! `f128` and integers wider than 64 bits compiled by `rustc` and run. The oracle for a result is
//! Rust's own `f32` arithmetic evaluated in the test and `piperine-numbers` called in the test.

use piperine_ir_fixtures::digital;
use piperine_ir_fixtures::alone;

use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};
use std::process::Command;

use alone::Alone;
use digital::Dx;
use piperine_ir::{BinaryOp, Expr, ExprId, FunctionSpec, Ir, IrBuilder, Local, MathFn, Method, Origin, UnaryOp, ValueType, WideData};
use piperine_transpile::print::ExpressionPrinter;
use piperine_transpile::rt::int::{Int, UInt};
use piperine_transpile::rt::numbers::half::Float16;
use piperine_transpile::rt::numbers::quad::Float128;
use piperine_transpile::rt::numbers::wide::{WideInt, WideUInt};
use piperine_transpile::rt::widths::{Convert, FloatRepr};
use piperine_transpile::rt::{NUMBER_FILES, Runtime, RuntimeForms};
use piperine_transpile::Rustc;

/// The helpers of this target.
struct Files;

impl Files {
    /// The paths of runtime files.
    fn paths(files: Vec<(&'static str, &'static str)>) -> Vec<&'static str> {
        files.into_iter().map(|(path, _)| path).collect()
    }
}

#[test]
fn a_design_without_number_widths_gets_todays_runtime_files() {
    // MD-63 item 1 and the cache keys: a design that uses none of the new types gets exactly the files it had,
    // whatever the switch `numbers: false` says, so no existing cache entry or golden moves
    assert_eq!(Files::paths(Runtime::files(false)), ["rt.rs"], "an analog-only design has `real.rs` alone as `rt.rs`");
    assert_eq!(
        Files::paths(Runtime::files(true)),
        ["rt.rs", "rt/real.rs", "rt/int.rs", "rt/pin.rs", "rt/control.rs", "rt/commits.rs", "rt/array.rs", "rt/table.rs"],
        "a digital design gets the files of `rt`"
    );
    for digital in [false, true] {
        assert_eq!(Runtime::files_for(RuntimeForms { digital, numbers: false }), Runtime::files(digital), "digital: {digital}");
    }

    // a design that uses a number width gets those files, then `rt/numbers.rs`, every library file and `rt/widths.rs`,
    // and a root that re-exports the types the printer spells
    let with = Runtime::files_for(RuntimeForms { digital: false, numbers: true });
    let with_paths = Files::paths(with.clone());
    assert_eq!(with_paths[1..8], Files::paths(Runtime::files(true))[1..], "the digital files follow the root unchanged");
    assert!(with_paths.contains(&"rt/numbers.rs") && with_paths.contains(&"rt/widths.rs"));
    for (path, text) in NUMBER_FILES {
        assert!(with.iter().any(|(file, body)| *file == path && *body == text), "{path} is embedded verbatim");
    }
    let root = with[0].1;
    for spelled in ["pub use numbers::half::Float16;", "pub use numbers::quad::Float128;", "pub use numbers::wide::{WideInt, WideUInt};", "pub use widths::{Convert, FloatRepr, IntegerRepr};"] {
        assert!(root.contains(spelled), "the root re-exports `{spelled}`:\n{root}");
    }
    // the library's own files are what the leaf crate holds on disk: verbatim, not a copy that can drift
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../piperine-numbers/src");
    for (path, text) in NUMBER_FILES {
        let name = path.trim_start_matches("rt/numbers/");
        assert_eq!(std::fs::read_to_string(directory.join(name)).expect("a library file"), text, "{name}");
    }
}

// ---- T116: the printed code, compiled by rustc and run ------------------------------------------------------------

/// The root of the probe's runtime: the files that need no ABI crate, and the numeric library.
const PROBE_ROOT: &str = "\
pub mod control;
pub mod int;
pub mod pin;
pub mod real;
pub mod numbers;
pub mod widths;

#[allow(unused_imports)]
pub use control::{Depth, Index, Panic, Trips};
#[allow(unused_imports)]
pub use pin::{Carried, LevelWords};
#[allow(unused_imports)]
pub use int::{Int, UInt, Unwrap};
#[allow(unused_imports)]
pub use real::{Dual, Real};
#[allow(unused_imports)]
pub use numbers::half::Float16;
#[allow(unused_imports)]
pub use numbers::quad::Float128;
#[allow(unused_imports)]
pub use numbers::wide::{WideInt, WideUInt};
#[allow(unused_imports)]
pub use widths::{Convert, FloatRepr, IntegerRepr};
";

/// A program built from printed functions and a `main`, compiled by `rustc` against the numeric runtime.
struct NumberProbe;

impl NumberProbe {
    /// What `main` writes to standard output; a failed build or run is a panic naming the standard error.
    fn run(functions: &[String], main: &str) -> String {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("rt/numbers")).expect("rt dirs");
        std::fs::write(root.join("rt.rs"), PROBE_ROOT).expect("rt.rs");
        for (path, text) in Runtime::files_for(RuntimeForms { digital: false, numbers: true }) {
            if !matches!(path, "rt.rs" | "rt/commits.rs" | "rt/array.rs" | "rt/table.rs") {
                std::fs::write(root.join(path), text).expect("a runtime file");
            }
        }
        let program = format!("mod rt;\n#[allow(unused_imports)]\nuse rt::Real;\n\n{}\n\n{main}\n", functions.join("\n\n"));
        std::fs::write(root.join("main.rs"), &program).expect("main.rs");
        let compiler = std::env::var_os("PIPERINE_RUSTC").unwrap_or_else(|| "rustc".into());
        let built = Command::new(compiler)
            .args(Rustc::FLAGS)
            .args(["--crate-type", "bin", "--crate-name", "probe", "-A", "warnings"])
            .arg(root.join("main.rs"))
            .arg("-o")
            .arg(root.join("probe"))
            .output()
            .expect("rustc runs");
        assert!(built.status.success(), "rustc failed:\n{}\n--- source ---\n{program}", String::from_utf8_lossy(&built.stderr));
        let ran = Command::new(root.join("probe")).output().expect("the program runs");
        assert!(ran.status.success(), "the program failed: {}", String::from_utf8_lossy(&ran.stderr));
        String::from_utf8_lossy(&ran.stdout).into_owned()
    }
}

/// A float type of the printer and the oracle that computes what its code must compute: Rust's own
/// `f32`, or `piperine-numbers` called directly in the test (each call is the library's own
/// function, so a difference is the printer's).
trait Sample: Copy + FloatRepr + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self> + PartialOrd {
    /// The width in bits.
    const BITS: u32;
    /// The IR type.
    fn ir_type() -> ValueType;
    /// The Rust spelling of the type in generated code.
    fn spelled() -> &'static str;
    /// The bit pattern.
    fn bits(self) -> u128;
    /// The nearest value to `x`, rounded once.
    fn of(x: f64) -> Self {
        Convert::float_to_float::<f64, Self>(x)
    }
    /// `self.min(other)`.
    fn minimum(self, other: Self) -> Self;
    /// `self.max(other)`.
    fn maximum(self, other: Self) -> Self;
    /// `self.floor()`.
    fn floor(self) -> Self;
    /// `self.sin()`.
    fn sin(self) -> Self;
    /// `self.sqrt()`.
    fn sqrt(self) -> Self;
    /// `self.exp()`.
    fn exp(self) -> Self;
    /// `self.abs()`.
    fn abs(self) -> Self;
    /// `self.powi(n)`.
    fn powi(self, n: i32) -> Self;
    /// `self.atan2(x)`.
    fn atan2(self, x: Self) -> Self;
    /// `self.is_nan()`.
    fn is_nan(self) -> bool;
    /// `self.clamp(lo, hi)`.
    fn clamp(self, lo: Self, hi: Self) -> Self;
    /// The quiet NaN.
    fn nan() -> Self;
}

impl Sample for f32 {
    const BITS: u32 = 32;
    fn ir_type() -> ValueType { ValueType::F32 }
    fn spelled() -> &'static str { "f32" }
    fn bits(self) -> u128 { u128::from(self.to_bits()) }
    fn minimum(self, other: Self) -> Self { self.min(other) }
    fn maximum(self, other: Self) -> Self { self.max(other) }
    fn floor(self) -> Self { f32::floor(self) }
    fn sin(self) -> Self { f32::sin(self) }
    fn sqrt(self) -> Self { f32::sqrt(self) }
    fn exp(self) -> Self { f32::exp(self) }
    fn abs(self) -> Self { f32::abs(self) }
    fn powi(self, n: i32) -> Self { f32::powi(self, n) }
    fn atan2(self, x: Self) -> Self { f32::atan2(self, x) }
    fn is_nan(self) -> bool { f32::is_nan(self) }
    fn clamp(self, lo: Self, hi: Self) -> Self { f32::clamp(self, lo, hi) }
    fn nan() -> Self { f32::NAN }
}

impl Sample for Float16 {
    const BITS: u32 = 16;
    fn ir_type() -> ValueType { ValueType::F16 }
    fn spelled() -> &'static str { "rt::Float16" }
    fn bits(self) -> u128 { u128::from(self.to_bits()) }
    fn minimum(self, other: Self) -> Self { self.min(other) }
    fn maximum(self, other: Self) -> Self { self.max(other) }
    fn floor(self) -> Self { Float16::floor(self) }
    fn sin(self) -> Self { Float16::sin(self) }
    fn sqrt(self) -> Self { Float16::sqrt(self) }
    fn exp(self) -> Self { Float16::exp(self) }
    fn abs(self) -> Self { Float16::abs(self) }
    fn powi(self, n: i32) -> Self { Float16::powi(self, n) }
    fn atan2(self, x: Self) -> Self { Float16::atan2(self, x) }
    fn is_nan(self) -> bool { Float16::is_nan(self) }
    fn clamp(self, lo: Self, hi: Self) -> Self { Float16::clamp(self, lo, hi) }
    fn nan() -> Self { Float16::NAN }
}

impl Sample for Float128 {
    const BITS: u32 = 128;
    fn ir_type() -> ValueType { ValueType::F128 }
    fn spelled() -> &'static str { "rt::Float128" }
    fn bits(self) -> u128 { self.to_bits() }
    fn minimum(self, other: Self) -> Self { self.min(other) }
    fn maximum(self, other: Self) -> Self { self.max(other) }
    fn floor(self) -> Self { Float128::floor(self) }
    fn sin(self) -> Self { Float128::sin(self) }
    fn sqrt(self) -> Self { Float128::sqrt(self) }
    fn exp(self) -> Self { Float128::exp(self) }
    fn abs(self) -> Self { Float128::abs(self) }
    fn powi(self, n: i32) -> Self { Float128::powi(self, n) }
    fn atan2(self, x: Self) -> Self { Float128::atan2(self, x) }
    fn is_nan(self) -> bool { Float128::is_nan(self) }
    fn clamp(self, lo: Self, hi: Self) -> Self { Float128::clamp(self, lo, hi) }
    fn nan() -> Self { Float128::NAN }
}

/// An integer type wider than 64 bits, and the oracle of what its printed code computes: the
/// library called directly in the test.
trait WideSample: Copy + Ord + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Rem<Output = Self> + BitAnd<Output = Self> + BitOr<Output = Self> + BitXor<Output = Self> + Neg<Output = Self> + Not<Output = Self> + Shl<UInt<8>, Output = Self> + Shr<UInt<8>, Output = Self> + piperine_transpile::rt::widths::IntegerRepr {
    /// The IR type.
    fn ir_type() -> ValueType;
    /// The Rust spelling of the type in generated code.
    fn spelled() -> &'static str;
    /// The limbs, low first, of the value `seed` builds (two's complement at the width).
    fn limbs(seed: u64) -> Vec<u64>;
    /// The value of those limbs.
    fn of(limbs: &[u64]) -> Self;
    /// The exact decimal text.
    fn decimal(self) -> String;
    /// `self.pow(e)`.
    fn power(self, exponent: u32) -> Self;
    /// `self.rem_euclid(d)`.
    fn euclid(self, divisor: Self) -> Self;
    /// `self.saturating_add(other)`.
    fn saturated(self, other: Self) -> Self;
    /// `self.add_overflows(other)`.
    fn overflows(self, other: Self) -> bool;
    /// `self.clamp(lo, hi)`.
    fn clamped(self, lo: Self, hi: Self) -> Self;
}

impl WideSample for WideUInt<200, 4> {
    fn ir_type() -> ValueType { ValueType::UInt(200) }
    fn spelled() -> &'static str { "rt::WideUInt::<200, 4>" }
    fn limbs(seed: u64) -> Vec<u64> { vec![0x1234_5678_9abc_def0 ^ seed, 0xfedc_ba98_7654_3210, 0x0f0f_0f0f_0f0f_0f0f, seed & 0xff] }
    fn of(limbs: &[u64]) -> Self { WideUInt::from_limbs(limbs) }
    fn decimal(self) -> String { self.to_decimal_string() }
    fn power(self, exponent: u32) -> Self { self.pow(exponent) }
    fn euclid(self, divisor: Self) -> Self { self.rem_euclid(divisor) }
    fn saturated(self, other: Self) -> Self { self.saturating_add(other) }
    fn overflows(self, other: Self) -> bool { self.add_overflows(other) }
    fn clamped(self, lo: Self, hi: Self) -> Self { self.clamp(lo, hi) }
}

impl WideSample for WideInt<100, 2> {
    fn ir_type() -> ValueType { ValueType::Int(100) }
    fn spelled() -> &'static str { "rt::WideInt::<100, 2>" }
    fn limbs(seed: u64) -> Vec<u64> { vec![0xdead_beef_0bad_f00d ^ seed, 0x0000_0007_bcde_f012] }
    fn of(limbs: &[u64]) -> Self { WideInt::from_limbs(limbs) }
    fn decimal(self) -> String { self.to_decimal_string() }
    fn power(self, exponent: u32) -> Self { self.pow(exponent) }
    fn euclid(self, divisor: Self) -> Self { self.rem_euclid(divisor) }
    fn saturated(self, other: Self) -> Self { self.saturating_add(other) }
    fn overflows(self, other: Self) -> bool { self.add_overflows(other) }
    fn clamped(self, lo: Self, hi: Self) -> Self { self.clamp(lo, hi) }
}

impl WideSample for WideUInt<65, 2> {
    fn ir_type() -> ValueType { ValueType::UInt(65) }
    fn spelled() -> &'static str { "rt::WideUInt::<65, 2>" }
    fn limbs(seed: u64) -> Vec<u64> { vec![0xffff_ffff_ffff_fff0 ^ seed, 1] }
    fn of(limbs: &[u64]) -> Self { WideUInt::from_limbs(limbs) }
    fn decimal(self) -> String { self.to_decimal_string() }
    fn power(self, exponent: u32) -> Self { self.pow(exponent) }
    fn euclid(self, divisor: Self) -> Self { self.rem_euclid(divisor) }
    fn saturated(self, other: Self) -> Self { self.saturating_add(other) }
    fn overflows(self, other: Self) -> bool { self.add_overflows(other) }
    fn clamped(self, lo: Self, hi: Self) -> Self { self.clamp(lo, hi) }
}

/// The functions of one design, built by hand, and the lines of the `main` that calls them with the
/// text each call must print.
struct Library {
    builder: IrBuilder,
    main: Vec<String>,
    expected: Vec<String>,
}

impl Library {
    const FILE: &'static str = "number_widths.msl";

    fn new() -> Library {
        Library { builder: IrBuilder::new(), main: Vec::new(), expected: Vec::new() }
    }

    fn origin() -> Origin {
        Origin { file: Self::FILE.into(), line: 1, col: 1 }
    }

    /// `fn name(a: p0, b: p1, ...) -> ret { <build> }`, the body built from the param reads.
    fn function(&mut self, name: &str, params: &[ValueType], ret: ValueType, build: impl FnOnce(&mut Dx, &[ExprId]) -> ExprId) {
        let locals: Vec<Local> = params.iter().enumerate().map(|(i, ty)| Local { name: format!("p{i}"), ty: ty.clone(), mutable: false }).collect();
        let mut x = Dx::new(&mut self.builder, Self::FILE, 1, 1);
        let reads: Vec<ExprId> = params.iter().enumerate().map(|(i, ty)| x.local(i as u32, ty.clone())).collect();
        let body = build(&mut x, &reads);
        self.builder.function(FunctionSpec { name: name.into(), origin: Self::origin(), params: locals.clone(), ret, locals, body });
    }

    /// A `main` line printing `call` (whose type is `ret`) as `shown`, and the text it must print.
    fn expect(&mut self, call: &str, ret: &ValueType, expected: String) {
        let shown = match ret {
            ValueType::F32 | ValueType::F16 | ValueType::F128 => format!("({call}).to_bits()"),
            ValueType::F64 => format!("({call}).value().to_bits()"),
            ValueType::Bool => format!("({call})"),
            ValueType::UInt(n) | ValueType::Int(n) if *n > 64 => format!("({call}).to_decimal_string()"),
            _ => format!("({call}).get()"),
        };
        self.main.push(format!("    println!(\"{{}}\", {shown});"));
        self.expected.push(expected);
    }

    /// The printed text of every function, and the `main` that runs the calls.
    fn program(self) -> (Vec<String>, String, Vec<String>) {
        let ir: Ir = Alone::finish(self.builder);
        let texts = ir.functions().map(|f| ExpressionPrinter::function(&ir, f.id()).unwrap_or_else(|e| panic!("{e}"))).collect();
        (texts, format!("fn main() {{\n{}\n}}", self.main.join("\n")), self.expected)
    }

    /// Builds and runs the program, comparing every printed line with the oracle's.
    fn check(self) {
        let (texts, main, expected) = self.program();
        let out = NumberProbe::run(&texts, &main);
        let got: Vec<&str> = out.lines().collect();
        assert_eq!(got.len(), expected.len(), "one line per call:\n{out}\n{main}");
        for (i, (g, e)) in got.iter().zip(&expected).enumerate() {
            assert_eq!(g, e, "call {i}: {}", Self::main_line(&main, i));
        }
    }

    /// The `i`th call line of `main`, for a failure message.
    fn main_line(main: &str, i: usize) -> String {
        main.lines().nth(i + 1).unwrap_or_default().to_string()
    }

    /// The Rust text of the value of `limbs` of the wide type `T`.
    fn wide_text<T: WideSample>(limbs: &[u64]) -> String {
        let list: Vec<String> = limbs.iter().map(|limb| format!("0x{limb:x}_u64")).collect();
        format!("{}::from_limbs(&[{}])", T::spelled(), list.join(", "))
    }

    /// Operators, shifts, comparisons, methods, literals and `as` of one wide integer type.
    fn wide_cases<T: WideSample>(&mut self, prefix: &str) {
        let ty = T::ir_type();
        let (la, lb) = (T::limbs(0x55), T::limbs(0x3));
        let (a, b) = (T::of(&la), T::of(&lb));
        let divisor = T::of(&[0x1_0000_0001, 0]);
        let (at, bt, dt) = (Self::wide_text::<T>(&la), Self::wide_text::<T>(&lb), Self::wide_text::<T>(&[0x1_0000_0001, 0]));
        let operators = [
            ("add", BinaryOp::Add, a + b, bt.clone()),
            ("sub", BinaryOp::Sub, a - b, bt.clone()),
            ("mul", BinaryOp::Mul, a * b, bt.clone()),
            ("div", BinaryOp::Div, a / divisor, dt.clone()),
            ("rem", BinaryOp::Rem, a % divisor, dt.clone()),
            ("and", BinaryOp::BitAnd, a & b, bt.clone()),
            ("or", BinaryOp::BitOr, a | b, bt.clone()),
            ("xor", BinaryOp::BitXor, a ^ b, bt.clone()),
        ];
        for (name, op, want, rhs) in operators {
            let ty_a = ty.clone();
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone(), ty_a.clone()], ty_a.clone(), |x, p| x.bin(op, p[0], p[1], ty_a));
            self.expect(&format!("{prefix}_{name}({at}, {rhs})"), &ty, want.decimal());
        }
        for (name, op, want) in [("lt", BinaryOp::Lt, a < b), ("ge", BinaryOp::Ge, a >= b), ("eq", BinaryOp::Eq, a == b), ("ne", BinaryOp::Ne, a != b)] {
            let ty_a = ty.clone();
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone(), ty_a], ValueType::Bool, |x, p| x.bin(op, p[0], p[1], ValueType::Bool));
            self.expect(&format!("{prefix}_{name}({at}, {bt})"), &ValueType::Bool, want.to_string());
        }
        for (name, op, want) in [("neg", UnaryOp::Neg, -a), ("not", UnaryOp::Not, !a)] {
            let ty_a = ty.clone();
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone()], ty_a.clone(), |x, p| x.un(op, p[0], ty_a));
            self.expect(&format!("{prefix}_{name}({at})"), &ty, want.decimal());
        }
        // shifts: by an `rt::UInt<8>`, and by a wide integer (§1.4.3)
        for (name, op, want) in [("shl", BinaryOp::Shl, a << UInt::<8>::new(70)), ("shr", BinaryOp::Shr, a >> UInt::<8>::new(70))] {
            let ty_a = ty.clone();
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone(), ValueType::UInt(8)], ty_a.clone(), |x, p| x.bin(op, p[0], p[1], ty_a));
            self.expect(&format!("{prefix}_{name}({at}, rt::UInt::<8>::new(70))"), &ty, want.decimal());
            let (ty_a, amount) = (ty.clone(), ValueType::UInt(70));
            self.function(&format!("{prefix}_{name}_wide"), &[ty_a.clone(), amount], ty_a.clone(), |x, p| x.bin(op, p[0], p[1], ty_a));
            self.expect(&format!("{prefix}_{name}_wide({at}, rt::WideUInt::<70, 2>::from_u64(70))"), &ty, want.decimal());
        }
        for (name, method, want) in [("min", Method::Min, a.min(b)), ("max", Method::Max, a.max(b)), ("rem_euclid", Method::RemEuclid, a.euclid(divisor)), ("saturating_add", Method::SaturatingAdd, a.saturated(b))] {
            let ty_a = ty.clone();
            let rhs = if name == "rem_euclid" { &dt } else { &bt };
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone(), ty_a.clone()], ty_a.clone(), |x, p| x.method(method, p[0], vec![p[1]], ty_a));
            self.expect(&format!("{prefix}_{name}({at}, {rhs})"), &ty, want.decimal());
        }
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_add_overflows"), &[ty_a.clone(), ty_a.clone()], ValueType::Bool, |x, p| x.method(Method::AddOverflows, p[0], vec![p[1]], ValueType::Bool));
        self.expect(&format!("{prefix}_add_overflows({at}, {bt})"), &ValueType::Bool, a.overflows(b).to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_pow"), &[ty_a.clone(), ValueType::UInt(32)], ty_a.clone(), |x, p| x.method(Method::Pow, p[0], vec![p[1]], ty_a));
        self.expect(&format!("{prefix}_pow({at}, rt::UInt::<32>::new(3))"), &ty, a.power(3).decimal());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_clamp"), &[ty_a.clone(), ty_a.clone(), ty_a.clone()], ty_a.clone(), |x, p| x.method(Method::Clamp, p[0], vec![p[1], p[2]], ty_a));
        self.expect(&format!("{prefix}_clamp({at}, {dt}, {bt})"), &ty, if divisor <= b { a.clamped(divisor, b) } else { a.clamped(b, divisor) }.decimal());
        // §1 rule 11: a zero divisor panics naming the operation; the printed `/` raises it
        self.main.push(format!("    println!(\"{{}}\", std::panic::catch_unwind(|| {prefix}_div({at}, {})).err().and_then(|p| p.downcast_ref::<String>().cloned()).is_some_and(|m| m.contains(\"integer division by zero\")));", Self::wide_text::<T>(&[0])));
        self.expected.push("true".to_string());
        // literals: `Expr::Wide` holds the limbs, `Expr::Int` a small value
        let (ty_a, id) = (ty.clone(), self.builder.wide(WideData { limbs: la.clone() }));
        self.function(&format!("{prefix}_literal"), &[], ty_a.clone(), |x, _| {
            let origin = x.origin();
            x.b.expr(Expr::Wide(id), ty_a, origin)
        });
        self.expect(&format!("{prefix}_literal()"), &ty, a.decimal());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_small"), &[], ty_a.clone(), |x, _| x.int(1234, ty_a));
        self.expect(&format!("{prefix}_small()"), &ty, T::of(&[1234, 0, 0, 0]).decimal());
        self.wide_casts::<T>(prefix, &at);
    }

    /// Every `as` of a wide integer type: to and from narrow integers, `bool`, other wide integers and floats.
    fn wide_casts<T: WideSample>(&mut self, prefix: &str, at: &str) {
        let (ty, a) = (T::ir_type(), T::of(&T::limbs(0x55)));
        let to_u8 = (ValueType::UInt(8), Convert::integer_to_integer::<T, UInt<8>>(a).get().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_to_u8"), &[ty_a.clone()], to_u8.0.clone(), |x, p| x.cast(p[0], ValueType::UInt(8), ValueType::UInt(8)));
        self.expect(&format!("{prefix}_to_u8({at})"), &to_u8.0, to_u8.1);
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_from_i8"), &[ValueType::Int(8)], ty_a.clone(), |x, p| x.cast(p[0], ty_a.clone(), ty_a));
        self.expect(&format!("{prefix}_from_i8(rt::Int::<8>::new(-3))"), &ty, Convert::integer_to_integer::<Int<8>, T>(Int::new(-3)).decimal());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_from_bool"), &[ValueType::Bool], ty_a.clone(), |x, p| x.cast(p[0], ty_a.clone(), ty_a));
        self.expect(&format!("{prefix}_from_bool(true)"), &ty, T::of(&[1, 0, 0, 0]).decimal());
        let (other, ty_a) = (ValueType::Int(130), ty.clone());
        self.function(&format!("{prefix}_to_i130"), &[ty_a.clone()], other.clone(), |x, p| x.cast(p[0], ValueType::Int(130), ValueType::Int(130)));
        self.expect(&format!("{prefix}_to_i130({at})"), &other, Convert::integer_to_integer::<T, WideInt<130, 3>>(a).to_decimal_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_to_f32"), &[ty_a.clone()], ValueType::F32, |x, p| x.cast(p[0], ValueType::F32, ValueType::F32));
        self.expect(&format!("{prefix}_to_f32({at})"), &ValueType::F32, Convert::integer_to_float::<T, f32>(a).bits().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_to_f128"), &[ty_a.clone()], ValueType::F128, |x, p| x.cast(p[0], ValueType::F128, ValueType::F128));
        self.expect(&format!("{prefix}_to_f128({at})"), &ValueType::F128, Convert::integer_to_float::<T, Float128>(a).bits().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_to_f64"), &[ty_a.clone()], ValueType::F64, |x, p| x.cast(p[0], ValueType::F64, ValueType::F64));
        self.expect(&format!("{prefix}_to_f64::<f64>({at})"), &ValueType::F64, Convert::integer_to_float::<T, f64>(a).to_bits().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_from_f64"), &[ValueType::F64], ty_a.clone(), |x, p| x.cast(p[0], ty_a.clone(), ty_a));
        self.expect(&format!("{prefix}_from_f64(1.5e30_f64)"), &ty, Convert::float_to_integer::<f64, T>(1.5e30).decimal());
    }

    /// A float literal of type `ty` holding `bits`.
    fn float_bits(x: &mut Dx, bits: u128, ty: ValueType) -> ExprId {
        let origin = x.origin();
        x.b.expr(Expr::FloatBits(bits), ty, origin)
    }

    /// The operations of every float type: operators, comparisons, `-`, methods, math functions, the literal and every `as`.
    fn float_cases<T: Sample>(&mut self) {
        let (ty, rust) = (T::ir_type(), T::spelled());
        let (a, b, c) = (T::of(2.5), T::of(0.1), T::of(-3.75));
        let text = |v: T| format!("{rust}::from_bits({}_u{})", v.bits(), T::BITS);
        let prefix = rust.replace("rt::", "").to_lowercase();
        let (at, bt, ct) = (text(a), text(b), text(c));
        for (name, op, want) in [("add", BinaryOp::Add, a + b), ("sub", BinaryOp::Sub, a - b), ("mul", BinaryOp::Mul, a * b), ("div", BinaryOp::Div, a / b)] {
            let (ty_a, ty_b) = (ty.clone(), ty.clone());
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone(), ty_b], ty_a.clone(), |x, p| x.bin(op, p[0], p[1], ty_a));
            self.expect(&format!("{prefix}_{name}({at}, {bt})"), &ty, want.bits().to_string());
        }
        for (name, op, want) in [("lt", BinaryOp::Lt, a < b), ("ge", BinaryOp::Ge, a >= b), ("eq", BinaryOp::Eq, a == b), ("ne", BinaryOp::Ne, a != b)] {
            let ty_a = ty.clone();
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone(), ty_a], ValueType::Bool, |x, p| x.bin(op, p[0], p[1], ValueType::Bool));
            self.expect(&format!("{prefix}_{name}({at}, {bt})"), &ValueType::Bool, want.to_string());
        }
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_neg"), &[ty_a.clone()], ty_a.clone(), |x, p| x.un(UnaryOp::Neg, p[0], ty_a));
        self.expect(&format!("{prefix}_neg({at})"), &ty, (-a).bits().to_string());
        for (name, method, want) in [("min", Method::Min, a.minimum(b)), ("max", Method::Max, a.maximum(b)), ("atan2", Method::Atan2, a.atan2(b))] {
            let ty_a = ty.clone();
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone(), ty_a.clone()], ty_a.clone(), |x, p| x.method(method, p[0], vec![p[1]], ty_a));
            self.expect(&format!("{prefix}_{name}({at}, {bt})"), &ty, want.bits().to_string());
        }
        for (name, method, want) in [("floor", Method::Floor, c.floor()), ("sin", Method::Sin, c.sin())] {
            let ty_a = ty.clone();
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone()], ty_a.clone(), |x, p| x.method(method, p[0], vec![], ty_a));
            self.expect(&format!("{prefix}_{name}({ct})"), &ty, want.bits().to_string());
        }
        for (name, f, want) in [("sqrt", MathFn::Sqrt, a.sqrt()), ("exp", MathFn::Exp, a.exp()), ("abs", MathFn::Abs, c.abs())] {
            let ty_a = ty.clone();
            let arg = if name == "abs" { &ct } else { &at };
            self.function(&format!("{prefix}_{name}"), &[ty_a.clone()], ty_a.clone(), |x, p| x.math(f, p[0], ty_a));
            self.expect(&format!("{prefix}_{name}({arg})"), &ty, want.bits().to_string());
        }
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_powi"), &[ty_a.clone(), ValueType::Int(32)], ty_a.clone(), |x, p| x.method(Method::Powi, p[0], vec![p[1]], ty_a));
        self.expect(&format!("{prefix}_powi({at}, rt::Int::<32>::new(3))"), &ty, a.powi(3).bits().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_clamp"), &[ty_a.clone(), ty_a.clone(), ty_a.clone()], ty_a.clone(), |x, p| x.method(Method::Clamp, p[0], vec![p[1], p[2]], ty_a));
        self.expect(&format!("{prefix}_clamp({ct}, {bt}, {at})"), &ty, c.clamp(b, a).bits().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_is_nan"), &[ty_a.clone()], ValueType::Bool, |x, p| x.method(Method::IsNan, p[0], vec![], ValueType::Bool));
        self.expect(&format!("{prefix}_is_nan({})", text(T::nan())), &ValueType::Bool, T::nan().is_nan().to_string());
        self.expect(&format!("{prefix}_is_nan({at})"), &ValueType::Bool, a.is_nan().to_string());
        // a literal of the position's type: `FloatBits` is read by it
        let (ty_a, bits) = (ty.clone(), b.bits());
        self.function(&format!("{prefix}_literal"), &[], ty_a.clone(), |x, _| Self::float_bits(x, bits, ty_a));
        self.expect(&format!("{prefix}_literal()"), &ty, b.bits().to_string());
        self.float_casts::<T>(&prefix, &at, &ct);
    }

    /// Every `as` of a float type: to and from `f64`, an integer of each width, and the other floats.
    fn float_casts<T: Sample>(&mut self, prefix: &str, at: &str, ct: &str) {
        let (ty, a, c) = (T::ir_type(), T::of(2.5), T::of(-3.75));
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_to_f64"), &[ty_a.clone()], ValueType::F64, |x, p| x.cast(p[0], ValueType::F64, ValueType::F64));
        self.expect(&format!("{prefix}_to_f64::<f64>({at})"), &ValueType::F64, Convert::float_to_float::<T, f64>(a).to_bits().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_from_f64"), &[ValueType::F64], ty_a.clone(), |x, p| x.cast(p[0], ty_a.clone(), ty_a));
        self.expect(&format!("{prefix}_from_f64(0.1_f64)"), &ty, T::of(0.1).bits().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_to_u8"), &[ty_a.clone()], ValueType::UInt(8), |x, p| x.cast(p[0], ValueType::UInt(8), ValueType::UInt(8)));
        self.expect(&format!("{prefix}_to_u8({at})"), &ValueType::UInt(8), Convert::float_to_integer::<T, UInt<8>>(a).get().to_string());
        self.expect(&format!("{prefix}_to_u8({ct})"), &ValueType::UInt(8), Convert::float_to_integer::<T, UInt<8>>(c).get().to_string());
        let ty_a = ty.clone();
        self.function(&format!("{prefix}_from_i32"), &[ValueType::Int(32)], ty_a.clone(), |x, p| x.cast(p[0], ty_a.clone(), ty_a));
        self.expect(&format!("{prefix}_from_i32(rt::Int::<32>::new(-7))"), &ty, Convert::integer_to_float::<Int<32>, T>(Int::new(-7)).bits().to_string());
        let (wide_ty, ty_a) = (ValueType::UInt(200), ty.clone());
        self.function(&format!("{prefix}_to_u200"), &[ty_a.clone()], wide_ty.clone(), |x, p| x.cast(p[0], wide_ty.clone(), wide_ty));
        self.expect(&format!("{prefix}_to_u200({at})"), &ValueType::UInt(200), Convert::float_to_integer::<T, WideUInt<200, 4>>(a).to_decimal_string());
        let (wide_ty, ty_a) = (ValueType::Int(100), ty.clone());
        self.function(&format!("{prefix}_from_i100"), &[wide_ty.clone()], ty_a.clone(), |x, p| x.cast(p[0], ty_a.clone(), ty_a));
        self.expect(&format!("{prefix}_from_i100(rt::WideInt::<100, 2>::from_i128(-123456789012345678))"), &ty, Convert::integer_to_float::<WideInt<100, 2>, T>(WideInt::from_i128(-123456789012345678)).bits().to_string());
        self.float_pair::<T, f32>(prefix, at, a);
        self.float_pair::<T, Float16>(prefix, at, a);
        self.float_pair::<T, Float128>(prefix, at, a);
    }

    /// `as` from `T` to `U`, `U` being another float type.
    fn float_pair<T: Sample, U: Sample>(&mut self, prefix: &str, at: &str, a: T) {
        if T::ir_type() == U::ir_type() {
            return;
        }
        let (from, to) = (T::ir_type(), U::ir_type());
        let name = format!("{prefix}_to_{}", U::spelled().replace("rt::", "").to_lowercase());
        let to_a = to.clone();
        self.function(&name, &[from], to.clone(), |x, p| x.cast(p[0], to_a.clone(), to_a));
        self.expect(&format!("{name}({at})"), &to, Convert::float_to_float::<T, U>(a).bits().to_string());
    }
}

#[test]
fn number_widths_compile_and_compute_as_their_library() {
    // CV-29, CV-30, CV-31 AC 1: the printed `f32` code computes what Rust's `f32` computes, the printed `f16` and `f128` code what the library does
    let mut library = Library::new();
    library.float_cases::<f32>();
    library.float_cases::<Float16>();
    library.float_cases::<Float128>();
    // CV-32 AC 1: every operator, shift, `as` and method of a wide integer wraps modulo 2^N as the library does
    library.wide_cases::<WideUInt<200, 4>>("u200");
    library.wide_cases::<WideInt<100, 2>>("i100");
    library.wide_cases::<WideUInt<65, 2>>("u65");
    library.check();
}
