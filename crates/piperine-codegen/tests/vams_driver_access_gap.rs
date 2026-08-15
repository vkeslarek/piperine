//! LRMC-C7-05 (T35, scope note documented in `piperine-lang`'s
//! `eval::tasks` module): the seven `$driver_*` functions parse and
//! elaborate (proven in `piperine-lang`'s own `vams_system_tasks.rs`),
//! but reaching a live `piperine_solver::digital::DriverTable` from a
//! JIT-compiled `connectmodule` analog body needs a SimCtx ABI extension
//! this task's stated scope doesn't reach — and, more load-bearing,
//! nothing populates that table until T36-T40 build §7.9's actual
//! driver-receiver segregation routing. This target proves the
//! CLAUDE.md-mandated fail-loud behavior for that gap: compiling a
//! `connectmodule` body that calls `$driver_count` fails loud, naming the
//! call, instead of silently compiling to a wrong always-zero result.

use piperine_lang::parse_and_elaborate;

const SRC: &str = r#"
    connectmodule bridge (inout logic a, inout electrical b);
        integer n;
        analog begin
            n = $driver_count(a);
        end
    endmodule

    discipline logic;
        domain discrete;
    enddiscipline
"#;

#[test]
fn driver_count_fails_loud_at_compile_instead_of_silently_returning_zero() {
    let design = parse_and_elaborate(SRC, &piperine_lang::SourceMap::dummy()).expect("bridge elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let Err(err) = piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["bridge"]) else {
        panic!("$driver_count must fail loud at compile time, not silently JIT to a wrong value");
    };
    let msg = format!("{err}").to_lowercase();
    assert!(msg.contains("driver_count"), "error should name the unrecognized syscall, got: {msg}");
}
