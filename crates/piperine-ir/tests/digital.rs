//! Scope: the digital node set of the IR (ir-rust phase B, RS-09): integer types and their
//! operators, carriers, drives, nets, events, reactions and
//! commits, each an IR node that `Ir::check` types and the printer writes; the `check()` entries
//! of a defective digital body; the M1 prints unchanged; the buck-slice bodies pinned as golden
//! texts. Expected texts are written from the MSL source each fixture transcribes.

use piperine_ir_fixtures::digital;

use piperine_ir_fixtures::devices;

use digital::{Buck, Dx, ValueAliases};
use devices::Devices;
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodySpec, Branch, BranchKind, CarrierRole, Child, Constant, Deps, Drive, FunctionId, FunctionSpec, InstanceId,
    InstanceSpec, Ir, IrBuilder, Local, Method, NetId, NetSpec, NodeRef, NodeSpec, Operator, Origin, QuantityRole, Real,
    Stmt, Target, Terminal, UnaryOp, ValueType,
};

/// Designs for the tests: a set of functions or bodies under an empty top body.
struct Design;

impl Design {
    fn origin(file: &str, line: u32, col: u32) -> Origin {
        Origin { file: file.to_string(), line, col }
    }

    fn local(name: &str, ty: ValueType) -> Local {
        Local { name: name.to_string(), ty, mutable: false }
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
        let base = x.int(128, ValueAliases::u(10));
        let setpoint = x.local(0, ValueAliases::u(8));
        let wide = x.cast(setpoint, ValueAliases::u(10), ValueAliases::u(10));
        let two = x.int(2, ValueAliases::i(32));
        let shifted = x.bin(BinaryOp::Shl, wide, two, ValueAliases::u(10));
        let body = x.bin(BinaryOp::Add, base, shifted, ValueAliases::u(10));
        b.function(FunctionSpec {
            name: "target_codes".into(),
            origin: Design::origin("control.msl", 17, 1),
            params: vec![Design::local("setpoint", ValueAliases::u(8))],
            ret: ValueAliases::u(10),
            locals: vec![Design::local("setpoint", ValueAliases::u(8))],
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
        let i32_ = ValueAliases::i(32);
        let mut x = Dx::new(b, "control.msl", 62, 21);
        let kp = x.int(64, i32_.clone());
        let e = x.local(0, ValueAliases::i(16));
        let e = x.cast(e, i32_.clone(), i32_.clone());
        let p = x.bin(BinaryOp::Mul, kp, e, i32_.clone());
        let let_p = x.let_(4, p);
        x.at(63, 21);
        let deriv = x.local(3, i32_.clone());
        let pole = x.int(2, i32_.clone());
        let filtered = x.bin(BinaryOp::Shr, deriv, pole, i32_.clone());
        let kd = x.int(1024, i32_.clone());
        let e = x.local(0, ValueAliases::i(16));
        let e_prev = x.local(1, ValueAliases::i(16));
        let de = x.bin(BinaryOp::Sub, e, e_prev, ValueAliases::i(16));
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
        let duty_max = x.int(2880, ValueAliases::u(12));
        let u_max = x.cast(duty_max, i32_.clone(), i32_.clone());
        let let_u_max = x.let_(7, u_max);
        x.at(72, 21);
        let u = x.local(6, i32_.clone());
        let zero = x.int(0, i32_.clone());
        let u_max = x.local(7, i32_.clone());
        let clamped = x.method(Method::Clamp, u, vec![zero, u_max], i32_.clone());
        let duty = x.cast(clamped, ValueAliases::u(12), ValueAliases::u(12));
        let body = x.block(vec![let_p, let_d, let_u, let_u_max], Some(duty), ValueAliases::u(12));
        b.function(FunctionSpec {
            name: "pid_duty".into(),
            origin: Design::origin("control.msl", 62, 13),
            params: vec![
                Design::local("e", ValueAliases::i(16)),
                Design::local("e_prev", ValueAliases::i(16)),
                Design::local("integ", i32_.clone()),
                Design::local("deriv", i32_.clone()),
            ],
            ret: ValueAliases::u(12),
            locals: vec![
                Design::local("e", ValueAliases::i(16)),
                Design::local("e_prev", ValueAliases::i(16)),
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
        let body = x.cast(clamped, ValueAliases::u(10), ValueAliases::u(10));
        b.function(FunctionSpec {
            name: "adc_code".into(),
            origin: Design::origin("sense.msl", 29, 9),
            params: vec![Design::local("v", ValueType::F64), Design::local("vref", ValueType::F64)],
            ret: ValueAliases::u(10),
            locals: vec![Design::local("v", ValueType::F64), Design::local("vref", ValueType::F64)],
            body,
        })
    }

    /// The operators of RS-09 AC 1 the buck does not use, at the widths 1 and 64:
    /// `fn mix(a: i64, b: u64, c: u1) -> i64 { (-(a % 7) ^ !a).max(a / 3).min((b as i64) - (c as i64)) }`.
    fn mix(b: &mut IrBuilder) -> FunctionId {
        let i64_ = ValueAliases::i(64);
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
        let bv = x.local(1, ValueAliases::u(64));
        let bv = x.cast(bv, i64_.clone(), i64_.clone());
        let c = x.local(2, ValueAliases::u(1));
        let c = x.cast(c, i64_.clone(), i64_.clone());
        let diff = x.bin(BinaryOp::Sub, bv, c, i64_.clone());
        let body = x.method(Method::Min, max, vec![diff], i64_.clone());
        let params = vec![Design::local("a", i64_.clone()), Design::local("b", ValueAliases::u(64)), Design::local("c", ValueAliases::u(1))];
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

    // The typing rules: a literal outside its type, a width of 0, an operator over two
    // integer types, and a node typed other than its form gives are each one entry.
    let mut b = IrBuilder::new();
    let mut x = Dx::new(&mut b, "bad.msl", 1, 1);
    let eight = x.int(8, ValueAliases::u(3));
    let three = x.int(3, ValueAliases::u(9));
    x.at(2, 1);
    let mixed = x.bin(BinaryOp::Add, eight, three, ValueAliases::u(3));
    x.at(3, 1);
    let one = x.int(1, ValueAliases::u(3));
    let wrong = x.bin(BinaryOp::BitAnd, mixed, one, ValueAliases::u(9));
    let f = b.function(FunctionSpec {
        name: "bad".into(),
        origin: Design::origin("bad.msl", 1, 1),
        params: vec![Design::local("w", ValueAliases::u(0))],
        ret: ValueAliases::u(9),
        locals: vec![Design::local("w", ValueAliases::u(0))],
        body: wrong,
    });
    let _ = f;
    let ir = Design::finish(b);
    let broken = ir.check();
    let at = |line: u32, what: &str| broken.iter().filter(|e| e.origin.line == line && e.what.contains(what)).count();
    assert_eq!(at(1, "integer literal 8 is outside the range of u3"), 1, "{broken:#?}");
    assert_eq!(at(1, "type u0 has a width outside 1..=4294967295"), 1, "{broken:#?}");
    assert_eq!(at(2, "the operands of a binary operator do not fit its types"), 1, "{broken:#?}");
    assert_eq!(at(3, "expression is typed u9, its form gives u3"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 4, "{broken:#?}");
    // A node typed other than its form gives prints its type in the structural form.
    assert!(ir.print_structural().contains("((((8u3 + 3u9) : u3) & 1u3) : u9)"), "{}", ir.print_structural());
}

/// Functions over integers and bits, each beside the MSL it transcribes.
struct Functions;

impl Functions {
    fn function(b: &mut IrBuilder, name: &str, file: &str, line: u32, params: Vec<Local>, ret: ValueType, extra: Vec<Local>, body: piperine_ir::ExprId) -> FunctionId {
        let mut locals = params.clone();
        locals.extend(extra);
        b.function(FunctionSpec { name: name.into(), origin: Design::origin(file, line, 1), params, ret, locals, body })
    }

    /// `power.msl:65`, the gate level of `PowerSwitch`: `fn gate_level(g: Bit) -> f64 { if g { 1.0 } else { 0.0 } }`.
    fn gate_level(b: &mut IrBuilder) -> FunctionId {
        let mut x = Dx::new(b, "power.msl", 65, 27);
        let cond = x.local(0, ValueType::Bool);
        let on = x.real(1.0);
        let off = x.real(0.0);
        let body = x.if_(cond, on, off, ValueType::F64);
        Functions::function(b, "gate_level", "power.msl", 65, vec![Design::local("g", ValueType::Bool)], ValueType::F64, vec![], body)
    }

    /// `power.msl:76`, the interlock of `GateDriver`:
    /// `fn interlock(hs_cmd: Bit, ls_cmd: Bit) -> Bit { hs_cmd & !ls_cmd }`.
    fn interlock(b: &mut IrBuilder) -> FunctionId {
        let level = ValueType::Bool;
        let mut x = Dx::new(b, "power.msl", 76, 15);
        let hs = x.local(0, level.clone());
        let ls = x.local(1, level.clone());
        let not_ls = x.un(UnaryOp::Not, ls, level.clone());
        let body = x.bin(BinaryOp::BitAnd, hs, not_ls, level.clone());
        let params = vec![Design::local("hs_cmd", level.clone()), Design::local("ls_cmd", level.clone())];
        Functions::function(b, "interlock", "power.msl", 76, params, level, vec![], body)
    }
}

#[test]
fn integer_and_bit_functions_print() {
    let mut b = IrBuilder::new();
    let rev3 = Buck::rev3(&mut b);
    Buck::dithered_width(&mut b, rev3);
    Functions::gate_level(&mut b);
    Functions::interlock(&mut b);
    let ir = Design::finish(b);
    assert_eq!(ir.check(), vec![]);

    // Every operand has its plain type: the frame index keeps `rev3` and the width in `u3` and `u9`.
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
    Printed::holds(&ir, &["fn gate_level(g: bool) -> f64", "  local g: bool", "  if g { 1.0 } else { 0.0 }"]);
    Printed::holds(&ir, &["fn interlock(hs_cmd: bool, ls_cmd: bool) -> bool", "  local hs_cmd: bool", "  local ls_cmd: bool", "  (hs_cmd & (!ls_cmd))"]);
    let full = ir.to_string();
    for form in [": bool = bitand e", ": f64 = if e"] {
        assert!(full.contains(form), "{form} missing from:\n{full}");
    }
}

#[test]
fn no_value_type_is_flagged() {
    // EU-01: a carrier read, a `prev()` and everything computed from them have the type the carrier
    // holds, so no printed form of the IR names a flag: the buck slice (every digital and mixed
    // form the IR has) prints none of `flagged`, `known_or`, `is_known`, `high` or `Unknown`.
    let mut b = IrBuilder::new();
    let rev3 = Buck::rev3(&mut b);
    Buck::dithered_width(&mut b, rev3);
    let slice: [fn(&mut IrBuilder) -> BodyId; 10] = [
        Buck::osc,
        Buck::ref_ramp,
        Buck::adc,
        Buck::setpoint_reg,
        Buck::dpwm,
        Buck::pid,
        Buck::gate_driver,
        Buck::comparator,
        Buck::switch,
        Buck::dff,
    ];
    for make in slice {
        make(&mut b);
    }
    let ir = Design::finish(b);
    let (structural, full) = (ir.print_structural(), ir.to_string());
    for form in ["flagged", "known_or", "is_known", ".high()", "Unknown", "merge(", "level_is", "level_value"] {
        assert!(!structural.contains(form) && !full.contains(form), "`{form}` is printed:\n{structural}");
    }
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
            "    b0.flow +> direct (probe(p, n) * if g { (1.0 / ron) } else { (1.0 / roff) })",
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
    b.carrier(body, Buck::carrier("w", ValueAliases::u(4), CarrierRole::Out, None, "bad.msl", 1, 30));
    let value = Dx::new(&mut b, "bad.msl", 2, 9).int(3, ValueAliases::u(8));
    b.drive(body, Drive { carrier: 0, value, after: None, origin: Design::origin("bad.msl", 2, 5), element: None, path: vec![] });
    let top = b.next_instance_id();
    let net = b.net(NetSpec { path: "Bad.q".into(), ty: ValueAliases::u(8), declared_by: top, init: None, origin: Design::origin("bad.msl", 1, 20) });
    let top = Stage::instance(&mut b, "Bad", body, None, vec![], vec![], vec![net], 1);
    let ir = b.finish(top);
    let broken = ir.check();
    let has = |what: &str| broken.iter().filter(|e| e.what.contains(what)).count();
    assert_eq!(has("the value of a drive must be bool, found u8"), 1, "{broken:#?}");
    assert_eq!(has("binds 1 nets, body Bad has 2 carriers"), 1, "{broken:#?}");
    assert_eq!(has("binds carrier q: bool to net Bad.q: u8"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 3, "{broken:#?}");
}

/// §13 rule 17: `on settle { q <- true; }` and `on settle && ready { .. }` build, pass
/// `Ir::check` (a guard is a `bool`, `settle` reads no expression) and print as `settle`.
#[test]
fn a_settle_event_is_built_checked_and_printed() {
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Sampler", "settle.msl", 1));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, "settle.msl", 1, 20));
    b.carrier(body, Buck::carrier("ready", ValueType::Bool, CarrierRole::In, None, "settle.msl", 1, 30));
    let bare = b.event(body, piperine_ir::Event { source: piperine_ir::EventSource::Settle, guard: None, origin: Design::origin("settle.msl", 2, 8) });
    let mut x = Dx::new(&mut b, "settle.msl", 3, 9);
    let ready = x.carrier(1, ValueType::Bool);
    let guard = ready;
    let guarded = b.event(body, piperine_ir::Event { source: piperine_ir::EventSource::Settle, guard: Some(guard), origin: Design::origin("settle.msl", 4, 8) });
    let mut x = Dx::new(&mut b, "settle.msl", 5, 9);
    let yes = x.boolean(true);
    let commit = x.commit(0, yes, None);
    Buck::reaction(&mut b, body, vec![bare, guarded], vec![commit], vec![], "settle.msl", 5);
    let top = Slice::design(&mut b, &[("sampler", body, 0, vec![ValueType::Bool, ValueType::Bool], vec![])]);
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "§13 rule 17: settle is an event with no expression of its own");
    Printed::holds(&ir, &["body Sampler", "  carrier q: bool out", "  carrier ready: bool in", "  event E0 settle", "  event E1 settle guard ready", "  reaction R0 on (E0, E1)"]);
    assert!(ir.to_string().contains("(settle -)"), "the structural form names a bare settle:\n{ir}");
}

/// One design that places bodies side by side under an empty top, every node port on a node and
/// every carrier on a net of its own, every param at its default.
struct Slice;

impl Slice {
    fn design(b: &mut IrBuilder, placed: &[(&str, BodyId, usize, Vec<ValueType>, Vec<f64>)]) -> InstanceId {
        let top_body = b.body(Buck::spec("Slice", "slice.msl", 1));
        let top = b.instance(InstanceSpec { path: "Slice".into(), name: "Slice".into(), body: top_body, parent: None, nodes: vec![], params: vec![], origin: Design::origin("slice.msl", 1, 1) });
        let el = Buck::electrical(b);
        for (line, (name, body, nodes, carriers, params)) in placed.iter().enumerate() {
            let line = line as u32 + 2;
            let path = format!("Slice.{name}");
            let at = b.next_instance_id();
            let nodes = (0..*nodes)
                .map(|n| NodeRef::Node(b.node(NodeSpec { path: format!("{path}.n{n}"), nature: el, declared_by: at, init: None, origin: Design::origin("slice.msl", line, 5) })))
                .collect();
            let nets = carriers
                .iter()
                .enumerate()
                .map(|(c, ty)| b.net(NetSpec { path: format!("{path}.c{c}"), ty: ty.clone(), declared_by: at, init: None, origin: Design::origin("slice.msl", line, 5) }))
                .collect();
            let params = params.iter().map(|v| Constant::Real(Real::new(*v))).collect();
            let id = b.instance(InstanceSpec { path, name: name.to_string(), body: *body, parent: Some(top), nodes, params, origin: Design::origin("slice.msl", line, 5) });
            b.bind_nets(id, nets);
        }
        top
    }
}

#[test]
fn events_and_reactions_print() {
    let mut b = IrBuilder::new();
    let osc = Buck::osc(&mut b);
    let ramp = Buck::ref_ramp(&mut b);
    let adc = Buck::adc(&mut b);
    let a2d = Buck::comparator(&mut b);
    let bit = || ValueType::Bool;
    let top = Slice::design(
        &mut b,
        &[
            ("osc", osc, 0, vec![bit()], vec![200e6]),
            ("ramp", ramp, 0, vec![bit(), bit(), ValueAliases::u(10), ValueAliases::u(10), bit()], vec![]),
            ("adc", adc, 1, vec![bit(), ValueAliases::u(10), bit()], vec![1.6, 500e-9, 5e-9]),
            ("a2d", a2d, 1, vec![bit()], vec![0.6, 50e-3]),
        ],
    );
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![]);

    // `on clk.changed()`: an event on the carrier, no guard; the commit with its delay
    // (dpwm.msl:10).
    Printed::holds(
        &ir,
        &[
            "body Osc",
            "  param f: f64 = 200000000.0 where (f > 0.0)",
            "  carrier clk: bool out init false",
            "  event E0 changed clk",
            "  reaction R0 on (E0)",
            "    clk <- (!clk) after (0.5 / f)",
        ],
    );
    // `tick.rise()` is `changed` guarded by the carrier's own read; `if`, `else if` and the commits of
    // control.msl:28-30, an `if` with no `else` closing on `()`.
    Printed::holds(
        &ir,
        &[
            "body RefRamp",
            "  carrier tick: bool in",
            "  carrier run: bool in",
            "  carrier target: u10 in",
            "  carrier reference: u10 out init 0",
            "  carrier at_target: bool out",
            "  drive at_target = (reference == target)",
            "  event E0 changed tick guard tick",
            "  reaction R0 on (E0)",
            "    if (!run) { reference <- 0u10 } else { if (reference < target) { reference <- (reference + 1u10) } else { if (reference > target) { reference <- (reference - 1u10) } else { () } } }",
        ],
    );
    // A reaction's own `let`, a probe in a reaction, two commits to one carrier (sense.msl:28-33).
    Printed::holds(
        &ir,
        &[
            "  event E0 changed sample guard sample",
            "  reaction R0 on (E0)",
            "    local q: u10",
            "    let q = ((((probe(ain, gnd) / vref) * 1024.0) + 0.5).floor().clamp(0.0, 1023.0) as u10)",
            "    code <- q after tconv",
            "    done <- true after tconv",
            "    done <- false after (tconv + tstrobe)",
        ],
    );
    // `above(e)` is `cross(e)` (convert.msl:11-12).
    Printed::holds(
        &ir,
        &[
            "body A2D",
            "  param vth: f64 = 0.6",
            "  param hyst: f64 = 0.05",
            "  node a: Electrical port",
            "  carrier d: bool out init false",
            "  event E0 cross ((probe(a, gnd) - vth) - (hyst / 2.0))",
            "  event E1 cross ((vth - (hyst / 2.0)) - probe(a, gnd))",
            "  reaction R0 on (E0)",
            "    d <- true",
            "  reaction R1 on (E1)",
            "    d <- false",
        ],
    );
    let full = ir.to_string();
    for line in ["discrete: drives ()  events ((changed c0 -))  reactions ((on (0) (s", "events ((cross e", ": commit c0 e", "event E0 changed clk  @dpwm.msl:10:22", "reaction R0 on (E0)  @dpwm.msl:10:5"] {
        assert!(full.contains(line), "{line} missing from:\n{full}");
    }

    // What each reads (design: the dependency set of a reaction is the carriers, params and
    // nodes its body reads).
    let body = |path: &str| ir.instance_at(path).expect("placed").body();
    assert_eq!(body("Slice.osc").carrier_reads(Target::Reaction(0)), vec![0]);
    assert_eq!(body("Slice.osc").dependencies(Target::Reaction(0)), Deps { params: vec![0], ..Deps::default() });
    assert_eq!(body("Slice.ramp").carrier_reads(Target::Reaction(0)), vec![1, 2, 3]);
    assert_eq!(body("Slice.ramp").carrier_reads(Target::Guard(0)), vec![0]);
    assert_eq!(body("Slice.adc").dependencies(Target::Reaction(0)), Deps { nodes: vec![0], params: vec![0, 1, 2], ..Deps::default() });
    assert_eq!(body("Slice.adc").carrier_reads(Target::Reaction(0)), Vec::<u32>::new());
    assert_eq!(body("Slice.a2d").dependencies(Target::Cross(1)), Deps { nodes: vec![0], params: vec![0, 1], ..Deps::default() });
    assert_eq!(body("Slice.a2d").dependencies(Target::Guard(1)), Deps::default());
    assert_eq!(body("Slice.a2d").dependencies(Target::Reaction(1)), Deps::default());

    // A commit of the wrong type and a guard that is no `bool` are typed like a drive.
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Bad", "bad.msl", 1));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, "bad.msl", 1, 20));
    let mut x = Dx::new(&mut b, "bad.msl", 2, 9);
    let guard = x.int(1, ValueAliases::u(8));
    x.at(3, 9);
    let value = x.int(3, ValueAliases::u(8));
    let commit = x.commit(0, value, None);
    let event = b.event(body, piperine_ir::Event { source: piperine_ir::EventSource::Changed(0), guard: Some(guard), origin: Design::origin("bad.msl", 2, 5) });
    Buck::reaction(&mut b, body, vec![event], vec![commit], vec![], "bad.msl", 3);
    let top = b.next_instance_id();
    let net = b.net(NetSpec { path: "Bad.q".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: Design::origin("bad.msl", 1, 20) });
    let top = Stage::instance(&mut b, "Bad", body, None, vec![], vec![], vec![net], 1);
    let broken = b.finish(top).check();
    let at = |line: u32, what: &str| broken.iter().filter(|e| e.origin.line == line && e.what.contains(what)).count();
    assert_eq!(at(2, "the guard of an event must be bool, found u8"), 1, "{broken:#?}");
    assert_eq!(at(3, "the value of a commit must be bool, found u8"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 2, "{broken:#?}");
}

#[test]
fn a_defective_digital_body_gives_one_entry_per_defect() {
    // One body, each defect of RS-09 AC 4 and of the task once:
    //
    //   module Bad(p: Electrical, n: Electrical, in en: Bit, out q: Bit, out aux: Bit, in stop: Bit) {   // line 1
    //       signal count: Bit<u4> = 0;                                                     // line 2
    //       en = true;                                      // line 3: drives an `in` port
    //       q = en;                                         // line 4: q is driven ...
    //   }
    //   on en.changed() ... on cross(count) ...             // lines 5, 6: a cross over an integer
    //   on E0, E7 {                                         // line 7: event 7 does not exist
    //       stop <- false;                                  // line 8: commits to an `in` port
    //       q <- true;                                      // line 9: ... and committed
    //   }
    //   analog { aux <- true;                               // line 10: a commit outside a reaction
    //            i(p, n) +> v(p, n) * if (en & true) { 1.0 } else { 0.0 }; }  // line 11
    const F: &str = "bad.msl";
    let mut b = IrBuilder::new();
    let el = Buck::electrical(&mut b);
    let port = |name: &str, col: u32| piperine_ir::BodyNode { name: name.into(), nature: el, role: piperine_ir::NodeRole::Port, init: None, origin: Design::origin(F, 1, col) };
    let mut x = Dx::new(&mut b, F, 10, 14);
    let yes = x.boolean(true);
    let stray = x.commit(3, yes, None);
    x.at(11, 22);
    let v = x.probe(Terminal::Node(0), Terminal::Node(1));
    let en = x.carrier(0, ValueType::Bool);
    let yes = x.boolean(true);
    let level = x.bin(BinaryOp::BitAnd, en, yes, ValueType::Bool);
    let one = x.real(1.0);
    let zero = x.real(0.0);
    let factor = x.if_(level, one, zero, ValueType::F64);
    let value = x.bin(BinaryOp::Mul, v, factor, ValueType::F64);
    let contribution = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] });
    let body = b.body(BodySpec {
        nodes: vec![port("p", 12), port("n", 27)],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        analog: Some(vec![stray, contribution]),
        ..Buck::spec("Bad", F, 1)
    });
    for (name, ty, role, init, line, col) in [
        ("en", ValueType::Bool, CarrierRole::In, None, 1, 42),
        ("stop", ValueType::Bool, CarrierRole::In, None, 1, 82),
        ("q", ValueType::Bool, CarrierRole::Out, None, 1, 57),
        ("aux", ValueType::Bool, CarrierRole::Out, None, 1, 69),
        ("count", ValueAliases::u(4), CarrierRole::Signal, Some(Constant::Int(0)), 2, 12),
    ] {
        b.carrier(body, Buck::carrier(name, ty, role, init, F, line, col));
    }
    let yes = Dx::new(&mut b, F, 3, 10).boolean(true);
    b.drive(body, Drive { carrier: 0, value: yes, after: None, origin: Design::origin(F, 3, 5), element: None, path: vec![] });
    let mut x = Dx::new(&mut b, F, 4, 9);
    let q = x.carrier(0, ValueType::Bool);
    b.drive(body, Drive { carrier: 2, value: q, after: None, origin: Design::origin(F, 4, 5), element: None, path: vec![] });
    let changed = b.event(body, piperine_ir::Event { source: piperine_ir::EventSource::Changed(0), guard: None, origin: Design::origin(F, 5, 8) });
    let count = Dx::new(&mut b, F, 6, 14).carrier(4, ValueAliases::u(4));
    b.event(body, piperine_ir::Event { source: piperine_ir::EventSource::Cross { e: count, ttol: None, etol: None }, guard: None, origin: Design::origin(F, 6, 8) });
    let mut x = Dx::new(&mut b, F, 8, 15);
    let no = x.boolean(false);
    let to_input = x.commit(1, no, None);
    x.at(9, 14);
    let yes = x.boolean(true);
    let to_driven = x.commit(2, yes, None);
    Buck::reaction(&mut b, body, vec![changed, 7], vec![to_input, to_driven], vec![], F, 7);
    let top = b.next_instance_id();
    let nets: Vec<NetId> = [("en", ValueType::Bool), ("stop", ValueType::Bool), ("q", ValueType::Bool), ("aux", ValueType::Bool), ("count", ValueAliases::u(4))]
        .into_iter()
        .map(|(n, ty)| b.net(NetSpec { path: format!("Bad.{n}"), ty, declared_by: top, init: None, origin: Design::origin(F, 1, 1) }))
        .collect();
    let p = b.node(NodeSpec { path: "Bad.p".into(), nature: el, declared_by: top, init: None, origin: Design::origin(F, 1, 12) });
    let top = Stage::instance(&mut b, "Bad", body, None, vec![NodeRef::Node(p), NodeRef::Ground], vec![], nets, 1);
    let broken = b.finish(top).check();

    let one = |item: &str, line: u32, what: &str| {
        let hits: Vec<_> = broken.iter().filter(|e| e.what.contains(what)).collect();
        assert_eq!(hits.len(), 1, "{what}: {broken:#?}");
        assert_eq!((hits[0].item.as_str(), hits[0].origin.file.as_str(), hits[0].origin.line), (item, F, line), "{what}");
    };
    one("body Bad drive en", 3, "drives the `in` port en: an input is written outside the body");
    one("body Bad reaction 0", 8, "commits to the `in` port stop: an input is written outside the body");
    one("body Bad carrier q", 1, "carrier q has both a drive and a commit");
    one("body Bad reaction 0", 7, "waits on event 7, out of range (2 events)");
    one("body Bad", 10, "a commit outside a reaction");
    one("body Bad event 1", 6, "a `cross` watches a real, found u4");
    assert_eq!(broken.len(), 6, "{broken:#?}");
}

#[test]
fn a_carrier_initial_value_outside_its_type_is_an_entry() {
    // part II §5: a carrier's `= init` is a value of its carried type, and §1 rule 6 gives
    // `UInt<4>` the values 0..=15 and `Int<4>` -8..=7. Each wrong initial value is one entry naming
    // the carrier, its origin and the value, never a panic later (verification F-B5):
    //
    //   module Inits(out a: Bit = 1, out b: Bit<u4> = 16, out c: Bit<i4> = -9, out d: Bit<f64> = true,
    //                out e: Bit<u4> = 15, out f: Bit<i4> = -8, out g: Bit<u4> = -1)
    const F: &str = "inits.msl";
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Inits", F, 1));
    for (name, ty, init, col) in [
        ("a", ValueType::Bool, Constant::Int(1), 18),
        ("b", ValueAliases::u(4), Constant::Int(16), 32),
        ("c", ValueAliases::i(4), Constant::Int(-9), 51),
        ("d", ValueType::F64, Constant::Bool(true), 70),
        ("e", ValueAliases::u(4), Constant::Int(15), 92),
        ("f", ValueAliases::i(4), Constant::Int(-8), 111),
        ("g", ValueAliases::u(4), Constant::Int(-1), 130),
    ] {
        b.carrier(body, Buck::carrier(name, ty, CarrierRole::Out, Some(init), F, 1, col));
    }
    let broken = Design::finish(b).check();
    let one = |carrier: &str, col: u32, what: &str| {
        let hits: Vec<_> = broken.iter().filter(|e| e.item == format!("body Inits carrier {carrier}")).collect();
        assert_eq!(hits.len(), 1, "{carrier}: {broken:#?}");
        assert_eq!((hits[0].origin.file.as_str(), hits[0].origin.line, hits[0].origin.col), (F, 1, col), "{carrier}");
        assert!(hits[0].what.contains(what), "{carrier}: {}", hits[0].what);
    };
    one("a", 18, "the initial value 1 of carrier a is not a value of its type bool");
    one("b", 32, "the initial value 16 of carrier b is not a value of its type u4");
    one("c", 51, "the initial value -9 of carrier c is not a value of its type i4");
    one("d", 70, "the initial value true of carrier d is not a value of its type f64");
    one("g", 130, "the initial value -1 of carrier g is not a value of its type u4");
    // `e = 15` and `f = -8` are the edges of their ranges: no entry
    assert_eq!(broken.len(), 5, "{broken:#?}");
}

#[test]
fn the_m1_prints_are_unchanged() {
    // RS-09 AC 5: every M1 fixture prints, in both forms, the bytes of its golden text.
    let device = |make: fn(&mut IrBuilder) -> BodyId, name: &str, line: u32, params: Vec<f64>| {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let top = Devices::alone(&mut b, body, name, line, params);
        b.finish(top)
    };
    let mut b = IrBuilder::new();
    let top = Devices::divider(&mut b);
    let cases = [
        ("resistor", device(Devices::resistor, "resistor", 5, vec![1000.0])),
        ("capacitor", device(Devices::capacitor, "capacitor", 11, vec![1e-12])),
        ("vsource", device(Devices::vsource, "vsource", 17, vec![10.0, 0.0])),
        ("isource", device(Devices::isource, "isource", 24, vec![1e-3, 0.0])),
        ("diode", device(Devices::diode, "diode", 31, vec![1e-14, 1.0, 0.0])),
        ("divider", b.finish(top)),
    ];
    for (name, ir) in cases {
        assert_eq!(ir.check(), vec![], "{name}");
        for (dir, text) in [("tests/golden", ir.print_structural()), ("tests/golden/full", ir.to_string())] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir).join(format!("{name}.txt"));
            let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(text, golden, "{} differs", path.display());
        }
    }
}

#[test]
fn the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts() {
    // The ten bodies of the buck slice (spec "Phase B scope"), each alone in a design with the
    // functions it calls, printed in both forms against `tests/golden/buck/` (the structural
    // form, msl-digital's contract) and `tests/golden/buck/full/`. `PIPERINE_UPDATE_GOLDEN=1`
    // rewrites them; the texts are read against the MSL each fixture transcribes.
    let bodies: [(&str, fn(&mut IrBuilder) -> BodyId); 10] = [
        ("osc", Buck::osc),
        ("dpwm", Buck::dpwm),
        ("setpoint_reg", Buck::setpoint_reg),
        ("ref_ramp", Buck::ref_ramp),
        ("pid", Buck::pid),
        ("gate_driver", Buck::gate_driver),
        ("adc", Buck::adc),
        ("comparator", Buck::comparator),
        ("switch", Buck::switch),
        ("dff", Buck::dff),
    ];
    for (name, make) in bodies {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let ir = Design::finish(b);
        assert_eq!(ir.check(), vec![], "{name}");
        let body = ir.body(body);
        // `rise()` is `changed` guarded by a read of the same carrier; `above(e)` is `cross(e)`
        // with no guard.
        for event in body.events() {
            match (&event.source, event.guard.map(|g| ir.expr(g))) {
                (piperine_ir::EventSource::Changed(c), Some(read)) => {
                    assert_eq!(read, &piperine_ir::Expr::Carrier(*c), "{name}: a rise reads its own carrier");
                }
                (piperine_ir::EventSource::Changed(_), None) => assert_eq!(name, "osc", "only the oscillator waits on a bare change"),
                (piperine_ir::EventSource::Cross { .. }, None) => assert_eq!(name, "comparator", "only the comparator crosses"),
                (source, guard) => panic!("{name}: an event outside the slice's forms: {source:?} guard {guard:?}"),
            }
        }
        for (dir, text) in [("tests/golden/buck", ir.print_structural()), ("tests/golden/buck/full", ir.to_string())] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir).join(format!("{name}.txt"));
            if std::env::var_os("PIPERINE_UPDATE_GOLDEN").is_some() {
                std::fs::create_dir_all(path.parent().expect("a directory")).expect("create the golden directory");
                std::fs::write(&path, &text).expect("write a golden");
            }
            let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(text, golden, "golden {} differs", path.display());
        }
    }
}

