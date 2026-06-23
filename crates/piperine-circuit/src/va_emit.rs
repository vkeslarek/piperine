//! Piperine → standard Verilog-A emitter.
//!
//! Device (VA) modules are compiled by OpenVAF, which parses *standard* Verilog-A
//! — `begin`/`end`, no `{}`, no `++`/compound assignment. Piperine is a VA superset,
//! so before handing a module to OpenVAF we lower our extensions back to standard
//! VA text. The two languages are otherwise the same, so most of this is an
//! identity pretty-print; the transforms that matter:
//!
//! - `{ … }` blocks  → `begin … end`
//! - `x += y` / `x++` (already desugared to `+=`/`-=` in the AST) → `x = x + y`
//! - everything else (contributions `<+`, `V()`/`I()`, `if`/`for`/`while`, params,
//!   branches, math) prints through unchanged.
//!
//! Only modules with an `analog` block and no `initial` block are emitted (the VA
//! device side). The testbench is run by the interpreter and never reaches OpenVAF.

use piperine_parser::ast::{self, Expr, Literal, BinOp, PrefixOp, AssignOp, Stmt, BlockItem, CaseItem};
use piperine_parser::model::{Document, Module, Parameter};

/// Emit a self-contained Verilog-A source for every VA device module in `document`.
pub fn emit_veriloga(document: &Document) -> String {
    let mut out = String::new();
    out.push_str("`include \"disciplines.vams\"\n`include \"constants.vams\"\n\n");
    for m in &document.modules {
        if m.analog_blocks.is_empty() || !m.initial_blocks.is_empty() {
            continue; // not a VA device module
        }
        out.push_str(&emit_module(m));
        out.push('\n');
    }
    out
}

/// True if `document` has at least one VA device module to compile.
pub fn has_va_modules(document: &Document) -> bool {
    document.modules.iter()
        .any(|m| !m.analog_blocks.is_empty() && m.initial_blocks.is_empty())
}

fn emit_module(m: &Module) -> String {
    let mut s = String::new();
    let ports: Vec<&str> = m.ports.iter().map(|p| p.name.as_str()).collect();
    s.push_str(&format!("module {}({});\n", m.name, ports.join(", ")));

    // Port directions, grouped.
    for dir in [ast::Direction::Input, ast::Direction::Output, ast::Direction::Inout] {
        let names: Vec<&str> = m.ports.iter()
            .filter(|p| p.direction == dir)
            .map(|p| p.name.as_str()).collect();
        if !names.is_empty() {
            s.push_str(&format!("  {} {};\n", dir_kw(dir), names.join(", ")));
        }
    }

    // Disciplines, grouped by discipline name (from ports + internal nets).
    let mut by_disc: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |disc: &Option<String>, name: String| {
        if let Some(d) = disc {
            if let Some(entry) = by_disc.iter_mut().find(|(dd, _)| dd == d) {
                entry.1.push(name);
            } else {
                by_disc.push((d.clone(), vec![name]));
            }
        }
    };
    for p in &m.ports { add(&p.discipline, p.name.clone()); }
    for n in &m.nets {
        for mem in &n.members { add(&n.discipline, mem.name.clone()); }
    }
    for (disc, names) in &by_disc {
        s.push_str(&format!("  {} {};\n", disc, names.join(", ")));
    }

    // Parameters.
    for p in &m.parameters {
        s.push_str(&format!("  {}\n", emit_param(p)));
    }

    // Branches.
    for b in &m.branches {
        let ports: Vec<String> = b.ports.iter().map(emit_expr).collect();
        s.push_str(&format!("  branch ({}) {};\n", ports.join(", "), b.names.join(", ")));
    }

    // Analog functions.
    for f in &m.functions {
        s.push_str(&emit_function(f));
    }

    // Analog blocks.
    for ab in &m.analog_blocks {
        let kw = if ab.is_initial { "analog initial " } else { "analog " };
        s.push_str(&format!("  {}{}\n", kw, emit_stmt(&ab.stmt, 1)));
    }

    s.push_str("endmodule\n");
    s
}

fn dir_kw(d: ast::Direction) -> &'static str {
    match d { ast::Direction::Input => "input", ast::Direction::Output => "output", ast::Direction::Inout => "inout" }
}

fn ty_kw(t: &Option<ast::Type>) -> &'static str {
    match t {
        Some(ast::Type::Integer) => "integer",
        Some(ast::Type::String)  => "string",
        _ => "real",
    }
}

fn emit_param(p: &Parameter) -> String {
    let mut s = format!("parameter {} {} = {}", ty_kw(&p.ty), p.name, emit_expr(&p.default_value));
    for c in &p.constraints {
        s.push(' ');
        s.push_str(&emit_constraint(c));
    }
    s.push(';');
    s
}

