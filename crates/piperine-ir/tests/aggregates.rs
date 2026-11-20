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

use digital::Ty;
use piperine_ir::{AggregateData, AggregateId, BinaryOp, Constant, Derives, Expr, ExprId, FieldsForm, FunctionId, FunctionSpec, Ir, IrBuilder, LevelPart, Local, Origin, Param, Place, Projection, Real, Stmt, StmtId, TypeData, TypeId, TypeKind, ValueType, VariantData};
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
    // §2 rule 5: an enum of one variant has a tag of zero levels
    let one_variant = Types::ir(|b| {
        let only = b.type_data(TypeData {
            name: "One".into(),
            path: "m::One".into(),
            origin: Types::origin(3),
            kind: TypeKind::Enum { tag_levels: 0, variants: vec![VariantData { name: "Only".into(), discriminant: 0, form: FieldsForm::Unit, fields: vec![] }] },
            derives: Derives::default(),
        });
        vec![ValueType::Enum(only)]
    });
    assert_eq!(one_variant.check(), vec![], "a one-variant enum has a zero-level tag");
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

// ---- T31: aggregate expressions and indexed places (CV-20) ------------------------------------------------------

/// The aggregate expressions of one small design, built over a shared arena.
struct Forms;

impl Forms {
    fn e(b: &mut IrBuilder, kind: Expr, ty: ValueType, line: u32) -> ExprId {
        Shape::expr(b, kind, ty, line)
    }

    fn pair(b: &mut IrBuilder) -> TypeId {
        Types::named(b, "Pair", 3, &[("a", ValueType::F64), ("b", ValueType::Bool)], Types::copyable())
    }

    /// A design whose body evaluates `stmts` and holds `locals`.
    fn design(locals: Vec<Local>, write: impl FnOnce(&mut IrBuilder) -> Vec<StmtId>) -> Ir {
        Shape::ir(|b| {
            let stmts = write(b);
            let value = Shape::dx(b, 40).real(0.0);
            Written { locals, stmts, value }
        })
    }

    /// `xs.len()` as the function `total(xs: &[f64]) -> u64`.
    fn total(b: &mut IrBuilder) -> FunctionId {
        let slice = ValueType::Slice(Box::new(ValueType::F64));
        let xs = Shape::local("xs", slice.clone());
        let read = Forms::e(b, Expr::Local(0), slice, 9);
        let body = Forms::e(b, Expr::Length(read), ValueType::UInt(64), 9);
        b.function(FunctionSpec { name: "total".into(), origin: Types::origin(9), params: vec![xs.clone()], ret: ValueType::UInt(64), locals: vec![xs], body })
    }
}

