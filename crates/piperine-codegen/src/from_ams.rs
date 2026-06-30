//! Lower `piperine_ams::Document` (Verilog-A/AMS) → `IrProgram`.

use std::collections::HashMap;

use piperine_ams::{
    Document,
    ast::{
        AssignOp, BinOp, BlockItem, CallArg, CaseItem, EventControl, EventExpr, Expr,
        ForStmt, FunctionRef, IndirectContribution, Literal, ParamAssignment, Path,
        PathSegment, PortConnection, PrefixOp, Stmt, TimingControl, TimingControlStmt,
        Type as AmsType,
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

pub fn ams_to_ir(doc: &Document) -> IrProgram {
    let modules = doc.modules.iter().map(convert_module).collect();
    IrProgram {
        source: "ams".into(),
        modules,
        functions: vec![],
    }
}

// ─── Module conversion ───────────────────────────────────────────────────────

fn convert_module(m: &piperine_ams::Module) -> IrModule {
    use piperine_ams::ast::Direction;

    let ports = m.ports.iter().map(|p| IrPort {
        name: p.name.clone(),
        direction: match p.direction {
            Direction::Input => IrDirection::In,
            Direction::Output => IrDirection::Out,
            Direction::Inout => IrDirection::Inout,
        },
        discipline: p.discipline.clone(),
    }).collect();

    let params = m.parameters.iter().map(|p| {
        let mut ctx = LowerCtx::new();
        IrParam {
            name: p.name.clone(),
            ty: type_to_ir(p.ty.as_ref()),
            default: Some(lower_expr(&p.default_value, &mut ctx)),
        }
    }).collect();

    let wires = m.nets.iter().flat_map(|net| {
        net.members.iter().map(|member| IrWire {
            name: member.name.clone(),
            discipline: net.discipline.clone(),
        })
    }).collect();

    let branches = m.branches.iter().flat_map(|br| {
        let (plus, minus) = extract_branch_ports(&br.ports);
        br.names.iter().map(move |name| IrBranch {
            name: name.clone(),
            plus: plus.clone(),
            minus: minus.clone(),
        })
    }).collect();

    let events = m.events.iter().flat_map(|ev| {
        ev.names.iter().map(|decl| IrEventDecl { name: decl.name.0.clone() })
    }).collect();

    let instances = m.instances.iter().map(|inst| {
        let connections: Vec<IrConnection> = inst.connections.iter().filter_map(|c| match c {
            PortConnection::Ordered(Some(e)) => {
                Some(IrConnection { port: None, net: path_leaf_ident(e).unwrap_or_else(|| "?".into()) })
            }
            PortConnection::Named { port, expr: Some(e) } => {
                Some(IrConnection { port: Some(port.0.clone()), net: path_leaf_ident(e).unwrap_or_else(|| "?".into()) })
            }
            PortConnection::Named { port, expr: None } => {
                Some(IrConnection { port: Some(port.0.clone()), net: "?".into() })
            }
            _ => None,
        }).collect();
        let params: Vec<(String, IrExpr)> = inst.param_assignments.iter().filter_map(|pa| {
            match pa {
                ParamAssignment::Named { param, expr } => {
                    let mut ctx = LowerCtx::new();
                    Some((param.0.clone(), lower_expr(expr, &mut ctx)))
                }
                ParamAssignment::SystemNamed { param, expr } => {
                    let mut ctx = LowerCtx::new();
                    Some((param.clone(), lower_expr(expr, &mut ctx)))
                }
                ParamAssignment::Ordered(expr) => {
                    let mut ctx = LowerCtx::new();
                    Some(("_".into(), lower_expr(expr, &mut ctx)))
                }
            }
        }).collect();
        IrInstance {
            label: inst.instance_name.clone(),
            module: inst.module_name.clone(),
            connections,
            params,
        }
    }).collect();

    // Combine all analog blocks into one body
    let mut ctx = LowerCtx::new();
    let mut all_stmts = vec![];
    for block in &m.analog_blocks {
        if block.is_initial {
            let inner = lower_stmt(&block.stmt, &mut ctx);
            all_stmts.push(IrStmt::AnalogEvent {
                kind: IrEventKind::InitialStep,
                body: inner,
            });
        } else {
            all_stmts.extend(lower_stmt(&block.stmt, &mut ctx));
        }
    }

    let analog = if all_stmts.is_empty() && ctx.state_vars.is_empty() && ctx.noise_sources.is_empty() {
        None
    } else {
        Some(IrAnalogBody {
            state_vars: ctx.state_vars,
            noise_sources: ctx.noise_sources,
            vars: vec![],
            stmts: all_stmts,
        })
    };

    // Functions and tasks
    let functions = m.functions.iter()
        .map(convert_function)
        .chain(m.tasks.iter().map(convert_task))
        .collect();

    // Module-level variables
    let vars = m.variables.iter().map(|v| IrVarDecl {
        name: v.name.clone(),
        ty: type_to_ir(Some(&v.ty)),
        init: v.default_value.as_ref().map(|e| {
            let mut c = LowerCtx::new();
            lower_expr(e, &mut c)
        }),
    }).collect();

    // Ground declarations
    let grounds = m.ground_decls.iter().flat_map(|g| {
        g.names.iter().map(|decl| IrGroundDecl {
            name: decl.name.0.clone(),
            discipline: g.discipline.as_ref().map(|d| d.0.clone()),
        })
    }).collect();

    // Continuous assigns
    let continuous_assigns = m.continuous_assigns.iter().flat_map(|ca| {
        ca.assignments.iter().map(|(lval, rval)| {
            let mut c = LowerCtx::new();
            let l = path_leaf_ident(lval).unwrap_or_else(|| "_".into());
            let r = lower_expr(rval, &mut c);
            IrStmt::ContinuousAssign { lval: l, expr: r, delay: None }
        })
    }).collect();

    IrModule {
        name: m.name.clone(),
        ports,
        params,
        wires,
        branches,
        events,
        vars,
        grounds,
        instances,
        connections: vec![],
        continuous_assigns,
        analog,
        digital: None,
        functions,
    }
}

fn extract_branch_ports(ports: &[Expr]) -> (String, String) {
    let plus = ports.first().and_then(path_leaf_ident).unwrap_or_else(|| "?".into());
    let minus = ports.get(1).and_then(path_leaf_ident).unwrap_or_else(|| "0".into());
    (plus, minus)
}

fn convert_function(f: &piperine_ams::Function) -> IrFunction {
    let params: Vec<String> = f.args.iter().map(|a| a.name.clone()).collect();
    let mut ctx = LowerCtx::new();
    // Seed the env with parameter defaults
    for p in &f.parameters {
        let val = lower_expr(&p.default_value, &mut ctx);
        ctx.env.insert(p.name.clone(), val);
    }
    // Seed variables
    for v in &f.variables {
        let val = v.default_value.as_ref()
            .map(|d| lower_expr(d, &mut ctx))
            .unwrap_or(IrExpr::Real(0.0));
        ctx.env.insert(v.name.clone(), val);
    }
    let mut body = vec![];
    for s in &f.body {
        body.extend(lower_stmt(s, &mut ctx));
    }
    // Implicit return of the function name variable
    if !ctx.env.contains_key(&f.name) {
        // If the function name was assigned, it's in env; add a return
    }
    if let Some(val) = ctx.env.get(&f.name) {
        body.push(IrStmt::Return(Some(val.clone())));
    }
    IrFunction { name: f.name.clone(), params, body }
}

fn convert_task(t: &piperine_ams::Task) -> IrFunction {
    let params: Vec<String> = t.ports.iter()
        .flat_map(|p| p.names.iter().map(|n| n.0.clone()))
        .collect();
    let mut ctx = LowerCtx::new();
    for v in &t.variables {
        let val = v.default_value.as_ref()
            .map(|d| lower_expr(d, &mut ctx))
            .unwrap_or(IrExpr::Real(0.0));
        ctx.env.insert(v.name.clone(), val);
    }
    let body = lower_stmt(&t.body, &mut ctx);
    IrFunction { name: t.name.clone(), params, body }
}

fn type_to_ir(ty: Option<&AmsType>) -> IrType {
    match ty {
        Some(AmsType::Integer) => IrType::Integer,
        Some(AmsType::Real) => IrType::Real,
        Some(AmsType::String) => IrType::String,
        Some(AmsType::Time) | Some(AmsType::Realtime) => IrType::Real,
        Some(AmsType::Reg) => IrType::Quad,
        Some(AmsType::Custom(_)) => IrType::Real,
        None => IrType::Real,
    }
}

// ─── Statement lowering ───────────────────────────────────────────────────────

fn lower_stmt(stmt: &Stmt, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    match stmt {
        Stmt::Empty(_) => vec![],

        Stmt::Assign(a) => {
            match a.assign.op {
                AssignOp::Contrib => {
                    let (nature, plus, minus) = parse_lval_contrib(&a.assign.lval);
                    scan_noise(&a.assign.rval, &plus, &minus, ctx);
                    let expr = lower_expr(&a.assign.rval, ctx);
                    let kind = if let Some(id) = first_state_ref(&expr) {
                        ContribKind::Reactive(id)
                    } else {
                        ContribKind::Resistive
                    };
                    vec![IrStmt::Contrib { nature, plus, minus, expr, kind }]
                }
                AssignOp::Eq => {
                    let name = path_leaf_ident(&a.assign.lval).unwrap_or_else(|| "_".into());
                    let val = lower_expr(&a.assign.rval, ctx);
                    ctx.env.insert(name, val);
                    vec![]
                }
            }
        }

        Stmt::If(s) => {
            let cond = lower_expr(&s.condition, ctx);
            let pre_env = ctx.env.clone();
            let mut then_ctx = ctx.clone();
            let then_ = lower_stmt(&s.then_branch, &mut then_ctx);
            let mut else_ctx = ctx.clone();
            let else_ = s.else_branch.as_ref()
                .map(|b| lower_stmt(b, &mut else_ctx))
                .unwrap_or_default();
            merge_branch_ctx(&pre_env, &then_ctx, &else_ctx, &cond, ctx);
            vec![IrStmt::If { cond, then_, else_, label: None }]
        }

        Stmt::Block(b) => {
            let mut out = vec![];
            for item in &b.items {
                match item {
                    BlockItem::VarDecl(vd) => {
                        for var in &vd.vars {
                            let val = var.default.as_ref()
                                .map(|d| lower_expr(d, ctx))
                                .unwrap_or(IrExpr::Real(0.0));
                            ctx.env.insert(var.name.0.clone(), val);
                        }
                    }
                    BlockItem::ParamDecl(pd) => {
                        for p in &pd.params {
                            let val = lower_expr(&p.default, ctx);
                            ctx.env.insert(p.name.0.clone(), val);
                        }
                    }
                    BlockItem::Stmt(s) => {
                        out.extend(lower_stmt(s, ctx));
                    }
                }
            }
            out
        }

        Stmt::Case(c) => {
            use piperine_ams::ast::CaseKind as AmsCaseKind;
            let kind = match c.kind {
                AmsCaseKind::Case => CaseKind::Case,
                AmsCaseKind::Casex => CaseKind::CaseX,
                AmsCaseKind::Casez => CaseKind::CaseZ,
            };
            let discriminant = lower_expr(&c.discriminant, ctx);
            let mut arms = vec![];
            let mut default = vec![];
            for case in &c.cases {
                match &case.item {
                    CaseItem::Exprs(exprs) => {
                        let body = lower_stmt(&case.stmt, &mut ctx.clone());
                        for e in exprs {
                            arms.push((lower_expr(e, ctx), body.clone()));
                        }
                    }
                    CaseItem::Default => {
                        default = lower_stmt(&case.stmt, &mut ctx.clone());
                    }
                }
            }
            vec![IrStmt::Case { discriminant, arms, default, kind, label: None }]
        }

        Stmt::Event(e) => {
            let kind = infer_event_kind(&e.event);
            let body = lower_stmt(&e.stmt, &mut ctx.clone());
            vec![IrStmt::AnalogEvent { kind, body }]
        }

        Stmt::Expr(e) => lower_expr_stmt(&e.expr, ctx),

        Stmt::For(f) => {
            try_unroll_for(f, ctx).unwrap_or_else(|| {
                // Can't unroll — emit a runtime for-loop if bounds are known,
                // otherwise lower body once as fallback.
                if let (Some((_, start)), Some((_, end, _))) =
                    (extract_int_assign(&f.init), extract_int_cmp(&f.condition))
                {
                    let step = extract_int_incr(&f.incr, "").map(|(_, s)| s).unwrap_or(1);
                    vec![IrStmt::For {
                        var: extract_int_assign(&f.init).map(|(n, _)| n).unwrap_or_default(),
                        start: IrExpr::Int(start),
                        end: IrExpr::Int(end),
                        step: IrExpr::Int(step),
                        body: lower_stmt(&f.for_body, &mut ctx.clone()),
                    }]
                } else {
                    lower_stmt(&f.for_body, &mut ctx.clone())
                }
            })
        }

        Stmt::While(w) => {
            let cond = lower_expr(&w.condition, ctx);
            let body = lower_stmt(&w.body, &mut ctx.clone());
            vec![IrStmt::While { cond, body }]
        }

        Stmt::Repeat(r) => {
            if let Some(n) = eval_const_int_expr(&r.count) {
                let mut out = vec![];
                for _ in 0..n.min(256) {
                    out.extend(lower_stmt(&r.body, ctx));
                }
                out
            } else {
                let count = lower_expr(&r.count, ctx);
                let body = lower_stmt(&r.body, &mut ctx.clone());
                vec![IrStmt::Repeat { count, body }]
            }
        }

        Stmt::Forever(f) => {
            let body = lower_stmt(&f.body, &mut ctx.clone());
            vec![IrStmt::Forever { body }]
        }

        Stmt::IndirectContrib(ic) => lower_indirect_contrib(ic, ctx),

        Stmt::NonBlockingAssign(nba) => {
            let lval = path_leaf_ident(&nba.lvalue).unwrap_or_else(|| "_".into());
            let expr = lower_expr(&nba.rvalue, ctx);
            let (delay, event) = lower_delay_event(&nba.delay_or_event);
            vec![IrStmt::NonBlocking { lval, expr, delay, event }]
        }

        Stmt::TimingControl(tc) => lower_timing_control(tc, ctx),

        Stmt::Wait(w) => {
            let cond = lower_expr(&w.condition, ctx);
            let body = Box::new(lower_stmt(&w.stmt, &mut ctx.clone()).into_iter().next().unwrap_or(IrStmt::Finish));
            vec![IrStmt::Wait { cond, body }]
        }

        Stmt::Fork(f) => {
            let branches: Vec<Vec<IrStmt>> = f.items.iter().map(|item| {
                match item {
                    BlockItem::Stmt(s) => lower_stmt(s, &mut ctx.clone()),
                    BlockItem::VarDecl(vd) => {
                        let mut tmp = vec![];
                        for var in &vd.vars {
                            let val = var.default.as_ref()
                                .map(|d| lower_expr(d, &mut LowerCtx::new()))
                                .unwrap_or(IrExpr::Real(0.0));
                            tmp.push(IrStmt::Assign {
                                lval: var.name.0.clone(),
                                expr: val,
                                delay: None,
                                event: None,
                            });
                        }
                        tmp
                    }
                    BlockItem::ParamDecl(_) => vec![],
                }
            }).collect();
            vec![IrStmt::Fork {
                label: f.label.as_ref().map(|n| n.0.clone()),
                branches,
                join: JoinKind::All,
            }]
        }

        Stmt::Disable(d) => {
            vec![IrStmt::Disable(path_to_string(&d.target))]
        }

        Stmt::EventTrigger(et) => {
            vec![IrStmt::Trigger(path_to_string(&et.event))]
        }

        Stmt::ProceduralAssign(pa) => {
            let lval = path_leaf_ident(&pa.lvalue).unwrap_or_else(|| "_".into());
            let expr = lower_expr(&pa.rvalue, ctx);
            vec![IrStmt::ProcAssign { lval, expr, is_force: pa.is_force }]
        }

        Stmt::ProceduralDeassign(pd) => {
            let lval = path_leaf_ident(&pd.lvalue).unwrap_or_else(|| "_".into());
            vec![IrStmt::ProcDeassign { lval, is_release: pd.is_release }]
        }
    }
}

/// Lower an optional timing control (delay or event) from an assignment.
fn lower_delay_event(tc: &Option<TimingControl>) -> (Option<IrExpr>, Option<IrEventSpec>) {
    match tc {
        Some(TimingControl::Delay(e)) | Some(TimingControl::DelayParen(e)) => {
            let mut ctx = LowerCtx::new();
            (Some(lower_expr(e, &mut ctx)), None)
        }
        Some(TimingControl::Event(ec)) => {
            let spec = convert_event_control(ec);
            (None, Some(spec))
        }
        None => (None, None),
    }
}

fn lower_timing_control(tc: &TimingControlStmt, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let inner = lower_stmt(&tc.stmt, ctx);
    match &tc.control {
        TimingControl::Delay(e) | TimingControl::DelayParen(e) => {
            let delay = lower_expr(e, ctx);
            inner.into_iter().map(|s| IrStmt::Delay {
                delay: delay.clone(),
                body: Box::new(s),
            }).collect()
        }
        TimingControl::Event(ec) => {
            let spec = convert_event_control(ec);
            inner.into_iter().map(|s| IrStmt::EventControl {
                spec: spec.clone(),
                body: Box::new(s),
            }).collect()
        }
    }
}

fn convert_event_control(ec: &EventControl) -> IrEventSpec {
    match ec {
        EventControl::Ident(p) => IrEventSpec::Named(path_to_string(p)),
        EventControl::Star => IrEventSpec::Named("*".into()),
        EventControl::Expr(exprs) => {
            let specs: Vec<IrEventSpec> = exprs.iter().map(convert_event_expr).collect();
            if specs.len() == 1 {
                specs.into_iter().next().unwrap()
            } else {
                IrEventSpec::Or(specs)
            }
        }
    }
}

fn convert_event_expr(ee: &EventExpr) -> IrEventSpec {
    match ee {
        EventExpr::Posedge(e) => IrEventSpec::Posedge(lower_expr(e, &mut LowerCtx::new())),
        EventExpr::Negedge(e) => IrEventSpec::Negedge(lower_expr(e, &mut LowerCtx::new())),
        EventExpr::Expr(e) => IrEventSpec::Change(lower_expr(e, &mut LowerCtx::new())),
        EventExpr::Ident(p) => IrEventSpec::Named(path_to_string(p)),
        EventExpr::AnalogEventFn(e) => {
            // cross(...), above(...), timer(...) — extract from the call
            if let Expr::Call(FunctionRef::Path(p), args) = e {
                let name = path_leaf_str(p).unwrap_or_default();
                let arg0 = args.first().map(|a| lower_expr(a.expr(), &mut LowerCtx::new())).unwrap_or(IrExpr::Real(0.0));
                match name.as_str() {
                    "cross" => {
                        let dir = args.get(1)
                            .and_then(|a| eval_const_int_expr(a.expr()))
                            .unwrap_or(0) as i8;
                        IrEventSpec::Cross(arg0, dir)
                    }
                    "above" => IrEventSpec::Above(arg0),
                    "timer" => IrEventSpec::Timer(arg0),
                    _ => IrEventSpec::Named(name),
                }
            } else {
                IrEventSpec::Named("?".into())
            }
        }
        EventExpr::DriverUpdate(e) => {
            IrEventSpec::Named(format!("driver_update({})", lower_expr(e, &mut LowerCtx::new())))
        }
        EventExpr::Or(a, b) => {
            let sa = convert_event_expr(a);
            let sb = convert_event_expr(b);
            IrEventSpec::Or(vec![sa, sb])
        }
    }
}

fn lower_expr_stmt(expr: &Expr, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    if let Expr::Call(FunctionRef::SysFun(name), args) = expr {
        match name.as_ref() {
            "$bound_step" => {
                let e = args.first()
                    .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                    .unwrap_or(IrExpr::Real(0.0));
                return vec![IrStmt::BoundStep(e)];
            }
            "$finish" | "$stop" => return vec![IrStmt::Finish],
            "$discontinuity" => {
                let n = args.first()
                    .and_then(|a: &CallArg| eval_const_int_expr(a.expr()))
                    .unwrap_or(0) as i32;
                return vec![IrStmt::Discontinuity(n)];
            }
            n if is_display_task(n) => {
                let severity = match n {
                    "$warning" => Severity::Warning,
                    "$error" => Severity::Error,
                    "$fatal" => Severity::Fatal,
                    _ => Severity::Info,
                };
                let ir_args: Vec<IrExpr> = args.iter().skip(1)
                    .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                    .collect();
                let fmt = args.first()
                    .and_then(|a: &CallArg| str_lit(a.expr()))
                    .unwrap_or_default();
                return vec![IrStmt::Diagnostic { severity, format: fmt, args: ir_args }];
            }
            _ => {}
        }
    }
    vec![]
}

fn is_display_task(name: &str) -> bool {
    matches!(name, "$display" | "$write" | "$strobe" | "$monitor" | "$warning" | "$error" | "$fatal" | "$info")
}

fn str_lit(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::StrLit(s)) => Some(s.trim_matches('"').to_string()),
        _ => None,
    }
}

