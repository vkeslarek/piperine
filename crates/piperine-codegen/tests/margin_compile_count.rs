//! Per-instance function-count guard (dv-core T15, DVC-10, design §E): a
//! circuit with 100 instances of a constrained model emits the same number
//! of JIT compilations as one instance — the margin functions live on the
//! per-module kernel, `Arc`-shared across instances (the `.disto`
//! function-count wall is what per-instance emission would repeat).
//!
//! Lives in its own test binary so [`AnalogKernel::compile_count`] deltas
//! are not polluted by concurrent tests in the same process (the
//! `compile_once_sweep.rs` pattern).

use piperine_codegen::CircuitCompiler;
use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

#[test]
fn a_hundred_instances_compile_no_more_functions_than_one() {
    let module = "\
mod Cell(inout p: Electrical, inout n: Electrical) { param hi: Real = 3.0; }
analog Cell { I(p, n) <+ V(p, n) / 1e3; }
constraint Cell { require head : V(p, n) <= hi; }
";
    let top = |n: usize| {
        let mut src = format!("{DISCIPLINE}{module}\nmod Top() {{\n    wire gnd : Electrical;\n");
        for i in 0..n {
            src.push_str(&format!(
                "    wire n{i} : Electrical;\n    c{i} : Cell(.p = n{i}, .n = gnd) {{}};\n"
            ));
        }
        src.push_str("}\n");
        src
    };

    let compile_count_for = |n: usize| {
        let design = parse_and_elaborate(&top(n), &piperine_lang::SourceMap::dummy()).expect("elaborate");
        let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
        let before = AnalogKernel::compile_count();
        let mut compiler = CircuitCompiler::new(&design, &bodies);
        compiler.build_circuit("Top").expect("circuit builds");
        AnalogKernel::compile_count() - before
    };

    let one = compile_count_for(1);
    let hundred = compile_count_for(100);
    assert!(one > 0, "a constrained module compiles at least one kernel");
    assert_eq!(
        hundred, one,
        "100 instances emit the same function count as 1 (per-declared-margin, never per-instance)"
    );
}
