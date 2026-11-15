# Solver Restructure verification

**Verdict**: PASS - F1 to F6 verified closed in round 2; N1, N2 (Low, round 2) closed by 394e6485 without a third round
**Proofs**: `proofs.md` at f9e81d02 (round 1 over ec35d7c5; round 2 over faf5d200, fix range a9ff229d..faf5d200)
**Verifier**: independent sub-agent (author != verifier)
**Round**: 2

## Round 2

The fix commits d230d0df..841cf1c0 were re-judged at HEAD faf5d200:

- **F1**: `words_in` matches whole identifiers and the scan now covers `Channel`, `FileId`, `FileError` and `Region`. Re-running probe 2 (the planted `pub use removed::{Region, Channel, FileId, FileError};` in `abi.rs`) makes `text_io_is_gone` and `regions_are_gone` FAIL. The self-test also holds the negative case (`RegionQueueless`, `channels`).
- **F2**: `StepLimits` is exported from `prelude` beside `StepperStrategy`, and `stepper_fold.rs` imports it from the facade only.
- **F3**: the role text and the paths are correct, the plugin and codegen references are gone, and the root shell is listed. A planted "They meet through events only." in `AGENTS.md` makes `instructions_list_the_kept_crates` FAIL. Two residuals remain: N1 and N2.
- **F4**: disclosed in `design.md`.
- **F5**: `Net::analog_nets` (associated fn) and `CircuitInstance::digital_failure` (private associated fn) have the same bodies as before. The only call sites changed are `nets()` and the one scheduler `map_err`. No logic change.
- **F6**: the scan reads table-form headers, and the new self-test `the_manifest_scan_reads_table_form_dependencies` proves it.
- **C38**: the proof now compares against the 353 test fns at e129b869, and it passes when run here.
- **Regression run at HEAD**: `cargo test` on the three solver crates is green, `suite_hygiene` is green (12 tests), and `cargo build --all-targets` on the four crates prints 0 warnings.

No hidden logic change was found in the moved code. The findings below are about guards that are looser than their criteria, the public facade, and the instruction files.

What was diffed, file by file, against e129b869 (comment and `use`-path lines set aside):

- `digital/scheduler.rs` -> `piperine-digital-solver/src/scheduler.rs`. The only changes are the generic `D: DigitalParticipant + ?Sized`, `DigitalLimits` in place of `PlanLimits` (same field values), and `DigitalError` in place of `Error::simple`. `is_digital()` replaces `capabilities().contains(DIGITAL)` at the one site. The removed `record_change` and `with_delay_table` belonged to the removed timing surface.
- `DigitalError`'s `Display` gives the same message text as the two old `format!`s. The owner wraps it with `digital_failure`, which builds `Error::simple(SolverDomain::Digital, ...)`. That call sits at the only caller (`core/circuit.rs`), which still uses `DigitalLimits::default()` with 1000 and 1e-12, as `PlanLimits::default()` did. `PlanLimits`' digital fields were never configurable on that path.
- `topology.rs`, `drivers.rs` and `DigitalDevice`: identical apart from the signature and one doc line.
- `predict_crossing`, `CrossDir` and `Prediction` match `piperine-ir/src/events.rs` byte for byte, and so do their two tests. `Prediction::next_step_bound` was dropped; nothing in the solver used it.
- `convergence.rs`: `StepLimits` replaces `&TransientAnalysisOptions`, and `clamp`/`max` read the same two fields.
- `Tolerances` and `Policy` are identical apart from paths. `newton_raphson`, `faer`, `linear` and `error` change paths only. `num`, `iv`, `circular_array`, `config` and `stamp_validation` are unchanged.
- `analog_nets` is `all_references().into_iter().map(Into::into)` over the same `left_values()` iterator: equivalent order and content.
- The driver edits in `dc`, `transient` and `ac` delete only the validation, margin and timing calls. Each deleted call was a no-op without an `EMITS_VALIDATION` element or a filled delay or timing table, and no kept producer sets either.
- No non-test source at e129b869 scheduled a `Region` other than `Active`, so `(time, seq)` ordering is the old order for every kept producer.

