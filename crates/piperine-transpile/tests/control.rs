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

use std::sync::{Mutex, OnceLock};

use alone::Alone;
use digital::{Buck, Dx, Ty};
use piperine_abi::caller::{Commit, Handle};
use piperine_abi::raw::{Module, cell, commit_flag, status};
use piperine_ir::{BinaryOp, BodyId, CarrierRole, Expr, ExprId, FunctionId, FunctionSpec, Ir, IrBuilder, Local, Origin, Place, Stmt, StmtId, ValueType};
use piperine_plan::Modules;
use piperine_transpile::print::ValuePrinter;
use piperine_transpile::{Compiled, Settings, Transpiler};
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

    /// `spin` and `clamp_up`, over values that may be unknown, and the MSL each transcribes.
    fn unknown_library() -> Ir {
        let (byte, flag) = (Ty::flagged(Ty::u(8)), Ty::flagged(ValueType::Bool));
        let mut b = IrBuilder::new();

        // control.msl:60  fn clamp_up(c: Bit, a: Bit<u8>) -> Bit<u8> { let mut x = a; if c { x = 5; } x }
        let locals = vec![Functions::local("c", flag.clone(), false), Functions::local("a", byte.clone(), false), Functions::local("x", byte.clone(), true)];
        let mut x = Dx::new(&mut b, FILE, 61, 5);
        let a = x.local(1, byte.clone());
        let let_x = x.let_(2, a);
        let c = x.local(0, flag.clone());
        let five = Lit::flagged(&mut x, 5);
        let set = Functions::assign(&mut b, 2, None, five);
        let mut x = Dx::new(&mut b, FILE, 62, 5);
        let branch = x.when(c, vec![set], None);
        let branch = x.stmt(branch);
        let result = x.local(2, byte.clone());
        let body = x.block(vec![let_x, branch], Some(result), byte.clone());
        Functions::define(&mut b, "clamp_up", 60, locals, 2, byte.clone(), body);

        // control.msl:70 and 80, one function each over the same loop:
        //     fn spin(stop: Bit) -> Bit<u8> {
        //         let mut i = 0; let mut tag = 0;
        //         loop { i += 1; tag = 7; if stop { break; } if i == 3 { break; } }
        //         i                                   // `spin_tag` returns `tag`
        //     }
        for (name, line, answer) in [("spin", 70, 1u32), ("spin_tag", 80, 2u32)] {
            let locals = vec![Functions::local("stop", flag.clone(), false), Functions::local("i", byte.clone(), true), Functions::local("tag", byte.clone(), true)];
            let mut x = Dx::new(&mut b, FILE, line + 1, 5);
            let (zero, zero_again) = (Lit::flagged(&mut x, 0), Lit::flagged(&mut x, 0));
            let (let_i, let_tag) = (x.let_(1, zero), x.let_(2, zero_again));
            let (one, seven) = (x.int(1, Ty::u(8)), Lit::flagged(&mut x, 7));
            let count = Functions::assign(&mut b, 1, Some(BinaryOp::Add), one);
            let mark = Functions::assign(&mut b, 2, None, seven);
            let mut x = Dx::new(&mut b, FILE, line + 2, 9);
            let stop = x.local(0, flag.clone());
            let leave = Functions::control(&mut b, Expr::Break(None), ValueType::Unit, line + 2);
            let leave = b.stmt(Stmt::Expr(leave));
            let mut x = Dx::new(&mut b, FILE, line + 2, 9);
            let if_stop = x.when(stop, vec![leave], None);
            let if_stop = x.stmt(if_stop);
            let (i, three) = (x.local(1, byte.clone()), x.int(3, Ty::u(8)));
            let is_three = x.bin(BinaryOp::Eq, i, three, flag.clone());
            let leave = Functions::control(&mut b, Expr::Break(None), ValueType::Unit, line + 3);
            let leave = b.stmt(Stmt::Expr(leave));
            let mut x = Dx::new(&mut b, FILE, line + 3, 9);
            let if_three = x.when(is_three, vec![leave], None);
            let if_three = x.stmt(if_three);
            let looped = Functions::control(&mut b, Expr::Loop { body: vec![count, mark, if_stop, if_three] }, ValueType::Unit, line + 2);
            let looped = b.stmt(Stmt::Expr(looped));
            let mut x = Dx::new(&mut b, FILE, line + 5, 5);
            let result = x.local(answer, byte.clone());
            let body = x.block(vec![let_i, let_tag, looped], Some(result), byte.clone());
            Functions::define(&mut b, name, line, locals, 1, byte.clone(), body);
        }
        Alone::finish(b)
    }

    /// The IR of `fact`, `twice` and `demo`, with the MSL each transcribes.
    fn function_library() -> Ir {
        let (u16, boolean) = (Ty::u(16), ValueType::Bool);
        let function = ValueType::Fn(vec![u16.clone()], Box::new(u16.clone()));
        let mut b = IrBuilder::new();

        // control.msl:100  fn fact(n: u16) -> u16 { if n < 2 { 1 } else { n * fact(n - 1) } }
        let locals = vec![Functions::local("n", u16.clone(), false)];
        let mut x = Dx::new(&mut b, FILE, 101, 5);
        let (n, two) = (x.local(0, u16.clone()), x.int(2, u16.clone()));
        let small = x.bin(BinaryOp::Lt, n, two, boolean.clone());
        let one = x.int(1, u16.clone());
        let (n, n_again, one_again) = (x.local(0, u16.clone()), x.local(0, u16.clone()), x.int(1, u16.clone()));
        let below = x.bin(BinaryOp::Sub, n_again, one_again, u16.clone());
        let rest = x.call(FunctionId(0), vec![below], u16.clone());
        let product = x.bin(BinaryOp::Mul, n, rest, u16.clone());
        let body = x.if_(small, one, product, u16.clone());
        Functions::define(&mut b, "fact", 100, locals, 1, u16.clone(), body);

        // control.msl:110  fn twice(f: fn(u16) -> u16, x: u16) -> u16 { f(f(x)) }
        let locals = vec![Functions::local("f", function.clone(), false), Functions::local("x", u16.clone(), false)];
        let mut x = Dx::new(&mut b, FILE, 111, 5);
        let (f, f_again, value) = (x.local(0, function.clone()), x.local(0, function.clone()), x.local(1, u16.clone()));
        let inner = Functions::control(&mut b, Expr::CallValue { callee: f_again, args: vec![value] }, u16.clone(), 111);
        let body = Functions::control(&mut b, Expr::CallValue { callee: f, args: vec![inner] }, u16.clone(), 111);
        Functions::define(&mut b, "twice", 110, locals, 2, u16.clone(), body);

        // control.msl:120  fn demo(x: u16) -> u16 { let add = |y| y + x; twice(add, 1) + twice(fact, 3) }
        let locals = vec![Functions::local("x", u16.clone(), false), Functions::local("add", function.clone(), false), Functions::local("y", u16.clone(), false)];
        let mut x = Dx::new(&mut b, FILE, 121, 5);
        let (y, captured) = (x.local(2, u16.clone()), x.local(0, u16.clone()));
        let sum = x.bin(BinaryOp::Add, y, captured, u16.clone());
        let closure = Functions::control(&mut b, Expr::Closure { params: vec![2], body: sum }, function.clone(), 121);
        let let_add = b.stmt(Stmt::Let { local: 1, init: closure });
        let mut x = Dx::new(&mut b, FILE, 121, 30);
        let (add, one) = (x.local(1, function.clone()), x.int(1, u16.clone()));
        let first = x.call(FunctionId(1), vec![add, one], u16.clone());
        let reference = Functions::control(&mut b, Expr::FunctionRef(FunctionId(0)), function.clone(), 122);
        let mut x = Dx::new(&mut b, FILE, 122, 5);
        let three = x.int(3, u16.clone());
        let second = x.call(FunctionId(1), vec![reference, three], u16.clone());
        let total = x.bin(BinaryOp::Add, first, second, u16.clone());
        let body = x.block(vec![let_add], Some(total), u16.clone());
        Functions::define(&mut b, "demo", 120, locals, 1, u16.clone(), body);

        // control.msl:130  fn forever(n: u16) -> u16 { forever(n + 1) }
        let locals = vec![Functions::local("n", u16.clone(), false)];
        let mut x = Dx::new(&mut b, FILE, 131, 5);
        let (n, one) = (x.local(0, u16.clone()), x.int(1, u16.clone()));
        let next = x.bin(BinaryOp::Add, n, one, u16.clone());
        let body = x.call(FunctionId(3), vec![next], u16.clone());
        Functions::define(&mut b, "forever", 130, locals, 1, u16.clone(), body);
        Alone::finish(b)
    }

    /// The printed text of every function of `ir`, in the order they were added.
    fn printed(ir: &Ir) -> Vec<String> {
        ir.functions().map(|f| ValuePrinter::function(ir, f.id()).unwrap_or_else(|e| panic!("{e}"))).collect()
    }
}

