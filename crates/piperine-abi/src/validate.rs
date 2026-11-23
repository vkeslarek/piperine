//! The shape-local rules of abi.md §9 (feature `solver-abi-core`, SAC-09):
//! the bind checks that need one shape and no net. Lists are read only
//! through `List::get`, every index is checked before it is followed, and no
//! constant is interpreted before it is known. The rules that need nets (one
//! writer, layout equality across a net, cycles, the parameter cone) belong
//! to the sub-solvers' bind.

use std::fmt;

use crate::layout::{F64, layout_eq, leaf_size};
use crate::raw::{
    Cell, List, WHOLE, DEFAULT, Function, GROUND, Layout, Pin, Reaction, Shape, Str, ambient, cell, fn_flag, global,
    input, kind, leaf, op, part, pin_flag, reaction_flag, role, statement_flag, watch,
};

/// One shape-local rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    /// A constant outside the set its field allows, or a part its cell does not have.
    UnknownConstant,
    /// An index past the end of the list it names.
    IndexOutOfRange,
    /// A pin kind and role pair outside abi.md §3.1.
    KindRole,
    /// A partial whose `to` or `at` is not the byte of an `F64`.
    PartialNotF64,
    /// A term whose `plus` or `minus` is not a continuous pin or `GROUND`.
    TermTargetNotContinuous,
    /// A function writing analog cells and pins (F2).
    MixedSorts,
    /// A discrete function reading an input or a continuous pin (F2, §4.2).
    DiscreteReadsAnalog,
    /// `CHANGED` read by a function that writes anything but event pins (§4.1).
    ChangedReadByWriter,
    /// A function writing a limit and a term without `OWNS_CORRECTION`, or
    /// with it and a partial against that limit (A3).
    LimitCorrection,
    /// A function reading a `QUIET` pin (P7).
    QuietRead,
    /// A reaction listening to a pin that is not an event pin (R1).
    ReactionOnNonEvent,
    /// A `PARAM` pin after a pin that is not one (P5).
    ParamAfterNonParam,
    /// A reaction's statement on a continuous pin: only discrete and event
    /// pins are committed to (§6).
    StatementOnContinuous,
    /// A `CROSS` watch whose `event` is not an event pin (§3, §4.3).
    CrossEventNotEvent,
    /// A term's `abstol` or `max` that is neither `DEFAULT` nor a discrete pin
    /// holding an `F64` at byte 0 (§3).
    TermBoundNotDiscreteF64,
    /// A list this consumer cannot read: its stride is below the element's
    /// size or breaks its alignment, or its pointer is misaligned (abi.md §12).
    UnreadableList,
    /// An event pin whose layout is not empty (§3: `EVENT`: empty; C4).
    EventNotEmpty,
    /// A continuous pin whose layout is not one `F64` (§3).
    ContinuousNotF64,
    /// A `check` that is not a discrete pin holding a `BOOL` at byte 0 (§3).
    CheckNotBool,
    /// `PREVIOUS` or `CHANGED` of a `QUIET` pin read by a reaction (P7).
    QuietChangeRead,
    /// An array pin (`count >= 1`) that is not `DISCRETE` (§3.1, arrays of carriers).
    ArrayNotDiscrete,
    /// An array pin's `init` that is neither one element nor `count` elements long (§3.1).
    ArrayInitLength,
    /// A read element at or past its pin's `count`, `WHOLE` on a pin that is no array or in a function, or an
    /// `elements` list of another length than `reads` (§3, arrays of carriers).
    ElementOutOfRange,
}

impl Rule {
    /// Every rule, in the order of the spec's list.
    pub const ALL: [Rule; 23] = [
        Rule::UnknownConstant,
        Rule::IndexOutOfRange,
        Rule::KindRole,
        Rule::PartialNotF64,
        Rule::TermTargetNotContinuous,
        Rule::MixedSorts,
        Rule::DiscreteReadsAnalog,
        Rule::ChangedReadByWriter,
        Rule::LimitCorrection,
        Rule::QuietRead,
        Rule::ReactionOnNonEvent,
        Rule::ParamAfterNonParam,
        Rule::StatementOnContinuous,
        Rule::CrossEventNotEvent,
        Rule::TermBoundNotDiscreteF64,
        Rule::UnreadableList,
        Rule::EventNotEmpty,
        Rule::ContinuousNotF64,
        Rule::CheckNotBool,
        Rule::QuietChangeRead,
        Rule::ArrayNotDiscrete,
        Rule::ArrayInitLength,
        Rule::ElementOutOfRange,
    ];

