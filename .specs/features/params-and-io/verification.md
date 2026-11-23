# Params and I/O verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 59ae2602 (the round 7 fix, 114 pass, 0 fail; re-run by the round 8 verifier after its own commit); round 7 at c93b8ef5, round 6 at defc9611, round 5 at b3045722, round 4 at dcbbea67, round 3 at e60177d8, round 2 at acefa2f6, round 1 at 0bed6a92
**Verifier**: independent sub-agent (author != verifier)
**Scope**: round 8 judges the fix of F33 to F35 (b7c1fd4b: a lent `BytesMut` out-buffer in `piperine-abi` `raw.rs` and the new `piperine_derive_2(k, params, out) -> u32`; the interpreter walk's refusal citing PI-05 AC 1; `git diff d65ecfea..59ae2602`, 10 files), the soundness of the ABI change and its callers, and whether F34's justification holds against PI-05 AC 1. Rounds 7 to 1 are kept below. F15 is out of scope (owned by `generics`).

**Contract.** `git diff origin/main -- tests/support/ngspice.rs crates/piperine-parity/src` is empty.

**Tests run (gate policy: targeted only, foreground).** Lane D env: `cargo test -p piperine-abi -p piperine-transpile --no-fail-fast`
(38 results, all green, no warning); `cargo test -p piperine-api --lib` (79 pass, the `Scales` derive under the new signature) and
`--test session --test derived --test bindings` (26, 17, 8 pass); `cargo build -p piperine-cli`; `check_proofs.py params-and-io`
after this commit (`proofs.md`, `source_params` and `source_tables` among the groups). No workspace run.

**Own examples (HEAD binary, `/var/tmp/piperine-tmp-d/v3`).** Unchanged results after the signature change: `w.big=9007199254740993`
1/7 V, `r: match md` after `w.md=Slow` 0.75 V, `rs: rt * 2.0` after `w.rt=500` 0.5 V, `w.rs=None` on the pinless wrapper 0.5 V, and a DC
sweep of a param others derive from (`m.rr` 1k to 3k: 0.5, 0.667, 0.75 V). Hold.

**The ABI change.** `BytesMut<'a>` is `#[repr(C)]`, its fields `pub(crate)`, built in safe code only by `from_slice(&'a mut [u8])`
(an exclusive borrow for `'a`), neither `Clone` nor `Copy` (no aliasing `&mut`), and `as_mut_slice(&mut self)` hands out a slice no
longer than the `&mut self` borrow; the zero-length case returns `&mut []`. Sound within the crate's trust model (abi.md §933: safe
code builds only valid views). Callers: `load.rs`'s `DeriveExport` and `Loaded::derive` (lends a `vec![0; width]`, accepts only
`written == width`), `Compiled::derive`, the `Derive` trait and its three impls (`Compiled`, `Evaluated`, the unit-test `Scales`), the
printer (`print/derive.rs`, `use piperine_abi::raw::BytesMut` in `library.rs`; it writes only `bytes.len() <= out.len()` and answers 0
otherwise) and `tests/derive.rs`: every one follows the new signature (`grep` of `.derive(`, `DeriveExport`, `impl Derive`). A library
built under the previous `piperine_derive_2` cannot be loaded with the new type: the cache key hashes the generated source and the
embedded ABI sources (`AbiSource::FILES`), both changed. The leak of F33 is gone. Two record gaps (F37).

