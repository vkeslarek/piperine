//! Scope: the control forms of the IR (composite-values phase A, CV-01 to CV-07): a mutable local
//! and its assignment, `loop`/`while`/`for`, `break` and `continue`, `return`, function values and
//! recursion. Each form is built by hand, checked (valid and refused), printed, and walked by the
//! dependency analysis, whose fixed points (a loop's later iteration, a recursive function's
//! result) are what these tests pin.

#[path = "support/digital.rs"]
#[allow(dead_code)]
mod digital;
#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

#[path = "support/shape.rs"]
#[allow(dead_code)]
mod shape;

use digital::{Buck, Dx, Ty};
use shape::{Entries, Shape, Written, FILE};
use piperine_ir::{
    Addend, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, Constant, Deps, Env, Expr, FunctionId, InstanceSpec, IrBuilder, NodeRef,
    NodeRole, Operator, Origin, Param, QuantityRole, Real, Stmt, Target, Terminal, ValueType,
};

// ---- T2: mutable locals and assignment (CV-01) -------------------------------------------------------------

#[test]
fn an_assignment_to_a_mutable_local_checks_and_prints() {
    // `let mut x = 1.0; x = 2.0; x += v(p, n);` (§1.5.12), then the contribution reads `x`.
    let ir = Shape::ir(|b| {
        let init = Shape::dx(b, 2).real(1.0);
        let declare = Shape::dx(b, 2).let_(0, init);
        let two = Shape::dx(b, 3).real(2.0);
        let set = Shape::assign(b, 0, None, two);
        let probe = Shape::probe(b, 4);
        let add = Shape::assign(b, 0, Some(BinaryOp::Add), probe);
        let read = Shape::dx(b, 5).local(0, ValueType::F64);
        Written { locals: vec![Shape::mutable("x", ValueType::F64)], stmts: vec![declare, set, add], value: read }
    });
    assert_eq!(ir.check(), vec![], "a mutable local is assigned");
    let text = ir.to_string();
    assert!(text.contains("local mut x: f64"), "the table says `mut`:\n{text}");
    assert!(text.contains("x = 2"), "the assignment prints as source:\n{text}");
    assert!(text.contains("x += probe(p, n)"), "the compound assignment prints as source:\n{text}");
    assert!(text.contains("assign 0 - e") && text.contains("assign 0 add e"), "the arena lines show the target and the operator:\n{text}");
}

#[test]
fn an_assignment_to_an_immutable_local_is_refused() {
    // `let x = 1.0; x = 2.0;` (§1.5.12: assignment only to a `let mut`).
    let ir = Shape::ir(|b| {
        let init = Shape::dx(b, 2).real(1.0);
        let declare = Shape::dx(b, 2).let_(0, init);
        let two = Shape::dx(b, 3).real(2.0);
        let set = Shape::assign(b, 0, None, two);
        let read = Shape::dx(b, 4).local(0, ValueType::F64);
        Written { locals: vec![Shape::local("x", ValueType::F64)], stmts: vec![declare, set], value: read }
    });
    let broken = ir.check();
    let hits = Entries::with(&broken, "not `mutable`");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].origin, Origin { file: FILE.into(), line: 3, col: 5 }, "the entry names the assigned value's position");
}

#[test]
fn an_assignment_is_typed_by_its_target() {
    // A value of another type, and a compound operator the target type lacks (`bool += bool`,
    // `f64 <= f64`: a comparison has no compound form), are refused.
    let ir = Shape::ir(|b| {
        let init = Shape::dx(b, 2).real(1.0);
        let declare = Shape::dx(b, 2).let_(0, init);
        let flag = Shape::dx(b, 3).boolean(true);
        let mismatched = Shape::assign(b, 0, None, flag);
        let one = Shape::dx(b, 4).real(1.0);
        let comparison = Shape::assign(b, 0, Some(BinaryOp::Le), one);
        let read = Shape::dx(b, 5).local(0, ValueType::F64);
        Written { locals: vec![Shape::mutable("x", ValueType::F64)], stmts: vec![declare, mismatched, comparison], value: read }
    });
    let broken = ir.check();
    assert_eq!(Entries::with(&broken, "the value of an assignment must be f64, found bool").len(), 1, "{broken:?}");
    assert_eq!(Entries::with(&broken, "`Le` does not apply").len(), 1, "{broken:?}");
}

