//! Scope: value code of `piperine_lang::elab` for msl-digital (DIG-08): integer types, literals,
//! operators, casts and methods lowered to the IR's own nodes, and integer constant expressions
//! folded at elaboration by the rules the compiled code follows (§1 rule 6, ROADMAP G4 and G5),
//! compared with the compiled code's own integers (`piperine_transpile::rt`).

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{BinaryOp, Expr, ExprId, Ir, Method, ValueType};
use piperine_transpile::rt::int::{Int, UInt};
use support::Elab;

/// Reading the elaborated IR of a test.
struct Read<'a> {
    ir: &'a Ir,
}

impl<'a> Read<'a> {
    /// The body expression of the function named `name`.
    fn body_of(&self, name: &str) -> ExprId {
        self.ir.functions().find(|f| f.name() == name).unwrap_or_else(|| panic!("no function `{name}` in:\n{}", self.ir.print_structural())).body()
    }

    /// Every expression reachable from `root`, `root` first.
    fn reach(&self, root: ExprId) -> Vec<ExprId> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            out.push(e);
            match self.ir.expr(e) {
                Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => stack.push(*expr),
                Expr::Binary { lhs, rhs, .. } => stack.extend([*lhs, *rhs]),
                Expr::Unwrap { is, value, .. } => stack.extend([*is, *value]),
                Expr::If { cond, then, otherwise } => stack.extend([*cond, *then, *otherwise]),
                Expr::Method { receiver, args, .. } => stack.extend(std::iter::once(*receiver).chain(args.iter().copied())),
                Expr::Call { args, .. } | Expr::Math { args, .. } => stack.extend(args.iter().copied()),
                Expr::Block { stmts, value } => {
                    for s in stmts {
                        if let piperine_ir::Stmt::Let { init, .. } | piperine_ir::Stmt::Expr(init) = self.ir.stmt(*s) {
                            stack.push(*init);
                        }
                    }
                    stack.extend(value.iter().copied());
                }
                _ => {}
            }
        }
        out
    }

    /// The text of the function named `name` in the structural print: its header and the lines
    /// under it.
    fn function_text(&self, name: &str) -> String {
        let text = self.ir.print_structural();
        let head = format!("fn {name}(");
        let start = text.find(&head).unwrap_or_else(|| panic!("no `{head}` in:\n{text}"));
        let rest = &text[start..];
        let end = rest[1..].find("\nfn ").or_else(|| rest[1..].find("\ninstance ")).map_or(rest.len(), |at| at + 1);
        rest[..end].to_string()
    }
}

/// A design whose `Top` calls each function of `functions` from its `analog` block, so that the
/// top reaches it.
fn reaching(items: &str, calls: &[&str]) -> String {
    let lets: Vec<String> = calls.iter().enumerate().map(|(k, call)| format!("let x{k} = {call};")).collect();
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {} }}", lets.join(" "))
}

