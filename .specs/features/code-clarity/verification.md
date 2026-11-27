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

## Phase 1, round 3

**Verdict**: FAIL on two Low findings. Neither one is a regression, and every gate is green.
**Head**: 0c52e420d (fixes 1ef85fe86 for R2-1, 15728fccf for R2-2 and 0c52e420d for R2-3)
**Verifier**: the same independent sub-agent as rounds 1 and 2; it authored none of the fixes.

### Gates at 0c52e420d

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | all pass: lib 37, boundaries 26, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 24, worklists 4 |
| `cargo test --no-fail-fast -p piperine-lang` | 83 targets, 1 577 passed, 0 failed |

### R2-1: the glossary additions

**Rows left flagged.** The coordinator decided that the 67 `undeclared_words` rows still flagged stay flagged and are not a defect. They are `doc`/`Doc*`, `impls`, `cfg`, `specs`, `pats`, `inf`, `vams`, `dev`, `backticked` and `subcommand`.

**Rows the additions cleared.** The additions cleared 193 rows. I took every deleted row and listed the names each new word now clears:

| Word | Basis | Names it clears | Honest |
| --- | --- | --- | --- |
| `add` (dictionary) | English verb | `add`, `add_crate`, `add_node`, `saturating_add`, … (54) | yes |
| `ok` (dictionary) | English | `ok`, `repr_ok`, `…_is_ok` (7) | yes |
| `piperine` | product name | `piperine_in`, `the_binary_is_named_piperine`, … (21) | yes |
| `tr` | §1.3(a) `bdf` (`TrBdf2`) | `TrBdf2`, `TrBdf2Phase` | yes |
| `sub`, `mul`, `nan` | Appendix H (`checked_sub`, `checked_mul`, `is_nan`) and Rust's `Sub`/`Mul` | `Sub`, `SaturatingMul`, `is_nan`, `quiet_nan`, `NanDelay`, `sub_scope`, … | yes. `sub_scope` uses the English prefix. |
| `div`, `neg` | Rust operator traits, the same precedent as §1.3(a)'s `shl`, `shr` and `bitand` | `ast::Div`, `ast::Neg`, `model::Div`, `wrapping_neg`, `neg_flips_value_and_tangent` | yes today. It would also clear a future `neg_rail` (negative) or `div_index` (divider); note for B9. |
| `ord`, `usize`, `isize` | Appendix H.8, Rust types | `Ord`, `PartialOrd`, `partial_ord` | yes |
| `mod`, `extern`, `dyn`, `async` | Rust and MSL keywords, the same precedent as `fn`, `impl` and `const` | lexer tokens `Mod`, `Extern`, `Dyn`, `Async`; `ModItem`, `load_mod`, `extern_module`, `ExternFunction`. The `mod` words mean the module keyword. None is the modulo operator: `ast.rs:487` `Mod(ModItem)`. | yes |
| `rev` | "reversed order, Appendix H's `rev`" | `Rev` and `IteratorRev` (Appendix H, which is fine), but also two other meanings | **no**, see R3-1 |

### R2-2: `CargoCommandLine`

- **What it reads.** It reads every `-p`, `--package X` and `--package=X` of every `cargo` command on a line, and stops at `--`, `|`, `;`, `&&` and `||`. Its unit tests cover `build`, `run`, `test` and `--package=`.
- **The ledger fails on a missing package.** I made throwaway edits to `integrated-tests/tasks.md:14` and reverted each one:
  - `` `cargo build -p piperine-nope --all-targets` `` failed `every_proof_names_a_test_that_exists` with `integrated-tests	Build	no package piperine-nope`.
  - `` `cargo build --package=piperine-nope2 …` `` failed with `no package piperine-nope2`.
- **`rename-crate` on the real tree.** I ran `cargo run -p piperine-repository-guards --bin repoint -- <scratch>/T139.tsv` with `rename-crate piperine-lang piperine-language`, then ran `git checkout -- .specs examples`. The tree is clean afterwards.
  - It changed 36 files: 35 feature files and `examples/buck/README.md`, 260 lines in all.
  - 259 of the changed lines differ from the old text only by `-p` or `--package` `piperine-lang` becoming `piperine-language`, with spacing kept.
  - The one other change is the README's prose path `crates/piperine-lang/tests/…` becoming `crates/piperine-language/…`. That is the prose rewrite working as intended.
  - Old flags remain only in historical records (`proofs.md`, `verification.md`, `t9-decision.md`), which are not Build or Run lines.
  - 33 of the 34 files whose Build line runs `cargo build -p piperine-lang` were rewritten. The one missed is `retire-vams-frontend/spec.md`: see R3-2.