#[test]
fn an_assignment_joins_what_it_reads_into_the_local() {
    // `let mut x = 0.0; x = v(p, n);` then the contribution of `x`: the flow reads both nodes,
    // which the `let` alone (a constant) would not say.
    let ir = Shape::ir(|b| {
        let zero = Shape::dx(b, 2).real(0.0);
        let declare = Shape::dx(b, 2).let_(0, zero);
        let probe = Shape::probe(b, 3);
        let set = Shape::assign(b, 0, None, probe);
        let read = Shape::dx(b, 4).local(0, ValueType::F64);
        Written { locals: vec![Shape::mutable("x", ValueType::F64)], stmts: vec![declare, set], value: read }
    });
    assert_eq!(ir.check(), vec![]);
    assert_eq!(Shape::reads(&ir), Deps { nodes: vec![0, 1], ..Deps::default() });
}

// ---- T3: loops, `break` and `continue` (CV-02) -----------------------------------------------------------------

#[test]
fn loops_check_type_and_print() {
    // `let x = loop { break 1.0 };  while true { break }  for k in 0..4 { continue }` and the
    // inclusive, reversed range `for k in (0..=4).rev()`.
    let ir = Shape::ir(|b| {
        let one = Shape::dx(b, 2).real(1.0);
        let leave = Shape::expr(b, Expr::Break(Some(one)), ValueType::F64, 2);
        let leave = b.stmt(Stmt::Expr(leave));
        let looped = Shape::expr(b, Expr::Loop { body: vec![leave] }, ValueType::F64, 2);
        let declare = Shape::dx(b, 2).let_(0, looped);

        let always = Shape::dx(b, 3).boolean(true);
        let stop = Shape::expr(b, Expr::Break(None), ValueType::Unit, 3);
        let stop = b.stmt(Stmt::Expr(stop));
        let whiled = Shape::expr(b, Expr::While { cond: always, body: vec![stop] }, ValueType::Unit, 3);
        let whiled = b.stmt(Stmt::Expr(whiled));

        let (low, high) = {
            let mut x = Shape::dx(b, 4);
            (x.int(0, Ty::u(8)), x.int(4, Ty::u(8)))
        };
        let skip = Shape::expr(b, Expr::Continue, ValueType::Unit, 4);
        let skip = b.stmt(Stmt::Expr(skip));
        let ranged = Shape::expr(b, Expr::ForRange { local: 1, low, high, inclusive: false, reverse: false, body: vec![skip] }, ValueType::Unit, 4);
        let ranged = b.stmt(Stmt::Expr(ranged));

        let (low, high) = {
            let mut x = Shape::dx(b, 5);
            (x.int(0, Ty::u(8)), x.int(4, Ty::u(8)))
        };
        let reversed = Shape::expr(b, Expr::ForRange { local: 1, low, high, inclusive: true, reverse: true, body: vec![] }, ValueType::Unit, 5);
        let reversed = b.stmt(Stmt::Expr(reversed));

        let read = Shape::dx(b, 6).local(0, ValueType::F64);
        Written { locals: vec![Shape::local("x", ValueType::F64), Shape::local("k", Ty::u(8))], stmts: vec![declare, whiled, ranged, reversed], value: read }
    });
    assert_eq!(ir.check(), vec![], "loops of every kind check");
    let text = ir.to_string();
    for printed in ["loop { break 1", "while true { break }", "for k in 0u8..4u8 { continue }", "for k in (0u8..=4u8).rev() {  }"] {
        assert!(text.contains(printed), "`{printed}` in:\n{text}");
    }

    // A `loop` is typed by its breaks: a value of another type is refused, and so is the bounds
    // of a range that are not of one integer type.
    let ir = Shape::ir(|b| {
        let one = Shape::dx(b, 2).real(1.0);
        let leave = Shape::expr(b, Expr::Break(Some(one)), ValueType::Bool, 2);
        let leave = b.stmt(Stmt::Expr(leave));
        let looped = Shape::expr(b, Expr::Loop { body: vec![leave] }, ValueType::Bool, 2);
        let declare = Shape::dx(b, 2).let_(0, looped);
        let (low, high) = {
            let mut x = Shape::dx(b, 3);
            (x.int(0, Ty::u(8)), x.int(4, Ty::u(9)))
        };
        let ranged = Shape::expr(b, Expr::ForRange { local: 1, low, high, inclusive: false, reverse: false, body: vec![] }, ValueType::Unit, 3);
        let ranged = b.stmt(Stmt::Expr(ranged));
        let read = Shape::dx(b, 4).real(0.0);
        Written { locals: vec![Shape::local("b", ValueType::Bool), Shape::local("k", Ty::u(8))], stmts: vec![declare, ranged], value: read }
    });
    let broken = ir.check();
    assert_eq!(Entries::with(&broken, "the value of a `break` must be bool, found f64").len(), 1, "{broken:?}");
    assert_eq!(Entries::with(&broken, "bounds of a `for` range must have one integer type").len(), 1, "{broken:?}");
}

