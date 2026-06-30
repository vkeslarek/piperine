use piperine_ams::Document;

#[test]
fn a10_elsif_directive_selects_correct_branch() {
    let src_branch_b = r#"
`define B
`ifdef A
  module PickA (a, c); inout a, c; endmodule
`elsif B
  module PickB (a, c); inout a, c; endmodule
`else
  module PickC (a, c); inout a, c; endmodule
`endif
"#;
    let doc_b = Document::parse(src_branch_b).expect("`elsif with B defined parses");
    let names_b: Vec<&str> = doc_b.modules.iter().map(|m| m.name.as_str()).collect();
    assert!(
        names_b.contains(&"PickB"),
        "A.10: with B defined, branch-B `module PickB` should be emitted; got {names_b:?}"
    );
    assert!(
        !names_b.contains(&"PickA") && !names_b.contains(&"PickC"),
        "A.10: only one branch should emit; got {names_b:?}"
    );

    let src_branch_a = r#"
`define A
`ifdef A
  module PickA (a, c); inout a, c; endmodule
`elsif B
  module PickB (a, c); inout a, c; endmodule
`else
  module PickC (a, c); inout a, c; endmodule
`endif
"#;
    let doc_a = Document::parse(src_branch_a).expect("`elsif with A defined parses");
    let names_a: Vec<&str> = doc_a.modules.iter().map(|m| m.name.as_str()).collect();
    assert!(
        names_a.contains(&"PickA"),
        "A.10: with A defined, branch-A wins; got {names_a:?}"
    );
    assert!(
        !names_a.contains(&"PickB") && !names_a.contains(&"PickC"),
        "A.10: only one branch should emit; got {names_a:?}"
    );

    let src_branch_c = r#"
`ifdef A
  module PickA (a, c); inout a, c; endmodule
`elsif B
  module PickB (a, c); inout a, c; endmodule
`else
  module PickC (a, c); inout a, c; endmodule
`endif
"#;
    let doc_c = Document::parse(src_branch_c).expect("`elsif with neither parses");
    let names_c: Vec<&str> = doc_c.modules.iter().map(|m| m.name.as_str()).collect();
    assert!(
        names_c.contains(&"PickC"),
        "A.10: with neither A nor B, `else wins; got {names_c:?}"
    );
    assert!(
        !names_c.contains(&"PickA") && !names_c.contains(&"PickB"),
        "A.10: only one branch should emit; got {names_c:?}"
    );
}