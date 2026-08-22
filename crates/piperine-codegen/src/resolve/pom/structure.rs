//! Module structure → IR: ports, params, wires, instances, connections,
//! and the value-type/const conversions they need.

use piperine_lang::pom::{Module, NetType, ValueType, Design};
use piperine_lang::ast::{Domain as VamsDomain, DisciplineDecl};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::NumberLit;
use crate::resolve::*;

const NOSPAN: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

// ─── Module conversion ───────────────────────────────────────────────────────

/// Determine the IR domain of a net type by inspecting its discipline.
///
/// Per LRM §3.6.2: `domain discrete` marks a digital (event-driven)
/// discipline; `domain continuous` (or an absent `domain` clause — the
/// conservative default) marks an analog one. This is the single place the
/// digital/analog boundary is decided for a net — every port, wire, and
/// terminal flows through here.
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

/// Returns `true` if the named discipline declares `domain discrete`
/// (LRM §3.6.2) — the digital/event-driven marker.
fn is_digital_discipline(prog: &Design, name: &str) -> bool {
    let Some(decl) = prog.discipline(name) else {
        // Unknown discipline — default to analog (the conservative case).
        return false;
    };
    discipline_domain(decl) == Some(VamsDomain::Discrete)
}

fn discipline_domain(decl: &DisciplineDecl) -> Option<VamsDomain> {
    decl.domain
}

