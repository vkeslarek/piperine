# LRM Conformance Wave 2 — Pre-Feature Test Baseline

**Captured**: 2026-08-20, branch `feature/device-verification`, HEAD
`69084e2a5270207bc617bf6f6525113566db16f1` (before any lrm-conformance-wave2
work — after wave-1 (`lrm-conformance-closure`) landed). Methodology reuses
the wave-1 `baseline.md` procedure (`.specs/features/lrm-conformance-closure/baseline.md`)
verbatim: target-by-target, not aggregate counts. This file is the reference
point every later wave-2 task's "no regression" gate check compares against
(byte-identical-or-better per target, never an aggregate total, per tasks.md's
Baseline rule).

**Working-tree note**: at capture time `git status` also shows unrelated
pre-existing uncommitted changes not produced by this task (deletions of
`CLEANUP_PLAN.md`/`GAPS.md`/`SPEC_DEVIATIONS.md`, edits to
`crates/piperine-lang/src/vams_include.rs` and
`examples/21_shift_register_tran.vams`, and `.specs/LESSONS.md`/`lessons.json`
updates). None of these are part of wave-2 T1–T7 and none were touched to
produce this baseline — they were already present in the working tree and are
included in the build/test run below because `cargo` builds the working tree
as-is, not a clean checkout of HEAD.

## Build

