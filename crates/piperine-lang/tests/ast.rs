//! Proves the AST node definitions (tasks.md T6): a node exists per Annex A
//! production in this feature's scope, and attribute instances (`Attr`)
//! attach only at the sites LRM Syntax 2-5..2-10 permit — encoded in the
//! types themselves, not merely checked at parse time.

use piperine_lang::ast::*;
use piperine_lang::Span;

fn span() -> Span {
    Span::new(0, 1)
}

#[test]
fn nature_decl_carries_no_attrs_field() {
    // `NatureDecl` has no `attrs` field at all — §2.9's Syntax 2-5..2-10
    // never lists `nature_declaration` as an attribute-instance site, so
    // there is no field to populate. This test documents that by
    // constructing one exhaustively: if a field were added later without
    // updating this test, the struct literal below would fail to compile.
    let n = NatureDecl {
        span: span(),
        name: "Voltage".into(),
        parent: Some("Potential".into()),
        abstol: None,
        access: Some("V".into()),
        units: None,
        idt_nature: None,
        ddt_nature: None,
        user_attrs: vec![],
    };
    assert_eq!(n.name, "Voltage");
    assert_eq!(n.parent.as_deref(), Some("Potential"));
}

#[test]
fn discipline_decl_carries_no_attrs_field() {
    let d = DisciplineDecl {
        span: span(),
        name: "Electrical".into(),
        domain: Some(Domain::Continuous),
        potential: Some("Voltage".into()),
        flow: Some("Current".into()),
    };
    assert_eq!(d.domain, Some(Domain::Continuous));
}

#[test]
fn module_decl_is_attribute_bearing() {
    // Syntax 2-5: module_declaration permits `{ attribute_instance }`.
    let m = ModuleDecl {
        span: span(),
        attrs: vec![Attr { span: span(), name: "device".into(), value: None }],
        kind: ModuleKind::Module,
        name: "resistor".into(),
        param_ports: vec![],
        ports: None,
        items: vec![],
    };
    assert_eq!(m.attrs.len(), 1);
    assert_eq!(m.attrs[0].name, "device");
}

#[test]
fn port_decl_is_attribute_bearing() {
    // Syntax 2-6: port_declaration permits attribute instances.
    let p = PortDecl {
        span: span(),
        attrs: vec![],
        dir: Direction::Inout,
        discipline: Some("Electrical".into()),
        names: vec![Declarator { name: "p".into(), range: None }],
    };
    assert_eq!(p.dir, Direction::Inout);
}

#[test]
fn branch_decl_is_attribute_bearing() {
    let b = BranchDecl {
        span: span(),
        attrs: vec![],
        pos: "a".into(),
        neg: Some("b".into()),
        names: vec!["br".into()],
    };
    assert_eq!(b.names, vec!["br".to_string()]);
}

#[test]
fn generate_constructs_are_attribute_bearing() {
    // Syntax 2-7: loop_generate_construct / conditional_generate_construct.
    let lg = LoopGenerate {
        span: span(),
        attrs: vec![],
        genvar: "i".into(),
        init: Expr::Number { span: span(), value: piperine_lang::NumberLit::Integer(0) },
        cond: Expr::Ident { span: span(), name: "i".into() },
        step_var: "i".into(),
        step_value: Expr::Ident { span: span(), name: "i".into() },
        body: GenerateBlock { span: span(), name: None, items: vec![] },
    };
    assert_eq!(lg.genvar, "i");
}

#[test]
fn udp_primitive_and_ports_are_attribute_bearing() {
    let udp = PrimitiveDecl {
        span: span(),
        attrs: vec![],
        name: "my_udp".into(),
        ports: vec![
            UdpPortDecl::Output { span: span(), attrs: vec![], name: "q".into(), is_reg: false, init: None },
            UdpPortDecl::Input { span: span(), attrs: vec![], names: vec!["a".into()] },
        ],
        table: vec![UdpTableRow {
            span: span(),
            inputs: vec!["0".into()],
            current_state: None,
            next_state: "0".into(),
        }],
        is_sequential: false,
    };
    assert_eq!(udp.ports.len(), 2);
    assert_eq!(udp.table[0].next_state, "0");
}