#[test]
fn integers_lower_to_typed_ir() {
    // DIG-08 AC 1: the buck's `rev3` and `dithered_width` (examples/buck/src/dpwm.msl), and the
    // remaining operators, a cast in each direction and the integer methods.
    let items = "fn rev3(k: u3) -> u3 { ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1) }\n\
                 fn dithered_width(duty: u12, frame: u3) -> u9 { let coarse = (duty >> 3) as u9; let fine = (duty & 7) as u3; if rev3(frame) < fine { coarse + 1 } else { coarse } }\n\
                 fn ops(a: i16, b: i16) -> i16 { let q = ((a / b) % 7) ^ !a; -q * (q - b) }\n\
                 fn bounded(x: i16) -> i16 { x.clamp(-256, 255).min(x).max(-1) }\n\
                 fn down(x: f64) -> u8 { x.floor() as u8 }\n\
                 fn up(x: u8) -> f64 { x as f64 }";
    let src = reaching(items, &["dithered_width(4095, 7)", "ops(1, 2)", "bounded(3)", "down(2.5)", "up(4)"]);
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };

    // The printed forms of the `ir-rust` T29 fixtures (crates/piperine-ir/tests/golden/buck/dpwm.txt),
    // here with every value known.
    assert!(read.function_text("rev3").contains("fn rev3(k: u3) -> u3\n  local k: u3\n  ((((k & 1u3) << 2i32) | (k & 2u3)) | ((k >> 2i32) & 1u3))"), "{}", read.function_text("rev3"));
    let dithered = read.function_text("dithered_width");
    for line in ["fn dithered_width(duty: u12, frame: u3) -> u9", "let coarse = ((duty >> 3i32) as u9)", "let fine = ((duty & 7u12) as u3)", "if (rev3(frame) < fine) { (coarse + 1u9) } else { coarse }"] {
        assert!(dithered.contains(line), "`{line}` in:\n{dithered}");
    }

    // Node by node: `UInt`, `Int`, integer literals, the operators, `Cast` and `Method`.
    let rev3 = read.reach(read.body_of("rev3"));
    assert_eq!(*ir.expr_type(rev3[0]), ValueType::UInt(3));
    assert!(matches!(ir.expr(rev3[0]), Expr::Binary { op: BinaryOp::BitOr, .. }));
    let ops_of = |name: &str| -> Vec<BinaryOp> { read.reach(read.body_of(name)).into_iter().filter_map(|e| if let Expr::Binary { op, .. } = ir.expr(e) { Some(*op) } else { None }).collect() };
    let rev3_ops = ops_of("rev3");
    for op in [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::Shl, BinaryOp::Shr] {
        assert!(rev3_ops.contains(&op), "{op:?} in rev3");
    }
    let literals: Vec<(i128, ValueType)> = rev3.iter().filter_map(|e| if let Expr::Int(v) = ir.expr(*e) { Some((*v, ir.expr_type(*e).clone())) } else { None }).collect();
    assert!(literals.contains(&(1, ValueType::UInt(3))) && literals.contains(&(2, ValueType::UInt(3))), "an operand literal takes the other operand's type: {literals:?}");
    assert!(literals.contains(&(2, ValueType::Int(32))), "a bare shift amount is an `i32` (§1 rule 7): {literals:?}");

    let ops_text = read.function_text("ops");
    assert!(ops_text.contains("fn ops(a: i16, b: i16) -> i16"), "{ops_text}");
    assert!(ops_text.contains("let q = (((a / b) % 7i16) ^ (!a))"), "{ops_text}");
    assert!(ops_text.contains("((-q) * (q - b))"), "{ops_text}");
    for op in [BinaryOp::Div, BinaryOp::Rem, BinaryOp::BitXor, BinaryOp::Mul, BinaryOp::Sub] {
        assert!(ops_of("ops").contains(&op), "{op:?} in ops");
    }

    let bounded = read.reach(read.body_of("bounded"));
    let methods: Vec<Method> = bounded.iter().filter_map(|e| if let Expr::Method { method, .. } = ir.expr(*e) { Some(*method) } else { None }).collect();
    assert_eq!(methods, [Method::Max, Method::Min, Method::Clamp], "outermost first");
    assert!(bounded.iter().all(|e| !matches!(ir.expr(*e), Expr::Method { .. }) || *ir.expr_type(*e) == ValueType::Int(16)), "each method gives the receiver's type");
    assert!(read.function_text("bounded").contains("x.clamp(-256i16, 255i16).min(x).max(-1i16)"), "{}", read.function_text("bounded"));

    let down = read.reach(read.body_of("down"));
    assert!(matches!(ir.expr(down[0]), Expr::Cast { to: ValueType::UInt(8), .. }) && *ir.expr_type(down[0]) == ValueType::UInt(8));
    assert!(down.iter().any(|e| matches!(ir.expr(*e), Expr::Method { method: Method::Floor, .. }) && *ir.expr_type(*e) == ValueType::F64));
    let up = read.reach(read.body_of("up"));
    assert!(matches!(ir.expr(up[0]), Expr::Cast { to: ValueType::F64, .. }) && *ir.expr_type(up[0]) == ValueType::F64);

    // The function signatures carry the integer types.
    let dw = ir.functions().find(|f| f.name() == "dithered_width").expect("dithered_width");
    assert_eq!(dw.params().iter().map(|p| p.ty.clone()).collect::<Vec<_>>(), [ValueType::UInt(12), ValueType::UInt(3)]);
    assert_eq!(*dw.ret(), ValueType::UInt(9));
}

#[test]
fn integer_constants_fold() {
    // DIG-08 AC 2: `N_PERIOD - 1` with `N_PERIOD: u9 = 400` is the literal 399 of `u9`; the
    // other constant expressions of `Dpwm` fold the same way (400 - 4 = 396).
    let items = "const N_PERIOD: u9 = 400;\nconst DEAD_COUNTS: u9 = 4;\nconst E_MIN: i16 = -256;\n\
                 fn last() -> u9 { N_PERIOD - 1 }\n\
                 fn wrap(count: u9) -> bool { count == N_PERIOD - 1 }\n\
                 fn tail(next: u9, off_at: u9) -> bool { next >= off_at + DEAD_COUNTS && next < N_PERIOD - DEAD_COUNTS }\n\
                 fn floor_e() -> i16 { E_MIN }\n\
                 fn widened() -> i32 { (N_PERIOD as i32) * 2 }";
    let src = reaching(items, &["last()", "wrap(3)", "tail(1, 2)", "floor_e()", "widened()"]);
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    let folded = |name: &str| {
        let body = read.body_of(name);
        (ir.expr(body).clone(), ir.expr_type(body).clone())
    };
    assert_eq!(folded("last"), (Expr::Int(399), ValueType::UInt(9)));
    assert_eq!(folded("floor_e"), (Expr::Int(-256), ValueType::Int(16)));
    assert_eq!(folded("widened"), (Expr::Int(800), ValueType::Int(32)), "a cast of a constant reads only constants: folded");
    assert!(read.function_text("wrap").contains("(count == 399u9)"), "{}", read.function_text("wrap"));
    let tail = read.function_text("tail");
    assert!(tail.contains("(next >= (off_at + 4u9))") && tail.contains("(next < 396u9)"), "{tail}");
}

/// One folded case: the function, the type its result has, the value the specification gives
/// (with the rule), and the value the compiled code's integers give.
struct Fold {
    function: &'static str,
    ty: ValueType,
    spec: i128,
    compiled: i128,
}

