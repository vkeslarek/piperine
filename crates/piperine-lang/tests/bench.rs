//! Parser/elaboration-level tests for the `bench` block (SPEC_BENCH.md).
//! End-to-end analysis behavior (`$op` actually solving) is covered in
//! `piperine-bench`'s own test suite — this crate only owns syntax,
//! attachment, availability validation, and override staging.

use piperine_lang::{parse_str, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_str(src).expect("parse failed").elaborate(&SourceMap::dummy()).expect("elaborate failed")
}

fn elab_err(src: &str) -> String {
    parse_str(src)
        .expect("parse failed")
        .elaborate(&SourceMap::dummy())
        .expect_err("expected elaboration to fail")
        .to_string()
}

const DUT: &str = "
    discipline Electrical { potential v: Real; flow i: Real; }
    mod Resistor(inout p: Electrical, inout n: Electrical) {
        param r: Real = 1e3;
    }
    mod SwitchOpenTest() {
        wire vsrc : Electrical;
        wire gnd : Electrical;
        resistor : Resistor(.p = vsrc, .n = gnd) { .r = 1e6 };
    }
";

#[test]
fn bench_attaches_to_its_module_by_name() {
    let src = format!(
        "{DUT}
        bench SwitchOpenTest {{
            fn test_open_circuit() {{
                $assert(true, \"ok\");
            }}
        }}"
    );
    let design = elab(&src);
    let bench = design.bench("SwitchOpenTest").expect("bench should attach");
    assert_eq!(bench.fns().len(), 1);
    assert_eq!(bench.entry_points().count(), 1);
    assert!(bench.fn_by_name("test_open_circuit").is_some());
}

#[test]
fn bench_on_unknown_module_is_a_fail_loud_error() {
    let src = format!("{DUT} bench DoesNotExist {{ fn go() {{ }} }}");
    let err = elab_err(&src);
    assert!(err.contains("unknown or generic module"), "unexpected error: {err}");
}

#[test]
fn bench_calling_an_unimplemented_task_is_rejected_at_elaboration() {
    let src = format!(
        "{DUT} bench SwitchOpenTest {{ fn go() {{ var r = $plot(w, \"title\"); }} }}"
    );
    let err = elab_err(&src);
    assert!(err.contains("$plot"), "unexpected error: {err}");
    assert!(err.contains("not yet implemented"), "unexpected error: {err}");
}

#[test]
fn bench_calling_op_and_diagnostics_elaborates_cleanly() {
    let src = format!(
        "{DUT}
        bench SwitchOpenTest {{
            fn test_open_circuit() {{
                var r = $op();
                $assert(true, \"ok\");
                $info(\"done\");
            }}
        }}"
    );
    elab(&src); // must not error
}

#[test]
fn overrides_apply_to_the_named_instance_param() {
    let design = elab(&format!("{DUT} bench SwitchOpenTest {{ fn go() {{}} }}"));
    design.set_param("resistor", "r", piperine_lang::Value::Real(2e6));
    let applied = design.with_overrides_applied("SwitchOpenTest").expect("override should apply");
    let module = applied.module("SwitchOpenTest").expect("module present");
    let resistor = module.instances.iter().find(|i| i.name() == "resistor").expect("instance present");
    let (_, value) = resistor.params.iter().find(|(name, _)| name == "r").expect("param staged");
    assert_eq!(*value, piperine_lang::Value::Real(2e6));
}

#[test]
fn overrides_on_unknown_instance_are_a_fail_loud_error() {
    let design = elab(&format!("{DUT} bench SwitchOpenTest {{ fn go() {{}} }}"));
    design.set_param("does_not_exist", "r", piperine_lang::Value::Real(1.0));
    let err = design.with_overrides_applied("SwitchOpenTest").expect_err("should fail");
    assert!(err.to_string().contains("does_not_exist"));
}

#[test]
fn fork_gives_each_entry_point_isolated_staging() {
    let design = elab(&format!("{DUT} bench SwitchOpenTest {{ fn go() {{}} }}"));
    let a = design.fork();
    a.set_param("resistor", "r", piperine_lang::Value::Real(2e6));
    assert!(a.has_overrides());
    let b = design.fork();
    assert!(!b.has_overrides(), "staging on one fork must not leak into another");
}