/// Build a module's [`SymbolTable`] and resolved [`Port`]s from its POM
/// structure (ports, params, wires, vars). Instance connections and param
/// overrides are resolved directly from the POM by
/// `device::circuit::InstanceBuilder`, at circuit-build time — they are
/// per-*instantiation*, not part of a module's own resolved shape, so
/// building them here would only be a structural twin nobody but the
/// circuit builder reads.
pub(crate) fn build_symbols_and_ports(m: &Module, prog: &Design) -> (SymbolTable, Vec<Port>) {
    use piperine_lang::pom::Direction as PomDirection;

    let mut symbols = SymbolTable::new();
    let mut ports = Vec::new();

    // 0. `ground`-declared names (LRM §3.6.4, LRMW2-07) — registered
    // before ports/wires so every later name lookup already sees them.
    for name in m.ground_names() {
        symbols.add_ground_name(name.clone());
    }

    // 1. Ports
    for p in m.ports() {
        let domain = domain_of(prog, p.net_type());
        let node_id = symbols.add_node(p.name(), domain);
        let direction = match p.direction() {
            PomDirection::Input => super::super::Direction::In,
            PomDirection::Output => super::super::Direction::Out,
            PomDirection::Inout => super::super::Direction::Inout,
        };
        ports.push(Port { node: node_id, direction });
    }

    // 2. Params
    for p in m.params() {
        let ty = elab_value_type_to_ir(p.value_type());
        let default = p.default().map(value_to_ir);
        symbols.add_param(p.name(), ty, default);
    }

    // 3. Wires. A scalar wire becomes one node named after itself; an
    // array wire becomes N distinct element nodes named `tap[0]`..`tap[N-1]`.
    // The flat form lifts an array wire onto the parent verbatim
    // (`flatten.rs`'s `inline_sub_instances`), and the spliced sub-instances'
    // port bindings reference it as `NetRef { net: "<inst>.tap", index:
    // Some(i) }` — the `Display` form `<inst>.tap[i]` is what `resolve_node`
    // looks up at circuit-build time. Registering one scalar node for the
    // lifted wire collapses every element onto a single net; registering
    // one node per element makes `resolve_node("<inst>.tap[i]")` resolve to
    // that element — and the parent-name fallback in `resolve_connections`
    // never fires for an indexed ref.
    for w in m.wires() {
        let domain = domain_of(prog, w.net_type());
        match w.net_type() {
            NetType::Array(_, n) => {
                for i in 0..*n {
                    symbols.add_node(format!("{}[{}]", w.name(), i), domain);
                }
            }
            NetType::Discipline(_) => {
                symbols.add_node(w.name(), domain);
            }
        }
    }

    // 4. Vars — module-level persistent state (GAPS §I.15). The slot is
    // allocated here; its initializer becomes a `VarDecl` statement in the
    // owning behavior during pass 2 (`lower_bodies`), once we know whether
    // it's analog or digital state.
    for v in m.vars() {
        let ty = elab_value_type_to_ir(v.value_type());
        symbols.add_var(v.name(), ty);
    }

    // 5. Functions (§4.7, LRMW2-44, T44): each formal becomes a `VarId` in
    // this same symbol table (a function's own scope is just the module
    // scope, per §4.7's "shall only reference locally-defined variables,
    // variables passed as arguments, locally-defined parameters and module
    // level parameters") and the function itself registers by name so
    // `resolve/pom/expr.rs`'s `inline_user_fn` (via `fn_by_name`) and the
    // digital `Codegen` path (`emit/resolver.rs::Resolver::new`, which
    // snapshots `symbols.fns()`) both find it.
    for f in m.functions() {
        let mut param_ids = Vec::with_capacity(f.params().len());
        for (name, ty) in f.params() {
            let ir_ty = match ty {
                piperine_lang::pom::TypeRef::Value(v) => elab_value_type_to_ir(v),
                piperine_lang::pom::TypeRef::Net(_) => Type::Real,
            };
            param_ids.push(symbols.add_var(name.clone(), ir_ty));
        }
        let returns = match f.ret() {
            piperine_lang::pom::TypeRef::Value(v) => Some(elab_value_type_to_ir(v)),
            piperine_lang::pom::TypeRef::Net(_) => Some(Type::Real),
        };
        symbols.add_fn(Function {
            name: f.name().to_string(),
            params: param_ids,
            directions: f.directions().to_vec(),
            defaults: f.defaults().to_vec(),
            returns,
            body: f.body().to_vec(),
        });
    }

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
        // Enum/Bundle/Tuple/FnPtr: PHDL-only value-layer types with no VAMS
        // Annex A production (MD-45) — a `Param`/`Var` can no longer
        // actually carry one post-frontend-swap (nothing parses one), so
        // this is an unreachable-but-total fallback, not a live path.
        ValueType::Enum(_) => Type::Integer,
        ValueType::Bundle(_) => Type::Real,
        ValueType::Array(inner, _) => elab_value_type_to_ir(inner),
        ValueType::FnPtr(_, _) => Type::Real,
        ValueType::Tuple(_) => Type::Real,
        // Logic(width): four-state values (LRMC-C1-02, `piperine-lang`'s
        // `value::LogicVec`). No VAMS construct types a `Param`/`Var` this
        // way yet (that needs further lrm-conformance-closure work beyond
        // this exhaustiveness fix), so — like the PHDL-only arms above —
        // this is an unreachable-but-total fallback for now. `Type::Quad`
        // is the nearest existing multi-state IR representation (a strict
        // superset of four-state); it is not a silent numeric coercion.
        ValueType::Logic(_) => Type::Quad,
    }
}

pub(crate) fn value_to_ir(v: &piperine_lang::value::Value) -> piperine_lang::ast::Expr {
    value_to_pom_expr(v)
}

/// Convert a POM runtime `Value` to a POM AST `Expr` for the analog/digital
/// paths. The Builder evaluates these via `Codegen`-trait dispatch.
pub(crate) fn value_to_pom_expr(v: &piperine_lang::value::Value) -> piperine_lang::ast::Expr {
    use piperine_lang::value::Value;
    use piperine_lang::ast::Expr;
    match v {
        Value::Real(f) => Expr::Number { span: NOSPAN, value: NumberLit::Real(*f) },
        Value::Int(i) => Expr::Number { span: NOSPAN, value: NumberLit::Integer(*i) },
        Value::Nat(n) => Expr::Number { span: NOSPAN, value: NumberLit::Integer(*n as i64) },
        Value::Bool(b) => Expr::Number { span: NOSPAN, value: NumberLit::Integer(i64::from(*b)) },
        _ => Expr::Number { span: NOSPAN, value: NumberLit::Real(0.0) },
    }
}