### R2-3: run time only

CC-12 AC 6 (`spec.md:157`) and C28 (`tasks.md:3936`) ask for run time only, quoting the user. C23 (`tasks.md:3915`) says "run time", and so do T152's Done-when (`tasks.md:3768`) and the Settled line. A grep of spec, design and tasks for `build time`, `build+run`, `build and run`, `CPU`, `timings` and `1 561` finds no build-time ask left. **Closed.**

### Round 3 findings

| # | Severity | Finding | Evidence / repro | Fix | Closed |
| --- | --- | --- | --- | --- | --- |
| R3-1 | Low | **`rev` in the glossary hides two other meanings.** The row's own meaning is "reversed order, Appendix H's `rev`". It also clears `diode_rev` (reverse bias) in `crates/piperine-parity/tests/parity_dc.rs:59` and `tests/scenarios.rs:490`. It also clears `GitRef::Rev` (revision) in `crates/piperine-project/src/manifest.rs:91`, `git.rs::branch_tag_and_rev_select_their_commit` and `manifest.rs::a_rev_is_a_commit_id_of_seven_to_forty_hex_digits`. "Revision" is Cargo's own `rev` key, so the last three could be argued as domain spelling. `diode_rev` is a plain abbreviation of "reverse". | `git diff 2f8fa92b8 0c52e420d -- crates/piperine-repository-guards/allowances \| grep -E '::(diode_rev\|Rev)\b'` | Keep `rev`, but restore the `diode_rev` rows as allowance rows. If the revision meaning stays, add it to the row's description. | |
| R3-2 | Low | **The ledger and the repointer read only `{tasks,checks}*.md`.** `retire-vams-frontend` skipped its Tasks phase, so it keeps `## Checks` in `spec.md`. Its Build line (`spec.md:62`, `cargo build -p piperine-lang`), its `Run msl` (`:63`) and its lone proofs (`:66` to `:78`) are therefore never checked or rewritten. The repoint run above left that file unchanged. After T139, these lines will dangle with no guard noticing, and T155's `check_proofs.py retire-vams-frontend` will fail. That contradicts C27 ("every proof of every feature"). | `grep -lE '^\*\*(Build\|Run [a-z]+)\*\*:' .specs/features/*/*.md \| grep -vE '/(tasks\|checks)[^/]*\.md$'` | Have `FeatureFile::read_all` also read the `## Checks` section of a `spec.md`. That is where the skill puts checks when the Tasks phase is skipped. | |

## Phase 1, round 4

**Verdict**: FAIL (one Medium and one Low finding). R3-1 and R3-2 are closed, and every gate is green. While checking R3-2, I found that the ledger never checks a lone `Proof:` command. That gap was present in all three earlier rounds.
**Head**: bd129208c (fixes 31585186a R3-1 and bd129208c R3-2)
**Verifier**: the same independent sub-agent as before; it authored none of the fixes.

### Gates at bd129208c

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | all pass: lib 39, boundaries 26, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 24, worklists 4 |

### R3-1: the third column of the glossary (closed)

- **`diode_rev` is a finding again.** `allowances/piperine-parity/undeclared_words.tsv` has `parity_dc.rs::diode_rev 1`, and `allowances/piperine/undeclared_words.tsv` has `tests/scenarios.rs::diode_rev 1`.
- **`GitRef::Rev` stays accepted.** No allowance row names `Rev`, `rev_select` or `a_rev_is`. The description of the `rev` row now names the git-revision meaning.
- **The column cannot hide an abbreviation.** `NamingRules::is_made_of_words` returns `false` when `means_otherwise_in` matches, so the column only takes acceptance away. It cannot accept a name that the dictionary and glossary would reject. A row listed there by mistake shows up as an unlisted violation, and the ratchet catches it.
- **The unit test passes.** `a_name_in_which_a_term_means_something_else_fails` passes.
- **`meanings` is a fair addition to the dictionary.** It is an English word, the plural of the dictionary's own `meaning`. The new field `other_meanings` needs it.
- **Parsing does not fail loud.** This is finding R4-2.

