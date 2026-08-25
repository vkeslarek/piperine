//! Analog operators (`ddt`, `idt`, `transition`, `laplace_*`, …) as a
//! trait + registry. Each operator lowers to a POM `Expr` marker call
//! `__<op>(state_id, resolved_args...)` — the flattener and Builder
//! dispatch on the marker name.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use piperine_lang::ast::{BinOp as VBinOp, Expr};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::NumberLit;

use crate::resolve::*;

use super::expr::resolve_expr;
use super::LowerCtx;

const NOSPAN: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn real(v: f64) -> Expr {
    Expr::Number { span: NOSPAN, value: NumberLit::Real(v) }
}

fn int(v: i64) -> Expr {
    Expr::Number { span: NOSPAN, value: NumberLit::Integer(v) }
}

/// One analog operator: `ddt(x)`, `laplace_np(x, num, den)`, etc.
pub(crate) trait AnalogOp: Send + Sync {
    /// Lower `name(args...)` to a POM `Expr` marker call with a state id.
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr;
}

fn arg(args: &[Expr], i: usize, ctx: &mut LowerCtx, default: f64) -> Expr {
    args.get(i).map(|a| resolve_expr(a, ctx)).unwrap_or_else(|| real(default))
}

/// Build a marker call: `__<name>(state_id, args...)`.
fn marker(name: &str, id: StateId, args: Vec<Expr>) -> Expr {
    let mut all = vec![int(id.0 as i64)];
    all.extend(args);
    Expr::Call { span: NOSPAN, name: name.to_string(), args: all }
}

/// A VAMS `{a, b, c}` concatenation used as a coefficient-list carrier
/// (real Verilog-A spells `laplace_nd(x, {1}, {1, tau})`'s num/den lists
/// this way — LRM §4.6.1's `concatenation` production, not an array
/// literal, which VAMS has no syntax for at all). A bare non-`Concat`
/// expression is treated as a one-element list.
fn to_coeff_vec(e: &Expr, ctx: &mut LowerCtx) -> Vec<Expr> {
    if let Expr::Concat { parts, .. } = e {
        parts.iter().map(|item| resolve_expr(item, ctx)).collect()
    } else {
        vec![resolve_expr(e, ctx)]
    }
}

/// A constant-real coefficient list — `table`'s `xs`/`ys` and any other
/// operator that needs its list literally evaluated at lowering time,
/// rather than carried through to the runtime marker.
fn const_coeff_vec(e: &Expr, ctx: &mut LowerCtx) -> Option<Vec<f64>> {
    let Expr::Concat { parts, .. } = e else { return None };
    parts
        .iter()
        .map(|item| match resolve_expr(item, ctx) {
            Expr::Number { value: NumberLit::Real(v), .. } => Some(v),
            Expr::Number { value: NumberLit::Integer(v), .. } => Some(v as f64),
            _ => None,
        })
        .collect()
}

struct Ddt;
impl AnalogOp for Ddt {
    /// §4.5.3: an omitted second argument carries no tolerance; otherwise
    /// it is either a bare `nature_identifier` (resolved through
    /// `ctx.nature_abstols` to that nature's declared `abstol`, a literal
    /// at lowering time) or a `constant_expression` (kept symbolic — may
    /// reference a module parameter, evaluated once per instance the same
    /// way `Idt`'s `ic` is).
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return real(0.0) };
        let x = resolve_expr(a0, ctx);
        let abstol = args.get(1).map(|a| match a {
            Expr::Ident { name, .. } if ctx.nature_abstols.contains_key(name) => {
                real(ctx.nature_abstols[name])
            }
            other => resolve_expr(other, ctx),
        });
        let id = ctx.alloc_state(StateKind::Ddt { abstol }, x.clone());
        marker("__ddt", id, vec![x])
    }
}

