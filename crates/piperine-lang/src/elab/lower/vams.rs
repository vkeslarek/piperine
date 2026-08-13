//! VAMS → POM structural lowering: `module`/`macromodule`/`connectmodule`
//! declarations (LRM Annex A.1.2) into POM [`Module`]s — ports (both
//! declaration-list and name-list styles, §3.7), nets/wires with
//! disciplines (§3.6.3), parameters (§3.4), instantiation (named + ordered
//! connections, parameter overrides, §6.3) and behavior-block attachment
//! (`analog`/`analog initial`/`initial`/`always`, structurally nested
//! `ModuleItem`s — unlike PHDL, VAMS has no separate top-level `BehaviorDecl`
//! matched by name).
//!
//! **VAMS-04, MD-25**: the POM `Module`/`Instance`/`Port` shape mirrors the
//! LRM directly — a discipline name is carried as-is (`NetType::Discipline`),
//! never re-typed through a PHDL-only net-capability lattice; the
//! instance→submodule→sub-instance tree is exactly the authored nesting
//! (no flattening, no connect-module insertion — those are later passes,
//! T25/T52, writing only the derived side artifact).

use std::collections::HashMap;

use piperine_vams::ast::{
    Declarator, Direction as AstDirection, Expr, ModuleDecl, ModuleItem, ParamType, PortConnection,
    PortDecl,
};

use crate::elab::const_eval::ConstEnv;
use crate::pom::{
    Behavior, Direction, ElabError, ElabErrorKind, Instance, Module, NetRef, NetType,
    Param, Port, Value, ValueType, Wire,
};

use super::Elaborator;

impl Elaborator {
    /// Elaborates one `ModuleDecl` into a POM `Module`. Structural
    /// (no cross-module resolution beyond looking up an instantiated
    /// module's port order in the already-registered symbol table).
    pub(crate) fn elab_module(&self, decl: &ModuleDecl) -> Result<Module, ElabError> {
        let env = ConstEnv::new();

        let mut ports = Vec::new();
        let mut wires = Vec::new();
        let mut params = Vec::new();
        let mut instances = Vec::new();
        let mut behaviors = Vec::new();

        // Declaration-list style port list (`module m(input electrical a, ...)`).
        for p in decl.ports.iter().flatten() {
            lower_port_decl(p, &mut ports)?;
        }

        for item in &decl.items {
            match item {
                // Name-list style: `module m(a, b); input electrical a; ...`.
                ModuleItem::Port(p) => lower_port_decl(p, &mut ports)?,
                ModuleItem::Net(n) => {
                    let discipline = n.discipline.as_deref().ok_or_else(|| {
                        ElabError::from(ElabErrorKind::Other(format!(
                            "net declaration has no discipline — undeclared-net discipline \
                             resolution (LRM Annex F) is not implemented until T26"
                        )))
                    })?;
                    for d in &n.names {
                        wires.push(declarator_to_wire(d, discipline, &env)?);
                    }
                }
                ModuleItem::Param(p) => {
                    let ty = param_value_type(p.ty);
                    for (name, expr) in &p.names {
                        let default = Some(eval_param_default(expr, &env)?);
                        params.push(Param {
                            span: None,
                            attributes: Vec::new(),
                            doc: None,
                            name: name.clone(),
                            ty: ty.clone(),
                            default,
                            bundle_origin: None,
                            distribution: None,
                        });
                    }
                }
                ModuleItem::Instantiation(inst_decl) => {
                    for ast_inst in &inst_decl.instances {
                        instances.push(self.lower_instance(
                            &inst_decl.module_name,
                            ast_inst,
                            &inst_decl.param_overrides,
                        )?);
                    }
                }
                ModuleItem::Behavior(b) => {
                    let stmts = match b.body.as_ref() {
                        piperine_vams::ast::Stmt::Block { stmts, .. } => stmts.clone(),
                        other => vec![other.clone()],
                    };
                    behaviors.push(Behavior::new(decl.name.clone(), b.kind, stmts));
                }
                // Branches (T23), genvar/generate (T25), ground/var/event
                // declarations, continuous assign, specify/gates: not yet
                // lowered — later tasks extend this match.
                ModuleItem::Ground(_)
                | ModuleItem::Branch(_)
                | ModuleItem::Var(_)
                | ModuleItem::Event(_)
                | ModuleItem::Genvar(_)
                | ModuleItem::AliasParam(_)
                | ModuleItem::Specparam(_)
                | ModuleItem::Defparam(_)
                | ModuleItem::Function(_)
                | ModuleItem::Task(_)
                | ModuleItem::ContinuousAssign(_)
                | ModuleItem::LoopGenerate(_)
                | ModuleItem::IfGenerate(_)
                | ModuleItem::CaseGenerate(_)
                | ModuleItem::GenerateRegion(_)
                | ModuleItem::Specify(_)
                | ModuleItem::Gate(_) => {}
            }
        }

        let module =
            Module::new(decl.name.clone(), ports, params, wires, instances, Vec::new(), behaviors);
        Ok(module)
    }