/// `sum_to`, `countdown`, `first_square_over`, `find_root` and `fact` as Rust writes them.
struct RustReference;

impl RustReference {
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

    /// `fact` as Rust writes it, on `u16`.
    fn fact(n: u16) -> u16 {
        if n < 2 { 1 } else { n.wrapping_mul(RustReference::fact(n - 1)) }
    }
}

#[test]
fn known_control_prints_plain_rust_and_runs() {
    // CV-01 to CV-03: a mutable local, compound assignment, `while`, `for` (inclusive and
    // reversed), `loop` with a `break` value, `continue`, `break` and `return` from inside a loop,
    // on integers and on a real
    let ir = Functions::library();
    let functions = Functions::printed(&ir);
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
        expected.push(format!("sum_to {n} {}", RustReference::sum_to(n)));
        main += &format!("    println!(\"countdown {n} {{}}\", countdown({}).get());\n", u(n));
        expected.push(format!("countdown {n} {}", RustReference::countdown(n)));
        main += &format!("    println!(\"first_square_over {n} {{}}\", first_square_over({}).get());\n", u(n));
        expected.push(format!("first_square_over {n} {}", RustReference::first_square_over(n)));
        main += &format!("    println!(\"find_root {n} {{}}\", find_root({}).get());\n", u(n));
        expected.push(format!("find_root {n} {}", RustReference::find_root(n)));
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
    let out = Probe::run(&Functions::printed(&ir), main);
    assert!(out.contains("loop @ control.msl:52:1") && out.contains("16777216"), "{out}");
}

