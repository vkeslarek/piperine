//! The margin kernel (dv-core T15, DVC-10): one JIT function per
//! **declared** pointwise margin, parameterized by instance (the
//! per-instance function-count guard lives in `margin_compile_count.rs`,
//! its own binary). Margins CSE against the residual's temp tape (a
//! module `var` the residual also computes is read through the tape, not
//! re-inlined). No constraint block → the capability is `None`, nothing
//! allocated. A reduced-class margin never reaches the kernel (D7) — the
//! `target` statements that carried them are deferred with dv-core's
//! other statements and are exercised in their own files.
//!
//! **Require-core migrated** (constraints-on-vams): these tests run
//! against the live producer. Deferred statements (`var` inside the
//! constraint block/`target`/`cover`) are exercised in their own files,
//! still red.

use piperine_codegen::SimCtx;
use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::resolve::MarginKind;
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

/// Compile `Dut` (two terminals, params + body + optional constraint
/// block verbatim).
fn kernel(params: &str, body: &str, constraint: &str) -> AnalogKernel {
    let src = format!(
        "{DISCIPLINES}
module Dut (inout electrical p, inout electrical n);
    {params}
    analog begin
        {body}
    end
{constraint}
endmodule
"
    );
    let design = parse_and_elaborate(&src, &source_map()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    AnalogKernel::compile(&bodies["Dut"]).expect("compile")
}

fn eval_margin(kernel: &AnalogKernel, index: usize, volts: &[f64], params: &[f64]) -> f64 {
    let vars = vec![0.0; kernel.num_vars()];
    let state = vec![0.0; kernel.num_state_slots()];
    kernel.eval_margin(index, volts, params, &state, &vars, &SimCtx::default())
}

/// DVC-10: one function per declared **pointwise** margin — each evaluated
/// independently, with the margin value parameterized by the instance's
/// params (the same kernel serves every instance).
#[test]
fn one_function_per_declared_pointwise_margin_parameterized_by_instance() {
    let kernel = kernel(
        "parameter real hi = 3.0;\n    parameter real lo = 1.0;",
        "I(p, n) <+ V(p, n) / 1e3;",
        "constraint begin\n\
             require head : V(p, n) <= hi;\n\
             require foot : V(p, n) >= lo;\n\
         end",
    );

    assert!(kernel.has_margins());
    assert_eq!(kernel.num_margins(), 2, "one function per declared require");
    let catalog = kernel.margin_catalog();
    assert_eq!(catalog[0].label, "head");
    assert_eq!(catalog[0].kind, MarginKind::Require);
    assert_eq!(catalog[1].label, "foot");
    assert!(catalog[0].analyses.is_empty(), "probe-only margins belong to every analysis");

    // m = hi − V and m = V − lo, read off the instance's own params.
    assert_eq!(eval_margin(&kernel, 0, &[2.0, 0.0], &[3.0, 1.0]), 1.0, "head: hi − V");
    assert_eq!(eval_margin(&kernel, 1, &[2.0, 0.0], &[3.0, 1.0]), 1.0, "foot: V − lo");
    // The same kernel, another instance's params — parameterization, not
    // recompilation.
    assert_eq!(eval_margin(&kernel, 0, &[2.0, 0.0], &[5.0, 1.0]), 3.0, "head at hi=5");
    assert_eq!(eval_margin(&kernel, 1, &[2.0, 0.0], &[3.0, 4.0]), -2.0, "foot violated at lo=4");
}

/// DVC-10: margins CSE against the residual's temp tape — a module `var`
/// the residual also computes (`vd = gain · V(p,n)`) is read by the margin
/// through the tape, so the shared subexpression is computed once per
/// evaluation, not re-inlined. Proven on values: the margin reads the
/// var's computed value, not its initial bank value.
#[test]
fn margins_read_module_vars_through_the_shared_temp_tape() {
    let src = format!(
        "{DISCIPLINES}
module Dut (inout electrical p, inout electrical n);
    parameter real gain = 2.0;
    parameter real hi = 3.0;
    real vd;
    analog begin
        vd = gain * V(p, n);
        I(p, n) <+ vd / 1e3;
    end
    constraint begin
        require head : vd <= hi;
    end
endmodule
"
    );
    let design = parse_and_elaborate(&src, &source_map()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let kernel = AnalogKernel::compile(&bodies["Dut"]).expect("compile");

    assert_eq!(kernel.num_margins(), 1);
    // vd = 2·V: at V = 1.0 the margin is 3 − 2·1 = 1; at V = 2.0 it is
    // 3 − 2·2 = −1 (violated). The var's *computed* value — its temp —
    // is what the margin reads; the bank's initial 0.0 would give 3.0.
    assert_eq!(eval_margin(&kernel, 0, &[1.0, 0.0], &[2.0, 3.0]), 1.0, "margin reads the tape value");
    assert_eq!(eval_margin(&kernel, 0, &[2.0, 0.0], &[2.0, 3.0]), -1.0, "tape value tracks the input");
}

/// DVC-10: a module with no constraint block compiles no margin
/// capability — `None`, no function, no catalog.
#[test]
fn no_constraint_block_compiles_nothing() {
    let kernel = kernel("parameter real r = 1e3;", "I(p, n) <+ V(p, n) / r;", "");
    assert!(!kernel.has_margins(), "no constraint block → no capability");
    assert_eq!(kernel.num_margins(), 0);
    assert!(kernel.margin_catalog().is_empty());
}
