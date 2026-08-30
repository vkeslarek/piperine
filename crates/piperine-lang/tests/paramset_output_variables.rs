//! LRM-COVERS: 6.4.3
//!
//! lrm-conformance-wave3 T28: a `paramset`-declared variable with a
//! description is an output variable of instances using that paramset
//! (§6.4.3, building on §3.2.1's `desc`/`units` output-variable rule,
//! T27) — LRM's own worked example, adapted:
//!
//! ```text
//! paramset smnpn npn;
//!     (*desc="cut-off frequency"*) real ft;
//!     ft = .gm/(2*3.14159*(.cpi + .cmu));
//! endparamset
//! ```
//!
//! "If the module `npn` had an output variable named `ft`, the
//! paramset's output variable would replace it" (rule 1); "If a paramset
//! variable without a description has the same name as an output
//! variable of the module, the module output variable of that name
//! shall not be available" (rule 2).

use piperine_lang::pom::Value;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

const NPN: &str = r#"
    module npn (inout electrical c, inout electrical b, inout electrical e);
        parameter real is = 1e-16;
        (* desc="transit frequency" *) real ft;
        real gm;
        real cpi;
        real cmu;
        analog begin
            gm = is;
            cpi = 1e-12;
            cmu = 1e-13;
            ft = 0.0;
            I(c, e) <+ gm * V(b, e);
        end
    endmodule
"#;

/// AC1 (T28 Done-when #1): `(* desc="cut-off frequency" *) real ft;` in a
/// paramset is readable as an opvar on an instance using that paramset —
/// the POM half (`Instance::paramset_output_vars`); codegen's own
/// `crates/piperine-codegen/tests/paramset_output_variable.rs` proves the
/// opvar-reporting half.
#[test]
fn described_paramset_variable_survives_to_instance_paramset_output_vars() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            (* desc=\"cut-off frequency\" *) real ft;
            ft = .gm/(2*3.14159*(.cpi + .cmu));
        endparamset
        module top ();
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let q1 = top.instance("q1").expect("q1 instantiated");
    assert_eq!(q1.module_name(), "npn", "paramset resolves to its associated module");
    let ft = q1.paramset_output_vars().iter().find(|v| v.name == "ft").expect("ft recorded");
    assert!(ft.described, "ft carries a desc attribute, so it is an LRM output variable");
}

/// AC4 (T28 Done-when #4): `.gm` (module-output reference) resolves
/// inside the paramset expression — elaboration accepts it and records
/// it as part of `ft`'s formula (proved observably, not just parsed, by
/// the module-var-name validation below and by the codegen test that
/// evaluates it).
#[test]
fn module_output_reference_resolves_inside_the_paramset_expression() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            (* desc=\"cut-off frequency\" *) real ft;
            ft = .gm/(2*3.14159*(.cpi + .cmu));
        endparamset
        module top ();
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    // Elaborating at all proves `.gm`/`.cpi`/`.cmu` resolved against
    // `npn`'s own declared vars — an unresolvable dotted reference fails
    // loud (next test).
    let _ = elab(&src);
}

/// A `.field` naming no variable of the associated module fails loud,
/// citing §6.4.3 — proves the resolution above is a real check, not a
/// no-op accept-everything.
#[test]
fn undeclared_module_output_reference_fails_loud() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            (* desc=\"cut-off frequency\" *) real ft;
            ft = .nonexistent_var;
        endparamset
        module top ();
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    let msg = elab_err(&src);
    assert!(msg.contains("nonexistent_var"), "error must name the bad reference, got: {msg}");
    assert!(msg.contains("6.4.3"), "error must cite §6.4.3, got: {msg}");
}

/// AC2 (T28 Done-when #2): a paramset output variable **replaces** a
/// same-named module output variable — `npn` already declares
/// `(* desc="transit frequency" *) real ft;`; the paramset's own `ft`
/// (a different description) is the one recorded for `q1`, and there is
/// exactly one `ft` entry (not two).
#[test]
fn paramset_output_variable_replaces_the_module_output_variable() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            (* desc=\"cut-off frequency\" *) real ft;
            ft = .gm/(2*3.14159*(.cpi + .cmu));
        endparamset
        module top ();
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let q1 = top.instance("q1").expect("q1 instantiated");
    let ft_entries: Vec<_> = q1.paramset_output_vars().iter().filter(|v| v.name == "ft").collect();
    assert_eq!(ft_entries.len(), 1, "exactly one `ft` entry — the paramset's own, replacing the module's");
    assert!(ft_entries[0].described);
}

/// AC3 (T28 Done-when #3): an undescribed paramset variable **hides**
/// the module's same-named output variable — `npn` declares
/// `(* desc="transit frequency" *) real ft;`; the paramset declares a
/// *bare* `real ft;` (no description) with no assignment. `ft` still
/// appears in `paramset_output_vars` (so the hiding is recorded) but
/// `described` is `false`.
#[test]
fn undescribed_paramset_variable_hides_the_module_output_variable() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            real ft;
        endparamset
        module top ();
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let q1 = top.instance("q1").expect("q1 instantiated");
    let ft = q1.paramset_output_vars().iter().find(|v| v.name == "ft").expect("ft recorded (hides the module's own)");
    assert!(!ft.described, "an undescribed paramset variable hides, but is not itself an output variable");
}

/// The paramset's own declared-parameter values are captured for the
/// formula's plain (non-dotted) identifiers to resolve against.
#[test]
fn paramset_parameter_values_are_captured_for_the_formula() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            parameter real scale = 2.0;
            (* desc=\"scaled ft\" *) real ft;
            ft = scale * .gm;
        endparamset
        module top ();
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let q1 = top.instance("q1").expect("q1 instantiated");
    let ft = q1.paramset_output_vars().iter().find(|v| v.name == "ft").expect("ft recorded");
    assert_eq!(ft.param_env.get("scale"), Some(&Value::Real(2.0)), "the paramset's own `scale` value is captured");
}

/// A formula assigning a variable never declared in the paramset body
/// fails loud, citing §6.4.3.
#[test]
fn assigning_an_undeclared_paramset_variable_fails_loud() {
    let src = format!(
        "{NPN}
        paramset smnpn (npn);
            never_declared = 1.0;
        endparamset
        module top ();
            smnpn q1 (c, b, e);
        endmodule
        "
    );
    let msg = elab_err(&src);
    assert!(msg.contains("never_declared"), "error must name the undeclared variable, got: {msg}");
    assert!(msg.contains("6.4.3"), "error must cite §6.4.3, got: {msg}");
}
