//! # Typecheck pass (`docs/GAPS.md` §B)
//!
//! Walks every module's `ports`, `wires`, `instances`, and `connections`
//! and reports:
//!
//! - **§B.1** — width mismatches on named connections
//!   (`Bit[8]` ↔ `Bit[4]`).
//! - **§B.2** — discipline crossings (`Electrical` ↔ `Thermal` etc.)
//!   unless one side is `Ground`, which is the universal reference.
//!
//! The pass runs **after** elaboration (when `Module::ports`, `Wire.ty`,
//! and `Instance.ports` are all `NetRef` + typed) and **before**
//! codegen, so the solver never sees silently-wrong widths or
//! disciplines. The pass is fail-loud — any violation returns
//! `ElabErrorKind::WidthMismatch` / `DisciplineCrossing` and the
//! elaboration chain reports it to the caller.

use std::collections::HashMap;

use crate::pom::net_type::{NetType, ValueType};
use crate::pom::{Behavior, BehaviorStmt};
use crate::parse::ast::Expr;
use crate::pom::{ElabError, ElabErrorKind, Module as DesignModule};

/// Run the typecheck pass over every module of the elaborated program.
///
/// Each module is checked independently. A failure in one module does
/// not abort the check of the remaining modules — the first error wins
/// (enumeration order is deterministic: declaration order in the AST).
pub fn typecheck_program(
    design: &crate::pom::Design,
) -> Result<(), ElabError> {
    for module in design.modules.values() {
        check_module(module, design)?;
    }
    Ok(())
}

