//! LRM-COVERS: 6.3.1, 3.4.5
//! LRMC-C5-06 (`.specs/features/lrm-conformance-closure/`, T28): `defparam`
//! (§6.3.1) actually changes the named instance's effective parameter
//! value, addressed by hierarchical name from anywhere in the design (not
//! scoped to the declaring module's own instantiation subtree) — every
//! fixture below is §6.3.1's own worked example, simplified to drop the
//! `spice_nmos`/`spice_pmos` submodule layer the LRM's text never actually
//! needs for its own point (each override targets `m1`/`m2`'s own
//! `gate_width` parameter, not anything deeper).
//!
//! LRMW2-21 (`lrm-conformance-wave2`, T28): a `localparam` (§3.4.5) is never
//! overridable, including via `defparam` — `ApplyDefparams` must consult
//! `Param.is_local` and fail loud with the typed `DefparamOnLocalparam`
//! error kind (E4009), never silently write the override.

use piperine_lang::pom::{ElabError, ElabErrorKind, Value};
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// §6.3.1's own example structure: `tgate` instantiates `m1`/`m2`;
/// `annotate` (no structural relationship to `tgate`) overrides both via
/// `defparam`.
const FIXTURE: &str = r#"
    module mosp (inout electrical drain, inout electrical gate, inout electrical source);
        parameter real gate_length = 0.3e-6;
        parameter real gate_width = 4.0e-6;
    endmodule

    module mosn (inout electrical drain, inout electrical gate, inout electrical source);
        parameter real gate_length = 0.3e-6;
        parameter real gate_width = 4.0e-6;
    endmodule

    module tgate ();
        electrical io1, io2, control, control_bar;
        mosn m1 (io1, io2, control);
        mosp m2 (io1, io2, control_bar);
    endmodule

    module annotate ();
        defparam tgate.m1.gate_width = 5e-6;
        defparam tgate.m2.gate_width = 10e-6;
    endmodule
"#;

#[test]
fn defparam_changes_the_named_instance_effective_parameter_value() {
    let d = elab(FIXTURE);
    let tgate = d.module("tgate").expect("tgate elaborated");
    let m1 = tgate.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    let m2 = tgate.instances.iter().find(|i| i.label.as_deref() == Some("m2")).expect("instance m2");
    assert_eq!(
        m1.params.iter().find(|(n, _)| n == "gate_width").map(|(_, v)| v.clone()),
        Some(Value::Real(5e-6)),
        "defparam tgate.m1.gate_width = 5e-6 must override m1's effective gate_width"
    );
    assert_eq!(
        m2.params.iter().find(|(n, _)| n == "gate_width").map(|(_, v)| v.clone()),
        Some(Value::Real(10e-6)),
        "defparam tgate.m2.gate_width = 10e-6 must override m2's effective gate_width"
    );
}

#[test]
fn defparam_only_touches_instance_param_values_not_pom_structure() {
    // MD-25: defparam writes Instance::params only -- the authored
    // instance/submodule structure (instance count, labels, module types,
    // port connections) must be exactly what the source declares, before
    // and after defparam application.
    let d = elab(FIXTURE);
    let tgate = d.module("tgate").expect("tgate elaborated");
    assert_eq!(tgate.instances.len(), 2, "defparam must not add or remove instances");
    let m1 = tgate.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    let m2 = tgate.instances.iter().find(|i| i.label.as_deref() == Some("m2")).expect("instance m2");
    assert_eq!(m1.module, "mosn", "m1's module type must be untouched");
    assert_eq!(m2.module, "mosp", "m2's module type must be untouched");
    assert_eq!(m1.ports.len(), 3, "m1's port connections must be untouched");
    assert_eq!(m2.ports.len(), 3, "m2's port connections must be untouched");
    assert_eq!(d.module("mosn").expect("mosn elaborated").params.len(), 2, "mosn's own param declarations are untouched");
    assert_eq!(d.module("mosp").expect("mosp elaborated").params.len(), 2, "mosp's own param declarations are untouched");
}

#[test]
fn defparam_naming_a_nonexistent_root_module_fails_loud() {
    let src = r#"
        module annotate ();
            defparam nonexistent_module.inst.p = 1.0;
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("nonexistent_module"), "names the missing module: {msg}");
}

#[test]
fn defparam_naming_a_nonexistent_instance_label_fails_loud() {
    let src = r#"
        module leaf ();
            parameter real p = 1.0;
        endmodule
        module tgate ();
            leaf l1 ();
        endmodule
        module annotate ();
            defparam tgate.nonexistent_instance.p = 2.0;
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("nonexistent_instance"), "names the missing instance: {msg}");
}

#[test]
fn defparam_target_missing_a_hierarchical_dot_fails_loud() {
    let src = r#"
        module annotate ();
            defparam bare_name = 1.0;
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("hierarchical"), "names the malformed target: {msg}");
}

/// LRMW2-21 / §3.4.5: `defparam` onto a `localparam` fails loud, by the
/// typed `DefparamOnLocalparam` error kind (E4009) -- never `is_err()`.
///
/// The target module (`holder`) instantiates `leaf`, whose own
/// `fixed_width` is declared `localparam` -- unreachable via any
/// instantiation override, and per §3.4.5 unreachable via `defparam` too.
#[test]
fn defparam_onto_a_localparam_fails_with_typed_error_kind() {
    let fixture = r#"
        module leaf ();
            localparam real fixed_width = 4.0e-6;
        endmodule
        module holder ();
            leaf leaf_inst ();
        endmodule
        module annotate ();
            defparam holder.leaf_inst.fixed_width = 9.0e-6;
        endmodule
    "#;
    let report = parse_and_elaborate(fixture, &SourceMap::dummy()).expect_err("defparam onto localparam must fail loud");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    match &err.kind {
        ElabErrorKind::DefparamOnLocalparam { target, param, .. } => {
            assert_eq!(target, "holder.leaf_inst.fixed_width", "error names the full defparam target");
            assert_eq!(param, "fixed_width", "error names the localparam being targeted");
        }
        other => panic!("expected DefparamOnLocalparam, got: {other:?}"),
    }
    assert!(report.to_string().contains("§3.4.5"), "error message cites the LRM clause: {report}");
}

/// Wave-1 regression (LRMC-C5-06): `defparam` onto a normal (non-local)
/// `parameter` must still succeed after the T28 `is_local` gate is added.
#[test]
fn defparam_onto_a_normal_parameter_still_works() {
    let d = elab(FIXTURE);
    let tgate = d.module("tgate").expect("tgate elaborated");
    let m1 = tgate.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    assert_eq!(
        m1.params.iter().find(|(n, _)| n == "gate_width").map(|(_, v)| v.clone()),
        Some(Value::Real(5e-6)),
        "defparam onto a normal parameter must still override its effective value"
    );
}