// ---- T9: unknown decisions (CV-08) ---------------------------------------------------------------

/// Literals of the IR's own forms.
struct Lit;

impl Lit {
    /// A `u8` literal that may be unknown, known to be `value`: `merge(value, value)`, which the IR
    /// types `Flagged<u8>` (§22.1.2 rule 2).
    fn flagged(x: &mut Dx<'_>, value: i128) -> ExprId {
        let (a, b) = (x.int(value, Ty::u(8)), x.int(value, Ty::u(8)));
        x.merge(a, b, Ty::flagged(Ty::u(8)))
    }
}

/// A value of the model that may be unknown: `None` is unknown.
type Level = Option<u64>;

/// A value that may be unknown, as the model computes it and as the generated program writes it.
struct LevelText;

impl LevelText {
    /// The merge of two values that may be unknown (§3.3.3): equal known values are kept.
    fn merged(a: Level, b: Level) -> Level {
        if a == b { a } else { None }
    }

    fn show(v: Level) -> String {
        v.map_or("?".to_string(), |v| v.to_string())
    }

    /// The program text of a `Flagged<UInt<8>>` that is `v`.
    fn byte_text(v: Level) -> String {
        v.map_or("rt::Flagged::<rt::UInt<8>>::unknown()".to_string(), |v| format!("rt::Flagged::known(rt::UInt::<8>::new({v}))"))
    }

