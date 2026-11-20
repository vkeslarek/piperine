//! Scope: the aggregate forms of the IR (composite-values phase B, CV-20): the type arena of source
//! structs and enums (`ValueType::{Tuple, Array, Slice, Struct, Enum, Option}`, `TypeData`), what
//! `Ir::check` demands of it (§1.3.7, §1.3.8, §2 rule 5, H.8), and how it prints. Each form is
//! built by hand, checked (valid and refused) and printed.

#[path = "support/digital.rs"]
#[allow(dead_code)]
mod digital;
#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;
#[path = "support/shape.rs"]
#[allow(dead_code)]
mod shape;

use piperine_ir::{Derives, FieldsForm, Ir, IrBuilder, Local, Origin, TypeData, TypeId, TypeKind, ValueType, VariantData};
use shape::{Entries, FILE, Shape, Written};

/// The builders of the type arena's entries.
struct Types;

impl Types {
    fn origin(line: u32) -> Origin {
        Origin { file: FILE.into(), line, col: 1 }
    }

    /// `struct <name> { <fields> }` at line `line`, deriving `derives`.
    fn named(b: &mut IrBuilder, name: &str, line: u32, fields: &[(&str, ValueType)], derives: Derives) -> TypeId {
        let fields = fields.iter().map(|(n, ty)| (n.to_string(), ty.clone())).collect();
        b.type_data(TypeData { name: name.into(), path: format!("m::{name}"), origin: Types::origin(line), kind: TypeKind::Struct { form: FieldsForm::Named, fields }, derives })
    }

    /// `enum <name> { Idle = 0, Data { n: u3 } = 1, Stop = 2 }` with a `tag_levels`-level tag.
    fn tx(b: &mut IrBuilder, tag_levels: u32, discriminants: [i128; 3]) -> TypeId {
        let variant = |name: &str, discriminant, form, fields: Vec<(String, ValueType)>| VariantData { name: name.into(), discriminant, form, fields };
        let variants = vec![
            variant("Idle", discriminants[0], FieldsForm::Unit, vec![]),
            variant("Data", discriminants[1], FieldsForm::Named, vec![("n".into(), ValueType::UInt(3))]),
            variant("Stop", discriminants[2], FieldsForm::Unit, vec![]),
        ];
        b.type_data(TypeData { name: "Tx".into(), path: "m::Tx".into(), origin: Types::origin(20), kind: TypeKind::Enum { tag_levels, variants }, derives: Derives { clone: true, copy: true, repr: true, ..Derives::default() } })
    }

    fn copyable() -> Derives {
        Derives { clone: true, copy: true, partial_eq: true, partial_ord: true, default: true, ..Derives::default() }
    }

    /// A design whose type arena `write` fills and whose locals have the types it returns.
    fn ir(write: impl FnOnce(&mut IrBuilder) -> Vec<ValueType>) -> Ir {
        Shape::ir(|b| {
            let types = write(b);
            let locals: Vec<Local> = types.into_iter().enumerate().map(|(i, ty)| Shape::local(&format!("v{i}"), ty)).collect();
            let value = Shape::dx(b, 30).real(0.0);
            Written { locals, stmts: vec![], value }
        })
    }
}

#[test]
fn the_type_arena_builds_checks_and_prints() {
    // §1.3.1 to §1.3.5: a struct with named fields, a tuple struct, a unit struct and an enum with
    // payloads, held by locals of every aggregate type; the printed arena names each shape.
    let ir = Types::ir(|b| {
        let pair = Types::named(b, "Pair", 3, &[("a", ValueType::F64), ("b", ValueType::Bool)], Types::copyable());
        let tuple_struct = b.type_data(TypeData {
            name: "Wrap".into(),
            path: "m::Wrap".into(),
            origin: Types::origin(5),
            kind: TypeKind::Struct { form: FieldsForm::Tuple, fields: vec![("0".into(), ValueType::UInt(8)), ("1".into(), ValueType::Struct(pair))] },
            derives: Derives::default(),
        });
        let unit = b.type_data(TypeData { name: "Marker".into(), path: "m::Marker".into(), origin: Types::origin(7), kind: TypeKind::Struct { form: FieldsForm::Unit, fields: vec![] }, derives: Derives::default() });
        let tx = Types::tx(b, 2, [0, 1, 2]);
        vec![
            ValueType::Struct(pair),
            ValueType::Struct(tuple_struct),
            ValueType::Struct(unit),
            ValueType::Enum(tx),
            ValueType::Tuple(vec![ValueType::UInt(8), ValueType::Bool]),
            ValueType::Array(Box::new(ValueType::F64), 3),
            ValueType::Option(Box::new(ValueType::Struct(pair))),
            ValueType::Array(Box::new(ValueType::Tuple(vec![ValueType::Enum(tx), ValueType::Option(Box::new(ValueType::F64))])), 4),
        ]
    });
    assert_eq!(ir.check(), vec![], "a well-formed arena checks");
    assert_eq!(ir.types().len(), 4);
    assert_eq!(ir.type_data(TypeId(0)).name, "Pair");
    let text = ir.to_string();
    for shape in [
        "type t0 = struct Pair { a: f64, b: bool }",
        "type t1 = struct Wrap(u8, t0)",
        "type t2 = struct Marker  path \"m::Marker\"",
        "type t3 = enum Tx [tag 2] { Idle = 0, Data { n: u3 } = 1, Stop = 2 }",
        "derive Clone, Copy, Repr",
        "local v4: (u8, bool)",
        "local v5: [f64; 3]",
        "local v6: Option<t0>",
        "local v7: [(t3, Option<f64>); 4]",
    ] {
        assert!(text.contains(shape), "the printed IR holds `{shape}`:\n{text}");
    }
}

