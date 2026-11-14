//! Canonical printer: an [`ast::File`] back to MSL source text.
//!
//! The layout is fixed (design §print): 4-space indentation, one item per
//! blank-line-separated block, one statement or declaration per line, and
//! comma-separated lists on one line when they fit in 80 columns and hold no
//! line break, one element per line with a trailing comma otherwise. Doc
//! comments and attributes are kept; plain comments are not part of the
//! tree. Printing then parsing gives back an equal tree (spans aside), and
//! printing that tree gives the same text.
//!
//! A node's text is built bottom-up as a `String`; a multi-line child is
//! indented by its parent.

use super::ast::*;

const WIDTH: usize = 80;

/// Print a file in the canonical layout.
pub fn print(file: &File) -> String {
    let mut out = String::new();
    for d in &file.inner_docs {
        out.push_str(&format!("{}\n", doc(d, "!")));
    }
    if !file.inner_docs.is_empty() && !file.items.is_empty() {
        out.push('\n');
    }
    let items: Vec<String> = file.items.iter().map(item).collect();
    out.push_str(&items.join("\n\n"));
    if !items.is_empty() {
        out.push('\n');
    }
    dec(&out)
}

// ---- verbatim text ----------------------------------------------------------------------------
//
// A string literal or doc comment is printed as written: layout (indentation,
// line fitting) must neither see nor touch its line breaks. While a node's text
// is built, a `\0` marks an escape: `\0n`, `\0r` stand for a line break and a
// carriage return inside such text, `\00` for a NUL; [`print`] restores them.

fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\0' => out.push_str("\00"),
            '\n' => out.push_str("\0n"),
            '\r' => out.push_str("\0r"),
            c => out.push(c),
        }
    }
    out
}

fn dec(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\0' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            _ => out.push('\0'),
        }
    }
    out
}

/// A doc comment in its own form; `kind` is `"/"` (outer) or `"!"` (inner).
fn doc(d: &Doc, kind: &str) -> String {
    let text = enc(&d.text);
    if d.block {
        let star = if kind == "!" { "!" } else { "*" };
        format!("/*{star}{text}*/")
    } else {
        format!("//{kind}{text}")
    }
}

// ---- layout helpers ---------------------------------------------------------------------------

/// Indent every non-blank line of `s` by one level.
fn indent(s: &str) -> String {
    s.lines()
        .map(|l| if l.is_empty() { String::new() } else { format!("    {l}") })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `open items close`: on one line when it fits, one item per line otherwise.
fn list(open: &str, items: &[String], close: &str) -> String {
    let inline = items.join(", ");
    if inline.len() + open.len() + close.len() <= WIDTH && !inline.contains('\n') {
        return format!("{open}{inline}{close}");
    }
    let lines: Vec<String> = items.iter().map(|i| indent(&format!("{i},"))).collect();
    format!("{open}\n{}\n{close}", lines.join("\n"))
}

/// `{ .. }` holding one entry per line, or `{}` when empty.
fn braces(lines: &[String]) -> String {
    if lines.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\n{}\n}}", indent(&lines.join("\n")))
    }
}

fn ident(i: &Ident) -> String {
    i.spelled()
}

fn meta(m: &Meta) -> String {
    let mut out = String::new();
    for d in &m.docs {
        out.push_str(&format!("{}\n", doc(d, "/")));
    }
    for a in &m.attrs {
        out.push_str(&format!("{}\n", attr(a)));
    }
    out
}

fn attr(a: &Attr) -> String {
    let input = match &a.input {
        AttrInput::None => String::new(),
        AttrInput::Eq(e) => format!(" = {}", expr(e)),
        AttrInput::List(args) => {
            let items: Vec<String> = args
                .iter()
                .map(|a| match a {
                    AttrArg::Path(p) => path(p),
                    AttrArg::Lit(l) => lit(l),
                    AttrArg::NameValue(n, l) => format!("{} = {}", n.spelled(), lit(l)),
                })
                .collect();
            format!("({})", items.join(", "))
        }
    };
    format!("#[{}{input}]", path(&a.path))
}

// ---- paths, generics, types -------------------------------------------------------------------

fn path(p: &Path) -> String {
    p.segments
        .iter()
        .map(|s| format!("{}{}", s.ident.spelled(), s.args.as_ref().map_or(String::new(), generic_args)))
        .collect::<Vec<_>>()
        .join("::")
}

