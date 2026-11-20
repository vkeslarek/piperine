# Buck Scenarios verification

**Verdict**: FAIL - every check's test holds its criterion (no check-level failure, no weakened assertion, no tolerance of SPEC moved), but five findings are open: the S9 wait derivation in SPEC §10.1 names the wrong worst case (F1), the spec's budget and decision text is stale after the S9 change (F2), a `Ramp` smaller than half a step schedules nothing (F3), and two cleanliness defects in the code this feature added (F4, F5).
**Proofs**: `proofs.md` at edb08487 (79 pass, 0 fail); HEAD 7032bfaa differs from it only by `proofs.md`
**Verifier**: independent sub-agent (author != verifier)

How it was probed: read spec.md, design.md, tasks.md `## Checks`, proofs.md, red-scenarios.md, `examples/buck/SPEC.md` §3 to §10.1 and the diff `4f505e09..7032bfaa`; read every test the proofs select (`tests/buck_scenarios.rs`, `crates/piperine-parity/src/scenario/*.rs`, `buck_supervisor.rs`, `docs.rs`); checked the hiccup off-by-one fix by hand (entry at sample s0 with `retry = 999`, `SoftStart` at s0 + 1000); recomputed the three late SPEC derivations by hand from SPEC's own numbers (§3.1 duty, §5.2 duty word, §5.3 dead time, §6.2/§6.3 gains); recomputed the S9 timeline (last window ends 7.438 ms, `stop` 7.46 ms); a throwaway `piperine-parity` test (deleted) on `Ramp` and `Trace`/`DiscreteTrace` edges. The scenario run was not re-run (26 min; proofs.md holds it at edb08487, and HEAD changes no code). No solver, codegen or language source is in the diff (the only `piperine-lang` change is the `buck.txt` elaboration fixture following the hiccup fix), so the ngspice parity oracles are not reachable from this feature.

## The three late SPEC corrections

