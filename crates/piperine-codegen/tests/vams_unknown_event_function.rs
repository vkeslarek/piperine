//! LRM-COVERS: 5.10.3.4
//! LRMC-C5-03 (T24): an unrecognized call-shaped analog event function
//! (`@(bogus_event(x))`) fails loud at lowering instead of being silently
//! swallowed by the flattener's event dispatch — the exact `_ => vec![]`
//! fallthrough class of bug that let `absdelta` itself go unwired before
//! this task.

use piperine_lang::parse_and_elaborate;

const SRC: &str = r#"
    module dut (inout electrical p, inout electrical n);
        real x;
        analog begin
            @(bogus_event(V(p, n)))
                x = 1.0;
        end
    endmodule
"#;

#[test]
fn unknown_analog_event_function_fails_loud() {
    let design = parse_and_elaborate(SRC, &piperine_lang::SourceMap::dummy()).expect("dut elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let Err(err) = piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["dut"]) else {
        panic!("an unrecognized analog event function must fail loud, not silently vanish");
    };
    let msg = format!("{err}").to_lowercase();
    assert!(
        msg.contains("bogus_event") || msg.contains("unknown") || msg.contains("unsupported"),
        "error should name the unrecognized event function, got: {msg}"
    );
}

/// The four real LRM event functions must still work — this fixture would
/// have failed loud too if the fix over-corrected into rejecting legitimate
/// names.
#[test]
fn real_event_functions_still_compile() {
    for event in ["cross(V(p, n))", "above(V(p, n))", "timer(1.0)", "absdelta(V(p, n), 0.1)"] {
        let src = format!(
            "module dut (inout electrical p, inout electrical n);
                real x;
                analog begin
                    @({event})
                        x = 1.0;
                end
            endmodule"
        );
        let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
            .unwrap_or_else(|e| panic!("{event} fixture elaborates: {e:?}"));
        let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
        piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["dut"])
            .unwrap_or_else(|e| panic!("{event} must still compile: {e:?}"));
    }
}