### R3-2: the `## Checks` section of a `spec.md` (closed)

`retire-vams-frontend` is the only feature folder with no `tasks*.md` or `checks*.md` and a `## Checks` in its `spec.md`.

**Repointer on a scratch run.** I ran `repoint` with `rename-crate piperine-lang piperine-language`, then `git checkout -- .specs examples`.

- In `spec.md` it changed only lines 62, 63 and 72: the Build line, `Run msl` and C3's proof.
- Lines 7 to 47 mention `piperine-lang` and `cargo build -p piperine-lang` (AC 3 at :45), and all of them sit outside the section. None changed.
- Across the tree, the run changed 37 files and 263 lines.

**Ledger.** `a_spec_is_read_only_inside_its_checks_section` proves the bounds: `Run early` before the section and `Run late` after it are not read. On the real tree, the earlier repoint run shows that the section's Build and Run lines are now read and rewritten.

**Point 3: the `cd crates/piperine-lang` proofs.** The proofs of C1, C2 and C5 (`spec.md:66,69,78`) `cd` into a path that `rename-crate` does not rewrite and the ledger does not check.

- T139's Done-when covers them: "`grep -rw piperine-lang` … find nothing outside `proofs*.md` and `retired.tsv`". T138 has the same line for `piperine-interp`.
- So T139 cannot close while these paths remain. That is not a guard, but the task text closes the gap for this rename. I do not count it as a finding.
- The general blindness to lone proofs is R4-1.

### Round 4 findings

| # | Severity | Finding | Evidence / repro | Fix | Closed |
| --- | --- | --- | --- | --- | --- |
| R4-1 | Medium | **The ledger never checks a lone `Proof:` command.** `ProofLedger::dangling_proofs` checks `Run` lines, `Build` commands and run-proof rows. It never calls `FeatureFile::lone_proofs()`, whose only caller is a unit test. T4 says the ledger reads every "lone `Proof:` command", and C27 says "every proof of every feature … names a test that exists". The guard holds neither for the 35 lone proofs that run `cargo test`. One kind it misses is the `o=$(cargo test …)` form at `solver-abi-integration/tasks.md:751,754,769` and `retire-vams-frontend/spec.md:75`. When B2 and B11 merge the targets these proofs name, nothing will fail. Neither will `move`, which handles only commands that start with `cargo test`. | Throwaway edits, each reverted: `solver-restructure/tasks.md:709` changed to `` Proof: `cargo test -p piperine-solver --test zzqx_gone` `` gives `proof_ledger` 4 passed. `solver-abi-integration/tasks.md:754` changed to `--test zzqx_gone` gives 4 passed, with or without a space after `$(`. | In `dangling_proofs`, run `command_problem` over each lone proof's command, recording the subject as the check id. List the lines that dangle at the base as allowance rows. Have `move` rewrite a `cargo test` that is not at the start of a command, and give it a unit test. | |
| R4-2 | Low | **`Glossary::parse` does not fail loud.** A row without a tab turns its whole text into one term. A fourth column is ignored. A third-column name that no declared name matches is kept without a word. For example, once `diode_rev` is renamed, its entry lingers. `Glossary::read` also treats a missing file as empty, although the flood of findings would make that loud anyway. `Allowance::parse` panics on a malformed row. The glossary, which decides acceptance, does not. | `naming_rules.rs:27-32` | Panic, with the file and line, on a row that is not 2 or 3 tab-separated cells. Have `every_declared_name_is_words_or_glossary` fail on a third-column name that no scanned item declares. | |

## Phase 1, round 5

**Verdict**: FAIL on one Low finding. R4-1 and R4-2 are closed, and every gate is green.
**Head**: d2d6ba465 (fixes 27a2f652d R4-1 and d2d6ba465 R4-2)
**Verifier**: the same independent sub-agent as before; it authored none of the fixes.

### Gates at d2d6ba465

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | all pass: lib 45, boundaries 26, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 24, worklists 4 |
| `cargo test --no-fail-fast -p piperine-lang` | 83 targets, 1 577 passed, 0 failed |