#[test]
fn folded_casts_shifts_and_sums_wrap_as_compiled_code_does() {
    let cases = [
        // G4: `as` from `f64` saturates to the target's range.
        Fold { function: "fn f0() -> u8 { 300.0 as u8 }", ty: ValueType::UInt(8), spec: 255, compiled: i128::from(UInt::<8>::from_f64(300.0).get()) },
        // G4: NaN gives 0.
        Fold { function: "fn f1() -> u8 { (0.0 / 0.0) as u8 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from(UInt::<8>::from_f64(f64::NAN).get()) },
        // G5: `<<` by the width or more gives 0.
        Fold { function: "fn f2() -> u8 { 1u8 << 9 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from((UInt::<8>::new(1) << Int::<32>::new(9)).get()) },
        // G5: a signed `>>` by the width or more gives the sign.
        Fold { function: "fn f3() -> i8 { -1i8 >> 9 }", ty: ValueType::Int(8), spec: -1, compiled: i128::from((Int::<8>::new(-1) >> Int::<32>::new(9)).get()) },
        // §1 rule 6: arithmetic wraps modulo 2^N.
        Fold { function: "fn f4() -> u9 { 511u9 + 1 }", ty: ValueType::UInt(9), spec: 0, compiled: i128::from((UInt::<9>::new(511) + UInt::<9>::new(1)).get()) },
        // G4 on a signed target, both ends, and truncation toward zero.
        Fold { function: "fn f5() -> i8 { -200.7 as i8 }", ty: ValueType::Int(8), spec: -128, compiled: i128::from(Int::<8>::from_f64(-200.7).get()) },
        Fold { function: "fn f6() -> i8 { 126.9 as i8 }", ty: ValueType::Int(8), spec: 126, compiled: i128::from(Int::<8>::from_f64(126.9).get()) },
        Fold { function: "fn f7() -> u8 { -3.9 as u8 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from(UInt::<8>::from_f64(-3.9).get()) },
        // §1 rule 1: among integer types `as` truncates (300 mod 256) and extends by the sign.
        Fold { function: "fn f8() -> u8 { 300 as u8 }", ty: ValueType::UInt(8), spec: 44, compiled: i128::from(Int::<32>::new(300).to_uint::<8>().get()) },
        Fold { function: "fn f9() -> u16 { -1i8 as u16 }", ty: ValueType::UInt(16), spec: 65535, compiled: i128::from(Int::<8>::new(-1).to_uint::<16>().get()) },
        // §1 rule 6 in two's complement: -128 / -1 wraps to -128; `*` wraps.
        Fold { function: "fn f10() -> i8 { -128i8 / -1 }", ty: ValueType::Int(8), spec: -128, compiled: i128::from((Int::<8>::new(-128) / Int::<8>::new(-1)).get()) },
        Fold { function: "fn f11() -> u8 { 255u8 * 2 }", ty: ValueType::UInt(8), spec: 254, compiled: i128::from((UInt::<8>::new(255) * UInt::<8>::new(2)).get()) },
        // `%` keeps the dividend's sign, as in Rust; `!` and unary `-` wrap.
        Fold { function: "fn f12() -> i8 { -7i8 % 3 }", ty: ValueType::Int(8), spec: -1, compiled: i128::from((Int::<8>::new(-7) % Int::<8>::new(3)).get()) },
        Fold { function: "fn f13() -> u4 { !0u4 }", ty: ValueType::UInt(4), spec: 15, compiled: i128::from((!UInt::<4>::new(0)).get()) },
        Fold { function: "fn f14() -> u4 { -1u4 }", ty: ValueType::UInt(4), spec: 15, compiled: i128::from((-UInt::<4>::new(1)).get()) },
        // An unsigned `>>` by the width or more is 0; within the width it is logical.
        Fold { function: "fn f15() -> u8 { 128u8 >> 8 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from((UInt::<8>::new(128) >> Int::<32>::new(8)).get()) },
        Fold { function: "fn f16() -> i8 { -128i8 >> 7 }", ty: ValueType::Int(8), spec: -1, compiled: i128::from((Int::<8>::new(-128) >> Int::<32>::new(7)).get()) },
        // `clamp`, `min`, `max` of constants.
        Fold { function: "fn f17() -> i16 { 300i16.clamp(-256, 255) }", ty: ValueType::Int(16), spec: 255, compiled: i128::from(Int::<16>::new(300).clamp(Int::<16>::new(-256), Int::<16>::new(255)).get()) },
    ];
    let items: Vec<&str> = cases.iter().map(|c| c.function).collect();
    let calls: Vec<String> = (0..cases.len()).map(|k| format!("f{k}()")).collect();
    let src = reaching(&items.join("\n"), &calls.iter().map(String::as_str).collect::<Vec<_>>());
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    for (k, case) in cases.iter().enumerate() {
        assert_eq!(case.spec, case.compiled, "{}: the compiled code's integers disagree with the specification", case.function);
        let body = read.body_of(&format!("f{k}"));
        assert_eq!((ir.expr(body).clone(), ir.expr_type(body).clone()), (Expr::Int(case.spec), case.ty.clone()), "{}", case.function);
    }
}

#[test]
fn a_constant_division_by_zero_is_e0104() {
    // DIG-08 AC 4, §1 rule 11: a panic while a const expression is evaluated is `E0104`.
    for (src, at) in [
        ("const K: u8 = 1 / 0;\nfn f() -> u8 { K }", "1 / 0"),
        ("fn f() -> u8 { 7 % 0 }", "7 % 0"),
        ("const Z: i8 = 0;\nfn f() -> i8 { 5i8 / Z }", "5i8 / Z"),
        ("fn f() -> i16 { 3i16.clamp(2, 1) }", "3i16.clamp(2, 1)"),
    ] {
        let src = reaching(src, &["f()"]);
        let err = Elab::run(&src, "Top").expect_err(at);
        assert_eq!(Elab::codes(&err), ["E0104"], "{at}");
        let span = err[0].diagnostic.span();
        assert_eq!(&src[span.start..span.end], at);
    }
    // A division whose divisor is not a constant is left to the run (§1 rule 11).
    Elab::ir(&reaching("fn f(d: u8) -> u8 { 7 / d }", &["f(0)"]), "Top");
}

/// The `local` lines of the structural print of an owner: `name: type` for each.
fn locals_in(text: &str) -> Vec<String> {
    text.lines().filter_map(|l| l.trim().strip_prefix("local ")).map(str::to_string).collect()
}

#[test]
fn an_enum_is_its_tag_and_a_match_is_an_if_chain() {
    // DIG-08 AC 9 (the name is kept: msl-digital C72 cites it), §2 rule 5, CV-20 AC 4: the enum is an IR enum.
    let items = "#[derive(Repr, PartialEq)]\npub enum Mode { Off, SoftStart, Regulate, Hiccup }\n\
                 #[derive(Repr, PartialEq)]\npub enum Fault { Clear, OverCurrent, OverVoltage }\n\
                 fn hiccup() -> Mode { Mode::Hiccup }\n\
                 fn clear() -> Fault { Fault::Clear }\n\
                 fn warning(s: Severity) -> bool { s == Severity::Warning }\n\
                 fn next(m: Mode, ok: bool) -> Mode {\n\
                   match m {\n\
                     Mode::SoftStart | Mode::Regulate => Mode::Regulate,\n\
                     Mode::Off => if ok { Mode::SoftStart } else { Mode::Off },\n\
                     Mode::Hiccup => Mode::SoftStart,\n\
                   }\n\
                 }\n\
                 fn severe(f: Fault) -> Severity { match f { Fault::Clear => Severity::Info, _ => Severity::Error } }\n\
                 fn once(f: Fault) -> u8 { match severe(f) { Severity::Info => 0, Severity::Warning => 1, Severity::Error => 2 } }";
    let calls = ["hiccup()", "clear()", "warning(Severity::Warning)", "next(Mode::Off, true)", "once(Fault::Clear)"];
    let ir = Elab::ir(&reaching(items, &calls), "Top");
    let read = Read { ir: &ir };
    let function = |name: &str| ir.functions().find(|f| f.name() == name).unwrap_or_else(|| panic!("no {name}"));

    // §2 rule 5: an enum of `V` unit variants has a tag of `ceil(log2 V)` levels; a `match` is an `if`
    // chain that reads its scrutinee once. The enums are the buck's (examples/buck/src/supervisor.msl)
    // and the stdlib's `Severity` (std/io.msl). `Mode`, `Fault` and `Severity` are IR enums since composite-values T48: a function's result and
    // parameter have the enum's arena type, `Hiccup` is `VariantOf` its 4th variant (discriminant 3).
    let enum_named = |name: &str| ValueType::Enum(piperine_ir::TypeId(ir.types().iter().position(|t| t.name == name).unwrap_or_else(|| panic!("no type {name}")) as u32));
    assert_eq!(*function("hiccup").ret(), enum_named("Mode"));
    assert!(matches!(ir.expr(read.body_of("hiccup")), Expr::VariantOf { variant: 3, fields, .. } if fields.is_empty()));
    assert_eq!(*function("clear").ret(), enum_named("Fault"));
    assert!(matches!(ir.expr(read.body_of("clear")), Expr::VariantOf { variant: 0, fields, .. } if fields.is_empty()));
    assert_eq!(function("warning").params()[0].ty, enum_named("Severity"));
    assert!(read.function_text("warning").contains("(s == (Severity::Warning() : t"), "{}", read.function_text("warning"));
    assert_eq!(function("next").params().iter().map(|p| p.ty.clone()).collect::<Vec<_>>(), [enum_named("Mode"), ValueType::Bool]);

    // The arms in order, each a variant test; an or-pattern is `||`; the last
    // arm is the final `else`; a param is read where it is.
    let next = read.function_text("next");
    assert!(next.contains("if (matches!(m, Mode::SoftStart) || matches!(m, Mode::Regulate)) { (Mode::Regulate() : t"), "{next}");
    assert!(next.contains("else { if matches!(m, Mode::Off) {"), "{next}");
    // `_` is the final `else`.
    assert!(read.function_text("severe").contains("if matches!(f, Fault::Clear) { (Severity::Info() : t"), "{}", read.function_text("severe"));
    // A scrutinee that is not a plain read is bound to a local once, and read through it.
    let once = read.function_text("once");
    assert_eq!(once.matches("severe(f)").count(), 1, "{once}");
    assert!(once.contains("let match$0 = severe(f)"), "{once}");
    assert!(once.contains("if matches!(match$0, Severity::Info) { 0u8 } else { if matches!(match$0, Severity::Warning) { 1u8 } else { 2u8 } }"), "{once}");
    // An enum of one variant has a tag of no level and is an IR enum all the same: its value is the
    // variant (composite-values T48; it cannot be carried, the checker says so by name).
    let src = reaching("#[derive(Repr, PartialEq)]\npub enum One { Only }\nfn only() -> One { One::Only }", &["only()"]);
    let ir = Elab::ir(&src, "Top");
    assert!(ir.expressions().any(|e| matches!(e, Expr::VariantOf { variant: 0, fields, .. } if fields.is_empty())));
}

#[test]
fn an_option_is_a_flag_and_a_payload() {
    // DIG-08 AC 10, rewritten for the explicit model (explicit-unknowns): an `Option<T>` is a `bool` (its presence)
    // and a `T` (its payload) through `Some`, `None`, `is_some`, `is_none`, `unwrap_or`, `if let` and `match`; no IR
    // type of value code is an `Option`. `supervise` holds `Supervisor`'s `fault_now` (examples/buck/src/supervisor.msl)
    let items = "#[derive(Repr, PartialEq)]\npub enum Fault { Clear, OverCurrent, OverVoltage }\n\
                 const OC_PERIODS: u4 = 8;\nconst OV_SAMPLES: u2 = 2;\n\
                 fn supervise(oc_n: u4, ov_n: u2) -> u8 {\n\
                   let fault_now = if oc_n >= OC_PERIODS { Some(Fault::OverCurrent) }\n\
                                   else if ov_n >= OV_SAMPLES { Some(Fault::OverVoltage) }\n\
                                   else { None };\n\
                   let latched = if let Some(f) = fault_now { f } else { Fault::Clear };\n\
                   let quiet = fault_now.is_none();\n\
                   let tripped = fault_now.is_some();\n\
                   let cause = fault_now.unwrap_or(Fault::Clear);\n\
                   let code: u8 = match fault_now { Some(Fault::OverCurrent) => 1, Some(f) => if f == latched { 2 } else { 3 }, None => 0 };\n\
                   if quiet && !tripped && cause == Fault::Clear { code } else { code + 1 }\n\
                 }\n\
                 fn literal() -> u8 { Some(3u8).unwrap_or(9) }\n\
                 fn pick(o: Option<u8>) -> u8 { o.unwrap_or(5) }";
    let ir = Elab::ir(&reaching(items, &["supervise(0, 0)", "literal()", "pick(Some(2))"]), "Top");
    let text = ir.print_structural();
    let read = Read { ir: &ir };

    // A `let` of an `Option` binds its flag and its payload; `Some`/`None` give each.
    let supervise = read.function_text("supervise");
    let locals = locals_in(&supervise);
    // `Fault` is an IR enum: the payload local holds the enum, not its tag.
    let fault = format!("t{}", ir.types().iter().position(|t| t.name == "Fault").expect("Fault is in the arena"));
    for local in ["fault_now.is: bool".to_string(), format!("fault_now.val: {fault}"), format!("f: {fault}"), format!("latched: {fault}"), "quiet: bool".to_string(), "tripped: bool".to_string(), format!("cause: {fault}"), "match$0.is: bool".to_string(), format!("match$0.val: {fault}")] {
        assert!(locals.contains(&local), "local {local} in:\n{supervise}");
    }
    let (over_current, over_voltage, clear) = (format!("(Fault::OverCurrent() : {fault})"), format!("(Fault::OverVoltage() : {fault})"), format!("(Fault::Clear() : {fault})"));
    for line in [
        "let fault_now.is = if (oc_n >= 8u4) { true } else { if (ov_n >= 2u2) { true } else { false } }".to_string(),
        format!("let fault_now.val = if (oc_n >= 8u4) {{ {over_current} }} else {{ if (ov_n >= 2u2) {{ {over_voltage} }} else {{ {clear} }} }}"),
        // `if let`: the flag decides, the binding reads the payload.
        format!("let latched = if fault_now.is {{ let f = fault_now.val; f }} else {{ {clear} }}"),
        "let quiet = (!fault_now.is)".to_string(),
        "let tripped = fault_now.is".to_string(),
        format!("let cause = if fault_now.is {{ fault_now.val }} else {{ {clear} }}"),
        // `match`: `Some(v)` is the flag and the payload tested for `v`, `Some(f)` binds the payload, `None` the last arm.
        "let match$0.is = fault_now.is".to_string(),
        "let match$0.val = fault_now.val".to_string(),
        "if (match$0.is && matches!(match$0.val, Fault::OverCurrent)) { 1u8 } else { if match$0.is { let f = match$0.val; if (f == latched) { 2u8 } else { 3u8 } } else { 0u8 } }".to_string(),
    ] {
        assert!(supervise.contains(&line), "`{line}` in:\n{supervise}");
    }

    // `unwrap_or` of a literal `Some` is its payload; of a pair it is an `if` on the flag.
    assert_eq!(ir.expr(read.body_of("literal")), &Expr::Int(3));
    let pick = read.function_text("pick");
    assert!(pick.contains("if o.is { o.val } else { 5u8 }"), "{pick}");
    assert!(!text.contains("flagged"), "no value type carries a flag:\n{text}");

    // The reaction form of `Supervisor` (supervisor.msl): `fault_now` is a flag and a payload, `if let Some(f) =
    // fault_now` reads the pair, and no local is flagged.
    let spec = std::fs::read_to_string(format!("{}/../../examples/buck/src/spec.msl", env!("CARGO_MANIFEST_DIR"))).expect("spec.msl");
    let src = format!(
        "{spec}\n{}\n{}\n{}\n{}\n{}",
        buck_item("supervisor.msl", "pub enum Mode"),
        buck_item("supervisor.msl", "pub enum Fault"),
        buck_item("supervisor.msl", "pub fn in_window"),
        buck_item("supervisor.msl", "pub fn ovp_threshold"),
        buck_module("supervisor.msl", "Supervisor")
    );
    let ir = Elab::ir(&src, "Supervisor");
    let text = ir.print_structural();
    let locals = locals_in(&text);
    let (mode, fault) = (ir.types().iter().position(|t| t.name == "Mode").expect("Mode"), ir.types().iter().position(|t| t.name == "Fault").expect("Fault"));
    assert!(locals.contains(&"fault_now.is: bool".to_string()) && locals.contains(&format!("fault_now.val: t{fault}")), "{text}");
    assert!(text.contains(&format!("if fault_now.is {{ let f = fault_now.val; mode <- (Mode::Hiccup() : t{mode}); fault <- f;")), "{text}");
}

/// The item of `examples/buck/src/<file>` that starts with `head`, through its closing brace.
fn buck_item(file: &str, head: &str) -> String {
    let text = std::fs::read_to_string(format!("{}/../../examples/buck/src/{file}", env!("CARGO_MANIFEST_DIR"))).expect("a buck source");
    let at = text.find(head).unwrap_or_else(|| panic!("no `{head}`"));
    let start = text[..at].rfind("#[derive").filter(|d| !text[*d..at].contains('}') && !text[*d..at].contains(";\n")).unwrap_or(at);
    let mut depth = 0usize;
    for (i, c) in text[at..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return text[start..at + i + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("`{head}` does not close")
}

/// A module of `examples/buck/src/<file>` with its `digital` block, as written.
fn buck_module(file: &str, name: &str) -> String {
    format!("{}\n{}", buck_item(file, &format!("pub module {name}(")), buck_item(file, &format!("digital for {name} ")))
}

#[test]
fn a_tuple_let_is_one_local_per_component() {
    // DIG-08 AC 11: `let (m, r) = (x, y);` is two locals, each typed by its own component.
    // `in_window` of the buck (examples/buck/src/supervisor.msl).
    let items = "pub fn in_window(meas: u10, reference: u10) -> bool {\n\
                   let (m, r) = (meas as u11, reference as u11);\n\
                   let half = r >> 4;\n\
                   m + half >= r && m <= r + half\n\
                 }\n\
                 fn mixed(u: u8) -> u8 { let (a, b) = (u, 1u8); a + b }";
    let ir = Elab::ir(&reaching(items, &["in_window(1, 2)", "mixed(3)"]), "Top");
    let read = Read { ir: &ir };
    let window = read.function_text("in_window");
    assert!(window.contains("let m = (meas as u11)\n  let r = (reference as u11)"), "{window}");
    let locals = locals_in(&window);
    assert!(locals.contains(&"m: u11".to_string()) && locals.contains(&"r: u11".to_string()), "{window}");
    let mixed = read.function_text("mixed");
    let locals = locals_in(&mixed);
    assert!(locals.contains(&"a: u8".to_string()) && locals.contains(&"b: u8".to_string()), "each component has its own type:\n{mixed}");
}

#[test]
fn numeric_rows_lower_to_methods_and_checked_ops_to_pairs() {
    // CAT-01, CAT-02 (design C6): every f64 and integer row of Appendix H lowers to one
    // `Expr::Method` of its own, with the type H gives (`bool` for `is_nan`, `is_finite`,
    // `is_infinite`, the receiver's otherwise); `exp`, `ln`, `sqrt`, `abs` of an `f64` stay
    // `Math` (the twins' text does not move). `checked_op(a, b)` is the pair
    // `(!op_overflows(a, b), a op b)`, and no IR type is an `Option`.
    let items = "fn real(x: f64, y: f64) -> f64 {\n\
                   let a = x.sin() + x.cos() + x.tan() + x.asin() + x.acos() + x.atan() + x.atan2(y) + x.sinh() + x.cosh() + x.tanh();\n\
                   let b = x.powf(y) + x.powi(3) + x.log10() + x.log2() + x.ceil() + x.round() + x.trunc() + x.fract() + x.signum() + x.rem_euclid(y) + x.hypot(y);\n\
                   let c = x.exp() + x.ln() + x.sqrt() + x.abs() + x.floor();\n\
                   let d = x.is_nan() || x.is_finite() || x.is_infinite();\n\
                   if d { a + b + c } else { 0.0 }\n\
                 }\n\
                 fn whole(k: i8, u: u8) -> i8 {\n\
                   let a = k.abs() + k.signum() + k.pow(2) + k.rem_euclid(3) + k.saturating_add(1) + k.saturating_sub(1) + k.saturating_mul(2);\n\
                   let b = u.pow(2) + u.rem_euclid(3) + u.saturating_add(1);\n\
                   a + (b as i8)\n\
                 }\n\
                 fn checked(a: u8, b: u8) -> u8 { let s = a.checked_add(b); let d = a.checked_sub(b); let m = a.checked_mul(b); s.unwrap_or(0) + d.unwrap_or(1) + m.unwrap_or(2) }";
    let ir = Elab::ir(&reaching(items, &["real(1.0, 2.0)", "whole(1, 2)", "checked(1, 2)"]), "Top");
    let text = ir.print_structural();
    assert!(!text.contains("Option"), "no IR type is an `Option`:\n{text}");
    let read = Read { ir: &ir };
    let methods = |name: &str| -> Vec<(Method, ValueType)> {
        let mut found: Vec<(Method, ValueType)> = read.reach(read.body_of(name)).into_iter().filter_map(|e| if let Expr::Method { method, .. } = ir.expr(e) { Some((*method, ir.expr_type(e).clone())) } else { None }).collect();
        found.sort_by_key(|(m, _)| m.name());
        found
    };
    use Method::*;
    let real: Vec<Method> = methods("real").into_iter().map(|(m, _)| m).collect();
    let mut want: Vec<Method> = vec![Sin, Cos, Tan, Asin, Acos, Atan, Atan2, Sinh, Cosh, Tanh, Powf, Powi, Log10, Log2, Ceil, Round, Trunc, Fract, Signum, RemEuclid, Hypot, Floor, IsNan, IsFinite, IsInfinite];
    want.sort_by_key(|m| m.name());
    assert_eq!(real, want);
    for (method, ty) in methods("real") {
        let expected = if matches!(method, IsNan | IsFinite | IsInfinite) { ValueType::Bool } else { ValueType::F64 };
        assert_eq!(ty, expected, "{}", method.name());
    }
    let real_text = read.function_text("real");
    for call in ["x.sin()", "x.atan2(y)", "x.powi(3i32)", "x.rem_euclid(y)", "x.is_nan()"] {
        assert!(real_text.contains(call), "`{call}` in:\n{real_text}");
    }
    assert!(read.reach(read.body_of("real")).iter().any(|e| matches!(ir.expr(*e), Expr::Math { f: piperine_ir::MathFn::Exp, .. })), "`exp` stays `Math`");

    let whole: Vec<Method> = methods("whole").into_iter().map(|(m, _)| m).collect();
    let mut want: Vec<Method> = vec![IntAbs, Signum, Pow, RemEuclid, SaturatingAdd, SaturatingSub, SaturatingMul, Pow, RemEuclid, SaturatingAdd];
    want.sort_by_key(|m| m.name());
    assert_eq!(whole, want);
    let whole_text = read.function_text("whole");
    assert!(whole_text.contains("k.abs()") && whole_text.contains("k.pow(2u32)") && whole_text.contains("u.saturating_add(1u8)"), "{whole_text}");

    // `checked_op`: the flag is `!overflows`, the payload the wrapping operation.
    let checked = read.function_text("checked");
    for line in [
        "let s.is = (!a.add_overflows(b))",
        "let s.val = (a + b)",
        "let d.is = (!a.sub_overflows(b))",
        "let d.val = (a - b)",
        "let m.is = (!a.mul_overflows(b))",
        "let m.val = (a * b)",
    ] {
        assert!(checked.contains(line), "`{line}` in:\n{checked}");
    }
    let overflows: Vec<(Method, ValueType)> = methods("checked");
    assert_eq!(overflows.iter().map(|(m, _)| *m).collect::<Vec<_>>(), [AddOverflows, MulOverflows, SubOverflows]);
    assert!(overflows.iter().all(|(_, ty)| *ty == ValueType::Bool));

    // Over constants the rows fold at elaboration with the values the spec states (CAT-02 AC 2,
    // 3, 6): `pow` and `abs` wrap, `saturating_*` clamp, `signum` and `rem_euclid` follow Rust.
    let folded = "fn k0() -> u8 { 2u8.pow(9) }\nfn k1() -> u8 { 250u8.saturating_add(10) }\nfn k2() -> i8 { (-100i8).saturating_sub(100) }\n\
                  fn k3() -> i8 { (-128i8).abs() }\nfn k4() -> i8 { (-5i8).signum() }\nfn k5() -> i8 { (-7i8).rem_euclid(3) }\nfn k6() -> u8 { 16u8.saturating_mul(16) }";
    let ir = Elab::ir(&reaching(folded, &["k0()", "k1()", "k2()", "k3()", "k4()", "k5()", "k6()"]), "Top");
    let read = Read { ir: &ir };
    for (k, want, ty) in [(0, 0, ValueType::UInt(8)), (1, 255, ValueType::UInt(8)), (2, -128, ValueType::Int(8)), (3, -128, ValueType::Int(8)), (4, -1, ValueType::Int(8)), (5, 2, ValueType::Int(8)), (6, 255, ValueType::UInt(8))] {
        let body = read.body_of(&format!("k{k}"));
        assert_eq!((ir.expr(body).clone(), ir.expr_type(body).clone()), (Expr::Int(want), ty), "k{k}");
    }
    // A constant `rem_euclid` by zero is a panic while a const expression is evaluated (`E0104`).
    let err = Elab::run(&reaching("fn f() -> u8 { 7u8.rem_euclid(0) }", &["f()"]), "Top").expect_err("rem_euclid by zero");
    assert_eq!(Elab::codes(&err), ["E0104"]);
}

#[test]
fn option_rows_lower_on_the_pair() {
    // CAT-03 (design C6): an `Option` is the pair `(is, val)`; the rows lower on it. `unwrap` and
    // `expect` are one `Unwrap`; `unwrap_or_else(|| d)` is `if is { val } else { d }`; `map` keeps
    // the flag and binds the closure's parameter to the payload; `and_then` takes the pair of the
    // closure's `Option` under that binding, only for `Some`; `a.or(b)` is `(a.is || b.is,
    // if a.is { a.val } else { b.val })`; `c.then(|| v)` and `c.then_some(v)` are `(c, v)`. No IR
    // type is an `Option` and no closure survives.
    let some = "let o = if c > 1 { Some(x) } else { None };";
    let items = format!(
        "fn unw(c: u8, x: u8) -> u8 {{ {some} o.unwrap() + o.expect(\"need the reading\") }}\n\
         fn els(c: u8, x: u8) -> u8 {{ {some} o.unwrap_or_else(|| 9) }}\n\
         fn mp(c: u8, x: u8) -> u8 {{ {some} let m = o.map(|v| v + 1); m.unwrap_or(0) }}\n\
         fn chain(c: u8, x: u8) -> u8 {{ {some} let m = o.and_then(|v| if v > 3 {{ Some(v) }} else {{ None }}); m.unwrap_or(0) }}\n\
         fn orr(c: u8, x: u8) -> u8 {{ {some} let p = if c > 5 {{ Some(x) }} else {{ None }}; let m = o.or(p); m.unwrap_or(0) }}\n\
         fn th(c: u8) -> u8 {{ let t = (c > 1).then(|| 7u8); let s = (c > 2).then_some(5u8); t.unwrap_or(0) + s.unwrap_or(0) }}"
    );
    let ir = Elab::ir(&reaching(&items, &["unw(2, 1)", "els(2, 1)", "mp(2, 1)", "chain(2, 1)", "orr(2, 1)", "th(2)"]), "Top");
    let text = ir.print_structural();
    assert!(!text.contains("Option") && !text.contains("|"), "no `Option` and no closure in the IR:\n{text}");
    let read = Read { ir: &ir };

    // `unwrap` and `expect`: an `Unwrap` of the flag and the payload; `expect` carries its text.
    let unwraps: Vec<Option<String>> = read.reach(read.body_of("unw")).into_iter().filter_map(|e| if let Expr::Unwrap { message, .. } = ir.expr(e) { Some(message.clone()) } else { None }).collect();
    assert_eq!(unwraps.len(), 2);
    assert!(unwraps.contains(&None) && unwraps.contains(&Some("need the reading".to_string())), "{unwraps:?}");
    let unw_text = read.function_text("unw");
    assert!(unw_text.contains("let o.is =") && unw_text.contains("let o.val ="), "{unw_text}");

    let els = read.function_text("els");
    assert!(els.contains("if o.is { o.val } else { 9u8 }"), "{els}");

    let mp = read.function_text("mp");
    assert!(mp.contains("let m.is = o.is"), "{mp}");
    assert!(mp.contains("let m.val = if o.is { let v = o.val; (v + 1u8) } else { 0u8 }"), "{mp}");

    let chain = read.function_text("chain");
    assert!(chain.contains("let m.is = if o.is { let v = o.val; if (v > 3u8) { true } else { false } } else { false }"), "{chain}");
    assert!(chain.contains("let m.val = if o.is { let v = o.val; if (v > 3u8) { v } else { 0u8 } } else { 0u8 }"), "{chain}");

    let orr = read.function_text("orr");
    assert!(orr.contains("let m.is = if o.is { true } else { p.is }"), "{orr}");
    assert!(orr.contains("let m.val = if o.is { o.val } else { p.val }"), "{orr}");

    let th = read.function_text("th");
    assert!(th.contains("let t.is = (c > 1u8)") && th.contains("let t.val = if (c > 1u8) { 7u8 } else { 0u8 }"), "{th}");
    assert!(th.contains("let s.is = (c > 2u8)") && th.contains("let s.val = 5u8"), "{th}");
}

#[test]
fn a_const_reading_a_run_value_is_e0188() {
    // §3.9.2: the initializer of a `const` that is not a const expression is `E0188` naming the
    // `const`; a `const` without a value outside a trait is the same code. A `const` that calls a function reading the
    // environment, which only a run gives, was `E0188` here: since params-and-io T36 (PI-12 AC 3, §3) that call is
    // `E0122` at the checker, naming the function and its class (`types_values`), and the evaluator is not reached.
    // (`temperature()` is constant during an analysis, so a function that reads only it is pure: generics D13 amended,
    // the §3 table; a `const` over it is the evaluator's `E0188`, as before)
    let pure = reaching("fn h() -> f64 { temperature().unwrap_or(1.0) }\nconst K: f64 = h();", &["K"]);
    let err = Elab::run(&pure, "Top").expect_err("a const reading the environment");
    assert_eq!(Elab::codes(&err), ["E0188"], "{pure}");
    let src = reaching("fn h() -> f64 { now() }\nconst K: f64 = h();", &["K"]);
    let err = Elab::run(&src, "Top").expect_err("a const calling a solve-reading function");
    assert_eq!(Elab::codes(&err), ["E0122"], "{src}");
    assert!(err[0].diagnostic.message.contains("`h`") && err[0].diagnostic.message.contains("solve-reading"), "{}", err[0].diagnostic.message);
    let src = reaching("const K: u8;", &["K"]);
    let err = Elab::run(&src, "Top").expect_err("has no value");
    assert_eq!(Elab::codes(&err), ["E0188"], "{src}");
    assert!(err[0].diagnostic.message.contains("`K`") && err[0].diagnostic.message.contains("has no value"), "{}", err[0].diagnostic.message);
}

#[test]
fn a_const_that_depends_on_itself_is_e0188() {
    // §3.9.2: a constant that depends on itself, directly or through another `const`, is `E0188`.
    for src in ["const K: u8 = K + 1;\nfn f() -> u8 { K }", "const A: u8 = B;\nconst B: u8 = A;\nfn f() -> u8 { A }"] {
        let src = reaching(src, &["f()"]);
        let err = Elab::run(&src, "Top").expect_err("a cycle");
        assert_eq!(Elab::codes(&err), ["E0188"], "{src}");
        assert!(err[0].diagnostic.message.contains("depends on itself"), "{}", err[0].diagnostic.message);
    }
}