struct Idt;
impl AnalogOp for Idt {
    /// §4.5.4: `idt(expr[, ic[, assert[, abstol|nature]]])` — this task
    /// (LRMW2-29) carries `assert`; the trailing `abstol`/`nature` stays
    /// out of scope here (tracked separately).
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return real(0.0) };
        let x = resolve_expr(a0, ctx);
        let ic = arg(args, 1, ctx, 0.0);
        let assert = args.get(2).map(|a| resolve_expr(a, ctx));
        let id = ctx.alloc_state(StateKind::Idt { ic: ic.clone(), assert: assert.clone() }, x.clone());
        let mut margs = vec![x, ic];
        if let Some(a) = assert {
            margs.push(a);
        }
        marker("__idt", id, margs)
    }
}

struct IdtMod;
impl AnalogOp for IdtMod {
    /// §4.5.5: `idtmod(expr[, ic[, modulus[, offset[, abstol|nature]]]])`
    /// — this task (LRMW2-30) carries `offset`; the trailing `abstol`/
    /// `nature` stays out of scope here (tracked separately, same as
    /// `Idt`'s own).
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return real(0.0) };
        let x = resolve_expr(a0, ctx);
        let ic = arg(args, 1, ctx, 0.0);
        let modulus = arg(args, 2, ctx, 1.0);
        let offset = arg(args, 3, ctx, 0.0);
        let id = ctx.alloc_state(
            StateKind::IdtMod { ic: ic.clone(), modulus: modulus.clone(), offset: offset.clone() },
            x.clone(),
        );
        marker("__idtmod", id, vec![x, ic, modulus, offset])
    }
}

/// Extract a node name from a `ddx` second argument: real Verilog-A spells
/// it `ddx(expr, V(node))` — a single-terminal access-function probe, LRM
/// §4.5.2 — so this unwraps `AccessFunction`'s `pos` when it names a bare
/// node, falling back to a bare `Ident` for any caller still spelling it
/// that way.
fn ddx_node_name(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::AccessFunction { pos, neg: None, .. } => match pos.as_ref() {
            Expr::Ident { name, .. } => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

struct Ddx;
impl AnalogOp for Ddx {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 2 {
            return real(0.0);
        }
        let x = resolve_expr(&args[0], ctx);
        let node_name = ddx_node_name(&args[1]).unwrap_or_else(|| "?".into());
        let node = ctx.require_node(&node_name);
        let id = ctx.alloc_state(StateKind::Ddx { node }, x.clone());
        marker("__ddx", id, vec![x, int(node.0 as i64)])
    }
}

struct Delay;
impl AnalogOp for Delay {
    /// §4.5.7: `absdelay(input, td[, maxdelay])`. "If maxdelay is
    /// specified... If td becomes greater than maxdelay, maxdelay will be
    /// used as a substitute for td" — a clamp, not an error (the LRM text
    /// is explicit; a `td > maxdelay` build-time failure would contradict
    /// it, so this deviates from that literal reading of the task and
    /// implements the LRM's own words instead, per C7 provenance).
    /// Lowered as `td > maxdelay ? maxdelay : td` — const-evaluable the
    /// same way `Idt`'s `ic` is, so a param-dependent `td`/`maxdelay` still
    /// resolves once per instance.
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return real(0.0) };
        let x = resolve_expr(a0, ctx);
        let td = arg(args, 1, ctx, 0.0);
        let delay = match args.get(2) {
            Some(md) => {
                let maxdelay = resolve_expr(md, ctx);
                Expr::Ternary {
                    span: NOSPAN,
                    cond: Box::new(Expr::Binary {
                        span: NOSPAN,
                        op: VBinOp::Gt,
                        lhs: Box::new(td.clone()),
                        rhs: Box::new(maxdelay.clone()),
                    }),
                    then_branch: Box::new(maxdelay),
                    else_branch: Box::new(td),
                }
            }
            None => td,
        };
        let id = ctx.alloc_state(StateKind::Delay { delay: delay.clone() }, x.clone());
        marker("__delay", id, vec![x, delay])
    }
}

