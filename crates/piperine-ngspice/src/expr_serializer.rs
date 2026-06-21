use piperine_parser::ast::{Expr, FunctionRef, Literal, BinOp, PrefixOp, PathSegment};
use piperine_circuit::NetResolver;

/// Convert a Piperine AST expression to an ngspice B-source expression string.
///
/// Design: recursive descent over Expr. Analog access functions V(), I() resolve
/// their net argument through the NetResolver. Unsupported constructs return Err.
pub fn serialize_ngspice_expr(expr: &Expr, r: &dyn NetResolver) -> Result<String, String> {
    match expr {
        Expr::Literal(lit) => serialize_literal(lit),

        Expr::Prefix(PrefixOp::Neg, inner) =>
            Ok(format!("-({})", serialize_ngspice_expr(inner, r)?)),
        Expr::Prefix(PrefixOp::Pos, inner) =>
            serialize_ngspice_expr(inner, r),
        Expr::Prefix(op, _) =>
            Err(format!("unsupported prefix op {:?} in B-source expression", op)),

        Expr::Binary(lhs, op, rhs) => {
            let l = serialize_ngspice_expr(lhs, r)?;
            let ro = serialize_ngspice_expr(rhs, r)?;
            let op_str = serialize_binop(op)?;
            Ok(format!("({l}{op_str}{ro})"))
        }

        Expr::Paren(inner) =>
            Ok(format!("({})", serialize_ngspice_expr(inner, r)?)),

        Expr::Select(cond, then, els) => {
            let c = serialize_ngspice_expr(cond, r)?;
            let t = serialize_ngspice_expr(then, r)?;
            let e = serialize_ngspice_expr(els, r)?;
            Ok(format!("({c})?({t}):({e})"))
        }

        // V(net) or V(net1, net2) — analog voltage access
        Expr::Call(FunctionRef::Path(p), args) if path_is("V", p) => {
            match args.len() {
                1 => {
                    let net = extract_net_path(&args[0])?;
                    Ok(format!("v({})", r.resolve(&net)))
                }
                2 => {
                    let n1 = extract_net_path(&args[0])?;
                    let n2 = extract_net_path(&args[1])?;
                    Ok(format!("v({},{})", r.resolve(&n1), r.resolve(&n2)))
                }
                _ => Err("V() takes 1 or 2 net arguments".into()),
            }
        }

        // I(branch) — branch current access
        Expr::Call(FunctionRef::Path(p), args) if path_is("I", p) => {
            let branch = extract_net_path(&args[0])?;
            Ok(format!("i({})", r.resolve(&branch)))
        }

        // ddt(expr) — time derivative
        Expr::Call(FunctionRef::Path(p), args) if path_is("ddt", p) => {
            let inner = serialize_ngspice_expr(&args[0], r)?;
            Ok(format!("ddt({})", inner))
        }

        // idt(expr, ic) — time integral
        Expr::Call(FunctionRef::Path(p), args) if path_is("idt", p) => {
            let inner = serialize_ngspice_expr(&args[0], r)?;
            let ic = if args.len() > 1 { serialize_ngspice_expr(&args[1], r)? } else { "0".into() };
            Ok(format!("idt({},{})", inner, ic))
        }

        // Math functions: abs sqrt exp ln log sin cos tan ...
        Expr::Call(FunctionRef::Path(p), args) => {
            let fname = path_leaf(p);
            let ng_name = match fname.as_str() {
                "abs"   => "abs",   "sqrt"  => "sqrt",
                "exp"   => "exp",   "ln"    => "ln",
                "log"   => "log",   "log10" => "log",
                "sin"   => "sin",   "cos"   => "cos",
                "tan"   => "tan",   "asin"  => "asin",
                "acos"  => "acos",  "atan"  => "atan",
                "atan2" => "atan2", "pow"   => "pow",
                "floor" => "floor", "ceil"  => "ceil",
                other   => return Err(format!("unknown function `{other}` in B-source expression")),
            };
            let arg_strs: Result<Vec<_>, _> = args.iter().map(|a| serialize_ngspice_expr(a, r)).collect();
            Ok(format!("{}({})", ng_name, arg_strs?.join(",")))
        }

        // $time, $temper — predefined simulator variables
        Expr::Call(FunctionRef::SysFun(name), _) if name == "time" => Ok("time".into()),
        Expr::Call(FunctionRef::SysFun(name), _) if name == "temper" => Ok("temper".into()),

        // Plain identifier — local variable reference, pass through
        Expr::Path(p) if p.qualifier.is_none() => Ok(path_leaf(p)),

        other => Err(format!("unsupported expression {:?} in B-source context", other)),
    }
}

fn serialize_literal(lit: &Literal) -> Result<String, String> {
    match lit {
        Literal::IntNumber(s) | Literal::StdRealNumber(s) | Literal::SiRealNumber(s) => {
            // ngspice understands SI suffixes too, so pass through
            Ok(s.clone())
        }
        Literal::Inf => Ok("1e308".into()),
        Literal::StrLit(_) => Err("string literal not valid in B-source expression".into()),
    }
}

fn serialize_binop(op: &BinOp) -> Result<&'static str, String> {
    Ok(match op {
        BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
        BinOp::Div => "/", BinOp::Pow => "**",
        BinOp::Eq  => "==", BinOp::Neq => "!=",
        BinOp::Lt  => "<",  BinOp::Le  => "<=",
        BinOp::Gt  => ">",  BinOp::Ge  => ">=",
        BinOp::AndAnd => "&&", BinOp::OrOr => "||",
        other => return Err(format!("unsupported operator {:?} in B-source expression", other)),
    })
}

/// Extract the flat dot-joined string from a hierarchical Path.
/// Path { qualifier: Some(Path{Ident("X1")}), segment: Ident("mid") } → "X1.mid"
fn extract_net_path(expr: &Expr) -> Result<String, String> {
    match expr {
        Expr::Path(p) => Ok(flatten_path(p)),
        other => Err(format!("expected net name (identifier path), got {:?}", other)),
    }
}

fn flatten_path(p: &piperine_parser::ast::Path) -> String {
    let seg = match &p.segment { PathSegment::Ident(s) => s.as_str(), PathSegment::Root => "root" };
    match &p.qualifier {
        Some(q) => format!("{}.{}", flatten_path(q), seg),
        None    => seg.to_string(),
    }
}

fn path_leaf(p: &piperine_parser::ast::Path) -> String {
    match &p.segment { PathSegment::Ident(s) => s.clone(), PathSegment::Root => "root".into() }
}

fn path_is(name: &str, p: &piperine_parser::ast::Path) -> bool {
    p.qualifier.is_none() && matches!(&p.segment, PathSegment::Ident(s) if s == name)
}
