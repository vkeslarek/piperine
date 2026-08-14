# LRM Conformance Closure — Pre-Feature Test Baseline

**Captured**: 2026-08-14, branch `feature/device-verification`, HEAD `87c2ded7`
(before any lrm-conformance-closure work). Methodology matches the MD-47 A/B
precedent (`.specs/STATE.md`): target-by-target, not aggregate counts. This
file is the reference point every later task's "no regression" gate check
compares against (byte-identical-or-better per target, never an aggregate
total).

## Build

`cargo build --workspace`: **0 warnings**, exit 0 (confirmed via a
touch-forced full rebuild of `piperine-lang` and its dependents, not just a
cached no-op run).

## `cargo build --workspace --tests` / `cargo test --workspace` gate result

This is the literal result of the gate command named in tasks.md, recorded
as-is (not required green):

```
$ cargo test --workspace
   ...
error[E0433]: failed to resolve: could not find `parse` in `piperine_lang`
   [... 28 total compile errors across 6 test targets in piperine-codegen
        and 22 in piperine-lang ...]
error: could not compile `piperine-lang` (test "...") due to N previous errors
$ echo $?
101
```

**`cargo test --workspace` currently exits 101 and runs zero tests**, because
28 of the workspace's 297 test targets (`crates/piperine-lang/tests/*.rs` ×22,
`crates/piperine-codegen/tests/*.rs` ×6) fail to compile against APIs that no
longer exist (`piperine_lang::parse::ast::*`, `piperine_lang::parse_str`,
`piperine_lang::elab::registry::{CallableDef,CallableRegistry}`,
`Module::constraints`/`.constraints()`, `piperine_lang::pom::QuantityClass`,
`pom::StmtClassNode`) — pre-MD-47 PHDL-era surface. Cargo's own compile-error
handling means **a single broken test target aborts the entire `cargo test`
invocation before any test runs**, workspace-wide or per-package alike (this
was independently confirmed: `cargo test -p piperine-lang` alone also exits
101 with zero tests run). This makes the literal gate command produce no
per-target signal at all — not usable as "the baseline" on its own.

### Methodology note (why this file has a second, deeper layer)

Because `cargo test --workspace`/`-p <crate>` both abort atomically on any
compile failure in that scope, and because cargo additionally stops
*scheduling new* compile jobs after the first failure (so which broken
targets get reported is non-deterministic across runs — confirmed: two
successive `cargo test -p piperine-lang` runs reported different 4-of-22 and
5-of-22 subsets of the same 22 broken targets), neither command alone can
enumerate the true state target-by-target. The reliable procedure used here:

1. `cargo build --workspace --tests --keep-going --message-format=json` — the
   one cargo flag (`--keep-going`, stable since 1.61, not exposed on
   `cargo test` itself) that keeps building every target regardless of
   sibling failures, in one pass. Parsing the JSON stream separates the
   297 on-disk test files (`ls crates/*/tests/*.rs tests/*.rs`) into 269
   that produce an executable and 28 that don't (compiler-message
   `level=error`, deduplicated by target name — this cross-checks exactly
   against the file-count enumeration, 269+28=297).
2. Each of the 269 compiled test binaries was then **executed directly**
   (bypassing `cargo test`'s own compile step entirely), from its own
   package's manifest directory with `CARGO_MANIFEST_DIR` set to match —
   reproducing cargo's actual per-target working-directory contract. This
   mattered in practice: an initial pass that ran every binary from the
   workspace root produced systematically wrong results for any test
   resolving fixtures via a path relative to its own crate (confirmed
   directly: `piperine-lang`'s `vams_headers` target showed 0 passed/4
   failed run from the workspace root and 4 passed/0 failed run from
   `crates/piperine-lang/`, byte-identical binary). All crate-level numbers
   below are from the corrected (own-directory) run; the 49 root-level
   `tests/*.rs` targets were already correct in the first pass since the
   root package's manifest directory *is* the workspace root.
3. One target, `piperine-plugin-macros`'s `compile_fail` (a `trybuild`
   UI-test target that shells out to `cargo check` against a from-scratch
   dependency set in `tests/trybuild/`), took >20 minutes without finishing
   even after its own dependency cache warmed from a first attempt, and was
   killed. It is recorded below as **TIMEOUT**, not pass or fail — not
   independently verified in this baseline capture.

## Summary

| | Count |
|---|---|
| Total on-disk test target files (`ls crates/*/tests/*.rs tests/*.rs`) | 297 |
| Targets that fail to compile | 28 |
| Targets that compile and were executed | 269 |
| — of those, fully passing (0 failed) | 109 |
| — of those, with ≥1 failing test case | 159 |
| — of those, timed out / not verified | 1 |
| Total individual test cases passed (sum across the 269 executed targets) | 926 |
| Total individual test cases failed (sum across the 269 executed targets) | 568 |
| Total individual test cases ignored | 0 |

**161 of 297 targets (54%) are currently red** (28 compile-fail + 159
runtime-fail, excluding the 1 timeout — using the 160 figure that includes
the timeout as unverified-but-not-confirmed-green). This is the honest
pre-feature state of `feature/device-verification` at `87c2ded7`, not a
regression introduced by this baseline capture. It is consistent with — and
much larger in scope than — the single known-red example tasks.md already
names (`system_task_migration_completeness.rs`, confirmed below at exactly
12/12 failing, matching tasks.md's T1 note and T45's stated "before" state).
The dominant visible pattern (spot-checked across several failing targets in
`piperine-lang`, `piperine-plugin`, and root `session.rs`) is fixtures using
a discipline-declaration form the current grammar rejects
(`discipline Electrical { potential v: Real; flow i: Real; }` — brace-block
syntax, not the LRM's keyword-delimited `discipline ... enddiscipline`
form) and PHDL-era APIs referenced from `piperine-lang`'s pre-MD-47 surface;
this feature does not attempt to explain or fix every individual failure —
that is exactly the closure work the later tasks (T10 onward) perform,
each checked against **this file**, target by target, not against "the
suite went green."

## Known-red targets

### Compile failures (28 targets — full list, all in `piperine-lang` and `piperine-codegen`)