fn generic_args(g: &GenericArgs) -> String {
    match g {
        GenericArgs::Angle { args, turbofish, .. } => {
            let items: Vec<String> = args
                .iter()
                .map(|a| match a {
                    GenericArg::Type(t) => ty(t),
                    GenericArg::Const(e) => expr(e),
                })
                .collect();
            // `<-` is the commit operator: a leading minus needs a space.
            let gap = if items.first().is_some_and(|i| i.starts_with('-')) { " " } else { "" };
            format!("{}<{gap}{}>", if *turbofish { "::" } else { "" }, items.join(", "))
        }
        GenericArgs::Paren { inputs, output, .. } => {
            let items: Vec<String> = inputs.iter().map(ty).collect();
            let ret = output.as_ref().map_or(String::new(), |o| format!(" -> {}", ty(o)));
            format!("({}){ret}", items.join(", "))
        }
    }
}

fn generics(g: &Option<Generics>) -> String {
    let Some(g) = g else { return String::new() };
    let items: Vec<String> = g
        .params
        .iter()
        .map(|p| match &p.kind {
            GenericParamKind::Type { bounds, default } => {
                let mut s = p.name.spelled();
                if !bounds.is_empty() {
                    s.push_str(&format!(": {}", bounds.iter().map(path).collect::<Vec<_>>().join(" + ")));
                }
                if let Some(d) = default {
                    s.push_str(&format!(" = {}", ty(d)));
                }
                s
            }
            GenericParamKind::Const { ty: t, default } => {
                let mut s = format!("const {}: {}", p.name.spelled(), ty(t));
                if let Some(d) = default {
                    s.push_str(&format!(" = {}", expr(d)));
                }
                s
            }
        })
        .collect();
    format!("<{}>", items.join(", "))
}

fn ty(t: &Type) -> String {
    match &t.kind {
        TypeKind::Path(p) => path(p),
        TypeKind::Array { elem, len } => format!("[{}; {}]", ty(elem), expr(len)),
        TypeKind::Slice(e) => format!("[{}]", ty(e)),
        TypeKind::Tuple(items) => match items.as_slice() {
            [one] => format!("({},)", ty(one)),
            _ => format!("({})", items.iter().map(ty).collect::<Vec<_>>().join(", ")),
        },
        TypeKind::Ref { static_lifetime, inner } => {
            format!("&{}{}", if *static_lifetime { "'static " } else { "" }, ty(inner))
        }
        TypeKind::Flip(inner) => format!("flip {}", ty(inner)),
        TypeKind::FnPtr { params, ret } => {
            let ret = ret.as_ref().map_or(String::new(), |r| format!(" -> {}", ty(r)));
            format!("fn({}){ret}", params.iter().map(ty).collect::<Vec<_>>().join(", "))
        }
        TypeKind::Infer => "_".to_string(),
    }
}

// ---- patterns ---------------------------------------------------------------------------------

/// A pattern where alternatives are not allowed at the top: parenthesized.
fn pat1(p: &Pat) -> String {
    match p.kind {
        PatKind::Or(_) => format!("({})", pat(p)),
        _ => pat(p),
    }
}