fn emit_constraint(c: &ast::Constraint) -> String {
    let (kw, v) = match c {
        ast::Constraint::From(v) => ("from", v),
        ast::Constraint::Exclude(v) => ("exclude", v),
    };
    let body = match v {
        ast::ConstraintValue::Range(r) => {
            let l = if r.inclusive_left { '[' } else { '(' };
            let rr = if r.inclusive_right { ']' } else { ')' };
            format!("{l}{}:{}{rr}", emit_expr(&r.start), emit_expr(&r.end))
        }
        ast::ConstraintValue::Expr(e) => format!("[{}]", emit_expr(e)),
        ast::ConstraintValue::Array(es) => {
            let items: Vec<String> = es.iter().map(emit_expr).collect();
            format!("{{{}}}", items.join(", "))
        }
    };
    format!("{kw} {body}")
}

fn emit_function(f: &piperine_parser::model::Function) -> String {
    let mut s = format!("  analog function {} {};\n", ty_kw(&f.return_type), f.name);
    for a in &f.args {
        s.push_str(&format!("    {} {};\n", dir_kw(a.direction), a.name));
        s.push_str(&format!("    real {};\n", a.name)); // simple: treat args as real
    }
    for v in &f.variables {
        s.push_str(&format!("    {} {};\n", ty_kw(&Some(v.ty.clone())), v.name));
    }
    for stmt in &f.body {
        s.push_str(&format!("    {}\n", emit_stmt(stmt, 2)));
    }
    s.push_str("  endfunction\n");
    s
}

// ── statements ────────────────────────────────────────────────────────────────

fn emit_stmt(stmt: &Stmt, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match stmt {
        Stmt::Empty(_) => ";".into(),
        Stmt::Block(b) => {
            let label = b.label.as_ref().map(|l| format!(" : {}", l.0)).unwrap_or_default();
            let mut s = format!("begin{label}\n");
            for item in &b.items {
                s.push_str(&pad);
                s.push_str("  ");
                s.push_str(&emit_block_item(item, indent + 1));
                s.push('\n');
            }
            s.push_str(&pad);
            s.push_str("end");
            s
        }
        Stmt::If(i) => {
            let mut s = format!("if ({}) {}", emit_expr(&i.condition), emit_stmt(&i.then_branch, indent));
            if let Some(e) = &i.else_branch {
                s.push_str(&format!("\n{pad}else {}", emit_stmt(e, indent)));
            }
            s
        }
        Stmt::While(w) => format!("while ({}) {}", emit_expr(&w.condition), emit_stmt(&w.body, indent)),
        Stmt::For(f) => format!(
            "for ({}; {}; {}) {}",
            emit_inline_assign(&f.init), emit_expr(&f.condition),
            emit_inline_assign(&f.incr), emit_stmt(&f.for_body, indent)
        ),
        Stmt::Repeat(r) => format!("repeat ({}) {}", emit_expr(&r.count), emit_stmt(&r.body, indent)),
        // Verilog-A has no `forever`; lower to `while (1)`.
        Stmt::Forever(f) => format!("while (1) {}", emit_stmt(&f.body, indent)),
        Stmt::Case(c) => {
            let mut s = format!("case ({})\n", emit_expr(&c.discriminant));
            for case in &c.cases {
                s.push_str(&pad); s.push_str("  ");
                match &case.item {
                    CaseItem::Default => s.push_str("default"),
                    CaseItem::Exprs(es) => {
                        let labels: Vec<String> = es.iter().map(emit_expr).collect();
                        s.push_str(&labels.join(", "));
                    }
                }
                s.push_str(&format!(": {}\n", emit_stmt(&case.stmt, indent + 1)));
            }
            s.push_str(&pad); s.push_str("endcase");
            s
        }
        Stmt::Event(e) => format!("@({}) {}", emit_expr(&e.event), emit_stmt(&e.stmt, indent)),
        Stmt::Assign(a) => format!("{};", emit_assign(&a.assign)),
        Stmt::Expr(e) => format!("{};", emit_expr(&e.expr)),
        Stmt::Return(r) => match &r.value {
            // Verilog-A analog functions return by assigning the function name; a bare
            // `return expr;` can't be expressed structurally here, so emit a comment.
            Some(e) => format!("/* return */ {};", emit_expr(e)),
            None => "/* return */;".into(),
        },
        // Not expressible in analog VA — emit a harmless comment so compilation
        // surfaces a clear gap rather than mangled output.
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Foreach(_)
        | Stmt::Assert(_) | Stmt::AssertRun(_) | Stmt::AssertWarn(_) =>
            "/* unsupported in analog block */;".into(),
    }
}

