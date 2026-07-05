//! Analog operators (`ddt`, `idt`, `transition`, `laplace_*`, …) as a
//! trait + registry, mirroring the [`EventKind`](crate::elab::event::EventKind)
//! pattern already used for `@`-events.
//!
//! Each operator owns its own lowering logic as an [`AnalogOp`] impl
//! instead of living as one arm in a giant `match` in `lower_call`. Adding
//! a new operator means adding a struct + a `register` call here, not
//! growing a shared match statement.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use piperine_lang::parse::ast::Expr;
use crate::lower::*;

use super::expr::lower_expr;
use super::LowerCtx;

/// One analog operator: `ddt(x)`, `laplace_np(x, num, den)`, etc.
pub(crate) trait AnalogOp: Send + Sync {
    /// Lower `name(args...)` (already resolved to this op) to an [`IrExpr`],
    /// typically an [`IrExpr::StateRef`] allocated via `ctx.alloc_state`.
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr;
}

fn arg(args: &[Expr], i: usize, ctx: &mut LowerCtx, default: f64) -> IrExpr {
    args.get(i).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(default))
}

struct Ddt;
impl AnalogOp for Ddt {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let Some(a0) = args.first() else { return IrExpr::Real(0.0) };
        let x = lower_expr(a0, ctx);
        let id = ctx.alloc_state(StateKind::Ddt, x);
        IrExpr::State(id)
    }
}

struct Idt;
impl AnalogOp for Idt {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let Some(a0) = args.first() else { return IrExpr::Real(0.0) };
        let x = lower_expr(a0, ctx);
        let ic = arg(args, 1, ctx, 0.0);
        let id = ctx.alloc_state(StateKind::Idt { ic }, x);
        IrExpr::State(id)
    }
}

struct IdtMod;
impl AnalogOp for IdtMod {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let Some(a0) = args.first() else { return IrExpr::Real(0.0) };
        let x = lower_expr(a0, ctx);
        let ic = arg(args, 1, ctx, 0.0);
        let modulus = arg(args, 2, ctx, 1.0);
        let id = ctx.alloc_state(StateKind::IdtMod { ic, modulus }, x);
        IrExpr::State(id)
    }
}

struct Ddx;
impl AnalogOp for Ddx {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        if args.len() < 2 {
            return IrExpr::Real(0.0);
        }
        let x = lower_expr(&args[0], ctx);
        let node_name = super::expr::ident_from_expr(Some(&args[1])).unwrap_or_else(|| "?".into());
        let node = ctx.require_node(&node_name);
        let id = ctx.alloc_state(StateKind::Ddx { node }, x);
        IrExpr::State(id)
    }
}

struct Delay;
impl AnalogOp for Delay {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let Some(a0) = args.first() else { return IrExpr::Real(0.0) };
        let x = lower_expr(a0, ctx);
        let delay = arg(args, 1, ctx, 0.0);
        let id = ctx.alloc_state(StateKind::Delay { delay }, x);
        IrExpr::State(id)
    }
}

struct Transition;
impl AnalogOp for Transition {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let Some(a0) = args.first() else { return IrExpr::Real(0.0) };
        let x = lower_expr(a0, ctx);
        let delay = arg(args, 1, ctx, 0.0);
        let rise = arg(args, 2, ctx, 0.0);
        let fall = arg(args, 3, ctx, 0.0);
        let tol = arg(args, 4, ctx, 0.0);
        let id = ctx.alloc_state(StateKind::Transition { delay, rise, fall, tol }, x);
        IrExpr::State(id)
    }
}

struct Slew;
impl AnalogOp for Slew {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let Some(a0) = args.first() else { return IrExpr::Real(0.0) };
        let x = lower_expr(a0, ctx);
        let rise = arg(args, 1, ctx, 0.0);
        let fall = arg(args, 2, ctx, 0.0);
        let id = ctx.alloc_state(StateKind::Slew { rise, fall }, x);
        IrExpr::State(id)
    }
}

/// `laplace_np` / `laplace_zp` / `laplace_pm` / `laplace_nm` / `laplace_npm`
/// — one struct, five registrations differing only by `variant`.
struct Laplace {
    variant: &'static str,
}
impl AnalogOp for Laplace {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        if args.len() < 3 {
            return IrExpr::Real(0.0);
        }
        let x = lower_expr(&args[0], ctx);
        
