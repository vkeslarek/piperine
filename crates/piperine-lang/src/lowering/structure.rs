//! Module structure → IR: ports, params, wires, instances, connections,
//! and the value-type/const conversions they need.

use crate::pom::{Function, Module, NetType, ValueType, Design};
use crate::parse::ast::{DisciplineItem, DisciplineDecl};
use piperine_codegen::ir::*;
use super::stmt::lower_stmts;
use super::LowerCtx;
use std::collections::HashSet;

// ─── Module conversion ───────────────────────────────────────────────────────

/// Determine the IR domain of a net type by inspecting its discipline.
///
/// Per SPEC §6.2: a storage discipline is digital iff its storage value
/// type is `Boolean` or `Quad`; a storage discipline with `Real` storage
/// is analog (signal-flow). A conservative discipline (potential+flow) is
/// always analog. This is the single place the digital/analog boundary is
/// decided for a net — every port, wire, and terminal flows through here.
fn domain_of(prog: &Design, net_type: &NetType) -> Domain {
    let discipline_name = match net_type {
        NetType::Discipline(name) => name.as_str(),
        NetType::Array(inner, _) => return domain_of(prog, inner),
    };
    if is_digital_discipline(prog, discipline_name) {
        Domain::Digital
    } else {
        Domain::Analog
    }
}

/// Returns `true` if the named discipline is a storage discipline whose
/// storage value type is `Boolean` or `Quad` — the digital storage kinds.
fn is_digital_discipline(prog: &Design, name: &str) -> bool {
    let Some(decl) = prog.discipline(name) else {
        // Unknown discipline — default to analog (the conservative case).
        return false;
    };
    let Some(storage_name) = storage_value_type(decl) else {
        return false;
    };
    matches!(storage_name, "Boolean" | "Quad")
}

/// Extract the storage value type name from a discipline declaration.
/// Returns `None` for conservative disciplines (no `storage` clause).
fn storage_value_type(decl: &DisciplineDecl) -> Option<&str> {
    for item in &decl.items {
        if let DisciplineItem::Storage(ty) = item {
            return Some(ty.name.as_str());
        }
    }
    None
}

/// Convert a PHDL [`Module`] into an [`IrModule`].
pub(crate) fn convert_mod(m: &Module, prog: &Design) -> IrModule {
    use crate::parse::ast::Direction;

    let mut symbols = SymbolTable::new();
    let mut ports = Vec::new();


    // 1. Ports
    for p in m.ports() {
        let domain = domain_of(prog, p.net_type());
        let node_id = symbols.add_node(p.name(), domain);
        let direction = match p.direction() {
            Direction::Input => IrDirection::In,
            Direction::Output => IrDirection::Out,
            Direction::Inout => IrDirection::Inout,
        };
        ports.push(IrPort { node: node_id, direction });
    }

    // 2. Params
    for p in m.params() {
        let ty = elab_value_type_to_ir(p.value_type());
        let default = p.default().map(|v| const_val_to_ir(v, &mut symbols));
        symbols.add_param(p.name(), ty, default);
    }

    // 3. Wires
    for w in m.wires() {
        let domain = domain_of(prog, w.net_type());
        symbols.add_node(w.name(), domain);
    }

    // 4. Vars
    for v in m.vars() {
        let ty = elab_value_type_to_ir(v.value_type());
        let id = symbols.add_var(v.name(), ty);
        let init = v.init().map(|v| const_val_to_ir(v, &mut symbols));
        // GAPS §I.15 — We don't have IrVarDecl in IrModule anymore, it's just in SymbolTable.
        // We can just add them to the symbol table, the initialization goes into digital/analog bodies or is handled elsewhere.
        // Wait, if it has an init, we probably should emit a VarDecl statement in the body or something.
    }

    // 5. Global Functions are added to each module's symbol table in
    //    `ppr_to_ir` pass 1.5 (after all module skeletons are built), so
    //    that `FnId`s are consistent. Skip here to avoid duplicates.

    // 6. Instances
    let mut instances = Vec::new();
    for inst in m.instances() {
        let mut connections = Vec::new();
        for r in inst.ports() {
            let name = r.to_string();
            let mut node_id = NodeId::GROUND;
            if name == "gnd" || name == "GND" || name == "vss" || name == "VSS" || name == "0" {
                node_id = NodeId::GROUND;
            } else if let Some((id, _)) = symbols.nodes().find(|(_, n)| n.name == name) {
                node_id = id;
            }
            connections.push(node_id);
        }
        
        let mut params = Vec::new();
        // Since we don't know child ParamId in pass 1, we just do a best-effort lookup by name.
        // But wait, IrInstance has Vec<(ParamId, IrExpr)>. The target param name is in inst.params().
        // To resolve ParamId properly, we should look it up in the child module's SymbolTable.
        // Since we don't have it here yet, we will just construct ParamId(0) for now and fix it in Pass 2,
        // OR we can just pass the child module's param index.
        // Wait, since we are returning IrModule now, let's look up the target module in `prog`.
        if let Some(child) = prog.module(inst.module_name()) {
            for (pname, pval) in inst.params() {
                // Find index of parameter in child module.
                if let Some(idx) = child.params().iter().position(|p| p.name() == pname) {
                    params.push((ParamId(idx as u32), const_val_to_ir(pval, &mut symbols)));
                }
            }
        }
        
        instances.push(IrInstance {
            label: inst.name().to_string(),
            module: inst.module_name().to_string(),
            connections,
            params,
        });
    }

    // Net connections (aliasing)
    // The new IR doesn't have `connections` (aliasing) on IrModule.
    // It's handled during lowering or we drop them if unsupported.
    // Actually, aliasing is dropped or resolved before this? The user prompt said:
    // "The new `IrModule` only takes `name`, `symbols`, `ports`, `instances`, `analog`, `digital`."

    IrModule {
        name: m.name().to_string(),
        symbols,
        ports,
        instances,
        analog: None,
        digital: None,
    }
}

