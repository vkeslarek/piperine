//! Resolved nets in the IR (resolved-nets RN-03): a carrier of role `Resolved { driven }` reads the resolved value of
//! type `V` and is written with the driven type `X`, `Expr::Others` reads what the other drives resolve to,
//! `Expr::Resolve` combines the present drives of a resolver body through one instance of `resolve` per count, a net's
//! `Resolution` names its resolver, drives and holders, and `Ir::check` refuses each form that does not fit.

use piperine_ir::{
    BodyId, BodySpec, Broken, Carrier, Child, CarrierRole, Drive, Expr, ExprId, FunctionId, FunctionSpec, Holder, InstanceSpec, Ir, IrBuilder, Local, NetSpec, Origin, Resolution, ValueType,
};

/// The bodies and designs this target checks, each beside the MSL it transcribes:
///
/// ```text
/// nature Count { value: u8 }            // impl Resolve<bool, u8> for Count: how many drives are true
/// module Source() { signal w: Count; w = true; signal seen: Bit<u8> = 0; seen = w.others(); }
/// ```
struct Fixture;

impl Fixture {
    const FILE: &'static str = "count.msl";

    fn origin(line: u32) -> Origin {
        Origin { file: Fixture::FILE.into(), line, col: 5 }
    }

    fn spec(name: &str) -> BodySpec {
        BodySpec {
            name: name.into(),
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

    fn expr(b: &mut IrBuilder, kind: Expr, ty: ValueType, line: u32) -> ExprId {
        b.expr(kind, ty, Fixture::origin(line))
    }

    /// `resolve::<k>` of the nature, for each `k` in `0..=2`: it takes `[bool; k]` and returns `u8`; `returns` is the
    /// type the one of count `wrong` gives instead, when a test needs a function of another type.
    fn instances(b: &mut IrBuilder, wrong: Option<(usize, ValueType)>) -> Vec<FunctionId> {
        (0..=2usize)
            .map(|count| {
                let ret = wrong.as_ref().filter(|(at, _)| *at == count).map_or(ValueType::UInt(8), |(_, ty)| ty.clone());
                let body = Fixture::expr(b, Expr::Int(0), ret.clone(), 2);
                b.function(FunctionSpec {
                    name: format!("resolve~{count}"),
                    origin: Fixture::origin(2),
                    params: vec![Local { name: "drives".into(), ty: ValueType::Array(Box::new(ValueType::Bool), count as u64), mutable: false }],
                    ret,
                    locals: vec![],
                    body,
                })
            })
            .collect()
    }

    /// The resolver body `Count$resolve~2`: two `in` drives of the driven type and the `out` value, defined as the
    /// `Resolve` of both drives through `by_count`.
    fn resolver(b: &mut IrBuilder, by_count: Vec<FunctionId>, drives: Vec<u32>) -> BodyId {
        let body = b.body(Fixture::spec("Count$resolve~2"));
        for name in ["d0", "d1"] {
            b.carrier(body, Carrier { name: name.into(), ty: ValueType::Bool, role: CarrierRole::In, init: None, origin: Fixture::origin(3), count: None });
        }
        b.carrier(body, Carrier { name: "v".into(), ty: ValueType::UInt(8), role: CarrierRole::Out, init: None, origin: Fixture::origin(3), count: None });
        let value = Fixture::expr(b, Expr::Resolve { by_count, drives, except: None }, ValueType::UInt(8), 3);
        b.drive(body, Drive { carrier: 2, value, after: None, origin: Fixture::origin(3), element: None, path: vec![] });
        body
    }

    /// The holder body `Source`: `w` is resolved (value `u8`, driven `bool`), driven with `true`, and `seen` follows
    /// `w.others()`.
    fn source(b: &mut IrBuilder, driven: ValueType, drive_value: Expr, drive_ty: ValueType) -> BodyId {
        let body = b.body(Fixture::spec("Source"));
        b.carrier(body, Carrier { name: "w".into(), ty: ValueType::UInt(8), role: CarrierRole::Resolved { driven, port: true }, init: None, origin: Fixture::origin(4), count: None });
        b.carrier(body, Carrier { name: "seen".into(), ty: ValueType::UInt(8), role: CarrierRole::Signal, init: Some(piperine_ir::Constant::Int(0)), origin: Fixture::origin(5), count: None });
        let drive = Fixture::expr(b, drive_value, drive_ty, 4);
        b.drive(body, Drive { carrier: 0, value: drive, after: None, origin: Fixture::origin(4), element: None, path: vec![] });
        let others = Fixture::expr(b, Expr::Others(0), ValueType::UInt(8), 5);
        b.drive(body, Drive { carrier: 1, value: others, after: None, origin: Fixture::origin(5), element: None, path: vec![] });
        body
    }

    /// The design: `Source` is the top, its resolver the child `Source.w$resolve` over two drive nets, the first held
    /// by the top and the second by the host.
    fn design(mut b: IrBuilder, source: BodyId, resolver: BodyId) -> Ir {
        let top = b.next_instance_id();
        let value = b.net(NetSpec { path: "Source.w".into(), ty: ValueType::UInt(8), declared_by: top, init: None, origin: Fixture::origin(4) });
        let seen = b.net(NetSpec { path: "Source.seen".into(), ty: ValueType::UInt(8), declared_by: top, init: Some(piperine_ir::Constant::Int(0)), origin: Fixture::origin(5) });
        let drive = |b: &mut IrBuilder, k: u32| b.net(NetSpec { path: format!("Source.w$drive{k}"), ty: ValueType::Bool, declared_by: top, init: None, origin: Fixture::origin(4) });
        let (first, second) = (drive(&mut b, 0), drive(&mut b, 1));
        let others = b.net(NetSpec { path: "Source.w$others0".into(), ty: ValueType::UInt(8), declared_by: top, init: None, origin: Fixture::origin(4) });
        let root = b.instance(InstanceSpec { path: "Source".into(), name: "Source".into(), body: source, parent: None, nodes: vec![], params: vec![], origin: Fixture::origin(1) });
        b.bind_nets(root, vec![value, seen]);
        let child = b.instance(InstanceSpec { path: "Source.w$resolve".into(), name: "w$resolve".into(), body: resolver, parent: Some(root), nodes: vec![], params: vec![], origin: Fixture::origin(4) });
        b.bind_nets(child, vec![first, second, value]);
        b.resolve_net(value, Resolution { resolver: child, driven: ValueType::Bool, drives: vec![first, second], holders: vec![Some(Holder { instance: root, carrier: 0 }), None], others: vec![(0, others)] });
        b.finish(root)
    }

    /// The whole design with the drive written as `drive_value` of type `drive_ty` for a carrier driven with `driven`,
    /// and the `Resolve` of the resolver given by `by_count` over `drives`.
    fn checked(driven: ValueType, drive_value: Expr, drive_ty: ValueType, wrong: Option<(usize, ValueType)>, drives: Vec<u32>) -> (Ir, Vec<Broken>) {
        let mut b = IrBuilder::new();
        let by_count = Fixture::instances(&mut b, wrong);
        let resolver = Fixture::resolver(&mut b, by_count, drives);
        let source = Fixture::source(&mut b, driven, drive_value, drive_ty);
        let ir = Fixture::design(b, source, resolver);
        let broken = ir.check();
        (ir, broken)
    }
}

#[test]
fn a_resolved_net_is_built_checked_and_printed() {
    // RN-03: a `Resolved` carrier is driven with its driven type, `others()` reads the value type, `Resolve` is typed by
    // its instances' result, and the net records its resolver, drives and holders.
    let (ir, broken) = Fixture::checked(ValueType::Bool, Expr::Bool(true), ValueType::Bool, None, vec![0, 1]);
    assert_eq!(broken, vec![], "a well formed resolved net");

    let printed = ir.to_string();
    assert!(printed.contains("carrier w: u8 resolved bool"), "the role names the driven type:\n{printed}");
    assert!(printed.contains("w.others()"), "`others()` is printed:\n{printed}");
    assert!(printed.contains("resolve [\"resolve~0\", \"resolve~1\", \"resolve~2\"] of (d0, d1)"), "`resolve` names its instances and drives:\n{printed}");

    assert!(
        printed.contains("net Source.w: u8  declared by Source  resolved by Source.w$resolve of bool drives (Source.w$drive0, Source.w$drive1) held by (Source#0, host) others (0: Source.w$others0)"),
        "the net prints its resolution:\n{printed}"
    );

    let net = ir.nets().find(|net| net.path() == "Source.w").expect("the value net");
    let resolution = net.resolution().expect("the value net is resolved");
    assert_eq!(resolution.drives.len(), 2, "one drive net per drive");
    assert_eq!(resolution.holders.iter().filter(|holder| holder.is_none()).count(), 1, "the host holds one drive");
    assert_eq!(resolution.driven, ValueType::Bool, "the net records its driven type");
    assert!(ir.nets().filter(|other| other.path() != "Source.w").all(|other| other.resolution().is_none()), "only the value net is resolved");

    let round_trip: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serialises")).expect("deserialises");
    assert_eq!(round_trip, ir, "a resolved net survives serde");
}

#[test]
fn a_resolve_with_a_missing_count_or_another_type_is_refused() {
    // RN-03: `by_count` has one instance per count `0..=drives`, each taking `[X; k]` and returning `V`.
    let (_, short) = Fixture::checked(ValueType::Bool, Expr::Bool(true), ValueType::Bool, None, vec![0, 1, 1]);
    assert!(short.iter().any(|e| e.what.contains("names 3 instances of the function for 3 drives; it needs one per count 0..=3")), "a missing count: {short:#?}");

    let (_, other) = Fixture::checked(ValueType::Bool, Expr::Bool(true), ValueType::Bool, Some((1, ValueType::F64)), vec![0, 1]);
    assert!(other.iter().any(|e| e.what.contains("instance 1 (`resolve~1`) returns f64")), "an instance of another result type: {other:#?}");

    let (_, mixed) = Fixture::checked(ValueType::Bool, Expr::Bool(true), ValueType::Bool, None, vec![0, 2]);
    assert!(mixed.iter().any(|e| e.what.contains("`resolve` drives hold two types, bool and u8")), "drives of two types: {mixed:#?}");

    // a drive of a resolved carrier writes the driven type, not the value type
    let (_, wrong_drive) = Fixture::checked(ValueType::Bool, Expr::Int(1), ValueType::UInt(8), None, vec![0, 1]);
    assert!(wrong_drive.iter().any(|e| e.what.contains("the value of a drive must be bool, found u8")), "a drive of the value type is refused: {wrong_drive:#?}");

    // `others()` reads a resolved carrier only
    let mut b = IrBuilder::new();
    let by_count = Fixture::instances(&mut b, None);
    let resolver = Fixture::resolver(&mut b, by_count, vec![0, 1]);
    let source = Fixture::source(&mut b, ValueType::Bool, Expr::Bool(true), ValueType::Bool);
    let signal_others = Fixture::expr(&mut b, Expr::Others(1), ValueType::UInt(8), 6);
    b.drive(source, Drive { carrier: 1, value: signal_others, after: None, origin: Fixture::origin(6), element: None, path: vec![] });
    let broken = Fixture::design(b, source, resolver).check();
    assert!(broken.iter().any(|e| e.what.contains("`others()` of carrier 1, which is not a resolved carrier")), "others() of a signal: {broken:#?}");
}

/// A parent that binds the carriers of one child whose body has an `in` port `a` and a resolved carrier `w`, a `signal` (`port` false) or
/// a port (`port` true).
struct Placement;

impl Placement {
    /// The check of the design: `Parent` places `Holder` with `bound` carriers.
    fn broken(port: bool, bound: Vec<Option<u32>>) -> Vec<Broken> {
        let mut b = IrBuilder::new();
        let holder = b.body(Fixture::spec("Holder"));
        b.carrier(holder, Carrier { name: "a".into(), ty: ValueType::Bool, role: CarrierRole::In, init: None, origin: Fixture::origin(2), count: None });
        b.carrier(holder, Carrier { name: "w".into(), ty: ValueType::UInt(8), role: CarrierRole::Resolved { driven: ValueType::Bool, port }, init: None, origin: Fixture::origin(3), count: None });
        let parent = b.body(BodySpec { children: vec![Child { name: "h".into(), body: holder, nodes: vec![], params: vec![], origin: Fixture::origin(5) }], ..Fixture::spec("Parent") });
        b.carrier(parent, Carrier { name: "x".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: Some(piperine_ir::Constant::Bool(false)), origin: Fixture::origin(4), count: None });
        b.carrier(parent, Carrier { name: "y".into(), ty: ValueType::UInt(8), role: CarrierRole::Signal, init: Some(piperine_ir::Constant::Int(0)), origin: Fixture::origin(4), count: None });
        b.bind_child_carriers(parent, 0, bound);
        let top = b.next_instance_id();
        let net = |b: &mut IrBuilder, path: &str, ty: ValueType, init: piperine_ir::Constant| b.net(NetSpec { path: path.into(), ty, declared_by: top, init: Some(init), origin: Fixture::origin(4) });
        let x = net(&mut b, "Parent.x", ValueType::Bool, piperine_ir::Constant::Bool(false));
        let y = net(&mut b, "Parent.y", ValueType::UInt(8), piperine_ir::Constant::Int(0));
        let root = b.instance(InstanceSpec { path: "Parent".into(), name: "Parent".into(), body: parent, parent: None, nodes: vec![], params: vec![], origin: Fixture::origin(1) });
        b.bind_nets(root, vec![x, y]);
        let child = b.instance(InstanceSpec { path: "Parent.h".into(), name: "h".into(), body: holder, parent: Some(root), nodes: vec![], params: vec![], origin: Fixture::origin(5) });
        b.bind_nets(child, vec![x, y]);
        b.finish(root).check()
    }
}

#[test]
fn a_resolved_signal_is_no_port_of_a_child_binding() {
    // a resolved carrier that is a signal (`port: false`) is no port of its body: a parent that binds the carriers of the child
    // gives one entry for the `in` port only, and two entries are one too many; a resolved port (`port: true`) takes the second
    assert_eq!(Placement::broken(false, vec![Some(0)]), vec![], "one entry for the one port");
    let too_many = Placement::broken(false, vec![Some(0), Some(1)]);
    assert!(too_many.iter().any(|e| e.what.contains("binds 2 carriers, body Holder has 1 carrier ports")), "a resolved signal is counted as no port: {too_many:#?}");
    assert_eq!(Placement::broken(true, vec![Some(0), Some(1)]), vec![], "a resolved port is counted");
    let too_few = Placement::broken(true, vec![Some(0)]);
    assert!(too_few.iter().any(|e| e.what.contains("binds 1 carriers, body Holder has 2 carrier ports")), "a resolved port is counted as a port: {too_few:#?}");
}
