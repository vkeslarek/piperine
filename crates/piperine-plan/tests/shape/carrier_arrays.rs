//! The array pin in the plan (composite-values CV-23, CV-25, CV-27; abi.md §3): an array of carriers is one
//! `DISCRETE` pin with a `count` and the layout of one element, whose `init` is one element or every element; a read
//! of a constant element is a cell naming the element, a whole read a `WHOLE` cell, and an element or field commit a
//! statement over the byte range it writes inside one element.

use piperine_abi::raw::{Pin, Shape, WHOLE, cell, kind, part};
use piperine_ir::{
    AggregateData, BodyId, BodySpec, Carrier, CarrierRole, CommitTarget, Constant, Event, EventSource, Expr, InstanceSpec, Ir, IrBuilder, NetId, NetSpec, Origin, Projection,
    Reaction, Stmt, ValueType,
};
use piperine_plan::BodyPlan;

/// The bodies of this target, each beside the MSL it transcribes.
struct Fixture;

impl Fixture {
    /// A pin of `shape` by name.
    fn pin<'a, 's>(shape: &'a Shape<'s>, name: &str) -> &'a Pin<'s> {
        shape.pins.iter().find(|p| p.name.as_str() == name).unwrap_or_else(|| panic!("no pin `{name}`"))
    }

    const FILE: &'static str = "ram.msl";

    fn origin(line: u32) -> Origin {
        Origin { file: Fixture::FILE.into(), line, col: 5 }
    }

    /// The `Ram` body with no carrier yet.
    fn spec() -> BodySpec {
        BodySpec {
            name: "Ram".into(),
            origin: Fixture::origin(1),
            params: vec![],
            nodes: vec![],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        }
    }

    fn carrier(name: &str, ty: ValueType, init: Option<Constant>, count: Option<u64>, line: u32) -> Carrier {
        Carrier { name: name.into(), ty, role: CarrierRole::Signal, init, origin: Fixture::origin(line), count }
    }

    /// The checked design placing `body` once under the top.
    fn design(mut b: IrBuilder, body: BodyId, carriers: &[(&str, ValueType)]) -> Ir {
        let top = b.next_instance_id();
        let nets: Vec<NetId> = carriers
            .iter()
            .map(|(name, ty)| b.net(NetSpec { path: format!("Ram.{name}"), ty: ty.clone(), declared_by: top, init: None, origin: Fixture::origin(2) }))
            .collect();
        let id = b.instance(InstanceSpec { path: "Ram".into(), name: "Ram".into(), body, parent: None, nodes: vec![], params: vec![], origin: Fixture::origin(1) });
        b.bind_nets(id, nets).unwrap();
        let ir = b.finish(id);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        ir
    }