fn pat(p: &Pat) -> String {
    match &p.kind {
        PatKind::Wild => "_".to_string(),
        PatKind::Rest => "..".to_string(),
        PatKind::Ident { by_ref, mutable, name, sub } => {
            let mut s = String::new();
            if *by_ref {
                s.push_str("ref ");
            }
            if *mutable {
                s.push_str("mut ");
            }
            s.push_str(&name.spelled());
            if let Some(sub) = sub {
                s.push_str(&format!(" @ {}", pat1(sub)));
            }
            s
        }
        PatKind::Lit(e) => expr(e),
        PatKind::Path(p) => path(p),
        PatKind::Tuple(items) => match items.as_slice() {
            [one] => format!("({},)", pat(one)),
            _ => format!("({})", items.iter().map(pat).collect::<Vec<_>>().join(", ")),
        },
        PatKind::TupleStruct { path: p, pats } => {
            format!("{}({})", path(p), pats.iter().map(pat).collect::<Vec<_>>().join(", "))
        }
        PatKind::Struct { path: p, fields, rest } => {
            let mut items: Vec<String> = fields
                .iter()
                .map(|f| match &f.pat {
                    Some(q) => format!("{}: {}", f.name.spelled(), pat(q)),
                    None => f.name.spelled(),
                })
                .collect();
            if *rest {
                items.push("..".to_string());
            }
            if items.is_empty() {
                format!("{} {{}}", path(p))
            } else {
                format!("{} {{ {} }}", path(p), items.join(", "))
            }
        }
        PatKind::Or(alts) => alts.iter().map(pat1_alt).collect::<Vec<_>>().join(" | "),
        PatKind::Range { lo, hi, inclusive } => {
            let op = if *inclusive { "..=" } else { ".." };
            format!(
                "{}{op}{}",
                lo.as_ref().map_or(String::new(), |e| expr(e)),
                hi.as_ref().map_or(String::new(), |e| expr(e))
            )
        }
        PatKind::Ref(inner) => format!("&{}", pat1(inner)),
        PatKind::Slice(items) => format!("[{}]", items.iter().map(pat).collect::<Vec<_>>().join(", ")),
    }
}

/// An alternative of an `|` pattern: itself never an alternative.
fn pat1_alt(p: &Pat) -> String {
    pat1(p)
}

// ---- expressions ------------------------------------------------------------------------------

fn lit(l: &Lit) -> String {
    match &l.kind {
        LitKind::Int(s) => s.clone(),
        LitKind::Float { raw, .. } => raw.clone(),
        LitKind::Str(s) => format!("\"{}\"", enc(s)),
        LitKind::Bool(b) => b.to_string(),
        LitKind::Unknown => "Unknown".to_string(),
    }
}

fn binop(op: BinOp) -> &'static str {
    use BinOp::*;
    match op {
        Add => "+", Sub => "-", Mul => "*", Div => "/", Rem => "%", BitXor => "^", BitAnd => "&",
        BitOr => "|", Shl => "<<", Shr => ">>", And => "&&", Or => "||", Eq => "==", Ne => "!=",
        Lt => "<", Le => "<=", Gt => ">", Ge => ">=",
    }
}

fn args(a: &[Arg]) -> Vec<String> {
    a.iter()
        .map(|a| match &a.name {
            Some(n) => format!("{}: {}", n.spelled(), expr(&a.value)),
            None => expr(&a.value),
        })
        .collect()
}

fn field_inits(f: &[FieldInit]) -> String {
    if f.is_empty() {
        return "{}".to_string();
    }
    let items: Vec<String> = f
        .iter()
        .map(|f| match f.shorthand {
            true => f.name.spelled(),
            false => format!("{}: {}", f.name.spelled(), expr(&f.value)),
        })
        .collect();
    let inline = format!("{{ {} }}", items.join(", "));
    if inline.len() <= WIDTH && !inline.contains('\n') {
        inline
    } else {
        let lines: Vec<String> = items.iter().map(|i| indent(&format!("{i},"))).collect();
        format!("{{\n{}\n}}", lines.join("\n"))
    }
}

