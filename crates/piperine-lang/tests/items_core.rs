//! Proves the core declaration grammar (tasks.md T9) against LRM §3.4/
//! §3.6/§6.3: nature (incl. derived + standard attributes), discipline
//! (incl. domain/potential/flow), module/macromodule/connectmodule (both
//! port-list styles), ground, net declarations (discipline+range+nodeset),
//! and parameter/localparam/aliasparam/specparam (incl. value ranges,
//! string params, arrays).
//!
//! LRM-COVERS: 3.4, 3.4.1, A.2.4, A.2.5
//! Wave-2 (T69) re-reading, no new tests: §3.4 "Parameters" and §3.4.1
//! "Type specification" ("the parameter declaration can contain an
//! optional type specification ... the parameter keyword acts more as a
//! type qualifier than a type specifier") are `parameter_and_localparam_
//! both_parse` (`parameter real r`/`localparam integer n`, each asserted
//! against its declared `ParamType`) and `string_parameter_parses`
//! (`parameter string label`) directly.

use piperine_lang::ast::*;
use piperine_lang::{Grammar, Lexer};

fn grammar(src: &str) -> Grammar {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error for {src:?}: {e}"));
    Grammar::new(toks)
}

// ── nature, §3.6.1 ───────────────────────────────────────────────────────

#[test]
fn nature_with_all_five_standard_attributes_parses() {
    let src = "nature Voltage; units = \"V\"; abstol = 1e-6; access = V; idt_nature = Flux; ddt_nature = Charge; endnature";
    let n = grammar(src).parse_nature_decl().unwrap();
    assert_eq!(n.name, "Voltage");
    assert!(n.units.is_some());
    assert!(n.abstol.is_some());
    assert_eq!(n.access.as_deref(), Some("V"));
    assert_eq!(n.idt_nature.as_deref(), Some("Flux"));
    assert_eq!(n.ddt_nature.as_deref(), Some("Charge"));
}

#[test]
fn derived_nature_carries_its_parent() {
    let src = "nature Charge : Voltage; units = \"C\"; endnature";
    let n = grammar(src).parse_nature_decl().unwrap();
    assert_eq!(n.parent.as_deref(), Some("Voltage"));
}

#[test]
fn nature_user_defined_attribute_parses() {
    let src = "nature Voltage; units = \"V\"; symbol = \"V\"; endnature";
    let n = grammar(src).parse_nature_decl().unwrap();
    assert_eq!(n.user_attrs.len(), 1);
    assert_eq!(n.user_attrs[0].0, "symbol");
}

// ── discipline, §3.6.2 ───────────────────────────────────────────────────

#[test]
fn discipline_with_domain_potential_and_flow_parses() {
    let src = "discipline Electrical; domain continuous; potential Voltage; flow Current; enddiscipline";
    let d = grammar(src).parse_discipline_decl().unwrap();
    assert_eq!(d.name, "Electrical");
    assert_eq!(d.domain, Some(Domain::Continuous));
    assert_eq!(d.potential.as_deref(), Some("Voltage"));
    assert_eq!(d.flow.as_deref(), Some("Current"));
}

#[test]
fn discrete_domain_discipline_parses() {
    let src = "discipline Logic; domain discrete; enddiscipline";
    let d = grammar(src).parse_discipline_decl().unwrap();
    assert_eq!(d.domain, Some(Domain::Discrete));
}

#[test]
fn natureless_discipline_has_no_potential_or_flow() {
    let src = "discipline Untyped; domain continuous; enddiscipline";
    let d = grammar(src).parse_discipline_decl().unwrap();
    assert!(d.potential.is_none());
    assert!(d.flow.is_none());
}

// ── module: declaration-list port style, §Syntax 2-5 ─────────────────────

#[test]
fn module_with_declaration_style_ports_parses() {
    let src = "module resistor(inout electrical p, inout electrical n); endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    assert_eq!(m.kind, ModuleKind::Module);
    assert_eq!(m.name, "resistor");
    let ports = m.ports.expect("declaration-style ports");
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].dir, Direction::Inout);
    assert_eq!(ports[0].discipline.as_deref(), Some("electrical"));
    assert_eq!(ports[0].names[0].name, "p");
}

#[test]
fn macromodule_and_connectmodule_kinds_parse() {
    let m1 = grammar("macromodule m(); endmodule").parse_module_decl().unwrap();
    assert_eq!(m1.kind, ModuleKind::Macromodule);
    let m2 = grammar("connectmodule m(); endmodule").parse_module_decl().unwrap();
    assert_eq!(m2.kind, ModuleKind::Connectmodule);
}

// ── module: name-list port style with body re-declaration ────────────────

#[test]
fn module_with_name_list_ports_and_body_redeclaration_parses() {
    let src = "module resistor(p, n); inout electrical p; inout electrical n; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    assert!(m.ports.is_none(), "name-list style leaves ModuleDecl::ports as None");
    let port_items: Vec<_> =
        m.items.iter().filter(|i| matches!(i, ModuleItem::Port(_))).collect();
    assert_eq!(port_items.len(), 2);
}

// ── module attribute instances, §Syntax 2-5 ───────────────────────────────

