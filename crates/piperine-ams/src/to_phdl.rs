//! Verilog-AMS → PHDL source emitter.
//!
//! Converts a parsed [`Document`] into a PHDL string that the `piperine-lang`
//! parser can ingest.  The mapping is intentionally conservative — constructs
//! that have no PHDL analogue (branches, path-flow port-types, `$display` calls)
//! are either dropped or emitted as comments so the output still parses cleanly.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use crate::ast::{self, AssignOp, Direction};
use crate::model::{Document, Module, Parameter, Variable};

// ─────────────────────────────── public API ──────────────────────────────────

/// Convert a parsed Verilog-AMS document to PHDL source.
pub fn document_to_phdl(doc: &Document) -> String {
    let mut out = String::new();

    // 1. Collect all disciplines referenced in port declarations.
    let discs = collect_disciplines(doc);
    for d in &discs {
        emit_discipline(&mut out, d);
    }
    if !discs.is_empty() {
        out.push('\n');
    }

    // 2. Emit each module (declaration + behaviors).
    let known = known_discipline_names(doc);
    for module in &doc.modules {
        emit_module(&mut out, module, &known);
    }

    out
}

// ──────────────────────────────── disciplines ────────────────────────────────

fn collect_disciplines(doc: &Document) -> BTreeSet<String> {
    let known = known_discipline_names(doc);
    let mut set = BTreeSet::new();
    for m in &doc.modules {
        for p in &m.ports {
            if let Some(d) = &p.discipline {
                set.insert(disc_to_phdl(d));
            }
        }
        for n in &m.nets {
            if let Some(d) = net_discipline(n, &known) {
                set.insert(disc_to_phdl(&d));
            }
        }
    }
    set
}

/// Build a set of known discipline names — from explicit `discipline` declarations
/// in the document plus the standard Verilog-AMS built-ins.
fn known_discipline_names(doc: &Document) -> BTreeSet<String> {
    let builtins = ["electrical", "thermal", "magnetic", "fluidic", "rotational", "translational"];
    let mut names: BTreeSet<String> = builtins.iter().map(|s| s.to_string()).collect();
    for d in &doc.disciplines {
        names.insert(d.name.to_lowercase());
    }
    names
}

/// Extract the effective discipline of a `Net`, handling the common case where
/// `net_decl` in the grammar stores the discipline name as `ty: Type::Custom`
/// rather than `discipline` (because `is_type_kw()` matches `Ident Ident`).
fn net_discipline<'a>(n: &'a crate::model::Net, known: &BTreeSet<String>) -> Option<String> {
    if let Some(d) = &n.discipline {
        return Some(d.clone());
    }
    // Fall back to `ty` when the grammar consumed the discipline name as a
    // custom type.  This happens for `electrical p, n;` in standard VA files.
    if let Some(ast::Type::Custom(name)) = &n.ty {
        if known.contains(&name.0.to_lowercase()) {
            return Some(name.0.clone());
        }
    }
    None
}