    /// `signal mem: [Bit<u8>; 8] = 0x5a; signal bits: [Bit; 3] = [true, false, true]; signal en: Bit;` with a
    /// reaction that reads `mem[2]`, `mem[1].prev()` and the whole `bits`, and an event on `mem[3].changed()`.
    fn memory() -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let body = b.body(Fixture::spec());
        b.carrier(body, Fixture::carrier("mem", ValueType::UInt(8), Some(Constant::Int(0x5a)), Some(8), 2));
        let every = b.aggregate(AggregateData {
            ty: ValueType::Array(Box::new(ValueType::Bool), 3),
            variant: None,
            parts: vec![Constant::Bool(true), Constant::Bool(false), Constant::Bool(true)],
        });
        b.carrier(body, Fixture::carrier("bits", ValueType::Bool, Some(Constant::Aggregate(every)), Some(3), 3));
        b.carrier(body, Fixture::carrier("en", ValueType::Bool, None, None, 4));
        let go = b.event(body, Event { source: EventSource::Settle, guard: None, origin: Fixture::origin(5) });
        b.event(body, Event { source: EventSource::ElementChanged { carrier: 0, element: 3 }, guard: None, origin: Fixture::origin(6) });
        let two = b.expr(Expr::Int(2), ValueType::UInt(8), Fixture::origin(7));
        let one = b.expr(Expr::Int(1), ValueType::UInt(8), Fixture::origin(7));
        let read = b.expr(Expr::CarrierElement { carrier: 0, index: two }, ValueType::UInt(8), Fixture::origin(7));
        let previous = b.expr(Expr::ElementPrevious { carrier: 0, index: one }, ValueType::UInt(8), Fixture::origin(8));
        let whole = b.expr(Expr::Carrier(1), ValueType::Array(Box::new(ValueType::Bool), 3), Fixture::origin(9));
        let stmts = [read, previous, whole].into_iter().enumerate().map(|(k, init)| b.stmt(Stmt::Let { local: k as u32, init })).collect();
        let locals = vec![
            piperine_ir::Local { name: "a".into(), ty: ValueType::UInt(8), mutable: false },
            piperine_ir::Local { name: "b".into(), ty: ValueType::UInt(8), mutable: false },
            piperine_ir::Local { name: "c".into(), ty: ValueType::Array(Box::new(ValueType::Bool), 3), mutable: false },
        ];
        b.reaction(body, Reaction { on: vec![go], stmts, locals, origin: Fixture::origin(7) });
        (Fixture::design(b, body, &[("mem", ValueType::UInt(8)), ("bits", ValueType::Bool), ("en", ValueType::Bool)]), body)
    }

    /// `signal words: [(Bit, Bit<u16>); 4]; signal pair: (Bit, Bit<u16>, Bit<u8>);` with a reaction committing
    /// `words[2].1 <- 7`, `pair.2 <- 3` and `words[1] <- (false, 9)`.
    fn fields() -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let body = b.body(Fixture::spec());
        let word = ValueType::Tuple(vec![ValueType::Bool, ValueType::UInt(16)]);
        let pair = ValueType::Tuple(vec![ValueType::Bool, ValueType::UInt(16), ValueType::UInt(8)]);
        b.carrier(body, Fixture::carrier("words", word.clone(), None, Some(4), 2));
        b.carrier(body, Fixture::carrier("pair", pair, None, None, 3));
        let go = b.event(body, Event { source: EventSource::Settle, guard: None, origin: Fixture::origin(4) });
        let int = |b: &mut IrBuilder, n: i128, bits: u32| b.expr(Expr::Int(n), ValueType::UInt(bits), Fixture::origin(5));
        let (two, one) = (int(&mut b, 2, 8), int(&mut b, 1, 8));
        let (seven, three, nine) = (int(&mut b, 7, 16), int(&mut b, 3, 8), int(&mut b, 9, 16));
        let off = b.expr(Expr::Bool(false), ValueType::Bool, Fixture::origin(5));
        let value = b.expr(Expr::Tuple(vec![off, nine]), word, Fixture::origin(5));
        let stmts = vec![
            b.stmt(Stmt::Commit { carrier: 0, value: seven, after: None, target: Some(CommitTarget { element: Some(two), path: vec![Projection::Field(1)] }) }),
            b.stmt(Stmt::Commit { carrier: 1, value: three, after: None, target: Some(CommitTarget { element: None, path: vec![Projection::Field(2)] }) }),
            b.stmt(Stmt::Commit { carrier: 0, value, after: None, target: Some(CommitTarget { element: Some(one), path: vec![] }) }),
        ];
        b.reaction(body, Reaction { on: vec![go], stmts, locals: vec![], origin: Fixture::origin(5) });
        let word = ValueType::Tuple(vec![ValueType::Bool, ValueType::UInt(16)]);
        let pair = ValueType::Tuple(vec![ValueType::Bool, ValueType::UInt(16), ValueType::UInt(8)]);
        (Fixture::design(b, body, &[("words", word), ("pair", pair)]), body)
    }
}

