//! Scope: structure of `piperine_lang::elab` for msl-digital (DIG-09): discrete carriers as the
//! carriers of their body, the design's nets and the carrier bindings of each placement, drives,
//! events and heads, reactions and commits, child params over the parent's params, and the
//! invariants of elaboration over the buck. Expected values are read off the `.msl` and
//! `docs/spec/` (part II §5, part IV §12 and §13), never off the elaborator.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

#[path = "support/buck.rs"]
#[allow(dead_code)]
mod buck;

use piperine_ir::{Body, CarrierRole, Constant, Ir, ValueType};
use support::Elab;

/// Reading the elaborated IR of a test.
struct Read<'a> {
    ir: &'a Ir,
}

impl<'a> Read<'a> {
    fn body(&self, name: &str) -> Body<'a> {
        self.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}` in:\n{}", self.ir.print_structural()))
    }

    /// The name, role, type and initial value of each carrier of `body`, in order.
    fn carriers(&self, body: &str) -> Vec<(String, CarrierRole, ValueType, Option<Constant>)> {
        self.body(body).carriers().iter().map(|c| (c.name.clone(), c.role, c.ty.clone(), c.init)).collect()
    }

    /// The paths of the nets an instance binds, in its body's carrier order.
    fn nets_of(&self, instance: &str) -> Vec<String> {
        let at = self.ir.instance_at(instance).unwrap_or_else(|| panic!("no instance `{instance}`"));
        (0..at.nets().len()).map(|i| at.net(i).map(|n| n.path().to_string()).unwrap_or_default()).collect()
    }

    fn net_paths(&self) -> Vec<String> {
        self.ir.nets().map(|n| n.path().to_string()).collect()
    }
}

/// The declaration of the buck module `name` of `file` with an empty body: its ports as written,
/// none of its body items. A module whose body holds a definition elaborates whole only from T29.
fn ports_of(file: &str, name: &str) -> String {
    let item = buck::item(file, &format!("pub module {name}("));
    let open = item.find(") {").unwrap_or_else(|| panic!("no body after the ports of {name}"));
    format!("{} }}", &item[..open + 3])
}

#[test]
fn carriers_follow_ports_then_signals() {
    // DIG-09 AC 1, C75: `RefRamp` (examples/buck/src/control.msl) as written, its ports in
    // declaration order, `reference` with its initial value 0.
    let ir = Elab::ir(&ports_of("control.msl", "RefRamp"), "RefRamp");
    let read = Read { ir: &ir };
    use CarrierRole::{In, Out, Signal};
    assert_eq!(
        read.carriers("RefRamp"),
        vec![
            ("tick".to_string(), In, ValueType::Bool, None),
            ("run".to_string(), In, ValueType::Bool, None),
            ("target".to_string(), In, ValueType::UInt(10), None),
            ("reference".to_string(), Out, ValueType::UInt(10), Some(Constant::Int(0))),
            ("at_target".to_string(), Out, ValueType::Bool, None),
        ]
    );
    // `Pid` whole: its five ports, then its three signals, each folded initial value of the
    // carried type (`integ: Bit<i32> = 0`, `e_prev: Bit<i16> = 0`).
    let ir = Elab::ir(&buck::item("control.msl", "pub module Pid("), "Pid");
    let read = Read { ir: &ir };
    assert_eq!(
        read.carriers("Pid"),
        vec![
            ("tick".to_string(), In, ValueType::Bool, None),
            ("run".to_string(), In, ValueType::Bool, None),
            ("reference".to_string(), In, ValueType::UInt(10), None),
            ("meas".to_string(), In, ValueType::UInt(10), None),
            ("duty".to_string(), Out, ValueType::UInt(12), Some(Constant::Int(0))),
            ("integ".to_string(), Signal, ValueType::Int(32), Some(Constant::Int(0))),
            ("deriv".to_string(), Signal, ValueType::Int(32), Some(Constant::Int(0))),
            ("e_prev".to_string(), Signal, ValueType::Int(16), Some(Constant::Int(0))),
        ]
    );
    // A node port among carrier ports stays a node: `Adc(ain: Electrical, in sample: Bit, ..)`
    // has one node and three carriers; a carrier is never a node and a node never a carrier.
    let src = format!("{}\nmodule Top() {{ signal a: Electrical; signal s: Bit; signal c: Bit<u10>; signal d: Bit; let adc = Adc(a, s, c, d); }}", ports_of("sense.msl", "Adc"));
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    let adc = read.body("Adc");
    assert_eq!(adc.nodes().iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["ain"]);
    assert_eq!(adc.carriers().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["sample", "code", "done"]);
    assert_eq!(read.carriers("Adc")[1].3, Some(Constant::Int(0)), "`out code: Bit<u10> = 0`");
    assert_eq!(read.carriers("Adc")[2].3, Some(Constant::Bool(false)), "`out done: Bit = false`");
    // An initial value is folded (§5.2.3): over `const`s and operators, an enum variant as its tag.
    let src = "const K: u8 = 2 + 3;\n#[derive(Repr, Clone, Copy, PartialEq)]\nenum Mode { Off, On, Hold }\nmodule Top() { signal k: Bit<u8> = K * 2; signal m: Bit<Mode> = Mode::Hold; signal x: Bit<f64> = 0.5 * 3.0; }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.carriers("Top").iter().map(|c| (c.2.clone(), c.3)).collect::<Vec<_>>(), [(ValueType::UInt(8), Some(Constant::Int(10))), (ValueType::UInt(2), Some(Constant::Int(2))), (ValueType::F64, Some(Constant::Real(piperine_ir::Real::new(1.5))))]);
    // One that reads a param is a value per instance: `P0001`, no `Ir`.
    let err = Elab::run("module Top() { r: f64 = 1.0; signal x: Bit<f64> = r; }", "Top").expect_err("an initial value that reads a param");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("initial value") && err[0].diagnostic.message.contains("param"), "{}", err[0].diagnostic.message);
}

#[test]
fn every_signal_carrier_is_a_design_net() {
    // DIG-09 AC 4, C78: one net per signal carrier of each instance, named by its path; a carrier
    // port is no net of its own, it is bound to the parent's net it is connected to.
    let src = "module Leaf(in i: Bit, out o: Bit<u8>) { signal own: Bit = true; }\n\
               module Top() { signal a: Bit; signal b1: Bit<u8>; signal b2: Bit<u8>; let x = Leaf(a, b1); let y = Leaf(a, b2); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.net_paths(), ["Top.a", "Top.b1", "Top.b2", "Top.x.own", "Top.y.own"]);
    assert_eq!(read.nets_of("Top"), ["Top.a", "Top.b1", "Top.b2"]);
    assert_eq!(read.nets_of("Top.x"), ["Top.a", "Top.b1", "Top.x.own"], "`i` on `a`, `o` on `b1`, then its own signal");
    assert_eq!(read.nets_of("Top.y"), ["Top.a", "Top.b2", "Top.y.own"]);
    for net in ir.nets() {
        let declarer = net.declared_by().path().to_string();
        let expected = net.path().rsplit_once('.').map(|(head, _)| head.to_string()).unwrap_or_default();
        assert_eq!(declarer, expected, "a net is declared by the instance whose signal it is: {}", net.path());
    }
    // Two levels: a carrier handed down twice stays one net.
    let src = "module Leaf(in i: Bit) { }\nmodule Mid(in m: Bit) { let l = Leaf(m); }\nmodule Top() { signal a: Bit; let k = Mid(a); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.net_paths(), ["Top.a"]);
    assert_eq!(read.nets_of("Top.k.l"), ["Top.a"]);
}

#[test]
fn a_top_with_discrete_ports_gets_one_net_per_port() {
    // DIG-09 AC 5, C79: the host drives a top-level port (§5.2.3); each is a net `<Top>.<port>`.
    let ir = Elab::ir("module Top(in en: Bit, out pgood: Bit) { signal s: Bit; }", "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.net_paths(), ["Top.en", "Top.pgood", "Top.s"]);
    assert_eq!(read.nets_of("Top"), ["Top.en", "Top.pgood", "Top.s"]);
    assert!(ir.nets().all(|n| n.declared_by().path() == "Top"));
    // A continuous port on the top has nothing to connect to: `P0001` at the module's name,
    // naming the port.
    let src = "module Top(in en: Bit, p: Electrical) { }";
    let err = Elab::run(src, "Top").expect_err("a continuous port on the top");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("`p`"), "{}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "Top");
}

#[test]
fn a_carrier_with_no_value_and_no_drive_is_kept() {
    // Edge case 2, C164: `signal en: Bit;` with no drive is allowed and unknown (§5.2.8): one
    // carrier with no initial value, one net with none.
    let ir = Elab::ir("module Top() { signal en: Bit; }", "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.carriers("Top"), vec![("en".to_string(), CarrierRole::Signal, ValueType::Bool, None)]);
    assert!(read.body("Top").drives().is_empty());
    let net = ir.net_at("Top.en").expect("the net of `en`");
    assert_eq!(net.init(), None);
}

#[test]
fn a_placement_records_its_carrier_bindings() {
    // C171: the authored `Child` shows what a placement connects at each carrier port of the
    // child's body, in port order: the parent's carriers (nodes stay terminals).
    let src = "module Leaf(p: Electrical, in i: Bit, out o: Bit<u8>) { }\nmodule Plain(p: Electrical) { }\n\
               module Top() { signal n: Electrical; signal a: Bit; signal b: Bit<u8>; let x = Leaf(n, a, b); let q = Plain(n); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let top = read.body("Top");
    let (a, b) = (top.carriers().iter().position(|c| c.name == "a"), top.carriers().iter().position(|c| c.name == "b"));
    assert_eq!(top.child_carriers(0), [a.map(|i| i as u32), b.map(|i| i as u32)]);
    assert!(top.child_carriers(1).is_empty(), "a child with no carrier port binds none");
    assert_eq!(top.children()[0].nodes, [piperine_ir::Terminal::Node(0)], "the node port stays a terminal");
    assert!(ir.print_structural().contains("carriers (a, b)"), "{}", ir.print_structural());
}

#[test]
fn a_net_takes_the_init_of_its_declaring_carrier() {
    // C172 (ir-rust F-B2): a net's init is the folded initial value of the carrier that declares
    // it; with none, none. A child's `out` port with an initial value does not give the parent's
    // net one: the child's own carrier holds it (§5.2.6).
    let src = "module Leaf(out o: Bit = true) { }\n\
               module Top(out q: Bit<u4> = 3) { signal x: Bit = false; signal y: Bit<u8>; signal s: Bit; let l = Leaf(s); }";
    let ir = Elab::ir(src, "Top");
    let init = |path: &str| ir.net_at(path).unwrap_or_else(|| panic!("no net `{path}`")).init();
    assert_eq!(init("Top.q"), Some(Constant::Int(3)));
    assert_eq!(init("Top.x"), Some(Constant::Bool(false)));
    assert_eq!(init("Top.y"), None);
    assert_eq!(init("Top.s"), None);
    let read = Read { ir: &ir };
    assert_eq!(read.carriers("Leaf")[0].3, Some(Constant::Bool(true)), "the driver's carrier keeps its own initial value");
}

/// The structural print of `body` alone: its header and the lines under it.
fn body_text(ir: &Ir, body: &str) -> String {
    let text = ir.print_structural();
    let head = format!("body {body}\n");
    let start = text.find(&head).unwrap_or_else(|| panic!("no `{head}` in:\n{text}"));
    let rest = &text[start..];
    let end = rest[1..].find("\nbody ").or_else(|| rest[1..].find("\nfn ")).or_else(|| rest[1..].find("\ninstance ")).map_or(rest.len(), |at| at + 2);
    rest[..end].to_string()
}

#[test]
fn a_definition_is_a_drive() {
    // DIG-09 AC 2, C76: `GateDriver` (examples/buck/src/power.msl): `hs_gate = (hs_cmd & !ls_cmd)
    // after tpd;` is one drive of `hs_gate` with its `after`, and so is `ls_gate`, in source order.
    let ir = Elab::ir(&buck::item("power.msl", "pub module GateDriver("), "GateDriver");
    let read = Read { ir: &ir };
    let body = read.body("GateDriver");
    let carrier = |name: &str| body.carriers().iter().position(|c| c.name == name).map(|i| i as u32);
    assert_eq!(body.drives().len(), 2);
    assert_eq!(Some(body.drives()[0].carrier), carrier("hs_gate"));
    assert_eq!(Some(body.drives()[1].carrier), carrier("ls_gate"));
    for drive in body.drives() {
        let after = drive.after.expect("each definition has its `after`");
        assert_eq!(ir.expr(after), &piperine_ir::Expr::Param(0), "`after tpd` reads the param `tpd`");
    }
    let text = body_text(&ir, "GateDriver");
    assert!(text.contains("  drive hs_gate = (hs_cmd & (!ls_cmd)) after tpd\n  drive ls_gate = (ls_cmd & (!hs_cmd)) after tpd\n"), "{text}");
    // An undelayed definition has no `after`: `RefRamp`'s `at_target = reference == target;`, a
    // comparison of two carrier reads, which may be unknown (§3.3).
    let ir = Elab::ir(&buck::item("control.msl", "pub module RefRamp("), "RefRamp");
    let read = Read { ir: &ir };
    let body = read.body("RefRamp");
    assert_eq!(body.drives().len(), 1);
    assert_eq!(body.drives()[0].after, None);
    assert_eq!(ir.expr_type(body.drives()[0].value), &ValueType::Flagged(Box::new(ValueType::Bool)));
    assert!(body_text(&ir, "RefRamp").contains("  drive at_target = (reference == target)\n"));
}

