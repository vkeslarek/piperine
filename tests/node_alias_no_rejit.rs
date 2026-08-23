//! LRM-COVERS: 9.20
//!
//! LRMW2-49 (`lrm-conformance-wave2`, T56): a design containing a
//! constant-argument `$analog_node_alias` call must build once (MD-18)
//! and never re-JIT during a subsequent parameter sweep — the same
//! `AnalogKernel::compile_count()` delta technique
//! `tests/compile_once_sweep.rs` already established, applied to this
//! task's own construct. Lives in its own test binary (matching
//! `compile_once_sweep.rs`'s own reasoning) so the process-wide compile
//! counter isn't polluted by concurrently-running tests in another file.

use piperine::Session;
use piperine_codegen::AnalogKernel;

const SRC: &str = r#"
    module leaf (inout electrical p, inout electrical gnd);
        parameter real v = 0;
        analog V(p, gnd) <+ v;
    endmodule

    module top ();
        electrical a, b, gnd;
        leaf u1 (b, gnd);
        integer status;
        analog initial begin
            status = $analog_node_alias(a, "b");
        end
    endmodule
"#;

#[test]
fn node_alias_design_builds_once_and_a_sweep_recompiles_nothing() {
    let design = piperine_lang::parse_and_elaborate(SRC, &piperine_lang::SourceMap::dummy())
        .expect("fixture elaborates");

    let before_build = AnalogKernel::compile_count();
    let mut session = Session::compile(&design, "top").expect("session compiles");
    let per_build = AnalogKernel::compile_count() - before_build;
    assert!(per_build > 0, "a build must JIT at least one kernel");

    let before_sweep = AnalogKernel::compile_count();
    let values = [0.0, 1.0, 2.0, -1.0];
    {
        let mut sweep = session.sweep("u1", "v", &values);
        while let Some(point) = sweep.next() {
            point.expect("sweep point restamps");
        }
    }
    let sweep_compiles = AnalogKernel::compile_count() - before_sweep;

    assert_eq!(
        sweep_compiles, 0,
        "MD-18: a design containing $analog_node_alias must restamp on the one build \
         ({per_build} kernel(s)) and JIT nothing more during a sweep, got {sweep_compiles}"
    );
}
