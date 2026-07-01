import os
import re

def update_error():
    path = "crates/piperine-lang/src/pom/error.rs"
    with open(path, "r") as f:
        content = f.read()
    
    if "MultipleDrivers" not in content:
        replacement = """    /// GAPS §B.4 — two or more drivers on a net without a resolve clause.
    #[error("multiple drivers on net `{net}` in module `{module}` (discipline `{discipline}` does not resolve)")]
    MultipleDrivers {
        module: String,
        net: String,
        discipline: String,
    },
    /// A catch-all"""
        content = content.replace("    /// A catch-all", replacement)
        with open(path, "w") as f:
            f.write(content)

def update_lower():
    path = "crates/piperine-lang/src/elab/lower/mod.rs"
    with open(path, "r") as f:
        content = f.read()
    
    content = content.replace("crate::elab::typecheck::typecheck_program(&prog.modules)?;",
                              "crate::elab::typecheck::typecheck_program(&prog)?;")
    with open(path, "w") as f:
        f.write(content)

def update_typecheck():
    path = "crates/piperine-lang/src/elab/typecheck.rs"
    with open(path, "r") as f:
        content = f.read()

    # Change signature of typecheck_program
    content = content.replace(
        "pub fn typecheck_program(\n    modules: &HashMap<String, DesignModule>,\n) -> Result<(), ElabError> {",
        "pub fn typecheck_program(\n    design: &crate::pom::Design,\n) -> Result<(), ElabError> {"
    )
    content = content.replace(
        "    for module in modules.values() {\n        check_module(module, modules)?;\n    }",
        "    for module in design.modules.values() {\n        check_module(module, design)?;\n    }"
    )

    # Change signature of check_module
    content = content.replace(
        "fn check_module(\n    module: &DesignModule,\n    _all_modules: &HashMap<String, DesignModule>,\n) -> Result<(), ElabError> {",
        "fn check_module(\n    module: &DesignModule,\n    design: &crate::pom::Design,\n) -> Result<(), ElabError> {"
    )

    # Implement driver counting before the OK return in check_module
    driver_logic = """
    // B.4 - Driver counting
    // Union-find for connected nets
    let mut parent = HashMap::new();
    let mut find = |mut x: String| -> String {
        while let Some(p) = parent.get(&x) {
            if p == &x { break; }
            x = p.clone();
        }
        x
    };
    let mut union = |x: String, y: String| {
        let root_x = find(x);
        let root_y = find(y);
        if root_x != root_y {
            parent.insert(root_x, root_y);
        }
    };

    for conn in module.connections() {
        union(conn.lhs().to_string(), conn.rhs().to_string());
    }

    let mut drivers: HashMap<String, u64> = HashMap::new();
    let mut add_driver = |net: String| {
        let root = find(net);
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
                if p.direction() == &crate::parse::ast::Direction::Output || p.direction() == &crate::parse::ast::Direction::Inout {
                    if let Some(net_ref) = inst.ports().get(i) {
                        add_driver(net_ref.to_string());
                    }
                }
            }
        }
    }

    // 3. Forces and assigns in behaviors drive nets
    use crate::pom::behavior::BehaviorStmt;
    fn visit_behavior(stmts: &[BehaviorStmt], add_driver: &mut dyn FnMut(String)) {
        for stmt in stmts {
            match stmt {
                BehaviorStmt::Bind { dest, op, .. } => {
                    if op == &crate::parse::ast::BindOp::Force || op == &crate::parse::ast::BindOp::Assign {
                        // Extract base name from Expr if possible
                        if let crate::parse::ast::Expr::Ident(name) = dest {
                            add_driver(name.clone());
                        } else if let crate::parse::ast::Expr::Index(base, _) = dest {
                            if let crate::parse::ast::Expr::Ident(name) = &**base {
                                // We simplify and just use the base name for now, or we could try to format it
                                add_driver(name.clone());
                            }
                        } else if let crate::parse::ast::Expr::Field(base, field) = dest {
                            if let crate::parse::ast::Expr::Ident(name) = &**base {
                                add_driver(format!("{}_{}", name, field));
                            }
                        }
                    }
                }
                BehaviorStmt::If { then_body, else_body, .. } => {
                    visit_behavior(then_body, add_driver);
                    if let Some(eb) = else_body {
                        visit_behavior(eb, add_driver);
                    }
                }
                BehaviorStmt::Match { arms, .. } => {
                    for arm in arms {
                        visit_behavior(arm.body(), add_driver);
                    }
                }
                BehaviorStmt::For { body, .. } => visit_behavior(body, add_driver),
                BehaviorStmt::Event { body, .. } => visit_behavior(body, add_driver),
                _ => {}
            }
        }
    }
    for behavior in module.behaviors() {
        visit_behavior(behavior.body(), &mut add_driver);
    }

    for (root, count) in drivers {
        if count > 1 {
            // It has multiple drivers. Check if discipline resolves.
            // But we need the discipline of `root`.
            let mut resolved_type = None;
            // Let's find any net in the module that belongs to this root to get its type
            for (name, ty) in &locals {
                if find(name.clone()) == root {
                    resolved_type = Some(ty.clone());
                    break;
                }
            }
            if let Some(ty) = resolved_type {
                let d_name = ty.discipline_name();
                let mut resolves = false;
                if let Some(discipline) = design.disciplines.get(d_name) {
                    for item in &discipline.items {
                        if let crate::parse::ast::DisciplineItem::Resolve(_) = item {
                            resolves = true;
                            break;
                        }
                    }
                }
                if !resolves {
                    return Err(ElabError::MultipleDrivers {
                        module: module.name().to_string(),
                        net: root,
                        discipline: d_name.to_string(),
                    });
                }
            }
        }
    }
"""
    content = content.replace("    let _ = module.instances(); // silence unused\n\n    Ok(())", driver_logic + "\n    Ok(())")

    # Fix tests in typecheck.rs to create a dummy Design instead of HashMap
    test_prog_replace = """        let mut prog = crate::pom::Design {
            modules: HashMap::new(),
            disciplines: HashMap::new(),
            bundles: HashMap::new(),
            capabilities: HashMap::new(),
            impls: vec![],
            consts: HashMap::new(),
        };"""
    content = content.replace("let mut prog: HashMap<String, DesignModule> = HashMap::new();", test_prog_replace)
    content = content.replace("let mut prog = HashMap::new();", test_prog_replace)
    content = content.replace("prog.insert(", "prog.modules.insert(")

    with open(path, "w") as f:
        f.write(content)

if __name__ == "__main__":
    update_error()
    update_lower()
    update_typecheck()
