# Generics verification (phase C, T53 to T82, T116, T117)

**Verdict**: FAIL (finding 1, High: a check proves a false claim and the rule it names is silently not enforced)
**Proofs**: `proofs-c.md` at 9d35727d (84 pass, 2 fail: both C127 rows cite tests that no longer exist; build and hygiene pass)
**Verifier**: independent sub-agent (author != verifier), Opus, 2026-11-24, branch `feature/generics` at 9d35727d (= origin/feature/generics; diff `a8b1f34f..HEAD`, merge base with origin/main)
**Method**: `tasks-c.md` copied as `checks.md`/`tasks.md` into a scratch feature dir (`.specs/features/generics-c/`, deleted) and `check_proofs.py generics-c` run from the worktree root; own probes in a throwaway target `crates/piperine-lang/tests/zz_verifier_probe.rs` (deleted, never committed).
**Gate policy**: the verdict is FAIL, so the origin/main merge (862aecd2) and the final full suite were NOT run (they run only after a PASS).

## Judged areas

- **Interim gate (T80)**: deleted from the sources (`UnfinishedForms`, `check_admitting`, `Project::admitting`, `IN_FORCE` gone; no hit in `crates/*/src`, `src`, `docs`, `GAPS.md`). Leftover wording in two test files only (finding 8).
- **Book rows**: modules#1, #2, #6, #8, digital#21, core-std#8, cheatsheet#2, #3 flipped to `run` with value-level tests; modules#10, cheatsheet#8, natures#8 re-pinned to exact `assert_eq!` lists (no `contains` left), which is narrower than before, not wider. Two of the exact lists bake in diagnostics I judge wrong (findings 2, 3).
- **Digital engine contract (ab307001)**: `write_net` drops the `Change` only for a net whose `NetSpec::record` is set; that flag is set only on a stream's pin, a signal named like the stream is a different net and keeps reporting its changes. The api's size heuristic `is_record_write` is gone. `checkpoint.rs` now pins 0 changes of the record pin while still asserting its records. No real signal change is lost. Holds.
- **Cascading E0187 suppression (c5459276)**: `keep_handed_after_failure` marks only carriers at `out` ports of the failed placement as handed; the bound error is reported once. Holds.
- **Hoisted `out` carriers / `Session::discrete` array nets**: proven by `a_hoisted_generic_module_commits_to_the_carriers_its_out_ports_are_given` and the cheatsheet#2 run; `Session::discrete` reads each element of an array net (session.rs:922). No direct test of the array read; acceptable.
- **spec_codes / code_owners**: `E0322`, `E0340`, `E0342` raised (`types/generation.rs`, `elab/generation.rs`) and removed from `code_owners.tsv` and from GAPS §10 (17 -> 14). `E0701` retired (part VII §22.1.3 rule 2, `RETIRED_CODES`), recorded in the approved amendment. Consistent.
- **GAPS counts**: 0 silent, 16 loud = 20 less L30, L31, L69, L70. Arithmetic holds; but finding 1 is a silent behaviour with no row, and L58 is stale (finding 4).
- **T81 gallery list [E.1] -> seven fences**: legitimate: the selector is "every fence that writes a generic module", and seven do; widening the held set is stricter. The assertion itself is weak (finding 6).
- **POM navigability**: `Elaborator::new(&design)` borrows the POM immutably; `one_body_per_distinct_module_instance_and_the_pom_as_written` asserts the generic module, its generic list and the three placements as written and no instance in `Design::modules`. Holds.
- **Code cleanliness**: no new free function outside `#[test]`s; new methods are named in full words (one `lower_carrier_arg`); doc comments present on the new items read. No tolerance, contract or golden edited (`tests/support/ngspice.rs`, `crates/piperine-parity/src` deviation/contract untouched by the diff).

## Own examples (HEAD 9d35727d)