    /// The program text of a `Flagged<bool>` that is `v`.
    fn flag_text(v: Option<bool>) -> String {
        v.map_or("rt::Flagged::<bool>::unknown()".to_string(), |v| format!("rt::Flagged::known({v})"))
    }
}

#[test]
fn unknown_decisions_merge_written_locals_and_loop_exits() {
    // §3.3.3: `if c { x = 5; } x` with `c` unknown is the merge of both paths' `x`; a loop whose
    // `break` decision is unknown leaves at each iteration that may be the last, and the code after
    // it sees the merge of those exits, each local merged level by level
    let ir = Functions::unknown_library();
    let functions = Functions::printed(&ir);
    let text = functions.join("\n");
    // the arms hand back their value and the locals they write; the loop is `rt::Unrolled`
    assert!(text.contains(".choose(|| {"), "{text}");
    assert!(text.contains("let (_, merge_out0_) ="), "{text}");
    assert!(text.contains("rt::Unrolled::<_, (), ()>::run(\"loop @ control.msl:72:1\", (i, tag), |(mut i, mut tag)| {"), "{text}");
    assert!(text.contains("outcome_.exit()"), "{text}");

    let (mut main, mut expected) = (String::from("fn show(v: rt::Flagged<rt::UInt<8>>) -> String {\n    if v.is_known() { v.known_or(rt::UInt::<8>::new(0)).get().to_string() } else { \"?\".to_string() }\n}\nfn main() {\n"), Vec::new());
    // clamp_up: the model is the same `if` over `Level`
    for c in [Some(true), Some(false), None] {
        for a in [Some(5), Some(6), None] {
            main += &format!("    println!(\"clamp_up {{}}\", show(clamp_up({}, {})));\n", LevelText::flag_text(c), LevelText::byte_text(a));
            let then = Some(5);
            let model = match c {
                Some(true) => then,
                Some(false) => a,
                None => LevelText::merged(then, a),
            };
            expected.push(format!("clamp_up {}", LevelText::show(model)));
        }
    }
    // spin: the loop leaves at iteration 1 when `stop`, otherwise at 3; `tag` is 7 on every exit
    for stop in [Some(true), Some(false), None] {
        main += &format!("    println!(\"spin {{}}\", show(spin({})));\n", LevelText::flag_text(stop));
        main += &format!("    println!(\"spin_tag {{}}\", show(spin_tag({})));\n", LevelText::flag_text(stop));
        let count = match stop {
            Some(true) => Some(1),
            Some(false) => Some(3),
            None => LevelText::merged(Some(1), Some(3)),
        };
        expected.push(format!("spin {}", LevelText::show(count)));
        expected.push(format!("spin_tag {}", LevelText::show(LevelText::merged(Some(7), Some(7)))));
    }
    main += "}\n";
    let out = Probe::run(&functions, &main);
    assert_eq!(out.lines().collect::<Vec<_>>(), expected.iter().map(String::as_str).collect::<Vec<_>>());
}

/// The compiled `Looper` design, built once.
struct Design {
    compiled: Compiled,
    ir: std::sync::Arc<Ir>,
    _cache: tempfile::TempDir,
}

