//! Analog operators (`ddt`, `idt`, `transition`, `laplace_*`, …) as a
//! trait + registry. Each operator lowers to a POM `Expr` marker call
//! `__<op>(state_id, resolved_args...)` — the flattener and Builder
//! dispatch on the marker name.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use piperine_lang::parse::ast::{ArrayBody, Expr, Literal};

use crate::lower::*;

use super::expr::resolve_expr;
use super::LowerCtx;

/// One analog operator: `ddt(x)`, `laplace_np(x, num, den)`, etc.
pub(crate) trait AnalogOp: Send + Sync {
    /// Lower `name(args...)` to a POM `Expr` marker call with a state id.
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr;
}

fn arg(args: &[Expr], i: usize, ctx: &mut LowerCtx, default: f64) -> Expr {
    args.get(i).map(|a| resolve_expr(a, ctx)).unwrap_or(Expr::Literal(Literal::Real(default)))
}

/// Build a marker call: `__<name>(state_id, args...)`.
fn marker(name: &str, id: StateId, args: Vec<Expr>) -> Expr {
    let mut all = vec![Expr::Literal(Literal::Int(id.0 as u64))];
    all.extend(args);
    Expr::Call(Box::new(Expr::Ident(name.to_string())), all)
}

struct Ddt;
impl AnalogOp for Ddt {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return Expr::Literal(Literal::Real(0.0)) };
        let x = resolve_expr(a0, ctx);
        let id = ctx.alloc_state(StateKind::Ddt, x.clone());
        marker("__ddt", id, vec![x])
    }
}

struct Idt;
impl AnalogOp for Idt {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return Expr::Literal(Literal::Real(0.0)) };
        let x = resolve_expr(a0, ctx);
        let ic = arg(args, 1, ctx, 0.0);
        let id = ctx.alloc_state(StateKind::Idt { ic: ic.clone() }, x.clone());
        marker("__idt", id, vec![x, ic])
    }
}

struct IdtMod;
impl AnalogOp for IdtMod {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return Expr::Literal(Literal::Real(0.0)) };
        let x = resolve_expr(a0, ctx);
        let ic = arg(args, 1, ctx, 0.0);
        let modulus = arg(args, 2, ctx, 1.0);
        let id = ctx.alloc_state(
            StateKind::IdtMod { ic: ic.clone(), modulus: modulus.clone() },
            x.clone(),
        );
        marker("__idtmod", id, vec![x, ic, modulus])
    }
}

struct Ddx;
impl AnalogOp for Ddx {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 2 {
            return Expr::Literal(Literal::Real(0.0));
        }
        let x = resolve_expr(&args[0], ctx);
        let node_name = super::expr::ident_from_expr(Some(&args[1])).unwrap_or_else(|| "?".into());
        let node = ctx.require_node(&node_name);
        let id = ctx.alloc_state(StateKind::Ddx { node }, x.clone());
        marker("__ddx", id, vec![x, Expr::Literal(Literal::Int(node.0 as u64))])
    }
}

struct Delay;
impl AnalogOp for Delay {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return Expr::Literal(Literal::Real(0.0)) };
        let x = resolve_expr(a0, ctx);
        let delay = arg(args, 1, ctx, 0.0);
        let id = ctx.alloc_state(StateKind::Delay { delay: delay.clone() }, x.clone());
        marker("__delay", id, vec![x, delay])
    }
}

struct Transition;
impl AnalogOp for Transition {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return Expr::Literal(Literal::Real(0.0)) };
        let x = resolve_expr(a0, ctx);
        let delay = arg(args, 1, ctx, 0.0);
        let rise = arg(args, 2, ctx, 0.0);
        let fall = arg(args, 3, ctx, 0.0);
        let tol = arg(args, 4, ctx, 0.0);
        let id = ctx.alloc_state(
            StateKind::Transition { delay: delay.clone(), rise: rise.clone(), fall: fall.clone(), tol: tol.clone() },
            x.clone(),
        );
        marker("__transition", id, vec![x, delay, rise, fall, tol])
    }
}

