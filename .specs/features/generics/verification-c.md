# Generics verification (phase C, T53 to T82, T116, T117)

**Verdict**: PASS (round 6 at 12fc93043: findings 16, 17 and 18 closed; every phase C check proven at HEAD (117 pass); two non-blocking findings recorded: 19, Low, an explicit `--print` of a mirror's internal name still resolves; 20, Low-Medium, a loop through elements of an array is refused by the digital bind after compiling, not by `E0339` - the class is composite-values', this phase's run-time-index reads only widen it)
**Round 2**: re-verification at bc953965 (fixes 26fff208 for 1 and 3, 3b6cdc29 for 2 and 4 to 9; bookkeeping bc953965); `proofs-c.md` rerun at bc953965: 85 pass, 0 fail, tree clean, build and hygiene pass
**Proofs**: `proofs-c.md` at 9d35727d (84 pass, 2 fail: both C127 rows cite tests that no longer exist; build and hygiene pass)
**Verifier**: independent sub-agent (author != verifier), Opus, 2026-11-24, branch `feature/generics` at 9d35727d (= origin/feature/generics; diff `a8b1f34f..HEAD`, merge base with origin/main)
**Method**: `tasks-c.md` copied as `checks.md`/`tasks.md` into a scratch feature dir (`.specs/features/generics-c/`, deleted) and `check_proofs.py generics-c` run from the worktree root; own probes in a throwaway target `crates/piperine-lang/tests/zz_verifier_probe.rs` (deleted, never committed).
**Round 3**: re-verification at c94a26d1 (fixes 3db042d5 for 10, ef108666 for 11, 6cd8390e fixture; c94a26d1 bookkeeping); `proofs-c.md` rerun at c94a26d1: 89 pass, 0 fail, tree clean
**Round 4**: re-verification at ac587e0e (fixes 727c967c for 12 and 13's code, a92693fb for 13, 4f50259c E0338 per instance and the false E0335); `proofs-c.md` rerun at ac587e0e: 96 pass, 0 fail, tree clean
**Round 5**: re-verification at 6c87900da (fixes 856ff5dbc for 14 and 15, 20b3624e6 element readers on main's composite-values code, 96be45681 generated-instance members, 1405f53e4 D8 test); `proofs-c.md` rerun at 6c87900da: 112 pass, 0 fail, tree clean
**Round 6**: re-verification at 12fc93043 (fixes 1fd0c0566 for 16 and 17, 1e46f8050 for 18); `proofs-c.md` rerun at 12fc93043: 117 pass, 0 fail, tree clean
**Gate policy** (all rounds): the verdict is FAIL, so the origin/main merge (862aecd2) and the final full suite were NOT run (they run only after a PASS).

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
| C120 (round 1) | `book_core_std` `a_carrier_port_left_out_is_e0312_as_the_old_register_listing_was`; `book_cheatsheet` `an_array_of_bits_with_no_initial_value_is_e0187`; `book_natures` `the_slice_listing_bounds_t_by_default_as_gallery_e6_does` | **no** - the BA2 test passes on the top's own `signal c: Bit;`, and the claim is false: `signal s: [Bit; 2];` is accepted (finding 1); the BA3 test is likewise satisfiable by `c` alone (the rule it names does hold at HEAD) |
| C121 | `types_modules.rs` `a_trailing_option_value_input_passed_by_name_binds_it`, `a_named_connection_that_is_no_trailing_option_input_is_e0140`; `elab_results.rs` `a_value_input_passed_by_name_reads_its_argument` | yes |
| C122 | `source_generation.rs` `generation_in_a_digital_block_runs_from_every_entry` | yes - check_model, check, elaborate (15 reactions) and compile from the plain entries; gate code absent |
| C123 | `types_generic_modules.rs` `a_generic_placement_feeds_back_through_its_unknown` | yes |
| C124 | `types_generic_modules.rs` `a_param_of_a_type_parameter_is_typed_under_its_bounds` | yes |
| C125 | `book_digital` `e0187_the_flip_flop_at_u8_has_no_default`; `types_generic_modules.rs` `a_carrier_handed_to_a_generic_out_port_needs_no_initial_value` | yes - exact one-diagnostic list |
| C126 | `types_generic_modules.rs` `an_underscore_generic_argument_is_inferred` | yes |
| C127 (round 1) | `book_modules` `the_memory_bank_is_refused_for_its_template_and_bundle`; `book_cheatsheet` `the_template_card_is_refused_for_its_template_and_bundle`; `book_natures` `a_bundle_is_refused_where_the_solver_reaches_it`; `book_generics`, `book_welcome`, `cli/book` | **no** - two proofs cite `the_ladder_is_refused_for_its_generation` and `the_half_reactions_are_refused_for_their_digital_generation`, which do not exist (the rows run, C70/C73): proof red (finding 5); the exact pins hold what HEAD prints, including diagnostics judged wrong in findings 2 and 3 |

## Findings

| # | Severity | Finding | Evidence | Failure scenario | Fix | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | High | A carrier array of a type with no `Default` and no initial value is silently accepted, and C120's test proves it is `E0187` only because the top's own `signal c: Bit;` is `E0187` | `tests/book/cheatsheet.rs:212-218` (`signal c: Bit;` in `Top`, `faulty.first("E0187")` with no message); probes at HEAD: `[Bit; 2]`, `[Bit<u8>; 2]`, `[Bit; N]` at `M::<2>`, and `[Bit; 2]` in a top read by a reaction all check and elaborate clean, while a scalar `signal s: Bit;` is `E0187` | a user writes the Book's old cheatsheet#2 `signal s: [Bit; N];`; §3.3.4 says it has no value at (0, 0) and must be refused, Piperine runs it from some unstated value; GAPS shows 0 silent rows | enforce §3.3.4 for arrays (element type `Default` or an initial value) or record a silent row with an owner; rewrite the test with `signal c: Bit = false;` and assert the message names `s`; same `= false` in `tests/book/natures.rs:279` || 26fff208; C120 (`an_array_of_bits_with_no_initial_value_is_e0187` now gives `c = false`, asserts the only code and the message naming `s`, and adds `[Bit<u8>; 2]`, `[Bit; N]` at `M::<2>` and a top array committed in a reaction) - partly: see finding 10 |
| 2 | Medium | Phase D forms are refused with language errors, not `P0001` naming the form, and the exact re-pins freeze that | `tests/book/cheatsheet.rs:152` pins `E0187` "`module `Bsim4`` does not satisfy the bound `Fn` of `C` in `Inner`"; GAPS row modules#10 records `E0181` for the template `C`; C78's test (`types_generics.rs:451`) has no `Fn(P)` case | a user's valid template-bound module (§8.4) is told their code is wrong (`E0187`/`E0181`) instead of "not supported yet", against GN-29 AC 2; when phase D lands the pins must be undone as wrong errors, not flipped | refuse a `Fn(P)` bound and an un-inferable template parameter with `P0001` naming the form until phase D; add both to `phase_d_and_e_forms_stay_p0001_by_name`; re-pin || 3b6cdc29; C78 (`phase_d_and_e_forms_stay_p0001_by_name` gains a given and an inferred `C: Fn(..)`, all codes `P0001`); cheatsheet#8 and modules#10 re-pinned to the `P0001` |
| 3 | Low | natures#8's exact pin includes three `E0222` "a parent never writes a child's drive" for definitions through the members of the refused bundle | `tests/book/natures.rs:138-146`, GAPS natures#8 row | the cause is the `P0001` bundle (L25); the three `E0222` are cascades of it, the same kind of noise c5459276 removed for `E0187`; a reader is told a spec-legal definition is illegal | suppress diagnostics on members of a refused bundle type, or record the cascade under L25 explicitly as owed noise || 26fff208 (`definition.rs`: a field with a direction is refused once, at the bundle); C127 natures#8 pin holds the two `P0001` only |
| 4 | Medium | GAPS L58 still lists `since`, `Stable`, `Setup`, `Hold` and `MinPulse` (`since`) as refused stdlib ("Generics (phase E)"), though they check and elaborate, and core-std#8 runs `Setup` | `GAPS.md` L58 row; probes above; `tests/book/core_std.rs:134` | the gap list overclaims refusals, against T82's What ("moves each L58 item phase C unblocked ... or out, when it runs"); the stdlib coverage numbers read lower than they are | move each item that runs out of L58 (phase E keeps only its value proofs, T106 to T108); hold it in `docs.rs` || 3b6cdc29; GAPS L58 drops `since`, `Stable`, `Setup`, `Hold`, `MinPulse`; `docs.rs` holds it (C81) |
| 5 | Low | Check bookkeeping: C127 cites two tests that no longer exist (proof red), and C82's cheatsheet#2 proof line names a test that exists only as a prefix | `proofs-c.md`: C127 rows "no test ran"; tests are `the_module_card_runs_for_its_const_generic_without_its_value_input` and `the_module_card_runs_with_its_value_input_and_its_out_carrier` | the phase cannot close green at `HEAD` with its own checks | drop the two C127 proofs (C70, C73 hold those rows) and name both cheatsheet#2 tests in C82 || bc953965; C127 drops the two stale proofs, C82 names both cheatsheet#2 tests; proofs 85 pass |
| 6 | Low | `the_fences_generics_phase_c_lifted_are_checked` forbids messages no source emits after T80, so it holds nothing | `crates/piperine-lang/tests/std_galleries.rs:476,491-514`; the generic unsupported path now says "generation `for`/`if` ... is not supported yet" (`types/unsupported.rs:141`), which none of the three forbidden strings contains | a regression that sends generation or a generic module back to `P0001` passes the gallery proof | assert each of the seven fences checks clean, or pin its exact remaining refusals with their owners || 3b6cdc29; C82 (`the_fences_generics_phase_c_lifted_are_checked` pins the exact ten remaining refusals of the seven fences; traced: F.3's probe-through-natures and `fall` refusals are on `c.bl`/`c.wl`, members of the bundle `c: Cell`, so they follow from L25) |
| 7 | Low | C75's test tries a `set` during the run but no sweep, and asserts `contains("count")` rather than the structural naming | `crates/piperine-api/tests/session.rs:741-743` | a sweep of a structural param that silently re-elaborates or ignores the value passes | add a `dc` sweep of `Top.count` expecting `E0803`; assert the message names it structural || 3b6cdc29; C75 (a `dc` sweep of `Top.count` is `E0803`; both messages asserted to contain "`Top.count` is a structural param") |
| 8 | Low | Interim-gate leftovers in test docs after T80 | `crates/piperine-lang/tests/types_generic_modules.rs:14-15` (`struct Admitted`, "still behind the unfinished-forms gate"); `crates/piperine-lang/tests/elab_generic_modules.rs:9` ("generic modules admitted") | stale docs describe a mechanism that no longer exists | rename the harness and its doc || 3b6cdc29 (harness renamed, gate wording gone; `lower_carrier_argument`) |
| 9 | Cosmetic | Nested backticks in two messages | "`module `Bsim4`` does not satisfy ..." (`tests/book/cheatsheet.rs:156`); "expected `node of nature `Electrical``" (Short over two natures) | unreadable diagnostic text | spell the inner name without its own backticks || 3b6cdc29 (`node of nature Electrical`; the `Fn` bound message is now the `P0001`) |

## Round 2 (bc953965)

**Proofs**: `check_proofs.py` on `tasks-c.md` at bc953965 (scratch feature dir, deleted): 85 pass, 0 fail, tree clean, build with no warning, `suite_hygiene` passes.

**What the coordinator asked to watch:**

- **Finding 1's driver rule vs §3.3.4.** The rule (`types/module.rs:526-551`, `types/definition.rs:172-177`): an array of carriers of a non-`Default` type with no initial value is `E0187` unless the *array* is in `undelayed` (a whole undelayed definition), in `undelayed_elements` (an undelayed definition of *any one* element) or in `handed` (the array or *any one* element handed to an `out` port); a zero-length array is exempt. Whole-array granularity is looser than the spec: an array is `K` carriers (§5.1.4), §3.3.4 exempts only "a carrier an undelayed definition drives" or "a signal whose `out` port in a child holds the initial value", and §8.2.20 says "A level no definition names holds the carrier's initial value". The elements no driver names have neither. Finding 10.
- **Fixture, README and part VII edits.** Legitimate spec-following: each edited declaration was an array (or an `out` array port) of `bool`/`u8`/`u32` carriers with no initial value and no driver, which §3.3.4 refuses; the edits add `= [false; N]`/`[0; N]` or a `Default` bound (`README.md` `Delay<T: Repr + Default, ..>`, whose `r: [Bit<T>; N]` has none; part VII §22.1.3's `sh` fragment). No assertion was relaxed: each edited test keeps its expected values and codes; `a_definition_and_commits_of_one_element_are_e0221` moved the initial value only into the case with no definition (E0338 forbids it beside one). The two cases that stay clean without an initial value are the ones finding 10 names.
- **Stale generation rows in `types_buck`/`types_subset`.** Those tables list forms still refused with `P0001`; generation now types (T77, T80), so the rows had to leave (they would assert a refusal that no longer happens). Each removal leaves a comment naming the target that now proves the form (`types_generation`). Legitimate.
- **Finding 6, F.3's refusals.** Traced in `docs/spec/appendix_f_gallery_mixed.md:262-282`: `BehavioralCell(c: Cell)` probes `i(c.bl)`/`v(c.bl)` and reacts on `c.wl.fall()`; `c` is the bundle `Cell` (`in wl: Bit, bl: Electrical`), so the probe "through an access that names several natures" and the "method `fall` on this receiver" are both reads through members of the refused bundle. They follow from L25 as the pin says. They are `P0001` (not supported), not user errors, so they are not the wrong-code cascade finding 3 removed; their wording names the access, not the bundle (noted, no finding).

**Own examples at bc953965** (throwaway target, deleted):

- `signal b: [Bit; 4]; b[0] = clk;` and a reaction reading `b[2]`: clean, elaborates. `b[1]`, `b[2]`, `b[3]` have no driver and no initial value. **Fails** (finding 10).
- `module Cell(in d: Bit, out q: Bit) { q = d; }`, `signal y: [Bit; 4]; let c = Cell(clk, y[2]);` and a reaction reading `y[0]`: clean. **Fails** (finding 10).
- `signal b: [Bit; 4];` committed only at `b[1]` in a reaction: `E0187` naming `b`. Holds.
- `[[Bit; 2]; 3]` with no initial value: `E0187`. `[Bit; 0]`: clean. `[Bit<T>; 2]` in `M<T: Repr>`: `E0187` naming `T`. A whole array handed to `Reg(.., out q: [Bit; 4])`: the port `q` is `E0187` in the child, the parent's array is not. Hold.
- A generic function `apply<F: Fn(f64) -> f64>` called with a closure and with a function name: clean (the template-parameter `P0001` of finding 2 is confined to modules). Holds.
- `signal s: [Bit; 2]; s = x;` (a whole undelayed definition): `P0001` (a whole-array definition is not supported) **and** `E0187` on `s` - the refused definition is not counted as the driver, so a cascade is reported beside the `P0001` (finding 11).

### Checks changed in round 2

| Check | Test | Holds |
| --- | --- | --- |
| C75 | `session.rs` (api) `a_structural_param_overrides_before_the_run_and_is_refused_during_it` | yes - override, `set` and `dc` sweep, each message naming `Top.count` as structural |
| C78 | `types_generics.rs` `phase_d_and_e_forms_stay_p0001_by_name` | yes - template, placement through one, and a given or inferred `C: Fn(..)` all `P0001` |
| C82 | Book run tests; `std_galleries.rs` `the_fences_generics_phase_c_lifted_are_checked` | yes - the seven fences pinned to their exact ten remaining refusals, each owned (L25, L32) |
| C120 | `book_cheatsheet` `an_array_of_bits_with_no_initial_value_is_e0187`; `book_natures` `the_slice_listing_bounds_t_by_default_as_gallery_e6_does`; `book_core_std` `a_carrier_port_left_out_is_e0312_as_the_old_register_listing_was` | yes for the cases it names (only code `E0187`, message names `s`/`buf`); the partially driven array is not covered (finding 10) |
| C127 | the five remaining re-pins plus `book_generics`, `book_welcome`, `cli/book` | yes - proofs green, exact lists |

### Round 2 findings

| # | Severity | Finding | Evidence | Failure scenario | Fix | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| 10 | Medium-High | An array of carriers with no `Default` and no initial value is exempted from `E0187` as a whole when one element has an undelayed definition or is handed to an `out` port; its other elements start silently at an unstated value | `types/module.rs:548` (`undelayed_elements.contains(&decl)` and `handed.iter().any(.. carrier == decl)` test the array, not the element); `types_carrier_arrays.rs:170` pins `bits[0] = true;` + `bits[1] <- false;` clean (`bits[1]` is commit-driven with no initial value), `:193` pins `Cell(x[1], y[2])` clean; `crates/piperine-parity/tests/carrier_arrays/arrays.msl:29,39,41` reads and commits `lines[1]`, which nothing initialises (only `lines[3]` is handed); probes above | a user defines `b[0] = clk;` and reads `b[2]`; §3.3.4 and §8.2.20 say `b[2]` has no value and the model must be refused, Piperine runs it from some default; `GAPS.md` §3 says no silent row is open. The fix also meets a spec tension: E0338 forbids any initial value on an array with an undelayed element definition, so the other elements cannot be given one | either judge per element (an element no driver names needs an initial value, and §8.2.21's "on the levels defined" lets an initial value cover the rest - which needs E0338 narrowed to the defined elements), or record the hole as a silent row/spec disagreement with an owner and a user decision; then pin `bits[2]`/`y[0]`/`lines[1]` to the chosen outcome | 3db042d5 - per element (`types/coverage.rs`): `E0187` names the elements no driver names, an initial value is the value of the elements left and `E0338` only once all are defined (AFK default 2026-11-24, pending user review); the three pinned cases moved, partial cases pinned in `types_carrier_arrays.rs` |
| 11 | Low | A whole-array undelayed definition is `P0001` and also `E0187` on the array: the refused definition is not counted as the driver | `signal s: [Bit; 2]; s = x;` gives both | the user is told the array lacks an initial value although the definition (refused as unsupported) is its driver - the cascade class of finding 3 | record the whole definition as the driver before refusing it, or suppress `E0187` on a carrier whose drive was refused | ef108666 - a refused whole-array or sub-array drive counts as the driver |

## Round 3 (c94a26d1)

**Proofs**: `check_proofs.py` on `tasks-c.md` at c94a26d1 (scratch dir, deleted): 89 pass, 0 fail, tree clean, build and hygiene pass.

**Own examples at c94a26d1** (throwaway target, deleted). Fixed length, now per element: `b[0] = clk` on `[Bit; 4]` is `E0187` "`b[1..4]`"; `Cell(clk, y[2])` is `E0187` "`y[0..2], y[3]`"; a hand-over of `y[0]` and a commit of `y[1]` is `E0187` "`y[1]`"; `= [false; 4]` beside `b[0] = clk` is clean; every element defined with an initial value is `E0338`; all elements handed is clean. Hold. A run-time index in a definition or a connection (`b[at] = clk`, `Cell(clk, y[at])`) is `E0188` at elaboration (not silent). Holds. Then:

- `module M<const N: usize>(in clk: Bit) { signal b: [Bit; N]; b[0] = clk; }` reading `b[N - 1]`, placed as `M::<4>`: check_model, check and elaboration all clean. **Fails** (finding 12).
- `module M(in clk: Bit) { n: usize = 4; signal b: [Bit; n]; b[0] = clk; }`: clean, body `M~n=4`. **Fails** (finding 12).
- `module M<const N: usize>(..) { signal y: [Bit; N]; let c = Cell(clk, y[0]); }` at `M::<4>`: clean. **Fails** (finding 12).
- `signal y: [Bit; 4]; for k in 0..2 { let c = Cell(clk, y[k]); }` (a fixed length, a generation index): clean; `y[2]`, `y[3]` have no driver. **Fails** (finding 12).
- `signal y: [Bit; 2] = [false; 2]; let c = Cell(clk, y[0]);` plus a commit `y[1] <- true`: `E0223` on `y`; without the initial value `E0187` "`y[1]`". No spelling of this shape exists for a non-`Default` type (finding 13).

**The coordinator's questions:**

- **(a) E0223 whole-array vs narrowed E0338.** Inconsistent with the amendment. §8.2.21 was read "on the levels defined" so an initial value is the value of the elements no undelayed definition names; §5.2.6 states the same kind of rule for a hand-over ("the instance holds the drive, so the declaring module may give none"), and the instance holds the drive of the handed element only. Keeping `E0223` whole-array means an array with one element handed to a child and another driven by the parent (commit) cannot be written for a non-`Default` type at all: no initial value is `E0187` on the committed element, an initial value is `E0223`. That shape ran before this round: `arrays.msl` `Toggles` handed `lines[3]` and committed `lines[1]`; 3db042d5 moved the commit to a separate array `own` because it no longer has a spelling. Finding 13.
- **(b) Run-time index and generic length.** A run-time index is not a hole: a definition or a connection at one is `E0188` at elaboration (above). A generic or param length is a hole: DC11 says "a length not known yet (generic) is judged as before: one element drive stands for all", and nothing re-judges the coverage per instance at elaboration, so `M::<4>`, `M~n=4` and a generation loop that names some elements run with undriven elements. These are the forms phase C delivers (generic modules, structural params, generation). Not spec-justified: §3.3.4 as amended this very round says the rule holds per element. Finding 12.
- **(c) Moved fixtures.** The asserted values are kept: `source_carrier_arrays.rs` `an_element_given_to_an_out_port_is_driven_by_the_child` still asserts `l3 == x[0]`'s parity and `l1 == n % 2 == 1` (now read from `own[1]`, the same toggle); `hoisted_out.msl` holds `grid[0]` with `Idle::<3>` at `false` and `a_generic_module_takes_a_whole_array_and_a_window_of_one_at_a_port_of_length_rows` keeps "row 0 keeps its first value". `elab_carrier_arrays.rs` adds the hand-overs that cover the remaining rows; its window assertions are unchanged. What was lost is the mixed drive of one array (finding 13), not a value.

### Round 3 findings

| # | Severity | Finding | Evidence | Failure scenario | Fix | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| 12 | High | Per-element `E0187` is skipped for an array whose length is a generic argument or a param, and for elements named through a generation index: one element drive still stands for the whole array, and no elaboration-time check follows | `types/coverage.rs` `ElementCoverage::cover` (`Element::At(_) | Element::Run(..) if self.total.is_none() => self.everything = true`); DC11 in `design-c.md` ("judged as before"); probes above (`M::<4>`, `M~n=4`, `Cell(clk, y[0])` in `M<N>`, `for k in 0..2` over `[Bit; 4]`) | a user writes a generic bank `[Bit; N]` and drives row 0; at `N = 4` rows 1 to 3 have no value and the model runs from an unstated one - the silent behaviour finding 1 and 10 were about, now confined to generic modules and generation, the forms this phase ships; `GAPS.md` §3 says no silent row is open | re-judge coverage per instance at elaboration where lengths and generation indices are folded (the elaborator already folds both), raising `E0187` naming the elements; or keep the checker's deferral and record a silent row with an owner. Pin `M::<4>`, `M~n=4` and the partial generation loop || 727c967c (checker defers `Element::Unfolded` and generic lengths; `elab/element_coverage.rs` judges `E0187`/`E0223` per instance) and 4f50259c (`E0338` per instance); `elab_carrier_arrays.rs` per-instance tests; round 4 probes hold |
| 13 | Medium | `E0223` stays whole-array while `E0338` was narrowed, so an array with some elements handed to children and others driven by the parent has no spelling for a non-`Default` type | `types/module.rs:511` (`has_initial(decl)` on any handed element); DC11 "E0223 stays whole-array"; `arrays.msl` `Toggles` lost its mixed `lines[1]`/`lines[3]` drive in 3db042d5 | the CV-25 shape "the child drives one element, the parent the others" is refused either way (`E0187` or `E0223`) for `Bit`, `Bit<u8>` and every type without `Default` | narrow `E0223` as `E0338` was: an initial value is the value of the elements no `out` port holds, `E0223` only when every element is handed (amend §5.2.6 with §8.2.21, same AFK note), and restore the mixed `lines` case; or have the user decide | 727c967c (code), a92693fb (fixture, spec); C129; `Toggles` hands `lines[3]`, commits `lines[1]`, keeps its initial value and its asserted values; §5.2.6 amended |

## Round 4 (ac587e0e)

**Proofs**: `check_proofs.py` on `tasks-c.md` at ac587e0e (scratch dir, deleted): 96 pass, 0 fail, tree clean, build and hygiene pass.

**Findings 12 and 13 (own examples at ac587e0e, throwaway target, deleted).** `M<const N>` with `[Bit; N]` and `b[0] = clk` at `M::<4>`: `E0187` "`b[1..4]` in `M::<4>`"; at `M::<1>` clean. A param length `[Bit; n]`: `E0187` "`b[1..4]` in `M~n=4`". `for k in 0..2 { Cell(clk, y[k]) }` over `[Bit; 4]`: `E0187` "`y[2..4]` in `Top`"; `for k in 0..N` over `[Bit; N]`: clean. An initial value with `y[0]` handed and `y[1]` committed: clean (the `Toggles` mixed case is restored in `arrays.msl` with its asserted values `l3`, `l1` unchanged); every element handed with an initial value: `E0223` (fixed length at check, generic length at `M::<3>`). `E0338` at `M::<1>` for `b[0] = a` with an initial value, clean at larger `N`. Hold.

**The E0335 change (4f50259c), probed for false negatives.** `element_of` gives the outermost level, generic or not, a scale of 0, so a constant index there names its element exactly; an inner generic level returns `Element::Unfolded`, and `Unfolded` overlaps everything (`element_target.rs:32`), which is conservative. Probes: generic 1-D `b[1]` twice: `E0335`; `b[0]`, `b[1]`: clean; outer-generic nested `g[1][0]` twice: `E0335`, `g[1][0]`/`g[0][1]`: clean; a definition and a commit of `b[1]` in a generic array: `E0221`; `b[1]` handed twice: `E0220`; `b[5]` at `N = 4`: `E0104` at elaboration. **No false negative found.** Two false positives remain (finding 15): the "judged at elaboration" claim for a nested generic inner length holds for coverage, not for `E0335`.

### Round 4 findings

| # | Severity | Finding | Evidence | Failure scenario | Fix | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| 14 | Medium-High | Generation still refuses, with `P0001`, a carrier, a definition and a hoisted placement (a module with a value input or a result) declared inside a generation `for`/`if`; §8.2.23 says an item declared in a generation `for` makes an array, GN-17 AC 1 ("each named item an array by ordinal") is marked delivered, and `GAPS.md` closed L31 with no row naming what is left | `crates/piperine-lang/src/elab/generation.rs:218-231` (`admit_generated`); recorded only in the B12 Handoff of `tasks-c.md` ("Refused loudly ... a carrier, a definition and a hoisted placement"); probes: `for k in 0..2 { signal s: Bit = false; }` is `P0001` "a carrier inside generation" plus a second `P0001` for `s[1] <-` (cascade); `for k in 0..2 { let y = idt(1.0); }` is `P0001`; `for k in 0..3 { y[k] = clk; }` is `P0001` "a definition inside generation" | a user writes the spec's generated array of signals (§8.2.23, spec.md:55 "generated arrays of signals and instances (GN-17)") and is told it is not supported, while GAPS and the Book say generation runs (GAPS §9 III §8 "generation ... run (phase C)") | implement the three forms, or record a loud row (owner, size) and narrow GN-17's delivered mark and the GAPS §9 sentence to what runs; pin each refusal by name; suppress the cascade `P0001` on a refused generated carrier | 856ff5dbc (fixer); C131/C132 |
| 15 | Low | `E0335` is a false positive on legal code in two shapes: a nested array whose inner length is generic (`g[1][0]` and `g[0][1]` of `[[Bit; N]; 2]`), and definitions inside generation that name disjoint elements (`for k in 0..2 { b[k] = a; } b[3] = !a;`, which is then reported `E0335` before the `P0001` of finding 14) | `types/element_target.rs:165-188` (`Unfolded` overlaps everything); probes above | a user is told two definitions share an element when they do not; the generation case shows a user error ahead of the real refusal | defer `E0335`/`E0221` for `Unfolded` elements to the elaborator, as coverage now is, or refuse the shape with `P0001` by name | 856ff5dbc (fixer); C132 |

## Round 5 (6c87900da)

**Proofs**: `check_proofs.py` on `tasks-c.md` at 6c87900da (scratch dir, deleted): 112 pass, 0 fail, tree clean, build and hygiene pass. Also run: `cargo test -p piperine-digital-solver -p piperine-plan`, all green.

**Findings 14 and 15 (own examples at 6c87900da, throwaway target, deleted).** `for k in 0..2 { signal s: Bit = false; }` with `s[1] <- true`: elaborates; without the initial value `E0187`. `for k in 0..2 { let y = idt(1.0); }`: elaborates. `for k in 0..3 { y[k] = clk; }` over `[Bit; 4]`: `E0187` "`y[3]` in `Top`"; `0..4`: clean. Generated `y[k]` plus `y[1]`: `E0335` "`y[1]` in `Top`"; plus `y[3]`: clean; `y[0]` in every iteration: `E0335`. `g[1][0]`/`g[0][1]` of `[[Bit; N]; 2]` at `N = 3`: clean; `g[1][0]` twice: `E0335`. A generated hand-over plus a second of `y[2]`: `E0220`; a generated definition and a commit of `y[1]`: `E0221`. `y[2 * k]`, `y[2 * k + 1]`: clean. A generated `let w` read outside its loop as `w[2]`: elaborates. Hold.

**The digital-solver ordering (`NetPart`, 20b3624e6).** `Dag::successors` adds an edge from a writer to a reader only when the parts overlap; a whole read or write of a net (`NetPart::whole`, `elements: None`) overlaps every part, so the change can only remove edges between disjoint elements of one array net. Read parts come from the function's `elements` list (abi.md §3: one entry per read cell, `WHOLE` for a whole array, empty meaning every read is at element 0), indexed over all cells before the `PIN` filter, which matches "one entry per `reads` entry"; a window binding offsets the element by `first`. Writes are whole unless the pin is bound to an element or a window. Whole-array and run-time-index reads in function code are now lowered to one constant read per element (`elements_of_array`), so no function reads an array net through a single element-0 cell by accident. I found no path that drops a needed edge; the digital-solver and plan suites are green. The cost of that lowering is finding 17.

**The mirror carrier.** `elements.rs:258-266` makes a carrier `<array>$analog[<at>]` per element the `analog` block reads. It is an ordinary carrier of the body, so the host lists it: `piperine run` of `module Top() { signal x: [Bit; 2] = [true, false]; signal a: Electrical; }` / `analog for Top { v(a) +> if x[0] { 1.0 } else { 0.0 }; }` prints `Top.x$analog[0]  true` in the operating-point table beside `Top.x` and `Top.a`. Finding 16.

**L76 and L77 owners.** L76 (an item of a generation `if` that only some iterations take: what a missing element reads) is a spec question; `spec-closure` fits. L77 (generation in the body of a hoisted module) has no open spec question: §8.2.7 and §8.3.12 both apply as written, it is an implementation residue of this phase's generation and hoisting. Finding 18.

### Round 5 findings

| # | Severity | Finding | Evidence | Failure scenario | Fix | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| 16 | Medium-High | The analog block's per-element mirror carrier `<array>$analog[k]` is an ordinary carrier of the body, so the host shows it to the user as a signal of the design | `crates/piperine-lang/src/elab/elements.rs:258-266`; `piperine run mirror.msl --analysis op` prints `Top.x$analog[0]  true` | every design whose `analog` block reads an element of an array of carriers lists an internal net the user never wrote, in the op table (and so in whatever lists the top's signals: probes, records, the API's discrete reads); an internal transform leaks into the host surface | mark the mirror as internal (not a signal of the top: no op row, no probe name, no record), or read the element on the analog side without a carrier; pin that the op table lists only `Top.x` and `Top.a` || 1fd0c0566 (`CarrierRole::Internal`: no default probe, op row, metadata entry or host name); round 6: op table, transient table, CSV, SVG and JSON Lines records of a design reading `x[0]` in `analog` hold no `$analog` name (finding 19 for an explicit `--print`) |
| 17 | Medium | A read of an array of carriers at a run-time index in a definition (`q = mem[at]`) lowers to one constant read per element of the array | `elements.rs` `element_part` -> `elements_of_array` in function code; `signal mem: [Bit<u8>; 1024] = [7; 1024]; signal at: Bit<u16> = 3; signal q: Bit<u8>; q = mem[at];` runs its op in 6.5 s; at 65 536 elements `piperine run` had not finished compiling after 300 s (killed) | a RAM read port written as a definition does not build at a realistic size; before 20b3624e6 it was a plan panic (loud), now it is an unbounded build | read the array through one `WHOLE` cell (abi.md R2 already defines the view) and order the function after every element's writer by a whole `NetPart`, or refuse the form above a size with `P0001` by name; add a size test || 1fd0c0566 (one `WHOLE` cell for a run-time element in a function, abi.md §3 amended, `validate.rs` admits `FunctionRead`; ordered after every element's writer); `ram.msl` 65 536 elements (C137) |
| 18 | Low | L77's owner is `spec-closure`, but it is not a spec question: generation in the body of a hoisted module is an implementation residue of `generics` (generation, hoisting) | `GAPS.md` L77 row | the row lands on a lane that decides spec text, not on the feature whose forms it completes | owner `generics` (phase D or E, with a task), or state the spec question it waits on || 1e46f8050 (L77 owned by `generics` phase E, T118) |

## Round 6 (12fc93043)

**Proofs**: `check_proofs.py` on `tasks-c.md` at 12fc93043 (scratch dir, deleted): 117 pass, 0 fail, tree clean, build and hygiene pass.

**The mirror, everywhere a user sees.** Model: `signal x: [Bit; 2] = [true, false]; signal a: Electrical;`, `analog { v(a) +> if x[0] { 1.0 } else { 0.0 }; }`, a commit of `x[0]` at 1 us. `piperine run --analysis op`: rows `Top.clk`, `Top.x`, `Top.a` only. Transient with `--csv`, `--plot`, `--records`: the CSV header is `time,Top.clk,Top.x[0],Top.x[1],Top.a`, and none of the three files holds `$analog`. Metadata lists `In`/`Out`/`Reached` carriers only; the host's `drive` and `source` lookups skip `Internal`. An explicit `--print 'x$analog[0]'` (or `Top.x$analog[0]`) still resolves and prints the mirror (finding 19). `Session::discrete` reads a net by its path, so only a caller who already has the internal name reaches it.

**The ABI amendment (abi.md §3, `WHOLE` in a function's read).** Consistent with R2 (the `ArrayView` a `WHOLE` read cell holds is already defined for reactions), edited in place under major 1 as the user decided for this ABI. `validate.rs` admits `WHOLE` for `FunctionRead` only and still refuses it for a write. The callers of discrete functions handle it: the digital solver builds the view per call (`WholeReads::windows_of`), and the generated device's thunk sizes cells through `cell_lens_of` with the elements list. The analog solver and sensitivity call functions with rows or partials, which a discrete definition does not have. The dag reads a `WHOLE` entry as the whole net (`NetPart::of` drops it to `elements: None`), which overlaps every element's writer, so the read orders after all of them.

**False cycles with a `WHOLE` read.** `b[0] = 9; q = b[at];` orders correctly (`q` = 5 at `at = 2`, `b` = `[9, 5, 5, 5]`). `b[1] = b[at] + 1` (one element written, the array read whole) and `b[0] = c[at]; c[1] = b[at] + 1` are refused by the digital bind: "the compiled design cannot bind: Digital: bind rule cycle broken: Top.drive:b[1]" (and `...b[0]; Top.drive:c[1]`). That is loud, and it names the drives. Both are possible loops, since `at` may select the written element, so the conservative edge is sound. The same refusal meets a loop between constant elements (`b[1] = b[2] + 1; b[2] = b[1];`), which is composite-values' code on main, while a scalar loop (`p = r + 1; r = p;`) is a type-check error. Finding 20.

### Round 6 findings

| # | Severity | Finding | Evidence | Failure scenario | Fix | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| 19 | Low | `--print` resolves an internal mirror by its name | `piperine run mirror.msl --analysis op --print 'x$analog[0]'` prints `Top.x$analog[0]  true`; `Probe::resolve` does not skip `CarrierRole::Internal` (only the default list does) | a user who guesses or reads (IR print) the name sees an elaborator artifact as a signal; nothing lists it, so it is not discoverable by accident | have `Probe::resolve` refuse an `Internal` carrier as "names no signal of the top" | not blocking; for the next `generics` phase |
| 20 | Low-Medium | A loop through elements of an array of carriers is refused by the digital bind after rustc compiles the device, not by `E0339` (§8.2.3, "a loop through a one-driver carrier", elaboration) | `b[1] = b[2] + 1; b[2] = b[1];` and `b[1] = b[at] + 1` both end in "Digital: bind rule cycle broken: Top.drive:b[1]..."; the scalar form is refused by the checker | the user waits for a full compile and gets a solver bind message with internal drive labels instead of the language code; the class predates this phase (composite-values' element definitions), and this phase's run-time-index reads only reach it through conservative edges that are sound | raise `E0339` naming the carrier and the elements at elaboration, where element drives are known; record a GAPS row (owner `composite-values` or `spec-closure`) until then | not blocking; route to the class owner |

## Final suite (after the round 6 PASS)

**Merge**: origin/main 7bf501f0e merged into `feature/generics` at 41cb566b3. origin/main 862aecd2 (composite-values phase D) had never been merged into the branch, so the merge carries phase D's code, not docs only. Conflicts resolved:
- `GAPS.md` counts: 0 silent, 2 broken, 17 loud. L13 is closed by composite-values D; S3, L30, L31, L69 and L70 are closed and L76 and L77 found by generics C. There are 17 loud rows in §4.
- the `docs.rs` count assertions follow the same counts.
- `elab/consts.rs` declares both `floats` and `generated`.
- `types/mod.rs` exports both `ElementCoverage` and `FloatText`.
- `types_subset.rs`: the `f32`, `Float<N>` and generation rows are all gone.

**Run** (PIPERINE_NGSPICE unset; `/var/tmp/piperine-tmp-d` cleared first, `helpers/` kept):
- `cargo build --workspace --exclude piperine-lang-server --all-targets`: exit 0, 0 warnings.
- Tests, package by package:

| Package | Passed | Failed |
| --- | --- | --- |
| piperine-abi | 58 | 0 |
| piperine-analog-solver | 109 | 0 |
| piperine-api | 295 | 0 |
| piperine-digital-solver | 160 | 0 |
| piperine-interp | 71 | 0 |
| piperine-ir | 157 | 0 |
| piperine-plan | 64 | 0 |
| piperine-solver | 401 | 0 |
| piperine-numbers | 22 | 0 |
| piperine-transpile | 165 | 0 |
| piperine-project | 53 | 0 |
| piperine-lang | 1375 | 0 |
| piperine-cli | 198 | 0 |
| piperine (root, `--no-fail-fast`) | 419 | **1** |

- `piperine-parity -- --nocapture`: 518 passed, 0 failed.
- The seven buck targets (`buck_whole`, `buck_dpwm`, `buck_control`, `buck_boundary`, `buck_power`, `buck_supervisor`, `buck_closed_loop`): 23 passed.
- `buck_scenarios`: 61 passed.
- cli `twins`: 6 passed.
- No release profile was built.

**Red: 1.** `piperine --test book_compiler` `a_compiler_code_says_what_the_chapter_says` fails with "compiler~79f44db1: now runs: make the row `run` and drop its `GAPS.md` section 11 line". The test shows `P0001` with a generic module (`module Gen<const N: u32>() { }` placed as `Gen::<2>`). Phase C makes that run, so the Book claim "P0001" needs another unfinished form as its example (a template, phase D). The same test text is on both sides of the merge, so this was red on the branch before the merge too. No phase C check covered `book_compiler`; my targeted rounds missed it. Not fixed, per the gate policy.