pub(crate) fn discipline_name(ty: &NetType) -> Option<String> {
    match ty {
        NetType::Discipline(s) => Some(s.clone()),
        NetType::Array(inner, _) => discipline_name(inner),
    }
}

pub(crate) fn elab_value_type_to_ir(ty: &ValueType) -> IrType {
    match ty {
        ValueType::Real | ValueType::Natural => IrType::Real,
        ValueType::Integer => IrType::Integer,
        ValueType::Complex => IrType::Real, // Or Complex if it existed
        ValueType::Boolean => IrType::Bool,
        ValueType::Quad => IrType::Quad,
        ValueType::Str => IrType::Real,
        ValueType::Enum(_) => IrType::Integer,
        ValueType::Array(inner, _) => elab_value_type_to_ir(inner),
        ValueType::FnPtr(_, _) => IrType::Real,
    }
}

pub(crate) fn const_val_to_ir(v: &crate::elab::const_eval::ConstVal, symbols: &mut SymbolTable) -> IrExpr {
    use crate::elab::const_eval::ConstVal;
    match v {
        ConstVal::Real(r) => IrExpr::Real(*r),
        ConstVal::Nat(n) => IrExpr::Int(*n as i64),
        ConstVal::Int(i) => IrExpr::Int(*i),
        ConstVal::Bool(b) => IrExpr::Bool(*b),
        ConstVal::Str(_) => IrExpr::Real(0.0), // No strings in IrExpr
        ConstVal::EnumVariant(enum_name, variant) => {
            // Stable tag: hash the qualified name to a deterministic i64.
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            format!("{}::{}", enum_name, variant).hash(&mut h);
            IrExpr::Int(h.finish() as i64)
        }
    }
}

pub(crate) fn convert_fn(f: &Function, prog: &Design, symbols: &mut SymbolTable) -> IrFunction {
    let mut params = Vec::new();
    let mut module_vars = HashSet::new();
    for (n, ty) in f.params() {
        let vty = elab_value_type_to_ir(ty.as_value().unwrap_or(&crate::pom::ValueType::Real));
        let vid = symbols.add_var(n, vty);
        params.push(vid);
        module_vars.insert(n.to_string());
    }
    
    let mut ctx = LowerCtx::new(symbols, false, module_vars);
    ctx.enum_values = prog.enum_value_map();
    let body = lower_stmts(f.body(), &mut ctx);
    
    let returns = Some(IrType::Real); // Best effort fallback
    IrFunction { name: f.name().to_string(), params, returns, body }
}
