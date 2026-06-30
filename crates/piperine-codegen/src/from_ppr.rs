//! Lower `ElabProgram` (PPR/PHDL) → `IrProgram`.

use std::collections::HashMap;

use piperine_lang::{
    elab::ir::{
        ElabBehaviorStmt, ElabFn, ElabMod, ElabProgram, ElabNetType,
        ElabValueType,
    },
    parse::ast::{
        ArrayBody, BehaviorKind, BindOp, BinaryOp, Block, EventSpec, Expr, Literal,
        Pattern, Stmt, UnaryOp,
    },
};

use crate::ir::*;

// ─── Context ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct LowerCtx {
    env: HashMap<String, IrExpr>,
    state_vars: Vec<IrStateVar>,
    noise_sources: Vec<IrNoiseSource>,
    counter: u32,
}

impl LowerCtx {
    fn new() -> Self {
        Self {
            env: HashMap::new(),
            state_vars: vec![],
            noise_sources: vec![],
            counter: 0,
        }
    }

    fn alloc_state(&mut self, kind: IrStateKind, arg: IrExpr) -> u32 {
        let id = self.counter;
        self.counter += 1;
        self.state_vars.push(IrStateVar { id, kind, arg });
        id
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn ppr_to_ir(prog: &ElabProgram) -> IrProgram {
    let mut modules: HashMap<String, IrModule> = prog
        .modules
        .iter()
        .map(|(name, m)| (name.clone(), convert_mod(m)))
        .collect();

    // Attach behaviors to their modules
    for behavior in &prog.behaviors {
        let Some(module) = modules.get_mut(&behavior.name) else { continue };
        let mut ctx = LowerCtx::new();
        let stmts = lower_stmts(&behavior.body, &mut ctx);
        match behavior.kind {
            BehaviorKind::Analog => {
                module.analog = Some(IrAnalogBody {
                    state_vars: ctx.state_vars,
                    noise_sources: ctx.noise_sources,
                    vars: vec![],
                    stmts,
                });
            }
            BehaviorKind::Digital => {
                module.digital = Some(IrDigitalBody {
                    inputs: vec![],
                    outputs: vec![],
                    state_vars: vec![],
                    stmts,
                });
            }
        }
    }

    // Convert global functions
    let functions = prog.functions.values().map(convert_fn).collect();

    IrProgram {
        source: "ppr".into(),
        modules: modules.into_values().collect(),
        functions,
    }
}

// ─── Module conversion ───────────────────────────────────────────────────────

fn convert_mod(m: &ElabMod) -> IrModule {
    use piperine_lang::parse::ast::Direction;

    let ports = m.ports.iter().map(|p| {
        IrPort {
            name: p.name.clone(),
            direction: match p.direction {
                Direction::Input => IrDirection::In,
                Direction::Output => IrDirection::Out,
                Direction::Inout => IrDirection::Inout,
            },
            discipline: discipline_name(&p.ty),
        }
    }).collect();

    let params = m.params.iter().map(|p| {
        IrParam {
            name: p.name.clone(),
            ty: elab_value_type_to_ir(&p.ty),
            default: p.default.as_ref().map(const_val_to_ir),
        }
    }).collect();

    let wires = m.wires.iter().map(|w| {
        IrWire {
            name: w.name.clone(),
            discipline: discipline_name(&w.ty),
        }
    }).collect();

    let instances = m.instances.iter().filter_map(|inst| {
        Some(IrInstance {
            label: inst.label.clone().unwrap_or_else(|| inst.module.clone()),
            module: inst.module.clone(),
            connections: inst.ports.iter().map(|r| IrConnection {
                port: None,
                net: r.to_string(),
            }).collect(),
            params: inst.params.iter().map(|(k, v)| (k.clone(), const_val_to_ir(v))).collect(),
        })
    }).collect();

    // Net connections (aliasing)
    let connections = m.connections.iter().map(|c| IrConnectionDecl {
        lhs: c.lhs.to_string(),
        rhs: c.rhs.to_string(),
    }).collect();

    IrModule {
        name: m.name.clone(),
        ports,
        params,
        wires,
        branches: vec![],
        events: vec![],
        vars: vec![],
        grounds: vec![],
        instances,
        connections,
        continuous_assigns: vec![],
        analog: None,
        digital: None,
        functions: vec![],
    }
}

fn discipline_name(ty: &ElabNetType) -> Option<String> {
    match ty {
        ElabNetType::Discipline(s) => Some(s.clone()),
        ElabNetType::Array(inner, _) => discipline_name(inner),
    }
}

fn elab_value_type_to_ir(ty: &ElabValueType) -> IrType {
    match ty {
        ElabValueType::Real | ElabValueType::Natural => IrType::Real,
        ElabValueType::Integer => IrType::Integer,
        ElabValueType::Complex => IrType::Complex,
        ElabValueType::Boolean => IrType::Bool,
        ElabValueType::Quad => IrType::Quad,
        ElabValueType::Str => IrType::String,
        ElabValueType::Enum(_) => IrType::Integer,
        ElabValueType::Array(inner, _) => elab_value_type_to_ir(inner),
        ElabValueType::FnPtr(_, _) => IrType::Void,
    }
}

fn const_val_to_ir(v: &piperine_lang::elab::const_eval::ConstVal) -> IrExpr {
    use piperine_lang::elab::const_eval::ConstVal;
    match v {
        ConstVal::Real(r) => IrExpr::Real(*r),
        ConstVal::Nat(n) => IrExpr::Int(*n as i64),
        ConstVal::Int(i) => IrExpr::Int(*i),
        ConstVal::Bool(b) => IrExpr::Bool(*b),
        ConstVal::Str(s) => IrExpr::String(s.clone()),
    }
}

fn convert_fn(f: &ElabFn) -> IrFunction {
    let params: Vec<String> = f.params.iter().map(|(n, _)| n.clone()).collect();
    let mut ctx = LowerCtx::new();
    let body = lower_stmts(&f.body, &mut ctx);
    IrFunction { name: f.name.clone(), params, body }
}

// ─── Statement lowering ───────────────────────────────────────────────────────

fn lower_stmts(stmts: &[ElabBehaviorStmt], ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let mut out = vec![];
    for stmt in stmts {
        out.extend(lower_stmt(stmt, ctx));
    }
    out
}

fn lower_stmt(stmt: &ElabBehaviorStmt, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    match stmt {
        ElabBehaviorStmt::VarDecl { name, default: Some(expr), .. } => {
            let val = lower_expr(expr, ctx);
            ctx.env.insert(name.clone(), val);
            vec![]
        }
        ElabBehaviorStmt::VarDecl { name, default: None, .. } => {
            ctx.env.insert(name.clone(), IrExpr::Real(0.0));
            vec![]
        }
        ElabBehaviorStmt::Bind { dest, op: BindOp::Assign, src } => {
            if let Expr::Ident(name) = dest {
                let val = lower_expr(src, ctx);
                ctx.env.insert(name.clone(), val);
                vec![]
            } else {
                vec![]
            }
        }
        ElabBehaviorStmt::Bind { dest, op: BindOp::Contrib, src } => {
            let (nature, plus, minus) = parse_contrib_dest(dest);
            scan_noise(src, &plus, &minus, ctx);
            let expr = lower_expr(src, ctx);
            let kind = if let Some(id) = first_state_ref(&expr) {
                ContribKind::Reactive(id)
            } else {
                ContribKind::Resistive
            };
            vec![IrStmt::Contrib { nature, plus, minus, expr, kind }]
        }
        ElabBehaviorStmt::Bind { dest, op: BindOp::Force, src } => {
            let (nature, plus, minus) = parse_contrib_dest(dest);
            let expr = lower_expr(src, ctx);
            vec![IrStmt::Force { nature, plus, minus, expr }]
        }

        ElabBehaviorStmt::If { cond, then_body, else_body } => {
            let cond_ir = lower_expr(cond, ctx);
            let pre_env = ctx.env.clone();
            let mut then_ctx = ctx.clone();
            let then_ = lower_stmts(then_body, &mut then_ctx);
            let mut else_ctx = ctx.clone();
            let else_ = else_body
                .as_ref()
                .map(|b| lower_stmts(b, &mut else_ctx))
                .unwrap_or_default();
            merge_branch_ctx(&pre_env, &then_ctx, &else_ctx, &cond_ir, ctx);
            vec![IrStmt::If { cond: cond_ir, then_, else_, label: None }]
        }

        ElabBehaviorStmt::Match { expr, arms } => {
            lower_match(expr, arms, ctx)
        }

        ElabBehaviorStmt::Event { spec, guard, body } => {
            let kinds = convert_event_spec(spec, ctx);
            let body_ir = lower_stmts(body, &mut ctx.clone());
            // Wrap body in guard if present
            let body_with_guard = if let Some(guard_expr) = guard {
                let guard_ir = lower_expr(guard_expr, &mut ctx.clone());
                vec![IrStmt::If { cond: guard_ir, then_: body_ir, else_: vec![], label: None }]
            } else {
                body_ir
            };
            kinds.into_iter().map(|kind| IrStmt::AnalogEvent {
                kind,
                body: body_with_guard.clone(),
            }).collect()
        }

        ElabBehaviorStmt::Diagnostic { sys, args } => {
            let bare = sys.trim_start_matches('$');
            // Special system tasks that are not display-family
            match bare {
                "bound_step" => {
                    let e = args.first()
                        .map(|a| lower_expr(a, ctx))
                        .unwrap_or(IrExpr::Real(0.0));
                    return vec![IrStmt::BoundStep(e)];
                }
                "finish" | "stop" => return vec![IrStmt::Finish],
                "discontinuity" => {
                    let n = args.first()
                        .and_then(|a| {
                            if let Expr::Literal(Literal::Int(n)) = a {
                                Some(*n as i32)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    return vec![IrStmt::Discontinuity(n)];
                }
                _ => {}
            }
            let severity = match bare {
                "warning" | "warn" => Severity::Warning,
                "error" => Severity::Error,
                "fatal" => Severity::Fatal,
                _ => Severity::Info,
            };
            // Extract format string from first arg if it's a string literal
            let (fmt, ir_args) = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                (s.clone(), args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect())
            } else {
                (String::new(), args.iter().map(|a| lower_expr(a, ctx)).collect())
            };
            vec![IrStmt::Diagnostic { severity, format: fmt, args: ir_args }]
        }

        ElabBehaviorStmt::Expr(e) => lower_expr_stmt(e, ctx),
    }
}

fn lower_expr_stmt(expr: &Expr, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    if let Expr::SysCall(name, args) = expr {
        match name.trim_start_matches('$') {
            "bound_step" => {
                let e = args.first()
                    .map(|a| lower_expr(a, ctx))
                    .unwrap_or(IrExpr::Real(0.0));
                return vec![IrStmt::BoundStep(e)];
            }
            "finish" | "stop" => return vec![IrStmt::Finish],
            "discontinuity" => {
                let n = args.first()
                    .and_then(|a| {
                        if let Expr::Literal(Literal::Int(n)) = a {
                            Some(*n as i32)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                return vec![IrStmt::Discontinuity(n)];
            }
            n if matches!(n, "display" | "write" | "strobe" | "monitor" | "warning" | "warn" | "error" | "fatal" | "info") => {
                let severity = match n {
                    "warning" | "warn" => Severity::Warning,
                    "error" => Severity::Error,
                    "fatal" => Severity::Fatal,
                    _ => Severity::Info,
                };
                let (fmt, ir_args) = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                    (s.clone(), args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect())
                } else {
                    (String::new(), args.iter().map(|a| lower_expr(a, ctx)).collect())
                };
                return vec![IrStmt::Diagnostic { severity, format: fmt, args: ir_args }];
            }
            _ => {}
        }
    }
    vec![]
}

/// Desugar a `match` into an if/else-if chain.
fn lower_match(expr: &Expr, arms: &[piperine_lang::elab::ir::ElabMatchArm], ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let discriminant = lower_expr(expr, ctx);
    let mut default_body = vec![];
    let mut chain: Vec<(IrExpr, Vec<IrStmt>)> = vec![];

    for arm in arms {
        match &arm.pat {
            Pattern::Wildcard => {
                default_body = lower_stmts(&arm.body, &mut ctx.clone());
            }
            Pattern::Path(p) => {
                let pat_expr = IrExpr::Param(p.segments.join("::"));
                let body = lower_stmts(&arm.body, &mut ctx.clone());
                chain.push((pat_expr, body));
            }
        }
    }

    // Build if/else-if chain from the arms
    if chain.is_empty() {
        return default_body;
    }

    // Build from the inside out
    let mut result = default_body;
    for (pat, body) in chain.into_iter().rev() {
        let cond = IrExpr::Binary(IrBinOp::Eq, Box::new(discriminant.clone()), Box::new(pat));
        result = vec![IrStmt::If {
            cond,
            then_: body,
            else_: result,
            label: None,
        }];
    }
    result
}

// ─── Event spec conversion ───────────────────────────────────────────────────

fn convert_event_spec(spec: &EventSpec, ctx: &mut LowerCtx) -> Vec<IrEventKind> {
    match spec {
        EventSpec::Initial => vec![IrEventKind::InitialStep],
        EventSpec::Final => vec![IrEventKind::FinalStep],
        EventSpec::Named { name, arg } => {
            let arg_ir = lower_expr(arg, &mut ctx.clone());
            match name.as_str() {
                "cross" => vec![IrEventKind::Cross { dir: 0, expr: Some(arg_ir) }],
                "above" => vec![IrEventKind::Above { expr: Some(arg_ir) }],
                "timer" => vec![IrEventKind::Timer { period: Some(arg_ir) }],
                _ => vec![IrEventKind::InitialStep],
            }
        }
        EventSpec::Or(specs) => {
            specs.iter().flat_map(|s| convert_event_spec(s, ctx)).collect()
        }
    }
}

// ─── Destination parsing ──────────────────────────────────────────────────────

fn parse_contrib_dest(dest: &Expr) -> (IrNature, String, String) {
    if let Expr::Call(func, args) = dest {
        if let Expr::Ident(name) = func.as_ref() {
            let nature = access_to_nature(name);
            let plus = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
            let minus = ident_from_expr(args.get(1)).unwrap_or_else(|| "?".into());
            return (nature, plus, minus);
        }
    }
    (IrNature::Flow("I".into()), "?".into(), "?".into())
}

/// Map an access function name to its nature (potential or flow).
fn access_to_nature(name: &str) -> IrNature {
    match name {
        "V" => IrNature::Potential("V".into()),
        "I" => IrNature::Flow("I".into()),
        _ => IrNature::Flow(name.into()),
    }
}

fn ident_from_expr(e: Option<&Expr>) -> Option<String> {
    match e? {
        Expr::Ident(s) => Some(s.clone()),
        _ => None,
    }
}

// ─── Noise extraction ──────────────────────────────────────────────────────────

fn scan_noise(expr: &Expr, plus: &str, minus: &str, ctx: &mut LowerCtx) {
    match expr {
        Expr::Call(func, args) => {
            if let Expr::Ident(name) = func.as_ref() {
                match name.as_str() {
                    "white_noise" => {
                        let psd = args.first()
                            .map(|a| lower_expr(a, ctx))
                            .unwrap_or(IrExpr::Real(0.0));
                        let label = args.get(1).and_then(|a| {
                            if let Expr::Literal(Literal::String(s)) = a { Some(s.clone()) } else { None }
                        });
                        ctx.noise_sources.push(IrNoiseSource {
                            plus: plus.into(),
                            minus: minus.into(),
                            kind: IrNoise::White { psd },
                            label,
                        });
                        return;
                    }
                    "flicker_noise" => {
                        let psd = args.first()
                            .map(|a| lower_expr(a, ctx))
                            .unwrap_or(IrExpr::Real(0.0));
                        let exponent = args.get(1)
                            .map(|a| lower_expr(a, ctx))
                            .unwrap_or(IrExpr::Real(1.0));
                        let label = args.get(2).and_then(|a| {
                            if let Expr::Literal(Literal::String(s)) = a { Some(s.clone()) } else { None }
                        });
                        ctx.noise_sources.push(IrNoiseSource {
                            plus: plus.into(),
                            minus: minus.into(),
                            kind: IrNoise::Flicker { psd, exponent },
                            label,
                        });
                        return;
                    }
                    _ => {}
                }
            }
            for arg in args {
                scan_noise(arg, plus, minus, ctx);
            }
        }
        Expr::Binary(l, _, r) => {
            scan_noise(l, plus, minus, ctx);
            scan_noise(r, plus, minus, ctx);
        }
        Expr::Unary(_, inner) => scan_noise(inner, plus, minus, ctx),
        Expr::If { cond, then_body, else_body } => {
            scan_noise_expr_block(cond, plus, minus, ctx);
            scan_noise_block(then_body, plus, minus, ctx);
            scan_noise_block(else_body, plus, minus, ctx);
        }
        _ => {}
    }
}

fn scan_noise_block(block: &Block, plus: &str, minus: &str, ctx: &mut LowerCtx) {
    for s in &block.stmts {
        if let Stmt::Bind { op: BindOp::Contrib, src, .. } = s {
            scan_noise(src, plus, minus, ctx);
        }
    }
}

fn scan_noise_expr_block(expr: &Expr, plus: &str, minus: &str, ctx: &mut LowerCtx) {
    scan_noise(expr, plus, minus, ctx);
}

// ─── Expression lowering ──────────────────────────────────────────────────────

fn lower_expr(expr: &Expr, ctx: &mut LowerCtx) -> IrExpr {
    match expr {
        Expr::Literal(Literal::Real(f)) => IrExpr::Real(*f),
        Expr::Literal(Literal::Int(n)) => IrExpr::Int(*n as i64),
        Expr::Literal(Literal::Bool(b)) => IrExpr::Bool(*b),
        Expr::Literal(Literal::String(s)) => IrExpr::String(s.clone()),
        Expr::Literal(Literal::Quad(s)) => {
            let val = match s.trim_start_matches("0q") {
                "0" | "" => 0u8,
                "1" => 1,
                "X" | "x" => 2,
                "Z" | "z" => 3,
                _ => 0,
            };
            IrExpr::Quad(val)
        }

        Expr::Ident(name) => {
            if let Some(val) = ctx.env.get(name) {
                val.clone()
            } else {
                IrExpr::Param(name.clone())
            }
        }

        Expr::Path(p) => {
            let name = p.segments.join("::");
            if let Some(val) = ctx.env.get(&name) {
                val.clone()
            } else {
                IrExpr::Param(name)
            }
        }

        Expr::Unary(UnaryOp::Neg, inner) => {
            IrExpr::Unary(IrUnOp::Neg, Box::new(lower_expr(inner, ctx)))
        }
        Expr::Unary(UnaryOp::Not, inner) => {
            IrExpr::Unary(IrUnOp::Not, Box::new(lower_expr(inner, ctx)))
        }

        Expr::Binary(lhs, op, rhs) => {
            let l = Box::new(lower_expr(lhs, ctx));
            let r = Box::new(lower_expr(rhs, ctx));
            IrExpr::Binary(lower_binop(op), l, r)
        }

        Expr::Call(func, args) => lower_call(func, args, ctx),

        Expr::SysCall(name, args) => lower_syscall(name, args, ctx),

        Expr::If { cond, then_body, else_body } => {
            let c = Box::new(lower_expr(cond, ctx));
            let t = Box::new(block_value(then_body, ctx));
            let e = Box::new(block_value(else_body, ctx));
            IrExpr::Select(c, t, e)
        }

        Expr::Block(b) => block_value(b, ctx),

        Expr::Index(base, idx) => {
            IrExpr::Index(
                Box::new(lower_expr(base, ctx)),
                Box::new(lower_expr(idx, ctx)),
            )
        }

        Expr::Slice(base, range) => {
            IrExpr::Slice(
                Box::new(lower_expr(base, ctx)),
                Box::new(IrRange {
                    start: lower_expr(&range.start, ctx),
                    end: lower_expr(&range.end, ctx),
                    inclusive: range.inclusive,
                }),
            )
        }

        Expr::Field(base, field) => {
            // Flatten bundle field access: a.field → a_field
            let base_name = expr_to_name(base);
            IrExpr::Param(format!("{base_name}_{field}"))
        }

        Expr::Array(body) => lower_array(body, ctx),

        // Unsupported in analog scalar context
        Expr::BundleLit { .. } | Expr::Lambda { .. } => IrExpr::Real(0.0),
    }
}

fn lower_array(body: &ArrayBody, ctx: &mut LowerCtx) -> IrExpr {
    match body {
        ArrayBody::List(exprs) => {
            IrExpr::Array(exprs.iter().map(|e| lower_expr(e, ctx)).collect())
        }
        ArrayBody::Repeat(v, n) => {
            IrExpr::ArrayRepeat(
                Box::new(lower_expr(v, ctx)),
                Box::new(lower_expr(n, ctx)),
            )
        }
        ArrayBody::Comprehension(expr, var, range) => {
            // Try to unroll if bounds are const
            if let (Some(start), Some(end)) = (
                eval_const_int(&range.start),
                eval_const_int(&range.end),
            ) {
                let inclusive = range.inclusive as i64;
                let mut elems = vec![];
                for i in start..(end + inclusive) {
                    let mut iter_ctx = ctx.clone();
                    iter_ctx.env.insert(var.clone(), IrExpr::Int(i));
                    elems.push(lower_expr(expr, &mut iter_ctx));
                }
                IrExpr::Array(elems)
            } else {
                IrExpr::Array(vec![])
            }
        }
    }
}

fn eval_const_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int(n)) => Some(*n as i64),
        Expr::Literal(Literal::Bool(true)) => Some(1),
        Expr::Literal(Literal::Bool(false)) => Some(0),
        Expr::Literal(Literal::Real(f)) => Some(*f as i64),
        _ => None,
    }
}

fn expr_to_name(expr: &Expr) -> String {
    match expr {
        Expr::Ident(s) => s.clone(),
        Expr::Path(p) => p.segments.join("::"),
        Expr::Field(base, field) => format!("{}_{}", expr_to_name(base), field),
        _ => "_".into(),
    }
}

fn block_value(block: &Block, ctx: &mut LowerCtx) -> IrExpr {
    // Process statements for side effects (var decls)
    for s in &block.stmts {
        if let Stmt::VarDecl { name, default: Some(expr), .. } = s {
            let val = lower_expr(expr, ctx);
            ctx.env.insert(name.clone(), val);
        }
    }
    if let Some(e) = &block.expr {
        return lower_expr(e, ctx);
    }
    // Last stmt that's an Expr
    for s in block.stmts.iter().rev() {
        if let Stmt::Expr(e) = s {
            return lower_expr(e, ctx);
        }
    }
    IrExpr::Real(0.0)
}

fn lower_call(func: &Expr, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
    let name = match func {
        Expr::Ident(s) => s.as_str(),
        _ => return IrExpr::Real(0.0),
    };

    match name {
        "V" | "I" => {
            if args.len() >= 2 {
                let plus = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
                let minus = ident_from_expr(Some(&args[1])).unwrap_or_else(|| "?".into());
                IrExpr::BranchAccess { access: name.to_string(), plus, minus }
            } else if args.len() == 1 {
                let node = ident_from_expr(Some(&args[0])).unwrap_or_else(|| "?".into());
                IrExpr::BranchAccess { access: name.to_string(), plus: node, minus: "0".into() }
            } else {
                IrExpr::BranchAccess { access: name.to_string(), plus: "?".into(), minus: "0".into() }
            }
        }
        "ddt" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let id = ctx.alloc_state(IrStateKind::Ddt, arg);
            IrExpr::StateRef(id)
        }
        "idt" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let ic = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Idt { ic }, arg);
            IrExpr::StateRef(id)
        }
        "idtmod" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let ic = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let modulus = args.get(2).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(1.0));
            let id = ctx.alloc_state(IrStateKind::IdtMod { ic, modulus }, arg);
            IrExpr::StateRef(id)
        }
        "ddx" if args.len() >= 2 => {
            let arg = lower_expr(&args[0], ctx);
            let node = ident_from_expr(Some(&args[1])).unwrap_or_else(|| "?".into());
            let id = ctx.alloc_state(IrStateKind::Ddx { node }, arg);
            IrExpr::StateRef(id)
        }
        "delay" | "absdelay" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let delay = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Delay { delay }, arg);
            IrExpr::StateRef(id)
        }
        "transition" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let delay = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let rise = args.get(2).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let fall = args.get(3).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let tol = args.get(4).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Transition { delay, rise, fall, tol }, arg);
            IrExpr::StateRef(id)
        }
        "slew" if !args.is_empty() => {
            let arg = lower_expr(&args[0], ctx);
            let rise = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let fall = args.get(2).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Slew { rise, fall }, arg);
            IrExpr::StateRef(id)
        }
        "laplace_np" | "laplace_zp" | "laplace_pm" | "laplace_nm" | "laplace_npm"
            if args.len() >= 3 =>
        {
            let arg = lower_expr(&args[0], ctx);
            let num = lower_expr(&args[1], ctx);
            let den = lower_expr(&args[2], ctx);
            let id = ctx.alloc_state(
                IrStateKind::Laplace {
                    variant: name.trim_start_matches("laplace_").to_string(),
                    num,
                    den,
                },
                arg,
            );
            IrExpr::StateRef(id)
        }
        "zi_zd" | "zi_zp" | "zi_nd" | "zi_np" if args.len() >= 4 => {
            let arg = lower_expr(&args[0], ctx);
            let num = lower_expr(&args[1], ctx);
            let den = lower_expr(&args[2], ctx);
            let sample_dt = lower_expr(&args[3], ctx);
            let id = ctx.alloc_state(
                IrStateKind::ZTransform {
                    variant: name.trim_start_matches("zi_").to_string(),
                    num,
                    den,
                    sample_dt,
                },
                arg,
            );
            IrExpr::StateRef(id)
        }
        "ac_stim" => {
            let mag = args.first().map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(1.0));
            let phase = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            IrExpr::AcStim { mag: Box::new(mag), phase: Box::new(phase) }
        }
        "white_noise" | "flicker_noise" => {
            // Noise sources tracked separately via scan_noise; return 0 in expr position.
            IrExpr::Real(0.0)
        }
        "analysis" => {
            let kind = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "dc".into()
            };
            IrExpr::Sim(SimQuery::Analysis(kind))
        }
        _ => {
            let ir_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
            IrExpr::Call(name.to_string(), ir_args)
        }
    }
}

