# msl-stdlib verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at e12c8946 (HEAD 5a267335 adds only `.specs/` commits)
**Verifier**: independent sub-agent (author != verifier), Opus, round 1
**Diff range**: 0ae471d1..5a267335 (implementation a7fb0985..2dde7ddc)

Summary: 113 checks, 107 hold, 5 weak, 1 no. Two Medium findings (one test that cannot fail, one rule of STD-06 not implemented), three Low, two Info. The library itself is sound: the hand-traced bodies match their sources, the intrinsic list is exactly the five string readers, `Transient` is complete, and the galleries build clean.

## Judgement of the library bodies (Q2 "as many as possible")

| Item | Verdict | How judged |
| --- | --- | --- |
| `pnjlim`, `fetlim`, `limvds` | correct against the decision | compared branch by branch with `git show e129b869:crates/piperine-ir/src/limit.rs`: same tests, same constants; `ng_min`/`ng_max` are written as `a < b ? a : b` / `a > b ? a : b`, identical including NaN and ties. `fetlim` and `limvds` also match ngspice `devsup.c`. `pnjlim` does not (F4) |
| `deltalim` | correct | `new.clamp(old - delta, old + delta)`, the §22.2.4 Resolution verbatim |
| `db10`, `db20` | correct | `10·log10(x)`, `20·log10(|x|)`; §22.2.16 rule 2 states the non-finite cases |
| `normal` | correct | SplitMix64 constants (`0x9E3779B97F4A7C15`, `0xBF58476D1CE4E5B9`, `0x94D049BB133111EB`, shifts 30/27/31) match the reference; integer wrap is §1 rule 6; `(z >> 11) as f64 / 2^53` is in [0, 1); Box-Muller on `1 - a` in (0, 1] keeps `ln` finite; two draws and the state after the second |
| `poly_q`, `den_q` | correct | coefficient update `c[k] -= ρ·c[k-1]` (complex), descending `k`, leading 1 kept; `den_q` drops the leading 1, matching §22.2.12 rule 1 (`1 + Σ den[k] z^-(k+1)`) |
| `poly_s` | correct | `1/ρ = (a/m, -b/m)`; a zero root shifts the polynomial up one power (factor `s`) and clears `c[0]` |
| `conjugate_closed` | correct | per complex root, count of its conjugate equals count of itself |
| `zi_zp`/`zi_zd`/`zi_np` length rules | correct | `poly_q` of N roots is N+1 coefficients; `zi_nd`'s generics infer from the arrays |
| `laplace_nd` | correct for `den[D-1] != 0` | controllable canonical form: `w[k] = W^(k)` for k < D-1, `top = W^(D-1)`, the last row is `den(s)·W = x`, the output `Σ num[k]·W^(k)`; D == 1 gives `num[0]·x/den[0]`. The length rule `N <= D` does not make H proper when the top coefficient is zero (F3) |
| `laplace_zp`/`zd`/`np` heads | correct | `N <= D`, `N < D`, `N <= D + 1` are exactly the length rules that give `laplace_nd` `N' <= D'` |
| `noise_table` | correct | `noise(table_model(freq(), &table, "1C"), name)`, the §22.2.3 Resolution |
| `same` | correct | level-by-level `Option<bool>` equality, unknown equal to unknown |
| intrinsic list | exact | `extern "intrinsic"` appears only in `std/values.msl` (`matches_bits`, `logic`), `std/analog.msl` (`table_model`), `std/io.msl` (`parse_table`, `parse_hex`); each takes a `&str`; nothing outside `std/` and the tests declares one |

## Other questions of the dispatch