- **S2b, 1.38 ms ± 0.05 ms - sound.** The mechanism is SPEC's own: §5.3's 2 × 20 ns dead time freewheels through the high-side diode while the current is negative (the ramp) and through the low side after it, a shift of 2 · 20 ns / 2 µs · 13.4 V = 0.27 V = 72 duty LSBs at 12 V (§5.2) = 1 150 Q4 units, which KI = 8 per code per sample absorbs in ≈ 55 samples (110 µs) at a 2 to 3 code error: 1.28 ms + ≈ 0.1 ms. That is hand-checkable from SPEC's parameters and does not use the measured 1.360 ms. The averaged model without the term reproduces SPEC's old 1.284 ms, which is evidence the term is the whole difference. The ± 0.05 ms tolerance is unchanged (D9). Note for the user: the dead-time models gave 1.376 to 1.389 ms, and 1.38 was chosen over the closer 1.39; both centres contain 1.360 ms, so the choice does not decide the verdict. Changing the nominal value is a design choice the user has to confirm (the alternative is dead-time compensation in the DPWM).
- **S5, 26.5 to 30.5 µs and 16.5 µs ± 20 ns - sound, centre from an uncommitted model.** 16.5 µs = 8 × 2 µs + 500 ns is exact from §8.2/§4.1, and the test asserts it (`cut` latches at the wrap that ends a cut period, `dpwm.msl:77`, so `first rise − 2 µs` is the start of the first cut period). The old window was infeasible on SPEC's numbers: even at the 90 % clamp the first cut period starts at +6 µs, so 22.5 µs at best. The 28.5 µs centre comes from the diagnosis's scratch switched model (not committed). Hand-checking its period gains from the listed duty words ((D · 12 − 3.2) · 2 µs / 4.7 µH: 0.78, 1.75, 2.11, 1.96 A) gives 7.15 A at +12 µs, so the 7.5 A peak falls in that period. The ± 1 period is the quantization of the first cut, so the window admits exactly {26.5, 28.5, 30.5} µs. The window is not fitted to the measured value, but it rests on the same loop simulation the measurement came from.
- **S9, 450 µs wait and `stop` 7.46 ms - conclusion holds, derivation text wrong (F1).** The wait is a measurement instant SPEC never fixed. The S9 bounds (§4.3 totals, 10 mV, 1 code, 6.25 mV drift) do not move. The `stop` covers the plan: 264 µs start + 9 × 514 µs + 6 × 200 µs ramps + 802 µs + 546 µs setpoint moves = 7.438 ms. The §10.1 row's worst case is wrong, though. The ramp lag is Δu / (N · Ki), and the 9 V → 12 V ramp at 5.0 V, 5 A (corner 8) moves the duty word 5.2 · 3200 · (1/9 − 1/12) = 462 LSBs, a lag of 9.2 codes, not the 6.2 codes of 12 V → 9 V at 3.3 V that the row names as "at most". The 12 → 16 V ramp at 5.0 V (corner 9) gives 6.9 codes. Redone with 9.2 codes, the decay to ¼ code takes τ · ln(9.2 / 0.25) = 3.6 τ ≈ 375 to 400 µs, not 3.2 τ = 350 µs, so 450 µs is still enough on the derivation's own terms. The MSL run itself shows more residue than the derivation predicts: corner 9 at 450 µs (≈ 4 τ) still has a 2.78 mV half-window drift and 8.64 mV of ripple against 10 mV, while the derivation expects under 1.6 mV by 3.2 τ. The margin at corner 9 is thin, and the derivation does not explain it.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `buck_scenarios.rs` `the_package_declares_the_five_scenario_runs` | yes - path dependency text and the sorted run list compared exactly |
| C2 | `buck_scenarios.rs` `every_run_samples_the_thirteen_signals_on_a_10_ns_grid` | yes - top, both steps, streams, `max_steps` and the ordered print list per run |
| C3 | `buck_scenarios.rs` `every_run_accepts_the_five_ports_and_three_params` | yes - every drive and set unwrapped on every run |
| C4 | `buck_scenarios.rs` `the_scenario_package_checks_with_no_diagnostic` | yes - `check()` errors fail; package name, run count, typed asserted |
| C5 | `piperine-cli/tests/buck.rs` `the_buck_has_its_bench_and_two_runs` | yes |
| C6 | `buck_scenarios.rs` `opening_a_run_leaves_only_the_package_sources` | yes - full recursive file set compared while the session is open |
| C7 | `scenario::column::tests::a_missing_column_panics_naming_the_name_and_the_columns` | yes |
| C8 | `scenario::trace::tests::settled_into_is_the_first_row_from_which_every_row_stays_inside` | yes - re-entry case, last-row-outside none, all-inside; band edges inclusive (own probe) |
| C9 | `scenario::trace::tests::deviation_is_the_largest_absolute_difference_with_its_time` | yes - tie on both sides picks the first |
| C10 | `scenario::trace::tests::peak_to_peak_is_the_largest_minus_the_smallest` | yes |
| C11 | `scenario::trace::tests::mean_is_the_arithmetic_mean_of_the_rows` | yes - unequal spacing shows it is the row mean |
| C12 | `scenario::trace::tests::first_crossing_is_the_first_row_at_or_past_the_level_after_a_time` | yes - "at" counts, `after` honoured, none returned |
| C13 | `scenario::discrete::tests::first_time_is_the_first_row_holding_the_value_after_a_time` | yes |
| C14 | `scenario::discrete::tests::holds_names_the_first_row_that_departs` | yes - an `Unknown` row also departs (own probe) |
| C15 | `scenario::stimulus::tests::a_ramp_sets_once_per_equal_step_and_ends_on_its_target` | yes on the two named ramps; a ramp under half a step schedules nothing (F3) |
| C16 | `buck_scenarios.rs` `setpoint_regulates_at_3_3_v_with_pgood_before_its_first_stimulus` | yes - mode, pgood and vout at the 1.25 ms row |
| C17 | `buck_scenarios.rs` `s2_reference_reaches_800_codes_0_544_ms_after_the_write` | yes - ± 2 µs, first time after the write |
| C18 | `buck_scenarios.rs` `s2_vout_settles_into_5_v_within_1_percent_at_0_54_ms` | yes - settling over [write, 2.00 ms), panics when never settled |
| C19 | `buck_scenarios.rs` `s2_overshoot_is_at_most_15_mv` | yes - maximum over the whole window ≤ 5.015 V |
| C20 | `buck_scenarios.rs` `s2_pgood_stays_high` | yes - every row; an empty window panics |
| C21 | `buck_scenarios.rs` `s2b_reference_reaches_160_codes_1_28_ms_after_the_write` | yes |
| C22 | `buck_scenarios.rs` `s2b_vout_settles_into_1_v_within_1_percent_at_1_38_ms` | yes - holds the amended criterion with SPEC's ± 0.05 ms; the 1.38 ms nominal is derived from §5.3 (see above) and the user still has to confirm it |
| C23 | `buck_scenarios.rs` `s2b_inductor_current_goes_negative` | yes - minimum < 0 A and printed (the ≈ −0.6 A is report-only, D8) |
| C24 | `buck_scenarios.rs` `s2b_sink_limit_never_trips` | yes |
| C25 | `buck_scenarios.rs` `s2b_no_over_voltage_fault` | yes - fault and mode, every row |
| C26 | `buck_scenarios.rs` `setpoint_passes_every_check` | yes - `seen > 0` and `failed == 0` |
| C27 | `buck_scenarios.rs` `the_families_run_at_once_in_their_own_build_directories` | yes - latest start before earliest end proves all five overlap; distinct directories hold by construction (`tempdir`) |
| C28 | `buck_scenarios.rs` `every_family_reports_its_wall_time_and_rows` | yes |
| C29 | `scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run` | yes - the `OnceLock` in `Suite::get` gives the "never re-run" half |
| C30 | `buck_scenarios.rs` `load_line_regulates_at_3_3_v_with_pgood_before_its_first_stimulus` | yes |
| C31 | `buck_scenarios.rs` `s3_load_step_up_stays_within_165_mv` | yes |
| C32 | `buck_scenarios.rs` `s3_load_step_down_stays_within_165_mv` | yes |
| C33 | `buck_scenarios.rs` `s3_load_step_up_recovers_within_200_us` | yes |
| C34 | `buck_scenarios.rs` `s3_load_step_down_recovers_within_200_us` | yes |
| C35 | `buck_scenarios.rs` `s3_pgood_stays_high` | yes |
| C36 | `buck_scenarios.rs` `s3_no_current_limit` | yes |
| C37 | `buck_scenarios.rs` `s4_line_step_up_stays_within_165_mv` | yes |
| C38 | `buck_scenarios.rs` `s4_line_step_down_stays_within_165_mv` | yes |
| C39 | `buck_scenarios.rs` `s4_line_step_up_recovers_within_200_us` | yes - measured from the step's start, as the criterion says |
| C40 | `buck_scenarios.rs` `s4_line_step_down_recovers_within_200_us` | yes |
| C41 | `buck_scenarios.rs` `s4_no_fault` | yes |
| C42 | `buck_scenarios.rs` `s8_off_at_the_sample_after_en_falls` | yes - 0 to 2.02 µs, both ends |
| C43 | `buck_scenarios.rs` `s8_both_switches_stay_off` | yes - both commands, entry + one grid row to the end |
| C44 | `buck_scenarios.rs` `s8_pgood_low_from_off` | yes |
| C45 | `buck_scenarios.rs` `s8_vout_decays_with_the_load_time_constant` | yes - "decreases in every row" judged to `vntol` (1 µV), a documented solver resolution and not a widened bound; τ within ± 2 % of 1.452 ms |
| C46 | `buck_scenarios.rs` `load_line_passes_every_check` | yes |
| C47 | `buck_scenarios.rs` `faults_regulates_at_3_3_v_with_pgood_before_its_first_stimulus` | yes |
| C48 | `buck_scenarios.rs` `s5_every_period_is_cut_until_hiccup` | yes |
| C49 | `buck_scenarios.rs` `s5_inductor_current_stays_at_or_below_7_6_a` | yes - over the step to the entry |
| C50 | `buck_scenarios.rs` `s5_hiccup_with_over_current_26_5_to_30_5_us_after_the_step` | yes - window and the exact 16.5 µs ± 20 ns mechanism; fault code checked at the entry row |
| C51 | `buck_scenarios.rs` `s5_pgood_falls_within_8_us_of_the_deviation` | yes - the deviation is the supervisor's integer window |
| C52 | `buck_scenarios.rs` `s5_both_switches_stay_off_in_hiccup` | yes - from entry + one grid row (the 20 ns commit of §10.1) to the retry |
| C53 | `buck_scenarios.rs` `s5_retry_comes_2_ms_after_the_hiccup_with_the_overload_on` | yes - 2.000 ms within one grid row and before 3.32 ms |
| C54 | `buck_scenarios.rs` `s5_retry_without_the_overload_regulates_and_clears_the_fault` | yes |
| C55 | `buck_scenarios.rs` `s6_sink_limit_trips` | yes |
| C56 | `buck_scenarios.rs` `s6_inductor_current_is_held_at_minus_3_a` | yes - two-sided ± 0.1 A |
| C57 | `buck_scenarios.rs` `s6_over_voltage_within_two_samples_of_3_81_v` | yes - 0 to 4.52 µs, fault code checked at entry |
| C58 | `buck_scenarios.rs` `s6_reports_its_timing_and_rise_rate` | yes - report-only by D8; the assertion only rules out a degenerate value, as the criterion asks |
| C59 | `buck_scenarios.rs` `s6_both_switches_stay_off_after_over_voltage` | yes - to the last row |
| C60 | `buck_scenarios.rs` `faults_passes_every_check` | yes |
| C61 | `buck_scenarios.rs` `brownout_regulates_at_3_3_v_with_pgood_before_its_first_stimulus` | yes |
| C62 | `buck_scenarios.rs` `s7_off_at_the_sample_after_vin_falls_through_8_v` | yes - "not before" held by the lower bound 0 |
| C63 | `buck_scenarios.rs` `s7_both_switches_off_while_off` | yes |
| C64 | `buck_scenarios.rs` `s7_pgood_low_while_off` | yes |
| C65 | `buck_scenarios.rs` `s7_fault_unchanged_while_off` | yes |
| C66 | `buck_scenarios.rs` `s7_soft_start_at_the_sample_after_vin_rises_through_8_5_v` | yes - `Off` held over the gap, then 0 to 2.02 µs |
| C67 | `buck_scenarios.rs` `brownout_passes_every_check` | yes |
| C68 | `buck_scenarios.rs` `s9_dc_error_within_section_4_3_at_every_corner` | yes - §4.3 totals unchanged; an unsettled corner is named first |
| C69 | `buck_scenarios.rs` `s9_ripple_at_most_10_mv_at_every_corner` | yes - 10 mV unchanged; corner 9 measures 8.64 mV (see F1) |
| C70 | `buck_scenarios.rs` `s9_no_idle_tone_above_1_code_at_every_corner` | yes - an unknown code panics rather than passes |
| C71 | `buck_scenarios.rs` `s9_every_corner_is_settled` | yes - 6.25 mV unchanged |
| C72 | `buck_scenarios.rs` `steady_state_passes_every_check` | yes |
| C73 | `buck_supervisor.rs` `each_hiccup_lasts_1000_samples` | yes - counted against SPEC's 1000, not against the reference model that shared the off-by-one |
| C74 | `piperine-cli/tests/docs.rs` `the_buck_spec_holds_the_bench_bounds` | yes for AC 2 to AC 5 as written; it does not hold the new §10.1 S9 row, whose numbers are wrong (F1) |
| C75 | `piperine-cli/tests/docs.rs` `the_buck_readme_points_at_the_scenarios` | yes |
| C76 | `piperine-cli/tests/docs.rs` `gaps_narrows_what_buck_scenarios_ran` | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 - SPEC §10.1 "Wait before an S9 window" names the wrong worst case: "at most 6.2 codes (12 V → 9 V at 3.3 V)" and "3.2 τ = 350 µs" | §3.1/§5.2: the 9 → 12 V ramp at 5.0 V, 5 A moves the duty word 5.2 · 3200 · (1/9 − 1/12) = 462 LSBs, a lag of 462 / (100 · 0.5) = 9.2 codes; 12 → 16 V at 5.0 V is 6.9 codes. τ · ln(9.2/0.25) = 3.6 τ ≈ 375 to 400 µs. The commit title says "3.2 tau", the test doc says "about 4". MSL corner 9 still drifts 2.78 mV at 450 µs, against the derivation's < 1.6 mV | rewrite the row with the 9.2-code worst case (corner 8) and 3.6 τ ≈ 400 µs, keeping 450 µs as the wait; state that corner 9's measured residue (2.78 mV drift, 8.64 mV ripple) exceeds the linear prediction. Optionally hold "450 µs" in `docs.rs` AC 4. No bound moves | |
| F2 - spec.md is stale after the S9 change | Goals "16.6 ms … longest run (about 4.7 ms)"; the Scenario budget **Target** row "16.59 ms, 17 to 24 min wall" (the five runs now sum to 20.0 ms and the longest is 7.46 ms, ≈ 26 min); D5 "S9 line steps take 10 µs" (they are 200 µs ramps, SPEC §10.1); D14 "the longest run (4.7 ms) needs about 5.3 M"; Success Criteria "≈ 4.7 ms"; the S9 assumption row's rationale still cites "S3/S4 recovery ≤ 200 µs" | update those lines to the 7.46 ms `steady_state` run (≈ 8.5 M steps, still under 15 M) and SPEC §10.1's wait | |
| F3 - a `Ramp` whose span is under half a step schedules no set, so the param never reaches `to` (BS-02 AC 8: "the last at the end of the duration and equal to `b`") | throwaway test: `Ramp { from: 12.0, to: 12.04, step: 0.1, .. }.points()` returns `[]`. `from == to` also returns `[]`. No buck scenario hits it today | make `count` at least 1 (`max(1)`), so the last point is always `(start + duration, to)`, and add that case to `a_ramp_sets_once_per_equal_step_and_ends_on_its_target` | |
| F4 - dead and misnamed code in the harness (cleanliness: one purpose per type, OO names, no dead code) | `Trace::first_rise_in_any_row` (`trace.rs:122`) is used only by its own unit test, superseded by `largest_rise`. `Departure` (`discrete.rs:9`, "a row … that breaks what was asked of it") is also the type of every row of `DiscreteTrace` (`rows: Vec<Departure>`). `Ran` is a verb, not a noun | delete `first_rise_in_any_row` and its assertions; split `Departure` into a row type (e.g. `DiscreteSample`) and keep `Departure` for `holds`'s error, or rename to one name that fits both; consider `RunOutcome`/`CompletedRun` for `Ran` | |
| F5 - undocumented fields and a free function (cleanliness: every struct and method doc-commented, no free functions) | undocumented fields: `NamedColumn::{index,label}`, `Trace`/`DiscreteTrace::{label,window,rows}`, `ScenarioRun::{package,run,stimulus}`, `ScenarioBatch::runs`, `Stimulus::actions`, `PlantedPackage::dir`, and in `tests/buck_scenarios.rs` `BuckHost::stimulus`, `Waveforms::ran`, `Suite::results`, `SteadyStatePlan::{host,corners,supply,setpoint_code}` (its `cursor` and `Corner`'s fields are documented). Free fn `open` in `stimulus.rs`'s test module, where the sibling modules use helper types (`Tables`, `Records`) | add a `///` line to each field; move `open` onto a helper type in the test module | |

## Needs the user's judgment (not findings: the decisions were taken under the AFK rule)

1. **Approved criteria were edited in place.** BS-03 AC 7 (1.29 → 1.38 ms), BS-05 AC 3 (16 to 20 µs → 26.5 to 30.5 µs plus the 16.5 µs mechanism) and BS-07's preamble (200 → 450 µs) were rewritten after the AFK approval. The skill says a change appends a new criterion and never edits an approved one. Each change carries its derivation and none loosens a tolerance, but the user should confirm each one, as red-scenarios.md asks.
2. **S2b**: accept the 1.38 ms nominal (dead time made explicit in SPEC) or add dead-time compensation to the DPWM (red-scenarios.md advises against it because of the risk to S3/S4).
3. **S9 margin**: corner 9 passes at 8.64 mV against 10 mV with 2.78 mV of residue at 450 µs. A 500 µs wait would add ≈ 0.45 ms to the longest run.
4. **SPEC model claims the MSL run does not meet**: §4.3 and the §10 S9 model column say "steady-state error within ± 2.4 mV, constant code", but the run measures 3.51 mV (corner 5) and 2.94 mV (corner 4), with a 1-code swing at corners 5 and 9. These are inside every pass criterion (§4.3 totals, 1 code). Under "code follows spec", the model column needs updating or the gap needs explaining.
