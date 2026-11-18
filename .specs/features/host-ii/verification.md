# Host II verification

**Verdict**: FAIL at the Verifier's read; findings F1 to F7 and F9 to F13 closed in the fix round, F8 closed in part only (see the note after the findings) - two silent wrong answers in X9 (a placement written with a method call derives nothing, F1; timed `set`s derive against future values, F2), metadata that misreports derived params (F3), session options whose reach into six of the eight analyses no test holds (F5), and three behaviours that contradict the approved spec (F6 nodeset refused, F7 `tf` row names, F8 the PSS twin).
**Proofs**: `proofs.md` at fab38867 (131 pass, 0 fail) after the fix round; judged at ca8dcafb and the rows revised for the fix round
**Verifier**: independent sub-agent (author != verifier)

How it was probed: own MSL examples through the `piperine` binary built at HEAD (3-level derive chain, dc sweeps through it, interpreter, method-call placements, `piperine info` of a run with `params`, the five twins through the run keys, a pz network with a finite zero, PSS with and without the step cap, a reaction-driven square wave); a throwaway `piperine-api` test (deleted) for `set_at`/`drive` ordering, undo, same-port same-time, a drive past the stop; mutations in a scratch copy (deleted) of `session.rs`, `params.rs`, `host.rs`; the five new parity targets with `env -u PIPERINE_NGSPICE` against ngspice-47 (no SKIP). Tree clean at the end.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `request::tests::option_keys_are_read_for_every_analysis` | yes - the nine keys for all eight analyses (op, dc, ac, transient, tf, pz, sens, pss); `option_reach` holds that each reaches its solver (F5, F12) |
| C2 | `cli::tests::every_run_key_has_its_flag` | yes - every flag exists and reaches its field; "a flag wins" is held by `request::tests::a_flag_wins_over_its_option_key`, not by this proof |
| C3 | `request::tests::tolerance_keys_must_be_positive` | yes - 0 and a negative for each of the five keys, P0010 at the value naming the key |
| C4 | `request::tests::temp_must_be_above_absolute_zero` | yes - -273.15 itself refused |
| C5 | `request::tests::bounds_must_be_whole_numbers_of_at_least_one` | yes |
| C6 | `options.rs` `a_nodeset_naming_nothing_is_e0801` | yes - E0801 at open, naming the entry |
| C7 | `options.rs` `no_option_gives_the_default_context` | yes - every field compared with `Context::default()`; byte identity is C83 |
| C8 | `options.rs` `tolerance_keys_reach_the_context` | yes - the context holds the values; that each analysis receives it is C111 to C118 (F5) |
| C9 | `options.rs` `temp_is_celsius_read_by_temperature` | yes - `op` here, every other analysis in C111 to C118 (F5) |
| C10 | `options.rs` `a_nodeset_seeds_the_operating_point_and_keeps_its_solution` | yes - `op` here, `dc` and the other analyses in C111 to C118 (F5, F6) |
| C11 | `options.rs` `resolution_rounds_an_at_to_its_tick` | yes - 1.4 us fires at 1 us with the tick, at 1.4 us without |
| C12 | `options.rs` `maxrounds_bounds_the_operating_point` | yes - E0506 naming 1 against a design that needs two rounds |
| C13 | `options.rs` `maxmicro_bounds_an_instant` | yes - 5 and the default 1000 both named |
| C14 | `options.rs` `a_smaller_trtol_takes_more_steps` | yes - 3241 against 115 steps on `tran_rc` (the amended criterion) |
| C14a | `options.rs` `a_run_past_max_steps_fails_and_succeeds_when_raised` | yes - the budget is named, raised it succeeds |
| C15 | `parity_temp.rs` `temp_res` | yes - the Session layer with `--temp` is C120 (F9) |
| C16 | `parity_temp.rs` `temp_res_at_the_default_temperature` | yes - the Session layer is C120 (F9) |
| C17 | `table::tests::a_transfer_table_has_three_rows_in_order` | yes - rows `gain`, `input_impedance`, `output_impedance` in that order (F7) |
| C18 | `parity_tf.rs` `tf_divider` | yes - the Session layer with `input`/`output` is C121 (F9) |
| C19 | `request::tests::tf_and_pz_need_input_and_output` | yes |
| C20 | `analyses.rs` `tf_input_must_be_a_voltage_source` | yes - P0010 naming `Top.r1`, plus the happy path by entry and by path |
| C21 | `analyses.rs` `tf_output_naming_nothing_is_e0801` | yes |
| C22 | `table::tests::roots_list_poles_then_zeros` | yes - order and columns; empty zeros |
| C23 | `parity_pz.rs` `pz_lowpass` | yes - one pole and one finite zero compared root for root under PZ_ROOT 1e-9 (F10); the Session layer is C122 |
| C24 | `analyses.rs` `pz_of_a_circuit_without_poles_fails` | yes |
| C25 | `request::tests::tf_and_pz_need_input_and_output` | yes for AC 3; AC 4 and 5 for pz rest on the shared resolution code, no pz test |
| C26 | `table::tests::a_sensitivity_table_has_a_row_per_wrt` | yes |
| C27 | `parity_sens.rs` `sens_divider` | yes - the Session layer with `wrt` is C123 (F9) |
| C28 | `request::tests::sens_needs_a_non_empty_wrt` | yes |
| C29 | `analyses.rs` `sens_wrt_naming_nothing_is_e0801` | yes |
| C30 | `analyses.rs` `sens_of_a_discrete_net_is_refused` | yes |
| C31 | `analyses.rs` `pss_gives_one_period_of_rows` | yes - t0, t0 + T, monotone, closing orbit; "one row per accepted step" is `len > 4` |
| C32 | `analyses.rs` `pss_on_a_grid_gives_the_grid_rows` | yes - 101 rows at t0 + k*h |
| C33 | `analyses.rs` `pss_starts_its_orbit_at_tstab` | yes - two tstab values and the closed form at 10 us |
| C34 | `parity_pss.rs` `pss_rc` | partly - the analog `now()` pulse with a 1 ns step cap, not the spec's reaction-driven square wave (F8, OPEN); the Session layer is C124 |
| C35 | `request::tests::pss_needs_a_positive_period_and_a_non_negative_tstab` | yes |
| C36 | `analyses.rs` `pss_that_does_not_converge_fails` | yes - asserts the message `does not repeat: |x(2T) - x(T)| = 1.000e0`, the residual of a 3 us shot over a 4 us drive (F12) |
| C37 | `analyses.rs` `a_pss_plot_draws_its_rows` | yes |
| C38 | `cli/tests/analyses.rs` `each_new_analysis_prints_header_records_and_table` | yes |
| C39 | `cli/tests/analyses.rs` `each_new_analysis_writes_csv` | yes |
| C40 | `request::tests::plot_is_refused_for_tf_pz_and_sens` | yes |
| C41 | `request::tests::the_eight_analysis_names` | yes |
| C42 | `metadata.rs` `a_param_prints_its_value_in_every_row` | yes - a param no pin holds is C110 (F4) |
| C43 | `metadata.rs` `the_flow_of_a_single_flow_contribution_prints` | yes |
| C44 | `metadata.rs` `a_flow_of_several_contributions_is_e0801` | yes |
| C45 | `metadata.rs` `metadata_lists_every_param_with_type_value_where_and_structural` | yes - and a derived param shows its recomputed value (C109, F3) |
| C46 | `metadata.rs` `metadata_lists_ports_and_streams` | yes |
| C47 | `metadata.rs` `metadata_runs_no_analysis` | yes |
| C48 | `cli/tests/info.rs` `info_prints_params_ports_and_streams` | yes |
| C49 | `cli/tests/info.rs` `info_of_a_broken_design_prints_diagnostics_and_exits_1` | yes |
| C50 | `host_actions.rs` `a_drive_at_zero_is_the_initial_value` | yes - plus the undriven case |
| C51 | `host_actions.rs` `a_drive_later_fires_a_reaction_at_its_time` | yes - edge stamped 1 us; my off-grid drive at 1.234567 us fired at that time |
| C52 | `host_actions.rs` `driving_a_carrier_that_is_not_a_top_input_is_e0804` | yes - `out` port, node, signal; E0801 for nothing |
| C53 | `host_actions.rs` `a_drive_of_the_wrong_type_is_e0802` | yes |
| C54 | `host_actions.rs` `an_undriven_top_input_is_unknown` | yes |
| C55 | `host_actions.rs` `a_set_at_zero_applies_before_the_analysis` | yes |
| C56 | `host_actions.rs` `a_timed_set_applies_after_the_step_to_its_time` | yes - the row at `at` holds the old value |
| C57 | `session::tests::set_at_refuses_a_structural_param_after_the_start_before_the_run` | yes - `Session::set_at` on an IR marked structural: E0803 naming the param, the session still runs; removing `refuse_timed` fails it (F12) |
| C58 | `host_actions.rs` `a_negative_at_is_e0806` | yes |
| C59 | `host_actions.rs` `a_timed_set_failing_where_is_e0802` | yes for the set param; a derived child failing `where` in a timed set is C126 (F12) |
| C60 | `host_actions.rs` `subscribe_before_the_run_narrows_the_streams` | yes |
| C61 | `host_actions.rs` `subscribe_after_the_run_is_e0805` | yes |
| C62 | `cli/tests/run.rs` `a_fatal_run_prints_its_msg_and_exits_1` | yes - one fatal path after the rebase; `finish` exits 0 |
| C63 | `cli/tests/commands.rs` `test_prints_fail_fatal_with_the_msg` | yes - exact stdout (its comment mentions another run that it does not run) |
| C64 | `verdict::tests::a_finish_record_does_not_fail_the_verdict` | yes |
| C65 | `verdict::tests::the_last_record_of_each_target_is_kept_in_first_seen_order` | yes |
| C66 | `cli/tests/commands.rs` `test_prints_target_margins_after_the_run_line` | yes - exact stdout, negative margin, exit 0 |
| C67 | `verdict::tests::a_negative_margin_does_not_fail` | yes - std's `target` writes no `pass` field, so no path fails it |
| C68 | `digital_only.rs` `a_digital_only_transient_reaches_its_stop` | yes |
| C69 | `digital_only.rs` `a_digital_only_grid_holds_net_values` | yes |
| C70 | `digital_only.rs` `a_digital_only_run_ended_by_finish_is_ok` | yes (restated criterion) |
| C71 | `cli/tests/book.rs` `blinky_runs_to_its_stop_in_the_book` | yes |
| C72 | `params::tests::a_set_derives_its_children` | yes - with a hand-written `Derive`; generated code is held by C99 and C77; my 3-level chain (`q * k` over two levels) gives 0.75, 0.6, 0.428571 as computed |
| C73 | `derived.rs` `a_sweep_recomputes_the_child_at_each_point` | yes - one level; my two-level sweep also right |
| C74 | `derived.rs` `a_timed_set_recomputes_at_the_same_instant` | yes for one timed set of a one-input binding; two timed sets on a two-input binding are C107 and C108 (F2) |
| C75 | `derived.rs` `a_refused_set_leaves_the_param_the_circuit_reads_as_it_was` | yes - `Mid` reads `rr` through the node, so a missing undo gives 0.222 against 0.375 (F12) |
| C76 | `derived.rs` `a_set_neither_elaborates_nor_compiles` | yes - the cache is file-for-file unchanged |
| C77 | `derived.rs` `the_s2_probe_prints_three_quarters` | yes for `r: rr`; the method-call placement is C102 to C105 (F1) |
| C78 | `derived.rs` `a_body_let_reads_the_new_param` | yes |
| C79 | `derived.rs` `a_definition_is_recomputed_at_the_set` | yes |
| C80 | `params::tests::a_derived_param_cannot_be_set_directly` | yes - names `t` and `Top`; also via session in C75's test |
| C81 | `params::tests::the_interpreter_refuses_a_set_others_derive_from` | yes - CLI `--backend interp` refuses with `--backend rust` too |
| C82 | `params::tests::no_set_gives_the_folded_constants` | yes - every instance's params against the instance table |
| C83 | `twins.rs` `every_twin_prints_the_bytes_it_printed_before_the_long_run` | yes - byte goldens; a placement of literals is folded (`ChildValue::Const`), so the twins' plans are empty and their generated source unchanged |
| C84 | `parity_corpus.rs` `every_twin_has_a_cir_and_a_test` | yes - count 16 is the sum of both lanes' twins (append-only) |
| C85 | `parity_temp.rs` `temp_res` | yes - the harness's skip rule is shared by every target |
| C86 | `cli/tests/book.rs` `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes |
| C87 | `cli/tests/book.rs` `the_chapter_runs_each_new_analysis` | yes |
| C88 | `cli/tests/book.rs` `blinky_runs_to_its_stop_in_the_book` | yes |
| C89 | `cli/tests/docs.rs` `gaps_drops_what_host_ii_closed` | yes as text; S2 is not closed for method-call placements (F1) |
| C90 | `cli/tests/docs.rs` `the_roadmap_lists_the_host_follow_ups` | yes |
| C91 | root `host_ii` `the_root_runs_tf_a_drive_and_a_timed_set` | yes |
| C92 | `session_bounds.rs` `an_operating_point_past_max_rounds_fails_naming_the_bound` | yes |
| C93 | `session_bounds.rs` `an_instant_past_the_microstep_bound_fails_naming_it` | yes |
| C94 | `netlist::tests::with_microsteps_reaches_the_builder` | yes |
| C95 | `host_writes.rs` `write_net_before_a_run_is_an_initial_value` | yes |
| C96 | `host_writes.rs` `a_scheduled_write_commits_at_its_time` | yes |
| C97 | `parity_baseline` `parity_divider_op_dc` | yes |
| C98 | `derive::tests::a_chain_derives_top_down` | yes for the forms it builds; every form of `Expr` is C106 (F1) |
| C99 | `derive.rs` (transpile) `compiled_derive_evaluates_the_placement_expression` | yes - loaded function values, NaN for unknown `k` and wrong count |
| C100 | `ngspice::tests::print_lines_parse_to_named_values` | yes |
| C101 | `contract::tests::new_domains_carry_a_rationale` | yes - PZ_ROOT is 1e-9 / 1e-8 and its rationale is the numdgt=10 print precision (F10) |
| C102 | `a_method_call_placement_derives_through_a_param_flag` | yes - `r: rr.max(10.0)`, `rr = 3000` prints 0.75; fails with the old walk |
| C103 | `a_method_call_placement_derives_at_each_sweep_point` | yes - three different rows |
| C104 | `a_method_call_placement_derives_in_a_timed_set` | yes |
| C105 | `the_interpreter_refuses_a_set_a_method_call_placement_derives_from` | yes |
| C106 | `derive::tests::every_form_that_holds_an_operand_reports_the_params_it_reads` | yes - one walker shared by the derive plan and `ParamPath::reads` |
| C107 | `a_timed_set_derives_against_the_sets_due_before_it` | yes - the verifier's q*k scenario, rows 0.6/0.6/0.6/0.818 |
| C108 | `a_timed_set_given_out_of_time_order_derives_in_time_order` | yes |
| C109 | `metadata_lists_a_derived_param_at_the_value_the_run_uses` | yes |
| C110 | `a_param_no_pin_holds_prints_the_value_the_run_used` | yes - op, dc and a timed set in a transient |
| C111 | `an_operating_point_moves_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C112 | `a_dc_sweep_moves_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C113 | `an_ac_sweep_moves_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C114 | `a_transient_moves_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C115 | `a_transfer_function_moves_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C116 | `the_poles_move_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C117 | `a_sensitivity_moves_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C118 | `a_periodic_steady_state_moves_with_temp_and_nodeset` | yes - replacing the context in that arm fails it; the seed picks the root |
| C119 | `a_pss_orbit_that_does_not_close_at_the_step_says_so` | yes - the twin without a step cap fails with the new text |
| C120 | `temp_res_through_a_session` | yes - the run keys reach the comparison (dropping `temp` fails it) |
| C121 | `tf_divider_through_a_session` | yes - the run keys reach the comparison (dropping `temp` fails it) |
| C122 | `pz_lowpass_through_a_session` | yes - the run keys reach the comparison (dropping `temp` fails it) |
| C123 | `sens_divider_through_a_session` | yes - the run keys reach the comparison (dropping `temp` fails it) |
| C124 | `pss_rc_through_a_session` | yes - the run keys reach the comparison (dropping `temp` fails it) |
| C125 | `the_table_covers_every_twin` | yes - one row per twin, the free-running step of pss_rc beside its capped one |
| C126 | `a_timed_set_whose_derived_child_fails_where_is_refused` | yes - skipping the check in the timed path fails it |
| C127 | `an_action_past_the_stop_is_e0806_at_the_call` | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (silent, HII-18 AC 1/2/3/6, S2) - a placement value written with a method call derives nothing: `Mid { rr } { res: Resistor { r: rr.max(10.0) } }`, `analog for Mid { i(p, n) +> 0.0 * rr; }`, `--param Top.m.rr=3000` prints `Top.b 5.000000e-1` (0.75 expected); the dc sweep 1k/2k/3k prints 0.5 three times; the interpreter does not refuse. Without the `analog for Mid` line it is the loud H5 error instead | `piperine-plan/src/derive.rs` `params_read` descends only Unary/Cast/Binary/If/Math/Call; `Expr::Method`, `KnownOr`, `IsKnown`, `High` read nothing, so `readers()` is empty. `ParamPath::reads` in `session.rs` does descend them, which is why `sens --wrt m.rr` refuses the same design | one walker over every `Expr` variant, shared by `DerivePlan` and `ParamPath::reads`; a `derived.rs` test with a method-call placement (set, sweep, timed set, interpreter refusal) | 72265b6c - `ParamReads`, one exhaustive walk shared by the derive plan and `ParamPath::reads`; C102 to C106 |
| F2 (silent, HII-18 AC 3, HII-13) - timed sets derive against values that are not yet true. `rr: q * k`: `set_at(q, 1500, 2us)` then `set_at(k, 3, 0)` gives rows 0.818, 0.818, 0.818, 0.75 (true: 0.6, 0.6, 0.6, 0.818); `set_at(q, 1500, 2us)` then `set_at(k, 3, 1us)` gives 0.5, 0.5, 0.818, 0.75 (true: 0.5, 0.5, 0.6, 0.818). The final state is wrong in both | `host.rs` `set_at` calls `self.params.apply(&changes)` at schedule time, so a later action (earlier in time, or at 0) derives from a future value, and the earlier-scheduled derived values are frozen with the old inputs | keep the table at the run's start and derive timed sets in time order at run start (each from the table as of its `at`), or refuse a second action that feeds a binding already scheduled; test two-input bindings with out-of-order and at-0-after-timed sets | a6854b73 - timed sets kept as given, derived in time order against the table as of each time; C107, C108 |
| F3 (silent, HII-10 AC 1, HII-11) - metadata reports a derived param at its elaborated value while the run uses the recomputed one: a run with `params = { "Top.o.q" = 1500.0 }` prints `Top.b 7.5e-1` but `piperine info main` lists `Top.o.m.rr 1.000000e3` and `Top.o.m.res.r 1.000000e3` (3000 both) | `Session::metadata_of` overlays only `spec.params` on the IR constants; it never reads `Params` (nor host `set_at`s) | build the metadata values from the session's `Params` table (and for `describe`, from a `Params` with the request's sets applied); test a derived child's value | a6854b73 - metadata and `describe` read the params table (`Evaluated` derive for `describe`); C109 |
| F4 (loud, HII-09 AC 1) - `print` of a param no device pin holds fails: `--print Top.o.q` on the chain design is `Backend("`Top.o.q` has no value in the circuit: no net of the digital plan has that path")`; the spec says it prints the value the run used | `Probe::Param` reads the param's net; a pass-through param (one only placements read) has none | read `Probe::Param` from the session's `Params` table (with timed sets applied at their time) | a6854b73 - `Held` reads a pass-through param from the table at each row's time; C110 |
| F5 (test gap, HII-02 AC 2 to 4, "for every analysis") - nothing proves the options reach dc, ac, tf, pz, sens or pss, nor that a nodeset seeds each dc point | mutation in a scratch copy: `Context::default()` in the dc, tf, pz, sens and pss arms, `with_context` dropped from ac, `apply_initial_conditions` dropped from dc; `piperine-api` lib + options, analyses, derived, host_actions, metadata, digital_only, session, probes and `piperine-cli` analyses, run, twins all green | per analysis, one test whose result moves with an option (e.g. `temp` through `TcResistor` in dc/ac/tf/sens/pss, `reltol` or `maxrounds` in dc, a nodeset on a dc point of the cubic) | fb5ab449 - `option_reach`: each of the eight analyses moves with `temp` and `nodeset` against a closed form; C111 to C118 |
| F6 (loud, spec divergence, HII-01 AC 1, HII-02 AC 4) - `nodeset` is refused (P0010) on ac, transient, tf, pz, sens and pss; the spec accepts the keys "for every analysis" and seeds "the operating point". The refusal was the worker's rule (handoff T6/T13/T18), not an amendment the user approved | `session.rs` `nodeset_of`; `options.rs` `a_nodeset_on_a_run_that_takes_its_own_operating_point_is_p0010` | give the solver's internal operating point a seed door (as `apply_initial_conditions` on dc), or put the narrowing to the user as a spec amendment | fb5ab449 - `Context::nodeset` seeds every `DcSolver::new` (ac, transient, tf, pz, sens, pss); the P0010 refusal is gone; C111 to C118 |
| F7 (loud, spec divergence, HII-04 AC 1) - the `tf` table's rows are `gain`, `input_resistance`, `output_resistance`; the spec and C17 say `input_impedance`, `output_impedance` (contract.md records the rename, the spec was not amended) | `table.rs` `Table::transfer`; CLI output of `tf_divider`; C17's test asserts the code's names | rename the rows to the spec's, or amend HII-04 AC 1 with the user | 6ce8257c - rows `input_impedance` and `output_impedance`; C17's test, the analyses and book assertions and the Book listing follow |
| F8 (loud, misleading, HII-07 AC 4 and the user story) - the PSS twin is not the spec's: AC 4 names an RC "driven by a square wave ... made by a reaction of the bench"; that form fails (`every(2u, 2u)` clock into `D2A`: "digital state is not periodic at T (and does not close within 4·T)", H15). The analog `now()` pulse that replaced it converges only with the 1 ns cap: `piperine run tests/ngspice/pss_rc.msl --analysis pss --period 4e-6 --tstab 8e-6` with no `--max-step` fails "the drive is not periodic at T=4.000e-6" (abs(x(2T) - x(T)) = 1.6e-3), and with `--max-step 4e-9` the same (2.3e-4). The drive is periodic; the step misplaces the 1 ps edges | probes at HEAD; `parity_pss.rs` `PssRun::STEP = 1e-9` and `analyses.rs` `pss()` pin the cap | give PSS shots edge breakpoints for an analog pulse and a relative `transition` time (H15), or amend AC 4 with the user; at least make the error say the orbit did not close at this step rather than that the drive is not periodic | 0610c6aa (PARTIAL, STOPPED per the ruling) - the misleading message is fixed (C119); the twin as the spec writes it is NOT delivered: see the F8 note below |
| F9 (test layer, HII-03 to HII-07 "WHEN the twin runs ...") - the five parity targets call the solver through `Bench` with `Context::default()` or a hand-set temperature; none runs the twin through `Session`, so `temp` as a run key, `input`/`output` resolution, `wrt`, and the pss arm's grid are never compared with ngspice. My CLI runs of all five agree with ngspice, so no wrong value is behind it today | `parity_{temp,tf,pz,sens,pss}.rs` | run each twin through `Session` with its run keys in the parity target (the Bench comparison can stay) | 80d13acf - `HostRun` runs each twin through `Session` with its run keys; C120 to C124 |
| F10 (tolerance, MD-38) - `PZ_ROOT` (1e-5 rel, 1e-6 abs) is justified as "ngspice prints six significant digits", but `pz_lowpass.cir` sets `numdgt=10` and ngspice prints `-1.333333333e3`; the bound sits ~1e4 above the printed precision and the observed deviation is 2.5e-10 relative. And the twin has no finite zero, so "root for root" never compares a zero | `contract.rs` rationale; `parity_pz` output; `pz_lowpass.cir` | tighten the bound to the printed precision or rewrite the rationale on a stated property of ngspice's pz search; add a zero to the twin (an RC lead, zero at -1/(R1 C1), agrees with ngspice in my probe) | d78517a4 - PZ_ROOT 1e-9 / 1e-8 with the numdgt=10 rationale; the twin has a finite zero; C23, C101 |
| F11 (parity hole) - `deviation.rs` `OWN_TARGET` drops all five new twins "as not a waveform or a sweep", but `pss_rc` is a waveform and `temp_res` an operating point, the two forms the table holds; the table's free-running column exists to show what a step cap hides, which is exactly F8 | `crates/piperine-parity/tests/deviation.rs` | keep `pss_rc` (with its free-running step) and `temp_res` in the table; exclude only tf, pz, sens | 031b050a - `temp_res` and `pss_rc` are rows of the deviation table; C125 |
| F12 (loose assertions) - C75's target cannot see a missing undo; a timed set whose derived child fails `where` (`check_set` on derived changes in `host.rs`) has no test (mutation skipping it: green); C57 never calls `set_at` (removing `refuse_timed` from it stays green); C36 accepts "residual" or "period" or "repeat"; C1 covers four of eight analyses | mutations in a scratch copy; test bodies | make `Mid` read `rr` in C75's design and assert the node after the refusal; add a timed derived-child refusal test; drive C57 through `set_at` on a session whose IR is marked; assert the residual; add tf, pz, sens, pss to C1 | 89857df0 - C75, C57, C36, C1 tightened and the timed-derived-where test added; C126 |
| F13 (low) - a host action after the stop is accepted and never applied: `drive("clk", true, 5e-6)` on a 2 us run returns Ok and the run delivers nothing for it; the spec has no rule, E0806 covers only the past | throwaway probe | refuse (or warn) an `at` past the transient's stop at the call, or say in the API doc that it is dropped | 641cc2e8 - E0806 for an action past the transient's stop; C127 |

