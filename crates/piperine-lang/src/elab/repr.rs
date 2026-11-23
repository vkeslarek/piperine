//! A hand-written `impl Repr for T` (part I §2 rules 2 and 4, GN-05), carried end to end: the type's IR entry
//! names its `WIDTH` and two IR functions, `T.encode` (the type to `[bool; WIDTH]`) and `T.decode`
//! (`[bool; WIDTH]` to `Option` of the type), so a carrier of it commits the levels `encode` gives and reads
//! its value back by `decode`.
//!
//! An `Option` result is no IR value of elaboration, so the author's `decode` is two IR functions, one for each
//! half (`T.decode.is`, `T.decode.val`); the IR's `decode` is the function that packs them again.

use piperine_ir::{Expr, FunctionId, FunctionSpec, Local, TypeId as IrTypeId, ValueType};

use super::elaborator::Elaboration;
use super::variants::Half;
use crate::pom::{DeclId, DeclKind};
use crate::types::{ImplTarget, Ty};

/// A hand-written layout whose functions exist, set on its type once they are in the IR.
pub(super) struct HandWrittenRepr {
    /// The arena entry of the type.
    ty: IrTypeId,
    /// The number of levels.
    width: u32,
    /// `T.encode`.
    encode: FunctionId,
    /// `T.decode`, the whole `Option` result.
    decode: FunctionId,
}

impl Elaboration<'_, '_> {
    /// The hand-written layout of every struct and enum the run reached that has an `impl Repr`, its functions
    /// lowered; call before the functions are added to the IR. A type that is not reached has no entry and no
    /// layout. After a diagnostic, none.
    pub(super) fn lower_hand_written_reprs(&mut self) -> Vec<HandWrittenRepr> {
        let impls: Vec<(DeclId, DeclId)> = self
            .impls
            .impls()
            .iter()
            .filter(|entry| entry.builtin_trait == Some("Repr"))
            .filter_map(|entry| match entry.target {
                ImplTarget::Declared(decl) => Some((entry.decl, decl)),
                _ => None,
            })
            .collect();
        let mut layouts = Vec::new();
        for (owner, decl) in impls {
            if self.type_arena.id_of(decl).is_none() {
                continue;
            }
            if let Some(layout) = self.lower_hand_written_repr(owner, decl) {
                layouts.push(layout);
            }
        }
        layouts
    }

    /// The layout the `impl Repr` `owner` gives the type `decl`.
    fn lower_hand_written_repr(&mut self, owner: DeclId, decl: DeclId) -> Option<HandWrittenRepr> {
        let ty = self.type_arena.id_of(decl)?;
        let own = self.value_type(&Self::declared_type(self.design.decl_data(decl).kind.clone(), decl))?;
        let width = self.hand_written_width(owner)?;
        let encode_item = self.impls.item(owner, "encode")?;
        let decode_item = self.impls.item(owner, "decode")?;
        let (encode, _) = self.request(encode_item, Half::Whole)?;
        let (presence, _) = self.request(decode_item, Half::Presence)?;
        let (payload, _) = self.request(decode_item, Half::Payload)?;
        let decode = self.pack_decode(decode_item, &own, width, presence, payload);
        Some(HandWrittenRepr { ty, width, encode, decode })
    }

    /// The checker's type of the struct or enum `decl`, not generic.
    fn declared_type(kind: DeclKind, decl: DeclId) -> Ty {
        match kind {
            DeclKind::Enum(_) => Ty::enumeration(decl),
            _ => Ty::structure(decl),
        }
    }

    /// The folded `WIDTH` of the `impl Repr` `owner`.
    fn hand_written_width(&self, owner: DeclId) -> Option<u32> {
        let width = self.impls.item(owner, "WIDTH")?;
        let DeclKind::Const(data) = &self.design.decl_data(width).kind else { return None };
        u32::try_from(self.types.integer_constant(data.value?)?).ok()
    }

    /// The IR function `Type.decode` that gives the whole `Option` of the author's `decode`: it calls the
    /// two halves with its one argument and packs their results.
    fn pack_decode(&mut self, item: DeclId, own: &ValueType, width: u32, presence: FunctionId, payload: FunctionId) -> FunctionId {
        let origin = self.locator.decl(item);
        let levels = ValueType::Array(Box::new(ValueType::Bool), u64::from(width));
        let option = ValueType::Option(Box::new(own.clone()));
        let is_args = vec![self.emit(Expr::Local(0), levels.clone(), origin.clone())];
        let is = self.emit(Expr::Call { function: presence, args: is_args }, ValueType::Bool, origin.clone());
        let val_args = vec![self.emit(Expr::Local(0), levels.clone(), origin.clone())];
        let value = self.emit(Expr::Call { function: payload, args: val_args }, own.clone(), origin.clone());
        let body = self.emit(Expr::OptionPack { is, value }, option.clone(), origin.clone());
        let levels_param = Local { name: "levels".into(), ty: levels, mutable: false };
        let name = self.impl_qualified_name(item);
        self.variants.add_synthetic(FunctionSpec { name, origin, params: vec![levels_param.clone()], ret: option, locals: vec![levels_param], body })
    }

    /// The `encode` of the hand-written `Repr` of what the carrier `decl` holds; `None` for a derived layout.
    pub(super) fn hand_written_encode(&self, decl: DeclId) -> Option<DeclId> {
        let Some(Ty::Carrier { value, .. }) = self.types.carrier(decl) else { return None };
        let entry = self.impls.hand_written_repr(value)?;
        self.impls.item(entry.decl, "encode")
    }

    /// The initial value of a carrier of a hand-written layout: the levels word `encode` gives for the folded
    /// `expr`. `None` after a diagnostic.
    pub(super) fn encoded_init(&mut self, expr: crate::pom::ExprId, encode: DeclId, frame: usize) -> Option<piperine_ir::Constant> {
        let bindings = self.frame_bindings(frame);
        match self.consts.encoded_word(expr, encode, &bindings) {
            Ok(word) => Some(word),
            Err(fault) => {
                self.const_fault(&fault, "a carrier's initial value");
                None
            }
        }
    }

    /// Writes each layout on its type; the functions are in the IR by now.
    pub(super) fn set_hand_written_reprs(&mut self, layouts: Vec<HandWrittenRepr>) {
        if !self.diagnostics.is_empty() {
            return;
        }
        for layout in layouts {
            self.ir.set_repr(layout.ty, piperine_ir::ReprForm::HandWritten { width: layout.width, encode: layout.encode, decode: layout.decode });
        }
    }
}
