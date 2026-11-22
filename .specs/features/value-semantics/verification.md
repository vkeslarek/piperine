# Value Semantics verification (MD-76, implemented on `feature/composite-values`)

**Verdict**: FAIL
**Proofs**: `proofs.md` at e7b208ad (41 pass, 0 fail)
**Fix round**: every finding is closed by the commit in its Closed column; `proofs.md` is rerun at HEAD by `check_proofs.py` (round 2, no second Verifier).
**Verifier**: independent sub-agent (author != verifier), 2026-11-22, branch `feature/composite-values` at e7b208ad

**The user's decisions, checked against the code** (throwaway sources run with the branch's `piperine` binary, Rust backend):

- No borrows, pointers or moves: `&`, `&mut`, `*x`, lifetimes and `[T]` are `E0101`/`E0905`/a parse error (C1 to C3); a value is read twice and copied (C14). Copies copy: `let b = a; b.x = 7;` leaves `a.x` 1, an array copy and a callee's rebound argument leave the original unchanged, in a reaction and in a `const fn`.
- No `Clone`/`Copy`: `#[derive(Repr, Clone, PartialEq)]` is `E0161` "cannot find `Clone`" with the help "every value is copied; delete the name"; `.clone()` is `E0161` naming the method and the type.
- `let mut` is `E0903` with the help "write the plain form `let x`: every local may be assigned"; every `let` is assignable (C8).
- Parameters, closure parameters, captures, body `let`s, params, consts and carriers are not places: `E0183` naming the target (a carrier: "it is a carrier, and only a local that a `let` or a pattern binds is a place").
- `resolve` const-generic in `N`: `std/nets.msl` `wired<const N: usize>(d: [Logic; N])`, `tri<T: Repr, const N: usize>`, `sum<const N: usize>` (C25). Running them is `resolved-nets`'.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `every_reference_form_is_e0101_once` (crates/piperine-lang/tests/msl_errors.rs:1086) | yes - proof green; the test asserts the code or value its criterion fixes |
| C2 | `a_lifetime_is_e0905_static_included` (crates/piperine-lang/tests/msl_errors.rs:1114) | yes - proof green; the test asserts the code or value its criterion fixes |
| C3 | `a_slice_type_does_not_parse` (crates/piperine-lang/tests/msl_errors.rs:1129) | yes - proof green; the test asserts the code or value its criterion fixes |
| C4 | `binary_ampersand_and_star_still_parse` (crates/piperine-lang/tests/msl_errors.rs:1139) | yes - proof green; the test asserts the code or value its criterion fixes |
| C5 | `the_markers_are_reserved_words_with_a_help_each` (crates/piperine-lang/tests/msl_errors.rs:329) | yes - one `E0903` at each word with its help (re-run by the Verifier: 'write the plain form `let x`: every local may be assigned') |
| C6 | `let_mut_in_a_module_body_is_e0903` (crates/piperine-lang/tests/msl_errors.rs:322) | yes - proof green; the test asserts the code or value its criterion fixes |
| C7 | `e0102_and_e0331_are_raised_nowhere` (crates/piperine-lang/tests/msl_errors.rs:348) | yes - proof green; the test asserts the code or value its criterion fixes |
| C8 | `a_let_without_mut_runs_in_a_function_a_reaction_and_analog` (crates/piperine-parity/tests/source_value_semantics.rs:107) | yes - `=` and every compound operator in a function, a reaction and `analog`, against Rust |
| C9 | `a_parameter_a_capture_and_a_body_let_are_not_places` (crates/piperine-lang/tests/types_value_semantics.rs) | yes - a fn parameter, a closure parameter, a captured name, a body `let`, a param, a `const` and a carrier (`E0183` "it is a carrier"); `self` is proven by `generics` when methods land (round 2, 8dc2aed3) |
| C10 | `an_assigned_loop_variable_does_not_change_the_loop` (crates/piperine-parity/tests/source_value_semantics.rs:125) | yes - the body sees i + 10 and the loop still counts four |
| C11 | `a_deferred_let_is_assigned_twice_and_never_read_first` (crates/piperine-lang/tests/types_value_semantics.rs:74) | yes - proof green; the test asserts the code or value its criterion fixes |
| C12 | `a_closure_keeps_the_value_it_captured` (crates/piperine-parity/tests/source_value_semantics.rs:136) | yes - the closure keeps 3, the local reads 10 |
| C13 | `a_callee_cannot_change_the_callers_local` (crates/piperine-parity/tests/source_value_semantics.rs:148) | yes - callee returns 6, the caller's local stays 5; the Verifier's own example (a callee that rebinds and bumps a struct) agrees |
| C14 | `a_value_is_read_twice` (crates/piperine-parity/tests/source_value_semantics.rs:157) | yes - two reads and a copy assigned, against Rust |
| C15 | `clone_and_copy_are_e0161_with_a_help` (crates/piperine-lang/tests/types_value_semantics.rs:141) | yes - `E0161` with the help 'every value is copied; delete the name' (re-run by the Verifier from a source file) |
| C16 | `an_enum_without_copy_is_carried_and_compared` (crates/piperine-parity/tests/source_value_semantics.rs:166) | yes - proof green; the test asserts the code or value its criterion fixes |
| C17 | `clone_as_a_method_is_e0161` (crates/piperine-lang/tests/types_value_semantics.rs:160) | yes - `clone()` is `E0161` naming the method and the type (re-run by the Verifier) |
| C18 | `a_range_index_is_an_array_of_the_folded_length` (crates/piperine-lang/tests/types_value_semantics.rs:99) | yes - proof green; the test asserts the code or value its criterion fixes |
| C19 | `a_range_bound_that_is_not_const_is_e0188` (crates/piperine-lang/tests/types_value_semantics.rs:108) | yes - proof green; the test asserts the code or value its criterion fixes |
| C20 | `a_range_index_out_of_the_array_is_refused` (crates/piperine-lang/tests/types_value_semantics.rs:115) | yes - proof green; the test asserts the code or value its criterion fixes |
| C21 | `an_array_rest_pattern_binds_an_array` (crates/piperine-lang/tests/types_value_semantics.rs:124) | yes - proof green; the test asserts the code or value its criterion fixes |
| C22 | `an_array_is_passed_as_itself` (crates/piperine-lang/tests/types_value_semantics.rs:133) | yes - proof green; the test asserts the code or value its criterion fixes |
| C23 | `str_is_a_value_type_in_every_position` (crates/piperine-lang/tests/types_value_semantics.rs:85) | yes - proof green; the test asserts the code or value its criterion fixes |
| C24 | `the_reporting_functions_emit_the_same_records` (crates/piperine-parity/tests/source_value_semantics.rs:198) | yes - proof green; the test asserts the code or value its criterion fixes |
| C25 | `std_holds_no_removed_form` (crates/piperine-lang/tests/std_library.rs:802) | yes - no removed form in any std file, and the three resolvers take `[T; N]` under `const N` (D7) |
| C26 | `every_part_vii_fence_is_in_its_file` (crates/piperine-lang/tests/std_excerpts.rs:237) | yes - proof green; the test asserts the code or value its criterion fixes |
| C27 | `section_22_no_longer_says_std_is_in_the_earlier_spelling` (crates/piperine-lang/tests/spec_text.rs:1804) | yes - proof green; the test asserts the code or value its criterion fixes |
| C28 | `the_library_items_that_took_borrows_give_the_same_results` (crates/piperine-parity/tests/source_value_semantics.rs:223) | yes - proof green; the test asserts the code or value its criterion fixes |
| C29 | `no_msl_file_uses_a_removed_form` (crates/piperine-lang/tests/suite_hygiene.rs:1149) | yes - proof green; the test asserts the code or value its criterion fixes |
| C30 | `startup_prints_the_whole_start_on_its_grid` (crates/piperine-cli/tests/buck.rs:90) | yes - proof green; the test asserts the code or value its criterion fixes |
| C31 | `the_printer_and_the_object_model_hold_no_removed_form` (crates/piperine-lang/tests/msl_print.rs:369) | yes - proof green; the test asserts the code or value its criterion fixes |
| C32 | `builtin_table_matches_appendix_h` (crates/piperine-lang/tests/pom_resolve.rs:876) | yes - proof green; the test asserts the code or value its criterion fixes |
| C33 | `gaps_drops_the_value_semantics_gap` (crates/piperine-cli/tests/docs.rs:314) | yes - proof green; the test asserts the code or value its criterion fixes |
| C34 | `e0903_mut_is_not_a_word_of_the_language` (tests/book/language.rs:232) | yes - proof green; the test asserts the code or value its criterion fixes |
| C35 | `e0101_nothing_is_lent` (tests/book/types.rs:214) | yes - proof green; the test asserts the code or value its criterion fixes |
| C36 | `every_twin_prints_the_bytes_it_printed_before_the_move` (crates/piperine-cli/tests/twins.rs:146) | yes - twin byte goldens unchanged on the branch |
| C37 | `the_five_m1_bodies_print_as_the_golden_files` (crates/piperine-transpile/tests/golden.rs:57) | yes - transpile goldens unchanged on the branch |
| C38 | `the_ir_holds_no_slice` (crates/piperine-ir/tests/aggregates.rs:461) | yes - proof green; the test asserts the code or value its criterion fixes |
| C39 | `a_local_is_assignable_exactly_when_its_body_assigns_it` (crates/piperine-ir/tests/aggregates.rs:224) | yes - proof green; the test asserts the code or value its criterion fixes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 1. tasks.md does not record the work: every `Done when` box of T1 to T15 is unchecked though all fifteen tasks are committed, `**Status**` still says "awaiting approval" under an `**Approved**` line, and that line is not in the form the gate reads (`- user: "<their words>"`), so `validate_verification.py` refuses the tasks as unapproved | `.specs/features/value-semantics/tasks.md`:3 (`**Approved**: 2026-11-20 - user confirmed the plan ... ("Ok, perfeito!" ...)`), :14 (`**Status**: awaiting approval`), every `- [ ]` under T1 to T15 | Tick the boxes, set `**Status**: Approved`, and write line 3 as `**Approved**: 2026-11-20 - user: "Ok, perfeito!"` with the context after it | 5ac409e3: boxes ticked, Status Approved, the Approved line is `user: "Ok, perfeito!"` dated 2026-11-20 |
| 2. C9's test is looser than VS-03 AC 2: it omits the carrier and `self` | `crates/piperine-lang/tests/types_value_semantics.rs`:51-62 holds six of the eight targets; the carrier is held only by composite-values' `assigning_a_non_place_is_e0183_naming_the_target` (types_control.rs:95); `self` cannot be written until `generics` brings methods | Add the carrier case to C9's test; record in the check that `self` is proven by `generics` when methods land | 8dc2aed3: the carrier case sits in C9's test; `self` is named as proven by `generics` |
| 3. Cleanliness (user rules: no free function): the new targets hold free helper functions, the parity ones copied from `source_control.rs` | `crates/piperine-parity/tests/source_value_semantics.rs`:34 `project`, :40 `real_bytes`, :48 `byte`, :54 `first_commits`, :68 `operating_point`, :81 `integer_operators`, :97 `real_operators`, :190 `records_of`, :213 `pattern_value`, :218 `pattern_matches`; `crates/piperine-lang/tests/types_value_semantics.rs`:12 `source`, :17 `outcome`, :27 `assert_clean`, :33 `assert_refused` | Give each an owner type in a shared support module (with composite-values Finding 8) | b045fc3d: SourceRun, CarrierBytes, PlaceOperators, LevelPattern and CheckedSource replace the free helpers |