fn expr(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Lit(l) => lit(l),
        ExprKind::Path(p) => path(p),
        ExprKind::Unary { op, expr: x } => {
            let (sym, space) = match op {
                UnOp::Neg => ("-", false),
                UnOp::Not => ("!", false),
                UnOp::Deref => ("*", false),
                UnOp::Ref => ("&", true),
            };
            let inner = expr(x);
            // `& &x` must not read as `&&`; `- -x` is fine but kept apart too.
            let sep = if (space && inner.starts_with('&')) || (sym == "-" && inner.starts_with('-')) { " " } else { "" };
            format!("{sym}{sep}{inner}")
        }
        ExprKind::Binary { op, lhs, rhs } => format!("{} {} {}", expr(lhs), binop(*op), expr(rhs)),
        ExprKind::Assign { op, lhs, rhs } => {
            let sym = op.map_or("=".to_string(), |o| format!("{}=", binop(o)));
            format!("{} {sym} {}", expr(lhs), expr(rhs))
        }
        ExprKind::Cast { expr: x, ty: t } => format!("{} as {}", expr(x), ty(t)),
        ExprKind::Call { callee, args: a, params } => {
            let mut s = format!("{}{}", expr(callee), list("(", &args(a), ")"));
            if let Some(p) = params {
                s.push(' ');
                s.push_str(&field_inits(p));
            }
            s
        }
        ExprKind::MethodCall { recv, method, generics: g, args: a } => {
            format!(
                "{}.{}{}{}",
                expr(recv),
                method.spelled(),
                g.as_ref().map_or(String::new(), generic_args),
                list("(", &args(a), ")")
            )
        }
        ExprKind::Field { expr: x, name } => format!("{}.{}", expr(x), name.spelled()),
        ExprKind::Index { expr: x, index } => format!("{}[{}]", expr(x), expr(index)),
        ExprKind::Range { lo, hi, inclusive } => format!(
            "{}{}{}",
            lo.as_ref().map_or(String::new(), |x| expr(x)),
            if *inclusive { "..=" } else { ".." },
            hi.as_ref().map_or(String::new(), |x| expr(x))
        ),
        ExprKind::Paren(x) => format!("({})", expr(x)),
        ExprKind::Tuple(items) => match items.as_slice() {
            [one] => format!("({},)", expr(one)),
            _ => list("(", &items.iter().map(expr).collect::<Vec<_>>(), ")"),
        },
        ExprKind::Array(items) => list("[", &items.iter().map(expr).collect::<Vec<_>>(), "]"),
        ExprKind::ArrayRepeat { elem, len } => format!("[{}; {}]", expr(elem), expr(len)),
        ExprKind::Struct { path: p, fields } => format!("{} {}", path(p), field_inits(fields)),
        ExprKind::Block(b) => block(b),
        ExprKind::If { cond, then, else_ } => {
            let mut s = format!("if {} {}", expr(cond), block(then));
            if let Some(x) = else_ {
                s.push_str(&format!(" else {}", expr(x)));
            }
            s
        }
        ExprKind::Let { pat: p, expr: x } => format!("let {} = {}", pat(p), expr(x)),
        ExprKind::Match { scrutinee, arms } => {
            let lines: Vec<String> = arms.iter().map(arm).collect();
            format!("match {} {}", expr(scrutinee), braces(&lines))
        }
        ExprKind::While { cond, body } => format!("while {} {}", expr(cond), block(body)),
        ExprKind::Loop { body } => format!("loop {}", block(body)),
        ExprKind::For { pat: p, iter, body } => {
            format!("for {} in {} {}", pat1(p), expr(iter), block(body))
        }
        ExprKind::Break(v) => v.as_ref().map_or("break".to_string(), |x| format!("break {}", expr(x))),
        ExprKind::Continue => "continue".to_string(),
        ExprKind::Return(v) => v.as_ref().map_or("return".to_string(), |x| format!("return {}", expr(x))),
        ExprKind::Closure { is_move, params, ret, body } => {
            let ps: Vec<String> = params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => format!("{}: {}", pat1(&p.pat), ty(t)),
                    None => pat1(&p.pat),
                })
                .collect();
            let head = if ps.is_empty() { "||".to_string() } else { format!("|{}|", ps.join(", ")) };
            let mv = if *is_move { "move " } else { "" };
            match ret {
                Some(r) => format!("{mv}{head} -> {} {}", ty(r), expr(body)),
                None => format!("{mv}{head} {}", expr(body)),
            }
        }
        ExprKind::Try(x) => format!("{}?", expr(x)),
        ExprKind::Emit { stream, fields } => {
            let mut items = vec![lit(stream)];
            items.extend(fields.iter().map(|f| format!("{} = {}", f.name.spelled(), expr(&f.value))));
            format!("emit!{}", list("(", &items, ")"))
        }
        ExprKind::IncludeStr(l) => format!("include_str!({})", lit(l)),
        ExprKind::Error => "/* error */".to_string(),
    }
}

fn arm(a: &Arm) -> String {
    let mut s = attrs_only(&a.meta);
    s.push_str(&pat(&a.pat));
    if let Some(g) = &a.guard {
        s.push_str(&format!(" if {}", expr(g)));
    }
    s.push_str(&format!(" => {},", expr(&a.body)));
    s
}

/// Documentation and attributes, each on its own line, in front of a node.
fn attrs_only(m: &Meta) -> String {
    meta(m)
}

// ---- blocks and statements --------------------------------------------------------------------

