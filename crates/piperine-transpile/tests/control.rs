//! Scope: the printer's control flow (composite-values CV-01 to CV-08): functions of hand-built IR
//! with mutable locals, assignment, loops and exits, printed, compiled by `rustc` with the runtime
//! and run, each compared with the same Rust written in the test. Under known control the text is
//! plain Rust and every loop is counted by an `rt::Trips` (Q5 (b)).
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/alone.rs"]
mod alone;
#[allow(dead_code)]
#[path = "support/probe.rs"]
mod probe;

use alone::Alone;
use digital::{Dx, Ty};
use piperine_ir::{BinaryOp, Expr, ExprId, FunctionId, FunctionSpec, Ir, IrBuilder, Local, Origin, Place, Stmt, StmtId, ValueType};
use piperine_transpile::print::ValuePrinter;
use probe::Probe;

const FILE: &str = "control.msl";

/// Builds the functions of this file: each takes its source line so an origin names it.
struct Functions;

impl Functions {
    fn local(name: &str, ty: ValueType, mutable: bool) -> Local {
        Local { name: name.into(), ty, mutable }
    }

    fn origin(line: u32) -> Origin {
        Origin { file: FILE.into(), line, col: 1 }
    }

    /// `fn name(params) -> ret { body }` at `line`; `locals` start with the params.
    fn define(b: &mut IrBuilder, name: &str, line: u32, locals: Vec<Local>, params: usize, ret: ValueType, body: ExprId) -> FunctionId {
        b.function(FunctionSpec { name: name.into(), origin: Self::origin(line), params: locals[..params].to_vec(), ret, locals, body })
    }

    /// `local op= value;`
    fn assign(b: &mut IrBuilder, local: u32, op: Option<BinaryOp>, value: ExprId) -> StmtId {
        b.stmt(Stmt::Assign { place: Place { local, path: vec![] }, op, value })
    }

    /// A control expression at `line` of type `ty`.
    fn control(b: &mut IrBuilder, kind: Expr, ty: ValueType, line: u32) -> ExprId {
        b.expr(kind, ty, Self::origin(line))
    }
}

