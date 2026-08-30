//! LRM-COVERS: 3.13, 3.13.1, 3.13.2, 3.13.3, 3.13.4
//!
//! lrm-conformance-wave3 (Phase 6, T39): §3.13 "Namespace" and its four
//! subsections.
//!
//! §3.13.1 ("Nature and discipline" — "identifiers defined as natures or
//! disciplines have a global scope"): `natures_and_disciplines_have_
//! global_scope_across_modules` — one `nature`/`discipline` pair declared
//! once, used to type nets in two independent modules with no
//! re-declaration.
//!
//! §3.13.2 ("Access functions" — "automatically added to that module's
//! name space UNLESS there is another identifier defined with the same
//! name"): `a_local_variable_shadows_a_custom_access_function_name` — a
//! nature's own `access = myacc;` name, re-declared as a plain `real`
//! variable inside the module using it, is a legal local variable, not a
//! collision. (The universal `V`/`I` access functions are grammar
//! productions, not identifiers — `module dut(); real V; endmodule` is a
//! *parse* error, not a namespace question this clause's shadowing rule
//! addresses; only a discipline's own `access` name is a genuine
//! identifier that can be shadowed.)
//!
//! §3.13.3/§3.13.4 ("Net"/"Branch" — "can only be declared inside a
//! module scope; a net/branch can not be declared local to a block"):
//! `a_net_declared_inside_an_analog_block_is_a_parse_error` and
//! `a_branch_declared_inside_an_analog_block_is_a_parse_error` — the
//! Annex A grammar itself only has a net/branch declaration production
//! at the module-item level, so a block-local attempt is a structural
//! parse failure, not merely an unenforced rule.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy()).expect_err("expected a parse/elaboration error").to_string()
}

#[test]
fn natures_and_disciplines_have_global_scope_across_modules() {
    let src = "
        nature Voltage;
            units = \"V\";
            access = V;
        endnature
        discipline elec;
            potential Voltage;
        enddiscipline
        module a (inout elec p);
        endmodule
        module b (inout elec p, inout elec q);
        endmodule
        module top ();
            elec x, y, z;
            a a1 (x);
            b b1 (y, z);
        endmodule
    ";
    let d = elab(src);
    assert!(d.module("a").is_some() && d.module("b").is_some(), "both modules elaborate against the one global `elec` discipline");
}

#[test]
fn a_local_variable_shadows_a_custom_access_function_name() {
    let src = "
        nature Voltage;
            units = \"V\";
            access = myacc;
        endnature
        discipline elec;
            potential Voltage;
        enddiscipline
        module dut (inout elec a);
            real myacc;
            analog begin
                myacc = 3.0;
                V(a) <+ myacc;
            end
        endmodule
    ";
    // Must elaborate: `myacc` the local real variable shadows `myacc`
    // the discipline's own access-function name in this module's
    // namespace, per §3.13.2's own rule.
    let _ = elab(src);
}

#[test]
fn a_net_declared_inside_an_analog_block_is_a_parse_error() {
    let src = "module dut ();\nanalog begin\nelectrical a;\nend\nendmodule\n";
    let err = elab_err(src);
    assert!(!err.is_empty());
}

#[test]
fn a_branch_declared_inside_an_analog_block_is_a_parse_error() {
    let src = "module dut (inout electrical p, inout electrical n);\nanalog begin\nbranch (p, n) br;\nend\nendmodule\n";
    let err = elab_err(src);
    assert!(!err.is_empty());
}