struct Slew;
impl AnalogOp for Slew {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return Expr::Literal(Literal::Real(0.0)) };
        let x = resolve_expr(a0, ctx);
        let rise = arg(args, 1, ctx, 0.0);
        let fall = arg(args, 2, ctx, 0.0);
        let id = ctx.alloc_state(StateKind::Slew { rise: rise.clone(), fall: fall.clone() }, x.clone());
        marker("__slew", id, vec![x, rise, fall])
    }
}

/// `table(x, xs, ys[, mode])` — 1-D measured-data lookup (spec Part V §2).
/// `xs`/`ys` must be constant real arrays with `xs` strictly increasing;
/// interpolation is linear with end clamp. Lowered to a **pure piecewise
/// expression** (nested selects) — no state, so the Jacobian falls out of
/// the normal symbolic-diff path as the segment slope.
struct Table;
impl Table {
    fn const_array(e: &Expr, ctx: &mut LowerCtx) -> Option<Vec<f64>> {
        let Expr::Array(ArrayBody::List(list)) = e else { return None };
        list.iter()
            .map(|item| match resolve_expr(item, ctx) {
                Expr::Literal(Literal::Real(v)) => Some(v),
                Expr::Literal(Literal::Int(v)) => Some(v as f64),
                _ => None,
            })
            .collect()
    }

    fn err(ctx: &mut LowerCtx, what: &'static str) -> Expr {
        ctx.errors.push(super::LowerError {
            module: ctx.module_name.clone(),
            what,
            name: "table".to_string(),
        });
        Expr::Literal(Literal::Real(0.0))
    }
}
impl AnalogOp for Table {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 3 {
            return Self::err(ctx, "table(x, xs, ys) needs at least 3 arguments");
        }
        if let Some(mode) = args.get(3)
            && !matches!(mode, Expr::Literal(Literal::String(s)) if s == "linear")
        {
            return Self::err(ctx, "table interpolation mode: only \"linear\" is supported");
        }
        let Some(xs) = Self::const_array(&args[1], ctx) else {
            return Self::err(ctx, "table breakpoints (xs) must be a constant real array");
        };
        let Some(ys) = Self::const_array(&args[2], ctx) else {
            return Self::err(ctx, "table data (ys) must be a constant real array");
        };
        if xs.len() != ys.len() {
            return Self::err(ctx, "table xs and ys must have the same length");
        }
        if xs.len() < 2 {
            return Self::err(ctx, "table needs at least 2 points");
        }
        if !xs.windows(2).all(|w| w[0] < w[1]) {
            return Self::err(ctx, "table breakpoints (xs) must be strictly increasing");
        }
        let x = resolve_expr(&args[0], ctx);

        let real = |v: f64| Expr::Literal(Literal::Real(v));
        let block = |e: Expr| piperine_lang::parse::ast::Block {
            stmts: Vec::new(),
            expr: Some(Box::new(e)),
        };
        // Segment i: ys[i] + (x − xs[i]) · slope_i.
        let seg = |i: usize, x: &Expr| {
            let slope = (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]);
            Expr::Binary(
                Box::new(real(ys[i])),
                piperine_lang::parse::ast::BinaryOp::Add,
                Box::new(Expr::Binary(
                    Box::new(Expr::Binary(
                        Box::new(x.clone()),
                        piperine_lang::parse::ast::BinaryOp::Sub,
                        Box::new(real(xs[i])),
                    )),
                    piperine_lang::parse::ast::BinaryOp::Mul,
                    Box::new(real(slope)),
                )),
            )
        };
        // Fold back-to-front: else-most = last segment (extrapolation is a
        // flat clamp on both ends).
        let n = xs.len();
        let mut expr = real(ys[n - 1]); // x ≥ xs[n−1] → clamp high
        for i in (0..n - 1).rev() {
            // x < xs[i+1] → segment i (for i = 0 the low clamp is applied
            // below), else previous accumulation.
            expr = Expr::If {
                cond: Box::new(Expr::Binary(
                    Box::new(x.clone()),
                    piperine_lang::parse::ast::BinaryOp::Lt,
                    Box::new(real(xs[i + 1])),
                )),
                then_body: block(seg(i, &x)),
                else_body: block(expr),
            };
        }
        // Low clamp: x ≤ xs[0] → ys[0].
        Expr::If {
            cond: Box::new(Expr::Binary(
                Box::new(x.clone()),
                piperine_lang::parse::ast::BinaryOp::Lt,
                Box::new(real(xs[0])),
            )),
            then_body: block(real(ys[0])),
            else_body: block(expr),
        }
    }
}

