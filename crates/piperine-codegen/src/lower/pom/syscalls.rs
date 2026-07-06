//! System functions (`$temperature`, `$vt`, `$simparam`, …) — for the POM
//! analog path, syscalls are kept as `Expr::SysCall` with resolved args.
//! The Builder dispatches on the syscall name at JIT emit time.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use piperine_lang::parse::ast::Expr;

use super::expr::resolve_expr;
use super::LowerCtx;

/// One `$`-prefixed system function. For the POM path, most syscalls are
/// kept as `Expr::SysCall` — the Builder resolves them at emit time.
#[allow(dead_code)]
pub(crate) trait SystemFunction: Send + Sync {
    fn lower(&self, name: &str, args: &[Expr], ctx: &mut LowerCtx) -> Expr;
}

/// Default: keep the syscall as-is with resolved args.
#[allow(dead_code)]
struct KeepSyscall;
impl SystemFunction for KeepSyscall {
    fn lower(&self, name: &str, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let resolved: Vec<Expr> = args.iter().map(|a| resolve_expr(a, ctx)).collect();
        Expr::SysCall(name.to_string(), resolved)
    }
}

#[allow(dead_code)]
pub(crate) struct SyscallRegistry {
    funcs: HashMap<&'static str, Arc<dyn SystemFunction + Send + Sync>>,
}

impl SyscallRegistry {
    fn register(&mut self, name: &'static str, f: impl SystemFunction + 'static) {
        self.funcs.insert(name, Arc::new(f));
    }

    fn with_builtins() -> Self {
        let mut r = Self { funcs: HashMap::new() };
        let names = [
            "temperature", "vt", "abstime", "mfactor",
            "xposition", "yposition", "angle", "simparam",
            "param_given", "port_connected", "limit", "analysis", "random",
        ];
        for n in names {
            r.register(n, KeepSyscall);
        }
        r
    }

    /// Looks up the handler for `name` (already lowercased, `$` stripped).
    /// Falls back to the `Random` handler for the whole `dist_*` family.
    pub(crate) fn lookup(&self, name: &str) -> Option<Arc<dyn SystemFunction + Send + Sync>> {
        if let Some(f) = self.funcs.get(name) {
            return Some(f.clone());
        }
        if name.starts_with("dist_") {
            return self.funcs.get("random").cloned();
        }
        None
    }
}

pub(crate) fn syscalls() -> &'static SyscallRegistry {
    static REGISTRY: LazyLock<SyscallRegistry> = LazyLock::new(SyscallRegistry::with_builtins);
    &REGISTRY
}
