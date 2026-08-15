//! Proves the remaining declaration grammar (tasks.md T11): UDPs
//! (combinational + sequential state tables), specify/endspecify, every
//! IEEE-1364 gate primitive with drive strengths/delay, paramset,
//! connectrules/connect, config — plus the `parse_source_file` top-level
//! entry point that ties every `Item` kind together.

use piperine_lang::ast::*;
use piperine_lang::{Grammar, Lexer};

fn grammar(src: &str) -> Grammar {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error for {src:?}: {e}"));
    Grammar::new(toks)
}

fn one_module_item(src: &str) -> ModuleItem {
    let full = format!("module m(); {src} endmodule");
    let m = grammar(&full).parse_module_decl().unwrap_or_else(|e| panic!("parse error for {src:?}: {e}"));
    assert_eq!(m.items.len(), 1, "expected exactly one item for {src:?}, got {:?}", m.items);
    let mut items = m.items;
    items.remove(0)
}

// ── UDP, Annex A.5 ────────────────────────────────────────────────────────

#[test]
fn combinational_udp_parses_with_no_current_state() {
    let src = "primitive my_and (q, a, b); output q; input a, b; table 0 0 : 0; 0 1 : 0; 1 0 : 0; 1 1 : 1; endtable endprimitive";
    let p = grammar(src).parse_primitive_decl().unwrap();
    assert_eq!(p.name, "my_and");
    assert!(!p.is_sequential);
    assert_eq!(p.table.len(), 4);
    assert_eq!(p.table[0].inputs, vec!["0".to_string(), "0".to_string()]);
    assert_eq!(p.table[0].next_state, "0");
    assert!(p.table[0].current_state.is_none());
    assert_eq!(p.table[3].next_state, "1");
}

#[test]
fn sequential_udp_parses_with_current_state_column() {
    let src = "primitive my_latch (q, d, clk); output reg q; input d, clk; table 1 1 : ? : 1; 1 0 : ? : 0; 0 ? : ? : - ; endtable endprimitive";
    let p = grammar(src).parse_primitive_decl().unwrap();
    assert!(p.is_sequential);
    assert_eq!(p.table[0].current_state.as_deref(), Some("?"));
    assert_eq!(p.table[0].next_state, "1");
}

#[test]
fn udp_ports_capture_output_reg_and_input_kinds() {
    let src = "primitive p (q, a); output reg q; input a; table 0 : ? : 0; endtable endprimitive";
    let p = grammar(src).parse_primitive_decl().unwrap();
    match &p.ports[0] {
        UdpPortDecl::Output { is_reg, name, .. } => {
            assert!(is_reg);
            assert_eq!(name, "q");
        }
        other => panic!("expected UdpPortDecl::Output, got {other:?}"),
    }
    match &p.ports[1] {
        UdpPortDecl::Input { names, .. } => assert_eq!(names, &vec!["a".to_string()]),
        other => panic!("expected UdpPortDecl::Input, got {other:?}"),
    }
}

// ── specify, Annex A.7.1 ──────────────────────────────────────────────────

#[test]
fn specify_block_parses_without_rejecting_its_contents() {
    match one_module_item("specify (a => b) = 1.0; endspecify") {
        ModuleItem::Specify(_) => {}
        other => panic!("expected ModuleItem::Specify, got {other:?}"),
    }
}

// ── every IEEE-1364 gate primitive, Annex A.4.2 ───────────────────────────

#[test]
fn every_gate_primitive_keyword_parses_with_its_kind() {
    let cases: &[(&str, GateKind)] = &[
        ("and", GateKind::And), ("nand", GateKind::Nand), ("or", GateKind::Or), ("nor", GateKind::Nor),
        ("xor", GateKind::Xor), ("xnor", GateKind::Xnor), ("buf", GateKind::Buf), ("not", GateKind::Not),
        ("bufif0", GateKind::Bufif0), ("bufif1", GateKind::Bufif1),
        ("notif0", GateKind::Notif0), ("notif1", GateKind::Notif1),
        ("nmos", GateKind::Nmos), ("pmos", GateKind::Pmos), ("cmos", GateKind::Cmos),
        ("rnmos", GateKind::Rnmos), ("rpmos", GateKind::Rpmos), ("rcmos", GateKind::Rcmos),
        ("pullup", GateKind::Pullup), ("pulldown", GateKind::Pulldown),
        ("tran", GateKind::Tran), ("rtran", GateKind::Rtran),
        ("tranif0", GateKind::Tranif0), ("tranif1", GateKind::Tranif1),
        ("rtranif0", GateKind::Rtranif0), ("rtranif1", GateKind::Rtranif1),
    ];
    for (kw, expected) in cases {
        let src = format!("{kw} g1(a, b);");
        match one_module_item(&src) {
            ModuleItem::Gate(g) => assert_eq!(g.kind, *expected, "kind mismatch for {kw:?}"),
            other => panic!("expected ModuleItem::Gate for {kw:?}, got {other:?}"),
        }
    }
}