### R4-1: lone proofs (closed)

Each throwaway edit below was reverted, and the tree is clean.

| Edit | Result |
| --- | --- |
| `solver-restructure/tasks.md:709` to `` `cargo test -p piperine-solver --test zzqx_gone` `` | fails: `solver-restructure	C2	no test target piperine-solver/zzqx_gone` |
| `solver-abi-integration/tasks.md:754` `o=$(cargo test … --test zzqx_gone` | fails: `solver-abi-integration	C22	no test target piperine-solver/zzqx_gone` |
| the same line with `o=$( cargo test` (a space after `$(`) | fails, with the same row |
| `root-e2e/tasks.md:1166` `--test 'zzqx_*'` | fails: `root-e2e	C16	no test target piperine-parity/zzqx_*` |
| `root-e2e/tasks.md:1166` `--test 'parity_zz?'` | fails |

**The glob cannot keep a dangling proof alive.** `names_target` is an anchored match: `*` matches any run of characters, `?` matches one character, and the whole name must match. A pattern that matches no target fails exactly like a missing name. This follows cargo, which accepts Unix glob patterns for `--test` and refuses a pattern that matches nothing.

**The two new allowance rows are honest.**
- `code-clarity C12	no test target piperine-parity/parity` is the target that T88 creates ("the twelve `parity_*` into `parity`"; its Done-when runs `cargo test -p piperine-parity --test parity`).
- `code-clarity C13	no test target piperine/book` is the target that T89 creates ("The Book is one target").
- Neither target exists today (`crates/piperine-parity/tests/parity.rs` and `tests/book.rs` are absent). These are forward proofs of this feature, the same kind as the 37 `code-clarity` rows already there.

**Redirections.** `2>&1` now ends a command's arguments, so `move` no longer reads it as a test filter. The unit test `a_redirection_is_no_test_filter` covers it.

### R4-2: glossary parsing (closed)

- A throwaway row `zzq the bad row`, with no tab, makes `every_declared_name_is_words_or_glossary` panic with ``data/glossary.tsv:112: `zzq the bad row` is not `term<TAB>meaning[<TAB>names]` ``.
- A throwaway third-column name `zzqx_renamed`, which no item declares, makes the same test fail and print `zzqx_renamed`.
- I reverted both edits.
- The unit tests cover a row with no tab, a row of four cells, and an undeclared third-column name.
- `shell`, `redirection`, `tab` and `third` are plain English words.

### The two points left to the verifier

1. **Test names inside a lone proof's `for t in …` loop.** The coordinator's reading does not hold for the proofs in the tree. Those loops use no cargo filter. They grep the printed result line:
   - `solver-abi-integration/tasks.md:751,754,769` run `printf '%s' "$o" | grep -q "test $t ... ok"`;
   - `:769` also greps `grep -c "test its_devices_pass_the_partials_verifier ... ok"` and `grep -l 'fn its_devices_pass_the_partials_verifier' crates/piperine-solver/tests/*.rs`.

   Once B2 moves a target into a `mod`, cargo prints `test <module>::<name> ... ok`. The grep `"test <name> ... ok"` no longer matches, because the substring after `test ` is now `<module>::`. `move` rewrites the `--test` in the substitution, but not the loop's grep text or the `tests/*.rs` path. So a plain merge, not only a retire, breaks these proofs. The ledger does not read the loop names, so nothing fails before T155, whose `check_proofs.py` run will fail on them loudly. This is warranted as **R5-1, Low**. It is three proofs in one feature, and T155 catches it, but the repointer exists to keep proofs true through a move.
2. **`move` when the `--test` value carries the closing `)`.** For `x=$( cargo test -p a --test old)`, the `module::` filter would land after `merged)`, outside the substitution. No proof in the tree has this form today: the four substitution proofs end their argument list with `2>&1)`. **A note, not a finding.** Add a unit test if T33 meets that form.

### Round 5 findings