- `module M(in clk: Bit) { signal s: [Bit; 2]; }` placed from a top whose `c` is initialised: check_model, check and elaboration all clean. Same for `[Bit<u8>; 2]`, for `signal s: [Bit; 2];` in the top itself (read in a reaction), and for `M<const N: usize>` with `[Bit; N]` at `M::<2>`. A scalar `signal s: Bit;` in the same place is `E0187`. **Fails** (finding 1).
- `Setup(clk, d, v) { ts: 1n }`, `Hold(clk, d) { th: 1n }`, `Stable(en, d)` at `u8`, `MinPulse(d) { w: 1n }`: all check and elaborate (`Setup::<bool>`, `Hold::<bool>`, `Stable::<u8>`). GAPS L58 still lists them refused (finding 4).
- Nested generation `for i in 0..2 { for j in 0..3 { pub let r = R(..); } }`; a generation range over a param (`0..n`, body `Top~n=2`); `Tap::<1>` at the `where N >= 1` boundary; `reset: T` with `T = u8`: all elaborate. Hold.
- `Short(a, b)` with `Electrical`/`Thermal`: `E0180` naming both natures. Holds (message has nested backticks, finding 9).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C51 | `elab_generics.rs` `encode_and_decode_of_every_layout_lower_through_the_level_cast`; `source_generics.rs` `encode_and_decode_run_at_every_layout` | yes - commits compared per layout to Rust-computed bytes |
| C52 | `types_traits.rs` `a_hand_written_repr_on_a_generic_type_is_typed_per_argument`; `source_generics.rs` `a_generic_hand_written_repr_is_carried_with_its_layout` | yes |
| C53 | `source_generics.rs` `a_carrier_of_a_hand_written_type_starts_at_its_default_levels` | yes |
| C54 | `types_generic_modules.rs` `generic_module_arguments_are_explicit_or_inferred`; `source_generic_modules.rs` `generic_modules_run_with_explicit_and_inferred_arguments` | yes |
| C55 | `types_generic_modules.rs` `a_behavior_block_binds_its_modules_generic_names` | yes |
| C56 | `types_generic_modules.rs` `behavior_names_that_differ_are_e0403_naming_both_lists` | yes |
| C57 | `elab_generic_modules.rs` `a_false_module_where_is_e0304_naming_the_instance`; `book_modules` `e0304_a_generic_ladder_of_zero_sections` | yes - exact code list and `Tap::<0>`; my `Tap::<1>` boundary holds |
| C58 | `types_generic_modules.rs` `const_expressions_in_written_types_are_accepted_everywhere` | yes |
| C59 | `elab_generics.rs` `a_const_fn_and_a_generic_repeat_fold_in_a_const_context` | yes |
| C60 | `elab_generic_modules.rs` `one_body_per_distinct_module_instance_and_the_pom_as_written` | yes - exact sorted body names, POM module and instance counts |
| C61 | `ir/generics.rs` `a_body_instance_is_built_checked_and_printed`; `transpile/generics.rs` `module_instance_names_compile_and_list_in_the_library` | yes |
| C62 | `source_generic_modules.rs` `short_merges_two_nodes_of_each_nature` | yes - three natures, the joined divider equals the written one |
| C63 | `types_generic_modules.rs` `two_natures_for_one_nature_parameter_are_e0180_naming_both`; `source_generic_modules.rs` `short_over_two_natures_is_refused_naming_both` | yes |
| C64 | `types_generation.rs` `generated_items_are_typed_as_arrays_by_ordinal`; `source_generation.rs` `generated_instances_run_and_read_by_ordinal` | yes |
| C65 | `elab_generation.rs` `generation_for_makes_arrays_and_if_keeps_the_taken_arm`; `source_generation.rs` `a_generation_if_places_the_taken_arm` | yes |
| C66 | `types_generation.rs` `a_generation_range_that_is_not_const_is_e0340`; `book_modules` `e0340_a_generation_range_read_from_a_signal` | yes |
| C67 | `elab_generation.rs` `an_untaken_arm_or_an_index_outside_is_e0342` | yes |
| C68 | `types_generation.rs` `a_param_inside_generation_is_e0322` | yes |
| C69 | `elab_generation.rs` `a_recursion_by_generation_unrolls` | yes |
| C70 | `source_generation.rs` `the_generic_ladder_equals_the_ladder_written_out`; `book_modules` `the_ladder_runs_for_an_operating_point_and_a_sweep` | yes - bit-exact at N = 1, 3, 8, same instants |
| C71 | `elab_generation.rs` `a_generation_recursion_that_does_not_end_is_e0341` | yes |
| C72 | `types_generation.rs` `generation_in_a_digital_block_is_typed_per_iteration`; `source_generation.rs` `a_generated_bank_reacts_per_element`, `generation_in_a_digital_block_runs_from_every_entry` | yes - per-element latched values and times |
| C73 | `elab_generation.rs` `generated_reactions_are_one_per_iteration_in_source_order`; `book_modules` `the_half_reactions_report_each_stage_in_time_order` | yes |
| C74 | `elab_generic_modules.rs` `a_param_read_by_a_generic_argument_or_a_length_is_structural` | yes - exact structural list `[count, n]` |
| C75 | `session.rs` (api) `a_structural_param_overrides_before_the_run_and_is_refused_during_it` | partly - the override and a `set` are held; no sweep during the run is tried and the message is asserted to contain `count`, not that it names the param as structural (finding 7) |
| C76 | `types_generic_modules.rs` `a_generic_module_body_is_checked_once_at_its_definition`; `types_model.rs` `an_unreached_generic_module_with_an_error_fails_the_model_check` | yes |
| C77 | `interp/frozen.rs` `each_generics_form_is_refused_by_name` | yes |
| C78 | `types_generics.rs` `phase_d_and_e_forms_stay_p0001_by_name` | partly - the template and the placement through one are `P0001`; the `Fn(P)` bound and an un-inferred template parameter (phase D) are refused with `E0187`/`E0181`, not `P0001`, and no case covers them (finding 2) |
| C79 | `cli/twins.rs` `every_twin_prints_the_bytes_it_printed_before_the_move` | yes |
| C80 | build + `suite_hygiene` | yes - exit 0, no warning |
| C81 | `cli/docs.rs` `gaps_drops_l70`, `gaps_drops_l69`, `gaps_drops_s3`, `gaps_drops_what_generics_phase_c_closed` | yes for the rows named; L58 is not held (finding 4) |
| C82 | `book_digital`, `book_cheatsheet`, `book_core_std`, `book_modules` run tests; `std_galleries.rs` `the_fences_generics_phase_c_lifted_are_checked`; `book_inventory` | partly - the Book runs assert values; the cheatsheet#2 proof line matches only `..._without_its_value_input` (finding 5); the gallery test forbids strings no source emits any more (finding 6) |
| C83 | `elab_generic_modules.rs` `a_chain_of_growing_module_arguments_is_e0341` | yes |
| C84 | `source_generic_modules.rs` `a_nature_parameter_takes_its_default`, `a_const_generic_of_zero_makes_an_empty_module` | yes |
| C85 | `elab_generation.rs` `an_empty_generation_range_makes_an_empty_array` | yes |
| C86 | `source_generics.rs` `cast_and_same_from_source_agree_with_the_removed_lowering`, `a_cast_to_a_tuple_holding_an_enum_runs`; `types_generics.rs` `no_intrinsic_stands_for_cast_or_same`; `docs.rs` `gaps_drops_the_cast_and_same_intrinsics` | yes |
| C120 | `book_core_std` `a_carrier_port_left_out_is_e0312_as_the_old_register_listing_was`; `book_cheatsheet` `an_array_of_bits_with_no_initial_value_is_e0187`; `book_natures` `the_slice_listing_bounds_t_by_default_as_gallery_e6_does` | **no** - the BA2 test passes on the top's own `signal c: Bit;`, and the claim is false: `signal s: [Bit; 2];` is accepted (finding 1); the BA3 test is likewise satisfiable by `c` alone (the rule it names does hold at HEAD) |
| C121 | `types_modules.rs` `a_trailing_option_value_input_passed_by_name_binds_it`, `a_named_connection_that_is_no_trailing_option_input_is_e0140`; `elab_results.rs` `a_value_input_passed_by_name_reads_its_argument` | yes |
| C122 | `source_generation.rs` `generation_in_a_digital_block_runs_from_every_entry` | yes - check_model, check, elaborate (15 reactions) and compile from the plain entries; gate code absent |
| C123 | `types_generic_modules.rs` `a_generic_placement_feeds_back_through_its_unknown` | yes |
| C124 | `types_generic_modules.rs` `a_param_of_a_type_parameter_is_typed_under_its_bounds` | yes |
| C125 | `book_digital` `e0187_the_flip_flop_at_u8_has_no_default`; `types_generic_modules.rs` `a_carrier_handed_to_a_generic_out_port_needs_no_initial_value` | yes - exact one-diagnostic list |
| C126 | `types_generic_modules.rs` `an_underscore_generic_argument_is_inferred` | yes |
| C127 | `book_modules` `the_memory_bank_is_refused_for_its_template_and_bundle`; `book_cheatsheet` `the_template_card_is_refused_for_its_template_and_bundle`; `book_natures` `a_bundle_is_refused_where_the_solver_reaches_it`; `book_generics`, `book_welcome`, `cli/book` | **no** - two proofs cite `the_ladder_is_refused_for_its_generation` and `the_half_reactions_are_refused_for_their_digital_generation`, which do not exist (the rows run, C70/C73): proof red (finding 5); the exact pins hold what HEAD prints, including diagnostics judged wrong in findings 2 and 3 |

