//! LRM §4.6: analysis-dependent functions — `analysis()` must reject an
//! unknown analysis name loudly (never silently match `"dc"`) and honor
//! the real `analysis_identifier` set; `ac_stim`'s optional leading
//! analysis-name string must not be consumed as the magnitude argument.
//!
//! `noise_table`/`noise_table_log` (SPEC_DEVIATION.md 2.6's third finding):
//! re-verified at HEAD — the finding is STALE. The audited evidence
//! (`analog_ops.rs:371-376`'s `real(0.0)` fallback for `noise_table`) no
//! longer exists; `noise_table`/`noise_table_log` are not registered in
//! `AnalogOpRegistry` at all (only `keywords.rs` still names them as
//! reserved words). An unrecognized call in an analog expression is a
//! loud `CodegenError::Unsupported` (`emit_analog`'s fallback arm), not a
//! silent `0.0` — `noise_table_call_fails_loud_not_silently_zero` below is
//! the proof. This task does not implement the tabulated-PSD feature
//! itself (a real feature gap, tracked separately) — only re-verifies
//! that the specific SPEC_DEVIATION finding (silent 0.0) does not
//! reproduce.
//!
//! LRM-COVERS: 4.6, 4.6.1, 4.6.2, 4.6.3, 4.6.4.3, 4.6.4.4, 4.5.10
//! T69: §4.6.4.3 "noise_table" and §4.6.4.4 "noise_table_log" are exactly
//! `noise_table_call_fails_loud_not_silently_zero`/`noise_table_log_call_
//! fails_loud_not_silently_zero` below — both keywords are reserved
//! (`keywords.rs`) but never registered in `AnalogOpRegistry`, so both hit
//! the same generic-call `CodegenError::Unsupported` fallback, not a
//! silent value. §4.5.10 "last_crossing" is the same unregistered-keyword
//! shape (confirmed absent from `AnalogOpRegistry::with_builtins()`) —
//! `last_crossing_call_fails_loud_not_silently_zero` proves the identical
//! loud-failure path.
//! Wave-2 (T69) re-reading, no new tests: §4.6.1 "Analysis" is `analysis_
//! unknown_name_fails_loud_by_typed_kind` (the `analysis("bogus")` name
//! set); §4.6.2 "DC analysis" is `analysis_dc_is_true_only_for_the_dc_
//! kind` (`analysis("dc")` true only for the DC analysis kind, across
//! DC/AC/tran); §4.6.3 "AC stimulus" is `ac_stim_analysis_name_argument_
//! is_not_consumed_as_magnitude` (both the named and bare `ac_stim(...)`
//! forms).

use std::sync::Arc;

use piperine_codegen::{AnalogKernel, CodegenError, SimCtx};
use piperine_lang::parse_and_elaborate;

fn compile(module_name: &str, src: &str) -> Result<Arc<AnalogKernel>, CodegenError> {
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("{module_name} elaborates: {e}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed");
    let module = bodies.get(module_name).expect("module lowered");
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    Ok(compiled.analog().expect("analog body").clone())
}

// ── analysis(): unknown names fail loud, never silently `dc` ───────────────

const BOGUS_SRC: &str = "
module BogusAnalysis ( inout electrical p, inout electrical n );
    analog I(p, n) <+ analysis(\"bogus\") ? 1.0 : 0.0;
endmodule
";

/// spec AC11: `analysis(\"bogus\")` is a loud error naming the valid set,
/// not a silent match against `\"dc\"` (kind 0).
#[test]
fn analysis_unknown_name_fails_loud_by_typed_kind() {
    let result = compile("BogusAnalysis", BOGUS_SRC);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("BogusAnalysis must fail to compile"),
    };
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("bogus"), "names the offending value: {msg}");
            assert!(msg.contains("dc") && msg.contains("static") && msg.contains("nodeset"),
                "names the valid set: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported, got {other:?}"),
    }
}

// ── analysis("dc"): true only in DC, across DC/AC/tran ──────────────────────

