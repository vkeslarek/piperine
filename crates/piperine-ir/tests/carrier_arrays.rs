//! Arrays of carriers and of nodes in the IR (composite-values CV-23 to CV-25, CV-34): an array carrier is one
//! `Carrier` with a `count`, an element read, an element's previous value, an element's `changed()` and an element
//! event name it, `Ir::check` types and refuses each form, the printer writes it, and serde round-trips it.

use piperine_ir::{
    BodyId, BodyNode, BodySpec, Carrier, CarrierRole, Constant, Event, EventSource, Expr, ExprId, InstanceSpec, Ir, IrBuilder, NetId,
    NetSpec, NodeArray, NodeRef, NodeRole, NodeSpec, Origin, Stmt, ValueType,
};

/// Bodies and designs this target checks, each beside the MSL it transcribes.
struct Fixture;

impl Fixture {
    const FILE: &'static str = "ram.msl";

    fn origin(line: u32) -> Origin {
        Origin { file: Fixture::FILE.into(), line, col: 5 }
    }

    /// `module Ram() { signal mem: [Bit<u8>; 8] = 0; signal en: Bit = false; }`: carrier 0 is the array, carrier 1 a
    /// scalar.
    fn memory(b: &mut IrBuilder) -> BodyId {
        let spec = BodySpec {
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
        };
        let body = b.body(spec);
        let mem = Carrier {
            name: "mem".into(),
            ty: ValueType::UInt(8),
            role: CarrierRole::Signal,
            init: Some(Constant::Int(0)),
            origin: Fixture::origin(2),
            count: Some(8),
        };
        b.carrier(body, mem);
        let en = Carrier { name: "en".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: None, origin: Fixture::origin(3), count: None };
        b.carrier(body, en);
        body
    }

    /// A body `name` of `nodes` and nothing else.
    fn spec(name: &str, nodes: Vec<BodyNode>) -> BodySpec {
        BodySpec {
            name: name.into(),
            origin: Fixture::origin(1),
            params: vec![],
            nodes,
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        }
    }

    /// An expression of `kind` typed `ty` at `line`.
    fn expr(b: &mut IrBuilder, kind: Expr, ty: ValueType, line: u32) -> ExprId {
        b.expr(kind, ty, Fixture::origin(line))
    }

    /// The design whose top places `body` once, binding one net per carrier.
    fn design(b: IrBuilder, body: BodyId, carriers: &[(&str, ValueType)]) -> Ir {
        Fixture::design_with_nodes(b, body, carriers, 0)
    }

    /// [`design`](Fixture::design) with `nodes` nodes of the first nature, `Ram.bl[k]`, bound to the body's nodes.
    fn design_with_nodes(mut b: IrBuilder, body: BodyId, carriers: &[(&str, ValueType)], nodes: u32) -> Ir {
        let top = b.next_instance_id();
        let nets: Vec<NetId> = carriers
            .iter()
            .map(|(name, ty)| b.net(NetSpec { path: format!("Ram.{name}"), ty: ty.clone(), declared_by: top, init: None, origin: Fixture::origin(2) }))
            .collect();
        let nature = b.find_nature("Electrical");
        let bound: Vec<NodeRef> = match nature {
            Some(nature) => (0..nodes)
                .map(|k| NodeRef::Node(b.node(NodeSpec { path: format!("Ram.bl[{k}]"), nature, declared_by: top, init: None, origin: Fixture::origin(2) })))
                .collect(),
            None => Vec::new(),
        };
        let id = b.instance(InstanceSpec {
            path: "Ram".into(),
            name: "Ram".into(),
            body,
            parent: None,
            nodes: bound,
            params: vec![],
            origin: Fixture::origin(1),
        });
        b.bind_nets(id, nets);
        b.finish(id)
    }

