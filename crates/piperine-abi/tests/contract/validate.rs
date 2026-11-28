//! Shape-local validation (feature `solver-abi-core`, SAC-09, abi.md §9):
//! each of the fifteen rules refuses one planted defect in the resistor
//! (§11.1) or the D flip-flop (§11.4), naming the rule, the entry and its
//! origin; a zeroed entry is an unknown constant; an index is checked before
//! it is followed; the valid fixtures pass.

use piperine_abi::layout::{BIT, BYTE, F64, LOW};
use piperine_abi::raw::{
    Shape, Cell, DEFAULT, Partial, Statement, ambient, cell, fn_flag, global, input, kind, op, part, pin_flag, role, watch,
};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::validate::{Rule, Validator};

/// The specs of a shape, mutable before it is built.
#[derive(Clone, Default)]
struct Specs {
    pins: Vec<PinSpec<'static>>,
    terms: Vec<TermSpec<'static>>,
    inputs: Vec<InputSpec<'static>>,
    watches: Vec<WatchSpec<'static>>,
    functions: Vec<FunctionSpec<'static>>,
    reactions: Vec<ReactionSpec<'static>>,
}

impl Specs {
    fn build(self) -> ShapeRef {
        let mut s = ShapeBuf::new();
        for p in self.pins {
            s.pin(p);
        }
        for t in self.terms {
            s.term(t);
        }
        for i in self.inputs {
            s.input(i);
        }
        for w in self.watches {
            s.watch(w);
        }
        for f in self.functions {
            s.function(f);
        }
        for r in self.reactions {
            s.reaction(r);
        }
        s.freeze()
    }
}

const R_DEFAULT: [u8; 8] = [0, 0, 0, 0, 0, 0x40, 0x8f, 0x40];

/// (rule, a defect planted in the raw shape, the entry the error names, its origin)
type RawRow = (Rule, fn(&mut Shape), &'static str, &'static str);

/// (rule, the defect, the entry the error names, its origin)
type Row = (Rule, fn() -> Specs, &'static str, &'static str);

