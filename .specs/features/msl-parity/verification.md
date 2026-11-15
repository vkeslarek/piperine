# MSL Parity verification

**Verdict**: FAIL - the M1 parity claim holds (every fixture goes red under a real disagreement); four Minor findings on tests looser than their criterion
**Proofs**: `proofs.md` at bd4c1ed8 (HEAD f890b3bb adds only proofs.md)
**Verifier**: independent sub-agent (author != verifier), Opus, round 1, ngspice-47 at /usr/bin/ngspice

## Checks

| Check | Test | Holds | Note |
| --- | --- | --- | --- |
| C1 | `twins` `the_corpus_holds_the_library_and_the_twins` | yes | checks the five `module` names and every twin/.cir pair on disk |
| C2 | `twins` `every_twin_builds_without_diagnostics` | yes | asserts an empty diagnostic list per twin |
| C3 | `twins` `top_nodes_carry_the_cir_names` | yes | exact sorted set of top signals equals the .cir node set |
| C4 | `twins` `the_cir_fixtures_are_byte_identical` | yes | length plus FNV-1a hash of all six .cir |
| C5 | `bench` `a_twin_loads_into_a_bound_circuit` | yes | solves the loaded divider and reads 7.5 V |
| C6 | `bench` `a_failing_stage_is_named_with_its_diagnostics` | partly | stage and fixture asserted; the span is never asserted and the code check accepts any `E0` substring (F3) |
| C7 | `bench` `nodes_and_sources_are_reached_by_cir_names` | yes | v(vin) and the source flow by .cir names; sign of the flow is held by C25 instead |
| C8 | `harness` `ngspice_runs_in_batch_in_an_empty_scratch_directory` | yes | real ngspice-47 run; empty cwd asserted; the absolute path comes from `canonicalize` in `Ngspice::fixture_path` |
| C9 | `harness` `each_run_has_its_own_scratch_removed_on_drop` | yes | distinct paths, both gone after drop |
| C10 | `harness` `a_failed_run_or_missing_export_names_the_fixture` | yes | non-zero exit and missing export both named, with captured stdout |
| C11 | `lib` `golden::tests::an_unparsable_row_names_file_and_row` | yes | row 2 `1.0 abc`, a short row and a bad printed value |
| C12 | `lib` `golden::tests::an_export_is_found_with_or_without_data_suffix` | yes | both spellings read |
| C13 | `lib` `contract::tests::a_rejection_names_the_ngspice_version` | yes | every mutation run below printed `reference: ** ngspice-47 ...` |
| C14 | `lib` `ngspice::tests::the_binary_comes_from_the_variable_or_the_path` | yes | variable wins over PATH, empty variable is unset, bad path fails |
| C15 | `lib` `ngspice::tests::a_missing_binary_fails_naming_the_opt_out` | yes | confirmed live: PATH without ngspice turns parity_op red naming PIPERINE_NGSPICE=skip |
| C16 | `lib` `ngspice::tests::skip_passes_and_prints_one_line` | yes | the printed line itself is not captured by the test; `--nocapture` run shows one `SKIP <fixture>` line per parity test |
| C17 | `bench` `a_skipped_reference_still_solves_the_msl_side` | partly | does not go through a parity test's skip path; `compared` is vacuous (F2); live M19/M20 show today's parity tests do fail under skip |
| C18 | `lib` `contract::tests::the_five_contracts_carry_bounds_and_rationale` | yes | exact values 1e-3/1e-6, 1e-3/1e-12, 1e-3/1e-12, 0/1e-4, 1e-3/1e-6 |
| C19 | `lib` `contract::tests::holds_is_abstol_plus_reltol_of_the_larger` | yes | bisects the edge to one ulp and checks it against the formula with max(abs) of both sides; symmetric |
| C20 | `lib` `contract::tests::a_rejection_names_point_values_bound_and_rationale` | yes | fixture, quantity, all four coordinates, both values, abstol, reltol, rationale |
| C21 | `lib` `contract::tests::phase_is_gated_on_the_magnitude_floor` | yes | at the floor not compared, one ulp above compared |
| C22 | `corpus` `no_parity_target_compares_against_a_literal` | partly | the matcher sees only `<x>.abs() <op> <digit>`; `1e-3 > (a - b).abs()` escapes (F4) |
| C23 | `op` `divider` | yes | M1 (R1 x1.01) red on v(mid) |
| C24 | `op` `diode_series` | yes | M4 (D2 reversed) red on v(a) |
| C25 | `diode_iv` in `parity_dc` | yes | M5 (is x1.01), M6 (diode reversed), M8 (n=1.001) red; all 37 rows compared; reverse half sits under the 1 pA floor (F1) |
| C26 | `ac` `ac_rc` | yes | M9 (C x1.01), M10 (R x1.001), M11 (ac sign) red on phase; M21 (ac x1.002) red on magnitude |
| C27 | `lib` `golden::tests::a_grid_mismatch_names_the_point` | yes | half a unit accepted, one unit rejected; live M15 (40 points), M16 (linear), M17 (stop x1.0000001) red |
| C28 | `tran` `tran_rc` | yes | M12 (C x1.01), M13 (C x1.001), M14 (no held node) red; stop 9.95 us red naming 1e-5 s |
| C29 | `tran` `the_transient_step_cap_is_a_tenth_of_the_print_step` | yes | same `TranRun::options` the fixture test uses |
| C30 | `corpus` `every_twin_has_a_cir_and_a_test` | yes | six twins counted; a commented-out `fn <name>()` would still satisfy it (noted, not a finding) |
| C31 | `corpus` `a_literal_tolerance_in_a_parity_target_is_reported` | partly | proves the forward form only (F4) |
| C32 | `corpus` `the_guards_report_planted_text` | yes | missing .cir and missing test both reported |
| C33 | `twins` `the_current_source_twin_exists` | yes | the values are held by C34's comparison |
| C34 | `op` `isrc_r` | yes | M2 (R x1.01) and M3 (source reversed) red |
| C35 | `lib` `contract::tests::two_zeros_are_accepted` | yes | including -0.0 |
| C36 | `lib` `golden::tests::a_wrong_row_count_names_both_counts` | yes | fixture, 3 and 37 named |
| C37 | `lib` `waveform::tests::a_time_outside_the_result_is_none` | yes | both ends, one ps outside |
| C38 | `harness` `parallel_runs_do_not_share_a_scratch_directory` | yes | four threads, pairwise distinct |

