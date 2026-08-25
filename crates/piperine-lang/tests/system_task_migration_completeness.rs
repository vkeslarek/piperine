//! LRM-COVERS: 9.17.2
//! §9.17.2 ("$bound_step task"): `bound_step_bounds_the_compiled_kernel_
//! hint` proves `analog $bound_step(2.5e-9)` produces a real, live effect
//! on the compiled `AnalogKernel` (`has_bound_step()`/`eval_bound_step`),
//! not merely that the name parses.
//!
//! §9 system-task coverage (LRMC-C7-10, T45) — rewritten against real
//! `.vams` sources through the actual pipeline
//! (`parse_and_elaborate` → `lower_bodies` → `CircuitCompiler`/
//! `AnalogKernel::compile`), replacing the dead PHDL-syntax fixture
//! this crate's own `tests/suite_hygiene.rs` would otherwise have
//! flagged as testing a deleted frontend (MD-44/MD-45).
//!
//! Each of the eleven original `valid_diagnostics` entries (`write`,
//! `strobe`, `display`, `info`, `warning`, `error`, `fatal`,
//! `bound_step`, `finish`, `stop`, `discontinuity`) gets a test
//! asserting its *effect* — real output through a `BufferSink`
//! (T43/LRMC-C7-07) captured through a real `CircuitCompiler`-built
//! circuit's transient run, a real compiled-kernel behavior difference
//! (`has_bound_step`/`has_discontinuity`, T41/T42), or a real,
//! LRM-consistent fail-loud (analog has no `$finish`/`$stop` support —
//! `flatten/analog.rs`'s own `"finish" | "stop"` arm) — never merely
//! that the name parses/elaborates (piperine-lang's elaborator itself
//! never validates a `$name` against any registry; that check happens
//! at codegen time, per the `undeclared_diagnostic_name_...` negative
//! control below).
//!
//! **Dev-dependency note (Cargo.toml):** this target needs a compiled
//! kernel to assert real effects, so `piperine-lang`'s dev-dependencies
//! gain `piperine-codegen`/`piperine-solver` — the same dev-only edge
//! shape `piperine-solver`'s own test suite already has on
//! `piperine-lang`+`piperine-codegen` (never a real build cycle:
//! `piperine-codegen` depends on `piperine-lang` normally, this is the
//! reverse edge, live only under `cargo test`).

use std::rc::Rc;

use piperine_codegen::CircuitCompiler;
use piperine_lang::{parse_and_elaborate, SourceMap};
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{BufferSink, Context, TransientAnalysisOptions};

fn digital_module(stmt: &str) -> String {
    format!(
        "
        discipline logic;
            domain discrete;
        enddiscipline

        module dut (input logic clk, output logic y);
            always @(posedge clk) begin
                y <= 1;
                {stmt}
            end
        endmodule

        module top ();
            logic clk_net, q_net;
            dut c1 (clk_net, q_net);
        endmodule
        "
    )
}