- **`Transient` everywhere.** `grep -rnw Tran` over `docs/spec`, `docs/pom`, `docs/book/content.html`, `src/` and `std/` finds only the module of §22.3.7 (Index rows, §22.3.7/8, F gallery, Appendix C/G prose). The built-in row is `Transient`; `docs/pom/resolution.md` and `model.md` say `Transient`; the Book's kind list says `Transient`. Holds.
- **Galleries.** Zero diagnostics over every file, `check()` empty, no Unresolved slot in the root crate, the control against `pub mod prelude {}` sees E0161 in every fence. Holds.
- **The S1 scoping.** The nine tests the T9 commit names (seven via `support/pom.rs::build`, two in `pom_crates.rs`) assert the structure and resolution of root-crate items; over an empty std every assertion is kept and still meaningful, so they are scoped, not weakened. `from_json_never_panics_on_a_corrupted_field` ran over an empty std before this feature too, so its coverage is unchanged. But `build()` is now the default for every later test, and one new test inherited it where its criterion needs the real prelude (F1).
- **Performance.** Not a per-build POM problem (F6): the 220 s is the fuzz's shape, one full serialize + `from_json` per numeric leaf.
- **Process.** T16 before T15 is a real dependency (`monitor` calls `require` of `io`) and was disclosed. T6 and T7 were amended 19 s and 4 s after their first commit, before the next commit, locally (reflog) — benign. The tests noted green-before are negative controls (`a_nature_impl_wins_over_a_blanket`, `an_unlinked_member_keeps_its_slot`, `a_private_std_item_is_e0161`) that hold on both sides by construction; each of their features has a positive test that was red. No finding.

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `spec_text.rs` `s22_says_std_is_the_normative_source` | yes |
| C2 | `spec_text.rs` `s22_maps_sections_to_namespaces` | yes |
| C3 | `spec_text.rs` `s22_drops_the_implied_pub_resolution` | yes |
| C4 | `spec_text.rs` `g38_is_closed` | yes |
| C5 | `spec_text.rs` `appendix_c_says_parsed_and_resolved` | yes |
| C6 | `spec_text.rs` `s1_constants_are_std_items` | yes |
| C7 | `spec_text.rs` `analysis_kinds_list_transient` | yes |
| C8 | `spec_text.rs` `no_analysis_tran_remains` | yes |
| C9 | `spec_text.rs` `tran_in_fences_is_the_module` | yes |
| C10 | `spec_text.rs` `appendix_g_records_the_stdlib_decisions` | yes |
| C11 | `spec_text.rs` `s3_lists_the_intrinsic_abi` | yes |
| C12 | `spec_text.rs` `s3_states_the_intrinsic_contract` | yes |
| C13 | `spec_text.rs` `e0125_covers_intrinsic_outside_std` | yes |
| C14 | `spec_text.rs` `s22_lists_the_engine_items` | yes |
| C15 | `spec_text.rs` `s22_rule_4_names_extern_intrinsic` | yes |
| C16 | `spec_text.rs` `b1_still_counts_65_constructs` | yes |
| C17 | `spec_text.rs` `e0707_is_replaced_by_e0317` | weak - asserts `E0317` appears in §22.2.13 and §22.2.14, not that it is given for the two named conditions (F5) |
| C18 | `spec_text.rs` `laplace_states_the_proper_rule` | yes |
| C19 | `spec_text.rs` `g64_names_the_normal_generator` | yes |
| C20 | `pom_resolve.rs` `intrinsic_fn_is_accepted_in_std` | yes |
| C21 | `pom_resolve.rs` `intrinsic_fn_outside_std_is_e0125` | yes |
| C22 | `pom_resolve.rs` `intrinsic_fn_with_a_body_is_e0125` | yes |
| C23 | `pom_resolve.rs` `is_intrinsic_reads_the_abi` | yes |
| C24 | `pom_resolve.rs` `extern_c_fn_is_not_intrinsic` | yes |
| C25 | `pom_resolve.rs` `a_call_to_an_intrinsic_resolves` | yes |
| C26 | `pom_resolve.rs` `self_assoc_resolves_through_repr_supertrait` | yes |
| C27 | `pom_resolve.rs` `self_assoc_resolves_through_a_user_supertrait` | yes |
| C28 | `pom_resolve.rs` `self_assoc_missing_everywhere_is_e0161` | yes |
| C29 | `pom_resolve.rs` `blanket_method_resolves_on_a_carrier` | weak - positive case holds, but the criterion's "a visible trait" is neither tested nor implemented: a private trait of another namespace links (F2) |
| C30 | `pom_resolve.rs` `blanket_method_resolves_on_self_in_a_nature_impl` | yes |
| C31 | `pom_resolve.rs` `a_nature_impl_wins_over_a_blanket` | yes |
| C32 | `pom_resolve.rs` `an_unlinked_member_keeps_its_slot` | yes |
| C33 | `pom_resolve.rs` `a_blanket_call_is_a_reference_of_the_trait_method` | yes |
| C34 | `std_library.rs` `std_root_declares_nine_namespaces` | yes |
| C35 | `std_library.rs` `each_namespace_holds_its_section` | yes |
| C36 | `std_library.rs` `prelude_is_eight_reexports` | yes |
| C37 | `std_library.rs` `embedded_list_names_every_file_once` | yes |
| C38 | `std_library.rs` `every_std_file_is_embedded` | yes |
| C39 | `std_library.rs` `every_index_item_is_declared` | yes |
| C40 | `std_library.rs` `every_item_but_the_intrinsics_has_a_body` | yes |
| C41 | `std_library.rs` `intrinsics_are_exactly_the_engine_table` | yes - set equality with the §22 table plus `len() == 5` |
| C42 | `std_library.rs` `every_intrinsic_reads_a_str` | yes |
| C43 | `std_library.rs` `every_index_item_is_pub` | yes |
| C44 | `std_library.rs` `std_declares_no_core_name` | yes |
| C45 | `std_library.rs` `constants_have_the_values_of_s1` | yes - parsed values compared exactly; `inf` text compared |
| C46 | `std_library.rs` `root_forms_place_their_coefficient_form` | yes |
| C47 | `std_library.rs` `root_params_carry_a_where` | yes |
| C48 | `std_library.rs` `laplace_nd_has_state_signals_and_an_analog_block` | yes |
| C49 | `std_library.rs` `noise_table_is_noise_of_table_model` | yes |
| C50 | `std_library.rs` `pure_bodies_name_no_primitive` | yes |
| C51 | `std_library.rs` `tgate_places_two_tran` | yes |
| C52 | `std_library.rs` `a_std_item_resolves_by_its_single_name` | yes |
| C53 | `std_library.rs` `prelude_exports_equal_the_index` | yes - exports read from the namespaces the prelude globs, compared both ways with the Index plus the 11 extras |
| C54 | `std_library.rs` `a_std_path_names_the_same_declaration` | yes |
| C55 | `std_library.rs` `a_private_std_item_is_e0161` | yes |
| C56 | `std_library.rs` `no_export_has_a_core_name` | yes |
| C57 | `std_library.rs` `bit_rise_resolves_into_std_values` | yes |
| C58 | `std_library.rs` `known_resolves_into_std_values` | yes |
| C59 | `std_library.rs` `converters_resolve_into_std_convert` | yes |
| C60 | `std_library.rs` `monitors_resolve_into_std_monitor` | yes |
| C61 | `std_library.rs` `embedded_std_has_no_diagnostics` | yes |
| C62 | `std_library.rs` `std_has_no_unresolved_member` | yes - `decls().exprs()` includes behavior blocks; probe over every std expr: 129 member slots, 0 Unresolved |
| C63 | `std_library.rs` `embedded_std_passes_check` | yes |
| C64 | `std_library.rs` `a_replaced_std_without_an_item_is_e0161` | yes |
| C65 | `std_library.rs` `transient_is_the_kind_and_tran_the_module` | yes |
| C66 | `std_excerpts.rs` `every_part_vii_fence_is_in_its_file` | yes - whole-line consecutive match; control `a_fence_one_character_off_is_not_found` fails on one character |
| C67 | `std_excerpts.rs` `part_vii_examples_are_fragments` | yes |
| C68 | `std_excerpts.rs` `the_forward_guard_covers_part_vii` | yes |
| C69 | `std_excerpts.rs` `a_missing_fence_is_named` | yes |
| C70 | `std_excerpts.rs` `a_fence_one_character_off_is_not_found` | yes |
| C71 | `std_excerpts.rs` `every_std_item_is_in_a_fence` | weak - the reverse guard accepts `rust,fragment` fences and matches by substring, looser than "inside a `rust` fence" (F5) |
| C72 | `std_excerpts.rs` `every_std_behavior_block_is_in_a_fence` | weak - same matcher as C71: any non-compile_fail fence, substring match (F5) |
| C73 | `std_excerpts.rs` `prelude_items_outside_part_vii_are_shown_pub` | yes |
| C74 | `std_excerpts.rs` `a_missing_item_is_named` | yes |
| C75 | `std_excerpts.rs` `the_reverse_guard_covers_std` | yes |
| C76 | `std_galleries.rs` `gallery_fixtures_exist` | yes |
| C77 | `std_galleries.rs` `every_fixture_line_is_spec_text` | yes |
| C78 | `std_galleries.rs` `the_fixture_table_covers_every_gallery` | yes |
| C79 | `std_galleries.rs` `the_fixture_table_names_only_used_models` | yes |
| C80 | `std_galleries.rs` `every_gallery_builds_with_no_diagnostics` | yes - counts diagnostics of every file, std included; 18 fences |
| C81 | `std_galleries.rs` `every_gallery_passes_check` | yes |
| C82 | `std_galleries.rs` `no_gallery_has_an_unresolved_member` | yes |
| C83 | `std_galleries.rs` `every_gallery_references_its_std_items` | yes |
| C84 | `std_galleries.rs` `the_gate_builds_18_fences` | yes |
| C85 | `std_galleries.rs` `a_dirty_gallery_is_reported_by_fence` | yes |
| C86 | `std_galleries.rs` `galleries_fail_against_an_empty_prelude` | yes - control builds against `pub mod prelude {}`; at least one E0161 per fence |
| C87 | `std_galleries.rs` `every_gallery_lists_a_std_item` | yes |
| C88 | `pom_resolve.rs` `the_constants_are_not_builtin` | yes |
| C89 | `pom_resolve.rs` `k_b_resolves_to_std_constants` | yes |
| C90 | `pom_resolve.rs` `transient_is_a_builtin_variant` | yes |
| C91 | `pom_resolve.rs` `the_core_rows_are_unchanged` | yes |
| C92 | `pom_resolve.rs` `a_model_constant_hides_the_prelude_constant` | no - built over an empty std (`support/pom.rs::build`), so there is no prelude `Q`/`PI` to hide; the test passes whatever the prelude does. Probe over the real std: behavior is correct (F1) |
| C93 | `std_library.rs` `the_fixture_prelude_is_gone` | yes |
| C94 | `std_library.rs` `no_pom_test_was_removed` | yes - counts `#[test]` lines against the floors |
| C95 | `pom_docs.rs` `the_builtin_table_doc_has_no_const_row` | yes |
| C96 | `pom_docs.rs` `resolution_doc_describes_intrinsics` | yes |
| C97 | `pom_docs.rs` `resolution_doc_describes_the_supertrait_rule` | yes |
| C98 | `pom_docs.rs` `members_doc_lists_the_blanket_step` | yes |
| C99 | `cargo test -p piperine-lang --doc` | yes - lone proof `cargo test -p piperine-lang --doc` exercises docs/pom examples against the embedded std |
| C100 | `std_library.rs` `std_root_comment_names_the_namespaces` | yes |
| C101 | `book.rs` `the_book_source_is_in_docs_book` | yes |
| C102 | `book.rs` `build_py_writes_an_ignored_index` | yes |
| C103 | `book.rs` `the_core_std_chapter_follows_packages` | yes |
| C104 | `book.rs` `the_chapter_names_every_core_name` | yes |
| C105 | `book.rs` `the_chapter_has_a_part_per_namespace` | yes |
| C106 | `book.rs` `the_chapter_names_every_prelude_export` | yes |
| C107 | `book.rs` `the_chapter_explains_the_prelude_and_the_engine_items` | weak - `std::prelude` is checked anywhere in the chapter, not under the "engine" heading; the criterion is ambiguous (F5) |
| C108 | `book.rs` `chapter_blocks_build_against_std` | yes |
| C109 | `book.rs` `every_msl_block_parses` | yes |
| C110 | `book.rs` `packages_and_cheatsheet_name_std` | yes |
| C111 | `book.rs` `the_book_has_at_least_40_msl_blocks` | yes |
| C112 | `book.rs` `a_missing_name_is_listed` | yes |
| C113 | `book.rs` `the_book_says_transient` | yes |