## Mutations

Each mutation was applied to the working tree, the named target run with ngspice-47, and the file restored with `git checkout`; the tree was clean afterwards.

| Mutation | Target | Result | Reported at |
| --- | --- | --- | --- |
| M1 divider R1 1k to 1.01k | parity_op | red | v(mid), operating point |
| M2 isrc_r R1 x1.01 | parity_op | red | v(a) |
| M3 isrc_r source reversed | parity_op | red | v(a) |
| M4 diode_series D2 reversed | parity_op | red | v(a) |
| M5 diode_iv is x1.01 | parity_dc | red | i(v1) at 0.25 V |
| M6 diode_iv diode reversed | parity_dc | red | i(v1) at -1 V |
| M7 diode without the gmin shunt | parity_dc, parity_op | green | below the 1 pA floor (F1) |
| M8 diode_iv n 1.001 | parity_dc | red | i(v1) at 0.25 V |
| M9 ac_rc C x1.01 | parity_ac | red | phase at 1.26 kHz |
| M10 ac_rc R x1.001 | parity_ac | red | phase at 12.6 kHz |
| M11 ac_rc stimulus sign flipped | parity_ac | red | phase at 1 kHz |
| M21 ac_rc stimulus x1.002 | parity_ac | red | magnitude at 1 kHz |
| M15 AC sweep one point short | parity_ac | red | grid 41 vs 40 |
| M16 AC sweep linear | parity_ac | red | grid point 1 |
| M17 AC stop x(1+1e-7) | parity_ac | red | grid point 2 |
| M12 tran_rc C x1.01 | parity_tran | red | v(out) at 0.1 us |
| M13 tran_rc C x1.001 | parity_tran | red | v(out) at 0.1 us |
| M14 tran_rc without the held node | parity_tran | red | v(out) at 0.1 us |
| M18 transient stopped at 9.95 us | parity_tran | red | time 1e-5 s outside the result |
| M22 diode plus a constant 1 pA | parity_dc | green | below the 1 pA floor (F1) |
| M19 diode_iv type error, skip set | parity_dc | red | stage typeck, after the SKIP line |
| M20 tran_rc duplicate signal, skip set | parity_tran | red | stage parse, after the SKIP line |
| PATH without ngspice | parity_op | red x3 | `ngspice not found ... PIPERINE_NGSPICE=skip` |
| PIPERINE_NGSPICE=/nope | parity_dc | red | `names no file` |
| PIPERINE_NGSPICE=skip | all four parity targets | green | one `SKIP <fixture>: ...` line per fixture test |