Common causes (grep-confirmed against the error output): references to
`piperine_lang::parse::ast::*` / `piperine_lang::parse_str` (module removed,
T21/PHDL-parser deletion per CLAUDE.md's "Files not to edit casually"
history), `piperine_lang::elab::registry::{CallableDef,CallableRegistry}`
(removed), `Module::constraints`/`.constraints()` (field/method removed),
`piperine_lang::pom::QuantityClass` (moved to `piperine_codegen::resolve`),
`pom::StmtClassNode` (removed), plus 2 unrelated `E0282` type-inference
errors (`extern_grammar.rs:263`, `param_diff.rs:163`) that would need their
own fix independent of the API-removal pattern.

### All compile-failing targets (28)

| Crate | Target | Status |
|---|---|---|
| piperine-codegen | digital_codegen_gaps | COMPILE_FAIL |
| piperine-codegen | digital_jit | COMPILE_FAIL |
| piperine-codegen | margin_lowering | COMPILE_FAIL |
| piperine-codegen | param_diff | COMPILE_FAIL |
| piperine-codegen | resolve_lowering | COMPILE_FAIL |
| piperine-codegen | silent_bugs | COMPILE_FAIL |
| piperine-lang | bench_removed | COMPILE_FAIL |
| piperine-lang | callable_registry | COMPILE_FAIL |
| piperine-lang | constraint_class | COMPILE_FAIL |
| piperine-lang | constraint_elab | COMPILE_FAIL |
| piperine-lang | constraint_parse | COMPILE_FAIL |
| piperine-lang | constraint_pom | COMPILE_FAIL |
| piperine-lang | constraint_scope | COMPILE_FAIL |
| piperine-lang | cover_parse | COMPILE_FAIL |
| piperine-lang | elab | COMPILE_FAIL |
| piperine-lang | extern_grammar | COMPILE_FAIL |
| piperine-lang | extern_missing_native_binding | COMPILE_FAIL |
| piperine-lang | extern_type_registry | COMPILE_FAIL |
| piperine-lang | fmt_doc_comment | COMPILE_FAIL |
| piperine-lang | operator_and_impl_method_registries | COMPILE_FAIL |
| piperine-lang | overload_resolution | COMPILE_FAIL |
| piperine-lang | parse_elab | COMPILE_FAIL |
| piperine-lang | pom_serde | COMPILE_FAIL |
| piperine-lang | prelude | COMPILE_FAIL |
| piperine-lang | reserved_words | COMPILE_FAIL |
| piperine-lang | rfport | COMPILE_FAIL |
| piperine-lang | spice_headers | COMPILE_FAIL |
| piperine-lang | tol_clause | COMPILE_FAIL |

### All targets with >=1 failing test (159)