#[cfg(test)]
mod tests {
    //! Array-wire symbol-table registration (NGP-18 codegen integration,
    //! Fix A). The flatten pass (T21/NGP-20) lifts an array wire onto the
    //! parent and rewrites the spliced sub-instances' port bindings to
    //! `<inst>.tap[i]`. The symbol table is the place that lifted wire
    //! becomes addressable from circuit-build time — these tests prove it
    //! expands to N distinct element nodes, not one collapsed scalar.

    use super::build_symbols_and_ports;
    use piperine_lang::{parse_and_elaborate, SourceMap};

    /// The simplest case: `electrical tap[2:0];` registers three element
    /// nodes `tap[0]`..`tap[2]`, and the parent wire name `tap` is not
    /// itself a node — the bug that collapsed every element onto a single
    /// net (NGP-18 codegen integration).
    #[test]
    fn array_wire_registers_one_node_per_element() {
        let src = "
             module Mid ();
                 electrical tap[2:0];
             endmodule
        ";
        let design = parse_and_elaborate(src, &SourceMap::dummy())
            .expect("elaborates");
        let (symbols, _) = build_symbols_and_ports(
            design.module("Mid").expect("Mid exists"),
            &design,
        );
        let names: Vec<String> = symbols.nodes().map(|(_, n)| n.name.clone()).collect();
        assert!(names.contains(&"tap[0]".to_string()), "tap[0] registered: {names:?}");
        assert!(names.contains(&"tap[1]".to_string()), "tap[1] registered: {names:?}");
        assert!(names.contains(&"tap[2]".to_string()), "tap[2] registered: {names:?}");
        assert!(
            !names.iter().any(|n| n == "tap"),
            "the parent wire name is not a node — elements replace it: {names:?}"
        );
        assert_eq!(
            names.iter().filter(|n| n.starts_with("tap")).count(),
            3,
            "exactly three tap nodes — one per element"
        );
    }

    /// The end-to-end shape NGP-18 cares about: a parent module `Top`
    /// instantiates a child `Seg` whose body declares `electrical bus[1:0]`.
    /// Flatten lifts `bus` onto the flat-Top as `s.bus[1:0]`
    /// (`parse_and_elaborate` runs the flatten pass); the symbol table for
    /// flat-Top must then carry `s.bus[0]` and `s.bus[1]` as distinct
    /// element nodes.
    #[test]
    fn lifted_array_wire_resolves_per_element_in_flat_parent() {
        let src = "
             module Seg (inout electrical p, inout electrical n);
                 electrical bus[1:0];
                 res r0 (p, bus[0]);
                 res r1 (bus[0], n);
             endmodule
             module res (inout electrical p, inout electrical n);
                 parameter real r = 1.0;
             endmodule
             module Top ();
                 electrical gnd;
                 electrical a;
                 Seg s (a, gnd);
             endmodule
        ";
        let design = parse_and_elaborate(src, &SourceMap::dummy())
            .expect("elaborates (flatten lifts the array wire)");
        let flat = design.flat_module("Top").expect("Top flat form exists");
        let (symbols, _) = build_symbols_and_ports(flat, &design);
        let names: Vec<String> = symbols.nodes().map(|(_, n)| n.name.clone()).collect();
        assert!(
            names.contains(&"s.bus[0]".to_string()),
            "lifted element `s.bus[0]` registered: {names:?}"
        );
        assert!(
            names.contains(&"s.bus[1]".to_string()),
            "lifted element `s.bus[1]` registered: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n == "s.bus"),
            "no scalar parent node — the bug that collapsed every element: {names:?}"
        );
        let bus0 = symbols.nodes().find(|(_, n)| n.name == "s.bus[0]").map(|(id, _)| id);
        let bus1 = symbols.nodes().find(|(_, n)| n.name == "s.bus[1]").map(|(id, _)| id);
        assert!(bus0.is_some(), "s.bus[0] resolves to a node");
        assert!(bus1.is_some(), "s.bus[1] resolves to a node");
        assert_ne!(bus0, bus1, "the two element nodes are distinct NodeIds");
    }
}
