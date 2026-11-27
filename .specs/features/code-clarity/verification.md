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