Coverage of the 76 removed tests:

- Every `#[test]` fn under `crates/piperine-solver` at e129b869 (353) is either present in the three crates at HEAD or named in `removed.txt`. No test was dropped silently.
- Each removed name sits in a deleted file or target, or is wholly about a removed item: `jacobian_capability_bits_compose_correctly` asserts only `HAS_DISTO2`, `HAS_DISTO3` and `NUMERIC_JACOBIAN`. The three `events.rs` region tests asserted the `Region` priority.
- The kept same-source and two-driver behaviours are still covered by `two_distinct_drivers_contend_per_table_7_3`, which is kept, and by probe 1 below.

The five ported targets:

- Every line containing `assert`, `tol` or a `1e-` literal is textually identical to e129b869. The test bodies are unchanged and only the circuit construction differs.
- The elements follow the Verilog-A they replace constant for constant: `0.02585`, `3.14159265358979`, the clock window from `0.25` to `0.75`, the comparator threshold `0.5`, and the `$param_given` rebuild class.

Own examples, run at HEAD as throwaway tests (deleted, tree clean):

1. Digital crate: same-source writes at one time, seq (0,1) and then (1,0). The higher seq wins both ways. With `max_delta_cycles = 1` the run errors exactly at the cap with the old text. An event at `t + 2e-12` stays outside the epsilon and is not applied. `predict_crossing` gives `None` on `0 -> 0` for `Either` and `Falling`, and returns `t0` exactly when `v0 = 0` and the crossing is `Rising`. All of these held.
2. Planted `pub use removed::{Region, Channel, FileId, FileError};` in `abi.rs`: all 7 `removed_surface` tests stayed green. This is F1.
3. Planted `#[ignore]` in `crates/piperine-digital-solver/src/limits.rs`: `no_ignored_tests` failed and named `limits.rs:27`. The moved guard reaches the sub-solvers.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `workspace_shape.rs` `solver_manifest_has_no_in_tree_dev_dependency` | yes |
| C2 | `cargo test -p piperine-solver` | yes |
| C3 | the 15 named fns in `sens`, `adjoint_dc`, `adjoint_vs_sens`, `transient_reentry`, `pss` | yes - assertion and tolerance lines identical to e129b869 |
| C4 | `helper_devices.rs` `divider_reads_the_analytic_ratio` | yes - `< 1.0e-9` against `vin*r2/(r1+r2)` |
| C5 | `helper_devices.rs` `rc_step_reads_one_minus_exp` | yes - stops at `tau`, `< 1.0e-3` against `1 - e^-1` |
| C6 | `helper_devices.rs` `diode_satisfies_its_own_equation` | yes |
| C7 | `workspace_shape.rs` `retired_crates_are_gone` | yes |
| C8 | `workspace_shape.rs` `no_manifest_names_a_retired_crate` | yes |
| C9 | `cargo metadata --no-deps` | yes |
| C10 | `workspace_shape.rs` `root_tests_hold_no_end_to_end_target` | yes |
| C11 | `workspace_shape.rs` `examples_hold_no_vams_or_python` | yes |
| C12 | `workspace_shape.rs` `default_members_are_the_green_crates` | yes |
| C13 | `workspace_shape.rs` `workflows_name_no_retired_crate` | yes |
| C14 | `removed_surface.rs` `disto_is_gone` | yes |
| C15 | `removed_surface.rs` `text_io_is_gone` | yes - closed (F1 d230d0df) |
| C16 | `removed_surface.rs` `verification_channels_are_gone` | yes |
| C17 | `removed_surface.rs` `specify_timing_is_gone` | yes |
| C18 | `removed_surface.rs` `regions_are_gone` | yes - closed (F1 d230d0df) |
| C19 | `removed_surface.rs` `simprobe_is_gone` | yes |
| C20 | `removed_surface.rs` `the_scan_catches_a_planted_name` | yes - covers the whole-identifier matcher, positive and negative (F1 d230d0df) |
| C21 | `crate_boundaries.rs` `the_three_solver_crates_exist` | yes |
| C22 | `crate_boundaries.rs` `sub_solvers_depend_on_no_workspace_crate` | yes - `path =` also catches table-form entries |
| C23 | `crate_boundaries.rs` `owner_depends_on_both_sub_solvers_only` | yes - closed (F6 0c98cf15) |
| C24 | `crate_boundaries.rs` `analog_sources_name_no_digital_type` | yes |
| C25 | `crate_boundaries.rs` `digital_sources_name_no_analog_type` | yes |
| C26 | `crate_boundaries.rs` `each_sub_solver_holds_its_modules` | yes |
| C27 | `faer.rs` `singular_system_fails_loud_through_the_existing_error_family` | yes |
| C28 | `interface.rs` `external_model_emits_through_the_sink` | yes |
| C29 | `event_order.rs` `same_time_events_order_by_seq` | yes - the scheduler drains the same `Ord`; probe 1 confirms it through `evaluate_until_stable` |
| C30 | `abi_surface`, `prelude_surface` byte-identical and green | yes |
| C31 | `parity_baseline` byte-identical and green | yes |
| C32 | baseline minus `removed.txt` all `... ok` | yes - independently confirmed against the e129b869 test-fn set |
| C33 | `cargo build --all-targets` on the four crates, no warning | yes |
| C34 | `workspace_shape.rs` `instructions_list_the_kept_crates` | yes for placement, deleted-crate references and the root shell (F3 480bb9d6); weak on the boundary wording - `text.contains("target")` is vacuous (N1) |
| C35 | `workspace_shape.rs` `instructions_have_no_entry_for_a_retired_crate` | yes - the body references to `piperine-plugin` are gone and are now guarded by C34's text check (F3 480bb9d6) |
| C36 | `workspace_shape.rs` `state_records_the_solver_structure_decision` | yes - quotes checked against `context.md` |
| C37 | `git diff` with no added `assert` line outside the ported targets | yes - every modified kept test file was read by hand |
| C38 | `baseline.txt` holds every e129b869 solver test fn | yes - tightened in 841cf1c0; 353 names, run here |
| C39 | `cargo test -p piperine-lang --test suite_hygiene` | yes |
| C40 | 11 e129b869 fn names present | yes - 12 `#[test]` fns run: the 11 originals plus the walk test |
| C41 | no removed-crate path in the guard | yes |
| C42 | `suite_hygiene.rs` `the_walk_covers_the_three_solver_crates` | yes - probe 3 |
| C43 | `workspace_shape.rs` `hygiene_guard_lives_in_piperine_lang` | yes |
| C44 | `suite_hygiene.rs` `the_detectors_recognise_what_they_forbid` | yes - no detector function changed in the diff |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (Medium) `removed_surface` does not guard every name in SR-03 AC 2 and AC 5, so AC 7 ("fail if `prelude.rs` or `abi.rs` names any item") is not held | `removed_surface.rs` lists leave out `Channel`, `FileId`, `FileError` and bare `Region`. Probe 2 planted `pub use removed::{Region, Channel, FileId, FileError};` in `abi.rs` and all 7 tests passed | add the four names. For the short ones, scan with an identifier-boundary match (`\bRegion\b`, `\bChannel\b`), or scan only `prelude.rs`/`abi.rs` for them, so that unrelated words do not trip it |closed d230d0df |
| F2 (Low) `StepLimits` cannot be reached through the facade, while `StepperStrategy`, whose methods take it, is exported by `prelude` (MD-17) | `prelude.rs:45` exports `StepperStrategy`, and neither `prelude` nor `abi` exports `StepLimits`. `tests/stepper_fold.rs` had to add `use piperine_analog_solver::convergence::StepLimits` | re-export `StepLimits` beside `StepperStrategy` in `prelude` (and in `abi`), and switch `stepper_fold.rs` to the facade path |closed 09f4fc62 |
| F3 (Low) `CLAUDE.md` and `AGENTS.md` describe the moved code at its old place and state the target boundary as if it were in place now (SR-06 AC 1) | The `CLAUDE.md` `piperine-solver` row lists `math/`, `ConvergencePlan` and `TrBdf2`. Its "Solver architecture" section cites `analyses/convergence.rs` and `math/integration.rs` and says "Scheduler: Returns `Result<(), Error>`". The `AGENTS.md` map gives `piperine-solver/ ... math/`. Both say the sub-solvers "meet through events only", but the spec's Out of Scope keeps the `digital: &[LogicValue]` snapshot until feature (B). `CLAUDE.md` still names `crates/piperine-plugin/tests/extern_stub.rs`, `PluginHost::seed_schemas` and "codegen's own operator registry". The root `piperine` package is not listed as non-building | move `math/`, the convergence plan, `TrBdf2`/LTE and `Tolerances` to the analog row and update the paths. Phrase the event-only boundary as the target (MD-60 item 1) with today's state (a crate boundary, with the shared digital snapshot). Drop the deleted-crate references and add the root shell to the non-building list |closed 480bb9d6 |
| F4 (Low) The generic scheduler signature breaks inference at call sites that pass `&mut []`, which the design said would "compile unchanged" | `tests/digital_driver_segregation.rs` (3 sites) and the `events.rs` unit test needed `&mut [] as &mut [Box<...>]` | disclose this in `design.md` under the digital crate's Interfaces (no behaviour change). No code change needed |closed e153e614 |
| F5 (Low) Two new module-level free functions break MD-13 rule 2 ("No loose functions", `AGENTS.md`) | `pub(crate) fn digital_failure` in `core/result.rs:428`, and `pub fn analog_nets` in `core/net.rs:157` | make them owned, for example `Net::analog(&Netlist)` as an associated fn and a method on `CircuitInstance` (or a local extension trait) for the digital error |closed 7ea89680 |
| F6 (Low) The owner's manifest scan misses table-form dependencies | `workspace_dependencies` keeps only lines that start with `piperine-`; a `[dependencies.piperine-lang]` header with `path = ...` would pass `owner_depends_on_both_sub_solvers_only` | also read `[dependencies.piperine-*]`, `[dev-dependencies.piperine-*]` and `[build-dependencies.piperine-*]` headers as dependency keys |closed 0c98cf15 |
| N1 (Low, round 2) The C34 boundary-wording assertion is vacuous, and the CLAUDE.md summary sentence is garbled | `workspace_shape.rs` `assert!(text.contains("target"))` is satisfied by "binary target" and "test target". With `CLAUDE.md` rewritten to call events "the only boundary" and "current", `workspace_shape` stayed 11/11 green. `CLAUDE.md:10-11` reads "Events are the only boundary between the analog and the digital side is the target boundary (MD-60)" | reword to "The events-only boundary between the analog and the digital side is the target (MD-60)", and assert a specific phrase (for example `contains("target boundary")` plus `!contains("Events are the only boundary")`) | closed 394e6485 (orchestrator; same decision as msl-packages N1-N3: fix + merge without a third round) |
| N2 (Low, round 2) `design.md` still describes the replaced error conversion | the digital crate's "Errors" bullet says "The owner has `impl From<DigitalError> for Error`". The orphan rule forbids that impl; the code uses the private `CircuitInstance::digital_failure` | reword the bullet to name `CircuitInstance::digital_failure` | closed 394e6485 (orchestrator; same decision as msl-packages N1-N3: fix + merge without a third round) |

## Notes (process, no fix owed)

- The B2 guards (T8-T12) were written after the change they guard, and T26 and T27 had no red run. Probes 2 and 3 show that the hygiene guard and the planted-name scan do fail when they should. The red that was never shown is the F1 gap.
- The T21 commit subject starts with an uppercase word (`Newton, ...`), which `check_commit.py` flags.
- `root_targets_match_the_naming_rule` is now vacuous because the root `tests/` holds no `.rs`. `workspace_shape::root_tests_hold_no_end_to_end_target` enforces the stronger rule. The user approved removing the root floor.
- Cosmetic: the `crates/piperine-solver/src/analyses/ac.rs` arm of the ignored-doc registry is mis-indented, the `repo_root()` doc in `support/sources.rs` still says the consumers live in the root `tests/`, and `CLAUDE.md` lacks a blank line before `## Documentation`.