#[test]
fn gate_instance_captures_name_and_terminals() {
    match one_module_item("and g1(y, a, b);") {
        ModuleItem::Gate(g) => {
            assert_eq!(g.instances.len(), 1);
            assert_eq!(g.instances[0].name.as_deref(), Some("g1"));
            assert_eq!(g.instances[0].terminals.len(), 3);
        }
        other => panic!("expected ModuleItem::Gate, got {other:?}"),
    }
}

#[test]
fn gate_with_delay_and_multiple_instances_parses() {
    match one_module_item("buf #2 g1(y1, a), g2(y2, b);") {
        ModuleItem::Gate(g) => {
            assert!(g.delay.is_some());
            assert_eq!(g.instances.len(), 2);
        }
        other => panic!("expected ModuleItem::Gate, got {other:?}"),
    }
}

#[test]
fn unnamed_gate_instance_parses() {
    match one_module_item("not (y, a);") {
        ModuleItem::Gate(g) => assert!(g.instances[0].name.is_none()),
        other => panic!("expected ModuleItem::Gate, got {other:?}"),
    }
}

// ── paramset ───────────────────────────────────────────────────────────

#[test]
fn paramset_declares_module_target_and_params() {
    let src = "paramset ps (resistor); parameter real r = 10e3; endparamset";
    let p = grammar(src).parse_paramset_decl().unwrap();
    assert_eq!(p.name, "ps");
    assert_eq!(p.module_name, "resistor");
    assert_eq!(p.params.len(), 1);
    assert_eq!(p.params[0].names[0].name, "r");
}

// ── connectrules, chapter 7 ────────────────────────────────────────────

#[test]
fn connectrules_with_block_body_parses() {
    let src = "connectrules cr; connect electrical, electrical { merged = 1; } endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(cr.name, "cr");
    assert_eq!(cr.rules.len(), 1);
    assert_eq!(cr.rules[0].disciplines, vec!["electrical".to_string(), "electrical".to_string()]);
    assert_eq!(cr.rules[0].body.len(), 1);
}

#[test]
fn connectrules_with_semicolon_only_rule_parses() {
    let src = "connectrules cr; connect electrical, logic; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert!(cr.rules[0].body.is_empty());
}

// ── connect_insertion, §7.7.1 Syntax 7-6 (LRMC-C7-02, T31) ──────────────

#[test]
fn connect_insertion_bare_module_name_parses() {
    let src = "connectrules cr; connect bridge; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    let rule = &cr.rules[0];
    assert_eq!(rule.connectmodule, Some("bridge".to_string()));
    assert_eq!(rule.mode, None);
    assert!(rule.params.is_empty());
    assert_eq!(rule.overrides, None);
}

#[test]
fn connect_insertion_with_merged_mode_parses() {
    let src = "connectrules cr; connect bridge merged; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(cr.rules[0].mode, Some(ConnectMode::Merged));
}

#[test]
fn connect_insertion_with_split_mode_parses() {
    let src = "connectrules cr; connect bridge split; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(cr.rules[0].mode, Some(ConnectMode::Split));
}

#[test]
fn connect_insertion_with_parameter_value_assignment_parses() {
    let src = "connectrules cr; connect bridge #(.res(50)); endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    let rule = &cr.rules[0];
    assert_eq!(rule.params.len(), 1);
}

#[test]
fn connect_insertion_bare_port_overrides_parses() {
    let src = "connectrules cr; connect bridge logic, electrical; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    let rule = &cr.rules[0];
    assert_eq!(rule.connectmodule, Some("bridge".to_string()));
    assert_eq!(
        rule.overrides,
        Some(ConnectPortOverrides::Both("logic".to_string(), "electrical".to_string()))
    );
    assert_eq!(rule.disciplines, vec!["logic".to_string(), "electrical".to_string()]);
}

#[test]
fn connect_insertion_input_output_overrides_parses() {
    let src = "connectrules cr; connect bridge input logic, output electrical; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(
        cr.rules[0].overrides,
        Some(ConnectPortOverrides::InputOutput("logic".to_string(), "electrical".to_string()))
    );
}

#[test]
fn connect_insertion_output_input_overrides_parses() {
    let src = "connectrules cr; connect bridge output logic, input electrical; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(
        cr.rules[0].overrides,
        Some(ConnectPortOverrides::OutputInput("logic".to_string(), "electrical".to_string()))
    );
}

#[test]
fn connect_insertion_inout_overrides_parses() {
    let src = "connectrules cr; connect bridge inout logic, inout electrical; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(
        cr.rules[0].overrides,
        Some(ConnectPortOverrides::InOut("logic".to_string(), "electrical".to_string()))
    );
}