/// Capitalise the first letter of a discipline name so `electrical` →
/// `Electrical` (PHDL types are PascalCase).
fn disc_to_phdl(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn emit_discipline(out: &mut String, name: &str) {
    // Hardcode the standard electrical nature mapping.  Non-electrical
    // disciplines keep the same shape; only the nature names differ.
    let (potential, flow) = match name.to_ascii_lowercase().as_str() {
        "electrical" => ("v", "i"),
        "magnetic"   => ("mmf", "flux"),
        "thermal"    => ("temp", "pwr"),
        _            => ("potential", "flow"),
    };
    writeln!(
        out,
        "discipline {} {{ potential {}: Real; flow {}: Real; }}",
        name, potential, flow
    )
    .unwrap();
}

// ───────────────────────────────── modules ───────────────────────────────────

fn emit_module(out: &mut String, m: &Module, known: &BTreeSet<String>) {
    // Build a name→PHDL-discipline map from net declarations (handles the common
    // case where `electrical p, n;` stores "electrical" in `Net.ty` not `Net.discipline`).
    let net_disc_map: HashMap<String, String> = m.nets.iter().flat_map(|n| {
        net_discipline(n, &known).into_iter().flat_map(move |d| {
            n.members.iter().map(move |mem| (mem.name.clone(), disc_to_phdl(&d)))
        })
    }).collect();

    // ── port list ──
    let port_strs: Vec<String> = m.ports.iter().map(|p| {
        let dir = match p.direction {
            Direction::Inout  => "inout",
            Direction::Input  => "input",
            Direction::Output => "output",
        };
        let ty = p.discipline.as_deref()
            .map(disc_to_phdl)
            .or_else(|| net_disc_map.get(&p.name).cloned())
            .unwrap_or_else(|| "Real".to_string());
        format!("{} {}: {}", dir, p.name, ty)
    }).collect();

    write!(out, "mod {}({})", m.name, port_strs.join(", ")).unwrap();

    // ── module body (params, vars, internal wires, structural instances) ──
    let port_names: BTreeSet<&str> = m.ports.iter().map(|p| p.name.as_str()).collect();

    let internal_nets: Vec<_> = m.nets.iter().flat_map(|n| {
        let ty = net_discipline(n, &known).map(|d| disc_to_phdl(&d))
            .unwrap_or_else(|| "Real".to_string());
        n.members.iter().filter(|mem| !port_names.contains(mem.name.as_str()))
            .map(move |mem| (mem.name.clone(), ty.clone()))
    }).collect();

    let has_body = !m.parameters.is_empty()
        || !m.variables.is_empty()
        || !internal_nets.is_empty()
        || !m.instances.is_empty();

    if has_body {
        out.push_str(" {\n");
        for p in &m.parameters {
            emit_param(out, p);
        }
        for v in &m.variables {
            emit_var(out, v);
        }
        for (name, ty) in &internal_nets {
            writeln!(out, "    wire {}: {};", name, ty).unwrap();
        }
        for inst in &m.instances {
            emit_instance(out, inst);
        }
        out.push_str("}\n");
    } else {
        out.push_str(";\n");
    }

    // ── analog behavior ──
    if !m.analog_blocks.is_empty() {
        writeln!(out, "\nanalog {} {{", m.name).unwrap();
        for block in &m.analog_blocks {
            if block.is_initial {
                out.push_str("    @ initial {\n");
                emit_stmt_indented(out, &block.stmt, 2);
                out.push_str("    }\n");
            } else {
                emit_stmt_indented(out, &block.stmt, 1);
            }
        }
        out.push('}');
        out.push('\n');
    }

    out.push('\n');
}

// ──────────────────── params / vars / instances ──────────────────────────────

fn emit_param(out: &mut String, p: &Parameter) {
    let ty = p.ty.as_ref().map(va_type_to_phdl).unwrap_or("Real");
    let default = emit_expr(&p.default_value);
    writeln!(out, "    param {}: {} = {};", p.name, ty, default).unwrap();
}

fn emit_var(out: &mut String, v: &Variable) {
    let ty = va_type_to_phdl(&v.ty);
    if let Some(def) = &v.default_value {
        writeln!(out, "    var {}: {} = {};", v.name, ty, emit_expr(def)).unwrap();
    } else {
        writeln!(out, "    var {}: {};", v.name, ty).unwrap();
    }
}

fn emit_instance(out: &mut String, inst: &crate::model::Instance) {
    // PHDL named instance: `inst_name: ModuleName(port, ...) { .p = v, ... };`
    let ports: Vec<String> = inst.connections.iter().filter_map(|c| match c {
        ast::PortConnection::Ordered(Some(e)) => Some(emit_expr(e)),
        ast::PortConnection::Named { port, expr: Some(e) } => {
            Some(format!(".{} = {}", port.0, emit_expr(e)))
        }
        _ => None,
    }).collect();

    let params: Vec<String> = inst.param_assignments.iter().filter_map(|pa| match pa {
        ast::ParamAssignment::Named { param, expr } =>
            Some(format!(".{} = {}", param.0, emit_expr(expr))),
        ast::ParamAssignment::Ordered(e) => Some(emit_expr(e)),
        ast::ParamAssignment::SystemNamed { .. } => None,
    }).collect();

    write!(out, "    {}: {}({})", inst.instance_name, inst.module_name, ports.join(", ")).unwrap();
    if !params.is_empty() {
        write!(out, " {{ {} }}", params.join(", ")).unwrap();
    }
    out.push_str(";\n");
}

fn va_type_to_phdl(ty: &ast::Type) -> &'static str {
    match ty {
        ast::Type::Real | ast::Type::Realtime => "Real",
        ast::Type::Integer | ast::Type::Time  => "Integer",
        ast::Type::String  => "Str",
        _                  => "Real",
    }
}

// ─────────────────────────────── statements ──────────────────────────────────

fn indent_str(level: usize) -> &'static str {
    match level {
        0 => "",
        1 => "    ",
        2 => "        ",
        3 => "            ",
        _ => "                ",
    }
}