fn lower_syscall(name: &str, args: &[Expr], ctx: &mut LowerCtx) -> IrExpr {
    match name.trim_start_matches('$').to_lowercase().as_str() {
        "temperature" => IrExpr::Sim(SimQuery::Temperature),
        "vt" => {
            if args.is_empty() {
                IrExpr::Sim(SimQuery::Vt(None))
            } else {
                IrExpr::Sim(SimQuery::Vt(Some(Box::new(lower_expr(&args[0], ctx)))))
            }
        }
        "abstime" => IrExpr::Sim(SimQuery::Abstime),
        "mfactor" => IrExpr::Sim(SimQuery::Mfactor),
        "xposition" => IrExpr::Sim(SimQuery::XPosition),
        "yposition" => IrExpr::Sim(SimQuery::YPosition),
        "angle" => IrExpr::Sim(SimQuery::Angle),
        "simparam" => {
            let key = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            let default = args.get(1).map(|a| lower_expr(a, ctx)).unwrap_or(IrExpr::Real(0.0));
            IrExpr::Sim(SimQuery::Simparam { key, default: Box::new(default) })
        }
        "param_given" => {
            let name = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            IrExpr::Sim(SimQuery::ParamGiven(name))
        }
        "port_connected" => {
            let name = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            IrExpr::Sim(SimQuery::PortConnected(name))
        }
        "limit" => {
            let kind = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "?".into()
            };
            let limit_args = args.iter().skip(1).map(|a| lower_expr(a, ctx)).collect();
            IrExpr::Sim(SimQuery::Limit { kind, args: limit_args })
        }
        "random" => {
            IrExpr::Sim(SimQuery::Random { kind: "random".into(), args: vec![] })
        }
        n if n.starts_with("dist_") => {
            let dist_args = args.iter().map(|a| lower_expr(a, ctx)).collect();
            IrExpr::Sim(SimQuery::Random { kind: n.to_string(), args: dist_args })
        }
        "analysis" => {
            let kind = if let Some(Expr::Literal(Literal::String(s))) = args.first() {
                s.clone()
            } else {
                "dc".into()
            };
            IrExpr::Sim(SimQuery::Analysis(kind))
        }
        _ => IrExpr::Real(0.0),
    }
}

