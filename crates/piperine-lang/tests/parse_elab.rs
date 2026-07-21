use piperine_lang::parse_str;
use piperine_lang::parse::ast::*;
use piperine_lang::parse::ast::SourceFile;

/// Flatten a SourceFile into categorized lists (replaces the old model::Document).
struct Document {
    pub uses: Vec<Path>,
    pub modules: Vec<ModuleDeclaration>,
    pub behaviors: Vec<BehaviorDecl>,
    pub disciplines: Vec<DisciplineDecl>,
    pub bundles: Vec<BundleDecl>,
    pub enums: Vec<EnumDecl>,
    pub capabilities: Vec<CapabilityDecl>,
    pub impls: Vec<ImplDecl>,
    pub functions: Vec<FnDecl>,
    pub consts: Vec<ConstDecl>,
}

impl Document {
    fn from_ast(source: SourceFile) -> Self {
        let mut doc = Document {
            uses: vec![], modules: vec![], behaviors: vec![], disciplines: vec![],
            bundles: vec![], enums: vec![], capabilities: vec![],
            impls: vec![], functions: vec![], consts: vec![],
        };
        for item in source.items {
            match item {
                Item::ModuleDeclaration(m)        => doc.modules.push(m),
                Item::BehaviorDecl(b)   => doc.behaviors.push(b),
                Item::DisciplineDecl(d) => doc.disciplines.push(d),
                Item::BundleDecl(b)     => doc.bundles.push(b),
                Item::EnumDecl(e)       => doc.enums.push(e),
                Item::CapabilityDecl(c) => doc.capabilities.push(c),
                Item::ImplDecl(i)       => doc.impls.push(i),
                Item::FnDecl(f)         => doc.functions.push(f),
                Item::UseDecl(u)        => doc.uses.push(u),
                Item::ConstDecl(c)      => doc.consts.push(c),
                // Grammar only (declared-language-surface Phase 1) — not yet
                // consumed by this test's flattened `Document` view.
                Item::ExternDecl(_)     => {}
            }
        }
        doc
    }
}
use std::fs;
use std::path::PathBuf;

// ─────────────────────────── Integration: parse all example files ───────────

#[test]
fn test_parse_all_examples() {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests/examples");

    let mut count = 0;
    for entry in fs::read_dir(d).expect("Failed to read examples dir") {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("phdl") {
            let content = fs::read_to_string(&path).expect("Failed to read file");
            match parse_str(&content) {
                Ok(ast) => {
                    assert!(!ast.items.is_empty(), "AST is empty for {:?}", path);
                    // Also test model flattening.
                    let doc = Document::from_ast(ast);
                    let _ = doc; // ensure it doesn't panic
                    count += 1;
                }
                Err(err) => {
                    panic!("Failed to parse {:?}: {}", path, err);
                }
            }
        }
    }
    assert!(count >= 14, "Expected at least 14 example files, found {}", count);
}

// ─────────────────────────── Structural: core.phdl ─────────────────────────

#[test]
fn test_core_structure() {
    let src = include_str!("examples/core.phdl");
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);

    // Disciplines
    assert!(doc.disciplines.len() >= 1, "Expected at least 1 discipline");
    assert_eq!(doc.disciplines[0].name, "Electrical");

    // Modules: Resistor, Capacitor, VSource, Diode, Comparator, BitToVoltage
    assert!(doc.modules.len() >= 4, "Expected at least 4 modules, got {}", doc.modules.len());
    let mod_names: Vec<&str> = doc.modules.iter().map(|m| m.name.as_str()).collect();
    assert!(mod_names.contains(&"Resistor"));
    assert!(mod_names.contains(&"Capacitor"));
    assert!(mod_names.contains(&"VSource"));
    assert!(mod_names.contains(&"Diode"));

    // Behaviors
    assert!(doc.behaviors.len() >= 4, "Expected at least 4 behaviors, got {}", doc.behaviors.len());

    // Functions
    assert!(doc.functions.len() >= 1, "Expected at least 1 function");
    assert_eq!(doc.functions[0].sig.name, "thermal_voltage");
}

// ─────────────────────────── Structural: SAR ADC ───────────────────────────