#[test]
fn ir_check_types_every_aggregate_expression() {
    // §1.5.25: tuples, arrays, struct and variant expressions, field and element reads, a view of
    // an array passed to a function, `len`, and an assignment through an element and a field.
    let array_type = ValueType::Array(Box::new(ValueType::F64), 3);
    let ir = Forms::design(vec![Shape::mutable("a", array_type.clone())], |b| {
        let pair = Forms::pair(b);
        let tx = Types::tx(b, 2, [0, 1, 2]);
        let (one, yes) = (Shape::dx(b, 2).real(1.0), Shape::dx(b, 2).boolean(true));
        let tuple = Forms::e(b, Expr::Tuple(vec![one, yes]), ValueType::Tuple(vec![ValueType::F64, ValueType::Bool]), 3);
        let first = Forms::e(b, Expr::Field { expr: tuple, index: 0 }, ValueType::F64, 3);
        let array = Forms::e(b, Expr::Array(vec![one, first, one]), array_type.clone(), 4);
        let declare = Shape::dx(b, 4).let_(0, array);
        let repeated = Forms::e(b, Expr::ArrayRepeat { element: one, count: 3 }, array_type.clone(), 5);
        let index = Shape::dx(b, 5).int(1, Ty::u(8));
        let element = Forms::e(b, Expr::Index { expr: repeated, index }, ValueType::F64, 5);
        let read = Forms::e(b, Expr::Local(0), array_type.clone(), 6);
        let view = Forms::e(b, Expr::SliceOf { expr: read, low: Some(index), high: None, inclusive: false }, ValueType::Slice(Box::new(ValueType::F64)), 6);
        let total = Forms::total(b);
        let call = Forms::e(b, Expr::Call { function: total, args: vec![view] }, ValueType::UInt(64), 6);
        let built = Forms::e(b, Expr::StructOf { ty: pair, fields: vec![element, yes] }, ValueType::Struct(pair), 7);
        let flag = Forms::e(b, Expr::Field { expr: built, index: 1 }, ValueType::Bool, 7);
        let three = Shape::dx(b, 8).int(3, Ty::u(3));
        let variant = Forms::e(b, Expr::VariantOf { ty: tx, variant: 1, fields: vec![three] }, ValueType::Enum(tx), 8);
        let set = b.stmt(Stmt::Assign { place: Place { local: 0, path: vec![Projection::Index(index)] }, op: None, value: one });
        let add = b.stmt(Stmt::Assign { place: Place { local: 0, path: vec![Projection::Index(index)] }, op: Some(BinaryOp::Add), value: one });
        let mut stmts = vec![declare, set, add];
        stmts.extend([call, flag, variant].map(|e| b.stmt(Stmt::Expr(e))));
        stmts
    });
    assert_eq!(ir.check(), vec![], "well-typed aggregate expressions check");
    let text = ir.to_string();
    for form in ["array_repeat", "struct_of t0", "variant_of t1 v1", "slice_of", "length e", "index e", "field e", "tuple (e"] {
        assert!(text.contains(form), "the arena lines name `{form}`:\n{text}");
    }
    for form in ["Pair(", "Tx::Data(", ".len()", "total(a[e6..])", "a[1u8] = e0", "a[e6] += e0"] {
        assert!(text.contains(form), "the tree text writes `{form}`:\n{text}");
    }
}

#[test]
fn ill_typed_aggregate_expressions_are_refused() {
    // Each mistake of §1.5.25 and §1.3: a part of the wrong type or count, a variant out of range,
    // a field out of range, an index that is not an integer, `len` of a scalar, a slice stored in a
    // `let`, an assignment through a field of an array.
    let array_type = ValueType::Array(Box::new(ValueType::F64), 2);
    let ir = Forms::design(vec![Shape::mutable("a", array_type.clone()), Shape::local("s", ValueType::Slice(Box::new(ValueType::F64)))], |b| {
        let pair = Forms::pair(b);
        let tx = Types::tx(b, 2, [0, 1, 2]);
        let (one, yes) = (Shape::dx(b, 2).real(1.0), Shape::dx(b, 2).boolean(true));
        let wrong_tuple = Forms::e(b, Expr::Tuple(vec![one, yes]), ValueType::Tuple(vec![ValueType::F64, ValueType::F64]), 3);
        let short_array = Forms::e(b, Expr::Array(vec![one]), array_type.clone(), 4);
        let mixed_array = Forms::e(b, Expr::Array(vec![one, yes]), array_type.clone(), 5);
        let swapped = Forms::e(b, Expr::StructOf { ty: pair, fields: vec![yes, one] }, ValueType::Struct(pair), 6);
        let missing = Forms::e(b, Expr::StructOf { ty: pair, fields: vec![one] }, ValueType::Struct(pair), 7);
        let nowhere = Forms::e(b, Expr::VariantOf { ty: tx, variant: 7, fields: vec![] }, ValueType::Enum(tx), 8);
        let tuple = Forms::e(b, Expr::Tuple(vec![one, yes]), ValueType::Tuple(vec![ValueType::F64, ValueType::Bool]), 9);
        let beyond = Forms::e(b, Expr::Field { expr: tuple, index: 5 }, ValueType::F64, 9);
        let read = Forms::e(b, Expr::Local(0), array_type.clone(), 10);
        let real_index = Forms::e(b, Expr::Index { expr: read, index: one }, ValueType::F64, 10);
        let length_of_scalar = Forms::e(b, Expr::Length(one), ValueType::UInt(64), 11);
        let slice_type = ValueType::Slice(Box::new(ValueType::F64));
        let view = Forms::e(b, Expr::SliceOf { expr: read, low: None, high: None, inclusive: false }, slice_type, 12);
        let stored = Shape::dx(b, 12).let_(1, view);
        let index = Shape::dx(b, 13).int(0, Ty::u(8));
        let through_field = b.stmt(Stmt::Assign { place: Place { local: 0, path: vec![Projection::Field(0)] }, op: None, value: one });
        let wrong_value = b.stmt(Stmt::Assign { place: Place { local: 0, path: vec![Projection::Index(index)] }, op: None, value: yes });
        let mut stmts = vec![stored, through_field, wrong_value];
        stmts.extend([wrong_tuple, short_array, mixed_array, swapped, missing, nowhere, beyond, real_index, length_of_scalar].map(|e| b.stmt(Stmt::Expr(e))));
        stmts
    });
    let broken = ir.check();
    for (phrase, count) in [
        ("expression is typed (f64, f64), its form gives (f64, bool)", 1),
        ("an array of 2 elements is written with 1", 1),
        ("an element of an array must be f64, found bool", 1),
        ("a field of a struct expression must be f64, found bool", 1),
        ("a field of a struct expression takes 2 parts, found 1", 1),
        ("variant 7 out of range", 1),
        ("field 5 of a value of type (f64, bool), which has no such field", 1),
        ("an index must have an integer type, found f64", 1),
        ("`len` of a value of type f64", 1),
        ("a slice stands only as a call argument or a method receiver", 1),
        ("a projection of local 0 reaches into a value of type [f64; 2]", 1),
        ("the value of an assignment must be f64, found bool", 1),
    ] {
        assert_eq!(Entries::with(&broken, phrase).len(), count, "`{phrase}` in {broken:#?}");
    }
}

