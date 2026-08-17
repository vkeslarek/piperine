//! Compiling a module and dropping it returns its JIT code pages.
//!
//! `cranelift_jit`'s `Memory::drop` leaks every allocation on purpose ("leak
//! memory to guarantee validity of function pointers"), so a `JITModule` held
//! and dropped without [`free_memory`](cranelift_jit::JITModule::free_memory)
//! costs the process its code pages permanently. `emit::OwnedJit` makes that
//! call for every compiled kernel; this is the guard that keeps it made.
//!
//! Measured, not asserted: the probe compiles the same body in rounds and
//! watches RSS. Leaking, the growth is linear and never stops (≈8 kB per
//! compile for the small body below, far more for a MOSFET); returning the
//! pages, RSS reaches a working set in the first round and then flattens.
//! The threshold is deliberately loose — this catches "the pages are never
//! returned", not allocator jitter.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_lang::parse_and_elaborate;

/// Compiles per measured round. Enough that a per-compile leak dwarfs the
/// allocator noise the threshold tolerates.
const ROUND: usize = 500;

/// The most the resident set may grow across the measured rounds. Leaking, the
/// same span costs ≈8 MB (500 compiles × 2 rounds × ≈8 kB); returning the pages,
/// it costs a few tens of kB.
const MAX_GROWTH_KB: i64 = 2_048;

fn rss_kb() -> i64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("read /proc/self/status");
    status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .expect("VmRSS in /proc/self/status")
        .parse()
        .expect("VmRSS is a number")
}

/// A body with enough shape to allocate real code pages: a nonlinear resistive
/// contribution (its symbolic Jacobian is emitted too) plus a charge one.
fn body() -> std::collections::HashMap<String, piperine_codegen::resolve::pom::LoweredBody> {
    let src = "
module Dut(inout electrical p, inout electrical n);
    parameter real r = 1.0e3;
    real g, vd;
    analog begin
        g = 1.0 / r;
        vd = V(p, n);
        I(p, n) <+ g * vd + 1.0e-14 * (exp(vd / 0.026) - 1.0);
        I(p, n) <+ ddt(1.0e-12 * vd);
    end
endmodule
";
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    piperine_codegen::resolve::lower_bodies(&design).expect("lowering")
}

#[test]
fn dropping_a_kernel_returns_its_jit_pages() {
    let bodies = body();
    let compile_round = || {
        for _ in 0..ROUND {
            let kernel = AnalogKernel::compile(&bodies["Dut"]).expect("compile");
            std::hint::black_box(&kernel);
        }
    };

    // The first round pays for the allocator's working set and Cranelift's
    // one-time ISA setup; the measurement starts after it.
    compile_round();
    let settled = rss_kb();
    compile_round();
    compile_round();
    let growth = rss_kb() - settled;

    assert!(
        growth < MAX_GROWTH_KB,
        "resident set grew {growth} kB over {} further compiles (limit {MAX_GROWTH_KB} kB) — a \
         dropped `AnalogKernel` is not returning its JIT code pages, which is what \
         `emit::OwnedJit` exists to do",
        2 * ROUND
    );
}
