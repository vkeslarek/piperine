//! The state of value lowering (§1.5, §3): [`ExpressionTables`], what a run learns of the IR expressions it adds, and
//! [`ValueScope`], what the value code being lowered names. Both go to `elab/value/` with the methods that read them.

use std::collections::BTreeMap;

use piperine_ir::{ExprId as LoweredExpr, Local, Origin, StmtId as LoweredStmt, ValueType};

use crate::elab::consts::Bindings;
use super::control::LoopFrame;
use super::exits::Divergences;
use super::returns::{FunctionResult, InlineExit};
use super::types::TypeArena;
use crate::pom::{DeclId, ExprId, FileId};

/// What one run learns of the value code it adds to the IR: the type, continuity and origin of every expression, the
/// expressions that diverge, and the IR type of every struct and enum.
#[derive(Default)]
pub(in crate::elab) struct ExpressionTables {
    /// The type of every IR expression added so far, by id.
    pub(in crate::elab) expr_types: Vec<ValueType>,
    /// Whether each IR expression added so far is continuous (part III §8.2), by id.
    pub(in crate::elab) expr_continuous: Vec<bool>,
    /// Whether each IR statement added so far reads a continuous value, by id.
    pub(in crate::elab) stmt_continuous: Vec<bool>,
    /// The origin of every IR expression added so far, by id.
    pub(in crate::elab) expr_origins: Vec<Origin>,
    /// The diverging expressions lowered so far (`exits.rs`).
    pub(in crate::elab) divergences: Divergences,
    /// The IR type each reached struct and enum has (`types.rs`).
    pub(in crate::elab) type_arena: TypeArena,
}

/// What the value code being lowered names and the control flow it sits in: the owner's file, params and locals, the
/// frame and the constants it reads through, the loops, the `return`s and the temporaries of inlined calls.
pub(in crate::elab) struct ValueScope {
    /// The file of the code being lowered.
    pub(in crate::elab) file: FileId,
    /// Param declaration to its index in the body's params.
    pub(in crate::elab) params: BTreeMap<DeclId, u32>,
    /// The locals the code has declared so far, by index.
    pub(in crate::elab) locals: Vec<Local>,
    /// The index in `locals` of each binding the code has declared so far.
    pub(in crate::elab) local_index: BTreeMap<DeclId, u32>,
    /// The `Option` locals: a `let` binding of type `Option<T>` to its flag local and its
    /// payload local.
    pub(in crate::elab) options: BTreeMap<DeclId, (u32, u32)>,
    /// The frame whose names the code reads (`Elaboration::frames`): 0, the body's own module.
    pub(in crate::elab) frame: usize,
    /// The values of the key params of the body being lowered (`structural.rs`): a condition that
    /// reads only these and constants is folded.
    pub(in crate::elab) keys: Bindings,
    /// The constant each loop variable of the unrolled analog loops the code sits in, and each
    /// parameter of the unrolled recursive calls it sits in, stands for (`unroll.rs`).
    pub(in crate::elab) unrolled: Bindings,
    /// The source expression of each plain `let` met so far: a standalone scope (`derivatives.rs`)
    /// reads a block-local `let` as this expression.
    pub(in crate::elab) let_sources: BTreeMap<DeclId, ExprId>,
    /// The initializer of each local a plain `let` bound: a condition that reads the local reads
    /// what it holds (`selectors.rs`).
    pub(in crate::elab) local_inits: BTreeMap<u32, LoweredExpr>,
    /// The loops the code being lowered sits in, innermost last (`control.rs`).
    pub(in crate::elab) loops: Vec<LoopFrame>,
    /// What a `return` of the function being lowered gives (`returns.rs`); `None` outside a function
    /// whose body returns early.
    pub(in crate::elab) result: Option<FunctionResult>,
    /// How many `if` and `match` decisions that did not fold the code being lowered sits under.
    pub(in crate::elab) decisions: u32,
    /// The `let`s of the temporaries an inlined `Option` call reads (`returns.rs`), declared before the
    /// statement being lowered so every read of the pair sees them bound.
    pub(in crate::elab) pending: Vec<LoweredStmt>,
    /// The inlined `Option` functions being lowered that leave early, innermost last (`returns.rs`).
    pub(super) inline_exits: Vec<InlineExit>,
}

impl ValueScope {
    /// The value scope of code written in `file`: no param, no local, frame 0, outside every loop and function.
    pub(in crate::elab) fn new(file: FileId) -> Self {
        ValueScope {
            file,
            params: BTreeMap::new(),
            locals: Vec::new(),
            local_index: BTreeMap::new(),
            options: BTreeMap::new(),
            frame: 0,
            keys: Bindings::new(),
            unrolled: Bindings::new(),
            let_sources: BTreeMap::new(),
            local_inits: BTreeMap::new(),
            loops: Vec::new(),
            result: None,
            decisions: 0,
            pending: Vec::new(),
            inline_exits: Vec::new(),
        }
    }
}