// ---- T32: variant tests, `Option` packing and defaults (CV-11, CV-12, CV-20) -------------------------------------

#[test]
fn variant_tests_option_packs_and_comparisons_check() {
    // §1.3.4 to §1.3.10, H.8: a variant test and a variant field read, an `Option` built from its
    // pair and read back, `default`, and `==` and `<` on aggregates the derives allow.
    let ir = Forms::design(vec![], |b| {
        let pair = Forms::pair(b);
        let tx = Types::tx(b, 2, [0, 1, 2]);
        let (one, yes) = (Shape::dx(b, 2).real(1.0), Shape::dx(b, 2).boolean(true));
        let three = Shape::dx(b, 3).int(3, Ty::u(3));
        let data = Forms::e(b, Expr::VariantOf { ty: tx, variant: 1, fields: vec![three] }, ValueType::Enum(tx), 3);
        let is_data = Forms::e(b, Expr::IsVariant { expr: data, variant: 1 }, ValueType::Bool, 4);
        let payload = Forms::e(b, Expr::VariantField { expr: data, variant: 1, field: 0 }, ValueType::UInt(3), 4);
        let packed = Forms::e(b, Expr::OptionPack { is: yes, value: one }, ValueType::Option(Box::new(ValueType::F64)), 5);
        let is = Forms::e(b, Expr::OptionPart { option: packed, part: LevelPart::Is }, ValueType::Bool, 5);
        let value = Forms::e(b, Expr::OptionPart { option: packed, part: LevelPart::Value }, ValueType::F64, 5);
        let fresh = Forms::e(b, Expr::Default(ValueType::Struct(pair)), ValueType::Struct(pair), 6);
        let built = Forms::e(b, Expr::StructOf { ty: pair, fields: vec![one, yes] }, ValueType::Struct(pair), 6);
        let equal = Forms::e(b, Expr::Binary { op: BinaryOp::Eq, lhs: fresh, rhs: built }, ValueType::Bool, 7);
        let ordered = Forms::e(b, Expr::Binary { op: BinaryOp::Lt, lhs: fresh, rhs: built }, ValueType::Bool, 7);
        let tuple = Forms::e(b, Expr::Tuple(vec![one, yes]), ValueType::Tuple(vec![ValueType::F64, ValueType::Bool]), 8);
        let same_tuple = Forms::e(b, Expr::Binary { op: BinaryOp::Ne, lhs: tuple, rhs: tuple }, ValueType::Bool, 8);
        let same_option = Forms::e(b, Expr::Binary { op: BinaryOp::Eq, lhs: packed, rhs: packed }, ValueType::Bool, 8);
        [is_data, payload, is, value, equal, ordered, same_tuple, same_option].map(|e| b.stmt(Stmt::Expr(e))).to_vec()
    });
    assert_eq!(ir.check(), vec![], "well-typed variant, option and comparison forms check");
    let text = ir.to_string();
    for form in ["is_variant e", "variant_field e", "option_pack e", "option_part e", "default t0"] {
        assert!(text.contains(form), "the arena lines name `{form}`:\n{text}");
    }
    assert!(text.contains("matches!(Tx::Data(3u3), Tx::Data)"), "the tree text writes the variant test:\n{text}");
}