## Decisions reviewed

| Decision | Judgement | Evidence | Risk |
| --- | --- | --- | --- |
| AC grid at the export's print precision (f9165f89) | sound | the window is half a unit in the 9th digit, 5e-9 relative; M16 and M17 (a 1e-7 relative shift) go red; a grid error inside 5e-9 moves the RC response by less than 1e-8 relative, far under AC_MAGNITUDE | none; the contracts are untouched and the export precision is read per token |
| Step aimed at the stop ends at the stop (9b6a38e2) | correct and safe | only fires when the step target lies within one tick of `stop_time`, so the end moves by under one tick; landings keep their branch; `cargo test -p piperine-solver` 209 passed in 41 targets, 0 failed, `parity_baseline` 7/7 and untouched by the commit (diff: `transient.rs`, `abi_tran.rs` only) | none found |
| Tolerance semantics | as written | `holds` is `abs(g - a) <= abstol + reltol * max(abs(g), abs(a))`, both sides' magnitude, values exactly the context.md decisions | none |
| All exported points compared | yes | op: every printed node; dc: loop over all 37 rows; ac: 41 after `same_grid` asserts equal length; tran: all 100 rows, outside-the-result panics | none |
| ngspice really runs | yes | `Ngspice::run` spawns the binary per test in a scratch cwd, the `--version` line is carried into every failure message (seen on every red mutation) | none |

## Twins

| Twin | Circuit match | Analysis match | Note |
| --- | --- | --- | --- |
| divider | V1 10 V, R1 1k vin-mid, R2 3k mid-0 | op, v(vin) v(mid) | exact |
| diode_series | V1 5 V, R1 1k, D1 a-b, D2 b-0, is 1e-14 n 1 | op, v(a) v(b) | ngspice defaults rs 0, cjo 0, bv inf, area 1, 27 C; MSL: no rs, cj0 0, 300.15 K, gmin 1e-12 from the solver defaults |
| diode_iv | V1, R1 1 ohm, D1 a-0 same model | dc V1 -1 to 0.8 step 0.05 (37 rows), i(v1) | sign convention matches (+1.01e-12 A at -1 V) |
| ac_rc | V1 AC 1, R 1000, C 1.5915494309189535e-9 | ac dec 10 1k 10Meg (41 rows) | exact |
| tran_rc | V1 1 V, R 1000, same C | tran 0.1u 10u uic (100 rows, no t=0 row) | uic replaced by the held node `out = 0.0` (M14 shows it matters) |
| isrc_r | I1 0 a 1 mA, R1 1k | op, v(a) = +1 V | `Isource(gnd, a)` pushes the current into a, as SPICE `I1 0 a` |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (Minor) The reverse half of `diode_iv` is not resolved: a diode without the gmin shunt, or with a constant 1 pA added, stays green, so the spec's assumption that the model must carry `gmin() * vd` is held by no test | M7 and M22 green; at -1 V golden 1.01e-12 A vs the 1e-12 A floor of `DC_CURRENT` (`crates/piperine-parity/src/contract.rs:66`) | record the resolution limit in spec.md's diode assumption row; holding gmin needs a point where gmin current clears the floor (a new fixture, since .cir files are frozen) or a user decision on the floor | |
| F2 (Minor) C17's test does not run a parity test's skip path: it calls `Ngspice::run` and `Bench::load` side by side, and `compared` is always 0 by construction; a parity test that returned on `Skipped` before `Bench::load` would keep it green | `crates/piperine-parity/tests/bench.rs:56-73`; M19/M20 show today's parity tests do fail under skip | drive the skip path through the parity tests' own code (e.g. with `Lookup::Skip` and a broken twin) and assert the stage failure | |
| F3 (Minor) C6's test asserts neither the span nor an exact code: `contains("P0002") or contains("E0")` | `crates/piperine-parity/tests/bench.rs:38`; the stage name is also a heuristic (`P0004` means elaborate, anything else typeck) at `crates/piperine-parity/src/bench.rs:45` | assert the exact code and the rendered span of the planted error | |
| F4 (Minor) The literal-tolerance guard catches one textual form only: `1e-3 > (a - b).abs()` and a second `.abs()` on a line pass it | `crates/piperine-parity/tests/parity_corpus.rs:30-41` (`find` takes the first `.abs()`, only the text after it is read) | match both operand orders and every occurrence; plant the reversed form in `a_literal_tolerance_in_a_parity_target_is_reported` | |
