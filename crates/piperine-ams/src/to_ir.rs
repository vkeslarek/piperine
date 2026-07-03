//! Lower `crate::Document` (Verilog-A/AMS) → `IrProgram`.

use std::collections::HashMap;

use crate::{
    Document,
    ast::{
        AssignOp, BinOp, BlockItem, CallArg, CaseItem, Expr,
        ForStmt, FunctionRef, Literal, ParamAssignment, Path,
        PathSegment, PortConnection, PrefixOp, Stmt,
        Type as AmsType,
    },
};

use piperine_codegen::ir::*;

// ─── Context ──────────────────────────────────────────────────────────────────

struct ModuleCtx<'a> {
    syms: &'a mut SymbolTable,
    nodes: HashMap<String, NodeId>,
    params: HashMap<String, ParamId>,
    vars: HashMap<String, VarId>,
    branches: HashMap<String, (NodeId, NodeId)>,
    fns: HashMap<String, FnId>,
    natures: HashMap<String, NatureId>,
}

impl<'a> ModuleCtx<'a> {
    fn new(syms: &'a mut SymbolTable) -> Self {
        let mut ctx = Self {
            syms,
            nodes: HashMap::new(),
            params: HashMap::new(),
            vars: HashMap::new(),
            branches: HashMap::new(),
            fns: HashMap::new(),
            natures: HashMap::new(),
        };
        ctx.nodes.insert("0".into(), NodeId::GROUND);
        ctx.nodes.insert("gnd".into(), NodeId::GROUND);
        ctx
    }

    fn get_node(&mut self, name: &str) -> NodeId {
        if let Some(&id) = self.nodes.get(name) {
            id
        } else {
            let id = self.syms.add_node(name, Domain::Analog);
            self.nodes.insert(name.into(), id);
            id
        }
    }

    fn get_nature(&mut self, name: &str, kind: NatureKind) -> NatureId {
        if let Some(&id) = self.natures.get(name) {
            id
        } else {
            let id = self.syms.add_nature(name, kind);
            self.natures.insert(name.into(), id);
            id
        }
    }
}

#[derive(Clone)]
struct LowerCtx {
    env: HashMap<String, IrExpr>,
    noise_sources: Vec<IrNoiseSource>,
    states: Vec<StateId>,
}

