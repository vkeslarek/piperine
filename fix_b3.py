import os

def fix_b3():
    path = "crates/piperine-lang/src/elab/lower/module.rs"
    with open(path, "r") as f:
        content = f.read()

    # We need to build local_types inside elab_mod_inner
    elab_inner_target = """        let mut ports = Vec::new();
        for port in &decl.ports.clone() {
            ports.extend(self.expand_port(port, env, type_subst)?);
        }

        let mut items: Vec<ModBodyItem> = Vec::new();
        let body = decl.body.clone();
        self.lower_mod_stmts(&body, env, type_subst, &mut items)?;"""

    elab_inner_replacement = """        let mut ports = Vec::new();
        let mut local_types: HashMap<String, String> = HashMap::new();
        for port in &decl.ports {
            let resolved_name = type_subst.get(&port.ty.name).map(|s| s.as_str()).unwrap_or(&port.ty.name);
            local_types.insert(port.name.clone(), resolved_name.to_string());
            ports.extend(self.expand_port(port, env, type_subst)?);
        }

        for stmt in &decl.body {
            if let ModStmt::WireDecl { name, ty } = stmt {
                let resolved_name = type_subst.get(&ty.name).map(|s| s.as_str()).unwrap_or(&ty.name);
                local_types.insert(name.clone(), resolved_name.to_string());
            }
        }

        let mut items: Vec<ModBodyItem> = Vec::new();
        let body = decl.body.clone();
        self.lower_mod_stmts(&body, env, type_subst, &local_types, &mut items)?;"""

    content = content.replace(elab_inner_target, elab_inner_replacement)

    # Change signature of lower_mod_stmts
    content = content.replace(
        "    pub(crate) fn lower_mod_stmts(\n        &mut self,\n        stmts: &[ModStmt],\n        env: &mut ConstEnv,\n        type_subst: &HashMap<String, String>,\n        out: &mut Vec<ModBodyItem>,\n    )",
        "    pub(crate) fn lower_mod_stmts(\n        &mut self,\n        stmts: &[ModStmt],\n        env: &mut ConstEnv,\n        type_subst: &HashMap<String, String>,\n        local_types: &HashMap<String, String>,\n        out: &mut Vec<ModBodyItem>,\n    )"
    )
    content = content.replace(
        "            self.lower_mod_stmt(&stmt, env, type_subst, out)?;",
        "            self.lower_mod_stmt(&stmt, env, type_subst, local_types, out)?;"
    )

    # Change signature of lower_mod_stmt
    content = content.replace(
        "    pub(crate) fn lower_mod_stmt(\n        &mut self,\n        stmt: &ModStmt,\n        env: &mut ConstEnv,\n        type_subst: &HashMap<String, String>,\n        out: &mut Vec<ModBodyItem>,\n    )",
        "    pub(crate) fn lower_mod_stmt(\n        &mut self,\n        stmt: &ModStmt,\n        env: &mut ConstEnv,\n        type_subst: &HashMap<String, String>,\n        local_types: &HashMap<String, String>,\n        out: &mut Vec<ModBodyItem>,\n    )"
    )
    
    # Update nested calls to lower_mod_stmts in StructuralFor and StructuralIf
    content = content.replace(
        "                    self.lower_mod_stmts(&body, env, type_subst, out)?;",
        "                    self.lower_mod_stmts(&body, env, type_subst, local_types, out)?;"
    )
    content = content.replace(
        "                self.lower_mod_stmts(&taken, env, type_subst, out)?;",
        "                self.lower_mod_stmts(&taken, env, type_subst, local_types, out)?;"
    )

    # Change WireDecl
    wire_target = """            ModStmt::WireDecl { name, ty } => {
                let nt = self.resolve_net_type(ty, env, type_subst)?;
                out.push(ModBodyItem::Wire(Wire { name: name.clone(), ty: nt }));
            }"""
    wire_replacement = """            ModStmt::WireDecl { name, ty } => {
                let resolved_name = type_subst.get(&ty.name).map(|s| s.as_str()).unwrap_or(&ty.name);
                if let Some(bundle) = self.bundles.get(resolved_name).cloned() {
                    if self.is_net_capable_bundle(resolved_name) {
                        for field in &bundle.fields {
                            let field_ty = self.resolve_net_type(&field.ty, env, type_subst)?;
                            out.push(ModBodyItem::Wire(Wire {
                                name: format!("{}_{}", name, field.name),
                                ty: field_ty,
                            }));
                        }
                        return Ok(());
                    }
                }
                let nt = self.resolve_net_type(ty, env, type_subst)?;
                out.push(ModBodyItem::Wire(Wire { name: name.clone(), ty: nt }));
            }"""
    content = content.replace(wire_target, wire_replacement)

    # Change Connection
    conn_target = """            ModStmt::Connection { lhs, rhs } => {
                let lhs_ref = self.eval_net_ref(lhs, env)?;
                let rhs_ref = self.eval_net_ref(rhs, env)?;
                out.push(ModBodyItem::Conn(Connection { lhs: lhs_ref, rhs: rhs_ref }));
            }"""
    conn_replacement = """            ModStmt::Connection { lhs, rhs } => {
                let mut is_bundle_conn = false;
                if let (crate::parse::ast::Expr::Ident(l_name), crate::parse::ast::Expr::Ident(r_name)) = (lhs, rhs) {
                    if let Some(l_ty_name) = local_types.get(l_name) {
                        if let Some(bundle) = self.bundles.get(l_ty_name).cloned() {
                            if self.is_net_capable_bundle(l_ty_name) {
                                is_bundle_conn = true;
                                for field in &bundle.fields {
                                    let l_ref = crate::pom::net_type::NetRef::simple(format!("{}_{}", l_name, field.name));
                                    let r_ref = crate::pom::net_type::NetRef::simple(format!("{}_{}", r_name, field.name));
                                    out.push(ModBodyItem::Conn(Connection { lhs: l_ref, rhs: r_ref }));
                                }
                            }
                        }
                    }
                }
                
                if !is_bundle_conn {
                    let lhs_ref = self.eval_net_ref(lhs, env)?;
                    let rhs_ref = self.eval_net_ref(rhs, env)?;
                    out.push(ModBodyItem::Conn(Connection { lhs: lhs_ref, rhs: rhs_ref }));
                }
            }"""
    content = content.replace(conn_target, conn_replacement)
    
    # Instance port bindings:
    # Wait, instance port bindings are expanded too!
    inst_target = """                // Resolve port connections to concrete net references.
                let elab_ports = ports
                    .iter()
                    .map(|p| self.eval_net_ref(p, env))
                    .collect::<Result<Vec<_>, _>>()?;"""
    
    inst_replacement = """                // Resolve port connections to concrete net references.
                let mut elab_ports = Vec::new();
                for p in ports {
                    let mut expanded = false;
                    if let crate::parse::ast::Expr::Ident(p_name) = p {
                        if let Some(ty_name) = local_types.get(p_name) {
                            if let Some(bundle) = self.bundles.get(ty_name).cloned() {
                                if self.is_net_capable_bundle(ty_name) {
                                    expanded = true;
                                    for field in &bundle.fields {
                                        elab_ports.push(crate::pom::net_type::NetRef::simple(format!("{}_{}", p_name, field.name)));
                                    }
                                }
                            }
                        }
                    }
                    if !expanded {
                        elab_ports.push(self.eval_net_ref(p, env)?);
                    }
                }"""
    content = content.replace(inst_target, inst_replacement)

    with open(path, "w") as f:
        f.write(content)

if __name__ == "__main__":
    fix_b3()
