//! LRM-COVERS: 4.5.11, 4.5.11.1, 4.5.11.2, 4.5.11.3, 4.5.11.4, 4.5.11.5, 5.6
//! LRMC-C1-12/C5-01 (`.specs/features/lrm-conformance-closure/`, T21):
//! `laplace_zd`/`laplace_nd`/`laplace_zp`/`laplace_np` (the four real LRM
//! §4.5.11 spellings) resolve through `AnalogKernel::compile`; the
//! fabricated `laplace_pm`/`nm`/`npm` registry-typo spellings fail loud as
//! unknown operators instead. The registry/keyword guard itself (proven
//! able to fail) lives inline in `resolve/pom/analog_ops.rs` — `pub(crate)`
//! internals aren't reachable from this crate-external integration test.
//!
//! **Fixture adaptation (disclosed):** §4.5.11.5's own examples write
//! `V(out) <+ laplace_zp(V(in), ...)` across a 4-port module (separate
//! `in`/`out` pairs); the `dut` fixture here is a plain 2-terminal module
//! contributing directly on `(p,n)`. `V(p,n) <+ laplace_*(...)` hits a
//! pre-existing, unrelated restriction (`flatten/analog.rs`: "ddt in a
//! potential contribution must integrate a branch current") because
//! `laplace_*` expands through `ddt` internally; `I(p,n) <+ laplace_*(...)`
//! (a flow contribution, equally valid VAMS and equally proves resolution)
//! avoids it. The coefficient arguments are copied verbatim from the LRM
//! text either way.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_lang::parse_and_elaborate;

fn compile_body(body: &str) -> Result<AnalogKernel, piperine_codegen::CodegenError> {
    let src = format!(
        "module dut (inout electrical p, inout electrical n);
            analog begin
                {body}
            end
        endmodule"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("dut elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    AnalogKernel::compile(&bodies["dut"])
}

#[test]
fn laplace_zd_resolves() {
    compile_body("I(p, n) <+ laplace_zd(V(p,n), '{0,1}, '{-1,0,1});").expect("laplace_zd must compile");
}

#[test]
fn laplace_nd_resolves() {
    // §4.5.11.5's own second example: `laplace_nd(V(in), '{0,1}, '{-1,0,1})`.
    compile_body("I(p, n) <+ laplace_nd(V(p,n), '{0,1}, '{-1,0,1});").expect("laplace_nd must compile");
}

#[test]
fn laplace_zp_resolves() {
    // §4.5.11.5's own first example: `laplace_zp(V(in), '{-1,0}, '{-1,-1,-1,1})`.
    compile_body("I(p, n) <+ laplace_zp(V(p,n), '{-1,0}, '{-1,-1,-1,1});").expect("laplace_zp must compile");
}

#[test]
fn laplace_np_resolves() {
    compile_body("I(p, n) <+ laplace_np(V(p,n), '{-1,0}, '{-1,-1,-1,1});").expect("laplace_np must compile");
}

#[test]
fn fabricated_laplace_pm_fails_loud() {
    let Err(err) = compile_body("I(p, n) <+ laplace_pm(V(p,n), '{-1,0}, '{-1,-1,-1,1});") else {
        panic!("laplace_pm has no LRM basis and must not resolve");
    };
    let msg = format!("{err}").to_lowercase();
    assert!(
        msg.contains("laplace_pm") || msg.contains("unknown") || msg.contains("unsupported"),
        "error should name the unresolved call, got: {msg}"
    );
}

#[test]
fn fabricated_laplace_nm_fails_loud() {
    assert!(
        compile_body("I(p, n) <+ laplace_nm(V(p,n), '{-1,0}, '{-1,-1,-1,1});").is_err(),
        "laplace_nm has no LRM basis and must not resolve"
    );
}

#[test]
fn fabricated_laplace_npm_fails_loud() {
    assert!(
        compile_body("I(p, n) <+ laplace_npm(V(p,n), '{-1,0}, '{-1,-1,-1,1});").is_err(),
        "laplace_npm has no LRM basis and must not resolve"
    );
}