// ─── Event kind inference ──────────────────────────────────────────────────────

fn infer_event_kind(event: &Expr) -> IrEventKind {
    match event {
        Expr::Path(p) => {
            match path_leaf_str(p).as_deref() {
                Some("initial_step") => IrEventKind::InitialStep,
                Some("final_step") => IrEventKind::FinalStep,
                _ => IrEventKind::InitialStep,
            }
        }
        Expr::Call(FunctionRef::Path(p), args) => {
            let name = path_leaf_str(p).unwrap_or_default();
            let arg0 = args.first().map(|a| lower_expr(a.expr(), &mut LowerCtx::new()));
            match name.as_str() {
                "initial_step" => IrEventKind::InitialStep,
                "final_step" => IrEventKind::FinalStep,
                "cross" => {
                    let dir = args.get(1)
                        .and_then(|a| eval_const_int_expr(a.expr()))
                        .unwrap_or(0) as i8;
                    IrEventKind::Cross { dir, expr: arg0 }
                }
                "above" => IrEventKind::Above { expr: arg0 },
                "timer" => {
                    let period = args.first()
                        .map(|a| lower_expr(a.expr(), &mut LowerCtx::new()));
                    IrEventKind::Timer { period }
                }
                _ => IrEventKind::InitialStep,
            }
        }
        _ => IrEventKind::InitialStep,
    }
}

