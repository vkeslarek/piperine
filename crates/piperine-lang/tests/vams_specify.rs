//! LRM-COVERS: A.7.2, A.7.3, A.7.4
//! `specify` blocks (T15, LRMW3-06): real grammar productions for
//! `specparam` declarations and simple path declarations (parallel `=>`
//! and full `*>`, 1/2/3/6/12 delay values) — replacing the old
//! token-skipping stub. Grammar-level only: these tests read the parsed
//! AST directly (`Parser`), never elaboration — later tasks (T16 system
//! timing checks, T17 unsupported forms, T18 POM lowering) extend this
//! same target.

use piperine_lang::ast::{Item, ModuleItem, SpecifyBlock};
use piperine_lang::Parser;

fn parse_specify(module_src: &str) -> SpecifyBlock {
    let full = format!(
        "module top(a, y, clk, d);\n\
         inout a, y, clk, d;\n\
         {module_src}\n\
         endmodule\n"
    );
    let file = Parser::new().parse(&full).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    for item in &file.items {
        if let Item::Module(m) = item {
            for mi in &m.items {
                if let ModuleItem::Specify(sb) = mi {
                    return sb.clone();
                }
            }
        }
    }
    panic!("no specify block found in parsed module");
}

#[test]
fn parallel_path_with_two_delays_parses() {
    let sb = parse_specify("specify\n(a => y) = (1.0, 1.0);\nendspecify");
    assert_eq!(sb.paths.len(), 1);
    let p = &sb.paths[0];
    assert!(!p.full, "=> is the parallel form");
    assert_eq!(p.inputs, vec!["a".to_string()]);
    assert_eq!(p.outputs, vec!["y".to_string()]);
    assert_eq!(p.delays.len(), 2);
}

#[test]
fn full_connection_path_parses_input_and_output_lists() {
    let sb = parse_specify("specify\n(a, clk *> y, d) = 1.5;\nendspecify");
    assert_eq!(sb.paths.len(), 1);
    let p = &sb.paths[0];
    assert!(p.full, "*> is the full-connection form");
    assert_eq!(p.inputs, vec!["a".to_string(), "clk".to_string()]);
    assert_eq!(p.outputs, vec!["y".to_string(), "d".to_string()]);
    assert_eq!(p.delays.len(), 1);
}

#[test]
fn specparam_inside_specify_block_parses() {
    let sb = parse_specify("specify\nspecparam tr = 1.0;\nendspecify");
    assert_eq!(sb.specparams.len(), 1);
    assert_eq!(sb.specparams[0].names[0].0, "tr");
}

#[test]
fn specify_block_is_no_longer_an_unread_span() {
    // Every item recognized by the grammar lands on a typed list — the
    // old `items: vec![]` shape (a raw skipped span) is gone.
    let sb = parse_specify(
        "specify\n\
         specparam tr = 1.0;\n\
         (a => y) = (1.0, 1.0);\n\
         endspecify",
    );
    assert_eq!(sb.specparams.len() + sb.paths.len(), 2, "every specify_item lands on a typed list");
}

#[test]
fn one_and_six_delay_value_forms_parse() {
    let sb1 = parse_specify("specify\n(a => y) = 1.0;\nendspecify");
    assert_eq!(sb1.paths[0].delays.len(), 1);

    let sb6 = parse_specify("specify\n(a => y) = (1.0, 1.0, 1.0, 1.0, 1.0, 1.0);\nendspecify");
    assert_eq!(sb6.paths[0].delays.len(), 6);
}

#[test]
fn wrong_delay_count_fails_loud() {
    let full = "module top(a, y);\ninout a, y;\nspecify\n(a => y) = (1.0, 1.0, 1.0, 1.0);\nendspecify\nendmodule\n";
    let err = Parser::new().parse(full).expect_err("4 delays is not a valid A.7.4 count");
    let msg = err.to_string();
    assert!(msg.contains("A.7.4"), "error cites the production: {msg}");
}

#[test]
fn multiple_inputs_before_parallel_arrow_fails_loud() {
    let full = "module top(a, b, y);\ninout a, b, y;\nspecify\n(a, b => y) = 1.0;\nendspecify\nendmodule\n";
    let err = Parser::new().parse(full).expect_err("parallel form takes exactly one input");
    let msg = err.to_string();
    assert!(msg.contains("A.7.2"), "error cites the production: {msg}");
}
