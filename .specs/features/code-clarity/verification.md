# Code Clarity verification

## Phase 1 (B0 Ground, T1 to T8)

**Verdict**: FAIL (one Medium finding, five Low)
**Head**: 013901355 (T8 9e74b6af2 plus the merge of origin/main), base 93c831ffc
**Verifier**: independent sub-agent (Opus; author != verifier). No `proofs.md` exists yet: `check_proofs.py` runs after T155. This round reran the phase gates itself.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, then filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | lib 33, boundaries 26, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 24, worklists 4: all pass |
| `cargo test --no-fail-fast -p piperine-solver -p piperine-lang -p piperine-cli -p piperine-transpile` | 185 result lines, 2 338 passed, 0 failed |
| CC-09: `git diff 93c831ffc 013901355` over `crates/piperine-parity`, goldens, twins, `tests/`, `examples/`, `contract.rs` | empty |

### Checks

| Check | Test | Holds |
| --- | --- | --- |
| T1 MD-n entry | `.specs/STATE.md` MD-85 | yes: names the guards crate with its allowances, dev-only test support, one module layout, `LanguageItems`, `Refusal` and the MD-28 amendment; the Handoff names the base and the answers to OQ1 to OQ5 |
| T2 no reference left | `git grep -iE 'piperine-lang-server\|editors/vscode' -- . ':!.specs'` | yes: only GAPS B3's history line and the planted detector fixtures in `suite_hygiene.rs`. `release.yml` builds and packs `piperine-cli` only, `release` now `needs: build`, and `ide.yml` and `.vscode/` are gone. CLAUDE.md drops "never --workspace bare". GAPS B3 reads "No language server" and matches the summary row at GAPS:338; ROADMAP's `lang-server` row agrees |
| T3 scan rules (Appendix G) | `item_scan.rs` unit tests (13) | yes: one test per rule, including the nested `fn`, the `#[cfg(test)] mod x;` file, the `#[path]` include and the 15-line closure. `#![deny(missing_docs)]` is present, and the manifest has no in-tree dependency |
| C27 / T4 ledger | `proof_ledger::every_proof_names_a_test_that_exists` (and the other three) | yes: a planted dangling proof fails the test, and so does a stale allowance row (see below). `repoint.rs` has one byte-for-byte unit test per row kind |
| T5 moved tests keep names | the 46 base `#[test]` names of `workspace_shape`, `crate_boundaries` and `suite_hygiene` | yes: all 46 exist under `boundaries` and `suite`, and 4 are new. `repoints/T5.tsv` moves the three targets and replaces the 16 solver-restructure proofs. The 19 new lines of `solver-abi-integration/removed.txt` are the base-list tests that left `piperine-solver`, each tagged "moved to …" |
| C3 `the_dependency_graph_is_the_documented_one` | `boundaries/dependency_graph.rs` | yes: an exact per-crate equality against the manifests, so any added or dropped arrow fails |
| C4, C5, C7, C8 (ownership) | `ownership::*` | yes: each fails on an unlisted violation and on a stale row (see below) |
| C1, C2 (structure) | `structure::*` | yes: same proof |
| C6, C7 (naming) | `naming::*` | yes: same proof; the dictionary is looser than findings §1.3, see finding 4 |
| C14, C15 (documentation) | `documentation::*` | yes: same proof |
| C8, C9, C24 (worklists) | `worklists::*` | yes: a planted row with no item fails |
| C25 `no_crate_has_more_targets_than_its_plan` | `suite/target_plan.rs` | yes, for "more than". It does not force the plan down after a merge; the spec does not ask it to |

**Ratchet proof (scrutiny point 1).** Throwaway edits were made, then reverted with `git checkout`; the tree is clean afterwards. The planted violations:

- `pub fn zzqx_helper` in `piperine-numbers/src/lib.rs`;
- `fn zzqx_build` in `piperine-numbers/tests/conversions.rs`;
- `pub mod helpers` and `pub struct Context` in `piperine-numbers`;
- a dangling proof in `integrated-tests/tasks.md`.

