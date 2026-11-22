# Explicit Unknowns verification

**Verdict**: PASS for B1-B3 (orchestrator, 2026-11-22: all 17 findings closed; 16 and 17 by 758c5ea9 and 1f5b35aa, pom_docs green; B4-B8 checks out of scope)
**Proofs**: `proofs.md` at 971b88ac (27 pass, 71 fail: the 70 B4-B8 checks and C92, all owned by tasks not yet built)
**Verifier**: independent sub-agent (author != verifier); fix round re-verified by a second independent sub-agent over 564a8aa0..971b88ac
**Scope**: partial-batch verification of B1-B3 (T1-T18 and the fix commits 9c73998d..143575e9), the specification and Book amendment; no code changed. The 25 checks those batches prove are judged (C41-C51, C71-C83, C85). The other 66 checks belong to B4-B8; their tests do not exist yet and their proofs are red at HEAD by design (`proofs.md`: 27 pass, 70 fail, all 70 in B4-B8 groups). They are listed below as out of scope and were not judged.

Targets run besides `check_proofs.py`: `piperine-lang` `msl_errors`, `pom_docs`, `pom_resolve`, `std_library`, `book`, `std_galleries`, `spec_codes`, `std_excerpts` (all green); `docs/book/build.py` builds (Part 1 Fundamentals, Part 2 In depth). `check_commit.py` passes on all 32 commit messages of the range.

## Fix round (564a8aa0..971b88ac)

All 15 findings are closed by their commits; each was read against its diff and the tree at 971b88ac (per-finding notes in the Findings table). Every in-scope check holds. Two new findings (16, 17) are open, both in `docs/pom`, both small; the verdict stays FAIL until they are closed.

Targets run besides `check_proofs.py`: `piperine-lang` `spec_text`, `pom_docs`, `std_library`, `pom_resolve`, `book`, `suite_hygiene`; `piperine-cli` `docs`, `book`; the 24 root `book_*` targets (32 targets, all green).

- **EU-04 AC 7 (finding 6)** is appended as criterion 7, worded as an amendment ("added after the approval of AC 4, which it narrows", marked to be shown to the user); AC 4 is byte-for-byte as approved. It matches §3.3.4 and §5.2.12: a delayed definition is not exempt, and a child's `out` port without an initial value stays `E0187` under AC 4. C92 (owner T30) proves it. The user has not seen the amendment yet.
- **docs/pom (finding 14)** may keep describing the `resolve` field, `LitKind::Unknown` and `level` now: those pages document the POM API the code has today, and `pom_docs` ties their tables to the code, so rewriting them before T28, T29 and `resolved-nets` T16 would make them false the other way. What the finding asked for is there: each statement is registered in `pom_docs::DocAheadOfTheCode::WAITING` with its owner, the guard fails both when a form leaves the code and when a page drops a statement, and resolved-nets T16 is told to rewrite every statement of the field on `reflection.md`, `model.md` and `mapping.md`. The statements not listed one by one (`model.md:1160, 1369`, `reflection.md:635`, `mapping.md:19`) are on those pages and trip with them. One page is outside the registry (finding 17).
- **natures#2 (finding 3)**: the test keeps a stand-in `nature Sda { value: Option<bool> }` instead of prepending the chapter's own `Sda` (which is `E0161` today, as `Driven` is not in the library); the stand-in's value type has a `Default`, which is what the finding's defect was. Accepted.
- **C79 (finding 5)** is now owned by T30, out of this partial verification. Its test passes at HEAD only through the two named `E0150` exemptions, which T30 removes; T18's Done-when says so.
- **Leftover sweep.** `Wire`, `Unknown`, `known_or`, `is_known`, `.level(`, `maybe`, `resolve:` over `docs/spec`, `docs/book/content.html`, `docs/pom`, `GAPS.md`, `ROADMAP.md`: in the specification only G.72 (the record of what was removed); in the Book only the two exempt `E0150` blocks (T30); `GAPS.md` and `ROADMAP.md` none; in `docs/pom` the registered statements plus findings 16 and 17. The fixed listings (`Shift2`, `UartShift`, `Tran`, `Front`) agree with §3.3.4, §22.1.3 and §22.3.7; `Tran` is now the text of §22.3.7. Restored L59 keeps the original row's layout and drops only its unknown clause; the 36-loud count matches.

## Judgements asked for

