//! Name resolution and typed-value tagging shared by the digital and analog
//! emission paths.

use std::collections::HashMap;

use cranelift_codegen::ir::Value;

use crate::resolve::{FnId, NodeId, ParamId, SymbolTable, VarId};

// ─── Name resolution ──────────────────────────────────────────────────────────

/// Name → resolved id maps, built once from the `SymbolTable`.
pub struct Resolver {
    pub vars: HashMap<String, VarId>,
    pub nodes: HashMap<String, NodeId>,
    pub params: HashMap<String, ParamId>,
    pub fns: HashMap<String, FnId>,
}

impl Resolver {
    pub fn from_symbols(symbols: &SymbolTable) -> Self {
        Self {
            vars: symbols.vars().map(|(id, v)| (v.name.clone(), id)).collect(),
            nodes: symbols.nodes().map(|(id, n)| (n.name.clone(), id)).collect(),
            params: symbols.params().map(|(id, p)| (p.name.clone(), id)).collect(),
            fns: symbols.fns().map(|(id, f)| (f.name.clone(), id)).collect(),
        }
    }

    /// `$param_given("name")` resolution: exact param name first, then a
    /// unique flattened bundle field (`narrow` → `model_narrow`) — the
    /// syscall's argument predates bundle flattening. Mirrors
    /// `LowerCtx::require_param_given`; keep the two in sync.
    pub fn param_given(&self, name: &str) -> Option<ParamId> {
        if let Some(&id) = self.params.get(name) {
            return Some(id);
        }
        let suffix = format!("_{name}");
        let mut matches = self.params.iter().filter(|(n, _)| n.ends_with(&suffix));
        match (matches.next(), matches.next()) {
            (Some((_, &id)), None) => Some(id),
            _ => None,
        }
    }
}

// ─── Typed values ─────────────────────────────────────────────────────────────

/// A value plus its digital type.
#[derive(Clone, Copy)]
pub struct Typed {
    pub value: Value,
    pub ty: DigTy,
}

impl Typed {
    pub fn real(value: Value) -> Self {
        Self { value, ty: DigTy::Real }
    }

    pub fn int(value: Value) -> Self {
        Self { value, ty: DigTy::Int }
    }

    pub fn quad(value: Value) -> Self {
        Self { value, ty: DigTy::Quad }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DigTy {
    /// Two-state integer/boolean (`i64`).
    Int,
    /// `f64`.
    Real,
    /// Four-state logic in `i64`: 0, 1, 2 = X, 3 = Z.
    Quad,
}
