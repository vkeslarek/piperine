# MSL Packages verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at d08dec18 (118 pass, 0 fail; only spec-doc commits since, verified against HEAD 150e45c6)
**Verifier**: independent sub-agent (author != verifier), Opus
**Round**: 1

Probes were throwaway targets copied into `crates/piperine-lang/tests/` and `crates/piperine-project/tests/`, run with `cargo test -p <crate> --test <probe> -- --nocapture`, then deleted. Sources kept in the session scratchpad (`verify-packages/zz_verify_probe.rs`, `zz_verify_probe_project.rs`).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `spec_text.rs` `s35_defines_crate_root_and_dependency_name` | yes - exact substrings and table cells of the decided wording |
| C2 | `spec_text.rs` `s35_orders_the_first_segment_lookup` | yes - exact substrings and table cells of the decided wording |
| C3 | `spec_text.rs` `s35_local_names_win_in_paths_and_clash_in_use` | yes - exact substrings and table cells of the decided wording |
| C4 | `spec_text.rs` `s35_crate_and_super_are_per_crate` | yes - exact substrings and table cells of the decided wording |
| C5 | `spec_text.rs` `s35_only_pub_crosses_a_crate` | yes - exact substrings and table cells of the decided wording |
| C6 | `spec_text.rs` `std_is_implicit_and_the_prelude_is_std_prelude` | yes - exact substrings and table cells of the decided wording |
| C7 | `spec_text.rs` `e0161_row_covers_an_unknown_crate` | yes - exact substrings and table cells of the decided wording |
| C8 | `spec_text.rs` `b36_lists_extern_crate` | yes - exact substrings and table cells of the decided wording |
| C9 | `msl_errors.rs` `extern_crate_is_a_removed_form` | yes - one `E0900` at `extern`, message and help asserted, `extern "C"` unaffected |
| C10 | `spec_text.rs` `e0163_is_defined_in_3_5` | yes - exact substrings and table cells of the decided wording |
| C11 | `spec_text.rs` `phase_package_is_listed` | yes - exact substrings and table cells of the decided wording |
| C12 | `spec_text.rs` `s36_fixes_the_package_layout` | yes - exact substrings and table cells of the decided wording |
| C13 | `spec_text.rs` `s36_lists_every_manifest_key` | yes - fixed (F11 closed at dd62877e); was: asserts a row for each key and the closing `E0171` sentence, but not that no other key row exists, which "exactly" claims (F11) |
| C14 | `spec_text.rs` `s36_names_are_identifiers` | yes - exact substrings and table cells of the decided wording |
| C15 | `spec_text.rs` `s36_states_the_dependency_sources` | yes - exact substrings and table cells of the decided wording |
| C16 | `spec_text.rs` `s36_gives_the_version_requirement_table` | yes - exact substrings and table cells of the decided wording |
| C17 | `spec_text.rs` `s36_has_no_search_path_and_no_cycle` | yes - exact substrings and table cells of the decided wording |
| C18 | `spec_text.rs` `s36_states_the_lockfile_rule` | yes - exact substrings and table cells of the decided wording |
| C19 | `spec_text.rs` `s36_defines_the_package_codes` | yes - exact substrings and table cells of the decided wording |
| C20 | `spec_text.rs` `appendix_g_records_the_package_decisions` | yes - exact substrings and table cells of the decided wording |
| C21 | `spec_text.rs` `glossary_and_file_map_name_crates_and_packages` | yes - exact substrings and table cells of the decided wording |
| C22 | `pom_crates.rs` `graph_keeps_crates_and_edges_as_given` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C23 | `pom_crates.rs` `graph_rejects_invalid_and_reserved_names` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C24 | `pom_crates.rs` `graph_rejects_a_cycle_and_names_it` | yes - exact `Cycle` names for a 3-cycle and a self-edge, graph unchanged |
| C25 | `pom_crates.rs` `graph_rejects_a_repeated_dependency_name` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C26 | `pom_crates.rs` `graph_rejects_a_foreign_id` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C27 | `pom_crates.rs` `piperine_lang_takes_the_graph_as_data` | yes - substring scan of Cargo.toml for the three names |
| C28 | `pom_crates.rs` `single_source_design_has_a_root_crate_named_by_its_file` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C29 | `pom_crates.rs` `every_design_holds_std_last` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C30 | `pom_crates.rs` `graph_crates_become_design_crates_in_order` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C31 | `pom_crates.rs` `mod_files_load_beside_their_own_crate_root` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C32 | `pom_crates.rs` `an_unreadable_crate_root_is_one_e0161` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C33 | `pom_crates.rs` `every_crate_keeps_its_authored_tree` | yes - every file's `to_ast` equals its parse; the three modules' ancestor chains end at their own crate root; root items per crate |
| C34 | `pom_crates.rs` `disk_and_memory_graphs_build_the_same_design` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C35 | `pom_crates.rs` `a_dependency_name_starts_a_path` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C36 | `pom_crates.rs` `an_instance_links_to_a_module_of_a_dependency` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C37 | `pom_crates.rs` `crate_names_the_root_of_the_containing_crate` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C38 | `pom_crates.rs` `a_renamed_dependency_resolves_by_its_dependency_name` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C39 | `pom_crates.rs` `a_local_name_wins_over_a_dependency_in_a_path` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C40 | `pom_crates.rs` `a_use_path_that_names_a_local_and_a_dependency_is_e0163` | yes - fixed (F11 closed at dd62877e); was: yes - one `E0163`, no `E0161`, both remedies in help, segments `Unresolved`, five use forms; the span is not asserted |
| C41 | `pom_crates.rs` `crate_prefix_names_the_local_one_in_a_clash` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C42 | `pom_crates.rs` `std_is_a_dependency_of_every_crate` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C43 | `pom_crates.rs` `the_prelude_reaches_every_crate` | yes - fixed (F3 closed at 4f2703dc); was: asserts the const `P` in expressions; the check names a port `p: Bit` (type namespace), which no test exercises (F3) |
| C44 | `pom_crates.rs` `a_dependency_name_hides_a_prelude_item` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C45 | `pom_crates.rs` `a_crate_that_is_no_dependency_is_e0161_with_help` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C46 | `pom_crates.rs` `a_private_item_of_another_crate_is_e0161` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C47 | `pom_crates.rs` `a_pub_item_in_a_private_namespace_is_e0161` | yes - fixed (F11 closed at dd62877e); was: asserts one `E0161`; the check says the message names `inner`, not asserted (F11) |
| C48 | `pom_crates.rs` `super_at_a_crate_root_is_e0161` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C49 | `pom_crates.rs` `a_glob_of_a_dependency_brings_only_pub_items` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C50 | `pom_crates.rs` `a_pub_use_re_exports_across_crates` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C51 | `pom_crates.rs` `embedded_std_builds_clean_and_has_a_prelude` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C52 | `pom_crates.rs` `prelude_items_resolve_without_an_import` | yes - fixed (F3 closed at 4f2703dc); was: const `P` in expressions, not the nature `Bit` in a port the check names (F3) |
| C53 | `pom_crates.rs` `only_pub_items_of_std_prelude_are_implicit` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C54 | `pom_crates.rs` `std_source_replaces_the_embedded_library` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C55 | `pom_crates.rs` `a_namespace_item_hides_a_prelude_item` | yes - fixed (F3 closed at 4f2703dc); was: local const hides prelude const; the check names a local `nature Bit` (F3) |
| C56 | `pom_crates.rs` `std_files_are_numbered_last` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C57 | `pom_crates.rs` `std_files_see_the_prelude` | yes - fixed (F3 closed at 4f2703dc); was: a const in `std` uses `P`; the check names a module port `p: Bit` (F3) |
| C58 | `pom_crates.rs` `a_crate_reports_its_name_root_and_files` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C59 | `pom_crates.rs` `declarations_and_files_know_their_crate` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C60 | `pom_crates.rs` `a_crate_lists_its_declared_dependencies` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C61 | `pom_crates.rs` `collections_list_the_root_crate_and_a_crate_lists_its_own` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C62 | `pom_crates.rs` `by_name_lookups_prefer_the_root_crate_then_crate_order` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C63 | `pom_crates.rs` `crates_are_found_by_name` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C64 | `pom_crates.rs` `crate_find_inverts_qualified_name` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C65 | `pom_crates.rs` `a_multi_crate_design_round_trips_through_json` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C66 | `pom_crates.rs` `from_json_rejects_out_of_range_crate_links` | yes - each of six links set to 99999 is `Err` naming 99999; a companion test proves no panic over 40 corruptions |
| C67 | `pom_crates.rs` `from_json_rejects_a_dependency_cycle` | yes - fixed (F11 closed at dd62877e); was: rejects 2-, 1- and 3-crate cycles and accepts a diamond; `contains('b')` holds for any message (F11) |
| C68 | `pom_crates.rs` `from_json_rejects_a_crate_whose_root_is_not_its_own` | yes - asserts the resolution/crate/value the criterion fixes and counts every diagnostic |
| C69 | `pom_docs.rs` `docs_describe_crates_and_no_prelude_sources` | yes - page text and the mapping row asserted |
| C70 | `pom_docs.rs` `mapping_has_a_crate_row` | yes - page text and the mapping row asserted |
| C71 | `cargo test -p piperine-lang --doc` | yes - lone proof `cargo test -p piperine-lang --doc` exit code |
| C72 | `manifest.rs` `a_missing_manifest_is_e0170` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C73 | `manifest.rs` `invalid_toml_is_e0170_at_its_span` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C74 | `manifest.rs` `a_package_error_renders_as_a_miette_report` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C75 | `manifest.rs` `a_valid_manifest_reads_every_package_field` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C76 | `manifest.rs` `an_unknown_key_is_e0171_at_the_key` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C77 | `manifest.rs` `a_missing_or_mistyped_package_key_is_e0171` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C78 | `manifest.rs` `a_bad_package_name_is_e0171` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C79 | `manifest.rs` `a_bad_version_is_e0171` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C80 | `manifest.rs` `every_fault_of_a_manifest_is_reported` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C81 | `manifest.rs` `dependencies_read_in_source_order` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C82 | `manifest.rs` `a_dependency_needs_exactly_one_source` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C83 | `manifest.rs` `git_refs_are_exclusive_and_git_only` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C84 | `manifest.rs` `a_bad_dependency_name_or_requirement_is_e0171` | yes - one `E0171`/`E0170` each, span text asserted, plus extra cases |
| C85 | `paths.rs` `path_dependencies_are_relative_to_their_manifest` | yes - `../libs/util` then `../base` only reachable relative to each manifest; cwd is the crate dir, unrelated |
| C86 | `paths.rs` `the_graph_mirrors_the_manifests` | yes - code, file and span text asserted, positive and negative case |
| C87 | `paths.rs` `a_shared_dependency_is_one_crate` | yes - code, file and span text asserted, positive and negative case |
| C88 | `paths.rs` `a_path_without_a_package_is_e0172` | yes - code, file and span text asserted, positive and negative case |
| C89 | `paths.rs` `a_package_without_a_crate_root_is_e0175` | yes - code, file and span text asserted, positive and negative case |
| C90 | `paths.rs` `a_package_of_another_name_is_e0173` | yes - code, file and span text asserted, positive and negative case |
| C91 | `paths.rs` `package_key_renames_a_dependency` | yes - code, file and span text asserted, positive and negative case |
| C92 | `paths.rs` `version_requirements_follow_the_caret_table` | yes - eight rows on both sides; `0.0`/`0` skip "below" correctly (nothing is below 0.0.0) |
| C93 | `paths.rs` `an_unsatisfied_requirement_is_e0173` | yes - code, file and span text asserted, positive and negative case |
| C94 | `paths.rs` `a_dependency_cycle_is_e0174_with_its_chain` | yes - code, file and span text asserted, positive and negative case |
| C95 | `paths.rs` `a_package_that_depends_on_itself_is_e0174` | yes - code, file and span text asserted, positive and negative case |
| C96 | `git.rs` `a_git_dependency_checks_out_the_default_branch` | yes - local repositories; file content and commit asserted per case |
| C97 | `git.rs` `branch_tag_and_rev_select_their_commit` | yes - local repositories; file content and commit asserted per case |
| C98 | `git.rs` `an_unreachable_repository_or_ref_is_e0172` | yes - local repositories; file content and commit asserted per case |
| C99 | `git.rs` `two_commits_of_one_repository_are_two_crates` | yes - local repositories; file content and commit asserted per case |
| C100 | `git.rs` `one_commit_of_one_repository_is_one_crate` | yes - local repositories; file content and commit asserted per case |
| C101 | `git.rs` `a_git_package_resolves_its_own_dependencies` | yes - local repositories; file content and commit asserted per case |
| C102 | `lock.rs` `the_lockfile_records_each_git_package` | yes - exact lockfile bytes or commits asserted |
| C103 | `lock.rs` `a_locked_commit_is_kept_when_the_branch_moves` | yes - exact lockfile bytes or commits asserted |
| C104 | `lock.rs` `no_git_package_no_lockfile` | yes - exact lockfile bytes or commits asserted |
| C105 | `lock.rs` `the_lockfile_is_deterministic` | yes - exact lockfile bytes or commits asserted |
| C106 | `lock.rs` `update_moves_the_lock_to_the_new_commit` | yes - exact lockfile bytes or commits asserted |
| C107 | `lock.rs` `stale_entries_leave_the_lockfile` | yes - exact lockfile bytes or commits asserted |
| C108 | `lock.rs` `an_invalid_lockfile_is_e0176` | yes - truncated, `version = 2` and no-version each one `E0176` in the lockfile, `target/deps` absent; also under `update` |
| C109 | `lock.rs` `a_locked_checkout_needs_no_repository` | yes - source repository and the `db` clone deleted, second resolution reads the kept checkout |
| C110 | `lock.rs` `only_the_root_lockfile_counts` | yes - fixed (F2 closed at 0e76f02f); was: the dependency's lockfile pins the dependency itself, which is checked out before that file could be read; a resolver that merged dependency lockfiles still passes (F2) |
| C111 | `project.rs` `the_project_root_is_the_nearest_manifest` | yes |
| C112 | `project.rs` `load_project_builds_the_design_of_a_package_graph` | yes |
| C113 | `project.rs` `a_package_error_stops_load_project` | yes |
| C114 | `project.rs` `a_model_error_is_a_diagnostic_not_a_package_error` | yes |
| C115 | `project.rs` `the_crate_holds_no_retired_surface` | yes - Cargo.toml deps, three files and five words over `src/`; spec says "sources", tests intentionally spell `[plugins]` |
| C116 | `project.rs` `the_crate_reports_exactly_the_package_phase_codes` | yes |
| C117 | `cargo build -p piperine-project --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')` | yes - lone proof: build of all targets, fails on any `warning`/`error` line |