/// The buck's `Controller` (examples/buck/src/top.msl) as written, over the buck's own constants,
/// enums and `target_codes`, its children declared with the buck's ports and no body.
fn controller() -> String {
    let children = [("control.msl", "SetpointReg"), ("control.msl", "RefRamp"), ("control.msl", "Pid"), ("supervisor.msl", "Supervisor"), ("dpwm.msl", "Dpwm")].map(|(file, name)| ports_of(file, name));
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        buck::spec(),
        buck::item("supervisor.msl", "pub enum Mode"),
        buck::item("supervisor.msl", "pub enum Fault"),
        buck::item("control.msl", "pub fn target_codes"),
        children.join("\n"),
        buck::item("top.msl", "pub module Controller("),
    )
}

#[test]
fn an_expression_at_an_in_port_is_a_carrier_and_a_drive() {
    // DIG-09 AC 3, C77: `RefRamp(done, run, target_codes(setpoint.known_or(0)), ..)` in
    // `Controller` adds the signal carrier `ramp.target` to `Controller` with its drive, and binds
    // the child's port `target` to it (§8.2.9).
    let ir = Elab::ir(&controller(), "Controller");
    let read = Read { ir: &ir };
    let body = read.body("Controller");
    let at = body.carriers().iter().position(|c| c.name == "ramp.target").expect("the carrier `ramp.target`");
    let carrier = &body.carriers()[at];
    assert_eq!((carrier.role, &carrier.ty, carrier.init), (CarrierRole::Signal, &ValueType::UInt(10), None), "a signal of `target`'s type, its value is the drive's");
    let drives: Vec<_> = body.drives().iter().filter(|d| d.carrier == at as u32).collect();
    assert_eq!(drives.len(), 1, "one drive of `ramp.target`");
    assert_eq!(drives[0].after, None);
    assert!(body_text(&ir, "Controller").contains("  drive ramp.target = target_codes(setpoint.known_or(0u8))\n"), "{}", body_text(&ir, "Controller"));
    let ramp = body.children().iter().position(|c| c.name == "ramp").expect("the child `ramp`");
    let setpoint = body.carriers().iter().position(|c| c.name == "setpoint").map(|i| i as u32);
    let (done, run, reference, at_target) = ["done", "run", "reference", "at_target"].map(|n| body.carriers().iter().position(|c| c.name == n).map(|i| i as u32)).into();
    assert_eq!(body.child_carriers(ramp), [done, run, Some(at as u32), reference, at_target], "the port `target` is bound to `ramp.target`");
    assert!(setpoint.is_some());
    // The carrier is a net of the instance, like any signal.
    assert!(ir.net_at("Controller.ramp.target").is_some());
    // A carrier given as it is binds the port, with no carrier or drive of its own: only one
    // anonymous definition in `Controller`.
    assert_eq!(body.carriers().iter().filter(|c| c.name.contains('.')).count(), 1);
}