    fn sentence(self) -> &'static str {
        match self {
            Rule::UnknownConstant => "an unknown constant, or a part its cell does not have",
            Rule::IndexOutOfRange => "an index out of range",
            Rule::KindRole => "a pin kind and role pair outside abi.md §3.1",
            Rule::PartialNotF64 => "a partial whose `to` or `at` is not an F64",
            Rule::TermTargetNotContinuous => "a term target that is not a continuous pin",
            Rule::MixedSorts => "a function writing both analog cells and pins (F2)",
            Rule::DiscreteReadsAnalog => "a discrete function reading an input or a continuous pin",
            Rule::ChangedReadByWriter => "CHANGED read by a function that writes a discrete pin or an analog cell",
            Rule::LimitCorrection => {
                "a function writing a limit and a term without OWNS_CORRECTION, or with it and a partial against that limit"
            }
            Rule::QuietRead => "a function reading a QUIET pin",
            Rule::ReactionOnNonEvent => "a reaction listening to a non-event pin",
            Rule::ParamAfterNonParam => "a PARAM pin after a non-PARAM pin (P5)",
            Rule::StatementOnContinuous => "a statement on a continuous pin",
            Rule::CrossEventNotEvent => "a CROSS watch whose event pin is not an event pin",
            Rule::TermBoundNotDiscreteF64 => "a term whose abstol or max pin is not a discrete F64",
            Rule::UnreadableList => "a list whose stride or pointer does not fit its element",
            Rule::EventNotEmpty => "an event pin whose layout is not empty",
            Rule::ContinuousNotF64 => "a continuous pin whose layout is not one F64",
            Rule::CheckNotBool => "a check that is not a discrete BOOL pin",
            Rule::QuietChangeRead => "PREVIOUS or CHANGED of a QUIET pin read by a reaction",
            Rule::ArrayNotDiscrete => "an array pin that is not DISCRETE",
            Rule::ArrayInitLength => "an array pin's init that is neither one element nor count elements",
            Rule::ElementOutOfRange => "a read element past its pin's count, or a misplaced WHOLE read",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.sentence())
    }
}

/// A broken rule: which, at which entry (`"pin p"`, `"function i"`, …), and
/// the entry's source position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeError {
    pub rule: Rule,
    pub entry: String,
    pub origin: String,
    /// What exactly is wrong, for the reader.
    pub detail: String,
}

impl fmt::Display for ShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} ({}): {}", self.origin, self.entry, self.rule, self.detail)
    }
}

impl std::error::Error for ShapeError {}

/// The bytes a cell holds, as far as a check needs them.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CellBytes<'s> {
    F64,
    Byte,
    I64,
    Layout(&'s Layout<'s>),
    /// A `WHOLE` read of an array pin: the cell holds an `ArrayView`.
    View,
}

impl CellBytes<'_> {
    /// Whether byte `at` starts an `F64` of this cell.
    fn f64_at(self, at: u32) -> bool {
        match self {
            CellBytes::F64 => at == 0,
            CellBytes::Byte | CellBytes::I64 | CellBytes::View => false,
            CellBytes::Layout(l) => l.fields.iter().any(|f| {
                f.leaf == leaf::F64 && at >= f.offset && (at - f.offset).is_multiple_of(8) && (at - f.offset) / 8 < f.count
            }),
        }
    }
}

/// How a cell is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    FunctionRead,
    FunctionWrite,
    ReactionRead,
}

/// The entry an error names.
struct Entry<'s> {
    what: &'static str,
    name: &'s Str<'s>,
    origin: &'s Str<'s>,
}

impl Entry<'_> {
    fn fail(&self, rule: Rule, detail: String) -> ShapeError {
        ShapeError {
            rule,
            entry: format!("{} {}", self.what, self.name.as_str()),
            origin: self.origin.as_str().to_string(),
            detail,
        }
    }

    /// An `elements` list is empty or has one entry per read.
    fn check_elements(&self, reads: usize, elements: List<u64>) -> Result<(), ShapeError> {
        if elements.len() != 0 && elements.len() != reads {
            return Err(self.fail(Rule::ElementOutOfRange, format!("{} elements for {reads} reads", elements.len())));
        }
        Ok(())
    }
}

