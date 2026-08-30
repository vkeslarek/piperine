//! LRM-COVERS: 3.3, 4.2.13
//! T66/LRMW2-63: `{a, b}` string concatenation and `{n{s}}` string
//! replication, previously rejected loud in every codegen path
//! (`digital_expr.rs`'s blanket `Concat`/`Replication` error). Scoped to
//! the elaboration-constant case (every operand a string literal, the
//! replication count a compile-time-constant non-negative integer) —
//! `elab::const_eval::ConstEnv::eval` folds it to a plain `Value::Str`
//! before it ever reaches codegen, reused directly by `$display`'s
//! format-string argument (`emit/stmt.rs`'s `S::TaskCall` arm). A dynamic
//! (non-constant) operand still fails to fold and the call silently no-ops,
//! matching T43's original disclosed scope for a non-literal format
//! string — unchanged by this task.
//!
//! Fixtures reproduce LRM §3.3's own worked concatenation examples
//! (`a = {"Hi", b}`, `b = {5{"Hi"}}`) as constant string literals, asserted
//! through `$display` output capture (`BufferSink`), mirroring
//! `vams_digital_display.rs`'s proven harness.

use std::rc::Rc;

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue, Region};
use piperine_solver::prelude::{BufferSink, Context, TransientAnalysisOptions};

/// Runs `counter`'s single clocked block once (one posedge) and returns the
/// captured stdout.
fn run_once(display_call: &str) -> String {
    let src = format!(
        r#"
        discipline logic;
            domain discrete;
        enddiscipline

        module counter (input logic clk, output logic q);
            integer count;
            always @(posedge clk) begin
                count <= count + 1;
                {display_call}
            end
        endmodule

        module top ();
            logic clk_net, q_net;
            counter c1 (clk_net, q_net);
        endmodule
    "#
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("compiles");

    let sink = Rc::new(BufferSink::new());
    let mut ctx = Context::default();
    ctx.io = sink.clone();

    let clk = DigitalNet(*info.digital_nets.get("clk_net").expect("net `clk`"));
    circuit.digital_state.schedule(DigitalEvent {
        time: 0.5, net: clk, value: LogicValue::Zero, source: usize::MAX, seq: 0, region: Region::Active, strength: None });
    circuit.digital_state.schedule(DigitalEvent {
        time: 1.0, net: clk, value: LogicValue::One, source: usize::MAX, seq: 1, region: Region::Active, strength: None });

    let opts = TransientAnalysisOptions::new(2.0, 0.1);
    let mut tran = circuit.transient(opts, ctx).unwrap();
    tran.solve().expect("transient run completes");
    sink.stdout()
}

/// LRM §3.3's own example: `a = {"Hi", b}` where `b` is itself a string
/// literal — plain concatenation of string contents (no bit-packing).
/// `$display` appends its own trailing newline (LRMC-C7-07, T43's
/// established behavior), so the expected output is `"Hi there\n"`.
#[test]
fn concatenation_joins_string_literals() {
    let out = run_once(r#"$display({"Hi ", "there"});"#);
    assert_eq!(out, "Hi there\n", "{{\"Hi \", \"there\"}} should concatenate to \"Hi there\": {out:?}");
}

/// A three-part concatenation — proves the fold isn't hardcoded to two
/// operands.
#[test]
fn concatenation_joins_three_string_literals() {
    let out = run_once(r#"$display({"A", "B", "C"});"#);
    assert_eq!(out, "ABC\n", "{{\"A\",\"B\",\"C\"}} should concatenate to \"ABC\": {out:?}");
}

/// LRM §3.3's own example: `b = {5{"Hi"}}` — replication repeats the
/// enclosed string `count` times.
#[test]
fn replication_repeats_a_string_literal_n_times() {
    let out = run_once(r#"$display({5{"Hi"}});"#);
    assert_eq!(out, "HiHiHiHiHi\n", "{{5{{\"Hi\"}}}} should repeat \"Hi\" 5 times: {out:?}");
}

/// Replication over a multi-part body: `{n{a, b}}` repeats the whole
/// concatenation `a, b` as one unit `n` times (LRM §4.2.13), not `a`
/// repeated then `b` once.
#[test]
fn replication_repeats_a_multi_part_concatenation_as_one_unit() {
    let out = run_once(r#"$display({2{"Hi", "!"}});"#);
    assert_eq!(out, "Hi!Hi!\n", "{{2{{\"Hi\",\"!\"}}}} should repeat the whole \"Hi!\" unit twice: {out:?}");
}

/// Zero-count replication yields the empty string (LRM places no floor on
/// the count beyond non-negative), nested inside a surrounding
/// concatenation — `{"X", {0{"Hi"}}, "Y"}` must fold to `"XY"`, not to
/// nothing: a discriminating check that the zero-count part actually
/// resolved to `""` and got joined in, rather than the fold bailing out on
/// the whole `$display` call the way pre-fix code no-ops on *any*
/// `Concat`/`Replication` (which would also print nothing here, making a
/// bare `{0{"Hi"}}` alone indistinguishable from "unsupported").
#[test]
fn zero_replication_yields_the_empty_string_and_still_joins() {
    let out = run_once(r#"$display({"X", {0{"Hi"}}, "Y"});"#);
    assert_eq!(out, "XY\n", "{{\"X\", {{0{{\"Hi\"}}}}, \"Y\"}} should fold the zero-replication part to \"\" and join to \"XY\": {out:?}");
}
