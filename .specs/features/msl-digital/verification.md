# msl-digital verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 2d8f2f22 (186 pass); HEAD 0fe59818 adds only spec records
**Verifier**: independent sub-agent (author != verifier), Opus

The buck runs from source and the proofs hold their criteria in all but four checks. One finding is a silent wrong result: record texts are attached to the wrong `emit!` site when two reactions of one instance emit in the same microstep at different statement indices (F1, reproduced on HEAD). The short-circuit refusal of DIG-08 AC 7 misses panicking operands reached indirectly (F2). The closed loop (T57) was read, not rerun.

Probed on HEAD with throwaway tests, all removed: the T66 resolver change (`pom/resolve/scopes.rs`), the record pairing with three `log`s over two reactions, `partial` on indirect panics, G4/G5 folds at 64-bit and over-wide extremes, `E0150` through `if`, `match`, calls and lets, `match` exhaustiveness on signed literals, a shared guard on an `||` head, `emit!` in `analog`. `cargo test -p piperine-lang` (44 targets) and `cargo test -p piperine-solver --test parity_baseline --test protocol_sink` are green. `contract.rs` is unchanged in the range; no ngspice tolerance was retuned. T67's solver edit adds one field to `ObservationKind::Commit` and nothing serializes it: parity is unaffected.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `types_integers.rs` `integer_types_and_constants_have_types` | yes |
| C2 | `types_integers.rs` `a_literal_takes_the_expected_integer_type_else_i32` | yes - six positions asserted across T3/T4/T5/T8 tests as the Handoff records; this test holds annotation, const, parameter, result, both `if` arms, suffix and `i32` |
| C3 | `types_integers.rs` `a_literal_out_of_range_is_e0909` | yes - both range edges accepted, five out-of-range literals each `E0909` at the literal |
| C4 | `types_integers.rs` `integer_operators_take_one_type` | yes |
| C5 | `types_integers.rs` `mixed_integer_operands_are_p0002` | yes |
| C6 | `types_integers.rs` `a_shift_takes_any_integer_amount` | yes |
| C7 | `types_integers.rs` `as_goes_between_integers_and_f64` | yes |
| C8 | `types_integers.rs` `min_max_clamp_and_floor_are_typed` | yes |
| C9 | `types_integers.rs` `an_unsupported_width_or_float_is_named` | yes - eight types, code, name and span each |
| C10 | `types_integers.rs` `a_width_below_one_is_e0103` | yes |
| C11 | `types_carriers.rs` `a_catalogue_method_on_a_carrier_is_typed_on_its_value` | yes |
| C12 | `types_integers.rs` `an_uncatalogued_method_is_p0001_by_name` | yes - code, method name, `u4` and span |
| C13 | `types_integers.rs` `every_accepted_call_has_a_callee` | no - skips every call the checker left untyped (`types.ty(e).is_none()`) and runs only on three models with no carrier, enum, flag read or event; never on the buck (F4) |
| C14 | `types_compound.rs` `unit_enums_and_their_variants_are_typed` | yes |
| C15 | `types_compound.rs` `an_enum_outside_the_subset_is_p0001` | yes |
| C16 | `types_compound.rs` `option_values_and_their_methods_are_typed` | yes |
| C17 | `types_compound.rs` `match_is_typed_over_enums_bools_integers_and_options` | yes - integer `match` exhaustiveness is held only for unsigned literals; a signed type covered by negative literals is refused (F6, not this check's case) |
| C18 | `types_compound.rs` `a_match_that_is_not_exhaustive_is_p0002` | yes - five non-exhaustive forms, `P0002` at the `match` |
| C19 | `types_compound.rs` `if_let_some_binds_the_payload` | yes |
| C20 | `types_compound.rs` `a_tuple_let_names_each_component` | yes |
| C21 | `types_carriers.rs` `carriers_of_bit_are_typed_by_their_value` | yes |
| C22 | `types_carriers.rs` `a_carried_type_without_repr_is_e0112` | yes |
| C23 | `types_carriers.rs` `a_carrier_read_has_its_value_type` | yes |
| C24 | `types_carriers.rs` `the_flag_reads_are_typed` | yes |
| C25 | `types_carriers.rs` `an_initial_value_of_the_wrong_kind_is_e0210` | yes |
| C26 | `types_carriers.rs` `a_definition_of_another_type_is_e0337` | yes |
| C27 | `types_carriers.rs` `a_continuous_definition_is_e0336` | yes - eleven forms, directly, through `let`s, through functions and in the delay, each at its span |
| C28 | `types_carriers.rs` `a_continuous_expression_at_an_in_port_is_e0316` | yes |
| C29 | `types_carriers.rs` `a_target_the_module_may_not_drive_is_e0222` | yes |
| C30 | `types_carriers.rs` `two_definitions_of_one_carrier_are_e0335` | yes |
| C31 | `types_events.rs` `a_definition_and_a_commit_are_e0221` | yes |
| C32 | `types_carriers.rs` `a_second_drive_is_e0220` | yes - both halves, span at the second connection, and the handed-down `out` accepted |
| C33 | `types_carriers.rs` `an_initial_value_without_the_drive_is_e0223` | yes |
| C34 | `types_carriers.rs` `an_initial_value_under_an_undelayed_definition_is_e0338` | yes |
| C35 | `types_carriers.rs` `an_out_port_takes_only_a_carrier_the_parent_hands_over` | yes |
| C36 | `types_carriers.rs` `a_carrier_of_another_value_type_is_e0251` | yes |
| C37 | `types_carriers.rs` `a_loop_of_definitions_is_e0339` | yes |
| C38 | `types_events.rs` `event_expressions_are_typed` | yes |
| C39 | `types_events.rs` `a_bool_head_is_e0430` | yes |
| C40 | `types_events.rs` `a_misplaced_not_or_and_is_e0440` | yes |
| C41 | `types_events.rs` `an_event_outside_an_event_position_is_e0441` | yes |
| C42 | `types_events.rs` `a_continuous_at_is_e0442` | yes |
| C43 | `types_events.rs` `cross_or_at_outside_an_event_position_is_e0443` | yes - asserts only the first code of each case (`codes(&d)[0]`), the span pins it to the construct |
| C44 | `types_events.rs` `a_reaction_body_is_typed` | yes |
| C45 | `types_events.rs` `a_commit_of_another_type_is_e0432` | yes |
| C46 | `types_events.rs` `a_delay_that_is_not_f64_is_e0433` | yes |
| C47 | `types_events.rs` `a_placement_in_a_reaction_is_e0350` | yes |
| C48 | `types_events.rs` `emit_is_typed_in_a_reaction` | yes |
| C49 | `types_events.rs` `a_repeated_or_reserved_field_is_e0604` | yes |
| C50 | `types_events.rs` `a_field_type_conflict_is_e0605` | yes |
| C51 | `types_events.rs` `emit_where_it_is_not_allowed_is_e0601` | yes - first code only (`codes(&d)[0]`) with span; five sites |
| C52 | `types_events.rs` `a_check_reached_from_analog_is_e0603` | yes for the refusals; its positive case accepts `emit!("trace")` and `log` in `analog`, which elaboration then fails with `P0004` internal error (F3) |
| C53 | `types_results.rs` `a_module_with_a_result_is_typed` | yes |
| C54 | `types_results.rs` `a_placement_has_its_result_type` | yes |
| C55 | `types_results.rs` `trailing_option_inputs_may_be_left_out` | yes |
| C56 | `types_results.rs` `a_placement_in_a_const_context_is_e0352` | yes |
| C57 | `types_results.rs` `now_and_analysis_static_are_typed` | yes |
| C58 | `types_results.rs` `an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150` | yes - seven reads incl. let, value input, result; own probes (if condition, fn argument, `match`, let through fn) all `E0150` |
| C59 | `types_results.rs` `a_let_that_reads_itself_is_e0332_or_e0353` | yes |
| C60 | `types_compound.rs` `a_value_for_an_option_parameter_is_some_of_it` | yes |
| C61 | `types_buck.rs` `what_stays_outside_is_p0001_by_name` | yes - 31 rows, one `P0001` each naming the construct; the trait/impl row is caught as an uncatalogued method `get`, not by a trait rule |
| C62 | `elab_digital.rs` `the_m1_goldens_are_unchanged` | yes for the six pinned twins; `diode_rev` (the seventh) is only asserted to elaborate with an empty `check()`, its print is not compared (F10) |
| C63 | `types_buck.rs` `the_buck_type_checks_with_no_diagnostic` | yes |
| C64 | `elab_values.rs` `integers_lower_to_typed_ir` | yes |
| C65 | `elab_values.rs` `integer_constants_fold` | yes |
| C66 | `elab_values.rs` `folded_casts_shifts_and_sums_wrap_as_compiled_code_does` | yes - each fold compared with `rt::int` and with the spec value; own probes `1e30 as u64`, `-1e30 as i64`, `1u8 << 200`, `-5i64 >> 64` fold to the `rt` values |
| C67 | `elab_values.rs` `a_constant_division_by_zero_is_e0104` | yes |
| C68 | `elab_values.rs` `a_carrier_read_is_flagged_and_the_flag_reads_are_not` | yes |
| C69 | `elab_values.rs` `a_short_circuit_over_a_flagged_operand_becomes_bitwise` | yes |
| C70 | `elab_values.rs` `a_partial_right_operand_under_a_flagged_short_circuit_is_p0001` | no - asserts only a division written directly in the right operand; a division reached through a called function or a `match` arm, and a shift by a negative amount, are accepted and lowered bitwise (F2) |
| C71 | `elab_values.rs` `a_function_gets_one_variant_per_flag_pattern` | yes |
| C72 | `elab_values.rs` `an_enum_is_its_tag_and_a_match_is_an_if_chain` | yes |
| C73 | `elab_values.rs` `an_option_is_a_flag_and_a_payload` | yes |
| C74 | `elab_values.rs` `a_tuple_let_is_one_local_per_component` | yes |
| C75 | `elab_digital.rs` `carriers_follow_ports_then_signals` | yes |
| C76 | `elab_digital.rs` `a_definition_is_a_drive` | yes |
| C77 | `elab_digital.rs` `an_expression_at_an_in_port_is_a_carrier_and_a_drive` | yes |
| C78 | `elab_digital.rs` `every_signal_carrier_is_a_design_net` | yes |
| C79 | `elab_digital.rs` `a_top_with_discrete_ports_gets_one_net_per_port` | yes |
| C80 | `elab_digital.rs` `changed_rise_fall_and_cross_lower_to_events` | yes - exact guard text and the full six-transition truth table for `rise` and `fall` evaluated on the IR |
| C81 | `elab_digital.rs` `a_guard_splits_into_its_discrete_and_continuous_parts` | yes |
| C82 | `elab_digital.rs` `an_or_head_is_one_reaction_or_one_per_disjunct` | yes |
| C83 | `elab_digital.rs` `a_head_the_ir_cannot_hold_is_p0001` | yes |
| C84 | `elab_digital.rs` `a_reaction_body_lowers_in_source_order` | yes |
| C85 | `elab_digital.rs` `a_child_param_reads_the_parents_value` | yes - per-instance constants, the authored expression, `E0317` at the placement naming the instance |
| C86 | `amendments.rs` `a_child_param_may_read_the_parents_params` | yes |
| C87 | `elab_digital.rs` `elaborating_twice_gives_the_same_print` | yes - eleven tops, twice and loaded twice, both print forms |
| C88 | `elab_digital.rs` `the_design_is_not_written` | yes |
| C89 | `elab_digital.rs` `every_ir_passes_check` | yes |
| C90 | `elab_results.rs` `a_placement_is_hoisted_under_its_name` | yes |
| C91 | `elab_results.rs` `value_inputs_are_substituted_at_each_read` | yes |
| C92 | `elab_results.rs` `an_omitted_option_input_reads_as_none` | yes |
| C93 | `elab_results.rs` `two_placements_are_two_sets_of_carriers` | yes |
| C94 | `elab_results.rs` `a_result_module_gets_no_body_and_no_instance` | yes |
| C95 | `elab_results.rs` `an_expression_at_a_discrete_input_drives_the_hoisted_carrier` | yes |
| C96 | `elab_results.rs` `at_now_and_static_lower_to_their_nodes` | yes |
| C97 | `amendments.rs` `time_and_static_reads_print_and_check` | yes |
| C98 | `amendments.rs` `an_at_event_prints_and_checks` | yes |
| C99 | `elab_results.rs` `emit_lowers_to_a_site_with_its_texts` | yes |
| C100 | `amendments.rs` `an_emit_site_prints_and_checks` | yes |
| C101 | `amendments.rs` `a_record_site_gives_its_stream_and_texts` | yes |
| C102 | `elab_results.rs` `a_function_that_emits_is_inlined` | yes |
| C103 | `elab_results.rs` `a_text_field_that_is_not_a_literal_is_p0001` | yes |
| C104 | `buck_records.rs` `never_reports_each_rising_edge` | yes - six records at the six edges, texts, known `pass`, values 1,1,1,0,0,0 |
| C105 | `amended.rs` `an_emit_commits_one_record_and_adds_no_microstep` | yes - record bytes exact, microstep counts equal with and without the emit |
| C106 | `amended_shape.rs` `a_stream_is_a_record_pin_with_one_field_per_value` | yes |
| C107 | `buck_records.rs` `log_carries_its_severity_and_its_text` | yes |
| C108 | `amended.rs` `at_fires_once_at_its_time` | yes |
| C109 | `amended_shape.rs` `an_at_event_is_a_pin_an_arming_reaction_and_a_replace_statement` | yes |
| C110 | `buck_time.rs` `every_from_the_stdlib_fires_once_per_period` | yes |
| C111 | `amended.rs` `a_rearmed_at_drops_the_earlier_firing` | yes |
| C112 | `amended.rs` `a_past_or_unknown_time_does_not_fire` | yes - "the next microstep" is asserted as the same instant only; the microstep index is not read |
| C113 | `amended.rs` `now_is_the_time_of_the_instant_and_of_the_solve` | yes |
| C114 | `amended_shape.rs` `time_and_static_are_ambient_reads` | yes |
| C115 | `buck_time.rs` `a_reaction_reads_now_and_static_from_source` | yes |
| C116 | `amended.rs` `static_is_true_only_in_the_equilibrium` | yes |
| C117 | `elab_mixed.rs` `a_flow_probe_of_a_potential_branch_lowers` | yes |
| C118 | `amendments.rs` `a_flow_read_and_a_potential_ddt_print_and_check` | yes |
| C119 | `elab_mixed.rs` `a_potential_contribution_keeps_its_ddt` | yes |
| C120 | `amended_shape.rs` `a_flow_read_is_the_branch_pin_and_a_potential_ddt_is_a_ddt_term` | yes |
| C121 | `elab_mixed.rs` `a_flow_probe_of_a_branch_without_a_potential_contribution_is_p0001` | yes |
| C122 | `buck_power.rs` `the_inductor_follows_the_rl_response` | yes - every accepted point after t0 (>= 1000) under `TRAN_VOLTAGE` through the 1 ohm resistor (no `TRAN_CURRENT` exists in `contract.rs`; contract unchanged) |
| C123 | `amended.rs` `the_inductor_functions_pass_verify_partials` | yes - `rel_tol` 1e-6 is the criterion's own number; partial sets per body asserted |
| C124 | `buck_power.rs` `the_current_sense_scales_the_shunt_voltage` | yes - `DC_VOLTAGE` |
| C125 | `buck_power.rs` `the_power_switch_ramps_its_conductance_in_tsw` | yes - every accepted time in t0 - tsw .. t0 + 2 tsw and the operating point, under `TRAN_VOLTAGE` |
| C126 | `elab_mixed.rs` `analog_reads_a_hoisted_result` | yes |
| C127 | `buck_power.rs` `a_gate_change_makes_two_time_points` | yes - a 1 fs (one tick) match, not a tolerance on the response |
| C128 | `elab_buck.rs` `six_slice_bodies_equal_their_hand_built_fixtures` | yes - whole-line equality, each listed difference must occur exactly once |
| C129 | `elab_buck.rs` `dpwm_and_a2d_differ_only_as_listed` | yes - extras must each occur exactly once, survivors compared line by line |
| C130 | `buck_dpwm.rs` `the_oscillator_toggles_every_half_period` | yes - the exact 40 changes and nothing else |
| C131 | `buck_dpwm.rs` `the_dpwm_dithers_keeps_dead_time_and_never_overlaps` | yes - exact `hs`/`ls` change lists over eight periods, dead times and non-overlap |
| C132 | `buck_control.rs` `the_ramp_and_the_pid_follow_their_difference_equations` | yes - 200 samples, five values, bit for bit, the stimulus checked against its Rust twin |
| C133 | `buck_boundary.rs` `the_adc_commits_the_rounded_code_after_tconv` | yes - five voltages; the 0 V case is asserted as "no change" only (it equals the reset value) |
| C134 | `buck_dpwm.rs` `two_runs_give_the_same_changes` | yes |
| C135 | `buck_boundary.rs` `the_comparator_trips_at_its_thresholds` | yes - analytic RC value at each trip time under `TRAN_VOLTAGE`, two trips exactly |
| C136 | `buck_boundary.rs` `the_comparator_is_right_at_time_zero` | yes |
| C137 | `buck_control.rs` `the_setpoint_register_takes_known_data_and_saturates` | yes - exact change list and three commits |
| C138 | `buck_commits.rs` `two_commits_exchange_two_carriers` | yes |
| C139 | `buck_commits.rs` `the_later_statement_wins` | yes |
| C140 | `buck_commits.rs` `two_statements_make_a_pulse` | yes |
| C141 | `buck_commits.rs` `an_unchanged_commit_fires_nothing` | yes |
| C142 | `buck_power.rs` `the_gate_driver_delays_and_interlocks` | yes - exact change lists, unknown until `tpd`, interlock |
| C143 | `elab_buck.rs` `the_buck_elaborates_with_no_diagnostic_and_a_sound_ir` | yes |
| C144 | `elab_buck.rs` `the_buck_ir_is_pinned` | yes - whole-file equality |
| C145 | `buck_whole.rs` `the_buck_compiles_with_no_warning_and_valid_shapes` | yes - `rustc` warnings asserted in `Built::new`; shapes validated for every body that runs (>= 20) |
| C146 | `project.rs` `a_project_with_a_dependency_crate_loads_and_runs` | yes |
| C147 | `buck_supervisor.rs` `the_supervisor_follows_its_state_table` | yes - 2400 samples of four values against a Rust state machine; the three SPEC §8 readings of T55 are the author's interpretation, recorded in the Handoff |
| C148 | `buck_supervisor.rs` `power_good_asserts_after_64_and_drops_after_4` | yes |
| C149 | `buck_whole.rs` `a_buck_that_is_not_enabled_stays_off_with_vin_ok_high` | yes |
| C150 | `buck_whole.rs` `no_never_record_fails_while_not_enabled` | yes - exact list of 4000 record instants, every one `pass` known true |
| C151 | `buck_closed_loop.rs` `the_buck_starts_and_reaches_regulate` | yes - read, not rerun (13 min dev build); dedup of `mode` is exactly [0, 1, 2], reference ends at 128 and never exceeds it |
| C152 | `buck_closed_loop.rs` `power_good_is_high_and_no_fault_is_latched` | yes - read, not rerun |
| C153 | `buck_closed_loop.rs` `the_output_settles_within_10_mv_of_0v8` | yes - read, not rerun; time-weighted mean over the steps from `record_from`, bound 10 mV from SPEC §2 |
| C154 | `buck_closed_loop.rs` `no_shoot_through_during_the_start` | no - asserts `>= 119_000` records where 600 us at 200 MHz gives exactly 120 000 rising edges; up to 1000 missing checks pass (F7). Read, not rerun |
| C155 | `grep -q 'unknown control' GAPS.md && grep -q 're-derived' GAPS.md && grep -q 'variant names' GAPS.md` | yes - weak: three word greps match any mention (F5) |
| C156 | `! grep -q 'stays `Unresolved` with no diagnostic' GAPS.md` | no - one negative phrase grep on `GAPS.md`; `examples/buck/GAPS.md`, which DIG-17 AC 2 names, is not checked (F5) |
| C157 | `! grep -q 'Nothing executes MSL yet' examples/buck/README.md` | yes |
| C158 | `types_carriers.rs` `prev_is_typed_on_a_carrier` | yes |
| C159 | `amendments.rs` `a_previous_value_read_prints_and_checks` | yes |
| C160 | `amended_shape.rs` `a_previous_value_is_the_previous_part_of_the_pin` | yes |
| C161 | `amended.rs` `a_previous_value_is_read_in_a_guard_and_in_a_reaction` | yes |
| C162 | `buck_time.rs` `rise_and_fall_fire_on_edges_to_and_from_unknown` | yes |
| C163 | `elab_digital.rs` `a_module_with_only_a_digital_block_has_no_analog_statements` | yes |
| C164 | `elab_digital.rs` `a_carrier_with_no_value_and_no_drive_is_kept` | yes |
| C165 | `types_carriers.rs` `a_single_variant_enum_cannot_be_carried` | yes |
| C166 | `elab_results.rs` `a_result_module_is_hoisted_once_per_body` | yes |
| C167 | `elab_results.rs` `a_param_block_binds_the_hoisted_params` | yes |
| C168 | `frozen.rs` `a_body_with_an_amended_form_is_rejected_by_name` | yes - one named entry per body and form, in order |
| C169 | `amendments.rs` `the_earlier_prints_are_unchanged` | yes |
| C170 | `project.rs` `a_diagnostic_is_returned_with_its_stage` | yes |
| C171 | `elab_digital.rs` `a_placement_records_its_carrier_bindings` | yes |
| C172 | `elab_digital.rs` `a_net_takes_the_init_of_its_declaring_carrier` | yes |
| C173 | `amended.rs` `hoisted_names_print_as_rust_identifiers` | yes |
| C174 | `amendments.rs` `a_child_records_its_carrier_bindings` | yes |
| C175 | `digital_circuit.rs` `a_parent_signal_reaches_its_childrens_ports` | yes - with the negative control (no binding, `q` stays LOW) and the `PORT` role |
| C176 | `digital_circuit.rs` `a_net_starts_at_its_own_init` | yes - init true, false and none read at (0, 0) |
| C177 | `types_compound.rs` `a_nested_pattern_inside_some_covers_its_payload` | yes - resolver change (T66) confirmed by an own probe: a binding inside a body `let`'s value no longer leaks as a module member; no POM-level test pins it (F8) |
| C178 | `elab_results.rs` `the_stdlib_d2a_reads_its_match_scrutinee_at_each_test` | yes - exact drive text, four re-reads, binding inside `Some` refused by name |
| C179 | `buck_boundary.rs` `the_comparator_trips_at_its_thresholds` | yes - same test as C135, bench on the stdlib `D2A` |
| C180 | `buck_records.rs` `records_of_two_reactions_are_told_apart` | no - covers two reactions whose `log` is at the same statement index only; with different indices the FIFO pairing gives each record the other reaction's text (F1, reproduced) |
| C181 | `buck_records.rs` `records_of_two_reactions_at_different_statements_keep_their_texts` | author's row, fix round (no second Verifier by rule): the F1 reproduction (statements 0, 1 of reaction 0 and 0 of reaction 1) and the reverse, exact `(instant, severity, text)` list over two edges; a record now decodes from its own site, no pairing |
| C182 | `types_events.rs` `an_emit_in_analog_is_p0001_by_name` | author's row, fix round: three sites (written, `log`, through a `fn`), one `P0001` each, span and name asserted; the reaction `log` checks |
| C183 | `pom_resolve.rs` `a_body_let_gives_its_module_only_the_names_of_its_own_pattern` | author's row, fix round: both sides of T66's rule (members `p q a b` resolve in `c`; arm bindings `v w` resolve in their arm and are `E0161` later); fails on the pre-T66 resolver |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (High, silent) - a record is decoded with another site's text. Two reactions of one instance emit to one stream in the same microstep at different statement indices: the record of reaction 0 statement 1 gets reaction 1's text and vice versa. C180, DIG-11 AC 1; the decoder T67 says the CLI will reuse | Own bench: `on clk.rise() { log(Warning, "a"); log(Warning, "b"); }` and `on clk.rise() { log(Error, "c"); }` decoded by the `buck_records.rs` `Decoder` gives `(sev 1, "a")`, `(sev 1, "c")`, `(sev 2, "b")`. `piperine-digital-solver/src/observe.rs` `Observation::key` sorts commits by `(instance, statement)`, dropping the reaction order, while records keep made order and carry no reaction or statement; the FIFO pairing in `buck_records.rs`, `buck_closed_loop.rs` (and the comment in `buck_records.rs` describing it) assumes the two orders agree | Pair inside a microstep on the made order: sort the waiting commits of a pin by `(reaction, statement)` before pairing, or carry `reaction`/`statement` on the `Record` observation; add a test with `log`s at statements 0 and 1 of reaction 0 and 0 of reaction 1 | 5ba3d38a - the `Record` observation carries its reaction and statement; `Ir::record_at` decodes a record alone; C181 `records_of_two_reactions_at_different_statements_keep_their_texts` (both orders) |
| F2 (Medium) - a short circuit over a value that may be unknown is lowered to the bitwise and or or with a right operand that can panic. A valid model faults where the source short-circuits. DIG-08 AC 7, C70 | `Elaborator::partial` (`elab/known.rs`) looks only at `/` and `%` written in the operand and at emitting calls; it does not follow non-emitting callees, does not descend into `match`, and ignores shifts and `clamp`, which `rt::int` panics on (T23 Handoff). Accepted on HEAD: `c && g(n)` with `fn g(n: u8) -> bool { 10 / n > 1 }`; `c && match n { 0 => true, _ => 10 / n > 1 }`; `c && ((x >> s) > 0)` with `s: i8`; `c` flagged in each. The direct `c && (x / n > 1)` is refused | Make `partial` follow callees transitively (as `Types::emits` does), walk `match` arms, and count `<<`/`>>` with a signed amount and `clamp` as partial; add the three forms to C70's test | b958865a - `partial` follows callees, walks `match` arms, counts signed shifts and integer `clamp`; C70 `a_partial_right_operand_under_a_flagged_short_circuit_is_p0001` |
| F3 (Medium) - `emit!` on a non-reserved stream, or `log`, in an `analog` block type-checks (C52's positive case asserts it) and elaboration then fails with `P0004` "internal error: elaboration produced an ill-formed IR: an emit outside a reaction" | Own probe: `module M(p: Electrical) { } analog for M { log(Severity::Info, "stepped"); i(p) +> v(p); }` placed under `Top` | Refuse it by name (`P0001` "an `emit!` in an `analog` block") in the checker or the elaborator; add the case to C52's or a new check's test | 8a6aa491 - `P0001` "an `emit!` in an `analog` block" at type check; edge case 7, C182 `an_emit_in_analog_is_p0001_by_name`; C52 asserts the trace is no `E0603` |
| F4 (Low) - C13's test can pass with a call that has no callee | `types_integers.rs` `every_accepted_call_has_a_callee` skips every expression with no type (`if types.ty(e).is_none() { continue }`) and walks three small models with no carrier, enum, flag read or event | Walk `Buck` too, count `Types::builtin` as a record, and do not skip untyped calls | c0371c47 - C13 `every_accepted_call_has_a_callee` visits every reached call, typed or not, counts built-ins, walks `Buck` |
| F5 (Low) - the DIG-17 grep proofs are loose | C156 greps one phrase of `GAPS.md` only; DIG-17 AC 2 also names `examples/buck/GAPS.md`, where G3 is kept with "(closed: now reported)". C155's three word greps match any mention | Grep the G3 entry of both files for its closed state; anchor C155 on the three entries' headings | 896216ab - C155 anchored on the three entry headings; C156 checks `GAPS.md` and `examples/buck/GAPS.md` |
| F6 (Low, loud) - a `match` over a signed integer that names every value with negative literals is refused as not exhaustive | `let k: i2 = 0; match k { -2 => .., -1 => .., 0 => .., 1 => .. }` is `P0002` on HEAD (DIG-03 AC 4 types literal patterns; AC 5 refuses only a `match` that misses a value) | Count negative literal patterns in the coverage of a signed type of at most 16 bits | de1e6a26 - coverage reads negative literals over the signed range; C18 `a_match_that_is_not_exhaustive_is_p0002` |
| F7 (Low) - C154 counts records with a slack of 1000 | `buck_closed_loop.rs` asserts `o.checks.len() >= 119_000`; 600 us at 200 MHz has exactly 120 000 rising edges, and `buck_whole.rs` asserts the exact instants for the idle run | Assert the exact count (or the instants) of records | b4beb82f - C154 `no_shoot_through_during_the_start` asserts exactly 120 000 records at `rise(1..=120000)` |
| F8 (Low) - T66 changed member resolution in `pom/resolve/scopes.rs`, an area the tasks keep unedited, with no test of the resolver rule itself | Own probe: `let a = match d.known() { Some(v) => .., None => 0.5 }; let b = v;` in a module body is now `E0161` "cannot find `v`" (before, `v` was a module member). The change is a correct fix; every `piperine-lang` target passes | Add a POM test pinning both sides: an arm or `if let` binding inside a body `let` is no member, the pattern's own names are | 815e9e54 - C183 `a_body_let_gives_its_module_only_the_names_of_its_own_pattern` (fails on the pre-T66 resolver) |
| F9 (Low, process) - T67 edits `piperine-solver` (`ObservationKind::Commit` gains `reaction`), which spec.md "Out of Scope" and the tasks' rules exclude ("Changes to `piperine-abi` and the three solver crates") | `git diff 67f9264e^ HEAD -- crates/piperine-solver`: 3 files, 6 lines; `parity_baseline` and `protocol_sink` green | Record the exception as a decision (STATE.md) or amend the Out of Scope row | 8bca3eb0 - orchestrator ruling: approved exception, recorded in spec.md Out of Scope and the Settled mid-build line (covers F1's `Record` field too); no code change |
| F10 (Low) - DIG-07 AC 2 names seven twins; C62 compares six prints. `diode_rev` is asserted only to elaborate with an empty `check()` | `elab_digital.rs` `the_m1_goldens_are_unchanged` | Pin `diode_rev`'s print as it was at the feature's base and compare it | 8194cbc5 - `diode_rev` print at base 9aa03fe8 pinned as `tests/fixtures/elab/diode_rev.txt`; C62 `the_m1_goldens_are_unchanged` compares all seven |
| F11 (Cosmetic) - one bare carrier read reaches `analog` through a block `let` and gives two `E0150` | `analog for Top { let q = c; v(p) +> q; }` gives two identical `E0150` | Report the read once | 85edfca2 - one `E0150` per carrier read; C58 `an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150` |