impl Design {
    /// The `Looper` body (control.msl:90): a loop that commits, with an exit on an input that may be
    /// unknown.
    ///
    /// ```text
    /// on clk.rise() {
    ///     let mut i = 0;
    ///     loop { q <- i; if stop { break; } i += 1; if i == 2 { break; } }
    /// }
    /// ```
    fn looper(b: &mut IrBuilder) -> BodyId {
        let byte = Ty::flagged(Ty::u(8));
        let body = b.body(Buck::spec("Looper", FILE, 90));
        for (name, ty, role, col) in [("clk", ValueType::Bool, CarrierRole::In, 20), ("stop", ValueType::Bool, CarrierRole::In, 30), ("q", Ty::u(8), CarrierRole::Out, 40)] {
            b.carrier(body, Buck::carrier(name, ty, role, None, FILE, 90, col));
        }
        let rise = Buck::rise(b, body, 0, FILE, 91, 8);
        let mut x = Dx::new(b, FILE, 92, 9);
        let zero = Lit::flagged(&mut x, 0);
        let let_i = x.let_(0, zero);
        let i = x.local(0, byte.clone());
        let commit = x.commit(2, i, None);
        let stop = x.carrier(1, ValueType::Bool);
        let leave = Functions::control(b, Expr::Break(None), ValueType::Unit, 94);
        let leave = b.stmt(Stmt::Expr(leave));
        let mut x = Dx::new(b, FILE, 94, 9);
        let if_stop = x.when(stop, vec![leave], None);
        let if_stop = x.stmt(if_stop);
        let one = x.int(1, Ty::u(8));
        let step = Functions::assign(b, 0, Some(BinaryOp::Add), one);
        let mut x = Dx::new(b, FILE, 95, 9);
        let (i, two) = (x.local(0, byte.clone()), x.int(2, Ty::u(8)));
        let is_two = x.bin(BinaryOp::Eq, i, two, Ty::flagged(ValueType::Bool));
        let leave = Functions::control(b, Expr::Break(None), ValueType::Unit, 95);
        let leave = b.stmt(Stmt::Expr(leave));
        let mut x = Dx::new(b, FILE, 95, 9);
        let if_two = x.when(is_two, vec![leave], None);
        let if_two = x.stmt(if_two);
        let looped = Functions::control(b, Expr::Loop { body: vec![commit, if_stop, step, if_two] }, ValueType::Unit, 93);
        let looped = b.stmt(Stmt::Expr(looped));
        Buck::reaction(b, body, vec![rise], vec![let_i, looped], vec![Functions::local("i", byte, true)], FILE, 91);
        body
    }

    fn shared() -> &'static Mutex<Design> {
        static DESIGN: OnceLock<Mutex<Design>> = OnceLock::new();
        DESIGN.get_or_init(|| {
            let mut b = IrBuilder::new();
            Design::looper(&mut b);
            let ir = std::sync::Arc::new(Alone::finish(b));
            let cache = tempfile::tempdir().expect("a temp dir");
            let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
            let compiled = Transpiler::new(ir.clone()).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(compiled.report().warnings, "", "the generated source compiles with no warning");
            Mutex::new(Design { compiled, ir, _cache: cache })
        })
    }

    fn module() -> &'static Module {
        let design = Design::shared().lock().unwrap_or_else(|e| e.into_inner());
        let body = design.ir.bodies().find(|b| b.name() == "Looper").expect("the body").id();
        Modules::module(&design.compiled, body).expect("Looper has a module")
    }
}

/// The reaction of the `Looper` design, run with an input that may be unknown.
struct LooperReaction;

impl LooperReaction {
    /// The bytes of a `Bit<u8>` or `Bit` pin holding `value` (known), or unknown (two zero bytes).
    fn pin(value: Option<u8>) -> Vec<u8> {
        value.map_or(vec![0, 0], |v| vec![v, 1])
    }

    /// Runs the reaction with `stop` and returns what it committed: statement, bytes, delay, flags.
    fn run_reaction(stop: Option<u8>) -> Vec<(u32, Vec<u8>, f64, u32)> {
        let mut handle = Handle::create(Design::module(), &[]).expect("an instance");
        let shape = handle.shape();
        let reaction = *shape.reactions.get(0).expect("one reaction");
        let mut input: Vec<Vec<u8>> = Vec::new();
        for c in (0..reaction.reads.len()).map(|i| *reaction.reads.get(i).expect("a read")) {
            assert_eq!(c.kind, cell::PIN, "a reaction of this body reads pins");
            let name = shape.pins.get(c.index as usize).expect("a pin").name.as_str().to_string();
            input.push(match name.as_str() {
                "clk" => LooperReaction::pin(Some(1)),
                "stop" => LooperReaction::pin(stop),
                "resolution" => 1e-15f64.to_le_bytes().iter().copied().chain([1]).chain([0; 7]).collect(),
                other => panic!("the reaction reads `{other}`"),
            });
        }
        let reads: Vec<&[u8]> = input.iter().map(Vec::as_slice).collect();
        let mut made = Vec::new();
        handle
            .react(0, &reads, &mut |c: Commit<'_>| {
                made.push((c.statement, c.bytes.to_vec(), c.after, c.flags));
                status::OK
            })
            .expect("the reaction runs");
        made
    }
}