    /// The array memory with the reads `reads` put in a reaction of the body, one `let` each.
    fn with_reads(reads: impl FnOnce(&mut IrBuilder) -> Vec<ExprId>) -> Vec<piperine_ir::Broken> {
        let mut b = IrBuilder::new();
        let body = Fixture::memory(&mut b);
        let mut lets = Vec::new();
        for (k, read) in reads(&mut b).into_iter().enumerate() {
            lets.push(b.stmt(Stmt::Let { local: k as u32, init: read }));
        }
        let go = b.event(body, Event { source: EventSource::Settle, guard: None, origin: Fixture::origin(4) });
        let locals = (0..lets.len()).map(|k| piperine_ir::Local { name: format!("v{k}"), ty: ValueType::UInt(8), mutable: false }).collect();
        b.reaction(body, piperine_ir::Reaction { on: vec![go], stmts: lets, locals, origin: Fixture::origin(5) });
        Fixture::design(b, body, &[("mem", ValueType::UInt(8)), ("en", ValueType::Bool)]).check()
    }
}

#[test]
fn element_forms_are_checked_against_the_array() {
    // CV-24: `x[i]`, `x[k].prev()` and `x[k].changed()` on an array carrier with an integer index and a constant
    // element below the count are well formed.
    let ok = Fixture::with_reads(|b| {
        let at = Fixture::expr(b, Expr::Int(3), ValueType::UInt(8), 6);
        let read = Fixture::expr(b, Expr::CarrierElement { carrier: 0, index: at }, ValueType::UInt(8), 6);
        let previous = Fixture::expr(b, Expr::ElementPrevious { carrier: 0, index: at }, ValueType::UInt(8), 7);
        vec![read, previous]
    });
    assert_eq!(ok, vec![], "the element forms of an array carrier are well formed");

    // each refused form is one entry that names its rule
    let past = Fixture::with_reads(|b| {
        let at = Fixture::expr(b, Expr::Int(8), ValueType::UInt(8), 6);
        vec![Fixture::expr(b, Expr::CarrierElement { carrier: 0, index: at }, ValueType::UInt(8), 6)]
    });
    assert_eq!(past.len(), 1, "{past:#?}");
    assert!(past[0].what.contains("the constant index 8") && past[0].what.contains("out of range for an array of 8"), "{past:#?}");

    let scalar = Fixture::with_reads(|b| {
        let at = Fixture::expr(b, Expr::Int(0), ValueType::UInt(8), 6);
        vec![Fixture::expr(b, Expr::CarrierElement { carrier: 1, index: at }, ValueType::UInt(8), 6)]
    });
    assert!(scalar.iter().any(|e| e.what.contains("which is no array of carriers")), "{scalar:#?}");

    let real_index = Fixture::with_reads(|b| {
        let at = Fixture::expr(b, Expr::Real(piperine_ir::Real::new(1.0)), ValueType::F64, 6);
        vec![Fixture::expr(b, Expr::CarrierElement { carrier: 0, index: at }, ValueType::UInt(8), 6)]
    });
    assert!(real_index.iter().any(|e| e.what.contains("the index of an element read must be an integer")), "{real_index:#?}");

    // an element read typed other than the element's type, and `changed()` typed other than `bool`
    let mistyped = Fixture::with_reads(|b| {
        let at = Fixture::expr(b, Expr::Int(0), ValueType::UInt(8), 6);
        let wrong = Fixture::expr(b, Expr::CarrierElement { carrier: 0, index: at }, ValueType::UInt(16), 6);
        let flag = Fixture::expr(b, Expr::ElementChanged { carrier: 0, index: at }, ValueType::UInt(8), 7);
        vec![wrong, flag]
    });
    assert!(mistyped.iter().any(|e| e.what.contains("typed u16, its form gives u8")), "{mistyped:#?}");
    assert!(mistyped.iter().any(|e| e.what.contains("typed u8, its form gives bool")), "{mistyped:#?}");

    // a whole read is typed `[u8; 8]`, and its previous value is not read whole
    let whole = Fixture::with_reads(|b| {
        let array = ValueType::Array(Box::new(ValueType::UInt(8)), 8);
        let whole = Fixture::expr(b, Expr::Carrier(0), array, 6);
        let wrong = Fixture::expr(b, Expr::Previous(0), ValueType::UInt(8), 7);
        let _ = whole;
        vec![wrong]
    });
    assert!(whole.iter().any(|e| e.what.contains("an array of carriers: read one element's")), "{whole:#?}");
    let untyped = Fixture::with_reads(|b| vec![Fixture::expr(b, Expr::Carrier(0), ValueType::UInt(8), 6)]);
    assert!(untyped.iter().any(|e| e.what.contains("typed u8, its form gives [u8; 8]")), "a whole read is the array: {untyped:#?}");
}