## Known items judged

- T1 tests after spec text: a process deviation only; the C1 to C10 guards are exact-substring and would fail on any other wording. T2 to T25 commit bodies each record a red run (spot-checked all 27 code/spec commits since 6f65df5a).
- B2 cascade: confirmed, see F4. Cycle name order (`a -> b -> c -> a`, starting at the dependency's target) is consistent between `CrateGraph` and the resolver; no finding. Keyword/`std` root-file stems: F7.
- B3 guard scans `src/` only: matches PKG-15 AC 1 ("its sources"); the tests spell `[plugins]` on purpose. Accepted. Missing-key spans widened to one character: accepted (renders an underline).
- B4 E0176 for more cases (no `version`, unknown key, entry not a table): all are "not a valid lockfile"; accepted. Non-cycle `GraphError` as E0171: unreachable from a valid manifest (names validated, TOML keys unique); accepted. No test for interim T19 criterion: superseded by C88 to C95. Caret rows `0.0`/`0` skip "below": correct. `tests/support` item-level `#[allow(dead_code)]`: not file-scope, allowed. Reworded commit subjects: no effect.
- `crates/piperine-project/build.rs` removal: the crate builds clean (C117) and nothing references it.
- U6: holds as the spec states it. Probes: ordinary type, expression and pattern paths (`util::E::A` in a `match`) pick the local `mod util` with no diagnostic; `use util::K`, `use util::{K}`, `use util::*`, `use util as u`, a block `use`, and a glob-imported local `util` are each one `E0163`; `use crate::util::K` and `use self::util::K` resolve locally; a nested `mod m { use util::K; }` reaches the dependency (Rust-correct). One divergence from "Rust 2018 exactly": F10.
- POM navigability: holds. Each crate keeps its own root (C33); with a populated `std_source` (a module in `std` and in `std::prelude`) `design.modules()`, `consts()`, `namespaces()` list only the root crate's items and `root_crate().decls()` holds no `std` declaration.

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (Medium) - a requirement the manifest reader accepts panics the resolver: `Requirement::admits` overflows on `major + 1`, `minor + 1`, `patch + 1` | `crates/piperine-project/src/manifest.rs:56` and `:62`. Repro: `x = { path = "p", version = "18446744073709551615" }` parses; `admits(1.0.0)` panics "attempt to add with overflow"; same for `"0.0.18446744073709551615"`. In release it wraps and rejects every version with a wrong E0173 | compute the bound with `checked_add` (no upper bound on overflow) or reject components that cannot be bumped as E0171; add the case to C92's test | f5ab1373 |
| F2 (Medium) - C110's test cannot fail: a resolver that also honoured dependencies' lockfiles passes it | `lock.rs` `only_the_root_lockfile_counts`: the dependency's lockfile pins the dependency itself, which is already checked out before that file exists on disk. Probe: `util`'s committed lockfile pins its own git dependency `base` at an old commit; resolution takes `base`'s head (behaviour correct, untested) | pin a transitive git package in the dependency's lockfile and assert the root resolution ignores it | 0e76f02f |
| F3 (Low) - C43, C52, C55, C57 name a `Bit` nature in a port type; their tests use a `const P` in expressions only, so the type namespace of the prelude is never exercised | `pom_crates.rs` `the_prelude_reaches_every_crate`, `prelude_items_resolve_without_an_import`, `a_namespace_item_hides_a_prelude_item`, `std_files_see_the_prelude`. Probe: `pub mod prelude { pub nature Bit { value: f64 } }` with `module M(a: Bit)` in a dependency, in `std`, and a local `nature Bit` all resolve as claimed | add the port-type case each check names to its test | 4f2703dc |
| F4 (Low) - an `E0163` import binds nothing, so each later use of its name adds an `E0161` | Repro: app `mod util { pub const K: u32 = 5; }\nuse util::K;\nconst X: u32 = K;` with dependency `util` gives `["E0163", "E0161 cannot find K"]`. PKG-05 AC 6 forbids an `E0161` only for the `use` path itself, so it is within the letter; rustc binds an error placeholder and reports once | decide: bind the name to an error resolution that suppresses further diagnostics, or record the cascade as intended in §3.5 | b4537a14 |
| F5 (Low) - `Design::from_json` accepts a crate table that breaks PKG-04 AC 2: `root_crate`/`std_crate` swapped (std not last, `std_crate()` named `app`) and a dependency name that is no identifier | Probe: swap `root_crate` and `std_crate` in the JSON of `app -> util` -> `Ok`, `root_crate().name() == "std"`; set a dependency name to `"not a name"` -> `Ok`. `check_crates` validates ranges, roots and cycles only | `check_crates`: root crate is 0, std crate is the last and named `std`, every crate and dependency name passes `is_crate_name` | ca38f5b7 |
| F6 (Low) - the lockfile `commit` is not checked against §3.6.14 (40 hexadecimal digits) | `lockfile.rs` `parse_at` takes any string. Probe: `commit = "<branch name>"` is accepted and silently rewritten; `commit = "zz"` is `E0172` (not `E0176`); the value is joined as a directory name under `target/deps/<url>/` (`resolver.rs` `home.join(locked)`), so `".."` is taken as an existing checkout | reject a commit that is not 40 hex digits as `E0176` at its span | f38976ca |
| F7 (Low) - a root file stem that is a keyword or `std` names the root crate: `std.msl` gives two crates named `std`, and `krate("std")` returns the root crate, not `std_crate()`; `fn.msl` gives a crate named `fn` | Probe with `Design::builder().source("std.msl", ..)`: crates `["std", "std"]`, `krate("std").is_std() == false`. PKG-04 AC 1 sanitises non-identifiers only; §3.6.4 forbids these names for packages | decide the rule (e.g. append `_` as for a leading digit) and add the cases to C28's table | 4eb455d3 |
| F8 (Low) - one repository spelled two ways is two crates at one commit | Probe: `mid` names `<repo>/`, `app` names `<repo>`, same default branch -> 4 crates instead of 3. §3.6.10: "one repository at one commit" names one crate. `Identity::Git` keys on the raw URL string | normalise the URL (trailing `/`, `.git`, a local path canonicalised) before keying, or state in §3.6.10 that the URL is compared as written | 0bed27bc |
| F9 (Low) - `rev` accepts any revspec, not only a commit id | Probe: `rev = "<7-char prefix>"`, `rev = "master"`, `rev = "HEAD"` each resolve (`git.rs` `revparse_single(r)`). C97 says "a full commit id"; §3.6.7 leaves `rev` unconstrained | decide whether `rev` is a commit id (reject others as E0172/E0171) and state it in §3.6.7 | 17551fe0 |
| F10 (Low) - U6 says "Rust 2018 exactly", but the clash ignores namespaces: a local value item shadows a dependency in a type path | Probe: app `fn util() -> u32 { 1 }\nconst X: u32 = util::K;` with dependency `util` -> `E0161 cannot find K` (Rust resolves `util::` in the type namespace to the crate); `use util::K;` beside `fn util` -> `E0163` (no ambiguity in Rust). Matches the namespace-blind wording of §3.5.4/5 | user decision: keep the spec's rule (and drop "exactly" from the record) or make rules 4 and 5 consider only items of the type namespace | 30bc9ab0 |
| F11 (Low) - assertions looser than their check | C13 checks each key row is present, not that no other key row exists ("exactly those eleven"); C47 asserts the code, not that the message names `inner`; C67 `p.contains('b')` holds for any message containing "b" | count the key rows; assert `inner` in C47's message; assert the chain `app -> b -> app` | dd62877e |