#[test]
fn a_reaction_loop_committing_after_an_unknown_decision_commits_maybe() {
    // MD-70, §13 rule 1: the first iteration commits `q <- 0` before any decision (definite); the
    // `stop` that may be unknown splits the loop, and what the iterations after it commit may not
    // happen, so `q <- 1` is a `MAYBE | MERGE` commit. With `stop` known the loop is Rust's and
    // every commit is plain
    let maybe_merge = commit_flag::MAYBE | commit_flag::MERGE;
    assert_eq!(LooperReaction::run_reaction(None), [(0, LooperReaction::pin(Some(0)), 0.0, 0), (0, LooperReaction::pin(Some(1)), 0.0, maybe_merge)]);
    assert_eq!(LooperReaction::run_reaction(Some(0)), [(0, LooperReaction::pin(Some(0)), 0.0, 0), (0, LooperReaction::pin(Some(1)), 0.0, 0)]);
    assert_eq!(LooperReaction::run_reaction(Some(1)), [(0, LooperReaction::pin(Some(0)), 0.0, 0)]);
}

// ---- T10: function values and recursion (CV-05 to CV-07) -------------------------------------------------

#[test]
fn function_values_and_recursion_compile_and_run() {
    // §1.5.10, §1.5.14, §3 rule 12: a function used as a value, called through it, a closure
    // capturing by value, and a recursive function; the recursive function carries a depth, the
    // others keep their text
    let ir = Functions::function_library();
    let functions = Functions::printed(&ir);
    let text = functions.join("\n");
    assert!(text.contains("fn fact(n: rt::UInt<16>, depth_: rt::Depth) -> rt::UInt<16> {"), "{text}");
    assert!(text.contains("fact(n - rt::UInt::<16>::new(1), depth_.deeper(\"fact\"))"), "{text}");
    assert!(text.contains("fn twice(f: &dyn Fn(rt::UInt<16>) -> rt::UInt<16>, x: rt::UInt<16>) -> rt::UInt<16> {"), "{text}");
    assert!(text.contains("let add: &dyn Fn(rt::UInt<16>) -> rt::UInt<16> = &move |y| y + x;"), "{text}");
    assert!(text.contains("&move |arg0_| fact(arg0_, rt::Depth::root())"), "{text}");

    let mut main = String::from("fn main() {\n");
    let mut expected = Vec::new();
    for n in [0u16, 1, 5, 8] {
        main += &format!("    println!(\"fact {n} {{}}\", fact(rt::UInt::<16>::new({n}), rt::Depth::root()).get());\n");
        expected.push(format!("fact {n} {}", RustReference::fact(n)));
        main += &format!("    println!(\"demo {n} {{}}\", demo(rt::UInt::<16>::new({n})).get());\n");
        // twice(add, 1) is 1 + x + x; twice(fact, 3) is fact(fact(3))
        expected.push(format!("demo {n} {}", (1 + 2 * n).wrapping_add(RustReference::fact(RustReference::fact(3)))));
    }
    main += "}\n";
    let out = Probe::run(&functions, &main);
    assert_eq!(out.lines().collect::<Vec<_>>(), expected.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn a_recursion_past_the_depth_bound_faults_by_name() {
    // Q5 (b): a recursion that never ends is a panic naming the function, not a stack overflow
    let ir = Functions::function_library();
    let main = "fn main() {\n    let caught = std::panic::catch_unwind(|| forever(rt::UInt::<16>::new(0), rt::Depth::root()));\n    let payload = caught.err().expect(\"the recursion panics\");\n    println!(\"{}\", payload.downcast_ref::<String>().expect(\"text\"));\n}\n";
    let out = Probe::run(&Functions::printed(&ir), main);
    assert!(out.contains("forever") && out.contains("1024"), "{out}");
}