fn known(value: u32, set: &[u32]) -> bool {
    set.contains(&value)
}

fn known_bits(value: u32, all: u32) -> bool {
    value & !all == 0
}

const KINDS: &[u32] = &[kind::CONTINUOUS, kind::DISCRETE, kind::EVENT];
const ROLES: &[u32] = &[role::PORT, role::PARAM, role::GLOBAL, role::PRIVATE];
const GLOBALS: &[u32] = &[
    global::TEMPERATURE,
    global::RESOLUTION,
    global::DC,
    global::TRAN,
    global::AC,
    global::NOISE,
    global::START,
    global::SETTLE,
    global::NAMED,
];
const PIN_FLAGS: u32 = pin_flag::STRUCTURAL
    | pin_flag::OPTIONAL
    | pin_flag::HAS_INIT
    | pin_flag::HAS_CHECK
    | pin_flag::QUIET
    | pin_flag::RECORD
    | pin_flag::RELAX;
const OPS: &[u32] = &[op::FLOW, op::DDT, op::DELAY];
const INPUTS: &[u32] = &[input::LIMIT, input::NOISE, input::STIM];
const WATCHES: &[u32] = &[watch::VAR, watch::CROSS, watch::CAP, watch::TRACE];
const CELLS: &[u32] = &[cell::PIN, cell::TERM, cell::INPUT, cell::WATCH, cell::AMBIENT];
const PARTS: &[u32] = &[
    part::VALUE,
    part::PREVIOUS,
    part::CHANGED,
    part::TIME,
    part::LAST,
    part::AGE,
    part::PROPOSED,
    part::LIMITED,
    part::PSD,
    part::RE,
    part::IM,
];
const AMBIENTS: &[u32] = &[
    ambient::TIME,
    ambient::FREQ,
    ambient::STATIC,
    ambient::GMIN,
    ambient::SOURCE_SCALE,
    ambient::TICK,
    ambient::HORIZON,
];
const STATEMENT_FLAGS: u32 = statement_flag::LATER | statement_flag::REPLACE;

/// Checks every shape-local rule of abi.md §9; the first broken one is returned.
pub fn validate(shape: &Shape) -> Result<(), ShapeError> {
    check_lists(shape)?;
    let mut seen_non_param = false;
    for pin in shape.pins.iter() {
        check_pin(shape, pin)?;
        let e = Entry { what: "pin", name: &pin.name, origin: &pin.origin };
        if pin.role == role::PARAM && seen_non_param {
            return Err(e.fail(Rule::ParamAfterNonParam, "PARAM pins come first (P5)".into()));
        }
        seen_non_param |= pin.role != role::PARAM;
    }
    for term in shape.terms.iter() {
        let e = Entry { what: "term", name: &term.name, origin: &term.origin };
        if !known(term.op, OPS) {
            return Err(e.fail(Rule::UnknownConstant, format!("op {}", term.op)));
        }
        for (side, target) in [("plus", term.plus), ("minus", term.minus)] {
            if target == GROUND {
                continue;
            }
            let Some(pin) = shape.pins.get(target as usize) else {
                return Err(e.fail(Rule::IndexOutOfRange, format!("{side} = {target}")));
            };
            if pin.kind != kind::CONTINUOUS {
                return Err(e.fail(
                    Rule::TermTargetNotContinuous,
                    format!("{side} is pin {}", pin.name.as_str()),
                ));
            }
        }
        for (what, index) in [("abstol", term.abstol), ("max", term.max)] {
            if index == DEFAULT {
                continue;
            }
            let Some(pin) = shape.pins.get(index as usize) else {
                return Err(e.fail(Rule::IndexOutOfRange, format!("{what} = {index}")));
            };
            if pin.kind != kind::DISCRETE || !CellBytes::Layout(&pin.layout).f64_at(0) {
                return Err(e.fail(
                    Rule::TermBoundNotDiscreteF64,
                    format!("{what} is pin {}", pin.name.as_str()),
                ));
            }
        }
    }
    for inp in shape.inputs.iter() {
        if !known(inp.kind, INPUTS) {
            let e = Entry { what: "input", name: &inp.name, origin: &inp.origin };
            return Err(e.fail(Rule::UnknownConstant, format!("kind {}", inp.kind)));
        }
    }
    for w in shape.watches.iter() {
        let e = Entry { what: "watch", name: &w.name, origin: &w.origin };
        if !known(w.kind, WATCHES) {
            return Err(e.fail(Rule::UnknownConstant, format!("kind {}", w.kind)));
        }
        check_layout(&e, &w.layout)?;
        if w.kind == watch::CROSS {
            let Some(pin) = shape.pins.get(w.event as usize) else {
                return Err(e.fail(Rule::IndexOutOfRange, format!("event = {}", w.event)));
            };
            if pin.kind != kind::EVENT {
                return Err(e.fail(Rule::CrossEventNotEvent, format!("event is pin {}", pin.name.as_str())));
            }
        }
        for (what, index) in [("ttol", w.ttol), ("etol", w.etol)] {
            if w.kind == watch::CROSS && index != DEFAULT && shape.pins.get(index as usize).is_none() {
                return Err(e.fail(Rule::IndexOutOfRange, format!("{what} = {index}")));
            }
        }
    }
    for f in shape.functions.iter() {
        check_function(shape, f)?;
    }
    for r in shape.reactions.iter() {
        check_reaction(shape, r)?;
    }
    Ok(())
}

