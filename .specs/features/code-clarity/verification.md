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

## Phase 2 (B1 Lean tests I, T9 to T14)

**Verdict**: FAIL (one Medium finding, five Low)
**Head**: 8203b71af on `feature/code-clarity-p2`, base 4742ea9c5
**Verifier**: independent sub-agent (Opus; author != verifier). No `proofs.md` exists yet, because `check_proofs.py` runs after T155. This round reran the phase gates itself.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, then filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | lib 45, boundaries 19, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 30, worklists 4: all pass |
| `cargo test -p piperine-solver` | 370 passed, 0 failed |
| `cargo test -p piperine-cli` | 144 passed, 0 failed |
| `cargo test -p piperine-lang` | 1 433 passed, 0 failed |
| CC-09: `git diff 4742ea9c5..HEAD` over `crates/piperine-parity`, `crates/piperine-transpile`, `tests/`, `docs/`, `examples/`, the IR goldens and `contract.rs` | empty |

### Checks

| Check | Test | Holds |
| --- | --- | --- |
| T9: no test is named for a retired crate, PHDL or the Verilog-AMS front end | a name scan of every `#[test]` for `phdl`, `vams`, `verilog`, `lang_server` | no, on the letter: `workspace_shape.rs:147` `examples_hold_no_vams_or_python` remains (finding P2-3). `removed_surface.rs` is gone, and so are the 7 `workspace_shape` rows and the 3 hygiene tests with their helpers |
| T9, T10, T11: `every_retired_check_is_recorded` and the ledger | `proof_ledger::*` (4 pass) | yes. Each of the 236 `#[test]` names that left `removed_surface.rs`, `workspace_shape.rs`, `suite_hygiene.rs`, `docs.rs` and `spec_text.rs` has a `retire` or `replace` row in `repoints/T9.tsv`, `T10.tsv` or `T11.tsv`; a set difference found none missing |
| Orphan `Run` lines | each `**Run x**:` line deleted from a feature file | yes. 46 deleted lines were all used at the base and have zero uses at the head: every proof that named them was repointed in the same commit. No proof at the head names a missing `Run` line outside the code-clarity prose files |
| T10: moved tests keep their names and assertions | the 9 tests of `suite/repository_records.rs` against their base bodies in `docs.rs` | yes: 8 are byte-equal in their assertions, and 1 differs only by `read` becoming `RepositoryRecords::read`. Four of the nine are sentence pins the task did not ask to keep (finding P2-2) |
| T10: the 56 retired `docs.rs` tests are wording pins | read of `the_buck_spec_holds_the_bench_bounds`, `abi_md_describes_the_unreleased_contract`, `the_book_no_longer_says_settle_is_not_compiled`, `the_scenario_package_states_its_measured_step_count`, `gaps_section_11_holds_no_net_row`, `gaps_rows_carry_no_unknown_control_clause`, `the_per_level_feature_is_recorded_as_dropped`, and the name list | yes. Each asserts `contains` or `!contains` on a document. Five of them read a document that is not a tracking file (the Book, `examples/buck/SPEC.md`, `abi.md`, a `Piperine.toml` comment, a feature spec), but the retire reason calls it one (finding P2-6) |
| T11: `spec_structure` keeps the structural guards with their names | the 5 tests against their base bodies in `spec_text.rs` | yes: the assertions are identical. The 5 tests cover the 4 kinds the task names, plus `b36_rows_carry_a_code_and_a_replacement` (a table-shape check). The `//!` header is present |
| T11: the 144 retired tests are wording pins | a scan of every base body for tests with few `contains` calls and a loop | yes, within the task's wording. Two of the retired tests are scans rather than sentences: `appendix_h_renders_every_code_span`, a backtick lint, and `tran_in_fences_is_the_module`, a token check over every `rust` fence. Neither is among the four kinds the task keeps, so retiring them is in scope; recorded as a note |
| T12, T13: every uncited test has a verdict, and no removed test was the only assertion of its rule | `.specs/features/code-clarity/uncited_tests.tsv` (371 rows) and reads of 30 uncited tests across solver, digital-solver, analog-solver, lang, api, parity, interp and ir | **no**. Every row is `kept: it alone proves that <test name with spaces>; no other test asserts it on the same input`, and both commit bodies say no test asserts the same rule on the same input. That is false for at least 5 rows (finding P2-1) |
| T14: `every_duplicate_of_the_inventory_has_a_verdict` with no open row; `the_parity_oracles_are_not_reduced` | `worklists::*`, `proof_ledger::*` | yes. 58 rows, all `kept: <what differs>`, each with a specific reason. 10 pairs were read side by side (below). Keeping the T2 layer pairs agrees with design.md:282 and T90, which removes the `commits` four later. Two reasons are wrong (finding P2-4) |
| Allowances: rows only deleted, or moved net-zero | `git diff` of `allowances/` | yes: 191 deletions and 3 insertions. The 3 insertions are the `spec_text.rs::{b1_counts_60_constructs, b36_rows_carry_a_code_and_a_replacement, appendix_h_is_in_the_file_map_and_nav}` rows re-keyed to `spec_structure.rs`, each with count 1, net zero. The `docs.rs` word rows of the moved README and ROADMAP tests were deleted rather than moved; in their place `readme` and `roadmap` joined `data/dictionary.txt`. That widens the word check tree-wide by two words, which T124's dictionary review should confirm |

**The 10 Appendix E pairs read.** Each is a different input or a different layer, and the verdict holds for all ten:

- `e0901_there_is_no_println` and `e0404_a_commit_belongs_in_a_reaction`: different Book claims and different sources (a module body against an analog block).
- `an_unknown_top_is_one_diagnostic`: `Types::check`, which asserts P0003 and the std-module case, against `Elaborator::run`.
- `m_and_dtemp_are_ordinary_names`: E0161 resolution against E0320 binding.
- `abs_has_the_derivative_of_sign`: two `Dual` implementations.
- `the_adc_commits_the_rounded_code_after_tconv`: hand-built IR against source, and the source side is `test = false`.
- `a_simparam_entry_naming_an_option_is_p0010`: the cli flag against the api manifest.
- `a_limited_diode_reaches_the_unlimited_operating_point`: the circuit builder against the plan.
- `the_crate_depends_on_the_abi_crate_only`: each crate's own manifest.
- `divider`: the Q9 twin.

### P2-1 (Medium): T12 and T13 kept duplicates; the "kept" reasons are generated from the test name

T12's verdict is `kept: <the rule it alone proves>`, given after a test is compared with the others. Each of the 371 rows instead restates its own test name with spaces and appends the same clause. Reading the uncited clusters turned up these tests, which are asserted again by another test on the same input. The survivor holds at least the same assertion:

| Uncited test | Survivor | Why it is a duplicate |
| --- | --- | --- |
| `digital-solver/tests/advance.rs:342` `a_changed_analog_read_net_lands_by_analog_read_changed` | `advance.rs:471` `a_commit_that_changes_a_carrier_the_analog_side_reads_lands_in_both_rules` (cited: transient-performance C7) | The same inverter design, `analog = ["y"]`, `host_commit(a, HIGH, Tick(4))`, `advance(Tick(10))`, and the same `Landing { Tick(4), AnalogReadChanged }`. The survivor loops over both rules, so the Declared rule (`engine_with`'s rule) is included |
| `advance.rs:349` `a_reaction_that_reads_a_continuous_pin_lands_by_continuous_read` | `advance.rs:481` `a_reaction_that_reads_a_continuous_pin_lands_in_both_rules` (cited: C8) | The same sampler design, `fire g @ 4` and `advance(10)`. The uncited test checks the cause only; the survivor checks the tick and the cause under both rules |
| `advance.rs:368` `an_irreversible_reaction_lands_by_irreversible` | `advance.rs:604` `an_irreversible_reaction_lands_in_both_rules` | The same stub design, `fire g @ 4` and `advance(10)`, and the same cause `Irreversible`. The survivor also checks the tick and both rules |
| `digital-solver/tests/queue.rs:192` `a_replace_commit_removes_every_pending_commit_of_its_statement` | `queue.rs:74` `replace_removes_every_pending_commit_of_the_statement` | The same three pushes of statement 0, `(0,0,1,0,5)`, `(0,3,2,0,20)` and the REPLACE `(0,0,9,1,12)`. The survivor adds one commit of another statement and asserts the drained values `[3, 9]`, which implies the uncited test's `len == 1` and `due_at(12) == [9]` |
| `lang/tests/buck_example.rs:80` `the_package_builds_with_no_diagnostics` | `lang/tests/types_buck.rs:94`, inside `the_buck_type_checks_with_no_diagnostic` | The same `examples/buck` graph, loaded the same way (manifest name, then `src/lib.msl`), and the same `design.diagnostics().is_empty()` |

There is also a near-duplicate that should be merged: `advance.rs` `a_finish_at_an_instant_run_ahead_is_final_in_both_rules` repeats `advance_returns_final_at_a_finish` on the same input, both rules included, but without the checkpoint assertion.

Repro: read the line pairs above, or run `cargo test -p piperine-digital-solver --test advance -- lands_by_ lands_in_both_rules`. Both forms pass on the same design.

Fix:

- Remove the five tests with their survivors recorded, as T12 says.
- Re-run the comparison over pairs in the same file that share a fixture call. That is where these five were found; the cross-repository similarity pass missed them.
- Replace the generated reason on the rows that remain with the rule that differs, at least for every uncited test that shares a file with a test of a similar name.

One thing the run-time goal should take into account: these crates run in seconds, so the fix trims lines, not test time. The suite's run time is in the compiled-backend crates and the compile cache (findings §3.2), and B2 and B11 handle those.

### P2-2 (Low): T10 kept four sentence pins outside its keep-list

T10 says to keep `gaps_section_10_lists_exactly_the_owned_codes`, `the_roadmap_is_forward_looking` and the buck README's command lines. It also kept these, which P1 names as the kind to remove:

- `the_instructions_name_the_integrated_gate_and_the_buck_on_demand` (`repository_records.rs:58`, CLAUDE.md `contains`);
- `the_instruction_files_say_the_host_crates_build` (`:84`, a CLAUDE.md table-row sentence);
- `the_buck_readme_points_at_the_scenarios` (`:69`), which has three `!contains` absence pins on stale README sentences.

The two `generics/checks.md` guards are structural (phase completeness, unique ids), and keeping them is right. In addition, `:98` still says delivery history is "held by the `state_records_*` tests above", and those tests are retired.

Fix: retire the three pins with their proofs, or record why each is structural, and correct the comment.

### P2-3 (Low): T9's Done-when, "no test is named for the Verilog-AMS front end", fails on one test

`crates/piperine-repository-guards/tests/boundaries/workspace_shape.rs:147` `examples_hold_no_vams_or_python` guards against `.vams` and `.py` files returning under `examples/`.

Fix: retire it, or rename it to the present rule, for example `examples_hold_only_msl_packages`. Renaming needs repointing.

### P2-4 (Low): two kinds of T14 reason are wrong

- The 8 T2 layer pairs, `an_unchanged_commit_fires_nothing` (`duplicate_tests.tsv:14`) among them, are labelled "parity oracle (Q9)". Q9 is the `parity_*` against `tests/scenarios.rs` question. These pairs are kept because design.md:282 and T90 defer the layer deduplication.
- `the_adc_commits_the_rounded_code_after_tconv` and `the_comparator_trips_at_its_thresholds` (`:41`) say the parity definition is "over the MSL source". In fact `boundary.rs` is the hand-built IR side, and the source side `buck_boundary.rs` is `test = false`.

Fix: cite T90 or §3.3 T2 in the reason, and state which side is which.

### P2-5 (Low): T12's verdicts landed in T11's commit, outside the guarded worklist

`uncited_tests.tsv` arrived in 980f640e0 (T11), and c2a1b0ef2 (T12) changes only `tasks.md`; the commit body admits the staging mistake. T12 and T13 name `worklists/duplicate_tests.tsv` as **Where**, but the verdicts sit in `.specs/features/code-clarity/uncited_tests.tsv`, which no guard reads. "Every uncited test has a verdict" is therefore not mechanically held. The coordinator chose the file. The cost is that a later task can remove or rename a test without the table noticing.

Fix: none required if the coordinator accepts it. Otherwise, make the `worklists` guard read the file.

### P2-6 (Low): the T10 retire reason misnames five documents as tracking files

`repoints/T10.tsv:31` (`the_buck_spec_holds_the_bench_bounds`, `examples/buck/SPEC.md`), `:33` (`the_book_no_longer_says_settle_is_not_compiled`, the Book), `abi_md_describes_the_unreleased_contract` (`abi.md`), `the_scenario_package_states_its_measured_step_count` and `the_per_level_feature_is_recorded_as_dropped` all read "a pin on a sentence of a tracking file". They are wording pins, so retiring them is right, but the documents they read are not tracking files, and these reasons are copied into the closed features' checks.

Fix: say "a pin on a sentence of <the file>".

### Phase 2 findings

| Finding | Severity | Evidence | Fix | Closed |
| --- | --- | --- | --- | --- |
| P2-1 uncited duplicates kept; generated reasons | Medium | `advance.rs:342,349,368`, `queue.rs:192`, `buck_example.rs:80` | remove the five with their survivors; re-scan same-file pairs; give the rule that differs | |
| P2-2 sentence pins kept by T10 | Low | `repository_records.rs:58,69,84,98` | retire them or justify each; fix the comment | |
| P2-3 a test named for the VAMS front end | Low | `workspace_shape.rs:147` | retire or rename | |
| P2-4 Q9 mislabel, ADC pair inverted | Low | `duplicate_tests.tsv:14,41` | cite T90 or §3.3 T2; name the sides | |
| P2-5 T12 file in T11's commit, unguarded | Low | 980f640e0, c2a1b0ef2 | accept, or guard the file | |
| P2-6 "tracking file" reason on non-tracking documents | Low | `repoints/T10.tsv:31,33` | name the file | |

## Phase 2 round 2

**Verdict**: FAIL (one Medium finding, four Low)
**Head**: 39295ddfc (the fix of round 1, on top of 6375cebe5)
**Verifier**: independent sub-agent (Opus; author != verifier). This round reran the gates itself.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` | lib 45, boundaries 19, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 27, worklists 5: all pass |
| `cargo test -p piperine-digital-solver` | 161 passed, 0 failed |
| `cargo test -p piperine-lang` | 1 432 passed, 0 failed (round 1 had 1 433; `the_package_builds_with_no_diagnostics` left) |

### Round 1 findings

| Finding | Status at 39295ddfc |
| --- | --- |
| P2-1 | **Closed.** The 5 duplicates are gone, each with a `retire` row naming its survivor (`repoints/T12.tsv`, `T13.tsv`). The finish pair is merged: `a_finish_at_an_instant_run_ahead_is_final_in_both_rules` now takes `before = d.checkpoint()` per rule and asserts `d.checkpoint() == before` after `Final(Tick(6))`, so it holds both assertions of `advance_returns_final_at_a_finish` under both rules, and a `replace` row moves that test's proofs. The `RemovedUnknownForms` successor row (`suite_hygiene.rs:1069`) now names `replace_removes_every_pending_commit_of_the_statement`. Ten more uncited tests found no further duplicate (detail below). The rewritten reasons are acceptable with two wrong rows (R2-3) |
| P2-2 | Three pins retired with reasons that name the file. `the_buck_readme_says_how_to_run_it` is half wording pin (R2-4) |
| P2-3 | Renamed to `examples_hold_only_package_sources` with a `replace` row, but the repointed proof is broken (R2-1) |
| P2-4 | **Closed.** The 8 layer pairs cite design.md:282 and T90. The ADC and comparator pairs name the hand-built-IR side (`boundary.rs`) and the on-demand source side (`buck_boundary.rs`, `test = false`). `traces_are_recorded_only_for_a_named_stream` states the two inputs |
| P2-5 | Guard added. It fails on an unlisted test, a stale row and a repeated row, but passes any verdict text (R2-2) |
| P2-6 | **Closed.** `T10.tsv:21,22,31,33,39` name the document each test read (abi.md, `examples/buck/SPEC.md`, the Book, the `Piperine.toml` comment, the per-level spec) |

**Ten more uncited tests checked** (none is a duplicate):

- `probe_selection` `…_requested_state_observable` and `…_var_observable`: they request different observables.
- `param_text.rs` `an_integer_above_2_pow_53_is_read_exactly` against `session.rs` `…_is_set_exactly`: the parser alone, against the session plus `describe`.
- `types_structure` `a_parent_may_drive_a_childs_pub_resolved_signal` against `source_nets` `a_parent_drives_…`: a type check against a run, giving 121.
- `integration.rs` `the_jump_estimate_is_zero…` against `the_state_estimate_is_zero…`: `jump_lte` against `state_lte`.
- `failure_rules` `section_12_unresolvable_noise_reference_node…` against `…output_node…`: different fields.
- `ir/operators` delay against noise read: different operators.
- `usage.rs` `help_lists_exactly_the_eight_commands`: this one is the strongest of its group. The cited Book test `help_lists_the_commands_and_version_prints_the_version` asserts only `len() == 8` plus the version, which `version_prints_the_workspace_version` already asserts. So the weaker test is the cited one. It is out of T13's scope (it is cited and needs repointing) and is noted here for B11.

**Throwaway edits for the P2-5 guard.** Each edit was reverted with `git checkout`, and the tree is clean.

- A planted `#[test] fn zzqx_planted_uncited_check` in `piperine-plan/tests/nan_param.rs` fails the guard: "unnamed tests with no row".
- A row for a test that does not exist fails it: "rows whose test is gone".
- A duplicated first row fails it: "rows that stand more than once".
- The first `piperine-abi` row's verdict replaced by `TODO`, or by an empty cell, passes the guard (R2-2).

**Code rules (`src/unnamed_tests.rs`).** There is no free function: one unit type, `UnnamedTests`, with methods. The names are full words. Every item has a `///`, including both `const`s, and so does the module's `//!`.

### R2-1 (Medium): the repointed proof of solver-restructure C11 can no longer match

`.specs/features/solver-restructure/tasks.md:737` now reads:

`Proof: run boundaries | test workspace_shape::workspace_shape::examples_hold_only_package_sources ... ok`

The module segment is doubled. Cargo prints `test workspace_shape::examples_hold_only_package_sources ... ok`; this was checked by running `cargo test -p piperine-repository-guards --test boundaries`. `check_proofs.py` will therefore find no such line, and C11 loses its proof at T155.

`proof_ledger` cannot see the mistake. `proof_ledger.rs:156` (`test.rsplit("::").next()`) and `repoint.rs:37` compare only the last segment of a test path.

Repro: `grep -n examples_hold_only .specs/features/solver-restructure/tasks.md`.

Fix:

1. Correct the line.
2. Find out whether the `repoint` binary or a hand edit doubled the prefix. If it was the binary, fix `repoint` for a rename inside a module, and add a unit test for it.
3. Consider having the ledger compare the full module path a proof names with the module the test lives in.

### R2-2 (Low): the unnamed-tests guard accepts any verdict and leaves out two crates on a rule that does not exist

`UnnamedTests::verdict_rows` (`unnamed_tests.rs:90`) keys on the first three cells and never reads the fourth. A row whose verdict is empty or `TODO` passes. The guard proves only that the rows and the inventory match one to one; it does not prove that every test has a verdict. Fix: refuse a row whose fourth cell does not start with `kept: `.

`JUDGED_CRATES` (`:10-27`) leaves out `piperine-numbers` and `piperine-project`, "the two that CC-12 AC 2 leaves out". CC-12 AC 2 (spec.md:153) leaves out no crate. Both crates have no uncited test today (22 and 53 tests, all cited), so including them costs no row. Excluding them means a future uncited test there escapes the guard. Fix: include both, or invert the list to exclude only the guards crate. Either way, correct the comment.

The header of `uncited_tests.tsv` (line 2) also names the guard `every_uncited_test_has_exactly_one_verdict`; the test is `every_unnamed_test_has_exactly_one_verdict`.

### R2-3 (Low): the generated reasons are mostly specific, but two rows are wrong

The 366 rows have the form "asserts <comment or first assertion>; its nearest test in the file, <name>, asserts <…>". In a sample of 15 rows, 13 state what the test proves in a way that differs visibly from its neighbour. That meets T12's `kept: <the rule it alone proves>`; it is generated rather than judged per row, but the text comes from the test itself. 98 rows fall back to the raw text of an assertion, and 147 are cut off with `…`.

These rows are wrong:

- `section_12_unresolvable_noise_reference_node_is_an_analysis_error` reads "asserts its run to completion without an error". The test (`failure_rules.rs:186-200`) asserts that the analysis fails with a `Noise` error naming "reference node". Its nearest test gets the same text, so the row states no difference.
- `a_std_file_sees_another_namespace_through_the_prelude` gives the identical text for itself and its neighbour `bit_rise_resolves_into_std_values`.
- Weaker, from the sample: `a_commit_reaches_the_owner`'s neighbour text, "asserts 'functions, 3'", says nothing.

Fix: write these rows by hand, and scan for rows whose two halves are equal.

### R2-4 (Low): `the_buck_readme_says_how_to_run_it` is half a wording pin

T10 keeps "the buck README's command lines". The command-line assertions of `repository_records.rs:19` are in scope: `piperine run startup`, `piperine test`, `--print-step 1e-8 --csv`, `--plot`. Its other assertions are the same kind of pin as `the_buck_readme_points_at_the_scenarios`, which this round retired with the reason "three of them absences":

- two sentence pins: "prints the whole start" and "No such host exists yet";
- six `!contains` pins on stale sentences: "from = 599e-6", "last microsecond", "two-row table", and three on "no host API…" / "no testbench".

Fix: keep the command-line assertions and retire the sentence and absence pins, recording the narrowing in `retired.tsv`. Or retire the whole test, as was done for its sibling.

### Round 2 findings

| Finding | Severity | Evidence | Fix | Closed |
| --- | --- | --- | --- | --- |
| R2-1 doubled module in a repointed proof | Medium | `solver-restructure/tasks.md:737`; `proof_ledger.rs:156` | correct the line; fix `repoint` if it is the cause; make the ledger compare the module | |
| R2-2 the guard accepts any verdict; two crates left out | Low | `unnamed_tests.rs:10-27,90`; `uncited_tests.tsv:2` | require `kept: `; include numbers and project; fix the guard name | |
| R2-3 two wrong generated reasons | Low | `uncited_tests.tsv` rows `section_12_unresolvable_noise_reference_node…`, `a_std_file_sees_another_namespace…` | rewrite them by hand; scan for equal halves | |
| R2-4 README sentence and absence pins kept | Low | `repository_records.rs:19-36` | narrow to the command lines, or retire | |

## Phase 2 round 3

**Verdict**: PASS
**Head**: acf28ab8e. Four fix commits sit on 21bad1466: 3e504144c (R2-1), dbda0d62b (R2-2), 1f98685e5 (R2-3), acf28ab8e (R2-4).
**Verifier**: independent sub-agent (Opus; author != verifier; the fixer was another Opus agent). The gates were rerun at the head.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` | lib 49, boundaries 19, documentation 2, naming 2, ownership 5, proof_ledger 4, structure 2, suite 27, worklists 5: all pass |
| `cargo test -p piperine-digital-solver` | 161 passed, 0 failed |
| `cargo test -p piperine-lang` | 1 432 passed, 0 failed |

### Round 2 findings

| Finding | Status | Evidence |
| --- | --- | --- |
| R2-1 | **Closed** (3e504144c) | See "R2-1" below |
| R2-2 | **Closed** (dbda0d62b) | See "R2-2" below |
| R2-3 | **Closed** (1f98685e5) | See "R2-3" below |
| R2-4 | **Closed** (acf28ab8e) | See "R2-4" below |

**R2-1.**

- C11 now reads `test workspace_shape::examples_hold_only_package_sources ... ok`.
- `repoint` replaces the whole written path. The unit test `a_replace_inside_a_module_writes_the_module_once` shows that the module is written once.
- The ledger builds each test's cargo path. `SourceTree::modules_reached_from` gives the module path from the target root. `TestTargets::test_paths` adds each test's inline `mod` scope.
- `TestTarget::path_names` accepts the full path or a `::`-suffix of it, and nothing else.

Throwaway edits, reverted with `git checkout`; the tree is clean:

| Edit | Result |
| --- | --- |
| C11 with `crate_boundaries::` as its module | fails, "names no test crate_boundaries::…" |
| C11 with `workspace_shape::workspace_shape::` (the round-2 bug) | fails |
| C11 with the bare name | passes |
| host-ii C1, a unit test, rewritten from `request::tests::…` to `recorder::tests::…` | fails |

Unit-test roots come from `lib.rs`, `main.rs` and `src/bin/*`. The 215 module-path proofs at the head pass, `clilib`'s `cli::tests::…` and `apilib`'s `request::tests::…` among them. `a_test_path_follows_the_module_declarations_from_the_root` covers a file module, an inline `mod tests` and a `mod inline { mod deep; }`.

There are 209 `proof_ledger.tsv` rows before and after. With the leading module path removed, the old and new sets are identical. Only the 12 rows whose proof writes a path (`rt::…`, `recorder::tests::…`, `verdict::tests::…`, `plot::tests::…`) changed spelling. No dangling proof was added and none was hidden.

**R2-2.**

- `UnnamedTests::is_judged` requires `kept: ` followed by a non-empty rule that is not `-` and not `TODO`. Its unit test covers 8 cases that are not judged.
- A throwaway `TODO` verdict fails `every_unnamed_test_has_exactly_one_verdict` with "rows whose verdict is not `kept: <rule>`". It was reverted.
- `JUDGED_CRATES` now holds `piperine-numbers` and `piperine-project`. Neither has an uncited test today, so the inventory gains no row.
- The file header names the guard correctly.

**R2-3.**

- 366 rows; none has equal halves, and 1 contains a `…`.
- `section_12_unresolvable_noise_reference_node_is_an_analysis_error` and `a_std_file_sees_another_namespace_through_the_prelude` were rewritten by hand.

I sampled 15 rows (seed 7). 12 state the rule and a visible difference from the neighbour; examples are the two `emit!` positions of `msl_errors`, `edge_gnd_name_routes_to_ground` against `ac3_node_is_idempotent`, the `Logic<4>` pattern against the enum names, and `a_type_that_contains_itself_is_refused`. Two are thin but true:

- `test_setup_error_propagates`, "asserts 'result.is_err()'";
- `analytic_and_fallback_paths_coexist…`, "asserts Engine attribution (DVG-07)".

One is wrong. `reset_clears_stamps_without_reallocating` reads "asserts Stamp a matrix entry and an RHS entry", which is the setup comment at `faer.rs:236`, not what the test checks.

A scan for setup-style openings found two more of the same kind: `a_derivative_chains_through_a_discrete_function` ("asserts Run the instant at t = 0 by hand…") and the DVG-07 row above.

The 123 halves that quote an assertion verbatim do meet T12's `kept: <the rule it alone proves>`. The quote is the exact value the test fixes, such as `TrBdf2::jump_lte(..) < 1e-9` against the neighbour's `state_lte`, or the SP-05 messages "a port on ground" against "a port on a node not in the circuit". That is more precise than a paraphrase.

Verdict on R2-3: closed. The three setup-comment rows are a bookkeeping imprecision in a `.specs` record that no proof reads. They do not change which test is kept, or whether a duplicate exists. They are noted, not opened as a finding:

- `reset_clears_stamps_without_reallocating`;
- `a_derivative_chains_through_a_discrete_function`;
- `analytic_and_fallback_paths_coexist_and_the_fallback_is_named`.

The coordinator may rewrite them by hand in any later commit.

**R2-4.**

- `the_buck_readme_says_how_to_run_it` now asserts only the four command lines: `piperine run startup`, `piperine test`, `--print-step 1e-8 --csv` and `--plot`. This is T10's keep-list, "the buck README's command lines".

On the claims:

- **msl-cli C157** reads "names `piperine run startup` and no longer says nothing executes". The first half is still proved exactly. The second half's absence pin is gone, and the annotation says so.
- **long-run C58** reads "describes the whole-start run". `--print-step 1e-8 --csv` with `--plot` is the whole-start run's invocation, so the proof still falsifies the claim's checkable core: drop those flags from the README and the test fails.

The annotation "(narrowed by code-clarity T10: …)" is the honest record. It matches Q13's decision that a prose pin's criterion loses its prose proof with a note naming code-clarity. Retiring C58 would discard a proof that still holds part of the claim. I judge the annotation right. The only gap is that no guard reads "narrowed", as `every_retired_check_is_recorded` reads "retired". That is acceptable for two rows.

**Code rules** (`test_targets.rs`, `source_tree.rs`, `unnamed_tests.rs`, `repoint.rs`):

- no free function;
- full words;
- a `///` on every new field and method (`paths`, `unit_paths`, `path_names`, `holds_path`, `test_paths`, `modules_reached_from`, `is_judged`, `rows_without_a_verdict`);
- no `#[allow]`;
- the new unit tests are in `mod tests` blocks.

### Round 3 findings

None. I probed:

- wrong, doubled and bare module paths in a run proof;
- a wrong module in a unit-test proof;
- a `TODO` verdict;
- the re-spelled allowance rows (the same set modulo the path);
- 15 sampled reasons and a setup-comment scan (three imprecise rows, noted above);
- the narrowed README test against C157 and C58.

Phase 2 (T9 to T14) is verified. The round 1 and round 2 findings are closed by 39295ddfc, 3e504144c, dbda0d62b, 1f98685e5 and acf28ab8e.

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

## Phase 3 round 2

**Verdict**: FAIL (one Low finding)
**Head**: 84e215ff4. It is d2fb2cc4d (the merge of Phase 2) followed by the fix for P3-1, P3-2, P3-3 and the removal of `repeated.rs`.
**Verifier**: independent sub-agent (Opus; author != verifier). I reran every gate with `CARGO_TARGET_DIR=/var/tmp/piperine-target-a` and read the full logs.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards --no-fail-fast` | 11 result lines, 116 passed, 0 failed |
| `cargo test -p piperine-lang --no-fail-fast` | 84 result lines, 1 439 passed, 0 failed; `refusals` 7/7 |
| `cargo test -p piperine-parity --no-fail-fast` | **64 result lines, 645 passed, 0 failed**, the same as round 1 |

**Parity count (point 5).** No test is lost. The full run gives 64 result lines and 645 passing tests, as in round 1. There are 74 `.rs` files under `parity/tests` at both 40a85da92 and the head, and the diff 40a85da92..84e215ff4 does not touch `crates/piperine-parity`. The fixer's "23 targets, 176 passed" is therefore a partial view of the run, not the suite.

**Lang count.** It fell from 1 584 to 1 439, a drop of 145. Phase 2 explains the whole drop: it retired 144 `spec_text` tests and removed one duplicate in `buck_example`.

### Merge (point 1)

- **Allowances.** For each of the 129 files under the guards' `allowances/`, `data/` and `worklists/`, I compared the merge with the rows kept by both sides plus each side's additions. The formula is `(base ∩ phase-3 ∩ phase-2) ∪ (phase-3 − base) ∪ (phase-2 − base)`, and it holds for all 129 files. No row was resurrected and none was dropped.
- **`verification.md`.** Lines 1–673 of the head are byte-identical to the Phase 2 side's file, Phase 2 rounds 1–3 included. The Phase 3 round 1 section is byte-identical to `d679ea223`'s.

### Round 1 findings

| Finding | Status |
| --- | --- |
| P3-1 | **Closed in substance.** The nine shared ddx/event-function forms and `AssignmentToFieldOrElement` are `Unswept`. Of the 35 round-1 owners, all 17 `Gap("L74")` rows left; the other 18 are unchanged. The 18 that remain were spot-checked against GAPS: L73 ×2, L75 ×2, L76, L83, L84 ×2, L85, L86 ×3, L87, L92 ×2, L94, L95, L96. Each row's last column quotes the form's words. The strengthened guard is unsound in both directions, though (finding P3R2-1) |
| P3-2 | **Closed.** `ValueForm::TryExpression`. `ValueForm::PrimitiveWithNoSignatureRow` (a call of a core primitive or built-in with no signature row) is a value or expression form, so `ValueForm` is an honest home and a family of one would add nothing. `ItemForm` holds the 24 declaration kinds. `EventFunctionForm` holds the 11 event-function forms. The nine shared forms stay in `AnalogForm` even though event functions raise them in digital bodies; a family such as an "expanded body" form would say so, but that is a naming nicety and stays a note |
| P3-3 | **Closed.** `tasks.md:689` is ticked |

### `repeated.rs` removal (point 4)

The file and `share.py` are gone, and the sites build the variant directly.

- **No two rows share words.** `each_form_words_live_in_one_row` asserts it.
- **No site builds a refusal from text.** `RefusedForm` has no text variant, `"P0001"` is only in `refusal.rs`, and every detail fills a `{}` of a row.
- **T21's reworded Done-when.** It says "a form's words are defined in exactly one row and each repeated form is one named variant, not one message per site". That still states the finding it closes, R3's 25 messages written at 68 sites. T21's **What** still reads "is raised by one method of its owner", which now contradicts the Done-when. The Done-when is marked "coordinator decision … pending review", so this is noted for the user's review rather than raised as a finding.

### P3R2-1 (Low): the 20-character guard admits a wrong owner and refuses honest ones

`every_gap_owner_names_a_gaps_row` (`refusals.rs:113-125`) requires each fragment of 20 characters or more between `{}` to appear verbatim somewhere in the owning GAPS line, prose included. Five throwaway edits were made, each reverted with `git checkout`:

| Edit | Guard | Should be |
| --- | --- | --- |
| `FormOfOptionValue` `L92`→`L83` | fails | fails ✓ |
| `LetOfPatternThatIsNoName` → `Gap("L74")` | fails, but only because the fragment "… no name in" ends in "in" | fails ✓ (by accident) |
| `AssignmentToFieldOrElement` → `Gap("L74")`, the round-1 wrong owner | **passes**: L74's prose contains the phrase | fails ✗ |
| `WhileOrLoopInFnCalledInside` → `Gap("L74")`; raised only at `symbolic.rs:189` inside a ddx body; L74 quotes "a `while` or `loop` in a `fn` called inside a `ddx`" | **fails**: the row's text adds ": it has no finite symbolic reading" | passes ✗ |
| `MatchArmWhosePatternBindsNameIn` → `Gap("L75")`; L75 quotes "a `match` arm whose pattern binds a name" | **fails**: the row's text adds ", in a function that returns an `Event`" | passes ✗ |

GAPS quotes messages abbreviated, so "every long fragment verbatim" rejects honest owners. Meanwhile, matching anywhere in the line, prose included, admits a form whose phrase L74 merely mentions.

As a result L74, the one GAPS row a source file actually cites (`symbolic.rs:15`), now owns no form. Seven forms raised only inside a ddx body are honestly L74's:

- `WhileOrLoopInFnCalledInside`
- `BreakContinueOrReturnWithNoValue`
- `ExpressionStatementThatIsNoAssignmentIf`
- `StatementThatIsNoLetOfName`
- `IfLetInFnCalledInsideDdx`
- `FnCalledInsideDdxThatForksInto`
- `FnWithNoValueInsideDdx`

At least `MatchArmWhosePatternBindsNameIn` and `WhileLoopBreakContinueOrReturnWith` are likewise L75's.

None of this leaves a form unowned: L74, L75 and `Unswept` all close in `spec-closure`. That is why this is Low. The table still drops the one ownership the task's rule ("`Gap(id)` where a site already cites a `GAPS.md` row") names explicitly, and the guard would let the round-1 mistake back in.

Fix:

1. Match the form against the owning row's last column (the quoted `P0001` messages) only, not its prose.
2. Accept a quote that is a prefix of the form's text, or that equals its text with the context suffix (`{}` or the body clause) removed.
3. Restore `Gap("L74")` on the seven ddx-only forms and `Gap("L75")` on the event-function forms L75 quotes.
4. Lower `unswept_count.txt` to match.

The coordinator asked about "five forms with short text" that lost their owner. I could not find them: the 17 owners lost between 40a85da92 and the head are all `L74` rows, none of them short.

### Phase 3 round 2 findings

| Finding | Severity | Evidence | Fix | Closed |
| --- | --- | --- | --- | --- |
| P3R2-1 the guard admits `AssignmentToFieldOrElement`→L74 and refuses honest L74/L75 owners; L74 owns nothing | Low | `refusals.rs:113-125`; five throwaway edits above; `symbolic.rs:102,128,189,193,198,239,252` | match only the quoted column, as a prefix or with the context removed; restore the ddx-only L74 and quoted L75 owners | |

## Phase 3 round 3

**Verdict**: FAIL (one Medium finding, one Low)
**Head**: a53a47e22 (on a16a6e5e2)
**Verifier**: independent sub-agent (Opus; author != verifier). The fixer is another Opus agent. I reran the gates on target-a and read the full logs. I probed with the head's `piperine` binary, `/var/tmp/piperine-target-a/debug/piperine run <file> --analysis op`, on lone `.msl` files that place a `fn` inside a `ddx`.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards --no-fail-fast` | 11 result lines, 116 passed, 0 failed |
| `cargo test -p piperine-lang --no-fail-fast` | 84 result lines, 1 439 passed, 0 failed; `refusals` 7/7 |

### Round 2 finding and the four proofs

Throwaway owner edits on `forms.rs`, each reverted with `git checkout`:

| Owner | Guard | Expected |
| --- | --- | --- |
| `AssignmentToFieldOrElement` → `Gap("L74")` | fails: L74's `P0001` column quotes none of it | fails ✓ |
| `FormOfOptionValue` → `Gap("L83")` | fails | fails ✓ |
| `WhileOrLoopInFnCalledInside` = `Gap("L74")` (the head) | passes. L74 quotes "a `while` or `loop` in a `fn` called inside a `ddx`", the text cut at `:` | passes ✓ |
| `MatchArmWhosePatternBindsNameIn` = `Gap("L75")` (the head) | passes. L75 quotes "a `match` arm whose pattern binds a name", the text cut at `,` | passes ✓ |
| `LetOfPatternThatIsNoName` → `Gap("L74")` | **passes** | should fail: the form is shared (P3-1) |
| `LetOfPatternThatIsNoName` → `Gap("L75")` | **passes** | should fail: the form is shared (P3-1) |

Reading only the `P0001` cell closes the prose leak of round 2: `AssignmentToFieldOrElement` is refused now. The over-match is P3R3-2.

**Owners.**

- 28 `Gap` and 498 `Unswept`, with `unswept_count.txt` at 498.
- The new rows are seven ddx-only forms raised only in `symbolic.rs:102,128,189,193,198,239,252`, owned by L74, and three event-function forms, owned by L75. Those three join the two L75 rows from before.

**GAPS edits, judged one by one.**

| Edit | The code emits it? | The row covers it? | Verdict |
| --- | --- | --- | --- |
| L74 "an `if let` in a `fn` called inside a `ddx`" (tightened) | yes, probed (`if let Some(z) = o` in the `fn`) | yes (prose: "an `if let`") | honest |
| L74 "a statement that is no `let` of a name, assignment, `if` or `for`" | yes, probed (a deferred `let y;`, a `let … else`) | yes: a deferred `let` and `let … else` have no finite symbolic reading, and a `match` statement is in the prose | honest |
| L74 "a `fn` called inside a `ddx` that forks into more than 4096 paths" | yes, probed (thirteen unfolded `if`s in a `for 0..13`) | **no**. L74's prose says `ddx` runs `if`, and names no path cap. `symbolic.rs:15` lists while/loop/break/match statement/`let` pattern/call for effect, not the cap. It is a limit of the pass (`symbolic.rs:27`), not one of the forms the row describes | quote added to fit the guard |
| L74 "a `break`, a `continue` or a `return` with no value" | **not shown**. `return;` inside the `fn` is `E0180` at type check; `continue` outside a loop is `E0185`; `break` inside a `for` raises the nested-loop form instead | prose: "a `break` or `continue`" | quote of a message no probe printed |
| L74 "a `fn` with no value inside a `ddx`" | **not shown**. A `fn` whose `if`/`else` both `return` with a trailing `;` runs; the checker gives every such `fn` an `f64` value | prose: no | quote of a message no probe printed |
| L86 "a function with an `Option` parameter or result used as a value" | yes, verbatim the text of `closures.rs:79`. The row's prose already named it as "the lowering's own refusal … behind the two the probe meets" | yes | honest. The prose records that the probe did not print it |

Section 4's column is "Diagnostic (probe)", and GAPS's **Evidence** line says a diagnostic "is quoted as the binary prints it" from a lone-file probe.

No other guard or pin reads these cells:

- `repository_records::gaps_section_10…` reads §10.
- `tests/book/inventory_guard.rs` reads row ids and §11.
- `UnnamedTests` reads words for test names; the quotes add none.

**T21.** The **What** line now says each repeated message "is one named variant whose words live in one row … each site builds that variant directly". It is marked as the coordinator's decision pending review, and it agrees with the Done-when. Closed.

### P3R3-1 (Medium): GAPS L74 now quotes messages no probe printed, so the guard passes

To give the seven ddx-only forms their L74 owner, three quotes were written into L74's "Diagnostic (probe)" column without a probe that prints them. Two of them may be unreachable:

- "a `break`, a `continue` or a `return` with no value" (`BreakContinueOrReturnWithNoValue`, `symbolic.rs:193`).
- "a `fn` with no value inside a `ddx`" (`FnWithNoValueInsideDdx`, `symbolic.rs:102`).
- "… forks into more than 4096 paths" is printable, but L74's prose does not describe it. The prose says `if` runs.

These cells were edited to fit the test. An unreachable refusal's owner is the sweep, which proves it unreachable (R4's `P0004` pattern), not a GAPS row. A form the row does not describe needs the row's prose to grow, under the owning feature, before its quote can be added.

Fix:

1. Take the first two quotes out of L74 and set those forms back to `Unswept`, or give each a probe that prints it.
2. For the 4096-path cap, either add it to L74's prose as a known limit of the symbolic pass, or return the form to `Unswept`.
3. Adjust `unswept_count.txt`.

### P3R3-2 (Low): the clause cut lets a shared form take either owner

`QuotedMessages::message_names` (`refusals.rs:141-145`) cuts at every `:`, `,`, ` in ` or ` before ` in the text, not only at its trailing context clause.

The shared `LetOfPatternThatIsNoName`, "a `let` of a pattern that is no name in {}", is raised for ddx bodies and for event functions. Cut at " in ", it matches L74's quote. Filling `{}` with "a `fn` that returns an `Event`", it matches L75's. So `Gap("L74")` and `Gap("L75")` both pass (the throwaway edits above).

An early cut can also let a short quote own an unrelated form whose words share the prefix before the first comma or " in ". No such pair exists among today's quotes, so the risk is latent.

The hand decision that keeps the nine shared forms `Unswept` is right, but no test holds it.

Fix (either):

- Split each shared form by `ExpandedBody` into a ddx form and an event-function form. Each then has one honest owner, and the ambiguity disappears. This also answers the family question of round 2.
- Restrict the cut to the last opener, and refuse a `Gap` on a form whose `{}` is the body clause.

### Phase 3 round 3 findings

| Finding | Severity | Evidence | Fix | Closed |
| --- | --- | --- | --- | --- |
| P3R3-1 L74 quotes two unprobed messages and an undescribed cap to satisfy the guard | Medium | `GAPS.md` L74 last column; `symbolic.rs:102,193,252`; the probes above (`E0180`, `E0185`, a clean run) | drop the two quotes and set their forms back to `Unswept`, or probe them; add the cap to the prose or set it back to `Unswept` | |
| P3R3-2 the clause cut admits either owner on a shared form | Low | `refusals.rs:141-145`; `LetOfPatternThatIsNoName` passes with both `L74` and `L75` | split the shared forms by body, or cut only the trailing clause | |

## Phase 3 round 4

**Verdict**: PASS (one latent weakness recorded as a note)
**Head**: 20d79a232 (on bcb5974ac)
**Verifier**: independent sub-agent (Opus; author != verifier). Gates rerun on target-a with the full logs read. Probes use the head's `piperine run <file> --analysis op`.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards --no-fail-fast` | 11 result lines, 116 passed, 0 failed |
| `cargo test -p piperine-lang --no-fail-fast` | 84 result lines, 1 439 passed, 0 failed; `refusals` 7/7 |

### The six proofs (throwaway owner edits on `forms.rs`, each reverted; tree clean)

| Owner | Guard | Expected |
| --- | --- | --- |
| head as committed | passes | passes ✓ |
| `AssignmentToFieldOrElement` → L74 | fails: L74's column quotes none of it | fails ✓ |
| `FormOfOptionValue` → L83 | fails | fails ✓ |
| `LetOfPatternThatIsNoName` → L74 | fails: "its last `{}` is the body it is raised in" | fails ✓ |
| `LetOfPatternThatIsNoName` → L75 | fails, same message | fails ✓ |
| `IfLetInFunctionThatReturnsEvent` → L74 | fails | fails ✓ |
| `IfLetInFnCalledInsideDdx` → L75 | fails | fails ✓ |

`WhileOrLoopInFnCalledInside` = L74 and `MatchArmWhosePatternBindsNameIn` = L75 are the committed owners, and they pass.

### Round 3 findings

| Finding | Status |
| --- | --- |
| P3R3-1 | **Closed** (20d79a232). The three unprobed L74 quotes are gone, and `BreakContinueOrReturnWithNoValue`, `FnWithNoValueInsideDdx` and `FnCalledInsideDdxThatForksInto` are `Unswept` again. L74's column now holds seven quotes. Every one is printed by the binary: the `while`, the nested `break`, the unfolded `for` bound, the `if let`, the tuple `let`, the expression statement and the deferred `let`/`let … else` were each probed at this head, and each message begins with the quote. Counts: 25 `Gap`, 501 `Unswept`, and `unswept_count.txt` is 501. The 25 are the 18 I checked in round 2, plus 4 ddx-only L74 forms whose message the binary prints, plus 3 L75 forms. Each passes the strengthened guard |
| P3R3-2 | **Closed for shared forms.** `detail_is_context` refuses a `Gap` on any form whose last clause carries a `{}`, which is the body. Both shared-form proofs above fail as they should. Over-matching remains possible in a latent form (note below) |

### Note: the last-clause cut still lets a very short quote own an unrelated form

`context_clause` (`refusals.rs:140-145`) treats the last ` in `, `:`, `,` or ` before ` as the context clause. But in several forms the last ` in ` belongs to the subject, not to a context. Replicating the rule over the 526 rows gives 18 forms whose cut prefix is under 20 characters. For example:

- "a `let` in a param value" cuts to "a `let`";
- "a node declared in generation of a module a parent reads through" (L94) cuts to "a node declared";
- "this pattern in a const expression" cuts to "this pattern".

Six prefixes are shared by several forms ("a `let`" ×4, "an `if let`" ×2, "this statement" ×2, …).

Throwaway proof: I added `"a `let`"` to L83's column and set `StructureForm::LetInParamValue` → L83. The guard **passes**. Both edits were reverted.

So the test's doc comment, "a quote cannot own a form by its first words alone", is not true in general. No current quote is that short, so no owner at the head is wrong. That is why this is a note and not a finding.

A cheap hardening, if wanted: refuse a match whose cut prefix is shorter than the quote's own row needs to be distinctive, for example under 20 characters. Or treat ` in ` as a context opener only when the clause names a body (` in a `fn``, ` in a function`, ` in {}`).

### Round 4 findings

None. I probed:

- the six owner proofs;
- a short-prefix over-match (latent, noted above);
- every L74 quote against the binary;
- the 25 owners through the guard.

Phase 3 (T15 to T22) is verified. The round 1 to round 3 findings are closed by 84e215ff4, a53a47e22 and 20d79a232.

## Phase 4 (B3 Small defects of structure, back end, T23 to T32)

**Verdict**: FAIL (five Medium findings, four Low)
**Head**: dee56eda8 on `feature/code-clarity-p4`, base a0bba62cb
**Verifier**: independent sub-agent (Opus; author != verifier). No `proofs.md` exists yet, because `check_proofs.py` runs after T155. This round reran the phase gates itself.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, then filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets) | all pass (boundaries 20, suite 27, lib 49, worklists 5, proof_ledger 4, the rest 2 each) |
| `cargo test -p piperine-solver` | 371 passed, 0 failed |
| `cargo test -p piperine-analog-solver` | 111 passed, 0 failed |
| `cargo test -p piperine-digital-solver` | 161 passed, 0 failed |
| `cargo test -p piperine-plan` | 68 passed, 0 failed |
| `cargo test -p piperine-transpile` (goldens included; the implementer skipped it after T31/T32) | 171 passed, 0 failed |
| `cargo test -p piperine-interp` | 72 passed, 0 failed |
| `cargo test -p piperine-ir` | 162 passed, 0 failed |
| `cargo test -p piperine-api` (the implementer skipped it after T31/T32) | 307 passed, 0 failed |
| `cargo test -p piperine-cli` (twins) | 144 passed, 0 failed |
| `cargo test -p piperine-parity` | 645 passed, 0 failed |
| `cargo test -p piperine --test book_inventory` | 18 passed, 0 failed |
| CC-09: `git diff a0bba62cb..HEAD` over goldens, twins, `docs/`, `.msl` corpora, `tests/book*` and `crates/piperine-parity/src/contract.rs` | empty, except one new test appended to `crates/piperine-transpile/tests/reject.rs`. The parity tests and `tests/scenarios.rs` changed by call path only (below) |

### Checks

| Check | Test | Holds |
| --- | --- | --- |
| T23: no `expect` on the binding in `analyses/` or `core/circuit.rs` | grep, plus the solver targets | yes. `abi` and `recipe` are held by value. Residue: see P4-9 |
| T24: a test per site drives the input that panicked and reads the error | `pz.rs` `a_matrix_with_an_infinite_entry_is_refused`; `tests/pss.rs` `a_shot_with_no_step_is_an_error_not_a_panic` | partly. The pz test fails without the new non-finite guard, so it proves the refusal. The base code could not panic there, though: a non-finite root was already skipped before the sort (`pz.rs:248`). The pss test never runs a shooting analysis (P4-7) |
| T25: no `unwrap`/`expect` outside `#[cfg(test)]` in analog-solver `src` | grep; each of the five remaining files has its hits below its `#[cfg(test)]` line | yes |
| T26: a test per crate builds the offending `Ir` and reads the error | `structure_shape.rs` `an_own_terminal_on_a_branch_is_a_plan_error_not_a_panic`; `reject.rs` `an_own_terminal_on_a_branch_is_a_rejection_not_a_panic` | plan: yes, because it calls `BodyPlan::build` directly. transpile: **no**, the test is vacuous (P4-5). The 7 panics of `ir/src/build.rs` are deferred with no owner (P4-3) |
| T27: no `eprintln!` in the two crates' `src`; the same bytes through the sink | grep; `convergence.rs` `a_trace_line_reaches_the_sink_with_the_bytes_the_flag_printed` | yes. `writeln!(stderr, "{line}")` writes what `eprintln!("{..}")` wrote. The three format strings are unchanged. The only cost of dropping `Copy` is one `.clone()` in `dc.rs` |
| T28: both `lib.rs` hold no `struct`/`enum`/`impl`/`fn`; public paths unchanged; goldens byte-identical | grep of both `lib.rs`; the transpile `golden` target | yes. No golden file changed. `compile.rs` was folded into `transpiler.rs` as a rename plus the moved type |
| T29: `no_type_of_the_inventory_is_defined_in_two_crates` passes; cli and parity green | `boundaries::no_type_of_the_inventory_is_defined_in_two_crates` | passes, but the guard names the wrong owner for `LogicPattern` (P4-2). The S21 stop condition was met and not honoured (P4-1) |
| T29: parity and `tests/scenarios.rs` edits are call paths only | a normalised diff: `Backend::{selected,parse,VARIABLE}` and `BackendSelection::…` both mapped to one token | yes. All 70 changed lines pair one to one, and every assertion, message and filter is identical. The 16 import lines add `BackendSelection` |
| T30: the 12 items gone; every back-end row has a verdict | `findings.md` §4.1 against the tree; `worklists/test_only_items.tsv` | **no**. 8 of 12 removed. The box is ticked, and tasks.md carries no Deferred note (P4-3). 41 back-end rows are `kept: <host>` with a doc comment. The 2 `piperine-abi` rows remain `-` (P4-8) |
| T31: no `SPEC_DEVIATION` in the back-end crates; each closure named | grep; commit body; `solver-abi-integration/design.md`, `solver-abi-analog/design.md:92` | yes. One stale reference remains (P4-8) |
| T32: `every_single_implementor_trait_is_removed_or_justified` passes with no open row | `worklists.rs` and the tsv | **no** on the letter: `lang::In` and `numbers::Limbs` are `-`. The box is ticked. The three linear-algebra traits are kept against the task text (P4-4) |

### P4-1 (Medium): T29 removed the `GND` alias after its stop condition fired

T29 reads: "S21: stop and report if a corpus, example **or test** uses `GND`, `vss` or `VSS` as ground". design.md's Error Handling table says the same ("T29 stops and reports before removing the aliases").

- `crates/piperine-solver/src/core/builder.rs:243` `edge_gnd_name_routes_to_ground` asserted `b.node("GND") == ground`, which is a test using `GND` as ground.
- The implementer inverted that assertion to `assert_ne!` and went on.

S21 is in "What may move", so the end state is probably what the user wants. The stop-and-report step is not optional, though, and inverting an assertion is the "test edited to pass" pattern that the rules forbid.

Corpus check at the head (`git grep -nwE 'GND|vss|VSS'`, outside `.specs`): no `.msl` corpus, example, Book listing, `docs/` page, parity rig or twin uses an alias as ground. The `pump_tolerance` hits are comments naming the 555's pin.

One residue matters for the decision. The ground pseudo-net's public label is still `"GND"` (`core/net.rs:84`). A host that feeds a net label back into `CircuitBuilder::node` or into a noise reference now gets a fresh node, or `E0801`. No in-tree caller does this.

**Fix**: put the decision to the user and record it. Either ratify the inverted test, with a decision line in STATE/design, or restore the test and keep the aliases until the user decides. Consider whether `Net`'s ground label should become `gnd`.

### P4-2 (Medium): the new guard gives `LogicPattern` to the wrong crate

The guard at `crates/piperine-repository-guards/tests/boundaries/inventory_types.rs:9` reads `("LogicPattern", "piperine-api")`, and the baseline row it seeds is `crates/piperine-lang/src/types/logic_pattern.rs::LogicPattern`.

T52 says the opposite: "`LogicPattern` is declared once (`piperine-lang`) and `api/src/logic_pattern.rs` is gone". Its Done-when is "`no_type_of_the_inventory_is_defined_in_two_crates` passes with no allowance row". After T52 the guard flags the lang type that T52 keeps, so T52 cannot close without editing the guard.

**Fix**: make the owner `piperine-lang`. The baseline row then names `crates/piperine-api/src/logic_pattern.rs::LogicPattern` (allowance `piperine-api/`).

### P4-3 (Medium): three deferrals have no owner, and two Done-when boxes are ticked while false

| Task | What is left | Recorded where | Owner |
| --- | --- | --- | --- |
| T26 | the 7 out-of-range panics of `crates/piperine-ir/src/build.rs:222,556,566,583,586,595,598` | tasks.md "Deferred … Reported to the coordinator" | none. No later task names `build.rs`. "Not a user-input path" contradicts T26's own title: a host misusing `IrBuilder` is the case it covers |
| T30 | `BinaryFormat::round_integer`, `GenericParam::default_type`, `Ty::has_repr`, `DeferReason::InferredGenerics` | commit body only. tasks.md has no Deferred note, and "The 12 items are gone" is ticked | none |
| T32 | rows `lang::In`, `numbers::Limbs` (`-`) | tasks.md Deferred | none. "with no open row" is ticked. T32's own gate names `numbers` ("Quick for abi, numbers, …"), so the task expected lane K to touch it |

The lane argument is sound for wave 1: lane K owns `src/**` of solver, analog-solver, digital-solver, plan, transpile, interp, ir, api and cli only. But a deferral needs a named closer.

**Fix**: untick the false boxes. Add each leftover to a task whose lane owns the path, and name that task in the Deferred note. For example:

- T51 (lane F, owns `piperine-numbers/src` in wave 2) for `round_integer` and `Limbs`;
- T47 to T50 for the three `pom`/`types` items and `In`;
- a lane-K wave-2 IR task (T55 or T56) for `build.rs`, with lane F's five call sites coordinated at rebase.

### P4-4 (Medium): T32 keeps the layer it was told to remove

T32's text is explicit. It says to remove "the linear-algebra layer over `faer` (`LinearSystem`, `SymbolicMatrix`, `SymbolicLinearSystem`, `FaerToNdarray`)", and to keep only `DiscreteSide` and `In`. findings.md A3 says the same: "the linear-algebra layer over `faer` has one backend and three traits, remove".

The worklist instead reads `kept: … removing it edits tests` (`worklists/single_implementor_traits.tsv:1-3`, the doc comments at `crates/piperine-analog-solver/src/math/linear.rs:23,40,51`).

That is a refusal written as a verdict. The test-side cost is small:

- the import at `crates/piperine-analog-solver/tests/support/system.rs:177`;
- the `Solver` alias at `:19`, if `L` goes;
- three `LinearSystem::new` ascriptions in `faer.rs`'s own `#[cfg(test)]` module (`:225,279,349`).

That cost is exactly when a task should stop and report, not re-scope itself. `NetRead`'s keep is a genuine reason: `DiscreteSide` has an associated `Mark`, so it is not object-safe, and `StepView` must erase it. I accept that one.

**Fix**: remove the three traits, with the support-file edit named in the commit as the call-path exception. Or take it to the user as a scope change and give it an owner task.

### P4-5 (Medium): T26's transpile test passes at the base and never reaches the code it claims to prove

`crates/piperine-transpile/tests/reject.rs:127` `an_own_terminal_on_a_branch_is_a_rejection_not_a_panic` cannot fail on the claim, for two reasons:

- `Transpiler::new` runs `ir.check()` first and returns `Error::Rejected` (`transpiler.rs:35-38`). `check()` refuses the own terminal.
- Even past that, the body has `analog: None` and no initial node, so `runs` is false and `BodyPlan::build` is never called.

The base `Transpiler::new` (a0bba62cb `lib.rs`) is identical in both respects, so this test passes with the fix reverted. Neither of the two print sites it was meant to cover (`print/contribution.rs:78`, `print/value.rs:401`) is reached.

From the public API those sites are unreachable behind `check()`. The honest proof is a crate-internal test that hands the printer an `Own` terminal. The alternative is to state that the sites are unreachable and say so in the Done-when.

Related (Low): `BodyPlan::new` stays `pub` and still panics (`crates/piperine-plan/src/plan.rs:555-560`). All production callers moved to `build`, but a host calling `new` on an unchecked body still panics.

### P4-6 (Low): T24's pss test does not drive the shooting

`crates/piperine-solver/tests/pss.rs:144` calls `TransientAnalysisResult::final_step` on an empty result. Reverting any of the four `pss.rs` sites to `.last().unwrap()` keeps it green. Its doc comment says "the shooting reads the error", which it does not show.

The pz test asserts only `is_err()`, not `SolverDomain::Pz`. Its message says "NaN" while the entry is `f64::INFINITY`. My own example, a `NaN` entry, is caught by the same `is_finite` guard.

**Fix**: assert the domain. Either drive `PssSolver` with a stub that returns an empty shot, or narrow the claim to the helper.

### P4-7 (Low): two test edits outside the call-path exception

- **T23** rewrote the setup of `sp.rs` `zero_ports_fails_loud`: it now builds through `CircuitBuilder`, because the `pub(crate)` `from_netlist` gained two parameters. The assertion is unchanged, but this is not a "`pub` fn became a method" edit.
- **T29** inverted `edge_gnd_name_routes_to_ground` (P4-1).

### P4-8 (Low): small residues

- The two `piperine-abi` rows of `test_only_items.tsv` are still `-`. abi is a back-end crate, though not one of lane K's wave-1 paths. They need an owner, like P4-3.
- `.specs/features/solver-abi-analog/design.md:92` still says "(the code's `SPEC_DEVIATION` markers)", and T31 deleted those markers.

### P4-9 (Low): `bound()` still says a circuit can be unbound

`CircuitInstance::bound`/`bound_mut` (`crates/piperine-solver/src/core/circuit.rs:56,65`) return `Option`, which is always `Some`. About 30 test and host call sites still `.expect("bound")`. The field is right, but the public type still states the invariant T23 removed. Changing it edits tests, so it needs an owner (for example the B5b solver task).

### Allowances and the dictionary

- **Allowances.** Rows were deleted or moved net-zero (`long_functions` `BodyPlan::new` → `build`, `undocumented_items` `compile.rs` → `transpiler.rs`, `apply_iv`). The one new file is `allowances/piperine-lang/inventory_types_in_two_crates.tsv` (2 rows). T29's Done-when sanctions it ("the rows T51 and T52 close are allowance rows"), so it is not an added allowance in the forbidden sense. Its `LogicPattern` row points at the wrong crate (P4-2).
- **Dictionary.** `transpiler` joining `data/dictionary.txt` is a domain word, the crate's own name. Accepted.

### Code rules on new code

- **Free functions.** The only new free functions are `#[test]` functions. `ShapeLookup`, `RealCell`, `GroundName`, `BackendSelection`, `TraceSink` and `PlanError` are types with methods.
- **Docs and suppressions.** Every new item has a `///`. There is no macro and no `#[allow]`.
- **Unwraps.** No new `unwrap`/`expect` is reachable from input. The two added are in `#[cfg(test)]`.

Three notes, not findings:

- `RealCell::read` zero-extends a short buffer instead of failing.
- `NewtonRaphsonSolver::commit` labels an empty buffer with the singular-matrix error.
- `TraceSink::write` drops a poisoned lock.

All three are unreachable after `validate`, and none is silent on a real path.

### Round 1 findings

| Finding | Severity | Closed |
| --- | --- | --- |
| P4-1 S21 stop condition overridden | Medium | |
| P4-2 `LogicPattern` owner reversed in the guard | Medium | |
| P4-3 deferrals with no owner; ticked boxes that are false (T26, T30, T32) | Medium | |
| P4-4 linear-algebra traits kept against T32 | Medium | |
| P4-5 transpile T26 test vacuous | Medium | |
| P4-6 pss test does not drive the shooting; pz assertion loose | Low | |
| P4-7 test edits outside the exception | Low | |
| P4-8 abi rows open; stale design reference | Low | |
| P4-9 `bound()` still `Option` | Low | |

## Phase 4 round 2

**Verdict**: PASS. All nine round 1 findings are closed in code by 471de10ca. Two Low notes stay open for the coordinator and block nothing.
**Head**: 471de10ca on `feature/code-clarity-p4`, base a0bba62cb
**Verifier**: independent sub-agent (Opus; author != verifier). Gates were rerun in the foreground, with target dir `-d`.

### Gates at 471de10ca

| Gate | Result |
| --- | --- |
| `**Build**` line (`cargo build --workspace --all-targets`, filtered for `^(warning|error)`) | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` | 116 passed, 0 failed |
| `cargo test -p piperine-abi` | 58 passed |
| `cargo test -p piperine-analog-solver` | 111 passed |
| `cargo test -p piperine-solver` | 371 passed |
| `cargo test -p piperine-plan` | 68 passed |
| `cargo test -p piperine-interp` | 72 passed |
| `cargo test -p piperine-transpile` (the `golden` target included) | 172 passed |
| `cargo test -p piperine-api` | 307 passed |
| `cargo test -p piperine-cli` (the `twins` target included) | 144 passed |
| `cargo test -p piperine-parity` | 645 passed |
| `cargo test -p piperine --test book_inventory` | 18 passed |
| `cargo test -p piperine --test integrated` (`tests/integrated/package.rs` was edited) | 42 passed |
| CC-09: `git diff a0bba62cb..HEAD` over goldens, twins, `docs/`, `examples/`, `.msl`, `tests/book*` and `contract.rs` | empty. The only change near a golden is `tests/golden.rs` dropping `.expect(..)` on `bound()`, a call path |

### Closure of round 1

| Finding | Closed by | Evidence |
| --- | --- | --- |
| P4-1 | 471de10ca | `GroundName::names` accepts `gnd`/`GND`/`vss`/`VSS` again (`core/ground.rs`). `edge_gnd_name_routes_to_ground` carries its base assertion (`assert_eq!(g, n2)`). The ground behaviour is now identical to a0bba62cb; only the owner changed. S21 is a T29 Deferred note to the user, with the `GND` label as the reason |
| P4-2 | 471de10ca | The guard owner is `piperine-lang` (`inventory_types.rs:9`). The `LogicPattern` row moved from `allowances/piperine-lang/` to `allowances/piperine-api/inventory_types_in_two_crates.tsv` as `api/src/logic_pattern.rs::LogicPattern 1`. That is net-zero: one row out, one row in, the same count, and T52 deletes it |
| P4-3 | 471de10ca | Owner lines exist as unticked Done-when items: T43 `lang::In`; T48 `InferredGenerics`; T49 `Ty::has_repr`; T50 `default_type`; T51 `round_integer` and `Limbs`; T55 the 7 `build.rs` panics. The T26, T30 and T32 boxes now say what is true, with the open parts unticked. Lane reach: see note R2-2 |
| P4-4 | 471de10ca | `LinearSystem`, `SymbolicMatrix` and `SymbolicLinearSystem` are gone (`math/linear.rs` holds `AsIndex` and `Stamp`), and `NewtonRaphsonSolver<A, E>` has no `L`. The methods are inherent, with `///` on each. Test edits are the import at `support/system.rs`, the `Solver` alias and three `FaerSparseLinearSystem::new` ascriptions, which are type paths only. `reset_clears_stamps_without_reallocating` and the other faer tests are unchanged |
| P4-5 | 471de10ca | `BodyPlan::new` is gone. Every `.expect("the body plans")` in `src` sits below a `#[cfg(test)]` line: `print/{analog,cross,digital,limit,shape,site}.rs` at 790/97/314/224/453/377. The two production callers, `interp/src/interpreter.rs:76` and `transpile/src/transpiler.rs:47`, `map_err` into `Rejected`. The vacuous `reject.rs` test is gone. The two new unit tests call `ValuePrinter::probe(Own, Ground)` and `ContributionSums::branch_text` on an `Own` branch. Both APIs exist at a0bba62cb (`analog.rs:80` `values()`, `contribution.rs:75`), where those exact arms are `panic!(..)` (base `value.rs:401`, `contribution.rs:80`), so both tests would fail there. "A checked Ir cannot reach them" holds: `probe` is called only with a branch's terminals (`analog.rs:659`), and `check.rs:318` refuses an `Own` branch terminal |
| P4-6 | 471de10ca | pz asserts `Error::Simple { domain: Pz, .. }` with "infinite". pss is renamed `a_result_with_no_step_has_no_final_step_error` and asserts the `Transient` domain. The Done-when and the doc comment narrow the claim to the reader. Judgement below |
| P4-7 | 471de10ca | The T23 Notes name the `sp.rs` setup change |
| P4-8 | 471de10ca | The two `piperine-abi` rows are `kept: <host>`, with doc comments at `abi/src/raw.rs` and `abi/src/verify.rs`. `solver-abi-analog/design.md:92` now says the markers were deleted by T31 |
| P4-9 | 471de10ca | `bound()` returns `&Bound` and `bound_mut()` returns `&mut Bound`. The api edits (`probe.rs:281,289`, `pins.rs:22`, `session.rs:721,793,888,938`) drop an `Option` path that could not be `None`. The removed `none("the circuit binds no design")` was unreachable. A normalised diff of every test edit (`BodyPlan::new(x)` → `build(x).expect(..)`, `.bound().expect/unwrap` → `.bound()`) leaves no other change, and no assertion moved |

**P4-6 judgement.** T24's claim is "an empty shoot is an error, not a panic". After the fix, every one of the five `pss.rs` sites returns `Err` through `final_step()?`. The t=0 site returns through `ok_or_else(.. Pss ..)`. So no panic remains, which is the claim, and the reader's error is pinned by the test.

The `?` is not dead code worth deleting. It is the T25-style replacement of an `unwrap` on an invariant: shooting records its first step, so the error path is unreachable today, and the `?` keeps a future empty result from becoming a panic. Removing it would bring back the `unwrap` T24 exists to remove. The narrowed wording is honest. I accept it.

### Round 2 notes (Low, not blocking)

- **R2-1.** `GroundName::TEXT` (`crates/piperine-solver/src/core/ground.rs:8`) is a new `pub` item that nothing references, which is the kind of item T30 removes. The type doc also says "whatever the case", but only four spellings match (`Gnd` does not). Fix: drop `TEXT`, or use it in `names`, and say "these four spellings".
- **R2-2.** Two owner tasks reach paths that the design's Lanes table does not give their lane in their wave:
  - T49 (lane F, wave 2, `{msl,pom}/**`) removes `Ty::has_repr` in `lang/src/types/ty.rs`, which is a wave-3 path.
  - T55 (lane K, wave 2) edits `lang/src/elab/{place,resolvers}.rs`.

  No other lane owns those paths in wave 2, so nothing collides. The table should still name them, or T49's item should move to a P10 task.

## Phase 6 (B6a Owners in `msl/`: lexer, printer, parser, T41 to T46)

**Verdict**: PASS (no Medium or High finding; four Low findings and three notes)
**Head**: 9eb8c0a42 on `feature/code-clarity-p6`, base 7bfe85f7d.
**Verifier**: independent sub-agent (Opus; author != verifier). Sonnet wrote T41–T43 and T45–T46, and Opus wrote T44. I reran the gates myself, one at a time, with `CARGO_TARGET_DIR=/var/tmp/piperine-target-a`.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, then filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-lang` | 84 result lines, 1 439 passed, 0 failed |
| `cargo test -p piperine-repository-guards` | 11 result lines, 116 passed, 0 failed |
| `cargo test -p piperine-cli` (twins included) | 12 result lines, 144 passed, 0 failed |
| `cargo test -p piperine-parity` | 64 result lines, 645 passed, 0 failed |
| `cargo test -p piperine-project` | 7 result lines, 53 passed, 0 failed |
| `cargo test -p piperine --test book_inventory` | 18 passed, 0 failed |

### Checks

| Check | Test or probe | Holds |
| --- | --- | --- |
| CC-09 behaviour: expression grammar (T44) | A side-by-side read of the base `build`/`block_like`/`stmt` against the head's `ExpressionGrammar` methods | yes. The Pratt table has the same levels, operators and binding powers (15 postfix … 4 `\|\|`). The postfix `first_of` order is method/field, call, index, `?`. The atom `first_of` order is unchanged (14 alternatives, `.labelled("expression")`), as are the range `first_of`, the assignment operator `choice`, the statement `first_of` order (item, then meta + empty/let/signal/on/block-like/simple), the `Recovery::recover` wrapper, and the `else` select that is not listed among expected tokens. `.boxed()` sits where it did on `assignment`, `operators`, `block`, `path_expression` and every `first_of` arm. `emit_expression` folds the four "first argument" arms into one, with the same span per variant and the same message |
| CC-09 behaviour: printer (T42) | A multiset diff of every non-comment line of the base `print.rs` against the head's `print.rs` + `print/*.rs` | yes. Every differing line is a rename (`enc`→`escape`, `dec`→`restore`, `doc`→`documentation_comment`, `binop`→`binary_operator`, `vis`→`visibility`, `dir`→`direction`), a `Self::` prefix, a `pub(super)`, or a `use`. No format string changed |
| CC-09 behaviour: check (T46), lexer (T41) | Diff read of `check.rs`; `float_dot`→`closes_float`/`float_closed_by_dot`, `unescape` | yes. `check.rs` is indentation plus `ParseCheck::` prefixes and three renames. The lexer's float rule is the same predicate |
| CC-09 behaviour, whole crate | The unedited assertions of `msl_errors` (recovery, spans, E0910 depth/chain), `msl_format`/`msl_print` (round trip, idempotence, 20 000-link chains), `msl_corpus`, `std_library`, the CLI twins, the Book inventory | yes, all pass |
| Test edits are call paths only | `git diff --word-diff` of `crates/piperine-lang/tests` and of the CLI and project changes, as token counts | yes. The only removed or added tokens are `parse`→`SourceParser::parse`, `parse_expr`→`ExpressionGrammar::parse`, `parse_{type,pat,generics,module_item,block}`→`<Grammar>::parse`, `lex`/`lex_bytes`→`Lexer::…`, `print`→`Printer::print`, `render(d, …)`→`d.render(…)`, and their `use` lines. No assertion, literal or expected value changed |
| T43 deferrals closed | `grep 'fn parse_(expr\|type\|pat\|generics\|module_item\|block)'` over `crates/` | yes. None remain: T44 took `parse_expr`; T45 took `TypeGrammar`/`PatternGrammar`/`GenericsGrammar::parse`; T46 took `ModuleGrammar`/`BlockGrammar::parse` |
| T46 "0 `free_functions` rows for `msl/`" | `grep -c msl/ allowances/piperine-lang/free_functions.tsv` | yes, 0 rows. `diag::render` is `Diagnostic::render` |
| T44 split rows | `fnlen` probe: brace-matched length of every `fn` in `msl/` | yes. Every `ExpressionGrammar` method is ≤ 60 lines; `block_like` and `stmt` are gone; `classify_number` and `lex_with_comments` are also under 60. The ten functions over 60 (`expr_inner` 103, `ParseCheck::statement` 80, `expression_inner` 87, `lex_limited` 95, `ItemGrammar::fn_decl` 99, `ItemGrammar::definition` 188, `recover_item` 122, `module_decl` 79, `PatternGrammar::definition` 193, `TypeGrammar::definition` 113) are exactly the ten rows `long_functions.tsv` carries at the head |
| Cross-lane edits (T46) | Diff of `piperine-cli/src/commands/fmt.rs`, `render/diagnostic.rs`, `piperine-project/src/error.rs` | yes. Each is `diag::render(d, …)`→`d.render(…)` and a dropped `use` |
| Allowances only delete or move | `git diff` of `allowances/` and `worklists/` | yes. `free_functions` −161, `owners` −11, `repeated_type_names` −2. `undeclared_words` +16/−61: the 16 are printer rows moved from `print.rs` to `print/*.rs`. `undocumented_items` +43/−87: each `+` is the same item moved to its owner, some renamed (`expr`→`expression`, `stmt`→`statement`, `dir`→`direction`, `path_with`→`path_with_sugar`, `build`→`definition`). `long_functions` +10/−15 |
| Dictionary and glossary | `data/dictionary.txt`, `data/glossary.tsv` | yes. `postfix`, `punctuation`, `recovery` and `unclosed` are English words, used in `apply_postfix`, `TokenInput::punctuation`, `mod recovery` and `unclosed_delimiters`. `si` is a glossary row with a meaning, used in `classify_si`, `is_si_letter`, `malformed_si` and `si_exponent` |
| No free fn, no macro, no `#[allow]`/`#[expect]` in new code | `grep` over `msl/` | yes. `macro_rules` appears only as the parser's refusal of the token |
| No `unwrap`/`expect` on user-input paths | `grep 'unwrap(\|expect(\|panic!\|unreachable!'` over `msl/` | yes. The lexer's two `expect`s and the entry's bookkeeping `expect`s are gone. The one `panic!` left is `ParseDiagnostics::removed` (`diagnostics.rs:23`), and every caller passes a `&'static str` literal from the grammar (including the `row: &'static str` of `item.rs:457`), never input. A wrong row is a programming error that a test parse of each form hits |
| Commit subjects, one per task, `tasks.md` in each | `git show --stat` | yes |

### P6-1 (Low): `ItemGrammar` is a public type with no public method

`crates/piperine-lang/src/msl/parser/mod.rs:34` re-exports `pub use item::ItemGrammar`, and `item.rs:7` declares it `pub`. All of its methods are `pub(crate)` or private, so the type is an inert name in `piperine_lang`'s public surface, and nothing outside the crate names it. Repro: `grep -rn ItemGrammar crates tests | grep -v src/msl/parser/` prints nothing. The fix is `pub(crate)` on both lines (as with `ParseCheck`). `MAX_CHAIN` also became reachable through the new re-export (`mod.rs:38`). That one matches msl-polish's design, which lists it as public surface, so it is not a finding.

### P6-2 (Low): `BlockGrammar` is a shell around one test-only entry point, and the `parse` name hides all six from the test-only worklist

`BlockGrammar` (`expr.rs:97-107`) holds only `parse`; the block production stays `ExpressionGrammar::block`. `GenericsGrammar` is honest by comparison, because it owns the `parameters` production that `ItemGrammar` uses. None of the six fragment `parse` methods (`ExpressionGrammar`, `TypeGrammar`, `PatternGrammar`, `GenericsGrammar`, `ModuleGrammar`, `BlockGrammar`) has a caller outside `tests/`. Repro: `grep -rn 'Grammar::parse' crates --include=*.rs | grep -v /tests/` finds only the definitions. T43's and T44's notes say why the guard misses them: `parse` is a word product code spells, so `test_only_items` cannot see that these particular `parse`s are test-only.

This is not a regression: the base free functions were just as test-only and were not in the worklist either. The design choice is defensible, since a fragment entry point is the grammar's own `parse`. But the method makes the coverage gap invisible rather than recording it. A `kept: <test surface of the grammar>` row for each of the six would be the honest record. `SourceParser` in place of the plan's `Parser` is justified by the `chumsky::Parser` clash through `use super::*` and is fine.

### P6-3 (Low): ten `long_functions` rows were renamed and left open, against design's "each task gives the rows of its files a verdict"

T44 split its rows and deleted them, as its box says. T42, T43, T45 and T46 instead moved their rows to the new paths with verdict `-`:

- `Printer::expr_inner`
- `SourceParser::lex_limited`
- `ItemGrammar::{definition,fn_decl}`
- `PatternGrammar::definition`
- `TypeGrammar::definition`
- `ParseCheck::{statement,expression_inner}`
- `ModuleGrammar::{module_decl,recover_item}`

Source: `worklists/long_functions.tsv:66-75`.

T45's text and Done-when do not ask for a split or a verdict, so T45 is not failed. `design.md:412` does ask for one, though ("each task above gives the rows of its files a verdict, `split` or `kept: <reason>`"), and T41's box is the only one in this phase that says so explicitly. The open rows pass today only because open rows are allowance rows until the closing task (`tasks.md:3739`). They are debt carried forward, not judged.

### P6-4 (Low): five Done-when boxes say "no existing test edited" while their commits edit tests

T41 (`msl_lexer.rs`, `msl_print.rs`, `spec_structure.rs`), T42, T43 (16 test files), T45 and T46 each tick "`cargo test -p piperine-lang` passes with no existing test edited", yet each commit edits test files. Every edit is a call path, so the substance holds. Only T44's box names the exception ("the calls of `parse_expr`"). The other five boxes are literally untrue and should carry the same parenthesis.

### Notes (not findings)

- `Lexer::float_closed_by_dot` (`lexer.rs:365`) replaced `expect` with `let … else { return }`. If the invariant ever broke, the popped token and the dot would be dropped silently instead of panicking. `closes_float` (`lexer.rs:353`) is checked just before and guarantees the `Int`, so the branch is unreachable. Behaviour is unchanged.
- The renamed undocumented items (`ParseCheck::expression`, `ItemGrammar::direction`, `TypeGrammar::path_with_sugar`, the three `definition`s) kept their `undocumented_items` rows. That is allowed as a move, but these are the identifiers the authors touched most, and a `///` would have closed the rows.
- a09b3c257 inserted an empty line 3 into `data/dictionary.txt`. It is cosmetic, and the guard ignores it.

### Phase 6 findings

| Finding | Severity | Status |
| --- | --- | --- |
| P6-1 `ItemGrammar` public and inert | Low | open |
| P6-2 `BlockGrammar` shell; six test-only `parse`s invisible to the worklist | Low | open |
| P6-3 ten `long_functions` rows renamed with no verdict | Low | open (debt to the closing task) |
| P6-4 "no existing test edited" boxes untrue as worded | Low | open |

## Phase 5 (B2a Device fixtures and the back-end merges, T33 to T40)

**Verdict**: FAIL (three Medium findings, five Low)
**Head**: ef7a2da28 on `feature/code-clarity-p5`, base 97d327d58
**Verifier**: independent sub-agent (Opus; author != verifier). There is no `proofs.md` yet, because `check_proofs.py` runs after T155. This round reran the phase gates itself, one at a time.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, then filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (all targets, after touching `src/*.rs`) | all pass: lib 50, boundaries 20, suite 29 (with the two new ratchets), proof_ledger 4, worklists 5, ownership 5, the rest 2 each |
| `cargo test -p piperine-abi` | 59 passed (58 at the base, plus the T33 test) |
| `cargo test -p piperine-analog-solver` | 111 passed, the same as the base |
| `cargo test -p piperine-digital-solver` | 161 passed, the same as the base |
| `cargo test -p piperine-device-fixtures` | 1 passed (`its_devices_pass_the_partials_verifier`) |
| `cargo test -p piperine-solver`, and `--tests -- --test-threads=1` | 371 passed both ways, the same as the base |
| `cargo test -p piperine-parity` | 645 passed |
| `cargo test -p piperine --test book_inventory` | 18 passed |
| CC-09: `git diff 97d327d58..HEAD` over `docs/`, `examples/`, `tests/`, `crates/piperine-{parity,transpile,cli}`, `*.msl` and goldens | empty. `crates/piperine-parity/src/contract.rs` is untouched |

### Timing (the user's goal: a leaner, faster suite)

Measured in the same target dir with a warm dependency cache. The base came from a `git archive` of 97d327d58 in `/var/tmp`, which was deleted afterwards.

| `piperine-solver` | 97d327d58 (57 binaries) | ef7a2da28 (8 binaries) |
| --- | --- | --- |
| `touch src/lib.rs`, then `cargo test --no-run` (lib rebuild plus every test link) | 68.2 s | 50.8 s (the fixtures crate rebuild included) |
| `cargo test -p piperine-solver`, with everything built | 6.25 s | 1.67 s |

The same 371 tests run in both. The gain is real: link time drops by about a quarter and run time to a quarter.

### Tests kept (scrutiny 1)

- I extracted every `#[test] fn` from the five crates at both revisions, 699 before and 701 after. Each name keeps its count.
- The two additions are the T33 test `a_device_that_only_evaluates_reacts_and_continues` and the fixtures crate's `its_devices_pass_the_partials_verifier` (28 copies of that name now, 27 before).
- For every test I compared the assertion count and the numeric and string literals between the two revisions. 4 tests differ, and all four are the boundary edits judged under scrutiny 5:
  - `crate_declares_no_dependency`
  - `the_crate_depends_on_the_abi_crate_only`, in both analog and digital
  - `every_hand_written_device_is_in_a_partials_check`, see P5-5
  - `a_whole_read_cell_holds_an_array_view`, where only the `extern "C" fn` moved into `ArrayViews`
- Every other body change is a call-path rewrite: a free helper became `Type::method`, and `add_vsrc` became `CircuitBuilding::add_voltage_source`.
- The repoints are all `move` rows, 11 + 13 + 16 + 22 + 30 = 92. Four modules carry a full-word name rather than the former target's: `abi_sens`→`abi_sensitivity`, `adjoint_vs_sens`→`adjoint_against_sensitivity`, `sens`→`sensitivity`, `stepper_fold`→`step_fold`. Each has its `move` row, and I accept them under the full-words rule.
- `proof_ledger` passes.

### Seeded allowances (scrutiny 2)

- At T34 (026c8d269), `test_support_shared_by_path` holds 548 rows with a summed count of 548. `allow_in_test_code` holds 243 rows with a summed count of 498, and `allow_attributes` holds 3 rows with a count of 4.
- At the base, `allow_attributes` held 246 rows with a count of 502. 243 + 3 = 246 and 498 + 4 = 502, so the move is net-zero.
- At the head, the files hold 401 rows/401 and 169 rows/352. The back-end crates have no row in either file.
- The rows are exact keys `file::item<TAB>count`, with no wildcard. `Allowance::judge` fails on a count above the real one (stale) and on one below it (unlisted), so a green guard proves the rows are exactly today's violations.
- Throwaway probes, all reverted, with `git status` clean afterwards:
  - I appended `#[allow(dead_code)] #[path = "host_set.rs"] mod probe_duplicate;` to `solver/tests/host/main.rs` and raised `piperine-lang/tests/book.rs`'s two rows to 2. Both ratchets failed, each naming the new row as unlisted and the raised row as stale.
  - I put a free `#[allow]`ed fn into `device-fixtures/src/lib.rs`. `no_free_helper_function_in_test_code` failed on it, so the fixtures crate is held to the test-code rules.

### Judgements

- **Scrutiny 5, the fixtures dev-cycle.**
  - design.md:255 names the crate's users as "abi, analog-solver, digital-solver, solver (dev-dependency)". The fixtures crate depends on solver, so the dev-cycle is inherent to the approved design. The `DOCUMENTED_ARROWS` row and the three boundary edits are its necessary consequence.
  - Every loosening is scoped to the one name `piperine-device-fixtures`.
  - A normal (non-dev) dependency on it from a back-end crate would be a cargo cycle, and `DOCUMENTED_ARROWS` would refuse it anyway. Accepted.
  - Cost noted: `cargo test -p piperine-abi` now builds the whole back end.
- **Scrutiny 5, "each device once".** Not met: P5-2.
- **Scrutiny 5, `*-fixtures` as test code.**
  - Acceptable as a classification. It is what makes `test_only_items` and the test-code ratchets right, and `#![deny(missing_docs)]` covers the documentation rule.
  - It also removes the crate from `long_functions` (none over 60 lines today), `single_implementor_traits`, `repeated_type_names` (which would have flagged P5-2) and the module-structure rule.
  - `is_fixtures_crate` (`source_tree.rs:120`) has no unit test. Low: P5-8.
- **Scrutiny 6, `context_init`.** The reason is real. `Context::init_global` sets faer's global parallelism behind a process `Once`, and the test asserts that `Context::default()` has not done so. Any earlier solver build in the same binary would make it fail or pass vacuously. The approved T39 text said 6, though: P5-6.
- **Scrutiny 9, the code rules.**
  - There is no free function, no macro and no `#[allow]` in the new test or fixtures code (the guards are green, and the probe above shows they reach the fixtures).
  - Names pass the dictionary guard, and no new undeclared word was added (the allowances only shrank: abi 53→42, analog 120→96, digital 37→27, solver 288→216).
  - Doc quality is poor: P5-7.

### Checks

| Check | Test | Holds |
| --- | --- | --- |
| T33: a device with `eval` only, and its `react` thunk returns `Continue` | `thunks::thunks::a_device_that_only_evaluates_reacts_and_continues` | yes. `EvalOnly` declares one reaction and writes no `react`. The test asserts `status::OK` and that nothing is committed |
| T34: `#![deny(missing_docs)]`, no free function, one `PartialsCheck` over every device, once | `device-fixtures/tests/partials.rs` `its_devices_pass_the_partials_verifier` | **no**. 27 copies plus 4 variants remain, and the check misses `digital::PulseGenerator` (P5-1, P5-5). "Each device once" and "analog-only ones with no `react`" are unmet (P5-2, P5-4) |
| T34: the two ratchets pass under `**Run suite**` | `suite::test_code::{no_test_support_is_shared_by_path,no_allow_in_test_code}` | yes, both ways (probes above) |
| T35: 2 targets, a header each, names kept, no `#[path]`/`#[allow]`/free helper; plan 2 | `target_plan`, the ratchets, the name diff | yes |
| T36: 3 targets, names kept, `support/devices.rs` gone, plan 3 | the same | yes |
| T37: 3 targets, names kept, `support/devices.rs` gone, plan 3 | the same | yes |
| T38: the three targets pass with `--test-threads=1` and the default; names kept | solver run both ways | yes on the Done-when. **no** on the What: the 27 copies did not leave, and there are no `replace` rows (P5-1) |
| T39: 7 targets, `tests/helpers/` gone, plan 7, both ways | the same | yes, against an approved text the implementer edited from 6 (P5-6) |
| T40: no row of the four crates in the three allowances | the allowance files | honestly unticked. The `tests/` rows are closed and 36 `src/` rows remain. The named owners do not carry them (P5-3) |

### P5-1 (Medium): the 27 partials-verifier copies were not retired, and nothing owns them

- tasks.md:1166 (T38 What): "the 27 copies of `its_devices_pass_the_partials_verifier` leave with `replace` rows to the one of T34". T34's Done-when (tasks.md:1086) ticks "`its_devices_pass_the_partials_verifier`, once".
- At the head:
  - 27 copies remain in `crates/piperine-solver/tests/`, plus `its_device_passes_the_partials_verifier` in `analyses/noise_report.rs:137` and `transient/truncation.rs:222`, and `its_corner_source_…`/`its_once_faulting_device_…` in `truncation.rs:216,227`.
  - The 59 `impl Device for` in the solver's tests stay target-local.
  - `repoints/T38.tsv` has no `replace` row.
- Both boxes are ticked, and no task owns the remainder.
- Repro: `grep -rln 'fn its_devices_pass_the_partials_verifier' crates/piperine-solver/tests | wc -l` prints 27.
- Fix: either move the target-local devices into the fixtures crate and retire the copies with `replace` rows, or untick the T34 and T38 items and name an owner task with a Done-when line.

### P5-2 (Medium): "each device once" is not met, and the M6 file was copied rather than replaced

- T34 What: "each device once" and "`solver/src/analyses/test_devices.rs` (M6)" among the sources replaced.
- The fixtures crate declares one device type several times:
  - `Resistor` 3× (`examples.rs:30`, `circuit.rs:43`, `analyses.rs:18`).
  - `Capacitor` and `VoltageSource` 3× each.
  - `Diode`, `Comparator`, `Counter`, `Timer`, `DataFlipFlop`, `Inverter`, `Recorder`, `Sampler`, `Stub`, `Faulty`, `Tracer`, `Inductor`, `NoisyResistor` and `Call` 2× each, for 18 repeated names in all.
- The three `Resistor`s are the same `I = V/r` device. They differ in the `origin` string, the default bytes and whether `∂/∂r` is declared. The commit's reason, that "merging would change behaviour", holds for the stateful digital variants but not for these.
- `fixtures/src/analyses.rs` (391 lines) is a copy of `solver/src/analyses/test_devices.rs`, which still exists and is still what `pz.rs:381` and `sp.rs:246` use. The copy's only consumer is the fixtures partials test.
- Repro: `grep -rlE '^\s*pub struct Resistor\b' crates/piperine-device-fixtures/src` lists 3 files.
- Fix: merge the pure-analog duplicates. The `test_devices.rs` side is lane K's `src/**`, so either drop `analyses.rs` until lane K repoints `pz`/`sp` to it, or give that repoint an owner line.

### P5-3 (Medium): the T40 deferral names owners whose text does not carry it, and the dependency is circular

- T40's box (tasks.md:1219) says the 36 `test_helper_functions` rows stay open "for lane K's back-end phase, T77 to T85 (T84 for the solver analyses)". None of T77 to T85 mentions `test_helper_functions`, `#[cfg(test)]` helpers, or `test_devices.rs`.
  - T84 (tasks.md:2208) closes the **free-function** allowance of product code.
  - T78 splits `raw.rs` by section.
- T77 and the lane-K phase "Start after T40" (tasks.md:178, 2057), so T40 waits on lane K and lane K waits on T40.
- The box itself is honestly unticked.
- Rows still open: abi `raw.rs` 1; analog `convergence.rs`, `math/{faer,integration,newton_raphson,singular}.rs`, `stamp_validation.rs` 6; solver `pz.rs` 5, `sp.rs` 3, `test_devices.rs` 20, `protocol/statistics.rs` 1.
- Fix:
  - Add a Done-when line naming these rows to the owning tasks (T77 for abi, T79 or T80 for analog, T84 for solver).
  - Make T40's remaining item depend on them, or split T40 so lane K's start does not wait on its open half.

### P5-4 (Low): 55 trivial `react` bodies survive the T33 default

- T34 What: "analog-only ones with no `react`".
- `crates/piperine-device-fixtures/src` still holds 55 `fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> { Ok(Flow::Continue) }`, for example `circuit.rs:88,159,221`. Each is exactly the new default.
- Repro: `grep -rn -A1 'fn react' crates/piperine-device-fixtures/src | grep -c 'Ok(Flow::Continue)'`.

### P5-5 (Low): the "every device is in a partials check" guard no longer reaches the shared devices

- At the base, `every_hand_written_device_is_in_a_partials_check` scanned `tests/helpers/*.rs` with `device_impls` and required each helper device in a check.
- At the head (`solver/tests/abi_devices/abi_devices.rs:77`), it scans only the solver's `tests/`. The fixtures crate's coverage rests on the hand-written list `EveryDevice` (`device-fixtures/tests/partials.rs`), which no scan holds to the crate's `impl Device for` set.
- That list already misses `digital::PulseGenerator` (`device-fixtures/src/digital.rs:916`). Before, that device lived in digital-solver's support, where no partials check ran, so nothing regressed for it. The guard's reach is what shrank.
- Repro: `grep -c PulseGenerator crates/piperine-device-fixtures/tests/partials.rs` prints 0.
- Fix: build `EveryDevice` from a scan of the crate's `impl Device for` items, or add a test that compares the two.

### P5-6 (Low): an approved Done-when was edited without a recorded approval

- T39's Done-when read "6 targets … Plan lowered to 6". The implementer changed it to 7 in tasks.md:1198-1199.
- The reason is sound (scrutiny 6), and C25's "75 link steps" total shifts with it, as does the new `piperine-device-fixtures 1` plan row.
- An approved task text needs the coordinator's or the user's word in a Deferred or Notes line. A silent rewrite of the criterion does not do that.

### P5-7 (Low): tautological doc comments and a stray comment

- 147 doc comments in the fixtures crate read "The shape helper `shape`." or "The module helper `module`." (for example `circuit.rs:49,67`). They satisfy `///` on every item and say nothing, against the manual-style docs rule.
- `crates/piperine-abi/tests/thunks/thunks.rs:321` holds a leftover `// 2.0f64` as the first line of `impl ProbeCalls`.

### P5-8 (Low): `is_fixtures_crate` is unpinned and broader than its doc

- `source_tree.rs:120` treats every crate whose name ends in `-fixtures` as test code. That matches the planned `piperine-ir-fixtures` (T63) and is fine, but no unit test pins it.
- It also removes the crate from the product-only `repeated_type_names`, `long_functions` and `single_implementor_traits` guards.
- Fix: add a unit test in `item_scan.rs`'s scenarios. A test-origin `repeated_type_names` would also have caught P5-2.

## Phase 5 round 2

**Verdict**: FAIL (one Medium finding, one Low note)
**Head**: eacc83455 on `feature/code-clarity-p5` (fix commits e49f059ea, e10dcb63e, 475feb12b, eacc83455 on 472c8c2cb)
**Verifier**: independent sub-agent (Opus; author != verifier). Same environment as round 1 (`/var/tmp/piperine-target-t`), with the guard sources touched first.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` | 121 passed, 0 failed |
| `cargo test -p piperine-abi` | 59 passed |
| `cargo test -p piperine-analog-solver` | 111 passed |
| `cargo test -p piperine-digital-solver` | 161 passed |
| `cargo test -p piperine-device-fixtures` | 2 passed |
| `cargo test -p piperine-solver`, and `--tests -- --test-threads=1` | 337 passed both ways |
| `cargo test -p piperine-parity` | **645 passed** over 64 result lines. Every binary's count is identical to round 1. The fixer's "449" is a counting artefact, not lost tests |
| `cargo test -p piperine --test book_inventory` | 18 passed |
| CC-09: `git diff 97d327d58..HEAD` over `docs/`, `examples/`, `tests/`, `crates/piperine-{parity,transpile,cli,api}`, `*.msl` and goldens | empty |

### Timing of `piperine-solver`

- The run, with everything built, took 1.71 s, against 1.67 s in round 1 and 6.25 s at the base.
- `touch src/lib.rs` followed by `--no-run` took 29.7 s, against 50.8 s in round 1 and 68.2 s at the base.

### Closure of round 1

| Finding | Closed by | Evidence |
| --- | --- | --- |
| P5-1 | 475feb12b | See below; the reconciliation holds. One replaced proof, C61, is not held by its survivor (P5R2-1) |
| P5-2 | e49f059ea, e10dcb63e | No repeated type name in the fixtures crate (`uniq -d` is empty). `repeated_type_names` now covers fixtures, with a unit test. `test_devices.rs` places the fixtures' devices (`AnalysisDevices`), and `analyses.rs` is no longer a second copy |
| P5-3 | e49f059ea | The 36 `test_helper_functions` rows are gone: the allowance falls from 1004 to 968 and none of the four crates has the file. The helpers are associated functions of `Cases` structs. No `src/` unit test changed its assertion signature. T40's first box is ticked, with the coordinator's decision recorded |
| P5-4 | e49f059ea | No `react` in the fixtures crate is the bare `Ok(Flow::Continue)` body |
| P5-5 | 475feb12b | `every_device_of_the_crate_is_in_the_list` fails on a device with no row and on a row with no device (probes below). `digital::PulseGenerator` is listed |
| P5-6 | coordinator amendment | T39 carries "Coordinator amendment (user AFK, pending review)" |
| P5-7 | eacc83455 | 0 "helper `…`" docs remain, and the docs name what the item is. The stray `// 2.0f64` comment at `thunks.rs:321` is gone |
| P5-8 | e10dcb63e | `item_scan.rs` `a_fixtures_crate_is_test_code_and_a_product_crate_is_not` pins `is_fixtures_crate`, including the negative `piperine-fixtures-core` |

### Tests removed, reconciled

- `piperine-solver` lost 35 tests and gained 1 (`every_hand_written_device_lives_in_the_fixtures_crate`): 371 − 35 + 1 = 337.
- The 35 removed tests are:
  - the 27 `its_devices_pass_the_partials_verifier` copies;
  - 4 variants: `its_device_…` ×2, `its_corner_source_…` and `its_once_faulting_device_…`;
  - `the_fixture_devices_pass_the_partials_check` (array_windows);
  - `every_helper_device_passes_at_generic_points`;
  - `every_hand_written_device_is_in_a_partials_check`;
  - `a_check_of_another_module_does_not_count`.
- `repoints/T38.tsv` and `T39.tsv` hold 15 + 20 = **35** `replace` rows, one per removed test. The brief said 36 rows; there are 35, and they match exactly.
- `piperine-device-fixtures` gained `every_device_of_the_crate_is_in_the_list`.

### The partials checks reproduce on the same points

Each held point of the old copies is carried over unchanged as a `Check::Holding` or `Check::Fixtures` row:

| Device | Check | Point or fixture kind |
| --- | --- | --- |
| FlakyC | `Holding` | `(0,3,0.0)` |
| AcFault | `Holding` | `(0,1,0.0)` |
| Collapsible | `Holding` | `(0,0,1e3)` |
| LateOscillator | `Holding` | `(0,2,0)` |
| JumpOscillator | `Holding` | `(0,3,0)` |
| LateSource | `Holding` | `(0,3,0)` |
| OpenAboveDc | `Holding` | `(0,2,0)` |
| StepSource | `Holding` | `(0,5,0)` |
| FaultOnce | `Holding` | `(0,2,0)` |
| NanSource | `Fixtures` | `NonFinite` |
| Oscillator | `Fixtures` | `Impure` |
| AlwaysLimited | `Fixtures` | `AlwaysLimited` |

- Every other old copy called `verify_every_function` at the generic points, and its devices are `Check::Generic` rows.
- The 30 devices of `every_helper_device_passes_at_generic_points` are all `Generic` rows, under their merged names: the mixed copies became `digital::*`, `BitDigitalToAnalog` and `ConductanceTracer`.
- The exemptions are the round-1 set minus `bad_limit`, which now builds a raw module and has no `impl Device`. Each exemption carries a reason.
- `every_device_function_passes_the_partials_verifier` (the `Verdict::Exact` table) is kept.
- The list has 128 rows.

### Merges and the two data changes

- Across every test that kept its name, I compared assertion counts, numbers and strings between ef7a2da28 and HEAD.
  - 29 tests differ, each only by losing a string literal.
  - Every lost literal is a module name in a construction that moved to the fixtures crate, such as `module::<Diode>("Diode")`.
  - No assertion, number or message changed.
- `circuit` ORIGIN changed from `"abi_devices.rs"` to `"abi.md:11"`. No test or product file names the old string. `boundary/plan.rs` asserted `abi.md:11` on the former `examples::Resistor`, and that assertion is unchanged.
- The merged `mixed::Comparator` watches changed from `rise`/`fall` to `up`/`down`. No solver, api or parity test reads a watch name of a fixtures device. The `"rise"` in `api/tests/options.rs:350` belongs to that crate's own device, and `assembly_watch.rs:346` builds its own.
- The digital merges also change the module name, for example `"Timer"` to `"timer"`. They bump the same thread-local counter keyed by the same device name, so `ReactionCounts::of` call sites are unchanged.
- Conclusion: fixture data only, with no user-visible output.

### Probes (throwaway edits to `device-fixtures/tests/partials.rs`, each reverted; `git status` clean)

| Edit | Result |
| --- | --- |
| Delete the `circuit::Resistor` row | `every_device_of_the_crate_is_in_the_list` FAILED: "devices no row of EveryDevice names: [\"circuit::Resistor\"]" |
| Add a row labelled `circuit::Ghost` | FAILED: "rows whose device is gone: [\"circuit::Ghost\"]" |
| Keep the label `circuit::Resistor` but give the row `circuit::Capacitor::module()` | **both tests pass**, so the Resistor is never checked (P5R2-1) |

### P5R2-1 (Medium): C61 "a check of another module does not count" is re-pointed to a test that does not hold it

- `a_check_of_another_module_does_not_count` planted a label naming `Foo` over `Bar`'s module and asserted that `Foo` stays unchecked (HYG-16 AC 3; `solver-hygiene/verification.md:112` judged it on exactly that).
- The `replace` row moves it to `every_device_of_the_crate_is_in_the_list`, and `solver-hygiene/tasks.md:1193-1194` now proves C61 with that test.
- That test compares labels with the scanned `impl Device for` names only. A row's `module` is never tied to its label. The third probe shows a row labelled `circuit::Resistor` that checks the Capacitor's module, which leaves the Resistor unchecked with every test green.
- C60 ("in a partials check of its **own** module") has the same gap.
- Fix: derive each row's label and module from the one type, for example a `ListedDevice::of::<T: Device>(check)` built on `std::any::type_name::<T>()` and `T`'s own `module()`. Alternatively, assert that each row's `module` is the module its label's type builds, and plant the probe-3 case as a test.

### Note (Low, not blocking)

- `BadLimit` (`analog.rs:170`) and `Rogue` (`digital.rs:877`) assemble raw modules by hand, without `impl Device`, so `DeviceScan` cannot see them.
- Both are deliberate rule-breakers, so leaving them unlisted is right. The scan's doc should say that it only reaches `impl Device` devices.

## Phase 5 round 3

**Verdict**: PASS (one Low note, not blocking)
**Head**: 7009c644d on `feature/code-clarity-p5` (fix commit 7009c644d on 8e15d6703)
**Verifier**: independent sub-agent (Opus; author != verifier). Same environment as before, with the guard sources touched first.

### Gates

| Gate | Result |
| --- | --- |
| `**Build**` line | 0 warnings, 0 errors |
| `cargo test -p piperine-device-fixtures` | 4 passed: `its_devices_pass_the_partials_verifier`, `every_device_of_the_crate_is_in_the_list`, `a_check_of_another_module_does_not_count - should panic`, `a_row_takes_its_label_from_its_type` |
| `cargo test -p piperine-solver` | 337 passed |
| `cargo test -p piperine-repository-guards` | 121 passed (`proof_ledger` included) |

### Closure of round 2

| Finding | Closed by | Evidence |
| --- | --- | --- |
| P5R2-1 | 7009c644d | Each row is built as `ListedDevice::of::<T>(module, check)`, with the label taken from `type_name::<T>()`. `of` asserts `ptr::eq(module.prototype_ptr(), T::prototype())`. `a_check_of_another_module_does_not_count` is `#[should_panic(expected = "holds the module of another device")]` and plants round 2's repro (`of::<circuit::Resistor>(circuit::Capacitor::module())`). The C61 proof (`solver-hygiene/tasks.md:1194`) and the T38 `replace` row point at it |

### Judgements

1. **The prototype pointer ties a row to its type, with one harmless exception.**
   - A throwaway test collected `prototype_ptr` over all 128 rows and found 127 distinct pointers.
   - The one shared prototype belongs to `solver_devices::lifecycle::{Counted, Failing}`. Both return `LifecycleShapes::shared()` (`solver_devices/lifecycle.rs:31,51`).
   - `of::<Counted>(Failing::module())` is accepted, which is a false pass. It is harmless: that shape has 0 functions, so `PartialsCheck::run` returns before checking anything for either device, and no function can go unchecked by the swap.
   - A device whose instance shape varies (for example `Collapsible`) is still tied correctly. Its module's prototype is its type's static prototype, and the check runs on the shape that the module's own `create` returns.
2. **The `#[should_panic]` test fails for the right reason.**
   - `expected` names the assertion's message, and only `of`'s assertion carries that text. A panic for another cause would fail the test.
   - As a throwaway, I replaced the assertion with `let _ = own;`. The test then FAILED ("should panic ... FAILED"), so it holds the rule.
3. **C60 and C61 are honestly proved.**
   - C61 ("a check of another module does not count") is the planted swap in the `#[should_panic]` test.
   - C60 ("every hand-written device is in a partials check of its own module") is `every_device_of_the_crate_is_in_the_list`. It builds every row through `of`, so it fails on a wrong module as well as on a missing or stale row.
   - The `sees` matcher of `check_proofs.py` requires the proof's words in order on one line. It accepts the printed `test a_check_of_another_module_does_not_count - should panic ... ok`.

All the probes were reverted, and `git status` is clean.

### Note (Low, not blocking)

- **R3-1.** `Counted` and `Failing` share one prototype, so the type-to-module tie cannot tell them apart.
- No check is lost today.
- Fix: give `Failing` its own static shape, or assert in `every_device_of_the_crate_is_in_the_list` that the rows' prototype pointers are distinct.

## Phase 7 (B6a Owners in `pom/`, and the front end's small defects, T47 to T52)

**Verdict**: PASS (no Medium or High finding; seven Low findings and three notes)
**Head**: ed23b656a on `feature/code-clarity-p7`, base 4e027c56b.
**Verifier**: independent sub-agent (Opus; author != verifier). Sonnet wrote T47–T52. I reran the gates myself, one at a time, with `CARGO_TARGET_DIR=/var/tmp/piperine-target-a`, after touching `crates/piperine-repository-guards/src/*.rs`.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, then filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` | 11 result lines, 117 passed, 0 failed |
| `cargo test -p piperine-lang` | 84 result lines, 1 439 passed, 0 failed |
| `cargo test -p piperine-numbers` (`purity` included) | 10 result lines, 22 passed, 0 failed |
| `cargo test -p piperine-api` (`host_patterns` included) | 28 result lines, 307 passed, 0 failed |
| `cargo test -p piperine-project` | 7 result lines, 53 passed, 0 failed |
| `cargo test -p piperine-cli` (twins included) | 12 result lines, 144 passed, 0 failed |
| `cargo test -p piperine-parity` | 64 result lines, 645 passed, 0 failed |
| `cargo test -p piperine-transpile --test golden` | 7 passed, 0 failed |
| `cargo test -p piperine --test book_inventory` | 18 passed, 0 failed |

### Checks

| Check | Test or probe | Holds |
| --- | --- | --- |
| CC-09 behaviour: `DynamicInteger` (T51) | `diff` of the base `lang/src/elab/consts/integers.rs` against the head's `numbers/src/dynamic.rs` | yes. The only differences are the module `//!` line, three `use piperine_numbers::…` → `use super::…`, and `pub(in crate::elab)` → `pub` on 31 items. No body line changed, so every integer result is the old one. `tests/purity.rs` passes with the new file. `dynamic.rs` is not in `transpile/src/rt.rs`'s `NUMBER_FILES`, so generated code does not move |
| CC-09 behaviour: E0702 text (T52) | A side-by-side read of the base `api/src/logic_pattern.rs::parse` against the head's `LogicPattern::parse_with_width` + `Failure::logic_pattern`, with my own cases (`""` at width 4, `" 10xz "`, `10X1`, `10xz1`) | yes, byte-identical. Both trim the text and check the character before the length. Both accept only lowercase `0 1 x z`, and the level count equals `chars().count()`. The two format strings are the same character for character, including `(§22.1.4)`. `what` was not dropped. It moved from `parse` to `Failure::logic_pattern(fault, text, width, what)`, and both host call sites (`host.rs:88`, `param_text.rs:110`) pass the same `what` as before (`the port \`{port}\``, `` `{label}` ``) |
| CC-09 behaviour: the pin bytes (T52) | Read of the base and head `bytes` | yes. The same plane layout, with `PatternLevel::{Zero,One,Z,X}` in place of `'0' '1' 'z' _`. `CellShape::logic_planes` is the old `planes_of`, verbatim. `host_patterns` passes unedited in its assertions |
| CC-09 behaviour: out-of-range float (T52) | Read of the two removed blocks against `LiteralText::is_out_of_range` (`override_text.rs:36`) | yes. The infinite class and the spelled `inf`/`infinity` (sign-stripped, lowercase) predicate are the same expression the elaborator and the host each wrote |
| CC-09 behaviour, whole front end | lang (unedited assertions), the CLI twins, parity, transpile goldens, `book_inventory` | yes, all green (table above) |
| Owner lines closed by earlier deferrals | `grep` over `crates/`, `docs/`, `tests/` | yes. `DeferReason::InferredGenerics`, `Ty::has_repr`, `GenericParam::default_type` and `BinaryFormat::round_integer` have no definition and no use, and their `docs/pom` rows are gone. The api `LogicPattern` file is deleted. `piperine_api::LogicPattern` is now a re-export of `piperine_lang::types::LogicPattern`. `numbers::Limbs` has a `kept:` verdict (see P7-1) |
| `inventory_types_in_two_crates` empty | `ls allowances/*/inventory_types_in_two_crates.tsv`; `boundaries/inventory_types.rs` | yes. Both files are deleted, and the guard's `OWNERS` (`LogicPattern`→lang, `DynamicInteger`→numbers, `Backend`→api) passes with no allowance |
| `ShapeCheck` → `DeclarationShapeCheck` (T48) | `data/owners.tsv:19`, `resolve/shape.rs:16`, `transpile/src/shape_check.rs:10` | yes, honest. `piperine-transpile` already declares a public `ShapeCheck`, and the inventory guard would have flagged a second one. The renamed row names a real type with a `run` method that owns the pass. No row was dropped, because `NameResolution`, `MemberResolution` and `LeafResolution` still sit next to it. The four `owners` allowance rows they replace are deleted |
| Impls away from their type: `Lowerer`, `Decl` | `lower/mod.rs` + `lower/lowerer.rs`; `reflect/mod.rs` + `reflect/handle.rs`, `module.rs`, `nature.rs`, `behavior.rs` | yes, it meets the owner intent. Each struct is declared once, in the `mod.rs` of the family whose files extend it, and every impl sits in a child file of that module. A struct in `lowerer.rs` or `handle.rs` would put `body.rs`/`items.rs`'s `impl Lowerer` and `module.rs`'s `Decl::as_module` beside it rather than below it, which is what the guard measures. Each `mod.rs` `//!` names the file that holds the core impl. Both `mod.rs` files are under the 60-line ceiling |
| SPEC_DEVIATION comments (T49) | `msl-pom/design.md` diff; `msl-elab/spec.md:151` | partly; see P7-6. The two `pom` ones were design-text divergences (`QuantityRole` because `Quantity` is taken in `pom`, and `ModLayout`'s `is_root`). Amending the feature's design text is the closure that T31's approved text and design.md:612 name, and these are designs that could not be built as written, not a deviation from the spec. They are fixed. The third, in `tests/elab_unlowered.rs`, is a spec-level divergence (ELAB-06 AC 3 says `P0001`, the code gives `E0320`). Its comment now cites amendment (d), which predates this phase. That relabels it; it does not fix it |
| `suite_hygiene` registry → 0 | `suite_hygiene.rs` diff | stricter, not weaker. The registry now returns `None` for every path and the count is 0, so any `mod.rs` over 60 lines fails. The removed "the walk reaches the three registered files" assertion only existed to prove the exemptions were visible; the `> 10` floor still guards blindness. Leftovers are in P7-3 |
| Test edits are call paths only | `git diff 4e027c56b..HEAD -- '*/tests/*'` | yes. `is_crate_name` → `CrateGraph::is_crate_name` (3 sites and the `use`); `LogicPattern::parse(t, w, what)` → `parse_with_width(t, w)` plus `Failure::logic_pattern` in the error arm; one comment in `elab_unlowered.rs`; the hygiene registry. No assertion, literal or expected value changed. `BodyWalk::new(design)` is `pub(crate)`, so tests do not call it |
| Allowances only delete or move | Row multiset diff by item name of `allowances/` and `worklists/` | yes. `free_functions` −86, `owners` −6, `undocumented_items` −26 net (every `+` row is a moved item with a deleted twin), `undeclared_words` −11 net (the four `decl` rows sum 14 → 14, `imports.rs` 1 → `context.rs` 1, and `from_i128`/`to_i128` move lang → numbers), `repeated_type_names` −1 lang/−1 api, `crate_docs` −1, `impls_away` 1 renamed, `long_functions` 2 renamed (see P7-7) |
| `test_only_items` reflection rows "kept" | `grep` of each of the 15 methods in `docs/pom/reflection.md` | yes. Each one has its own method row (for example `is_binding` at :900, `bundle_view` at :782, `is_intrinsic` at :460, `struct_named` at :70, `is_pub` at :251). They are documented public reflection API with no in-tree consumer yet, which is the honest verdict for a library surface |
| No free fn, no macro, no `#[allow]`, no `#[expect]` in new code | `grep` of the `+` lines under `crates/*/src` | yes, none. The 5 `unreachable!` are moved from `reflect/hardware.rs` (5 removed, 5 added) and sit on handle-kind invariants, not on user input |
| No `unwrap` on user-input paths | same grep | yes. `dynamic.rs` uses `unwrap_or_default` on `minus` results that are non-negative by construction, as before |
| Full words, `///` on new items | read of `BodyWalk`, `NameResolution`, `MemberResolution`, `LeafResolution`, `DeclarationShapeCheck`, `Classifier`, `PatternFault`, `Failure::logic_pattern`, `CellShape::logic_planes`, `LiteralText::is_out_of_range`, `CrateGraph::is_crate_name` | yes. Every new item is documented and spelled in full (`expression_children`, `statement_own_expressions`, `descendants_depth_first`). The undocumented `Decl::{new,id,design,kind}` and `Lowerer` fields are moved code that keeps its rows |
| Every public path of `piperine_lang::pom` unchanged (T50) | `pub use` lines of base `reflect/mod.rs` against head | yes for T50: the same names are re-exported from `pom`. T49 removed `pom::is_crate_name` (it is now `CrateGraph::is_crate_name`); that is the intended free-fn removal, and every caller was edited |
| Commits, one per task, `tasks.md` in each | `git show --stat` | yes |

### P7-1 (Low): `Limbs` is kept, but its reason is not written on the trait

T51's Done-when reads "the trait is removed, or kept with its reason written on it (T32)". The verdict exists only in `worklists/single_implementor_traits.tsv:2`. The trait's doc at `crates/piperine-numbers/src/limbs.rs:9` still says only "Arithmetic on a magnitude held as little-endian `u64` limbs." Compare `In` (`msl/parser/base.rs:97-101`) and `NetRead` (`solver/src/protocol/step.rs:18-19`), which carry their reason on the item. Repro: `grep -n -B3 'pub trait Limbs' crates/piperine-numbers/src/limbs.rs`. The row also says "the file is embedded verbatim" as if that blocked the edit. It does not: no transpile golden holds the text of `limbs.rs` (`grep -rl 'Arithmetic on a magnitude' crates/piperine-transpile/tests` is empty), so one `///` line on the trait closes this.

### P7-2 (Low): `docs/pom/model.md` still documents the free `is_crate_name`

T49 removed `piperine_lang::pom::is_crate_name` and made it `CrateGraph::is_crate_name`. `docs/pom/model.md:266` (`InvalidName` unless `is_crate_name(name)`) and `:270` (`` `is_crate_name(name: &str) -> bool` is true for … ``) still describe a free function. It should be a row of the `CrateGraph` method table above it. Repro: `grep -n is_crate_name docs/pom/model.md`.

### P7-3 (Low): `suite_hygiene.rs` keeps dead scaffolding after the registry emptied

These are in `crates/piperine-repository-guards/tests/suite/suite_hygiene.rs`:

- At :91-92 the comment is two half-sentences spliced together ("so the floor is 10; the walk / no `mod.rs` is registered as oversized any more").
- The `registered_oversized_mod_rs` doc at :102-111 still says "Two remain …", although the registry is now empty.
- The registry itself is `fn registered_oversized_mod_rs(_path: &str) -> Option<&'static str> { None }`, a function whose parameter is ignored.

The guard is stricter, as it should be. Only the text and the dead registry are wrong. Either keep the registry as a documented empty table or delete it and the two calls in `mod_rs_declares_only` (:692, :698), together with the `exempted` count.

### P7-4 (Low): "no existing test edited" boxes and commit bodies are untrue as worded (P6-4 again)

- T48 and T50 tick "`cargo test -p piperine-lang` passes with no existing test edited", and their commit bodies say "No test edited", yet each edits `tests/suite/suite_hygiene.rs`.
- T49's box says the same, while its commit edits `lang/tests/pom_crates.rs` and `lang/tests/elab_unlowered.rs`. Its body does say so.

Every edit is a call path, a comment or the exemption registry the task required deleting, so the substance holds. The wording does not.

### P7-5 (Low): the deferral boxes of T30 and T32 stay unticked although Phase 7 closed them

`tasks.md:992` (T30: "The other 4 (`round_integer`, `GenericParam::default_type`, `Ty::has_repr`, `DeferReason::InferredGenerics`) are removed by T51, T50, T49 and T48") and `tasks.md:1038` (T32: "no open row: `lang::In` (T43) and `numbers::Limbs` (T51)") are still `[ ]`. Their closing tasks have landed (T32's row waits on P7-1). Repro: `sed -n '992p;1038p' .specs/features/code-clarity/tasks.md`.

### P7-6 (Low, for the user): ELAB-06 AC 3 is relabeled, not fixed

`crates/piperine-lang/tests/elab_unlowered.rs:82` replaced `SPEC_DEVIATION (ELAB-06 AC 3)` with "ELAB-06 AC 3 is met earlier than written (msl-elab spec, amendment (d))".

- The criterion (`.specs/features/msl-elab/spec.md:169`) still says a placement that sets `m`/`dtemp` is `P0001`, while the code returns `E0320` from the front end.
- Amendment (d) (`:151`) was accepted by an orchestrator "under the user's standing delegation", not in the user's words.
- Under the project rule (code follows spec; a divergence is a defect, never an accepted documented state), the comment change only moved the record.

This comment was not one of M5's nine, the amendment predates this phase, and the model is still refused with no `Ir`. It is not a regression of this phase. The user should decide whether to fix the code (`P0001` at the placement) or rewrite AC 3 itself.

### P7-7 (Low): two renamed `long_functions` rows with no verdict (P6-3 again)

`worklists/long_functions.tsv` renamed `pom/index.rs::::build` → `IndexBuilder::build` and `pom/resolve/mod.rs::::run` → `NameResolution::run`, both still `-`. Both are appended at the end of the file, out of path order (:132-133). design.md:412 asks each task to give its files' rows a verdict.

### Notes (not findings)

- **The `piperine-api` public surface moved in T52.** `pub mod logic_pattern` is gone. `piperine_api::LogicPattern` is the lang type, so `parse` now takes one argument and returns `Result<_, char>`, and `planes_of` became `CellShape::logic_planes`. A host that called the old `parse(text, width, what)` fails to compile; it does not misbehave silently. Nothing in `docs/`, the Book or Python names it. It is a consequence of "declared once", and the commit body states it.
- `pom/resolve/classify.rs:189`: the doc of `Classifier::run` is indented 8 spaces instead of 4. This is cosmetic.
- `Classifier` holds `design` only. T47's "holds the `Ctx` they thread" reads loosely: the base functions threaded `design: &Design`, not a `Ctx`, so this is correct.

### Phase 7 findings

| Finding | Severity | Status |
| --- | --- | --- |
| P7-1 `Limbs` reason not on the trait | Low | open |
| P7-2 `model.md` documents a free `is_crate_name` | Low | open |
| P7-3 `suite_hygiene` dead registry and stale comments | Low | open |
| P7-4 "no existing test edited" untrue as worded (T48–T50) | Low | open |
| P7-5 T30/T32 deferral boxes unticked | Low | open |
| P7-6 ELAB-06 AC 3 relabeled, not fixed | Low | open (user decision) |
| P7-7 two `long_functions` rows renamed with no verdict | Low | open (debt to the closing task) |

## Phase 10 (B6b `Checker` split into per-concern views, T69 to T76)

**Verdict**: PASS (no Medium or High finding; seven Low findings and four notes)
**Head**: fb5b86c57 on `feature/code-clarity-p10`, base c44a85b5a (`git merge-base feature/code-clarity-p10 feature/code-clarity`).
**Verifier**: independent sub-agent (Opus; author != verifier). Sonnet wrote T70–T76, Opus wrote T69. I reran the gates myself, one at a time, with `CARGO_TARGET_DIR=/var/tmp/piperine-target-a`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, after touching `crates/piperine-repository-guards/src/*.rs`.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` (`**Run ownership**` included: `an_inherent_impl_lives_with_its_type`, `every_owner_of_the_design_exists_and_has_its_methods` ok) | 11 result lines, 121 passed, 0 failed |
| `cargo test -p piperine-lang` | 84 result lines, 1 439 passed, 0 failed (the same 84/1 439 as Phase 7's head) |
| `cargo test -p piperine-transpile` (goldens included) | 31 result lines, 172 passed, 0 failed |
| `cargo test -p piperine-cli` (twins included) | 12 result lines, 144 passed, 0 failed |
| `cargo test -p piperine-parity` | 64 result lines, 645 passed, 0 failed |
| `cargo test -p piperine-api` | 28 result lines, 307 passed, 0 failed |
| `cargo test -p piperine --test book_inventory --test integrated` | 18 + 42 passed, 0 failed |
| `cargo test -p piperine --test book_claims` | 6 passed, 1 failed: `every_claim_row_is_a_sentence_of_its_chapter`, its only offence `structure~58f974e3` (N3) |

### Checks

| Check | Test or probe | Holds |
| --- | --- | --- |
| No test lost, weakened or edited | `git diff --name-status c44a85b5a HEAD` outside `crates/piperine-lang/src/types/`, `allowances/`, `worklists/`, `.specs/`; `#[test]` names under `types/` base and head | yes. The diff touches nothing outside those four places: no `tests/` file, no golden, no `.msl`, no doc. `types/` holds no in-source test at base or head. So every crate's test-name set is unchanged, and lang's counts match the Phase 7 head exactly |
| T69: `Checker` has two fields; each table documents what it holds | `check.rs:61-67`, `CheckState` at `:71-91`, each `*/tables.rs` | yes. `Checker { state, worklist }`. `CheckState` holds `design`, `diagnostics`, `values`, `modules`, `analog`, `carriers`, `generics`, `traits`, `result`; every field and every table struct has a `///`. There is no `PatternTables` (see P10-3) |
| T70–T75: no `impl Checker` in `value/`, `pattern/`, `generics/`, `traits/`, `module/`, `analog/`, `carrier/` | `grep -rn '^impl.*Checker' crates/piperine-lang/src/types` | yes. The only one is `check.rs:132`. Each directory's `mod.rs` declares its view (`ExpressionTyping`, `PatternTyping`, `GenericsSolver`, `TraitTyping`, `ModuleTyping`, `AnalogTyping`, `CarrierTyping`), and every `impl` of it is below that `mod.rs` |
| Table fields private to their directory | `*/tables.rs` | yes, by the compiler. Every table field is `pub(super)` from `<dir>/tables.rs`, so visible in that directory only; another concern asks through a method (`is_open_let`, `begin_placement`, `replace_generic_item`, `save`/`restore` …) |
| T76: no `Checker` row in `an_inherent_impl_lives_with_its_type` | `allowances/piperine-lang/impls_away_from_their_type.tsv`; the guard | yes. All 55 `types/*::Checker` rows are deleted, the only `types/` row left is `traits/repr.rs::Types` (moved from `repr.rs`), and the guard passes |
| T76: 8 root files and the directories of design.md's map | `ls crates/piperine-lang/src/types` | yes: `attributes`, `callee`, `catalogue`, `check`, `mod`, `result_table`, `ty`, `whole_model`, and seven directories. design.md:346-353 lists seven directories (`value`, `pattern`, `module`, `analog`, `carrier`, `generics`, `traits`), so the box's "6" was a slip in the box and "the real count" is right; how the box was changed is P10-1 |
| CC-09 behaviour: `check_ahead` with `SavedValues`/`SavedModules` | base `check.rs` `check_ahead` against head `check.rs:250-263`, `value/tables.rs:211-230`, `module/tables.rs:87-95`, `carrier/tables.rs:56-63` | yes. The base saved `owner, context, event_slot, in_guard, fn_where` by copy and took `book, lets_open, placing, flow`, then set `context = Module`, `event_slot = None`, `in_guard = false`, `fn_where = false`. The head sets aside exactly those ten values (the four takes in `save`), resets the same four, leaves `owner` as the base did, and restores all ten. The order of takes is irrelevant (disjoint fields) |
| CC-09 behaviour: E0904 now raised in `Checker::run` | base `attributes.rs::check_attributes`/`check_attribute` against head `AttributeCatalogue::misplaced`/`fault_of` and `check.rs:180-182` | yes, same text, code, label, span, file and order. Both walk declarations then statements in index order and call the same `coded(file, "E0904", span, message, "an attribute A.1 does not allow here")` before `reach(top)`; the message `match` is the same arm for arm. `run_model` did not check attributes before and does not now. `coded` is the base `module.rs` body verbatim, and `report`'s span dedupe is unchanged |
| CC-09 behaviour: the pass order | base and head `run`, `drain`, `declaration`, `reach`, `within` | yes. `declaration` sets `owner` and resets `flow` (`ValueTables::begin_declaration`) as before; `run` calls attributes, reach, drain, unread template locals, node choices, then continuity in the same order. `Types::empty` starts every result field at the value the base `Checker::new` gave it, and the head fills `result` in place instead of moving the fields at the end |
| CC-09 behaviour: the four split functions | base `expr.rs::path`, `analog.rs::{probe, core_call}`, `check.rs::lower_type_node` against head `value/expr.rs::{path, local_read}`, `analog/analog.rs::{probe, probe_access, core_call, judge_core_place}`, `check.rs::{lower_type_node, lower_builtin_type}` | yes. Each extracted method is the old arm or block verbatim (field reads re-pointed to the tables, calls re-pointed to the views), called from the same place, so no call is reordered. See note N1 |
| Allowances and worklists: no growth, deletions closed, moves net-zero | row diff of every `allowances/piperine-lang/*.tsv` and `worklists/*.tsv` | yes. `impls_away` −55 +1 (moved `repr.rs::Types`); `owners` −7 (`CheckState`, the six views; each is now a declared type with methods, which `every_owner_of_the_design_exists_and_has_its_methods` checks); `undocumented_items` −11 +7, every `+` a moved twin, the 4 net deletions (`Checker::new`, `binary`, `infer_kind`, `literal`) now carry a `///`; `free_functions` 3 moved; `repeated_type_names` 11 moved; `undeclared_words` per identifier: no identifier grows, `exprs`, `impls`, `drive_impls`, `carrier_drive_impls` each −1 (806 → 802); `test_only_items` 2 moved; `long_functions` −13 +9 |
| `owners.tsv` rows of `ModuleTyping`, `AnalogTyping`, `CarrierTyping` | `data/owners.tsv:25-27`; `module/mod.rs`, `analog/mod.rs`, `carrier/mod.rs` | yes. Each is declared once in its `mod.rs`, with `new` and the moved methods (144, 26, 54 per the commit bodies), so the guard's "declared and has a method" holds. The rows' task column is off by one (P10-7) |
| `long_functions` verdicts under `types/` | `worklists/long_functions.tsv:85-93`; lengths measured at the head | partly; see P10-4. Four rows are closed by a real split (`Checker::new` by T69's partition, `path`, `core_call`, `lower_type_node`). Nine are `kept:`. `binary` (74 lines, one arm per operator rule) and `infer_kind` (84, an exhaustive dispatch) meet design.md:412. `let_statement` (74), `placement` (77), `connections` (62), `probe` (63) are close to the line and their reasons describe the code. `method_call` (123) and `check_as` (131, with its `closure` row) do not |
| Implementer's decisions | tasks.md T70, T72, T76 annotations; commit bodies | see the judgements below |
| No free fn, macro, `#[allow]`, `#[expect]`, `unwrap` added | `grep` of the `+` lines of `git diff c44a85b5a HEAD -- crates/` | yes, none (the one `unwrap()` hit is the doc of a moved method) |
| Full words, `///` on new items | read of `CheckState`, `DeclarationWorklist`, `Diagnostics`, the seven views, the five `*Tables`, `SavedValues`, `SavedModules`, `MisplacedAttribute`, the accessors and the table methods | yes. All new items are documented (the `undocumented_items` guard passes with no new row) and spelled in full |
| Visibility | `pub(in crate::types)` and `pub(crate)` on the `+` lines | no widening. A file at `types/` root wrote `pub(super)`, which is `crate::types`; a moved item another concern names is `pub(in crate::types)`, the same reach, and one used only in its directory stays `pub(super)`, which is narrower. The only `pub(crate)` is the existing `ElementCoverage` re-export. The `pub` re-exports of `types/mod.rs` name the same items by their new paths |
| Commits, one per task, subjects as approved | `git log c44a85b5a..HEAD` | yes, eight commits with tasks.md's subjects. T72's is 74 characters and names "the trait table" while the commit gives the methods to the view `TraitTyping` (P10-6) |

### Judgements on the eight decisions

1. **Views borrow `&mut Checker` and deref to it** (T70 annotation). Agreed. `reach` and `check_ahead` re-enter `declaration()`, so a view over `&mut CheckState` could not reach the worklist without a rewrite of the control flow, which design.md:328 rejects. The cost is that the boundary is enforced for fields (the compiler keeps each table private to its directory) but only by convention for methods: through `Deref` a view can call any `Checker` method, including another view's accessor. The two-phase borrow workaround writes `self.checker.` in 87 places next to `self.` for the same thing; it is explained in the T70 body and is cosmetic.
2. **One `tables.rs` per directory, no `PatternTables`** (T69 body). Agreed: no field belongs to patterns alone. The departure is not written under T69's box (P10-3).
3. **`TraitTyping` instead of methods of `TraitTable`** (T72 annotation). Agreed. `TraitTable` does not exist; the data type is `ImplTable`, shared as `Rc` and built once. The moved methods report diagnostics and reach declarations, which a data type cannot. `ImplTable` keeps the methods that read only itself, and `traits.rs` → `traits/impl_table.rs` names the file for its type.
4. **`coded` moved to `check.rs`** (T73 body). Agreed: it is `report` with a code, called from every concern; leaving it in `module/` would make every other view call it through `modules()`. Its body is unchanged.
5. **`check_attributes` → `AttributeCatalogue::misplaced`** (T76). Agreed and verified behaviour-neutral (Checks). It also turns the walk into a function of the design alone, which is what the catalogue is.
6. **Seven directories, not six** (T76 box). The count is right: design.md's map has seven rows under `types/`. The way the box was changed is not (P10-1).
7. **`check.rs` left at 793 lines** (T76 annotation). Design is violated: design.md:346 says the root holds "`Checker` (the declaration worklist and the pass order, nothing else)", and T76's What says `check.rs` "holds the declaration worklist and the pass order". At the head it also holds reporting (13 methods, `file_of_expr` included), type lowering (`lower_type`, `lower_type_node`, `lower_builtin_type`, `writes_no_arguments_to_a_plain_type`, `sized_type`, `integer_type`, `is_std`), signatures (`signature`, `declared_signature`, `signature_parts{,_of}`), function and constant checking (`function{,_of,_body}`, `constant`), and the module pass (`module`, `module_of`, `module_item`, `inside_generation`). Base `check.rs` was 883 lines, so the file shrank by the 68-field struct only. No Done-when box requires more, the departure is declared, and T76's guard criterion (C7: no `impl Checker` outside its file) holds, so it is not a failed box. It is P10-2, with a proposed owner.
8. **Visibility widening**. There is none (Checks): `pub(in crate::types)` restates the reach `pub(super)` had at the root.

### P10-1 (Low): approved Done-when text rewritten in place (P5-6 again)

- T76's second box read "`types/` holds 8 files at its root and 6 directories, as design.md's map". At the head it reads "… and 7 directories, as design.md's map (it lists seven: …; this box said six)". The number in the approved text was changed in the box itself.
- T70–T76 append parentheticals inside the boxes ("(no test edited; every moved item was `pub(super)`)", "(the last one, `attributes.rs`, went when …)", "(and `cargo test -p piperine-cli --test twins`, …)").

The substance is right (the count is seven, and the parentheticals are true). The rule is that approved text stays and a departure is an annotation under the box. Repro: `git diff c44a85b5a HEAD -- .specs/features/code-clarity/tasks.md`. Fix: restore the approved box texts and move each parenthetical to an indented `Note:` line under its box; for T76, keep "6 directories" struck or quoted and say under it that design.md:346-353 lists seven.

### P10-2 (Low, for the user): `check.rs` holds more than the worklist and the pass order

See judgement 7. A follow-up that finishes T76's What without behaviour risk, one concern per commit as T70–T75 did:

- **Reporting** → methods of `Diagnostics` (already declared in `check.rs:106-112`, holding `list` and `reported_spans`): `report`, `report_distinct`, `coded`, `mark`/`annotate_since`, `reported`. `refuse*` and `mismatch*` need `design` for names and can take it as an argument, or stay as one-line `Checker` forwarders.
- **Type lowering** (`lower_type*`, `lower_builtin_type`, `sized_type`, `integer_type`, `writes_no_arguments_to_a_plain_type`) → `ExpressionTyping` in `value/`, which already owns `tuple_type`, `array_type`, `option_type`, `enum_type` and `struct_type`, the methods `lower_type_node`'s arms call.
- **Functions and constants** (`function*`, `constant`, `signature*`) → `ExpressionTyping` (or `GenericsSolver` for the generic signature part).
- **The module pass** (`module`, `module_of`, `module_item`, `inside_generation`) → `ModuleTyping`.

`check.rs` then keeps `run`, `run_model`, `drain`, `reach`, `check_ahead`, `declaration`, `within` and the seven accessors. Owner: a new Phase 10 task (T76a) or the first task of P14, which design.md:115 already gives lane F over `types/{value,generics,…}`; either way it amends T76 or adds a task, so it needs the user's approval.

### P10-3 (Low): T69's `PatternTables` departure is recorded only in the commit body

T69's What names `PatternTables`; none exists. The reason ("no field belongs to patterns alone", c0a1b5786) is sound, but tasks.md carries no annotation, while T70, T72 and T76 do. Fix: one `Note:` line under T69.

### P10-4 (Low): two `kept:` verdicts are not the design's kind of reason

design.md:412 gives "an exhaustive `match` with one-line arms" as the example reason. Two rows do not fit:

- `value/call.rs::ExpressionTyping::method_call` (123 lines). The verdict says "each rule is a guard and a hand-off", but it holds a 30-line `match res` that computes the built-in and a 30-line miss arm that builds `E0161` with its help text. Both are self-contained blocks that a method each would take with no call reordered.
- `value/expr.rs::ExpressionTyping::check_as` (131 lines) and its `closure` row. The verdict says the order of the guards is the order of the diagnostics. That holds, but a split keeps it: the `If` arm, the integer `Neg` arm and the trailing mismatch block are multi-line and independent.

"The order is the order of the diagnostics" is a reason not to reorder, not a reason not to split, since this phase's own four splits kept the order. Fix: mark the two rows `split` and owe them to the P10-2 follow-up, or split them now.

### P10-5 (Low): stale names in doc comments

- `check.rs:204`: "the whole-model check (`model.rs`)". The file is now `whole_model.rs`.
- `value/aggregate.rs:6`: "kept in `Checker::layouts`". `layouts` is a field of `Types` (`CheckState::result`) now.

Repro: `grep -rn '\`model.rs\`\|Checker::layouts' crates/piperine-lang/src`.

### P10-6 (Low): T72's subject is 74 characters and names the wrong owner

`35662380c refactor(types): the generics solver and the trait table own their methods` is 74 characters, over the usual 72. It is the subject tasks.md approved, so the text is faithful to the plan, but after decision 3 the methods belong to the view `TraitTyping` and `ImplTable` is unchanged. The body says so. No action on history; note it for whoever writes the merge summary.

### P10-7 (Low): `data/owners.tsv` task column is off by one for four rows

`owners.tsv:25-28` assigns `ModuleTyping` to T72, `AnalogTyping` to T73, `CarrierTyping` to T74 and `GenericsSolver` to T75. They landed in T73, T74, T75 and T72 respectively. This is inherited from the base (the phase did not edit the file), and the guard reads only the crate and type columns, so it is cosmetic. Fix: correct the four task cells.

### Notes (not findings)

- **N1: the splits go beyond design.md:356.** design.md:356 says a commit "moves the files with `git mv`, changes the `impl` headers and the receivers, and nothing else". T70, T74 and T76 also split four long functions to close their `long_functions` rows. Each split is verbatim and reorders nothing (Checks), and each commit body declares it. It is a small, declared, behaviour-neutral departure.
- **N2: merge with the integration branch.** `git merge-tree feature/code-clarity HEAD` reports one content conflict, in `worklists/long_functions.tsv`: Phase 9 edited rows next to the `types/` rows. `verification.md` will also conflict, since both branches append a section. Both are textual; keep both sides.
- **N3: `book_claims`.** Red at this head for one reason only: the panic lists one offence, `structure~58f974e3: the chapter no longer has this sentence`. The row is in `tests/book/inventory.tsv:675` at the base, which this phase does not touch, and the integration branch removed it in cf16edbaa (P9-1). It goes green on merge; it is not a Phase 10 regression.
- **N4:** The views' `Deref` lets a view reach any `Checker` method, so "a call into another concern goes through `Checker`'s accessor" is a convention, not a check. If the follow-up in P10-2 empties `check.rs`, the views will need only the worklist, the reporting and the accessors from it.

### Phase 10 findings

| Finding | Severity | Status |
| --- | --- | --- |
| P10-1 approved box text rewritten in place (T70–T76) | Low | open |
| P10-2 `check.rs` holds more than the worklist and pass order | Low | open (user decision; proposed T76a or P14) |
| P10-3 `PatternTables` departure not annotated under T69 | Low | open |
| P10-4 `method_call`, `check_as` `kept:` verdicts | Low | open (with P10-2) |
| P10-5 stale `model.rs` / `Checker::layouts` docs | Low | open |
| P10-6 T72 subject 74 characters, names `TraitTable` | Low | note |
| P10-7 `owners.tsv` task column off by one | Low | open |

## Phase 10 — re-verification

**Verdict**: PASS (round 1 closes P10-1 to P10-5 and P10-7; two new Low findings, cosmetic; no Medium or High)
**Head**: ac2508f75 on `feature/code-clarity-p10` (fix round 1), checked against 10617e6cc (the verified head plus the Phase 10 section).
**Verifier**: the same independent sub-agent (Opus; author != verifier). I reran the gates myself, in the foreground, with `CARGO_TARGET_DIR=/var/tmp/piperine-target-a`, after touching `crates/piperine-repository-guards/src/*.rs`.

### Gates rerun at the head

| Gate | Result |
| --- | --- |
| `**Build**` line: `cargo build --workspace --all-targets`, filtered for `^(warning|error)` | 0 warnings, 0 errors |
| `cargo test -p piperine-repository-guards` | 11 result lines, 121 passed, 0 failed |
| `cargo test -p piperine-lang` | 84 result lines, 1 439 passed, 0 failed (unchanged) |
| `cargo test -p piperine-transpile` (goldens included) | 31 result lines, 172 passed, 0 failed |
| `cargo test -p piperine-cli` (twins included) | 12 result lines, 144 passed, 0 failed |
| `cargo test -p piperine-parity` | 64 result lines, 645 passed, 0 failed |
| `cargo test -p piperine-api` | 28 result lines, 307 passed, 0 failed |
| `cargo test -p piperine --test book_inventory --test integrated` | 18 + 42 passed, 0 failed |
| `cargo test -p piperine --test book_claims` | 6 passed, 1 failed: the same single offence, `structure~58f974e3` (N3) |

### Closure of round 1's findings

| Finding | Closure | Holds |
| --- | --- | --- |
| P10-1 approved box text rewritten | tasks.md T70–T76 | yes. Every box carries its approved text again: T76's reads "8 files at its root and 6 directories, as design.md's map". Each departure is an indented `Note:` under its box, and T76's note says the "6" was a slip and the tree has seven |
| P10-2 `check.rs` beyond the worklist | `check.rs` 793 → 363 lines; `value/lowering.rs`, `value/functions.rs`, `module/body.rs`; `Diagnostics` methods; design.md annotation after :356; T76 note | yes, with a residual I accept (judgement below). I extracted every moved method from 10617e6cc's `check.rs` and from its new file, stripped the receiver re-pointing (`self.<view>().`, `self.checker.`, `pub(..)`), and diffed the bodies. 14 of 18 are identical. The other four differ only by binding a value to a `let` before a call, for the borrow (`signature_parts`, `function`, `module`: `let generic_item = …; replace_generic_item(generic_item)`), or by `super::Obligation` → `Obligation` (`function_of`). `diagnostics_mark` and `reported` (both `list.len()`) became `Diagnostics::count`. The `Diagnostics` bodies of `report`, `report_distinct`, `coded`, `refuse`, `refuse_with` and `annotate_since` are the old `Checker` bodies over `self.list`/`self.reported_spans`. `mismatch*` are unchanged |
| P10-3 `PatternTables` | tasks.md T69 | yes. A `Note (P10-3)` under the first box |
| P10-4 `method_call`, `check_as` | `value/call.rs`, `value/expr.rs`; `long_functions.tsv` | yes. `method_call` → `method_builtin` (the `match res`, returning `ControlFlow`), `method_on_value` and `method_not_found` (the miss arm). `check_as` → `expected_flow`, `if_as`, `negated_integer_as` and `report_found_mismatch`. Each block is moved verbatim and called where it stood. The three old rows are gone. The new row `expected_flow` (77 lines) is `kept:` as an exhaustive match whose multi-line arms are now methods; that is design.md:412's reason, and it is honest |
| P10-4: the one non-verbatim `return` | `value/expr.rs`, the two `Lit` arms of `expected_flow` | yes, unreachable as claimed. `let LitKind::Int(text) = &lit.kind else { return };` (which left `check_as` early) became `… else { return None };` (which would continue to `report_found_mismatch(None)`, a no-op, and record `expected`). Each arm's guard is `matches!(&lit.kind, LitKind::Int(..))` or `LitKind::Float { .. }`, so the `else` never runs. The same holds for the new `let ExprKind::MethodCall { .. } = &node.kind else { return None }` in the three `method_*` helpers: every caller passes the `node` that `method_call` already destructured as a `MethodCall` |
| P10-5 stale docs | `check.rs:185`, `value/aggregate.rs:6` | yes: `whole_model.rs`, `Types::layouts`. `grep` finds no stale `Checker::` path for a moved method in `crates/piperine-lang/src` |
| P10-6 subject | — | note only, as recorded |
| P10-7 `owners.tsv` task column | `data/owners.tsv:25-28` | yes: T73, T74, T75, T72 |

### Judgement on what `check.rs` keeps (P10-2, design.md:346)

`check.rs` now holds the worklist (`reach`, `drain`, `check_ahead`, `declaration`, `refuse_declaration`), the pass order (`run`, `run_model`, `sorted_diagnostics`, `within`), the seven view accessors, and these as well:

- six one-line forwarders: `report`, `coded`, `refuse`, `refuse_with` to `Diagnostics`, and `file_of_expr`;
- the three `mismatch*`;
- `is_std` and `project_defines`.

**Acceptable.** The forwarders are the entry point every view reaches through `Deref`. Inlining `self.state.diagnostics.x(..)` at about 200 sites fails wherever an argument borrows `self` (`self.file_of_expr(e)`, `self.state.design`), because two-phase borrows do not pass through `DerefMut`. Hoisting each argument into a `let` would grow every site and buy nothing. The new design.md annotation names exactly this residue, so the design text and the code agree again. That is the closure T31 and design.md:612 prescribe for a design that could not be built as written.

**What still carries logic, and its precise owner, if the user wants `check.rs` to be forwarders only:**

- `mismatch_coded` holds a rule, not a forwarder: it first asks `ModuleTyping::untyped_template_read` and only then reports. Its owner is `ExpressionTyping`. `mismatch` and `mismatch_text` would follow it, and `Checker` would keep a one-line forwarder.
- `project_defines` is a query of the impl table. Its owner is `TraitTyping`, or `ImplTable::defines_method` called with `is_std` at the two call sites.
- `is_std` is a query of a declaration. Its owner is `Decl`, which already has `krate()`.

This is about 25 lines and has no behaviour risk. I record it as a note (N5), not a finding.

### New observations at the round-1 head

### R1-1 (Low): 43 redundant `self.expressions()` hops inside `ExpressionTyping`

The round moved `lower_type`, `signature`, `function` and the other moved methods onto `ExpressionTyping`, and re-pointed every call to them as `self.expressions().x(..)`, including the 43 calls inside `ExpressionTyping`'s own `impl` (in `value/lowering.rs` 11, `functions.rs` 9, `expr.rs` 6, `operators.rs` 4, `call.rs` 3, `associated.rs` 3, `aggregate.rs` 3, `methods.rs` 2, `closures.rs` 1, `compound.rs` 1). There were 0 at 10617e6cc. Each hop derefs to `Checker` and builds a second view of the same checker to call a method the first already has. It is behaviour-neutral but reads as if the call left the concern. Probe: `sed -i 's/self\.expressions()\./self./g' crates/piperine-lang/src/types/value/*.rs` and `cargo check -p piperine-lang` compile clean in a scratch target. The edit was reverted, and `git status` is clean. Fix: apply that substitution.

### R1-2 (Low): `report_found_mismatch` is not formatted

The body of `value/expr.rs:179-191` is indented 4 spaces, at the level of the `fn`, and its closing braces sit at 4 as well. Fix: `rustfmt` the file, or indent by hand.

### Notes

- **N5**: see the judgement above. `mismatch*`, `project_defines` and `is_std` have named owners, should the user want `check.rs` reduced to forwarders.
- `method_not_found` ends in an explicit `return None;` (`value/call.rs:448`), the old arm's last line kept verbatim. As a function tail it is `clippy::needless_return`; clippy is not a gate here.
- No test, golden, doc, `.msl` or file outside `types/`, `repository-guards` data and `.specs/` changed (`git diff --name-only 10617e6cc HEAD`). Allowances shrink or move: `undocumented_items` −3 (`declaration`, `file_of_expr`, `lower_type_node`, each now documented); `undeclared_words` 1 moved (`ret`, `check.rs` → `value/functions.rs`); `long_functions` −3 +1 (`expected_flow`, a new name for the core of `check_as`). Nothing in a `+` line adds a macro, an `#[allow]`, an `#[expect]`, an `unwrap`, a free function or a `pub(crate)`.
- `book_claims`: still red for the one stale row `structure~58f974e3` only, which the integration branch removed in cf16edbaa (N3); this round does not touch `tests/book/`.

### Phase 10 findings after round 1

| Finding | Severity | Status |
| --- | --- | --- |
| P10-1 approved box text rewritten | Low | closed |
| P10-2 `check.rs` beyond the worklist | Low | closed (residue accepted, design annotated; N5 names the owners) |
| P10-3 `PatternTables` note | Low | closed |
| P10-4 `method_call`, `check_as` verdicts | Low | closed |
| P10-5 stale docs | Low | closed |
| P10-6 T72 subject | Low | note |
| P10-7 `owners.tsv` task column | Low | closed |
| R1-1 43 redundant `self.expressions()` hops | Low | open |
| R1-2 `report_found_mismatch` unformatted | Low | open |