    /// Lowers one `Instance` (from a `module_identifier inst1(...), ...;`
    /// list) into a POM `Instance` — resolves ordered/named connections
    /// against the target module's registered port order (§6.3).
    fn lower_instance(
        &self,
        module_name: &str,
        ast_inst: &piperine_vams::ast::Instance,
        param_overrides: &[PortConnection],
    ) -> Result<Instance, ElabError> {
        let target = self.syms.module_decls.get(module_name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::UndefinedModule(module_name.to_string()))
        })?;
        let target_port_names = target_port_order(target);

        let all_positional = ast_inst.connections.iter().all(|c| c.name.is_none());
        let mut ports = Vec::new();
        if all_positional {
            for c in &ast_inst.connections {
                let expr = c.value.as_ref().ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "instance `{}`: a port connection with no value is only legal for an unconnected port (`.p()`), \
                         which is not yet supported",
                        ast_inst.name
                    )))
                })?;
                ports.push(expr_to_net_ref(expr)?);
            }
        } else {
            let mut by_name: HashMap<&str, &Expr> = HashMap::new();
            for c in &ast_inst.connections {
                let name = c.name.as_deref().ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "instance `{}`: cannot mix ordered and named port connections",
                        ast_inst.name
                    )))
                })?;
                if let Some(v) = &c.value {
                    by_name.insert(name, v);
                }
            }
            for port_name in &target_port_names {
                let expr = by_name.get(port_name.as_str()).ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "instance `{}`: no connection given for port `{port_name}` of module `{module_name}`",
                        ast_inst.name
                    )))
                })?;
                ports.push(expr_to_net_ref(expr)?);
            }
        }

        let env = ConstEnv::new();
        let mut params = Vec::new();
        for ov in param_overrides {
            let name = ov.name.clone().ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: `#(...)` parameter overrides must be named",
                    ast_inst.name
                )))
            })?;
            let expr = ov.value.as_ref().ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: parameter override `{name}` has no value",
                    ast_inst.name
                )))
            })?;
            params.push((name, eval_param_default(expr, &env)?));
        }

        Ok(Instance {
            span: None,
            label_span: None,
            type_span: None,
            attributes: Vec::new(),
            doc: None,
            label: Some(ast_inst.name.clone()),
            module: module_name.to_string(),
            ports,
            params,
        })
    }
}

/// The target module's port order (declaration-list style first, else
/// name-list style items in declaration order) — needed to resolve
/// positional (ordered) instance connections (§6.3).
fn target_port_order(decl: &ModuleDecl) -> Vec<String> {
    if let Some(ports) = &decl.ports {
        ports.iter().flat_map(|p| p.names.iter().map(|d| d.name.clone())).collect()
    } else {
        decl.items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::Port(p) => Some(p.names.iter().map(|d| d.name.clone())),
                _ => None,
            })
            .flatten()
            .collect()
    }
}

fn lower_port_decl(p: &PortDecl, out: &mut Vec<Port>) -> Result<(), ElabError> {
    let discipline = p.discipline.as_deref().ok_or_else(|| {
        ElabError::from(ElabErrorKind::Other(
            "port declaration has no discipline — undeclared-net discipline resolution \
             (LRM Annex F) is not implemented until T26"
                .to_string(),
        ))
    })?;
    let direction = match p.dir {
        AstDirection::Input => Direction::Input,
        AstDirection::Output => Direction::Output,
        AstDirection::Inout => Direction::Inout,
    };
    let env = ConstEnv::new();
    for d in &p.names {
        out.push(Port {
            span: None,
            attributes: Vec::new(),
            doc: None,
            direction,
            name: d.name.clone(),
            ty: declarator_net_type(d, discipline, &env)?,
        });
    }
    Ok(())
}

fn declarator_to_wire(d: &Declarator, discipline: &str, env: &ConstEnv) -> Result<Wire, ElabError> {
    Ok(Wire {
        span: None,
        attributes: Vec::new(),
        doc: None,
        name: d.name.clone(),
        ty: declarator_net_type(d, discipline, env)?,
    })
}

/// A `[msb:lsb]` range widens a scalar discipline into `NetType::Array`
/// (LRM §3.7/3.6.3 `range`) — width is `|msb - lsb| + 1`.
fn declarator_net_type(d: &Declarator, discipline: &str, env: &ConstEnv) -> Result<NetType, ElabError> {
    let base = NetType::Discipline(discipline.to_string());
    match &d.range {
        None => Ok(base),
        Some(r) => {
            let msb = env.eval_int(&r.msb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: format!("net `{}` range msb", d.name), source: e })
            })?;
            let lsb = env.eval_int(&r.lsb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: format!("net `{}` range lsb", d.name), source: e })
            })?;
            let width = (msb - lsb).unsigned_abs() + 1;
            Ok(NetType::Array(Box::new(base), width))
        }
    }
}

fn param_value_type(ty: Option<ParamType>) -> ValueType {
    match ty {
        Some(ParamType::Integer) => ValueType::Integer,
        Some(ParamType::Real) | Some(ParamType::Realtime) | Some(ParamType::Time) | None => ValueType::Real,
        Some(ParamType::String) => ValueType::Str,
    }
}

fn eval_param_default(expr: &Expr, env: &ConstEnv) -> Result<Value, ElabError> {
    env.eval(expr)
        .map_err(|e| ElabError::from(ElabErrorKind::ConstEval { context: "parameter default".into(), source: e }))
}

/// A port-connection value resolves to a `NetRef` — a bare identifier, or
/// `name[index]` for a bus tap.
fn expr_to_net_ref(expr: &Expr) -> Result<NetRef, ElabError> {
    match expr {
        Expr::Ident { name, .. } => Ok(NetRef::simple(name.clone())),
        Expr::Index { base, index, .. } => {
            let Expr::Ident { name, .. } = base.as_ref() else {
                return Err(ElabError::from(ElabErrorKind::Other(
                    "port connection: only `name[index]` indexing is supported".to_string(),
                )));
            };
            let idx = ConstEnv::new().eval_nat(index).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: "port connection index".into(), source: e })
            })?;
            Ok(NetRef::indexed(name.clone(), idx))
        }
        _ => Err(ElabError::from(ElabErrorKind::Other(
            "port connection must be a net name or `name[index]`".to_string(),
        ))),
    }
}