/// The IR of the functions of the tests below, with the MSL each transcribes.
fn library() -> Ir {
    let (u16, boolean) = (Ty::u(16), ValueType::Bool);
    let mut b = IrBuilder::new();

    // control.msl:1  fn sum_to(n: u16) -> u16 { let mut s = 0; let mut i = 0; while i < n { s += i; i += 1; } s }
    let locals = vec![Functions::local("n", u16.clone(), false), Functions::local("s", u16.clone(), true), Functions::local("i", u16.clone(), true)];
    let mut x = Dx::new(&mut b, FILE, 2, 5);
    let (zero, zero_again) = (x.int(0, u16.clone()), x.int(0, u16.clone()));
    let (let_s, let_i) = (x.let_(1, zero), x.let_(2, zero_again));
    let (i, n) = (x.local(2, u16.clone()), x.local(0, u16.clone()));
    let below = x.bin(BinaryOp::Lt, i, n, boolean.clone());
    let i_value = x.local(2, u16.clone());
    let one = x.int(1, u16.clone());
    let add = Functions::assign(&mut b, 1, Some(BinaryOp::Add), i_value);
    let step = Functions::assign(&mut b, 2, Some(BinaryOp::Add), one);
    let looped = Functions::control(&mut b, Expr::While { cond: below, body: vec![add, step] }, ValueType::Unit, 3);
    let looped = b.stmt(Stmt::Expr(looped));
    let mut x = Dx::new(&mut b, FILE, 4, 5);
    let result = x.local(1, u16.clone());
    let body = x.block(vec![let_s, let_i, looped], Some(result), u16.clone());
    Functions::define(&mut b, "sum_to", 1, locals, 1, u16.clone(), body);

    // control.msl:10  fn countdown(n: u16) -> u16 {
    //     let mut s = 0;
    //     for k in (0..=n).rev() { if k == 3 { continue; } if k < 2 { break; } s *= 2; s += k; }
    //     s
    // }
    let locals = vec![Functions::local("n", u16.clone(), false), Functions::local("s", u16.clone(), true), Functions::local("k", u16.clone(), false)];
    let mut x = Dx::new(&mut b, FILE, 11, 5);
    let zero = x.int(0, u16.clone());
    let let_s = x.let_(1, zero);
    let (k, three) = (x.local(2, u16.clone()), x.int(3, u16.clone()));
    let is_three = x.bin(BinaryOp::Eq, k, three, boolean.clone());
    let (k, two) = (x.local(2, u16.clone()), x.int(2, u16.clone()));
    let below_two = x.bin(BinaryOp::Lt, k, two, boolean.clone());
    let doubled = x.int(2, u16.clone());
    let k_value = x.local(2, u16.clone());
    let skip = Functions::control(&mut b, Expr::Continue, ValueType::Unit, 12);
    let skip = b.stmt(Stmt::Expr(skip));
    let stop = Functions::control(&mut b, Expr::Break(None), ValueType::Unit, 13);
    let stop = b.stmt(Stmt::Expr(stop));
    let mut x = Dx::new(&mut b, FILE, 12, 9);
    let if_three = x.when(is_three, vec![skip], None);
    let if_three = x.stmt(if_three);
    let if_two = x.when(below_two, vec![stop], None);
    let if_two = x.stmt(if_two);
    let double = Functions::assign(&mut b, 1, Some(BinaryOp::Mul), doubled);
    let add = Functions::assign(&mut b, 1, Some(BinaryOp::Add), k_value);
    let (low, high) = {
        let mut x = Dx::new(&mut b, FILE, 11, 20);
        (x.int(0, u16.clone()), x.local(0, u16.clone()))
    };
    let looped = Functions::control(&mut b, Expr::ForRange { local: 2, low, high, inclusive: true, reverse: true, body: vec![if_three, if_two, double, add] }, ValueType::Unit, 11);
    let looped = b.stmt(Stmt::Expr(looped));
    let mut x = Dx::new(&mut b, FILE, 16, 5);
    let result = x.local(1, u16.clone());
    let body = x.block(vec![let_s, looped], Some(result), u16.clone());
    Functions::define(&mut b, "countdown", 10, locals, 1, u16.clone(), body);

    // control.msl:20  fn first_square_over(limit: u16) -> u16 {
    //     let mut i = 1;
    //     loop { if i * i > limit { break i; } i += 1; }
    // }
    let locals = vec![Functions::local("limit", u16.clone(), false), Functions::local("i", u16.clone(), true)];
    let mut x = Dx::new(&mut b, FILE, 21, 5);
    let one = x.int(1, u16.clone());
    let let_i = x.let_(1, one);
    let (i, i_again) = (x.local(1, u16.clone()), x.local(1, u16.clone()));
    let square = x.bin(BinaryOp::Mul, i, i_again, u16.clone());
    let limit = x.local(0, u16.clone());
    let over = x.bin(BinaryOp::Gt, square, limit, boolean.clone());
    let (found, one) = (x.local(1, u16.clone()), x.int(1, u16.clone()));
    let leave = Functions::control(&mut b, Expr::Break(Some(found)), ValueType::Unit, 22);
    let leave = b.stmt(Stmt::Expr(leave));
    let mut x = Dx::new(&mut b, FILE, 22, 9);
    let if_over = x.when(over, vec![leave], None);
    let if_over = x.stmt(if_over);
    let step = Functions::assign(&mut b, 1, Some(BinaryOp::Add), one);
    let looped = Functions::control(&mut b, Expr::Loop { body: vec![if_over, step] }, u16.clone(), 22);
    let mut x = Dx::new(&mut b, FILE, 24, 5);
    let body = x.block(vec![let_i], Some(looped), u16.clone());
    Functions::define(&mut b, "first_square_over", 20, locals, 1, u16.clone(), body);

    // control.msl:30  fn find_root(n: u16) -> u16 { for k in 0..n { if k * k == 49 { return k; } } 255 }
    let locals = vec![Functions::local("n", u16.clone(), false), Functions::local("k", u16.clone(), false)];
    let mut x = Dx::new(&mut b, FILE, 31, 5);
    let (k, k_again, forty_nine) = (x.local(1, u16.clone()), x.local(1, u16.clone()), x.int(49, u16.clone()));
    let square = x.bin(BinaryOp::Mul, k, k_again, u16.clone());
    let is_root = x.bin(BinaryOp::Eq, square, forty_nine, boolean.clone());
    let root = x.local(1, u16.clone());
    let give = Functions::control(&mut b, Expr::Return(Some(root)), ValueType::Unit, 31);
    let give = b.stmt(Stmt::Expr(give));
    let mut x = Dx::new(&mut b, FILE, 31, 9);
    let if_root = x.when(is_root, vec![give], None);
    let if_root = x.stmt(if_root);
    let (zero, n) = (x.int(0, u16.clone()), x.local(0, u16.clone()));
    let looped = Functions::control(&mut b, Expr::ForRange { local: 1, low: zero, high: n, inclusive: false, reverse: false, body: vec![if_root] }, ValueType::Unit, 31);
    let looped = b.stmt(Stmt::Expr(looped));
    let mut x = Dx::new(&mut b, FILE, 32, 5);
    let fallback = x.int(255, u16.clone());
    let body = x.block(vec![looped], Some(fallback), u16.clone());
    Functions::define(&mut b, "find_root", 30, locals, 1, u16.clone(), body);

    // control.msl:40  fn doubled(x: f64, n: u8) -> f64 { let mut y = x; for _k in 0..n { y *= 2.0; } y }
    let locals = vec![
        Functions::local("x", ValueType::F64, false),
        Functions::local("n", Ty::u(8), false),
        Functions::local("y", ValueType::F64, true),
        Functions::local("k", Ty::u(8), false),
    ];
    let mut x = Dx::new(&mut b, FILE, 41, 5);
    let start = x.local(0, ValueType::F64);
    let let_y = x.let_(2, start);
    let two = x.real(2.0);
    let (zero, n) = (x.int(0, Ty::u(8)), x.local(1, Ty::u(8)));
    let scale = Functions::assign(&mut b, 2, Some(BinaryOp::Mul), two);
    let looped = Functions::control(&mut b, Expr::ForRange { local: 3, low: zero, high: n, inclusive: false, reverse: false, body: vec![scale] }, ValueType::Unit, 42);
    let looped = b.stmt(Stmt::Expr(looped));
    let mut x = Dx::new(&mut b, FILE, 43, 5);
    let result = x.local(2, ValueType::F64);
    let body = x.block(vec![let_y, looped], Some(result), ValueType::F64);
    Functions::define(&mut b, "doubled", 40, locals, 2, ValueType::F64, body);

    Alone::finish(b)
}