        let to_vec = |e: &Expr, ctx: &mut LowerCtx| -> Vec<IrExpr> {
            if let Expr::Array(piperine_lang::parse::ast::ArrayBody::List(list)) = e {
                list.iter().map(|item| lower_expr(item, ctx)).collect()
            } else {
                vec![lower_expr(e, ctx)]
            }
        };
        let num = to_vec(&args[1], ctx);
        let den = to_vec(&args[2], ctx);
        
        let variant = match self.variant {
            "zp" => crate::lower::LaplaceKind::ZerosPoles,
            "np" => crate::lower::LaplaceKind::NumPoles,
            "zd" => crate::lower::LaplaceKind::ZerosDen,
            _ => crate::lower::LaplaceKind::NumDen,
        };
        let id = ctx.alloc_state(
            StateKind::Laplace { variant, num, den },
            x,
        );
        IrExpr::State(id)
    }
}

/// `zi_zd` / `zi_zp` / `zi_nd` / `zi_np` — one struct, four registrations.
struct ZTransform {
    variant: &'static str,
}
impl AnalogOp for ZTransform {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        if args.len() < 4 {
            return IrExpr::Real(0.0);
        }
        let x = lower_expr(&args[0], ctx);
        
        let to_vec = |e: &Expr, ctx: &mut LowerCtx| -> Vec<IrExpr> {
            if let Expr::Array(piperine_lang::parse::ast::ArrayBody::List(list)) = e {
                list.iter().map(|item| lower_expr(item, ctx)).collect()
            } else {
                vec![lower_expr(e, ctx)]
            }
        };
        let num = to_vec(&args[1], ctx);
        let den = to_vec(&args[2], ctx);
        let sample_dt = lower_expr(&args[3], ctx);
        
        let variant = match self.variant {
            "zp" => crate::lower::ZKind::ZerosPoles,
            "np" => crate::lower::ZKind::NumPoles,
            "zd" => crate::lower::ZKind::ZerosDen,
            _ => crate::lower::ZKind::NumDen,
        };
        
        let id = ctx.alloc_state(
            StateKind::ZTransform { variant, num, den, sample_dt },
            x,
        );
        IrExpr::State(id)
    }
}

struct AcStim;
impl AnalogOp for AcStim {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
        let mag = arg(args, 0, ctx, 1.0);
        let phase = arg(args, 1, ctx, 0.0);
        IrExpr::AcStim { mag: Box::new(mag), phase: Box::new(phase) }
    }
}

/// `white_noise`/`flicker_noise` are extracted separately by `scan_noise`
/// (which walks contribution RHS trees before lowering); in expression
/// position they contribute nothing to the residual itself.
struct NoiseCall;
impl AnalogOp for NoiseCall {
    fn lower(&self, _args: &[Expr], _ctx: &mut LowerCtx) -> IrExpr {
        IrExpr::Real(0.0)
    }
}

pub(crate) struct AnalogOpRegistry {
    ops: HashMap<String, Arc<dyn AnalogOp + Send + Sync>>,
}

impl AnalogOpRegistry {
    fn register(&mut self, names: &[&str], op: impl AnalogOp + 'static) {
        let op: Arc<dyn AnalogOp + Send + Sync> = Arc::new(op);
        for &n in names {
            self.ops.insert(n.to_string(), op.clone());
        }
    }

    fn with_builtins() -> Self {
        let mut r = Self { ops: HashMap::new() };
        r.register(&["ddt"], Ddt);
        r.register(&["idt"], Idt);
        r.register(&["idtmod"], IdtMod);
        r.register(&["ddx"], Ddx);
        r.register(&["delay", "absdelay"], Delay);
        r.register(&["transition"], Transition);
        r.register(&["slew"], Slew);
        for variant in ["np", "zp", "pm", "nm", "npm"] {
            r.register(&[&format!("laplace_{variant}")], Laplace { variant });
        }
        for variant in ["zd", "zp", "nd", "np"] {
            r.register(&[&format!("zi_{variant}")], ZTransform { variant });
        }
        r.register(&["ac_stim"], AcStim);
        r.register(&["white_noise", "flicker_noise"], NoiseCall);
        r
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<Arc<dyn AnalogOp + Send + Sync>> {
        self.ops.get(name).cloned()
    }
}

pub(crate) fn analog_ops() -> &'static AnalogOpRegistry {
    static REGISTRY: LazyLock<AnalogOpRegistry> = LazyLock::new(AnalogOpRegistry::with_builtins);
    &REGISTRY
}
