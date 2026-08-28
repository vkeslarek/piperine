//! LRM-COVERS: A.1.5, 3.4.4
//! Three silent no-op arms in `elab_module`'s `ModuleItem` match (and one
//! in `register_items`'s `Item` match) stop vanishing (T22, LRMW3-08 /
//! spec P2 AC4): a `config` declaration, a module-scope `specparam`
//! (outside a `specify` block), and an in-module `task` declaration each
//! fail loud citing their Annex A clause and naming a replacement —
//! `ElabErrorKind::UnsupportedConstruct`, the same variant T17 introduced
//! for unlowerable `specify` forms.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected an elaboration error")
        .to_string()
}

#[test]
fn config_declaration_fails_loud_citing_a_1_5_and_naming_piperine_toml() {
    let src = r#"
module leaf (inout electrical p, inout electrical n);
endmodule
config cfg1;
    design leaf;
endconfig
"#;
    let msg = elab_err(src);
    assert!(msg.contains("config"), "names the construct: {msg}");
    assert!(msg.contains("cfg1"), "names the specific config declaration: {msg}");
    assert!(msg.contains("A.1.5"), "cites the production: {msg}");
    assert!(
        msg.contains("Piperine.toml") || msg.contains("SourceMap"),
        "names the replacement per spec P2 AC4: {msg}"
    );
}

#[test]
fn module_scope_specparam_fails_loud_citing_its_clause() {
    let src = "
module leaf (inout electrical p, inout electrical n);
    specparam tr = 1.0;
endmodule
";
    let msg = elab_err(src);
    assert!(msg.contains("specparam"), "names the construct: {msg}");
    assert!(msg.contains("tr"), "names the specific specparam: {msg}");
    assert!(msg.contains("3.4.4"), "cites the clause: {msg}");
    assert!(msg.contains("specify"), "names the replacement (a specify-block specparam): {msg}");
}

#[test]
fn in_module_task_declaration_fails_loud_citing_its_clause() {
    let src = "
module leaf (inout electrical p, inout electrical n);
    task t1;
        ;
    endtask
endmodule
";
    let msg = elab_err(src);
    assert!(msg.contains("task"), "names the construct: {msg}");
    assert!(msg.contains("t1"), "names the specific task: {msg}");
    assert!(msg.contains("A.2.7"), "cites the clause: {msg}");
}

/// The regression the task's own Done-when demands: no silent `=> {}`
/// arm remains for any of the three constructs. Each fixture above
/// already proves this by content (not by reading the match arms), but a
/// module with *none* of these constructs must still elaborate cleanly —
/// the fail-loud checks must not have become false-positive rejections.
#[test]
fn a_module_with_none_of_the_three_still_elaborates_cleanly() {
    let src = "
module leaf (inout electrical p, inout electrical n);
    analog V(p, n) <+ 1.0;
endmodule
";
    parse_and_elaborate(src, &SourceMap::dummy()).expect("a module using none of the three constructs elaborates");
}