## Findings

| Finding | Severity | Evidence | Repro | Fix | Closed |
| --- | --- | --- | --- | --- | --- |
| F1 C92's test cannot fail: `a_model_constant_hides_the_prelude_constant` builds over an empty std, so no prelude `Q` or `PI` exists to be hidden | Medium | `pom_resolve.rs` `a_model_constant_hides_the_prelude_constant` calls `build`, which is `support/pom.rs::build` = `std_source("lib.msl","pub mod prelude;\n").std_source("prelude.msl","")` | probe over the real std: `const Q: f64 = 1.0; const Y: f64 = Q; mod m { const PI: f64 = 3.0; const Z: f64 = PI; }` gives no diagnostic, `Q -> Q` and `PI -> m::PI` (root crate). Behaviour is right; the test does not hold it | build that test with `Design::builder().source("top.msl", ..)` (the embedded std), and assert both uses resolve to root declarations | |
| F2 STD-06 AC 1 says "a visible trait"; `blanket_method` scans every `impl` of the design, so a carrier method links to a trait no scope can see, and the first impl in declaration order wins silently | Medium | `src/pom/resolve/members.rs:153` `blanket_method` iterates `0..decl_count()` with no visibility or ambiguity check; the T7 commit body names the gap; C29's test has no negative case for an invisible trait | `mod a { trait Hidden { fn zz(self) -> bool; } impl<T: Repr> Hidden for T { fn zz(self) -> bool { true } } }` + `module M(in p: Bit) { let x = p.zz(); }`: no diagnostic, `p.zz()` resolves to `a::Hidden.zz` | filter blanket impls to traits visible at the call (in scope or through the prelude); add the negative case to C29's test. If the orchestrator decides "all traits" is intended, amend STD-06 AC 1 with the user instead | |
| F3 `laplace_nd`'s proper rule is on array lengths only: a zero top coefficient (`den[D-1] == 0`) gives an improper H(s) and fewer poles than the `D-1` states rule 5 promises | Low | `std/analog.msl:250` `den: [f64; D] where den.iter().any(|d| *d != 0.0)`; §22.2.14 rule 4 states `N <= D` | `laplace_nd(v(a)) { num: [0.0, 1.0], den: [1.0, 0.0] }` builds with no diagnostic; by the body, the last row is `w[0] = x` and `y = ddt(x)`, H(s) = s (the case Q6 excluded). (The POM evaluates no `where`, so this is a spec/body gap, not a test gap) | add `den[D - 1] != 0.0` to the `where` of `den` in `laplace_nd` and Part VII (rule 3 or 4) | |
| F4 Part VII §22.2.4 Resolution says `pnjlim` is "the ngspice function … written in MSL, branch by branch"; ngspice's `DEVpnjlim` is not that function | Low | `/home/keslarek/Git/vams-corpus/sources/ngspice/src/spicelib/devices/devsup.c:50-84`: forward step `vold + vt*(2+log(arg-2))` with `arg = dv/vt`, a separate step for `arg <= 0`, and a reverse-bias branch (`vnew < 0` limited to `-vold-1` / `2*vold-1`). The std body is the SPICE3 form from `limit.rs` (`1 + dv/vt`, no reverse branch). `fetlim`/`limvds` do match devsup.c | read devsup.c beside `std/analog.msl:21-27` | reword the Resolution: "transliterated from Piperine's former `limit.rs` (the SPICE3 form)"; or port the current devsup.c branches (a user decision, since the spec chose `limit.rs`) | |
| F5 Three assertions looser than their criterion: C17 checks only that `E0317` appears in §22.2.13/14; C71/C72's reverse guard accepts `rust,fragment` fences and matches by substring; C107 looks for `std::prelude` anywhere in the chapter, not under the "engine" heading | Low | `spec_text.rs` `e0707_is_replaced_by_e0317`; `std_excerpts.rs` `all_fence_texts` (filters only `CompileFail`) and `item_failures` (`f.contains(&wanted)`); `book.rs` `the_chapter_explains_the_prelude_and_the_engine_items` | e.g. moving a std item's only listing into a `rust,fragment` fence keeps C71 green | C17: assert the sentence that names both conditions; C71/72: take `Class::Valid` fences only; C107: settle the wording with the orchestrator, then assert where `std::prelude` sits | |
| F6 Cost of the embedded std, measured: an empty design builds in 151 ms (debug), serializes to 745 KB JSON with 30 141 numeric leaves, `from_json` 160 ms. The 220 s fuzz is the test's shape (≈0.27 s per mutated leaf; the full fuzz over the real std would run about 4.5 h), not a slow POM | Info | probe `p1_timing` / `p5_from_json_sampled_over_real_std` at HEAD | 804 sampled mutations over the real std ran in 216 s with 0 panics (the S3 class did not reappear) | none required. Optional: let `Design` serialization skip the embedded std (it is reconstructible) | |
| F7 `analysis(Tran)` in a model still builds with no diagnostic: `Tran` resolves to the module of `std::nets` | Info | probe: `module M() { let k = analysis(Tran); }`: diags `[]`, `Tran -> Decl(nets::Tran)` | as stated | none now (no type checker); the doc and Book guards stop the spelling in the specification. A later type pass should reject a module as an `Analysis` argument | |