#[test]
fn break_outside_a_loop_is_refused() {
    // `break;` and `continue;` at the top of the block (§1.8), and `break 1.0` out of a `while`,
    // which only a `loop` may carry a value out of.
    let ir = Shape::ir(|b| {
        let stop = Shape::expr(b, Expr::Break(None), ValueType::Unit, 2);
        let stop = b.stmt(Stmt::Expr(stop));
        let skip = Shape::expr(b, Expr::Continue, ValueType::Unit, 3);
        let skip = b.stmt(Stmt::Expr(skip));
        let always = Shape::dx(b, 4).boolean(true);
        let one = Shape::dx(b, 4).real(1.0);
        let valued = Shape::expr(b, Expr::Break(Some(one)), ValueType::Unit, 4);
        let valued = b.stmt(Stmt::Expr(valued));
        let whiled = Shape::expr(b, Expr::While { cond: always, body: vec![valued] }, ValueType::Unit, 4);
        let whiled = b.stmt(Stmt::Expr(whiled));
        let value = Shape::dx(b, 5).real(0.0);
        Written { locals: vec![], stmts: vec![stop, skip, whiled], value }
    });
    let broken = ir.check();
    let outside = Entries::with(&broken, "outside a loop");
    assert_eq!(outside.len(), 2, "{broken:?}");
    assert_eq!(outside[0].origin.line, 2, "`break` is named where it stands");
    assert_eq!(outside[1].origin.line, 3, "`continue` is named where it stands");
    assert_eq!(Entries::with(&broken, "with a value out of a loop that is not `loop`").len(), 1, "{broken:?}");
}

