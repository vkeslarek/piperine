//! `x.encode()` and `T::decode(levels)` on a value or a type of a `T: Repr` parameter (part I §2, H.8 rule `Repr`),
//! lowered per instance: the receiver's type is the one the substitution gives `T`, so the levels are written out for it.
//!
//! `bool` is one level, an unsigned or signed integer of `N` bits is `N` levels, least significant first, and a type
//! with a hand-written `impl Repr` is the call of its own `encode` and `decode`. Any other layout (`f64`, a derived
//! struct, enum, tuple or array) is refused by name: it needs an IR form no phase A task adds.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::compound::Pair;
use super::elaborator::Elaboration;
use super::lower::Scope;
use super::variants::Half;
use crate::pom::{Arg, DeclId, ExprId, ExprKind, Resolution};
use crate::types::{ConstTerm, GenericValue, Length, Ty};

impl Elaboration<'_, '_> {
    /// `ty` with each `[_; T::WIDTH]` that is still generic (the consts evaluator folds the built-in layouts) folded
    /// to the `WIDTH` of the hand-written `impl Repr` of the type `T` is given.
    pub(super) fn with_hand_written_widths(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Array(element, Length::Generic(ConstTerm::Width(parameter))) => {
                let element = Box::new(self.with_hand_written_widths(element));
                let width = match self.consts.substitution().get(*parameter) {
                    Some(GenericValue::Type(given)) => self.impls.hand_written_repr(given).map(|entry| entry.decl).and_then(|owner| self.hand_written_width(owner)),
                    _ => None,
                };
                match width {
                    Some(levels) => Ty::Array(element, Length::Fixed(u64::from(levels))),
                    None => Ty::Array(element, Length::Generic(ConstTerm::Width(*parameter))),
                }
            }
            Ty::Array(element, length) => Ty::Array(Box::new(self.with_hand_written_widths(element)), length.clone()),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|item| self.with_hand_written_widths(item)).collect()),
            Ty::Option(inner) => Ty::Option(Box::new(self.with_hand_written_widths(inner))),
            other => other.clone(),
        }
    }

    /// `x.encode()` where the checker typed no method (a `T: Repr` parameter): the levels of the receiver. `None` when
    /// the call is no such encode, with no diagnostic; after a diagnostic when it is one and cannot be lowered.
    pub(super) fn lower_parameter_encode(&mut self, scope: &mut Scope, expr: ExprId, recv: ExprId, name: &str, origin: Origin) -> Option<Option<LoweredExpr>> {
        if name != "encode" {
            return None;
        }
        let span = self.design.expr_node(expr).span;
        let Some(ty) = self.typed(recv) else { return None };
        Some(self.encode_levels(scope, recv, &ty, span, origin))
    }

    fn encode_levels(&mut self, scope: &mut Scope, recv: ExprId, ty: &Ty, span: crate::msl::Span, origin: Origin) -> Option<LoweredExpr> {
        if let Some(owner) = self.impls.hand_written_repr(ty).map(|entry| entry.decl) {
            let item = self.impls.item(owner, "encode")?;
            let argument = self.lower_expr(scope, recv)?;
            let (function, ret) = self.request(item, Half::Whole)?;
            return Some(self.emit(Expr::Call { function, args: vec![argument] }, ret, origin));
        }
        let value = self.lower_expr(scope, recv)?;
        match ty {
            Ty::Bool => Some(self.emit(Expr::Array(vec![value]), ValueType::Array(Box::new(ValueType::Bool), 1), origin)),
            Ty::UInt(bits) | Ty::Int(bits) if *bits <= 64 => {
                let integer = self.type_of(value);
                let levels: Vec<LoweredExpr> = (0..u128::from(*bits)).map(|k| self.level_of_bit(value, &integer, k, &origin)).collect();
                Some(self.emit(Expr::Array(levels), ValueType::Array(Box::new(ValueType::Bool), u64::from(*bits)), origin))
            }
            _ => {
                self.unsupported(scope.file, span, "`encode` on a value of a type whose layout is derived (only `bool`, integers up to 64 bits and hand-written `Repr` types run)");
                None
            }
        }
    }

    /// Level `k` of the integer `value`: `((value >> k) & 1) != 0`.
    fn level_of_bit(&mut self, value: LoweredExpr, integer: &ValueType, k: u128, origin: &Origin) -> LoweredExpr {
        let shift = self.emit(Expr::Int(k as i128), integer.clone(), origin.clone());
        let shifted = self.emit(Expr::Binary { op: BinaryOp::Shr, lhs: value, rhs: shift }, integer.clone(), origin.clone());
        let one = self.emit(Expr::Int(1), integer.clone(), origin.clone());
        let bit = self.emit(Expr::Binary { op: BinaryOp::BitAnd, lhs: shifted, rhs: one }, integer.clone(), origin.clone());
        let zero = self.emit(Expr::Int(0), integer.clone(), origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::Ne, lhs: bit, rhs: zero }, ValueType::Bool, origin.clone())
    }

    /// `T::decode(levels)` of a type parameter `T` (a path call the checker typed as `Option<T>` with no callee): the
    /// pair of the value the levels give. `None` when `expr` is no such call, with no diagnostic.
    pub(super) fn lower_parameter_decode(&mut self, scope: &mut Scope, expr: ExprId, callee: ExprId, args: &[Arg], origin: &Origin) -> Option<Option<Pair>> {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return None };
        let segments = self.design.path_node(*path).segments.clone();
        let [owner, member] = segments.as_slice() else { return None };
        let Resolution::Decl(parameter) = owner.res else { return None };
        if member.name.text != "decode" || self.consts.substitution().get(parameter).is_none() {
            return None;
        }
        let Some(Ty::Option(target)) = self.typed(expr) else { return None };
        let span = self.design.expr_node(expr).span;
        let [levels] = args else { return None };
        Some(self.decode_levels(scope, levels.value, &target, span, origin))
    }

    fn decode_levels(&mut self, scope: &mut Scope, levels: ExprId, target: &Ty, span: crate::msl::Span, origin: &Origin) -> Option<Pair> {
        if let Some(owner) = self.impls.hand_written_repr(target).map(|entry| entry.decl) {
            let item: DeclId = self.impls.item(owner, "decode")?;
            let first = self.lower_expr(scope, levels)?;
            let (presence, is_ty) = self.request(item, Half::Presence)?;
            let (payload, val_ty) = self.request(item, Half::Payload)?;
            let second = self.lower_expr(scope, levels)?;
            let is = self.emit(Expr::Call { function: presence, args: vec![first] }, is_ty, origin.clone());
            let val = self.emit(Expr::Call { function: payload, args: vec![second] }, val_ty, origin.clone());
            return Some(Pair { is, val, literal: None });
        }
        let whole = self.lower_expr(scope, levels)?;
        let present = self.emit(Expr::Bool(true), ValueType::Bool, origin.clone());
        let index = |this: &mut Self, k: u128| {
            let at = this.emit(Expr::Int(k as i128), ValueType::UInt(32), origin.clone());
            this.emit(Expr::Index { expr: whole, index: at }, ValueType::Bool, origin.clone())
        };
        match target {
            Ty::Bool => {
                let val = index(self, 0);
                Some(Pair { is: present, val, literal: Some(true) })
            }
            Ty::UInt(bits) if *bits <= 64 => {
                let integer = ValueType::UInt(u32::from(*bits));
                let mut sum = self.emit(Expr::Int(0), integer.clone(), origin.clone());
                for k in 0..u128::from(*bits) {
                    let cond = index(self, k);
                    let weight = self.emit(Expr::Int(1_i128 << k), integer.clone(), origin.clone());
                    let none = self.emit(Expr::Int(0), integer.clone(), origin.clone());
                    let term = self.emit(Expr::If { cond, then: weight, otherwise: none }, integer.clone(), origin.clone());
                    sum = self.emit(Expr::Binary { op: BinaryOp::Add, lhs: sum, rhs: term }, integer.clone(), origin.clone());
                }
                Some(Pair { is: present, val: sum, literal: Some(true) })
            }
            _ => {
                self.unsupported(scope.file, span, "`decode` into a signed integer or a type whose layout is derived (only `bool`, unsigned integers up to 64 bits and hand-written `Repr` types run)");
                None
            }
        }
    }
}