#[test]
fn test_sar_adc_structure() {
    let src = include_str!("examples/sar_adc.phdl");
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);

    // Enum
    assert_eq!(doc.enums.len(), 1);
    assert_eq!(doc.enums[0].name, "SarState");
    assert_eq!(doc.enums[0].variants.len(), 3);
    assert!(doc.enums[0].repr.is_some());

    // Modules: Dac, SarAdc
    assert!(doc.modules.len() >= 2);
    let dac = doc.modules.iter().find(|m| m.name == "Dac").unwrap();
    assert_eq!(dac.const_params, vec!["N"]);
    assert_eq!(dac.ports.len(), 3);

    let sar = doc.modules.iter().find(|m| m.name == "SarAdc").unwrap();
    assert_eq!(sar.const_params, vec!["N"]);
    assert!(sar.ports.len() >= 5); // clk, start, vin, gnd, result, done

    // Behaviors: analog Dac, analog SarAdc, digital SarAdc
    assert!(doc.behaviors.len() >= 3);
    let analog_behaviors: Vec<_> = doc.behaviors.iter()
        .filter(|b| b.kind == BehaviorKind::Analog)
        .collect();
    let digital_behaviors: Vec<_> = doc.behaviors.iter()
        .filter(|b| b.kind == BehaviorKind::Digital)
        .collect();
    assert!(analog_behaviors.len() >= 2);
    assert!(digital_behaviors.len() >= 1);
}

// ─────────────────────────── Structural: capabilities ──────────────────────

#[test]
fn test_capabilities_structure() {
    let src = include_str!("examples/capabilities.phdl");
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);

    // Capabilities: Add, Sub, Mul, Number
    assert!(doc.capabilities.len() >= 3);
    let cap_names: Vec<&str> = doc.capabilities.iter().map(|c| c.name.as_str()).collect();
    assert!(cap_names.contains(&"Add"));
    assert!(cap_names.contains(&"Number"));

    // Number has supers
    let number = doc.capabilities.iter().find(|c| c.name == "Number").unwrap();
    assert_eq!(number.supers, vec!["Add", "Sub", "Mul"]);

    // Bundles: Pair, UInt
    assert!(doc.bundles.len() >= 2);
    let uint = doc.bundles.iter().find(|b| b.name == "UInt").unwrap();
    assert_eq!(uint.const_params, vec!["N"]);

    // Impls
    assert!(doc.impls.len() >= 1);
    let add_impl = doc.impls.iter().find(|i| i.capability == Some("Add".into())).unwrap();
    assert_eq!(add_impl.ty, "UInt");
}

// ─────────────────────────── Structural: delta sigma ───────────────────────

#[test]
fn test_delta_sigma_structure() {
    let src = include_str!("examples/delta_sigma.phdl");
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);

    // Module
    assert_eq!(doc.modules.len(), 1);
    assert_eq!(doc.modules[0].name, "DeltaSigma");

    // Has both analog and digital behaviors
    assert_eq!(doc.behaviors.len(), 2);
    let kinds: Vec<_> = doc.behaviors.iter().map(|b| &b.kind).collect();
    assert!(kinds.contains(&&BehaviorKind::Analog));
    assert!(kinds.contains(&&BehaviorKind::Digital));
}

// ─────────────────────────── Structural: electrothermal ────────────────────

#[test]
fn test_electrothermal_structure() {
    let src = include_str!("examples/electrothermal.phdl");
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);

    // Two disciplines
    assert_eq!(doc.disciplines.len(), 1);
    assert_eq!(doc.disciplines[0].name, "Thermal");

    // Module with 3 ports (p, n, th)
    assert_eq!(doc.modules.len(), 1);
    assert_eq!(doc.modules[0].name, "HeatedResistor");
    assert_eq!(doc.modules[0].ports.len(), 3);
}

// ─────────────────────────── BUG-4 fix: Connection stores both sides ──────

#[test]
fn test_connection_stores_both_sides() {
    let src = r#"
mod Test ( inout a : Electrical, inout b : Electrical ) {
    wire node : Electrical[3];
    node[0] = a;
    node[2] = b;
}
"#;
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);
    let module = &doc.modules[0];

    let connections: Vec<_> = module.body.iter().filter_map(|s| {
        if let ModuleStatement::Connection { lhs, rhs, .. } = s {
            Some((lhs, rhs))
        } else { None }
    }).collect();

    assert_eq!(connections.len(), 2, "Expected 2 connections");

    // First: node[0] = a → lhs is Index(Ident("node"), Int(0)), rhs is Ident("a")
    match connections[0].0 {
        Expr::Index(base, idx) => {
            assert!(matches!(base.as_ref(), Expr::Ident(n) if n == "node"));
            assert!(matches!(idx.as_ref(), Expr::Literal(Literal::Int(0))));
        }
        _ => panic!("Expected Index expr for LHS, got {:?}", connections[0].0),
    }
    assert!(matches!(connections[0].1, Expr::Ident(n) if n == "a"));
}