/// Check a single module. Errors are returned on the first violation.
fn check_module(
    module: &DesignModule,
    design: &crate::pom::Design,
) -> Result<(), ElabError> {
    // Build a name → (NetType, declared-index-width) table from the
    // module's ports and wires. We resolve instance port-of-port names
    // (`u1.p`) on demand below using the `port_map` of referenced
    // modules (the elaboration is hierarchical).
    let mut locals: HashMap<String, NetType> = HashMap::new();
    for p in module.ports() {
        locals.insert(p.name().to_string(), p.net_type().clone());
    }
    for w in module.wires() {
        locals.insert(w.name().to_string(), w.net_type().clone());
    }

    // Check every connection.
    for conn in module.connections() {
        let l_ty = resolve_connection_end(conn.lhs(), module, &locals)
            .ok_or_else(|| ElabErrorKind::Other(format!(
                "typecheck ({}): cannot resolve lhs net `{}`",
                module.name(), conn.lhs()
            )))?;
        let r_ty = resolve_connection_end(conn.rhs(), module, &locals)
            .ok_or_else(|| ElabErrorKind::Other(format!(
                "typecheck ({}): cannot resolve rhs net `{}`",
                module.name(), conn.rhs()
            )))?;

        let l_w = l_ty.width();
        let r_w = r_ty.width();
        if l_w != r_w {
            return Err(ElabError::from(ElabErrorKind::WidthMismatch {
                module: module.name().to_string(),
                lhs: conn.lhs().to_string(),
                rhs: conn.rhs().to_string(),
                lhs_w: l_w,
                rhs_w: r_w,
            }));
        }

        // B.2 — discipline compatibility. `Ground` ↔ any conservative
        // discipline is always allowed (Ground is the universal reference).
        let l_d = l_ty.discipline_name().to_string();
        let r_d = r_ty.discipline_name().to_string();
        if l_d != r_d && l_d != "Ground" && r_d != "Ground" {
            return Err(ElabError::from(ElabErrorKind::DisciplineCrossing {
                module: module.name().to_string(),
                lhs: l_d,
                rhs: r_d,
            } ));
        }
    }
    
    // Check behaviors (GAPS §B.5)
    for behavior in &module.behaviors {
        check_behavior(module, behavior, &locals, design)?;
    }

    // Check every instance port binding.
    // TODO (GAPS §B + §I.14): validate each `inst.ports[i]` against
    // the child module's `port(name).ty`. Deferred until I.14
    // (hierarchical ports / bundle-aware expansion) lands.

    // B.4 - Driver counting
    // Union-find for connected nets
    let mut parent: HashMap<String, String> = HashMap::new();

    fn find(parent: &HashMap<String, String>, mut x: String) -> String {
        while let Some(p) = parent.get(&x) {
            if p == &x { break; }
            x = p.clone();
        }
        x
    }

    for conn in module.connections() {
        let root_x = find(&parent, conn.lhs().to_string());
        let root_y = find(&parent, conn.rhs().to_string());
        if root_x != root_y {
            parent.insert(root_x, root_y);
        }
    }

    let mut drivers: HashMap<String, u64> = HashMap::new();
    let mut add_driver = |net: String| {
        let root = find(&parent, net);
        *drivers.entry(root).or_insert(0) += 1;
    };

    // 1. Module input ports drive internal nets
    for p in module.ports() {
        if p.direction() == &crate::parse::ast::Direction::Input || p.direction() == &crate::parse::ast::Direction::Inout {
            add_driver(p.name().to_string());
        }
    }

    // 2. Child instance output ports drive connected nets
    for inst in module.instances() {
        if let Some(child) = design.modules.get(inst.module_name()) {
            for (i, p) in child.ports().iter().enumerate() {
                if (p.direction() == &crate::parse::ast::Direction::Output || p.direction() == &crate::parse::ast::Direction::Inout)
                    && let Some(net_ref) = inst.ports().get(i) {
                        add_driver(net_ref.to_string());
                    }
            }
        }
    }

    // 3. Forces and assigns in behaviors drive nets
    use crate::pom::behavior::BehaviorStmt;
    for behavior in module.behaviors() {
        let mut visit = |stmt: &BehaviorStmt| {
            if let BehaviorStmt::Bind { dest, op, .. } = stmt {
                if op == &crate::parse::ast::BindOp::Force || op == &crate::parse::ast::BindOp::Assign {
                    // Extract base name from Expr if possible
                    if let crate::parse::ast::Expr::Ident(name) = dest {
                        add_driver(name.clone());
                    } else if let crate::parse::ast::Expr::Index(base, _) = dest {
                        if let crate::parse::ast::Expr::Ident(name) = &**base {
                            // We simplify and just use the base name for now, or we could try to format it
                            add_driver(name.clone());
                        }
                    } else if let crate::parse::ast::Expr::Field(base, field) = dest
                        && let crate::parse::ast::Expr::Ident(name) = &**base {
                            add_driver(format!("{}_{}", name, field));
                        }
                }
            }
        };
        // `walk_stmts` is pre-order and includes the root — no separate
        // root visit, or every driver double-counts.
        for stmt in behavior.body() {
            stmt.walk_stmts(&mut visit);
        }
    }

    for (root, count) in drivers {
        if count > 1 {
            // It has multiple drivers. Check if discipline resolves.
            // But we need the discipline of `root`.
            let mut resolved_type = None;
            // Let's find any net in the module that belongs to this root to get its type
            for (name, ty) in &locals {
                if find(&parent, name.clone()) == root {
                    resolved_type = Some(ty.clone());
                    break;
                }
            }
            if let Some(ty) = resolved_type {
                let d_name = ty.discipline_name();
                let mut resolves = false;
                if d_name == "Ground" {
                    resolves = true;
                } else if let Some(discipline) = design.disciplines.get(d_name) {
                    for item in &discipline.items {
                        match item {
                            crate::parse::ast::DisciplineItem::Resolve(_) => resolves = true,
                            crate::parse::ast::DisciplineItem::Nature { kind: crate::parse::ast::NatureKind::Flow, .. } => resolves = true,
                            _ => {}
                        }
                    }
                }
                if !resolves {
                    return Err(ElabError::from(ElabErrorKind::MultipleDrivers {
                        module: module.name().to_string(),
                        net: root,
                        discipline: d_name.to_string(),
                    } ));
                }
            }
        }
    }

    Ok(())
}

