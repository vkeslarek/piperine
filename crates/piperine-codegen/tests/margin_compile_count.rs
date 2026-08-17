//! Per-instance function-count guard (dv-core T15, DVC-10, design §E): a
//! circuit with 100 instances of a constrained model emits the same number
//! of JIT compilations as one instance — the margin functions live on the
//! per-module kernel, `Arc`-shared across instances (the `.disto`
//! function-count wall is what per-instance emission would repeat).
//!
//! Lives in its own test binary so [`AnalogKernel::compile_count`] deltas
//! are not polluted by concurrent tests in the same process (the
//! `compile_once_sweep.rs` pattern).
//!
//! **Require-core migrated** (constraints-on-vams): these tests run
//! against the live producer. Deferred statements (`var`/`target`/
//! `cover`) are exercised in their own files, still red.

use piperine_codegen::CircuitCompiler;
use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_lang::parse_and_elaborate;

const DISCIPLINES: &str = "`include \"disciplines.vams\"";

/// The `SourceMap` reaching `piperine-lang`'s bundled headers from this
/// crate (`SourceMap::dummy`'s relative paths only resolve from that
/// crate's dir).
fn source_map() -> piperine_lang::SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

#[test]
fn a_hundred_instances_compile_no_more_functions_than_one() {
    let module = "\
module Cell (inout electrical p, inout electrical n);
    parameter real hi = 3.0;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
";
    let top = |n: usize| {
        let mut src = format!("{DISCIPLINES}\n{module}\nmodule Top ();\n    electrical gnd;\n");
        for i in 0..n {
            src.push_str(&format!("    electrical n{i};\n    Cell c{i} (.p(n{i}), .n(gnd));\n"));
        }
        src.push_str("endmodule\n");
        src
    };

    let compile_count_for = |n: usize| {
        let design = parse_and_elaborate(&top(n), &source_map()).expect("elaborate");
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
