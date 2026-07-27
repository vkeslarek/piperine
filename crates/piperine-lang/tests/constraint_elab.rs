//! dv-core T8 (DVC-04, DVC-07) — constraint elaboration: every reference
//! resolves against the module the block attaches to (nets, params,
//! module vars, block `var`s, child instance ports through the authored
//! tree), predicates and comparisons type-check, and each spec'd
//! violation fails loud naming the reference and the use site — one
//! negative fixture per clause.

use piperine_lang::parse::ast::ConstraintStmt;
use piperine_lang::pom::ValueType;
use piperine_lang::{parse_and_elaborate, parse_str, SourceMap};

fn elaborate(src: &str) -> Result<piperine_lang::pom::Design, miette::Report> {
    parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &SourceMap::dummy())
}

fn elab_err(src: &str) -> String {
    elaborate(src).map(|_| ()).expect_err("fixture must fail elaboration").to_string()
}

const HARNESS: &str = "
mod Mos ( inout d : Electrical, inout s : Electrical ) {
    param vds_max : Real = 3.3;
    wire int_n : Electrical;
}
mod Top ( inout p : Electrical, inout gnd : Electrical ) {
    m1 : Mos ( p, gnd );
}";

/// DVC-04 AC1: a block with all three statement forms elaborates, and the
/// module's POM node carries it with the statements in authored form and
/// the resolved `var` type in the side table.
#[test]
fn constraint_block_elaborates_onto_the_module() {
    let design = elaborate(&format!(
        "{HARNESS}\n\
         constraint Top {{\n\
             require vds_ok : V(m1.d, m1.s) <= m1.vds_max;\n\
             var margin_v : Real = m1.vds_max - V(m1.d, m1.s);\n\
             target ac_gain(V(p, gnd)) >= 1.0 tol 0.1;\n\
         }}"
    ))
    .expect("a well-formed constraint block must elaborate");
    let module = design.module("Top").expect("Top elaborated");
    assert_eq!(module.constraints.len(), 1, "exactly one block attaches");
    let block = &module.constraints[0];
    assert_eq!(block.name(), "Top");
    assert_eq!(block.stmts().len(), 3);
    assert!(
        matches!(&block.stmts()[0], ConstraintStmt::Require { name, .. } if name == "vds_ok"),
        "statements keep authored form and order: {:?}",
        block.stmts()
    );
    assert!(
        matches!(&block.stmts()[2], ConstraintStmt::Target { tol: Some(_), .. }),
        "the target keeps its tol scale: {:?}",
        block.stmts()[2]
    );
    assert_eq!(
        block.var_type("margin_v"),
        Some(&ValueType::Real),
        "the var's resolved annotation type lives in the side table"
    );
}

/// DVC-07 AC5: a child instance's **port** net (`V(m1.d, m1.s)`) and a
/// child instance's param (`m1.vds_max`) resolve through the authored
/// instance tree — covered by the positive fixture above, asserted here
/// against the elaborated design directly.
#[test]
fn child_instance_port_nets_resolve() {
    let design = elaborate(&format!(
        "{HARNESS}\n\
         constraint Top {{ require vds_ok : V(m1.d, m1.s) <= m1.vds_max; }}"
    ))
    .expect("a child instance's port nets must resolve");
    let block = &design.module("Top").expect("Top elaborated").constraints[0];
    assert_eq!(block.stmts().len(), 1);
}

/// DVC-04 AC2: an unknown net in a probe fails loud, naming the reference
/// and the use site.
#[test]
fn unknown_net_fails_loud_naming_reference_and_use_site() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{ require vds_ok : V(m1.d, nosuch) <= 1.0; }}"
    ));
    assert!(msg.contains("nosuch"), "error names the reference: {msg}");
    assert!(msg.contains("require `vds_ok`"), "error names the use site: {msg}");
}

/// DVC-04 AC2: an unknown child-instance **port** fails loud, naming the
/// reference and the use site.
#[test]
fn unknown_instance_port_fails_loud() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{ require vds_ok : V(m1.gate, m1.s) <= 1.0; }}"
    ));
    assert!(msg.contains("gate"), "error names the reference: {msg}");
    assert!(msg.contains("m1"), "error names the instance: {msg}");
    assert!(msg.contains("require `vds_ok`"), "error names the use site: {msg}");
}

