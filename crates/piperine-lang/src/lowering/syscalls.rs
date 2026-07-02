//! System functions (`$temperature`, `$vt`, `$simparam`, …) as a trait +
//! registry, mirroring [`analog_ops`](super::analog_ops).
//!
//! Adding a new `$`-syscall means adding a struct + a `register` call
//! here, not growing the shared match in `lower_syscall`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use crate::parse::ast::{Expr, Literal};
use piperine_codegen::ir::*;

use super::expr::lower_expr;
use super::LowerCtx;

/// One `$`-prefixed system function. `name` is the syscall name actually
/// invoked (lowercased, `$` stripped) — most impls ignore it, but the
/// `dist_*` family needs it to build the right [`SimQuery::Random`] kind.
pub(crate) trait SystemFunction: Send + Sync {
    fn lower(&self, name: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr;
}

fn string_arg(args: &[Expr], i: usize) -> String {
    match args.get(i) {
        Some(Expr::Literal(Literal::String(s))) => s.clone(),
        _ => "?".into(),
    }
}

struct Temperature;
impl SystemFunction for Temperature {
    fn lower(&self, _: &str, _args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::Temperature)
    }
}

struct Vt;
impl SystemFunction for Vt {
    fn lower(&self, _: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        if args.is_empty() {
            IrExpr::Sim(SimQuery::Vt(None))
        } else {
            IrExpr::Sim(SimQuery::Vt(Some(Box::new(lower_expr(&args[0], ctx)))))
        }
    }
}

struct Abstime;
impl SystemFunction for Abstime {
    fn lower(&self, _: &str, _args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::Abstime)
    }
}

struct Mfactor;
impl SystemFunction for Mfactor {
    fn lower(&self, _: &str, _args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::Mfactor)
    }
}

struct XPosition;
impl SystemFunction for XPosition {
    fn lower(&self, _: &str, _args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::Position(piperine_codegen::ir::Axis::X))
    }
}

struct YPosition;
impl SystemFunction for YPosition {
    fn lower(&self, _: &str, _args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::Position(piperine_codegen::ir::Axis::Y))
    }
}

struct Angle;
impl SystemFunction for Angle {
    fn lower(&self, _: &str, _args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::Angle)
    }
}

struct Simparam;
impl SystemFunction for Simparam {
    fn lower(&self, _: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let key = string_arg(args, 0);
        let default = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
        IrExpr::Sim(SimQuery::Simparam { key, default: Box::new(default) })
    }
}

struct ParamGiven;
impl SystemFunction for ParamGiven {
    fn lower(&self, _: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::ParamGiven(ctx.lookup_param(&string_arg(args, 0)).unwrap_or(piperine_codegen::ir::ParamId(0))))
    }
}

struct PortConnected;
impl SystemFunction for PortConnected {
    fn lower(&self, _: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Sim(SimQuery::PortConnected(ctx.lookup_node(&string_arg(args, 0)).unwrap_or(piperine_codegen::ir::NodeId::GROUND)))
    }
}

struct Limit;
impl SystemFunction for Limit {
    fn lower(&self, _: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let kind = string_arg(args, 0);
        let limit_args = args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect();
        IrExpr::Sim(SimQuery::Limit { kind, args: limit_args })
    }
}

struct Analysis;
impl SystemFunction for Analysis {
    fn lower(&self, _: &str, args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        let kind = match args.first() {
            Some(Expr::Literal(Literal::String(s))) => s.clone(),
            _ => "dc".into(),
        };
        let kind = match string_arg(args, 0).as_str() {
            "ac" => piperine_codegen::ir::Analysis::Ac,
            "dc" => piperine_codegen::ir::Analysis::Dc,
            "tran" => piperine_codegen::ir::Analysis::Tran,
            "noise" => piperine_codegen::ir::Analysis::Noise,
            _ => piperine_codegen::ir::Analysis::Dc,
        };
        IrExpr::Sim(SimQuery::Analysis(kind))
    }
}

/// Handles bare `$random` and the whole `$dist_*` family — `name` carries
/// which one was actually called.
struct Random;
impl SystemFunction for Random {
    fn lower(&self, name: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let dist_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
        IrExpr::Sim(SimQuery::Random { kind: name.to_string(), args: dist_args })
    }
}

pub(crate) struct SyscallRegistry {
    funcs: HashMap<&'static str, Arc<dyn SystemFunction + Send + Sync>>,
}

impl SyscallRegistry {
    fn register(&mut self, name: &'static str, f: impl SystemFunction + 'static) {
        self.funcs.insert(name, Arc::new(f));
    }

    fn with_builtins() -> Self {
        let mut r = Self { funcs: HashMap::new() };
        r.register("temperature", Temperature);
        r.register("vt", Vt);
        r.register("abstime", Abstime);
        r.register("mfactor", Mfactor);
        r.register("xposition", XPosition);
        r.register("yposition", YPosition);
        r.register("angle", Angle);
        r.register("simparam", Simparam);
        r.register("param_given", ParamGiven);
        r.register("port_connected", PortConnected);
        r.register("limit", Limit);
        r.register("analysis", Analysis);
        r.register("random", Random);
        r
    }

    /// Looks up the handler for `name` (already lowercased, `$` stripped).
    /// Falls back to the `Random` handler for the whole `dist_*` family,
    /// since those all produce a `SimQuery::Random` differing only by kind.
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
