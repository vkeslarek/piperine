//! Scope: structure of `piperine_lang::elab` for msl-digital (DIG-09): discrete carriers as the
//! carriers of their body, the design's nets and the carrier bindings of each placement, drives,
//! events and heads, reactions and commits, child params over the parent's params, and the
//! invariants of elaboration over the buck. Expected values are read off the `.msl` and
//! `docs/spec/` (part II §5, part IV §12 and §13), never off the elaborator.



use piperine_ir::{Body, CarrierRole, Constant, Ir, ValueType};
use piperine_lang_fixtures::buck::BuckSource;
use piperine_lang_fixtures::elaboration::SourceElaboration;

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
        self.body(body).carriers().iter().map(|c| (c.name.clone(), c.role.clone(), c.ty.clone(), c.init)).collect()
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

/// Associated test helpers of `digital`, grouped on one fixture type.
struct DigitalFixture;

impl DigitalFixture {
    /// The declaration of the buck module `name` of `file`: its ports and its body items as written (a port that no
    /// body item drives would need an initial value of its own, §3.3.4).
    fn ports_of(file: &str, name: &str) -> String {
        BuckSource::item(file, &format!("pub module {name}("))
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

    /// The buck's `Controller` (examples/buck/src/top.msl) as written, over the buck's own constants,
    /// enums and `target_codes`, its children declared with the buck's ports and no body.
    fn controller() -> String {
        let children = [("control.msl", "SetpointReg"), ("control.msl", "RefRamp"), ("control.msl", "Pid"), ("supervisor.msl", "Supervisor"), ("dpwm.msl", "Dpwm")].map(|(file, name)| DigitalFixture::ports_of(file, name));
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            BuckSource::spec(),
            BuckSource::item("supervisor.msl", "pub enum Mode"),
            BuckSource::item("supervisor.msl", "pub enum Fault"),
            BuckSource::item("control.msl", "pub fn target_codes"),
            children.join("\n"),
            BuckSource::item("top.msl", "pub module Controller("),
        )
    }

    /// A known or unknown `bool`: `None` is unknown (§3.3).
    /// The value of a lowered event guard over the previous and the present value of the carrier
    /// `c` it reads. Any other form is a failure of the test, never a guess.
    fn guard_value(ir: &Ir, e: piperine_ir::ExprId, c: u32, prev: bool, cur: bool) -> bool {
        use piperine_ir::{BinaryOp, Expr, UnaryOp};
        let value = |e| DigitalFixture::guard_value(ir, e, c, prev, cur);
        match ir.expr(e) {
            Expr::Bool(b) => *b,
            Expr::Carrier(k) if *k == c => cur,
            Expr::Previous(k) if *k == c => prev,
            Expr::Unary { op: UnaryOp::Not, expr } => !value(*expr),
            Expr::Binary { op: BinaryOp::And, lhs, rhs } => value(*lhs) && value(*rhs),
            Expr::Binary { op: BinaryOp::Or, lhs, rhs } => value(*lhs) || value(*rhs),
            other => panic!("a form an edge guard does not hold: {other:?}"),
        }
    }

    /// A module `M` with a node, two carriers in and one out, placed by `Top`, and `digital` as its
    /// `digital` block.
    fn edges(digital: &str) -> String {
        format!("module M(a: Electrical, in c: Bit, in d: Bit<u8>, out q: Bit = false) {{ }}\ndigital for M {{ {digital} }}\nmodule Top() {{ signal n: Electrical; signal c: Bit = false; signal d: Bit<u8> = 0; signal q: Bit; let m = M(n, c, d, q); }}")
    }

    /// The text of a golden print of `crates/piperine-ir/tests/golden/buck/` (`ir-rust` T29): the
    /// slice transcribed by hand from the buck's `.msl`, without the flag the explicit model removed.
    fn golden(name: &str) -> String {
        BuckSource::golden(name)
    }

    /// The lines of the reaction `R<k>` of a body print: its header, its locals and its statements.
    fn reaction_text(body: &str, k: usize) -> String {
        let head = format!("  reaction R{k} on");
        let start = body.find(&head).unwrap_or_else(|| panic!("no `{head}` in:\n{body}"));
        let rest = &body[start..];
        let end = rest[1..].find("\n  reaction ").or_else(|| rest[1..].find("\nbody ")).map_or(rest.len(), |at| at + 2);
        rest[..end].trim_end().to_string()
    }

    /// `examples/buck` as a one-crate graph over the embedded standard library, as
    /// `tests/types_buck.rs` loads it.
    fn buck_design() -> piperine_lang::pom::Design {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck");
        let mut graph = piperine_lang::pom::CrateGraph::new();
        graph.add_crate("buck", dir.join("src/lib.msl")).expect("a valid crate name");
        piperine_lang::pom::Design::load_crates(graph)
    }

    /// Every design and top of the invariants: the buck's tops over one loaded package, then the
    /// stdlib's.
    fn invariant_cases() -> Vec<(String, piperine_lang::pom::Design, String)> {
        let mut cases: Vec<(String, piperine_lang::pom::Design, String)> = BUCK_TOPS.iter().map(|t| (format!("buck {t}"), DigitalFixture::buck_design(), t.to_string())).collect();
        cases.extend(STD_TOPS.iter().map(|(name, src)| (format!("std {name}"), SourceElaboration::design(src), "Top".to_string())));
        cases
    }
}