/// Functions over `same` and `cast`, each beside the MSL it transcribes.
struct Levels;

impl Levels {
    const F: &'static str = "levels.msl";

    /// `fn alike(a: Bit<u8>, b: Bit<u8>) -> bool { same(a, b) }`.
    fn alike(b: &mut IrBuilder) {
        let word = ValueAliases::u(8);
        let mut x = Dx::new(b, Levels::F, 4, 9);
        let (l, r) = (x.local(0, word.clone()), x.local(1, word.clone()));
        let body = x.same(l, r);
        let params = vec![Design::local("a", word.clone()), Design::local("b", word)];
        Functions::function(b, "alike", Levels::F, 4, params, ValueType::Bool, vec![], body);
    }

    /// `fn recode(a: u4) -> Option<i4> { cast::<u4, i4>(a) }` and, to a type that decodes only some tags,
    /// `fn tagged(a: u2) -> Option<u2> { cast::<u2, State>(a) }` (tags 0, 1, 2): a cast is an `Option` (§22.1.3).
    fn casts(b: &mut IrBuilder) {
        let mut x = Dx::new(b, Levels::F, 5, 9);
        let a = x.local(0, ValueAliases::u(4));
        let body = x.level_cast(a, ValueAliases::i(4), None, ValueType::Option(Box::new(ValueAliases::i(4))));
        Functions::function(b, "recode", Levels::F, 5, vec![Design::local("a", ValueAliases::u(4))], ValueType::Option(Box::new(ValueAliases::i(4))), vec![], body);
        let mut x = Dx::new(b, Levels::F, 6, 9);
        let a = x.local(0, ValueAliases::u(2));
        let body = x.level_cast(a, ValueAliases::u(2), Some(vec![0, 1, 2]), ValueType::Option(Box::new(ValueAliases::u(2))));
        Functions::function(b, "tagged", Levels::F, 6, vec![Design::local("a", ValueAliases::u(2))], ValueType::Option(Box::new(ValueAliases::u(2))), vec![], body);
    }
}