| # | Severity | Finding | Evidence / repro | Fix | Closed |
| --- | --- | --- | --- | --- | --- |
| R5-1 | Low | **The result-line greps of lone proofs are neither repointed nor checked.** `move` rewrites the `cargo test` inside a proof but not the proof's own `grep "test $t ... ok"` lines or its `tests/*.rs` path. After a merge they look for `test <name> ... ok` while cargo prints `test <module>::<name> ... ok`, and the ledger does not read them. | `solver-abi-integration/tasks.md:751,754,769`. Repro: move `piperine-solver/protocol_crossings` into a merged target; `:754` then prints `no pass line for a_cap_watch_bounds_the_next_step`. | Two options. (a) Rewrite those three proofs as run proofs (`Proof: run <name> \| test <t> ... ok`), which `check_proofs.py` matches word by word, so a module prefix is tolerated and the ledger checks each name. (b) Have `move` also rewrite `"test $t ... ok"` to `"test <module>::$t ... ok"` and `tests/*.rs` paths. (a) is the simpler. | |

## Phase 1, round 6

**Verdict**: PASS
**Head**: 09983f4c3 (fixes 2b84ff729 R5-1 and 09983f4c3 the `move` placement)
**Verifier**: the same independent sub-agent as before; it authored none of the fixes.

### Gates at 09983f4c3

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | all pass: lib 45, boundaries 26, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 24, worklists 4 |

### R5-1: solver-abi-integration proofs (closed)

- **C21.** Before, one lone proof ran `cargo test` over `abi_dc`, `abi_tran`, `abi_ac`, `abi_noise` and `limiting_report`, then grepped for 7 exact pass lines. Now `**Run drivers**` runs the same five targets, and there are 7 run-proof rows with the same 7 names. The claim is the same. The ledger now checks every name, and `check_proofs.py` matches word by word, so a `module::` prefix is tolerated.
- **C22.** It is the same change for `protocol_crossings`, through `**Run crossings**` and two rows.
- **C27.** Before, the three named tests were grepped from `cargo test -p piperine-solver`. A count required "pass lines of `its_devices_pass_the_partials_verifier`" to equal "files under `tests/*.rs` declaring it", with that count above 20. Now:
  - the three tests, plus `its_devices_pass_the_partials_verifier`, are run rows under `**Run solver**`;
  - the count proof matches `^test ([A-Za-z0-9_]+::)*its_devices_pass_the_partials_verifier \.\.\. ok$`;
  - it compares that with `grep -rhow --include='*.rs' 'fn its_devices_pass_the_partials_verifier' crates/piperine-solver/tests`, which counts declarations, recursively;
  - it keeps `m > 20`.

  Counting declarations instead of files is at least as strict, because a second declaration in one file must also pass. The old and new `m` are both 27.
- **Survives a move.** I prefixed every `its_devices…` line of a saved `cargo test -p piperine-solver` output with `merged::inner::`, and the count stayed 27. The recursive grep reads `tests/<folder>/*.rs`, so a folder move does not change it either.
- **Run once at the head, and both pass.**
  - `cargo test -p piperine-solver --test abi_dc --test abi_tran --test abi_ac --test abi_noise --test limiting_report` exits 0, and all 7 names print `ok`.
  - `cargo test -p piperine-solver --test protocol_crossings` exits 0, and both names print `ok`.
  - The C27 lone proof, run as written, exits 0, with n = m = 27.
- **The ledger fails on a dangling row.** I made two throwaway edits and reverted both, and the tree is clean.
  - `Proof: run crossings | test zzqx_cap_gone ... ok` fails with `solver-abi-integration	C22	run crossings names no test zzqx_cap_gone`.
  - `**Run drivers**` with `--test zzqx_abi` fails with `solver-abi-integration	Run drivers	no test target piperine-solver/zzqx_abi`.

### `move` inside a substitution (closed)

The unit test `a_cargo_test_inside_a_substitution_is_read_and_moved` asserts both cases:

- `x=$( cargo test -p a --test old)` becomes `x=$( cargo test -p b --test merged old::)`;
- `x=$(cargo test -p a --test old inner);` becomes `x=$(cargo test -p a --test merged old::inner);`.

In both, the filter stays inside the `)`, and the trailing `;` is kept.

### Remaining note: test names in lone proofs

I scanned every lone `Proof:` of every `tasks*.md`, `checks*.md` and `spec.md` `## Checks` section. I looked for `for t in`, `test $t`, a grep of `"test <name>`, or `... ok`. Two lines remain, and neither carries the R5-1 risk:

- `solver-abi-integration/tasks.md:723` (C11) greps `differential.txt`. That is a frozen record of the run T10 took; its target was deleted in T22. It is not live cargo output, so no move can change it.
- `msl-pom/tasks.md:1315` greps `test result: ok. [1-9][0-9]+ passed` of a doc-test run. That is a count, and no name is involved.

`retire-vams-frontend/spec.md:75` sums `passed` counts and requires 49. That is also count-only, and a merge keeps the count.

### Round 6 findings

None. I probed the following, and every check held:

- the equivalence of the 3 rewritten proofs;
- the 3 proofs, run at the head;
- a simulated module and folder move against the C27 count;
- 2 dangling-row edits;
- the `)` placement;
- a tree-wide scan for name-grepping lone proofs.

Phase 1 (T1 to T8) is verified. Every round 1 to round 5 finding is closed by the commits named in the rounds above.

## Phase 3 (B4 Refusals have an owner, T15 to T22)

**Verdict**: FAIL (one Medium finding, two Low)
**Head**: 40a85da92 on `feature/code-clarity-p3`, base 4742ea9c5. It does not include Phase 2.
**Verifier**: independent sub-agent (Opus; author != verifier). The implementer changed after T16. I reran the gates myself in the foreground, with `CARGO_TARGET_DIR=/var/tmp/piperine-target-a`.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, then filtered for `^(warning|error)` | 0 warnings, 0 errors (the full log was read) |
| `cargo test -p piperine-lang --no-fail-fast` | 84 result lines, 1 584 passed, 0 failed; `refusals` has 7 tests, all pass |
| `cargo test -p piperine-repository-guards` | lib 46, boundaries 26, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 24, worklists 4: all pass |
| `cargo test -p piperine-parity --no-fail-fast` | 64 result lines, 645 passed, 0 failed (the `rt.rs:335` panics in the log are the compiled-fault tests, which pass) |

### Checks