// ─── Contribution dest parsing ───────────────────────────────────────────────

fn parse_lval_contrib(lval: &Expr) -> (IrNature, String, String) {
    if let Expr::Call(FunctionRef::Path(p), args) = lval {
        let name = path_leaf_str(p).unwrap_or_default();
        let nature = access_to_nature(&name);
        let plus = positional_path_leaf(args.first()).unwrap_or_else(|| "?".into());
        let minus = positional_path_leaf(args.get(1)).unwrap_or_else(|| "0".into());
        return (nature, plus, minus);
    }
    (IrNature::Flow("I".into()), "?".into(), "?".into())
}

/// Map an access function name to its nature (potential or flow).
/// "V" → Potential, "I" → Flow. Custom access functions default to Flow
/// (current-like) since flow contributions are the common case in analog
/// modeling. The codegen can refine this with a nature table lookup.
fn access_to_nature(name: &str) -> IrNature {
    match name {
        "V" => IrNature::Potential("V".into()),
        "I" => IrNature::Flow("I".into()),
        _ => IrNature::Flow(name.into()),
    }
}

fn positional_path_leaf(arg: Option<&CallArg>) -> Option<String> {
    let arg = arg?;
    if let CallArg::Positional(Expr::Path(p)) = arg {
        path_leaf_str(p)
    } else {
        None
    }
}

