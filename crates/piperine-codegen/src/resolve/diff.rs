//! Symbolic differentiation of analog expressions (POM `Expr`,
//! `piperine_lang::ast::Expr`).
//!
//! The Jacobian is built by differentiating each flattened contribution
//! w.r.t. every branch voltage it reads, then emitting the derivative like
//! any other expression. `__state_load` / `__ddt` reads are constants within
//! a Newton iteration (reactive parts are handled by the charge Jacobian).

use piperine_lang::ast::{BinOp, Expr, UnOp};

use super::symbols::NodeId;

/// The `(__temp)/(__dtemp)`-tape marker used by every existing (single-order)
/// call site — `__temp(id)` leaves differentiate to `__dtemp(id)`.
const TAPE_D1: &[(&str, &str)] = &[("__temp", "__dtemp")];

/// `∂expr / ∂V(plus, minus)` as a new POM expression. The `resolver` maps
/// node names to `NodeId`s so `V(p,n)` branch accesses can be identified.
pub fn d_dv(
    expr: &Expr,
    plus: NodeId,
    minus: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    d_dv_named(expr, plus, minus, resolve_node, "__dtemp")
}

/// `∂expr / ∂V(plus, minus)` with a custom tape-marker name: `__temp(id)`
/// leaves differentiate to `__deriv_marker(id)`, so the emitted tape's own
/// entries reference the *same* marker (self-consistent tape). The `.disto`
/// kernels keep one first-derivative tape per branch, each under its own
/// name; a tape built with the wrong name would read another branch's
/// values through multi-level temp chains.
pub fn d_dv_named(
    expr: &Expr,
    plus: NodeId,
    minus: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    deriv_marker: &'static str,
) -> Expr {
    differentiate(expr, &|p, m| {
        if p == plus && m == minus {
            lit(1.0)
        } else {
            lit(0.0)
        }
    }, &|_| lit(0.0), resolve_node, &[("__temp", deriv_marker)])
}

/// `∂expr / ∂V(node)` — the `ddx` derivative w.r.t. a single node potential.
pub fn d_dnode(
    expr: &Expr,
    node: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    differentiate(expr, &|p, m| {
        let sign = f64::from(p == node) - f64::from(m == node);
        lit(sign)
    }, &|_| lit(0.0), resolve_node, TAPE_D1)
}

/// `∂expr / ∂param` — the parameter derivative (DVG-01). Same chain-rule
/// walk as [`d_dv`] / [`d_dnode`], but the differentiation variable is a
/// *parameter* (a bare `Expr::Ident`), not a branch voltage: an ident leaf
/// matching `param` seeds `1.0`, every other ident (and every `V`/`I` branch
/// read) seeds `0.0`.
///
/// Temp leaves map to `__dtemp` exactly as in [`d_dv`] — the caller (the
/// `∂F/∂p` kernel compiler) installs the parameter-derivative tape
/// (`d_dp(temps[id], param)` per slot) as the `__dtemp` tape before emitting
/// the result, so multi-level temp chains resolve self-consistently through
/// one shared mechanism rather than a second differentiator.
///
/// `resolve_node` is not a parameter here: parameter differentiation never
/// touches a branch read (their seed is always zero), so node resolution is
/// irrelevant and a constant `None` closure is used internally.
pub fn d_dp(expr: &Expr, param: &str) -> Expr {
    differentiate(
        expr,
        &|_p, _m| lit(0.0),
        &|name| if name == param { lit(1.0) } else { lit(0.0) },
        &|_| None,
        TAPE_D1,
    )
}

