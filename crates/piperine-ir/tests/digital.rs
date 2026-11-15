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
    BinaryOp, BodyId, BodySpec, FunctionId, FunctionSpec, InstanceSpec, Ir, IrBuilder, Local, Method,
    Origin, UnaryOp, ValueType,
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
