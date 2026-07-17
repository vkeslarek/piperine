//! LIVE-10 — MD-18 enforcement for the Python live session: `compile()`
//! elaborates + JITs **once**, and a `set` + `op` loop never recompiles.
//! Lives in its own test binary so [`AnalogKernel::compile_count`] deltas
//! are not polluted by concurrent tests in the same process (same pattern
//! as `piperine-bench/tests/compile_once_sweep.rs`).

use piperine_codegen::AnalogKernel;
use piperine_python::embed::run_script;

/// Write `body` to a temp file named `name`; return its path as a `String`.
fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

/// LIVE-10 AC1: a live session's whole `set` + `op` loop JITs exactly one
/// circuit build — the same kernel count as a bare `compile()` + single
/// `op`. (One `#[test]` — a second test in this file would run concurrently
/// and pollute the compile-count deltas.)
#[test]
fn live_session_compiles_once_across_set_op_loop() {
    let phdl = write_temp("piperine_live_count_fixture.phdl", DIVIDER_PHDL);

    // Reference: one compile() + one op — the per-build kernel count.
    let single = format!(
        "import _piperine\n\
         s = _piperine.load(\"{phdl}\").module(\"Divider\").compile()\n\
         assert abs(s.op().v(\"mid\") - 2.0) < 1e-9\n"
    );
    let single_path = write_temp("piperine_live_count_single.py", &single);
    let before_single = AnalogKernel::compile_count();
    run_script(&single_path).expect("single-op session");
    let per_build = AnalogKernel::compile_count() - before_single;
    assert!(per_build > 0, "a build must JIT at least one kernel");

    // The live loop: one compile(), then 10 × (set + op).
    let looped = format!(
        r#"import _piperine
s = _piperine.load("{phdl}").module("Divider").compile()
for i in range(10):
    r = 1e3 + 500.0 * i
    s.set("r_top", "r", r)
    want = 5.0 * 2e3 / (r + 2e3)
    got = s.op().v("mid")
    assert abs(got - want) < 1e-9, (r, got, want)
"#
    );
    let looped_path = write_temp("piperine_live_count_loop.py", &looped);
    let before_loop = AnalogKernel::compile_count();
    run_script(&looped_path).expect("set+op loop session");
    let loop_compiles = AnalogKernel::compile_count() - before_loop;

    assert_eq!(
        loop_compiles, per_build,
        "MD-18: a 10-point set+op loop must JIT one build ({per_build} kernel(s)), \
         got {loop_compiles}"
    );

    for p in [phdl, single_path, looped_path] {
        let _ = std::fs::remove_file(p);
    }
}