#[test]
fn an_array_of_carriers_is_one_array_pin() {
    // CV-23, CV-27 AC 1, abi.md §3: one DISCRETE pin per array, `count` elements of one element's layout, never one
    // pin per element
    let (ir, body) = Fixture::memory();
    let plan = BodyPlan::build(ir.body(body)).expect("the body plans");
    let shape = plan.shape().shape();
    assert_eq!(shape.pins.iter().filter(|p| p.kind == kind::DISCRETE && p.name.as_str() != "en").count(), 2, "two arrays, two pins");
    let mem = Fixture::pin(shape, "mem");
    assert_eq!((mem.kind, mem.count, mem.layout.size), (kind::DISCRETE, 8, 1), "`[Bit<u8>; 8]`: eight elements of one byte");
    // §3.1: `[x; K]` gives one element's bytes
    assert_eq!(mem.init.as_slice(), [0x5a], "the repeat form holds one element");
    let bits = Fixture::pin(shape, "bits");
    assert_eq!((bits.count, bits.layout.size), (3, 1));
    // §3.1: `[a, b, c]` gives every element's bytes
    assert_eq!(bits.init.as_slice(), [1, 0, 1], "the each form holds every element");
    assert_eq!(Fixture::pin(shape, "en").count, 0, "a scalar carrier keeps count 0");

    // abi.md §3: the reaction reads `mem[2]`, `mem[1].prev()` as element cells and `bits` through a WHOLE cell
    let reaction = shape.reactions.iter().find(|r| r.name.as_str() == "reaction:0").expect("the reaction");
    let reads: Vec<(u32, u32, u64)> = reaction
        .reads
        .iter()
        .enumerate()
        .map(|(k, c)| (c.index, c.part, reaction.elements.get(k).copied().unwrap_or(0)))
        .collect();
    let (mem_pin, bits_pin) = (shape.pins.iter().position(|p| p.name.as_str() == "mem").unwrap() as u32, shape.pins.iter().position(|p| p.name.as_str() == "bits").unwrap() as u32);
    assert!(reaction.reads.iter().all(|c| c.kind == cell::PIN));
    assert!(reads.contains(&(mem_pin, part::VALUE, 2)), "mem[2]: {reads:?}");
    assert!(reads.contains(&(mem_pin, part::PREVIOUS, 1)), "mem[1].prev(): {reads:?}");
    assert!(reads.contains(&(bits_pin, part::VALUE, WHOLE)), "the whole array: {reads:?}");

    // CV-24: `on mem[3].changed()` is the event function reading that element's value and CHANGED byte
    let event = shape.functions.iter().find(|f| f.name.as_str() == "event:1").expect("the element event's function");
    let event_reads: Vec<(u32, u32, u64)> = event.reads.iter().enumerate().map(|(k, c)| (c.index, c.part, event.elements.get(k).copied().unwrap_or(0))).collect();
    assert_eq!(event_reads, vec![(mem_pin, part::VALUE, 3), (mem_pin, part::CHANGED, 3)], "element 3's value and changed byte");
}

#[test]
fn a_field_commit_is_a_statement_over_its_levels() {
    // CV-25, abi.md §3: a commit declares the byte range it writes inside one element; `words[2].1 <- 7` writes the
    // `u16` at byte 1 of a 3-byte element, `pair.2 <- 3` the byte at 3 of a scalar pin, and `words[1] <- ..` the whole
    // element
    let (ir, body) = Fixture::fields();
    let plan = BodyPlan::build(ir.body(body)).expect("the body plans");
    let shape = plan.shape().shape();
    let words = shape.pins.iter().position(|p| p.name.as_str() == "words").unwrap() as u32;
    let pair = shape.pins.iter().position(|p| p.name.as_str() == "pair").unwrap() as u32;
    assert_eq!((Fixture::pin(shape, "words").count, Fixture::pin(shape, "words").layout.size), (4, 3));
    let reaction = shape.reactions.iter().find(|r| r.name.as_str() == "reaction:0").expect("the reaction");
    let writes: Vec<(u32, u32, u32)> = reaction.writes.iter().map(|w| (w.pin, w.offset, w.len)).collect();
    assert_eq!(writes, vec![(words, 1, 2), (pair, 3, 1), (words, 0, 3)], "the byte range of each commit");
}