#[test]
fn aggregate_forms_the_derives_do_not_allow_are_refused() {
    // §1.3.10 and H.8: `==` on a type without `PartialEq`, `<` without `PartialOrd`, two types
    // compared, arithmetic on aggregates, `default` of an enum and of a type without `Default`,
    // a variant test of a non-enum, and a derive whose fields lack the trait (rule 6).
    let ir = Forms::design(vec![], |b| {
        let pair = Forms::pair(b);
        let tx = Types::tx(b, 2, [0, 1, 2]);
        let bare = Types::named(b, "Bare", 5, &[("a", ValueType::Bool)], Derives::default());
        Types::named(b, "Holder", 6, &[("t", ValueType::Enum(tx))], Derives { partial_eq: true, ..Derives::default() });
        let (one, yes) = (Shape::dx(b, 2).real(1.0), Shape::dx(b, 2).boolean(true));
        let tuple = Forms::e(b, Expr::Tuple(vec![one, yes]), ValueType::Tuple(vec![ValueType::F64, ValueType::Bool]), 8);
        let made = Forms::e(b, Expr::StructOf { ty: bare, fields: vec![yes] }, ValueType::Struct(bare), 9);
        let no_eq = Forms::e(b, Expr::Binary { op: BinaryOp::Eq, lhs: made, rhs: made }, ValueType::Bool, 9);
        let no_ord = Forms::e(b, Expr::Binary { op: BinaryOp::Lt, lhs: made, rhs: made }, ValueType::Bool, 10);
        let built = Forms::e(b, Expr::StructOf { ty: pair, fields: vec![one, yes] }, ValueType::Struct(pair), 11);
        let mixed = Forms::e(b, Expr::Binary { op: BinaryOp::Eq, lhs: built, rhs: tuple }, ValueType::Bool, 11);
        let sum = Forms::e(b, Expr::Binary { op: BinaryOp::Add, lhs: built, rhs: built }, ValueType::Struct(pair), 12);
        let enum_default = Forms::e(b, Expr::Default(ValueType::Enum(tx)), ValueType::Enum(tx), 13);
        let bare_default = Forms::e(b, Expr::Default(ValueType::Struct(bare)), ValueType::Struct(bare), 14);
        let scalar_variant = Forms::e(b, Expr::IsVariant { expr: one, variant: 0 }, ValueType::Bool, 15);
        let missing = Forms::e(b, Expr::OptionPart { option: one, part: LevelPart::Is }, ValueType::Bool, 16);
        [no_eq, no_ord, mixed, sum, enum_default, bare_default, scalar_variant, missing].map(|e| b.stmt(Stmt::Expr(e))).to_vec()
    });
    let broken = ir.check();
    for (phrase, count) in [
        ("on t2, which does not implement PartialEq", 1),
        ("on t2, which does not implement PartialOrd", 1),
        ("compares t0 with (f64, bool)", 1),
        ("`Add` does not apply to aggregates", 1),
        ("`default` of type t1, which does not implement Default", 1),
        ("`default` of type t2, which does not implement Default", 1),
        ("a variant of a value of type f64, which is not an enum", 1),
        ("an option part of a value of type f64, which is not an Option", 1),
        ("derive PartialEq requires every field to implement it (H.8 rule 6)", 1),
    ] {
        assert_eq!(Entries::with(&broken, phrase).len(), count, "`{phrase}` in {broken:#?}");
    }
}

