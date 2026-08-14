//! VAMS T23 (VAMS-01, VAMS-02) — lowering the continuous-domain
//! declarations: `nature` (incl. derived chains, §3.6.1), `discipline`
//! (§3.6.2), and named/implicit `branch`es resolved from access-function
//! call sites inside `analog` behaviors (§5.4).

use piperine_lang::pom::BranchKind;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

const VOLTAGE_CURRENT: &str = r#"
nature Voltage;
    units = "V";
    abstol = 1e-6;
    access = V;
endnature

nature Current;
    units = "A";
    abstol = 1e-12;
    access = I;
endnature
"#;

#[test]
fn base_nature_lowers_its_standard_attributes() {
    let d = elab(&format!("{VOLTAGE_CURRENT}\nmodule empty(); endmodule\n"));
    let v = d.nature("Voltage").expect("Voltage nature elaborated");
    assert_eq!(v.units.as_deref(), Some("V"));
    assert_eq!(v.abstol, Some(1e-6));
    assert_eq!(v.access.as_deref(), Some("V"));
    assert_eq!(v.parent, None);
}

#[test]
fn derived_nature_inherits_unset_attributes_from_its_parent() {
    let src = format!(
        "{VOLTAGE_CURRENT}\nnature LogicVoltage : Voltage;\n    access = LV;\nendnature\nmodule empty(); endmodule\n"
    );
    let d = elab(&src);
    let lv = d.nature("LogicVoltage").expect("LogicVoltage elaborated");
    // Overrides its own `access`...
    assert_eq!(lv.access.as_deref(), Some("LV"));
    // ...but inherits abstol/units from Voltage (not re-declared here).
    assert_eq!(lv.abstol, Some(1e-6));
    assert_eq!(lv.units.as_deref(), Some("V"));
    assert_eq!(lv.parent.as_deref(), Some("Voltage"));
}

#[test]
fn discipline_lowers_with_its_potential_and_flow_natures() {
    let src = format!("{VOLTAGE_CURRENT}\ndiscipline electrical;\n    potential Voltage;\n    flow Current;\nenddiscipline\nmodule empty(); endmodule\n");
    let d = elab(&src);
    let disc = d.discipline("electrical").expect("electrical discipline elaborated");
    assert_eq!(disc.potential.as_deref(), Some("Voltage"));
    assert_eq!(disc.flow.as_deref(), Some("Current"));
}

#[test]
fn discipline_binding_an_undeclared_nature_fails_loud_citing_the_clause() {
    let src = "
        discipline electrical;
            potential Voltage;
        enddiscipline
        module empty(); endmodule
    ";
    let msg = elab_err(src);
    assert!(msg.contains("Voltage"), "names the undeclared nature: {msg}");
}

#[test]
fn named_branch_declaration_lowers_as_a_probe_until_contributed() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            branch (p, n) br;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let br = m.branch("br").expect("br declared");
    assert_eq!(br.kind(), BranchKind::Probe, "no contribution seen yet");
    assert_eq!(br.pos().net(), "p");
    assert_eq!(br.neg().map(|n| n.net()), Some("n"));
}

#[test]
fn contribution_through_a_named_branch_sets_its_kind() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            branch (p, n) br;
            analog begin
                V(br) <+ 1.0;
            end
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.branch("br").expect("br").kind(), BranchKind::Potential);
}

#[test]
fn implicit_branch_created_from_a_two_terminal_access_function() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            analog begin
                I(p, n) <+ 1.0;
            end
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.branches().len(), 1, "one implicit branch created");
    let br = &m.branches()[0];
    assert!(br.is_implicit());
    assert_eq!(br.kind(), BranchKind::Flow);
    assert_eq!(br.pos().net(), "p");
    assert_eq!(br.neg().map(|n| n.net()), Some("n"));
}

#[test]
fn conflicting_potential_and_flow_contributions_on_one_branch_fail_loud() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            branch (p, n) br;
            analog begin
                V(br) <+ 1.0;
                I(br) <+ 1.0;
            end
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("br"), "names the conflicting branch: {msg}");
}

// ── LRMC-C1-09: custom discipline access-name resolution (§4.4/§5.4.2/§5.4.3) ──

const THERMAL: &str = r#"
nature Temperature;
    units = "K";
    abstol = 1e-4;
    access = Temp;
endnature

nature Power;
    units = "W";
    abstol = 1e-9;
    access = Pwr;
endnature

discipline thermal;
    potential Temperature;
    flow Power;
enddiscipline
"#;

#[test]
fn custom_discipline_accessor_resolves_potential_via_named_branch() {
    // headers/disciplines.vams's own thermal discipline: `Temp` is the
    // potential accessor (Annex D.1), not `V` — a non-electrical
    // discipline entirely, exercising the access_kinds lookup (LRMC-C1-09)
    // rather than the hardcoded V/I/potential/flow path.
    let d = elab(&format!(
        "{THERMAL}\nmodule pkg (inout thermal p, inout thermal n);\n\
         branch (p, n) br;\nanalog begin\nTemp(br) <+ 300.0;\nend\nendmodule\n"
    ));
    let m = d.module("pkg").expect("pkg elaborated");
    assert_eq!(m.branch("br").expect("br").kind(), BranchKind::Potential);
}

#[test]
fn custom_discipline_accessor_resolves_flow_via_implicit_branch() {
    let d = elab(&format!(
        "{THERMAL}\nmodule pkg (inout thermal p, inout thermal n);\n\
         analog begin\nPwr(p, n) <+ 1.0;\nend\nendmodule\n"
    ));
    let m = d.module("pkg").expect("pkg elaborated");
    assert_eq!(m.branches().len(), 1, "one implicit branch created");
    let br = &m.branches()[0];
    assert!(br.is_implicit());
    assert_eq!(br.kind(), BranchKind::Flow);
}

#[test]
fn branch_kind_conflict_fires_for_a_non_electrical_discipline_too() {
    // §5.4.2's conflict rule is discipline-agnostic; before LRMC-C1-09 the
    // conflict check only ever ran for the hardcoded V/I names, so a
    // thermal-discipline conflict silently passed. Same fixture shape as
    // `conflicting_potential_and_flow_contributions_on_one_branch_fail_loud`,
    // over `Temp`/`Pwr` instead of `V`/`I`.
    let src = format!(
        "{THERMAL}\nmodule pkg (inout thermal p, inout thermal n);\n\
         branch (p, n) br;\nanalog begin\nTemp(br) <+ 300.0;\nPwr(br) <+ 1.0;\nend\nendmodule\n"
    );
    let msg = elab_err(&src);
    assert!(msg.contains("br"), "names the conflicting branch: {msg}");
}
