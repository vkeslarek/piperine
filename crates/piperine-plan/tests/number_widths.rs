//! The layouts and bytes of the number widths in the plan (composite-values phase D, CV-29 to
//! CV-32; abi.md §3.1 P3, P4): `f32` is the `F32` leaf, `f16` one `U16`, `f128` one `U64` field of
//! `count` 2 and an integer of `N > 64` bits one `U64` field of `count` `ceil(N/64)`, low limb
//! first, alone, in a struct, in an array and in a record field. The oracle is the spec's §2 table
//! (the IEEE 754 bit pattern; two's complement) written out by hand in each expectation.

use piperine_abi::raw::{Layout, Pin, Shape, kind, leaf};
use piperine_ir::{AggregateData, BodyId, BodySpec, Carrier, CarrierRole, Constant, Derives, FieldsForm, InstanceSpec, Ir, IrBuilder, NetId, NetSpec, Origin, TypeData, TypeKind, ValueType, WideData};
use piperine_plan::{BodyPlan, CarrierLayout, CompositeLayout, NumberLayout, PinLayout};

/// The designs of this target.
struct Fixture;

impl Fixture {
    fn origin(line: u32) -> Origin {
        Origin { file: "numbers.msl".into(), line, col: 5 }
    }

    /// The body `Num` with no carrier yet.
    fn spec() -> BodySpec {
        BodySpec { name: "Num".into(), origin: Fixture::origin(1), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] }
    }

    fn carrier(name: &str, ty: ValueType, init: Option<Constant>, count: Option<u64>, line: u32) -> Carrier {
        Carrier { name: name.into(), ty, role: CarrierRole::Signal, init, origin: Fixture::origin(line), count }
    }

    /// The checked design placing `body` once, a net per carrier.
    fn design(mut b: IrBuilder, body: BodyId, carriers: &[(&str, ValueType)]) -> Ir {
        let top = b.next_instance_id();
        let nets: Vec<NetId> = carriers
            .iter()
            .map(|(name, ty)| b.net(NetSpec { path: format!("Num.{name}"), ty: ty.clone(), declared_by: top, init: None, origin: Fixture::origin(2) }))
            .collect();
        let id = b.instance(InstanceSpec { path: "Num".into(), name: "Num".into(), body, parent: None, nodes: vec![], params: vec![], origin: Fixture::origin(1) });
        b.bind_nets(id, nets);
        let ir = b.finish(id);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        ir
    }

    /// A checked design of one body with no carrier.
    fn empty() -> Ir {
        let mut b = IrBuilder::new();
        let body = b.body(Fixture::spec());
        Fixture::design(b, body, &[])
    }

    /// `(f128, u100)` as a source struct `Pair`.
    fn pair(b: &mut IrBuilder) -> ValueType {
        let id = b.type_data(TypeData {
            name: "Pair".into(),
            path: "m::Pair".into(),
            origin: Fixture::origin(3),
            kind: TypeKind::Struct { form: FieldsForm::Named, fields: vec![("x".into(), ValueType::F128), ("n".into(), ValueType::UInt(100))] },
            derives: Derives::default(),
            instance: None,
            repr: Default::default(),
        });
        ValueType::Struct(id)
    }

    /// A pin of `shape` by name.
    fn pin<'a, 's>(shape: &'a Shape<'s>, name: &str) -> &'a Pin<'s> {
        shape.pins.iter().find(|p| p.name.as_str() == name).unwrap_or_else(|| panic!("no pin `{name}`"))
    }

    /// The fields of a layout as `(leaf, offset, count)`.
    fn fields(layout: &Layout<'_>) -> Vec<(u32, u32, u32)> {
        layout.fields.iter().map(|f| (f.leaf, f.offset, f.count)).collect()
    }
}