Separately, a stale row naming `gone.rs`, `Ghost` or `zzqx_gone` went into each of these allowance files:

- `free_functions`
- `test_helper_functions`
- `impls_away_from_their_type`
- `owners`
- `allow_attributes`
- `crate_docs`
- `empty_module_names`
- `undeclared_words`
- `repeated_type_names`
- `undocumented_items`
- `missing_docs_deny`
- `proof_ledger.tsv`
- `worklists/duplicate_tests.tsv`

Every guard failed on both kinds of edit and printed the offending row.

**Seeding (scrutiny point 2).** Every allowance key is `file::item`, and none is a wildcard. Counts are judged both ways (`Allowance::judge`), so a count above the real number fails as stale. All 222 rows of `proof_ledger.tsv` name a feature, a check and a test that do not exist at the base. None of them is caused by T2 or T5: no row names the language server or a moved target.

**T8 (scrutiny point 8).** `suite-report.md` states that the two timing columns were not taken. It names the findings §3.1 values (1 561 CPU-s build, 2 202 s run at 93c831ffc) only as an option the coordinator may accept. It does not record that the coordinator did accept them. That gap is finding 5.

### Findings

| # | Severity | Finding | Evidence / repro | Fix | Closed |
| --- | --- | --- | --- | --- | --- |
| 1 | Medium | The `**Build**:` lines of 27 feature files still run targets that T5 deleted. `ProofLedger` reads only `Run` lines, lone `Proof:` commands and proof rows, so the ledger stays green. Each of these lines will fail when T155 runs `check_proofs.py <feature>` for every feature. The repoint fixture (`repoint.rs:478`) has no `**Build**` line, so no test covers this. | `grep -lE '^\*\*Build\*\*:.*(piperine-lang --test suite_hygiene\|--test (workspace_shape\|crate_boundaries))' .specs/features/*/*.md` lists 27 files, e.g. `generics/tasks-e.md:14`, `integrated-tests/tasks.md:14`, `nested-reach/tasks.md:13`. The targets `crates/piperine-lang/tests/suite_hygiene.rs` and `crates/piperine-solver/tests/{workspace_shape,crate_boundaries}.rs` no longer exist. | Make `ProofLedger` read the commands of `**Build**` lines and `Repointer` rewrite them, then rerun `repoint repoints/T5.tsv` and add a fixture row. Alternatively, list the 27 lines as allowance rows. | |
| 2 | Low | The Done-when boxes of T1 to T8 are all still `- [ ]`. Only the Handoff says "T1 to T8 done". Skill rule 3 requires tasks.md to be marked complete in each task's commit. | `tasks.md:314-315, 336-339, 360-362, 383-385, 406-408, 429-432, 453-455, 476-479` | Tick the boxes that hold. | |
| 3 | Low | Items of the new guards crate have no `///` doc. The crate-wide rule is "a `///` on every new item", but `#![deny(missing_docs)]` covers `pub` items only, and the documentation guard skips test code. | `tests/ownership.rs:17,21`; `tests/worklists.rs:17,21`; `tests/proof_ledger.rs:19,23,28`; `src/repoint.rs:482,494,499`; `src/feature_file.rs:250`; `src/worklist.rs:61`; `src/documentation_rules.rs:12`; `src/naming_rules.rs:98`; moved `tests/suite/suite_hygiene.rs:201,250` and `suite_hygiene/sources.rs:66` | Add one-line docs. | |
| 4 | Low | The naming guard's word lists are wider than findings §1.3. `data/dictionary.txt` admits `pats`, which §1.3 (b) lists to spell out as `pattern`. `data/glossary.tsv` adds about 21 terms that §1.3 (a) does not list (`tr`, `inf`, `dev`, `doc`, `docs`, `impls`, `vams`, `cfg`, `ord`, `neg`, …). A future `walk_pats` or `inf_bound` would pass. | `grep -x pats crates/piperine-repository-guards/data/dictionary.txt`; `cut -f1 data/glossary.tsv` | Drop `pats` from the dictionary. Either justify each extra glossary term in the file or move it to an allowance row. | |
| 5 | Low | `suite-report.md` `## Before` has no timing columns. It offers the §3.1 numbers as an option but does not record the coordinator's AFK decision to adopt them. | `suite-report.md:37-48` | State the decision and add the two columns with the §3.1 values marked "from findings §3.1, at 93c831ffc". | |
| 6 | Low | `.gitignore` keeps the orphan heading "# VS Code extension build artifacts" with nothing under it. | `.gitignore:27` | Delete the line. | |

