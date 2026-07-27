//! The margin kernel (dv-core T15, DVC-10): one JIT function per
//! **declared** pointwise margin, parameterized by instance (the
//! per-instance function-count guard lives in `margin_compile_count.rs`,
//! its own binary). Margins CSE against the residual's temp tape (a
//! module `var` the residual also computes is read through the tape, not
//! re-inlined). No constraint block → the capability is `None`, nothing
//! compiled. Only pointwise margins reach the kernel; reduced ones do
//! not (D7).

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::resolve::MarginKind;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// Compile `Dut` (two terminals, params + body + optional constraint block
/// verbatim).
fn kernel(params: &str, body: &str, constraint: &str) -> AnalogKernel {
    let src = format!(
        "{DISCIPLINE}
mod Dut(inout p: Electrical, inout n: Electrical) {{ {params} }}
analog Dut {{ {body} }}
{constraint}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    AnalogKernel::compile(&bodies["Dut"]).expect("compile")
}

fn eval_margin(kernel: &AnalogKernel, index: usize, volts: &[f64], params: &[f64]) -> f64 {
    let vars = vec![0.0; kernel.num_vars()];
    let state = vec![0.0; kernel.num_state_slots()];
    kernel.eval_margin(index, volts, params, &state, &vars, &SimCtx::default())
}

/// DVC-10: one function per declared **pointwise** margin — the reduced
/// target never reaches the kernel — each evaluated independently, with
/// the margin value parameterized by the instance's params (the same
/// kernel serves every instance).
#[test]
fn one_function_per_declared_pointwise_margin_parameterized_by_instance() {
    let kernel = kernel(
        "param hi: Real = 3.0; param lo: Real = 1.0;",
        "I(p, n) <+ V(p, n) / 1e3;",
        "constraint Dut {\n\
             require head : V(p, n) <= hi;\n\
             require foot : V(p, n) >= lo;\n\
             target ac_unity_gain_freq(V(p, n)) >= 10e6 tol 1e6;\n\
         }",
    );

    assert!(kernel.has_margins());
    assert_eq!(kernel.num_margins(), 2, "the reduced target stays out of the kernel (D7)");
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
        "{DISCIPLINE}
mod Dut(inout p: Electrical, inout n: Electrical) {{
    param gain: Real = 2.0;
    param hi: Real = 3.0;
    var vd : Real = 0.0;
}}
analog Dut {{
    vd = gain * V(p, n);
    I(p, n) <+ vd / 1e3;
}}
constraint Dut {{
    require head : vd <= hi;
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let kernel = AnalogKernel::compile(&bodies["Dut"]).expect("compile");

    assert_eq!(kernel.num_margins(), 1);
    // vd = 2·V: at V = 1.0 the margin is 3 − 2·1 = 1; at V = 2.0 it is
    // 3 − 2·2 = −1 (violated). The var's *computed* value — its temp —
    // is what the margin reads; the bank's initial 0.0 would give 3.0.
    assert_eq!(eval_margin(&kernel, 0, &[1.0, 0.0], &[2.0, 3.0, 0.0]), 1.0, "margin reads the tape value");
    assert_eq!(eval_margin(&kernel, 0, &[2.0, 0.0], &[2.0, 3.0, 0.0]), -1.0, "tape value tracks the input");
}

/// DVC-10: a module with no constraint block compiles no margin
/// capability — `None`, no function, no catalog.
#[test]
fn no_constraint_block_compiles_nothing() {
    let kernel = kernel("param r: Real = 1e3;", "I(p, n) <+ V(p, n) / r;", "");
    assert!(!kernel.has_margins(), "no constraint block → no capability");
    assert_eq!(kernel.num_margins(), 0);
    assert!(kernel.margin_catalog().is_empty());
}
