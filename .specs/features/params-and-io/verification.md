# Params and I/O verification (B1 to B4)

**Verdict**: FAIL
**Proofs**: `proofs.md` at a28fdf9b (B1 to B4: every in-scope check passes; the 52 failing rows are the B5/B6 checks, whose tests do not exist yet, and `Run sourcetables`, a target T41 creates)
**Verifier**: independent sub-agent (author != verifier)
**Scope**: batches B1 to B4 (T1 to T28) only. B5 and B6 (T29 to T44: integer, enum and `Option` params, `E0122` for named functions, `parse_table`, `parse_hex`, `table_model`) wait on `composite-values` and are marked "deferred", not judged; `validate_verification.py` cannot pass until they land.

Range judged: `git diff origin/main...HEAD` (f9a29dd0..a28fdf9b, 118 files).

**Contract.** `git diff origin/main -- crates/piperine-parity/src tests/support` touches only `host.rs`
(`HostRun::settings` made `pub`); no `ToleranceContract`, no `contract.rs`, no golden. The new
`tests/e2e/twins/tran_pwl_rc.cir` runs `tran 0.1u 10u 0 10n uic`, the piperine side `max_step 1e-8`:
the same explicit cap on both sides. ngspice was on `PATH` for every parity run (no SKIP).

**Spec-code guard.** Recounted independently: 189 codes are the first cell of an error table under
`docs/spec/`; `E0102` and `E0331` appear in the text only (retired by the value-semantics amendment;
`E0331` is still raised by the parser, allowed). 51 codes are spelled by no non-comment source line,
and they equal the 51 rows of `code_owners.tsv` exactly. Every code this batch claims (`E0180`,
`E0182`, `E0141`, `E0188`, `E0204`, `E0205`, `E0315`, `E0333`, `E0904`, `E0244`, `E0354`, `E0906`,
`E0105`, `E0810`, `E0811`, `E0361`, `E0610`, `E0612`, `E0613`) is spelled in `crates/*/src`; `P0002` is
spelled nowhere.

**Replaced assertions.** Every `P0002` -> `E0180` replacement (types_*, elab_*, cli, book) keeps its
source and position and asserts the new code; none is loosened. Moved codes: `E0441` -> `E0354`
(placement result outside an event position), `P0002` -> `E0315` (value at a node port), `P0001` ->
`E0811` (top value input), `P0002` -> `E0182` (`1.0 as bool`), `P0002` -> `E0141` (count). Two
replacements assert `E0180` where the spec names another code (F1, F2).

**Formatter pass (H12).** Mechanical, verified: HEAD's `piperine fmt` applied to the origin/main copy
of each of the 19 changed files of `std/` and `examples/buck` gives the HEAD file byte for byte, and
the same holds for all 60 changed `rust` fences of Appendices D, E, F and Part VII; outside fences
Part II, IV and VII are unchanged. The buck targets ran unedited in the root suite.

**Own examples (HEAD binary, throwaway, not committed).** `v(p, q, gnd)`: `E0180` "a probe takes 1
or 2 nodes, found 3" (F1). `include_str!` in a param default, a module `let` and a `fn` body: `E0610`
each (holds). A non-exhaustive `match`: `E0180` in `types_compound`, where Part I's `E0184` row names
it (F2). `piperine run top.msl` from the source's own directory with `include_str!("d.txt")`: reads
the file (the CLI resolves the root path; D14's no-directory case is reachable only from an in-memory
source). Module-typed params, E0361 and E0319 through a template, the POM after elaboration: hold.

**Records.** `GAPS.md`, `ROADMAP.md` and `STATE.md` follow B4. `STATE.md`'s `MD-74` is also taken by
`feature/unknown-semantics-events` (commit 9df4b885, "settle and limexp"); that branch also moves
`GAPS.md`'s loud count (to 40) where this one asserts 39, and `docs.rs` here pins `"### MD-74:"` and
`"39 loud"` — renumbering at merge must update `gaps_no_longer_lists_x5_l20_l39_h12` and
`gaps_drops_what_msl_fixes_closed` too.