// ─────────────────────────── BUG-5/6 fix: array instance names ────────────

#[test]
fn test_named_array_instance() {
    let src = r#"
mod Test ( inout a : Electrical, inout gnd : Ground ) {
    wire tap : Electrical[4];
    for i in 0..4 {
        rseg[i] : Resistor ( a, tap[i] ) { .r = 100.0 };
    }
}
"#;
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);
    let module = &doc.modules[0];

    // Find the for loop
    let for_stmt = module.body.iter().find(|s| matches!(s, ModuleStatement::StructuralFor { .. })).unwrap();
    if let ModuleStatement::StructuralFor { body, .. } = for_stmt {
        assert_eq!(body.len(), 1);
        if let ModuleStatement::Instance { name, array_index, module: mod_name, .. } = &body[0] {
            assert_eq!(name.as_deref(), Some("rseg"));
            assert!(array_index.is_some(), "array_index should be Some");
            assert_eq!(mod_name, "Resistor");
        } else {
            panic!("Expected Instance");
        }
    }
}

// ─────────────────────────── BUG-3 fix: inclusive slices ───────────────────

#[test]
fn test_inclusive_slice() {
    let src = r#"
fn test_fn(xs: UInt[8]) -> UInt[4] {
    return xs[0..=3];
}
"#;
    let ast = parse_str(src).unwrap();
    if let Item::FnDecl(f) = &ast.items[0] {
        assert_eq!(f.sig.name, "test_fn");
        // The body should contain a return with a slice expression.
        if let Some(Stmt::Return(expr)) = f.body.stmts.first() {
            if let Expr::Slice(_, range) = expr {
                assert!(range.inclusive, "Expected inclusive range ..=");
            } else {
                panic!("Expected Slice expr, got {:?}", expr);
            }
        }
    }
}

// ─────────────────────────── BUG-7 fix: event body is Block ───────────────

#[test]
fn test_event_body_is_block() {
    let src = r#"
mod Sync ( input d : Bit, input clk : Bit, output q : Bit ) { var m : Bit = 0; }
digital Sync {
    q <- m;
    @ posedge(clk) { m = d; }
}
"#;
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);
    let behavior = &doc.behaviors[0];
    let event = behavior.body.iter().find(|s| matches!(s, Stmt::Event { .. })).unwrap();
    if let Stmt::Event { body, .. } = event {
        // body is now a Block, not Vec<Stmt>
        assert!(!body.stmts.is_empty());
    }
}

// ─────────────────────────── BUG-8 fix: operator precedence ───────────────

#[test]
fn test_precedence_bitor_lowest() {
    // a | b & c should parse as a | (b & c) since & binds tighter than |
    let src = r#"
fn test_fn(a: Boolean, b: Boolean, c: Boolean) -> Boolean {
    return a | b & c;
}
"#;
    let ast = parse_str(src).unwrap();
    if let Item::FnDecl(f) = &ast.items[0] {
        if let Some(Stmt::Return(expr)) = f.body.stmts.first() {
            // Should be Binary(a, BitOr, Binary(b, BitAnd, c))
            if let Expr::Binary(lhs, BinaryOp::BitOr, rhs) = expr {
                assert!(matches!(lhs.as_ref(), Expr::Ident(n) if n == "a"));
                assert!(matches!(rhs.as_ref(), Expr::Binary(_, BinaryOp::BitAnd, _)));
            } else {
                panic!("Expected BitOr at top level, got {:?}", expr);
            }
        }
    }
}

// ─────────────────────────── BUG-9 fix: block expression ──────────────────

#[test]
fn test_block_expression() {
    let src = r#"
fn test_fn() -> Real {
    var x : Real = { var y : Real = 1.0; y };
    return x;
}
"#;
    let ast = parse_str(src).unwrap();
    if let Item::FnDecl(f) = &ast.items[0] {
        if let Some(Stmt::VarDecl { default: Some(expr), .. }) = f.body.stmts.first() {
            assert!(matches!(expr, Expr::Block(_)), "Expected Block expr, got {:?}", expr);
        }
    }
}

// ─────────────────────────── BUG-11 fix: <+ and <- in Stmt/Block ──────────