#[test]
fn a_commit_in_a_loop_is_one_statement() {
    // `on clk.rise() { loop { q <- true; break } }`: the ABI numbers a commit once however
    // often the loop runs (§12.1.14), so the reaction has one statement, the commit.
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Latch", FILE, 1));
    b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, FILE, 1, 19));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), FILE, 1, 30));
    let rise = Buck::rise(&mut b, body, 0, FILE, 2, 22);
    let mut x = Dx::new(&mut b, FILE, 2, 37);
    let high = x.boolean(true);
    let commit = x.commit(1, high, None);
    let stop = Shape::expr(&mut b, Expr::Break(None), ValueType::Unit, 2);
    let stop = b.stmt(Stmt::Expr(stop));
    let looped = Shape::expr(&mut b, Expr::Loop { body: vec![commit, stop] }, ValueType::Unit, 2);
    let looped = b.stmt(Stmt::Expr(looped));
    Buck::reaction(&mut b, body, vec![rise], vec![looped], vec![], FILE, 2);
    let top = b.next_instance_id();
    let clk = b.net(piperine_ir::NetSpec { path: "Latch.clk".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: Shape::origin() });
    let q = b.net(piperine_ir::NetSpec { path: "Latch.q".into(), ty: ValueType::Bool, declared_by: top, init: Some(Constant::Bool(false)), origin: Shape::origin() });
    let top = b.instance(InstanceSpec { path: "Latch".into(), name: "Latch".into(), body, parent: None, nodes: vec![], params: vec![], origin: Shape::origin() });
    b.bind_nets(top, vec![clk, q]);
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![]);
    let statements = ir.instance_at("Latch").expect("placed").body().reaction_statements(0);
    assert_eq!(statements, vec![commit], "the commit in the loop body is the reaction's one statement");
}

#[test]
fn a_loop_reaching_a_probe_on_a_later_iteration_reads_it() {
    // `let mut prev = 0.0; let mut acc = 0.0; for k in 0..3 { acc = prev; prev = v(p, n); }`:
    // `acc` takes the probe through `prev` on the second iteration, so the contribution of `acc`
    // reads the nodes. Written in straight line (the same two assignments, no loop) it reads none.
    let looped = Shape::ir(|b| {
        let (zero, zero_again) = (Shape::dx(b, 2).real(0.0), Shape::dx(b, 3).real(0.0));
        let (prev, acc) = (Shape::dx(b, 2).let_(0, zero), Shape::dx(b, 3).let_(1, zero_again));
        let (before, probe) = (Shape::dx(b, 5).local(0, ValueType::F64), Shape::probe(b, 6));
        let (take, remember) = (Shape::assign(b, 1, None, before), Shape::assign(b, 0, None, probe));
        let (low, high) = {
            let mut x = Shape::dx(b, 4);
            (x.int(0, Ty::u(8)), x.int(3, Ty::u(8)))
        };
        let ranged = Shape::expr(b, Expr::ForRange { local: 2, low, high, inclusive: false, reverse: false, body: vec![take, remember] }, ValueType::Unit, 4);
        let ranged = b.stmt(Stmt::Expr(ranged));
        let read = Shape::dx(b, 7).local(1, ValueType::F64);
        let locals = vec![Shape::mutable("prev", ValueType::F64), Shape::mutable("acc", ValueType::F64), Shape::local("k", Ty::u(8))];
        Written { locals, stmts: vec![prev, acc, ranged], value: read }
    });
    assert_eq!(looped.check(), vec![]);
    assert_eq!(Shape::reads(&looped).nodes, vec![0, 1], "the second iteration reads what the first one stored");

    let straight = Shape::ir(|b| {
        let (zero, zero_again) = (Shape::dx(b, 2).real(0.0), Shape::dx(b, 3).real(0.0));
        let (prev, acc) = (Shape::dx(b, 2).let_(0, zero), Shape::dx(b, 3).let_(1, zero_again));
        let (before, probe) = (Shape::dx(b, 5).local(0, ValueType::F64), Shape::probe(b, 6));
        let (take, remember) = (Shape::assign(b, 1, None, before), Shape::assign(b, 0, None, probe));
        let read = Shape::dx(b, 7).local(1, ValueType::F64);
        Written { locals: vec![Shape::mutable("prev", ValueType::F64), Shape::mutable("acc", ValueType::F64)], stmts: vec![prev, acc, take, remember], value: read }
    });
    assert_eq!(straight.check(), vec![]);
    assert_eq!(Shape::reads(&straight).nodes, Vec::<u32>::new(), "without the loop `acc` took `prev` before the probe was stored");
}