### Observations (not findings)

- `compiled.rs`'s `#![allow(dead_code)]` is gone, and the build is clean. Its dead items stay hidden by the includers' pre-existing `#[allow(dead_code)]` (`transpile/tests/resolve.rs:16`, `operators.rs:18`). Those are allowance rows owned by the test-support tasks, so this meets T5's text.
- `cli/tests/docs.rs` dropped the GAPS language-server pin rather than re-pinning "No language server". The pin's subject left the tree, and T10 deletes the target anyway.
- `workspace_shape::hygiene_guard_lives_in_piperine_lang` keeps its name (the rule says tests keep their names), but it now asserts the opposite location. A comment says why.
- `Allowance::parse` reads a malformed count as 1 (`allowance.rs:107`). `Findings::seed` is a `pub` method with no caller.
- `suite_hygiene/sources.rs` keeps its own `SourceWalk` beside `SourceTree::rust_files_under`. T5 asked for the free helpers to become methods of `SourceTree` (M7), so this is a small duplicate.

## Phase 1, round 2

**Verdict**: FAIL. Two Medium findings and one Low.
**Head**: d457291c0. The fixes are a41e59dca and d457291c0, on top of 25e3b199f.
**Verifier**: independent sub-agent, the same one as round 1. The author is not the verifier.

### Gates rerun at d457291c0

| Gate | Result |
| --- | --- |
| `**Build**` line (`cargo build --workspace --all-targets`) | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | All pass: lib 35, boundaries 26, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 24, worklists 4 |
| `cargo test --no-fail-fast -p piperine-solver -p piperine-lang -p piperine-cli -p piperine-transpile` | 185 targets, 2 338 passed, 0 failed |

### Round 1 findings

| # | Status | Evidence |
| --- | --- | --- |
| 1 | Closed for `cargo test` commands; part of it reopens as R2-2 | The 27 Build lines now run `piperine-repository-guards --test suite suite_hygiene::` or `--test boundaries`. A grep for the deleted targets finds 0 files. **Throwaway proof**: I prepended `` `cargo test -p piperine-lang --test suite_hygiene` `` to the Build line at `integrated-tests/tasks.md:14`. `every_proof_names_a_test_that_exists` failed with `integrated-tests	Build	no test target piperine-lang/suite_hygiene`. I then reverted the edit. The new allowance row `msl-parser	Build	no test target piperine/suite_hygiene` is real: `msl-parser/tasks.md:14` runs `cargo test -p piperine --test suite_hygiene`, and the root package has had no such target since before the base. |
| 2 | Closed | The 26 Done-when boxes of T1 to T8 are ticked. None is left open in lines 300 to 480. |
| 3 | Closed | Every item of the guards crate now carries `///`. A re-scan of `src/` and `tests/` finds none missing, apart from the `visit_*` trait methods. |
| 4 | Reopened as R2-1 | `pats` and `specs` are gone from the dictionary. But the cut went past §1.3(a) and Appendix H: see R2-1. |
| 5 | Closed | The settled decision is written in `suite-report.md`, `tasks.md` (the Settled line, T8, T152) and `spec.md` AC 1, each quoting the user. `Run s` holds the §3.1 values, and the build column is gone. Spec AC 6 and the checks C23 and C28 were not updated: see R2-3. |
| 6 | Closed | The `.gitignore` heading is removed. |

### Round 1 observations

