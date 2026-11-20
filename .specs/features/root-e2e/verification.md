# Root E2E verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 8eabbf7b (HEAD 07e6b47b adds only `proofs.md`)
**Verifier**: independent sub-agent (author != verifier)

Read: `spec.md`, `design.md`, `contract.md`, `tasks.md` (`## Checks`), `divergences.md`, `proofs.md`,
`GAPS.md`, `git diff origin/main...HEAD` (merge base `1a3d69dc`). Re-run at HEAD by the Verifier:
`cargo test -p piperine-lang` (45 targets, 0 failed), `cargo test -p piperine-api --test digital_only
--test records` (15 passed), `cargo test --test book_inventory --test book_claims` (23 passed). Own
examples: a throwaway binary in the scratchpad over `piperine-lang` (nothing committed, tree clean).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `tests/book/inventory_guard.rs` `every_block_of_the_book_has_one_row` | yes - one row per `<pre data-lang>` with its FNV fingerprint; planted missing, moved and extra rows fail (`the_guard_names_a_block_without_a_row_and_a_row_naming_no_test`) |
| C2 | `tests/book/inventory_guard.rs` `every_row_names_an_existing_test` | yes - `Coverage::resolve` finds a top-level `fn` in the named target; planted unknown test and unknown target fail |
| C3 | `tests/book/inventory_guard.rs` `the_guard_names_a_block_without_a_row_and_a_row_naming_no_test` | yes - planted tree, each rule names the block id |
| C4 | `tests/book/inventory_guard.rs` `run_rows_name_tests_that_run_their_model` | yes - textual (the body names the id and calls `.run(`); it does not prove the run is of that block, but every sampled test loads the block by id through `Chapter`/`Inventory` |
| C5 | `tests/book/welcome.rs` `the_rc_step_charges_as_the_book_says` | yes - 0.632/0.865 at the printed precision, the trip at 693.1 ns, 0.04 ns early |
| C6 | `tests/book/inventory_guard.rs` `refused_rows_name_tests_that_expect_their_refusal` | yes - `Refusal::expect` compares code and exact message |
| C7 | `tests/book/inventory_guard.rs` `refused_rows_and_gaps_section_11_agree` | yes - both directions, gap id an entry; planted variants in `the_gaps_guard_names_a_missing_and_an_extra_row` |
| C8 | `tests/book/errors.rs` `a_refusal_that_runs_fails_naming_its_row` | yes - catches the panic and asserts the row id and "now runs" |
| C9 | `tests/book/claims_guard.rs` `every_value_sentence_has_a_claim_row` | yes - for what `ClaimDetector` flags (a heuristic by design; planted sentences in C12) |
| C10 | `tests/book/claims_guard.rs` `holding_claims_name_root_tests_that_cite_them` | yes - textual citation; sampled `holds` tests (welcome~b1f1d1ff, values~4b156a2d, packages~5018120b, from-rust~272b0be7, the buck rows) assert the stated value. `binary` rows need no citation: F5 |
| C11 | `tests/book/claims_guard.rs` `every_claim_row_is_a_sentence_of_its_chapter` | yes |
| C12 | `tests/book/claims_guard.rs` `the_detector_flags_a_value_a_code_and_an_outcome` | yes - seven flagged, five plain, a repeat counted once |
| C13 | `crates/piperine-parity/tests/parity_corpus.rs` `every_twin_has_its_files_in_one_scenario_directory` | yes - planted trees in `the_layout_guards_report_planted_trees` |
| C14 | `crates/piperine-parity/tests/parity_corpus.rs` `tests_ngspice_holds_no_twin` | yes |
| C15 | `crates/piperine-cli/tests/twins.rs` `every_twin_prints_the_bytes_it_printed_before_the_move` | yes - exact bytes after the header per backend; the seven old goldens moved with 0-line diffs, `.cir` byte-identical, the 16 `.msl` differ in their first comment line only |
| C16 | `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test 'parity_*'` | yes |
| C17 | `git diff --quiet <merge-base> -- contract.rs parity_{op,dc,ac,tran,tf,pz,sens,pss,temp}.rs` | yes - no tolerance edited; `tests/scenarios.rs` writes values only through `ToleranceContract` (the 1e-9-of-a-step grid match and the `tran_floor` 1 us window copy `parity_tran.rs`) |
| C18 | `tests/scenarios.rs` `every_twin_has_a_scenario_test` | yes |
| C19 | `tests/scenarios.rs` `divider` | yes - `DC_VOLTAGE` on `vin`, `mid`; `Check::report` fails when ngspice ran and nothing was compared |
| C20 | `tests/scenarios.rs` `tran_rc` | yes - 100 points under `TRAN_VOLTAGE`, deviation 8.6e-5 printed in `proofs.md` |
| C21 | `tests/scenarios.rs` `a_disabled_ngspice_still_runs_the_scenario` (SKIP line) | yes |
| C22 | `tests/scenarios.rs` `each_scenario_runs_in_a_directory_of_its_own` | yes |
| C23 | `tests/book/inventory_guard.rs` `a_model_across_blocks_names_one_test` | yes - planted split fails |
| C24 | `crates/piperine-cli/tests/book.rs` `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes |
| C25 | `crates/piperine-parity/tests/parity_corpus.rs` `a_scenario_with_benches_holds_a_manifest` | yes |
| C26 | `tests/book/inventory_guard.rs` `no_block_row_is_pending` | yes - real tree plus a planted pending row |
| C27 | `tests/book/claims_guard.rs` `no_claim_row_is_pending` | yes - planted pending rows in `the_claim_guard_names_a_sentence_without_a_row` |
| C28 | `crates/piperine-lang/tests/suite_hygiene.rs` `the_root_manifest_names_its_test_targets` | yes - planted manifest, a `[[bin]]` path ignored |
| C29 | `crates/piperine-lang/tests/suite_hygiene.rs` `root_targets_match_the_naming_rule` | yes - the two exemptions carry their reason |
| C30 | `crates/piperine-cli/tests/docs.rs` `gaps_drops_what_root_e2e_closed` | yes - spot-checked: `E0503`-`E0508` are `RunCode` variants raised in `piperine-analog-solver`/`piperine-solver`, `E0804`-`E0806` in `piperine-api/src/host.rs`, `E0803` in `session.rs`; the 64 codes left in §10 have no raise site |
| C31 | `crates/piperine-cli/tests/docs.rs` `the_roadmap_and_state_record_root_e2e` | yes |
| C32 | `crates/piperine-parity/src/corpus.rs` `corpus::tests::a_name_in_two_scenarios_is_refused` | yes |
| C33 | `tests/book/inventory_guard.rs` `every_gaps_row_names_its_owner` | yes - planted owner without a spec fails |
| C34 | `tests/book/inventory_guard.rs` `disputed_rows_are_listed_for_the_user` | yes - planted unlisted row fails |
| C35 | `tests/book/inventory_guard.rs` `no_row_diverges` | yes - real tree plus planted `diverges` and `disputed` rows |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (Major) - `emit!` outside behavior is still `E0900`, not `E0601`, when written at the top of a file. Part VI §19 rule 10 and its table (`E0601`: "`emit!` written outside a reaction, `analog` or a `fn`") and the Book (errors~64ee8d1d, `holds`) state it for every position; Phase 9 fixed the module body only. `tests/book/cheatsheet.rs` `the_files_card_is_refused_for_its_table_and_its_statement` asserts today's `E0900` for the cheat sheet card (cheatsheet#10), and GAPS §11's `Today` repeats it, so a spec divergence is recorded as current behavior under L57 instead of being fixed | own example at HEAD: `parse("emit!(\"hello\");\nmodule M() { }")` gives `E0900 unexpected identifier \`emit\`, expected item`; `module M() { let x = 1.0; emit!("hello"); }` gives `E0601`. `errors~64ee8d1d`'s test covers one position (a `let` in a module body) | recover a file-level `emit!` statement as `E0601` in the item parser, as `recover_item` now does in a module body; add the case to `msl_errors.rs`; flip cheatsheet#10's assertion and its §11 `Today` to `E0601`; extend `e0601_is_an_emit_outside_a_reaction_analog_or_fn` to the file-level position | 4052a73b; `msl_errors::an_emit_statement_at_the_top_of_a_file_reports_e0601`, `book_errors::e0601_is_an_emit_outside_a_reaction_analog_or_fn`, `book_cheatsheet::the_files_card_is_refused_for_its_table_and_its_statement` (C7) |
| F2 (Minor) - `E0350` for a placement in a reaction body is decided on the syntax of the `on` head: only `on <ident>(..)` is recognised. `on every(..) && en { }` and `on (every(..)) { }` nested in a reaction stay `E0404`, although each places `every` in the reaction body (part III §8.3.4) | own example: `digital for M { on at(1u) { on every(0.0, 10u) && en { } } }` gives `E0404 \`on\` in a reaction`; the parenthesised form the same. `Ctx::nested_reaction` (`msl/parser/check.rs`) matches `ExprKind::Call` with a one-segment path only | find a call of a module anywhere in the nested event expression (or decide at resolution, where the callee is known to be a module); add the two forms to `an_on_every_inside_a_reaction_is_a_placement_in_a_reaction_body_e0350` | 7b584360; `msl_errors::an_on_every_inside_a_reaction_is_a_placement_in_a_reaction_body_e0350` |
| F3 (Minor) - the `E0432` fix passes the rule's code into binary and unary operands but not through parentheses, `if` or a block: `count <- (count + 1.5)` is still `P0002` where `count <- count + 1.5` is `E0432` (§12.1.9) | own example: `P0002 mismatched types: expected \`u9\`, found \`f64\`` for the parenthesised commit. `types/expr.rs` `check_as`: the `Paren` and `If` arms call `self.check(..)` (code `P0002`) | use `check_as(.., code)` in the `Paren`, `If` and block-value arms; add the parenthesised commit to `an_operand_of_another_type_in_a_commit_is_e0432` | 10daf5d5; `types_events::an_operand_of_another_type_in_a_commit_is_e0432` |
| F4 (Minor) - new free function in new code, against the user's rule (no free functions, every function a method of a type) | `crates/piperine-api/tests/digital_only.rs` `fn run_streams(..)` added by f08204d0 (the file's older `run`/`run_source` are pre-existing). No other new non-test free function in the diff; the lang fixes are methods (`Ctx::nested_reaction`, `Ctx::clashing_item`, `Ctx::clash_diagnostic`, `Checker::report_distinct`, `Streams::finish_of`) | make it a method of a small owner type (e.g. a `BlinkyRun` with `streams`), or a method on the existing fixture | 825668f3; `digital_only::a_finish_ends_the_run_ok_when_the_sim_stream_is_not_kept` (`Kept::run_streams`) |
| F5 (Minor) - a `binary` claim row needs only an existing `piperine-cli` test, not one that cites or asserts the claim, so a row can point at an unrelated test and pass; `holds` rows must cite their id, `binary` rows need not. One sampled row is only half held: cli~3cd92481 ("`--help` lists them, and `--version` prints the version") names `usage::help_lists_exactly_the_eight_commands`, which never runs `--version` (`version_prints_the_workspace_version` does). `prose` (32 rows) is unconstrained by design; sampled, all defensible except cli~3d1453da (the buck "steps 12 V down to 0.8 V"), which a `binary` buck test already holds | `tests/book/claims_guard.rs` `binary_claims_without_a_command_line_test` resolves the test only; `tests/book/inventory.tsv` cli~3cd92481 | require a `binary` row's test body to cite the claim id, as `asserted_claims_without_a_citing_test` does, and add the citations; point cli~3cd92481 at a test that runs both flags; make cli~3d1453da `binary` over the buck test | 7746f554; `book_claims::binary_claims_name_command_line_tests` (C10), `usage::help_lists_the_commands_and_version_prints_the_version` |
| F6 (Cosmetic) - stale or incomplete text: `divergences.md` T36 paragraph still says the buck row is "Left `disputed` for the user, with the test asserting today's values", contradicting its own table row and the user's ruling (the Book now follows §16 step 4, row `holds`); `Tree::guard` doc comments are truncated ("The rules over this tree, ", `inventory_guard.rs`) or stale ("with the chapters the registry settles", `claims_guard.rs`, the registry is gone); `Check::new` in `tests/scenarios.rs` has no doc comment; GAPS §10 says `E0803` to `E0806` "are raised by `host.rs`", but `E0803` is raised in `piperine-api/src/session.rs` | the files named | correct the four texts | 07a3b2f1; the four texts corrected |