#[test]
fn every_shape_local_rule_is_enforced() {
    let table: [Row; 22] = [
        (Rule::UnknownConstant, || { let mut s = CheckedShapes::resistor(); s.terms[0].op = 9; s }, "term i", "res.msl:3:5"),
        (Rule::IndexOutOfRange, || { let mut s = CheckedShapes::resistor(); s.functions[0].reads[1].index = 7; s }, "function i", "res.msl:3:9"),
        (Rule::KindRole, || {
            let mut s = CheckedShapes::resistor();
            s.pins[1].role = role::GLOBAL;
            s.pins[1].global = global::TEMPERATURE;
            s
        }, "pin p", "res.msl:1:17"),
        (Rule::PartialNotF64, || { let mut s = CheckedShapes::resistor(); s.functions[0].partials[2].at = 8; s }, "function i", "res.msl:3:9"),
        (Rule::TermTargetNotContinuous, || { let mut s = CheckedShapes::resistor(); s.terms[0].plus = 0; s }, "term i", "res.msl:3:5"),
        (Rule::MixedSorts, || { let mut s = CheckedShapes::resistor(); s.functions[0].writes.push(CheckedShapes::pin_cell(0, part::VALUE)); s }, "function i", "res.msl:3:9"),
        (Rule::DiscreteReadsAnalog, || {
            let mut s = CheckedShapes::flip_flop();
            s.functions[0].reads.push(Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE });
            s
        }, "function rise", "dff.msl:2:5"),
        (Rule::ChangedReadByWriter, || { let mut s = CheckedShapes::flip_flop(); s.functions[0].writes = vec![CheckedShapes::pin_cell(2, part::VALUE)]; s }, "function rise", "dff.msl:2:5"),
        (Rule::LimitCorrection, || CheckedShapes::resistor_writing_a_limit(0), "function i", "res.msl:3:9"),
        (Rule::QuietRead, || { let mut s = CheckedShapes::flip_flop(); s.pins[0].flags = pin_flag::QUIET; s }, "function rise", "dff.msl:2:5"),
        (Rule::ReactionOnNonEvent, || { let mut s = CheckedShapes::flip_flop(); s.reactions[0].on = vec![1]; s }, "reaction sample", "dff.msl:3:5"),
        (Rule::ParamAfterNonParam, || { let mut s = CheckedShapes::resistor(); s.pins.rotate_left(1); s.terms[0].plus = 0; s.terms[0].minus = 1; s }, "pin r", "res.msl:2:5"),
        (Rule::StatementOnContinuous, || {
            let mut s = CheckedShapes::flip_flop();
            s.pins.push(PinSpec { origin: "dff.msl:4:5", ..PinSpec::continuous("a", role::PRIVATE) });
            s.reactions[0].writes[0].pin = 4;
            s
        }, "reaction sample", "dff.msl:3:5"),
        (Rule::CrossEventNotEvent, || {
            let mut s = CheckedShapes::flip_flop();
            s.watches.push(WatchSpec {
                name: "up",
                origin: "dff.msl:5:5",
                kind: watch::CROSS,
                layout: &F64,
                event: 1,
                ttol: DEFAULT,
                etol: DEFAULT,
            });
            s
        }, "watch up", "dff.msl:5:5"),
        (Rule::TermBoundNotDiscreteF64, || { let mut s = CheckedShapes::resistor(); s.terms[0].abstol = 1; s }, "term i", "res.msl:3:5"),
        (Rule::EventNotEmpty, || { let mut s = CheckedShapes::flip_flop(); s.pins[3].layout = &BYTE; s }, "pin rise", "dff.msl:2:5"),
        (Rule::ContinuousNotF64, || { let mut s = CheckedShapes::resistor(); s.pins[1].layout = &BYTE; s }, "pin p", "res.msl:1:17"),
        (Rule::CheckNotBool, || {
            let mut s = CheckedShapes::resistor();
            s.pins[0].flags |= pin_flag::HAS_CHECK;
            s.pins[0].check = 1;
            s
        }, "pin r", "res.msl:2:5"),
        (Rule::QuietChangeRead, || {
            let mut s = CheckedShapes::flip_flop();
            s.pins[1].flags = pin_flag::QUIET;
            s.reactions[0].reads = vec![CheckedShapes::pin_cell(1, part::PREVIOUS)];
            s
        }, "reaction sample", "dff.msl:3:5"),
        (Rule::ArrayNotDiscrete, || { let mut s = CheckedShapes::resistor(); s.pins[1].count = 4; s }, "pin p", "res.msl:1:17"),
        (Rule::ArrayInitLength, || {
            let mut s = CheckedShapes::flip_flop();
            s.pins[2].count = 4;
            s.pins[2].flags = pin_flag::HAS_INIT;
            s.pins[2].init = &[0, 0, 0];
            s
        }, "pin q", "dff.msl:1:18"),
        (Rule::ElementOutOfRange, || {
            let mut s = CheckedShapes::flip_flop();
            s.pins[1].count = 4;
            s.reactions[0].elements = vec![4];
            s
        }, "reaction sample", "dff.msl:3:5"),
    ];
    // Defects below the specs (a list read by a stride the producer did not use).
    let raw_rows: [RawRow; 1] = [(
        Rule::UnreadableList,
        // SAFETY: the same live pointer and length, read by a stride that is refused.
        |s| s.terms = unsafe { piperine_abi::raw::List::from_raw_parts(s.terms.as_ptr(), s.terms.len(), 8) },
        "shape",
        "",
    )];
    for rule in Rule::ALL {
        let rows = table.iter().filter(|row| row.0 == rule).count() + raw_rows.iter().filter(|row| row.0 == rule).count();
        assert_eq!(rows, 1, "one row for {rule:?}");
    }
    for (rule, plant, entry, origin) in raw_rows {
        let built = CheckedShapes::resistor().build();
        let mut shape = *built.shape();
        plant(&mut shape);
        let err = Validator::validate(&shape).expect_err(&format!("{rule:?} is not refused"));
        assert_eq!((err.rule, err.entry.as_str(), err.origin.as_str()), (rule, entry, origin));
    }
    for (rule, plant, entry, origin) in table {
        let shape = plant().build();
        let err = Validator::validate(shape.shape()).expect_err(&format!("{rule:?} is not refused"));
        assert_eq!(err.rule, rule, "the planted {rule:?} defect is reported as {err}");
        assert_eq!(err.entry, entry, "{rule:?}");
        assert_eq!(err.origin, origin, "{rule:?}");
        assert!(err.to_string().contains(origin) && err.to_string().contains(entry));
    }

    // A3, the second half: with OWNS_CORRECTION, a partial against that limit.
    let mut s = CheckedShapes::resistor_writing_a_limit(fn_flag::OWNS_CORRECTION);
    assert!(Validator::validate(s.clone().build().shape()).is_ok(), "OWNS_CORRECTION and no partial against the limit is legal");
    s.functions[0].reads.push(Cell { kind: cell::INPUT, index: 0, part: part::VALUE });
    s.functions[0].partials.push(Partial { write: 0, to: 0, read: 3, at: 0 });
    let err = Validator::validate(s.build().shape()).expect_err("a partial against the owned limit");
    assert_eq!((err.rule, err.entry.as_str()), (Rule::LimitCorrection, "function i"));
}