#[test]
fn test_contrib_force_in_event_block() {
    let src = r#"
mod LcTank ( inout p : Electrical, inout n : Electrical ) { param l : Real = 1.0e-6; }
analog LcTank {
    I(p, n) <+ c * ddt(V(p, n));
    @ initial { V(p, n) = 1.0; }
}
"#;
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);
    let behavior = &doc.behaviors[0];

    // The main body should have a Bind with Contrib
    let bind = behavior.body.iter().find(|s| matches!(s, Stmt::Bind { op: BindOp::Contrib, .. }));
    assert!(bind.is_some(), "Expected a <+ contribution in behavior body");

    // The event body should parse successfully (@ initial { V(p, n) = 1.0; })
    let event = behavior.body.iter().find(|s| matches!(s, Stmt::Event { .. }));
    assert!(event.is_some(), "Expected an event block");
}

// ─────────────────────────── BUG-2 fix: error on empty radix ──────────────

#[test]
fn test_error_on_empty_binary_literal() {
    // 0b with no digits should produce an error, not a panic.
    let result = parse_str("mod X () { param p : Natural = 0b; }");
    assert!(result.is_err(), "Expected error for empty binary literal");
}

// ─────────────────────────── BUG-10 fix: :: in postfix ────────────────────

#[test]
fn test_path_in_expression() {
    let src = r#"
fn test_fn() -> Complex {
    var c : Complex = Complex::polar(1.0, 0.5);
    return c;
}
"#;
    let ast = parse_str(src).unwrap();
    if let Item::FnDecl(f) = &ast.items[0] {
        if let Some(Stmt::VarDecl { default: Some(expr), .. }) = f.body.stmts.first() {
            // Should be Call(Path(Complex::polar), [1.0, 0.5])
            if let Expr::Call(callee, args) = expr {
                assert!(matches!(callee.as_ref(), Expr::Path(p) if p.segments == vec!["Complex", "polar"]));
                assert_eq!(args.len(), 2);
            } else {
                panic!("Expected Call expr, got {:?}", expr);
            }
        }
    }
}

// ─────────────────────────── Error cases ──────────────────────────────────

#[test]
fn test_error_on_empty_file() {
    let ast = parse_str("").unwrap();
    assert!(ast.items.is_empty());
}

#[test]
fn test_error_on_malformed_module() {
    let result = parse_str("mod { }");
    assert!(result.is_err());
}

#[test]
fn test_default_param_must_be_trailing() {
    // the language spec Part I §9.1: a non-defaulted parameter cannot follow a defaulted
    // one — defaults are trailing-only.
    let result = parse_str("fn bad(x: Real = 1.0, y: Real) -> Real { x }");
    let err = result.expect_err("expected a parse error for a non-trailing default");
    assert!(
        err.to_string().contains("non-defaulted parameter cannot follow a defaulted one"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_default_param_parses_when_trailing() {
    let ast = parse_str("fn good(x: Real, y: Real = 1.0) -> Real { x }").unwrap();
    let f = ast
        .items
        .iter()
        .find_map(|i| if let Item::FnDecl(f) = i { Some(f) } else { None })
        .expect("fn parsed");
    let params: Vec<_> = f.sig.params.iter().collect();
    assert_eq!(params.len(), 2);
    assert!(matches!(params[0], FnParam::Typed { default: None, .. }));
    assert!(matches!(params[1], FnParam::Typed { default: Some(_), .. }));
}

#[test]
fn test_error_on_missing_semicolon() {
    let result = parse_str("use foo::bar");
    assert!(result.is_err());
}

// ─────────────────────────── Use declarations ─────────────────────────────

#[test]
fn test_use_declarations() {
    let src = r#"
use devices::passives::Resistor;
use std::math;
mod X ();
"#;
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);
    assert_eq!(doc.uses.len(), 2);
    assert_eq!(doc.uses[0].segments, vec!["devices", "passives", "Resistor"]);
    assert_eq!(doc.uses[1].segments, vec!["std", "math"]);
}

// ─────────────────────────── Language features file ───────────────────────

#[test]
fn test_language_features_structure() {
    let src = include_str!("examples/language_features.phdl");
    let ast = parse_str(src).unwrap();
    let doc = Document::from_ast(ast);

    // Disciplines
    assert!(doc.disciplines.len() >= 2); // Bit, Logic

    // Enums
    assert!(doc.enums.len() >= 3); // SwState, Phase, OpCode
    let phase = doc.enums.iter().find(|e| e.name == "Phase").unwrap();
    assert_eq!(phase.variants.len(), 4);

    // Bundles
    assert!(doc.bundles.len() >= 4); // FilterSpec, DiffPair, Stream, Complex

    // Functions (map, reduce)
    assert!(doc.functions.len() >= 2);

    // Modules (RcChain)
    assert!(doc.modules.len() >= 1);
}