fn check_pin(shape: &Shape, pin: &Pin) -> Result<(), ShapeError> {
    let e = Entry { what: "pin", name: &pin.name, origin: &pin.origin };
    if !known(pin.kind, KINDS) {
        return Err(e.fail(Rule::UnknownConstant, format!("kind {}", pin.kind)));
    }
    if !known(pin.role, ROLES) {
        return Err(e.fail(Rule::UnknownConstant, format!("role {}", pin.role)));
    }
    if pin.role == role::GLOBAL && !known(pin.global, GLOBALS) {
        return Err(e.fail(Rule::UnknownConstant, format!("global {}", pin.global)));
    }
    if !known_bits(pin.flags, PIN_FLAGS) {
        return Err(e.fail(Rule::UnknownConstant, format!("flags {:#x}", pin.flags)));
    }
    check_layout(&e, &pin.layout)?;
    let allowed = match pin.kind {
        kind::CONTINUOUS => pin.role == role::PORT || pin.role == role::PRIVATE,
        kind::EVENT => pin.role != role::PARAM,
        _ => true,
    };
    if !allowed {
        return Err(e.fail(Rule::KindRole, format!("kind {} with role {}", pin.kind, pin.role)));
    }
    if pin.kind == kind::EVENT && pin.layout.size != 0 {
        return Err(e.fail(Rule::EventNotEmpty, format!("layout of {} bytes", pin.layout.size)));
    }
    if pin.kind == kind::CONTINUOUS && !layout_eq(&pin.layout, &F64) {
        return Err(e.fail(Rule::ContinuousNotF64, format!("layout of {} bytes", pin.layout.size)));
    }
    if pin.count > 0 {
        if pin.kind != kind::DISCRETE {
            return Err(e.fail(Rule::ArrayNotDiscrete, format!("count {} on kind {}", pin.count, pin.kind)));
        }
        let element = pin.layout.size as u64;
        let init = pin.init.len as u64;
        if pin.flags & pin_flag::HAS_INIT != 0
            && init != element
            && Some(init) != element.checked_mul(pin.count)
        {
            return Err(e.fail(
                Rule::ArrayInitLength,
                format!("init of {init} bytes for {} elements of {element} bytes", pin.count),
            ));
        }
    }
    if pin.flags & pin_flag::HAS_CHECK != 0 {
        let Some(check) = shape.pins.get(pin.check as usize) else {
            return Err(e.fail(Rule::IndexOutOfRange, format!("check = {}", pin.check)));
        };
        let bool_at_0 = check.layout.fields.iter().any(|f| f.leaf == leaf::BOOL && f.offset == 0);
        if check.kind != kind::DISCRETE || !bool_at_0 {
            return Err(e.fail(Rule::CheckNotBool, format!("check is pin {}", check.name.as_str())));
        }
    }
    Ok(())
}

fn check_layout(e: &Entry<'_>, layout: &Layout) -> Result<(), ShapeError> {
    for f in layout.fields.iter() {
        let Some(width) = leaf_size(f.leaf) else {
            return Err(e.fail(Rule::UnknownConstant, format!("field {} leaf {}", f.name.as_str(), f.leaf)));
        };
        let end = f.offset as u64 + width as u64 * f.count as u64;
        if end > layout.size as u64 {
            return Err(e.fail(
                Rule::IndexOutOfRange,
                format!("field {} lies past the layout's {} bytes", f.name.as_str(), layout.size),
            ));
        }
    }
    Ok(())
}

