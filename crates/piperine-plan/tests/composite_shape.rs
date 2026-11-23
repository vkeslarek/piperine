//! Scope: the ABI layout of an IR aggregate type (composite-values T56, CV-17, CV-20, CV-11;
//! spec §2, abi.md P3 and P4): the leaves a composite pin lists, the equality of two pins of one
//! net, a one-variant enum's payload-only layout and the zero levels of an empty struct. Each
//! expected offset is the byte offset §2's table gives: the parts' value bytes one after another,
//! the first lowest, and nothing else (abi.md P3).

use piperine_abi::layout::layout_eq;
use piperine_abi::raw::leaf;
use piperine_ir::{
    AggregateData, BodySpec, Constant, Derives, FieldsForm, InstanceSpec, Ir, IrBuilder, Origin, Real, TypeData, TypeId, TypeKind, ValueType, VariantData,
};
use piperine_plan::{CompositeLayout, PinLayout};

/// The types of the fixture's arena.
struct Types {
    pair: TypeId,
    tx: TypeId,
    marker: TypeId,
    single: TypeId,
}

impl Types {
    fn origin() -> Origin {
        Origin { file: "composite.msl".into(), line: 1, col: 1 }
    }

    fn derives() -> Derives {
        Derives { clone: true, copy: true, partial_eq: true, repr: true, ..Derives::default() }
    }

    /// `struct Pair { a: f64, b: bool }`, `enum Tx { Idle = 0, Data { n: u3 } = 1, Stop = 2 }`
    /// with a 2-level tag, `struct Marker`, and `enum Only { One { n: u8 } }`.
    fn add(b: &mut IrBuilder) -> Types {
        let pair = b.type_data(TypeData {
            name: "Pair".into(),
            path: "m::Pair".into(),
            origin: Types::origin(),
            kind: TypeKind::Struct { form: FieldsForm::Named, fields: vec![("a".into(), ValueType::F64), ("b".into(), ValueType::Bool)] },
            derives: Types::derives(),
            instance: None,
            repr: Default::default(),
        });
        let variant = |name: &str, discriminant, form, fields: Vec<(String, ValueType)>| VariantData { name: name.into(), discriminant, form, fields };
        let tx = b.type_data(TypeData {
            name: "Tx".into(),
            path: "m::Tx".into(),
            origin: Types::origin(),
            kind: TypeKind::Enum {
                tag_levels: 2,
                variants: vec![
                    variant("Idle", 0, FieldsForm::Unit, vec![]),
                    variant("Data", 1, FieldsForm::Named, vec![("n".into(), ValueType::UInt(3))]),
                    variant("Stop", 2, FieldsForm::Unit, vec![]),
                ],
            },
            derives: Types::derives(),
            instance: None,
            repr: Default::default(),
        });
        let marker = b.type_data(TypeData { name: "Marker".into(), path: "m::Marker".into(), origin: Types::origin(), kind: TypeKind::Struct { form: FieldsForm::Unit, fields: vec![] }, derives: Types::derives(), instance: None, repr: Default::default() });
        let single = b.type_data(TypeData {
            name: "Only".into(),
            path: "m::Only".into(),
            origin: Types::origin(),
            kind: TypeKind::Enum { tag_levels: 0, variants: vec![variant("One", 0, FieldsForm::Named, vec![("n".into(), ValueType::UInt(8))])] },
            derives: Types::derives(),
            instance: None,
            repr: Default::default(),
        });
        Types { pair, tx, marker, single }
    }
}

/// A design holding only the arena of `Types`, under an empty top.
struct Design;

impl Design {
    /// The design and the arena's ids; `extra` adds more entries (constants) before `finish`.
    fn of(extra: impl FnOnce(&mut IrBuilder, &Types)) -> (Ir, Types) {
        let mut b = IrBuilder::new();
        let types = Types::add(&mut b);
        extra(&mut b, &types);
        let top_body = b.body(BodySpec {
            name: "Top".into(),
            origin: Types::origin(),
            params: vec![],
            nodes: vec![],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![], params: vec![], origin: Types::origin() });
        (b.finish(top), types)
    }
}

/// The leaves of a layout as `(name, leaf, offset)`.
struct Leaves;

impl Leaves {
    fn of(ir: &Ir, ty: &ValueType) -> (Vec<(String, u32, u32)>, u32) {
        let layout = CompositeLayout::of(ir, ty).expect("a layout");
        (layout.leaves().iter().map(|l| (l.name.clone(), l.leaf, l.offset)).collect(), layout.width())
    }

    fn named(rows: &[(&str, u32, u32)]) -> Vec<(String, u32, u32)> {
        rows.iter().map(|(n, l, o)| (n.to_string(), *l, *o)).collect()
    }
}

