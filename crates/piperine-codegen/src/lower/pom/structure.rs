//! Module structure → IR: ports, params, wires, instances, connections,
//! and the value-type/const conversions they need.

use piperine_lang::pom::{Function as PomFunction, Module, NetType, ValueType, Design};
use piperine_lang::parse::ast::{DisciplineItem, DisciplineDecl};
use crate::lower::*;
use super::stmt::lower_stmts;
use super::expr::lower_expr;
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

/// Build a module's [`SymbolTable`] and resolved [`Port`]s from its POM
/// structure (ports, params, wires, vars). Instance connections and param
/// overrides are resolved directly from the POM by
/// `device::circuit::InstanceBuilder`, at circuit-build time — they are
/// per-*instantiation*, not part of a module's own resolved shape, so
/// building them here would only be a structural twin nobody but the
/// circuit builder reads.
pub(crate) fn build_symbols_and_ports(m: &Module, prog: &Design) -> (SymbolTable, Vec<Port>) {
    use piperine_lang::parse::ast::Direction;

    let mut symbols = SymbolTable::new();
    let mut ports = Vec::new();

    // 1. Ports
    for p in m.ports() {
        let domain = domain_of(prog, p.net_type());
        let node_id = symbols.add_node(p.name(), domain);
        let direction = match p.direction() {
            Direction::Input => super::super::Direction::In,
            Direction::Output => super::super::Direction::Out,
            Direction::Inout => super::super::Direction::Inout,
        };
        ports.push(Port { node: node_id, direction });
    }

    // 2. Params
    for p in m.params() {
        let ty = elab_value_type_to_ir(p.value_type());
        let default = p.default().map(value_to_ir);
        symbols.add_param(p.name(), ty, default);
    }

    // 3. Wires
    for w in m.wires() {
        let domain = domain_of(prog, w.net_type());
        symbols.add_node(w.name(), domain);
    }

    // 4. Vars — module-level persistent state (GAPS §I.15). The slot is
    // allocated here; its initializer becomes a `VarDecl` statement in the
    // owning behavior during pass 2 (`lower_bodies`), once we know whether
    // it's analog or digital state.
    for v in m.vars() {
        let ty = elab_value_type_to_ir(v.value_type());
        symbols.add_var(v.name(), ty);
    }

    // 5. Global functions are added to each module's symbol table in
    //    `lower_bodies` pass 1.5 (after all module skeletons are built), so
    //    that `FnId`s are consistent. Skip here to avoid duplicates.

    (symbols, ports)
}

pub(crate) fn elab_value_type_to_ir(ty: &ValueType) -> Type {
    match ty {
        ValueType::Real | ValueType::Natural => Type::Real,
        ValueType::Integer => Type::Integer,
        ValueType::Complex => Type::Real, // Or Complex if it existed
        ValueType::Boolean => Type::Bool,
        ValueType::Quad => Type::Quad,
        ValueType::Str => Type::Real,
        ValueType::Enum(_) => Type::Integer,
        // Bundle-typed params are flattened per-field before this runs
        // (`convert_fn`); the scalar fallback is never a field's own type.
        ValueType::Bundle(_) => Type::Real,
        ValueType::Array(inner, _) => elab_value_type_to_ir(inner),
        ValueType::FnPtr(_, _) => Type::Real,
    }
}

pub(crate) fn value_to_ir(v: &piperine_lang::value::Value) -> IrExpr {
    use piperine_lang::value::Value;
    match v {
        Value::Real(r) => IrExpr::Real(*r),
        Value::Nat(n) => IrExpr::Int(*n as i64),
        Value::Int(i) => IrExpr::Int(*i),
        Value::Bool(b) => IrExpr::Bool(*b),
        Value::Str(_) => IrExpr::Real(0.0), // No strings in IrExpr
        Value::EnumVariant(enum_name, variant) => {
            // Stable tag: hash the qualified name to a deterministic i64.
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            format!("{}::{}", enum_name, variant).hash(&mut h);
            IrExpr::Int(h.finish() as i64)
        }
        // Non-scalar values (collections, closures, …) cannot reach here:
        // POM param/var storage only ever holds const scalars (rejected at
        // fold time). Mirror the string fallback for the remaining scalars.
        _ => IrExpr::Real(0.0),
    }
}