/// The bytes of `c` under `access`, after checking its constants and index.
pub(crate) fn cell_bytes<'s>(
    shape: &'s Shape,
    c: &Cell,
    element: u64,
    access: Access,
) -> Result<CellBytes<'s>, (Rule, String)> {
    if !known(c.kind, CELLS) {
        return Err((Rule::UnknownConstant, format!("cell kind {}", c.kind)));
    }
    if !known(c.part, PARTS) {
        return Err((Rule::UnknownConstant, format!("cell part {}", c.part)));
    }
    let no_part = || Err((Rule::UnknownConstant, format!("part {} of cell kind {} index {}", c.part, c.kind, c.index)));
    let out_of_range = || Err((Rule::IndexOutOfRange, format!("cell kind {} index {}", c.kind, c.index)));
    let read = access != Access::FunctionWrite;
    match c.kind {
        cell::PIN => {
            let Some(pin) = shape.pins.get(c.index as usize) else { return out_of_range() };
            if element != 0 {
                let array = pin.kind == kind::DISCRETE && pin.count > 0;
                let in_range = element == WHOLE && access == Access::ReactionRead || element < pin.count;
                if !(array && in_range && read) {
                    return Err((
                        Rule::ElementOutOfRange,
                        format!("element {element} of pin {} of {} elements", pin.name.as_str(), pin.count),
                    ));
                }
                if element == WHOLE {
                    return Ok(CellBytes::View);
                }
            }
            match (pin.kind, c.part, read) {
                (kind::CONTINUOUS, part::VALUE, true) => Ok(CellBytes::F64),
                (kind::DISCRETE, part::VALUE, _) | (kind::DISCRETE, part::PREVIOUS, true) => {
                    Ok(CellBytes::Layout(&pin.layout))
                }
                (kind::DISCRETE, part::CHANGED, true) | (kind::EVENT, part::CHANGED, _) => Ok(CellBytes::Byte),
                _ => no_part(),
            }
        }
        cell::AMBIENT => {
            if !known(c.index, AMBIENTS) {
                return Err((Rule::UnknownConstant, format!("ambient {}", c.index)));
            }
            // The environment of §12.1.12 and §14.4: a reaction may read the continuation
            // values (`gmin()`, `source_scale()`), as an analog function does.
            let reaction_ok = matches!(
                c.index,
                ambient::TIME | ambient::STATIC | ambient::TICK | ambient::HORIZON | ambient::GMIN | ambient::SOURCE_SCALE
            );
            let function_ok = !matches!(c.index, ambient::TICK | ambient::HORIZON);
            let ok = match access {
                Access::FunctionRead => function_ok,
                Access::ReactionRead => reaction_ok,
                Access::FunctionWrite => false,
            };
            if !ok || c.part != part::VALUE {
                return no_part();
            }
            Ok(match c.index {
                ambient::STATIC => CellBytes::Byte,
                ambient::TICK | ambient::HORIZON => CellBytes::I64,
                _ => CellBytes::F64,
            })
        }
        _ if access == Access::ReactionRead => no_part(),
        cell::TERM => {
            let Some(term) = shape.terms.get(c.index as usize) else { return out_of_range() };
            match (c.part, read) {
                (part::VALUE, false) => Ok(CellBytes::F64),
                (part::TIME, false) if term.op == op::DELAY => Ok(CellBytes::F64),
                _ => no_part(),
            }
        }
        cell::INPUT => {
            let Some(inp) = shape.inputs.get(c.index as usize) else { return out_of_range() };
            match (inp.kind, c.part, read) {
                (_, part::VALUE, true) => Ok(CellBytes::F64),
                (input::LIMIT, part::LAST, true) => Ok(CellBytes::F64),
                (input::LIMIT, part::AGE, true) => Ok(CellBytes::Byte),
                (input::LIMIT, part::PROPOSED | part::LIMITED, false)
                | (input::NOISE, part::PSD, false)
                | (input::STIM, part::RE | part::IM, false) => Ok(CellBytes::F64),
                _ => no_part(),
            }
        }
        _ => {
            let Some(w) = shape.watches.get(c.index as usize) else { return out_of_range() };
            match (c.part, read) {
                (part::VALUE, false) if w.kind == watch::CROSS || w.kind == watch::CAP => Ok(CellBytes::F64),
                (part::VALUE, false) => Ok(CellBytes::Layout(&w.layout)),
                _ => no_part(),
            }
        }
    }
}