#[test]
fn a_loop_trip_count_reaches_what_the_loop_assigns() {
    // `let mut n = 0.0; while v(p, n) < 1.0 { n += 1.0 }`: how often the body ran depends on the
    // probe, so `n` reads the nodes though the body only adds a constant.
    let ir = Shape::ir(|b| {
        let zero = Shape::dx(b, 2).real(0.0);
        let declare = Shape::dx(b, 2).let_(0, zero);
        let (probe, one) = (Shape::probe(b, 3), Shape::dx(b, 3).real(1.0));
        let cond = Shape::dx(b, 3).bin(BinaryOp::Lt, probe, one, ValueType::Bool);
        let step = Shape::dx(b, 4).real(1.0);
        let add = Shape::assign(b, 0, Some(BinaryOp::Add), step);
        let whiled = Shape::expr(b, Expr::While { cond, body: vec![add] }, ValueType::Unit, 3);
        let whiled = b.stmt(Stmt::Expr(whiled));
        let read = Shape::dx(b, 5).local(0, ValueType::F64);
        Written { locals: vec![Shape::mutable("n", ValueType::F64)], stmts: vec![declare, whiled], value: read }
    });
    assert_eq!(ir.check(), vec![]);
    assert_eq!(Shape::reads(&ir).nodes, vec![0, 1], "the condition decides the trip count (§3.3.3)");
}

// ---- T4: `return` (CV-03) ------------------------------------------------------------------------------

#[test]
fn a_return_checks_against_the_result_type() {
    // `fn early(x: f64) -> f64 { if x > 0.0 { return x; } 0.0 }` called from the contribution.
    let ir = Shape::ir(|b| {
        let early = {
            let mut x = Shape::dx(b, 9);
            let (read, zero) = (x.local(0, ValueType::F64), x.real(0.0));
            let positive = x.bin(BinaryOp::Gt, read, zero, ValueType::Bool);
            let result = x.local(0, ValueType::F64);
            let returned = Shape::expr(x.b, Expr::Return(Some(result)), ValueType::F64, 9);
            let returned = x.b.stmt(Stmt::Expr(returned));
            let guarded = Shape::dx(b, 9).when(positive, vec![returned], None);
            let guarded = Shape::dx(b, 9).stmt(guarded);
            let fallback = Shape::dx(b, 10).real(0.0);
            let body = Shape::dx(b, 9).block(vec![guarded], Some(fallback), ValueType::F64);
            Shape::function(b, "early", body)
        };
        let probe = Shape::probe(b, 3);
        let call = Shape::dx(b, 3).call(early, vec![probe], ValueType::F64);
        Written { locals: vec![], stmts: vec![], value: call }
    });
    assert_eq!(ir.check(), vec![], "a return of the result type checks");
    assert!(ir.to_string().contains("{ return x }"), "the return prints as source:\n{ir}");
    assert_eq!(Shape::reads(&ir).nodes, vec![0, 1], "what the function returns is what the call reads");

    // A `return` of another type, and one without a value in a function that returns `f64`.
    let ir = Shape::ir(|b| {
        let flag = Shape::dx(b, 9).boolean(true);
        let wrong = Shape::expr(b, Expr::Return(Some(flag)), ValueType::Bool, 9);
        let bare = Shape::expr(b, Expr::Return(None), ValueType::Unit, 10);
        let (wrong, bare) = (b.stmt(Stmt::Expr(wrong)), b.stmt(Stmt::Expr(bare)));
        let value = Shape::dx(b, 11).real(0.0);
        let body = Shape::dx(b, 9).block(vec![wrong, bare], Some(value), ValueType::F64);
        let wrong = Shape::function(b, "wrong", body);
        let probe = Shape::probe(b, 3);
        let call = Shape::dx(b, 3).call(wrong, vec![probe], ValueType::F64);
        Written { locals: vec![], stmts: vec![], value: call }
    });
    let broken = ir.check();
    assert_eq!(Entries::with(&broken, "the value of a `return` must be f64, found bool").len(), 1, "{broken:?}");
    assert_eq!(Entries::with(&broken, "`return` without a value in a function that returns f64").len(), 1, "{broken:?}");
}