#[test]
fn composite_layouts_list_their_leaves() {
    // §2: a struct's fields first-lowest, nested fields dotted, a tuple's items numbered, an
    // array's elements indexed, an enum's tag then the widest payload, an option's presence byte
    // then the payload (CV-17 AC 1)
    let (ir, t) = Design::of(|_, _| {});
    let pair = ValueType::Struct(t.pair);
    assert_eq!(Leaves::of(&ir, &pair), (Leaves::named(&[("a", leaf::F64, 0), ("b", leaf::BOOL, 8)]), 9), "struct Pair: a at 0, b at 8, nine bytes");
    let nested = ValueType::Tuple(vec![ValueType::UInt(12), pair.clone()]);
    assert_eq!(
        Leaves::of(&ir, &nested),
        (Leaves::named(&[("0", leaf::U16, 0), ("1.a", leaf::F64, 2), ("1.b", leaf::BOOL, 10)]), 11),
        "a tuple's item 1 is the nested struct"
    );
    let array = ValueType::Array(Box::new(ValueType::Int(16)), 3);
    assert_eq!(Leaves::of(&ir, &array), (Leaves::named(&[("[0]", leaf::I16, 0), ("[1]", leaf::I16, 2), ("[2]", leaf::I16, 4)]), 6), "an array has one field per element");
    let option = ValueType::Option(Box::new(ValueType::F64));
    assert_eq!(Leaves::of(&ir, &option), (Leaves::named(&[("some", leaf::BOOL, 0), ("value", leaf::F64, 1)]), 9), "an option is its presence byte then its payload");
    let tx = ValueType::Enum(t.tx);
    assert_eq!(
        Leaves::of(&ir, &tx),
        (Leaves::named(&[("tag", leaf::U8, 0), ("Data.n", leaf::U8, 1)]), 2),
        "a payload enum is the tag, then the widest payload (Data's u3)"
    );
    let layout = CompositeLayout::of(&ir, &pair).expect("a layout");
    assert_eq!(layout.width(), 9);
    assert!(layout.abi_fields().iter().all(|f| f.count == 1), "every leaf is one scalar");
}

#[test]
fn two_pins_of_a_composite_net_have_equal_layouts() {
    // abi.md P4: the layout is a function of the type alone, so the pin of an `out` port and the
    // pin of the `in` port it meets are equal, whatever their names (CV-20 AC 3)
    let (ir, t) = Design::of(|_, _| {});
    let ty = ValueType::Struct(t.pair);
    let (a, b) = (PinLayout::in_ir(&ir, &ty).expect("a layout"), PinLayout::in_ir(&ir, &ty).expect("a layout"));
    let (fa, fb) = (a.fields(), b.fields());
    assert!(layout_eq(&a.layout(&fa), &b.layout(&fb)), "equal layouts for one type");
    let other = PinLayout::in_ir(&ir, &ValueType::Tuple(vec![ValueType::F64, ValueType::Bool])).expect("a layout");
    let other_fields = other.fields();
    assert!(layout_eq(&a.layout(&fa), &other.layout(&other_fields)), "names do not count: a struct and a tuple of the same leaves are equal");
}

#[test]
fn a_one_variant_enum_lays_out_its_payload_only() {
    // §2 rule 5: one variant needs no tag, so the layout is the payload
    let (ir, t) = Design::of(|_, _| {});
    assert_eq!(Leaves::of(&ir, &ValueType::Enum(t.single)), (Leaves::named(&[("One.n", leaf::U8, 0)]), 1), "no `tag` leaf");
}

#[test]
fn an_empty_struct_is_carried_with_zero_levels() {
    // CV-17 edge: an empty struct and `()` hold no value byte: no leaf, no byte
    let (ir, t) = Design::of(|_, _| {});
    for ty in [ValueType::Struct(t.marker), ValueType::Tuple(vec![])] {
        let layout = CompositeLayout::of(&ir, &ty).expect("a layout");
        assert_eq!((layout.leaves().len(), layout.width()), (0, 0), "{ty}: no leaf, no byte");
    }
}

#[test]
fn a_constant_is_the_bytes_the_layout_names() {
    // the bytes of a default (abi.md P3): the value bytes at the leaves' offsets, an enum's
    // unused payload zero
    let (ir, t, pair_value, data_value) = {
        let mut ids = None;
        let (ir, t) = Design::of(|b, t| {
            let pair = b.aggregate(AggregateData { ty: ValueType::Struct(t.pair), variant: None, parts: vec![Constant::Real(Real::new(1.5)), Constant::Bool(true)] });
            let data = b.aggregate(AggregateData { ty: ValueType::Enum(t.tx), variant: Some(1), parts: vec![Constant::Int(5)] });
            ids = Some((pair, data));
        });
        let (pair, data) = ids.expect("ids");
        (ir, t, Constant::Aggregate(pair), Constant::Aggregate(data))
    };
    let mut expected = 1.5f64.to_le_bytes().to_vec();
    expected.push(1);
    assert_eq!(CompositeLayout::bytes(&ir, &ValueType::Struct(t.pair), pair_value), Some(expected));
    assert_eq!(CompositeLayout::bytes(&ir, &ValueType::Enum(t.tx), data_value), Some(vec![1, 5]), "tag 1 (Data), n = 5");
    assert_eq!(CompositeLayout::bytes(&ir, &ValueType::Enum(t.tx), pair_value), None, "a struct constant is no enum value");
}