/// Builds `top` (wrapping `dut`), forces one posedge on `clk`, runs a
/// short transient with a `BufferSink` wired in, and hands the sink to
/// `check` for inspection.
fn digital_sink_effect(module_src: &str, check: impl FnOnce(&BufferSink)) {
    let design = parse_and_elaborate(module_src, &SourceMap::dummy()).expect("digital fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("compiles");

    let sink = Rc::new(BufferSink::new());
    let mut ctx = Context::default();
    ctx.io = sink.clone();

    let clk = DigitalNet(*info.digital_nets.get("clk_net").expect("net `clk_net`"));
    circuit.digital_state.schedule(DigitalEvent {
        time: 0.5,
        net: clk,
        value: LogicValue::Zero,
        source: usize::MAX,
        seq: 0,
        region: Region::Active,
    });
    circuit.digital_state.schedule(DigitalEvent {
        time: 1.0,
        net: clk,
        value: LogicValue::One,
        source: usize::MAX,
        seq: 1,
        region: Region::Active,
    });

    let opts = TransientAnalysisOptions::new(2.0, 0.1);
    let mut tran = circuit.transient(opts, ctx).unwrap();
    tran.solve().expect("transient run completes");

    check(&sink);
}

#[test]
fn write_produces_output_with_no_trailing_newline() {
    let src = digital_module(r#"$write("w");"#);
    digital_sink_effect(&src, |sink| {
        assert_eq!(sink.stdout(), "w", "no newline added by $write");
    });
}

#[test]
fn strobe_produces_output() {
    let src = digital_module(r#"$strobe("s");"#);
    digital_sink_effect(&src, |sink| {
        assert!(sink.stdout().contains('s'), "got: {:?}", sink.stdout());
    });
}

#[test]
fn display_produces_output_with_trailing_newline() {
    let src = digital_module(r#"$display("d");"#);
    digital_sink_effect(&src, |sink| {
        assert_eq!(sink.stdout(), "d\n");
    });
}

#[test]
fn info_produces_output() {
    let src = digital_module(r#"$info("i");"#);
    digital_sink_effect(&src, |sink| {
        assert!(sink.stdout().contains('i'), "got: {:?}", sink.stdout());
    });
}

#[test]
fn warning_produces_output() {
    let src = digital_module(r#"$warning("w2");"#);
    digital_sink_effect(&src, |sink| {
        assert!(sink.stdout().contains("w2"), "got: {:?}", sink.stdout());
    });
}

/// `$error`/`$fatal` differ observably from `$display`-family: they route
/// to `Channel::Stderr`, not stdout.
#[test]
fn error_writes_to_stderr() {
    let src = digital_module(r#"$error("e");"#);
    digital_sink_effect(&src, |sink| {
        assert!(sink.stderr().contains('e'), "got stderr: {:?}", sink.stderr());
        assert_eq!(sink.stdout(), "", "$error must not also write stdout");
    });
}

#[test]
fn fatal_writes_to_stderr() {
    let src = digital_module(r#"$fatal("f");"#);
    digital_sink_effect(&src, |sink| {
        assert!(sink.stderr().contains('f'), "got stderr: {:?}", sink.stderr());
    });
}

/// `$bound_step` has a real numeric effect on the compiled kernel: with
/// it, `has_bound_step()` is true and `eval_bound_step` returns the
/// declared bound instead of infinity.
#[test]
fn bound_step_bounds_the_compiled_kernel_hint() {
    let src = "
        module dut ();
            analog $bound_step(2.5e-9);
        endmodule
    ";
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let kernel = piperine_codegen::AnalogKernel::compile(&bodies["dut"]).expect("compiles");
    assert!(kernel.has_bound_step());
    let sim = piperine_codegen::SimCtx::default();
    let hint = kernel.eval_bound_step(&[], &[], &[], &[], &sim);
    assert!((hint - 2.5e-9).abs() < 1e-15, "got {hint}");
}

/// `$discontinuity` has a real numeric effect too (LRMC-C7-11, T42): the
/// compiled kernel reports a live flag distinguishing it from a module
/// that never calls it.
#[test]
fn discontinuity_flag_is_live_on_the_compiled_kernel() {
    let src = "
        module dut ();
            analog $discontinuity(0);
        endmodule
    ";
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let kernel = piperine_codegen::AnalogKernel::compile(&bodies["dut"]).expect("compiles");
    assert!(kernel.has_discontinuity());

    let without = "module dut (); real x; analog x = 1.0; endmodule";
    let design2 = parse_and_elaborate(without, &SourceMap::dummy()).expect("elaborates");
    let bodies2 = piperine_codegen::resolve::lower_bodies(&design2).expect("lowers");
    let kernel2 = piperine_codegen::AnalogKernel::compile(&bodies2["dut"]).expect("compiles");
    assert!(!kernel2.has_discontinuity(), "a module that never calls $discontinuity gets no flag");
}

/// `$finish`/`$stop` have a real, LRM-consistent effect in an analog
/// body today: they fail loud at compile time (no runtime-termination
/// mechanism exists for the analog kernel) rather than silently
/// vanishing — `flatten/analog.rs`'s own `"finish" | "stop"` arm.
#[test]
fn finish_fails_loud_in_an_analog_body() {
    let src = "module dut (); analog $finish(); endmodule";
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let err = match piperine_codegen::AnalogKernel::compile(&bodies["dut"]) {
        Ok(_) => panic!("$finish in an analog body must fail loud"),
        Err(e) => e,
    };
    assert!(format!("{err}").to_lowercase().contains("finish"));
}

#[test]
fn stop_fails_loud_in_an_analog_body() {
    let src = "module dut (); analog $stop(); endmodule";
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let err = match piperine_codegen::AnalogKernel::compile(&bodies["dut"]) {
        Ok(_) => panic!("$stop in an analog body must fail loud"),
        Err(e) => e,
    };
    let msg = format!("{err}").to_lowercase();
    assert!(msg.contains("stop") || msg.contains("finish"));
}

/// The negative control: piperine-lang's own elaborator never validates
/// a `$name` against any registry (confirmed empirically — any
/// syntactically valid `$name(...)` elaborates), so the real "did this
/// name resolve to a recognized system task" check happens at codegen
/// time, through the digital compiler's own fail-loud dispatch
/// (`emit/stmt.rs`'s `S::TaskCall` arm).
#[test]
fn undeclared_diagnostic_name_still_fails_loud_at_compile_time() {
    let src = digital_module("$totally_not_a_real_diagnostic();");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates (no registry check here)");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let err = match compiler.build_circuit_mapped("top") {
        Ok(_) => panic!("an unrecognized task call must fail loud"),
        Err(e) => e,
    };
    assert!(
        format!("{err}").contains("totally_not_a_real_diagnostic"),
        "error should name the unrecognized diagnostic: {err}"
    );
}