#[test]
fn each_new_scalar_has_its_leaf_count_and_bytes() {
    // §2 table, abi.md §3.1 P3: alone, a scalar is the leaf, its count, its size and its bytes
    let f32_layout = CarrierLayout::of(&ValueType::F32).expect("an f32 pin");
    assert_eq!((f32_layout.size, f32_layout.align, Fixture::fields(f32_layout)), (4, 4, vec![(leaf::F32, 0, 1)]));
    let f16_layout = CarrierLayout::of(&ValueType::F16).expect("an f16 pin");
    assert_eq!((f16_layout.size, f16_layout.align, Fixture::fields(f16_layout)), (2, 2, vec![(leaf::U16, 0, 1)]));
    let f128_layout = CarrierLayout::of(&ValueType::F128).expect("an f128 pin");
    assert_eq!((f128_layout.size, f128_layout.align, Fixture::fields(f128_layout)), (16, 8, vec![(leaf::U64, 0, 2)]));
    assert_eq!(CarrierLayout::of(&ValueType::UInt(200)).map(|l| l.size), None, "a wide integer's layout depends on its width: it is a composite leaf");

    // the bit patterns of 1.5 in each format, little-endian
    assert_eq!(CarrierLayout::bytes(&ValueType::F32, Constant::FloatBits(0x3fc0_0000)), Some(vec![0x00, 0x00, 0xc0, 0x3f]));
    assert_eq!(CarrierLayout::bytes(&ValueType::F16, Constant::FloatBits(0x3e00)), Some(vec![0x00, 0x3e]));
    assert_eq!(CarrierLayout::bytes(&ValueType::F128, Constant::FloatBits(0x3fff_8000_0000_0000_0000_0000_0000_0000)), Some({
        let mut bytes = vec![0u8; 16];
        bytes[14] = 0xff;
        bytes[15] = 0x3f;
        bytes[13] = 0x80;
        bytes
    }));
    assert_eq!(CarrierLayout::bytes(&ValueType::F16, Constant::FloatBits(0x1_0000)), None, "a bit past the width is no f16");
    assert_eq!(CarrierLayout::bytes(&ValueType::F32, Constant::Real(1.5f64.into())), None, "an f64 constant is no f32");

    // a wide integer: one U64 field of ceil(N/64) elements, low limb first
    let ir = Fixture::empty();
    for (width, count) in [(65u32, 2u32), (128, 2), (129, 3), (200, 4), (4096, 64)] {
        let layout = CompositeLayout::of(&ir, &ValueType::UInt(width)).expect("a wide integer");
        assert_eq!((layout.width(), layout.leaves().len()), (count * 8, 1), "u{width}");
        let fields = layout.abi_fields();
        assert_eq!((fields[0].leaf, fields[0].offset, fields[0].count), (leaf::U64, 0, count), "u{width}");
        assert_eq!(NumberLayout::field_of(&ValueType::Int(width)), Some((leaf::U64, count, count * 8)));
    }
    assert_eq!(NumberLayout::field_of(&ValueType::UInt(64)), None, "64 bits and below keep today's layouts");
}

