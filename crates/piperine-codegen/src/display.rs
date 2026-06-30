//! Pseudo-language printer for the IR — for debugging only.

use std::fmt;
use crate::ir::*;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn indent_str(n: usize) -> String {
    " ".repeat(n * 2)
}

fn ir_type_str(ty: IrType) -> &'static str {
    match ty {
        IrType::Real => "Real",
        IrType::Integer => "Integer",
        IrType::String => "String",
        IrType::Bool => "Bool",
        IrType::Quad => "Quad",
        IrType::Complex => "Complex",
        IrType::Void => "Void",
    }
}

fn format_delay_event(delay: &Option<IrExpr>, event: &Option<IrEventSpec>) -> String {
    let mut s = String::new();
    if let Some(d) = delay {
        s.push_str(&format!("#{d}"));
    }
    if let Some(e) = event {
        s.push_str(&format!("@({e})"));
    }
    s
}

fn write_stmts(
    f: &mut fmt::Formatter<'_>,
    stmts: &[IrStmt],
    indent: usize,
) -> fmt::Result {
    for s in stmts {
        write_stmt(f, s, indent)?;
    }
    Ok(())
}

fn write_stmt(f: &mut fmt::Formatter<'_>, stmt: &IrStmt, indent: usize) -> fmt::Result {
    let pad = indent_str(indent);
    match stmt {
        // ── Contributions ──
        IrStmt::Contrib { nature, plus, minus, expr, kind } => {
            let access = nature.access();
            match kind {
                ContribKind::Resistive => {
                    writeln!(f, "{pad}contrib {access}({plus}, {minus}) += {expr}")?;
                }
                ContribKind::Reactive(id) => {
                    writeln!(f, "{pad}reactive[{id}] {access}({plus}, {minus}) += {expr}")?;
                }
            }
        }
        IrStmt::Force { nature, plus, minus, expr } => {
            let access = nature.access();
            writeln!(f, "{pad}force {access}({plus}, {minus}) = {expr}")?;
        }
        IrStmt::IndirectContrib {
            contrib_nature, contrib_plus, contrib_minus,
            probe_nature, probe_plus, probe_minus,
            expr,
        } => {
            let cn = contrib_nature.access();
            let pn = probe_nature.access();
            writeln!(f, "{pad}indirect {cn}({contrib_plus}, {contrib_minus}) : {pn}({probe_plus}, {probe_minus}) = {expr}")?;
        }

        // ── Control flow ──
        IrStmt::If { cond, then_, else_, label } => {
            if let Some(l) = label {
                writeln!(f, "{pad}{l}: if ({cond}) {{")?;
            } else {
                writeln!(f, "{pad}if ({cond}) {{")?;
            }
            write_stmts(f, then_, indent + 1)?;
            if !else_.is_empty() {
                writeln!(f, "{pad}}} else {{")?;
                write_stmts(f, else_, indent + 1)?;
            }
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::Case { discriminant, arms, default, kind, label } => {
            let kw = match kind {
                CaseKind::Case => "case",
                CaseKind::CaseX => "casex",
                CaseKind::CaseZ => "casez",
            };
            if let Some(l) = label {
                writeln!(f, "{pad}{l}: {kw} ({discriminant}) {{")?;
            } else {
                writeln!(f, "{pad}{kw} ({discriminant}) {{")?;
            }
            for (expr, body) in arms {
                writeln!(f, "{}  {expr}: {{", pad)?;
                write_stmts(f, body, indent + 2)?;
                writeln!(f, "{}  }}", pad)?;
            }
            if !default.is_empty() {
                writeln!(f, "{}  default: {{", pad)?;
                write_stmts(f, default, indent + 2)?;
                writeln!(f, "{}  }}", pad)?;
            }
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::For { var, start, end, step, body } => {
            writeln!(f, "{pad}for ({var} = {start}; {var} < {end}; {var} += {step}) {{")?;
            write_stmts(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::While { cond, body } => {
            writeln!(f, "{pad}while ({cond}) {{")?;
            write_stmts(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::Repeat { count, body } => {
            writeln!(f, "{pad}repeat ({count}) {{")?;
            write_stmts(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::Forever { body } => {
            writeln!(f, "{pad}forever {{")?;
            write_stmts(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::Return(None) => writeln!(f, "{pad}return")?,
        IrStmt::Return(Some(e)) => writeln!(f, "{pad}return {e}")?,

        // ── Declarations ──
        IrStmt::VarDecl { name, ty, init } => {
            let ty_str = ir_type_str(*ty);
            match init {
                Some(e) => writeln!(f, "{pad}var {name}: {ty_str} = {e}")?,
                None => writeln!(f, "{pad}var {name}: {ty_str}")?,
            }
        }

        // ── Digital assignments ──
        IrStmt::NonBlocking { lval, expr, delay, event } => {
            let prefix = format_delay_event(delay, event);
            writeln!(f, "{pad}{lval}{prefix} <= {expr}")?;
        }
        IrStmt::Assign { lval, expr, delay, event } => {
            let prefix = format_delay_event(delay, event);
            writeln!(f, "{pad}{lval}{prefix} = {expr}")?;
        }
        IrStmt::ContinuousAssign { lval, expr, delay } => {
            match delay {
                Some(d) => writeln!(f, "{pad}assign {lval} = {expr} #{d}")?,
                None => writeln!(f, "{pad}assign {lval} = {expr}")?,
            }
        }
        IrStmt::ProcAssign { lval, expr, is_force } => {
            if *is_force {
                writeln!(f, "{pad}force {lval} = {expr}")?;
            } else {
                writeln!(f, "{pad}assign {lval} = {expr}")?;
            }
        }
        IrStmt::ProcDeassign { lval, is_release } => {
            if *is_release {
                writeln!(f, "{pad}release {lval}")?;
            } else {
                writeln!(f, "{pad}deassign {lval}")?;
            }
        }

        // ── Timing & events ──
        IrStmt::Delay { delay, body } => {
            writeln!(f, "{pad}#{delay} {{")?;
            write_stmt(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::EventControl { spec, body } => {
            writeln!(f, "{pad}@({spec}) {{")?;
            write_stmt(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::Wait { cond, body } => {
            writeln!(f, "{pad}wait ({cond}) {{")?;
            write_stmt(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }
        IrStmt::Fork { label, branches, join } => {
            let join_str = match join {
                JoinKind::All => "join",
                JoinKind::Any => "join_any",
                JoinKind::None => "join_none",
            };
            if let Some(l) = label {
                writeln!(f, "{pad}fork {l} {{")?;
            } else {
                writeln!(f, "{pad}fork {{")?;
            }
            for branch in branches {
                writeln!(f, "{}  begin", pad)?;
                write_stmts(f, branch, indent + 2)?;
                writeln!(f, "{}  end", pad)?;
            }
            writeln!(f, "{pad}}} {join_str}")?;
        }
        IrStmt::Disable(name) => writeln!(f, "{pad}disable {name}")?,
        IrStmt::Trigger(name) => writeln!(f, "{pad}->{name}")?,

        // ── Analog events ──
        IrStmt::AnalogEvent { kind, body } => {
            let kind_str = match kind {
                IrEventKind::InitialStep => "initial_step".to_string(),
                IrEventKind::FinalStep => "final_step".to_string(),
                IrEventKind::Cross { dir, expr } => match expr {
                    Some(e) => format!("cross({e}, dir={dir})"),
                    None => format!("cross(dir={dir})"),
                },
                IrEventKind::Above { expr } => match expr {
                    Some(e) => format!("above({e})"),
                    None => "above".to_string(),
                },
                IrEventKind::Timer { period } => match period {
                    Some(p) => format!("timer({p})"),
                    None => "timer".to_string(),
                },
            };
            writeln!(f, "{pad}@ {kind_str} {{")?;
            write_stmts(f, body, indent + 1)?;
            writeln!(f, "{pad}}}")?;
        }

        // ── Simulator control ──
        IrStmt::BoundStep(e) => writeln!(f, "{pad}$bound_step({e})")?,
        IrStmt::Finish => writeln!(f, "{pad}$finish")?,
        IrStmt::Discontinuity(n) => writeln!(f, "{pad}$discontinuity({n})")?,
        IrStmt::Diagnostic { severity, format, args } => {
            let sev = match severity {
                Severity::Info => "$display",
                Severity::Warning => "$warning",
                Severity::Error => "$error",
                Severity::Fatal => "$fatal",
            };
            if args.is_empty() {
                writeln!(f, "{pad}{sev}(\"{format}\")")?;
            } else {
                let args_str = args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(", ");
                writeln!(f, "{pad}{sev}(\"{format}\", {args_str})")?;
            }
        }
    }
    Ok(())
}

// ─── IrEventSpec Display ─────────────────────────────────────────────────────

impl fmt::Display for IrEventSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrEventSpec::Posedge(e) => write!(f, "posedge({e})"),
            IrEventSpec::Negedge(e) => write!(f, "negedge({e})"),
            IrEventSpec::Change(e) => write!(f, "change({e})"),
            IrEventSpec::Cross(e, dir) => write!(f, "cross({e}, {dir})"),
            IrEventSpec::Above(e) => write!(f, "above({e})"),
            IrEventSpec::Initial => write!(f, "initial"),
            IrEventSpec::Final => write!(f, "final"),
            IrEventSpec::Timer(e) => write!(f, "timer({e})"),
            IrEventSpec::Named(n) => write!(f, "{n}"),
            IrEventSpec::Or(specs) => {
                write!(f, "(")?;
                for (i, s) in specs.iter().enumerate() {
                    if i > 0 { write!(f, " | ")?; }
                    write!(f, "{s}")?;
                }
                write!(f, ")")
            }
        }
    }
}

// ─── IrExpr Display ──────────────────────────────────────────────────────────

impl fmt::Display for IrExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrExpr::Real(v) => {
                let abs = v.abs();
                if abs == 0.0 || (abs >= 0.001 && abs < 1e6) {
                    write!(f, "{v}")
                } else {
                    write!(f, "{v:.6e}")
                }
            }
            IrExpr::Int(n) => write!(f, "{n}"),
            IrExpr::String(s) => write!(f, "\"{s}\""),
            IrExpr::Bool(b) => write!(f, "{b}"),
            IrExpr::Quad(q) => match q {
                0 => write!(f, "0q0"),
                1 => write!(f, "0q1"),
                2 => write!(f, "0qX"),
                3 => write!(f, "0qZ"),
                _ => write!(f, "0q{q}"),
            },
            IrExpr::Param(s) => write!(f, "{s}"),
            IrExpr::Var(s) => write!(f, "var:{s}"),
            IrExpr::BranchAccess { access, plus, minus } => {
                write!(f, "{access}({plus}, {minus})")
            }
            IrExpr::StateRef(id) => write!(f, "state[{id}]"),
            IrExpr::Sim(q) => match q {
                SimQuery::Temperature => write!(f, "$temperature"),
                SimQuery::Vt(None) => write!(f, "$vt"),
                SimQuery::Vt(Some(e)) => write!(f, "$vt({e})"),
                SimQuery::Abstime => write!(f, "$abstime"),
                SimQuery::Mfactor => write!(f, "$mfactor"),
                SimQuery::XPosition => write!(f, "$xposition"),
                SimQuery::YPosition => write!(f, "$yposition"),
                SimQuery::Angle => write!(f, "$angle"),
                SimQuery::Simparam { key, default } => {
                    write!(f, "$simparam(\"{key}\", {default})")
                }
                SimQuery::Analysis(kind) => write!(f, "analysis(\"{kind}\")"),
                SimQuery::ParamGiven(name) => write!(f, "$param_given(\"{name}\")"),
                SimQuery::PortConnected(name) => write!(f, "$port_connected(\"{name}\")"),
                SimQuery::Limit { kind, args } => {
                    let args_str = args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(", ");
                    write!(f, "$limit({kind}, {args_str})")
                }
                SimQuery::Random { kind, args } => {
                    let args_str = args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(", ");
                    write!(f, "${kind}({args_str})")
                }
            },
            IrExpr::Call(name, args) => {
                write!(f, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
            IrExpr::Binary(op, l, r) => {
                let op_str = match op {
                    IrBinOp::Add => "+",
                    IrBinOp::Sub => "-",
                    IrBinOp::Mul => "*",
                    IrBinOp::Div => "/",
                    IrBinOp::Rem => "%",
                    IrBinOp::Pow => "**",
                    IrBinOp::Eq => "==",
                    IrBinOp::Ne => "!=",
                    IrBinOp::Lt => "<",
                    IrBinOp::Le => "<=",
                    IrBinOp::Gt => ">",
                    IrBinOp::Ge => ">=",
                    IrBinOp::And => "&&",
                    IrBinOp::Or => "||",
                    IrBinOp::BitAnd => "&",
                    IrBinOp::BitOr => "|",
                    IrBinOp::BitXor => "^",
                    IrBinOp::Shl => "<<",
                    IrBinOp::Shr => ">>",
                    IrBinOp::AShl => "<<<",
                    IrBinOp::AShr => ">>>",
                };
                write!(f, "({l} {op_str} {r})")
            }
            IrExpr::Unary(op, e) => {
                let op_str = match op {
                    IrUnOp::Neg => "-",
                    IrUnOp::Not => "!",
                    IrUnOp::BitNot => "~",
                    IrUnOp::RedAnd => "&",
                    IrUnOp::RedNand => "~&",
                    IrUnOp::RedOr => "|",
                    IrUnOp::RedNor => "~|",
                    IrUnOp::RedXor => "^",
                    IrUnOp::RedXnor => "~^",
                };
                write!(f, "{op_str}({e})")
            }
            IrExpr::Select(c, t, e) => {
                write!(f, "({c} ? {t} : {e})")
            }
            IrExpr::Concat(exprs) => {
                write!(f, "{{")?;
                for (i, e) in exprs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{e}")?;
                }
                write!(f, "}}")
            }
            IrExpr::Replicate(count, exprs) => {
                write!(f, "{{{{{count}}} {{")?;
                for (i, e) in exprs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{e}")?;
                }
                write!(f, "}}}}")
            }
            IrExpr::Array(exprs) => {
                write!(f, "[")?;
                for (i, e) in exprs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{e}")?;
                }
                write!(f, "]")
            }
            IrExpr::ArrayRepeat(v, n) => write!(f, "[{v}; {n}]"),
            IrExpr::Index(b, i) => write!(f, "{b}[{i}]"),
            IrExpr::Slice(b, r) => {
                if r.inclusive {
                    write!(f, "{b}[{}..={}]", r.start, r.end)
                } else {
                    write!(f, "{b}[{}..{}]", r.start, r.end)
                }
            }
            IrExpr::PartSelect(b, msb, lsb) => write!(f, "{b}[{msb}:{lsb}]"),
            IrExpr::PartSelectIndexed { base, idx, width, up } => {
                if *up {
                    write!(f, "{base}[{idx} +: {width}]")
                } else {
                    write!(f, "{base}[{idx} -: {width}]")
                }
            }
            IrExpr::Mintypmax(min, typ, max) => write!(f, "({min}:{typ}:{max})"),
            IrExpr::PortFlow(p) => write!(f, "<{p}>"),
            IrExpr::AcStim { mag, phase } => write!(f, "ac_stim({mag}, {phase})"),
            IrExpr::BundleLit { ty, fields } => {
                write!(f, "{ty} {{ ")?;
                for (i, (name, val)) in fields.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, ".{name} = {val}")?;
                }
                write!(f, " }}")
            }
            IrExpr::Lambda { params, body } => {
                write!(f, "|{}| {body}", params.join(", "))
            }
        }
    }
}

// ─── IrAnalogBody Display ────────────────────────────────────────────────────

impl fmt::Display for IrAnalogBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for sv in &self.state_vars {
            let kind_str = match &sv.kind {
                IrStateKind::Ddt => format!("ddt({})", sv.arg),
                IrStateKind::Idt { ic } => format!("idt({}, ic={})", sv.arg, ic),
                IrStateKind::IdtMod { ic, modulus } => {
                    format!("idtmod({}, ic={}, mod={})", sv.arg, ic, modulus)
                }
                IrStateKind::Ddx { node } => format!("ddx({}, node={})", sv.arg, node),
                IrStateKind::Delay { delay } => format!("delay({}, t={})", sv.arg, delay),
                IrStateKind::Transition { delay, rise, fall, tol } => {
                    format!("transition({}, td={}, tr={}, tf={}, tol={})", sv.arg, delay, rise, fall, tol)
                }
                IrStateKind::Slew { rise, fall } => {
                    format!("slew({}, rise={}, fall={})", sv.arg, rise, fall)
                }
                IrStateKind::Laplace { variant, num, den } => {
                    format!("laplace_{variant}({}, num={}, den={})", sv.arg, num, den)
                }
                IrStateKind::ZTransform { variant, num, den, sample_dt } => {
                    format!("zi_{variant}({}, num={}, den={}, dt={})", sv.arg, num, den, sample_dt)
                }
                IrStateKind::Cross { dir } => format!("cross({}, dir={dir})", sv.arg),
                IrStateKind::Timer { period } => format!("timer(period={period})"),
            };
            writeln!(f, "  state[{}] = {kind_str}", sv.id)?;
        }
        if !self.state_vars.is_empty() {
            writeln!(f)?;
        }

        for v in &self.vars {
            let ty = ir_type_str(v.ty);
            match &v.init {
                Some(e) => writeln!(f, "  var {}: {ty} = {e}", v.name)?,
                None => writeln!(f, "  var {}: {ty}", v.name)?,
            }
        }

        writeln!(f, "  analog {{")?;
        write_stmts(f, &self.stmts, 2)?;

        for ns in &self.noise_sources {
            let label_comment = ns.label.as_deref().map(|l| format!("  // \"{l}\"")).unwrap_or_default();
            match &ns.kind {
                IrNoise::White { psd } => {
                    writeln!(f, "    noise white I({}, {}): {psd}{label_comment}", ns.plus, ns.minus)?;
                }
                IrNoise::Flicker { psd, exponent } => {
                    writeln!(f, "    noise flicker I({}, {}): {psd} / f^{exponent}{label_comment}", ns.plus, ns.minus)?;
                }
            }
        }

        writeln!(f, "  }}")
    }
}

// ─── IrDigitalBody Display ───────────────────────────────────────────────────

impl fmt::Display for IrDigitalBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  digital {{")?;
        if !self.inputs.is_empty() {
            writeln!(f, "    inputs: {}", self.inputs.join(", "))?;
        }
        if !self.outputs.is_empty() {
            writeln!(f, "    outputs: {}", self.outputs.join(", "))?;
        }
        write_stmts(f, &self.stmts, 2)?;
        writeln!(f, "  }}")
    }
}

// ─── IrFunction Display ──────────────────────────────────────────────────────

impl fmt::Display for IrFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  fn {}({}) {{", self.name, self.params.join(", "))?;
        write_stmts(f, &self.body, 2)?;
        writeln!(f, "  }}")
    }
}

// ─── IrModule Display ────────────────────────────────────────────────────────

impl fmt::Display for IrModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module {}(", self.name)?;
        if !self.ports.is_empty() {
            writeln!(f)?;
            for p in &self.ports {
                let dir = match p.direction {
                    IrDirection::In => "input",
                    IrDirection::Out => "output",
                    IrDirection::Inout => "inout",
                };
                let disc = p.discipline.as_deref().unwrap_or("Electrical");
                writeln!(f, "  {dir} {disc} {},", p.name)?;
            }
        }
        writeln!(f, ")")?;

        for p in &self.params {
            let ty = match p.ty {
                IrType::Real => "Real",
                IrType::Integer => "Integer",
                IrType::String => "String",
                IrType::Bool => "Bool",
                IrType::Quad => "Quad",
                IrType::Complex => "Complex",
                IrType::Void => "Void",
            };
            match &p.default {
                Some(d) => writeln!(f, "  param {}: {ty} = {d}", p.name)?,
                None => writeln!(f, "  param {}: {ty}", p.name)?,
            }
        }

        for w in &self.wires {
            let disc = w.discipline.as_deref().unwrap_or("Electrical");
            writeln!(f, "  wire {}: {disc}", w.name)?;
        }

        for br in &self.branches {
            writeln!(f, "  branch ({}, {}) {}", br.plus, br.minus, br.name)?;
        }

        for ev in &self.events {
            writeln!(f, "  event {}", ev.name)?;
        }

        for v in &self.vars {
            let ty = ir_type_str(v.ty);
            match &v.init {
                Some(e) => writeln!(f, "  var {}: {ty} = {e}", v.name)?,
                None => writeln!(f, "  var {}: {ty}", v.name)?,
            }
        }

        for g in &self.grounds {
            match &g.discipline {
                Some(d) => writeln!(f, "  ground {}: {d}", g.name)?,
                None => writeln!(f, "  ground {}", g.name)?,
            }
        }

        for inst in &self.instances {
            write!(f, "  {}: {}(", inst.label, inst.module)?;
            for (i, c) in inst.connections.iter().enumerate() {
                if i > 0 { write!(f, ", ")?; }
                match &c.port {
                    Some(port) => write!(f, ".{port}({})", c.net)?,
                    None => write!(f, "{}", c.net)?,
                }
            }
            write!(f, ")")?;
            if !inst.params.is_empty() {
                write!(f, " #(")?;
                for (i, (k, v)) in inst.params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, ".{k} = {v}")?;
                }
                write!(f, ")")?;
            }
            writeln!(f)?;
        }

        for conn in &self.connections {
            writeln!(f, "  connect {} = {}", conn.lhs, conn.rhs)?;
        }

        for ca in &self.continuous_assigns {
            write_stmt(f, ca, 1)?;
        }

        for func in &self.functions {
            write!(f, "{func}")?;
        }

        if let Some(body) = &self.analog {
            writeln!(f)?;
            write!(f, "{body}")?;
        }

        if let Some(body) = &self.digital {
            writeln!(f)?;
            write!(f, "{body}")?;
        }

        Ok(())
    }
}

// ─── IrProgram Display ───────────────────────────────────────────────────────

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "// IR pseudo-language (source: {})", self.source)?;
        for func in &self.functions {
            write!(f, "{func}")?;
        }
        for m in &self.modules {
            writeln!(f)?;
            write!(f, "{m}")?;
        }
        Ok(())
    }
}