const DC_PROBE_SRC: &str = "
module DcProbe ( inout electrical p, inout electrical n );
    analog I(p, n) <+ analysis(\"dc\") ? 1.0 : 0.0;
endmodule
";

/// spec AC11: `analysis(\"dc\")` reads true only when `SimCtx.current_analysis`
/// is the DC kind (0) — asserted across the DC/AC/tran encodings
/// `emit_analog_syscall`'s `\"analysis\"` arm dispatches on.
#[test]
fn analysis_dc_is_true_only_for_the_dc_kind() {
    let kernel = compile("DcProbe", DC_PROBE_SRC).expect("compiles");
    let params: [f64; 0] = [];
    let node_voltages = [0.0_f64, 0.0];
    // (analysis kind encoding, expected analysis("dc") result)
    for (kind, expect_dc) in [(0u64, 1.0), (1u64, 0.0), (2u64, 0.0), (3u64, 0.0)] {
        let sim = SimCtx { current_analysis: kind, ..SimCtx::default() };
        let mut rhs = [0.0_f64; 2];
        kernel.eval_residual(&node_voltages, &params, &vec![0.0; kernel.num_state_slots()], &[], &sim, &mut rhs);
        assert_eq!(rhs[0], expect_dc, "analysis(\"dc\") under CURRENT_ANALYSIS={kind}: rhs[0] = {}", rhs[0]);
    }
}

// ── ac_stim: the analysis-name string is not consumed as magnitude ─────────

const AC_STIM_NAMED_SRC: &str = "
module AcStimNamed ( inout electrical p, inout electrical n );
    parameter real mag = 3.0;
    analog V(p, n) <+ ac_stim(\"ac\", mag, 0.0);
endmodule
";

const AC_STIM_BARE_SRC: &str = "
module AcStimBare ( inout electrical p, inout electrical n );
    parameter real mag = 3.0;
    analog V(p, n) <+ ac_stim(mag, 0.0);
endmodule
";

/// spec: `ac_stim(\"ac\", mag, phase)` applies `mag` as the actual AC
/// stimulus magnitude — not the leading string. Discriminates against the
/// pre-fix bug (which read `args[0]` — the `\"ac\"` string — as `mag`,
/// silently dropping the real magnitude): the named form's resolved
/// magnitude must equal `mag` (3.0), matching the bare (no analysis-name)
/// form exactly, not some string-derived garbage value.
#[test]
fn ac_stim_analysis_name_argument_is_not_consumed_as_magnitude() {
    let named = compile("AcStimNamed", AC_STIM_NAMED_SRC).expect("named form compiles");
    let bare = compile("AcStimBare", AC_STIM_BARE_SRC).expect("bare form compiles");
    assert!(named.has_force_ac_stim(), "named form must produce an ac_stim force");
    assert!(bare.has_force_ac_stim(), "bare form must produce an ac_stim force");

    let params = [3.0_f64]; // `mag` parameter
    let node_voltages = [0.0_f64, 0.0];
    let sim = SimCtx::default();

    // `V(p, n) <+ ac_stim(...)` is a force-branch contribution — its AC
    // stimulus lives in `Forces::ac_mag`/`ac_phase` (`num_forces` sized),
    // not the standalone `AcStim` capability (`eval_ac_stim`, which serves
    // `ac_stim(...)` used elsewhere in an expression).
    let mut named_mags = vec![0.0_f64; named.num_forces()];
    let mut named_phases = vec![0.0_f64; named.num_forces()];
    named.eval_force_ac_stim(&node_voltages, &params, &vec![0.0; named.num_state_slots()], &[], &sim, &mut named_mags, &mut named_phases);

    let mut bare_mags = vec![0.0_f64; bare.num_forces()];
    let mut bare_phases = vec![0.0_f64; bare.num_forces()];
    bare.eval_force_ac_stim(&node_voltages, &params, &vec![0.0; bare.num_state_slots()], &[], &sim, &mut bare_mags, &mut bare_phases);

    assert_eq!(named_mags, vec![3.0], "the named form's magnitude must be `mag` (3.0), not the \"ac\" string: {named_mags:?}");
    assert_eq!(named_mags, bare_mags, "named and bare forms must resolve to the same magnitude");
    assert_eq!(named_phases, bare_phases, "named and bare forms must resolve to the same phase");
}

// ── noise_table: stale-finding re-verification ──────────────────────────────

const NOISE_TABLE_SRC: &str = "
module NoiseTableUser ( inout electrical p, inout electrical n );
    parameter real tab[0:3] = '{0.0, 1.0, 1.0, 2.0};
    analog begin
        I(p, n) <+ V(p, n);
        I(p, n) <+ white_noise(noise_table(tab));
    end
endmodule
";

/// Re-verification: SPEC_DEVIATION.md 2.6 claimed `noise_table` lowers to
/// a silent `real(0.0)` (`analog_ops.rs:371-376`, dated evidence). That
/// registry entry no longer exists — `noise_table` isn't in
/// `AnalogOpRegistry` at all, so it falls through to `emit_analog`'s
/// generic-call fallback, which is a loud `CodegenError::Unsupported`,
/// never a silent value. This is the "finding no longer reproduces" path
/// (Wave-2 standing rule 1): proving the loud failure, not implementing
/// the tabulated-PSD feature itself.
#[test]
fn noise_table_call_fails_loud_not_silently_zero() {
    let result = compile("NoiseTableUser", NOISE_TABLE_SRC);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("NoiseTableUser must fail to compile, not silently return 0.0"),
    };
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("noise_table"), "names the unrecognized call: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported, got {other:?}"),
    }
}

const NOISE_TABLE_LOG_SRC: &str = "
module NoiseTableLogUser ( inout electrical p, inout electrical n );
    parameter real tab[0:3] = '{0.0, 1.0, 1.0, 2.0};
    analog begin
        I(p, n) <+ V(p, n);
        I(p, n) <+ white_noise(noise_table_log(tab));
    end
endmodule
";

/// Same shape as `noise_table_call_fails_loud_not_silently_zero`, for the
/// `noise_table_log` variant (§4.6.4.4) — also unregistered, also a loud
/// `CodegenError::Unsupported`, never silent.
#[test]
fn noise_table_log_call_fails_loud_not_silently_zero() {
    let result = compile("NoiseTableLogUser", NOISE_TABLE_LOG_SRC);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("NoiseTableLogUser must fail to compile, not silently return 0.0"),
    };
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("noise_table_log"), "names the unrecognized call: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported, got {other:?}"),
    }
}

const LAST_CROSSING_SRC: &str = "
module LastCrossingUser ( inout electrical p, inout electrical n );
    analog begin
        I(p, n) <+ last_crossing(V(p, n), 1);
    end
endmodule
";

/// §4.5.10 "last_crossing": reserved (`keywords.rs`) but absent from
/// `AnalogOpRegistry::with_builtins()`, so it hits the same generic-call
/// `CodegenError::Unsupported` fallback as `noise_table` above — a real,
/// disclosed feature gap, never a silent no-op.
#[test]
fn last_crossing_call_fails_loud_not_silently_zero() {
    let result = compile("LastCrossingUser", LAST_CROSSING_SRC);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("LastCrossingUser must fail to compile, not silently return 0.0"),
    };
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("last_crossing"), "names the unrecognized call: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported, got {other:?}"),
    }
}