fn check_function(shape: &Shape, f: &Function) -> Result<(), ShapeError> {
    let e = Entry { what: "function", name: &f.name, origin: &f.origin };
    if !known_bits(f.flags, fn_flag::OWNS_CORRECTION) {
        return Err(e.fail(Rule::UnknownConstant, format!("flags {:#x}", f.flags)));
    }
    e.check_elements(f.reads.len(), f.elements)?;
    let mut reads = Vec::with_capacity(f.reads.len());
    for (i, c) in f.reads.iter().enumerate() {
        reads.push(cell_bytes(shape, c, f.elements.get(i).copied().unwrap_or(0), Access::FunctionRead).map_err(|(rule, d)| e.fail(rule, d))?);
    }
    let mut writes = Vec::with_capacity(f.writes.len());
    for c in f.writes.iter() {
        writes.push(cell_bytes(shape, c, 0, Access::FunctionWrite).map_err(|(rule, d)| e.fail(rule, d))?);
    }
    for (i, p) in f.partials.iter().enumerate() {
        let (Some(w), Some(r)) = (writes.get(p.write as usize), reads.get(p.read as usize)) else {
            return Err(e.fail(Rule::IndexOutOfRange, format!("partial {i}: write {} read {}", p.write, p.read)));
        };
        if !w.f64_at(p.to) || !r.f64_at(p.at) {
            return Err(e.fail(Rule::PartialNotF64, format!("partial {i}: to {} at {}", p.to, p.at)));
        }
    }

    let writes_pin = f.writes.iter().any(|c| c.kind == cell::PIN);
    let writes_analog = f.writes.iter().any(|c| c.kind != cell::PIN);
    if writes_pin && writes_analog {
        return Err(e.fail(Rule::MixedSorts, "writes analog cells and pins".into()));
    }
    let pin_of = |c: &Cell| shape.pins.get(c.index as usize).filter(|_| c.kind == cell::PIN);
    if writes_pin {
        for c in f.reads.iter() {
            // `GMIN` and `SOURCE_SCALE` hold their values at rest through every instant (the
            // last solve of a sequence), so a discrete function may read them (§12.1.12, §14.4).
            let environment = c.kind == cell::AMBIENT && matches!(c.index, ambient::GMIN | ambient::SOURCE_SCALE);
            let analog = (matches!(c.kind, cell::INPUT | cell::AMBIENT) && !environment)
                || pin_of(c).is_some_and(|p| p.kind == kind::CONTINUOUS);
            if analog {
                return Err(e.fail(
                    Rule::DiscreteReadsAnalog,
                    format!("reads cell kind {} index {}", c.kind, c.index),
                ));
            }
        }
    }
    let event_only = f.writes.iter().all(|c| pin_of(c).is_some_and(|p| p.kind == kind::EVENT));
    if !event_only && f.reads.iter().any(|c| c.kind == cell::PIN && c.part == part::CHANGED) {
        return Err(e.fail(Rule::ChangedReadByWriter, "reads CHANGED".into()));
    }

    let limit_writes: Vec<u32> = f
        .writes
        .iter()
        .filter(|c| c.kind == cell::INPUT && (c.part == part::PROPOSED || c.part == part::LIMITED))
        .map(|c| c.index)
        .collect();
    let writes_term = f.writes.iter().any(|c| c.kind == cell::TERM);
    if !limit_writes.is_empty() && writes_term {
        if f.flags & fn_flag::OWNS_CORRECTION == 0 {
            return Err(e.fail(Rule::LimitCorrection, "writes a limit and a term without OWNS_CORRECTION".into()));
        }
        for p in f.partials.iter() {
            let against = f.reads.get(p.read as usize);
            if against.is_some_and(|c| c.kind == cell::INPUT && limit_writes.contains(&c.index)) {
                return Err(e.fail(
                    Rule::LimitCorrection,
                    "OWNS_CORRECTION with a partial against the limit it writes".into(),
                ));
            }
        }
    }

    for c in f.reads.iter() {
        if let Some(p) = pin_of(c)
            && p.flags & (pin_flag::QUIET | pin_flag::RECORD) != 0
        {
            return Err(e.fail(Rule::QuietRead, format!("reads quiet pin {}", p.name.as_str())));
        }
    }
    Ok(())
}

