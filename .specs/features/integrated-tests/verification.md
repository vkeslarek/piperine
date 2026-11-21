# Integrated Tests verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 3b8e4b0f (HEAD 5b28f348 adds only `proofs.md`)
**Verifier**: independent sub-agent (author != verifier), post-merge on `main` 5b28f348

Runs by the Verifier at 5b28f348 (`CARGO_TARGET_DIR=/var/tmp/piperine-target-b`, `CARGO_PROFILE_DEV_DEBUG=0`,
debug build, warm target directory): `cargo test -p piperine --test integrated` 34 passed, 40 s and 44 s
wall (two runs; the test binary reports 39.6 s / 44.2 s); `cargo test -p piperine-cli --test integrated`
3 passed, 44 s wall; full root `env -u PIPERINE_NGSPICE cargo test --no-fail-fast`: exit 0, 295 test targets `ok`, 0 failed, 1 293 s (21 min 33 s) wall; the only buck-named targets it ran are the static `buck_example`, `elab_buck`, `types_buck` (D3 Out of Scope), and its `Running tests/<file>.rs` lines confirm the package-relative path form the C2 grep matches.
The on-demand buck run was not repeated (the author's B3: 61/61 `buck_scenarios`, all four lines green).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `suite_hygiene` `the_on_demand_targets_are_the_buck_set` | yes - the twelve of D3, sorted, exact |
| C2 | shell proof over `cargo test --no-run` | yes - lists `tests/integrated/main.rs`; the negated grep matches the `Executable tests/<file>` form cargo prints for every package |
| C3 | `buck_whole` `the_buck_compiles_with_no_warning_and_valid_shapes` | yes |
| C4 | `git diff --quiet 856d2668 HEAD -- <ten buck files>` | yes - re-run, clean |
| C5 | `suite_hygiene` `on_demand_targets_are_registered` | no - the rule holds only for the exact line `test = false`: a flag with a trailing comment (`test = false # slow`) or a `[[test]] # x` header is legal TOML, cargo skips the target, and `OnDemandManifest::targets` returns nothing (Verifier probe), so the target is switched off with no registry entry (finding 1) |
| C6 | `suite_hygiene` `the_on_demand_registry_names_only_on_demand_targets` | yes |
| C7 | `suite_hygiene` `the_buck_readme_names_every_on_demand_target` | no - `undocumented` is a substring test without the crate: `--test buck` is found inside `--test buck_whole`, and either crate's `book_buck` inside the other's line; a README whose block drops the whole `piperine-cli` line passes (Verifier probe) (finding 2) |
| C8 | `suite_hygiene` `the_on_demand_reader_finds_test_false` | yes for the planted shapes it names; see C5 for the shapes it misses |
| C9 | `integrated` `package::the_package_declares_the_five_runs_and_checks_clean` | yes |
| C10 | `integrated` `package::every_run_compiles_with_no_warning_and_valid_shapes` | yes - `Built::new` asserts empty `rustc` warnings; `validate` on every running body |
| C11 | `integrated` `package::opening_every_run_leaves_only_the_package_sources` | yes |
| C12 | `integrated` `package::the_runs_execute_at_once_in_their_own_build_directories` | yes - distinct build dirs and pairwise overlapping intervals |
| C13 | `piperine-parity` lib `scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run` | yes at the harness layer (pre-existing test); `Suite::ran`'s panic path of the root target has no test of its own |
| C14 | `integrated` `package::every_run_reports_its_wall_time_and_rows` | yes |
| C15 | `piperine-parity` lib `scenario::run::tests::a_completed_run_keeps_every_record_in_delivery_order` | yes - four records, fields, instants and order |
| C16 | `integrated` `threshold::the_counters_read_four_rises_and_four_falls` | yes |
| C17 | `integrated` `threshold::the_edges_stream_holds_eight_alternating_records` | yes |
| C18 | `integrated` `threshold::each_edge_record_is_where_the_capacitor_crosses_its_threshold` | yes - closed form at the record's instant vs 0.55/0.45 V; independently re-derived first crossings 5.7985 us and 10.7918 us |
| C19 | `integrated` `threshold::each_edge_falls_in_its_half_period` | yes |
| C20 | `integrated` `threshold::the_capacitor_follows_its_closed_form` | yes - 421 rows |
| C21 | `integrated` `sample_hold::each_sample_commits_the_code_of_its_held_voltage` | yes - expected codes written as literals, schedule cross-checked |
| C22 | `integrated` `sample_hold::the_code_appears_after_tconv_with_a_50_ns_done_strobe` | yes on the 10 ns grid - the five high rows bound the strobe to (40, 50] ns, not exactly 50 ns; the `tconv` edge is observable only on the three samples whose code changes (11, 21, 31) |
| C23 | `integrated` `sample_hold::the_hold_node_stays_put_while_holding` | yes - the host steps fall inside holds, so a switch that failed to open moves the node by a code step |
| C24 | `integrated` `sample_hold::the_hold_node_tracks_the_source` | yes |
| C25 | `integrated` `pwm_filter::each_period_holds_its_duty_on_the_high_and_low_sides` | yes - every row bit for bit against `PwmReference`, plus the counts |
| C26 | `integrated` `pwm_filter::the_dead_time_is_two_clock_periods_on_both_edges` | yes |
| C27 | `integrated` `pwm_filter::the_sides_never_overlap_and_every_never_check_passes` | yes - 6 000 exact |
| C28 | `integrated` `pwm_filter::the_driven_duty_is_latched_at_the_next_wrap` | yes - 25 and 60 written as literals |
| C29 | `integrated` `pwm_filter::the_filter_follows_its_closed_form` | yes - breakpoints from the reference, not the table |
| C30 | `integrated` `state_machine::the_phase_follows_the_reference_at_every_edge` | yes - 220 edges; the thirteen changes re-derived by the Verifier from the rules (edges 11, 31, 41, 61, 81, 91, 111, 118, 148, 168, 178, 198, 218): the correction from twelve is right and independent of the run |
| C31 | `integrated` `state_machine::the_enums_print_as_their_variant_indices` | yes |
| C32 | `integrated` `state_machine::the_capacitor_follows_its_closed_form_through_each_phase` | yes |
| C33 | `integrated` `state_machine::the_trip_record_is_where_the_capacitor_crosses_0_91_v` | yes |
| C34 | `integrated` `state_machine::two_edges_with_ov_enter_fault_with_the_dump_switch_on` | yes |
| C35 | `integrated` `state_machine::the_hiccup_lasts_30_clock_periods_then_retries` | yes |
| C36 | `integrated` `state_machine::a_clean_retry_clears_the_fault_and_resumes_the_cycle` | yes - retry peak re-derived at about 0.871 V, 39 mV under the 0.91 V trip |
| C37 | `integrated` `state_machine::one_phase_record_per_change_at_its_edge` | yes |
| C38 | `integrated` `pi_loop::every_duty_follows_the_pi_reference` | yes - 100 duties in table and record, bit for bit |
| C39 | `integrated` `pi_loop::the_code_stays_within_three_of_128_after_50_us` | yes - bound from 2.56 codes per duty count, not from the measured 127 to 128 |
| C40 | `integrated` `pi_loop::every_regulated_check_passes_after_50_us` | yes - 50 mV from 25 + 20 + 3.9 mV, derived |
| C41 | `integrated` `pi_loop::one_loop_record_per_sample` | yes |
| C42 | `integrated` `pi_loop::the_table_has_10001_rows_on_the_10_ns_grid` | yes |
| C43 | `integrated` `pi_loop::the_duty_saturates_and_the_integrator_holds` | yes, indirectly - the integral-hold assertion compares the reference with itself; the run's integrator is held only through C38's bit-for-bit duties (the run does not print `integ`). First saturated sample = 1 re-derived (u = 96, then 106) |
| C44 | `source_host` `a_host_square_into_the_rc_matches_tran_square_rc` | yes - same corners, 10 ns cap both sides, `TRAN_VOLTAGE`; with no ngspice it degrades to `Reference::Skipped` as `tran_pwl_rc` does |
| C45 | `grep "tran 0.1u 42u 0 10n uic"` | yes |
| C46 | `git diff --diff-filter=MD 856d2668 HEAD -- contract.rs, golden, twins` | yes - and `git diff 856d2668 -- crates/piperine-parity/src tests/support crates/piperine-transpile/tests/golden` touches only `scenario/run.rs` and `scenario/planted.rs` (harness, no tolerance) |
| C47 | `piperine-cli` `integrated` `pi_loop_prints_its_run_line_and_10001_rows_ending_near_128` | yes |
| C48 | `piperine-cli` `integrated` `pi_loop_prints_no_failed_check_and_records_every_passing_one` | yes |
| C49 | `piperine-cli` `integrated` `piperine_test_passes_pi_loop_and_threshold` | yes |
| C50 | `piperine-cli` `book` `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes, loosely - `>= 28` is a floor (as `>= 29` was), not "lowered by exactly one"; the buck block's `data-cli-lines="1-8"` selection is no longer validated by any target (finding 4) |
| C51 | `piperine-cli` `book_buck` `the_buck_block_is_what_the_binary_prints` | yes (on demand; the author's run) |
| C52 | `piperine-cli` `book` `the_buck_block_shows_the_example_files` | yes |
| C53 | `piperine` `book_buck` `the_buck_bench_starts_with_the_supervisor_off` | yes - assertions moved verbatim |
| C54 | `piperine` `book_cli` `the_buck_bench_of_the_chapter_is_the_example` | yes - text equality stays by default; fingerprints of `cli#44`/`cli#47` still guarded by `book_inventory` |
| C55 | `book_inventory` `every_block_of_the_book_has_one_row` | yes |
| C56 | `book_claims` `binary_claims_name_command_line_tests` | yes |
| C57 | `source_commits` `two_commits_exchange_two_carriers` | yes - assertions unchanged (diff read) |
| C58 | `source_records` `never_reports_each_rising_edge` | yes - assertions unchanged; local `Osc` has the buck's 200 MHz default |
| C59 | `source_time` `every_from_the_stdlib_fires_once_per_period` | yes |
| C60 | `! test -e` the three old files | yes |
| C61 | `test -f digital/time.msl && ! grep buck::` | yes (the targets still `#[path]`-include `tests/buck/support.rs`, documented in Settled B1) |
| C62 | `docs` `the_buck_readme_says_when_to_run_the_buck_on_demand` | yes |
| C63 | `docs` `the_buck_readme_points_at_the_scenarios` | yes |
| C64 | `docs` `the_instructions_name_the_integrated_gate_and_the_buck_on_demand` | yes |
| C65 | `grep "^### MD-77"` in `.specs/STATE.md` | yes |
| C66 | `workspace_shape` `root_tests_hold_no_end_to_end_target` | yes for its criterion; but any `.rs` under `tests/integrated/` is now accepted whether or not `main.rs` declares it as a `mod`, so an undeclared scenario file is never compiled and its tests never run (finding 3) |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 1. A `test = false` target escapes the registry when the line carries a TOML comment (or the `[[test]]` header does): cargo skips it, `suite_hygiene` stays green - the exact silent switch-off IT-02 AC 1 forbids | `suite_hygiene.rs` `OnDemandManifest::targets`: `compact == "test=false"` and `line == "[[test]]"`; Verifier probe: `targets("[[test]]\nname = \"slow\"\ntest = false # on demand\n")` returns `[]` | strip a `#` comment before matching (header and key lines), or parse with the `toml` crate; add the commented shapes to `the_on_demand_reader_finds_test_false` | |
| 2. The README guard matches `--test <name>` as a substring without the crate: `--test buck` is satisfied by `--test buck_whole`, and the two `book_buck` entries satisfy each other; the whole `piperine-cli` line can vanish with C7 green | `OnDemandRegistry::undocumented`: `block.contains(&format!("--test {target}"))`; Verifier probe with a block lacking the CLI line returns no offence | match `--test <name>` as a whole word on a line that also holds `-p <crate package>` (the root `.` as `-p piperine`); plant the prefix case and the two-crate case in the test | |
| 3. `workspace_shape` now accepts any `.rs` under a declared `main.rs`'s directory without checking it is a `mod` of that target: a new `tests/integrated/<scenario>.rs` that `main.rs` forgets is never compiled, its tests never run, and no guard fails | `workspace_shape.rs` `RootTestFiles::is_module_of_declared_main` (`file.starts_with(dir)` only) | require each such file to be named by a `mod <stem>;` line of the declared `main.rs` (or `#[path]`), and plant an orphan in `a_declared_main_target_accounts_for_its_own_modules_only` | |
| 4. The buck listing's `data-cli-lines="1-8"` is no longer validated: `book`'s `checked()` drops project `buck` before `selection()`, and `BookListing` ignores `data-cli-*` other than output/project | `crates/piperine-cli/tests/book.rs` `checked()`; `crates/piperine-cli/tests/support/book_listing.rs` | apply the same range check in `book_buck` (or keep the selection check in `book` for every block, the buck's included, while skipping only its run) | |
| 5. The buck README says six `buck_scenarios` tests are red today, while the author's on-demand run at this tree passed 61/61 | `examples/buck/README.md` "On demand" last paragraph; `tasks.md` Settled B3 | drop or correct the sentence (and the spec's Out-of-Scope note is historical) | |

**Capabilities the default run no longer proves at whole-design level** (moved on demand by D3, not re-proven
by the suite; listed for the record, not as findings unless the user wants them back):
inductor / RL response and current sense (`buck_power`); gate-driver delays and interlock (`buck_power`);
DPWM dither and release count, and run-to-run determinism ("two runs equal", `buck_dpwm`); `RefRamp`
soft-start slew and `SetpointReg` (known data taken, unknown ignored, saturation) (`buck_control`); deep
hierarchy paths (`Startup.buck.ctl.reference`-style nested instance printing; the suite's tops are one level
deep); `Stimulus::ramp` and `Trace` measurements (settling, overshoot, decay constant) on a real run (now unit
tests of the harness only; the suite judges closed forms instead); the CLI's `idle` run line budget (at most
16 lines); the Book claims `cli~3d1453da`, `cli~6153ef01`, `cli~5010f432` (held by `piperine-cli/buck`, now
on demand); `--all-targets` no longer compiles the on-demand targets (the `**Build**` line and MD-77 cover it
at feature close only).