// ---- T33: composite constants (CV-18, CV-20) -------------------------------------------------------------------

/// Composite constants of one design: a param whose default is a struct that holds a struct and an array.
struct Constants;

impl Constants {
    /// `Pair { a: 1.5, b: true }` (agg0), `[1, 2, 3]` (agg1) and `Line { pair, taps }` (agg2) as the
    /// default of the param `line`; `mutate` changes the last one before it is added.
    fn ir(mutate: impl FnOnce(&mut AggregateData)) -> Ir {
        Shape::ir_with_params(|b| {
            let pair = Forms::pair(b);
            let taps = ValueType::Array(Box::new(ValueType::UInt(8)), 3);
            let line = Types::named(b, "Line", 4, &[("pair", ValueType::Struct(pair)), ("taps", taps.clone())], Derives::default());
            let made_pair = b.aggregate(AggregateData { ty: ValueType::Struct(pair), variant: None, parts: vec![Constant::Real(Real::new(1.5)), Constant::Bool(true)] });
            let made_taps = b.aggregate(AggregateData { ty: taps, variant: None, parts: vec![Constant::Int(1), Constant::Int(2), Constant::Int(3)] });
            let mut data = AggregateData { ty: ValueType::Struct(line), variant: None, parts: vec![Constant::Aggregate(made_pair), Constant::Aggregate(made_taps)] };
            mutate(&mut data);
            let made_line = b.aggregate(data);
            let param = Param { name: "line".into(), ty: ValueType::Struct(line), default: Some(Constant::Aggregate(made_line)), check: None, structural: false, origin: Types::origin(6) };
            let value = Shape::dx(b, 40).real(0.0);
            (Written { locals: vec![], stmts: vec![], value }, vec![param])
        })
    }
}

#[test]
fn composite_constants_check_print_and_round_trip() {
    // §1.5.25: a struct constant holding another and an array, as a param default; it checks, prints
    // spelled out, survives a JSON round trip, and each way of breaking it is named.
    let ir = Constants::ir(|_| {});
    assert_eq!(ir.check(), vec![], "a well-formed composite constant checks");
    assert_eq!(ir.aggregates().len(), 3);
    let text = ir.to_string();
    for line in ["aggregate agg0: t0 = Pair(1.5, true)", "aggregate agg1: [u8; 3] = [1, 2, 3]", "aggregate agg2: t1 = Line(Pair(1.5, true), [1, 2, 3])"] {
        assert!(text.contains(line), "the printed IR holds `{line}`:\n{text}");
    }
    let json = serde_json::to_string(&ir).expect("serialize");
    let back: Ir = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, ir, "a JSON round trip gives an equal IR");
    assert_eq!(back.to_string(), text);

    let cases: Vec<(Ir, &str)> = vec![
        (Constants::ir(|data| data.parts.truncate(1)), "the constant of type t1 has 2 parts, found 1"),
        (Constants::ir(|data| data.parts[1] = Constant::Bool(true)), "part 1 of a composite constant is true, not a value of [u8; 3]"),
        (Constants::ir(|data| data.variant = Some(0)), "only an enum constant names a variant"),
        (Constants::ir(|data| data.parts[0] = Constant::Aggregate(AggregateId(2))), "a composite constant contains itself"),
        (Constants::ir(|data| data.ty = ValueType::Struct(TypeId(0))), "the default of param line"),
    ];
    for (broken, phrase) in cases {
        let found = broken.check();
        assert!(!Entries::with(&found, phrase).is_empty(), "`{phrase}` in {found:#?}");
    }
}