## Rulings asked for

- **E0163 secondary label, block-level clash.** Not a spec divergence. Part I §3.5 rule 5 and the
  `E0163` row fix the code and the ambiguity only; no rule requires a secondary label. The Book's
  rendering (errors#2) is the top-of-namespace clash, which now carries the label, and the Book says
  "These two renderings are illustrative". Leaving the block-level `use` clash with its primary label
  alone is a consistency gap, recorded in `divergences.md` (T35); it needs no finding. A later
  diagnostics pass may give both clashes the same label.
- **Buck start-up rewrite (user decision: the Book follows §16 step 4).** Faithful. The table, the
  paragraph and the "Run it" sentence now give the run of mixed#8: the first record before the tick
  acts (err 3.300, duty 0.0033), 11.56 V and -8.26 at 10 us, 10.79 V at 20 us, duty clamped at 0 until
  70 us (2.00 V, duty 0.0013), each period adding ki times err, the error under 1 mV near 7 ms, the
  table's first row the 11.8 V operating point. `the_buck_starts_from_the_operating_point_with_the_first_tick_applied`
  asserts each of these; spec and code are untouched.
- **Other Book edits.** The status paragraphs no longer list `sin` as refused (it runs:
  `the_math_methods_run_and_the_settle_event_does_not`); the reverse-bias cell 5.0e-20 A matches the
  measured 4.98e-20 A; the generated listing positions `devices.msl:4:1` and `8:34` are the binary's;
  the `nodeset` sentence matches part VIII and `a_nodeset_seeds_the_operating_point_of_ac_and_transient`.
- **No test weakened, skipped or deleted.** The diff removes no `#[test]` and adds no `#[ignore]` or
  file-scope `#![allow]`; the only expectation edits are the Phase 9 flips named in the fix commits
  and the existing tests the tasks list (paths, counts). The `finish_of` fix is correct (it reads the
  `sim` record whatever the stream filter keeps) and is tested where it lives (`digital_only`).