// ─── Noise extraction ──────────────────────────────────────────────────────────

fn scan_noise(expr: &Expr, plus: &str, minus: &str, ctx: &mut LowerCtx) {
    match expr {
        Expr::Call(FunctionRef::Path(p), args) => {
            match path_leaf_str(p).as_deref() {
                Some("white_noise") => {
                    let psd = args.first()
                        .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                        .unwrap_or(IrExpr::Real(0.0));
                    let label = args.get(1).and_then(|a: &CallArg| str_lit(a.expr()));
                    ctx.noise_sources.push(IrNoiseSource {
                        plus: plus.into(),
                        minus: minus.into(),
                        kind: IrNoise::White { psd },
                        label,
                    });
                }
                Some("flicker_noise") => {
                    let psd = args.first()
                        .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                        .unwrap_or(IrExpr::Real(0.0));
                    let exponent = args.get(1)
                        .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                        .unwrap_or(IrExpr::Real(1.0));
                    let label = args.get(2).and_then(|a: &CallArg| str_lit(a.expr()));
                    ctx.noise_sources.push(IrNoiseSource {
                        plus: plus.into(),
                        minus: minus.into(),
                        kind: IrNoise::Flicker { psd, exponent },
                        label,
                    });
                }
                _ => {
                    for arg in args.iter() {
                        scan_noise(arg.expr(), plus, minus, ctx);
                    }
                }
            }
        }
        Expr::Binary(l, _, r) => {
            scan_noise(l, plus, minus, ctx);
            scan_noise(r, plus, minus, ctx);
        }
        Expr::Paren(inner) => scan_noise(inner, plus, minus, ctx),
        Expr::Prefix(_, inner) => scan_noise(inner, plus, minus, ctx),
        Expr::Select(c, t, e) => {
            scan_noise(c, plus, minus, ctx);
            scan_noise(t, plus, minus, ctx);
            scan_noise(e, plus, minus, ctx);
        }
        _ => {}
    }
}