struct Transition;
impl AnalogOp for Transition {
    /// §10.3/LRMC-C10-02: an omitted `rise`/`fall` argument uses the
    /// in-force `` `default_transition `` value instead of `0.0`, when one
    /// is recorded (`ctx.default_transition`, T56) — an *explicit*
    /// argument still wins, since `arg` only falls back when the caller's
    /// `args` list doesn't reach that position at all.
    ///
    /// `tol` (§4.5.8's `time_tol`, LRMW2-36) was already captured here into
    /// `StateKind::Transition` — the gap this task closes is downstream,
    /// in `kernel/analog/compile.rs`/`kernel.rs` dropping it when building
    /// `RuntimeState::Transition` ("intentionally not carried" until now).
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return real(0.0) };
        let x = resolve_expr(a0, ctx);
        let (default_rise, default_fall) = ctx.default_transition.unwrap_or((0.0, 0.0));
        let delay = arg(args, 1, ctx, 0.0);
        let rise = arg(args, 2, ctx, default_rise);
        let fall = arg(args, 3, ctx, default_fall);
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
    /// §4.5.9: `slew(expr[, max_pos_slew_rate[, max_neg_slew_rate]])`. "If
    /// the max_neg_slew_rate is not specified, it defaults to the opposite
    /// of the max_pos_slew_rate" — an omitted (but not merely absent
    /// entirely: only when `rise` itself was given) `max_neg_slew_rate`
    /// reuses `rise`'s expression, not a bare `0.0` (which limited the
    /// falling edge to a dead stop instead of mirroring the rising limit —
    /// `device/analog/operators.rs`/`piperine_ir::ops::Operator::slew`
    /// both store the fall rate as a magnitude via `.abs()`, so reusing
    /// `rise` verbatim already yields the LRM's `-max_pos_slew_rate`
    /// magnitude without needing an explicit negation here).
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        let Some(a0) = args.first() else { return real(0.0) };
        let x = resolve_expr(a0, ctx);
        let rise = arg(args, 1, ctx, 0.0);
        let fall = args.get(2).map(|a| resolve_expr(a, ctx)).unwrap_or_else(|| rise.clone());
        let id = ctx.alloc_state(StateKind::Slew { rise: rise.clone(), fall: fall.clone() }, x.clone());
        marker("__slew", id, vec![x, rise, fall])
    }
}

/// `table(x, xs, ys[, mode])` — 1-D measured-data lookup (spec Part V §2).
/// `xs`/`ys` must be constant real concatenation lists (`{...}`) with `xs`
/// strictly increasing; interpolation is linear with end clamp. Lowered to
/// a **pure piecewise expression** (nested ternaries) — no state, so the
/// Jacobian falls out of the normal symbolic-diff path as the segment
/// slope.
struct Table;
impl Table {
    fn err(ctx: &mut LowerCtx, what: &'static str) -> Expr {
        ctx.errors.push(super::LowerError {
            module: ctx.module_name.clone(),
            what,
            name: "table".to_string(),
        });
        real(0.0)
    }

    /// Map a `signal::Table` validation failure onto the existing loud-error
    /// path, keeping today's user-facing text unchanged (AC 17) — the value
    /// carried by `TableError::UnsupportedMode` is not echoed today either.
    fn err_from(e: piperine_ir::behavior::table::TableError, ctx: &mut LowerCtx) -> Expr {
        use piperine_ir::behavior::table::TableError;
        let what: &'static str = match e {
            TableError::TooFewPoints => "table needs at least 2 points",
            TableError::LengthMismatch => "table xs and ys must have the same length",
            TableError::NotStrictlyIncreasing => {
                "table breakpoints (xs) must be strictly increasing"
            }
            TableError::UnsupportedMode(_) => {
                "table interpolation mode: only \"linear\" is supported"
            }
        };
        Self::err(ctx, what)
    }
}
impl AnalogOp for Table {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 3 {
            return Self::err(ctx, "table(x, xs, ys) needs at least 3 arguments");
        }
        let Some(xs) = const_coeff_vec(&args[1], ctx) else {
            return Self::err(ctx, "table breakpoints (xs) must be a constant real {..} list");
        };
        let Some(ys) = const_coeff_vec(&args[2], ctx) else {
            return Self::err(ctx, "table data (ys) must be a constant real {..} list");
        };
        // Any mode argument that is not the "linear" string literal fails
        // through `Table::with_mode` below with the same message as today,
        // regardless of what the argument actually was.
        let mode = match args.get(3) {
            None => "linear".to_string(),
            Some(Expr::Str { value, .. }) => value.clone(),
            Some(_) => "?".to_string(),
        };
        let table = match piperine_ir::behavior::table::Table::with_mode(xs, ys, &mode) {
            Ok(t) => t,
            Err(e) => return Self::err_from(e, ctx),
        };
        let x = resolve_expr(&args[0], ctx);

