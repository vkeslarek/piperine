//! Module structure → IR: ports, params, wires, instances, connections,
//! and the value-type/const conversions they need.

use crate::pom::{Function, Module, NetType, ValueType};

use piperine_codegen::ir::*;

use super::stmt::lower_stmts;
use super::LowerCtx;

// ─── Module conversion ───────────────────────────────────────────────────────

/// Convert a PHDL [`Module`] into an [`IrModule`], lowering ports, params,
/// wires, instances, and connections.
pub(crate) fn convert_mod(m: &Module) -> IrModule {
    use crate::parse::ast::Direction;

    let ports = m.ports().iter().map(|p| {
        IrPort {
            name: p.name().to_string(),
            direction: match p.direction() {
                Direction::Input => IrDirection::In,
                Direction::Output => IrDirection::Out,
                Direction::Inout => IrDirection::Inout,
            },
            discipline: discipline_name(p.net_type()),
        }
    }).collect();

    let params = m.params().iter().map(|p| {
        IrParam {
            name: p.name().to_string(),
            ty: elab_value_type_to_ir(p.value_type()),
            default: p.default().map(const_val_to_ir),
        }
    }).collect();

    let wires = m.wires().iter().map(|w| {
        IrWire {
            name: w.name().to_string(),
            discipline: discipline_name(w.net_type()),
        }
    }).collect();

    // GAPS §I.15 — module-level persistent `var`s. Mirrored into
    // `IrAnalogBody.vars` too (see `ppr_to_ir`) so the analog body is
    // self-describing about which names are runtime state vs. params.
    let vars = m.vars().iter().map(|v| {
        IrVarDecl {
            name: v.name().to_string(),
            ty: elab_value_type_to_ir(v.value_type()),
            init: v.init().map(const_val_to_ir),
        }
    }).collect();

    let instances = m.instances().iter().filter_map(|inst| {
        Some(IrInstance {
            label: inst.name().to_string(),
            module: inst.module_name().to_string(),
            connections: inst.ports().iter().map(|r| IrConnection {
                port: None,
                net: r.to_string(),
            }).collect(),
            params: inst.params().iter().map(|(k, v)| (k.clone(), const_val_to_ir(v))).collect(),
        })
    }).collect();

    // Net connections (aliasing)
    let connections = m.connections().iter().map(|c| IrConnectionDecl {
        lhs: c.lhs().to_string(),
        rhs: c.rhs().to_string(),
    }).collect();

    IrModule {
        name: m.name().to_string(),
        ports,
        params,
        wires,
        branches: vec![],
        events: vec![],
        vars,
        grounds: vec![],
        instances,
        connections,
        continuous_assigns: vec![],
        analog: None,
        digital: None,
        functions: vec![],
    }
}

/// Extract the discipline name from a [`NetType`], recursing through arrays.
pub(crate) fn discipline_name(ty: &NetType) -> Option<String> {
    match ty {
        NetType::Discipline(s) => Some(s.clone()),
        NetType::Array(inner, _) => discipline_name(inner),
    }
}

/// Map an elaborated [`ValueType`] to the corresponding [`IrType`].
pub(crate) fn elab_value_type_to_ir(ty: &ValueType) -> IrType {
    match ty {
        ValueType::Real | ValueType::Natural => IrType::Real,
        ValueType::Integer => IrType::Integer,
        ValueType::Complex => IrType::Complex,
        ValueType::Boolean => IrType::Bool,
        ValueType::Quad => IrType::Quad,
        ValueType::Str => IrType::String,
        ValueType::Enum(_) => IrType::Integer,
        ValueType::Array(inner, _) => elab_value_type_to_ir(inner),
        ValueType::FnPtr(_, _) => IrType::Void,
    }
}

/// Convert a compile-time evaluated constant into an [`IrExpr`] literal.
pub(crate) fn const_val_to_ir(v: &crate::elab::const_eval::ConstVal) -> IrExpr {
    use crate::elab::const_eval::ConstVal;
    match v {
        ConstVal::Real(r) => IrExpr::Real(*r),
        ConstVal::Nat(n) => IrExpr::Int(*n as i64),
        ConstVal::Int(i) => IrExpr::Int(*i),
        ConstVal::Bool(b) => IrExpr::Bool(*b),
        ConstVal::Str(s) => IrExpr::String(s.clone()),
    }
}

/// Convert a PHDL [`Function`] (user-defined, global) into an [`IrFunction`].
pub(crate) fn convert_fn(f: &Function) -> IrFunction {
    let params: Vec<String> = f.params().iter().map(|(n, _)| n.clone()).collect();
    let mut ctx = LowerCtx::new();
    let body = lower_stmts(f.body(), &mut ctx);
    IrFunction { name: f.name().to_string(), params, body }
}