Not judged: the Book's prose beyond its held listings, `GAPS.md`/`ROADMAP.md` wording beyond the pinned assertions, performance of the PSS shots.

**F8 note (fix round, stopped per the ruling).** The `every(2u, 2u)` clock into `D2A` does not shoot, and closing it is not a contained change. At `tstab = 8 us`, `T = 4 us` the periodicity check finds two nets that differ between the start and the arrival of a period: `PssRc.clock.every$0.k`, the `u64` counter of the stdlib `every` (4 against 6, +2 per period), and `PssRc.drive.ramp.t0`, the `f64` of the stdlib `transition` that holds the absolute time of its last edge (a different value each period). A counter-free clock written as a reaction (`on init() { clk <- false after half; } on clk.changed() { clk <- !clk after half; }`) leaves only `ramp.t0`. `t0` is an ordinary `signal` of `transition`'s `digital for` block in `crates/piperine-lang/std/analog.msl`, committed from `now()`: the solver sees 16 bytes, not a time, so comparing it "relative to the period" needs either a time-stamp marker carried from the language through the IR, the plan and the digital net spec (lang, IR, plan, transpile, digital solver: the protocol redesign the ruling says to stop at), or an edit to the stdlib `transition` (lane A). The `now()` pulse's edges need a breakpoint source for an analog expression, a second mechanism the analog solver does not have. `GAPS.md` H15 stays. Delivered: the misleading message (C119).