/// The printed text of every function of `ir`, in the order they were added.
fn printed(ir: &Ir) -> Vec<String> {
    ir.functions().map(|f| ValuePrinter::function(ir, f.id()).unwrap_or_else(|e| panic!("{e}"))).collect()
}

/// `sum_to` as Rust writes it, on `u16` (the `rt` integers wrap at their width).
fn sum_to(n: u16) -> u16 {
    let (mut s, mut i) = (0u16, 0u16);
    while i < n {
        s = s.wrapping_add(i);
        i = i.wrapping_add(1);
    }
    s
}

/// `countdown` as Rust writes it.
fn countdown(n: u16) -> u16 {
    let mut s = 0u16;
    for k in (0..=n).rev() {
        if k == 3 {
            continue;
        }
        if k < 2 {
            break;
        }
        s = s.wrapping_mul(2);
        s = s.wrapping_add(k);
    }
    s
}

/// `first_square_over` as Rust writes it.
fn first_square_over(limit: u16) -> u16 {
    let mut i = 1u16;
    loop {
        if i.wrapping_mul(i) > limit {
            break i;
        }
        i = i.wrapping_add(1);
    }
}

/// `find_root` as Rust writes it.
fn find_root(n: u16) -> u16 {
    for k in 0..n {
        if k.wrapping_mul(k) == 49 {
            return k;
        }
    }
    255
}