        let (xs, ys) = table.points();
        // Segment i: ys[i] + (x − xs[i]) · slope_i.
        let seg = |i: usize, x: &Expr| {
            let slope = table.segment_slope(i);
            Expr::Binary {
                span: NOSPAN,
                op: VBinOp::Add,
                lhs: Box::new(real(ys[i])),
                rhs: Box::new(Expr::Binary {
                    span: NOSPAN,
                    op: VBinOp::Mul,
                    lhs: Box::new(Expr::Binary {
                        span: NOSPAN,
                        op: VBinOp::Sub,
                        lhs: Box::new(x.clone()),
                        rhs: Box::new(real(xs[i])),
                    }),
                    rhs: Box::new(real(slope)),
                }),
            }
        };
        // Fold back-to-front: else-most = last segment (extrapolation is a
        // flat clamp on both ends).
        let n = xs.len();
        let mut expr = real(ys[n - 1]); // x ≥ xs[n−1] → clamp high
        for i in (0..n - 1).rev() {
            // x < xs[i+1] → segment i (for i = 0 the low clamp is applied
            // below), else previous accumulation.
            expr = Expr::Ternary {
                span: NOSPAN,
                cond: Box::new(Expr::Binary {
                    span: NOSPAN,
                    op: VBinOp::Lt,
                    lhs: Box::new(x.clone()),
                    rhs: Box::new(real(xs[i + 1])),
                }),
                then_branch: Box::new(seg(i, &x)),
                else_branch: Box::new(expr),
            };
        }
        // Low clamp: x ≤ xs[0] → ys[0].
        Expr::Ternary {
            span: NOSPAN,
            cond: Box::new(Expr::Binary {
                span: NOSPAN,
                op: VBinOp::Lt,
                lhs: Box::new(x.clone()),
                rhs: Box::new(real(xs[0])),
            }),
            then_branch: Box::new(real(ys[0])),
            else_branch: Box::new(expr),
        }
    }
}

struct Laplace {
    variant: &'static str,
}
impl AnalogOp for Laplace {
    /// §4.5.11/4.5.14: `laplace_*(expr, num_or_zeros, den_or_poles[, ε])`
    /// — the trailing `abstol_expression` (LRMW2-35) is a `constant_
    /// expression`, resolved once per instance the same way `Ddt`'s own
    /// `abstol` is (`ctx` carries no nature-lookup case here — §4.5.14's
    /// table spells it `abstol`, never `nature_identifier`, unlike `ddt`).
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 3 {
            return real(0.0);
        }
        let x = resolve_expr(&args[0], ctx);
        let num = to_coeff_vec(&args[1], ctx);
        let den = to_coeff_vec(&args[2], ctx);
        let abstol = args.get(3).map(|a| resolve_expr(a, ctx));
        let variant = match self.variant {
            "zp" => LaplaceKind::ZerosPoles,
            "np" => LaplaceKind::NumPoles,
            "zd" => LaplaceKind::ZerosDen,
            _ => LaplaceKind::NumDen,
        };
        let id = ctx.alloc_state(
            StateKind::Laplace { variant, num: num.clone(), den: den.clone(), abstol: abstol.clone() },
            x.clone(),
        );
        let marker_args = vec![
            x,
            Expr::Concat { span: NOSPAN, parts: num },
            Expr::Concat { span: NOSPAN, parts: den },
        ];
        marker("__laplace", id, marker_args)
    }
}