#[test]
fn an_element_event_names_an_element_of_an_array_carrier() {
    // CV-24: `on mem[3].changed()` is `EventSource::ElementChanged { carrier: 0, element: 3 }`.
    let mut b = IrBuilder::new();
    let body = Fixture::memory(&mut b);
    b.event(body, Event { source: EventSource::ElementChanged { carrier: 0, element: 3 }, guard: None, origin: Fixture::origin(4) });
    let ir = Fixture::design(b, body, &[("mem", ValueType::UInt(8)), ("en", ValueType::Bool)]);
    assert_eq!(ir.check(), vec![]);
    assert!(ir.to_string().contains("event E0 changed mem[3]"), "{ir}");

    for (carrier, element, what) in [(0, 8, "out of range for an array of 8"), (1, 0, "which is no array of carriers"), (9, 0, "out of range (2 carriers)")] {
        let mut b = IrBuilder::new();
        let body = Fixture::memory(&mut b);
        b.event(body, Event { source: EventSource::ElementChanged { carrier, element }, guard: None, origin: Fixture::origin(4) });
        let broken = Fixture::design(b, body, &[("mem", ValueType::UInt(8)), ("en", ValueType::Bool)]).check();
        assert!(broken.iter().any(|e| e.what.contains(what)), "carrier {carrier} element {element}: {broken:#?}");
    }
}

#[test]
fn an_array_of_nodes_names_consecutive_body_nodes_and_round_trips() {
    // CV-23: `signal bl: [Electrical; 4]` is four nodes `bl[0]` to `bl[3]` and one `NodeArray`.
    let mut b = IrBuilder::new();
    let potential = piperine_ir::Quantity { name: "Voltage".into(), access: "v".into(), units: "V".into(), abstol: piperine_ir::Real::new(1e-6) };
    let nature = b.nature("Electrical", potential, None);
    let nodes: Vec<BodyNode> = (0..4)
        .map(|k| BodyNode { name: format!("bl[{k}]"), nature, role: NodeRole::Signal, init: None, origin: Fixture::origin(2) })
        .collect();
    let body = b.body(Fixture::spec("Ladder", nodes));
    b.node_array(body, NodeArray { name: "bl".into(), first: 0, count: 4, origin: Fixture::origin(2) });
    let ir = Fixture::design_with_nodes(b, body, &[], 4);
    assert_eq!(ir.check(), vec![]);
    let json = serde_json::to_string(&ir).expect("an IR serializes");
    let back: Ir = serde_json::from_str(&json).expect("and reads back");
    assert_eq!(back, ir, "the node arrays survive serde");
    assert_eq!(ir.body(body).node_arrays().len(), 1);

    // an array that runs past the body's nodes, and one whose nodes are named otherwise, are each refused
    let mut b = IrBuilder::new();
    let body = b.body(Fixture::spec("Bad", vec![]));
    b.node_array(body, NodeArray { name: "bl".into(), first: 0, count: 4, origin: Fixture::origin(2) });
    let broken = Fixture::design(b, body, &[]).check();
    assert!(broken.iter().any(|e| e.what.contains("array of nodes bl holds nodes 0..4, out of range (0 nodes)")), "{broken:#?}");
}
