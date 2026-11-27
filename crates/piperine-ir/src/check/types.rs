//! The type check's passes in their order, the checks of every type written in the IR (an integer
//! width in range, a struct or an enum of the arena, a carrier and a stream field a pin can hold, a
//! carrier's initial value of its type, a generic instance named for its arguments, a hand-written
//! `Repr` of the right shape), and the assertions the code walk makes of an expression's type.

use std::collections::BTreeSet;

use super::TypeCheck;
use super::walk::CodeWalk;
use crate::ids::{ExprId, TypeId};
use crate::model::{BodyData, Carrier, CarrierRole, Expr, FunctionData, Origin, ReprForm, TypeData, TypeKind, ValueType};
use crate::typing::Typing;

/// A type written in the IR, with the item that wrote it and where.
type WrittenType = (String, Origin, ValueType);

impl TypeCheck<'_> {
    /// Runs the type passes in their order.
    pub(super) fn run(&mut self) {
        self.declared_types();
        self.type_arena();
        self.aggregate_constants();
        self.composite_params();
        self.code();
    }

    // ---- the types written -------------------------------------------------------------------

    /// Every type written in the IR is well formed: an integer width in 1..=2^32 - 1 (Q4 of the
    /// number widths, which supersedes RS-Q6's 1..=64: a width past 64 is a `U64` field of
    /// `ceil(N/64)` elements).
    fn declared_types(&mut self) {
        let ir = self.reader.ir;
        let mut found: Vec<WrittenType> = Vec::new();
        for body in &ir.bodies {
            self.body_types(body, &mut found);
        }
        for net in &ir.nets {
            found.push((format!("net {}", net.path), net.origin.clone(), net.ty.clone()));
        }
        for function in &ir.functions {
            let item = format!("function {}", function.name);
            for l in &function.locals {
                found.push((format!("{item} local {}", l.name), function.origin.clone(), l.ty.clone()));
            }
            found.push((format!("{item} result"), function.origin.clone(), function.ret.clone()));
        }
        for (i, data) in ir.exprs.iter().enumerate() {
            found.push((format!("expr {i}"), data.origin.clone(), data.ty.clone()));
            if let Expr::Cast { to, .. } = &data.kind {
                found.push((format!("expr {i}"), data.origin.clone(), to.clone()));
            }
        }
        for (item, origin, ty) in found {
            if let Some(what) = self.malformed(&ty) {
                self.findings.report(item, origin, what);
            }
        }
        for (index, data) in ir.types.iter().enumerate() {
            self.type_instance(data);
            self.hand_written_repr(index, data);
        }
    }

    /// The types `body` writes (its params, locals, stream fields and carriers) into `found`; a
    /// stream field or a carrier a pin cannot hold, and a carrier's initial value, are checked here.
    fn body_types(&mut self, body: &BodyData, found: &mut Vec<WrittenType>) {
        let item = format!("body {}", body.name);
        for p in &body.params {
            found.push((format!("{item} param {}", p.name), p.origin.clone(), p.ty.clone()));
        }
        for l in &body.locals {
            found.push((format!("{item} local {}", l.name), body.origin.clone(), l.ty.clone()));
        }
        for r in &body.reactions {
            for l in &r.locals {
                found.push((format!("{item} reaction local {}", l.name), r.origin.clone(), l.ty.clone()));
            }
        }
        for stream in &body.streams {
            let mut names: BTreeSet<&str> = BTreeSet::new();
            for f in &stream.fields {
                let at = format!("{item} stream {}", stream.name);
                if !self.pin_can_hold(&f.ty) {
                    self.findings.report(at.clone(), body.origin.clone(), format!("a stream field holds `bool`, `f64` or an integer type, found {} for {}", f.ty, f.name));
                }
                if !names.insert(f.name.as_str()) {
                    self.findings.report(at.clone(), body.origin.clone(), format!("stream {} has two fields named {}", stream.name, f.name));
                }
                found.push((at, body.origin.clone(), f.ty.clone()));
            }
        }
        for c in &body.carriers {
            self.carrier_type(&item, c);
            found.push((format!("{item} carrier {}", c.name), c.origin.clone(), c.ty.clone()));
        }
    }

    /// A carrier of the body named `item` holds a type a pin can hold, and its initial value is a
    /// value of the type it is driven with.
    fn carrier_type(&mut self, item: &str, c: &Carrier) {
        if !self.pin_can_hold(&c.ty) {
            self.findings.report(
                format!("{item} carrier {}", c.name),
                c.origin.clone(),
                format!("a carrier holds `bool`, `f64`, an integer type, an enum, or a tuple, array, struct or `Option` of them, found {}", c.ty),
            );
        } else if let Some(init) = c.init
            // a resolved carrier reads `ty` and is driven with `driven`: its initial value is its first drive's (§5.2.3)
            && let held = match &c.role {
                CarrierRole::Resolved { driven, .. } => driven,
                _ => &c.ty,
            }
            && !self.reader.holds(held, init)
            // an array carrier's initial value is one element (`[x; K]`) or every element (`[a, b, c]`)
            && !c.count.is_some_and(|count| self.reader.holds(&ValueType::Array(Box::new(c.ty.clone()), count), init))
        {
            self.findings.report(
                format!("{item} carrier {}", c.name),
                c.origin.clone(),
                format!("the initial value {init} of carrier {} is not a value of its type {held}", c.name),
            );
        }
    }

    /// Whether a pin can hold a value of `ty`: a scalar, or a tuple, an array, a struct, an enum's
    /// payload or an `Option` whose parts a pin can hold (§2: every part has a `Repr`).
    fn pin_can_hold(&self, ty: &ValueType) -> bool {
        let mut stack = vec![ty];
        let mut seen = BTreeSet::new();
        while let Some(t) = stack.pop() {
            match t {
                ValueType::Tuple(items) => stack.extend(items),
                ValueType::Array(element, _) => stack.push(element),
                ValueType::Option(inner) => stack.push(inner),
                ValueType::Struct(id) | ValueType::Enum(id) => {
                    let Some(data) = self.reader.ir.types.get(id.index()) else { return false };
                    if !seen.insert(id.index()) {
                        continue;
                    }
                    match &data.kind {
                        TypeKind::Struct { fields, .. } => stack.extend(fields.iter().map(|(_, field)| field)),
                        TypeKind::Enum { variants, .. } => stack.extend(variants.iter().flat_map(|v| v.fields.iter().map(|(_, field)| field))),
                    }
                }
                scalar if Typing::scalar(scalar) => {}
                _ => return false,
            }
        }
        true
    }

    /// A type that is a generic instance is named `item<arguments>`.
    fn type_instance(&mut self, data: &TypeData) {
        if let Some(instance) = &data.instance
            && data.name != instance.type_name()
        {
            self.findings.report(format!("type {}", data.name), data.origin.clone(), format!("a generic instance of `{}` is named `{}`, not `{}`", instance.item, instance.type_name(), data.name));
        }
    }

    /// A hand-written `Repr` (GN-05): `encode` takes the type and returns `[bool; width]`, `decode`
    /// takes `[bool; width]` and returns `Option` of the type, and `width` is at least one level.
    fn hand_written_repr(&mut self, index: usize, data: &TypeData) {
        let ReprForm::HandWritten { width, encode, decode } = data.repr else { return };
        let item = format!("type {}", data.name);
        let own = match data.kind {
            TypeKind::Struct { .. } => ValueType::Struct(TypeId(index as u32)),
            TypeKind::Enum { .. } => ValueType::Enum(TypeId(index as u32)),
        };
        let levels = ValueType::Array(Box::new(ValueType::Bool), u64::from(width));
        if width == 0 {
            self.findings.report(item.clone(), data.origin.clone(), "a hand-written Repr has at least one level".to_string());
        }
        let (Some(encode), Some(decode)) = (self.reader.ir.functions.get(encode.index()), self.reader.ir.functions.get(decode.index())) else {
            self.findings.report(item, data.origin.clone(), "a hand-written Repr names a function outside the arena".to_string());
            return;
        };
        let takes = |function: &FunctionData, ty: &ValueType| function.params.len() == 1 && function.params[0].ty == *ty;
        if !takes(encode, &own) || encode.ret != levels {
            self.findings.report(item.clone(), data.origin.clone(), format!("the encode of a hand-written Repr takes {own} and returns {levels}, `{}` does not", encode.name));
        }
        if !takes(decode, &levels) || decode.ret != ValueType::Option(Box::new(own.clone())) {
            self.findings.report(item, data.origin.clone(), format!("the decode of a hand-written Repr takes {levels} and returns Option<{own}>, `{}` does not", decode.name));
        }
    }

    /// What is wrong with `ty`, read through function types and aggregate parts.
    pub(super) fn malformed(&self, ty: &ValueType) -> Option<String> {
        let types = &self.reader.ir.types;
        let mut stack = vec![ty];
        while let Some(t) = stack.pop() {
            match t {
                ValueType::UInt(0) | ValueType::Int(0) => {
                    return Some(format!("type {t} has a width outside 1..=4294967295"));
                }
                ValueType::Fn(params, ret) => {
                    stack.extend(params);
                    stack.push(ret);
                }
                ValueType::Struct(id) | ValueType::Enum(id) if id.index() >= types.len() => {
                    return Some(format!("type {t} names a type outside the arena ({} types)", types.len()));
                }
                ValueType::Struct(id) if !matches!(types[id.index()].kind, TypeKind::Struct { .. }) => {
                    return Some(format!("type {t} names an enum as a struct"));
                }
                ValueType::Enum(id) if !matches!(types[id.index()].kind, TypeKind::Enum { .. }) => {
                    return Some(format!("type {t} names a struct as an enum"));
                }
                ValueType::Tuple(parts) => stack.extend(parts),
                ValueType::Array(element, _) | ValueType::Option(element) => stack.push(element),
                _ => {}
            }
        }
        None
    }

    // ---- the assertions of the code walk -------------------------------------------------------

    /// Reports `what` at `origin`, against the owner of the code being walked.
    pub(super) fn wrong(&mut self, walk: &CodeWalk, origin: Origin, what: String) {
        self.findings.report(walk.owner.item.clone(), origin, what);
    }

    /// `id`, placed where `want` is required (a `let`, an addend, a `where`, a function body),
    /// must have that type.
    pub(super) fn require_type(&mut self, walk: &CodeWalk, id: ExprId, want: &ValueType) {
        if let Some(got) = self.reader.type_of(id) {
            if got != want {
                let origin = self.reader.expression_origin(id);
                self.wrong(walk, origin, format!("expression must be {want}, found {got}"));
            }
        }
    }

    /// `id`, a value written to a carrier of type `want` (or a delay, `want` being `f64`): its type
    /// is `want`.
    pub(super) fn carried(&mut self, walk: &CodeWalk, id: ExprId, want: &ValueType, position: &str) {
        self.want(walk, id, want, position);
    }

    /// `id` must have type `want`; an entry at its origin otherwise.
    pub(super) fn want(&mut self, walk: &CodeWalk, id: ExprId, want: &ValueType, position: &str) {
        if let Some(got) = self.reader.type_of(id) {
            if got != want {
                let origin = self.reader.expression_origin(id);
                self.wrong(walk, origin, format!("{position} must be {want}, found {got}"));
            }
        }
    }

    /// The recorded type `got` of a digital form must be `want`, the type its form gives its
    /// operands; `None` when the operands fit no rule of `what`.
    pub(super) fn typed(&mut self, walk: &CodeWalk, origin: &Origin, got: &ValueType, want: Option<ValueType>, what: &str) {
        match want {
            Some(want) => self.own(walk, origin, got, &want),
            None => self.wrong(walk, origin.clone(), format!("the operands of {what} do not fit its types")),
        }
    }

    /// The recorded type `got` of an expression must be what its form produces.
    pub(super) fn own(&mut self, walk: &CodeWalk, origin: &Origin, got: &ValueType, want: &ValueType) {
        if got != want {
            self.wrong(walk, origin.clone(), format!("expression is typed {got}, its form gives {want}"));
        }
    }
}