/// `∂²expr / ∂V(a,b)∂V(c,d)` — the `.disto` 2nd-derivative kernel's core
/// (DISTO-03), composed from two single-variable passes through a private
/// intermediate tape marker (`__dtemp_inner`) rather than literally calling
/// [`d_dv`] twice:
///
/// 1. First pass (w.r.t. `(a,b)`): `__temp(id)` leaves become
///    `__dtemp_inner(id)` — kept distinct from the caller-visible `__dtemp`
///    marker so it cannot collide with the second pass's own use of that name.
/// 2. Second pass (w.r.t. `(c,d)`) resolves *two* leaf kinds at once:
///    - a `__temp(id)` leaf surviving from the original expression (product/
///      quotient/power rules keep an untouched copy of each operand)
///      differentiates to `__dtemp(id)` — the ordinary first-derivative tape
///      for branch `(c,d)`, exactly what [`d_dv`] would build for that
///      branch. The caller installs this tape (via
///      `Builder::set_deriv_tape`)
///      before evaluating the result.
///    - a `__dtemp_inner(id)` leaf (the first pass's own tape reference)
///      differentiates to `__ddtemp(id)` — the genuine second-order cross
///      term, i.e. `d²(temps[id])/dV(a,b)dV(c,d)`. The caller builds this
///      tape by calling `d_dv_twice` on each `temps[id]` itself and installs
///      it via `Builder::set_ddtemp_tape`.
///
/// A `__dtemp_inner(id)` leaf can also *survive* the second pass unchanged —
/// the product/quotient/power rules clone their undifferentiated operands,
/// and a cloned `__dtemp_inner` still means `d(temps[id])/dV(a,b)`, a *first*
/// derivative w.r.t. the pair's `(a,b)` branch. The caller therefore installs
/// a third tape — the `(a,b)`-branch first-derivative tape, via
/// `Builder::set_deriv_tape2` — so `emit_dtemp2` can evaluate those leaves.
///
/// Every caller (the `disto2`/`disto3` kernel compilers) must build and
/// install all three tapes — the `(c,d)`-branch `__dtemp` tape, the
/// `(a,b)`-branch `__dtemp_inner` tape, and the cross `__ddtemp` tape —
/// before emitting the result, mirroring how
/// `AnalogCompiler::compile_jacobian` installs its
/// single derivative tape before emitting a first-order Jacobian row.
pub fn d_dv_twice(
    expr: &Expr,
    a: NodeId,
    b: NodeId,
    c: NodeId,
    d: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    d_dv_twice_named(expr, a, b, c, d, resolve_node, ("__dtemp_inner", "__dtemp", "__ddtemp"))
}