fn block(b: &Block) -> String {
    braces(&b.stmts.iter().map(stmt).collect::<Vec<_>>())
}

fn stmt(s: &Stmt) -> String {
    format!("{}{}", meta(&s.meta), stmt_kind(&s.kind))
}

fn stmt_kind(k: &StmtKind) -> String {
    match k {
        StmtKind::Let { pat: p, ty: t, init, else_ } => {
            let mut out = format!("let {}", pat1(p));
            if let Some(t) = t {
                out.push_str(&format!(": {}", ty(t)));
            }
            if let Some(i) = init {
                out.push_str(&format!(" = {}", expr(i)));
            }
            if let Some(b) = else_ {
                out.push_str(&format!(" else {}", block(b)));
            }
            out.push(';');
            out
        }
        StmtKind::Expr { expr: e, semi } => {
            format!("{}{}", expr(e), if *semi { ";" } else { "" })
        }
        StmtKind::Item(i) => item(i),
        StmtKind::Contribution { lhs, rhs } => format!("{} +> {};", expr(lhs), expr(rhs)),
        StmtKind::Commit { place, value, after } => {
            let mut out = format!("{} <- {}", expr(place), expr(value));
            if let Some(a) = after {
                out.push_str(&format!(" after {}", expr(a)));
            }
            out.push(';');
            out
        }
        StmtKind::Empty => ";".to_string(),
        StmtKind::Signal { name, ty: t, init } => signal(name, t, init),
        StmtKind::Reaction(r) => reaction(r),
    }
}

fn signal(name: &Ident, t: &Type, init: &Option<Expr>) -> String {
    let init = init.as_ref().map_or(String::new(), |e| format!(" = {}", expr(e)));
    format!("signal {}: {}{init};", name.spelled(), ty(t))
}

fn reaction(r: &Reaction) -> String {
    format!("on {} {}", expr(&r.event), block(&r.body))
}

// ---- items ------------------------------------------------------------------------------------

fn vis(public: bool) -> &'static str {
    if public { "pub " } else { "" }
}

fn dir(d: &Option<Dir>) -> &'static str {
    match d {
        Some(Dir::In) => "in ",
        Some(Dir::Out) => "out ",
        None => "",
    }
}

fn item(i: &Item) -> String {
    format!("{}{}{}", meta(&i.meta), vis(i.public), item_kind(&i.kind))
}

fn item_kind(k: &ItemKind) -> String {
    match k {
        ItemKind::Fn(f) => fn_decl(f),
        ItemKind::Struct(s) => format!("struct {}{}{}", s.name.spelled(), generics(&s.generics), fields(&s.fields, true)),
        ItemKind::Enum(e) => {
            let lines: Vec<String> = e.variants.iter().map(variant).collect();
            format!("enum {}{} {}", e.name.spelled(), generics(&e.generics), braces(&lines))
        }
        ItemKind::Impl(i) => {
            let head = match &i.trait_ {
                Some(t) => format!("{} for {}", path(t), ty(&i.self_ty)),
                None => ty(&i.self_ty),
            };
            format!("impl{} {head} {}", generics(&i.generics), braces(&impl_items(&i.items)))
        }
        ItemKind::Trait(t) => {
            let supers = if t.supertraits.is_empty() {
                String::new()
            } else {
                format!(": {}", t.supertraits.iter().map(path).collect::<Vec<_>>().join(" + "))
            };
            format!("trait {}{}{supers} {}", t.name.spelled(), generics(&t.generics), braces(&impl_items(&t.items)))
        }
        ItemKind::Const(c) => const_item(c),
        ItemKind::Use(u) => format!("use {};", use_tree(u)),
        ItemKind::Mod(m) => match &m.items {
            None => format!("mod {};", m.name.spelled()),
            Some(items) => {
                let blocks: Vec<String> = items.iter().map(item).collect();
                if blocks.is_empty() {
                    format!("mod {} {{}}", m.name.spelled())
                } else {
                    format!("mod {} {{\n{}\n}}", m.name.spelled(), indent(&blocks.join("\n\n")))
                }
            }
        },
        ItemKind::Nature(n) => nature(n),
        ItemKind::Module(m) => module(m),
        ItemKind::Behavior(b) => behavior(b),
        ItemKind::Error => "/* error */".to_string(),
    }
}