// ─── Expression lowering ──────────────────────────────────────────────────────

fn lower_expr(expr: &Expr, ctx: &mut LowerCtx) -> IrExpr {
    match expr {
        Expr::Literal(lit) => lower_literal(lit),

        Expr::Paren(inner) => lower_expr(inner, ctx),

        Expr::Path(p) => {
            let name = path_to_string(p);
            if let Some(val) = ctx.env.get(&name) {
                val.clone()
            } else {
                IrExpr::Param(name)
            }
        }

        Expr::PortFlow(p) => IrExpr::PortFlow(path_to_string(p)),

        Expr::Prefix(op, inner) => {
            let e = Box::new(lower_expr(inner, ctx));
            match op {
                PrefixOp::Neg => IrExpr::Unary(IrUnOp::Neg, e),
                PrefixOp::Not => IrExpr::Unary(IrUnOp::Not, e),
                PrefixOp::BitNot => IrExpr::Unary(IrUnOp::BitNot, e),
                PrefixOp::Pos => *e,
                PrefixOp::ReduceAnd => IrExpr::Unary(IrUnOp::RedAnd, e),
                PrefixOp::ReduceNand => IrExpr::Unary(IrUnOp::RedNand, e),
                PrefixOp::ReduceOr => IrExpr::Unary(IrUnOp::RedOr, e),
                PrefixOp::ReduceNor => IrExpr::Unary(IrUnOp::RedNor, e),
                PrefixOp::ReduceXor => IrExpr::Unary(IrUnOp::RedXor, e),
                PrefixOp::ReduceXnor1 | PrefixOp::ReduceXnor2 => IrExpr::Unary(IrUnOp::RedXnor, e),
            }
        }

        Expr::Binary(l, op, r) => {
            let lir = Box::new(lower_expr(l, ctx));
            let rir = Box::new(lower_expr(r, ctx));
            IrExpr::Binary(lower_binop(op), lir, rir)
        }

        Expr::Select(c, t, e) => {
            IrExpr::Select(
                Box::new(lower_expr(c, ctx)),
                Box::new(lower_expr(t, ctx)),
                Box::new(lower_expr(e, ctx)),
            )
        }

        Expr::Call(func_ref, args) => lower_call(func_ref, args, ctx),

        Expr::Index(base, idx) => {
            IrExpr::Index(
                Box::new(lower_expr(base, ctx)),
                Box::new(lower_expr(idx, ctx)),
            )
        }

        Expr::PartSelect(base, msb, lsb) => {
            IrExpr::PartSelect(
                Box::new(lower_expr(base, ctx)),
                Box::new(lower_expr(msb, ctx)),
                Box::new(lower_expr(lsb, ctx)),
            )
        }

        Expr::PartSelectUp(base, idx, width) => {
            IrExpr::PartSelectIndexed {
                base: Box::new(lower_expr(base, ctx)),
                idx: Box::new(lower_expr(idx, ctx)),
                width: Box::new(lower_expr(width, ctx)),
                up: true,
            }
        }

        Expr::PartSelectDown(base, idx, width) => {
            IrExpr::PartSelectIndexed {
                base: Box::new(lower_expr(base, ctx)),
                idx: Box::new(lower_expr(idx, ctx)),
                width: Box::new(lower_expr(width, ctx)),
                up: false,
            }
        }

        Expr::Array(exprs) => {
            IrExpr::Array(exprs.iter().map(|e| lower_expr(e, ctx)).collect())
        }

        Expr::Concat(exprs) => {
            IrExpr::Concat(exprs.iter().map(|e| lower_expr(e, ctx)).collect())
        }

        Expr::Replicate(count, exprs) => {
            IrExpr::Replicate(
                Box::new(lower_expr(count, ctx)),
                exprs.iter().map(|e| lower_expr(e, ctx)).collect(),
            )
        }

        Expr::Mintypmax(min, typ, max) => {
            IrExpr::Mintypmax(
                Box::new(lower_expr(min, ctx)),
                Box::new(lower_expr(typ, ctx)),
                Box::new(lower_expr(max, ctx)),
            )
        }
    }
}