| Crate | Target | Result | First failure |
|---|---|---|---|
| piperine | compile_once_sweep | 0 passed; 1 failed | thread 'sweep_compiles_once_and_matches_the_staged_path' (212590) panicked at tests/compile_once_sweep.rs:23:46:  |
| piperine | dc_host_proof | 0 passed; 1 failed | thread 'nested_and_source_sweeps_restamp_one_compilation' (212612) panicked at tests/dc_host_proof.rs:67:10:  |
| piperine | disto | 0 passed; 2 failed | thread 'cubic_stage_hd2_hd3_match_closed_form_through_the_host' (212634) panicked at tests/disto.rs:42:85:  |
| piperine | host_capability_parity | 0 passed; 4 failed | thread 'pnjlim_junction_matches_between_phdl_and_a_host_device' (212658) panicked at tests/host_capability_parity.rs:423:81:  |
| piperine | host_introspect | 0 passed; 8 failed | thread 'unknown_instance_label_fails_loud' (212688) panicked at tests/host_introspect.rs:56:10:  |
| piperine | host_limiting | 0 passed; 2 failed | thread 'stats_limiting_accessible_on_nonlinear_circuit' (212711) panicked at tests/host_limiting.rs:104:10:  |
| piperine | host_noise | 0 passed; 3 failed | thread 'sum_of_integrated_sq_matches_total_squared' (212736) panicked at tests/host_noise.rs:53:10:  |
| piperine | host_object_model | 0 passed; 3 failed | thread 'staged_overrides_stay_isolated_from_the_parent_design' (212760) panicked at tests/host_object_model.rs:83:49:  |
| piperine | host_parity | 5 passed; 7 failed | thread 'host_parity_model_surface_matches_on_both_hosts' (212787) panicked at tests/host_parity.rs:286:62:  |
| piperine | host_probe | 0 passed; 7 failed | thread 'opvar_read_fails_loud_when_not_recorded' (212817) panicked at tests/host_probe.rs:61:10:  |
| piperine | host_result_types | 0 passed; 3 failed | thread 'disto_result_is_the_typed_api_struct' (212843) panicked at tests/host_result_types.rs:121:85:  |
| piperine | host_session_builder | 0 passed; 5 failed | thread 'disto_kernels_are_opt_in_and_asking_without_them_is_loud' (212871) panicked at tests/host_session_builder.rs:70:66:  |
| piperine | host_soa | 0 passed; 5 failed | thread 'diode_card_with_bv_max_driven_past_aborts_strict_naming_instance_and_constraint' (212894) panicked at tests/host_soa.rs:40:29:  |
| piperine | host_waveform_measure | 1 passed; 8 failed | thread 'overshoot_is_zero_for_first_order_rc' (212923) panicked at tests/host_waveform_measure.rs:44:70:  |
| piperine | net_selector | 2 passed; 3 failed | thread 'v_still_accepts_bare_netref' (212953) panicked at tests/net_selector.rs:28:66:  |
| piperine | ngspice_tran | 23 passed; 2 failed | thread 'ngspice_uic_rc' (212981) panicked at tests/ngspice_tran.rs:179:29:  |
| piperine | opvar_host | 0 passed; 5 failed | thread 'unknown_instance_label_fails_loud' (213027) panicked at tests/opvar_host.rs:51:10:  |
| piperine | plugin_parity | 1 passed; 1 failed | thread 'plugin_parity_surfaces_match_on_both_hosts' (213051) panicked at tests/plugin_parity.rs:198:79:  |
| piperine | pss_host | 0 passed; 2 failed | thread 'rectifier_ripple_matches_settled_transient' (213073) panicked at tests/pss_host.rs:72:10:  |
| piperine | pz | 0 passed; 3 failed | thread 'series_rlc_has_the_analytic_complex_conjugate_pole_pair' (213097) panicked at tests/pz.rs:76:84:  |
| piperine | session | 0 passed; 10 failed | thread 'run_op_solves_and_reads_back_by_net_name' (213122) panicked at tests/session.rs:46:10:  |
| piperine | session_analyses | 0 passed; 3 failed | thread 'session_set_on_unknown_param_is_a_loud_error' (213152) panicked at tests/session_analyses.rs:110:66:  |
| piperine | session_compile | 0 passed; 1 failed | thread 'session_compiles_once_and_set_op_matches_fresh_builds' (213175) panicked at tests/session_compile.rs:49:77:  |
| piperine | session_dc_sweep | 0 passed; 1 failed | thread 'dc_sweep_returns_a_trace_over_the_swept_axis_matching_fresh_builds' (213197) panicked at tests/session_dc_sweep.rs:29:75:  |
| piperine | session_sweep | 0 passed; 2 failed | thread 'non_structural_sweep_never_rebuilds_and_matches_fresh_builds' (213219) panicked at tests/session_sweep.rs:45:67:  |
| piperine | session_sweep_grid | 0 passed; 2 failed | thread 'map_failure_surfaces_with_the_combinations_coordinates' (213242) panicked at tests/session_sweep_grid.rs:31:75:  |
| piperine | session_tf | 0 passed; 2 failed | thread 'tf_current_source_input_fails_loud' (213265) panicked at tests/session_tf.rs:58:79:  |
| piperine | sp | 0 passed; 2 failed | thread 'shunt_c_lowpass_s21_matches_closed_form_rolloff' (213289) panicked at tests/sp.rs:37:10:  |
| piperine | suite_hygiene | 9 passed; 1 failed | thread 'mod_rs_declares_only' (213313) panicked at tests/suite_hygiene.rs:547:5:  |
| piperine | tline | 0 passed; 4 failed | thread 'tline_bad_td_is_loud' (213342) panicked at tests/tline.rs:124:5:  |
| piperine | tol_inert | 0 passed; 5 failed | thread 'op_is_bit_identical_with_and_without_tol' (213369) panicked at tests/tol_inert.rs:72:10:  |
| piperine | trace_generic | 2 passed; 2 failed | thread 'transient_is_real_and_ac_is_complex_on_the_same_container' (213396) panicked at tests/trace_generic.rs:32:83:  |
| piperine | units_freq | 2 passed; 1 failed | thread 'session_ac_accepts_f64_and_si_string_identically' (213420) panicked at tests/units_freq.rs:31:66:  |
| piperine | urc_compile_count | 0 passed; 1 failed | thread 'flatten_preserves_kernel_keying_and_restamp_invariants' (213442) panicked at tests/urc_compile_count.rs:62:29:  |
| piperine | xfmr | 0 passed; 4 failed | thread 'xfmr_coupled_lc_energy_transfer' (213466) panicked at tests/xfmr.rs:97:10:  |
| piperine-api | aging_drift | 0 passed; 3 failed | thread 'a_drift_crossing_a_rebuild_class_boundary_fails_loud' (213487) panicked at crates/piperine-api/tests/aging_drift.rs:73:67:  |
| piperine-api | center_policy | 0 passed; 2 failed | thread 'a_monte_carlo_at_the_centered_point_shows_higher_yield_than_nominal' (213509) panicked at crates/piperine-api/tests/center_policy.rs:52:67:  |
| piperine-api | coverage_merge | 1 passed; 4 failed | thread 'a_solved_circuits_coverage_report_reflects_its_operating_point' (213531) panicked at crates/piperine-api/tests/coverage_merge.rs:34:95:  |
| piperine-api | driver_posture | 2 passed; 2 failed | thread 'high_sigma_policy_fails_loud_with_no_tol_declarations' (213555) panicked at crates/piperine-api/tests/driver_posture.rs:43:67:  |
| piperine-api | event_window | 0 passed; 2 failed | thread 'window_outside_the_tran_range_is_not_exercised' (213577) panicked at crates/piperine-api/tests/event_window.rs:75:80:  |
| piperine-api | high_sigma_sampling | 0 passed; 3 failed | thread 'a_tail_sample_is_replayable_from_seed_and_index' (213598) panicked at crates/piperine-api/tests/high_sigma_sampling.rs:51:67:  |
| piperine-api | margin_gradients | 0 passed; 6 failed | thread 'an_ac_defined_reduced_metric_fails_loud_naming_the_ac_gap' (213624) panicked at crates/piperine-api/tests/margin_gradients.rs:109:80:  |
| piperine-api | margins_channel | 1 passed; 4 failed | thread 'tran_trace_margins_read_with_time_argmins' (213649) panicked at crates/piperine-api/tests/margins_channel.rs:41:80:  |
| piperine-api | margins_sweep | 0 passed; 3 failed | thread 'fluent_sweep_reduces_to_worst_across_with_coordinates' (213670) panicked at crates/piperine-api/tests/margins_sweep.rs:41:10:  |
| piperine-api | model_descriptors | 2 passed; 5 failed | thread 'instance_reflects_label_and_instantiated_module' (213692) panicked at crates/piperine-api/tests/model_descriptors.rs:38:55:  |
| piperine-api | model_design | 2 passed; 9 failed | thread 'const_reads_a_global_and_reports_none_for_an_unknown_name' (213729) panicked at crates/piperine-api/tests/model_design.rs:97:41:  |
| piperine-api | model_instance | 0 passed; 6 failed | thread 'trace_bound_view_returns_waveforms_and_guards_the_op_side' (213771) panicked at crates/piperine-api/tests/model_instance.rs:53:36:  |
| piperine-api | model_module | 0 passed; 7 failed | thread 'compile_returns_a_live_session_that_restamps_without_recompiling' (213794) panicked at crates/piperine-api/tests/model_module.rs:39:36:  |
| piperine-api | monitor_composition | 0 passed; 2 failed | thread 'a_monitor_detects_a_violation_the_top_level_cannot_see_with_instance_provenance' (213820) panicked at crates/piperine-api/tests/monitor_composition.rs:56:82:  |
| piperine-api | monte_carlo_compile_once | 0 passed; 1 failed | thread 'a_thousand_samples_cost_zero_additional_compiles' (213840) panicked at crates/piperine-api/tests/monte_carlo_compile_once.rs:45:83:  |
| piperine-api | monte_carlo_report | 0 passed; 4 failed | thread 'yield_fraction_is_the_share_of_samples_with_every_margin_nonnegative' (213863) panicked at crates/piperine-api/tests/monte_carlo_report.rs:72:67:  |
| piperine-api | monte_carlo_sampling | 0 passed; 8 failed | thread 'draws_apply_through_the_ordinary_restamp_path' (213888) panicked at crates/piperine-api/tests/monte_carlo_sampling.rs:77:67:  |
| piperine-api | optimize_blackbox | 0 passed; 2 failed | thread 'a_mixed_fixture_composes_gradient_feasibility_with_black_box_objective' (213910) panicked at crates/piperine-api/tests/optimize_blackbox.rs:49:67:  |
| piperine-api | optimize_compile_once | 0 passed; 1 failed | thread 'the_whole_optimization_run_costs_zero_additional_compiles' (213931) panicked at crates/piperine-api/tests/optimize_compile_once.rs:40:85:  |
| piperine-api | optimize_policy | 0 passed; 3 failed | thread 'a_two_parameter_fixture_converges_to_its_known_optimum_and_stays_feasible' (213951) panicked at crates/piperine-api/tests/optimize_policy.rs:43:67:  |
| piperine-api | posture_config | 1 passed; 1 failed | thread 'posture_changes_between_analyses_never_recompile' (213973) panicked at crates/piperine-api/tests/posture_config.rs:41:64:  |
| piperine-api | reduced_metrics | 0 passed; 3 failed | thread 'a_reduced_target_is_a_normalized_margin_without_argmin' (213994) panicked at crates/piperine-api/tests/reduced_metrics.rs:60:86:  |
| piperine-api | scoped_require_scope | 0 passed; 3 failed | thread 'strict_dc_fails_when_dc_scoped_require_is_violated_strict_tran_passes' (214018) panicked at crates/piperine-api/tests/scoped_require_scope.rs:74:80:  |
| piperine-api | sensitivity_surface | 0 passed; 5 failed | thread 'sensitivity_returns_a_typed_value_with_engine_attribution' (214041) panicked at crates/piperine-api/tests/sensitivity_surface.rs:75:80:  |
| piperine-api | sequencer_dc_ac | 2 passed; 1 failed | thread 'ac_with_a_sequencer_present_fails_loud_naming_the_gap' (214063) panicked at crates/piperine-api/tests/sequencer_dc_ac.rs:131:6:  |
| piperine-api | sequencer_ordering_and_catalog | 1 passed; 1 failed | thread 'rebuild_class_set_param_on_a_host_device_fails_loud' (214084) panicked at crates/piperine-api/tests/sequencer_ordering_and_catalog.rs:175:6:  |
| piperine-api | smoke | 1 passed; 1 failed | thread 'op_analysis_through_piperine_api' (214106) panicked at crates/piperine-api/tests/smoke.rs:41:64:  |
| piperine-api | temp_sweep | 0 passed; 1 failed | thread 'temp_sweep_diode_forward_drop_shifts_minus_2mv_per_c' (214127) panicked at crates/piperine-api/tests/temp_sweep.rs:46:10:  |
| piperine-api | violations_channel | 0 passed; 5 failed | thread 'op_violations_carry_severity_and_provenance' (214149) panicked at crates/piperine-api/tests/violations_channel.rs:95:80:  |
| piperine-codegen | analog_device_numerics | 0 passed; 9 failed | thread 'test_jit_compiles' (214174) panicked at crates/piperine-codegen/tests/analog_device_numerics.rs:218:93:  |
| piperine-codegen | analog_kernel | 0 passed; 6 failed | thread 'a_capacitor_emits_charge_and_charge_jacobian_but_no_resistive_current' (214199) panicked at crates/piperine-codegen/tests/analog_kernel.rs:28:80:  |
| piperine-codegen | bypass_capability | 0 passed; 6 failed | thread 'a_plain_resistor_opts_in' (214228) panicked at crates/piperine-codegen/tests/bypass_capability.rs:29:80:  |
| piperine-codegen | checkpoint_digital | 0 passed; 4 failed | thread 'digital_device_checkpoints_registers' (214250) panicked at crates/piperine-codegen/tests/checkpoint_digital.rs:29:77:  |
| piperine-codegen | checkpoint_limiter | 0 passed; 6 failed | thread 'checkpoint_after_restore_recaptures_clean_state' (214272) panicked at crates/piperine-codegen/tests/checkpoint_limiter.rs:30:77:  |
| piperine-codegen | circuit_from_design | 0 passed; 4 failed | thread 'jit_device_exposes_params_through_introspection' (214300) panicked at crates/piperine-codegen/tests/circuit_from_design.rs:67:77:  |
| piperine-codegen | codegen_api | 0 passed; 7 failed | thread 'ir_digital_compile_dff' (214323) panicked at crates/piperine-codegen/tests/codegen_api.rs:64:77:  |
| piperine-codegen | cover_bin_mapping | 0 passed; 7 failed | thread 'an_optimizer_loop_with_cover_off_leaves_the_database_untouched' (214348) panicked at crates/piperine-codegen/tests/cover_bin_mapping.rs:31:80:  |
| piperine-codegen | digital_fusion | 0 passed; 4 failed | thread 'fusion_bit_identical_static_adder' (214375) panicked at crates/piperine-codegen/tests/digital_fusion.rs:37:79:  |
| piperine-codegen | disto_capabilities | 0 passed; 3 failed | thread 'jit_device_never_declares_numeric_jacobian' (214395) panicked at crates/piperine-codegen/tests/disto_capabilities.rs:23:77:  |
| piperine-codegen | disto_jit | 0 passed; 8 failed | thread 'disto2_nonlinear_charge_second_derivative' (214420) panicked at crates/piperine-codegen/tests/disto_jit.rs:17:77:  |
| piperine-codegen | flatten_hierarchy | 0 passed; 5 failed | thread 'two_mid_level_instances_produce_distinct_spliced_labels' (214448) panicked at crates/piperine-codegen/tests/flatten_hierarchy.rs:49:79:  |
| piperine-codegen | has_sensitivity | 1 passed; 2 failed | thread 'a_phdl_device_without_a_kernel_does_not_declare_has_sensitivity' (214470) panicked at crates/piperine-codegen/tests/has_sensitivity.rs:16:80:  |
| piperine-codegen | jit_memory | 0 passed; 1 failed | thread 'dropping_a_kernel_returns_its_jit_pages' (214490) panicked at crates/piperine-codegen/tests/jit_memory.rs:55:80:  |
| piperine-codegen | limiters | 0 passed; 7 failed | thread 'limit_catalog_pnjlim_infers_voltage_step' (214512) panicked at crates/piperine-codegen/tests/limiters.rs:277:77:  |
| piperine-codegen | live_params | 0 passed; 6 failed | thread 'jit_device_set_errors_are_loud_with_flattened_param_names' (214536) panicked at crates/piperine-codegen/tests/live_params.rs:50:10:  |
| piperine-codegen | live_set_never_recompiles | 0 passed; 1 failed | thread 'live_sets_never_recompile_across_restamp_and_temperature_cycles' (214561) panicked at crates/piperine-codegen/tests/live_set_never_recompiles.rs:127:10:  |
| piperine-codegen | margin_ac_points | 0 passed; 3 failed | thread 'ac_margins_are_evaluated_per_frequency_with_frequency_argmin' (214582) panicked at crates/piperine-codegen/tests/margin_ac_points.rs:45:80:  |
| piperine-codegen | margin_compile_count | 0 passed; 1 failed | thread 'a_hundred_instances_compile_no_more_functions_than_one' (214603) panicked at crates/piperine-codegen/tests/margin_compile_count.rs:36:87:  |
| piperine-codegen | margin_dc_points | 0 passed; 3 failed | thread 'collect_dc_records_the_margin_of_the_final_solution' (214623) panicked at crates/piperine-codegen/tests/margin_dc_points.rs:46:80:  |
| piperine-codegen | margin_evaluation | 0 passed; 7 failed | thread 'non_finite_margin_is_a_loud_violation_never_clamped' (214650) panicked at crates/piperine-codegen/tests/margin_evaluation.rs:35:80:  |
| piperine-codegen | margin_kernel | 0 passed; 3 failed | thread 'no_constraint_block_compiles_nothing' (214672) panicked at crates/piperine-codegen/tests/margin_kernel.rs:27:80:  |
| piperine-codegen | margin_postures | 0 passed; 5 failed | thread 'strict_aborts_mid_transient_when_the_op_is_satisfied' (214696) panicked at crates/piperine-codegen/tests/margin_postures.rs:67:80:  |
| piperine-codegen | margin_tran_points | 0 passed; 3 failed | thread 'uic_skips_t0_while_a_computed_op_is_evaluated' (214719) panicked at crates/piperine-codegen/tests/margin_tran_points.rs:17:79:  |
| piperine-codegen | model_descriptor | 1 passed; 8 failed | thread 'force_terminal_pairs_catalog_surfaces_branch_names' (214740) panicked at crates/piperine-codegen/tests/model_descriptor.rs:22:77:  |
| piperine-codegen | observable_catalog | 0 passed; 7 failed | thread 'observable_costs_are_in_unit_range' (214771) panicked at crates/piperine-codegen/tests/observable_catalog.rs:21:77:  |
| piperine-codegen | opvar_bridge | 0 passed; 8 failed | thread 'read_opvars_returns_compiled_module_vars' (214798) panicked at crates/piperine-codegen/tests/opvar_bridge.rs:26:77:  |
| piperine-codegen | param_admissibility | 0 passed; 4 failed | thread 'a_rebuild_class_parameter_emits_no_derivative' (214822) panicked at crates/piperine-codegen/tests/param_admissibility.rs:28:80:  |
| piperine-codegen | param_kernel | 0 passed; 4 failed | thread 'param_jacobian_matches_finite_difference_of_the_residual' (214846) panicked at crates/piperine-codegen/tests/param_kernel.rs:24:80:  |
| piperine-codegen | param_stamps | 1 passed; 3 failed | thread 'param_stamps_pushes_the_analytic_derivative_into_a_caller_sink' (214866) panicked at crates/piperine-codegen/tests/param_stamps.rs:31:80:  |
| piperine-codegen | table_op | 0 passed; 3 failed | thread 'length_mismatch_fails_loud' (214890) panicked at crates/piperine-codegen/tests/table_op.rs:13:14:  |
| piperine-codegen | temperature_dtemp | 1 passed; 3 failed | thread 'dtemp_instance_composes_into_effective_temperature' (214912) panicked at crates/piperine-codegen/tests/temperature_dtemp.rs:29:10:  |
| piperine-codegen | terminal_bridge | 0 passed; 9 failed | thread 'digital_kernel_directions_are_in_for_inputs_out_for_outputs' (214937) panicked at crates/piperine-codegen/tests/terminal_bridge.rs:25:77:  |
| piperine-lang | bundle_connections | 0 passed; 1 failed | thread 'bundle_connections_fan_out' (214962) panicked at crates/piperine-lang/tests/bundle_connections.rs:23:96:  |
| piperine-lang | cast_impl_methods | 0 passed; 9 failed | thread 'bare_cast_call_has_no_special_case_meaning_but_is_not_yet_rejected' (214982) panicked at crates/piperine-lang/tests/cast_impl_methods.rs:125:5:  |
| piperine-lang | fail_loud_call_resolution | 0 passed; 7 failed | thread 'extern_fn_call_with_mismatched_signature_fails_loud' (215011) panicked at crates/piperine-lang/tests/fail_loud_call_resolution.rs:75:5:  |
| piperine-lang | fail_loud_type_attr_resolution | 0 passed; 4 failed | thread 'undeclared_type_reference_still_fails_loud_after_call_resolution_changes' (215039) panicked at crates/piperine-lang/tests/fail_loud_type_attr_resolution.rs:86:5:  |
| piperine-lang | introspection_attrs | 0 passed; 4 failed | thread 'unknown_field_on_prelude_schema_fails_loud' (215062) panicked at crates/piperine-lang/tests/introspection_attrs.rs:64:5:  |
| piperine-lang | introspection_meta | 0 passed; 18 failed | thread 'bad_field_type_fails_loud_at_elaboration' (215093) panicked at crates/piperine-lang/tests/introspection_meta.rs:334:5:  |
| piperine-lang | operator_resolution | 0 passed; 5 failed | thread 'ddt_call_with_wrong_arg_type_fails_loud' (215120) panicked at crates/piperine-lang/tests/operator_resolution.rs:100:5:  |
| piperine-lang | system_task_migration_completeness | 0 passed; 12 failed | thread 'undeclared_diagnostic_name_still_fails_loud' (215152) panicked at crates/piperine-lang/tests/system_task_migration_completeness.rs:109:5:  |
| piperine-lang | tol_elab | 0 passed; 6 failed | thread 'wrong_argument_spelling_fails_loud' (215179) panicked at crates/piperine-lang/tests/tol_elab.rs:123:5:  |
| piperine-lang | type_casts | 0 passed; 3 failed | thread 'test_b5_implicit_cast_rejected' (215200) panicked at crates/piperine-lang/tests/type_casts.rs:33:9:  |
| piperine-lang | urc_flatten | 0 passed; 3 failed | thread 'urc2_and_urc10_flatten_to_expected_leaf_counts' (215221) panicked at crates/piperine-lang/tests/urc_flatten.rs:105:33:  |
| piperine-lang-server | completion_and_symbols | 0 passed; 5 failed | thread 'schema_completion_filters_by_typed_prefix' (215247) panicked at crates/piperine-lang-server/tests/completion_and_symbols.rs:35:5:  |
| piperine-lang-server | completion_suppression | 1 passed; 2 failed | thread 'completion_right_after_module_close_brace_never_offers_for_and_offers_mod' (215268) panicked at crates/piperine-lang-server/tests/completion_suppression.rs:33:5:  |
| piperine-lang-server | diagnostics | 2 passed; 5 failed | thread 'attr_arg_bad_type_diagnostic_points_at_the_specific_argument' (215289) panicked at crates/piperine-lang-server/tests/diagnostics.rs:114:10:  |
| piperine-lang-server | document_highlight | 0 passed; 1 failed | thread 'document_highlight_excludes_other_scope_and_comment_matches' (215323) panicked at crates/piperine-lang-server/tests/document_highlight.rs:28:5:  |
| piperine-lang-server | hover | 0 passed; 7 failed | thread 'hover_on_attr_schema_use_lists_its_fields' (215345) panicked at crates/piperine-lang-server/tests/hover.rs:131:5:  |
| piperine-lang-server | import_goto | 0 passed; 1 failed | thread 'goto_on_use_path_and_imported_module' (215383) panicked at crates/piperine-lang-server/tests/import_goto.rs:15:5:  |
| piperine-lang-server | instance_highlight | 0 passed; 4 failed | thread 'clicking_unlabeled_instance_type_name_resolves_to_the_module' (215404) panicked at crates/piperine-lang-server/tests/instance_highlight.rs:22:5:  |
| piperine-lang-server | project_index | 0 passed; 2 failed | thread 'standalone_document_has_no_project_unit' (215427) panicked at crates/piperine-lang-server/tests/project_index.rs:62:5:  |
| piperine-lang-server | protocol | 3 passed; 9 failed | thread 'harness_drives_hover_round_trip' (215449) panicked at crates/piperine-lang-server/tests/protocol.rs:237:37:  |
| piperine-lang-server | references_and_rename | 1 passed; 3 failed | thread 'rename_edits_only_the_binding_uses_other_scope_untouched' (215499) panicked at crates/piperine-lang-server/tests/references_and_rename.rs:53:59:  |
| piperine-lang-server | symbol_resolution | 0 passed; 5 failed | thread 'occurrences_at_returns_exactly_the_indexed_uses' (215529) panicked at crates/piperine-lang-server/tests/symbol_resolution.rs:90:5:  |
| piperine-plugin | e2e | 0 passed; 6 failed | thread 'unregistered_type_fails_loud' (215556) panicked at crates/piperine-plugin/tests/e2e.rs:25:6:  |
| piperine-plugin | extern_stub | 0 passed; 1 failed | thread 'shipped_extern_stub_is_inert_and_only_stdlib_schemas_seed' (215576) panicked at crates/piperine-plugin/tests/extern_stub.rs:81:6:  |
| piperine-plugin | hooks | 0 passed; 1 failed | thread 'read_only_hooks_observe_the_pipeline' (216246) panicked at crates/piperine-plugin/tests/hooks.rs:28:6:  |
| piperine-plugin | inject | 0 passed; 5 failed | thread 'injected_device_honors_staged_params' (216267) panicked at crates/piperine-plugin/tests/inject.rs:24:6:  |
| piperine-plugin | staging | 0 passed; 3 failed | thread 'conflicting_specs_from_two_plugins_fail_loud' (216290) panicked at crates/piperine-plugin/tests/staging.rs:29:6:  |
| piperine-plugin-macros | script_hook | 2 passed; 1 failed | thread 'each_hook_fires_with_its_phase_payload' (216312) panicked at crates/piperine-plugin-macros/tests/script_hook.rs:107:79:  |
| piperine-python | ac | 0 passed; 1 failed |   |
| piperine-python | analyses | 0 passed; 6 failed |   |
| piperine-python | check_posture | 1 passed; 2 failed | thread 'every_posture_reaches_the_analysis' (216389) panicked at crates/piperine-python/tests/check_posture.rs:76:5:  |
| piperine-python | design | 0 passed; 3 failed |   |
| piperine-python | disto_parity | 0 passed; 1 failed | thread 'python_disto_matches_rust_disto' (216438) panicked at crates/piperine-python/tests/disto_parity.rs:48:14:  |
| piperine-python | extract | 1 passed; 2 failed | thread 'extract_over_waveform_returns_named_measurement_dict' (216459) panicked at crates/piperine-python/tests/extract.rs:93:5:  |
| piperine-python | four_parity | 0 passed; 1 failed | thread 'python_fourier_matches_rust_fourier' (216480) panicked at crates/piperine-python/tests/four_parity.rs:46:14:  |
| piperine-python | host_device_library | 3 passed; 1 failed | thread 'pip_device_registers_python_host_devices_end_to_end' (216501) panicked at crates/piperine-python/tests/host_device_library.rs:29:70:  |
| piperine-python | host_plot | 0 passed; 1 failed | thread 'plot_and_bode_render_with_matplotlib_and_fail_loud_without_it' (216523) panicked at crates/piperine-python/tests/host_plot.rs:104:5:  |
| piperine-python | host_sweep | 0 passed; 1 failed | thread 'python_sweep_rebuilds_once_and_matches_fresh_builds' (216543) panicked at crates/piperine-python/tests/host_sweep.rs:96:5:  |
| piperine-python | host_sweep_grid | 0 passed; 1 failed | thread 'python_grid_iterates_and_maps_to_a_shaped_ndarray' (216563) panicked at crates/piperine-python/tests/host_sweep_grid.rs:73:5:  |
| piperine-python | live_facade | 0 passed; 1 failed | thread 'facade_live_session_has_error_parity_and_scheduled_sets' (216594) panicked at crates/piperine-python/tests/live_facade.rs:52:10:  |
| piperine-python | live_optimize_example | 0 passed; 1 failed | thread 'live_optimize_example_passes_with_one_live_compilation' (216614) panicked at crates/piperine-python/tests/live_optimize_example.rs:38:57:  |
| piperine-python | live_session | 0 passed; 1 failed | thread 'live_session_compiles_once_across_set_op_loop' (216634) panicked at crates/piperine-python/tests/live_session.rs:56:30:  |
| piperine-python | margins_surface | 0 passed; 3 failed | thread 'loud_margin_reads_fail_identically_through_python' (216654) panicked at crates/piperine-python/tests/margins_surface.rs:21:5:  |
| piperine-python | naming_cleanup | 0 passed; 1 failed | thread 'naming_cleanup_and_properties' (216676) panicked at crates/piperine-python/tests/naming_cleanup.rs:91:5:  |
| piperine-python | net_ref_enums | 0 passed; 1 failed | thread 'cross_direction_and_scale_enums' (216696) panicked at crates/piperine-python/tests/net_ref_enums.rs:82:5:  |
| piperine-python | noise | 0 passed; 1 failed |   |
| piperine-python | opvar_host | 0 passed; 1 failed | thread 'opvar_and_opvars_are_readable_from_python' (216741) panicked at crates/piperine-python/tests/opvar_host.rs:76:30:  |
| piperine-python | pz_parity | 0 passed; 1 failed | thread 'python_pz_matches_rust_pz' (216761) panicked at crates/piperine-python/tests/pz_parity.rs:44:14:  |
| piperine-python | scripted_plugin | 4 passed; 1 failed | thread 'all_five_hooks_fire_with_their_payloads' (216781) panicked at crates/piperine-python/tests/scripted_plugin.rs:64:6:  |
| piperine-python | sens_parity | 0 passed; 2 failed | thread 'python_sens_matches_rust_sens' (216827) panicked at crates/piperine-python/tests/sens_parity.rs:42:6:  |
| piperine-python | sensitivity_parity | 0 passed; 2 failed | thread 'python_sensitivity_matches_rust_sensitivity' (216847) panicked at crates/piperine-python/tests/sensitivity_parity.rs:40:10:  |
| piperine-python | session_analyses | 0 passed; 1 failed | thread 'session_exposes_sens_pz_disto_tf_dc_with_typed_results' (216868) panicked at crates/piperine-python/tests/session_analyses.rs:78:5:  |
| piperine-python | simulation_error | 2 passed; 3 failed | thread 'non_converging_run_raises_convergence_error' (216890) panicked at crates/piperine-python/tests/simulation_error.rs:161:5:  |
| piperine-python | smoke | 0 passed; 1 failed | thread 'uniform_shape_matches_rust_host' (216912) panicked at crates/piperine-python/tests/smoke.rs:194:5:  |
| piperine-python | sp_parity | 0 passed; 1 failed | thread 'python_sp_matches_rust_sp' (216949) panicked at crates/piperine-python/tests/sp_parity.rs:36:10:  |
| piperine-python | typed_configs | 2 passed; 2 failed | thread 'session_dc_accepts_nodeset' (216970) panicked at crates/piperine-python/tests/typed_configs.rs:131:5:  |
| piperine-python | waveform | 0 passed; 2 failed |   |
| piperine-solver | adjoint_dc | 0 passed; 3 failed | thread 'one_transpose_solve_serves_every_requested_parameter' (217020) panicked at crates/piperine-solver/tests/adjoint_dc.rs:65:10:  |
| piperine-solver | adjoint_vs_sens | 0 passed; 4 failed | thread 'adjoint_matches_sens_on_a_nonlinear_diode_circuit' (217042) panicked at crates/piperine-solver/tests/adjoint_vs_sens.rs:94:10:  |
| piperine-solver | pss | 0 passed; 3 failed | thread 'sine_rc_converges_to_the_analytic_phasor' (217065) panicked at crates/piperine-solver/tests/pss.rs:105:10:  |
| piperine-solver | sens | 0 passed; 4 failed | thread 'divider_sensitivities_match_analytic_values' (217086) panicked at crates/piperine-solver/tests/sens.rs:78:10:  |
| piperine-solver | transient_reentry | 0 passed; 1 failed | thread 'reentry_from_captured_state_matches_continuous_run' (217108) panicked at crates/piperine-solver/tests/transient_reentry.rs:71:71:  |