- **The approved decisions.** §3.3 (R1-R6), H.8/H.10 (D2-D4), §4 rule 8 (`Resolve<X: Repr, V: Repr = X>`, E0203), §5.2 (one driver unless `Resolve`, E0220 naming the trait, E0225, §5.2.15), §8.2.3 (relaxation from absence, E0511), §22.3 (five strengths, `Driven`, `strongest`, no strength trait, no library `Wire`/`Tri`/`SumNet`), `abi.md` (major 1 edited in place, no known byte, no `commit_with`) implement R1-R10 and Q1-Q5 as approved. The `resolve` field is gone from Part II and Appendix B's grammar. The leftovers are findings 1, 2 and 14.
- **The E0187 exemption (§3.3.4, §5.1 error row) is confirmed.** The user's rule is "has a Default, may be left uninitialized; has none, must be specified". An undelayed definition specifies the carrier's value at every instant from `(0, 0)` (§8.2.29), and §8.2.21/E0338 already forbid an initial value under one, so without the exemption a `bool` carrier with a definition could be written neither way. A child's `out` port is the carrier's one driver and holds the initial value itself (§5.2.6 forbids one on the signal, E0223); E0187 then falls on that port if it gives none. The value is specified, once, by the driver, which is the rule. The exemption is scoped right: a delayed definition is not exempt (its carrier has no value before the first delay elapses). The defect is in the records, not the rule: spec.md EU-04 AC 4 still says any signal or `out` port without an initial value and without `Default` is E0187 (finding 6).
- **Row-owner audit.** T40's list (14 rows) and T48's list (5 rows) equal, row for row, the S8 rows of `GAPS.md` section 11, and each needs only what its phase delivers, with two exceptions: values~37a548f2 (an undriven top `in` port refusing the analysis) is made to run by T38 (EU-04 AC 6, C16), not "T21, T22 and T30" as its row says (finding 10); and natures#2 is misowned (finding 3): the block declares `[Bit; 8]` (composite-values), `Bit<Option<u8>>` (phase A), `Bit<Logic<4>> = logic("10xx")` (phase B) and `signal sda: Sda` whose `Sda` is the chapter's `impl Resolve<Driven>` nature (resolved-nets). composite-values lands first and can only re-pin the row, so the feature that really makes it run is `resolved-nets`. core-std#6 needs `Resolve`, `Driven` (so `Logic`, phase B) and the generic `Pullup`; `resolved-nets` runs after generics and phase B (ROADMAP queue 7), so its owner is right. The L21 rows digital#16-#20 and natures#3 are right for the same reason. A refused row's test fails "now runs" by `Refusal::expect`/`Refusal::of`.
- **Spec-ahead lists.** `AHEAD_OF_STD`/`BEHIND_SPEC`, `INDEX_AHEAD_OF_STD`, `STD_BEHIND_THE_INDEX`, `STD_BEHIND_THE_BOOK`, `BLOCKS_AHEAD_OF_STD`, `SPEC_TRAITS_AHEAD_OF_THE_TABLE`, `RETIRED_AHEAD_OF_THE_PARSER` each name a task and fail on a stale entry, except `INDEX_MEMBERS_AHEAD_OF_STD` (finding 12). The `msl_errors` floor 19 to 18 is justified: E0908 was the one parse-phase code retired (E0906 was elaboration), and the bound is a floor. `docs/pom/mapping.md` lists the 62 constructs of B.1.2; its note on `resolve`, `Unknown`, `level` is honest but not guarded (finding 14).
- **Citations (§22.3.3).** The LRM 2.4 citations are confirmed against `e0685173^:VAMS-LRM-2-4.md` (§9.22.3, Figure 9-3, "IEEE Std 1364-2005 ... subclauses 7.10 and 7.11"); the ngspice manual's section 8.6 is "Predefined Node Types for event driven simulation" in the published manual. IEEE 1800 §6.6.7 and IEEE 1164 are kept on secondary sources, as the commit body says. Logisim, Digital, IEEE 1666 and the unconfirmed clause ranges are dropped. Nothing invented. One sentence goes past the confirmed text: that `large`, `medium`, `small` are charge-storage strengths and the other five Verilog's drive strengths comes from IEEE 1364, which was not read; it is correct, and is noted, not a finding.
- **The Book.** Self-contained (no "same as Rust"/"as in Rust"), respectful, Part 1 before Part 2, every new listing has an inventory row and a test. "Driving with strength" is right against the spec: the AndWire table, the open-drain table (Z/Z/pull is 1 at Pull, a strong 0 wins, no pull-up reads `Driven::Z`), the keeper's weak X before any drive, the Strong conflict giving X at Strong, E0511 for an instability, `Resolve<Option<u8>, Data>` with E0225. Leftovers are findings 1 and 5.
- **Hygiene.** One commit per task plus fix commits; messages pass `check_commit.py`. Findings 11 (displaced doc comments) and 13 (tasks.md never marked).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `src` `a_binary_carrier_is_read_as_its_value` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C2 | `src` `control_on_a_carrier_runs_one_path` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C3 | `ir` `no_value_type_is_flagged` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C4 | `types` `unknown_is_a_name_that_does_not_resolve` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C5 | `types` `the_flag_methods_do_not_resolve` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C6 | `types` `maybe_is_an_ordinary_field_name` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C7 | `hygiene` `retired_unknown_codes_are_raised_nowhere` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C8 | `ops` `control_refuses_a_logic_and_an_option` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C9 | `logic` `analog_reads_a_logic_through_a_function` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C10 | `ops` `analog_refuses_a_bare_logic` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C11 | `src` `a_written_initial_value_is_the_value_at_the_start` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C12 | `src` `a_carrier_without_a_value_starts_at_its_default` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C13 | `src` `an_element_never_committed_reads_its_initial_value` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C14 | `types` `a_missing_initial_value_is_e0187_naming_the_carrier` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C15 | `types` `an_undriven_carrier_without_default_is_e0187` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C16 | `session` `an_undriven_top_port_without_default_refuses_the_analysis` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C17 | `types` `the_types_that_have_a_default` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C18 | `types` `deriving_default_over_a_number_is_e0187` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C19 | `types` `no_primitive_numeric_type_has_a_default` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C20 | `src` `an_unbound_option_param_is_none` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C21 | `src` `an_option_carrier_is_read_by_matching` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C22 | `logic` `an_option_of_logic_is_none_or_a_whole_vector` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C23 | `ops` `logic_is_compiled_from_the_library_source` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C24 | `logic` `logic_lays_out_two_planes` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C25 | `logic` `logic_reads_a_four_state_pattern` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C26 | `logic` `at_gives_the_state_of_one_level` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C27 | `logic` `and_follows_the_four_state_table` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C28 | `logic` `or_follows_the_four_state_table` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C29 | `logic` `xor_follows_the_four_state_table` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C30 | `logic` `not_follows_the_four_state_table` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C31 | `logic` `shifts_move_states_and_fill_with_zero` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C32 | `logic` `a_logic_and_a_binary_value_combine_level_by_level` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C33 | `ops` `arithmetic_and_ordering_refuse_a_logic` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C34 | `logic` `equality_of_logic_is_a_logic` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C35 | `logic` `same_compares_the_four_states` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C36 | `logic` `the_four_tests_of_a_logic` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C37 | `ops` `deriving_partial_eq_over_a_logic_is_e0187` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C38 | `logic` `a_binary_value_converts_to_logic` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C39 | `logic` `a_logic_converts_to_an_option_of_bits` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C40 | `logic` `to_volts_gives_the_level_of_each_state` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C41 | `spectext` `a_nature_takes_trait_impls` | yes - quotes §3.1.9, the E0134 row (bundle only) and §3.7.13 by text |
| C42 | `spectext` `resolve_is_a_trait_of_two_parameters` | yes - the trait signature, `V` is the value type, E0203 row, and the sentence that no field/keyword is left; grammar productions read by hand: no `resolve` in `nature_field` (Part II, Appendix B) |
| C43 | `spectext` `many_drives_need_the_resolve_capability` | yes |
| C44 | `spectext` `a_drive_picks_its_impl_by_its_type` | yes |
| C45 | `spectext` `resolve_receives_plain_values_for_every_count` | yes |
| C46 | `spectext` `a_drive_without_a_value_starts_at_its_default` | yes - asserts §5.2.12's `a type without it is E0187` since 1a8fdafd (finding 8 closed) |
| C47 | `spectext` `a_relaxing_drive_starts_absent` | yes |
| C48 | `spectext` `a_loop_that_does_not_settle_is_e0511` | yes - also asserts the old 'not an error' row is gone and the row's phase is `run` |
| C49 | `spectext` `strongest_resolves_by_strength` | yes - the five-level order and the four-row table; operator code read by hand: `strongest` is order-independent on the table's cases |
| C50 | `spectext` `the_library_declares_no_net_nature` | yes - Part III, Appendix C, G and index.md no longer name a library `Wire` (949f4d84), and the absence walk now covers `Wire` (finding 2 closed) |
| C51 | `spectext` `the_gallery_works_four_nets` | yes |
| C52 | `src` `a_change_is_a_different_value` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C53 | `src` `the_first_commit_is_a_change` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C54 | `src` `prev_is_the_initial_value_before_a_change` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C55 | `src` `an_edge_is_a_transition` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C56 | `src` `a_carrier_that_starts_high_has_not_risen` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C57 | `src` `settle_runs_as_before` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C58 | `src` `an_event_read_is_a_bool` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C59 | `src` `a_false_guard_runs_nothing` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C60 | `records` `a_record_holds_its_declared_fields_only` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C61 | `records` `a_require_is_a_pass_or_a_fail` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C62 | `records` `a_fatal_record_ends_the_run` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C63 | `clirun` `a_logic_cell_prints_its_pattern` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C64 | `clirun` `an_option_cell_prints_none_or_the_value` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C65 | `cache` `an_entry_of_the_earlier_contract_is_rebuilt` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C66 | `abi` `the_shipped_layouts_hold_the_value_only` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C67 | `abicontract` `the_host_has_one_commit` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C68 | `abi` `an_option_pin_is_presence_then_payload` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C69 | `abi` `a_logic_pin_is_two_planes` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C70 | `relax` `a_drive_pin_ends_with_a_plain_driving_byte` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C71 | `spectext` `section_3_3_states_the_explicit_model` | yes - six named rules, the Default sentence and 'differs from Rust'; the section now says seven rules (130718cb, finding 7 closed) |
| C72 | `spectext` `section_22_1_holds_the_four_state_tables` | yes - &, |, ^, ! rows and the `==` sentence; the four planes' code checked by hand against every table cell |
| C73 | `spectext` `the_removed_words_are_absent_from_the_specification` | yes - the walk now flags a standalone `Wire` outside G.72 (949f4d84); the old-model listings are rewritten (finding 2 closed) |
| C74 | `spectext` `appendix_g_records_the_explicit_model` | yes |
| C75 | `docs` `abi_md_describes_the_unreleased_contract` | yes |
| C76 | `spectext` `appendix_h_holds_default_from_and_the_operator_traits` | yes |
| C77 | `bookvalues` `the_values_chapter_teaches_the_explicit_model` | yes |
| C78 | `booklang` `the_book_says_numbers_have_no_default` | yes |
| C79 | `bookinv` `the_book_uses_no_removed_name` | out of scope - moved to T30 by 6e6a8a4f (finding 5); its test passes at HEAD only through the two named E0150 exemptions T30 removes; not judged in this partial verification |
| C80 | `booknatures` `the_book_drives_nets_with_strength` | yes - scans the whole Book for a standalone `Wire` (e45b2df8); none is left (finding 1 closed) |
| C81 | `bookinv` `every_listing_that_does_not_run_names_its_gap` | yes - names a gap that exists; natures#2 is pinned to L21/`resolved-nets` (1875a83b, finding 3 closed) |
| C82 | `bookabi` `the_abi_chapter_describes_the_unreleased_contract` | yes |
| C83 | `docs` `state_holds_the_explicit_unknowns_decision` | yes - also asserts MD-70's 'Superseded whole by MD-78' (07483c3a, finding 9 closed) |
| C84 | `docs` `gaps_closes_s6_and_tracks_the_flag` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C85 | `docs` `the_per_level_feature_is_recorded_as_dropped` | yes |
| C86 | `hygiene` `no_source_uses_a_removed_unknown_form` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C87 | `hygiene` `every_unknown_test_has_a_successor` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C88 | `excerpts` `every_fence_is_an_excerpt_of_std` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C89 | `twins` `every_twin_matches_its_golden` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C90 | `buckcli` `the_buck_runs_from_the_cli` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C91 | `integrated` `the_integrated_suite_passes` | out of scope - owned by B4-B8, red at HEAD by design; not judged in this partial verification |
| C92 | `types` `a_definition_or_a_child_out_port_specifies_the_value` | out of scope - added by the fix round (EU-04 AC 7, finding 6), owned by T30, red at HEAD by design; not judged in this partial verification |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| 1. The Book still names a library `Wire`, which EU-18 AC 4 forbids, and C80 passes | `docs/book/content.html:352` ("resolved natures such as `Wire`"), `:3641` ("`Bidir` is a pad on a `Wire`"), `:5307` (`Wire` listed as ordinary `std` source); `tests/book/natures.rs:254-258` scans only the natures and digital chapters. `STD_BEHIND_THE_BOOK` (`crates/piperine-lang/tests/book.rs:207-209`) omits `Wire` from its list although its own comment names it, because the core-std chapter still names it | rewrite the three sentences (a nature the model declares; `Bidir` generic over `W: Nature + Resolve<Driven>`; drop `Wire` from the std list); make the C80 test scan the whole Book; add `Wire` to `STD_BEHIND_THE_BOOK` | e45b2df8 (confirmed) |
| 2. Old-model listings survive in the specification outside every check | `docs/spec/part_iii_structure.md:106` (port table `sda: Wire`); `:443-447` `Shift2`: `signal q1: Bit;` and `out q2: Bit` are driven by commits with no initial value, E0187 under §3.3.4; `:454-457` `UartShift`: `signal sh: [Bit; 8];` has no initial value and `byte = cast(sh);` assigns the `Option<u8>` that `cast` now returns (§22.1.3 writes `cast(sh).unwrap()`); `:460-464` `Tran(a: Wire, b: Wire)` drives `Logic::Z`, contradicting §22.3.7; `appendix_c_implementation.md:126` "`Tran` on `Wire`s"; `index.md:228-232` `Front`: `out q: Bit` committed with no initial value | bring the five listings and two rows to the explicit model (initial values, `cast(..).unwrap()`, the generic `Tran<W>` with `Driven::Z`, a declared nature); add `Wire` to the absence walk of `spec_text.rs` (outside G.72) | 949f4d84 (confirmed) |
| 3. natures#2 is owned by a feature that cannot make it run | `GAPS.md:504`, `ROADMAP.md:59`, tasks.md Handoff (line 1528) and `tests/book/inventory_guard.rs:648` name `composite-values`/L24; the block also needs phase A (`Option`), phase B (`Logic<4>`, `logic`) and `resolved-nets` (`Sda` is the chapter's `impl Resolve<Driven>` nature); `tests/book/natures.rs:81` stands `Sda` in as `nature Sda { value: bool }`, which after phase A is itself E0187 | owner `resolved-nets` (gap L21), the row text naming all four needs; the test prepends the chapter's own `Sda` declaration; composite-values' Handoff line says it re-pins the row, not flips it | 1875a83b (confirmed) |
| 4. L59 was deleted whole, losing two gaps that are not about unknowns | `GAPS.md` lines 25 and 33 count L59 closed; the deleted row also tracked a non-literal closure of `map`/`and_then`/`then` and a non-literal `expect` message (`P0001`, msl-fixes Handoff items 14 and 16), which the code still refuses; T40, not B2, was to close it; `GAPS.md:477` (digital#7) still cites `known_or(clean)` and L59 | restore L59 without its "presence may be unknown" clause, fix the counts, rewrite the digital#7 row without `known_or` | e85a6ce0 (confirmed) |
| 5. C79 passes while the Book holds removed names | `tests/book/inventory_guard.rs:586-614` exempts two `E0150` output blocks (`content.html:291, 299, 3758, 3766`: `known`, `known_or`, `is_known`, `high()`); EU-18 AC 3 is false at HEAD | move C79's proof owner to T30 (where the exempt count goes to 0, as the Handoff says) and say so in tasks.md, or keep T18 and mark C79 red until T30 | 6e6a8a4f (confirmed) |
| 6. The E0187 exemption is sound but spec.md still contradicts it | `spec.md` EU-04 AC 4 makes any signal or `out` port without an initial value and without `Default` E0187; `part_i_values.md` §3.3.4 and `part_ii_carriers.md:243` exempt a carrier an undelayed definition drives and a signal held by a child's `out` port; T30 writes C14 against AC 4 | append a criterion to spec.md (EU-04 AC 7, new ID: no E0187 for a carrier driven by an undelayed definition or held by a child's `out` port; E0187 falls on that port) with a check owned by T30, and show it to the user, since it narrows an approved criterion | 0687a2d1 (confirmed) |
| 7. §3.3 says it states six rules and states seven | `docs/spec/part_i_values.md:1146` "states the six rules that follow"; §3.3.7 is a seventh static rule | "seven rules", or fold §3.3.7 into §3.3.5 | 130718cb (confirmed) |
| 8. C46's test does not assert the E0187 half of its criterion | `crates/piperine-lang/tests/spec_text.rs:1955-1965` asserts the default start and "needs no initial value", not §5.2.12's "a type without it is `E0187`" | assert that sentence | 1a8fdafd (confirmed) |
| 9. C83's test does not assert MD-70's own mark | `crates/piperine-cli/tests/docs.rs:342-363` checks MD-78's text and MD-64's mark; `STATE.md:1565` holds "Superseded whole by MD-78" unasserted | assert MD-70's body holds "Superseded whole by MD-78" | 07483c3a (confirmed) |
| 10. values~37a548f2 names the wrong tasks | `GAPS.md:499` says T21, T22 and T30 make it run; the refusal of an analysis on an undriven top `in` port is EU-04 AC 6, T38 (C16) | name T38 in the row | f2b241a3 (confirmed) |
| 11. Two new constants displaced the doc comments of existing functions | `crates/piperine-lang/tests/pom_resolve.rs:809-812`: `appendix_h_names`'s `///` lines now document `SPEC_TRAITS_AHEAD_OF_THE_TABLE`; `crates/piperine-lang/tests/std_library.rs:581-588`: `pub_names`'s `///` line now documents `INDEX_AHEAD_OF_STD` | move each constant above its function's doc comment | a649635a, f2999641 (confirmed) |
| 12. `INDEX_MEMBERS_AHEAD_OF_STD` has no stale-entry guard | `crates/piperine-lang/tests/std_library.rs:588` is used only as a filter (`:676`); `the_index_lists_stay_honest` (`:826`) checks the other two lists | assert in `the_index_lists_stay_honest` that each member is still undeclared | f2999641 (confirmed) |
| 13. tasks.md marks no task of B1-B3 done | `.specs/features/explicit-unknowns/tasks.md` lines 163-565: 62 `- [ ]` and no `- [x]` for T1-T18; `**Status**: Draft` | tick the Done-when boxes of T1-T18 | b99e513b (confirmed) |
| 14. docs/pom still teaches the `resolve` field with no owner | `docs/pom/reflection.md:635-657` (`nature Wire { value: bool, resolve: pick }`, `resolve_fn`), `docs/pom/model.md:608, 627, 1369`; `docs/pom/mapping.md:120` says the implementation keeps them, but no test fails when T28, T29 or `resolved-nets` remove them | register them with their owning tasks (T28, T29, `resolved-nets`) and guard the mapping note, as the other ahead-lists are | a78a516b (confirmed) |
| 15. The gate cannot read the two approval lines | `validate_verification.py` refuses `spec.md:3` and `tasks.md:13`: they read `**Approved**: 2026-11-21 - user, 2026-11-21: "Aceito os recomendados" ...`, not `**Approved**: <date> - user: "<their words>"`; the user's words are there | rewrite both lines in the gate's form, keeping the quoted words and the parenthetical after them | 5dc4beba (confirmed) |
| 16. `docs/pom/mapping.md` counts 65 constructs over a table of 62 | `docs/pom/mapping.md:53` "The 65 constructs Appendix B.1.2 lists"; its table has 62 rows, B.1 says "The language has **62** such constructs", `pom_docs.rs:216, 238` assert 62. The table was renumbered by B1-B3 and the sentence was not | "The 62 constructs" | closed 758c5ea9 (orchestrator; trivial, checked by pom_docs) |
| 17. `docs/pom/resolution.md` describes the `resolve` field and a library `Wire` outside the registry | `docs/pom/resolution.md:337` (the `E0261` row: "a discrete nature without `resolve`", "(`Wire`, `Electrical`, a signal-flow nature)"); not in `pom_docs::DocAheadOfTheCode::WAITING`, and resolved-nets T16 names only `reflection.md`, `model.md` and `mapping.md`, so nothing fails when T16 removes the field | register the row in `WAITING` with owner resolved-nets T16 and add `resolution.md` to T16's rewrite list | closed 1f5b35aa (orchestrator; pom_docs 16 pass) |