fn lower_literal(lit: &Literal) -> IrExpr {
    match lit {
        Literal::IntNumber(s) => IrExpr::Int(s.parse::<i64>().unwrap_or(0)),
        Literal::StdRealNumber(s) => IrExpr::Real(s.parse::<f64>().unwrap_or(0.0)),
        Literal::SiRealNumber(s) => IrExpr::Real(parse_si(s)),
        Literal::StrLit(s) => IrExpr::String(s.trim_matches('"').to_string()),
        Literal::Inf => IrExpr::Real(f64::INFINITY),
        Literal::SizedLit(s) => IrExpr::Int(parse_sized_lit(s).unwrap_or_else(|e| {
            // GAPS §A.11 — fail loud on 4-state digits. Today we'd panic;
            // a proper Result-based propagate is tracked as a follow-up.
            // Until then, propagating through `lower_expr` is a wider
            // refactor; the panic gives users a clear error rather than
            // the silent zero that used to ship.
            panic!("ams_to_ir: {e}");
        })),
    }
}

fn lower_call(func_ref: &FunctionRef, args: &[CallArg], ctx: &mut LowerCtx) -> IrExpr {
    match func_ref {
        FunctionRef::SysFun(name) => lower_sysfun(name, args, ctx),
        FunctionRef::Path(p) => lower_path_call(p, args, ctx),
    }
}

fn lower_sysfun(name: &str, args: &[CallArg], ctx: &mut LowerCtx) -> IrExpr {
    match name {
        "$temperature" => IrExpr::Sim(SimQuery::Temperature),
        "$vt" => {
            if args.is_empty() {
                IrExpr::Sim(SimQuery::Vt(None))
            } else {
                IrExpr::Sim(SimQuery::Vt(Some(Box::new(lower_expr(args[0].expr(), ctx)))))
            }
        }
        "$abstime" => IrExpr::Sim(SimQuery::Abstime),
        "$mfactor" => IrExpr::Sim(SimQuery::Mfactor),
        "$xposition" => IrExpr::Sim(SimQuery::XPosition),
        "$yposition" => IrExpr::Sim(SimQuery::YPosition),
        "$angle" => IrExpr::Sim(SimQuery::Angle),
        "$simparam" => {
            let key = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            let default = args.get(1)
                .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                .unwrap_or(IrExpr::Real(0.0));
            IrExpr::Sim(SimQuery::Simparam { key, default: Box::new(default) })
        }
        "$param_given" => {
            let name = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            IrExpr::Sim(SimQuery::ParamGiven(name))
        }
        "$port_connected" => {
            let name = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            IrExpr::Sim(SimQuery::PortConnected(name))
        }
        "$limit" => {
            let kind = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            let limit_args = args.iter().skip(1)
                .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                .collect();
            IrExpr::Sim(SimQuery::Limit { kind, args: limit_args })
        }
        "$random" => {
            IrExpr::Sim(SimQuery::Random { kind: "random".into(), args: vec![] })
        }
        n if n.starts_with("$dist_") => {
            let kind = n.trim_start_matches('$').to_string();
            let dist_args = args.iter()
                .map(|a: &CallArg| lower_expr(a.expr(), ctx))
                .collect();
            IrExpr::Sim(SimQuery::Random { kind, args: dist_args })
        }
        _ => IrExpr::Real(0.0),
    }
}

fn lower_analysis_call(args: &[CallArg]) -> IrExpr {
    let kind = args.first()
        .and_then(|a: &CallArg| str_lit(a.expr()))
        .unwrap_or_else(|| "dc".into());
    IrExpr::Sim(SimQuery::Analysis(kind))
}