#[test]
fn connect_insertion_full_form_with_mode_params_and_overrides_parses() {
    // Every optional piece of Syntax 7-6 present at once.
    let src = "connectrules cr; connect bridge merged #(.res(50)) logic, electrical; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    let rule = &cr.rules[0];
    assert_eq!(rule.connectmodule, Some("bridge".to_string()));
    assert_eq!(rule.mode, Some(ConnectMode::Merged));
    assert_eq!(rule.params.len(), 1);
    assert_eq!(
        rule.overrides,
        Some(ConnectPortOverrides::Both("logic".to_string(), "electrical".to_string()))
    );
}

// ── connect_resolution, §7.7.2 Syntax 7-7 (LRMC-C7-04/C10-20, T33) ──────

#[test]
fn connect_resolution_two_discipline_form_parses() {
    let src = "connectrules cr; connect x, y resolveto x; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert!(cr.rules.is_empty());
    assert_eq!(cr.resolutions.len(), 1);
    let res = &cr.resolutions[0];
    assert_eq!(res.disciplines, vec!["x".to_string(), "y".to_string()]);
    assert_eq!(res.target, ConnectResolutionTarget::Discipline("x".to_string()));
}

#[test]
fn connect_resolution_three_discipline_form_parses() {
    // LRM §7.7.2 Example 1's own worked rule set.
    let src = "connectrules cr; connect x,y,a resolveto a; connect x,y resolveto x; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(cr.resolutions.len(), 2);
    assert_eq!(cr.resolutions[0].disciplines, vec!["x".to_string(), "y".to_string(), "a".to_string()]);
    assert_eq!(cr.resolutions[0].target, ConnectResolutionTarget::Discipline("a".to_string()));
    assert_eq!(cr.resolutions[1].disciplines, vec!["x".to_string(), "y".to_string()]);
    assert_eq!(cr.resolutions[1].target, ConnectResolutionTarget::Discipline("x".to_string()));
}

#[test]
fn connect_resolution_exclude_target_parses() {
    let src = "connectrules cr; connect x, y resolveto exclude; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(cr.resolutions[0].target, ConnectResolutionTarget::Exclude);
}

#[test]
fn legacy_bare_discipline_pair_shell_still_parses_unchanged() {
    // T11's pre-existing shell form (no connectmodule_identifier) must
    // keep working: `connectmodule` stays `None`.
    let src = "connectrules cr; connect electrical, logic; endconnectrules";
    let cr = grammar(src).parse_connectrules_decl().unwrap();
    assert_eq!(cr.rules[0].connectmodule, None);
    assert_eq!(cr.rules[0].disciplines, vec!["electrical".to_string(), "logic".to_string()]);
}

// ── config ─────────────────────────────────────────────────────────────

#[test]
fn config_declaration_parses_without_rejecting_its_body() {
    let src = "config cfg; design work.top; default liblist work; endconfig";
    let c = grammar(src).parse_config_decl().unwrap();
    assert_eq!(c.name, "cfg");
}

// ── parse_source_file: every Item kind reachable from one entry point ───

#[test]
fn source_file_parses_every_top_level_item_kind() {
    let src = "
        discipline Electrical; domain continuous; potential Voltage; flow Current; enddiscipline
        nature Voltage; units = \"V\"; endnature
        module resistor(inout electrical p, inout electrical n); endmodule
        paramset ps (resistor); parameter real r = 1.0; endparamset
        connectrules cr; connect electrical, electrical; endconnectrules
        config cfg; design work.top; endconfig
        primitive prim (q, a); output q; input a; table 0 : 0; endtable endprimitive
    ";
    let sf = grammar(src).parse_source_file().unwrap();
    assert_eq!(sf.items.len(), 7);
    assert!(matches!(sf.items[0], Item::Discipline(_)));
    assert!(matches!(sf.items[1], Item::Nature(_)));
    assert!(matches!(sf.items[2], Item::Module(_)));
    assert!(matches!(sf.items[3], Item::Paramset(_)));
    assert!(matches!(sf.items[4], Item::Connectrules(_)));
    assert!(matches!(sf.items[5], Item::Config(_)));
    assert!(matches!(sf.items[6], Item::Primitive(_)));
}

#[test]
fn empty_source_file_parses_to_zero_items() {
    let sf = grammar("").parse_source_file().unwrap();
    assert!(sf.items.is_empty());
}

// ── errors ────────────────────────────────────────────────────────────

#[test]
fn unterminated_udp_table_is_a_typed_eof_error_naming_endtable() {
    let mut g = grammar("primitive p (q, a); output q; input a; table 0 : 0;");
    let err = g.parse_primitive_decl().unwrap_err();
    match err.kind {
        piperine_lang::ParseErrorKind::UnexpectedEof { expected } => {
            assert_eq!(expected, "'endtable'");
        }
        other => panic!("expected UnexpectedEof, got {other:?}"),
    }
}

#[test]
fn unknown_top_level_item_is_a_typed_error() {
    let mut g = grammar("frobnicate x;");
    let err = g.parse_source_file().unwrap_err();
    assert!(matches!(err.kind, piperine_lang::ParseErrorKind::UnexpectedToken { .. }));
}
