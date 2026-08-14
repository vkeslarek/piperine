//! Proves the behavioral/structural declaration grammar (tasks.md T10):
//! branch declarations, analog/analog-initial/initial/always constructs,
//! analog function/function/task, genvar/generate (for/if/case), module
//! instantiation (named+ordered connections, parameter overrides,
//! defparam) and continuous assign.

use piperine_lang::ast::*;
use piperine_lang::{Grammar, Lexer};

fn module_items(src: &str) -> Vec<ModuleItem> {
    let full = format!("module m(); {src} endmodule");
    let toks = Lexer::new(&full).tokenize().unwrap_or_else(|e| panic!("lex error: {e}"));
    let m = Grammar::new(toks).parse_module_decl().unwrap_or_else(|e| panic!("parse error for {src:?}: {e}"));
    m.items
}

fn one_item(src: &str) -> ModuleItem {
    let mut items = module_items(src);
    assert_eq!(items.len(), 1, "expected exactly one module item for {src:?}, got {items:?}");
    items.remove(0)
}

// ── branch, §5.2 ─────────────────────────────────────────────────────────

#[test]
fn named_two_terminal_branch_parses() {
    match one_item("branch (p, n) res_br;") {
        ModuleItem::Branch(b) => {
            assert_eq!(b.pos, "p");
            assert_eq!(b.neg.as_deref(), Some("n"));
            assert_eq!(b.names, vec!["res_br".to_string()]);
        }
        other => panic!("expected ModuleItem::Branch, got {other:?}"),
    }
}

#[test]
fn ground_referenced_branch_has_no_neg_terminal() {
    match one_item("branch (p) gnd_br;") {
        ModuleItem::Branch(b) => assert!(b.neg.is_none()),
        other => panic!("expected ModuleItem::Branch, got {other:?}"),
    }
}

#[test]
fn branch_declares_multiple_names() {
    match one_item("branch (p, n) br1, br2;") {
        ModuleItem::Branch(b) => assert_eq!(b.names.len(), 2),
        other => panic!("expected ModuleItem::Branch, got {other:?}"),
    }
}

// ── behavior constructs, §5.7 ─────────────────────────────────────────────

#[test]
fn analog_construct_parses() {
    match one_item("analog V(p,n) <+ 1.0;") {
        ModuleItem::Behavior(b) => {
            assert_eq!(b.kind, BehaviorKind::Analog);
            assert!(matches!(*b.body, piperine_lang::ast::Stmt::Contribution { .. }));
        }
        other => panic!("expected ModuleItem::Behavior, got {other:?}"),
    }
}

#[test]
fn analog_initial_construct_parses() {
    match one_item("analog initial x = 1;") {
        ModuleItem::Behavior(b) => assert_eq!(b.kind, BehaviorKind::AnalogInitial),
        other => panic!("expected ModuleItem::Behavior, got {other:?}"),
    }
}

#[test]
fn initial_and_always_constructs_parse() {
    match one_item("initial x = 1;") {
        ModuleItem::Behavior(b) => assert_eq!(b.kind, BehaviorKind::Initial),
        other => panic!("expected ModuleItem::Behavior, got {other:?}"),
    }
    match one_item("always @(posedge clk) q <= d;") {
        ModuleItem::Behavior(b) => assert_eq!(b.kind, BehaviorKind::Always),
        other => panic!("expected ModuleItem::Behavior, got {other:?}"),
    }
}

// ── analog function / function / task, §5.9/Annex A.2.6/A.2.7 ────────────

#[test]
fn analog_function_with_ports_and_return_type_parses() {
    let src = "analog function real gain; input x; real x; gain = x * 2.0; endfunction";
    match one_item(src) {
        ModuleItem::Function(f) => {
            assert_eq!(f.kind, FunctionKind::Analog);
            assert_eq!(f.name, "gain");
            assert_eq!(f.ret, Some(ParamType::Real));
            assert_eq!(f.ports.len(), 1);
            assert_eq!(f.ports[0].dir, Direction::Input);
            assert_eq!(f.decls.len(), 1, "the local `real x;` declaration");
        }
        other => panic!("expected ModuleItem::Function, got {other:?}"),
    }
}