**F34 against PI-05 AC 1.** PI-05 AC 1 reads "IF a design **run on the interpreter backend** has a param of a type other than `f64`
or `bool` THEN the system SHALL refuse it naming the param and the transpiled backend". `Session::describe` is not a run on any backend
(HII-10, "no backend builds it and no analysis runs"), and it walks every binding with `Evaluated` whatever backend the request
names (`session.rs:947`). So the justification holds for a request on the interpreter backend (the design would be refused there
anyway, and the test uses `interp`), but not for one on the Rust backend: there the refusal tells the host to "use --backend rust",
which it already does, and the description cannot show what a run would use for an integer, enum or composite bound from a param
(HII-10, §8.1.23). Loud, not silent; misleading and narrower than the spec (F36).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `sourceparams` `an_integer_param_runs_with_its_value` | yes - the parity bench pins 1/5 and 1/8 V under `DC_VOLTAGE`, so a wrong `n` fails |
| C2 | `elabparams` `an_integer_param_is_bound_and_read` | yes - `Constant::Int(4)` unbound, `Int(7)` bound, `n: 0` is `E0317` |
| C3 | `elabparams` `n_as_f64_reaches_the_contribution` | yes - the `Cast` to `F64` is lowered and the instance holds 7; the value reaching the contribution is pinned by C1 |
| C4 | `apisession` `a_param_flag_is_read_by_the_params_type_before_the_run` | yes (round 3) - `6.5`, `1e1`, `-1`, `256` for `u8` and `1`, `Medium` for `Mode` are each `E0802` naming param and value through the `--param` path; `clirun` repeats the `u8` cases through the binary |
| C5 | `clirun` `param_of_an_integer_from_the_command_line_reads_from_the_start` | yes - `l.n=6`, `l.n=0x6` and `l.n=300`; the real path it rides on is F9 |
| C6 | `bookdigital` `the_debouncer_pwm_and_pwm_check_print_their_records` | yes - the Book records are compared line by line |
| C7 | `sourceparams` `an_enum_param_runs_its_arm` | yes - 0.5 V for `Fast`, 0.25 V for `Slow` |
| C8 | `elabparams` `an_enum_param_selects_its_arm` | yes - the variant index of the default and of the binding |
| C9 | `apisession` `an_enum_param_set_by_variant_name` | yes - bare and `Mode::`-qualified names, shown back as the variant |
| C10 | `apisession` `an_unknown_variant_is_e0802_listing_the_variants` | yes - the message holds the param, the value and both variants |
| C11 | `elabparams` `an_option_param_left_out_is_none` | yes - an aggregate with no variant and no part |
| C12 | `elabparams` `an_option_param_bound_to_a_value_is_some` | yes (round 3) - `{ rs: 10.0 }` and `{ rs: Some(10.0) }` both give the `Some` aggregate holding 10.0; `{ rs: None }` none. A binding over a parent's param is refused (F21) |
| C13 | `sourceparams` `every_read_of_an_option_param_gives_the_bound_case` | yes (round 3) - `match`, `is_some`/`unwrap`, `if let` each run 0.75 V bound and 0.5 V left out under `DC_VOLTAGE` |
| C14 | `apisession` `an_option_param_set_to_a_value_or_none` | yes (round 3) - with C4's test, which opens with `--param Top.l.rs=10` and shows `Some(10)`; a structural `Option` param is F19, F20 |
| C15 | `elabinstances` `a_placement_through_a_module_param_places_its_template` | yes |
| C16 | `elabinstances` `a_parent_binding_replaces_the_template` | yes |
| C17 | `typesmodules` `a_param_block_on_a_placement_through_a_param_is_e0361` | yes |
| C18 | `elabinstances` `a_template_leaving_a_required_param_unbound_is_e0319` | yes |
| C19 | `typesmodules` `a_template_of_another_module_is_e0180_naming_both` | yes |
| C20 | `elabinstances` `the_pom_keeps_the_placement_through_the_param_and_the_ir_the_module` | yes |
| C21 | `interp` `a_param_of_another_type_is_refused_naming_the_transpiled_backend` | yes - the refusal names the param, its type and `--backend rust` |
| C22 | `typesvalues` `a_mismatch_no_row_names_is_e0180_naming_both_types` | yes - the exact message with both types |
| C23 | `speccodes` `no_source_spells_p0002` | yes - scans every non-comment line of `crates/*/src` |
| C24 | `typesvalues` `a_param_hiding_a_function_is_e0180_with_a_note_at_the_function` | yes |
| C25 | `clibook` `no_book_listing_prints_p0002` | yes |
| C26 | `typesvalues` `an_as_between_types_without_a_row_is_e0182` | yes |
| C27 | `typesvalues` `a_call_through_a_function_value_with_the_wrong_arity_is_e0141` | yes - after F1: a function value, a method call and a probe of three nodes are `E0141` |
| C28 | `elabvalues` `a_const_reading_a_run_value_is_e0188` | yes |
| C29 | `apisources` `a_dc_source_sets_the_port_at_the_operating_point` | yes |
| C30 | `apisources` `a_pwl_source_lands_on_every_corner` | yes - corners 2, 5, 6 us are off the 0.3 us step grid; a missed corner interpolates to ~0.97 V and fails the 1e-6 bound |
| C31 | `sourcehost` `a_host_driven_rc_matches_ac_rc_in_ac` | yes - ngspice ran (no SKIP); `AC_MAGNITUDE`, `AC_PHASE` unchanged |
| C32 | `apisources` `a_timed_source_change_is_a_jump` | yes - after F3: 10 ns rows pin 1 V before and 3 V after 2 us |
| C33 | `apisources` `an_undriven_node_port_is_e0810` | yes |
| C34 | `elabentry` `a_top_value_input_is_e0811` | yes |
| C35 | `elabentry` `a_top_value_input_is_e0811` | yes - Q1 is (a); same test as C34 |
| C36 | `spectext` `the_host_drives_a_tops_node_ports_e0810_e0811` | yes |
| C37 | `elabfiles` `include_str_reads_the_file_beside_the_source` | yes |
| C38 | `typesvalues` `include_str_outside_a_const_is_e0610` | yes - after F4: exactly `E0610` over an `analog` let, a module `let`, a param default and a `fn` body |
| C39 | `elabfiles` `an_unreadable_file_is_e0612_naming_the_path` | yes |
| C40 | `elabfiles` `a_file_that_is_not_utf8_is_e0613_with_the_offset` | yes |
| C41 | `elabfiles` `a_changed_file_needs_a_new_elaboration` | yes - at the `ConstEval` layer the IR is built from |
| C42 | `elabfiles` `parse_table_reads_64_rows_in_order` | yes - 64 pairs compared in order, blank lines, comments and `m` suffix |
| C43 | `elabfiles` `parse_hex_reads_words_at_their_addresses` | yes - words, `_`, both comment forms and moving `@address` records, compared whole |
| C44 | `elabfiles` `a_table_of_the_wrong_shape_is_e0614_naming_file_and_row` | yes - a wide row (row 2) and a short text, each exactly `E0614` with the file |
| C45 | `elabfiles` `a_token_that_is_not_a_real_is_e0706` | yes - `abc` and `inf`, file, row and token |
| C46 | `sourcetables` `linear_table_model_is_the_interpolation_at_op_and_dc_sweep` | yes (round 3) - 1e-15 relative over a real `dc` sweep of 29 points from -1 to 6 V (the bench's control is `1`; inside the table it equals `1C`) |
| C47 | `sourcetables` `discrete_control_gives_the_closest_sample` | yes - both sides of every midpoint |
| C48 | `sourcetables` `cubic_reproduces_a_cubic_polynomial` | yes (round 3) - 1e-12 relative at 41 points, and the default `L` extrapolation at both ends |
| C49 | `sourcetables` `extrapolation_c_and_l_at_each_end` | yes (round 3) - `1CL`, `1LC` and the default (no character, `L` at both) at both ends, 1e-15 |
| C50 | `sourcetables` `extrapolation_e_outside_is_e0705` | yes - `Failure::Host` coded `E0705`, the instance and the call with x |
| C51 | `elabfiles` `a_bad_control_string_is_e0703` | yes - five malformed strings, exactly one `E0703` each, and seven valid ones accepted |
| C52 | `elabfiles` `non_increasing_abscissae_are_e0704` | yes - the first offending row is the third |
| C53 | `sourcetables` `the_table_diode_passes_the_partials_verifier` | yes - `verify_partials` must be `Exact` at three biases inside the table |
| C54 | `spectext` `appendix_c2_states_the_spline_end_conditions` | yes |
| C55 | `clicommands` `fmt_check_passes_on_every_stdlib_file` | yes |
| C56 | `clicommands` `fmt_check_passes_in_examples_buck` | yes |
| C57 | `stdgalleries` `every_gallery_fence_is_in_the_formatters_form` | yes |
| C58 | `msformat` `formatting_keeps_the_printed_tree_of_every_shipped_source` | yes - after F5: the pre-format texts of the 21 changed files are formatted, the tree is kept and the output is the shipped file |
| C59 | `typesvalues` `a_borrow_outside_a_call_argument_is_not_e0102` | yes (round 3) - the criterion as superseded by A8: a `&` outside a call argument is refused and never `E0102` |
| C60 | `runerrors` `a_division_by_zero_under_known_control_fails_the_run_with_e0105` | yes |
| C61 | `typesvalues` `a_named_emitting_fn_where_emit_is_not_allowed_is_e0122` | yes - three calls, exactly `["E0122"]` at the call, the message names the function and class |
| C62 | `elabanalog` `a_quantity_attribute_outside_the_three_is_e0204` | yes |
| C63 | `elabanalog` `equal_access_names_are_e0205` | yes |
| C64 | `typesmodules` `an_expression_on_a_carrier_port_that_is_not_in_is_e0315` | yes |
| C65 | `typesmodules` `an_instance_without_a_result_used_as_a_value_is_e0333` | yes |
| C66 | `typesstructure` `an_attribute_a1_does_not_list_is_e0904` | yes |
| C67 | `speccodes` `this_features_codes_are_all_raised` | yes - `E0906` left out by MD-78 (recorded only in the Handoff, F8) |
| C68 | `typescarriers` `a_probe_on_a_childs_private_node_is_e0244` | yes |
| C69 | `typesresults` `a_continuous_result_read_where_probes_are_not_allowed_is_e0354` | yes |
| C70 | `hygiene` `retired_unknown_codes_are_raised_nowhere` | yes (round 4) - fails when any library source spells `"E0906"` outside a comment |
| C71 | `speccodes` `every_spec_code_is_raised_or_owned` | yes - 189 codes parsed (recounted independently), the 51 owned rows equal the unraised set |
| C72 | `speccodes` `every_owned_code_names_its_owner_and_construct` | yes |
| C73 | `speccodes` `no_owned_code_is_raised` | yes |
| C74 | `speccodes` `this_features_codes_are_all_raised` | yes |
| C75 | `docs` `gaps_no_longer_lists_this_features_ids` | yes - L20, L28, L39, L57, X5, H12 absent from the gap tables |
| C76 | `elabparams` `an_integer_default_converts_with_as` | yes - `Int(3)` and `Int(-2)` |
| C77 | `typesmodules` `an_option_param_read_as_a_number_is_e0180` | yes - exactly `E0180` at `0.0`, naming `Option<f64>` |
| C78 | `apisources` `a_node_port_tied_to_gnd_inside_is_still_refused_undriven` | yes |
| C79 | `elabfiles` `one_file_named_twice_is_read_once` | yes |
| C80 | `elabfiles` `an_infinite_row_value_is_e0706` | yes |
| C81 | `elabfiles` `a_one_row_table_under_d_is_e0704` | yes |
| C82 | `elabvalues` `a_const_that_depends_on_itself_is_e0188` | yes |
| C83 | `sourcehost` `a_host_driven_rc_step_matches_tran_rc` | yes - the configuration of the existing `tran_rc` twin (see note on step caps) |
| C84 | `sourcehost` `a_host_pwl_into_the_rc_matches_tran_pwl_rc` | yes - ngspice `tran 0.1u 10u 0 10n uic` and piperine `max_step 1e-8`: one explicit cap on both sides; `TRAN_VOLTAGE` unchanged |
| C85 | `clirun` `run_of_a_top_with_a_node_port_is_e0810` | yes |
| C86 | `elabanalog` `a_non_positive_abstol_is_e0204` | yes |
| C87 | `typesstructure` `a_listed_attribute_on_the_wrong_item_is_e0904` | yes |
| C88 | `typesresults` `an_event_result_outside_an_event_position_is_e0354` | yes |
| C89 | `typesvalues` `a_named_solve_reading_fn_in_a_const_is_e0122` | yes - direct and through another function, exactly `E0122` |
| C90 | `runerrors` `an_unwrap_of_none_under_known_control_fails_the_run_with_e0105` | yes |
| C91 | `elabentry` `a_top_node_port_gets_one_host_source_instance` | yes |
| C92 | `elabentry` `the_pom_keeps_a_top_with_node_ports_as_written` | yes |
| C93 | `typesmodules` `a_module_typed_param_takes_a_template` | yes |
| C94 | `apisession` `a_module_typed_param_is_not_set_by_text` | yes |
| C95 | `typesvalues` `include_str_in_a_const_is_a_str` | yes |
| C96 | `msformat` `every_stdlib_file_is_in_the_formatters_form` | yes |
| C97 | `docs` `gaps_no_longer_lists_x5_l20_l39_h12` | yes |
| C98 | `typesmodules` `integer_enum_and_option_params_type_check` | yes - the type check passes with every new type bound |
| C99 | `sourceparams` `an_option_param_runs_its_case` | yes - left out, `None` and `Some(3k)` at 1/2, 1/2 and 3/4 V; only `unwrap_or` runs (F12) |
| C100 | `apisession` `an_integer_param_set_by_text_in_range` | yes - five literals in radix and range read back as `Cell::Int` |
| C101 | `bookcorestd` `the_watchdog_builds_with_its_u16_param` | yes - the Book's own text builds and runs with `limit: u16` |
| C102 | `bookinventory` `every_block_of_the_book_has_one_row` | yes - passes at HEAD (T35 is deferred; the inventory guard holds after B4) |
| C103 | `elabfiles` `x_and_z_digits_are_unknown_levels` | yes - `0x`, `zz`, `1X` are `E0706` naming file, row and word (the criterion as amended by MD-78, F8) |
| C104 | `elabfiles` `an_address_the_text_does_not_write_is_e0614` | yes - a gap at address 2 and a word past the array, each `E0614` |
| C105 | `elabfiles` `a_word_that_is_not_hex_is_e0706` | yes - `g1` and a word that does not fit `u8` |
| C106 | `elabfiles` `too_few_points_for_the_degree_is_e0704` | yes - 2 points for `2`, 3 for `3` |
| C107 | `irchecks` `a_table_expression_is_built_checked_and_printed` | yes |
| C108 | `interp` `a_table_expression_is_refused_by_name` | yes |
| C109 | `bookpackages` `the_include_str_table_runs` | yes - the listing builds from its files and solves; the assertion reads the source node, not the table value |
| C110 | `bookpackages` `e0610_is_raised_for_include_str_in_a_module` | yes |
| C111 | `bookcheatsheet` `the_files_card_runs` | yes - the card without its statement runs |
| C112 | `docs` `gaps_section_10_lists_exactly_the_owned_codes` | yes - section 10 equals `code_owners.tsv` exactly |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1. A wrong argument count on a probe or a builtin is `E0180`, not `E0141` (PI-06 AC 6, design line 55 and the table at line 263) | `v(p, q, gnd)` at HEAD: `E0180` "mismatched types: a probe takes 1 or 2 nodes, found 3"; `types_events.rs:521` asserts `E0180` for `cross()` and `cross(v(p), 1p, 1u, 2u)`; C27's test covers only a function value and a method call | route the probe and core-primitive count checks through `Checker::arity_mismatch` (`E0141` with both counts); assert `E0141` in `types_events.rs:521` and add a probe case to `a_call_through_a_function_value_with_the_wrong_arity_is_e0141` | 731dab5c; C27 (`a_call_through_a_function_value_with_the_wrong_arity_is_e0141`, probe case added) and `types_events.rs` `cross_takes_ttol_and_etol_by_position_and_by_name` (now `E0141`) |
| F2. A non-exhaustive `match` is `E0180`, but Part I's `E0184` row names that position (PI-06 AC 1: `E0180` only where no other row names it) | `types_compound.rs:282` `a_match_that_is_not_exhaustive_is_e0180` (renamed from `_is_p0002`); `docs/spec/part_i_values.md:639` | raise `E0184` for exhaustiveness (the construct runs today, the amendment's own rule gives such codes here) and rename the test `_is_e0184`; `E0184`'s `code_owners.tsv` row must then leave (composite-values keeps its other forms) - or have the user rule that it waits for `composite-values` and record it in `GAPS.md` | baf51375 (composite-values, B3 T15); closed by deferral: `E0184` for a non-exhaustive `match` is implemented on feature/composite-values (its B3, T15) and lands with that feature; its `code_owners.tsv` row stays; `types_compound.rs:282` stays as it is until then |
| F3. C32's test does not pin the jump to its time | `host_sources.rs` `a_timed_source_change_is_a_jump`: print step 0.5 us and rows between 1.9 and 2.1 us skipped, so a change anywhere between 1.5 and 2.5 us, or a ramp across that span, passes | print at a step of 1e-8 near 2 us and assert 1 V just before 2 us and 3 V from 2 us on | 03472efa; C32 (`a_timed_source_change_is_a_jump`, 10 ns rows either side of 2 us) |
| F4. C38's test covers one position and accepts extra codes | `types_values.rs` `include_str_outside_a_const_is_e0610`: one `analog` let, `codes_of(&d).contains(..)` | assert `["E0610"]` exactly, over an `analog` let, a module `let`, a param default and a `fn` body (all `E0610` at HEAD) | abd6fd6b; C38 (`include_str_outside_a_const_is_e0610`, exactly `["E0610"]` over four positions) |
| F5. C58's test cannot fail: the shipped files are fixed points of the formatter, so formatting them changes nothing | `msl_format.rs` `formatting_keeps_the_printed_tree_of_every_shipped_source` compares `print(format(text))` with `print(text)` where `format(text) == text` (C96, C55, C56) | compare the printed tree of each shipped file with that of its pre-format text: e.g. a test that formats a committed unformatted copy (or the fences' pre-format text) and asserts both trees equal; the claim itself holds (verified: fmt of origin/main == HEAD) | c49ea25a; C58 (`formatting_keeps_the_printed_tree_of_every_shipped_source`, over the pre-format texts in `tests/fixtures/preformat/`; a renamed identifier in a fixture makes it fail) |
| F6. New code without doc comments, and free functions in new test code (user cleanliness rules) | `elab/host_sources.rs`: `HostSourceExpressions::{emit, binary, param}`, `HostSources::new`, fields `ir`, `origin`, `bodies`; `elab/files.rs`: `IncludedFiles::{texts, reads}`, `FileFault` variant fields; `api/source.rs`: `SegmentSet` fields; tests: free `settings`, `session`, `transient`, `open`, `run`, `piecewise`, `real` (`api/tests/host_sources.rs`, most undocumented), `fixture`, `design_of`, `constant` (`elab_files.rs`), `host_failure` (`run_errors.rs`), `nature_source` (`elab_analog.rs`), `first_param` (`elab_instances.rs`), `repository` (`cli/tests/commands.rs`), `printed_tree` (`msl_format.rs`); undocumented `HostDrivenRc::{session, table}` (`source_host.rs`), `RcTop::design` (`elab_entry.rs`); `HostSourceParam::Mag` abbreviates magnitude | document each; move the test helpers onto owner structs (as `Fixture`, `Scratch`, `HostDrivenRc` already are); name the variant `Magnitude` (the IR param name can stay `mag`) | c8416da4; C55 to C57, C96 and the targets that held the moved helpers |
| F7. (Blocker, gate) C59 and C70 prove retired behaviour with tests that no longer exist, so `check_proofs.py` is red (112 pass, 3 fail) and `validate_verification.py` cannot pass | `tasks.md:1376` names `a_borrow_outside_a_call_argument_is_e0102` (renamed `_is_not_e0102` in B5, `types_values.rs`); `tasks.md:1409` names target `types_unknowns`, deleted on main with `E0906` (MD-78): `cargo test -p piperine-lang --test types_unknowns` exits 101 | append criteria for the retirements (F8) and point C59 at `a_borrow_outside_a_call_argument_is_not_e0102` and C70 at a test that proves `E0906` is raised nowhere (e.g. `spec_codes`/`msl_errors`), then rerun `check_proofs.py` || 6b11cc8b; C59 (`a_borrow_outside_a_call_argument_is_not_e0102`) and C70 point at existing tests, `proofs.md` 114 pass (C70's test is loose: F24) |
| F8. (Major, records) The B5/B6 departures from approved criteria are recorded only in the tasks Handoff, not in `spec.md` | `spec.md:398` Amendments stop at A6 (2026-11-20); PI-09 AC 2 still says `x`/`z` make "four levels unknown", PI-12 AC 1 that `E0102` is raised, AC 12 that `E0906` is raised, PI-10 AC 1 writes `&IV` | add A7 to A10 (each citing §22.6.4, MD-78, Appendix A/G, §22.2.15) or append superseding criteria with new IDs; never edit the approved text silently || 6b11cc8b; spec.md A7 to A11 with PI-09 AC 2, PI-10 AC 1, PI-12 AC 1 and AC 12 marked (PI-02 AC 1 for A10 unmarked: F24) |
| F9. (Major) `--param` with a number bypasses the type's reading: a non-integer, an exponent, a negative value for an unsigned param and an enum tag are accepted, silently truncated or wrapped (D24, PI-01 AC 4, PI-02 AC 4: `E0802`) | `cli.rs:106` sends any text that parses as `f64` to `params` (the real path), which writes `ParamValue::Int(v as i64)` (`params.rs:106`); at HEAD `--param l.n=6.5` runs n = 6 (v(a) = 1/7), `l.n=1e1` runs 10, `l.n=-1` runs 255 (v(a) = 3.906e-3), `l.md=1` runs `Slow`; C4's test only calls `Session::set_text` | read every `--param` of an integer, enum or `Option` param through `ParamText` (or refuse a real that is not an integer in range in `Session::set_param`); add the four cases to C4/C5 through the CLI || 27e04297; C4 (`a_param_flag_is_read_by_the_params_type_before_the_run`), C5 (`clirun` `300`, `6.5`, `1e1`, `-1`); the change drops the structural refusal of a text (F19) |
| F10. (Major) `--param` of an `Option<T>` param to a value of `T` is refused (PI-03 AC 4: it is `Some` of it) | `--param l.rs=10` at HEAD: `E0802: l.rs is a composite param: name one of its leaves` (`session.rs:179`, the real path); `l.rs=Some(10)` works only because it is text | route numbers for `Option` params through `ParamText::option`; add `--param l.rs=10` to a CLI test || 27e04297; C4/C14 (`Top.l.rs=10` shows `Some(10)`) |
| F11. (Major) A placement binding `{ rs: 10.0 }` of an `Option<f64>` param is `E0180`; PI-03 AC 2 says it reads `Some(10.0)` (§3.2.3) | own example `Load { rs: 10.0 }(a)`: `E0180 mismatched types: expected Option<f64>, found f64`; `elab_params.rs:289` quotes the criterion and binds `Some(10.0)` (`:291`) | apply §3.2.3's `T` to `Option<T>` to a param binding, or record an amendment the user approves (the spec text says "an argument"); make C12 bind `10.0` || 3c85d77f; C12 (`{ rs: 10.0 }` is `Some(10.0)`); a binding over a parent's param is a new unrecorded `P0001` (F21) |
| F12. (Minor) C13's test does not hold "the value of the bound case" | `elab_params.rs:304`: `halves >= 2` and a `Param(0)` exist; no value of `unwrap_or`, `is_some`, `match` or `if let` is asserted, and the parity row runs `unwrap_or` only | a parity row per form with `Some` and `None` (own example: 0.1667 V and 0.5 V) || 22f74aee; C13 (`every_read_of_an_option_param_gives_the_bound_case`) |
| F13. (Minor) C46 and C48 assert looser bounds than their criteria; the default extrapolation is untested | `source_tables.rs:62` 1e-12 rel + 1e-12 abs (AC 1: 1e-15 relative) over repeated operating points (AC 1: a DC sweep); `:84` 1e-10 (AC 3: 1e-12); no test outside the table with no extrapolation character (rule 3, AC 4 "none is `L` at both") | tighten to the criteria's bounds, run a DC sweep analysis, add the default-extrapolation case || 22f74aee; C46, C48, C49 at 1e-15/1e-12 over a `dc` sweep and the default extrapolation |
| F14. (Minor) `parse_table` accepts an exponent with an SI suffix | `tables.rs:69` `real`: `1e-3k` folds to 1.0 (own example); Appendix A.2 rule 4: "`1e3k` ... are errors", and §22.6.4's resolution reads "the language's real literals" | refuse an exponent before an SI suffix with `E0706`; add the case to C45 || 3c85d77f; C45 (`exp.tbl` `1e-3k` is `E0706`) |
| F15. (Major, closure rule) A refused Book row still names this feature as its owner | `GAPS.md:411` `modules~1faf8c12` (L29, `m` and `dtemp`): "do not run until `params-and-io` implements them"; L29 is not in this spec's gaps and the feature closes | give the row (and L29) a real owning feature that runs before spec-closure, or implement it here || 6b11cc8b; the row moves to `generics` GN-26 AC 3 (user ruling); no `generics` task covers it yet (see the F15 note) |
| F16. (Minor, records) Stale mentions of this feature in `GAPS.md` | `GAPS.md:142` L40 and `:360` say the section 18 `Diode` needs `params-and-io` (it type checks, `elab_structure.rs:376`, and runs: own example 0.618 V); `:184` names `params-and-io` the owner of the NaN-param bit point, which this feature did not take; `:33` lists closed loud rows without L28 and L57 | rewrite L40 to its `generics` part, give `:184` an owner that will do it, add L28 and L57 to `:33` || 6b11cc8b, 22f74aee; L40 rewritten, the NaN row closed by `piperine-plan` `nan_param` (and run end to end, own example), L28 and L57 in the closed list |
| F17. (Minor) Integer text above 2^53 is rounded | `param_text.rs:96` `Ok(value as f64)`: `ParamText::Number(f64)` cannot hold `u64`/`i64` 9007199254740993, and `params.rs:106` `v as i64` saturates a `u64` above `i64::MAX`; D24 "an integer literal ... in range is the value" | carry an integer as `i128` (`ParamValue::Int`) end to end, or refuse a value an `f64` cannot hold || 27e04297; `apisession` `an_integer_above_2_pow_53_is_set_exactly`, `param_text` unit `an_integer_above_2_pow_53_is_read_exactly` (`Session::describe` still rounds: F23) |
| F18. (Minor, cleanliness) New code breaks the user's rules (recurrence of F6) | free functions in tests: `elab_files.rs` (`parsed_source`, `folded`, `refused`, `real`, `int`, `table_model_source`, `refusals`; `real` and `int` undocumented), `elab_params.rs` (`child_with`, `aggregate`, `placed_params` undocumented), `piperine-api/tests/session.rs` (`typed_session`, `shown`, `literal`), `source_tables.rs` (`project`, `line`, `holds`), `piperine-ir/tests/check.rs:511` (`table_ir`), `param_text.rs` unit tests (`design`, `read` undocumented); undocumented fields `Reading::{ir, label, text}` (`param_text.rs:24`) and the tuple fields of `ControlText`, `Samples` (`table_model.rs`); abbreviations `hl`, `hp` (`rt/table.rs:120`) | move the helpers onto owner structs, document them, name `hl`/`hp` || 3c85d77f, 22f74aee; helpers on `Placed`, `Parsed`, `Typed`, `Maps`, `Texts`; `Reading`, `ControlText`, `Samples` documented; `last_width`/`before_width` (a misplaced doc comment remains: F22) |
| F19. (Blocker, silent) A `--param` of a structural param whose value is no number (a unit enum's variant, `Some(..)`, `None`) is silently ignored: the run uses the default's variant. Regression of 27e04297 | own example `s.msl`: `Sw { md: Mode = Mode::Short }` with `match md` selecting `v(a, b) +> 0.25` or an open branch: `--param m.md=Open` gives v(a) = 0.25 V (wanted 1 V); `Ow { vs: Option<f64> }` with `match vs`: `--param o.vs=Some(0.5)` gives v(c) = 1 V (wanted 0.5 V). `ParamText::elaboration_value` (`param_text.rs`) gives no number for these texts, so `Session::elaborate` binds nothing, and `Session::set_target_text` (`session.rs`) only reads a structural text; before 27e04297 the texts loop raised `refuse_structural` (`E0803`) | give the elaborator the type's reading of every structural `--param` (variant tag, `Option` presence and payload) so the variant is selected, or refuse loud as before; a test per form (`m.md=Open`, `o.vs=Some(0.5)`, `o.vs=None`) through `--param` asserting the selected variant's value | 141fd5da; `apisession` `a_structural_params_text_selects_its_variant` (`m.md=Open` 1 V, `o.vs=Some(0.5)`, `0.5` 0.5 V, `None` 1 V) and `a_structural_params_bad_text_is_e0802`; own examples hold |
| F20. (Major) `--param` of a structural `Option<f64>` param to a plain value is an internal error (PI-03 AC 4: it is `Some` of it) | own example: `--param o.vs=0.5` is `P0004` "elaboration produced an ill-formed IR: param vs is bound to 0.5, not a value of Option<f64>"; `elaboration_value("0.5")` is 0.5 and goes into `Overrides` as a bare number for an `Option` param | bind `Some(0.5)` (with F19's reading), test it with F19's | 141fd5da; `apisession` `a_structural_params_text_selects_its_variant` (`o.vs=0.5` is 0.5 V) |
| F21. (Minor, records) A plain value over a parent's param bound to an `Option` param is a new `P0001` with no `GAPS.md` row and no test; the `Some(..)` spelling of it is `P0001` too | `elab/children.rs:300` "a value over the parent's params bound to an `Option` param without `Some`: write `Some(..)`"; own example `Mid { rt: f64 = 3k; let l = Load(p) { rs: rt }; }`, and `{ rs: Some(rt) }` is `P0001` "an expression the type checker gave no value type"; GAPS rule: a loud refusal is a row with an owner | implement it, or add a `GAPS.md` row with an owner and a test pinning both refusals | 141fd5da; `elabparams` `an_option_param_bound_over_the_parents_param_reads_some_of_it` (`{ rs: rt }`, `Some(rt)`, `rt * 2.0`); run by hand 0.75 V and 0.5 V (an `Option`-to-`Option` pass-through is F25) |
| F22. (Minor, cleanliness) `Params::changes` lost its doc comment: its text now heads `refuse_unfit`'s | `crates/piperine-api/src/params.rs:328-333`: the `changes` paragraph ("The params a `set` ... what a timed `set` schedules") is the first half of `refuse_unfit`'s doc, and `changes` at `:347` has none | move the paragraph back above `changes` | 61608529; `Params::changes` has its doc again, `refuse_unfit` its own |
| F23. (Minor) `Session::describe` of a param text above 2^53 shows 0 | `session.rs` describe loop: `scalar.exact_real().unwrap_or(f64::NAN)` is written to the table and `keep_exact` is not called, so `Metadata::of` shows `Cell::Int(NaN as i128)` = 0 (by reading; `piperine info` takes no `--param`, the API path does) | keep the exact integer in `describe` as `set_exact_integer` does, and extend `an_integer_above_2_pow_53_is_set_exactly` to `describe` | 61608529; `apisession` `an_integer_above_2_pow_53_is_set_exactly` extended to `Session::describe` |
| F24. (Minor, records) C70's test does not hold its claim; a stale `Run` line; A10 and A11 marks | C70 (`E0906` is retired) proves through `every_spec_code_is_raised_or_owned`, which no longer sees `E0906`; `suite_hygiene` `retired_unknown_codes_are_raised_nowhere` is the test that fails if a source raises it. `tasks.md:20` `**Run typesunknowns**` names the deleted target (`validate_tasks.py` warns). A10 changes PI-02 AC 1 ("of a unit enum"), which carries no mark; A11 is dated "B5" but is the fix round | point C70 at `suite_hygiene` (a `Run` line for it), drop `Run typesunknowns`, mark PI-02 AC 1 with A10, date A11 to the fix round | a67784ad; C70 proves through `hygiene` `retired_unknown_codes_are_raised_nowhere`, `Run typesunknowns` replaced by `Run hygiene`, PI-02 AC 1 marked A10, A11 dated to the fix rounds (A8's own text still names the old C70 proof: F25) |
| F25. (Minor, records) Loud refusals of the new param types through a wrapper's placement are not `GAPS.md` rows; A8 names the old C70 proof | own examples: `{ n: n }` and `{ md: md }` (`u8`, unit enum) are "a derived param that reads the integer param .. has no Rust lowering yet" (`print/derive.rs:79`); `{ rs: rs }` (`Option<f64>` to `Option<f64>`) is `P0001` "an expression the type checker gave no value type"; a structural enum pass-through is `P0001` "a composite param in a const expression in the value of a param that fixes structure"; `grep` of `GAPS.md` finds none. `spec.md` A8 still says C70 is `speccodes::every_spec_code_is_raised_or_owned` | a `GAPS.md` row per refusal with an owner (or implement them), and A8's text to `hygiene` `retired_unknown_codes_are_raised_nowhere` | bf92fd5a, 087b0df5, f1df7e30; `apisession` `params_passed_through_a_wrappers_placement_run` and `a_structural_enum_passed_through_a_wrapper_selects_the_variant`; own examples hold; A8 names the `hygiene` proof. What stays refused is F27 and F29 |
| F26. (Major) A unit enum with no `#[derive(..)]` as a param is a rustc refusal of the generated source, "a transpiler defect": b2e4efd9's rule (a param's type needs no `Repr`, §8.1.15) covers the aggregates a param holds, not a unit enum (PI-02 AC 1, A10) | own example `nr3.msl`: `enum Mode { Fast, Slow }`, `module Load(p) { md: Mode = Mode::Fast; }`, `analog` `match md`: `error[E0277]: the trait bound Mode: Carried is not satisfied` at `<Mode as rt::Carried>::from_pin`; with `#[derive(Repr, PartialEq)]` it runs. `TypeItems::pinned` is `lifted` (`collect_aggregates`), which a unit enum is not | give every type a param pin holds its `Carried` form (unit enums included), and a parity row with a derive-less unit enum param read by `match` next to `HeldRatingBench` | bf92fd5a; `sourceparams` `a_unit_enum_with_no_derive_is_a_param` (`GearLoad`, 0.5 and 0.25 V) |
| F27. (Major, records) A `set` of a param that a placement binds into a composite or `Option` param of a child is refused, against §8.1.23 ("every value bound from it is recomputed") and PI-03 AC 4, and nothing records the departure | `Wrap { rs: Option<f64>; let l = Load(p) { rs: rs }; }`: `--param w.rs=500` / `None` is `E0802`; `params.rs:407` "is read by the placement of the composite param .., which keeps the value it elaborated to"; `params_passed_through_a_wrappers_placement_run` asserts the refusal; no `GAPS.md` row, amendment or owner. On a wrapper with no pin the `--param` refusal is the generic "no PARAM pin `rs`" | recompute the bound composite (its bytes from the parent's value) or record the departure: a `GAPS.md` row with a real owner and an amendment of PI-03 AC 4's scope, and give the `--param` path the placement message | dbd3954e, 69791646 (A13); `apisession` `params_passed_through_a_wrappers_placement_run` (`w.rs` to `3000` and `None`, `rs2: rt` follows `rt`), own examples hold; what stays refused is F30, F31 |
| F28. (Blocker, silent) An integer above 2^53 set by a host skips the derive plan: what a placement binds from it keeps its old value, and a derived param takes the write that its plan refuses | own example `bg.msl` (`Wrap { big: u64; let l = Load(p) { big: big }; }`, both reading `big > 2^53`): `--param w.big=9007199254740993` runs the wrapper at 3 mS and the child at 1 mS (0.2 V, wanted 1/7 V); `--param w.l.big=9007199254740993` is accepted while `w.l.big=5` is `E0802` "derived ... set that param instead". `Session::set_exact_integer` (`session.rs`) writes the pin and `table.apply` with `derived: false`, never `Params::changes`; reachable since bf92fd5a lowers derives that read an integer | route the exact write through `Params::changes` (its refusals and readers), and either carry the exact integer to the derive function or refuse a value above 2^53 on a param others derive from; a test with a wrapper passing a `u64` down | dbd3954e; `apisession` `params_passed_through_a_wrappers_placement_run` (`Top.w.big` and `Top.w.i.big` above 2^53 are `E0802`, `9007199254740` follows); own example: loud, no stale child. The refusal itself is F31 item 1 |
| F29. (Minor, records) A derived enum value beyond pass-through is refused with no `GAPS.md` row, by an arena name | `{ md: if slow { Mode::Slow } else { Mode::Fast } }` and `{ md: match k { .. } }` in a placement: "`a derived param of type t0` has no Rust lowering yet" (`print/derive.rs`, `binding.ty` printed raw); §8.1.19 allows any expression over params as a bound value | a `GAPS.md` row with a real owner (or lower it: the tail can return the variant's tag), and print the type by its name (`Mode`) | dbd3954e; `apisession` `params_passed_through_a_wrappers_placement_run` (`md2` by `if`, `md3` by `match`); types printed by their written names. An enum read beyond pass-through is F32 |
| F30. (Major) A `set` of a composite param of an instance with no pin is refused when it changes nothing in the children bound from it | `pt.msl` (`Wrap { rs: Option<f64>; let l = Load(p) { rs: rs }; }`, no `analog` for `Wrap`): `--param w.rs=None` is `E0802` "no PARAM pin `rs`" (the child already holds `None`); the same for any leaf write equal to the child's bytes. `Session::write_leaf_change`: `written = pinned \|\| followers.is_empty()`, so an empty follower list sends the write to a missing pin | write to the pin only when one exists; refuse only a pinless param that no placement reads (the earlier rule), and test `None` on a wrapper whose child holds `None` | b113b52d; `apisession` `params_passed_through_a_wrappers_placement_run` (`Top.w.rs` to `None`); own example `w.rs=None` 0.5 V |
| F31. (Major, records) Two loud refusals of what §8.1.23 allows have no `GAPS.md` row and owner, and A13 misstates one | (1) an integer above 2^53 on a param a placement reads is `E0802` (`Params::refuse_exact`); (2) a `set` of a param read by a composite bound from an expression (`{ rs: rt * 2.0 }`, one param) is `E0802` "which this design computes from more than the param", while A13 says only a composite computed from more than one param stays refused; `git diff eef03cc1..defc9611 -- GAPS.md` is empty | implement (2) for an `Option` that is `Some` of the derive plan's `f64` value, or give both a `GAPS.md` row with a real owner; correct A13 and the refusal's message | b113b52d (refusals removed, A13 deleted); `apisession` `params_passed_through_a_wrappers_placement_run` (above 2^53 follows, `rs2: rt`); own examples `rt * 2.0`, `a + b`, `w.big=..993` 1/7 V |
| F32. (Major) A placement value that reads an enum param beyond passing it on refuses the whole design on the Rust backend | `ex_WrapR.msl`: `module WrapR(p) { md: Mode = Mode::Fast; let l = Res(p, gnd) { r: match md { Mode::Fast => 1k, Mode::Slow => 3k } }; }` fails the default run: "`a derived param that reads the param `md` of type Mode` has no Rust lowering yet" (`print/derive.rs`); §8.1.19 allows any expression over params; no amendment and no `GAPS.md` row | read an enum param in the derive function from its tag (the pin and the table hold it), with a parity row for `r` by `match md` before and after `--param w.md=Slow`; or a `GAPS.md` row with a real owner | b113b52d; own example `r: match md` 0.5 V and 0.75 V after `w.md=Slow`; `apisession` `params_passed_through_a_wrappers_placement_run` |
| F33. (Minor) Every call of the derive export leaks its answer | `print/derive.rs`: `List::from_slice(Box::leak(whole.into_boxed_slice()))` on each call, the host copying it out (`load.rs`); called once per derived param on each host `set`, DC sweep point and timed `set`, so a sweep of a param others derive from leaks per point; the module doc's "a recompute is a rare host `set`" does not hold for sweeps | let the host own the buffer: pass an output slice of the child's pin width (`PinLayout`) to `piperine_derive_2` and have it write and answer a status | b7c1fd4b; `piperine_derive_2(k, params, out: BytesMut) -> u32` writes a host-lent buffer, nothing is allocated for the host; `transpile` `derive.rs` (`compiled.derive(0, &bytes(2.0), 8)`), own DC sweep |
| F34. (Minor) The interpreter walk's missing derive is reported as a compiled design that does not match its plan | `Evaluated::derive` (`params.rs`) is `None` for a binding over an integer, enum or composite param; `Params::changes_from` maps `None` to `Failure::Backend` "the derive function .. gave no value (the compiled design does not match its plan)", reached by `Session::describe` with a host value | refuse by name on that path ("the description recomputes `f64` and `bool` bindings only" naming the binding), as the interpreter backend already does, with a test of `describe` over an enum-reading binding | b7c1fd4b; `apisession` `describing_a_set_the_interpreter_cannot_derive_names_the_interpreter_not_the_plan` (`E0802`, `--backend rust`, no "does not match"); for a Rust-backend request see F36 |
| F35. (Minor, cleanliness) A dead parameter and a message patched by text replacement | `Session::set_exact_integer(.., _ir: &Ir, ..)` keeps an unused parameter; `Session::write_leaf_change` rewrites a refusal with `message.replacen("refuses the value NaN", ..)`, a composite change that carries `NaN` as its value so the message the write builds is wrong first | drop `_ir`; give `Changed` the text to show (or build the composite refusal with it) instead of patching the message | b7c1fd4b; `_ir` dropped from `set_exact_integer`; `Changed::text`/`shown()` carries the host's text, the `replacen` patch is gone |
| F36. (Minor) `Session::describe` of a Rust-backend request refuses a value bound from an integer, enum or composite param and tells the host to "use --backend rust" | `session.rs:947` builds `Evaluated` for every request; `Evaluated::derive` refuses such a binding ("which the interpreter does not evaluate ...: use --backend rust"); PI-05 AC 1 governs a run on the interpreter backend only, and HII-10 says the description shows "the values a run uses"; the F34 test describes an `interp` request only | evaluate those bindings for the description (the elaborator's const evaluator folds every param type), or refuse naming the real limit of the description (and a `GAPS.md` row with an owner); test a `rust` request | |
| F37. (Minor, records) The new ABI view and the changed derive contract are not written down | `BytesMut<'a>` joins `piperine-abi` `raw.rs` but `.specs/features/device-abi/abi.md` (§933 lists `Str`, `Bytes`, `List`, `Module` as the views with private fields) does not name it; the export kept the symbol `piperine_derive_2` across a signature change although `print/derive.rs` says "the suffix is the version of this contract" (no build is affected: the cache key changes, and the branch is unmerged) | add `BytesMut` to abi.md beside `Bytes`; record the derive contract's current signature where MD-69 names it, or bump the suffix | |

## Round 7 record (fix of F30 to F32)

**Round 7 verdict**: FAIL
**Round 7 proofs**: `proofs.md` at c93b8ef5 (the round 6 fix, 114 pass, 0 fail; re-run by the round 7 verifier after its own commit); round 6 at defc9611, round 5 at b3045722, round 4 at dcbbea67, round 3 at e60177d8, round 2 at acefa2f6, round 1 at 0bed6a92
**Round 7 scope**: round 7 judges the fix of F30 to F32 (b113b52d: typed derive `piperine_derive_2(k, bytes) -> List<u8>`, A13 deleted, refusals removed; `git diff dc527c11..c93b8ef5`, 17 files across api, transpile, solver introspect), and three questions from the coordinator: (1) the leaked derive result, (2) the interpreter's `Evaluated` derive that gives no value, (3) the two panics from `mutated.rs`. Rounds 6 to 1 are kept below. F15 is out of scope (owned by `generics`).

**Contract.** `git diff origin/main -- tests/support/ngspice.rs crates/piperine-parity/src` is empty.

**Tests run (gate policy: targeted only, foreground).** Lane D env, the crates the fix touched: `cargo test -p piperine-transpile
--no-fail-fast` (every target green; `derive.rs` 2/2); `cargo test -p piperine-api --no-fail-fast` (22 targets, all green: `session`,
`derived`, `bindings`, `host_sources` among them); `cargo build -p piperine-cli`; `check_proofs.py params-and-io` after this
commit (`proofs.md`: `source_params`, `source_tables`, `run_errors`, `source_host`, `clirun`, `docs` and the `piperine-lang` groups).
`piperine-lang` is not touched by b113b52d. No workspace run.

**Own examples (HEAD binary, throwaway files under `/var/tmp/piperine-tmp-d/v3`, not committed).** 1 V behind 1 kOhm into the load.
- F28's case, now recomputed: `Wrap { big: u64 } -> Load { big: big }`, `--param w.big=9007199254740993` gives 1/7 V (both at 3 mS, by
  hand); a write to the derived `w.l.big` is still `E0802` "derived ... set that param instead".
- F30: `--param w.rs=None` on the pinless wrapper whose child holds `None` runs (0.5 V); `w.rs=500` 1/3 V, `Some(250)` 1/5 V,
  `w.md=Slow` and `w.n=3` 1/4 V.
- F31: `{ rs: rt * 2.0 }` with `w.rt=500` gives 0.5 V (rs = 1 kOhm); `{ rs: a + b }` with `w.a=500` gives 1/3 V. No refusal left.
- F32: `Res { r: match md { Mode::Fast => 1k, Mode::Slow => 3k } }` runs: 0.5 V, and 0.75 V after `w.md=Slow`.
- Regressions: `{ md: if slow .. }` after `w.slow=true` 1/4 V; `{ n: k + 1 }` at `k = 254` 3.906 mV; a pinless param nothing reads is
  still the loud "no PARAM pin" (unchanged since round 4).

**(1) The leaked derive result.** `piperine_derive_2` answers a `List<'static, u8>` over `Box::leak` of a fresh `Vec` on every call
(`print/derive.rs`), and the host copies it out (`load.rs`): every derive call leaks its answer. The module doc calls a recompute "a
rare host `set`", but every point of a DC sweep of a param others derive from, every timed `set` and every host loop of `set`s calls it
once per derived param. Bounded per call, unbounded over a session, and avoidable at no cost: the host owns the buffer (an output
`List`/slice the host allocates at the child's pin width, which it knows from `PinLayout`). A defect, not a design choice (F33).

**(2) The interpreter's `Evaluated`.** It gives `None` for a binding that reads an integer, an enum or a composite param, or yields
one. The caller (`Params::changes_from`) turns `None` into `Failure::Backend` "the derive function `k` of `..` gave no value (the
compiled design does not match its plan)": loud, never silent. It is reached by `Session::describe` with a host value (the API; `piperine
info` passes none); the interpreter backend itself never calls it (with no compiled design `changes` refuses "the interpreter does not
recompute them, use --backend rust"). The message blames a compiled design that does not exist on that path (F34, minor).

**(3) The two panics in the log.** `crates/piperine-transpile/tests/compiled.rs`
`a_panic_in_compiled_code_is_a_fault_and_the_instance_survives` (RS-06 AC 6) wraps `rustc` to inject `panic!("boom at {p}")` into the
generated `terms` and calls it at 200 and 300: the `thread '<unnamed>' panicked at .../mutated.rs:79:48: boom at 200` / `boom at 300`
lines are the panic hook's print of those deliberate panics, which the test asserts become faults. Expected; the target passes.

## Round 6 record (fix of F27 to F29)

**Round 6 verdict**: FAIL
**Round 6 proofs**: `proofs.md` at defc9611 (the round 5 fix, 114 pass, 0 fail; re-run by the round 6 verifier after its own commit); round 5 at b3045722, round 4 at dcbbea67, round 3 at e60177d8, round 2 at acefa2f6, round 1 at 0bed6a92
**Round 6 scope**: round 6 judges the fix of F27 to F29 (dbd3954e, 69791646 amendment A13; `git diff eef03cc1..defc9611`, 6 files) and each loud refusal left after it: a recorded amendment or `GAPS.md` row with a real owner, or a divergence from the spec to implement. Rounds 5 to 1 are kept below. F15 is out of scope (owned by `generics`).

**Contract.** `git diff origin/main -- tests/support/ngspice.rs crates/piperine-parity/src` is empty.

**Tests run (gate policy: targeted only, foreground only).** Lane D env: `cargo build -p piperine-cli`; `check_proofs.py params-and-io`
after this commit (`proofs.md`; `apisession` holds `params_passed_through_a_wrappers_placement_run`). No workspace run.

**Own examples (HEAD binary, throwaway files under `/var/tmp/piperine-tmp-d/v3`, not committed).** 1 V behind 1 kOhm into the load.
- F27: `Wrap { rs } -> Load { rs: rs }` (wrapper with no pin): `w.rs=500` 1/3 V, `w.rs=Some(250)` 1/5 V, `w.md=Slow` 1/4 V (by hand,
  hold). **`w.rs=None` is `E0802` "no PARAM pin `rs`"** while the child already holds `None` (F30).
- F28: `w.big=9007199254740993` (read by a placement) is `E0802` "is read by a placement, and an integer above 2^53 does not cross as
  an `f64`"; `w.l.big=..993` is `E0802` "derived ... set that param instead"; `2^53` and `5` run. Loud, no longer silent.
- F29: `{ md: if slow { .. } }` and `{ md: match k { .. } }` run (0.5 V, and 0.25 V after `w.slow=true` / `w.k=3`). Closed.
- Left refused: `Load { rs: rt * 2.0 }` runs (2 kOhm, 0.667 V) but `--param w.rt=500` is `E0802` "which this design computes from more
  than the param" although it reads one param; so is `{ rs: a + b }` for `w.a`. `Res { r: match md { Mode::Fast => 1k, Mode::Slow =>
  3k } }` over a wrapper's enum param fails the whole design on the Rust backend, default run included: "`a derived param that reads
  the param `md` of type Mode` has no Rust lowering yet".

**The refusals left, judged.** The rule: a loud refusal of something the spec allows is legitimate only as a recorded departure with
a real owner (a `GAPS.md` row naming the feature that lifts it); an amendment of this feature's own criteria does not change
`docs/spec`. A13 is written by the implementer in a fix round, not approved by the user, and `GAPS.md` gains no row in this range.
1. An integer above 2^53 set on a param a placement reads (F28's choice): Part III §8.1.23 and D24 allow it; refused because derive
   functions take `f64`. Loud and rare; legitimate only with a `GAPS.md` row and an owner (or carry integers as `i128`). Missing (F31).
2. A composite bound from an expression over params (`{ rs: rt * 2.0 }`, `{ rs: a + b }`) that a `set` would recompute: §8.1.23
   allows it. A13 says "a composite computed from more than one param is still refused", but a one-param expression is refused too
   and the message says "more than the param"; no `GAPS.md` row. The `SomeOf` case is the derive plan's own `f64` value wrapped in
   `Some`, so it is cheap to implement; until then it needs a row (F31).
3. A placement value that reads an enum param beyond pass-through: §8.1.19 allows any expression over params; A13 does not mention
   it and `GAPS.md` has no row; it refuses the design itself, not only a `set`. Should be implemented (an enum param is held as its
   tag, the derive function can read it back as the variant) or recorded with an owner (F32).

## Round 5 record (fix of F25, F26)

**Round 5 verdict**: FAIL
**Round 5 proofs**: `proofs.md` at b3045722 (the round 4 fix, 114 pass, 0 fail; re-run by the round 5 verifier after its own commit); round 4 at dcbbea67, round 3 at e60177d8, round 2 at acefa2f6, round 1 at 0bed6a92
**Round 5 scope**: round 5 judges the fix of F25, F26 and the cosmetic refusal text (bf92fd5a, 087b0df5, f1df7e30; `git diff 320c3b53..b3045722`, 11 files), the regressions they could introduce (`print/derive.rs`), and two questions from the coordinator: (a) a `set` of a wrapper's `Option` param passed through is `E0802`; (b) a derived enum expression beyond pass-through is refused with no `GAPS.md` row. Rounds 4 to 1 are kept below. F15 is out of scope (owned by `generics`).

**Contract.** `git diff origin/main -- tests/support/ngspice.rs crates/piperine-parity/src` is empty.

**Tests run (gate policy: targeted only, foreground only).** Lane D env: `cargo build -p piperine-cli`; `check_proofs.py params-and-io`
after this commit (`proofs.md`; `apisession`, `sourceparams` hold the new tests). No workspace run.

**Own examples (HEAD binary, throwaway files under `/var/tmp/piperine-tmp-d/v3`, not committed).** 1 V behind 1 kOhm into the load.
- F25 pass-through `Wrap { md, n, rs } -> Load { md: md, n: n, rs: rs }`: default 0.5 V, `w.md=Slow` 0.25 V, `w.n=3` 0.25 V (by hand,
  hold); `w.l.rs=500` 0.333 V (holds); `w.n=256` `E0802`. A structural enum handed down selects its variant (the new test, holds).
- `derive.rs` integer reads (`from_f64`): `{ n: k + 1 }` gives 1/3 V at `k = 1` and 1/6 V after `--param w.k=4`; `{ n: k + 1 }` at
  `k = 255` wraps to 0 (1 V) and `w.k=254` gives 255 (3.906 mV). Hold.
- **An integer above 2^53 set on a parent whose param a placement reads** (`Wrap { big: u64; let l = Load(p) { big: big }; }`, both
  with an `analog` block, 1 mS or 3 mS by `big > 2^53`): `--param w.big=9007199254740993` gives 0.2 V, the wrapper at 3 mS and the
  child still at 1 mS; wanted 1/7 V. `--param w.l.big=9007199254740993`, a derived param, is accepted (0.2 V) where `w.l.big=5` is
  `E0802` "derived from `big` of `Top.w` by its placement" (F28, silent).
- F26: `GearLoad` (derive-less unit enum) runs both variants (the new parity row); my `nr3.msl` example compiles too.
- Cosmetic: the pinless refusals now name the text given: `Top.w.md` `Open`, `Top.w.rs` `5` and `None`. Closed.
- (b) `{ md: if slow { Mode::Slow } else { Mode::Fast } }` and `{ md: match k { 0 => Mode::Fast, _ => Mode::Slow } }` are refused
  "`a derived param of type t0` has no Rust lowering yet" (`print/derive.rs`): loud, by an arena name (`t0`) instead of `Mode`, and no
  `GAPS.md` row (F29).

**(a) The `Option` pass-through.** Part III §8.1.23: "When a host `set` changes a param, every value bound from it (§8.1.19) is
recomputed"; PI-03 AC 4: a host value of `T` on an `Option<T>` param is read as `Some` of it. A `set` of `Top.w.rs` passed on as
`{ rs: rs }` is refused, so the spec's rule does not hold for an `Option` (nor for any composite: `params.rs:407`, from
composite-values ec6cad47, "keeps the value it elaborated to"). It is loud, but it is a departure from §8.1.23 that no
`GAPS.md` row, amendment or owner records; through `--param` on a wrapper with no pin it is also the generic "no PARAM pin `rs`"
refusal, not the placement message. Not consistent with the spec as written (F27).

**Findings of round 4.** F25 and F26 are closed by bf92fd5a, 087b0df5 and f1df7e30; the refusals still unrecorded after them are F27 and F29.

## Round 4 record (fix of F19 to F24)

**Round 4 verdict**: FAIL
**Round 4 proofs**: `proofs.md` at dcbbea67 (the round 3 fix, 114 pass, 0 fail; re-run by the round 4 verifier after its own commit); round 3 at e60177d8, round 2 at acefa2f6, round 1 at 0bed6a92
**Round 4 scope**: round 4 judges the fix of F19 to F24 and two transpiler fixes (141fd5da, 61608529, a67784ad, b2e4efd9; `git diff f351f6c1..dcbbea67`, 19 files) and the regressions they could introduce; rounds 3, 2 and 1 are kept below as they were judged. F15 is out of scope (owned by `generics`).

**Rulings applied (binding).** Round 3's, and: a leaf of a composite param reads its text by the leaf's type with the language's literal syntax (A12).

**Contract.** `git diff origin/main -- tests/support/ngspice.rs crates/piperine-parity/src` is empty.

**Tests run (gate policy: targeted only).** Lane D env: `cargo build -p piperine-cli`; `check_proofs.py params-and-io` after this commit
(`proofs.md`, which runs `apisession`, `elabparams`, `sourceparams` holding the new tests). No workspace run.

**Own examples (HEAD binary, throwaway files under `/var/tmp/piperine-tmp-d/v3`, not committed).**
- F19/F20, structural params selecting the contribution, 1 V behind 1 kOhm: `m.md=Open` and `m.md=Mode::Open` give 1 V, `Short`
  0.25 V; `o.vs=0.5` and `o.vs=Some(0.5)` give 0.5 V, `o.vs=None` 1 V; `k.k=0`/`0x0` select the open branch; `m.md=1`, `m.md=Bogus`,
  `o.vs=x`, `k.k=1.5` are `E0802` at elaboration naming param and value. Hold.
- F21: `Mid { rt: f64 = 3k; let l = Load(p) { rs: rt }; }` gives 0.75 V, and `Mid2 { rs: Some(rt * 1.0) }` with `rt: 1k` 0.5 V on rust;
  the interpreter refuses the `Option` param by name. Hold.
- A12, leaves: `l.cfg.n=0x3` runs 0.25 V; `3e0`, `3.5` (`u8` leaf) and `en=1` (`bool` leaf) are `E0802`; `en=false`, `g=2e-3` run. Hold.
- b2e4efd9: a struct `Cfg { g: f64, n: u8 }` with no derive as a param read by `analog`, bound at the placement and set by leaf: 1/3,
  1/7 (`md=Slow`) and 1/2 V by hand (holds). **A unit enum with no derive as a param** (`enum Mode { Fast, Slow }`, `md: Mode`, read by
  `match`) is a rustc refusal "the trait bound `Mode: Carried` is not satisfied (a transpiler defect)" (F26).
- **A module with no `analog` block** (the coordinator's question): (a) a param read by a `digital` block (the Book's `Watchdog`,
  `limit: u16`) is written: `w.limit=3` fires `stuck` at 3.5 us with 3 edges, `w.limit=0x5` at 5.5 us, `w.limit=70000` is `E0802`;
  (b) a wrapper's `f64` param read by a placement is derived and written (`m.rr=3000` 0.75 V, round 3); (c) a param nothing reads, on an
  instance with no pin (`Plain { gain, n, md, rs, cfg }` holding only a child): every `--param` of it is refused loud, `E0802` "no
  PARAM pin `gain`; the instance declares no writable parameters" (the same for `n`, `md`, `rs`, the leaves of `cfg`, and `Top.tg`).
  Never silent. The refusal of a param that has no effect is stricter than needed, and its message shows the tag (`refuses the value
  1` for `md=Open`) and `rs.value`/`None` for an `Option`: cosmetic, left to the user.
- Pass-through of the new param types from a wrapper (`Wrap { md: Mode; n: u8; rs: Option<f64>; let l = Load(p) { md: md, n: n, rs: rs }; }`):
  `{ n: n }` and `{ md: md }` are refused "`a derived param that reads the integer param ..` has no Rust lowering yet" (transpiler,
  already on main), `{ rs: rs }` is `P0001` "an expression the type checker gave no value type", and a structural pass-through
  (`WrapSw { md } -> Sw { md: md }` with `match md` selecting a contribution) is `P0001` "a composite param in a const expression in the
  value of a param that fixes structure". All loud; none is a `GAPS.md` row (F25).

**Findings of round 3.** F19 to F24 are closed by the commits in the table.

## Round 3 record (fix round of F7 to F18)

**Round 3 verdict**: FAIL
**Round 3 proofs**: `proofs.md` at e60177d8 (the fix round, 114 pass, 0 fail; re-run by the round 3 verifier after its own commit); round 2 at acefa2f6, round 1 at 0bed6a92
**Round 3 scope**: round 3 judges the fix round of F7 to F18 (27e04297, 3c85d77f, 22f74aee, 6b11cc8b, e60177d8; `git diff fd8ba657..e60177d8`, 29 files) and the regressions it could introduce; rounds 2 and 1 are kept below as they were judged.

**Rulings applied (user, AFK, binding).** F11: §3.2.3's implicit `Some` applies to a param binding, PI-03 AC 2 stands. F9, F10,
F17: a `--param` text always goes through its param's type reading. F15: the `m`/`dtemp` row is re-owned to `generics` (GN-26 AC 3).

**Contract.** `git diff origin/main -- tests/support/ngspice.rs crates/piperine-parity/src` is empty: no tolerance, contract or golden edited.

**Tests run (gate policy: targeted only).** The `CARGO_TARGET_DIR=/var/tmp/piperine-target-d` lane: `cargo build -p piperine-cli`;
`cargo test -p piperine-api --test bindings` (8/8, the structural-param bindings next to the `--param` change);
`check_proofs.py params-and-io` after this commit (`proofs.md`). No workspace run.

**Own examples (HEAD binary, throwaway files under `/var/tmp/piperine-tmp-d/v3`, not committed).**
- `--param` on a `u8`/`Mode`/`Option<f64>`/`u64` design, 1 V behind 1 kOhm: `l.n=6` 0.125 V, `l.n=0b110` 0.125 V, `l.md=Slow`
  0.125 V, `l.rs=10` and `l.rs=Some(10)` 9.524 mV, `l.rs=None` 0.1667 V (all by hand: hold); `l.n=6.5`, `1e1`, `-1` and `l.md=1`
  are `E0802` naming param and value (F9 holds); `l.big=9007199254740993` runs, and a `where big < 100` refuses it (the device's check).
- Real params through the new text path: `r0.r=3e3`, `v1.dc=2` on both backends, a placement-derived `m.rr=3000` on rust (0.75 V)
  and its interpreter refusal, a DC sweep of `m.rr`, `--nodeset`, composite leaves `l.cfg.g=3e-3` and `l.cfg.en=false`: hold.
- **Structural params of a type a number does not spell** (`match md { .. }` and `match vs { Some(x) => .., None => .. }` selecting
  the contribution): `--param m.md=Open` runs the `Short` variant (v(a) = 0.25 V, wanted 1 V) and `--param o.vs=Some(0.5)` runs the
  `None` variant (v(c) = 1 V, wanted 0.5 V), silently (F19); `--param o.vs=0.5` is `P0004` "elaboration produced an ill-formed IR:
  param vs is bound to 0.5, not a value of Option<f64>" (F20). A structural `u8` (`k.k=0`, `k.k=0x0`) selects its variant (holds).
- Option bindings: `Load(a, b) { rs: 3000.0 }` over `{ rs: 1000.0 }` gives 0.25 V (holds); `{ rs: 2.0 * 500.0 }` folds (holds);
  `{ rs: rt }` over a parent param `rt: f64` is a new `P0001` and `{ rs: Some(rt) }` a `P0001` too, with no `GAPS.md` row (F21).
- NaN params: `lo: f64 = 0.0 / 0.0` as a default and as a placement binding run on rust (the plan's pin and the library agree; the
  `GAPS.md` row is truly closed).

**Findings of round 2.** F7 to F18 are closed by the commits in the table (F9 with a regression, F19; F11 with a gap left
unrecorded, F21; F17 on the session path only, F23).

**F15 (owner check).** `generics` is a real owner in the plan: ROADMAP item 2 runs before `spec-closure` (item 10), its spec gives
L29 to phase D (GN-24 to GN-26). But **no task covers it**: `origin/feature/generics:.specs/features/generics/tasks.md` tasks
phases A and B only (T1 to T52; "phases C, D and E ... not tasked here"), and its T42 pins `m`/`dtemp` as `P0001`. GN-26 AC 3 was
appended to the approved `generics` spec on this branch, not on `feature/generics`. Reported to the user, not a finding of this feature.

## Round 2 record (B5, B6, closure)

**Round 2 verdict**: FAIL
**Round 2 proofs**: `proofs.md` at acefa2f6 (B5, B6 and the closure: 112 pass, 3 fail - C59 no test ran, C70 and `Run typesunknowns` exit 101); round 1 (B1 to B4) at 0bed6a92
**Round 2 scope**: round 2 judges batches B5 and B6 (T29 to T44) and the feature's closure, over `git diff origin/main...HEAD` (37911891..acefa2f6, 18 commits, 92 files); round 1 (B1 to B4, T1 to T28) is kept below as it was judged.

**Contract.** `git diff origin/main -- tests/support/ngspice.rs` is empty and `crates/piperine-parity/src` is
unchanged: no `ToleranceContract`, no contract file, no golden edited. The new parity rows (`source_params`,
`source_tables`) call the existing `ToleranceContract::DC_VOLTAGE` or bounds of their own.

**Tests run (gate policy: targeted only).** `check_proofs.py params-and-io` at acefa2f6 (env lane d): the
`**Build**` line green (zero warnings, `suite_hygiene`), every `Run` group green but `typesunknowns`; ngspice on
`PATH`. No workspace run.

**Own examples (HEAD binary, throwaway files under `/var/tmp/piperine-tmp-d/v1`, not committed).**
- `table_model`: control `1` with no extrapolation character gives -2 at x = -1 and 3 at x = 6 (`L` at both ends,
  rule 3: holds); a cubic sampled at exactly four points (`3`) is reproduced at 0.5, 1.5, 2.5 (holds); the
  quadratic spline of `2` matches Appendix C.2 by hand at 0.5, 1.5, 2.5, 4.5 and at both ends (holds).
- `Option` param read through `if let Some(x)`, `match` and `is_some` in `analog`: `Some(500)` gives 0.1667 V and
  `None` 0.5 V, as computed by hand (holds; the test of C13 does not show it, F12).
- The section 18 `Diode` with `rs: Option<f64>` and `match rs` runs an operating point (0.618 V) (holds; the
  records still say it waits on this feature, F16).
- `--param` against `n: u8`, `md: Mode`, `rs: Option<f64>`: `l.n=6.5` runs as 6, `l.n=1e1` as 10, `l.n=-1` as 255,
  `l.md=1` as `Slow`, `l.rs=10` is `E0802` "a composite param" (F9, F10); `l.rs=Some(10)`, `l.rs=None`,
  `l.md=Medium` (`E0802` listing `Fast`, `Slow`) and `l.n=256` (`E0802`) hold.
- A placement `Load { rs: 10.0 }` is `E0180` "expected `Option<f64>`, found `f64`" (F11).
- `parse_table` of a file holding `1e-3k` folds to 1.0 (F14); `.5` reads 0.5.

**Amendments of the B6 handoff, judged.** (a) `x`/`z` hex digits are `E0706`: legitimate - `docs/spec/part_vii_stdlib.md`
§22.6.4's resolution says so on main ("A digit `x` or `z` is `E0706`: the elements are integers") and MD-78 item 1
removed the known flag from integers; the code follows the spec. (b) `E0906` retired: legitimate - Appendix A's
resolution and Appendix G (`appendix_g_scope.md:417`) retire it. (c) `E0102` retired: legitimate (value semantics,
MD-76). (d) `table_model` takes its table by value: legitimate - §22.2.15's signature is `table: [(f64, f64); N]`.
(e) A payload-enum param runs: follows §8.1.15. (f) A composite-payload param is set by its leaves: the
composite-values rule. Each is a departure from an approved criterion of `spec.md` (PI-09 AC 2, PI-12 AC 1 and 12,
PI-10 AC 1), and none is written in `spec.md`'s Amendments or appended as a criterion: they live only in the tasks
Handoff (F8), and two checks still assert the retired behaviour (F7).

**Book.** No row of `tests/book/inventory.tsv` is refused for L20, L28, L39 or L57: the nine L28 rows, packages#12
and cheatsheet#10 run, and `GAPS.md`'s refused table no longer lists them. One refused row still names this feature
as its owner (F15).

**Records.** `GAPS.md` lists none of L20, L28, L39, L57, X5, H12 as a row; section 10 equals `code_owners.tsv`
(checked by C112, 29 codes). Stale mentions remain (F16).

## Round 1 record (B1 to B4)

**Round 1 verdict**: PASS for B1 to B4 at 0bed6a92 (B5 and B6 then deferred).

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

### Fix round (F1 to F6)

F1, F3, F4, F5 and F6 are closed by the commits in the table; F2 is closed by deferral to
`composite-values`. F1 also changed the counts that `types_analog.rs` asserted (`ac_stim()`,
`temperature(1.0)`, `bound_step()`, `freq(1.0)`: `E0141`, commit 0bed6a92). Rebased on origin/main
(one new commit, a Book chapter); main holds no `MD-74`, so no renumbering. Root `env -u
PIPERINE_NGSPICE cargo test --no-fail-fast`: exit 0, 301 test results, 3096 passed, 0 failed, 3
ignored. With ngspice on `PATH`: `source_host` 3/3, `parity_ac` 2/2, `parity_tran` 9/9.
`check_proofs.py`: 63 pass, 52 fail, the 52 being the B5/B6 checks and `Run sourcetables`.

## For the user

C83's DC-step comparison reuses main's frozen `tran_rc.cir` (ngspice's default step cap) against
piperine `max_step 1e-8`: the twin convention of the existing `tran_rc` test, not the same-cap
configuration of C84. Left as it is for the user or the spec closure to rule on.

Round 2: F11 needs a ruling (does §3.2.3's implicit `Some` apply to a param binding, as PI-03 AC 2 says?).

Round 3: F19 and F20 need an implementer (the `--param` of a structural param of enum or `Option` type); F15's owner `generics` has no task for GN-26 AC 3 yet (phase D is untasked, its T42 pins `m`/`dtemp` as `P0001`), and GN-26 AC 3 was added to the approved `generics` spec from this branch. A leaf of a composite param still takes any number (`l.cfg.n=0x3` is `E0802`, `l.cfg.en=1` is `true`): the leaf path is composite-values' D13, not this feature's type reading; left for the user to rule on.

Round 4: F25 and F26 need an implementer. A `--param` of a param nothing reads on an instance with no pin is refused loud (`E0802` "no PARAM pin"): not silent, but stricter than needed, with a message that shows an enum's tag and an `Option`'s `value` leaf; left for the user to rule on.

Round 5: F27, F28 and F29 need an implementer; F27 also needs a ruling if the departure from §8.1.23 is to stand (an amendment with an owner) rather than be implemented.

Round 6: F30 to F32 need an implementer; F31 item 1 (integers above 2^53 through a placement) is reasonable to defer, but only as a `GAPS.md` row with a real owner, and A13 is the implementer's amendment, not yet the user's.

Round 7: F30 to F32 are closed by the typed derive; F33 to F35 are minor and need an implementer. Every refusal of §8.1.19/§8.1.23 forms judged in round 6 is gone, so no `GAPS.md` row is owed by this feature for them.

Round 8: F33 to F35 are closed; F36 and F37 are minor and need an implementer.