## Findings

| # | Severity | Finding | Evidence | Failure scenario | Fix | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | High | A carrier array of a type with no `Default` and no initial value is silently accepted, and C120's test proves it is `E0187` only because the top's own `signal c: Bit;` is `E0187` | `tests/book/cheatsheet.rs:212-218` (`signal c: Bit;` in `Top`, `faulty.first("E0187")` with no message); probes at HEAD: `[Bit; 2]`, `[Bit<u8>; 2]`, `[Bit; N]` at `M::<2>`, and `[Bit; 2]` in a top read by a reaction all check and elaborate clean, while a scalar `signal s: Bit;` is `E0187` | a user writes the Book's old cheatsheet#2 `signal s: [Bit; N];`; §3.3.4 says it has no value at (0, 0) and must be refused, Piperine runs it from some unstated value; GAPS shows 0 silent rows | enforce §3.3.4 for arrays (element type `Default` or an initial value) or record a silent row with an owner; rewrite the test with `signal c: Bit = false;` and assert the message names `s`; same `= false` in `tests/book/natures.rs:279` | |
| 2 | Medium | Phase D forms are refused with language errors, not `P0001` naming the form, and the exact re-pins freeze that | `tests/book/cheatsheet.rs:152` pins `E0187` "`module `Bsim4`` does not satisfy the bound `Fn` of `C` in `Inner`"; GAPS row modules#10 records `E0181` for the template `C`; C78's test (`types_generics.rs:451`) has no `Fn(P)` case | a user's valid template-bound module (§8.4) is told their code is wrong (`E0187`/`E0181`) instead of "not supported yet", against GN-29 AC 2; when phase D lands the pins must be undone as wrong errors, not flipped | refuse a `Fn(P)` bound and an un-inferable template parameter with `P0001` naming the form until phase D; add both to `phase_d_and_e_forms_stay_p0001_by_name`; re-pin | |
| 3 | Low | natures#8's exact pin includes three `E0222` "a parent never writes a child's drive" for definitions through the members of the refused bundle | `tests/book/natures.rs:138-146`, GAPS natures#8 row | the cause is the `P0001` bundle (L25); the three `E0222` are cascades of it, the same kind of noise c5459276 removed for `E0187`; a reader is told a spec-legal definition is illegal | suppress diagnostics on members of a refused bundle type, or record the cascade under L25 explicitly as owed noise | |
| 4 | Medium | GAPS L58 still lists `since`, `Stable`, `Setup`, `Hold` and `MinPulse` (`since`) as refused stdlib ("Generics (phase E)"), though they check and elaborate, and core-std#8 runs `Setup` | `GAPS.md` L58 row; probes above; `tests/book/core_std.rs:134` | the gap list overclaims refusals, against T82's What ("moves each L58 item phase C unblocked ... or out, when it runs"); the stdlib coverage numbers read lower than they are | move each item that runs out of L58 (phase E keeps only its value proofs, T106 to T108); hold it in `docs.rs` | |
| 5 | Low | Check bookkeeping: C127 cites two tests that no longer exist (proof red), and C82's cheatsheet#2 proof line names a test that exists only as a prefix | `proofs-c.md`: C127 rows "no test ran"; tests are `the_module_card_runs_for_its_const_generic_without_its_value_input` and `the_module_card_runs_with_its_value_input_and_its_out_carrier` | the phase cannot close green at `HEAD` with its own checks | drop the two C127 proofs (C70, C73 hold those rows) and name both cheatsheet#2 tests in C82 | |
| 6 | Low | `the_fences_generics_phase_c_lifted_are_checked` forbids messages no source emits after T80, so it holds nothing | `crates/piperine-lang/tests/std_galleries.rs:476,491-514`; the generic unsupported path now says "generation `for`/`if` ... is not supported yet" (`types/unsupported.rs:141`), which none of the three forbidden strings contains | a regression that sends generation or a generic module back to `P0001` passes the gallery proof | assert each of the seven fences checks clean, or pin its exact remaining refusals with their owners | |
| 7 | Low | C75's test tries a `set` during the run but no sweep, and asserts `contains("count")` rather than the structural naming | `crates/piperine-api/tests/session.rs:741-743` | a sweep of a structural param that silently re-elaborates or ignores the value passes | add a `dc` sweep of `Top.count` expecting `E0803`; assert the message names it structural | |
| 8 | Low | Interim-gate leftovers in test docs after T80 | `crates/piperine-lang/tests/types_generic_modules.rs:14-15` (`struct Admitted`, "still behind the unfinished-forms gate"); `crates/piperine-lang/tests/elab_generic_modules.rs:9` ("generic modules admitted") | stale docs describe a mechanism that no longer exists | rename the harness and its doc | |
| 9 | Cosmetic | Nested backticks in two messages | "`module `Bsim4`` does not satisfy ..." (`tests/book/cheatsheet.rs:156`); "expected `node of nature `Electrical``" (Short over two natures) | unreadable diagnostic text | spell the inner name without its own backticks | |