/// DVC-04 AC2: an unknown instance path fails loud, naming it and the use
/// site.
#[test]
fn unknown_instance_path_fails_loud() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{ require vds_ok : V(m9.d, m9.s) <= 1.0; }}"
    ));
    assert!(msg.contains("m9"), "error names the instance path: {msg}");
    assert!(msg.contains("require `vds_ok`"), "error names the use site: {msg}");
}

/// DVC-04 AC2: an unknown `var` (or any unknown bare name) fails loud,
/// naming the reference and the use site.
#[test]
fn unknown_var_fails_loud() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{ require ok : missing >= 0.0; }}"
    ));
    assert!(msg.contains("missing"), "error names the reference: {msg}");
    assert!(msg.contains("require `ok`"), "error names the use site: {msg}");
}

/// DVC-07 AC5: a child's **internal** node fails loud — it crosses
/// encapsulation.
#[test]
fn child_internal_node_fails_loud() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{ require bad : V(m1.int_n, m1.s) <= 1.0; }}"
    ));
    assert!(msg.contains("int_n"), "error names the internal node: {msg}");
    assert!(
        msg.contains("internal") && msg.contains("encapsulation"),
        "error explains the encapsulation crossing: {msg}"
    );
}

/// DVC-04 AC4: a non-Boolean `require` predicate fails loud.
#[test]
fn non_boolean_require_predicate_fails_loud() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{ require bad : m1.vds_max + 1.0; }}"
    ));
    assert!(msg.contains("Boolean"), "error names the expected type: {msg}");
    assert!(msg.contains("require `bad`"), "error names the use site: {msg}");
}

/// DVC-04 AC4: a non-`Real` `target` operand fails loud. A `Bit` port
/// reads as `Boolean`, so `target d >= 1.0` is the fixture.
#[test]
fn non_real_target_operand_fails_loud() {
    let msg = elab_err(
        "mod Dig ( input d : Bit ) { }\n\
         constraint Dig { target d >= 1.0; }",
    );
    assert!(msg.contains("Real"), "error names the expected type: {msg}");
}

/// DVC-04 AC4: a `var` without its type annotation fails loud — at parse
/// time, exactly as every other `var` in the language (T6's parser
/// fixture); the elaboration-level proof is that no such statement can
/// reach elaboration.
#[test]
fn var_without_type_annotation_fails_loud() {
    let err = parse_str("constraint M { var x = 1.0; }")
        .expect_err("a var without its type annotation must not parse");
    assert!(err.byte_offset().is_some(), "error points at the offending token");
}

/// DVC-04 AC6: two statements sharing one label in a block fail loud —
/// including a `var`/`require` collision (one namespace per block).
#[test]
fn duplicate_statement_label_fails_loud() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{\n\
             var vds_ok : Real = 1.0;\n\
             require vds_ok : V(p, gnd) >= 0.0;\n\
         }}"
    ));
    assert!(msg.contains("duplicate") && msg.contains("vds_ok"), "error names the duplicated label: {msg}");
}

/// DVC-04 AC1: a `constraint` block naming a module that does not exist
/// fails loud, naming it.
#[test]
fn constraint_on_unknown_module_fails_loud() {
    let msg = elab_err("constraint Ghost { require ok : 1.0 >= 0.0; }");
    assert!(msg.contains("Ghost"), "error names the unknown module: {msg}");
}

/// DVC-09 (elaboration half of T7's guard): an undeclared helper *used in
/// a constraint* fails loud naming it — the end-to-end fixture T7
/// deferred to this task.
#[test]
fn undeclared_helper_in_a_constraint_fails_loud() {
    let msg = elab_err(&format!(
        "{HARNESS}\n\
         constraint Top {{ require ok : bw(V(p, gnd)) >= 1.0; }}"
    ));
    assert!(msg.contains("bw"), "error names the undeclared helper: {msg}");
}