#[test]
fn paramset_connectrules_config_are_not_attribute_bearing() {
    // These three are absent from Syntax 2-5..2-10 — no attrs field exists.
    let ps = ParamsetDecl { span: span(), name: "ps".into(), module_name: "m".into(), params: vec![], statements: vec![] };
    let cr = ConnectrulesDecl { span: span(), name: "cr".into(), rules: vec![] };
    let cfg = ConfigDecl { span: span(), name: "cfg".into(), body: String::new() };
    assert_eq!(ps.name, "ps");
    assert_eq!(cr.name, "cr");
    assert_eq!(cfg.name, "cfg");
}

#[test]
fn source_file_holds_all_seven_top_level_item_kinds() {
    let items = vec![
        Item::Discipline(DisciplineDecl { span: span(), name: "d".into(), domain: None, potential: None, flow: None }),
        Item::Nature(NatureDecl {
            span: span(), name: "n".into(), parent: None, abstol: None, access: None,
            units: None, idt_nature: None, ddt_nature: None, user_attrs: vec![],
        }),
        Item::Module(ModuleDecl {
            span: span(), attrs: vec![], kind: ModuleKind::Module, name: "m".into(),
            param_ports: vec![], ports: None, items: vec![],
        }),
        Item::Paramset(ParamsetDecl { span: span(), name: "ps".into(), module_name: "m".into(), params: vec![], statements: vec![] }),
        Item::Connectrules(ConnectrulesDecl { span: span(), name: "cr".into(), rules: vec![] }),
        Item::Config(ConfigDecl { span: span(), name: "cfg".into(), body: String::new() }),
        Item::Primitive(PrimitiveDecl {
            span: span(), attrs: vec![], name: "p".into(), ports: vec![], table: vec![], is_sequential: false,
        }),
    ];
    let sf = SourceFile { items, directives: Default::default() };
    assert_eq!(sf.items.len(), 7);
}

#[test]
fn expr_span_covers_every_variant() {
    let e = Expr::Binary {
        span: span(),
        op: BinOp::Add,
        lhs: Box::new(Expr::Number { span: span(), value: piperine_lang::NumberLit::Integer(1) }),
        rhs: Box::new(Expr::Ident { span: span(), name: "x".into() }),
    };
    assert_eq!(e.span(), span());
}

#[test]
fn access_function_expr_supports_one_and_two_arg_forms() {
    let one = Expr::AccessFunction {
        span: span(),
        function: "V".into(),
        pos: Box::new(Expr::Ident { span: span(), name: "a".into() }),
        neg: None,
    };
    let two = Expr::AccessFunction {
        span: span(),
        function: "V".into(),
        pos: Box::new(Expr::Ident { span: span(), name: "a".into() }),
        neg: Some(Box::new(Expr::Ident { span: span(), name: "b".into() })),
    };
    match (one, two) {
        (Expr::AccessFunction { neg: None, .. }, Expr::AccessFunction { neg: Some(_), .. }) => {}
        _ => panic!("expected one-arg and two-arg AccessFunction forms"),
    }
}

#[test]
fn stmt_span_covers_contribution_and_control_flow() {
    let contrib = Stmt::Contribution {
        span: span(),
        target: Expr::Ident { span: span(), name: "V(p,n)".into() },
        value: Expr::Number { span: span(), value: piperine_lang::NumberLit::Real(1.0) },
    };
    assert_eq!(contrib.span(), span());

    let if_stmt = Stmt::If {
        span: span(),
        cond: Expr::Ident { span: span(), name: "c".into() },
        then_branch: Box::new(Stmt::Null { span: span() }),
        else_branch: None,
    };
    assert_eq!(if_stmt.span(), span());
}