fn emit_stmt_indented(out: &mut String, stmt: &ast::Stmt, indent: usize) {
    let ind = indent_str(indent);
    match stmt {
        ast::Stmt::Empty(_) => {}

        ast::Stmt::Assign(s) => {
            let op_str = match s.assign.op {
                AssignOp::Contrib => "<+",
                AssignOp::Eq      => "=",
            };
            writeln!(
                out, "{}{} {} {};",
                ind,
                emit_expr(&s.assign.lval),
                op_str,
                emit_expr(&s.assign.rval)
            ).unwrap();
        }

        ast::Stmt::Expr(s) => {
            // Skip pure system-task calls ($display, $strobe, $monitor, $finish)
            if let ast::Expr::Call(ast::FunctionRef::SysFun(name), _) = &s.expr {
                if matches!(name.as_str(),
                    "display" | "strobe" | "monitor" | "finish" | "fatal" | "warning" | "error"
                ) {
                    return;
                }
            }
            writeln!(out, "{}{};", ind, emit_expr(&s.expr)).unwrap();
        }

        ast::Stmt::If(s) => {
            writeln!(out, "{}if ({}) {{", ind, emit_expr(&s.condition)).unwrap();
            emit_stmt_indented(out, &s.then_branch, indent + 1);
            if let Some(else_br) = &s.else_branch {
                writeln!(out, "{}}} else {{", ind).unwrap();
                emit_stmt_indented(out, else_br, indent + 1);
            }
            writeln!(out, "{}}}", ind).unwrap();
        }

        ast::Stmt::While(s) => {
            writeln!(out, "{}// while loop omitted", ind).unwrap();
            let _ = &s.condition;
        }

        ast::Stmt::For(s) => {
            // PHDL `for` only supports range-based; emit a comment for complex loops.
            writeln!(out, "{}// for loop omitted", ind).unwrap();
            let _ = &s.init;
        }

        ast::Stmt::Block(s) => {
            for item in &s.items {
                match item {
                    ast::BlockItem::Stmt(inner) => emit_stmt_indented(out, inner, indent),
                    // Variable decls inside blocks are already emitted at module level.
                    ast::BlockItem::VarDecl(_) | ast::BlockItem::ParamDecl(_) => {}
                }
            }
        }

        ast::Stmt::Event(s) => {
            // `@(initial_step)` → `@ initial { ... }`
            // `@(final_step)`   → `@ final { ... }`
            // Other events (cross, above, …) → comment.
            let spec = event_expr_to_spec(&s.event);
            writeln!(out, "{}@ {} {{", ind, spec).unwrap();
            emit_stmt_indented(out, &s.stmt, indent + 1);
            writeln!(out, "{}}}", ind).unwrap();
        }

        // Everything else: emit a comment placeholder.
        _ => {
            writeln!(out, "{}// unsupported statement", ind).unwrap();
        }
    }
}

fn event_expr_to_spec(e: &ast::Expr) -> String {
    match e {
        ast::Expr::Path(p) => {
            let s = path_to_string(p);
            match s.as_str() {
                "initial_step" => "initial".to_string(),
                "final_step"   => "final".to_string(),
                _              => format!("// {}", s),
            }
        }
        ast::Expr::Call(ast::FunctionRef::Path(p), _args) => {
            format!("// {}", path_to_string(p))
        }
        _ => "// event".to_string(),
    }
}

// ──────────────────────────────── expressions ────────────────────────────────

fn emit_expr(e: &ast::Expr) -> String {
    match e {
        ast::Expr::Literal(lit) => emit_literal(lit),

        ast::Expr::Prefix(op, inner) => {
            let op_str = match op {
                ast::PrefixOp::Neg    => "-",
                ast::PrefixOp::Not    => "!",
                ast::PrefixOp::BitNot => "!",
                ast::PrefixOp::Pos    => "",
                _                     => "-",
            };
            format!("{}{}", op_str, emit_expr_paren(inner))
        }

        ast::Expr::Binary(l, op, r) => {
            let op_str = binop_str(op);
            format!("{} {} {}", emit_expr_paren(l), op_str, emit_expr_paren(r))
        }

        ast::Expr::Paren(inner) => format!("({})", emit_expr(inner)),

        ast::Expr::Path(p) => path_to_string(p),
        ast::Expr::PortFlow(p) => path_to_string(p),

        ast::Expr::Call(func_ref, args) => emit_call(func_ref, args),

        ast::Expr::Select(_cond, then_e, _else_e) => {
            // Ternary not in PHDL analog — pick the then-branch as a best-effort.
            format!("({})", emit_expr(then_e))
        }

        ast::Expr::Index(base, idx) => {
            format!("{}[{}]", emit_expr(base), emit_expr(idx))
        }

        _ => "/* unsupported_expr */".to_string(),
    }
}