| Check | Test or probe | Holds |
| --- | --- | --- |
| T15 `p0001_is_worded_in_one_place` | `refusals.rs`; `git grep` | yes. The literal is only in `refusal.rs:112`, and the other `P0001` mentions in `src/` are comments |
| T16 label, no "M1 subset" in non-test files | `p0001_does_not_name_a_milestone`, `git grep "M1 subset"` | yes for `piperine-lang/src`. `api/src/session.rs:1044` is lane K's, as noted. No test pinned the old label: at the base the only Rust occurrences were the two `format!` labels and comments, so nothing needed re-pinning. GAPS §4.8's row is gone |
| T15–T20: no message changes (CC-09) | A static diff. I took every string literal and every `format!` template passed to a base refusal site (420 + 66), normalised `{..}` to `{}`, and looked each up among the 526 head row texts | yes. All of them match except `{what} in {position}` (now `Refusal::in_position`) and `{what} is not supported yet` (now `Refusal::diagnostic`), both rebuilt byte for byte. No test, golden, twin, Book listing or `contract.rs` changed: `git diff --name-only` outside `piperine-lang/src`, `.specs` and allowances lists only `refusals.rs`, `unswept_count.txt`, GAPS, ROADMAP, the parity rename and the guards' data. 1 584 unedited lang tests pass |
| T20 `RefusedForm` has no `Text` | `refusal.rs` | yes |
| T17 "no `Refusal` of `types/` from `Text`" | `tasks.md:689` | it holds at the head (`Text` is deleted), but the box is unticked (finding P3-3) |
| T21: no two sites build one form | `no_two_sites_build_the_same_refused_form`. Throwaway: `// ValueForm::Assignment ValueForm::Assignment` in `elab/mod.rs` | fails "ValueForm::Assignment x2"; reverted |
| T21: `types/unsupported.rs`, `elab/unlowered.rs` gone | `the_catch_all_refusal_files_are_gone` | yes |
| T21 R4, P0004 unreachable | read of `types/call.rs:441-446` against `elab/compound.rs:556-566`, and of `types/patterns.rs:278-314` against `elab/patterns.rs:314-333` | yes. The checker refuses an `expect` whose argument is not `ExprKind::Lit` of `LitKind::Str`, and the elaborator's guard fires only when the literal gives no string. The checker accepts a path pattern only for a variant or a `const`, and `path_value` handles exactly those two. `Elaborator::run` stops when `Types::check` fails (`elaborator.rs:54`). The dropped `scope`/`span` parameters fed only the removed refusals |
| T22 `every_refused_form_has_a_row` | `refusals.rs:137` | yes. Every variant is in `ALL`, in declaration order, with a non-empty text |
| T22 `the_unswept_count_only_shrinks` | throwaway: one `Gap("L83")` turned `Unswept`; one `Unswept` turned `Gap` | the first fails with "492 … the count never grows" and the second with "write 490"; both reverted |
| T22 `every_gap_owner_names_a_gaps_row` | throwaway `Gap("L999")` | fails; reverted. But the test checks only that the row exists, not that it owns the form (finding P3-1) |
| T22 ROADMAP | `ROADMAP.md:53` | yes. The `spec-closure` row hands over the sweep of `refusal/forms.rs` and ends at `unswept_count.txt` 0 |
| Coordinator decisions in commit bodies | T15 (d6566fd93): `Box<Refusal>`, no file or span, Run refusals and C9 until T139, target plan 82. 4373846e4: `ActionRefusal`. T17 (9ef70b1af): the diagnostic-code token with its unit tests, several `{}`, the `ROWS` table. T19 (600bd8e2e): the IR's `NotDifferentiable` as one `{}` form. T21 (9e3d2ac54): R4 `P0004`, the 48 repeated forms | yes. T15's body still says `p` joined the glossary, but `glossary.tsv` has no `p` at the head; T17 replaced it with the token |
| Naming-guard token | `naming_rules.rs` `a_diagnostic_code_is_one_accepted_part_and_a_bare_letter_or_short_number_is_not` | yes: `p0001` and `e0341` are accepted; `p`, `x12` and `p00012` are refused |
| Generated Rust | `refusal/{forms,repeated,syntax}.rs` | no `macro_rules`, no free function, no `#[allow]`. Every item has a `///`. `syntax.rs`'s `match`es are exhaustive with no wildcard. The generators (`generate.py`, `share.py`, `syntax.py`, `registry.json`) are tracked only under `.specs/features/code-clarity/refusal-gen/` |
| Allowances | `git diff` of `allowances/` | yes: 591 deletions and 0 additions. `undocumented_items`' `unsupported_decl` row came and went. Dictionary additions are real English: anything, catch, details, greater, here, image, inlining, less, rather, shrinks, these, tuple, unary, undefined, wildcard. `refutable`, `worded`, `milestone`, `xor` and `unswept` are glossary rows with a meaning |
| T20 test edit | `p0001_does_not_name_a_milestone`, efbb748c8 | meaning unchanged. `Refusal::of(Text("a form"))` became `Refusal::of(ValueForm::Assignment)`, with the expected message moving from `"a form is not supported yet"` to `"assignment is not supported yet"`. It still asserts the full message, the label, and the absence of "M1 subset". The test is the task's own (T16), not one that existed before the feature |

### P3-1 (Medium): the blanket L74 owner is wrong for the forms event functions raise, and for one general form

The T22 commit gives `Gap(id)` to a form whose text a GAPS row quotes, "or, for elab/symbolic.rs, L74". Two kinds of row get the wrong owner from that.

**Forms raised for both ddx and event functions.** `elab/symbolic.rs` serves two bodies: `ExpandedBody::Differentiated` ("a `fn` called inside a `ddx`", L74) and `ExpandedBody::EventFunction` ("a `fn` that returns an `Event`", L75). `event_functions.rs:176,196,204,233,246` runs the same steps with `EventFunction`. The forms whose `{}` is `body.describes()` or `context.describes()` are therefore raised for both bodies. Yet every one of them is `Gap("L74")`:

- `LetOfPatternThatIsNoName`
- `ForThatIsNotOverRangeWith`
- `ForWhoseBoundsDoNotFoldTo`
- `ForWhoseVariableIsNoNameIn`
- `ForOfMoreThanIterationsIn`
- `BreakContinueOrNestedWhileInLoop`
- `ElseThatIsNoBlockOrIf`
- `AssignmentToSomethingThatIsNoName`
- `CompoundAssignmentThroughOperatorOfTypeIn`