struct ZTransform {
    variant: &'static str,
}
impl ZTransform {
    fn err_t0(ctx: &mut LowerCtx) -> Expr {
        ctx.errors.push(super::LowerError {
            module: ctx.module_name.clone(),
            what: "zi_* t0 (§4.5.12 — a first-transition start time; this codegen has no \
                   zi_* sample-and-hold state to shift, only the ddt-shaped charge/resistive \
                   split laplace_*/zi_* both currently lower through) is not implemented",
            name: "zi_t0".to_string(),
        });
        real(0.0)
    }
}
impl AnalogOp for ZTransform {
    /// §4.5.12 (LRMW2-32): `zi_*(expr, num_or_zeros, den_or_poles, T[, τ[,
    /// t0]])`. `τ` — "the timestep is controlled to accurately resolve
    /// both the leading and trailing corner of the transition" — is a
    /// step-control lever, folded into `StateKind::ZTransform.tau` and
    /// (`kernel/analog/compile.rs`) the same per-instance LTE tolerance
    /// mechanism `Ddt`'s/`Laplace`'s own tolerance already feeds. `t0`
    /// ("the time of the first transition") has no honest equivalent —
    /// there is no sample-and-hold state to shift — so an explicit `t0`
    /// argument fails loud instead of being silently carried-and-ignored
    /// (`Self::err_t0`); an omitted `t0` (the LRM's own `t = 0` default)
    /// is unaffected.
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        if args.len() < 4 {
            return real(0.0);
        }
        if args.len() > 5 {
            return Self::err_t0(ctx);
        }
        let x = resolve_expr(&args[0], ctx);
        let num = to_coeff_vec(&args[1], ctx);
        let den = to_coeff_vec(&args[2], ctx);
        let sample_dt = resolve_expr(&args[3], ctx);
        let tau = args.get(4).map(|a| resolve_expr(a, ctx));
        let variant = match self.variant {
            "zp" => ZKind::ZerosPoles,
            "np" => ZKind::NumPoles,
            "zd" => ZKind::ZerosDen,
            _ => ZKind::NumDen,
        };
        let id = ctx.alloc_state(
            StateKind::ZTransform {
                variant,
                num: num.clone(),
                den: den.clone(),
                sample_dt: sample_dt.clone(),
                tau: tau.clone(),
            },
            x.clone(),
        );
        let mut marker_args = vec![
            x,
            Expr::Concat { span: NOSPAN, parts: num },
            Expr::Concat { span: NOSPAN, parts: den },
            sample_dt,
        ];
        if let Some(t) = tau {
            marker_args.push(t);
        }
        marker("__ztransform", id, marker_args)
    }
}

struct AcStim;
impl AnalogOp for AcStim {
    fn lower(&self, args: &[Expr], ctx: &mut LowerCtx) -> Expr {
        // `ac_stim([ "analysis_identifier" [, mag [, phase]]])` — LRM
        // §4.5.9/Annex A.8.7: the optional leading argument is a string
        // literal naming the analysis, never a magnitude value. Skipping
        // it here (rather than blindly treating `args[0]` as `mag`) is the
        // whole fix — codegen doesn't otherwise gate `ac_stim` by the
        // named analysis (it is already AC-only by construction).
        let rest: &[Expr] = if matches!(args.first(), Some(Expr::Str { .. })) { &args[1..] } else { args };
        let mag = arg(rest, 0, ctx, 1.0);
        let phase = arg(rest, 1, ctx, 0.0);
        Expr::SystemFunctionCall { span: NOSPAN, name: "$ac_stim".to_string(), args: vec![mag, phase] }
    }
}