#[test]
fn known_control_prints_plain_rust_and_runs() {
    // CV-01 to CV-03: a mutable local, compound assignment, `while`, `for` (inclusive and
    // reversed), `loop` with a `break` value, `continue`, `break` and `return` from inside a loop,
    // on integers and on a real
    let ir = library();
    let functions = printed(&ir);
    let text = functions.join("\n");

    // the text is plain Rust, its path and origin in the comment above each function, and each
    // loop counted by its own `rt::Trips` naming where the loop was written (Q5 (b))
    assert!(text.contains("// fn sum_to @ control.msl:1:1"), "{text}");
    assert!(text.contains("let mut s = rt::UInt::<16>::new(0);"), "{text}");
    assert!(text.contains("while i < n {"), "{text}");
    assert!(text.contains("s = s + i;"), "{text}");
    assert!(text.contains("let mut trips_ = rt::Trips::new(\"while @ control.msl:3:1\");"), "{text}");
    assert!(text.contains("for index_ in (rt::UInt::<16>::new(0).get()..=n.get()).rev() {"), "{text}");
    assert!(text.contains("continue"), "{text}");
    assert!(text.contains("break i"), "{text}");
    assert!(text.contains("return k"), "{text}");
    assert!(text.contains("y = y * R::constant(2.0);"), "{text}");

    let u = |v: u16| format!("rt::UInt::<16>::new({v})");
    let (inputs, mut main, mut expected) = ([0u16, 1, 2, 3, 4, 10, 50, 300], String::from("fn main() {\n"), Vec::new());
    for n in inputs {
        main += &format!("    println!(\"sum_to {n} {{}}\", sum_to({}).get());\n", u(n));
        expected.push(format!("sum_to {n} {}", sum_to(n)));
        main += &format!("    println!(\"countdown {n} {{}}\", countdown({}).get());\n", u(n));
        expected.push(format!("countdown {n} {}", countdown(n)));
        main += &format!("    println!(\"first_square_over {n} {{}}\", first_square_over({}).get());\n", u(n));
        expected.push(format!("first_square_over {n} {}", first_square_over(n)));
        main += &format!("    println!(\"find_root {n} {{}}\", find_root({}).get());\n", u(n));
        expected.push(format!("find_root {n} {}", find_root(n)));
    }
    main += "    println!(\"doubled {}\", doubled::<f64>(1.5, rt::UInt::<8>::new(5)));\n}\n";
    expected.push(format!("doubled {}", 1.5f64 * 32.0));
    let out = Probe::run(&functions, &main);
    assert_eq!(out.lines().collect::<Vec<_>>(), expected.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn a_known_loop_past_the_bound_panics_naming_it() {
    // Q5 (b): a loop that never ends is a panic naming the loop, not a hang: `loop {}` made of
    // `let mut i = 0; loop { i += 1; }` at control.msl:50
    let u16 = Ty::u(16);
    let mut b = IrBuilder::new();
    let locals = vec![Functions::local("i", u16.clone(), true)];
    let mut x = Dx::new(&mut b, FILE, 51, 5);
    let zero = x.int(0, u16.clone());
    let let_i = x.let_(0, zero);
    let one = x.int(1, u16.clone());
    let step = Functions::assign(&mut b, 0, Some(BinaryOp::Add), one);
    let looped = Functions::control(&mut b, Expr::Loop { body: vec![step] }, ValueType::Unit, 52);
    let looped = b.stmt(Stmt::Expr(looped));
    let mut x = Dx::new(&mut b, FILE, 53, 5);
    let body = x.block(vec![let_i, looped], None, ValueType::Unit);
    Functions::define(&mut b, "spin", 50, locals, 0, ValueType::Unit, body);
    let ir = Alone::finish(b);
    let main = "fn main() {\n    let caught = std::panic::catch_unwind(|| spin());\n    let payload = caught.err().expect(\"the loop panics\");\n    println!(\"{}\", payload.downcast_ref::<String>().expect(\"text\"));\n}\n";
    let out = Probe::run(&printed(&ir), main);
    assert!(out.contains("loop @ control.msl:52:1") && out.contains("16777216"), "{out}");
}
