# Unknown Semantics verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 2013bd1f (branch HEAD 1480cb15)
**Verifier**: independent sub-agent (author != verifier)
**Full suite (foreground, verifier's own run at 1480cb15)**: `env -u PIPERINE_NGSPICE cargo test --no-fail-fast` - exit 0, 261 targets, 2394 passed, 0 failed, 3 ignored (pre-existing `ignore` doc-tests of piperine-solver)

Own examples (throwaway test in `piperine-parity`, not committed), run at HEAD:

- nested unknown control, `if d { q <- 3; if e { q <- 5; } }` with `q` held 5, `d`, `e` unknown: expected unknown at 0.5 ns, got **no change** (F1); the same with `else { q2 <- 5; }` and under an unknown guard `on clk.rise() && en { g <- 3; if e { g <- 5; } }`: no change, expected unknown (F1)
- `match sel { 0 => w <- 1, 1 => w <- 1, _ => w <- 2 }`, `sel` unknown: unknown, correct
- `cast::<u8, i8>(200)`: bytes `[200, 1]` (-56), correct
- `fn noisy() -> bool { emit!(..); true }` called in value position in a reaction (`q <- noisy()` and `u && noisy()`): `P0004 internal error: an emit outside a reaction` (F3)

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `crates/piperine-parity/tests/source_unknowns.rs` `an_unknown_initial_value_starts_every_level_unknown` | yes |
| C2 | `crates/piperine-parity/tests/source_unknowns.rs` `an_unknown_initial_value_fires_no_change_at_zero` | yes |
| C3 | `crates/piperine-parity/tests/source_unknowns.rs` `a_const_holding_unknown_reads_unknown_everywhere` | yes |
| C4 | `crates/piperine-lang/tests/elab_unknowns.rs` `unknown_deciding_structure_is_e0151` | yes |
| C5 | `crates/piperine-parity/tests/source_unknowns.rs` `committing_unknown_makes_every_level_unknown` | yes |
| C6 | `crates/piperine-parity/tests/source_unknowns.rs` `level_reads_each_level_of_a_known_value` | yes |
| C7 | `crates/piperine-parity/tests/source_unknowns.rs` `level_with_a_known_index_is_known` | yes |
| C8 | `crates/piperine-lang/tests/elab_unknowns.rs` `width_is_the_level_count_of_each_repr_type` | yes |
| C9 | `crates/piperine-parity/tests/source_unknowns.rs` `level_out_of_range_is_a_panic_naming_k` | no - asserts `level 8` and `out of range` only; AC 4 also requires the message to name the expression (the origin is printed but not asserted) (F7) |
| C10 | `crates/piperine-parity/tests/source_unknowns.rs` `level_with_an_unknown_index_merges_every_level` | yes |
| C11 | `crates/piperine-parity/tests/source_unknowns.rs` `merge_of_equal_known_values_is_that_value` | yes |
| C12 | `crates/piperine-parity/tests/source_unknowns.rs` `merge_of_different_values_is_unknown_and_commutative` | yes |
| C13 | `crates/piperine-parity/tests/source_unknowns.rs` `same_compares_flags_and_values` | yes |
| C14 | `crates/piperine-parity/tests/source_unknowns.rs` `cast_keeps_each_level_and_its_flag` | yes - though 5 cannot tell reinterpretation from value conversion (probe: `cast::<u8,i8>(200)` gives -56, correct) (F5) |
| C15 | `crates/piperine-parity/tests/source_unknowns.rs` `cast_to_a_tag_with_no_variant_is_unknown` | no - `cast::<u2, Mode>(bad).known_or(Mode::Idle) == Mode::Idle` is also true if the cast gave a known `Idle`; nothing asserts the result is unknown (F5) |
| C16 | `crates/piperine-parity/tests/source_unknowns.rs` `logic_builds_known_and_unknown_values` | yes |
| C17 | `crates/piperine-parity/tests/source_unknowns.rs` `matches_bits_follows_its_table` | yes |
| C18 | `crates/piperine-lang/tests/types_unknowns.rs` `cast_between_widths_is_e0701` | yes |
| C19 | `crates/piperine-lang/tests/types_unknowns.rs` `a_bad_pattern_is_e0702` | yes |
| C20 | `crates/piperine-parity/tests/source_unknowns.rs` `an_unknown_if_runs_both_arms_without_a_fault` | yes |
| C21 | `crates/piperine-parity/tests/source_unknowns.rs` `both_arms_committing_commit_their_merge` | yes |
| C22 | `crates/piperine-parity/tests/source_unknowns.rs` `one_arm_committing_merges_with_the_value_then` | partly - the one-arm merge with the held value is proven; the check's clause "another reaction's commit in the same microstep included" is not exercised by this test (F4) |
| C23 | `crates/piperine-parity/tests/source_unknowns.rs` `equal_known_commits_stay_known` | yes |
| C24 | `crates/piperine-parity/tests/source_unknowns.rs` `an_if_expression_and_an_if_statement_commit_the_same` | yes |
| C25 | `crates/piperine-parity/tests/source_unknowns.rs` `an_unknown_match_merges_every_arm` | partly - arms commit 1/2/3, so skipping the wildcard arm still gives unknown; the wildcard's inclusion is not proven (F6; behaviour verified correct by probe) |
| C26 | `crates/piperine-parity/tests/source_unknowns.rs` `a_panic_under_unknown_control_makes_the_merge_unknown` | yes |
| C27 | `crates/piperine-parity/tests/source_unknowns.rs` `short_circuit_operands_follow_the_merge` | yes |
| C28 | `crates/piperine-parity/tests/source_unknowns.rs` `an_unknown_guard_runs_the_reaction_as_a_maybe` | yes |
| C29 | `crates/piperine-parity/tests/source_unknowns.rs` `definitions_lets_and_functions_merge_alike` | yes |
| C30 | `crates/piperine-lang/tests/spec_text.rs` `appendix_c_states_the_loop_bound_under_unknown_control` | yes |
| C31 | `crates/piperine-cli/tests/docs.rs` `gaps_carries_the_unknown_index_merge_to_composite_values` | yes |
| C32 | `crates/piperine-parity/tests/source_unknowns.rs` `an_unknown_delay_under_unknown_control_commits_unknown_next` | yes |
| C33 | `crates/piperine-parity/tests/source_unknowns.rs` `a_record_under_unknown_control_is_maybe` | yes |
| C34 | `crates/piperine-cli/tests/run.rs` `a_maybe_record_prints_maybe_and_a_known_one_does_not` | yes |
| C35 | `crates/piperine-parity/tests/source_unknowns.rs` `a_maybe_finish_does_not_end_the_run` | yes |
| C36 | `crates/piperine-parity/tests/source_unknowns.rs` `a_maybe_check_is_judged_as_unknown` | yes |
| C37 | `crates/piperine-parity/tests/source_unknowns.rs` `a_record_field_keeps_its_flag` | yes |
| C38 | `crates/piperine-parity/tests/source_unknowns.rs` `a_math_function_of_an_unknown_value_is_unknown` | yes |
| C39 | `crates/piperine-parity/tests/source_unknowns.rs` `known_or_with_an_unknown_default_gives_that_default` | yes |
| C40 | `crates/piperine-lang/tests/elab_unknowns.rs` `analog_reading_a_maybe_unknown_value_is_e0150` | partly - covers contribution, `known_or` default, math, `ddt`, `ac_stim` through a `const`; AC 3 also names `delay`, `limit`, `noise`, and the literal `Unknown` in `analog` is still P0001 (F2, F8) |
| C41 | `crates/piperine-lang/tests/types_unknowns.rs` `a_flag_total_function_may_take_a_carrier_in_analog` | yes |
| C42 | `crates/piperine-cli/tests/docs.rs` `gaps_drops_l18_and_l19` | yes |
| C43 | `crates/piperine-lang/tests/spec_text.rs` `appendix_c_lists_the_loop_bound_among_the_defaults` | yes |
| C44 | `crates/piperine-cli/tests/book.rs` `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes |
| C45 | `crates/piperine-parity/tests/source_unknowns.rs` `neither_arm_committing_commits_nothing` | yes |
| C46 | `crates/piperine-digital-solver/tests/instant.rs` `a_merge_commit_equal_to_the_value_fires_no_change` | yes |
| C47 | `crates/piperine-parity/tests/source_unknowns.rs` `merge_of_a_value_with_itself_is_that_value` | yes |
| C48 | `crates/piperine-parity/tests/source_unknowns.rs` `the_pfd3_reaction_keeps_state_unknown_until_a_known_commit` | yes |
| C49 | `crates/piperine-digital-solver/tests/queue.rs` `a_maybe_commit_converts_the_pending_it_would_remove` | yes |
| C50 | `crates/piperine-parity/tests/source_unknowns.rs` `delays_one_tick_apart_are_one_key` | yes |
| C51 | `crates/piperine-transpile/src/rt` `rt::control::tests::choose_runs_one_arm_or_merges_both` | yes |
| C52 | `crates/piperine-transpile/src/rt` `rt::commits::tests::a_join_merges_keys_by_carrier_and_due` | yes |
| C53 | `crates/piperine-transpile/tests/reaction.rs` `an_unknown_condition_merges_both_arms` | yes |
| C54 | `crates/piperine-abi/tests/layouts.rs` `merge_keeps_equal_known_fields_and_clears_the_rest` | yes |
| C55 | `crates/piperine-abi/tests/caller.rs` `a_host_without_commit_with_is_a_named_fault` | yes |
| C56 | `crates/piperine-ir/tests/digital.rs` `the_unknown_nodes_type_print_and_check` | yes |
| C57 | `crates/piperine-interp/tests/frozen.rs` `the_unknown_nodes_are_refused_by_name` | yes |
| C58 | `crates/piperine-plan/tests/digital_shape.rs` `every_record_layout_ends_with_maybe` | yes |
| C59 | `crates/piperine-api/tests/records.rs` `maybe_is_decoded_beside_the_fields` | yes |
| C60 | `crates/piperine-api/src/verdict.rs` `verdict::tests::a_maybe_fatal_is_not_fatal` | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (Major) Nested unknown control loses an earlier commit of the same path: a join reduces each path to its last commit per key, so a MERGE commit made by an inner join no longer merges with the plain commit the same path made before it; §3.3.3 is violated with every test green | own example above: `if d { q <- 3; if e { q <- 5; } }` over a held 5 stays 5 (expected merge(merge(5,3),5) = unknown); same for both-arms and for an unknown guard (`Commits::run`); `rt/commits.rs` `Paths::into_effects` keeps `last[side]` only | inside a path, resolve a key's MERGE commit against the earlier commit of the same key on that path (merge the bytes, drop MERGE) before the outer join; add the three cases to `rt::commits` tests and to `reaction_merge.msl`/`guards.msl` | 8fd18bc0; `rt::commits::tests::a_nested_merge_commit_merges_with_the_earlier_commit_of_its_path`, `source_unknowns.rs` `a_nested_one_armed_commit_merges_with_the_path_before_it` |
| F2 (Major, ruled: code follows spec) The literal `Unknown` in an `analog` block is `P0001`, not `E0150` (US-06 AC 3, goal 3, §3.3.5) | `crates/piperine-lang/src/types/expr.rs:410` (`unsupported("`Unknown` literal in an `analog` block")`), pinned by `crates/piperine-lang/tests/types_subset.rs:49` row "Unknown"; C40's test reaches E0150 only through a `const` | report `E0150` naming `Unknown` in `types/expr.rs`; move the `types_subset.rs` row out of the P0001 table into an E0150 assertion (`elab_unknowns.rs` `ANALOG_FORMS` gains `v(p) +> Unknown;`) | 2defd47c; `elab_unknowns.rs` `analog_reading_a_maybe_unknown_value_is_e0150` (the `Unknown` row) |
| F3 (Major, ruling: diverges from spec) A function that emits and returns a value, called in value position in a reaction, is the internal error `P0004`; §19 rule 10 makes such a `fn` callable wherever `emit!` is (a reaction), and the only error for a wrong position is `E0122`. It also limits US-04 AC 8 ("a right operand that can emit"): `u && noisy()` fails to elaborate | own example above, both under known control (`q <- noisy()`) and as a right operand; `piperine-ir/src/check.rs:869` "an emit outside a reaction" surfacing as P0004 from `elab/elaborator.rs:130` | inline an emitting function at a value-position call inside a reaction (as statement calls are), so its `emit!` runs against `commits_`; add a source-level test for `q <- noisy()` and `u && noisy()` (maybe record on the merged path). If deferred, a GAPS row with a spec code, never P0004 | 2b9d1ce9; `source_unknowns.rs` `a_function_that_emits_runs_in_a_value_position_of_a_reaction` |
| F4 (Minor) C22 claims "another reaction's commit in the same microstep included", its test has none | `source_unknowns.rs` `one_arm_committing_merges_with_the_value_then` (single reaction); the C2-order case is only in `instant.rs` `a_merge_commit_merges_with_the_value_then` (one device) | add a source-level case where a plain commit lands before the MERGE commit in the same microstep, or reword C22 to what is proven | 8fd18bc0; `source_unknowns.rs` `a_merge_commit_meets_another_reactions_commit_of_the_same_microstep` |
| F5 (Minor) C15 holds on a wrong result; C14 uses a value that cannot tell reinterpretation from conversion | `contracts.msl` `missing = cast::<u2, Mode>(bad).known_or(Mode::Idle) == Mode::Idle`; `signed = cast::<u8, i8>(a)` with `a = 5` | assert `!cast::<u2, Mode>(bad).is_known()`; cast 200 and assert -56 | 0e3cc2f6; `cast_keeps_each_level_and_its_flag` (200 gives -56), `cast_to_a_tag_with_no_variant_is_unknown` (not known) |
| F6 (Minor) C25 does not prove the wildcard arm is taken | `reaction_merge.msl` `Matching`: arms 1/2/3 give unknown without the wildcard | arms `0 => 1, 1 => 1, _ => 2` and assert unknown | 8fd18bc0; `an_unknown_match_merges_every_arm` (arms 1, 1, 2) |
| F7 (Minor) C9 does not assert the expression is named | `level_out_of_range_is_a_panic_naming_k` asserts `level 8` and `out of range` only; the message carries `levels.msl:<line>:<col>` | assert the origin of `x.level(8)` in the message | 0e3cc2f6; `level_out_of_range_is_a_panic_naming_k` asserts `levels.msl:30:` |
| F8 (Minor) C40 covers `ddt` and `ac_stim` but not `delay`, `limit`, `noise` named by AC 3 | `elab_unknowns.rs` `ANALOG_FORMS`; `known_in_analog` doc lists "ddt, limit or ac_stim" | add rows for `delay`, `limit`, `noise` (E0150 naming the source) | 2defd47c adds `limit` (and the literal `Unknown`); `delay` and `noise` are P0001 outside the subset (`types_subset.rs` rows), so no E0150 can name them until they lower |
| F9 (Minor) Code rules: a god method grew and touched methods have no doc comment | `print/reaction.rs` `DigitalPrinter::reaction` 109 -> 147 lines; `elab/consts.rs` `literal`, `path`, `unary`, `binary`, `conditional` (signatures rewritten to `Folded`) and `piperine-plan/src/plan.rs` `ReadSet::union` (made `pub(crate)`) have no `///` | split the commits/guard preamble of `reaction` into its own method; add doc comments | eb5caa2f; `DigitalPrinter::reaction` split, twin and transpile goldens unchanged |

Notes (no finding): the rulings hold - `choose`/`branch`/`Commits::controlled` are exact on flat control (equal-known kept, differing unknown, panic unknown), guarded reactions read `Resolution` and merge exactly (`settles`), `Merge for Flagged<T: Levelwise>` compares f64 by bits and `Dual` value plus every partial, hoisted commit/emit values compile and merge (`hoisted.msl`), a joined key keeps each statement's own delay (`a_joined_key_keeps_the_delay_of_each_statement`), C.2/C.2.1 state 64 with `spec_text` guards. `Record::maybe`/`surely`, the verdict (maybe check fails, maybe fatal ends nothing), CLI `maybe=true` and JSON `"maybe":true` (unit test in `render/json.rs`) match US-05. ABI: `MINOR` 1, `Host.commit_with` at offset 16, `commit_flag` 1/2 and the minor-0 fault agree with abi.md §2, §3 and C7. Moved pins (T6 `MINOR` 0->1, T14 record sizes, T19 `an_unknown_condition_merges_both_arms`, T21, T30 `..._is_an_if`, T35 GAPS counts 49->47, T15 resolution pin) follow the spec, none weakens a criterion.
