//! The discrete half of bind (feature `solver-abi-digital`, SAD-05 and
//! SAD-10 AC 14; abi.md §3.1 P1 and P4, §6 C1, §9): one writer per net, equal
//! layouts, an unbound global, `LATER` against the resolution and shape
//! validation. Fixtures are bare shapes: no device stands behind them.

use piperine_abi::layout::{BIT, F64};
use piperine_abi::raw::{
    Cell, Statement, cell, global, part, pin_flag, role, statement_flag,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::plan::{Analysis, Binding, NetWriter, ShapeInput, analyze};
use piperine_digital_solver::tick::Resolution;
use piperine_digital_solver::error::DigitalError;

#[test]
fn two_writers_of_one_net_are_refused_naming_both() {
    // C8, P1: function against function
    let (a, b) = (BindCases::inverter(), BindCases::inverter());
    let r = BindCases::analyze_of(&BindCases::nets(3), &[("u1", &a, vec![BindCases::net(0), BindCases::net(2)]), ("u2", &b, vec![BindCases::net(1), BindCases::net(2)])], Resolution::default());
    let (rule, what) = BindCases::bind_error(r);
    assert_eq!(rule, "P1");
    let text = what.join(" | ");
    assert!(text.contains("u1.y") && text.contains("u2.y"), "both pins named: {text}");

    // function against statement
    let (f, st) = (BindCases::inverter(), BindCases::setter(0, 0.0));
    let mut specs = BindCases::nets(3);
    specs.push(NetSpec::event("trig"));
    let r = BindCases::analyze_of(&specs, &[("inv", &f, vec![BindCases::net(0), BindCases::net(2)]), ("ff", &st, vec![BindCases::net(1), BindCases::net(2), BindCases::net(3)])], Resolution::default());
    let (rule, what) = BindCases::bind_error(r);
    assert_eq!(rule, "P1");
    let text = what.join(" | ");
    assert!(text.contains("inv.y") && text.contains("ff.q"), "both pins named: {text}");
}

#[test]
fn one_instance_writing_a_net_twice_is_one_writer() {
    // two statements of one instance naming one pin are one writer
    let mut s = ShapeBuf::new();
    let q = s.pin(PinSpec::discrete("q", role::PORT, &BIT));
    let t = s.pin(PinSpec::event("t", role::PRIVATE));
    for _ in 0..2 {
        s.reaction(ReactionSpec {
            on: vec![t],
            writes: vec![Statement { pin: q, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
    }
    let s = s.freeze();
    let mut specs = BindCases::nets(1);
    specs.push(NetSpec::event("t"));
    let a = BindCases::analyze_of(&specs, &[("u", &s, vec![BindCases::net(0), BindCases::net(1)])], Resolution::default()).expect("one writer");
    assert_eq!(a.writer(NetId(0)), NetWriter::Statements { instance: 0 });
}

#[test]
fn unequal_layouts_on_one_net_are_refused() {
    // C9, P4: a BIT pin and an F64 pin on one net
    let (a, b) = (BindCases::inverter(), BindCases::reader_of_global(0));
    let r = BindCases::analyze_of(
        &BindCases::nets(3),
        &[("u1", &a, vec![BindCases::net(0), BindCases::net(1)]), ("u2", &b, vec![BindCases::net(1), BindCases::net(2)])],
        Resolution::default(),
    );
    let (rule, what) = BindCases::bind_error(r);
    assert_eq!(rule, "P4");
    let text = what.join(" | ");
    assert!(text.contains("u1.y") && text.contains("u2.temperature"), "both pins named: {text}");
}

#[test]
fn equal_layouts_with_other_names_are_accepted() {
    // P4: names do not count
    let (a, b) = (BindCases::inverter(), BindCases::inverter());
    let r = BindCases::analyze_of(&BindCases::nets(3), &[("u1", &a, vec![BindCases::net(0), BindCases::net(1)]), ("u2", &b, vec![BindCases::net(1), BindCases::net(2)])], Resolution::default());
    assert!(r.is_ok(), "u1.y feeds u2.a: {:?}", r.err());
}

#[test]
fn an_unbound_global_is_refused_unless_optional() {
    let required = BindCases::reader_of_global(0);
    let r = BindCases::analyze_of(&BindCases::nets(1), &[("u", &required, vec![Binding::Absent, BindCases::net(0)])], Resolution::default());
    let (rule, what) = BindCases::bind_error(r);
    assert_eq!(rule, "unbound");
    assert!(what.join(" ").contains("u.temperature"), "the pin is named: {what:?}");

    let optional = BindCases::reader_of_global(pin_flag::OPTIONAL);
    let a = BindCases::analyze_of(&BindCases::nets(1), &[("u", &optional, vec![Binding::Absent, BindCases::net(0)])], Resolution::default())
        .expect("an OPTIONAL pin may be unbound");
    assert_eq!(a.writer(NetId(0)), NetWriter::Function { instance: 0, k: 0 });
    // an absent pin names no net, so the function's dependency list has none of it
    assert_eq!(a.functions()[0].reads, []);
    assert_eq!(a.functions()[0].writes, [NetId(0)]);
}

#[test]
fn later_is_dropped_when_its_bound_is_under_one_tick() {
    // C18: a delay bound of 1 ps
    let s = BindCases::setter(statement_flag::LATER, 1e-12);
    let pins = vec![BindCases::net(0), BindCases::net(1), Binding::Net(NetId(2))];
    let mut specs = BindCases::nets(2);
    specs.push(NetSpec::event("trig"));

    let fs = Resolution { seconds_per_tick: 1e-15 };
    let a = BindCases::analyze_of(&specs, &[("u", &s, pins.clone())], fs).expect("binds");
    assert!(a.later(0, 0, 0), "1 ps is 1000 ticks of 1 fs: the promise holds");

    let ns = Resolution { seconds_per_tick: 1e-9 };
    let a = BindCases::analyze_of(&specs, &[("u", &s, pins.clone())], ns).expect("no error when the flag is lost");
    assert!(!a.later(0, 0, 0), "1 ps is under 1 ns: the flag is dropped");

    // a statement without the flag stays without it
    let plain = BindCases::setter(0, 1e-12);
    let a = BindCases::analyze_of(&specs, &[("u", &plain, pins)], fs).expect("binds");
    assert!(!a.later(0, 0, 0));
}

#[test]
fn a_shape_that_fails_validation_fails_bind() {
    // a function that reads a pin the shape does not have
    let mut s = ShapeBuf::new();
    let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
    s.function(FunctionSpec {
        name: "bad",
        reads: vec![BindCases::pin(7, part::VALUE)],
        writes: vec![BindCases::pin(y, part::VALUE)],
        ..FunctionSpec::default()
    });
    let s = s.freeze();
    let (rule, what) = BindCases::bind_error(BindCases::analyze_of(&BindCases::nets(1), &[("u", &s, vec![BindCases::net(0)])], Resolution::default()));
    assert_eq!(rule, "validate");
    assert!(what.join(" ").contains("bad"), "the function is named: {what:?}");
}

#[test]
fn a_net_nobody_writes_is_written_by_the_host() {
    let a = BindCases::inverter();
    let an = BindCases::analyze_of(&BindCases::nets(2), &[("u", &a, vec![BindCases::net(0), BindCases::net(1)])], Resolution::default()).expect("binds");
    assert_eq!(an.writer(NetId(0)), NetWriter::Host, "u.a is only read");
    assert_eq!(an.writer(NetId(1)), NetWriter::Function { instance: 0, k: 0 });
}

#[test]
fn a_pin_bound_to_a_net_of_the_wrong_size_or_kind_is_refused() {
    let a = BindCases::inverter();
    let mut specs = BindCases::nets(2);
    specs[0] = NetSpec::discrete("wide", 16, 8);
    let (rule, _) = BindCases::bind_error(BindCases::analyze_of(&specs, &[("u", &a, vec![BindCases::net(0), BindCases::net(1)])], Resolution::default()));
    assert_eq!(rule, "P4", "a BIT pin on a 16-byte net");
    let mut specs = BindCases::nets(2);
    specs[1] = NetSpec::event("ev");
    let (rule, _) = BindCases::bind_error(BindCases::analyze_of(&specs, &[("u", &a, vec![BindCases::net(0), BindCases::net(1)])], Resolution::default()));
    assert_eq!(rule, "P4", "a discrete pin on an event net");
}

/// The plans the bind tests build.
pub struct BindCases;

impl BindCases {
    /// The pin helper `pin`.
    fn pin(index: u32, part: u32) -> Cell {
        Cell { kind: cell::PIN, index, part }
    }

    /// pins: a, y (PORT BIT); fn 0 reads a, writes y.
    fn inverter() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
        s.function(FunctionSpec {
            name: "inv",
            reads: vec![BindCases::pin(a, part::VALUE)],
            writes: vec![BindCases::pin(y, part::VALUE)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// pins: d, q (PORT BIT), trig (PRIVATE event); reaction 0 on trig reads d, S0 {q}.
    fn setter(flags: u32, min_after: f64) -> ShapeRef {
        let mut s = ShapeBuf::new();
        let d = s.pin(PinSpec::discrete("d", role::PORT, &BIT));
        let q = s.pin(PinSpec::discrete("q", role::PORT, &BIT));
        let trig = s.pin(PinSpec::event("trig", role::PRIVATE));
        s.reaction(ReactionSpec {
            name: "set",
            on: vec![trig],
            reads: vec![BindCases::pin(d, part::VALUE)],
            writes: vec![Statement { pin: q, offset: 0, len: 1, flags, min_after }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// pins: g (GLOBAL TEMPERATURE F64, flags); fn 0 reads g, writes nothing a net names but y.
    fn reader_of_global(flags: u32) -> ShapeRef {
        let mut s = ShapeBuf::new();
        let g = s.pin(PinSpec {
            global: global::TEMPERATURE,
            flags,
            ..PinSpec::discrete("temperature", role::GLOBAL, &F64)
        });
        let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
        s.function(FunctionSpec {
            name: "f",
            reads: vec![BindCases::pin(g, part::VALUE)],
            writes: vec![BindCases::pin(y, part::VALUE)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The nets helper `nets`.
    fn nets(n: usize) -> Vec<NetSpec> {
        (0..n).map(|i| NetSpec::discrete(&format!("n{i}"), 1, 1)).collect()
    }

    /// The net helper `net`.
    fn net(i: u32) -> Binding {
        Binding::Net(NetId(i))
    }

    /// The analyze of helper `analyze_of`.
    fn analyze_of(specs: &[NetSpec], inputs: &[(&str, &ShapeRef, Vec<Binding>)], r: Resolution) -> Result<Analysis, DigitalError> {
        let inputs: Vec<ShapeInput<'_>> =
            inputs.iter().map(|(path, shape, pins)| ShapeInput { path, shape: shape.shape(), pins }).collect();
        analyze(specs, &inputs, r)
    }

    /// The bind error helper `bind_error`.
    fn bind_error(e: Result<Analysis, DigitalError>) -> (&'static str, Vec<String>) {
        match e {
            Err(DigitalError::Bind { rule, what }) => (rule, what),
            Err(other) => panic!("expected a bind error, got {other:?}"),
            Ok(_) => panic!("expected a bind error, bind succeeded"),
        }
    }
}