fn check_reaction(shape: &Shape, r: &Reaction) -> Result<(), ShapeError> {
    let e = Entry { what: "reaction", name: &r.name, origin: &r.origin };
    if !known_bits(r.flags, reaction_flag::IRREVERSIBLE) {
        return Err(e.fail(Rule::UnknownConstant, format!("flags {:#x}", r.flags)));
    }
    for index in r.on.iter() {
        let Some(pin) = shape.pins.get(*index as usize) else {
            return Err(e.fail(Rule::IndexOutOfRange, format!("on {index}")));
        };
        if pin.kind != kind::EVENT {
            return Err(e.fail(Rule::ReactionOnNonEvent, format!("on pin {}", pin.name.as_str())));
        }
    }
    e.check_elements(r.reads.len(), r.elements)?;
    for (i, c) in r.reads.iter().enumerate() {
        cell_bytes(shape, c, r.elements.get(i).copied().unwrap_or(0), Access::ReactionRead).map_err(|(rule, d)| e.fail(rule, d))?;
        let quiet = shape.pins.get(c.index as usize).is_some_and(|p| p.flags & (pin_flag::QUIET | pin_flag::RECORD) != 0);
        if c.kind == cell::PIN && quiet && (c.part == part::PREVIOUS || c.part == part::CHANGED) {
            return Err(e.fail(Rule::QuietChangeRead, format!("reads part {} of quiet pin {}", c.part, c.index)));
        }
    }
    for (i, s) in r.writes.iter().enumerate() {
        if !known_bits(s.flags, STATEMENT_FLAGS) {
            return Err(e.fail(Rule::UnknownConstant, format!("statement {i} flags {:#x}", s.flags)));
        }
        let Some(pin) = shape.pins.get(s.pin as usize) else {
            return Err(e.fail(Rule::IndexOutOfRange, format!("statement {i} pin {}", s.pin)));
        };
        if pin.kind == kind::CONTINUOUS {
            return Err(e.fail(
                Rule::StatementOnContinuous,
                format!("statement {i} writes pin {}", pin.name.as_str()),
            ));
        }
        if s.offset as u64 + s.len as u64 > pin.layout.size as u64 {
            return Err(e.fail(
                Rule::IndexOutOfRange,
                format!("statement {i}: bytes {}..+{} past pin {}", s.offset, s.len, pin.name.as_str()),
            ));
        }
    }
    Ok(())
}

/// Every list of the shape is readable before any is walked (abi.md §12).
fn check_lists(shape: &Shape) -> Result<(), ShapeError> {
    let unreadable = |entry: String, origin: &Str, what: &str| ShapeError {
        rule: Rule::UnreadableList,
        entry,
        origin: origin.as_str().to_string(),
        detail: format!("the {what} list"),
    };
    let none = Str::EMPTY;
    for (ok, what) in [
        (shape.pins.is_readable(), "pins"),
        (shape.terms.is_readable(), "terms"),
        (shape.inputs.is_readable(), "inputs"),
        (shape.watches.is_readable(), "watches"),
        (shape.functions.is_readable(), "functions"),
        (shape.reactions.is_readable(), "reactions"),
    ] {
        if !ok {
            return Err(unreadable("shape".into(), &none, what));
        }
    }
    for p in shape.pins.iter() {
        if !p.layout.fields.is_readable() {
            return Err(unreadable(format!("pin {}", p.name.as_str()), &p.origin, "layout fields"));
        }
    }
    for w in shape.watches.iter() {
        if !w.layout.fields.is_readable() {
            return Err(unreadable(format!("watch {}", w.name.as_str()), &w.origin, "layout fields"));
        }
    }
    for f in shape.functions.iter() {
        if !(f.reads.is_readable() && f.writes.is_readable() && f.partials.is_readable() && f.elements.is_readable()) {
            return Err(unreadable(format!("function {}", f.name.as_str()), &f.origin, "reads, writes or partials"));
        }
    }
    for r in shape.reactions.iter() {
        if !(r.on.is_readable() && r.reads.is_readable() && r.writes.is_readable() && r.elements.is_readable()) {
            return Err(unreadable(format!("reaction {}", r.name.as_str()), &r.origin, "on, reads or writes"));
        }
    }
    Ok(())
}