fn fn_decl(f: &FnDecl) -> String {
    let mut s = String::new();
    if f.is_const {
        s.push_str("const ");
    }
    if let Some(abi) = &f.abi {
        s.push_str(&format!("extern {} ", lit(abi)));
    }
    s.push_str(&format!("fn {}{}", f.name.spelled(), generics(&f.generics)));
    let params: Vec<String> = f
        .params
        .iter()
        .filter_map(|p| match p {
            FnParam::SelfValue(_) => Some("self".to_string()),
            FnParam::Named { name, ty: t, .. } => Some(format!("{}: {}", name.spelled(), ty(t))),
            FnParam::Error(_) => None,
        })
        .collect();
    s.push_str(&list("(", &params, ")"));
    if let Some(r) = &f.ret {
        s.push_str(&format!(" -> {}", ty(r)));
    }
    if let Some(w) = &f.where_ {
        s.push_str(&format!(" where {}", expr(w)));
    }
    match &f.body {
        Some(b) => s.push_str(&format!(" {}", block(b))),
        None => s.push(';'),
    }
    s
}

fn const_item(c: &ConstItem) -> String {
    let value = c.value.as_ref().map_or(String::new(), |v| format!(" = {}", expr(v)));
    format!("const {}: {}{value};", c.name.spelled(), ty(&c.ty))
}

fn impl_items(items: &[ImplItem]) -> Vec<String> {
    items
        .iter()
        .map(|i| {
            let body = match &i.kind {
                ImplItemKind::Fn(f) => fn_decl(f),
                ImplItemKind::Const(c) => const_item(c),
            };
            format!("{}{}{body}", meta(&i.meta), vis(i.public))
        })
        .collect()
}

fn field(f: &Field) -> String {
    let body = match &f.name {
        Some(n) => format!("{}: {}", n.spelled(), ty(&f.ty)),
        None => ty(&f.ty),
    };
    format!("{}{}{}{body}", meta(&f.meta), vis(f.public), dir(&f.dir))
}

/// The body of a struct (`unit_semi`: a unit struct or tuple struct ends in
/// `;`) or of an enum variant.
fn fields(f: &Fields, unit_semi: bool) -> String {
    match f {
        Fields::Named(fs) => {
            let lines: Vec<String> = fs.iter().map(|f| format!("{},", field(f))).collect();
            format!(" {}", braces(&lines))
        }
        Fields::Tuple(fs) => {
            let items: Vec<String> = fs.iter().map(field).collect();
            format!("{}{}", list("(", &items, ")"), if unit_semi { ";" } else { "" })
        }
        Fields::Unit => if unit_semi { ";".to_string() } else { String::new() },
    }
}

fn variant(v: &Variant) -> String {
    let disc = v.discriminant.as_ref().map_or(String::new(), |d| format!(" = {}", expr(d)));
    format!("{}{}{}{disc},", meta(&v.meta), v.name.spelled(), fields(&v.fields, false))
}

fn use_tree(u: &UseTree) -> String {
    let prefix: Vec<String> = u.path.iter().map(|i| i.spelled()).collect();
    let joined = prefix.join("::");
    match &u.kind {
        UseKind::Simple { alias } => match alias {
            Some(a) => format!("{joined} as {}", a.spelled()),
            None => joined,
        },
        UseKind::Glob => if joined.is_empty() { "*".to_string() } else { format!("{joined}::*") },
        UseKind::Group(trees) => {
            let inner = trees.iter().map(use_tree).collect::<Vec<_>>().join(", ");
            if joined.is_empty() { format!("{{{inner}}}") } else { format!("{joined}::{{{inner}}}") }
        }
    }
}

fn nature(n: &NatureDecl) -> String {
    let lines: Vec<String> = n
        .fields
        .iter()
        .map(|f| format!("{}{}", meta(&f.meta), nature_field(&f.kind)))
        .collect();
    format!("nature {}{} {}", n.name.spelled(), generics(&n.generics), braces(&lines))
}

fn nature_field(k: &NatureFieldKind) -> String {
    match k {
            NatureFieldKind::Quantity { which, name, attrs } => {
                let w = match which {
                    Quantity::Potential => "potential",
                    Quantity::Flow => "flow",
                };
                let block = if attrs.is_empty() { String::new() } else { format!(" {}", field_inits(attrs)) };
                format!("{w}: {}{block},", name.spelled())
            }
            NatureFieldKind::Value(t) => format!("value: {},", ty(t)),
            NatureFieldKind::Resolve(e) => format!("resolve: {},", expr(e)),
    }
}

