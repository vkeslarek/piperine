use piperine_parser::ast::{CallArg, Expr, FunctionRef, Literal, BinOp, PrefixOp, PathSegment};

/// Extract positional expressions from call args (for B-source serialization).
fn pos_args(args: &[CallArg]) -> Vec<&Expr> {
    args.iter().map(|a| a.expr()).collect()
}
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
        Expr::Call(FunctionRef::Path(p), raw_args) if path_is("V", p) => {
            let args = pos_args(raw_args);
            match args.len() {
                1 => {
                    let net = extract_net_path(args[0])?;
                    Ok(format!("v({})", r.resolve(&net)))
                }
                2 => {
                    let n1 = extract_net_path(args[0])?;
                    let n2 = extract_net_path(args[1])?;
                    Ok(format!("v({},{})", r.resolve(&n1), r.resolve(&n2)))
                }
                _ => Err("V() takes 1 or 2 net arguments".into()),
            }
        }

        // I(branch) — branch current access
        Expr::Call(FunctionRef::Path(p), raw_args) if path_is("I", p) => {
            let args = pos_args(raw_args);
            let branch = extract_net_path(args[0])?;
            Ok(format!("i({})", r.resolve(&branch)))
        }

        // ddt(expr) — time derivative
        Expr::Call(FunctionRef::Path(p), raw_args) if path_is("ddt", p) => {
            let args = pos_args(raw_args);
            let inner = serialize_ngspice_expr(args[0], r)?;
            Ok(format!("ddt({})", inner))
        }

        // idt(expr, ic) — time integral
        Expr::Call(FunctionRef::Path(p), raw_args) if path_is("idt", p) => {
            let args = pos_args(raw_args);
            let inner = serialize_ngspice_expr(args[0], r)?;
            let ic = if args.len() > 1 { serialize_ngspice_expr(args[1], r)? } else { "0".into() };
            Ok(format!("idt({},{})", inner, ic))
        }

        // Inline user functions
        Expr::Call(FunctionRef::Path(p), raw_args) if r.get_function(&path_leaf(p)).is_some() => {
            let fname = path_leaf(p);
            let func = r.get_function(&fname).unwrap();
            let args = pos_args(raw_args);
            if args.len() != func.args.len() {
                return Err(format!("function `{}` expects {} arguments, got {}", fname, func.args.len(), args.len()));
            }
            // A simple user function should have `return EXPR;` as the body.
            // Search for `return EXPR` in the body.
            let mut ret_expr = None;
            for stmt in &func.body {
                if let piperine_parser::ast::Stmt::Return(ret) = stmt {
                    ret_expr = ret.value.as_ref();
                    break;
                }
                if let piperine_parser::ast::Stmt::Block(b) = stmt {
                    for item in &b.items {
                        if let piperine_parser::ast::BlockItem::Stmt(piperine_parser::ast::Stmt::Return(ret)) = item {
                            ret_expr = ret.value.as_ref();
                            break;
                        }
                    }
                }
            }
            if let Some(expr) = ret_expr {
                // substitute arguments
                // This is a bit tricky, we need to map arg names to values and rewrite the AST.
                // Or we can serialize the function's return expression with a wrapper NetResolver?
                // Actually, the simplest is to substitute arguments in the serialized string.
                // Let's create a custom NetResolver that overrides parameter lookups?
                // Wait, the arguments to a function are local variables.
                // We can clone the expression and walk it, replacing variables.
                let mut substituted_expr = expr.clone();
                for (i, arg_def) in func.args.iter().enumerate() {
                    let arg_val = args[i].clone();
                    replace_ident(&mut substituted_expr, &arg_def.name, &arg_val);
                }
                serialize_ngspice_expr(&substituted_expr, r)
            } else {
                Err(format!("user function `{}` has no return statement", fname))
            }
        }

        // Math functions: abs sqrt exp ln log sin cos tan ...
        Expr::Call(FunctionRef::Path(p), raw_args) => {
            let fname = path_leaf(p);
            let ng_name = match fname.as_str() {
                "abs"   => "abs",   "sqrt"  => "sqrt",
                "exp"   => "exp",   "ln"    => "ln",
                "log"   => "log",   "log10" => "log",
                "sin"   => "sin",   "cos"   => "cos",
                "tan"   => "tan",   "asin"  => "asin",
                "acos"  => "acos",  "atan"  => "atan",
                "sinh"  => "sinh",  "cosh"  => "cosh",
                "tanh"  => "tanh",
                "atan2" => "atan2", "pow"   => "pow",
                "floor" => "floor", "ceil"  => "ceil",
                other   => return Err(format!("unknown function `{other}` in B-source expression")),
            };
            let arg_strs: Result<Vec<_>, _> = raw_args.iter().map(|a| serialize_ngspice_expr(a.expr(), r)).collect();
            Ok(format!("{}({})", ng_name, arg_strs?.join(",")))
        }

        // $time, $temper — predefined simulator variables
        Expr::Call(FunctionRef::SysFun(name), _) if name == "time" => Ok("time".into()),
        Expr::Call(FunctionRef::SysFun(name), _) if name == "temper" => Ok("temper".into()),

        // Guard against other $system_tasks
        Expr::Call(FunctionRef::SysFun(_), _) => {
            Err("system tasks cannot appear in a behavioral expression; use the bare analog form".into())
        }


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
        // A string literal is an escape hatch: its content is treated as a
        // pre-written ngspice expression (e.g. `.v("v(a)*2")`). Strip the quotes.
        Literal::StrLit(s) => Ok(s.trim_matches('"').to_string()),
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

fn replace_ident(expr: &mut Expr, name: &str, replacement: &Expr) {
    match expr {
        Expr::Path(p) if path_is(name, p) => {
            *expr = replacement.clone();
        }
        Expr::Prefix(_, inner) | Expr::Paren(inner) => {
            replace_ident(inner, name, replacement);
        }
        Expr::Binary(lhs, _, rhs) => {
            replace_ident(lhs, name, replacement);
            replace_ident(rhs, name, replacement);
        }
        Expr::Select(cond, then, els) => {
            replace_ident(cond, name, replacement);
            replace_ident(then, name, replacement);
            replace_ident(els, name, replacement);
        }
        Expr::Call(_, args) => {
            for arg in args {
                match arg {
                    CallArg::Positional(e) => replace_ident(e, name, replacement),
                    CallArg::Named(_, e) => replace_ident(e, name, replacement),
                }
            }
        }
        Expr::Array(items) => {
            for item in items {
                replace_ident(item, name, replacement);
            }
        }
        Expr::Index(base, index) => {
            replace_ident(base, name, replacement);
            replace_ident(index, name, replacement);
        }
        _ => {}
    }
}