/// [`d_dv_twice`] with custom tape-marker names, so every installed tape's
/// entries reference only markers the caller installs (see [`d_dv_named`]):
/// pass 1 (w.r.t. `(a,b)`) maps `__temp` → `d1`; pass 2 (w.r.t. `(c,d)`)
/// maps `__temp` → `d2` and `d1` → `d12`.
pub fn d_dv_twice_named(
    expr: &Expr,
    a: NodeId,
    b: NodeId,
    c: NodeId,
    d: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    markers: (&'static str, &'static str, &'static str),
) -> Expr {
    let (d1, d2, d12) = markers;
    let seed1 = |p: NodeId, m: NodeId| {
        if p == a && m == b { lit(1.0) } else { lit(0.0) }
    };
    let inner = differentiate(expr, &seed1, &|_| lit(0.0), resolve_node, &[("__temp", d1)]);
    let seed2 = |p: NodeId, m: NodeId| {
        if p == c && m == d { lit(1.0) } else { lit(0.0) }
    };
    differentiate(&inner, &seed2, &|_| lit(0.0), resolve_node, &[("__temp", d2), (d1, d12)])
}

/// Completes [`d_dv_twice_named`]'s second pass on an *already-computed*
/// first pass (`inner` — some `temps[id]` differentiated once w.r.t.
/// `(a,b)` under marker `d1`), differentiating it once more w.r.t.
/// `(c,d)`. Equivalent to `d_dv_twice_named(expr, a, b, c, d, ..., d1, d2,
/// d12)` when `inner` is that same first pass, but without redoing it —
/// callers building one first-derivative tape per branch (once) and one
/// cross tape per branch *pair* (once) must not re-run the first pass for
/// every pair (DISTO-03 perf).
pub fn d_dv_once_more_named(
    inner: &Expr,
    c: NodeId,
    d: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    d1: &'static str,
    d2: &'static str,
    d12: &'static str,
) -> Expr {
    let seed2 = |p: NodeId, m: NodeId| {
        if p == c && m == d { lit(1.0) } else { lit(0.0) }
    };
    differentiate(inner, &seed2, &|_| lit(0.0), resolve_node, &[("__temp", d2), (d1, d12)])
}

/// `∂³expr / ∂V(a,b)∂V(c,d)∂V(e,f)` — the `.disto` 3rd-derivative kernel's
/// core (DISTO-03), three single-variable passes through private markers:
///
/// - pass 1 (w.r.t. `(a,b)`): `__temp` → `__dtemp1`
/// - pass 2 (w.r.t. `(c,d)`): `__temp` → `__dtemp2`, `__dtemp1` → `__ddtemp12`
/// - pass 3 (w.r.t. `(e,f)`): `__temp` → `__dtemp3`, `__dtemp1` → `__ddtemp13`,
///   `__dtemp2` → `__ddtemp23`, `__ddtemp12` → `__dddtemp123`
///
/// As with [`d_dv_twice`], product/quotient/power rules clone their
/// undifferentiated operands, so markers from every earlier level can
/// survive into the result. Before emitting, the caller must install seven
/// tapes (via `Builder::set_tape`):
/// `__dtemp1`/`__dtemp2`/`__dtemp3` — the first-derivative tapes of the
/// three branches; `__ddtemp12`/`__ddtemp13`/`__ddtemp23` — the three
/// pairwise cross tapes (each built with [`d_dv_twice`]); and
/// `__dddtemp123` — this function applied to each `temps[id]`.
pub fn d_dv_thrice(
    expr: &Expr,
    branch1: (NodeId, NodeId),
    branch2: (NodeId, NodeId),
    branch3: (NodeId, NodeId),
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    let (a, b) = branch1;
    let (c, d) = branch2;
    let (e, f) = branch3;
    let pass1 = differentiate(expr, &|p, m| {
        if p == a && m == b { lit(1.0) } else { lit(0.0) }
    }, &|_| lit(0.0), resolve_node, &[("__temp", "__dtemp1")]);
    let pass2 = differentiate(&pass1, &|p, m| {
        if p == c && m == d { lit(1.0) } else { lit(0.0) }
    }, &|_| lit(0.0), resolve_node, &[("__temp", "__dtemp2"), ("__dtemp1", "__ddtemp12")]);
    differentiate(&pass2, &|p, m| {
        if p == e && m == f { lit(1.0) } else { lit(0.0) }
    }, &|_| lit(0.0), resolve_node, &[
        ("__temp", "__dtemp3"),
        ("__dtemp1", "__ddtemp13"),
        ("__dtemp2", "__ddtemp23"),
        ("__ddtemp12", "__dddtemp123"),
    ])
}

/// Completes [`d_dv_thrice`]'s third pass on an *already-computed* second
/// pass (`pass2` — some `temps[id]` differentiated w.r.t. `(a,b)` then
/// `(c,d)`, via [`d_dv_twice_named`] with markers `__dtemp1`/`__dtemp2`/
/// `__ddtemp12`), differentiating it once more w.r.t. `(e,f)`. Equivalent
/// to `d_dv_thrice(expr, a, b, c, d, e, f, ...)` when `pass2` is that same
/// second pass, but without redoing the first two — the disto3 kernel
/// builds one first-derivative tape per branch and one cross tape per
/// branch *pair* (both shared across every triple); redoing them per
/// triple made compilation cubic-times-redundant on many-branch devices
/// (DISTO-03 perf).
pub fn d_dv_thrice_from_twice(
    pass2: &Expr,
    e: NodeId,
    f: NodeId,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
) -> Expr {
    differentiate(pass2, &|p, m| {
        if p == e && m == f { lit(1.0) } else { lit(0.0) }
    }, &|_| lit(0.0), resolve_node, &[
        ("__temp", "__dtemp3"),
        ("__dtemp1", "__ddtemp13"),
        ("__dtemp2", "__ddtemp23"),
        ("__ddtemp12", "__dddtemp123"),
    ])
}

/// Core chain-rule walk; `seed` gives the derivative of a branch read and
/// `ident_seed` gives the derivative of an identifier leaf (a parameter, when
/// differentiating by parameter — `lit(0.0)` for the unknown-differentiation
/// callers). `tapes` is an ordered list of `(leaf_marker, derivative_marker)`
/// pairs — a `__leaf_marker(id)` call becomes `Call(derivative_marker, [id])`
/// rather than being treated as an opaque zero-derivative constant. Every
/// existing (single-order) caller passes [`TAPE_D1`]; [`d_dv_twice`]
/// additionally activates a second pair to resolve leaves surviving from its
/// first pass.
fn differentiate(
    expr: &Expr,
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    ident_seed: &impl Fn(&str) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    tapes: &[(&str, &str)],
) -> Expr {
    match expr {
        Expr::Number { .. } | Expr::Str { .. } => lit(0.0),

        // `$limit` is transparent to differentiation: d(vnew)/dV.
        Expr::SystemFunctionCall { name, args, .. } if name.trim_start_matches('$') == "limit" => {
            args.get(1)
                .map(|vnew| differentiate(vnew, seed, ident_seed, resolve_node, tapes))
                .unwrap_or_else(|| lit(0.0))
        }
        Expr::SystemFunctionCall { .. } => lit(0.0),

        // Branch access: V(p,n) / I(br) — resolve node names and seed.
        Expr::AccessFunction { pos, neg, .. } => {
            let plus_name = ident_str(pos).unwrap_or_else(|| "0".into());
            let minus_name = neg.as_deref().and_then(ident_str).unwrap_or_else(|| "0".into());
            let p = resolve_node(&plus_name).unwrap_or(NodeId::GROUND);
            let m = resolve_node(&minus_name).unwrap_or(NodeId::GROUND);
            seed(p, m)
        }

        Expr::Call { name, args, .. } => {
            // A value-tape leaf: its derivative is the matching entry of
            // the derivative tape (`d(temps[id])/dV`), referenced as the
            // pair's derivative-marker leaf and emitted once per branch.
            let mut tape_hit = None;
            for (leaf_marker, deriv_marker) in tapes {
                if name == leaf_marker {
                    tape_hit = Some(*deriv_marker);
                    break;
                }
            }
            if let Some(deriv_marker) = tape_hit {
                if let Some(Expr::Number { value: piperine_lang::lexer::NumberLit::Integer(id), span }) =
                    args.first()
                {
                    return Expr::Call {
                        span: *span,
                        name: deriv_marker.into(),
                        args: vec![Expr::Number {
                            span: *span,
                            value: piperine_lang::lexer::NumberLit::Integer(*id),
                        }],
                    };
                }
                return lit(0.0);
            }
            // __state_load, __ddt, __idt, __delay, __slew, etc. — constants
            // within a Newton iteration.
            if name.starts_with("__") {
                return lit(0.0);
            }
            // Math builtins: chain rule.
            if super::math::math_fn(name).is_some() {
                return d_math(name, args, seed, ident_seed, resolve_node, tapes);
            }
            lit(0.0)
        }

        Expr::Ident { name, .. } => ident_seed(name),

        Expr::Unary { op: UnOp::Neg, operand, .. } => {
            neg(differentiate(operand, seed, ident_seed, resolve_node, tapes))
        }
        Expr::Unary { op: UnOp::Plus, operand, .. } => {
            differentiate(operand, seed, ident_seed, resolve_node, tapes)
        }
        Expr::Unary { .. } => lit(0.0),

        Expr::Binary { op, lhs, rhs, .. } => {
            d_binary(*op, lhs, rhs, seed, ident_seed, resolve_node, tapes)
        }

        Expr::Ternary { cond, then_branch, else_branch, .. } => Expr::Ternary {
            span: expr.span(),
            cond: cond.clone(),
            then_branch: Box::new(differentiate(then_branch, seed, ident_seed, resolve_node, tapes)),
            else_branch: Box::new(differentiate(else_branch, seed, ident_seed, resolve_node, tapes)),
        },

        // Concat/replication/index/part-select/assignment-pattern have no
        // continuous derivative in this analog Real-valued form (they're
        // the bit/vector-selection and literal-list forms) —
        // zero-derivative, not reached by any flattened analog
        // contribution today (flatten only ever feeds this differentiator
        // resistive/charge Real-valued expressions).
        // PortAccess never reaches codegen: elab::lower::vams rewrites it
        // into a sum of AccessFunction reads before a Module leaves the
        // elaborator (LRMC-C1-10) — zero-derivative here only satisfies
        // exhaustiveness, matching the other structurally-unreached forms.
        Expr::Concat { .. }
        | Expr::Replication { .. }
        | Expr::Index { .. }
        | Expr::PartSelect { .. }
        | Expr::AssignmentPattern { .. }
        | Expr::PortAccess { .. } => lit(0.0),
    }
}

fn d_binary(
    op: BinOp,
    a: &Expr,
    b: &Expr,
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    ident_seed: &impl Fn(&str) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    tapes: &[(&str, &str)],
) -> Expr {
    let da = differentiate(a, seed, ident_seed, resolve_node, tapes);
    let db = differentiate(b, seed, ident_seed, resolve_node, tapes);
    match op {
        BinOp::Add => add(da, db),
        BinOp::Sub => sub(da, db),
        BinOp::Mul => add(mul(da, b.clone()), mul(a.clone(), db)),
        BinOp::Div => div(
            sub(mul(da, b.clone()), mul(a.clone(), db)),
            mul(b.clone(), b.clone()),
        ),
        BinOp::LogAnd | BinOp::LogOr => lit(0.0),
        BinOp::Pow => {
            if is_zero(&db) {
                mul(
                    mul(b.clone(), binary(super::BinOp::Pow, a.clone(), sub(b.clone(), lit(1.0)))),
                    da,
                )
            } else {
                mul(
                    binary(super::BinOp::Pow, a.clone(), b.clone()),
                    add(mul(db, math1("ln", a.clone())), div(mul(b.clone(), da), a.clone())),
                )
            }
        }
        // Comparisons, case (in)equality, and bitwise ops have no
        // continuous derivative — zero.
        _ => lit(0.0),
    }
}

fn d_math(
    name: &str,
    args: &[Expr],
    seed: &impl Fn(NodeId, NodeId) -> Expr,
    ident_seed: &impl Fn(&str) -> Expr,
    resolve_node: &impl Fn(&str) -> Option<NodeId>,
    tapes: &[(&str, &str)],
) -> Expr {
    let u = args.first().cloned().unwrap_or_else(|| lit(0.0));
    let du = args.first().map(|a| differentiate(a, seed, ident_seed, resolve_node, tapes)).unwrap_or_else(|| lit(0.0));
    match name {
        "exp" | "limexp" => mul(math1(name, u), du),
        "ln" => div(du, u),
        // LRM Table 4-14: `log` is decimal (base-10), an alias of `log10` —
        // d/dx log10(x) = 1 / (x·ln 10).
        "log10" | "log" => div(du, mul(u, lit(std::f64::consts::LN_10))),
        "sqrt" => div(du, mul(lit(2.0), math1("sqrt", u))),
        "sin" => mul(math1("cos", u), du),
        "cos" => mul(neg(math1("sin", u)), du),
        "tan" => div(du, mul(math1("cos", u.clone()), math1("cos", u))),
        "asin" => div(du, math1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        "acos" => div(neg(du), math1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        "atan" => div(du, add(lit(1.0), mul(u.clone(), u))),
        "sinh" => mul(math1("cosh", u), du),
        "cosh" => mul(math1("sinh", u), du),
        "tanh" => mul(sub(lit(1.0), mul(math1("tanh", u.clone()), math1("tanh", u))), du),
        "abs" => mul(
            ternary(binary(super::BinOp::Ge, u.clone(), lit(0.0)), lit(1.0), lit(-1.0)),
            du,
        ),
        "pow" => {
            let v = args.get(1).cloned().unwrap_or_else(|| lit(1.0));
            let dv = args.get(1).map(|a| differentiate(a, seed, ident_seed, resolve_node, tapes)).unwrap_or_else(|| lit(0.0));
            binary(super::BinOp::Pow, u, v).d_via_pow(du, dv)
        }
        "min" | "max" => {
            let v = args.get(1).cloned().unwrap_or_else(|| lit(0.0));
            let dv = args.get(1).map(|a| differentiate(a, seed, ident_seed, resolve_node, tapes)).unwrap_or_else(|| lit(0.0));
            let pick_first = if name == "min" {
                binary(super::BinOp::Le, u, v)
            } else {
                binary(super::BinOp::Ge, u, v)
            };
            ternary(pick_first, du, dv)
        }
        _ => lit(0.0),
    }
}

/// Collect every distinct `V(p,n)`/`I(p,n)` branch pair in the tree.
pub fn collect_branches(expr: &Expr, out: &mut Vec<(NodeId, NodeId)>, resolve_node: &impl Fn(&str) -> Option<NodeId>) {
    walk(expr, &mut |e| {
        if let Expr::AccessFunction { pos, neg, .. } = e {
            let plus_name = ident_str(pos).unwrap_or_else(|| "0".into());
            let minus_name = neg.as_deref().and_then(ident_str).unwrap_or_else(|| "0".into());
            let p = resolve_node(&plus_name).unwrap_or(NodeId::GROUND);
            let m = resolve_node(&minus_name).unwrap_or(NodeId::GROUND);
            let pair = (p, m);
            if !out.contains(&pair) {
                out.push(pair);
            }
            return false;
        }
        true
    });
}

/// Walk an expression and call `f` on each node (pre-order).
pub fn visit_expr<F: FnMut(&Expr)>(expr: &Expr, f: &mut F) {
    walk(expr, &mut |e| {
        f(e);
        true
    });
}

/// Pre-order tree walk over a VAMS `Expr`. `f` returns `false` to skip a
/// node's children (used by [`collect_branches`] to stop at an
/// `AccessFunction` node rather than descending into `pos`/`neg`, which are
/// name expressions, not sub-expressions to differentiate). No shared
/// `Walk` trait exists post-MD-45 (the PHDL-only one was deleted with
/// `parse/`) — this is `diff.rs`'s own, self-contained per T29's "no other
/// file changed" isolation.
fn walk(expr: &Expr, f: &mut impl FnMut(&Expr) -> bool) {
    if !f(expr) {
        return;
    }
    match expr {
        // PortAccess never reaches codegen — see this file's other `walk`
        // arm above.
        Expr::Number { .. } | Expr::Str { .. } | Expr::Ident { .. } | Expr::PortAccess { .. } => {}
        Expr::Unary { operand, .. } => walk(operand, f),
        Expr::Binary { lhs, rhs, .. } => {
            walk(lhs, f);
            walk(rhs, f);
        }
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            walk(cond, f);
            walk(then_branch, f);
            walk(else_branch, f);
        }
        Expr::Concat { parts, .. } | Expr::Replication { parts, .. } => {
            for p in parts {
                walk(p, f);
            }
        }
        Expr::AssignmentPattern { replicate, parts, .. } => {
            if let Some(r) = replicate {
                walk(r, f);
            }
            for p in parts {
                walk(p, f);
            }
        }
        Expr::Index { base, index, .. } => {
            walk(base, f);
            walk(index, f);
        }
        Expr::PartSelect { base, msb, lsb, .. } => {
            walk(base, f);
            walk(msb, f);
            walk(lsb, f);
        }
        Expr::Call { args, .. } | Expr::SystemFunctionCall { args, .. } => {
            for a in args {
                walk(a, f);
            }
        }
        Expr::AccessFunction { pos, neg, .. } => {
            walk(pos, f);
            if let Some(n) = neg {
                walk(n, f);
            }
        }
    }
}

fn ident_str(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident { name, .. } => Some(name.clone()),
        _ => None,
    }
}

// ── Constant-folding constructors ──

fn lit(v: f64) -> Expr {
    Expr::Number { span: piperine_lang::error::PreprocessedSpan::new(0, 0), value: piperine_lang::lexer::NumberLit::Real(v) }
}

fn is_zero(e: &Expr) -> bool {
    matches!(e, Expr::Number { value: piperine_lang::lexer::NumberLit::Real(v), .. } if *v == 0.0)
}

fn is_one(e: &Expr) -> bool {
    matches!(e, Expr::Number { value: piperine_lang::lexer::NumberLit::Real(v), .. } if *v == 1.0)
}

fn add(a: Expr, b: Expr) -> Expr {
    if is_zero(&a) { return b; }
    if is_zero(&b) { return a; }
    binary(super::BinOp::Add, a, b)
}

fn sub(a: Expr, b: Expr) -> Expr {
    if is_zero(&b) { return a; }
    binary(super::BinOp::Sub, a, b)
}

fn mul(a: Expr, b: Expr) -> Expr {
    if is_zero(&a) || is_zero(&b) { return lit(0.0); }
    if is_one(&a) { return b; }
    if is_one(&b) { return a; }
    binary(super::BinOp::Mul, a, b)
}

fn div(a: Expr, b: Expr) -> Expr {
    if is_zero(&a) { return lit(0.0); }
    if is_one(&b) { return a; }
    binary(super::BinOp::Div, a, b)
}

fn neg(a: Expr) -> Expr {
    if let Expr::Number { value: piperine_lang::lexer::NumberLit::Real(v), .. } = &a {
        return lit(-v);
    }
    Expr::Unary { span: a.span(), op: UnOp::Neg, operand: Box::new(a) }
}

fn binary(op: super::BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let pom_op = op.to_pom();
    let span = lhs.span();
    Expr::Binary { span, op: pom_op, lhs: Box::new(lhs), rhs: Box::new(rhs) }
}

fn ternary(cond: Expr, then_branch: Expr, else_branch: Expr) -> Expr {
    let span = cond.span();
    Expr::Ternary { span, cond: Box::new(cond), then_branch: Box::new(then_branch), else_branch: Box::new(else_branch) }
}

fn math1(name: &str, a: Expr) -> Expr {
    let span = a.span();
    Expr::Call { span, name: name.to_string(), args: vec![a] }
}

// Extension trait for Pow differentiation.
trait PowDeriv {
    fn d_via_pow(self, du: Expr, dv: Expr) -> Expr;
}

impl PowDeriv for Expr {
    fn d_via_pow(self, du: Expr, dv: Expr) -> Expr {
        if is_zero(&dv) {
            let (u, v) = unpack_pow(&self);
            mul(mul(v.clone(), binary(super::BinOp::Pow, u.clone(), sub(v, lit(1.0)))), du)
        } else {
            let (u, v) = unpack_pow(&self);
            mul(
                binary(super::BinOp::Pow, u.clone(), v.clone()),
                add(mul(dv, math1("ln", u.clone())), div(mul(v, du), u)),
            )
        }
    }
}

fn unpack_pow(e: &Expr) -> (Expr, Expr) {
    if let Expr::Binary { op: BinOp::Pow, lhs, rhs, .. } = e {
        (lhs.as_ref().clone(), rhs.as_ref().clone())
    } else {
        (e.clone(), lit(1.0))
    }
}
