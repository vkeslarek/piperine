//! A hand-written `impl Repr for T` (part I §2 rules 2 and 4, GN-05), carried end to end: the type's IR entry
//! names its `WIDTH` and two IR functions, `T.encode` (the type to `[bool; WIDTH]`) and `T.decode`
//! (`[bool; WIDTH]` to `Option` of the type), so a carrier of it commits the levels `encode` gives and reads
//! its value back by `decode`.
//!
//! An `Option` result is no IR value of elaboration, so the author's `decode` is two IR functions, one for each
//! half (`T.decode.is`, `T.decode.val`); the IR's `decode` is the function that packs them again.


use super::ValueLowering;
use piperine_ir::{Expr, FunctionId, FunctionSpec, Local, TypeId as IrTypeId, ValueType};

use crate::elab::variants::Half;
use crate::pom::{DeclId, DeclKind};
use crate::elab::instances::{ConcreteValue, Instance};
use crate::types::{ImplTarget, Nominal, Ty};

/// A hand-written layout whose functions exist, set on its type once they are in the IR.
pub(in crate::elab) struct HandWrittenRepr {
    /// The arena entry of the type.
    ty: IrTypeId,
    /// The number of levels.
    width: u32,
    /// `T.encode`.
    encode: FunctionId,
    /// `T.decode`, the whole `Option` result.
    decode: FunctionId,
}