#[test]
fn a_zeroed_shape_entry_is_an_unknown_constant() {
    let zero_pin = PinSpec { kind: 0, role: 0, ..PinSpec::event("", role::PRIVATE) };
    let mut s = ShapeBuf::new();
    s.pin(zero_pin);
    let err = Validator::validate(s.freeze().shape()).expect_err("a zeroed pin");
    assert_eq!(err.rule, Rule::UnknownConstant);

    let mut specs = CheckedShapes::resistor();
    specs.terms[0] = TermSpec { name: "", origin: "", op: 0, plus: 0, minus: 0, abstol: 0, max: 0 };
    let err = Validator::validate(specs.build().shape()).expect_err("a zeroed term");
    assert_eq!(err.rule, Rule::UnknownConstant);

    let mut specs = CheckedShapes::resistor();
    specs.functions[0].reads[0] = Cell { kind: 0, index: 0, part: 0 };
    let err = Validator::validate(specs.build().shape()).expect_err("a zeroed cell");
    assert_eq!((err.rule, err.entry.as_str()), (Rule::UnknownConstant, "function i"));

    let mut specs = CheckedShapes::flip_flop();
    specs.reactions[0].writes[0] = Statement { pin: 2, offset: 0, len: 1, flags: 0x80, min_after: 0.0 };
    let err = Validator::validate(specs.build().shape()).expect_err("an unknown statement flag");
    assert_eq!((err.rule, err.entry.as_str()), (Rule::UnknownConstant, "reaction sample"));
}

#[test]
fn an_index_is_checked_before_it_is_followed() {
    for plant in [
        |s: &mut Specs| s.functions[0].reads[0].index = u32::MAX,
        |s: &mut Specs| s.functions[0].writes[0].index = u32::MAX,
        |s: &mut Specs| s.functions[0].partials[0].read = u32::MAX,
        |s: &mut Specs| s.terms[0].minus = u32::MAX - 2,
    ] {
        let mut specs = CheckedShapes::resistor();
        plant(&mut specs);
        let err = Validator::validate(specs.build().shape()).expect_err("an index of u32::MAX");
        assert_eq!(err.rule, Rule::IndexOutOfRange, "{err}");
    }
    for plant in [
        |s: &mut Specs| s.reactions[0].on[0] = u32::MAX,
        |s: &mut Specs| s.reactions[0].writes[0].pin = u32::MAX,
        |s: &mut Specs| s.reactions[0].reads[0].index = u32::MAX,
        |s: &mut Specs| s.reactions[0].writes[0].len = 3,
    ] {
        let mut specs = CheckedShapes::flip_flop();
        plant(&mut specs);
        let err = Validator::validate(specs.build().shape()).expect_err("an index out of range");
        assert_eq!((err.rule, err.entry.as_str()), (Rule::IndexOutOfRange, "reaction sample"), "{err}");
    }
}