#[test]
fn a_return_outside_a_function_is_refused() {
    // `return 1.0;` in the `analog` block of a body (§1.8: only a function returns).
    let ir = Shape::ir(|b| {
        let one = Shape::dx(b, 2).real(1.0);
        let returned = Shape::expr(b, Expr::Return(Some(one)), ValueType::F64, 2);
        let returned = b.stmt(Stmt::Expr(returned));
        let value = Shape::dx(b, 3).real(0.0);
        Written { locals: vec![], stmts: vec![returned], value }
    });
    let broken = ir.check();
    let hits = Entries::with(&broken, "`return` outside a function");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].origin.line, 2);
}

// ---- T5: function values and recursion (CV-06, CV-07) -----------------------------------------------

#[test]
fn a_function_value_is_called_through_its_reference() {
    // `let f = scale; f(v(p, n))` where `fn scale(x: f64) -> f64 { x * temperature() }`: the call
    // through the value reads the probe and what `scale` reads, the temperature.
    let ir = Shape::ir(|b| {
        let scale = {
            let mut x = Shape::dx(b, 9);
            let (read, temperature) = (x.local(0, ValueType::F64), Shape::temperature(b, 9));
            let body = Shape::dx(b, 9).bin(BinaryOp::Mul, read, temperature, ValueType::F64);
            Shape::function(b, "scale", body)
        };
        let reference = Shape::expr(b, Expr::FunctionRef(scale), ValueType::Fn(vec![ValueType::F64], Box::new(ValueType::F64)), 2);
        let probe = Shape::probe(b, 3);
        let call = Shape::expr(b, Expr::CallValue { callee: reference, args: vec![probe] }, ValueType::F64, 3);
        Written { locals: vec![], stmts: vec![], value: call }
    });
    assert_eq!(ir.check(), vec![], "a function value is called through its reference");
    assert!(ir.to_string().contains("scale(probe(p, n))"), "the call prints as source:\n{ir}");
    assert_eq!(Shape::reads(&ir), Deps { nodes: vec![0, 1], env: vec![Env::Temperature], ..Deps::default() });

    // The reference has the function's own type, and the call takes its arguments.
    let ir = Shape::ir(|b| {
        let scale = {
            let read = Shape::dx(b, 9).local(0, ValueType::F64);
            Shape::function(b, "scale", read)
        };
        let reference = Shape::expr(b, Expr::FunctionRef(scale), ValueType::Fn(vec![ValueType::Bool], Box::new(ValueType::F64)), 2);
        let call = Shape::expr(b, Expr::CallValue { callee: reference, args: vec![] }, ValueType::F64, 3);
        Written { locals: vec![], stmts: vec![], value: call }
    });
    let broken = ir.check();
    assert_eq!(Entries::with(&broken, "expression is typed fn(bool) -> f64, its form gives fn(f64) -> f64").len(), 1, "{broken:?}");
    assert_eq!(Entries::with(&broken, "calls a function value with 0 args, it takes 1").len(), 1, "{broken:?}");
}

/// `fn name(x) = if x > 0.0 { name(x - 1.0) + temperature() } else { x }`: the function is
/// recursive and reads what the recursion reaches.
struct Counting;