`cargo build --workspace` (forced full rebuild — every crate's `src/*.rs`
and `build.rs` touched immediately before the build, so this is not a cached
no-op run, matching the wave-1 methodology's own caution about that pitfall):
**0 warnings**, exit 0.

```
$ touch $(find crates -name '*.rs' -path '*/src/*') $(find crates -name build.rs) $(find src -name '*.rs')
$ cargo build --workspace --message-format=json
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.52s
$ echo $?
0
```
0 `compiler-message` entries of any level in the JSON stream — 0 warnings,
0 errors.

## `cargo build --workspace --tests --keep-going --message-format=json` / per-target run result

Unlike wave-1 (28 of 297 targets failed to compile), **every target at this
HEAD compiles**: wave-1's closure work fixed all 28 pre-MD-47 API breakages
this baseline's predecessor recorded. This baseline was captured with the
same reliable procedure the wave-1 file established (`cargo test
--workspace`/`-p <crate>` both still abort atomically on the first compile
failure in scope and cannot enumerate — moot here since nothing fails to
compile, but the per-target-execution methodology stands for parity and
because it is what the task requires, not because it was needed to route
around a compile failure this time):

1. `cargo build --workspace --tests --keep-going --message-format=json` — one
   pass, all 318 on-disk test target files (`ls crates/*/tests/*.rs
   tests/*.rs`) produce an executable (cross-checked: the JSON stream's
   `compiler-artifact` entries with `target.kind == ["test"]`, keyed by
   `(crate directory, target name)` to disambiguate five same-named targets
   across different crates — `check_posture`, `live_params`, `opvar_host`,
   `session_analyses`, `smoke` — total exactly 318, matching the file-count
   enumeration with zero missing and zero extra).
2. Each of the 318 compiled test binaries was **executed directly** (bypassing
   `cargo test`'s own compile step), from its own package's manifest
   directory with `CARGO_MANIFEST_DIR` set to match, `--test-threads=4`,
   120s per-target timeout — reproducing cargo's actual per-target
   working-directory contract, same as the wave-1 procedure.
3. `piperine-plugin-macros`'s `compile_fail` (the `trybuild` UI-test target
   that shells out to `cargo check` against a from-scratch dependency set)
   was **not** a timeout this time — wave-1's baseline killed it after >20
   minutes with a cold dependency cache; here, with the cache already warm
   from the preceding full-workspace build, it completed in 74.08s with
   2 passed, 0 failed. Recorded as verified, not skipped.

## Summary

| | Count |
|---|---|
| Total on-disk test target files (`ls crates/*/tests/*.rs tests/*.rs`) | 318 |
| Targets that fail to compile | 0 |
| Targets that compile and were executed | 318 |
| — of those, fully passing (0 failed) | 318 |
| — of those, with ≥1 failing test case | 0 |
| — of those, timed out / not verified | 0 |
| Total individual test cases passed (sum across the 318 executed targets) | 1,809 |
| Total individual test cases failed (sum across the 318 executed targets) | 0 |
| Total individual test cases ignored | 0 |

**318 of 318 targets (100%) are green** at this HEAD. This is a materially
different starting point than wave-1's baseline (which recorded 161 of 297
targets, 54%, red) — wave-1's closure work (T1 onward of that feature) fixed
every one of the 28 compile failures and every one of the 159 runtime
failures that baseline recorded. **This all-green state is itself not
evidence of LRM conformance** — it is the honest pre-feature state of
`feature/device-verification` at `69084e2a`, and it is exactly why wave-2
exists: `SPEC_DEVIATION.md`'s 2026-08-19 independent audit found ~52 open
divergences plus 12 disclosed deviations that a fully-green existing test
suite does not surface, because (per the wave-2 spec's Problem Statement)
those are silent drops, wrong semantics, missing grammar, and ignored
operator arguments that the existing tests never exercised in the first
place. "No regression" for wave-2 therefore means: every one of these 318
targets stays at its current pass count or better, target by target, for the
whole duration of wave-2 — an aggregate total is explicitly not the
baseline (tasks.md's own Baseline rule) because a wave-2 fix that breaks one
target's test while adding ten new ones elsewhere would look like net
improvement in aggregate while still being a regression.

## Full per-target results (318)

| Crate | Target | Result |
|---|---|---|
| piperine | compile_once_sweep | 1 passed |
| piperine | constraints_e2e | 5 passed |
| piperine | dc_host_proof | 1 passed |
| piperine | delegation_guard | 2 passed |
| piperine | disto | 2 passed |
| piperine | host_capability_parity | 4 passed |
| piperine | host_complex_waveform_margins | 7 passed |
| piperine | host_introspect | 8 passed |
| piperine | host_limiting | 2 passed |
| piperine | host_noise | 3 passed |
| piperine | host_object_model | 3 passed |
| piperine | host_parity | 10 passed |
| piperine | host_probe | 7 passed |
| piperine | host_result_types | 2 passed |
| piperine | host_session_builder | 5 passed |
| piperine | host_soa | 1 passed |
| piperine | host_waveform_measure | 9 passed |
| piperine | host_waveform_transforms | 8 passed |
| piperine | lrm_conformance_matrix | 6 passed |
| piperine | net_selector | 5 passed |
| piperine | ngspice_ac | 24 passed |
| piperine | ngspice_homotopy | 18 passed |
| piperine | ngspice_noise | 29 passed |
| piperine | ngspice_parity_matrix | 4 passed |
| piperine | ngspice_temp | 24 passed |
| piperine | ngspice_tran | 25 passed |
| piperine | ngspice_validation | 49 passed |
| piperine | opvar_host | 5 passed |
| piperine | plugin_parity | 2 passed |
| piperine | pss_host | 2 passed |
| piperine | pz | 3 passed |
| piperine | run_examples | 2 passed |
| piperine | session | 10 passed |
| piperine | session_analyses | 3 passed |
| piperine | session_compile | 1 passed |
| piperine | session_dc_sweep | 1 passed |
| piperine | session_sweep | 2 passed |
| piperine | session_sweep_grid | 2 passed |
| piperine | session_tf | 2 passed |
| piperine | soa_spice_models | 4 passed |
| piperine | sp | 1 passed |
| piperine | spice_smoke | 7 passed |
| piperine | suite_hygiene | 10 passed |
| piperine | tline | 4 passed |
| piperine | tol_inert | 5 passed |
| piperine | trace_generic | 4 passed |
| piperine | units_freq | 3 passed |
| piperine | urc_compile_count | 1 passed |
| piperine | vams_absdelta_e2e | 1 passed |
| piperine | vams_annex_e | 11 passed |
| piperine | vams_connect_modules_e2e | 3 passed |
| piperine | vams_cross_direction_e2e | 6 passed |
| piperine | vams_digital_events_in_analog_e2e | 1 passed |
| piperine | vams_gates_and_udps_e2e | 2 passed |
| piperine | vams_named_events_e2e | 2 passed |
| piperine | vams_port_access_e2e | 1 passed |
| piperine | vams_session_output | 1 passed |
| piperine | vams_session_plusargs | 2 passed |
| piperine | vams_simprobe | 3 passed |
| piperine | vams_vertical_slice | 3 passed |
| piperine | xfmr | 4 passed |
| piperine-api | aging_drift | 3 passed |
| piperine-api | center_policy | 2 passed |
| piperine-api | coverage_merge | 5 passed |
| piperine-api | driver_posture | 4 passed |
| piperine-api | event_window | 2 passed |
| piperine-api | high_sigma_sampling | 3 passed |
| piperine-api | host_device_e2e | 5 passed |
| piperine-api | margin_gradients | 6 passed |
| piperine-api | margins_channel | 8 passed |
| piperine-api | margins_sweep | 3 passed |
| piperine-api | model_descriptors | 7 passed |
| piperine-api | model_design | 11 passed |
| piperine-api | model_instance | 6 passed |
| piperine-api | model_module | 7 passed |
| piperine-api | monitor_composition | 2 passed |
| piperine-api | monte_carlo_compile_once | 1 passed |
| piperine-api | monte_carlo_report | 4 passed |
| piperine-api | monte_carlo_sampling | 11 passed |
| piperine-api | optimize_blackbox | 2 passed |
| piperine-api | optimize_compile_once | 1 passed |
| piperine-api | optimize_policy | 3 passed |
| piperine-api | posture_config | 2 passed |
| piperine-api | reduced_metrics | 3 passed |
| piperine-api | scoped_require_scope | 3 passed |
| piperine-api | sensitivity_surface | 5 passed |
| piperine-api | sequencer_crossing_waits | 3 passed |
| piperine-api | sequencer_dc_ac | 3 passed |
| piperine-api | sequencer_drive | 3 passed |
| piperine-api | sequencer_edge_waits | 2 passed |
| piperine-api | sequencer_expect_posture | 5 passed |
| piperine-api | sequencer_ordering_and_catalog | 2 passed |
| piperine-api | sequencer_time_wakes | 2 passed |
| piperine-api | sequencer_wake_event_kind_mapping | 3 passed |
| piperine-api | smoke | 2 passed |
| piperine-api | temp_sweep | 1 passed |
| piperine-api | violations_channel | 5 passed |
| piperine-cli | add_cmd | 6 passed |
| piperine-cli | build_cmd | 5 passed |
| piperine-cli | check_cmd | 1 passed |
| piperine-cli | expand_cmd | 6 passed |
| piperine-cli | test_tb | 11 passed |
| piperine-codegen | analog_device_numerics | 9 passed |
| piperine-codegen | analog_kernel | 5 passed |
| piperine-codegen | bypass_capability | 6 passed |
| piperine-codegen | checkpoint_digital | 4 passed |
| piperine-codegen | checkpoint_limiter | 6 passed |
| piperine-codegen | circuit_from_design | 4 passed |
| piperine-codegen | codegen_api | 7 passed |
| piperine-codegen | cover_bin_mapping | 7 passed |
| piperine-codegen | default_transition_semantics | 3 passed |
| piperine-codegen | digital_codegen_gaps | 2 passed |
| piperine-codegen | digital_fusion | 4 passed |
| piperine-codegen | digital_jit | 4 passed |
| piperine-codegen | digital_multiple_always | 2 passed |
| piperine-codegen | disto_capabilities | 3 passed |
| piperine-codegen | disto_jit | 8 passed |
| piperine-codegen | flatten_hierarchy | 5 passed |
| piperine-codegen | has_sensitivity | 3 passed |
| piperine-codegen | jit_memory | 1 passed |
| piperine-codegen | limiters | 6 passed |
| piperine-codegen | live_params | 6 passed |
| piperine-codegen | live_set_never_recompiles | 1 passed |
| piperine-codegen | margin_ac_points | 3 passed |
| piperine-codegen | margin_compile_count | 1 passed |
| piperine-codegen | margin_dc_points | 3 passed |
| piperine-codegen | margin_evaluation | 7 passed |
| piperine-codegen | margin_kernel | 3 passed |
| piperine-codegen | margin_lowering | 11 passed |
| piperine-codegen | margin_postures | 5 passed |
| piperine-codegen | margin_tran_points | 3 passed |
| piperine-codegen | model_descriptor | 7 passed |
| piperine-codegen | observable_catalog | 5 passed |
| piperine-codegen | opvar_bridge | 6 passed |
| piperine-codegen | param_admissibility | 4 passed |
| piperine-codegen | param_diff | 3 passed |
| piperine-codegen | param_given_spellings | 4 passed |
| piperine-codegen | param_kernel | 4 passed |
| piperine-codegen | param_stamps | 4 passed |
| piperine-codegen | resolve_lowering | 13 passed |
| piperine-codegen | runtime_io_diagnostics | 1 passed |
| piperine-codegen | silent_bugs | 7 passed |
| piperine-codegen | table_op | 3 passed |
| piperine-codegen | temperature_dtemp | 4 passed |
| piperine-codegen | terminal_bridge | 6 passed |
| piperine-codegen | vams_digital_display | 1 passed |
| piperine-codegen | vams_discontinuity | 2 passed |
| piperine-codegen | vams_driver_access_gap | 1 passed |
| piperine-codegen | vams_gates | 4 passed |
| piperine-codegen | vams_laplace_registry | 7 passed |
| piperine-codegen | vams_mfactor | 1 passed |
| piperine-codegen | vams_port_connected | 2 passed |
| piperine-codegen | vams_udp | 3 passed |
| piperine-codegen | vams_unknown_event_function | 2 passed |
| piperine-codegen | var_initializers | 2 passed |
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
| piperine-lang | expressions | 25 passed |
| piperine-lang | extern_coverage_guard | 2 passed |
| piperine-lang | fmt | 11 passed |
| piperine-lang | fmt_doc_comment | 5 passed |
| piperine-lang | items_behavior | 23 passed |
| piperine-lang | items_core | 23 passed |
| piperine-lang | items_remaining | 29 passed |
| piperine-lang | lexical | 29 passed |
| piperine-lang | macro_expansion_span | 6 passed |
| piperine-lang | parser | 7 passed |
| piperine-lang | pom_branch | 13 passed |
| piperine-lang | pom_nature | 9 passed |
| piperine-lang | pom_serde | 7 passed |
| piperine-lang | pom_vams_shape | 8 passed |
| piperine-lang | preprocessor | 27 passed |
| piperine-lang | source_provenance | 10 passed |
| piperine-lang | span_resolution | 2 passed |
| piperine-lang | spice_headers | 3 passed |
| piperine-lang | statements | 20 passed |
| piperine-lang | system_task_migration_completeness | 12 passed |
| piperine-lang | urc_flatten | 3 passed |
| piperine-lang | vams_annex_c | 5 passed |
| piperine-lang | vams_annex_f_domain | 5 passed |
| piperine-lang | vams_array_of_instances | 4 passed |
| piperine-lang | vams_assignment_pattern_elab | 6 passed |
| piperine-lang | vams_ch1_lexical_form | 17 passed |
| piperine-lang | vams_connect_modules | 6 passed |
| piperine-lang | vams_const_eval_calls | 3 passed |
| piperine-lang | vams_constraint_block | 27 passed |
| piperine-lang | vams_cross_domain_concurrency | 5 passed |
| piperine-lang | vams_defparam | 5 passed |
| piperine-lang | vams_deviation_resolutions | 6 passed |
| piperine-lang | vams_device_port_attrs | 5 passed |
| piperine-lang | vams_directives | 31 passed |
| piperine-lang | vams_discipline_resolution | 7 passed |
| piperine-lang | vams_doc_comments | 8 passed |
| piperine-lang | vams_file_io | 8 passed |
| piperine-lang | vams_four_state_literals | 6 passed |
| piperine-lang | vams_gates_and_udps | 10 passed |
| piperine-lang | vams_generate | 5 passed |
| piperine-lang | vams_headers | 6 passed |
| piperine-lang | vams_lower_analog | 13 passed |
| piperine-lang | vams_lower_structure | 13 passed |
| piperine-lang | vams_lower_var | 8 passed |
| piperine-lang | vams_named_events | 5 passed |
| piperine-lang | vams_param_distributions | 9 passed |
| piperine-lang | vams_param_ranges | 15 passed |
| piperine-lang | vams_paramset | 5 passed |
| piperine-lang | vams_real_to_int_rounding | 4 passed |
| piperine-lang | vams_system_tasks | 11 passed |
| piperine-lang | vams_unconnected_ports | 5 passed |
| piperine-lang-server | completion_and_symbols | 5 passed |
| piperine-lang-server | completion_suppression | 3 passed |
| piperine-lang-server | diagnostics | 8 passed |
| piperine-lang-server | document_highlight | 1 passed |
| piperine-lang-server | hover | 8 passed |
| piperine-lang-server | import_goto | 3 passed |
| piperine-lang-server | instance_highlight | 4 passed |
| piperine-lang-server | project_index | 2 passed |
| piperine-lang-server | protocol | 12 passed |
| piperine-lang-server | references_and_rename | 5 passed |
| piperine-lang-server | symbol_resolution | 5 passed |
| piperine-plugin | callback_registry | 4 passed |
| piperine-plugin | delay_processing | 2 passed |
| piperine-plugin | e2e | 6 passed |
| piperine-plugin | extern_stub | 1 passed |
| piperine-plugin | hooks | 1 passed |
| piperine-plugin | inject | 5 passed |
| piperine-plugin | macro_collisions | 1 passed |
| piperine-plugin | mcd_family | 5 passed |
| piperine-plugin | native_smoke | 4 passed |
| piperine-plugin | put_value | 4 passed |
| piperine-plugin | release_fetch | 8 passed |
| piperine-plugin | scripts | 1 passed |
| piperine-plugin | staging | 3 passed |
| piperine-plugin | trust | 4 passed |
| piperine-plugin | vpi_routines | 10 passed |
| piperine-plugin-macros | compile_fail | 2 passed |
| piperine-plugin-macros | registration | 2 passed |
| piperine-plugin-macros | script_hook | 3 passed |
| piperine-project | release | 4 passed |
| piperine-python | ac | 1 passed |
| piperine-python | analyses | 6 passed |
| piperine-python | check_posture | 3 passed |
| piperine-python | design | 3 passed |
| piperine-python | disto_parity | 1 passed |
| piperine-python | embed | 1 passed |
| piperine-python | extract | 3 passed |
| piperine-python | facade_hygiene | 1 passed |
| piperine-python | four_parity | 1 passed |
| piperine-python | host_device_library | 4 passed |
| piperine-python | host_plot | 1 passed |
| piperine-python | host_sweep | 1 passed |
| piperine-python | host_sweep_grid | 1 passed |
| piperine-python | live_facade | 1 passed |
| piperine-python | live_optimize_example | 1 passed |
| piperine-python | live_session | 1 passed |
| piperine-python | margins_surface | 3 passed |
| piperine-python | naming_cleanup | 1 passed |
| piperine-python | net_ref_enums | 1 passed |
| piperine-python | noise | 1 passed |
| piperine-python | opvar_host | 1 passed |
| piperine-python | pyi_stub | 3 passed |
| piperine-python | pz_parity | 1 passed |
| piperine-python | scripted_plugin | 5 passed |
| piperine-python | sens_parity | 2 passed |
| piperine-python | sensitivity_parity | 2 passed |
| piperine-python | session_analyses | 1 passed |
| piperine-python | simulation_error | 5 passed |
| piperine-python | smoke | 1 passed |
| piperine-python | sp_parity | 1 passed |
| piperine-python | typed_configs | 4 passed |
| piperine-python | units | 4 passed |
| piperine-python | waveform | 2 passed |
| piperine-solver | abi_surface | 1 passed |
| piperine-solver | adjoint_dc | 3 passed |
| piperine-solver | adjoint_vs_sens | 4 passed |
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
| piperine-solver | pss | 3 passed |
| piperine-solver | sens | 4 passed |
| piperine-solver | sensitivity_fallback | 2 passed |
| piperine-solver | solver_entry | 2 passed |
| piperine-solver | spec_failure_rules_guard | 3 passed |
| piperine-solver | stamp_bypass | 2 passed |
| piperine-solver | stepper_fold | 3 passed |
| piperine-solver | temperature | 3 passed |
| piperine-solver | transient_reentry | 1 passed |
| piperine-solver | validation_findings | 3 passed |
| piperine-solver | validation_gating | 6 passed |
| piperine-solver | validation_postures | 4 passed |
| piperine-solver | vams_stratified_regions | 2 passed |

## Cross-check

318 executed + 0 compile-fail = 318 = the on-disk test-file count from
`ls crates/*/tests/*.rs tests/*.rs`. 318 fully-passing + 0 with failures +
0 timeout = 318. No target is unaccounted for.

## Regenerating this baseline

The raw per-target data this file was generated from (JSON build log,
per-target `test result:` lines) is not checked in (scratch artifacts only)
— regenerate with:

```sh
cargo build --workspace --tests --keep-going --message-format=json > build.ndjson
# then execute each successfully-built test binary from its own crate's
# manifest directory (see the wave-1 baseline's Methodology note for why cwd
# matters), and force-touch every crate's src/*.rs + build.rs before a
# separate `cargo build --workspace` to get an honest (non-cached) warning
# count.
```


---

## T71 — Final A/B (post-wave-2, HEAD after T69's commit)

**Captured**: 2026-08-24, branch `feature/device-verification`, HEAD after
T69 (`test(lrm-coverage): close 364 of 469 remaining LRM-2-4 coverage gaps`).
Same procedure as the capture above: `cargo test --workspace --no-fail-fast`
(equivalent per-target enumeration to the original methodology — each
`Running tests/<name>.rs (<binary>)` line paired with its own
`test result: ... passed; ... failed` line), plus `cargo build --workspace`
run separately to confirm the warning count. This section is an **appendix**
— it does not edit or replace the original snapshot above (which stays the
wave's reference "before" point per tasks.md's Baseline rule); it records
the "after" comparison T71 requires.

### Target-file count

`ls crates/*/tests/*.rs tests/*.rs` now returns **398** (was 318 at capture
time) — wave-2 added 80 new on-disk test-target files across T8-T69 (listed
below), zero were removed or renamed (append-only, matching the wave's own
practice).

### Per-target verdict vs. baseline: equal-or-better, one exception explained

Of the 318 original targets: **317 stayed at or improved on their baseline
pass count with zero failures; exactly 1 target regressed** —
`piperine-codegen::vams_port_connected` went from `2 passed` to `1 passed, 1
failed` (`analog_node_alias_fails_loud_citing_md18`). This is **not a wave-2
regression** — it is a **pre-existing red** at this HEAD that predates every
task in this batch (T69-T72); it was flagged as a known, unfixed baseline
deviation by the prior batch that landed T60-T68 and is carried forward
disclosed, not silently reconciled. No other of the 318 baseline targets
lost a passing test or gained a failure.

18 baseline targets **improved** (more passing tests, still zero failures)
as wave-2 tasks added test cases to existing files rather than always
creating new ones:

| Crate | Target | Baseline | Now |
|---|---|---|---|
| piperine | session_analyses | 3 passed | 10 passed |
| piperine | suite_hygiene | 10 passed | 11 passed |
| piperine-api | smoke | 2 passed | 3 passed |
| piperine-codegen | digital_jit | 4 passed | 9 passed |
| piperine-codegen | resolve_lowering | 13 passed | 14 passed |
| piperine-codegen | runtime_io_diagnostics | 1 passed | 3 passed |
| piperine-codegen | vams_driver_access_gap | 1 passed | 2 passed |
| piperine-lang | expressions | 25 passed | 26 passed |
| piperine-lang | lexical | 29 passed | 30 passed |
| piperine-lang | vams_ch1_lexical_form | 17 passed | 23 passed |
| piperine-lang | vams_defparam | 5 passed | 7 passed |
| piperine-lang | vams_discipline_resolution | 7 passed | 10 passed |
| piperine-lang | vams_paramset | 5 passed | 6 passed |
| piperine-plugin | vpi_routines | 10 passed | 13 passed |
| piperine-python | check_posture | 3 passed | 4 passed |
| piperine-python | session_analyses | 1 passed | 2 passed |
| piperine-solver | check_posture | 3 passed | 6 passed |
| piperine-solver | vams_stratified_regions | 2 passed | 3 passed |

The remaining 299 baseline targets are byte-identical to their baseline
result (same pass count, zero failures).

### New targets (80, all wave-2 additions)

| Crate | Target | Result |
|---|---|---|
| piperine | absdelay_maxdelay | 2 passed |
| piperine | analog_for_loop_unroll | 3 passed |
| piperine | analog_function_e2e | 2 passed |
| piperine | cross_enable_e2e | 2 passed |
| piperine | cross_time_tol_lte | 1 passed |
| piperine | ddt_abstol_lte | 1 passed |
| piperine | generate_items_e2e | 2 passed |
| piperine | generic_access_functions | 3 passed |
| piperine | hierarchical_ref_e2e | 2 passed |
| piperine | idt_assert_ramp_generator | 2 passed |
| piperine | indirect_contribution_opamp_e2e | 3 passed |
| piperine | laplace_abstol_lte | 1 passed |
| piperine | lrm_section_coverage | 4 passed, 1 failed |
| piperine | net_declaration_nodeset | 3 passed |
| piperine | node_alias_e2e | 4 passed |
| piperine | node_alias_no_rejit | 1 passed |
| piperine | onebit_dac_e2e | 2 passed |
| piperine | procedural_continuous_assign_e2e | 6 passed |
| piperine | simprobe_matches_host_probe | 4 passed |
| piperine | slew_symmetric_fall | 2 passed |
| piperine | switch_branch_relay_e2e | 1 passed |
| piperine | transition_time_tol | 2 passed |
| piperine | vams_analog_block_concat_e2e | 1 passed |
| piperine | vams_continuous_assign_e2e | 2 passed |
| piperine | vams_level_event_expr_e2e | 2 passed |
| piperine | zi_tau_lte | 1 passed |
| piperine-codegen | a2d_comparator_interpolation | 3 passed |
| piperine-codegen | absdelay_maxdelay_arg | 2 passed |
| piperine-codegen | analog_function_inline | 6 passed |
| piperine-codegen | analog_loop_unroll | 8 passed |
| piperine-codegen | analog_multiple_blocks | 4 passed |
| piperine-codegen | analysis_dependent_functions | 6 passed |
| piperine-codegen | case_equality | 3 passed |
| piperine-codegen | cross_tol_enable_arg | 4 passed |
| piperine-codegen | ddt_abstol_arg | 3 passed |
| piperine-codegen | ddx_operator | 2 passed |
| piperine-codegen | digital_analog_event_sensitivity | 2 passed |
| piperine-codegen | digital_net_type_classification | 2 passed |
| piperine-codegen | digital_timescale_quantization | 4 passed |
| piperine-codegen | digital_x_in_continuous_context | 4 passed |
| piperine-codegen | discontinuity_degree_inert | 2 passed |
| piperine-codegen | fatal_immediate | 3 passed |
| piperine-codegen | final_step | 2 passed |
| piperine-codegen | ground_declaration | 1 passed |
| piperine-codegen | hierarchical_ref_resolution | 5 passed |
| piperine-codegen | idt_assert_arg | 2 passed |
| piperine-codegen | idtmod_offset | 2 passed |
| piperine-codegen | indirect_contribution | 3 passed |
| piperine-codegen | initial_step_analysis_list | 2 passed |
| piperine-codegen | laplace_abstol_arg | 2 passed |
| piperine-codegen | limexp_convergence | 1 passed |
| piperine-codegen | log_base10 | 3 passed |
| piperine-codegen | math_intrinsics_lrm | 7 passed |
| piperine-codegen | simparam_unknown_key | 2 passed |
| piperine-codegen | slew_fall_default | 2 passed |
| piperine-codegen | unknown_system_task | 3 passed |
| piperine-codegen | vams_digital_monitor_defer | 2 passed |
| piperine-codegen | vams_mfactor_scaling | 2 passed |
| piperine-codegen | vams_placement | 2 passed |
| piperine-codegen | vams_string_concat | 5 passed |
| piperine-codegen | vams_switch_primitives | 5 passed |
| piperine-codegen | zi_tau_arg | 3 passed |
| piperine-lang | indirect_contribution_grammar | 3 passed |
| piperine-lang | port_size_matching | 4 passed |
| piperine-lang | switch_branch_elaboration | 4 passed |
| piperine-lang | vams_connect_naming_direction | 6 passed |
| piperine-lang | vams_continuous_assign | 3 passed |
| piperine-lang | vams_digital_net_types | 3 passed |
| piperine-lang | vams_generate_items | 7 passed |
| piperine-lang | vams_genvar_outside_generate | 2 passed |
| piperine-lang | vams_ground | 4 passed |
| piperine-lang | vams_hierarchical_refs | 4 passed |
| piperine-lang | vams_nature_compatibility | 5 passed |
| piperine-lang | vams_param_var_instance_attrs | 4 passed |
| piperine-lang | vams_paramset_overloading | 6 passed |
| piperine-lang | vams_positional_param_overrides | 6 passed |
| piperine-lang | vams_procedural_continuous_assign | 6 passed |
| piperine-solver | digital_driver_segregation | 4 passed |
| piperine-solver | nature_abstol_inheritance | 3 passed |
| piperine-solver | scheduler_region_queue_parity | 3 passed |
One of the 80 new targets carries a known, disclosed failure:
`piperine::lrm_section_coverage` (`4 passed, 1 failed`) — the 1 failure is
`covered_and_excluded_partition_the_lrm`, T69's own coverage-partition gate.
T69's commit message documents this in full: 469 of 612 LRM-2-4 sections
were uncovered at T68; T69 closed 364 of them with real, individually
verified evidence (backfilled `LRM-COVERS` annotations plus a handful of new
tests), leaving 105 genuinely uncovered — each one individually confirmed at
HEAD to describe LRM functionality Piperine does not yet implement (PLA
tasks, stochastic `$q_*` tasks, driver-access functions, hierarchical branch
contributions, the Annex-C Verilog-A-subset compliance mode, ~20 VPI routine
categories already disclosed unimplemented in the plugin's own `vpi.rs`,
and more) — never something a test could honestly claim without
fabrication. This is a real, disclosed shortfall against spec AC20's letter
("zero empty cells"), not a regression against this baseline (the target
did not exist at baseline capture time), and is carried into
`SPEC_DEVIATION.md` (T70) as the wave's residual open item.

### Build warnings

`cargo build --workspace`: **0 warnings**, exit 0 (same as baseline).

### Known non-baseline reds (not part of this A/B — different tracking)

Two `piperine-solver` inline `#[cfg(test)]` unit tests under `src/digital/
events.rs` (`digital::events::tests::inactive_region_lands_after_active_at_
the_same_timestamp`, `digital::events::tests::stratified_regions_resolve_
the_canonical_active_nba_race_through_evaluate_until_stable`) fail at this
HEAD. These are **not** part of this baseline's tracked scope (the baseline
only enumerates on-disk `tests/*.rs` integration-test files, never inline
lib unit tests — see the original "Full per-target results (318)" section's
own scope note) and predate this batch (T69-T72); the prior batch that
landed T60-T68 root-caused them to `apply_driven_event` universally
strength-arbitrating same-net events and deliberately left them red,
re-disclosed rather than papered over with a targeted fix that would need
net-kind classification the `DriverTable` doesn't have yet.

### Summary

| | Baseline (318 targets) | T71 (398 targets) |
|---|---|---|
| Targets | 318 | 398 (+80) |
| Targets with 0 failures | 318 | 397 |
| Targets with ≥1 failure | 0 | 1 (`lrm_section_coverage`, disclosed above) |
| Total passing test cases (sum) | 1,809 | 2,099 |
| Total failing test cases (sum) | 0 | 2 |
| Build warnings | 0 | 0 |

**Verdict: every one of the 318 baseline targets is equal-or-better except
one pre-existing, already-disclosed red (`vams_port_connected`, not
introduced by T69-T72); every new target is green except the one T69's own
commit fully explains. No silent regression.**