### All fully-passing targets (109)

| Crate | Target | Result |
|---|---|---|
| piperine | delegation_guard | 2 passed |
| piperine | host_complex_waveform_margins | 7 passed |
| piperine | host_waveform_transforms | 8 passed |
| piperine | ngspice_ac | 24 passed |
| piperine | ngspice_homotopy | 18 passed |
| piperine | ngspice_noise | 29 passed |
| piperine | ngspice_parity_matrix | 4 passed |
| piperine | ngspice_temp | 24 passed |
| piperine | ngspice_validation | 49 passed |
| piperine | run_examples | 2 passed |
| piperine | spice_smoke | 7 passed |
| piperine | vams_annex_e | 9 passed |
| piperine | vams_connect_modules_e2e | 1 passed |
| piperine | vams_vertical_slice | 3 passed |
| piperine-api | host_device_e2e | 5 passed |
| piperine-api | sequencer_crossing_waits | 3 passed |
| piperine-api | sequencer_drive | 3 passed |
| piperine-api | sequencer_edge_waits | 2 passed |
| piperine-api | sequencer_expect_posture | 5 passed |
| piperine-api | sequencer_time_wakes | 2 passed |
| piperine-api | sequencer_wake_event_kind_mapping | 3 passed |
| piperine-cli | add_cmd | 6 passed |
| piperine-cli | build_cmd | 5 passed |
| piperine-cli | check_cmd | 1 passed |
| piperine-cli | test_tb | 11 passed |
| piperine-codegen | vams_gates | 4 passed |
| piperine-codegen | vams_udp | 3 passed |
| piperine-ir | behavior_extensibility | 1 passed |
| piperine-ir | diff | 12 passed |
| piperine-ir | interp | 15 passed |
| piperine-ir | measure_replay_identity | 3 passed |
| piperine-ir | parse | 17 passed |
| piperine-ir | print | 8 passed |
| piperine-ir | purity | 2 passed |
| piperine-ir | verify | 20 passed |
| piperine-lang | annex_a_grammar | 3 passed |
| piperine-lang | annex_b_keywords | 5 passed |
| piperine-lang | ast | 12 passed |
| piperine-lang | corpus | 3 passed |
| piperine-lang | errors | 9 passed |
| piperine-lang | errors_lrm | 7 passed |
| piperine-lang | expressions | 21 passed |
| piperine-lang | extern_coverage_guard | 2 passed |
| piperine-lang | fmt | 11 passed |
| piperine-lang | items_behavior | 23 passed |
| piperine-lang | items_core | 23 passed |
| piperine-lang | items_remaining | 16 passed |
| piperine-lang | lexical | 29 passed |
| piperine-lang | parser | 7 passed |
| piperine-lang | pom_branch | 13 passed |
| piperine-lang | pom_nature | 9 passed |
| piperine-lang | pom_vams_shape | 8 passed |
| piperine-lang | preprocessor | 23 passed |
| piperine-lang | statements | 20 passed |
| piperine-lang | vams_annex_c | 5 passed |
| piperine-lang | vams_connect_modules | 2 passed |
| piperine-lang | vams_const_eval_calls | 3 passed |
| piperine-lang | vams_deviation_resolutions | 6 passed |
| piperine-lang | vams_device_port_attrs | 5 passed |
| piperine-lang | vams_discipline_resolution | 4 passed |
| piperine-lang | vams_generate | 5 passed |
| piperine-lang | vams_headers | 4 passed |
| piperine-lang | vams_lower_analog | 8 passed |
| piperine-lang | vams_lower_structure | 13 passed |
| piperine-lang | vams_lower_var | 4 passed |
| piperine-lang | vams_param_ranges | 6 passed |
| piperine-lang | vams_system_tasks | 7 passed |
| piperine-plugin | macro_collisions | 1 passed |
| piperine-plugin | native_smoke | 4 passed |
| piperine-plugin | release_fetch | 8 passed |
| piperine-plugin | scripts | 1 passed |
| piperine-plugin | trust | 4 passed |
| piperine-plugin | vpi_routines | 4 passed |
| piperine-plugin-macros | registration | 2 passed |
| piperine-project | release | 4 passed |
| piperine-python | embed | 1 passed |
| piperine-python | facade_hygiene | 1 passed |
| piperine-python | pyi_stub | 3 passed |
| piperine-python | units | 4 passed |
| piperine-solver | abi_surface | 1 passed |
| piperine-solver | capabilities_contract | 4 passed |
| piperine-solver | check_posture | 3 passed |
| piperine-solver | checkpoint_restore | 8 passed |
| piperine-solver | composed_element | 2 passed |
| piperine-solver | context_init | 1 passed |
| piperine-solver | digital_events_unified | 6 passed |
| piperine-solver | digital_settle_subscriber | 4 passed |
| piperine-solver | digital_topology | 27 passed |
| piperine-solver | event_adapters | 8 passed |
| piperine-solver | event_rollback | 2 passed |
| piperine-solver | failure_rules | 6 passed |
| piperine-solver | lifecycle | 9 passed |
| piperine-solver | limiting_report | 2 passed |
| piperine-solver | live_params | 13 passed |
| piperine-solver | margin_eval_points | 3 passed |
| piperine-solver | mixed_signal | 25 passed |
| piperine-solver | parity_baseline | 6 passed |
| piperine-solver | prelude_surface | 1 passed |
| piperine-solver | probe_selection | 10 passed |
| piperine-solver | sensitivity_fallback | 2 passed |
| piperine-solver | solver_entry | 2 passed |
| piperine-solver | spec_failure_rules_guard | 3 passed |
| piperine-solver | stamp_bypass | 2 passed |
| piperine-solver | stepper_fold | 3 passed |
| piperine-solver | temperature | 3 passed |
| piperine-solver | validation_findings | 3 passed |
| piperine-solver | validation_gating | 6 passed |
| piperine-solver | validation_postures | 4 passed |
| piperine-solver | vams_stratified_regions | 2 passed |

### Skipped / timed out (1)

| Crate | Target | Status |
|---|---|---|
| piperine-plugin-macros | compile_fail | TIMEOUT (trybuild rebuild >20min in this sandbox; killed) |

## Cross-check

269 executed + 28 compile-fail = 297 = the on-disk test-file count from
`ls crates/*/tests/*.rs tests/*.rs`. 109 fully-passing + 159 with failures +
1 timeout = 269. No target is unaccounted for.

## Regenerating this baseline

The raw per-target data this file was generated from (JSON build log,
per-target `test result:` lines, first-failure lines) is not checked in
(scratch artifacts only) — regenerate with:

```sh
cargo build --workspace --tests --keep-going --message-format=json > build.ndjson
# then execute each successfully-built test binary from its own crate's
# manifest directory (see Methodology note above for why cwd matters).
```