| Observation | Status |
| --- | --- |
| `Allowance::parse` reads a malformed count as 1 | Closed. It now panics with `<file>:<line>: ... is not key<TAB>count`. The unit test `a_malformed_count_fails_loud_naming_the_file_and_line` expects `allowance.tsv:2`. |
| `Findings::seed` is an unused `pub` method | Closed. It is deleted. |
| `SourceWalk` duplicates `SourceTree` | Closed. It now uses `SourceTree::rust_files_under`. |

**`root_targets_match_the_naming_rule` is not weakened.** It only changes the order of the comparison: both vectors are sorted and then compared with `assert_eq!`. Duplicates are kept, so the comparison is still exact multiset equality. The change was needed because the new walk visits files in a different order.

### Round 2 findings

| # | Severity | Finding | Evidence / repro | Fix | Closed |
| --- | --- | --- | --- | --- | --- |
| R2-1 | Medium | **The naming cut now flags good names.** The coordinator's criterion was §1.3(a), plus Q2, plus the Appendix H math names. This glossary misses part of it. About 138 of the 252 new `undeclared_words` rows are names built only from three kinds of word. Those rows would send B9 and B10 tasks to rename correct names. (1) `add`: an English verb that is missing from `dictionary.txt`. The removed glossary row used to hide this gap. Rows include `add`, `add_crate`, `add_node` and `two_sources_add_their_powers`. (2) `piperine`: the product name. Rows include `the_binary_is_named_piperine` and `piperine_in`. (3) Appendix H method words. §1.3(a) lists Appendix H whole ("`rem_euclid`, …"), yet `is_nan`, `saturating_add`, `checked_mul`, `wrapping_sub` and the trait names `Add`, `Mul`, `Sub`, `IsNan`, `SaturatingAdd` are all rows. Appendix H words still absent from both lists: `add`, `sub`, `mul`, `nan`, `rev`, `log2`, `log10`. Also, §1.3(a) cites `bdf` (`TrBdf2`) as accepted, but removing `tr` made `TrBdf2` and `TrBdf2Phase` rows. | `grep -x add crates/piperine-repository-guards/data/dictionary.txt` finds nothing. `git diff 25e3b199f d457291c0 -- crates/piperine-repository-guards/allowances \| grep -E '::(add\|is_nan\|saturating_add\|piperine_in\|TrBdf2)\b'` lists the rows. | Add `add` to the dictionary. Add every word of the Appendix H names to the glossary, plus `piperine` and `tr` (for `TrBdf2`). Then delete the rows that become stale; the guard lists them. | |
| R2-2 | Medium | **Build lines are checked only for `cargo test`.** `ProofLedger::command_problem` and the repointer go through `CargoTestCommand`, so two gaps remain. First, a `cargo build -p <missing>` in a Build line passes the ledger. **Throwaway proof**: I prepended `` `cargo build -p piperine-nope --all-targets` `` to `integrated-tests/tasks.md:14`, all 4 `proof_ledger` tests passed, and I reverted the edit. Second, `rename-crate` leaves `cargo build -p <old>` unchanged. The unit test `a_crate_rename_changes_every_package_flag_and_prefix` (`repoint.rs:531`) asserts this non-rename: `.replace("cargo build -p gamma", "cargo build -p alpha")`. 36 feature files have Build lines with `cargo build … -p piperine-lang` or `-p piperine-interp`. After T138 and T139 those lines will point at missing crates, no guard will notice, and T155's `check_proofs.py` will fail on them. | `grep -lE '^\*\*Build\*\*:.*cargo build[^`]*-p piperine-(lang\|interp)\b' .specs/features/*/*.md \| wc -l` gives 36. | Parse every `-p` of any `cargo` command in a Build line, and report a missing package. Make `rename-crate` rewrite every `-p <old>`. Fix the unit test so it expects the rename. | |
| R2-3 | Low | **The user's run-time decision is not carried through the whole spec.** Spec CC-12 AC 6 still says "total build+run time" (`spec.md:157`). C23 (`tasks.md:3915`) and C28 (`tasks.md:3936`) still say "build and run time", so T153 or T155 would be asked for a column the user dropped. | the lines named | Reword AC 6, C23 and C28 to "run time", quoting the user as AC 1 does. | |