struct NoiseCall;
impl AnalogOp for NoiseCall {
    fn lower(&self, _args: &[Expr], _ctx: &mut LowerCtx) -> Expr {
        real(0.0)
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
        // §4.5.11: laplace_zd/laplace_nd/laplace_zp/laplace_np are the four
        // real LRM spellings (z-domain/s-domain numerator/denominator
        // coefficient forms) -- LRMC-C1-12/C5-01. `pm`/`nm`/`npm` were a
        // registry typo with no LRM basis; removed so those spellings fail
        // loud as unknown operators instead of silently resolving.
        for variant in ["zd", "nd", "zp", "np"] {
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

/// LRMC-C1-12/C5-01 registry/keyword guard: the exact bug class that let
/// `laplace_pm`/`nm`/`npm` (no LRM basis) into the registry while the
/// real `laplace_zd`/`laplace_nd` spellings sat unregistered, mechanically
/// caught from now on. `AnalogOpRegistry` and its `ops` field are
/// `pub(crate)`, so this lives as an inline unit test (same crate) rather
/// than an integration test under `tests/`, which cannot see them.
///
/// **Guard proven able to fail** (T21): temporarily reverted the
/// `laplace_*` variant list to `["np", "zp", "pm", "nm", "npm"]`, ran
/// `every_laplace_and_zi_keyword_is_registered` and watched it fail on
/// the missing `laplace_zd`/`laplace_nd` keys, then reverted back to the
/// fix. Not left in the tree — the mutation itself isn't part of the
/// shipped gate, only this record of it (same convention T9's
/// `lrm_conformance_matrix.rs` guard used).
#[cfg(test)]
mod registry_keyword_guard {
    //! LRM-COVERS: 4.5.12.1, 4.5.12.2, 4.5.12.3
    //! Wave-2 (T69) re-reading, no new tests: `every_laplace_and_zi_
    //! keyword_is_registered` below proves `zi_zp`/`zi_zd`/`zi_np` (§4.5.12.1
    //! "zi_zp", §4.5.12.2 "zi_zd", §4.5.12.3 "zi_np") are each a reserved
    //! keyword AND a real `AnalogOpRegistry` entry — genuinely registered,
    //! not the `Unsupported`-fallback shape `noise_table`/`last_crossing`
    //! fall through to. All four `zi_*` variants share the identical
    //! `ZTransform { variant }` lowering path (`r.register(&[&format!("zi_
    //! {variant}")], ZTransform { variant })` above); `zi_nd`'s own
    //! numerical behavior (the fourth variant) is proven end to end by
    //! `crates/piperine-codegen/tests/zi_tau_arg.rs` and root `tests/
    //! zi_tau_lte.rs` — this guard is the registration-level proof for the
    //! other three variants of that same shared codegen path.
    use super::*;
    use piperine_lang::Keywords;

    #[test]
    fn every_registry_key_is_a_reserved_keyword() {
        // `"delay"` is a pre-existing internal alias for the `absdelay`
        // operator (registered alongside it, `register(&["delay",
        // "absdelay"], Delay)`), not itself an LRM name -- a real gap
        // (a user's own `delay` function is silently shadowed), but
        // unrelated to LRMC-C1-12/C5-01's laplace_* registry-typo class
        // and out of this task's scope to fix. Disclosed, not silently
        // excluded.
        const KNOWN_NON_KEYWORD_ALIASES: &[&str] = &["delay"];
        let reg = AnalogOpRegistry::with_builtins();
        for key in reg.ops.keys() {
            if KNOWN_NON_KEYWORD_ALIASES.contains(&key.as_str()) {
                continue;
            }
            assert!(
                Keywords::is_reserved(key),
                "operator registry key `{key}` is not a reserved keyword in keywords.rs \
                 (every registered operator name must be reserved, or it could collide \
                 with a user identifier)"
            );
        }
    }

    #[test]
    fn every_laplace_and_zi_keyword_is_registered() {
        let reg = AnalogOpRegistry::with_builtins();
        for variant in ["zd", "nd", "zp", "np"] {
            let laplace_name = format!("laplace_{variant}");
            let zi_name = format!("zi_{variant}");
            assert!(Keywords::is_reserved(&laplace_name), "`{laplace_name}` must be a reserved keyword");
            assert!(reg.ops.contains_key(&laplace_name), "`{laplace_name}` must be registered");
            assert!(Keywords::is_reserved(&zi_name), "`{zi_name}` must be a reserved keyword");
            assert!(reg.ops.contains_key(&zi_name), "`{zi_name}` must be registered");
        }
    }

    #[test]
    fn fabricated_laplace_variants_are_gone() {
        for variant in ["pm", "nm", "npm"] {
            let name = format!("laplace_{variant}");
            assert!(!Keywords::is_reserved(&name), "`{name}` has no LRM basis and must not be a keyword");
            let reg = AnalogOpRegistry::with_builtins();
            assert!(!reg.ops.contains_key(&name), "`{name}` must not be registered");
        }
    }
}