/// A known or unknown `bool`: `None` is unknown (§3.3).
type Level = Option<bool>;

/// The value of a lowered event guard over the previous and the present value of the carrier
/// `c` it reads: the flag-total reads of §3.3.4 as the spec defines them. Any other form is a
/// failure of the test, never a guess.
fn guard_value(ir: &Ir, e: piperine_ir::ExprId, c: u32, prev: Level, cur: Level) -> Level {
    use piperine_ir::{BinaryOp, Expr, UnaryOp};
    let value = |e| guard_value(ir, e, c, prev, cur);
    match ir.expr(e) {
        Expr::Bool(b) => Some(*b),
        Expr::Carrier(k) if *k == c => cur,
        Expr::Previous(k) if *k == c => prev,
        Expr::IsKnown(x) => Some(value(*x).is_some()),
        Expr::High(x) => Some(value(*x) == Some(true)),
        Expr::KnownOr { value: x, default } => value(*x).or(value(*default)),
        Expr::Unary { op: UnaryOp::Not, expr } => value(*expr).map(|b| !b),
        Expr::Binary { op: BinaryOp::BitAnd, lhs, rhs } => match (value(*lhs), value(*rhs)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Expr::Binary { op: BinaryOp::BitOr, lhs, rhs } => match (value(*lhs), value(*rhs)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        other => panic!("a form an edge guard does not hold: {other:?}"),
    }
}

/// A module `M` with a node, two carriers in and one out, placed by `Top`, and `digital` as its
/// `digital` block.
fn edges(digital: &str) -> String {
    format!("module M(a: Electrical, in c: Bit, in d: Bit<u8>, out q: Bit) {{ }}\ndigital for M {{ {digital} }}\nmodule Top() {{ signal n: Electrical; signal c: Bit; signal d: Bit<u8>; signal q: Bit; let m = M(n, c, d, q); }}")
}

#[test]
fn changed_rise_fall_and_cross_lower_to_events() {
    // DIG-09 AC 6, C80. The stdlib's own text (§22.1) is what `rise` and `fall` mean.
    let values = std::fs::read_to_string(format!("{}/std/values.msl", env!("CARGO_MANIFEST_DIR"))).expect("std/values.msl");
    assert!(values.contains("self.changed() && self.prev().level(0) != Some(true)  && self.level(0) != Some(false)"), "the stdlib `rise` is the oracle");
    assert!(values.contains("self.changed() && self.prev().level(0) != Some(false) && self.level(0) != Some(true)"), "the stdlib `fall` is the oracle");
    let src = edges("on c.changed() { q <- true; }\non c.rise() { q <- true; }\non c.fall() { q <- false; }\non cross(v(a) - 0.5) { q <- true; }\non d.changed() && d.prev().known_or(0) == 3 { q <- d.prev().is_known(); }");
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    let m = read.body("M");
    let c = m.carriers().iter().position(|k| k.name == "c").expect("c") as u32;
    let d = m.carriers().iter().position(|k| k.name == "d").expect("d") as u32;
    use piperine_ir::{EventSource, Expr};
    let events = m.events();
    assert_eq!(events.len(), 5);
    assert_eq!((&events[0].source, events[0].guard), (&EventSource::Changed(c), None), "`changed` is the change, unguarded");
    assert_eq!(events[1].source, EventSource::Changed(c));
    assert_eq!(events[2].source, EventSource::Changed(c));
    // Term by term, the two conditions of each edge (design "From a source form to IR nodes").
    let rise = events[1].guard.expect("a rise has a guard");
    let fall = events[2].guard.expect("a fall has a guard");
    assert_eq!(ir.expr_type(rise), &ValueType::Bool, "every part is a flag-total read: the guard is never unknown");
    assert_eq!(ir.expr_type(fall), &ValueType::Bool);
    let text = ir.print_structural();
    assert!(text.contains("guard ((!c.prev().high()) & ((!c.is_known()) | c.known_or(false)))"), "{text}");
    assert!(text.contains("guard ((!(c.prev().is_known() & (!c.prev().known_or(true)))) & (!c.high()))"), "{text}");
    // IEEE 1364 posedge and negedge (§22.1): rise on 0→1, 0→x, x→1; fall on 1→0, 1→x, x→0; neither
    // on any other change.
    let (lo, hi, x) = (Some(false), Some(true), None);
    for (prev, cur, rises, falls) in [(lo, hi, true, false), (lo, x, true, false), (x, hi, true, false), (hi, lo, false, true), (hi, x, false, true), (x, lo, false, true)] {
        assert_eq!(guard_value(&ir, rise, c, prev, cur), Some(rises), "rise on {prev:?} -> {cur:?}");
        assert_eq!(guard_value(&ir, fall, c, prev, cur), Some(falls), "fall on {prev:?} -> {cur:?}");
    }
    // `cross(e)` watches `e`.
    let EventSource::Cross(watched) = events[3].source else { panic!("a cross: {:?}", events[3].source) };
    assert!(matches!(ir.expr(watched), Expr::Binary { .. }) && text.contains("cross (probe(a, gnd) - 0.5)"), "{text}");
    // `c.prev()` in a guard and in a reaction body is the previous-value read.
    assert_eq!(events[4].source, EventSource::Changed(d));
    assert!(text.contains("guard (d.prev().known_or(0u8) == 3u8)"), "{text}");
    let reaction = &m.reactions()[4];
    let piperine_ir::Stmt::Commit { value, .. } = ir.stmt(reaction.stmts[0]) else { panic!("a commit") };
    let Expr::IsKnown(inner) = ir.expr(*value) else { panic!("`is_known` of the previous value") };
    assert_eq!(ir.expr(*inner), &Expr::Previous(d));
    assert_eq!(ir.expr_type(*inner), &ValueType::Flagged(Box::new(ValueType::UInt(8))), "a previous value may be unknown, as a read");
}

#[test]
fn a_guard_splits_into_its_discrete_and_continuous_parts() {
    // DIG-09 AC 7, C81: `on x.changed() && en.high() && analysis(Static)` keeps `en.high()` on
    // the event and wraps the body in `if static` (abi.md F2: a probing guard is no discrete
    // function).
    let src = "module M(a: Electrical, in x: Bit, in en: Bit, out q: Bit) { }\n\
               digital for M {\n\
                 on x.changed() && en.high() && analysis(Static) { q <- true; }\n\
                 on x.changed() && v(a) > 0.5 && en.high() { q <- false; }\n\
                 on x.changed() && en { q <- true; }\n\
               }\n\
               module Top() { signal n: Electrical; signal x: Bit; signal en: Bit; signal q: Bit; let m = M(n, x, en, q); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let m = read.body("M");
    let en = m.carriers().iter().position(|k| k.name == "en").expect("en") as u32;
    use piperine_ir::{Env, Expr, Stmt};
    let guard = m.events()[0].guard.expect("the discrete part stays on the event");
    let Expr::High(read_en) = ir.expr(guard) else { panic!("`en.high()` alone: {:?}", ir.expr(guard)) };
    assert_eq!(ir.expr(*read_en), &Expr::Carrier(en));
    let wrapped = |r: usize| {
        let reaction = &m.reactions()[r];
        assert_eq!(reaction.stmts.len(), 1, "the body is one `if`");
        let Stmt::Expr(whole) = ir.stmt(reaction.stmts[0]) else { panic!("an `if` statement") };
        let Expr::If { cond, then, .. } = ir.expr(*whole) else { panic!("an `if`") };
        let Expr::Block { stmts, .. } = ir.expr(*then) else { panic!("the body as a block") };
        assert!(matches!(ir.stmt(stmts[0]), Stmt::Commit { .. }), "the body's commit, under the `if`");
        *cond
    };
    assert_eq!(ir.expr(wrapped(0)), &Expr::Env(Env::Static), "`analysis(Static)` is the `if`");
    // A probe is continuous wherever it stands among the conjuncts; the discrete ones stay on the
    // event in their order.
    let probe = wrapped(1);
    assert!(matches!(ir.expr(probe), Expr::Binary { .. }) && ir.print_structural().contains("if (probe(a, gnd) > 0.5) {"));
    let Expr::High(_) = ir.expr(m.events()[1].guard.expect("en.high()")) else { panic!("en.high() on the event") };
    // A guard that may be unknown is kept as written: no `known_or` is added, the generated code
    // decides it (ir-rust RS-11 AC 5).
    let flagged = m.events()[2].guard.expect("a guard");
    assert_eq!(ir.expr(flagged), &Expr::Carrier(en));
    assert_eq!(ir.expr_type(flagged), &ValueType::Flagged(Box::new(ValueType::Bool)));
    assert!(!matches!(ir.stmt(m.reactions()[2].stmts[0]), Stmt::Expr(_)), "no `if` without a continuous conjunct");
}

#[test]
fn an_or_head_is_one_reaction_or_one_per_disjunct() {
    // DIG-09 AC 8, C82.
    let src = "module M(a: Electrical, in x: Bit, in y: Bit, out q: Bit) { }\n\
               digital for M {\n\
                 on x.rise() || y.rise() { q <- true; }\n\
                 on cross(v(a)) || (x.changed() && v(a) > 0.0) { q <- false; }\n\
               }\n\
               module Top() { signal n: Electrical; signal x: Bit; signal y: Bit; signal q: Bit; let m = M(n, x, y, q); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let m = read.body("M");
    let (x, y) = (m.carriers().iter().position(|k| k.name == "x").expect("x") as u32, m.carriers().iter().position(|k| k.name == "y").expect("y") as u32);
    use piperine_ir::{EventSource, Stmt};
    assert_eq!(m.reactions().len(), 3, "one reaction for the first head, two for the second");
    // Discrete disjuncts: one reaction listing each event.
    assert_eq!(m.reactions()[0].on, [0, 1]);
    assert_eq!((&m.events()[0].source, &m.events()[1].source), (&EventSource::Changed(x), &EventSource::Changed(y)));
    // A disjunct whose guard probes: one reaction per disjunct, in source order, each on its own
    // event; the probing one under its `if`.
    assert_eq!(m.reactions()[1].on, [2]);
    assert_eq!(m.reactions()[2].on, [3]);
    assert!(matches!(m.events()[2].source, EventSource::Cross(_)));
    assert_eq!(m.events()[3].source, EventSource::Changed(x));
    assert!(matches!(ir.stmt(m.reactions()[1].stmts[0]), Stmt::Commit { .. }), "the cross runs its body as written");
    assert!(matches!(ir.stmt(m.reactions()[2].stmts[0]), Stmt::Expr(_)), "the change runs it under `if v(a) > 0.0`");
}

#[test]
fn a_head_the_ir_cannot_hold_is_p0001() {
    // DIG-09 AC 9, C83. Two reactions for one head would write a record twice when both fire.
    let src = "module M(a: Electrical, in x: Bit, out q: Bit) { }\n\
               digital for M { on cross(v(a)) || (x.changed() && v(a) > 0.0) { emit!(\"log\", n = 1.0); } }\n\
               module Top() { signal n: Electrical; signal x: Bit; signal q: Bit; let m = M(n, x, q); }";
    let err = Elab::run(src, "Top").expect_err("a split head over a body that emits");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("emits a record"), "{}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "cross(v(a)) || (x.changed() && v(a) > 0.0)");
    // An event read as a `bool` in a reaction body: the IR has no read of it (§13 rule 2).
    let src = "module M(in x: Bit, out q: Bit) { }\ndigital for M { on x.changed() { if x.rise() { q <- true; } } }\nmodule Top() { signal x: Bit; signal q: Bit; let m = M(x, q); }";
    let err = Elab::run(src, "Top").expect_err("an event read as a bool");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("an event read as a `bool`"), "{}", err[0].diagnostic.message);
}

/// The text of a golden print of `crates/piperine-ir/tests/golden/buck/` (`ir-rust` T29): the
/// slice transcribed by hand from the buck's `.msl`.
fn golden(name: &str) -> String {
    let path = format!("{}/../piperine-ir/tests/golden/buck/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The lines of the reaction `R<k>` of a body print: its header, its locals and its statements.
fn reaction_text(body: &str, k: usize) -> String {
    let head = format!("  reaction R{k} on");
    let start = body.find(&head).unwrap_or_else(|| panic!("no `{head}` in:\n{body}"));
    let rest = &body[start..];
    let end = rest[1..].find("\n  reaction ").or_else(|| rest[1..].find("\nbody ")).map_or(rest.len(), |at| at + 2);
    rest[..end].trim_end().to_string()
}

#[test]
fn a_reaction_body_lowers_in_source_order() {
    // DIG-09 AC 10, C84: `Adc`'s reaction (examples/buck/src/sense.msl) is one `let` holding a
    // probe of `ain`, then three commits each with its `after`, in source order.
    let src = format!("{}\nmodule Top() {{ signal a: Electrical; signal s: Bit; signal c: Bit<u10>; signal d: Bit; let adc = Adc(a, s, c, d); }}", buck::module_with_digital("sense.msl", "Adc"));
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    let adc = read.body("Adc");
    use piperine_ir::{Expr, Stmt};
    let reaction = &adc.reactions()[0];
    let carrier = |name: &str| adc.carriers().iter().position(|c| c.name == name).expect(name) as u32;
    let kinds: Vec<_> = reaction.stmts.iter().map(|s| ir.stmt(*s).clone()).collect();
    let Stmt::Let { init, .. } = &kinds[0] else { panic!("first the `let q`: {kinds:?}") };
    let probes = |e: piperine_ir::ExprId| {
        let mut stack = vec![e];
        let mut found = false;
        while let Some(x) = stack.pop() {
            match ir.expr(x) {
                Expr::Probe { plus, .. } => found |= *plus == piperine_ir::Terminal::Node(0),
                Expr::Cast { expr, .. } => stack.push(*expr),
                Expr::Method { receiver, args, .. } => stack.extend(std::iter::once(*receiver).chain(args.iter().copied())),
                Expr::Binary { lhs, rhs, .. } => stack.extend([*lhs, *rhs]),
                _ => {}
            }
        }
        found
    };
    assert!(probes(*init), "the `let` probes `ain`");
    let commits: Vec<(u32, bool)> = kinds[1..].iter().map(|s| match s {
        Stmt::Commit { carrier, after, .. } => (*carrier, after.is_some()),
        other => panic!("a commit: {other:?}"),
    }).collect();
    assert_eq!(commits, [(carrier("code"), true), (carrier("done"), true), (carrier("done"), true)], "code, then done twice, each `after`");
    assert!(reaction_text(&body_text(&ir, "Adc"), 0).ends_with("    code <- q after tconv\n    done <- true after tconv\n    done <- false after (tconv + tstrobe)"));

    // `Pid` (control.msl), statement by statement: the hand transcription of `ir-rust` T29 with
    // its one listed difference, `DUTY_MAX as i32` folded to `2880i32` (DIG-08 AC 2).
    let src = format!("{}\n{}", buck::spec(), buck::module_with_digital("control.msl", "Pid"));
    let ir = Elab::ir(&src, "Pid");
    let expected = reaction_text(&golden("pid"), 0).replace("(2880u12 as i32)", "2880i32");
    assert_eq!(reaction_text(&body_text(&ir, "Pid"), 0), expected);

    // `Dpwm`'s reaction (dpwm.msl), statement by statement against the same transcription; the
    // `Never` monitor beside it is a placement, not part of the reaction (its `require` is
    // inlined from T34 on), so it is left out here.
    let text = buck::module_with_digital("dpwm.msl", "Dpwm");
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|l| l.contains("let no_shoot_through")).expect("the `Never` placement of `Dpwm`");
    // The placement and the doc comment above it.
    let dpwm: String = lines.iter().enumerate().filter(|(k, _)| *k != at && !(*k + 1 == at && lines[*k].trim_start().starts_with("///"))).map(|(_, l)| *l).collect::<Vec<_>>().join("\n");
    let src = format!("{}\n{}\n{}\n{dpwm}", buck::spec(), buck::item("dpwm.msl", "fn rev3"), buck::item("dpwm.msl", "fn dithered_width"));
    let ir = Elab::ir(&src, "Dpwm");
    assert_eq!(reaction_text(&body_text(&ir, "Dpwm"), 0), reaction_text(&golden("dpwm"), 0));
}

#[test]
fn a_module_with_only_a_digital_block_has_no_analog_statements() {
    // Edge case 1, C163: `Osc` (dpwm.msl) has a `digital` block and no `analog` block.
    let ir = Elab::ir(&buck::module_with_digital("dpwm.msl", "Osc"), "Osc");
    let read = Read { ir: &ir };
    let osc = read.body("Osc");
    assert!(osc.analog().is_none(), "no analog statements");
    assert!(osc.branches().is_empty() && osc.nodes().is_empty());
    assert_eq!(osc.reactions().len(), 1);
    assert!(!body_text(&ir, "Osc").contains("analog"));
}