#[test]
fn digital_function_without_analog_prefix_parses() {
    let src = "function integer add; input a, b; begin add = a + b; end endfunction";
    match one_item(src) {
        ModuleItem::Function(f) => assert_eq!(f.kind, FunctionKind::Digital),
        other => panic!("expected ModuleItem::Function, got {other:?}"),
    }
}

#[test]
fn task_with_input_and_output_ports_parses() {
    let src = "task swap; input a; output b; begin b = a; end endtask";
    match one_item(src) {
        ModuleItem::Task(t) => {
            assert_eq!(t.name, "swap");
            assert_eq!(t.ports.len(), 2);
            assert_eq!(t.ports[1].dir, Direction::Output);
        }
        other => panic!("expected ModuleItem::Task, got {other:?}"),
    }
}

// ── genvar / generate, chapter 6 ──────────────────────────────────────────

#[test]
fn genvar_declares_multiple_names() {
    match one_item("genvar i, j;") {
        ModuleItem::Genvar(g) => assert_eq!(g.names, vec!["i".to_string(), "j".to_string()]),
        other => panic!("expected ModuleItem::Genvar, got {other:?}"),
    }
}

#[test]
fn loop_generate_parses_bounds_and_named_block_body() {
    let src = "genvar i; for (i = 0; i < 4; i = i + 1) begin : g electrical n; end";
    let items = module_items(src);
    match &items[1] {
        ModuleItem::LoopGenerate(lg) => {
            assert_eq!(lg.genvar, "i");
            assert_eq!(lg.step_var, "i");
            assert_eq!(lg.body.name.as_deref(), Some("g"));
            assert_eq!(lg.body.items.len(), 1);
        }
        other => panic!("expected ModuleItem::LoopGenerate, got {other:?}"),
    }
}

#[test]
fn loop_generate_single_item_body_without_begin_end_parses() {
    let src = "genvar i; for (i = 0; i < 4; i = i + 1) electrical n;";
    let items = module_items(src);
    match &items[1] {
        ModuleItem::LoopGenerate(lg) => {
            assert!(lg.body.name.is_none());
            assert_eq!(lg.body.items.len(), 1);
        }
        other => panic!("expected ModuleItem::LoopGenerate, got {other:?}"),
    }
}

#[test]
fn if_generate_with_else_parses_both_branches() {
    let src = "if (1) begin electrical a; end else begin electrical b; end";
    match one_item(src) {
        ModuleItem::IfGenerate(ig) => {
            assert_eq!(ig.then_branch.items.len(), 1);
            assert!(ig.else_branch.is_some());
        }
        other => panic!("expected ModuleItem::IfGenerate, got {other:?}"),
    }
}

#[test]
fn case_generate_parses_arms_and_default() {
    let src = "case (1) 0: begin electrical a; end default: begin electrical b; end endcase";
    match one_item(src) {
        ModuleItem::CaseGenerate(cg) => {
            assert_eq!(cg.arms.len(), 2);
            assert!(cg.arms[0].labels.is_some());
            assert!(cg.arms[1].labels.is_none());
        }
        other => panic!("expected ModuleItem::CaseGenerate, got {other:?}"),
    }
}

#[test]
fn generate_region_parses_multiple_items() {
    let src = "generate electrical a; electrical b; endgenerate";
    match one_item(src) {
        ModuleItem::GenerateRegion(gr) => assert_eq!(gr.items.len(), 2),
        other => panic!("expected ModuleItem::GenerateRegion, got {other:?}"),
    }
}

// ── instantiation, defparam, continuous assign ────────────────────────────