#[test]
fn the_valid_fixtures_pass() {
    assert_eq!(Validator::validate(CheckedShapes::resistor().build().shape()), Ok(()));
    assert_eq!(Validator::validate(CheckedShapes::flip_flop().build().shape()), Ok(()));
    assert_eq!(Validator::validate(ShapeBuf::new().freeze().shape()), Ok(()), "an empty shape");
    // The legal forms of the three bound rules: a CROSS watch firing an event
    // pin, a term bound read from a discrete F64 pin, a statement on a discrete pin.
    let mut ff = CheckedShapes::flip_flop();
    ff.watches.push(WatchSpec {
        name: "up",
        origin: "dff.msl:5:5",
        kind: watch::CROSS,
        layout: &F64,
        event: 3,
        ttol: DEFAULT,
        etol: DEFAULT,
    });
    assert_eq!(Validator::validate(ff.build().shape()), Ok(()));
    let mut res = CheckedShapes::resistor();
    res.terms[0].abstol = 0;
    res.terms[0].max = 0;
    assert_eq!(Validator::validate(res.build().shape()), Ok(()));
    // A `check` naming a discrete BOOL pin, and a QUIET pin read by value only.
    let mut ff = CheckedShapes::flip_flop();
    ff.pins.push(PinSpec { flags: pin_flag::HAS_CHECK, check: 0, ..PinSpec::discrete("k", role::PRIVATE, &BIT) });
    ff.pins[1].flags = pin_flag::QUIET;
    assert_eq!(Validator::validate(ff.build().shape()), Ok(()));
    // The flip-flop's pins are of every discrete kind the rules look at.
    let ff = CheckedShapes::flip_flop().build();
    let kinds: Vec<u32> = ff.shape().pins.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, [kind::DISCRETE, kind::DISCRETE, kind::DISCRETE, kind::EVENT]);
}

#[test]
fn a_shape_with_an_unreadable_list_is_refused() {
    let built = CheckedShapes::resistor().build();
    let mut shape = *built.shape();
    // SAFETY: the shape's live pins, read by a stride below the element (refused).
    shape.pins = unsafe { piperine_abi::raw::List::from_raw_parts(shape.pins.as_ptr(), shape.pins.len(), size_of::<piperine_abi::raw::Pin>() - 8) };
    let err = Validator::validate(&shape).expect_err("a pin list read below its element size");
    assert_eq!(err.rule, Rule::UnreadableList);
    let mut shape = *built.shape();
    let mut f = *shape.functions.get(0).unwrap();
    // SAFETY: the function's live reads, read by a stride of 4 (refused).
    f.reads = unsafe { piperine_abi::raw::List::from_raw_parts(f.reads.as_ptr(), f.reads.len(), 4) };
    let fs = [f];
    shape.functions = piperine_abi::raw::List::from_slice(&fs);
    let err = Validator::validate(&shape).expect_err("a cell list read below its element size");
    assert_eq!((err.rule, err.entry.as_str()), (Rule::UnreadableList, "function i"));
}

#[test]
fn a_reaction_may_read_gmin_and_source_scale() {
    // ENV-01 (docs/spec Part IV §12.1.12, §14.4): a reaction and a discrete function read the
    // environment `gmin()` and `source_scale()` as the ambient inputs GMIN and SOURCE_SCALE.
    // FREQ stays refused in a reaction, TIME in a discrete function (it changes at every
    // instant and no event follows it).
    let ambient_cell = |index| Cell { kind: cell::AMBIENT, index, part: part::VALUE };
    for index in [ambient::GMIN, ambient::SOURCE_SCALE] {
        let mut s = CheckedShapes::flip_flop();
        s.reactions[0].reads.push(ambient_cell(index));
        Validator::validate(s.build().shape()).unwrap_or_else(|e| panic!("a reaction reading ambient {index}: {e}"));
        let mut s = CheckedShapes::flip_flop();
        s.functions[0].reads.push(ambient_cell(index));
        Validator::validate(s.build().shape()).unwrap_or_else(|e| panic!("a discrete function reading ambient {index}: {e}"));
    }
    let mut s = CheckedShapes::flip_flop();
    s.reactions[0].reads.push(ambient_cell(ambient::FREQ));
    let e = Validator::validate(s.build().shape()).expect_err("FREQ in a reaction");
    assert_eq!(e.rule, Rule::UnknownConstant, "{e}");
    let mut s = CheckedShapes::flip_flop();
    s.functions[0].reads.push(ambient_cell(ambient::TIME));
    assert_eq!(Validator::validate(s.build().shape()).expect_err("TIME in a discrete function").rule, Rule::DiscreteReadsAnalog);
}