fn lower_path_call(p: &Path, args: &[CallArg], ctx: &mut LowerCtx) -> IrExpr {
    let name = path_leaf_str(p).unwrap_or_default();
    let positional: Vec<IrExpr> = args.iter()
        .filter(|a: &&CallArg| a.is_positional())
        .map(|a: &CallArg| lower_expr(a.expr(), ctx))
        .collect();

    match name.as_str() {
        "V" | "I" => {
            let plus = positional_path_leaf(args.first()).unwrap_or_else(|| "?".into());
            let minus = positional_path_leaf(args.get(1)).unwrap_or_else(|| "0".into());
            IrExpr::BranchAccess { access: name, plus, minus }
        }
        "ddt" => {
            let arg = positional.into_iter().next().unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Ddt, arg);
            IrExpr::StateRef(id)
        }
        "idt" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let ic = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Idt { ic }, arg);
            IrExpr::StateRef(id)
        }
        "idtmod" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let ic = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let modulus = positional.get(2).cloned().unwrap_or(IrExpr::Real(1.0));
            let id = ctx.alloc_state(IrStateKind::IdtMod { ic, modulus }, arg);
            IrExpr::StateRef(id)
        }
        "ddx" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let node = positional_path_leaf(args.get(1)).unwrap_or_else(|| "?".into());
            let id = ctx.alloc_state(IrStateKind::Ddx { node }, arg);
            IrExpr::StateRef(id)
        }
        "delay" | "absdelay" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let delay = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Delay { delay }, arg);
            IrExpr::StateRef(id)
        }
        "transition" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let delay = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let rise = positional.get(2).cloned().unwrap_or(IrExpr::Real(0.0));
            let fall = positional.get(3).cloned().unwrap_or(IrExpr::Real(0.0));
            let tol = positional.get(4).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(
                IrStateKind::Transition { delay, rise, fall, tol },
                arg,
            );
            IrExpr::StateRef(id)
        }
        "slew" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let rise = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let fall = positional.get(2).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = ctx.alloc_state(IrStateKind::Slew { rise, fall }, arg);
            IrExpr::StateRef(id)
        }
        "laplace_np" | "laplace_zp" | "laplace_pm" | "laplace_nm" | "laplace_npm" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let num = positional.get(1).cloned().unwrap_or(IrExpr::Array(vec![]));
            let den = positional.get(2).cloned().unwrap_or(IrExpr::Array(vec![]));
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
        "zi_zd" | "zi_zp" | "zi_nd" | "zi_np" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let num = positional.get(1).cloned().unwrap_or(IrExpr::Array(vec![]));
            let den = positional.get(2).cloned().unwrap_or(IrExpr::Array(vec![]));
            let sample_dt = positional.get(3).cloned().unwrap_or(IrExpr::Real(0.0));
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
            let mag = positional.get(0).cloned().unwrap_or(IrExpr::Real(1.0));
            let phase = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            IrExpr::AcStim { mag: Box::new(mag), phase: Box::new(phase) }
        }
        "white_noise" | "flicker_noise" => IrExpr::Real(0.0),
        "analysis" => lower_analysis_call(args),
        _ => IrExpr::Call(name.to_string(), positional),
    }
}

fn lower_binop(op: &BinOp) -> IrBinOp {
    match op {
        BinOp::Add => IrBinOp::Add,
        BinOp::Sub => IrBinOp::Sub,
        BinOp::Mul => IrBinOp::Mul,
        BinOp::Div => IrBinOp::Div,
        BinOp::Mod => IrBinOp::Rem,
        BinOp::Pow => IrBinOp::Pow,
        BinOp::Eq | BinOp::CaseEq => IrBinOp::Eq,
        BinOp::Neq | BinOp::CaseNeq => IrBinOp::Ne,
        BinOp::Lt => IrBinOp::Lt,
        BinOp::Le => IrBinOp::Le,
        BinOp::Gt => IrBinOp::Gt,
        BinOp::Ge => IrBinOp::Ge,
        BinOp::AndAnd => IrBinOp::And,
        BinOp::OrOr => IrBinOp::Or,
        BinOp::BitAnd => IrBinOp::BitAnd,
        BinOp::BitOr => IrBinOp::BitOr,
        BinOp::Xor | BinOp::XNor1 | BinOp::XNor2 => IrBinOp::BitXor,
        BinOp::Shl | BinOp::ArithShl => IrBinOp::Shl,
        BinOp::Shr | BinOp::ArithShr => IrBinOp::Shr,
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

fn path_to_string(p: &Path) -> String {
    let mut parts = vec![];
    collect_path_parts(p, &mut parts);
    parts.join(".")
}

fn collect_path_parts(p: &Path, out: &mut Vec<String>) {
    if let Some(q) = &p.qualifier {
        collect_path_parts(q, out);
    }
    if let PathSegment::Ident(s) = &p.segment {
        out.push(s.clone());
    }
}

fn path_leaf_str(p: &Path) -> Option<String> {
    match &p.segment {
        PathSegment::Ident(s) => Some(s.clone()),
        PathSegment::Root => None,
    }
}

fn path_leaf_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) => path_leaf_str(p),
        _ => None,
    }
}

// ─── SI suffix parsing ────────────────────────────────────────────────────────

fn parse_si(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() {
        return 0.0;
    }
    let last = s.chars().last().unwrap_or('0');
    let (mantissa, scale) = match last {
        'T' => (&s[..s.len()-1], 1e12),
        'G' => (&s[..s.len()-1], 1e9),
        'M' => (&s[..s.len()-1], 1e6),
        'K' | 'k' => (&s[..s.len()-1], 1e3),
        'm' => (&s[..s.len()-1], 1e-3),
        'u' => (&s[..s.len()-1], 1e-6),
        'n' => (&s[..s.len()-1], 1e-9),
        'p' => (&s[..s.len()-1], 1e-12),
        'f' => (&s[..s.len()-1], 1e-15),
        'a' => (&s[..s.len()-1], 1e-18),
        _ => (s, 1.0),
    };
    mantissa.trim().parse::<f64>().unwrap_or(0.0) * scale
}