fn emit_block_item(item: &BlockItem, indent: usize) -> String {
    match item {
        BlockItem::VarDecl(d) => {
            // `real x, y;` per type
            let names: Vec<String> = d.vars.iter().map(|v| {
                match &v.default {
                    Some(e) => format!("{} = {}", v.name.0, emit_expr(e)),
                    None => v.name.0.clone(),
                }
            }).collect();
            format!("{} {};", ty_kw(&Some(d.ty.clone())), names.join(", "))
        }
        BlockItem::ParamDecl(_) => String::new(), // params hoisted to module level
        BlockItem::Stmt(s) => emit_stmt(s, indent),
    }
}

/// A `for`/init clause assignment, inline (no trailing `;`).
fn emit_inline_assign(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Assign(a) => emit_assign(&a.assign),
        other => emit_stmt(other, 0).trim_end_matches(';').to_string(),
    }
}

/// Lower an assignment to VA. `=` stays `=`, `<+` stays `<+`, compound ops desugar
/// (`x += y` → `x = x + y`). `++`/`--` are already `+=`/`-=` in the AST.
fn emit_assign(a: &ast::Assign) -> String {
    let lhs = emit_expr(&a.lval);
    match &a.op {
        AssignOp::Eq      => format!("{lhs} = {}", emit_expr(&a.rval)),
        AssignOp::Contrib => format!("{lhs} <+ {}", emit_expr(&a.rval)),
        AssignOp::AddEq   => format!("{lhs} = {lhs} + {}", emit_expr(&a.rval)),
        AssignOp::SubEq   => format!("{lhs} = {lhs} - {}", emit_expr(&a.rval)),
        AssignOp::MulEq   => format!("{lhs} = {lhs} * {}", emit_expr(&a.rval)),
        AssignOp::DivEq   => format!("{lhs} = {lhs} / {}", emit_expr(&a.rval)),
        AssignOp::ModEq   => format!("{lhs} = {lhs} % {}", emit_expr(&a.rval)),
    }
}

// ── expressions (near-identity print) ─────────────────────────────────────────

fn emit_expr(e: &Expr) -> String {
    match e {
        Expr::Literal(l) => emit_lit(l),
        Expr::Path(p) => path_str(p),
        Expr::Paren(inner) => format!("({})", emit_expr(inner)),
        Expr::Prefix(op, inner) => format!("{}{}", prefix_str(op), emit_expr(inner)),
        Expr::Binary(l, op, r) => format!("{} {} {}", emit_expr(l), binop_str(op), emit_expr(r)),
        Expr::Select(c, t, e) => format!("{} ? {} : {}", emit_expr(c), emit_expr(t), emit_expr(e)),
        Expr::Index(b, i) => format!("{}[{}]", emit_expr(b), emit_expr(i)),
        Expr::PartSelect(b, m, l) => format!("{}[{}:{}]", emit_expr(b), emit_expr(m), emit_expr(l)),
        Expr::Array(items) => {
            let xs: Vec<String> = items.iter().map(emit_expr).collect();
            format!("'{{{}}}", xs.join(", "))
        }
        Expr::PortFlow(p) => format!("<{}>", path_str(p)),
        Expr::Call(func, args) => {
            let xs: Vec<String> = args.iter().map(|a| emit_expr(a.expr())).collect();
            let name = match func {
                ast::FunctionRef::SysFun(n) => format!("${}", n.trim_start_matches('$')),
                ast::FunctionRef::Path(p) => path_str(p),
            };
            format!("{name}({})", xs.join(", "))
        }
    }
}

fn emit_lit(l: &Literal) -> String {
    match l {
        Literal::IntNumber(s) | Literal::StdRealNumber(s) | Literal::SiRealNumber(s) => s.clone(),
        Literal::StrLit(s) => s.clone(),
        Literal::Inf => "inf".into(),
    }
}

fn path_str(p: &ast::Path) -> String {
    let mut parts = Vec::new();
    let mut cur = p;
    loop {
        match &cur.segment {
            ast::PathSegment::Ident(s) => parts.push(s.clone()),
            ast::PathSegment::Root     => parts.push("root".to_string()),
        }
        match &cur.qualifier {
            Some(q) => cur = q,
            None    => break,
        }
    }
    parts.reverse();
    parts.join(".")
}

fn prefix_str(op: &PrefixOp) -> &'static str {
    match op { PrefixOp::Neg => "-", PrefixOp::Pos => "+", PrefixOp::Not => "!", PrefixOp::BitNot => "~" }
}

fn binop_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*", BinOp::Div => "/",
        BinOp::Mod => "%", BinOp::Pow => "**",
        BinOp::Eq => "==", BinOp::Neq => "!=", BinOp::Lt => "<", BinOp::Le => "<=",
        BinOp::Gt => ">", BinOp::Ge => ">=", BinOp::AndAnd => "&&", BinOp::OrOr => "||",
        BinOp::Shl => "<<", BinOp::Shr => ">>", BinOp::BitAnd => "&", BinOp::BitOr => "|",
        BinOp::Xor => "^", BinOp::XNor1 => "^~", BinOp::XNor2 => "~^",
    }
}