/// The shapes the validation tests check: a resistor, a flip-flop and a resistor writing a limit.
struct CheckedShapes;

impl CheckedShapes {
    /// The pin cell helper `pin_cell`.
    fn pin_cell(index: u32, part: u32) -> Cell {
        Cell { kind: cell::PIN, index, part }
    }

     // 1e3f64, little-endian

    /// abi.md §11.1, `PARAM` first: pins r=0, p=1, n=2; term "i" FLOW {p, n};
    /// fn "i": reads [p, n, r] writes [T0] partials (0←0) (0←1) (0←2).
    fn resistor() -> Specs {
        Specs {
            pins: vec![
                PinSpec {
                    origin: "res.msl:2:5",
                    flags: pin_flag::HAS_INIT,
                    init: &R_DEFAULT,
                    ..PinSpec::discrete("r", role::PARAM, &F64)
                },
                PinSpec { origin: "res.msl:1:17", ..PinSpec::continuous("p", role::PORT) },
                PinSpec { origin: "res.msl:1:20", ..PinSpec::continuous("n", role::PORT) },
            ],
            terms: vec![TermSpec {
                name: "i",
                origin: "res.msl:3:5",
                op: op::FLOW,
                plus: 1,
                minus: 2,
                abstol: DEFAULT,
                max: DEFAULT,
            }],
            functions: vec![FunctionSpec {
                name: "i",
                origin: "res.msl:3:9",
                reads: vec![CheckedShapes::pin_cell(1, part::VALUE), CheckedShapes::pin_cell(2, part::VALUE), CheckedShapes::pin_cell(0, part::VALUE)],
                writes: vec![Cell { kind: cell::TERM, index: 0, part: part::VALUE }],
                partials: (0..3).map(|read| Partial { write: 0, to: 0, read, at: 0 }).collect(),
                ..FunctionSpec::default()
            }],
            ..Specs::default()
        }
    }

    /// abi.md §11.4: pins clk=0, d=1 (PORT BIT); q=2 (PORT BIT, init LOW);
    /// rise=3 (PRIVATE event). fn "rise": reads [clk.VALUE, clk.PREVIOUS,
    /// clk.CHANGED] writes [rise]. reaction "sample": on [rise] reads [d]
    /// writes [S0 {q}].
    fn flip_flop() -> Specs {
        Specs {
            pins: vec![
                PinSpec { origin: "dff.msl:1:10", ..PinSpec::discrete("clk", role::PORT, &BIT) },
                PinSpec { origin: "dff.msl:1:15", ..PinSpec::discrete("d", role::PORT, &BIT) },
                PinSpec {
                    origin: "dff.msl:1:18",
                    flags: pin_flag::HAS_INIT,
                    init: &LOW,
                    ..PinSpec::discrete("q", role::PORT, &BIT)
                },
                PinSpec { origin: "dff.msl:2:5", ..PinSpec::event("rise", role::PRIVATE) },
            ],
            functions: vec![FunctionSpec {
                name: "rise",
                origin: "dff.msl:2:5",
                reads: vec![CheckedShapes::pin_cell(0, part::VALUE), CheckedShapes::pin_cell(0, part::PREVIOUS), CheckedShapes::pin_cell(0, part::CHANGED)],
                writes: vec![CheckedShapes::pin_cell(3, part::CHANGED)],
                ..FunctionSpec::default()
            }],
            reactions: vec![ReactionSpec {
                name: "sample",
                origin: "dff.msl:3:5",
                on: vec![3],
                reads: vec![CheckedShapes::pin_cell(1, part::VALUE)],
                writes: vec![Statement { pin: 2, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
                ..ReactionSpec::default()
            }],
            ..Specs::default()
        }
    }

    /// A resistor whose function also writes a `LIMIT` input "vd".
    fn resistor_writing_a_limit(flags: u32) -> Specs {
        let mut s = CheckedShapes::resistor();
        s.inputs.push(InputSpec { name: "vd", origin: "res.msl:4:5", kind: input::LIMIT });
        s.functions[0].flags = flags;
        s.functions[0].writes.push(Cell { kind: cell::INPUT, index: 0, part: part::PROPOSED });
        s
    }
}