#[test]
fn a_new_scalar_inside_a_composite_is_one_leaf_with_its_count() {
    let mut b = IrBuilder::new();
    let pair = Fixture::pair(&mut b);
    let body = b.body(Fixture::spec());
    // `signal wide: u200 = 0x1_0000_0000_0000_0003;` (limbs 3, 1, 0, 0)
    let wide_value = b.wide(WideData { limbs: vec![3, 1, 0, 0] });
    b.carrier(body, Fixture::carrier("wide", ValueType::UInt(200), Some(Constant::Wide(wide_value)), None, 2));
    // `signal minus: i70 = -1;` (two limbs of ones)
    let minus_one = b.wide(WideData { limbs: vec![u64::MAX, u64::MAX] });
    b.carrier(body, Fixture::carrier("minus", ValueType::Int(70), Some(Constant::Wide(minus_one)), None, 3));
    b.carrier(body, Fixture::carrier("single", ValueType::F32, Some(Constant::FloatBits(0x3f80_0000)), None, 4));
    b.carrier(body, Fixture::carrier("half", ValueType::F16, None, None, 5));
    b.carrier(body, Fixture::carrier("quad", ValueType::F128, Some(Constant::FloatBits(1)), None, 6));
    let pair_value = {
        let wide = b.wide(WideData { limbs: vec![9, 0] });
        b.aggregate(AggregateData { ty: pair.clone(), variant: None, parts: vec![Constant::FloatBits(0x3fff_0000_0000_0000_0000_0000_0000_0000), Constant::Wide(wide)] })
    };
    b.carrier(body, Fixture::carrier("pair", pair.clone(), Some(Constant::Aggregate(pair_value)), None, 7));
    b.carrier(body, Fixture::carrier("row", ValueType::F128, None, Some(3), 8));
    let triple = ValueType::Array(Box::new(ValueType::F16), 3);
    b.carrier(body, Fixture::carrier("halves", triple.clone(), None, None, 9));
    let ir = Fixture::design(
        b,
        body,
        &[("wide", ValueType::UInt(200)), ("minus", ValueType::Int(70)), ("single", ValueType::F32), ("half", ValueType::F16), ("quad", ValueType::F128), ("pair", pair.clone()), ("row", ValueType::F128), ("halves", triple)],
    );
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()), "a U64 field of count > 1 and an f32 pin validate (abi.md §3.1 P3)");

    let wide = Fixture::pin(shape, "wide");
    assert_eq!((wide.kind, wide.layout.size, Fixture::fields(&wide.layout)), (kind::DISCRETE, 32, vec![(leaf::U64, 0, 4)]));
    let mut expected = vec![0u8; 32];
    expected[0] = 3;
    expected[8] = 1;
    assert_eq!(wide.init.as_slice(), expected.as_slice(), "u200 held as four limbs, low limb first");
    let minus = Fixture::pin(shape, "minus");
    assert_eq!((minus.layout.size, Fixture::fields(&minus.layout)), (16, vec![(leaf::U64, 0, 2)]));
    assert_eq!(minus.init.as_slice(), [0xff; 16].as_slice(), "i70 of -1 is sign-extended to the limb boundary");
    assert_eq!(Fixture::fields(&Fixture::pin(shape, "single").layout), vec![(leaf::F32, 0, 1)]);
    assert_eq!(Fixture::pin(shape, "single").init.as_slice(), [0, 0, 0x80, 0x3f].as_slice());
    assert_eq!(Fixture::fields(&Fixture::pin(shape, "half").layout), vec![(leaf::U16, 0, 1)]);
    let quad = Fixture::pin(shape, "quad");
    assert_eq!((quad.layout.size, quad.layout.align, Fixture::fields(&quad.layout)), (16, 8, vec![(leaf::U64, 0, 2)]));
    assert_eq!(quad.init.as_slice()[0], 1);

    // a struct is one leaf per field, each with its own count: `Pair { x: f128, n: u100 }`
    let pair_pin = Fixture::pin(shape, "pair");
    assert_eq!((pair_pin.layout.size, pair_pin.layout.align), (32, 1), "a composite is unaligned bytes");
    assert_eq!(Fixture::fields(&pair_pin.layout), vec![(leaf::U64, 0, 2), (leaf::U64, 16, 2)]);
    let names: Vec<&str> = pair_pin.layout.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["x", "n"]);
    assert_eq!(pair_pin.init.as_slice()[14..16], [0xff, 0x3f], "x is 1.0 in binary128: the top two bytes of the sixteen");
    assert_eq!(pair_pin.init.as_slice()[16], 9, "n is the value 9 in its limbs");

    // an array of an f128 carrier is an array pin of elements of one layout; an array value is one leaf per element
    let row = Fixture::pin(shape, "row");
    assert_eq!((row.count, row.layout.size, Fixture::fields(&row.layout)), (3, 16, vec![(leaf::U64, 0, 2)]));
    let halves = Fixture::pin(shape, "halves");
    assert_eq!(Fixture::fields(&halves.layout), vec![(leaf::U16, 0, 1), (leaf::U16, 2, 1), (leaf::U16, 4, 1)]);

    // P4: the layout is a function of the IR type alone, so two pins of one type are equal
    let (first, second) = (PinLayout::in_ir(&ir, &ValueType::UInt(200)).expect("u200"), PinLayout::in_ir(&ir, &ValueType::UInt(200)).expect("u200"));
    assert_eq!(first.size(), 32);
    assert!(piperine_abi::layout::layout_eq(&first.layout(&first.fields()), &second.layout(&second.fields())));
}

#[test]
fn a_record_field_of_a_new_scalar_is_laid_out_after_its_alignment() {
    // a stream's record pin lays each field at the next offset its alignment allows: a `bool`, then an f128
    // (align 8) at byte 8, then an f16 at byte 24
    use piperine_ir::{Stream, StreamField};
    let stream = Stream {
        name: "samples".into(),
        fields: vec![
            StreamField { name: "valid".into(), ty: ValueType::Bool },
            StreamField { name: "value".into(), ty: ValueType::F128 },
            StreamField { name: "tag".into(), ty: ValueType::F16 },
        ],
    };
    let layout = piperine_plan::RecordLayout::of(&stream).expect("a record of numbers");
    assert_eq!(layout.fields.iter().map(|(offset, _)| *offset).collect::<Vec<_>>(), vec![0, 8, 24]);
    assert_eq!((layout.size, layout.align), (32, 8));
}