**Note on step caps (C83).** The DC-step comparison reuses `tran_rc.cir` as it is on main (`tran 0.1u
10u uic`, ngspice's default TMAX) against piperine `max_step 1e-8`, the configuration the existing
`tran_rc` twin already uses; it passes, but it is not the same-cap configuration of C84. Not changed
here (frozen fixture); for the user to rule on.

**Test runs at a28fdf9b (lane B env).** `check_proofs.py`: build and `suite_hygiene` green; every
B1-B4 check passes. Root `env -u PIPERINE_NGSPICE cargo test --no-fail-fast`: exit 0, 301 test
results, 3096 passed, 0 failed, 3 ignored (pre-existing `ignore` doc fences of `piperine-solver`), the
buck targets and `buck_scenarios` included (re-run in full after the power loss). With ngspice on
`PATH`: `source_host` 3/3, `parity_ac` 2/2, `parity_tran` 9/9, no real SKIP (the one `SKIP uic_rc`
line is `parity_tran`'s test of the skip message itself).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `sourceparams` `an_integer_param_runs_with_its_value` | deferred - B5/B6 (T32), not judged |
| C2 | `elabparams` `an_integer_param_is_bound_and_read` | deferred - B5/B6 (T31), not judged |
| C3 | `elabparams` `n_as_f64_reaches_the_contribution` | deferred - B5/B6 (T31), not judged |
| C4 | `apisession` `an_integer_out_of_range_is_e0802` | deferred - B5/B6 (T33), not judged |
| C5 | `clirun` `param_of_an_integer_from_the_command_line_reads_from_the_start` | deferred - B5/B6 (T33), not judged |
| C6 | `bookdigital` `the_debouncer_pwm_and_pwm_check_print_their_records` | deferred - B5/B6 (T35), not judged |
| C7 | `sourceparams` `an_enum_param_runs_its_arm` | deferred - B5/B6 (T32), not judged |
| C8 | `elabparams` `an_enum_param_selects_its_arm` | deferred - B5/B6 (T31), not judged |
| C9 | `apisession` `an_enum_param_set_by_variant_name` | deferred - B5/B6 (T33), not judged |
| C10 | `apisession` `an_unknown_variant_is_e0802_listing_the_variants` | deferred - B5/B6 (T33), not judged |
| C11 | `elabparams` `an_option_param_left_out_is_none` | deferred - B5/B6 (T31), not judged |
| C12 | `elabparams` `an_option_param_bound_to_a_value_is_some` | deferred - B5/B6 (T31), not judged |
| C13 | `elabparams` `option_param_methods_read_the_bound_case` | deferred - B5/B6 (T31), not judged |
| C14 | `apisession` `an_option_param_set_to_a_value_or_none` | deferred - B5/B6 (T33), not judged |
| C15 | `elabinstances` `a_placement_through_a_module_param_places_its_template` | yes |
| C16 | `elabinstances` `a_parent_binding_replaces_the_template` | yes |
| C17 | `typesmodules` `a_param_block_on_a_placement_through_a_param_is_e0361` | yes |
| C18 | `elabinstances` `a_template_leaving_a_required_param_unbound_is_e0319` | yes |
| C19 | `typesmodules` `a_template_of_another_module_is_e0180_naming_both` | yes |
| C20 | `elabinstances` `the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module` | yes |
| C21 | `interp` `a_param_of_another_type_is_refused_naming_the_transpiled_backend` | deferred - B5/B6 (T34), not judged |
| C22 | `typesvalues` `a_mismatch_no_row_names_is_e0180_naming_both_types` | yes - the exact message with both types |
| C23 | `speccodes` `no_source_spells_p0002` | yes - scans every non-comment line of `crates/*/src` |
| C24 | `typesvalues` `a_param_hiding_a_function_is_e0180_with_a_note_at_the_function` | yes |
| C25 | `clibook` `no_book_listing_prints_p0002` | yes |
| C26 | `typesvalues` `an_as_between_types_without_a_row_is_e0182` | yes |
| C27 | `typesvalues` `a_call_through_a_function_value_with_the_wrong_arity_is_e0141` | partly - holds for a function value and a method call; PI-06 AC 6 also names a probe and a builtin, which still give `E0180` (F1) |
| C28 | `elabvalues` `a_const_reading_a_run_value_is_e0188` | yes |
| C29 | `apisources` `a_dc_source_sets_the_port_at_the_operating_point` | yes |
| C30 | `apisources` `a_pwl_source_lands_on_every_corner` | yes - corners 2, 5, 6 us are off the 0.3 us step grid; a missed corner interpolates to ~0.97 V and fails the 1e-6 bound |
| C31 | `sourcehost` `a_host_driven_rc_matches_ac_rc_in_ac` | yes - ngspice ran (no SKIP); `AC_MAGNITUDE`, `AC_PHASE` unchanged |
| C32 | `apisources` `a_timed_source_change_is_a_jump` | partly - rows every 0.5 us and 1.9 to 2.1 us skipped, so a change anywhere in (1.5, 2.5) us passes (F3) |
| C33 | `apisources` `an_undriven_node_port_is_e0810` | yes |
| C34 | `elabentry` `a_top_value_input_is_e0811` | yes |
| C35 | `elabentry` `a_top_value_input_is_e0811` | yes - Q1 is (a); same test as C34 |
| C36 | `spectext` `the_host_drives_a_tops_node_ports_e0810_e0811` | yes |
| C37 | `elabfiles` `include_str_reads_the_file_beside_the_source` | yes |
| C38 | `typesvalues` `include_str_outside_a_const_is_e0610` | partly - one position (an `analog` let) asserted with `contains`; probes at HEAD (param default, module `let`, `fn` body) all give `E0610`, so the rule holds (F4) |
| C39 | `elabfiles` `an_unreadable_file_is_e0612_naming_the_path` | yes |
| C40 | `elabfiles` `a_file_that_is_not_utf8_is_e0613_with_the_offset` | yes |
| C41 | `elabfiles` `a_changed_file_needs_a_new_elaboration` | yes - at the `ConstEval` layer the IR is built from |
| C42 | `elabfiles` `parse_table_reads_64_rows_in_order` | deferred - B5/B6 (T37), not judged |
| C43 | `elabfiles` `parse_hex_reads_words_at_their_addresses` | deferred - B5/B6 (T38), not judged |
| C44 | `elabfiles` `a_table_of_the_wrong_shape_is_e0614_naming_file_and_row` | deferred - B5/B6 (T37), not judged |
| C45 | `elabfiles` `a_token_that_is_not_a_real_is_e0706` | deferred - B5/B6 (T37), not judged |
| C46 | `sourcetables` `linear_table_model_is_the_interpolation_at_op_and_dc_sweep` | deferred - B5/B6 (T41), not judged |
| C47 | `sourcetables` `discrete_control_gives_the_closest_sample` | deferred - B5/B6 (T41), not judged |
| C48 | `sourcetables` `cubic_reproduces_a_cubic_polynomial` | deferred - B5/B6 (T41), not judged |
| C49 | `sourcetables` `extrapolation_c_and_l_at_each_end` | deferred - B5/B6 (T41), not judged |
| C50 | `sourcetables` `extrapolation_e_outside_is_e0705` | deferred - B5/B6 (T41), not judged |
| C51 | `elabfiles` `a_bad_control_string_is_e0703` | deferred - B5/B6 (T39), not judged |
| C52 | `elabfiles` `non_increasing_abscissae_are_e0704` | deferred - B5/B6 (T39), not judged |
| C53 | `sourcetables` `the_table_diode_passes_the_partials_verifier` | deferred - B5/B6 (T42), not judged |
| C54 | `spectext` `appendix_c2_states_the_spline_end_conditions` | yes |
| C55 | `clicommands` `fmt_check_passes_on_every_stdlib_file` | yes |
| C56 | `clicommands` `fmt_check_passes_in_examples_buck` | yes |
| C57 | `stdgalleries` `every_gallery_fence_is_in_the_formatters_form` | yes |
| C58 | `msformat` `formatting_keeps_the_printed_tree_of_every_shipped_source` | no - every shipped file is already a fixed point, so `format(text) == text` and the tree comparison cannot fail; the claim holds by this verification: `piperine fmt` of each origin/main file and fence gives exactly HEAD (19 files, 60 fences) (F5) |
| C59 | `typesvalues` `a_borrow_outside_a_call_argument_is_e0102` | deferred - B5/B6 (T29), not judged |
| C60 | `runerrors` `a_division_by_zero_under_known_control_fails_the_run_with_e0105` | yes |
| C61 | `typesvalues` `a_named_emitting_fn_where_emit_is_not_allowed_is_e0122` | deferred - B5/B6 (T36), not judged |
| C62 | `elabanalog` `a_quantity_attribute_outside_the_three_is_e0204` | yes |
| C63 | `elabanalog` `equal_access_names_are_e0205` | yes |
| C64 | `typesmodules` `an_expression_on_a_carrier_port_that_is_not_in_is_e0315` | yes |
| C65 | `typesmodules` `an_instance_without_a_result_used_as_a_value_is_e0333` | yes |
| C66 | `typesstructure` `an_attribute_a1_does_not_list_is_e0904` | yes |
| C67 | `speccodes` `this_features_codes_are_all_raised` | deferred - B5/B6 (T44), not judged |
| C68 | `typescarriers` `a_probe_on_a_childs_private_node_is_e0244` | yes |
| C69 | `typesresults` `a_continuous_result_read_where_probes_are_not_allowed_is_e0354` | yes |
| C70 | `typesunknowns` `unknown_with_no_inferable_type_is_e0906` | yes |
| C71 | `speccodes` `every_spec_code_is_raised_or_owned` | yes - 189 codes parsed (recounted independently), the 51 owned rows equal the unraised set |
| C72 | `speccodes` `every_owned_code_names_its_owner_and_construct` | yes |
| C73 | `speccodes` `no_owned_code_is_raised` | yes |
| C74 | `speccodes` `this_features_codes_are_all_raised` | deferred - B5/B6 (T44), not judged |
| C75 | `docs` `gaps_no_longer_lists_this_features_ids` | deferred - B5/B6 (T44), not judged |
| C76 | `elabparams` `an_integer_default_converts_with_as` | deferred - B5/B6 (T31), not judged |
| C77 | `typesmodules` `an_option_param_read_as_a_number_is_e0180` | deferred - B5/B6 (T30), not judged |
| C78 | `apisources` `a_node_port_tied_to_gnd_inside_is_still_refused_undriven` | yes |
| C79 | `elabfiles` `one_file_named_twice_is_read_once` | yes |
| C80 | `elabfiles` `an_infinite_row_value_is_e0706` | deferred - B5/B6 (T37), not judged |
| C81 | `elabfiles` `a_one_row_table_under_d_is_e0704` | deferred - B5/B6 (T39), not judged |
| C82 | `elabvalues` `a_const_that_depends_on_itself_is_e0188` | yes |
| C83 | `sourcehost` `a_host_driven_rc_step_matches_tran_rc` | yes - the configuration of the existing `tran_rc` twin (see note on step caps) |
| C84 | `sourcehost` `a_host_pwl_into_the_rc_matches_tran_pwl_rc` | yes - ngspice `tran 0.1u 10u 0 10n uic` and piperine `max_step 1e-8`: one explicit cap on both sides; `TRAN_VOLTAGE` unchanged |
| C85 | `clirun` `run_of_a_top_with_a_node_port_is_e0810` | yes |
| C86 | `elabanalog` `a_non_positive_abstol_is_e0204` | yes |
| C87 | `typesstructure` `a_listed_attribute_on_the_wrong_item_is_e0904` | yes |
| C88 | `typesresults` `an_event_result_outside_an_event_position_is_e0354` | yes |
| C89 | `typesvalues` `a_named_solve_reading_fn_in_a_const_is_e0122` | deferred - B5/B6 (T36), not judged |
| C90 | `runerrors` `an_unwrap_of_none_under_known_control_fails_the_run_with_e0105` | yes |
| C91 | `elabentry` `a_top_node_port_gets_one_host_source_instance` | yes |
| C92 | `elabentry` `the_pom_keeps_a_top_with_node_ports_as_written` | yes |
| C93 | `typesmodules` `a_module_typed_param_takes_a_template` | yes |
| C94 | `apisession` `a_module_typed_param_is_not_set_by_text` | yes |
| C95 | `typesvalues` `include_str_in_a_const_is_a_str` | yes |
| C96 | `msformat` `every_stdlib_file_is_in_the_formatters_form` | yes |
| C97 | `docs` `gaps_no_longer_lists_x5_l20_l39_h12` | yes |
| C98 | `typesmodules` `integer_enum_and_option_params_type_check` | deferred - B5/B6 (T30), not judged |
| C99 | `sourceparams` `an_option_param_runs_its_case` | deferred - B5/B6 (T32), not judged |
| C100 | `apisession` `an_integer_param_set_by_text_in_range` | deferred - B5/B6 (T33), not judged |
| C101 | `bookcorestd` `the_watchdog_builds_with_its_u16_param` | deferred - B5/B6 (T35), not judged |
| C102 | `bookinventory` `every_block_of_the_book_has_one_row` | yes - passes at HEAD (T35 is deferred; the inventory guard holds after B4) |
| C103 | `elabfiles` `x_and_z_digits_are_unknown_levels` | deferred - B5/B6 (T38), not judged |
| C104 | `elabfiles` `an_address_the_text_does_not_write_is_e0614` | deferred - B5/B6 (T38), not judged |
| C105 | `elabfiles` `a_word_that_is_not_hex_is_e0706` | deferred - B5/B6 (T38), not judged |
| C106 | `elabfiles` `too_few_points_for_the_degree_is_e0704` | deferred - B5/B6 (T39), not judged |
| C107 | `irchecks` `a_table_expression_is_built_checked_and_printed` | deferred - B5/B6 (T40), not judged |
| C108 | `interp` `a_table_expression_is_refused_by_name` | deferred - B5/B6 (T40), not judged |
| C109 | `bookpackages` `the_include_str_table_runs` | deferred - B5/B6 (T43), not judged |
| C110 | `bookpackages` `e0610_is_raised_for_include_str_in_a_module` | deferred - B5/B6 (T43), not judged |
| C111 | `bookcheatsheet` `the_files_card_runs` | deferred - B5/B6 (T43), not judged |
| C112 | `docs` `gaps_section_10_lists_exactly_the_owned_codes` | deferred - B5/B6 (T44), not judged |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1. A wrong argument count on a probe or a builtin is `E0180`, not `E0141` (PI-06 AC 6, design line 55 and the table at line 263) | `v(p, q, gnd)` at HEAD: `E0180` "mismatched types: a probe takes 1 or 2 nodes, found 3"; `types_events.rs:521` asserts `E0180` for `cross()` and `cross(v(p), 1p, 1u, 2u)`; C27's test covers only a function value and a method call | route the probe and core-primitive count checks through `Checker::arity_mismatch` (`E0141` with both counts); assert `E0141` in `types_events.rs:521` and add a probe case to `a_call_through_a_function_value_with_the_wrong_arity_is_e0141` | |
| F2. A non-exhaustive `match` is `E0180`, but Part I's `E0184` row names that position (PI-06 AC 1: `E0180` only where no other row names it) | `types_compound.rs:282` `a_match_that_is_not_exhaustive_is_e0180` (renamed from `_is_p0002`); `docs/spec/part_i_values.md:639` | raise `E0184` for exhaustiveness (the construct runs today, the amendment's own rule gives such codes here) and rename the test `_is_e0184`; `E0184`'s `code_owners.tsv` row must then leave (composite-values keeps its other forms) - or have the user rule that it waits for `composite-values` and record it in `GAPS.md` | |
| F3. C32's test does not pin the jump to its time | `host_sources.rs` `a_timed_source_change_is_a_jump`: print step 0.5 us and rows between 1.9 and 2.1 us skipped, so a change anywhere between 1.5 and 2.5 us, or a ramp across that span, passes | print at a step of 1e-8 near 2 us and assert 1 V just before 2 us and 3 V from 2 us on | |
| F4. C38's test covers one position and accepts extra codes | `types_values.rs` `include_str_outside_a_const_is_e0610`: one `analog` let, `codes_of(&d).contains(..)` | assert `["E0610"]` exactly, over an `analog` let, a module `let`, a param default and a `fn` body (all `E0610` at HEAD) | |
| F5. C58's test cannot fail: the shipped files are fixed points of the formatter, so formatting them changes nothing | `msl_format.rs` `formatting_keeps_the_printed_tree_of_every_shipped_source` compares `print(format(text))` with `print(text)` where `format(text) == text` (C96, C55, C56) | compare the printed tree of each shipped file with that of its pre-format text: e.g. a test that formats a committed unformatted copy (or the fences' pre-format text) and asserts both trees equal; the claim itself holds (verified: fmt of origin/main == HEAD) | |
| F6. New code without doc comments, and free functions in new test code (user cleanliness rules) | `elab/host_sources.rs`: `HostSourceExpressions::{emit, binary, param}`, `HostSources::new`, fields `ir`, `origin`, `bodies`; `elab/files.rs`: `IncludedFiles::{texts, reads}`, `FileFault` variant fields; `api/source.rs`: `SegmentSet` fields; tests: free `settings`, `session`, `transient`, `open`, `run`, `piecewise`, `real` (`api/tests/host_sources.rs`, most undocumented), `fixture`, `design_of`, `constant` (`elab_files.rs`), `host_failure` (`run_errors.rs`), `nature_source` (`elab_analog.rs`), `first_param` (`elab_instances.rs`), `repository` (`cli/tests/commands.rs`), `printed_tree` (`msl_format.rs`); undocumented `HostDrivenRc::{session, table}` (`source_host.rs`), `RcTop::design` (`elab_entry.rs`); `HostSourceParam::Mag` abbreviates magnitude | document each; move the test helpers onto owner structs (as `Fixture`, `Scratch`, `HostDrivenRc` already are); name the variant `Magnitude` (the IR param name can stay `mag`) | |