#[test]
fn carriers_follow_ports_then_signals() {
    // DIG-09 AC 1, C75: `RefRamp` (examples/buck/src/control.msl) as written, its ports in
    // declaration order, `reference` with its initial value 0.
    let ir = SourceElaboration::ir(&DigitalFixture::ports_of("control.msl", "RefRamp"), "RefRamp");
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
    let ir = SourceElaboration::ir(&BuckSource::item("control.msl", "pub module Pid("), "Pid");
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
    let src = format!("{}\nmodule Top() {{ signal a: Electrical; signal s: Bit = false; signal c: Bit<u10>; signal d: Bit; let adc = Adc(a, s, c, d); }}", DigitalFixture::ports_of("sense.msl", "Adc"));
    let ir = SourceElaboration::ir(&src, "Top");
    let read = Read { ir: &ir };
    let adc = read.body("Adc");
    assert_eq!(adc.nodes().iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["ain"]);
    assert_eq!(adc.carriers().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["sample", "code", "done"]);
    assert_eq!(read.carriers("Adc")[1].3, Some(Constant::Int(0)), "`out code: Bit<u10> = 0`");
    assert_eq!(read.carriers("Adc")[2].3, Some(Constant::Bool(false)), "`out done: Bit = false`");
    // An initial value is folded (§5.2.3): over `const`s and operators, an enum variant as a composite
    // constant of the variant (the enum is an IR enum since composite-values T48, `Mode::Hold` is variant 2).
    let src = "const K: u8 = 2 + 3;\n#[derive(Repr, PartialEq)]\nenum Mode { Off, On, Hold }\nmodule Top() { signal k: Bit<u8> = K * 2; signal m: Bit<Mode> = Mode::Hold; signal x: Bit<f64> = 0.5 * 3.0; }";
    let ir = SourceElaboration::ir(src, "Top");
    let read = Read { ir: &ir };
    let carriers = read.carriers("Top");
    assert_eq!(carriers.iter().map(|c| c.2.clone()).collect::<Vec<_>>(), [ValueType::UInt(8), ValueType::Enum(piperine_ir::TypeId(0)), ValueType::F64]);
    assert_eq!(carriers[0].3, Some(Constant::Int(10)));
    assert!(matches!(carriers[1].3, Some(Constant::Aggregate(id)) if ir.aggregate(id).variant == Some(2)), "`Mode::Hold` is the variant 2: {:?}", carriers[1].3);
    assert_eq!(carriers[2].3, Some(Constant::Real(piperine_ir::Real::new(1.5))));
    // One that reads a param is folded per body variant (AS-15 AC 1; it was `P0001` before `analog-structure`
    // T34): the param is structural and the variant `Top~r=1.0` holds the value of `r`.
    let ir = SourceElaboration::ir("module Top() { r: f64 = 1.0; signal x: Bit<f64> = r; }", "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.carriers("Top~r=1.0").iter().map(|c| c.3).collect::<Vec<_>>(), [Some(Constant::Real(piperine_ir::Real::new(1.0)))], "x starts at the value of r");
}

#[test]
fn every_signal_carrier_is_a_design_net() {
    // DIG-09 AC 4, C78: one net per signal carrier of each instance, named by its path; a carrier
    // port is no net of its own, it is bound to the parent's net it is connected to.
    let src = "module Leaf(in i: Bit, out o: Bit<u8> = 0) { signal own: Bit = true; }\n\
               module Top() { signal a: Bit = false; signal b1: Bit<u8>; signal b2: Bit<u8>; let x = Leaf(a, b1); let y = Leaf(a, b2); }";
    let ir = SourceElaboration::ir(src, "Top");
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
    let src = "module Leaf(in i: Bit) { }\nmodule Mid(in m: Bit) { let l = Leaf(m); }\nmodule Top() { signal a: Bit = false; let k = Mid(a); }";
    let ir = SourceElaboration::ir(src, "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.net_paths(), ["Top.a"]);
    assert_eq!(read.nets_of("Top.k.l"), ["Top.a"]);
}

#[test]
fn a_top_with_discrete_ports_gets_one_net_per_port() {
    // DIG-09 AC 5, C79: the host drives a top-level port (§5.2.3); each is a net `<Top>.<port>`.
    let ir = SourceElaboration::ir("module Top(in en: Bit, out pgood: Bit = false) { signal s: Bit = false; }", "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.net_paths(), ["Top.en", "Top.pgood", "Top.s"]);
    assert_eq!(read.nets_of("Top"), ["Top.en", "Top.pgood", "Top.s"]);
    assert!(ir.nets().all(|n| n.declared_by().path() == "Top"));
    // A value input on the top has no host source: `E0811` at its name (§23.1 rule 1).
    let src = "module Top(in en: Bit, p: f64) { }";
    let err = SourceElaboration::run(src, "Top").expect_err("a value input on the top");
    assert_eq!(SourceElaboration::codes(&err), ["E0811"]);
    assert!(err[0].diagnostic.message.contains("`p`"), "{}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "p");
}

#[test]
fn a_carrier_with_no_value_and_no_drive_is_kept() {
    // Edge case 2, C164: `signal en: Bit = false;` with no drive is allowed and holds its initial value in every
    // analysis (§5.2.8): one carrier and one net, both with that value.
    let ir = SourceElaboration::ir("module Top() { signal en: Bit = false; }", "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.carriers("Top"), vec![("en".to_string(), CarrierRole::Signal, ValueType::Bool, Some(Constant::Bool(false)))]);
    assert!(read.body("Top").drives().is_empty());
    let net = ir.net_at("Top.en").expect("the net of `en`");
    assert_eq!(net.init(), Some(Constant::Bool(false)));
}

#[test]
fn a_placement_records_its_carrier_bindings() {
    // C171: the authored `Child` shows what a placement connects at each carrier port of the
    // child's body, in port order: the parent's carriers (nodes stay terminals).
    let src = "module Leaf(p: Electrical, in i: Bit, out o: Bit<u8> = 0) { }\nmodule Plain(p: Electrical) { }\n\
               module Top() { signal n: Electrical; signal a: Bit = false; signal b: Bit<u8>; let x = Leaf(n, a, b); let q = Plain(n); }";
    let ir = SourceElaboration::ir(src, "Top");
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
    // it; a carrier handed to a child's `out` port has none. A child's `out` port with an initial value does not give the parent's
    // net one: the child's own carrier holds it (§5.2.6).
    let src = "module Leaf(out o: Bit = true) { }\n\
               module Top(out q: Bit<u4> = 3) { signal x: Bit = false; signal y: Bit<u8> = 0; signal s: Bit; let l = Leaf(s); }";
    let ir = SourceElaboration::ir(src, "Top");
    let init = |path: &str| ir.net_at(path).unwrap_or_else(|| panic!("no net `{path}`")).init();
    assert_eq!(init("Top.q"), Some(Constant::Int(3)));
    assert_eq!(init("Top.x"), Some(Constant::Bool(false)));
    assert_eq!(init("Top.y"), Some(Constant::Int(0)));
    assert_eq!(init("Top.s"), None);
    let read = Read { ir: &ir };
    assert_eq!(read.carriers("Leaf")[0].3, Some(Constant::Bool(true)), "the driver's carrier keeps its own initial value");
}

#[test]
fn a_definition_is_a_drive() {
    // DIG-09 AC 2, C76: `GateDriver` (examples/buck/src/power.msl): `hs_gate = (hs_cmd & !ls_cmd)
    // after tpd;` is one drive of `hs_gate` with its `after`, and so is `ls_gate`, in source order.
    let ir = SourceElaboration::ir(&BuckSource::item("power.msl", "pub module GateDriver("), "GateDriver");
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
    let text = DigitalFixture::body_text(&ir, "GateDriver");
    assert!(text.contains("  drive hs_gate = (hs_cmd & (!ls_cmd)) after tpd\n  drive ls_gate = (ls_cmd & (!hs_cmd)) after tpd\n"), "{text}");
    // An undelayed definition has no `after`: `RefRamp`'s `at_target = reference == target;`, a
    // comparison of two carrier reads.
    let ir = SourceElaboration::ir(&BuckSource::item("control.msl", "pub module RefRamp("), "RefRamp");
    let read = Read { ir: &ir };
    let body = read.body("RefRamp");
    assert_eq!(body.drives().len(), 1);
    assert_eq!(body.drives()[0].after, None);
    assert_eq!(ir.expr_type(body.drives()[0].value), &ValueType::Bool);
    assert!(DigitalFixture::body_text(&ir, "RefRamp").contains("  drive at_target = (reference == target)\n"));
}

#[test]
fn an_expression_at_an_in_port_is_a_carrier_and_a_drive() {
    // DIG-09 AC 3, C77: `RefRamp(done, run, target_codes(setpoint), ..)` in
    // `Controller` adds the signal carrier `ramp.target` to `Controller` with its drive, and binds
    // the child's port `target` to it (§8.2.9).
    let ir = SourceElaboration::ir(&DigitalFixture::controller(), "Controller");
    let read = Read { ir: &ir };
    let body = read.body("Controller");
    let at = body.carriers().iter().position(|c| c.name == "ramp.target").expect("the carrier `ramp.target`");
    let carrier = &body.carriers()[at];
    assert_eq!((carrier.role.clone(), &carrier.ty, carrier.init), (CarrierRole::Signal, &ValueType::UInt(10), None), "a signal of `target`'s type, its value is the drive's");
    let drives: Vec<_> = body.drives().iter().filter(|d| d.carrier == at as u32).collect();
    assert_eq!(drives.len(), 1, "one drive of `ramp.target`");
    assert_eq!(drives[0].after, None);
    assert!(DigitalFixture::body_text(&ir, "Controller").contains("  drive ramp.target = target_codes(setpoint)\n"), "{}", DigitalFixture::body_text(&ir, "Controller"));
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

#[test]
fn changed_rise_fall_and_cross_lower_to_events() {
    // DIG-09 AC 6, C80. The stdlib's own text (§22.1) is what `rise` and `fall` mean.
    let values = std::fs::read_to_string(format!("{}/std/values.msl", env!("CARGO_MANIFEST_DIR"))).expect("std/values.msl");
    assert!(values.contains("self.changed() && !self.prev() && self"), "the stdlib `rise` is the oracle");
    assert!(values.contains("self.changed() && self.prev() && !self"), "the stdlib `fall` is the oracle");
    let src = DigitalFixture::edges("on c.changed() { q <- true; }\non c.rise() { q <- true; }\non c.fall() { q <- false; }\non cross(v(a) - 0.5) { q <- true; }\non d.changed() && d.prev() == 3 { q <- true; }");
    let ir = SourceElaboration::ir(&src, "Top");
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
    assert_eq!(ir.expr_type(rise), &ValueType::Bool, "the guard is never unknown");
    assert_eq!(ir.expr_type(fall), &ValueType::Bool);
    let text = ir.print_structural();
    assert!(text.contains("guard ((!c.prev()) && c)"), "{text}");
    assert!(text.contains("guard (c.prev() && (!c))"), "{text}");
    // §22.1 (D9): rise on false to true, fall on true to false; neither on any other change.
    let (lo, hi) = (false, true);
    for (prev, cur, rises, falls) in [(lo, hi, true, false), (hi, lo, false, true), (lo, lo, false, false), (hi, hi, false, false)] {
        assert_eq!(DigitalFixture::guard_value(&ir, rise, c, prev, cur), rises, "rise on {prev:?} -> {cur:?}");
        assert_eq!(DigitalFixture::guard_value(&ir, fall, c, prev, cur), falls, "fall on {prev:?} -> {cur:?}");
    }
    // `cross(e)` watches `e`.
    let EventSource::Cross { e: watched, .. } = events[3].source else { panic!("a cross: {:?}", events[3].source) };
    assert!(matches!(ir.expr(watched), Expr::Binary { .. }) && text.contains("cross (probe(a, gnd) - 0.5)"), "{text}");
    // `c.prev()` in a guard and in a reaction body is the previous-value read.
    assert_eq!(events[4].source, EventSource::Changed(d));
    assert!(text.contains("guard (d.prev() == 3u8)"), "{text}");
}

#[test]
fn a_guard_splits_into_its_discrete_and_continuous_parts() {
    // DIG-09 AC 7, C81: `on x.changed() && en && analysis(Static)` keeps `en` on
    // the event and wraps the body in `if static` (abi.md F2: a probing guard is no discrete
    // function).
    let src = "module M(a: Electrical, in x: Bit, in en: Bit, out q: Bit = false) { }\n\
               digital for M {\n\
                 on x.changed() && en && analysis(Static) { q <- true; }\n\
                 on x.changed() && v(a) > 0.5 && en { q <- false; }\n\
                 on x.changed() && en { q <- true; }\n\
               }\n\
               module Top() { signal n: Electrical; signal x: Bit = false; signal en: Bit = false; signal q: Bit; let m = M(n, x, en, q); }";
    let ir = SourceElaboration::ir(src, "Top");
    let read = Read { ir: &ir };
    let m = read.body("M");
    let en = m.carriers().iter().position(|k| k.name == "en").expect("en") as u32;
    use piperine_ir::{Env, Expr, Stmt};
    let guard = m.events()[0].guard.expect("the discrete part stays on the event");
    assert_eq!(ir.expr(guard), &Expr::Carrier(en), "`en` alone");
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
    assert_eq!(ir.expr(m.events()[1].guard.expect("en")), &Expr::Carrier(en), "`en` on the event");
    // A guard that is a carrier is kept as written (ir-rust RS-11 AC 5).
    let carrier_guard = m.events()[2].guard.expect("a guard");
    assert_eq!(ir.expr(carrier_guard), &Expr::Carrier(en));
    assert_eq!(ir.expr_type(carrier_guard), &ValueType::Bool);
    assert!(!matches!(ir.stmt(m.reactions()[2].stmts[0]), Stmt::Expr(_)), "no `if` without a continuous conjunct");
}

#[test]
fn an_or_head_is_one_reaction_or_one_per_disjunct() {
    // DIG-09 AC 8, C82.
    let src = "module M(a: Electrical, in x: Bit, in y: Bit, out q: Bit = false) { }\n\
               digital for M {\n\
                 on x.rise() || y.rise() { q <- true; }\n\
                 on cross(v(a)) || (x.changed() && v(a) > 0.0) { q <- false; }\n\
               }\n\
               module Top() { signal n: Electrical; signal x: Bit = false; signal y: Bit = false; signal q: Bit; let m = M(n, x, y, q); }";
    let ir = SourceElaboration::ir(src, "Top");
    let read = Read { ir: &ir };
    let m = read.body("M");
    let (x, y) = (m.carriers().iter().position(|k| k.name == "x").expect("x") as u32, m.carriers().iter().position(|k| k.name == "y").expect("y") as u32);
    use piperine_ir::{EventSource, Stmt};
    // Discrete disjuncts: one reaction listing each event.
    assert_eq!(m.reactions()[0].on, [0, 1]);
    assert_eq!((&m.events()[0].source, &m.events()[1].source), (&EventSource::Changed(x), &EventSource::Changed(y)));
    // A head with a disjunct whose guard probes is one reaction on every disjunct's event, run
    // once per microstep (abi.md R1), its body under `(fired(e₂) && ..) || (fired(e₃) && v(a) > 0.0)`.
    assert_eq!(m.reactions().len(), 2, "one reaction for the first head, one for the second");
    assert_eq!(m.reactions()[1].on, [2, 3]);
    assert!(matches!(m.events()[2].source, EventSource::Cross { .. }));
    assert_eq!(m.events()[3].source, EventSource::Changed(x));
    assert!(matches!(ir.stmt(m.reactions()[1].stmts[0]), Stmt::Expr(_)), "the body runs under the `fired` guard");
    let text = ir.print_structural();
    assert!(text.contains("fired(E2)") && text.contains("fired(E3)"), "{text}");
}

#[test]
fn a_split_head_over_an_emitting_body_is_one_reaction() {
    // FIX-13 AC 3 (§13): two disjuncts over a body that emits a record are one reaction on both
    // events: it runs once per microstep, so it writes one record when both fire.
    let src = "module M(a: Electrical, in x: Bit, out q: Bit = false) { }\n\
               digital for M { on cross(v(a)) || (x.changed() && v(a) > 0.0) { emit!(\"log\", n = 1.0); } }\n\
               module Top() { signal n: Electrical; signal x: Bit = false; signal q: Bit; let m = M(n, x, q); }";
    let ir = SourceElaboration::ir(src, "Top");
    let m = Read { ir: &ir }.body("M");
    assert_eq!(m.reactions().len(), 1, "one reaction for the whole head");
    assert_eq!(m.reactions()[0].on, [0, 1], "on both events");
    let text = ir.print_structural();
    assert!(text.contains("fired(E0)") && text.contains("fired(E1)"), "{text}");
}

#[test]
fn settle_lowers_to_the_settle_event_source() {
    // §13 rules 10, 14, 17 (unknown-semantics-events UE-03): `settle` is `EventSource::Settle`,
    // alone, guarded (`settle && g`) and in a disjunction (`settle || ev`, one reaction on both).
    use piperine_ir::EventSource;
    let src = "module M(in x: Bit, in en: Bit, out q: Bit = false) { }\ndigital for M {\n on settle { q <- true; }\n on settle && en { q <- false; }\n on settle || x.changed() { q <- true; }\n}";
    let ir = SourceElaboration::ir(src, "M");
    let read = Read { ir: &ir };
    let m = read.body("M");
    let events = m.events();
    assert_eq!(events.len(), 4, "one event per disjunct: {:?}", events.iter().map(|e| &e.source).collect::<Vec<_>>());
    assert_eq!((&events[0].source, events[0].guard), (&EventSource::Settle, None), "`settle` alone is unguarded");
    assert_eq!(events[1].source, EventSource::Settle);
    assert!(events[1].guard.is_some(), "`settle && en` keeps its guard on the event");
    assert_eq!(events[2].source, EventSource::Settle, "a disjunction adds one event per disjunct, in source order");
    assert!(matches!(events[3].source, EventSource::Changed(_)));
    let heads: Vec<Vec<u32>> = m.reactions().iter().map(|r| r.on.clone()).collect();
    assert_eq!(heads, [vec![0], vec![1], vec![2, 3]], "the reaction of a disjunction listens to both events");
}

#[test]
fn an_event_read_as_a_bool_in_a_body_lowers_to_fired() {
    // §13 rule 2 (UE-01 AC 1, design D7): an event read as a `bool` in a reaction body is
    // `fired(k)`. `x.changed()` is the head's own event 0, reused; `x.rise()` is a new event 1
    // appended to the body's events and to no reaction's `on`. It replaces
    // `a_head_the_ir_cannot_hold_is_p0001`, which pinned the `P0001` this read used to meet.
    use piperine_ir::{EventSource, Expr, Stmt};
    let src = "module M(in x: Bit, out q: Bit = false) { }\ndigital for M { on x.changed() { if x.rise() { q <- true; } if x.changed() { q <- false; } } }";
    let ir = SourceElaboration::ir(src, "M");
    let read = Read { ir: &ir };
    let m = read.body("M");
    let events = m.events();
    assert_eq!(events.len(), 2, "the head's event and the one `rise` appended: {:?}", events.iter().map(|e| &e.source).collect::<Vec<_>>());
    assert!(matches!(events[0].source, EventSource::Changed(_)) && events[0].guard.is_none(), "the head's `changed`");
    assert!(matches!(events[1].source, EventSource::Changed(_)) && events[1].guard.is_some(), "`rise` is `changed` with its edge guard");
    assert_eq!(m.reactions()[0].on, [0], "the appended event is in no head");
    let fired: Vec<u32> = m.reactions()[0]
        .stmts
        .iter()
        .filter_map(|s| match ir.stmt(*s) {
            Stmt::Expr(e) => match ir.expr(*e) {
                Expr::If { cond, .. } => match ir.expr(*cond) {
                    Expr::Fired(k) => Some(*k),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(fired, [1, 0], "`if x.rise()` reads event 1, `if x.changed()` reuses the head's event 0");
}

#[test]
fn a_reaction_body_lowers_in_source_order() {
    // DIG-09 AC 10, C84: `Adc`'s reaction (examples/buck/src/sense.msl) is one `let` holding a
    // probe of `ain`, then three commits each with its `after`, in source order.
    let src = format!("{}\nmodule Top() {{ signal a: Electrical; signal s: Bit = false; signal c: Bit<u10>; signal d: Bit; let adc = Adc(a, s, c, d); }}", BuckSource::module_with_digital("sense.msl", "Adc"));
    let ir = SourceElaboration::ir(&src, "Top");
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
    assert!(DigitalFixture::reaction_text(&DigitalFixture::body_text(&ir, "Adc"), 0).ends_with("    code <- q after tconv\n    done <- true after tconv\n    done <- false after (tconv + tstrobe)"));

    // `Pid` (control.msl), statement by statement: the hand transcription of `ir-rust` T29 with
    // its one listed difference, `DUTY_MAX as i32` folded to `2880i32` (DIG-08 AC 2).
    let src = format!("{}\n{}", BuckSource::spec(), BuckSource::module_with_digital("control.msl", "Pid"));
    let ir = SourceElaboration::ir(&src, "Pid");
    // `Pid` reads `run`, `reference` and `meas` directly (explicit-unknowns T24, T31): the fixture, the earlier
    // transcription, is read without its flag (`golden`) and joins `windup` with `&&` and `||`
    let expected = DigitalFixture::reaction_text(&DigitalFixture::golden("pid"), 0)
        .replace("(2880u12 as i32)", "2880i32")
        .replace("(((u > u_max) & (e > 0i16)) | ((u < 0i32) & (e < 0i16)))", "(((u > u_max) && (e > 0i16)) || ((u < 0i32) && (e < 0i16)))");
    assert_eq!(DigitalFixture::reaction_text(&DigitalFixture::body_text(&ir, "Pid"), 0), expected);

    // `Dpwm`'s reaction (dpwm.msl), statement by statement against the same transcription; the
    // `Never` monitor beside it is a placement, not part of the reaction (its `require` is
    // inlined from T34 on), so it is left out here.
    let text = BuckSource::module_with_digital("dpwm.msl", "Dpwm");
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|l| l.contains("let no_shoot_through")).expect("the `Never` placement of `Dpwm`");
    // The placement and the doc comment above it.
    let dpwm: String = lines.iter().enumerate().filter(|(k, _)| *k != at && !(*k + 1 == at && lines[*k].trim_start().starts_with("///"))).map(|(_, l)| *l).collect::<Vec<_>>().join("\n");
    let src = format!("{}\n{}\n{}\n{dpwm}", BuckSource::spec(), BuckSource::item("dpwm.msl", "fn rev3"), BuckSource::item("dpwm.msl", "fn dithered_width"));
    let ir = SourceElaboration::ir(&src, "Dpwm");
    // `Dpwm` reads `run`, `duty`, `oc` and `sink_oc` directly (explicit-unknowns T24, T31): the fixture is read without
    // its flag, and its `&` and `|` over values that may be unknown are `&&` and `||` over plain ones
    let expected = DigitalFixture::reaction_text(&DigitalFixture::golden("dpwm"), 0)
        .replace("\"dithered_width~01\"(duty, (frame + 1u3))", "dithered_width(duty, (frame + 1u3))")
        .replace("((!wrap) & (hs_cut | (hs & oc)))", "((!wrap) && (hs_cut || (hs && oc)))")
        .replace("((!wrap) & (ls_cut | (ls & sink_oc)))", "((!wrap) && (ls_cut || (ls && sink_oc)))")
        .replace("((enabled & (!hs_trip)) & (next < w))", "((enabled && (!hs_trip)) && (next < w))")
        .replace("(hs & (!hs_next))", "(hs && (!hs_next))")
        .replace("((((enabled & (!hs_next)) & (!ls_trip)) & (next >= (off_at + 4u9))) & (next < 396u9))", "((((enabled && (!hs_next)) && (!ls_trip)) && (next >= (off_at + 4u9))) && (next < 396u9))");
    assert_eq!(DigitalFixture::reaction_text(&DigitalFixture::body_text(&ir, "Dpwm"), 0), expected);
}

#[test]
fn a_module_with_only_a_digital_block_has_no_analog_statements() {
    // Edge case 1, C163: `Osc` (dpwm.msl) has a `digital` block and no `analog` block.
    let ir = SourceElaboration::ir(&BuckSource::module_with_digital("dpwm.msl", "Osc"), "Osc");
    let read = Read { ir: &ir };
    let osc = read.body("Osc");
    assert!(osc.analog().is_none(), "no analog statements");
    assert!(osc.branches().is_empty() && osc.nodes().is_empty());
    assert_eq!(osc.reactions().len(), 1);
    assert!(!DigitalFixture::body_text(&ir, "Osc").contains("analog"));
}

#[test]
fn a_child_param_reads_the_parents_value() {
    // DIG-09 AC 11, C85: `Divider` (examples/buck/src/sense.msl) places `Resistor { r: r_top }`;
    // two `Divider`s with different values give their `Resistor` instances those values, the
    // constant of each instance in the flat table.
    let src = format!(
        "{}\n{}\nmodule Top() {{ signal a: Electrical; signal b: Electrical; signal c: Electrical; let d1 = Divider(a, b) {{ r_top: 30k, r_bot: 10k }}; let d2 = Divider(a, c) {{ r_top: 90k, r_bot: 20k }}; }}",
        BuckSource::item("power.msl", "pub module Resistor("),
        BuckSource::item("sense.msl", "pub module Divider(")
    );
    let ir = SourceElaboration::ir(&src, "Top");
    let r = |path: &str| ir.instance_at(path).unwrap_or_else(|| panic!("no `{path}`")).param(0).copied();
    use piperine_ir::{ChildValue, Expr, Real};
    assert_eq!(r("Top.d1.upper"), Some(Constant::Real(Real::new(30e3))));
    assert_eq!(r("Top.d1.lower"), Some(Constant::Real(Real::new(10e3))));
    assert_eq!(r("Top.d2.upper"), Some(Constant::Real(Real::new(90e3))));
    assert_eq!(r("Top.d2.lower"), Some(Constant::Real(Real::new(20e3))));
    // The authored child shows the expression (the IR half is C86): one body for `Divider`,
    // whatever values its instances take.
    let read = Read { ir: &ir };
    let divider = read.body("Divider");
    let [(param, ChildValue::Expr(value))] = divider.child_params(0) else { panic!("`r: r_top` as an expression: {:?}", divider.child_params(0)) };
    assert_eq!((*param, ir.expr(*value)), (0, &Expr::Param(0)), "`r` reads `r_top`, the first param of `Divider`");
    assert!(divider.children()[0].params.is_empty(), "no constant for a value that reads the parent");
    // A false `where` of the child is `E0317` at the placement in the parent, naming the instance.
    let src = format!(
        "{}\nmodule Shifted(top: Electrical, mid: Electrical) {{ r_top: f64 where r_top > 0.0; let upper = Resistor(top, mid) {{ r: r_top - 1.0 }}; }}\nmodule Top() {{ signal a: Electrical; signal b: Electrical; let ok = Shifted(a, b) {{ r_top: 2.0 }}; let bad = Shifted(a, b) {{ r_top: 0.5 }}; }}",
        BuckSource::item("power.msl", "pub module Resistor(")
    );
    let err = SourceElaboration::run(&src, "Top").expect_err("`r = -0.5` breaks the `where` of `Resistor`");
    assert_eq!(SourceElaboration::codes(&err), ["E0317"]);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "r_top - 1.0");
    assert!(err[0].diagnostic.message.contains("Top.bad.upper"), "{}", err[0].diagnostic.message);
    // The field-init shorthand of `Buck`'s `PowerStage { l, dcr, .. }` is `{ l: l, dcr: dcr }`.
    let src = "module Ind(p: Electrical, n: Electrical) { l: f64 where l > 0.0; dcr: f64 = 0.0; }\n\
               module Top() { l: f64 = 4.7u; dcr: f64 = 15m; signal a: Electrical; let ind = Ind(a, gnd) { l, dcr }; }";
    let ir = SourceElaboration::ir(src, "Top");
    let ind = ir.instance_at("Top.ind").expect("Top.ind");
    assert_eq!(ind.params(), [Constant::Real(Real::new(4.7e-6)), Constant::Real(Real::new(15e-3))]);
    let read = Read { ir: &ir };
    let top = read.body("Top");
    let given: Vec<(u32, &Expr)> = top.child_params(0).iter().map(|(p, v)| match v {
        ChildValue::Expr(e) => (*p, ir.expr(*e)),
        ChildValue::Const(c) => panic!("a constant {c}"),
    }).collect();
    assert_eq!(given, [(0, &Expr::Param(0)), (1, &Expr::Param(1))]);
}

/// The modules of `examples/buck/src` that may be a top (their ports are discrete), each one top
/// at a time; every other buck module is reached below `Buck`.
const BUCK_TOPS: &[&str] = &["Osc", "SetpointReg", "RefRamp", "Pid", "GateDriver", "Dpwm", "Supervisor", "Controller", "Buck"];

/// The modules of `std/{convert,monitor}.msl` that elaborate, each placed by a `Top` of its own.
/// (`D2A` matches with a binding, `AckWithin` has a `u32` param, `Bidir` a resolved carrier, the
/// other monitors are generic: each still `P0001`.)
const STD_TOPS: &[(&str, &str)] = &[
    ("A2D", "module Top() { signal a: Electrical; signal d: Bit; let x = A2D(a, d); }"),
    ("Never", "module Top() { signal c: Bit = false; signal b: Bit = false; let x = Never(c, b); }"),
];

#[test]
fn elaborating_twice_gives_the_same_print() {
    // DIG-09 AC 12, C87: two elaborations of one design, and of the design loaded twice, print
    // equal, origins included.
    for (case, design, top) in DigitalFixture::invariant_cases() {
        let run = |d: &piperine_lang::pom::Design| piperine_lang::elab::Elaborator::new(d).run(&top).unwrap_or_else(|e| panic!("{case}: {:?}", SourceElaboration::codes(&e)));
        let (first, second) = (run(&design), run(&design));
        assert_eq!(first.print_structural(), second.print_structural(), "{case}");
        assert_eq!(first.to_string(), second.to_string(), "{case}");
    }
    let (a, b) = (DigitalFixture::buck_design(), DigitalFixture::buck_design());
    let run = |d: &piperine_lang::pom::Design| piperine_lang::elab::Elaborator::new(d).run("Buck").unwrap_or_else(|e| panic!("{:?}", SourceElaboration::codes(&e)));
    assert_eq!(run(&a).to_string(), run(&b).to_string(), "the buck loaded twice");
}

#[test]
fn the_design_is_not_written() {
    // DIG-09 AC 13, C88: `Design::dump()` is equal before and after elaboration, the POM's
    // placements of hoisted modules (`let gate = transition(..)`) included.
    for (case, design, top) in DigitalFixture::invariant_cases() {
        let before = design.dump();
        let diagnostics = design.diagnostics().to_vec();
        piperine_lang::elab::Elaborator::new(&design).run(&top).unwrap_or_else(|e| panic!("{case}: {:?}", SourceElaboration::codes(&e)));
        assert_eq!(design.dump(), before, "{case}");
        assert_eq!(design.diagnostics(), diagnostics.as_slice(), "{case}");
    }
}

#[test]
fn every_ir_passes_check() {
    // DIG-09 AC 14, C89: every `Ir` returned has an empty `check()`.
    for (case, design, top) in DigitalFixture::invariant_cases() {
        let ir = piperine_lang::elab::Elaborator::new(&design).run(&top).unwrap_or_else(|e| panic!("{case}: {:?}", SourceElaboration::codes(&e)));
        assert!(ir.check().is_empty(), "{case}: {:?}", ir.check());
    }
}

#[test]
fn the_m1_goldens_are_unchanged() {
    // DIG-07 AC 2, C62: the seven M1 twins of `tests/e2e/twins/` elaborate to the text pinned in
    // `tests/fixtures/elab/*.txt`, which this feature never edits. `diode_rev` came after the
    // other six were pinned: its fixture is its print at the feature's base (`9aa03fe8`, the
    // `ir-rust` merge), added by the fix round (verification F10).
    let twins = [("divider", "Divider"), ("diode_iv", "DiodeIv"), ("diode_series", "DiodeSeries"), ("ac_rc", "AcRc"), ("tran_rc", "TranRc"), ("isrc_r", "IsrcR"), ("diode_rev", "DiodeRev")];
    let fixtures = std::fs::read_dir(format!("{}/tests/fixtures/elab", env!("CARGO_MANIFEST_DIR"))).expect("the fixtures").filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|x| x == "txt") && e.path().file_stem().is_some_and(|s| s != "buck")).count();
    assert_eq!(fixtures, twins.len(), "one fixture per pinned twin (`buck.txt` is the pin of the whole buck, `elab_buck`)");
    let twin = |file: &str| {
        let path: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "e2e", "twins", &format!("{file}.msl")].iter().collect();
        piperine_lang::pom::Design::builder().root_file(path).build()
    };
    for (file, top) in twins {
        let ir = piperine_lang::elab::Elaborator::new(&twin(file)).run(top).unwrap_or_else(|e| panic!("{file}: {:?}", SourceElaboration::codes(&e)));
        let golden = std::fs::read_to_string(format!("{}/tests/fixtures/elab/{file}.txt", env!("CARGO_MANIFEST_DIR"))).expect("the fixture");
        assert_eq!(ir.print_structural(), golden, "{file}");
        assert!(ir.check().is_empty(), "{file}");
    }
}

#[test]
fn cross_tolerances_lower_to_the_event() {
    // FIX-12 AC 1 and 2: `cross(e, 1p)`, `cross(e, etol: 1u)`, `cross(e, 1p, 1u)` and a param
    // tolerance lower onto the event; a tolerance left out stays absent (the solver's default).
    let src = "module M(a: Electrical, out q: Bit = false) { tt: f64 = 2p; }\n\
               digital for M {\n\
                 on cross(v(a) - 0.5) { q <- true; }\n\
                 on cross(v(a) - 0.5, 1p) { q <- true; }\n\
                 on cross(v(a) - 0.5, etol: 1u) { q <- true; }\n\
                 on cross(v(a) - 0.5, 1p, 1u) { q <- true; }\n\
                 on cross(v(a) - 0.5, tt, None) { q <- true; }\n\
               }\n\
               module Top() { signal n: Electrical; signal q: Bit; let m = M(n, q); }";
    let ir = SourceElaboration::ir(src, "Top");
    let m = Read { ir: &ir }.body("M");
    use piperine_ir::{EventSource, Expr, Real};
    let tolerances: Vec<(Option<&Expr>, Option<&Expr>)> = m
        .events()
        .iter()
        .map(|e| match e.source {
            EventSource::Cross { ttol, etol, .. } => (ttol.map(|t| ir.expr(t)), etol.map(|t| ir.expr(t))),
            ref other => panic!("a cross: {other:?}"),
        })
        .collect();
    let (p, u) = (Expr::Real(Real::new(1e-12)), Expr::Real(Real::new(1e-6)));
    assert_eq!(tolerances[0], (None, None), "left out: no tolerance");
    assert_eq!(tolerances[1], (Some(&p), None), "positional ttol");
    assert_eq!(tolerances[2], (None, Some(&u)), "named etol");
    assert_eq!(tolerances[3], (Some(&p), Some(&u)), "both");
    assert!(matches!(tolerances[4], (Some(Expr::Param(_)), None)), "a param, with None: {:?}", tolerances[4]);
}

#[test]
fn a_cross_tolerance_that_is_not_constant_is_p0001() {
    // FIX-12 AC 4: a tolerance that reads a carrier, a probe or a local.
    for tolerance in ["v(a)", "c", "{ let t = 1.0; t }"] {
        let src = format!(
            "module M(a: Electrical, in c: Bit<f64>, out q: Bit = false) {{ }}\n\
             digital for M {{ on cross(v(a) - 0.5, {tolerance}) {{ q <- true; }} }}\n\
             module Top() {{ signal n: Electrical; signal c: Bit<f64> = 0.0; signal q: Bit; let m = M(n, c, q); }}"
        );
        let err = SourceElaboration::run(&src, "Top").expect_err(tolerance);
        assert_eq!(SourceElaboration::codes(&err), ["P0001"], "{tolerance}");
        assert!(err[0].diagnostic.message.contains("a `cross` tolerance that is not a constant or a param"), "{tolerance}: {}", err[0].diagnostic.message);
    }
}

#[test]
fn an_event_function_call_lowers_to_its_body_event() {
    // §13 rule 7, design D14 (UE-04 AC 1, 2): a call of `fn handshake(req, ack) -> Event { req.rise() && ack }` is inlined
    // where it stands, so a reaction headed by it elaborates to the very body of one headed by `req.rise() && ack`; in a
    // module result and a nested function it is the same event, and an `||` of two calls is two events of one reaction
    let function = "fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack }\n";
    let module = "module M(in req: Bit, in ack: Bit, out q: Bit = false) { }\n";
    let by_function = SourceElaboration::ir(&format!("{function}{module}digital for M {{ on handshake(req, ack) {{ q <- true; }} }}"), "M");
    let by_hand = SourceElaboration::ir(&format!("{module}digital for M {{ on req.rise() && ack {{ q <- true; }} }}"), "M");
    assert_eq!(Read { ir: &by_function }.body("M").events().len(), 1, "one event: the body's");
    assert_eq!(by_function.print_structural(), by_hand.print_structural(), "the call is the event its body writes");
    // the function's parameters are the call's arguments, not the names of the function: swapped arguments swap the event
    let swapped = SourceElaboration::ir(&format!("{function}{module}digital for M {{ on handshake(ack, req) {{ q <- true; }} }}"), "M");
    let swapped_by_hand = SourceElaboration::ir(&format!("{module}digital for M {{ on ack.rise() && req {{ q <- true; }} }}"), "M");
    assert_eq!(swapped.print_structural(), swapped_by_hand.print_structural(), "ack rises with req high");
    // a function that calls another: the outer guard applies to the inner event
    let nested = format!("{function}fn gated(req: Bit, ack: Bit, en: Bit) -> Event {{ handshake(req, ack) && en }}\nmodule N(in req: Bit, in ack: Bit, in en: Bit, out q: Bit = false) {{ }}\n");
    let by_nesting = SourceElaboration::ir(&format!("{nested}digital for N {{ on gated(req, ack, en) {{ q <- true; }} }}"), "N");
    let nested_by_hand = SourceElaboration::ir(&format!("module N(in req: Bit, in ack: Bit, in en: Bit, out q: Bit = false) {{ }}\ndigital for N {{ on req.rise() && ack && en {{ q <- true; }} }}"), "N");
    assert_eq!(by_nesting.print_structural(), nested_by_hand.print_structural(), "the nested call is its body's event under the outer guard");
    // a module-body `let`: the same event where it is named
    let named = SourceElaboration::ir(&format!("{function}module L(in req: Bit, in ack: Bit, out q: Bit = false) {{ let ready = handshake(req, ack); }}\ndigital for L {{ on ready {{ q <- true; }} }}"), "L");
    let named_by_hand = SourceElaboration::ir("module L(in req: Bit, in ack: Bit, out q: Bit = false) { }\ndigital for L { on req.rise() && ack { q <- true; } }", "L");
    assert_eq!(named.print_structural(), named_by_hand.print_structural(), "a body let of the call is the call's event");
    // read in a guard and in a body: the event its body writes, read as a `bool` there (§13 rules 2 and 9)
    let guarded = SourceElaboration::ir(&format!("{function}module G(in req: Bit, in ack: Bit, in en: Bit, out q: Bit = false) {{ }}\ndigital for G {{ on en.rise() && handshake(req, ack) {{ q <- true; }} }}"), "G");
    // `en.rise()` heads it and the call's event (`req` rising, guarded by `ack`) is appended for the guard to read as `fired`
    assert_eq!(Read { ir: &guarded }.body("G").events().len(), 2, "a guard that reads the call appends its event");
    let read = SourceElaboration::ir(&format!("{function}module R(in req: Bit, in ack: Bit, out q: Bit = false) {{ }}\ndigital for R {{ on req.rise() || ack.rise() {{ q <- handshake(req, ack); }} }}"), "R");
    // the head's two events and the one the body's read appends (the call's event, guarded by `ack`): `fired` of the third
    assert_eq!(Read { ir: &read }.body("R").events().len(), 3, "a body that reads the call appends its event");
}

/// The reaction of `module M(clk, en, q)` on a head, over functions written before it.
struct Head;

impl Head {
    /// The module the reaction is of.
    const MODULE: &'static str = "module M(in clk: Bit, in en: Bit, out q: Bit = false) { }\n";

    /// The source of `module M` reacting on `head`, with `functions` written before it.
    fn source(functions: &str, head: &str) -> String {
        format!("{functions}{}digital for M {{ on {head} {{ q <- true; }} }}", Head::MODULE)
    }

    /// The elaborated IR of `module M` reacting on `head`, with `functions` written before it.
    fn reaction(functions: &str, head: &str) -> String {
        SourceElaboration::ir(&Head::source(functions, head), "M").print_structural()
    }

    /// The diagnostics the elaboration of `module M` reacting on `head` is refused with.
    fn refusal(functions: &str, head: &str) -> Vec<piperine_lang::pom::DesignDiagnostic> {
        SourceElaboration::run(&Head::source(functions, head), "M").expect_err("the event function is refused")
    }
}

#[test]
fn an_event_function_binds_its_arguments_as_a_call_does() {
    // §3.2.2 to §3.2.4 and §3.2.9 with §13 rule 7 (UE-04 AC 1): `a` and `b` are optional, so `edge(clk, b: Some(false))`
    // leaves `a` out (`unwrap_or(true)`), binds `b`, and fires on every rise; a named argument takes its place whatever
    // its position, and a value written for an `Option` parameter is the `Some` of it
    let function = "fn edge(c: Bit, a: Option<bool>, b: Option<bool>) -> Event { c.rise() && a.unwrap_or(true) && b.unwrap_or(false) }\n";
    // `unwrap_or(d)` of an `Option` is `if flag { payload } else { d }`, a left-out one the flag `false` and a zero payload
    let unwrapped = |given: Option<bool>, default: bool| match given {
        Some(value) => format!("if true {{ {value} }} else {{ {default} }}"),
        None => format!("if false {{ false }} else {{ {default} }}"),
    };
    let by_hand = |a: Option<bool>, b: Option<bool>| Head::reaction("", &format!("clk.rise() && ({}) && ({})", unwrapped(a, true), unwrapped(b, false)));
    assert_eq!(Head::reaction(function, "edge(clk, b: Some(true))"), by_hand(None, Some(true)), "a named argument binds `b`, `a` is left out");
    assert_eq!(Head::reaction(function, "edge(clk)"), by_hand(None, None), "both left out");
    assert_eq!(Head::reaction(function, "edge(clk, Some(false))"), by_hand(Some(false), None), "a positional `Some`");
    assert_eq!(Head::reaction(function, "edge(clk, b: Some(true), a: Some(false))"), by_hand(Some(false), Some(true)), "named arguments after the positional one, in any order");
    assert_ne!(Head::reaction(function, "edge(clk, b: Some(true))"), Head::reaction(function, "edge(clk, a: Some(true))"), "`a` and `b` are not interchanged");
}

#[test]
fn an_event_function_body_may_hold_lets_before_its_event() {
    // §3 rules 6 and 15 with §13: a `let` of the body names a value of the call's arguments and the event reads it
    let function = "fn armed(c: Bit, e: Bit) -> Event { let ready = e; c.rise() && ready }\n";
    assert_eq!(Head::reaction(function, "armed(clk, en)"), Head::reaction("", "clk.rise() && en"));
    assert_eq!(Head::reaction(function, "armed(en, clk)"), Head::reaction("", "en.rise() && clk"), "the let reads the argument of this call");
}

#[test]
fn an_event_function_may_recurse_while_its_decisions_fold() {
    // §3 rule 12: `any(clk, 2)` unrolls to three rise events of one reaction (`n` = 2, 1, 0), the decision `n <= 0` folding
    // in each call; the same recursion without its end is `E0126` naming the function
    let function = "fn any(c: Bit, n: i32) -> Event { if n <= 0 { c.rise() } else { c.rise() || any(c, n - 1) } }\n";
    assert_eq!(Head::reaction(function, "any(clk, 2)"), Head::reaction("", "clk.rise() || clk.rise() || clk.rise()"));
    let endless = format!("fn any(c: Bit) -> Event {{ c.rise() || any(c) }}\nmodule M(in clk: Bit, out q: Bit = false) {{ }}\ndigital for M {{ on any(clk) {{ q <- true; }} }}");
    let refused = SourceElaboration::run(&endless, "M").expect_err("a recursion that never ends");
    assert_eq!(SourceElaboration::codes(&refused), ["E0126"]);
    assert!(refused[0].diagnostic.message.contains("`any`"), "{}", refused[0].diagnostic.message);
}

#[test]
fn an_event_method_runs_the_function_its_impl_gives() {
    // §3.7.8 with §13 rule 7: a trait function that ends in `;` and returns `Event` is the body its `impl` gives for the
    // receiver's type, called as a method, by a path and through a bound
    let functions = "trait Armed { fn fires(self, c: Bit) -> Event; }\nimpl Armed for bool { fn fires(self, c: Bit) -> Event { c.rise() && self } }\nfn through<T: Armed>(a: T, c: Bit) -> Event { a.fires(c) }\n";
    let by_hand = Head::reaction("", "clk.rise() && en");
    assert_eq!(Head::reaction(functions, "en.fires(clk)"), by_hand, "as a method");
    assert_eq!(Head::reaction(functions, "Armed::fires(en, clk)"), by_hand, "by a path");
    assert_eq!(Head::reaction(functions, "through(en, clk)"), by_hand, "through a bound");
}

#[test]
fn a_generic_event_function_runs_for_the_nature_it_is_called_at() {
    // §3.8 with §13 rule 7 (UE-04 AC 1): `crossed<N>` is the `cross` of the potential of its argument, whose nature each
    // call instantiates: the same function is the electrical crossing for an `Electrical` node and the thermal one for a
    // `Thermal` node
    let function = "fn crossed<N: Nature>(a: N) -> Event { cross(potential(a)) }\n";
    let design = |head: &str| {
        let module = "module M(a: Electrical, t: Thermal, out q: Bit = false) { }\n";
        let top = "module Top() { signal n: Electrical; signal h: Thermal; signal q: Bit; let m = M(n, h, q); }";
        SourceElaboration::ir(&format!("{function}{module}digital for M {{ on {head} {{ q <- true; }} }}\n{top}"), "Top").print_structural()
    };
    assert_eq!(design("crossed(a)"), design("cross(v(a))"), "an electrical node");
    assert_eq!(design("crossed(t)"), design("cross(temp(t))"), "a thermal node");
    assert_ne!(design("crossed(a)"), design("crossed(t)"), "two instances of one function");
}

#[test]
fn an_event_function_as_the_result_of_a_module_is_its_event_in_the_module_frame() {
    // §13 (UE-04 AC 1): `module Ready(..) -> Event { handshake(c, e) }` placed in a head is the event of its body, over the
    // module's own ports (`Ready$0.c`, `Ready$0.e`)
    let functions = "fn handshake(c: Bit, e: Bit) -> Event { c.rise() && e }\nmodule Ready(in c: Bit, in e: Bit) -> Event { handshake(c, e) }\n";
    let printed = Head::reaction(functions, "Ready(clk, en)");
    assert!(printed.contains("event E0 changed Ready$0.c guard (((!Ready$0.c.prev()) && Ready$0.c) && Ready$0.e)"), "{printed}");
}

#[test]
fn an_event_function_chooses_between_events_by_a_value_decided_at_the_instant() {
    // §13 rules 1 and 7 with §3 rules 6 and 15: an `if` whose condition reads a carrier chooses between events, each
    // branch's event guarded by the choice, `(c && e1) || (!c && e2)`; a `let` reassigned in a `for` over constant bounds
    // is read by the choice; a `match` over the carrier is one arm per disjunct
    let by_if = "fn pick(c: Bit, s: Bit) -> Event { if s { c.rise() } else { c.fall() } }\n";
    assert_eq!(Head::reaction(by_if, "pick(clk, en)"), Head::reaction("", "(clk.rise() && en) || (clk.fall() && !en)"));
    assert_eq!(Head::reaction(by_if, "pick(clk, en)"), Head::reaction(by_if, "pick(clk, en)"));
    assert_ne!(Head::reaction(by_if, "pick(clk, en)"), Head::reaction(by_if, "pick(en, clk)"), "the choice reads the argument of the call");
    let chain = "fn tier(c: Bit, a: Bit, b: Bit) -> Event { if a { c.rise() } else if b { c.fall() } else { c.changed() } }\n";
    assert_eq!(Head::reaction(chain, "tier(clk, en, en)"), Head::reaction("", "(clk.rise() && en) || ((clk.fall() && en) && !en) || (clk.changed() && !en && !en)"), "an `else if` nests the choice");
    let counted = "fn counted(c: Bit, s: Bit) -> Event { let n = 0; for i in 0..3 { n += 1; } if s && n == 3 { c.rise() } else { c.fall() } }\n";
    let found = SourceElaboration::ir(&Head::source(counted, "counted(clk, en)"), "M");
    assert_eq!(Read { ir: &found }.body("M").events().len(), 2, "a `let` built by a `for` and read by the choice: both branches are events");
    let matched = "fn chosen(c: Bit, s: Bit) -> Event { match s { true => c.rise(), false => c.fall() } }\n";
    let found = SourceElaboration::ir(&Head::source(matched, "chosen(clk, en)"), "M");
    assert_eq!(Read { ir: &found }.body("M").events().len(), 2, "one event per arm of a `match` over a carrier");
}

#[test]
fn an_event_function_with_an_arm_it_cannot_guard_is_refused_by_name() {
    // an arm with a guard of its own, or a name bound by its pattern, has no disjunct of this form (GAPS §4 L76)
    let guarded = "fn g(c: Bit, s: Bit) -> Event { match s { true if c => c.rise(), _ => c.fall() } }\n";
    let refused = Head::refusal(guarded, "g(clk, en)");
    assert_eq!(SourceElaboration::codes(&refused), ["P0001"]);
    assert!(refused[0].diagnostic.message.contains("a `match` arm with a guard in a function that returns an `Event`"), "{}", refused[0].diagnostic.message);
    let binding = "fn b(c: Bit, s: Bit) -> Event { match s { x => c.rise() } }\n";
    let refused = Head::refusal(binding, "b(clk, en)");
    assert_eq!(SourceElaboration::codes(&refused), ["P0001"]);
    assert!(refused[0].diagnostic.message.contains("a `match` arm whose pattern binds a name, in a function that returns an `Event`"), "{}", refused[0].diagnostic.message);
}

#[test]
fn an_event_function_that_emits_or_loops_without_end_is_refused_by_name() {
    // §3 rule 5 with §19 rule 10: a `fn` that calls `emit!` is callable only in a reaction or `analog`, and an event
    // function is called in an event position, so the checker refuses it with `E0122` before elaboration (the spec's table
    // of where a `fn` is callable); a `while` has no finite reading at elaboration
    let emitting = "fn noisy(c: Bit) -> Event { emit!(\"x\", a = 1); c.rise() }\n";
    let refused = Head::refusal(emitting, "noisy(clk)");
    assert_eq!(SourceElaboration::codes(&refused), ["E0122"], "{refused:?}");
    let looping = "fn spin(c: Bit) -> Event { while false { } c.rise() }\n";
    let refused = Head::refusal(looping, "spin(clk)");
    assert_eq!(SourceElaboration::codes(&refused), ["P0001"]);
    assert!(refused[0].diagnostic.message.contains("a `while`, `loop`, `break`, `continue` or `return` with no value in a function that returns an `Event`"), "{}", refused[0].diagnostic.message);
    let option = "fn some(c: Bit, o: Option<bool>) -> Event { if let Some(x) = o { c.rise() } else { c.fall() } }\n";
    let refused = Head::refusal(option, "some(clk)");
    assert_eq!(SourceElaboration::codes(&refused), ["P0001"]);
    assert!(refused[0].diagnostic.message.contains("an `if let` in a function that returns an `Event`"), "{}", refused[0].diagnostic.message);
    let pattern = "fn pair(c: Bit) -> Event { let (a, b) = (1, 2); c.rise() }\n";
    let refused = Head::refusal(pattern, "pair(clk)");
    assert!(refused[0].diagnostic.message.contains("a `let` of a pattern that is no name in a `fn` that returns an `Event`"), "{}", refused[0].diagnostic.message);
}