// ─── Phi-node env merge ───────────────────────────────────────────────────────

fn merge_branch_ctx(
    pre_env: &std::collections::HashMap<String, IrExpr>,
    then_ctx: &LowerCtx,
    else_ctx: &LowerCtx,
    cond: &IrExpr,
    ctx: &mut LowerCtx,
) {
    let all_keys: std::collections::HashSet<&String> = then_ctx.env.keys()
        .chain(else_ctx.env.keys())
        .collect();

    for key in all_keys {
        let pre_val = pre_env.get(key);
        let then_val = then_ctx.env.get(key);
        let else_val = else_ctx.env.get(key);

        let then_changed = then_val != pre_val;
        let else_changed = else_val != pre_val;
        if !then_changed && !else_changed {
            continue;
        }

        let tv = then_val.or(pre_val).cloned().unwrap_or(IrExpr::Real(0.0));
        let ev = else_val.or(pre_val).cloned().unwrap_or(IrExpr::Real(0.0));
        ctx.env.insert(key.clone(), IrExpr::Select(
            Box::new(cond.clone()),
            Box::new(tv),
            Box::new(ev),
        ));
    }

    for sv in then_ctx.state_vars.iter().chain(else_ctx.state_vars.iter()) {
        if !ctx.state_vars.iter().any(|s| s.id == sv.id) {
            ctx.state_vars.push(sv.clone());
        }
    }
    ctx.noise_sources.extend(then_ctx.noise_sources.iter().cloned());
    ctx.noise_sources.extend(else_ctx.noise_sources.iter().cloned());
    ctx.counter = ctx.counter.max(then_ctx.counter).max(else_ctx.counter);
}

fn lower_binop(op: &BinaryOp) -> IrBinOp {
    match op {
        BinaryOp::Add => IrBinOp::Add,
        BinaryOp::Sub => IrBinOp::Sub,
        BinaryOp::Mul => IrBinOp::Mul,
        BinaryOp::Div => IrBinOp::Div,
        BinaryOp::Rem => IrBinOp::Rem,
        BinaryOp::Eq => IrBinOp::Eq,
        BinaryOp::Neq => IrBinOp::Ne,
        BinaryOp::Lt => IrBinOp::Lt,
        BinaryOp::Le => IrBinOp::Le,
        BinaryOp::Gt => IrBinOp::Gt,
        BinaryOp::Ge => IrBinOp::Ge,
        BinaryOp::BitAnd => IrBinOp::BitAnd,
        BinaryOp::BitOr => IrBinOp::BitOr,
        BinaryOp::BitXor => IrBinOp::BitXor,
    }
}