impl Counting {
    /// The function `name`, which must be function `me` of the design (the first one added):
    /// its body calls itself by that id.
    fn function(b: &mut IrBuilder, name: &str, me: FunctionId) -> FunctionId {
        let mut x = Shape::dx(b, 9);
        let (read, zero, one) = (x.local(0, ValueType::F64), x.real(0.0), x.real(1.0));
        let positive = x.bin(BinaryOp::Gt, read, zero, ValueType::Bool);
        let below = x.bin(BinaryOp::Sub, read, one, ValueType::F64);
        let again = x.call(me, vec![below], ValueType::F64);
        let temperature = Shape::temperature(b, 9);
        let sum = Shape::dx(b, 9).bin(BinaryOp::Add, again, temperature, ValueType::F64);
        let body = Shape::dx(b, 9).if_(positive, sum, read, ValueType::F64);
        Shape::function(b, name, body)
    }
}

#[test]
fn recursion_is_accepted_outside_analog() {
    // A param's `where` calls the recursive function; the `analog` block does not (§3 rule 12).
    let mut b = IrBuilder::new();
    let electrical = Buck::electrical(&mut b);
    let count = Counting::function(&mut b, "count", FunctionId(0));
    let check = {
        let mut x = Shape::dx(&mut b, 3);
        let (p, zero) = (x.param(0, ValueType::F64), x.real(0.0));
        let called = x.call(count, vec![p], ValueType::F64);
        x.bin(BinaryOp::Gt, called, zero, ValueType::Bool)
    };
    let value = Shape::dx(&mut b, 4).real(0.0);
    let contribute = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] });
    let port = |name: &str| BodyNode { name: name.into(), nature: electrical, role: NodeRole::Port, init: None, origin: Shape::origin() };
    let tolerance = Param { name: "n".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(3.0))), check: Some(check), structural: false, origin: Shape::origin() };
    let body = b.body(BodySpec {
        name: "Shape".into(),
        origin: Shape::origin(),
        params: vec![tolerance],
        nodes: vec![port("p"), port("n")],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: Some(vec![contribute]),
        children: vec![],
    });
    let top = b.instance(InstanceSpec {
        path: "Shape".into(),
        name: "Shape".into(),
        body,
        parent: None,
        nodes: vec![NodeRef::Ground, NodeRef::Ground],
        params: vec![Constant::Real(Real::new(3.0))],
        origin: Shape::origin(),
    });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "a recursion the `analog` block does not reach is accepted");
    // The walk reaches its fixed point: the recursion's own reads, the temperature and the param.
    let shape = ir.instance_at("Shape").expect("placed").body();
    assert_eq!(shape.dependencies(Target::Check(0)), Deps { params: vec![0], env: vec![Env::Temperature], ..Deps::default() });
}

#[test]
fn recursion_reached_from_analog_is_refused() {
    // The same recursive function, called from the contribution: §3 rule 12 has the elaborator
    // unroll it, so the IR never carries it. Directly, and through a second function.
    let ir = Shape::ir(|b| {
        let count = Counting::function(b, "count", FunctionId(0));
        let probe = Shape::probe(b, 3);
        let call = Shape::dx(b, 3).call(count, vec![probe], ValueType::F64);
        Written { locals: vec![], stmts: vec![], value: call }
    });
    let broken = ir.check();
    let hits = Entries::with(&broken, "calls itself");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].item, "function count");
    assert!(hits[0].what.contains("`analog` reaches it"), "the entry says why: {}", hits[0].what);

    // `outer` is not recursive but calls the recursive function: only `count` is named.
    let ir = Shape::ir(|b| {
        let count = Counting::function(b, "count", FunctionId(0));
        let read = Shape::dx(b, 8).local(0, ValueType::F64);
        let inner = Shape::dx(b, 8).call(count, vec![read], ValueType::F64);
        let outer = Shape::function(b, "outer", inner);
        let probe = Shape::probe(b, 3);
        let call = Shape::dx(b, 3).call(outer, vec![probe], ValueType::F64);
        Written { locals: vec![], stmts: vec![], value: call }
    });
    let broken = ir.check();
    let items: Vec<&str> = Entries::with(&broken, "calls itself").iter().map(|entry| entry.item.as_str()).collect();
    assert_eq!(items, vec!["function count"], "{broken:?}");
}