impl LowerCtx {
    fn new() -> Self {
        Self {
            env: HashMap::new(),
            noise_sources: vec![],
            states: vec![],
        }
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn ams_to_ir(doc: &Document) -> IrProgram {
    let mut param_maps: HashMap<String, HashMap<String, ParamId>> = HashMap::new();
    
    for m in &doc.modules {
        let mut pm = HashMap::new();
        for (i, p) in m.parameters.iter().enumerate() {
            pm.insert(p.name.clone(), ParamId(i as u32));
        }
        param_maps.insert(m.name.clone(), pm);
    }
    
    let modules = doc.modules.iter().map(|m| convert_module(m, &param_maps)).collect();
    IrProgram {
        source: Source::Ams,
        modules,
    }
}

// ─── Module conversion ───────────────────────────────────────────────────────

fn convert_module(m: &crate::Module, param_maps: &HashMap<String, HashMap<String, ParamId>>) -> IrModule {
    let mut symbols = SymbolTable::new();
    let mut module_ctx = ModuleCtx::new(&mut symbols);

    for p in &m.parameters {
        let mut ctx = LowerCtx::new();
        let default = lower_expr(&p.default_value, &mut ctx, &mut module_ctx);
        let id = module_ctx.syms.add_param(&p.name, type_to_ir(p.ty.as_ref()), Some(default));
        module_ctx.params.insert(p.name.clone(), id);
    }

    let ports = m.ports.iter().map(|p| IrPort {
        node: module_ctx.get_node(&p.name),
        direction: match p.direction {
            crate::ast::Direction::Input => IrDirection::In,
            crate::ast::Direction::Output => IrDirection::Out,
            crate::ast::Direction::Inout => IrDirection::Inout,
        },
    }).collect();

    for net in &m.nets {
        for member in &net.members {
            module_ctx.get_node(&member.name);
        }
    }

    for g in &m.ground_decls {
        for name in &g.names {
            module_ctx.nodes.insert(name.name.0.clone(), NodeId::GROUND);
        }
    }

    for v in &m.variables {
        let id = module_ctx.syms.add_var(&v.name, type_to_ir(Some(&v.ty)));
        module_ctx.vars.insert(v.name.clone(), id);
    }

    for br in &m.branches {
        let (plus, minus) = extract_branch_ports(&br.ports, &mut module_ctx);
        for name in &br.names {
            module_ctx.branches.insert(name.clone(), (plus, minus));
        }
    }

    let instances = m.instances.iter().map(|inst| {
        let connections: Vec<NodeId> = inst.connections.iter().filter_map(|c| match c {
            PortConnection::Ordered(Some(e)) | PortConnection::Named { expr: Some(e), .. } => {
                let name = path_leaf_ident(e).unwrap_or_else(|| "?".into());
                Some(module_ctx.get_node(&name))
            }
            _ => None,
        }).collect();
        
        let child_params = param_maps.get(&inst.module_name);
        let params: Vec<(ParamId, IrExpr)> = inst.param_assignments.iter().filter_map(|pa| {
            let mut ctx = LowerCtx::new();
            match pa {
                ParamAssignment::Named { param, expr } => {
                    let pid = child_params.and_then(|m| m.get(&param.0).copied()).unwrap_or(ParamId(0));
                    Some((pid, lower_expr(expr, &mut ctx, &mut module_ctx)))
                }
                ParamAssignment::SystemNamed { param, expr } => {
                    let pid = child_params.and_then(|m| m.get(param).copied()).unwrap_or(ParamId(0));
                    Some((pid, lower_expr(expr, &mut ctx, &mut module_ctx)))
                }
                ParamAssignment::Ordered(expr) => {
                    Some((ParamId(0), lower_expr(expr, &mut ctx, &mut module_ctx)))
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

    for f in &m.functions {
        let func = convert_function(f, &mut module_ctx);
        let id = module_ctx.syms.add_fn(func);
        module_ctx.fns.insert(f.name.clone(), id);
    }

    for t in &m.tasks {
        let func = convert_task(t, &mut module_ctx);
        let id = module_ctx.syms.add_fn(func);
        module_ctx.fns.insert(t.name.clone(), id);
    }

    let mut ctx = LowerCtx::new();
    let mut all_stmts = vec![];
    for block in &m.analog_blocks {
        if block.is_initial {
            let inner = lower_stmt(&block.stmt, &mut ctx, &mut module_ctx);
            all_stmts.push(IrStmt::AnalogEvent(IrAnalogEvent {
                source: EventSource::InitialStep,
                body: inner,
            }));
        } else {
            all_stmts.extend(lower_stmt(&block.stmt, &mut ctx, &mut module_ctx));
        }
    }

    let analog = if all_stmts.is_empty() && ctx.states.is_empty() && ctx.noise_sources.is_empty() {
        None
    } else {
        Some(IrAnalogBody {
            states: ctx.states,
            noise: ctx.noise_sources,
            stmts: all_stmts,
        })
    };

    IrModule {
        name: m.name.clone(),
        symbols,
        ports,
        instances,
        analog,
        digital: None,
    }
}

fn extract_branch_ports(ports: &[Expr], module_ctx: &mut ModuleCtx) -> (NodeId, NodeId) {
    let plus_name = ports.first().and_then(path_leaf_ident).unwrap_or_else(|| "?".into());
    let minus_name = ports.get(1).and_then(path_leaf_ident).unwrap_or_else(|| "0".into());
    let plus = module_ctx.get_node(&plus_name);
    let minus = module_ctx.get_node(&minus_name);
    (plus, minus)
}

fn convert_function(f: &crate::Function, module_ctx: &mut ModuleCtx) -> IrFunction {
    let mut ctx = LowerCtx::new();
    let mut params = vec![];
    for a in &f.args {
        let vid = module_ctx.syms.add_var(&a.name, IrType::Real);
        module_ctx.vars.insert(a.name.clone(), vid);
        params.push(vid);
    }
    for p in &f.parameters {
        let val = lower_expr(&p.default_value, &mut ctx, module_ctx);
        ctx.env.insert(p.name.clone(), val);
    }
    for v in &f.variables {
        let val = v.default_value.as_ref()
            .map(|d| lower_expr(d, &mut ctx, module_ctx))
            .unwrap_or(IrExpr::Real(0.0));
        ctx.env.insert(v.name.clone(), val);
    }
    let mut body = vec![];
    for s in &f.body {
        body.extend(lower_stmt(s, &mut ctx, module_ctx));
    }
    if let Some(val) = ctx.env.get(&f.name) {
        body.push(IrStmt::Return(Some(val.clone())));
    }
    IrFunction { name: f.name.clone(), params, returns: Some(IrType::Real), body }
}

fn convert_task(t: &crate::Task, module_ctx: &mut ModuleCtx) -> IrFunction {
    let mut ctx = LowerCtx::new();
    let mut params = vec![];
    for p in &t.ports {
        for n in &p.names {
            let vid = module_ctx.syms.add_var(&n.0, IrType::Real);
            module_ctx.vars.insert(n.0.clone(), vid);
            params.push(vid);
        }
    }
    for v in &t.variables {
        let val = v.default_value.as_ref()
            .map(|d| lower_expr(d, &mut ctx, module_ctx))
            .unwrap_or(IrExpr::Real(0.0));
        ctx.env.insert(v.name.clone(), val);
    }
    let body = lower_stmt(&t.body, &mut ctx, module_ctx);
    IrFunction { name: t.name.clone(), params, returns: None, body }
}

fn type_to_ir(ty: Option<&AmsType>) -> IrType {
    match ty {
        Some(AmsType::Integer) => IrType::Integer,
        Some(AmsType::Real) => IrType::Real,
        Some(AmsType::String) => IrType::Integer, // Not supported
        Some(AmsType::Time) | Some(AmsType::Realtime) => IrType::Real,
        Some(AmsType::Reg) => IrType::Quad,
        Some(AmsType::Custom(_)) => IrType::Real,
        None => IrType::Real,
    }
}

// ─── Statement lowering ───────────────────────────────────────────────────────

fn lower_stmt(stmt: &Stmt, ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> Vec<IrStmt> {
    match stmt {
        Stmt::Empty(_) => vec![],

        Stmt::Assign(a) => {
            match a.assign.op {
                AssignOp::Contrib => {
                    let (nature, plus, minus) = parse_lval_contrib(&a.assign.lval, module_ctx);
                    scan_noise(&a.assign.rval, plus, minus, ctx, module_ctx);
                    let expr = lower_expr(&a.assign.rval, ctx, module_ctx);
                    let kind = if let Some(id) = first_state_ref(&expr) {
                        ContribKind::Reactive(id)
                    } else {
                        ContribKind::Resistive
                    };
                    vec![IrStmt::Contrib { nature, plus, minus, expr, kind }]
                }
                AssignOp::Eq => {
                    let name = path_leaf_ident(&a.assign.lval).unwrap_or_else(|| "_".into());
                    let val = lower_expr(&a.assign.rval, ctx, module_ctx);
                    ctx.env.insert(name, val);
                    vec![]
                }
            }
        }

        Stmt::If(s) => {
            let cond = lower_expr(&s.condition, ctx, module_ctx);
            let pre_env = ctx.env.clone();
            let mut then_ctx = ctx.clone();
            let then_ = lower_stmt(&s.then_branch, &mut then_ctx, module_ctx);
            let mut else_ctx = ctx.clone();
            let else_ = s.else_branch.as_ref()
                .map(|b| lower_stmt(b, &mut else_ctx, module_ctx))
                .unwrap_or_default();
            merge_branch_ctx(&pre_env, &then_ctx, &else_ctx, &cond, ctx);
            vec![IrStmt::If { cond, then_, else_ }]
        }

        Stmt::Block(b) => {
            let mut out = vec![];
            for item in &b.items {
                match item {
                    BlockItem::VarDecl(vd) => {
                        for var in &vd.vars {
                            let val = var.default.as_ref()
                                .map(|d| lower_expr(d, ctx, module_ctx))
                                .unwrap_or(IrExpr::Real(0.0));
                            ctx.env.insert(var.name.0.clone(), val);
                        }
                    }
                    BlockItem::ParamDecl(pd) => {
                        for p in &pd.params {
                            let val = lower_expr(&p.default, ctx, module_ctx);
                            ctx.env.insert(p.name.0.clone(), val);
                        }
                    }
                    BlockItem::Stmt(s) => {
                        out.extend(lower_stmt(s, ctx, module_ctx));
                    }
                }
            }
            out
        }

        Stmt::Case(c) => {
            let scrutinee = lower_expr(&c.discriminant, ctx, module_ctx);
            let mut arms = vec![];
            let mut default = vec![];
            for case in &c.cases {
                match &case.item {
                    CaseItem::Exprs(exprs) => {
                        let body = lower_stmt(&case.stmt, &mut ctx.clone(), module_ctx);
                        for e in exprs {
                            arms.push((Pattern::Value(lower_expr(e, ctx, module_ctx)), body.clone()));
                        }
                    }
                    CaseItem::Default => {
                        default = lower_stmt(&case.stmt, &mut ctx.clone(), module_ctx);
                    }
                }
            }
            vec![IrStmt::Match { scrutinee, arms, default }]
        }

        Stmt::Event(e) => {
            let source = infer_event_kind(&e.event, ctx, module_ctx);
            let body = lower_stmt(&e.stmt, &mut ctx.clone(), module_ctx);
            vec![IrStmt::AnalogEvent(IrAnalogEvent { source, body })]
        }

        Stmt::Expr(e) => lower_expr_stmt(&e.expr, ctx, module_ctx),

        Stmt::For(f) => {
            try_unroll_for(f, ctx, module_ctx).unwrap_or_else(|| unsupported_stmt("Runtime for-loops"))
        }

        Stmt::While(_) => unsupported_stmt("while loops"),
        Stmt::Repeat(_) => unsupported_stmt("repeat loops"),
        Stmt::Forever(_) => unsupported_stmt("forever loops"),
        Stmt::IndirectContrib(_) => unsupported_stmt("indirect contributions"),
        Stmt::NonBlockingAssign(_) => unsupported_stmt("non-blocking assignments"),
        Stmt::TimingControl(_) => unsupported_stmt("timing control"),
        Stmt::Wait(_) => unsupported_stmt("wait statements"),
        Stmt::Fork(_) => unsupported_stmt("fork blocks"),
        Stmt::Disable(_) => unsupported_stmt("disable statements"),
        Stmt::EventTrigger(_) => unsupported_stmt("event triggers"),
        Stmt::ProceduralAssign(_) => unsupported_stmt("procedural assign"),
        Stmt::ProceduralDeassign(_) => unsupported_stmt("procedural deassign"),
    }
}

fn unsupported_stmt(name: &str) -> Vec<IrStmt> {
    vec![IrStmt::Diagnostic {
        severity: Severity::Fatal,
        format: format!("{} are not supported in IR", name),
        args: vec![],
    }]
}

fn lower_expr_stmt(expr: &Expr, ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> Vec<IrStmt> {
    if let Expr::Call(FunctionRef::SysFun(name), args) = expr {
        match name.as_ref() {
            "$bound_step" => {
                let e = args.first()
                    .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
                    .unwrap_or(IrExpr::Real(0.0));
                return vec![IrStmt::BoundStep(e)];
            }
            "$finish" | "$stop" => return vec![IrStmt::Finish],
            "$discontinuity" => {
                let n = args.first()
                    .and_then(|a: &CallArg| eval_const_int_expr(a.expr()))
                    .unwrap_or(0) as u8;
                return vec![IrStmt::Discontinuity(n)];
            }
            n if is_display_task(n) => {
                let severity = match n {
                    "$warning" => Severity::Warn,
                    "$error" => Severity::Error,
                    "$fatal" => Severity::Fatal,
                    _ => Severity::Info,
                };
                let ir_args: Vec<IrExpr> = args.iter().skip(1)
                    .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
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

fn infer_event_kind(event: &Expr, ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> EventSource {
    match event {
        Expr::Path(p) => {
            match path_leaf_str(p).as_deref() {
                Some("initial_step") => EventSource::InitialStep,
                Some("final_step") => EventSource::FinalStep,
                _ => EventSource::InitialStep,
            }
        }
        Expr::Call(FunctionRef::Path(p), args) => {
            let name = path_leaf_str(p).unwrap_or_default();
            let arg0 = args.first().map(|a| lower_expr(a.expr(), ctx, module_ctx)).unwrap_or(IrExpr::Real(0.0));
            match name.as_str() {
                "initial_step" => EventSource::InitialStep,
                "final_step" => EventSource::FinalStep,
                "cross" => {
                    let dir_val = args.get(1)
                        .and_then(|a| eval_const_int_expr(a.expr()))
                        .unwrap_or(0);
                    let dir = match dir_val {
                        1 => CrossDir::Rising,
                        -1 => CrossDir::Falling,
                        _ => CrossDir::Either,
                    };
                    EventSource::Cross { dir, expr: arg0 }
                }
                "above" => EventSource::Above { expr: arg0 },
                "timer" => {
                    EventSource::Timer { period: arg0 }
                }
                _ => EventSource::InitialStep,
            }
        }
        _ => EventSource::InitialStep,
    }
}

// ─── Contribution dest parsing ───────────────────────────────────────────────

fn parse_lval_contrib(lval: &Expr, module_ctx: &mut ModuleCtx) -> (NatureId, NodeId, NodeId) {
    if let Expr::Call(FunctionRef::Path(p), args) = lval {
        let name = path_leaf_str(p).unwrap_or_default();
        let kind = if name == "V" { NatureKind::Potential } else { NatureKind::Flow };
        let nature = module_ctx.get_nature(&name, kind);
        
        let plus_name = positional_path_leaf(args.first()).unwrap_or_else(|| "?".into());
        let (plus, minus) = if let Some(&(p, m)) = module_ctx.branches.get(&plus_name) {
            (p, m)
        } else {
            let p = module_ctx.get_node(&plus_name);
            let minus_name = positional_path_leaf(args.get(1)).unwrap_or_else(|| "0".into());
            let m = module_ctx.get_node(&minus_name);
            (p, m)
        };
        return (nature, plus, minus);
    }
    let nature = module_ctx.get_nature("I", NatureKind::Flow);
    let p = module_ctx.get_node("?");
    let m = module_ctx.get_node("?");
    (nature, p, m)
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

fn scan_noise(expr: &Expr, plus: NodeId, minus: NodeId, ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) {
    match expr {
        Expr::Call(FunctionRef::Path(p), args) => {
            match path_leaf_str(p).as_deref() {
                Some("white_noise") => {
                    let psd = args.first()
                        .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
                        .unwrap_or(IrExpr::Real(0.0));
                    let label = args.get(1).and_then(|a: &CallArg| str_lit(a.expr()));
                    ctx.noise_sources.push(IrNoiseSource {
                        plus,
                        minus,
                        kind: IrNoise::White { psd },
                        label,
                    });
                }
                Some("flicker_noise") => {
                    let psd = args.first()
                        .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
                        .unwrap_or(IrExpr::Real(0.0));
                    let exponent = args.get(1)
                        .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
                        .unwrap_or(IrExpr::Real(1.0));
                    let label = args.get(2).and_then(|a: &CallArg| str_lit(a.expr()));
                    ctx.noise_sources.push(IrNoiseSource {
                        plus,
                        minus,
                        kind: IrNoise::Flicker { psd, exponent },
                        label,
                    });
                }
                _ => {
                    for arg in args.iter() {
                        scan_noise(arg.expr(), plus, minus, ctx, module_ctx);
                    }
                }
            }
        }
        Expr::Binary(l, _, r) => {
            scan_noise(l, plus, minus, ctx, module_ctx);
            scan_noise(r, plus, minus, ctx, module_ctx);
        }
        Expr::Paren(inner) => scan_noise(inner, plus, minus, ctx, module_ctx),
        Expr::Prefix(_, inner) => scan_noise(inner, plus, minus, ctx, module_ctx),
        Expr::Select(c, t, e) => {
            scan_noise(c, plus, minus, ctx, module_ctx);
            scan_noise(t, plus, minus, ctx, module_ctx);
            scan_noise(e, plus, minus, ctx, module_ctx);
        }
        _ => {}
    }
}

// ─── Expression lowering ──────────────────────────────────────────────────────

fn lower_expr(expr: &Expr, ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> IrExpr {
    match expr {
        Expr::Literal(lit) => lower_literal(lit),

        Expr::Paren(inner) => lower_expr(inner, ctx, module_ctx),

        Expr::Path(p) => {
            let name = path_to_string(p);
            if let Some(val) = ctx.env.get(&name) {
                val.clone()
            } else if let Some(&pid) = module_ctx.params.get(&name) {
                IrExpr::Param(pid)
            } else if let Some(&vid) = module_ctx.vars.get(&name) {
                IrExpr::Var(vid)
            } else {
                IrExpr::Real(0.0)
            }
        }

        Expr::Prefix(op, inner) => {
            let e = Box::new(lower_expr(inner, ctx, module_ctx));
            match op {
                PrefixOp::Neg => IrExpr::Unary(IrUnOp::Neg, e),
                PrefixOp::Not => IrExpr::Unary(IrUnOp::Not, e),
                PrefixOp::BitNot => IrExpr::Unary(IrUnOp::BitNot, e),
                PrefixOp::Pos => *e,
                PrefixOp::ReduceAnd => IrExpr::Unary(IrUnOp::RedAnd, e),
                PrefixOp::ReduceOr => IrExpr::Unary(IrUnOp::RedOr, e),
                PrefixOp::ReduceXor => IrExpr::Unary(IrUnOp::RedXor, e),
                _ => *e,
            }
        }

        Expr::Binary(l, op, r) => {
            let lir = Box::new(lower_expr(l, ctx, module_ctx));
            let rir = Box::new(lower_expr(r, ctx, module_ctx));
            IrExpr::Binary(lower_binop(op), lir, rir)
        }

        Expr::Select(c, t, e) => {
            IrExpr::Select(
                Box::new(lower_expr(c, ctx, module_ctx)),
                Box::new(lower_expr(t, ctx, module_ctx)),
                Box::new(lower_expr(e, ctx, module_ctx)),
            )
        }

        Expr::Call(func_ref, args) => lower_call(func_ref, args, ctx, module_ctx),

        Expr::Index(base, idx) => {
            IrExpr::Index(
                Box::new(lower_expr(base, ctx, module_ctx)),
                Box::new(lower_expr(idx, ctx, module_ctx)),
            )
        }

        Expr::PartSelect(base, msb, lsb) => {
            IrExpr::Slice(
                Box::new(lower_expr(base, ctx, module_ctx)),
                Box::new(lower_expr(lsb, ctx, module_ctx)),
                Box::new(lower_expr(msb, ctx, module_ctx)),
                true,
            )
        }

        Expr::Array(exprs) => {
            IrExpr::Array(exprs.iter().map(|e| lower_expr(e, ctx, module_ctx)).collect())
        }

        _ => IrExpr::Real(0.0),
    }
}

fn lower_literal(lit: &Literal) -> IrExpr {
    match lit {
        Literal::IntNumber(s) => IrExpr::Int(s.parse::<i64>().unwrap_or(0)),
        Literal::StdRealNumber(s) => IrExpr::Real(s.parse::<f64>().unwrap_or(0.0)),
        Literal::SiRealNumber(s) => IrExpr::Real(parse_si(s)),
        Literal::StrLit(_) => IrExpr::Int(0),
        Literal::Inf => IrExpr::Real(f64::INFINITY),
        Literal::SizedLit(s) => IrExpr::Int(parse_sized_lit(s).unwrap_or_else(|e| panic!("ams_to_ir: {e}"))),
    }
}

fn lower_call(func_ref: &FunctionRef, args: &[CallArg], ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> IrExpr {
    match func_ref {
        FunctionRef::SysFun(name) => lower_sysfun(name, args, ctx, module_ctx),
        FunctionRef::Path(p) => lower_path_call(p, args, ctx, module_ctx),
    }
}

fn lower_sysfun(name: &str, args: &[CallArg], ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> IrExpr {
    match name {
        "$temperature" => IrExpr::Sim(SimQuery::Temperature),
        "$vt" => {
            if args.is_empty() {
                IrExpr::Sim(SimQuery::Vt(None))
            } else {
                IrExpr::Sim(SimQuery::Vt(Some(Box::new(lower_expr(args[0].expr(), ctx, module_ctx)))))
            }
        }
        "$abstime" => IrExpr::Sim(SimQuery::Abstime),
        "$mfactor" => IrExpr::Sim(SimQuery::Mfactor),
        "$xposition" => IrExpr::Sim(SimQuery::Position(Axis::X)),
        "$yposition" => IrExpr::Sim(SimQuery::Position(Axis::Y)),
        "$angle" => IrExpr::Sim(SimQuery::Angle),
        "$simparam" => {
            let key = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            let default = args.get(1)
                .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
                .unwrap_or(IrExpr::Real(0.0));
            IrExpr::Sim(SimQuery::Simparam { key, default: Box::new(default) })
        }
        "$param_given" => {
            let name = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            let pid = module_ctx.params.get(&name).copied().unwrap_or(ParamId(0));
            IrExpr::Sim(SimQuery::ParamGiven(pid))
        }
        "$port_connected" => {
            let name = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            let nid = module_ctx.get_node(&name);
            IrExpr::Sim(SimQuery::PortConnected(nid))
        }
        "$limit" => {
            let kind = args.first()
                .and_then(|a: &CallArg| str_lit(a.expr()))
                .unwrap_or_else(|| "?".into());
            let limit_args = args.iter().skip(1)
                .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
                .collect();
            IrExpr::Sim(SimQuery::Limit { kind, args: limit_args })
        }
        "$random" => {
            IrExpr::Sim(SimQuery::Random { kind: "random".into(), args: vec![] })
        }
        n if n.starts_with("$dist_") => {
            let kind = n.trim_start_matches('$').to_string();
            let dist_args = args.iter()
                .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
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
    let a = match kind.as_str() {
        "ac" => Analysis::Ac,
        "tran" => Analysis::Tran,
        "noise" => Analysis::Noise,
        _ => Analysis::Dc,
    };
    IrExpr::Sim(SimQuery::Analysis(a))
}

fn lower_path_call(p: &Path, args: &[CallArg], ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> IrExpr {
    let name = path_leaf_str(p).unwrap_or_default();
    let positional: Vec<IrExpr> = args.iter()
        .filter(|a: &&CallArg| a.is_positional())
        .map(|a: &CallArg| lower_expr(a.expr(), ctx, module_ctx))
        .collect();

    match name.as_str() {
        "V" | "I" => {
            let kind = if name == "V" { NatureKind::Potential } else { NatureKind::Flow };
            let nature = module_ctx.get_nature(&name, kind);
            
            let first_arg = positional_path_leaf(args.first()).unwrap_or_else(|| "?".into());
            let (plus, minus) = if let Some(&(p, m)) = module_ctx.branches.get(&first_arg) {
                (p, m)
            } else {
                let p = module_ctx.get_node(&first_arg);
                let minus_name = positional_path_leaf(args.get(1)).unwrap_or_else(|| "0".into());
                let m = module_ctx.get_node(&minus_name);
                (p, m)
            };
            IrExpr::Branch { nature, plus, minus }
        }
        "ddt" => {
            let arg = positional.into_iter().next().unwrap_or(IrExpr::Real(0.0));
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::Ddt, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "idt" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let ic = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::Idt { ic }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "idtmod" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let ic = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let modulus = positional.get(2).cloned().unwrap_or(IrExpr::Real(1.0));
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::IdtMod { ic, modulus }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "ddx" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let node_name = positional_path_leaf(args.get(1)).unwrap_or_else(|| "?".into());
            let node = module_ctx.get_node(&node_name);
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::Ddx { node }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "delay" | "absdelay" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let delay = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::Delay { delay }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "transition" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let delay = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let rise = positional.get(2).cloned().unwrap_or(IrExpr::Real(0.0));
            let fall = positional.get(3).cloned().unwrap_or(IrExpr::Real(0.0));
            let tol = positional.get(4).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::Transition { delay, rise, fall, tol }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "slew" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let rise = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            let fall = positional.get(2).cloned().unwrap_or(IrExpr::Real(0.0));
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::Slew { rise, fall }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "laplace_np" | "laplace_zp" | "laplace_pm" | "laplace_nm" | "laplace_npm" | "laplace_nd" | "laplace_zd" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let num = positional.get(1).cloned().unwrap_or(IrExpr::Array(vec![]));
            let den = positional.get(2).cloned().unwrap_or(IrExpr::Array(vec![]));
            let num_vec = match num { IrExpr::Array(v) => v, _ => vec![num] };
            let den_vec = match den { IrExpr::Array(v) => v, _ => vec![den] };
            let variant = match name.trim_start_matches("laplace_") {
                "np" => LaplaceKind::NumPoles,
                "zp" => LaplaceKind::ZerosPoles,
                "zd" => LaplaceKind::ZerosDen,
                _ => LaplaceKind::NumDen,
            };
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::Laplace { variant, num: num_vec, den: den_vec }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "zi_zd" | "zi_zp" | "zi_nd" | "zi_np" => {
            let arg = positional.first().cloned().unwrap_or(IrExpr::Real(0.0));
            let num = positional.get(1).cloned().unwrap_or(IrExpr::Array(vec![]));
            let den = positional.get(2).cloned().unwrap_or(IrExpr::Array(vec![]));
            let sample_dt = positional.get(3).cloned().unwrap_or(IrExpr::Real(0.0));
            let num_vec = match num { IrExpr::Array(v) => v, _ => vec![num] };
            let den_vec = match den { IrExpr::Array(v) => v, _ => vec![den] };
            let variant = match name.trim_start_matches("zi_") {
                "np" => ZKind::NumPoles,
                "zp" => ZKind::ZerosPoles,
                "zd" => ZKind::ZerosDen,
                _ => ZKind::NumDen,
            };
            let id = module_ctx.syms.add_state(IrStateVar { kind: IrStateKind::ZTransform { variant, num: num_vec, den: den_vec, sample_dt }, arg });
            ctx.states.push(id);
            IrExpr::State(id)
        }
        "ac_stim" => {
            let mag = positional.get(0).cloned().unwrap_or(IrExpr::Real(1.0));
            let phase = positional.get(1).cloned().unwrap_or(IrExpr::Real(0.0));
            IrExpr::AcStim { mag: Box::new(mag), phase: Box::new(phase) }
        }
        "white_noise" | "flicker_noise" => IrExpr::Real(0.0),
        "analysis" => lower_analysis_call(args),
        _ => {
            if let Some(&fid) = module_ctx.fns.get(&name) {
                IrExpr::Call(fid, positional)
            } else {
                IrExpr::MathCall(name, positional)
            }
        }
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

fn parse_sized_lit(s: &str) -> Result<i64, String> {
    if let Some(pos) = s.find('\'') {
        let rest = &s[pos+1..];
        if has_4state_digit(rest) {
            return Err(format!("4-state sized literal `{s}` is not yet supported in IR"));
        }
        if let Some(hex) = rest.strip_prefix('h').or_else(|| rest.strip_prefix('H')) {
            return i64::from_str_radix(hex, 16).map_err(|e| format!("invalid hex literal `{s}`: {e}"));
        }
        if let Some(bin) = rest.strip_prefix('b').or_else(|| rest.strip_prefix('B')) {
            return i64::from_str_radix(bin, 2).map_err(|e| format!("invalid binary literal `{s}`: {e}"));
        }
        if let Some(oct) = rest.strip_prefix('o').or_else(|| rest.strip_prefix('O')) {
            return i64::from_str_radix(oct, 8).map_err(|e| format!("invalid octal literal `{s}`: {e}"));
        }
        if let Some(dec) = rest.strip_prefix('d').or_else(|| rest.strip_prefix('D')) {
            return dec.parse::<i64>().map_err(|e| format!("invalid decimal literal `{s}`: {e}"));
        }
        return rest.parse::<i64>().map_err(|e| format!("invalid literal `{s}`: {e}"));
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

    for &sv in then_ctx.states.iter().chain(else_ctx.states.iter()) {
        if !ctx.states.contains(&sv) {
            ctx.states.push(sv);
        }
    }

    ctx.noise_sources.extend(then_ctx.noise_sources.iter().cloned());
    ctx.noise_sources.extend(else_ctx.noise_sources.iter().cloned());
}

// ─── For-loop unrolling ───────────────────────────────────────────────────────

fn try_unroll_for(f: &ForStmt, ctx: &mut LowerCtx, module_ctx: &mut ModuleCtx) -> Option<Vec<IrStmt>> {
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
        out.extend(lower_stmt(&f.for_body, &mut iter_ctx, module_ctx));
        for &sv in iter_ctx.states.iter() {
            if !ctx.states.contains(&sv) {
                ctx.states.push(sv);
            }
        }
        ctx.noise_sources.extend(iter_ctx.noise_sources);
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
        Expr::Literal(Literal::SizedLit(s)) => parse_sized_lit(s).ok(),
        Expr::Paren(inner) => eval_const_int_expr(inner),
        _ => None,
    }
}

fn first_state_ref(expr: &IrExpr) -> Option<StateId> {
    expr.find_state(&|_| true)
}