impl ValueLowering<'_, '_, '_> {
    /// The hand-written layout of every struct and enum the run reached that has an `impl Repr`, its functions
    /// lowered; call before the functions are added to the IR. A type that is not reached has no entry and no
    /// layout. After a diagnostic, none.
    pub(in crate::elab) fn lower_hand_written_reprs(&mut self) -> Vec<HandWrittenRepr> {
        let impls: Vec<(DeclId, DeclId)> = self
            .state.impls
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
            if self.state.values.type_arena.id_of(decl).is_none() {
                // a generic type has no entry of its own: each instance the run reached gets the layout of the `impl` for its
                // arguments, `Pair<f64>.encode` and `Pair<f64>.decode` (GN-32 AC 1)
                for (nominal, ty) in self.state.values.type_arena.instances_of(decl) {
                    if let Some(layout) = self.lower_hand_written_instance(owner, &nominal, ty) {
                        layouts.push(layout);
                    }
                }
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
        let ty = self.state.values.type_arena.id_of(decl)?;
        let own = self.value_type(&Self::declared_type(self.state.design.decl_data(decl).kind.clone(), decl))?;
        let width = self.hand_written_width(owner, &[])?;
        let encode_item = self.state.impls.item(owner, "encode")?;
        let decode_item = self.state.impls.item(owner, "decode")?;
        let (encode, _) = self.request(encode_item, Half::Whole)?;
        let (presence, _) = self.request(decode_item, Half::Presence)?;
        let (payload, _) = self.request(decode_item, Half::Payload)?;
        let decode = self.pack_decode(decode_item, &[], &own, width, presence, payload);
        Some(HandWrittenRepr { ty, width, encode, decode })
    }

    /// The layout the generic `impl Repr` `owner` gives the instance `nominal` of its type, registered as the arena
    /// entry `ty`: `encode` and `decode` lowered once for the instance's arguments (§3.8.10).
    fn lower_hand_written_instance(&mut self, owner: DeclId, nominal: &Nominal, ty: IrTypeId) -> Option<HandWrittenRepr> {
        let instance = Self::declared_type(self.state.design.decl_data(nominal.decl).kind.clone(), nominal.decl);
        let instance = match instance {
            Ty::Enum(_) => Ty::Enum(nominal.clone()),
            _ => Ty::Struct(nominal.clone()),
        };
        let arguments = self.hand_written_arguments(owner, &instance)?;
        let own = self.value_type(&instance)?;
        let width = self.hand_written_width(owner, &arguments)?;
        let encode_item = self.state.impls.item(owner, "encode")?;
        let decode_item = self.state.impls.item(owner, "decode")?;
        let (encode, _) = self.request_instance(encode_item, arguments.clone(), Half::Whole)?;
        let (presence, _) = self.request_instance(decode_item, arguments.clone(), Half::Presence)?;
        let (payload, _) = self.request_instance(decode_item, arguments.clone(), Half::Payload)?;
        let decode = self.pack_decode(decode_item, &arguments, &own, width, presence, payload);
        Some(HandWrittenRepr { ty, width, encode, decode })
    }

    /// The checker's type of the struct or enum `decl`, not generic.
    fn declared_type(kind: DeclKind, decl: DeclId) -> Ty {
        match kind {
            DeclKind::Enum(_) => Ty::enumeration(decl),
            _ => Ty::structure(decl),
        }
    }

    /// The IR function `Type.decode` that gives the whole `Option` of the author's `decode`: it calls the
    /// two halves with its one argument and packs their results.
    fn pack_decode(&mut self, item: DeclId, arguments: &[ConcreteValue], own: &ValueType, width: u32, presence: FunctionId, payload: FunctionId) -> FunctionId {
        let origin = self.state.locator.decl(item);
        let levels = ValueType::Array(Box::new(ValueType::Bool), u64::from(width));
        let option = ValueType::Option(Box::new(own.clone()));
        let is_args = vec![self.emit(Expr::Local(0), levels.clone(), origin.clone())];
        let is = self.emit(Expr::Call { function: presence, args: is_args }, ValueType::Bool, origin.clone());
        let val_args = vec![self.emit(Expr::Local(0), levels.clone(), origin.clone())];
        let value = self.emit(Expr::Call { function: payload, args: val_args }, own.clone(), origin.clone());
        let body = self.emit(Expr::OptionPack { is, value }, option.clone(), origin.clone());
        let levels_param = Local { name: "levels".into(), ty: levels, mutable: false };
        let name = if arguments.is_empty() { self.impl_qualified_name(item) } else { Instance { item, arguments: arguments.to_vec() }.name(self) };
        self.state.functions.variants.add_synthetic(FunctionSpec { name, origin, params: vec![levels_param.clone()], ret: option, locals: vec![levels_param], body })
    }

    /// The `encode` of the hand-written `Repr` of what the carrier `decl` holds, with the arguments its `impl` takes for
    /// that type; `None` for a derived layout.
    pub(in crate::elab) fn hand_written_encode(&self, decl: DeclId) -> Option<(DeclId, Vec<ConcreteValue>)> {
        let Some(Ty::Carrier { value, .. }) = self.state.consts.typed_carrier(decl) else { return None };
        let entry = self.state.impls.hand_written_repr(&value)?;
        let arguments = self.hand_written_arguments(entry.decl, &value)?;
        Some((self.state.impls.item(entry.decl, "encode")?, arguments))
    }

    /// The initial value of a carrier of a hand-written layout: the levels word `encode` gives for the folded
    /// `expr`, run for the arguments of its `impl`. `None` after a diagnostic.
    pub(in crate::elab) fn encoded_init(&mut self, expr: crate::pom::ExprId, encode: DeclId, arguments: &[ConcreteValue], frame: usize) -> Option<piperine_ir::Constant> {
        let bindings = self.frame_bindings(frame);
        let outer = self.elaboration.state.consts.set_substitution(self.instance_substitution(encode, arguments));
        let word = self.state.consts.encoded_word(expr, encode, &bindings);
        self.state.consts.set_substitution(outer);
        match word {
            Ok(word) => Some(word),
            Err(fault) => {
                self.const_fault(&fault, "a carrier's initial value");
                None
            }
        }
    }

    /// The levels word a carrier of a hand-written layout starts at when it writes no initial value: the levels `encode` gives
    /// for the `default` value of its type (GN-32 AC 2, §3.3.4), run for the arguments of its `impl`. `None` after a diagnostic.
    pub(in crate::elab) fn encoded_default(&mut self, encode: DeclId, arguments: &[ConcreteValue], default: piperine_ir::Constant) -> Option<piperine_ir::Constant> {
        let DeclKind::Fn(data) = &self.state.design.decl_data(encode).kind else { return None };
        let body = data.body.clone()?;
        let Some(crate::pom::StmtKind::Expr { expr: at, .. }) = body.stmts.last().map(|stmt| self.state.design.stmt_node(*stmt).kind.clone()) else { return None };
        let tree = self.imported_tree(default);
        let outer = self.elaboration.state.consts.set_substitution(self.instance_substitution(encode, arguments));
        let word = self.state.consts.encoded_tree_word(at, encode, &tree);
        self.state.consts.set_substitution(outer);
        match word {
            Ok(word) => Some(word),
            Err(fault) => {
                self.const_fault(&fault, "the levels of a `Default` value");
                None
            }
        }
    }

    /// Writes each layout on its type; the functions are in the IR by now.
    pub(in crate::elab) fn set_hand_written_reprs(&mut self, layouts: Vec<HandWrittenRepr>) {
        if !self.state.diagnostics.is_empty() {
            return;
        }
        for layout in layouts {
            self.state.builder.set_repr(layout.ty, piperine_ir::ReprForm::HandWritten { width: layout.width, encode: layout.encode, decode: layout.decode });
        }
    }
}