L75 names exactly the event-function side ("a `let` of a pattern that is no name, … a `while` or `loop`, a `break`…").

Repro: `crates/piperine-lang/tests/elab_digital.rs:937` asserts the message "a `let` of a pattern that is no name in a `fn` that returns an `Event`". That is raised as `AnalogForm::LetOfPatternThatIsNoName`, whose row (`forms.rs:1416`) says `Gap("L74")`. When `spec-closure` closes L74, this L75 case goes with it.

**A general form owned by a ddx gap.** `ValueForm::AssignmentToFieldOrElement` (`forms.rs:744`, `Gap("L74")`) is raised by `elab/control.rs:79`, `assigned_local`. That is the general assignment lowering behind `lower.rs:696` and `aggregate.rs:234`, not only a ddx body. No site cites L74 for it; L74's prose merely contains the phrase.

**The guard cannot catch either.** `every_gap_owner_names_a_gaps_row` (`refusals.rs:145`) checks only that `| L74 |` exists.

Fix:

- Give the event-function context its own forms, or own the shared forms by context, and own the event-function ones by L75.
- Return `AssignmentToFieldOrElement` to `Unswept`, unless it is shown to be reached only inside a ddx body.
- Consider having the guard require that the owning GAPS row quote the row's text, with `{}` as a wildcard. That holds for the quoted rows; the blanket rows would then need an explicit note.

### P3-2 (Low): three names stay vague or in the wrong family after deviation 7 ("names fixed")

- `ValueForm::Expression` is the "`?` expression" (`forms.rs:792`, from `syntax.rs` `ExprKind::Try`). The `?` was stripped from the name. A fitting name is `TryExpression`.
- `AnalogForm::Refused` (`forms.rs:1441`, text "{} `{}`", raised at `types/analog.rs:215`) is a core primitive or built-in with no signature row. The name says nothing, and the form is not analog.
- The event-function forms above live in `AnalogForm` though `event_functions.rs` raises them for digital bodies, and the item kinds (`EnumItem`, `ModItem`, `ImplItem`, …) live in `ValueForm`.

Fix:

- Rename the first two, for example to `TryExpression` and `PrimitiveWithNoSignatureRow`.
- Move the family classification with the P3-1 split.

### P3-3 (Low): T17's first Done-when box is unticked

`tasks.md:689`, "- [ ] No `Refusal` of `types/` is built from `RefusedForm::Text`". It holds at the head, because `Text` was deleted in T20, and T17's commit body asserts it. The tick was lost at the handoff.

Fix: tick it.

### Notes, not findings

- **`refusal/repeated.rs` is indirection only.** It holds 48 one-line methods of the form `fn cast() -> RefusedForm { ConstantForm::Cast.into() }`. Each site still calls `refuse(file, span, RefusedForm::cast())`, so no owner logic moved into a method. It is what T21's Done-when asks for: the test counts constructions per variant. But since T17–T20 made each variant the one owner of its words, R3 already held without these methods, and the textual count is satisfied by any wrapper. Whether to keep 237 lines of pass-through is the user's call (code-cleanliness rules).
- **Cross-lane allowance deletions.** The dictionary additions emptied about 190 stale `undeclared_words` rows in other crates' allowance files (parity 19, project 14, solver 12, root 123, numbers 4, transpile 4). Phase 2 (lane T) edits the same files, so expect merge conflicts at integration. They are deletions on both sides and resolve by taking both.
- **T15's commit body is out of date.** It says `p` joined the glossary; that was replaced by the token in T17, which is the coordinator's decision. History only.

### Phase 3 findings

| Finding | Severity | Evidence | Fix | Closed |
| --- | --- | --- | --- | --- |
| P3-1 wrong Gap owners: L74 blanket over L75 forms and a general form | Medium | `forms.rs:744,1369-1399,1416`; `event_functions.rs:176-246`; `elab_digital.rs:937`; `control.rs:79`; `refusals.rs:145` | split or own by context (L75); `AssignmentToFieldOrElement` to `Unswept`; make the guard check that the row quotes the text | |
| P3-2 vague and misplaced form names | Low | `forms.rs:792,1441`; `types/analog.rs:215` | rename; reclassify with P3-1 | |
| P3-3 T17 box unticked | Low | `tasks.md:689` | tick it | |