#[test]
fn module_instantiation_with_named_connections_parses() {
    let src = "resistor r1(.p(a), .n(b));";
    match one_item(src) {
        ModuleItem::Instantiation(inst) => {
            assert_eq!(inst.module_name, "resistor");
            assert_eq!(inst.instances.len(), 1);
            assert_eq!(inst.instances[0].name, "r1");
            assert_eq!(inst.instances[0].connections.len(), 2);
            assert_eq!(inst.instances[0].connections[0].name.as_deref(), Some("p"));
        }
        other => panic!("expected ModuleItem::Instantiation, got {other:?}"),
    }
}

#[test]
fn module_instantiation_with_ordered_connections_and_multiple_instances_parses() {
    let src = "resistor r1(a, b), r2(b, c);";
    match one_item(src) {
        ModuleItem::Instantiation(inst) => {
            assert_eq!(inst.instances.len(), 2);
            assert!(inst.instances[0].connections[0].name.is_none());
        }
        other => panic!("expected ModuleItem::Instantiation, got {other:?}"),
    }
}

#[test]
fn module_instantiation_with_parameter_overrides_parses() {
    let src = "resistor #(.r(10e3)) r1(a, b);";
    match one_item(src) {
        ModuleItem::Instantiation(inst) => {
            assert_eq!(inst.param_overrides.len(), 1);
            assert_eq!(inst.param_overrides[0].name.as_deref(), Some("r"));
        }
        other => panic!("expected ModuleItem::Instantiation, got {other:?}"),
    }
}

#[test]
fn net_declaration_still_dispatches_correctly_alongside_instantiation() {
    // Regression guard for the net-decl-vs-instantiation dispatch
    // ambiguity: a plain `electrical p, n;` must still be a NetDecl, not
    // be mistaken for an instantiation.
    match one_item("electrical p, n;") {
        ModuleItem::Net(n) => assert_eq!(n.names.len(), 2),
        other => panic!("expected ModuleItem::Net, got {other:?}"),
    }
}

#[test]
fn defparam_parses_target_and_value() {
    match one_item("defparam r1.r = 5e3;") {
        ModuleItem::Defparam(d) => assert_eq!(d.target, "r1.r"),
        other => panic!("expected ModuleItem::Defparam, got {other:?}"),
    }
}

#[test]
fn continuous_assign_parses() {
    match one_item("assign y = a & b;") {
        ModuleItem::ContinuousAssign(ca) => {
            assert!(matches!(ca.target, Expr::Ident { .. }));
            assert!(matches!(ca.value, Expr::Binary { .. }));
        }
        other => panic!("expected ModuleItem::ContinuousAssign, got {other:?}"),
    }
}

// ── attribute instances on the new sites, §Syntax 2-7 ─────────────────────

#[test]
fn branch_attribute_instance_parses() {
    match one_item("(* probe *) branch (p, n) br;") {
        ModuleItem::Branch(b) => assert_eq!(b.attrs.len(), 1),
        other => panic!("expected ModuleItem::Branch, got {other:?}"),
    }
}

// ── errors ─────────────────────────────────────────────────────────────

#[test]
fn unterminated_function_is_a_typed_error_naming_endfunction() {
    // The source truly ends instead of closing with `endfunction`;
    // `expect_kw` reports it as an UnexpectedToken naming what it wanted
    // and that end-of-file is what it found (the same descriptive pattern
    // as the T7 unclosed-paren case), not a bare "ran out of input".
    let full = "module m(); function integer f; input a; begin f = a; end";
    let toks = Lexer::new(full).tokenize().unwrap();
    let mut g = Grammar::new(toks);
    let err = g.parse_module_decl().unwrap_err();
    match err.kind {
        piperine_lang::ParseErrorKind::UnexpectedToken { expected, found } => {
            assert_eq!(expected, "'endfunction'");
            assert_eq!(found, "end of file");
        }
        other => panic!("expected UnexpectedToken, got {other:?}"),
    }
}