#[test]
fn same_and_cast_type_print_and_check() {
    let mut b = IrBuilder::new();
    Levels::alike(&mut b);
    Levels::casts(&mut b);
    let ir = Design::finish(b);
    assert_eq!(ir.check(), vec![], "every form is typed as §22.1.3 and §22.1.4 give");

    // The structural print writes each node by the name the source gives it, and a type only
    // where it differs from the one the form implies.
    Printed::holds(&ir, &["fn alike(a: u8, b: u8) -> bool", "  local a: u8", "  local b: u8", "  same(a, b)"]);
    Printed::holds(&ir, &["fn recode(a: u4) -> Option<i4>", "  local a: u4", "  cast::<i4>(a)"]);
    Printed::holds(&ir, &["fn tagged(a: u2) -> Option<u2>", "  local a: u2", "  cast::<u2 in [0, 1, 2]>(a)"]);
    let full = ir.to_string();
    for form in [": bool = same e", ": Option<i4> = level_cast e", ": Option<u2> = level_cast e", "tags [0, 1, 2]"] {
        assert!(full.contains(form), "{form} missing from:\n{full}");
    }

    // Each form whose operands fit no rule is one entry at its origin.
    let mut b = IrBuilder::new();
    let mut x = Dx::new(&mut b, Levels::F, 1, 9);
    let (l, r) = (x.local(0, ValueType::Bool), x.local(1, ValueAliases::u(1)));
    let alike = x.same(l, r);
    Functions::function(&mut b, "bad_same", Levels::F, 1, vec![Design::local("a", ValueType::Bool), Design::local("b", ValueAliases::u(1))], ValueType::Bool, vec![], alike);
    let mut x = Dx::new(&mut b, Levels::F, 2, 9);
    let a = x.local(0, ValueAliases::u(4));
    let wide = x.level_cast(a, ValueAliases::u(5), None, ValueType::Option(Box::new(ValueAliases::u(5))));
    Functions::function(&mut b, "bad_cast", Levels::F, 2, vec![Design::local("a", ValueAliases::u(4))], ValueType::Option(Box::new(ValueAliases::u(5))), vec![], wide);
    let broken = Design::finish(b).check();
    let at = |line: u32, what: &str| broken.iter().filter(|e| e.origin.line == line && e.what.contains(what)).count();
    assert_eq!(at(1, "the operands of `same` do not fit its types"), 1, "{broken:#?}");
    assert_eq!(at(2, "the operands of `cast` do not fit its types"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 2, "{broken:#?}");
}

