//! Scope: the digital node set of the IR (ir-rust phase B, RS-09): integer types and their
//! operators, unknown values and their reads, carriers, drives, nets, events, reactions and
//! commits, each an IR node that `Ir::check` types and the printer writes; the `check()` entries
//! of a defective digital body; the M1 prints unchanged; the buck-slice bodies pinned as golden
//! texts. Expected texts are written from the MSL source each fixture transcribes.

#[path = "support/digital.rs"]
#[allow(dead_code)]
mod digital;

use digital::{Buck, Dx, Ty};
use piperine_ir::{
    BinaryOp, BodyId, BodySpec, CarrierRole, Child, Constant, Deps, Drive, FunctionId, FunctionSpec, InstanceId,
    InstanceSpec, Ir, IrBuilder, Local, Method, NetId, NetSpec, NodeRef, NodeSpec, Origin, Real, Target, Terminal,
    UnaryOp, ValueType,
};

/// Designs for the tests: a set of functions or bodies under an empty top body.
struct Design;

impl Design {
    fn origin(file: &str, line: u32, col: u32) -> Origin {
        Origin { file: file.to_string(), line, col }
    }

    fn local(name: &str, ty: ValueType) -> Local {
        Local { name: name.to_string(), ty }
    }

    /// An empty body `Top`, placed as the design.
    fn finish(mut b: IrBuilder) -> Ir {
        let body = b.body(BodySpec {
            name: "Top".into(),
            origin: Design::origin("top.msl", 1, 1),
            params: vec![],
            nodes: vec![],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        Design::top(b, body)
    }

    fn top(mut b: IrBuilder, body: BodyId) -> Ir {
        let top = b.instance(InstanceSpec {
            path: "Top".into(),
            name: "Top".into(),
            body,
            parent: None,
            nodes: vec![],
            params: vec![],
            origin: Design::origin("top.msl", 1, 1),
        });
        b.finish(top)
    }

    /// `control.msl:17`: `fn target_codes(setpoint: u8) -> u10 { REF_BASE + ((setpoint as u10) << 2) }`,
    /// `REF_BASE` folded to its value 128 (`spec.msl:20`).
    fn target_codes(b: &mut IrBuilder) -> FunctionId {
        let mut x = Dx::new(b, "control.msl", 17, 44);
        let base = x.int(128, Ty::u(10));
        let setpoint = x.local(0, Ty::u(8));
        let wide = x.cast(setpoint, Ty::u(10), Ty::u(10));
        let two = x.int(2, Ty::i(32));
        let shifted = x.bin(BinaryOp::Shl, wide, two, Ty::u(10));
        let body = x.bin(BinaryOp::Add, base, shifted, Ty::u(10));
        b.function(FunctionSpec {
            name: "target_codes".into(),
            origin: Design::origin("control.msl", 17, 1),
            params: vec![Design::local("setpoint", Ty::u(8))],
            ret: Ty::u(10),
            locals: vec![Design::local("setpoint", Ty::u(8))],
            body,
        })
    }

    /// `control.msl:62-72`, the PID's output with its carrier reads as arguments, the constants
    /// folded (`spec.msl`: `KP_Q4` 64, `KD_Q4` 1024, `D_POLE_SHIFT` 2, `Q_SHIFT` 4, `DUTY_MAX`
    /// 2880):
    ///
    /// ```text
    /// fn pid_duty(e: i16, e_prev: i16, integ: i32, deriv: i32) -> u12 {
    ///     let p = KP_Q4 * (e as i32);
    ///     let d = (deriv >> D_POLE_SHIFT) + KD_Q4 * ((e - e_prev) as i32);
    ///     let u = (p + integ + d) >> Q_SHIFT;
    ///     let u_max = DUTY_MAX as i32;
    ///     u.clamp(0, u_max) as u12
    /// }
    /// ```
    fn pid_duty(b: &mut IrBuilder) -> FunctionId {
        let i32_ = Ty::i(32);
        let mut x = Dx::new(b, "control.msl", 62, 21);
        let kp = x.int(64, i32_.clone());
        let e = x.local(0, Ty::i(16));
        let e = x.cast(e, i32_.clone(), i32_.clone());
        let p = x.bin(BinaryOp::Mul, kp, e, i32_.clone());
        let let_p = x.let_(4, p);
        x.at(63, 21);
        let deriv = x.local(3, i32_.clone());
        let pole = x.int(2, i32_.clone());
        let filtered = x.bin(BinaryOp::Shr, deriv, pole, i32_.clone());
        let kd = x.int(1024, i32_.clone());
        let e = x.local(0, Ty::i(16));
        let e_prev = x.local(1, Ty::i(16));
        let de = x.bin(BinaryOp::Sub, e, e_prev, Ty::i(16));
        let de = x.cast(de, i32_.clone(), i32_.clone());
        let kick = x.bin(BinaryOp::Mul, kd, de, i32_.clone());
        let d = x.bin(BinaryOp::Add, filtered, kick, i32_.clone());
        let let_d = x.let_(5, d);
        x.at(64, 21);
        let p = x.local(4, i32_.clone());
        let integ = x.local(2, i32_.clone());
        let sum = x.bin(BinaryOp::Add, p, integ, i32_.clone());
        let d = x.local(5, i32_.clone());
        let sum = x.bin(BinaryOp::Add, sum, d, i32_.clone());
        let q = x.int(4, i32_.clone());
        let u = x.bin(BinaryOp::Shr, sum, q, i32_.clone());
        let let_u = x.let_(6, u);
        x.at(65, 25);
        let duty_max = x.int(2880, Ty::u(12));
        let u_max = x.cast(duty_max, i32_.clone(), i32_.clone());
        let let_u_max = x.let_(7, u_max);
        x.at(72, 21);
        let u = x.local(6, i32_.clone());
        let zero = x.int(0, i32_.clone());
        let u_max = x.local(7, i32_.clone());
        let clamped = x.method(Method::Clamp, u, vec![zero, u_max], i32_.clone());
        let duty = x.cast(clamped, Ty::u(12), Ty::u(12));
        let body = x.block(vec![let_p, let_d, let_u, let_u_max], Some(duty), Ty::u(12));
        b.function(FunctionSpec {
            name: "pid_duty".into(),
            origin: Design::origin("control.msl", 62, 13),
            params: vec![
                Design::local("e", Ty::i(16)),
                Design::local("e_prev", Ty::i(16)),
                Design::local("integ", i32_.clone()),
                Design::local("deriv", i32_.clone()),
            ],
            ret: Ty::u(12),
            locals: vec![
                Design::local("e", Ty::i(16)),
                Design::local("e_prev", Ty::i(16)),
                Design::local("integ", i32_.clone()),
                Design::local("deriv", i32_.clone()),
                Design::local("p", i32_.clone()),
                Design::local("d", i32_.clone()),
                Design::local("u", i32_.clone()),
                Design::local("u_max", i32_),
            ],
            body,
        })
    }

    /// `sense.msl:29`, the ADC's rounding with the probe as an argument:
    /// `fn adc_code(v: f64, vref: f64) -> u10 { (v / vref * 1024.0 + 0.5).floor().clamp(0.0, 1023.0) as u10 }`.
    fn adc_code(b: &mut IrBuilder) -> FunctionId {
        let mut x = Dx::new(b, "sense.msl", 29, 17);
        let v = x.local(0, ValueType::F64);
        let vref = x.local(1, ValueType::F64);
        let ratio = x.bin(BinaryOp::Div, v, vref, ValueType::F64);
        let full = x.real(1024.0);
        let scaled = x.bin(BinaryOp::Mul, ratio, full, ValueType::F64);
        let half = x.real(0.5);
        let rounded = x.bin(BinaryOp::Add, scaled, half, ValueType::F64);
        let floor = x.method(Method::Floor, rounded, vec![], ValueType::F64);
        let lo = x.real(0.0);
        let hi = x.real(1023.0);
        let clamped = x.method(Method::Clamp, floor, vec![lo, hi], ValueType::F64);
        let body = x.cast(clamped, Ty::u(10), Ty::u(10));
        b.function(FunctionSpec {
            name: "adc_code".into(),
            origin: Design::origin("sense.msl", 29, 9),
            params: vec![Design::local("v", ValueType::F64), Design::local("vref", ValueType::F64)],
            ret: Ty::u(10),
            locals: vec![Design::local("v", ValueType::F64), Design::local("vref", ValueType::F64)],
            body,
        })
    }

    /// The operators of RS-09 AC 1 the buck does not use, at the widths 1 and 64:
    /// `fn mix(a: i64, b: u64, c: u1) -> i64 { (-(a % 7) ^ !a).max(a / 3).min((b as i64) - (c as i64)) }`.
    fn mix(b: &mut IrBuilder) -> FunctionId {
        let i64_ = Ty::i(64);
        let mut x = Dx::new(b, "mix.msl", 1, 40);
        let a = x.local(0, i64_.clone());
        let seven = x.int(7, i64_.clone());
        let rem = x.bin(BinaryOp::Rem, a, seven, i64_.clone());
        let neg = x.un(UnaryOp::Neg, rem, i64_.clone());
        let a = x.local(0, i64_.clone());
        let not = x.un(UnaryOp::Not, a, i64_.clone());
        let xor = x.bin(BinaryOp::BitXor, neg, not, i64_.clone());
        let a = x.local(0, i64_.clone());
        let three = x.int(3, i64_.clone());
        let third = x.bin(BinaryOp::Div, a, three, i64_.clone());
        let max = x.method(Method::Max, xor, vec![third], i64_.clone());
        let bv = x.local(1, Ty::u(64));
        let bv = x.cast(bv, i64_.clone(), i64_.clone());
        let c = x.local(2, Ty::u(1));
        let c = x.cast(c, i64_.clone(), i64_.clone());
        let diff = x.bin(BinaryOp::Sub, bv, c, i64_.clone());
        let body = x.method(Method::Min, max, vec![diff], i64_.clone());
        let params = vec![Design::local("a", i64_.clone()), Design::local("b", Ty::u(64)), Design::local("c", Ty::u(1))];
        b.function(FunctionSpec {
            name: "mix".into(),
            origin: Design::origin("mix.msl", 1, 1),
            params: params.clone(),
            ret: i64_,
            locals: params,
            body,
        })
    }
}

/// The structural print of `ir` holds `lines`, consecutive and in order.
struct Printed;

impl Printed {
    fn holds(ir: &Ir, lines: &[&str]) {
        let text = ir.print_structural();
        let block = lines.join("\n");
        assert!(text.contains(&format!("{block}\n")), "missing:\n{block}\n--- in ---\n{text}");
    }
}

#[test]
fn integers_and_their_operators_print() {
    let mut b = IrBuilder::new();
    let rev3 = Buck::rev3(&mut b);
    Buck::dithered_width(&mut b, rev3);
    Design::target_codes(&mut b);
    Design::pid_duty(&mut b);
    Design::adc_code(&mut b);
    Design::mix(&mut b);
    let ir = Design::finish(b);
    // Every node is typed as the MSL source types it, so the IR's own rules accept it.
    assert_eq!(ir.check(), vec![]);

    // Literals print with their type as a suffix; `&`, `|`, `<<`, `>>`, `as` as written.
    Printed::holds(&ir, &["fn rev3(k: u3) -> u3", "  local k: u3", "  ((((k & 1u3) << 2i32) | (k & 2u3)) | ((k >> 2i32) & 1u3))"]);
    Printed::holds(
        &ir,
        &[
            "fn dithered_width(duty: u12, frame: u3) -> u9",
            "  local coarse: u9",
            "  local duty: u12",
            "  local fine: u3",
            "  local frame: u3",
            "  let coarse = ((duty >> 3i32) as u9)",
            "  let fine = ((duty & 7u12) as u3)",
            "  if (rev3(frame) < fine) { (coarse + 1u9) } else { coarse }",
        ],
    );
    Printed::holds(&ir, &["fn target_codes(setpoint: u8) -> u10", "  local setpoint: u8", "  (128u10 + ((setpoint as u10) << 2i32))"]);
    Printed::holds(
        &ir,
        &[
            "  let p = (64i32 * (e as i32))",
            "  let d = ((deriv >> 2i32) + (1024i32 * ((e - e_prev) as i32)))",
            "  let u = (((p + integ) + d) >> 4i32)",
            "  let u_max = (2880u12 as i32)",
            "  (u.clamp(0i32, u_max) as u12)",
        ],
    );
    // `floor` and `clamp` on `f64`, then the saturating cast to `u10`.
    Printed::holds(&ir, &["fn adc_code(v: f64, vref: f64) -> u10"]);
    Printed::holds(&ir, &["  ((((v / vref) * 1024.0) + 0.5).floor().clamp(0.0, 1023.0) as u10)"]);
    // `%`, unary `-`, `!` and `^` on `i64`, `min`, `max`, `u1` and `u64`.
    Printed::holds(&ir, &["fn mix(a: i64, b: u64, c: u1) -> i64"]);
    Printed::holds(&ir, &["  ((-(a % 7i64)) ^ (!a)).max((a / 3i64)).min(((b as i64) - (c as i64)))"]);

    // The full text names each node's form and type in its arena line.
    let full = ir.to_string();
    for form in [": u10 = int 128  @control.msl:17:44", ": u10 = cast e", ": i32 = clamp e", ": f64 = floor e", ": i64 = rem e", ": u3 = shl e", ": u3 = shr e", ": i64 = bitxor e", ": u3 = bitor e", ": u3 = bitand e"] {
        assert!(full.contains(form), "{form} missing from:\n{full}");
    }

    // The typing rules: a literal outside its type, a width outside 1..=64, an operator over two
    // integer types, and a node typed other than its form gives are each one entry.
    let mut b = IrBuilder::new();
    let mut x = Dx::new(&mut b, "bad.msl", 1, 1);
    let eight = x.int(8, Ty::u(3));
    let three = x.int(3, Ty::u(9));
    x.at(2, 1);
    let mixed = x.bin(BinaryOp::Add, eight, three, Ty::u(3));
    x.at(3, 1);
    let one = x.int(1, Ty::u(3));
    let wrong = x.bin(BinaryOp::BitAnd, mixed, one, Ty::u(9));
    let f = b.function(FunctionSpec {
        name: "bad".into(),
        origin: Design::origin("bad.msl", 1, 1),
        params: vec![Design::local("w", Ty::u(65))],
        ret: Ty::u(9),
        locals: vec![Design::local("w", Ty::u(65))],
        body: wrong,
    });
    let _ = f;
    let ir = Design::finish(b);
    let broken = ir.check();
    let at = |line: u32, what: &str| broken.iter().filter(|e| e.origin.line == line && e.what.contains(what)).count();
    assert_eq!(at(1, "integer literal 8 is outside the range of u3"), 1, "{broken:#?}");
    assert_eq!(at(1, "type u65 has a width outside 1..=64"), 1, "{broken:#?}");
    assert_eq!(at(2, "the operands of a binary operator do not fit its types"), 1, "{broken:#?}");
    assert_eq!(at(3, "expression is typed u9, its form gives u3"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 4, "{broken:#?}");
    // A node typed other than its form gives prints its type in the structural form.
    assert!(ir.print_structural().contains("((((8u3 + 3u9) : u3) & 1u3) : u9)"), "{}", ir.print_structural());
}

/// Functions over values that may be unknown, each beside the MSL it transcribes.
struct Unknowns;

impl Unknowns {
    fn function(b: &mut IrBuilder, name: &str, file: &str, line: u32, params: Vec<Local>, ret: ValueType, extra: Vec<Local>, body: piperine_ir::ExprId) -> FunctionId {
        let mut locals = params.clone();
        locals.extend(extra);
        b.function(FunctionSpec { name: name.into(), origin: Design::origin(file, line, 1), params, ret, locals, body })
    }

    /// `control.msl:12-14`, `SetpointReg`'s step written as a value, `data.known()` and
    /// `if let Some(d)` in msl-digital's lowering (`(x.is_known(), x.known_or(zero))`):
    /// `fn setpoint_next(data: Bit<u8>, code: Bit<u8>) -> u8 { if data.is_known() { let d = data.known_or(0); d.min(SETPOINT_MAX) } else { code } }`.
    fn setpoint_next(b: &mut IrBuilder) -> FunctionId {
        let u8_ = Ty::u(8);
        let mut x = Dx::new(b, "control.msl", 13, 9);
        let data = x.local(0, Ty::flagged(u8_.clone()));
        let cond = x.is_known(data);
        let data = x.local(0, Ty::flagged(u8_.clone()));
        let zero = x.int(0, u8_.clone());
        let d = x.known_or(data, zero, u8_.clone());
        let let_d = x.let_(2, d);
        let d = x.local(2, u8_.clone());
        let max = x.int(168, u8_.clone());
        let min = x.method(Method::Min, d, vec![max], u8_.clone());
        let then = x.block(vec![let_d], Some(min), u8_.clone());
        let code = x.local(1, Ty::flagged(u8_.clone()));
        let body = x.if_(cond, then, code, Ty::flagged(u8_.clone()));
        let params = vec![Design::local("data", Ty::flagged(u8_.clone())), Design::local("code", Ty::flagged(u8_.clone()))];
        Unknowns::function(b, "setpoint_next", "control.msl", 12, params, Ty::flagged(u8_.clone()), vec![Design::local("d", u8_)], body)
    }

    /// `power.msl:65`, the gate level of `PowerSwitch`: `fn gate_level(g: Bit) -> f64 { if g.high() { 1.0 } else { 0.0 } }`.
    fn gate_level(b: &mut IrBuilder) -> FunctionId {
        let mut x = Dx::new(b, "power.msl", 65, 27);
        let g = x.local(0, Ty::flagged(ValueType::Bool));
        let cond = x.high(g);
        let on = x.real(1.0);
        let off = x.real(0.0);
        let body = x.if_(cond, on, off, ValueType::F64);
        Unknowns::function(b, "gate_level", "power.msl", 65, vec![Design::local("g", Ty::flagged(ValueType::Bool))], ValueType::F64, vec![], body)
    }

    /// `power.msl:76`, the interlock of `GateDriver` over levels that may be unknown:
    /// `fn interlock(hs_cmd: Bit, ls_cmd: Bit) -> Bit { hs_cmd & !ls_cmd }`.
    fn interlock(b: &mut IrBuilder) -> FunctionId {
        let level = Ty::flagged(ValueType::Bool);
        let mut x = Dx::new(b, "power.msl", 76, 15);
        let hs = x.local(0, level.clone());
        let ls = x.local(1, level.clone());
        let not_ls = x.un(UnaryOp::Not, ls, level.clone());
        let body = x.bin(BinaryOp::BitAnd, hs, not_ls, level.clone());
        let params = vec![Design::local("hs_cmd", level.clone()), Design::local("ls_cmd", level.clone())];
        Unknowns::function(b, "interlock", "power.msl", 76, params, level, vec![], body)
    }

    /// §3.3.1 and §3.3.3: the literal `Unknown`, under a condition that may itself be unknown:
    /// `fn clear(run: Bit, x: Bit<u9>) -> Bit<u9> { if run { x } else { Unknown } }`.
    fn clear(b: &mut IrBuilder) -> FunctionId {
        let word = Ty::flagged(Ty::u(9));
        let mut x = Dx::new(b, "unknown.msl", 1, 45);
        let run = x.local(0, Ty::flagged(ValueType::Bool));
        let v = x.local(1, word.clone());
        let unknown = x.unknown(word.clone());
        let body = x.if_(run, v, unknown, word.clone());
        let params = vec![Design::local("run", Ty::flagged(ValueType::Bool)), Design::local("x", word.clone())];
        Unknowns::function(b, "clear", "unknown.msl", 1, params, word, vec![], body)
    }
}

#[test]
fn unknown_values_and_their_reads_print() {
    let mut b = IrBuilder::new();
    let rev3 = Buck::rev3_flagged(&mut b);
    Buck::dithered_width_flagged(&mut b, rev3);
    Unknowns::setpoint_next(&mut b);
    Unknowns::gate_level(&mut b);
    Unknowns::interlock(&mut b);
    Unknowns::clear(&mut b);
    let ir = Design::finish(b);
    assert_eq!(ir.check(), vec![]);

    // An unknown operand makes the result unknown (§3.3.2): the flagged frame index flags the
    // comparison and the width; the known `duty` keeps `coarse` and `fine` known.
    Printed::holds(&ir, &["fn \"rev3~1\"(k: flagged<u3>) -> flagged<u3>", "  local k: flagged<u3>", "  ((((k & 1u3) << 2i32) | (k & 2u3)) | ((k >> 2i32) & 1u3))"]);
    Printed::holds(
        &ir,
        &[
            "fn \"dithered_width~01\"(duty: u12, frame: flagged<u3>) -> flagged<u9>",
            "  local coarse: u9",
            "  local duty: u12",
            "  local fine: u3",
            "  local frame: flagged<u3>",
            "  let coarse = ((duty >> 3i32) as u9)",
            "  let fine = ((duty & 7u12) as u3)",
            "  if (\"rev3~1\"(frame) < fine) { (coarse + 1u9) } else { coarse }",
        ],
    );
    // `known_or` and `is_known` are known whatever their operand (§3.3.4); an arm that may be
    // unknown flags the `if`.
    Printed::holds(
        &ir,
        &[
            "fn setpoint_next(data: flagged<u8>, code: flagged<u8>) -> flagged<u8>",
            "  local code: flagged<u8>",
            "  local d: u8",
            "  local data: flagged<u8>",
            "  if data.is_known() { let d = data.known_or(0u8); d.min(168u8) } else { code }",
        ],
    );
    Printed::holds(&ir, &["fn gate_level(g: flagged<bool>) -> f64", "  local g: flagged<bool>", "  if g.high() { 1.0 } else { 0.0 }"]);
    Printed::holds(&ir, &["fn interlock(hs_cmd: flagged<bool>, ls_cmd: flagged<bool>) -> flagged<bool>"]);
    Printed::holds(&ir, &["  (hs_cmd & (!ls_cmd))"]);
    // `Unknown` has no type of its own: the structural form writes the one it is given.
    Printed::holds(&ir, &["fn clear(run: flagged<bool>, x: flagged<u9>) -> flagged<u9>"]);
    Printed::holds(&ir, &["  if run { x } else { (Unknown : flagged<u9>) }"]);

    let full = ir.to_string();
    for form in [": flagged<u9> = unknown  @unknown.msl:1:45", ": u8 = known_or e", ": bool = is_known e", ": bool = high e", ": flagged<bool> = bitand e"] {
        assert!(full.contains(form), "{form} missing from:\n{full}");
    }

    // Each misuse is one entry at its origin.
    let mut b = IrBuilder::new();
    let level = Ty::flagged(ValueType::Bool);
    let mut x = Dx::new(&mut b, "bad.msl", 1, 1);
    // `&&` decides control: an operand that may be unknown is lowered to `&` (§3.3.12).
    let a = x.local(0, level.clone());
    let c = x.local(0, level.clone());
    let and = x.bin(BinaryOp::And, a, c, ValueType::Bool);
    let s1 = x.stmt(and);
    // `known_or`'s default must be a known value of the base type.
    x.at(2, 1);
    let a = x.local(0, level.clone());
    let d = x.local(0, level.clone());
    let ko = x.known_or(a, d, ValueType::Bool);
    let s2 = x.stmt(ko);
    // `Unknown` must have a flagged type.
    x.at(3, 1);
    let u = x.unknown(Ty::u(9));
    let s3 = x.stmt(u);
    // `high()` reads a `bool`.
    x.at(4, 1);
    let w = x.int(3, Ty::u(8));
    let h = x.high(w);
    let s4 = x.stmt(h);
    // An `if` on a condition that may be unknown is itself unknown.
    x.at(5, 1);
    let a = x.local(0, level.clone());
    let one = x.int(1, Ty::u(9));
    let two = x.int(2, Ty::u(9));
    let dropped = x.if_(a, one, two, Ty::u(9));
    let s5 = x.stmt(dropped);
    x.at(6, 1);
    let unit = x.unit();
    let body = x.block(vec![s1, s2, s3, s4, s5], Some(unit), ValueType::Unit);
    b.function(FunctionSpec {
        name: "bad".into(),
        origin: Design::origin("bad.msl", 9, 1),
        params: vec![Design::local("a", level.clone())],
        ret: ValueType::Unit,
        locals: vec![Design::local("a", level), Design::local("nothing", Ty::flagged(ValueType::Unit))],
        body,
    });
    let ir = Design::finish(b);
    let broken = ir.check();
    let at = |line: u32, what: &str| broken.iter().filter(|e| e.origin.line == line && e.what.contains(what)).count();
    assert_eq!(at(1, "the operands of a binary operator do not fit its types"), 1, "{broken:#?}");
    assert_eq!(at(2, "the operands of `known_or` do not fit its types"), 1, "{broken:#?}");
    assert_eq!(at(3, "the literal `Unknown` must have a flagged type, found u9"), 1, "{broken:#?}");
    assert_eq!(at(4, "the operands of `high` do not fit its types"), 1, "{broken:#?}");
    assert_eq!(at(5, "expression is typed u9, its form gives flagged<u9>"), 1, "{broken:#?}");
    assert_eq!(at(9, "type flagged<()>: only `bool`, `f64` and an integer type may be unknown"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 6, "{broken:#?}");
}

/// The power stage of `power.msl:125-143`, cut to its gate path: the driver and the two switches
/// (`Switch` standing for `PowerSwitch`), the commands as ports, the gates as signals.
///
/// ```text
/// pub module Stage(in hs_cmd: Bit, in ls_cmd: Bit) {
///     signal vin: Electrical; signal sw: Electrical;
///     signal hs_gate: Bit; signal ls_gate: Bit;
///     let drv = GateDriver(hs_cmd, ls_cmd, hs_gate, ls_gate) { tpd: 10n };
///     let hs = Switch(vin, sw, hs_gate);
///     let ls = Switch(sw, gnd, ls_gate);
/// }
/// ```
struct Stage;

impl Stage {
    const F: &'static str = "stage.msl";

    fn instance(b: &mut IrBuilder, path: &str, body: BodyId, parent: Option<InstanceId>, nodes: Vec<NodeRef>, params: Vec<f64>, nets: Vec<NetId>, line: u32) -> InstanceId {
        let name = path.rsplit('.').next().unwrap_or(path).to_string();
        let params = params.into_iter().map(|v| Constant::Real(Real::new(v))).collect();
        let id = b.instance(InstanceSpec { path: path.into(), name, body, parent, nodes, params, origin: Design::origin(Stage::F, line, 5) });
        b.bind_nets(id, nets);
        id
    }

    fn design() -> Ir {
        let mut b = IrBuilder::new();
        let driver = Buck::gate_driver(&mut b);
        let switch = Buck::switch(&mut b);
        let el = Buck::electrical(&mut b);
        let child = |name: &str, body: BodyId, nodes: Vec<Terminal>, params: Vec<(u32, f64)>, line: u32| Child {
            name: name.into(),
            body,
            nodes,
            params: params.into_iter().map(|(i, v)| (i, Constant::Real(Real::new(v)))).collect(),
            origin: Design::origin(Stage::F, line, 5),
        };
        let signal = |name: &str, line: u32| piperine_ir::BodyNode { name: name.into(), nature: el, role: piperine_ir::NodeRole::Signal, init: None, origin: Design::origin(Stage::F, line, 5) };
        let stage = b.body(BodySpec {
            nodes: vec![signal("vin", 2), signal("sw", 2)],
            children: vec![
                child("drv", driver, vec![], vec![(0, 10e-9)], 4),
                child("hs", switch, vec![Terminal::Node(0), Terminal::Node(1)], vec![], 5),
                child("ls", switch, vec![Terminal::Node(1), Terminal::Ground], vec![], 6),
            ],
            ..Buck::spec("Stage", Stage::F, 1)
        });
        for (name, role, line, col) in [("hs_cmd", CarrierRole::In, 1, 17), ("ls_cmd", CarrierRole::In, 1, 33), ("hs_gate", CarrierRole::Signal, 3, 12), ("ls_gate", CarrierRole::Signal, 3, 33)] {
            b.carrier(stage, Buck::carrier(name, ValueType::Bool, role, None, Stage::F, line, col));
        }
        let top = b.next_instance_id();
        let vin = b.node(NodeSpec { path: "Stage.vin".into(), nature: el, declared_by: top, init: None, origin: Design::origin(Stage::F, 2, 12) });
        let sw = b.node(NodeSpec { path: "Stage.sw".into(), nature: el, declared_by: top, init: None, origin: Design::origin(Stage::F, 2, 33) });
        let nets: Vec<NetId> = ["hs_cmd", "ls_cmd", "hs_gate", "ls_gate"]
            .into_iter()
            .map(|n| b.net(NetSpec { path: format!("Stage.{n}"), ty: ValueType::Bool, declared_by: top, init: None, origin: Design::origin(Stage::F, 3, 5) }))
            .collect();
        let (vin, sw) = (NodeRef::Node(vin), NodeRef::Node(sw));
        let top = Stage::instance(&mut b, "Stage", stage, None, vec![vin, sw], vec![], nets.clone(), 1);
        Stage::instance(&mut b, "Stage.drv", driver, Some(top), vec![], vec![10e-9], nets.clone(), 4);
        Stage::instance(&mut b, "Stage.hs", switch, Some(top), vec![vin, sw], vec![20e-3, 1e6], vec![nets[2]], 5);
        Stage::instance(&mut b, "Stage.ls", switch, Some(top), vec![sw, NodeRef::Ground], vec![20e-3, 1e6], vec![nets[3]], 6);
        b.finish(top)
    }
}

#[test]
fn carriers_and_drives_print() {
    let ir = Stage::design();
    assert_eq!(ir.check(), vec![]);

    // Carriers after the nodes, with their role; a drive names its carrier and, delayed, its
    // `after` (power.msl:74-78).
    Printed::holds(
        &ir,
        &[
            "body GateDriver",
            "  param tpd: f64 = 1e-8 where (tpd > 0.0)",
            "  carrier hs_cmd: bool in",
            "  carrier ls_cmd: bool in",
            "  carrier hs_gate: bool out",
            "  carrier ls_gate: bool out",
            "  drive hs_gate = (hs_cmd & (!ls_cmd)) after tpd",
            "  drive ls_gate = (ls_cmd & (!hs_cmd)) after tpd",
        ],
    );
    // An `analog` block reads the gate through `high()` (§3.3.5).
    Printed::holds(
        &ir,
        &[
            "body Switch",
            "  param ron: f64 = 0.02 where (ron > 0.0)",
            "  param roff: f64 = 1000000.0 where (roff > ron)",
            "  node p: Electrical port",
            "  node n: Electrical port",
            "  carrier g: bool in",
            "  branch b0 (p, n) flow",
            "  analog",
            "    b0.flow +> direct (probe(p, n) * if g.high() { (1.0 / ron) } else { (1.0 / roff) })",
        ],
    );
    Printed::holds(&ir, &["  carrier hs_cmd: bool in", "  carrier ls_cmd: bool in", "  carrier hs_gate: bool signal", "  carrier ls_gate: bool signal"]);
    // Every carrier of an instance is bound to a net, as every node is bound to a node.
    Printed::holds(
        &ir,
        &[
            "instance Stage: Stage  nodes (Stage.vin, Stage.sw)  params ()  nets (Stage.hs_cmd, Stage.ls_cmd, Stage.hs_gate, Stage.ls_gate)  top",
            "instance Stage.drv: GateDriver  nodes ()  params (tpd = 1e-8)  nets (Stage.hs_cmd, Stage.ls_cmd, Stage.hs_gate, Stage.ls_gate)",
            "instance Stage.hs: Switch  nodes (Stage.vin, Stage.sw)  params (ron = 0.02, roff = 1000000.0)  nets (Stage.hs_gate)",
            "instance Stage.ls: Switch  nodes (Stage.sw, gnd)  params (ron = 0.02, roff = 1000000.0)  nets (Stage.ls_gate)",
        ],
    );
    Printed::holds(
        &ir,
        &[
            "net Stage.hs_cmd: bool  declared by Stage",
            "net Stage.hs_gate: bool  declared by Stage",
            "net Stage.ls_cmd: bool  declared by Stage",
            "net Stage.ls_gate: bool  declared by Stage",
        ],
    );
    let full = ir.to_string();
    for line in ["  body 0 discrete: drives ((c2 e", "  nets (0, 1, 2, 3)", "  net 3: declared by 0", "carrier hs_gate: bool out  @power.msl:74:63", "drive hs_gate = (hs_cmd & (!ls_cmd)) after tpd  @power.msl:76:5"] {
        assert!(full.contains(line), "{line} missing from:\n{full}");
    }

    // What a drive reads: both commands for the value, the param for the delay.
    let driver = ir.instance_at("Stage.drv").expect("the driver").body();
    assert_eq!(driver.carrier_reads(Target::Drive(0)), vec![0, 1]);
    assert_eq!(driver.carrier_reads(Target::Drive(1)), vec![0, 1]);
    assert_eq!(driver.dependencies(Target::Drive(0)), Deps::default());
    assert_eq!(driver.dependencies(Target::DriveAfter(1)), Deps { params: vec![0], ..Deps::default() });
    assert_eq!(driver.carrier_reads(Target::DriveAfter(1)), Vec::<u32>::new());
    // What the analog block reads: the probe, both params and the gate; the gate holds between
    // events, so the term stays affine in the potentials.
    let switch = ir.instance_at("Stage.hs").expect("a switch").body();
    assert_eq!(switch.carrier_reads(Target::Flow(0)), vec![0]);
    assert_eq!(switch.dependencies(Target::Flow(0)), Deps { nodes: vec![0, 1], params: vec![0, 1], ..Deps::default() });
    assert!(switch.is_affine(0));
    // A net lists the instances whose ports it reaches; the stage's own gates are its signals.
    let wired = |path: &str| -> Vec<String> { ir.net_at(path).expect("a net").instances().iter().map(|i| i.path().to_string()).collect() };
    assert_eq!(wired("Stage.hs_gate"), vec!["Stage.drv", "Stage.hs"]);
    assert_eq!(wired("Stage.hs_cmd"), vec!["Stage", "Stage.drv"]);
    assert_eq!(ir.instance_at("Stage.ls").expect("a switch").net(0).map(|n| n.path()), Some("Stage.ls_gate"));

    // A drive of the wrong type, a binding short of a net and a net of another type each give
    // one entry.
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Bad", "bad.msl", 1));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, "bad.msl", 1, 20));
    b.carrier(body, Buck::carrier("w", Ty::u(4), CarrierRole::Out, None, "bad.msl", 1, 30));
    let value = Dx::new(&mut b, "bad.msl", 2, 9).int(3, Ty::u(8));
    b.drive(body, Drive { carrier: 0, value, after: None, origin: Design::origin("bad.msl", 2, 5) });
    let top = b.next_instance_id();
    let net = b.net(NetSpec { path: "Bad.q".into(), ty: Ty::u(8), declared_by: top, init: None, origin: Design::origin("bad.msl", 1, 20) });
    let top = Stage::instance(&mut b, "Bad", body, None, vec![], vec![], vec![net], 1);
    let ir = b.finish(top);
    let broken = ir.check();
    let has = |what: &str| broken.iter().filter(|e| e.what.contains(what)).count();
    assert_eq!(has("the value of a drive must be bool, found u8"), 1, "{broken:#?}");
    assert_eq!(has("binds 1 nets, body Bad has 2 carriers"), 1, "{broken:#?}");
    assert_eq!(has("binds carrier q: bool to net Bad.q: u8"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 3, "{broken:#?}");
}