fn module(m: &ModuleDecl) -> String {
    let mut s = String::new();
    if let Some(abi) = &m.abi {
        s.push_str(&format!("extern {} ", lit(abi)));
    }
    s.push_str(&format!("module {}{}", m.name.spelled(), generics(&m.generics)));
    let ports: Vec<String> = m
        .ports
        .iter()
        .map(|p| {
            let default = p.default.as_ref().map_or(String::new(), |d| format!(" = {}", expr(d)));
            format!("{}{}{}: {}{default}", meta(&p.meta), dir(&p.dir), p.name.spelled(), ty(&p.ty))
        })
        .collect();
    s.push_str(&list("(", &ports, ")"));
    if let Some(r) = &m.ret {
        s.push_str(&format!(" -> {}", ty(r)));
    }
    if let Some(w) = &m.where_ {
        s.push_str(&format!(" where {}", expr(w)));
    }
    let mut lines: Vec<String> = m.body.iter().map(module_item).collect();
    if let Some(r) = &m.result {
        lines.push(expr(r));
    }
    s.push_str(&format!(" {}", braces(&lines)));
    s
}

fn module_item(i: &ModuleItem) -> String {
    let body = match &i.kind {
        ModuleItemKind::Param { name, ty: t, default, where_ } => {
            let mut s = format!("{}: {}", name.spelled(), ty(t));
            if let Some(d) = default {
                s.push_str(&format!(" = {}", expr(d)));
            }
            if let Some(w) = where_ {
                s.push_str(&format!(" where {}", expr(w)));
            }
            s.push(';');
            s
        }
        ModuleItemKind::Signal { name, ty: t, init } => signal(name, t, init),
        ModuleItemKind::Let { pat: p, value } => format!("let {} = {};", pat1(p), expr(value)),
        ModuleItemKind::Def { place, value, after } => {
            let after = after.as_ref().map_or(String::new(), |a| format!(" after {}", expr(a)));
            format!("{} = {}{after};", expr(place), expr(value))
        }
        ModuleItemKind::Gen(g) => generation(g, &|i| module_item(i)),
        ModuleItemKind::Error => "/* error */".to_string(),
    };
    format!("{}{}{body}", meta(&i.meta), vis(i.public))
}

/// `for`/`if` generation over items printed by `leaf`.
fn generation<X>(g: &Gen<X>, leaf: &dyn Fn(&X) -> String) -> String {
    let body = |items: &[X]| braces(&items.iter().map(leaf).collect::<Vec<_>>());
    match &g.kind {
        GenKind::For { pat: p, iter, body: b } => format!("for {} in {} {}", pat1(p), expr(iter), body(b)),
        GenKind::If { cond, then, else_ } => {
            let mut s = format!("if {} {}", expr(cond), body(then));
            match else_.as_deref() {
                Some(GenElse::Block(b)) => s.push_str(&format!(" else {}", body(b))),
                Some(GenElse::If(inner)) => s.push_str(&format!(" else {}", generation(inner, leaf))),
                None => {}
            }
            s
        }
    }
}

fn behavior(b: &BehaviorBlock) -> String {
    let (word, lines) = match &b.body {
        BehaviorBody::Analog(stmts) => ("analog", stmts.iter().map(stmt).collect::<Vec<_>>()),
        BehaviorBody::Digital(items) => ("digital", items.iter().map(digital_item).collect::<Vec<_>>()),
    };
    let names = if b.names.is_empty() {
        String::new()
    } else {
        format!("<{}>", b.names.iter().map(ident).collect::<Vec<_>>().join(", "))
    };
    format!("{word}{names} for {} {}", path(&b.target), braces(&lines))
}

fn digital_item(i: &DigitalItem) -> String {
    let body = match &i.kind {
        DigitalItemKind::Reaction(r) => reaction(r),
        DigitalItemKind::Gen(g) => generation(g, &|i| digital_item(i)),
        DigitalItemKind::Error => "/* error */".to_string(),
    };
    format!("{}{body}", meta(&i.meta))
}