#[test]
fn a_type_that_contains_itself_is_refused() {
    // §1.3.8: directly, through an array, through an `Option`, and through another struct.
    let direct = Types::ir(|b| {
        let node = b.type_data(TypeData { name: "Node".into(), path: "m::Node".into(), origin: Types::origin(3), kind: TypeKind::Struct { form: FieldsForm::Named, fields: vec![("next".into(), ValueType::Struct(TypeId(0)))] }, derives: Derives::default() });
        vec![ValueType::Struct(node)]
    });
    let broken = direct.check();
    assert_eq!(Entries::with(&broken, "contains itself").len(), 1, "{broken:?}");
    let around = Types::ir(|b| {
        let first = b.type_data(TypeData { name: "A".into(), path: "m::A".into(), origin: Types::origin(3), kind: TypeKind::Struct { form: FieldsForm::Named, fields: vec![("b".into(), ValueType::Array(Box::new(ValueType::Option(Box::new(ValueType::Struct(TypeId(1))))), 2))] }, derives: Derives::default() });
        Types::named(b, "B", 4, &[("a", ValueType::Tuple(vec![ValueType::Bool, ValueType::Struct(TypeId(0))]))], Derives::default());
        vec![ValueType::Struct(first)]
    });
    let broken = around.check();
    assert_eq!(Entries::with(&broken, "contains itself").len(), 2, "both types of the cycle are named: {broken:?}");
}

#[test]
fn an_enum_tag_holds_its_discriminants() {
    // §1.3.7 and §2 rule 5: a discriminant outside the tag, a repeated one, and a tag of no levels.
    let too_wide = Types::ir(|b| vec![ValueType::Enum(Types::tx(b, 2, [0, 1, 4]))]);
    assert_eq!(Entries::with(&too_wide.check(), "does not fit a 2-level tag").len(), 1);
    let repeated = Types::ir(|b| vec![ValueType::Enum(Types::tx(b, 2, [0, 1, 1]))]);
    assert_eq!(Entries::with(&repeated.check(), "two variants have the discriminant 1").len(), 1);
    let negative = Types::ir(|b| vec![ValueType::Enum(Types::tx(b, 2, [-1, 1, 2]))]);
    assert_eq!(Entries::with(&negative.check(), "does not fit a 2-level tag").len(), 1);
    let empty_tag = Types::ir(|b| vec![ValueType::Enum(Types::tx(b, 0, [0, 1, 2]))]);
    assert_eq!(Entries::with(&empty_tag.check(), "outside 1..=64").len(), 1);
}

#[test]
fn derives_imply_what_h8_says() {
    // H.8 rules 1 to 4: `Copy` needs `Clone`, `Ord` needs `Eq` and `PartialOrd`, an enum has no `Default`.
    let ir = Types::ir(|b| {
        let copy_only = Types::named(b, "C", 3, &[("a", ValueType::Bool)], Derives { copy: true, ..Derives::default() });
        let ord_only = Types::named(b, "O", 4, &[("a", ValueType::Bool)], Derives { ord: true, ..Derives::default() });
        vec![ValueType::Struct(copy_only), ValueType::Struct(ord_only)]
    });
    let broken = ir.check();
    assert_eq!(Entries::with(&broken, "derive Copy requires Clone").len(), 1, "{broken:?}");
    assert_eq!(Entries::with(&broken, "derive Ord requires Eq").len(), 1, "{broken:?}");
    assert_eq!(Entries::with(&broken, "derive Ord requires PartialOrd").len(), 1, "{broken:?}");
    let default_enum = Types::ir(|b| {
        let tx = b.type_data(TypeData {
            name: "E".into(),
            path: "m::E".into(),
            origin: Types::origin(3),
            kind: TypeKind::Enum { tag_levels: 1, variants: vec![VariantData { name: "A".into(), discriminant: 0, form: FieldsForm::Unit, fields: vec![] }] },
            derives: Derives { default: true, ..Derives::default() },
        });
        vec![ValueType::Enum(tx)]
    });
    assert_eq!(Entries::with(&default_enum.check(), "an enum does not derive Default").len(), 1);
}

#[test]
fn a_malformed_aggregate_type_is_refused() {
    // A struct id outside the arena, a struct id that names an enum, a slice held by an aggregate,
    // a field list that disagrees with its form, and two fields of one name.
    let outside = Types::ir(|_| vec![ValueType::Struct(TypeId(9))]);
    assert_eq!(Entries::with(&outside.check(), "outside the arena").len(), 1);
    let confused = Types::ir(|b| vec![ValueType::Struct(Types::tx(b, 2, [0, 1, 2]))]);
    assert_eq!(Entries::with(&confused.check(), "names an enum as a struct").len(), 1);
    let sliced = Types::ir(|_| vec![ValueType::Array(Box::new(ValueType::Slice(Box::new(ValueType::F64))), 2)]);
    assert_eq!(Entries::with(&sliced.check(), "a slice is a function parameter only").len(), 1);
    let unit_with_fields = Types::ir(|b| {
        let id = b.type_data(TypeData { name: "U".into(), path: "m::U".into(), origin: Types::origin(3), kind: TypeKind::Struct { form: FieldsForm::Unit, fields: vec![("x".into(), ValueType::Bool)] }, derives: Derives::default() });
        vec![ValueType::Struct(id)]
    });
    assert_eq!(Entries::with(&unit_with_fields.check(), "a unit form has no fields").len(), 1);
    let doubled = Types::ir(|b| vec![ValueType::Struct(Types::named(b, "D", 3, &[("x", ValueType::Bool), ("x", ValueType::F64)], Derives::default()))]);
    assert_eq!(Entries::with(&doubled.check(), "two fields are named x").len(), 1);
}