/// Convert a POM runtime `Value` to a POM AST `Expr` for the digital path
/// (register power-on inits). The digital `Builder` evaluates these via
/// `Codegen`-trait dispatch, so no `IrExpr` is involved.
pub(crate) fn value_to_pom_expr(v: &piperine_lang::value::Value) -> piperine_lang::parse::ast::Expr {
    use piperine_lang::parse::ast::{Expr, Literal};
    use piperine_lang::value::Value;
    match v {
        Value::Real(f) => Expr::Literal(Literal::Real(*f)),
        Value::Int(i) => Expr::Literal(Literal::Int(*i as u64)),
        Value::Nat(n) => Expr::Literal(Literal::Int(*n)),
        Value::Bool(b) => Expr::Literal(Literal::Bool(*b)),
        _ => Expr::Literal(Literal::Real(0.0)),
    }
}

pub(crate) fn convert_fn(
    f: &PomFunction,
    prog: &Design,
    symbols: &mut SymbolTable,
    errors: &mut Vec<super::LowerError>,
) -> Function {
    convert_fn_named(f.name(), f, prog, symbols, errors)
}

/// [`convert_fn`] with an explicit IR name — impl methods register as
/// `Type::method`. Bundle-typed params (GAPS §I.14 extended to fns) are
/// flattened into one scalar var per field (`m` : `ResModel` → `m_rsh`,
/// `m_kf`); the body's `m.rsh` resolves through the qualified-name var
/// lookup, and call sites expand a bundle argument to match.
pub(crate) fn convert_fn_named(
    ir_name: &str,
    f: &PomFunction,
    prog: &Design,
    symbols: &mut SymbolTable,
    errors: &mut Vec<super::LowerError>,
) -> Function {
    let mut params = Vec::new();
    let mut module_vars = HashSet::new();
    let mut bundle_bindings: Vec<(String, (String, Vec<String>))> = Vec::new();
    for (n, ty) in f.params() {
        if let Some(piperine_lang::pom::ValueType::Bundle(bname)) = ty.as_value() {
            let fields = bundle_field_names(prog, bname);
            for field in &fields {
                let flat = format!("{n}_{field}");
                let vid = symbols.add_var(&flat, Type::Real);
                params.push(vid);
                module_vars.insert(flat);
            }
            bundle_bindings.push((n.to_string(), (bname.clone(), fields)));
            continue;
        }
        let vty = elab_value_type_to_ir(ty.as_value().unwrap_or(&piperine_lang::pom::ValueType::Real));
        let vid = symbols.add_var(n, vty);
        params.push(vid);
        module_vars.insert(n.to_string());
    }
    
    let mut ctx = LowerCtx::new(symbols, format!("fn {ir_name}"), false, module_vars);
    ctx.enum_values = prog.enum_value_map();
    ctx.consts = LowerCtx::const_irs(prog);
    ctx.bundle_bindings = bundle_bindings.into_iter().collect();
    ctx.fn_bundle_sigs = fn_bundle_signatures(prog);
    // Lower default expressions (parallel to params) — elaboration
    // constants, so each lowers to a constant `IrExpr` (the language spec Part I §9.1).
    let defaults: Vec<Option<IrExpr>> = f
        .defaults()
        .iter()
        .map(|d| d.as_ref().map(|e| lower_expr(e, &mut ctx)))
        .collect();
    let body = lower_stmts(f.body(), &mut ctx);
    errors.append(&mut ctx.errors);

    let returns = Some(Type::Real); // Best effort fallback
    Function { name: ir_name.to_string(), params, defaults, returns, body }
}

/// Field names of a value bundle, declaration order.
pub(crate) fn bundle_field_names(prog: &Design, bundle: &str) -> Vec<String> {
    prog.bundle(bundle)
        .map(|b| b.fields.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default()
}

/// Bundle-typed parameter positions of every non-generic fn and impl
/// method (methods keyed `Type::method`, with `self` as the leading
/// position) — what call-site expansion consults.
pub(crate) fn fn_bundle_signatures(
    prog: &Design,
) -> std::collections::HashMap<String, Vec<Option<(String, Vec<String>)>>> {
    let sig_of = |params: &[(String, piperine_lang::pom::TypeRef)]| {
        params
            .iter()
            .map(|(_, ty)| match ty.as_value() {
                Some(piperine_lang::pom::ValueType::Bundle(b)) => {
                    Some((b.clone(), bundle_field_names(prog, b)))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut out = std::collections::HashMap::new();
    for f in prog.functions() {
        if !f.is_generic() {
            out.insert(f.name().to_string(), sig_of(f.params()));
        }
    }
    for ib in prog.impls() {
        for m in &ib.methods {
            let mut sig = vec![Some((ib.ty.clone(), bundle_field_names(prog, &ib.ty)))];
            sig.extend(sig_of(&m.params));
            out.insert(format!("{}::{}", ib.ty, m.name), sig);
        }
    }
    out
}
