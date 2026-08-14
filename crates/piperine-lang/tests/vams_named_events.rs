//! LRMC-C5-04 (`.specs/features/lrm-conformance-closure/`, T25): FORM half
//! — named event declarations (`event e;`, §5.10.4/Annex A.2.1.3) reach
//! `Module::events()` as an authored declaration list (MD-25: navigable
//! exactly as written), and `-> e;` parses in both analog and digital
//! statement positions. Runtime triggering is T26's own test surface.

use piperine_lang::ast::Stmt;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

#[test]
fn event_declaration_reaches_module_as_an_authored_declaration() {
    let d = elab(
        r#"
        module dut (inout electrical p, inout electrical n);
            event my_event;
            analog begin
                -> my_event;
            end
        endmodule
        "#,
    );
    let m = d.module("dut").expect("dut elaborated");
    assert_eq!(m.events(), &["my_event".to_string()]);
}

#[test]
fn multiple_events_in_one_declaration_all_reach_the_module() {
    let d = elab(
        r#"
        module dut (inout electrical p, inout electrical n);
            event e1, e2, e3;
        endmodule
        "#,
    );
    let m = d.module("dut").expect("dut elaborated");
    assert_eq!(m.events(), &["e1".to_string(), "e2".to_string(), "e3".to_string()]);
}

#[test]
fn event_trigger_parses_in_an_analog_statement_position() {
    let d = elab(
        r#"
        module dut (inout electrical p, inout electrical n);
            event e;
            analog begin
                -> e;
            end
        endmodule
        "#,
    );
    let m = d.module("dut").expect("dut elaborated");
    let analog = m.behaviors().iter().find(|b| b.is_analog()).expect("an analog behavior");
    assert!(
        analog.body().iter().any(|s| matches!(s, Stmt::EventTrigger { name, .. } if name == "e")),
        "the analog body must contain the -> e; trigger statement"
    );
}

#[test]
fn event_trigger_parses_in_a_digital_statement_position() {
    let d = elab(
        r#"
        discipline logic;
            domain discrete;
        enddiscipline
        module dut (input logic clk);
            event e;
            always @(posedge clk) begin
                -> e;
            end
        endmodule
        "#,
    );
    let m = d.module("dut").expect("dut elaborated");
    let digital = m.behaviors().iter().find(|b| b.is_digital()).expect("a digital behavior");
    let Stmt::EventControl { body, .. } = &digital.body()[0] else {
        panic!("expected the always block's EventControl statement");
    };
    let Stmt::Block { stmts, .. } = body.as_ref() else {
        panic!("expected a begin/end block");
    };
    assert!(
        stmts.iter().any(|s| matches!(s, Stmt::EventTrigger { name, .. } if name == "e")),
        "the digital always block must contain the -> e; trigger statement"
    );
}

#[test]
fn triggering_an_undeclared_event_fails_loud() {
    let src = r#"
        module dut (inout electrical p, inout electrical n);
            analog begin
                -> undeclared_event;
            end
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("undeclared_event"), "names the undeclared event: {msg}");
}