struct Laplace {
    variant: &'static str,
}
impl AnalogOp for Laplace {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 3 {
            return Expr::Literal(Literal::Real(0.0));
        }
        let x = resolve_expr(&args[0], ctx);
        let to_vec = |e: &Expr, ctx: &mut LowerCtx| -> Vec<Expr> {
            if let Expr::Array(ArrayBody::List(list)) = e {
                list.iter().map(|item| resolve_expr(item, ctx)).collect()
            } else {
                vec![resolve_expr(e, ctx)]
            }
        };
        let num = to_vec(&args[1], ctx);
        let den = to_vec(&args[2], ctx);
        let variant = match self.variant {
            "zp" => LaplaceKind::ZerosPoles,
            "np" => LaplaceKind::NumPoles,
            "zd" => LaplaceKind::ZerosDen,
            _ => LaplaceKind::NumDen,
        };
        let id = ctx.alloc_state(
            StateKind::Laplace { variant, num: num.clone(), den: den.clone() },
            x.clone(),
        );
        let mut marker_args = vec![x];
        marker_args.push(Expr::Array(ArrayBody::List(num)));
        marker_args.push(Expr::Array(ArrayBody::List(den)));
        marker("__laplace", id, marker_args)
    }
}

struct ZTransform {
    variant: &'static str,
}
impl AnalogOp for ZTransform {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 4 {
            return Expr::Literal(Literal::Real(0.0));
        }
        let x = resolve_expr(&args[0], ctx);
        let to_vec = |e: &Expr, ctx: &mut LowerCtx| -> Vec<Expr> {
            if let Expr::Array(ArrayBody::List(list)) = e {
                list.iter().map(|item| resolve_expr(item, ctx)).collect()
            } else {
                vec![resolve_expr(e, ctx)]
            }
        };
        let num = to_vec(&args[1], ctx);
        let den = to_vec(&args[2], ctx);
        let sample_dt = resolve_expr(&args[3], ctx);
        let variant = match self.variant {
            "zp" => ZKind::ZerosPoles,
            "np" => ZKind::NumPoles,
            "zd" => ZKind::ZerosDen,
            _ => ZKind::NumDen,
        };
        let id = ctx.alloc_state(
            StateKind::ZTransform { variant, num: num.clone(), den: den.clone(), sample_dt: sample_dt.clone() },
            x.clone(),
        );
        let mut marker_args = vec![x];
        marker_args.push(Expr::Array(ArrayBody::List(num)));
        marker_args.push(Expr::Array(ArrayBody::List(den)));
        marker_args.push(sample_dt);
        marker("__ztransform", id, marker_args)
    }
}

struct AcStim;
impl AnalogOp for AcStim {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let mag = arg(args, 0, ctx, 1.0);
        let phase = arg(args, 1, ctx, 0.0);
        Expr::SysCall("$ac_stim".to_string(), vec![mag, phase])
    }
}

struct NoiseCall;
impl AnalogOp for NoiseCall {
    fn lower(&self, _args: &[Expr], _ctx: &mut LowerCtx) -> Expr {
        Expr::Literal(Literal::Real(0.0))
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
        r.register(&["table"], Table);
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