fn emit_expr_paren(e: &ast::Expr) -> String {
    match e {
        ast::Expr::Binary(..) | ast::Expr::Select(..) => format!("({})", emit_expr(e)),
        _ => emit_expr(e),
    }
}

fn emit_literal(lit: &ast::Literal) -> String {
    match lit {
        ast::Literal::IntNumber(s)    => s.clone(),
        ast::Literal::StdRealNumber(s) => s.clone(),
        ast::Literal::SiRealNumber(s)  => si_to_float_string(s),
        ast::Literal::StrLit(s)        => format!("\"{}\"", s),
        ast::Literal::Inf              => "inf".to_string(),
        ast::Literal::SizedLit(s)      => s.clone(),
    }
}

fn emit_call(func_ref: &ast::FunctionRef, args: &[ast::CallArg]) -> String {
    let name = match func_ref {
        ast::FunctionRef::SysFun(n) => {
            // Strip `$` — math functions keep their name, task-style functions
            // ($display etc.) are already filtered at the statement level.
            n.clone()
        }
        ast::FunctionRef::Path(p) => path_to_string(p),
    };

    let arg_strs: Vec<String> = args.iter().map(|a| emit_expr(a.expr())).collect();
    format!("{}({})", name, arg_strs.join(", "))
}

fn binop_str(op: &ast::BinOp) -> &'static str {
    match op {
        ast::BinOp::Add    => "+",
        ast::BinOp::Sub    => "-",
        ast::BinOp::Mul    => "*",
        ast::BinOp::Div    => "/",
        ast::BinOp::Pow    => "**",
        ast::BinOp::Mod    => "%",
        ast::BinOp::Eq     => "==",
        ast::BinOp::Neq    => "!=",
        ast::BinOp::Lt     => "<",
        ast::BinOp::Le     => "<=",
        ast::BinOp::Gt     => ">",
        ast::BinOp::Ge     => ">=",
        ast::BinOp::OrOr   => "||",
        ast::BinOp::AndAnd => "&&",
        ast::BinOp::BitOr  => "|",
        ast::BinOp::BitAnd => "&",
        ast::BinOp::Xor    => "^",
        ast::BinOp::Shl    => "<<",
        ast::BinOp::Shr    => ">>",
        _                  => "+",
    }
}

// ─────────────────────────── path stringification ────────────────────────────

fn path_to_string(p: &ast::Path) -> String {
    let seg = match &p.segment {
        ast::PathSegment::Ident(s) => s.clone(),
        ast::PathSegment::Root     => String::new(),
    };
    match &p.qualifier {
        None    => seg,
        Some(q) => format!("{}.{}", path_to_string(q), seg),
    }
}

// ────────────────────── SI real number conversion ────────────────────────────

/// Convert a Verilog SI-suffixed real literal (e.g. `1k`, `10f`, `70K`) to a
/// plain floating-point string understood by the PHDL parser.
fn si_to_float_string(s: &str) -> String {
    if s.is_empty() {
        return "0.0".to_string();
    }

    // Last char may be an SI suffix.
    let last = s.chars().last().unwrap();
    let (mantissa_str, scale) = match last {
        'T' => (&s[..s.len()-1], 1e12_f64),
        'G' => (&s[..s.len()-1], 1e9),
        'M' => (&s[..s.len()-1], 1e6),
        'K' | 'k' => (&s[..s.len()-1], 1e3),
        'm' => (&s[..s.len()-1], 1e-3),
        'u' => (&s[..s.len()-1], 1e-6),
        'n' => (&s[..s.len()-1], 1e-9),
        'p' => (&s[..s.len()-1], 1e-12),
        'f' => (&s[..s.len()-1], 1e-15),
        'a' => (&s[..s.len()-1], 1e-18),
        _   => (s, 1.0),
    };

    let mantissa: f64 = mantissa_str.parse().unwrap_or(0.0);
    let value = mantissa * scale;

    // Prefer scientific notation for very small or very large values.
    if value == 0.0 {
        "0.0".to_string()
    } else if value.abs() >= 1e4 || value.abs() < 1e-3 {
        format!("{:e}", value)
    } else {
        format!("{}", value)
    }
}