#[test]
fn module_attribute_instance_parses() {
    let src = "(* device = \"my_device\" *) module m(); endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    assert_eq!(m.attrs.len(), 1);
    assert_eq!(m.attrs[0].name, "device");
    assert!(m.attrs[0].value.is_some());
}

// ── module parameter port list, §3.4 ──────────────────────────────────────

#[test]
fn module_parameter_port_list_parses() {
    let src = "module resistor #(parameter real r = 1.0e3) (inout electrical p, inout electrical n); endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    assert_eq!(m.param_ports.len(), 1);
    assert_eq!(m.param_ports[0].names[0].name, "r");
}

// ── ground, §3.6.4 ────────────────────────────────────────────────────────

#[test]
fn ground_declaration_inside_a_module_parses() {
    let src = "module m(); ground gnd; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Ground(g) => assert_eq!(g.name, "gnd"),
        other => panic!("expected ModuleItem::Ground, got {other:?}"),
    }
}

// ── net declaration: discipline + range + nodeset, §3.6.3/§3.6.3.2 ──────

#[test]
fn net_declaration_with_discipline_and_multiple_names_parses() {
    let src = "module m(); electrical a, b; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Net(n) => {
            assert_eq!(n.discipline.as_deref(), Some("electrical"));
            assert_eq!(n.names.len(), 2);
        }
        other => panic!("expected ModuleItem::Net, got {other:?}"),
    }
}

#[test]
fn net_declaration_with_bus_range_parses() {
    let src = "module m(); electrical bus[3:0]; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Net(n) => {
            let r = n.names[0].range.as_ref().expect("range");
            assert!(matches!(r.msb, Expr::Number { .. }));
            assert!(matches!(r.lsb, Expr::Number { .. }));
        }
        other => panic!("expected ModuleItem::Net, got {other:?}"),
    }
}

#[test]
fn net_declaration_with_nodeset_initial_value_parses() {
    let src = "module m(); electrical vout = 0.0; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Net(n) => assert!(n.nodeset.is_some()),
        other => panic!("expected ModuleItem::Net, got {other:?}"),
    }
}

// ── parameter / localparam, §3.4 ──────────────────────────────────────────

#[test]
fn parameter_and_localparam_both_parse() {
    let src = "module m(); parameter real r = 1.0; localparam integer n = 4; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Param(p) => {
            assert!(!p.is_local);
            assert_eq!(p.ty, Some(ParamType::Real));
        }
        other => panic!("expected ModuleItem::Param, got {other:?}"),
    }
    match &m.items[1] {
        ModuleItem::Param(p) => {
            assert!(p.is_local);
            assert_eq!(p.ty, Some(ParamType::Integer));
        }
        other => panic!("expected ModuleItem::Param, got {other:?}"),
    }
}

#[test]
fn string_parameter_parses() {
    let src = "module m(); parameter string label = \"hello\"; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Param(p) => {
            assert_eq!(p.ty, Some(ParamType::String));
            assert!(matches!(p.names[0].value, Expr::Str { .. }));
        }
        other => panic!("expected ModuleItem::Param, got {other:?}"),
    }
}

#[test]
fn parameter_value_range_from_and_exclude_parse() {
    let src = "module m(); parameter real g = 1.0 from [0:inf) exclude 0.5; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Param(p) => {
            assert_eq!(p.ranges.len(), 2);
            assert!(!p.ranges[0].excluded);
            assert!(p.ranges[0].hi_exclusive);
            assert!(p.ranges[1].excluded);
        }
        other => panic!("expected ModuleItem::Param, got {other:?}"),
    }
}

#[test]
fn parameter_array_declares_multiple_names_in_one_statement() {
    let src = "module m(); parameter real a = 1.0, b = 2.0, c = 3.0; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Param(p) => assert_eq!(p.names.len(), 3),
        other => panic!("expected ModuleItem::Param, got {other:?}"),
    }
}

// ── aliasparam, §3.4.7 ────────────────────────────────────────────────────

#[test]
fn aliasparam_parses_name_and_target() {
    let src = "module m(); aliasparam r = resistance; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::AliasParam(a) => {
            assert_eq!(a.name, "r");
            assert_eq!(a.target, "resistance");
        }
        other => panic!("expected ModuleItem::AliasParam, got {other:?}"),
    }
}

// ── specparam ──────────────────────────────────────────────────────────

#[test]
fn specparam_parses() {
    let src = "module m(); specparam tsetup = 2.0; endmodule";
    let m = grammar(src).parse_module_decl().unwrap();
    match &m.items[0] {
        ModuleItem::Specparam(s) => {
            assert_eq!(s.names[0].0, "tsetup");
        }
        other => panic!("expected ModuleItem::Specparam, got {other:?}"),
    }
}

// ── errors ─────────────────────────────────────────────────────────────

#[test]
fn unterminated_nature_is_a_typed_eof_error() {
    let mut g = grammar("nature Voltage; units = \"V\";");
    let err = g.parse_nature_decl().unwrap_err();
    assert!(matches!(err.kind, piperine_lang::ParseErrorKind::UnexpectedEof { .. }));
}

#[test]
fn malformed_module_item_is_a_typed_error() {
    let mut g = grammar("module m(); ??? endmodule");
    let err = g.parse_module_decl().unwrap_err();
    assert!(matches!(err.kind, piperine_lang::ParseErrorKind::UnexpectedToken { .. }));
}