/// Resolve a connection-end `NetRef` (e.g. `u1.p` or `node[3]`) to its
/// declared `NetType`. Walks the current module's `ports` and `wires` for
/// bare names; instance-port names (`u1.p`) look up the child module
/// in `all_modules` (when available).
///
/// Bus indices narrow the resolved type — `node[3]` for a `node :
/// Electrical[4]` bus resolves to `Electrical`, not `Electrical[4]`. The
/// `NetType` returned is the element type after one `Array` unwrap. This
/// matches the spec: connecting `node[3]` to a scalar `Electrical` port
/// is valid, and the widths both come out to 1.
fn resolve_connection_end(
    net: &crate::pom::net_type::NetRef,
    module: &DesignModule,
    locals: &HashMap<String, NetType>,
) -> Option<NetType> {
    let _ = module; // hierarchical path deferred (GAPS §I.14)

    let base_ty = locals.get(net.net()).cloned()?;
    if net.index().is_some() {
        // Peel exactly one Array dimension per indexed access. PHDL arrays
        // are row-major 1-D at the NetRef level; multi-dimensional arrays
        // are flattened by the elaborator. So `Array(inner, n)` with
        // index resolves to `inner`.
        if let NetType::Array(inner, _n) = base_ty {
            Some(*inner)
        } else {
            // Index on a non-array — elaboration would have errored already.
            None
        }
    } else {
        Some(base_ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(name: &str) -> NetType {
        NetType::Discipline(name.to_string())
    }
    fn scalar(name: &str) -> crate::pom::Module {
        crate::pom::Module::new(
            "T".into(),
            vec![crate::pom::module::Port { span: None, attributes: vec![],
                direction: crate::parse::ast::Direction::Inout,
                name: "p".into(),
                ty: ty(name),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )
    }

    #[test]
    fn width_mismatch_on_named_connection_is_caught() {
                let mut prog = crate::pom::Design::new();
        let _bad = scalar("Bit");
        // Override port width via a fake module — we keep it scalar here
        // and rely on the width mismatch coming from the second conn
        // entry's index difference.
        let bad_mod = crate::pom::Module::new(
            "T".into(),
            vec![crate::pom::module::Port { span: None, attributes: vec![],
                direction: crate::parse::ast::Direction::Inout,
                name: "p".into(),
                ty: NetType::Array(Box::new(ty("Bit")), 8),
            }],
            vec![],
            vec![
                crate::pom::module::Wire { span: None, attributes: vec![], name: "w".into(), ty: NetType::Array(Box::new(ty("Bit")), 4) },
            ],
            vec![],
            vec![crate::pom::module::Connection { span: None,
                lhs: crate::pom::net_type::NetRef::simple("p"),
                rhs: crate::pom::net_type::NetRef::simple("w"),
            }],
            vec![],
        );
        prog.modules.insert("T".into(), bad_mod);

        let err = typecheck_program(&prog).unwrap_err().to_string();
        assert!(err.contains("width"), "expected width mismatch message, got: {err}");
    }

    #[test]
    fn wire_width_mismatch_via_array_dim_is_caught() {
        // Two arrays of different widths connected.
        let bad_mod = crate::pom::Module::new(
            "T".into(),
            vec![],
            vec![],
            vec![
                crate::pom::module::Wire { span: None, attributes: vec![], name: "a".into(), ty: NetType::Array(Box::new(ty("Bit")), 8) },
                crate::pom::module::Wire { span: None, attributes: vec![], name: "b".into(), ty: NetType::Array(Box::new(ty("Bit")), 4) },
            ],
            vec![],
            vec![crate::pom::module::Connection { span: None,
                lhs: crate::pom::net_type::NetRef::simple("a"),
                rhs: crate::pom::net_type::NetRef::simple("b"),
            }],
            vec![],
        );
                let mut prog = crate::pom::Design::new();
        prog.modules.insert("T".into(), bad_mod);
        let err = typecheck_program(&prog).unwrap_err().to_string();
        assert!(err.contains("width") && err.contains("8") && err.contains("4"),
            "expected width mismatch naming both widths, got: {err}");
    }

    #[test]
    fn discipline_crossing_is_rejected() {
                let mut prog = crate::pom::Design::new();
        let bad_mod = crate::pom::Module::new(
            "T".into(),
            vec![
                crate::pom::module::Port { span: None, attributes: vec![],
                    direction: crate::parse::ast::Direction::Inout,
                    name: "e".into(),
                    ty: ty("Electrical"),
                },
                crate::pom::module::Port { span: None, attributes: vec![],
                    direction: crate::parse::ast::Direction::Inout,
                    name: "t".into(),
                    ty: ty("Thermal"),
                },
            ],
            vec![],
            vec![],
            vec![],
            vec![crate::pom::module::Connection { span: None,
                lhs: crate::pom::net_type::NetRef::simple("e"),
                rhs: crate::pom::net_type::NetRef::simple("t"),
            }],
            vec![],
        );
        prog.modules.insert("T".into(), bad_mod);
        let err = typecheck_program(&prog).unwrap_err().to_string();
        assert!(err.contains("discipline crossing") || err.contains("DisciplineCrossing"),
            "expected discipline-crossing error, got: {err}");
    }

    #[test]
    fn same_discipline_connection_passes() {
                let mut prog = crate::pom::Design::new();
        let ok_mod = crate::pom::Module::new(
            "T".into(),
            vec![],
            vec![],
            vec![
                crate::pom::module::Wire { span: None, attributes: vec![], name: "a".into(), ty: ty("Electrical") },
                crate::pom::module::Wire { span: None, attributes: vec![], name: "b".into(), ty: ty("Electrical") },
            ],
            vec![],
            vec![crate::pom::module::Connection { span: None,
                lhs: crate::pom::net_type::NetRef::simple("a"),
                rhs: crate::pom::net_type::NetRef::simple("b"),
            }],
            vec![],
        );
        prog.modules.insert("T".into(), ok_mod);
        assert!(typecheck_program(&prog).is_ok(), "same-discipline connect should validate");
    }

    #[test]
    fn multiple_drivers_on_digital_net_is_rejected() {
        let mut prog = crate::pom::Design::new();
        // A module that drives `out` from two instances.
        let bad_mod = crate::pom::Module::new(
            "T".into(),
            vec![],
            vec![],
            vec![
                crate::pom::module::Wire { span: None, attributes: vec![], name: "w".into(), ty: ty("Bit") },
            ],
            vec![
                crate::pom::Instance { span: None, attributes: vec![], label: Some("u1".into()),
                    module: "Driver".into(),
                    ports: vec![crate::pom::net_type::NetRef::simple("w")],
                    params: vec![],
                },
                crate::pom::Instance { span: None, attributes: vec![], label: Some("u2".into()),
                    module: "Driver".into(),
                    ports: vec![crate::pom::net_type::NetRef::simple("w")],
                    params: vec![],
                },
            ],
            vec![],
            vec![],
        );
        let driver_mod = crate::pom::Module::new(
            "Driver".into(),
            vec![
                crate::pom::module::Port { span: None, attributes: vec![],
                    direction: crate::parse::ast::Direction::Output,
                    name: "o".into(),
                    ty: ty("Bit"),
                }
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        prog.modules.insert("T".into(), bad_mod);
        prog.modules.insert("Driver".into(), driver_mod);
        
        let err = typecheck_program(&prog).unwrap_err().to_string();
        assert!(err.contains("multiple drivers") || err.contains("MultipleDrivers"),
            "expected multiple drivers error, got: {err}");
    }
}

/// The value type a discipline carries: its `storage` type for storage
/// disciplines, `Real` (the potential) for conservative ones.
fn discipline_value_type(discipline: &str, design: &crate::pom::Design) -> ValueType {
    use crate::parse::ast::DisciplineItem;
    let Some(decl) = design.discipline(discipline) else {
        // Ground and unknown disciplines read as a potential.
        return ValueType::Real;
    };
    for item in &decl.items {
        if let DisciplineItem::Storage(ty) = item {
            return match ty.name.as_str() {
                "Real" => ValueType::Real,
                "Boolean" => ValueType::Boolean,
                "Natural" => ValueType::Natural,
                "Integer" => ValueType::Integer,
                _ => ValueType::Quad,
            };
        }
    }
    ValueType::Real
}

fn check_behavior(
    module: &DesignModule,
    behavior: &Behavior,
    nets: &std::collections::HashMap<String, NetType>,
    design: &crate::pom::Design,
) -> Result<(), ElabError> {
    let mut locals: std::collections::HashMap<String, ValueType> = std::collections::HashMap::new();
    
    // A net reads/drives as the value its discipline carries: a
    // conservative discipline as `Real` (its potential), a storage
    // discipline as its declared storage type (SPEC §6.2).
    for (name, net_ty) in nets {
        locals.insert(name.clone(), discipline_value_type(net_ty.discipline_name(), design));
    }
    
    // Check each root statement, then recurse into its children via walk_stmts.
    for stmt in &behavior.body {
        // `walk_stmts` is pre-order and includes the root.
        let mut err: Option<ElabError> = None;
        stmt.walk_stmts(&mut |s| {
            if err.is_none() {
                if let Err(e) = check_one_stmt(s, module, behavior, &mut locals) {
                    err = Some(e);
                }
            }
        });
        if let Some(e) = err { return Err(e); }
    }
    Ok(())
}

/// Check a single statement node (no recursion into children).
fn check_one_stmt(
    stmt: &BehaviorStmt,
    module: &DesignModule,
    behavior: &Behavior,
    locals: &mut std::collections::HashMap<String, ValueType>,
) -> Result<(), ElabError> {
    match stmt {
        BehaviorStmt::VarDecl { name, .. } => {
            // Resolved type lives in the behavior's side table
            // (SIMPLIFICATION.md P3) — the statement keeps its surface
            // (unresolved) annotation.
            if let Some(vt) = behavior.var_types.get(name) {
                locals.insert(name.clone(), vt.clone());
            }
        }
        BehaviorStmt::Bind { dest, src, .. } => {
            let dest_ty = type_of_expr(dest, locals);
            let src_ty = type_of_expr(src, locals);
            if let (Some(d), Some(s)) = (dest_ty, src_ty)
                && d != s {
                    // Implicit widenings (SPEC §4/§6.1): `Boolean` widens to
                    // `Quad`; the integer literals `0`/`1` are also Boolean/
                    // Quad/Natural literals, so integer-typed sources may
                    // drive those destinations.
                    let widening_ok = matches!(
                        (&d, &s),
                        (ValueType::Quad, ValueType::Boolean)
                            | (ValueType::Boolean, ValueType::Integer)
                            | (ValueType::Quad, ValueType::Integer)
                            | (ValueType::Natural, ValueType::Integer)
                            | (ValueType::Boolean, ValueType::Natural)
                            | (ValueType::Quad, ValueType::Natural)
                    );
                    if !widening_ok {
                        return Err(ElabError::from(ElabErrorKind::Other(format!(
                            "typecheck ({}::{}): implicit cast from {:?} to {:?} not allowed. Use an explicit cast.",
                            module.name(), behavior.name, s, d
                        ))));
                    }
                }
        }
        _ => {}
    }
    Ok(())
}

fn type_of_expr(expr: &Expr, locals: &std::collections::HashMap<String, ValueType>) -> Option<ValueType> {
    match expr {
        Expr::Literal(crate::parse::ast::Literal::Int(_)) => Some(ValueType::Integer),
        Expr::Literal(crate::parse::ast::Literal::Real(_)) => Some(ValueType::Real),
        Expr::Literal(crate::parse::ast::Literal::Bool(_)) => Some(ValueType::Boolean),
        Expr::Literal(crate::parse::ast::Literal::Quad(_)) => Some(ValueType::Quad),
        Expr::Literal(crate::parse::ast::Literal::String(_)) => Some(ValueType::Str),
        Expr::Ident(name) => locals.get(name).cloned(),
        Expr::Cast(target, _) => {
            match target.as_str() {
                "real" => Some(ValueType::Real),
                "int" => Some(ValueType::Integer),
                "bit" => Some(ValueType::Quad),
                "Boolean" => Some(ValueType::Boolean),
                "Quad" => Some(ValueType::Quad),
                _ => None,
            }
        }
        Expr::Binary(lhs, op, _rhs) => {
            use crate::parse::ast::BinaryOp;
            match op {
                BinaryOp::Eq | BinaryOp::Neq | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge |
                BinaryOp::And | BinaryOp::Or => Some(ValueType::Boolean),
                _ => type_of_expr(lhs, locals),
            }
        }
        Expr::Unary(_, inner) => type_of_expr(inner, locals),
        _ => None, // Cannot infer or complex
    }
}