/// GAPS §A.11 — sized literals with `x`/`X`/`z`/`Z`/`?` digits used to
/// silently become 0 (because `i64::from_str_radix` rejects those
/// characters). This helper now returns an error so the caller can
/// surface a clear "4-state literal" message instead of silently
/// producing 0.
fn parse_sized_lit(s: &str) -> Result<i64, String> {
    if let Some(pos) = s.find('\'') {
        let rest = &s[pos+1..];
        // Detect 4-state digits before passing to from_str_radix.
        if has_4state_digit(rest) {
            return Err(format!(
                "4-state sized literal `{s}` is not yet supported in IR (GAPS §A.11); \
                 the digits x/X/z/Z/? in a sized literal must be expanded to a \
                 per-bit `Quad` representation first (see AGENTS.md `IrExpr::Quad`)"
            ));
        }
        if let Some(hex) = rest.strip_prefix('h').or_else(|| rest.strip_prefix('H')) {
            return i64::from_str_radix(hex, 16)
                .map_err(|e| format!("invalid hex literal `{s}`: {e}"));
        }
        if let Some(bin) = rest.strip_prefix('b').or_else(|| rest.strip_prefix('B')) {
            return i64::from_str_radix(bin, 2)
                .map_err(|e| format!("invalid binary literal `{s}`: {e}"));
        }
        if let Some(oct) = rest.strip_prefix('o').or_else(|| rest.strip_prefix('O')) {
            return i64::from_str_radix(oct, 8)
                .map_err(|e| format!("invalid octal literal `{s}`: {e}"));
        }
        if let Some(dec) = rest.strip_prefix('d').or_else(|| rest.strip_prefix('D')) {
            return dec.parse::<i64>()
                .map_err(|e| format!("invalid decimal literal `{s}`: {e}"));
        }
        return rest.parse::<i64>()
            .map_err(|e| format!("invalid literal `{s}`: {e}"));
    }
    s.parse::<i64>().map_err(|e| format!("invalid literal `{s}`: {e}"))
}

fn has_4state_digit(s: &str) -> bool {
    s.chars().any(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?'))
}

// ─── Phi-node env merge ───────────────────────────────────────────────────────

fn merge_branch_ctx(
    pre_env: &HashMap<String, IrExpr>,
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

// ─── For-loop unrolling ───────────────────────────────────────────────────────

fn try_unroll_for(f: &ForStmt, ctx: &mut LowerCtx) -> Option<Vec<IrStmt>> {
    let (var_name, start) = extract_int_assign(&f.init)?;
    let (cond_var, limit, inclusive) = extract_int_cmp(&f.condition)?;
    if cond_var != var_name { return None; }
    let (incr_var, step) = extract_int_incr(&f.incr, &var_name)?;
    if incr_var != var_name { return None; }
    if step <= 0 { return None; }

    let end = if inclusive { limit + 1 } else { limit };
    let iter_count = ((end - start) / step).max(0);
    if iter_count > 256 { return None; }

    let mut out = vec![];
    let mut i = start;
    while i < end {
        let mut iter_ctx = ctx.clone();
        iter_ctx.env.insert(var_name.clone(), IrExpr::Int(i));
        out.extend(lower_stmt(&f.for_body, &mut iter_ctx));
        for sv in iter_ctx.state_vars.iter() {
            if !ctx.state_vars.iter().any(|s| s.id == sv.id) {
                ctx.state_vars.push(sv.clone());
            }
        }
        ctx.noise_sources.extend(iter_ctx.noise_sources);
        ctx.counter = ctx.counter.max(iter_ctx.counter);
        i += step;
    }
    Some(out)
}

fn extract_int_assign(stmt: &Stmt) -> Option<(String, i64)> {
    if let Stmt::Assign(a) = stmt {
        if matches!(a.assign.op, AssignOp::Eq) {
            let name = path_leaf_ident(&a.assign.lval)?;
            let val = eval_const_int_expr(&a.assign.rval)?;
            return Some((name, val));
        }
    }
    None
}

fn extract_int_cmp(expr: &Expr) -> Option<(String, i64, bool)> {
    if let Expr::Binary(l, op, r) = expr {
        let var = path_leaf_ident(l)?;
        let limit = eval_const_int_expr(r)?;
        match op {
            BinOp::Lt => return Some((var, limit, false)),
            BinOp::Le => return Some((var, limit, true)),
            _ => {}
        }
    }
    None
}

fn extract_int_incr(stmt: &Stmt, var_name: &str) -> Option<(String, i64)> {
    if let Stmt::Assign(a) = stmt {
        if matches!(a.assign.op, AssignOp::Eq) {
            let name = path_leaf_ident(&a.assign.lval)?;
            if name != var_name { return None; }
            if let Expr::Binary(l, BinOp::Add, r) = &a.assign.rval {
                let lv = path_leaf_ident(l)?;
                if lv == var_name {
                    let step = eval_const_int_expr(r)?;
                    return Some((name, step));
                }
            }
        }
    }
    None
}

fn eval_const_int_expr(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::IntNumber(s)) => s.parse::<i64>().ok(),
        Expr::Literal(Literal::SiRealNumber(s)) => Some(parse_si(s) as i64),
        Expr::Literal(Literal::StdRealNumber(s)) => s.parse::<f64>().ok().map(|f| f as i64),
        Expr::Literal(Literal::SizedLit(s)) => match parse_sized_lit(s) {
            Ok(v) => Some(v),
            // 4-state literals (x/z/?) don't have an integer interpretation;
            // skip them here. The main `ams_to_ir` lowering already
            // errors loudly on the first occurrence (GAPS §A.11).
            Err(_) => None,
        },
        Expr::Paren(inner) => eval_const_int_expr(inner),
        _ => None,
    }
}

// ─── Indirect contribution ────────────────────────────────────────────────────

fn lower_indirect_contrib(ic: &IndirectContribution, ctx: &mut LowerCtx) -> Vec<IrStmt> {
    let (contrib_nature, contrib_plus, contrib_minus) = parse_lval_contrib(&ic.lvalue);
    let (probe_nature, probe_plus, probe_minus) = parse_lval_contrib(&ic.indirect_expr);
    scan_noise(&ic.rvalue, &contrib_plus, &contrib_minus, ctx);
    let expr = lower_expr(&ic.rvalue, ctx);
    vec![IrStmt::IndirectContrib {
        contrib_nature, contrib_plus, contrib_minus,
        probe_nature, probe_plus, probe_minus,
        expr,
    }]
}
