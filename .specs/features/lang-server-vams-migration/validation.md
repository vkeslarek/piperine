# Lang-Server + VS Code Extension VAMS Migration Validation

**Date**: 2026-08-16
**Spec**: `.specs/features/lang-server-vams-migration/spec.md`
**Diff range**: `a9c82140..8a775fce` (13 commits: `3dda4cf9`..`8a775fce`)
**Verifier**: independent sub-agent (author ≠ verifier), evidence gathered via two independent read-only forks + direct inspection/mutation testing by the Verifier itself

---

## Task Completion

| Task | Status  | Notes |
| ---- | ------- | ----- |
| T1   | ✅ Done | `package.json` id/aliases/extensions/grammar-ref/activationEvents/config-keys all confirmed |
| T2   | ✅ Done | `documentSelector`/watcher/`getConfiguration` calls confirmed |
| T3   | ✅ Done | `getOsSpecificBinaryName` fully deleted from both functions; plain `baseName` embedded-bin candidate confirmed |
| T4   | ✅ Done | `vams.tmLanguage.json` valid, `scopeName: source.vams`; `phdl.tmLanguage.json` deleted (162 lines) |
| T5   | ✅ Done | Real pre-existing bug confirmed and fixed (`cargo build -p piperine` → `-p piperine-cli`, since root `piperine` package is a lib with no `[[bin]]`); live `build-vsix.sh` run confirmed exit 0, correct `bin/` contents, correct `package.json` |
| T6   | ✅ Done | hover/document_highlight/import_goto migrated; no test deleted |
| T7   | ✅ Done | completion_and_symbols/completion_suppression/symbol_resolution migrated |
| T8   | ✅ Done | diagnostics/protocol migrated (heaviest pair) |
| T9   | ✅ Done | instance_highlight/project_index/references_and_rename migrated; one new test added (`rename_to_invalid_identifier_reports_a_verilog_ams_error`), not a deletion |
| T10  | ✅ Done | PHDL string replaced in `rename.rs`/`lib.rs`/`main.rs`; assertion updated to match, confirmed by mutation test (see Sensor) |
| T11  | ✅ Done | Dead Test Explorer bench scanner (`parseTestsInDocument`, controller, run profile, doc listeners) removed outright — 130 lines deleted, no orphaned registrations |
| T12  | ✅ Done | `EXTENSIONS` constant confirmed at `crates/piperine-lang-server/src/project.rs:160` = `["va", "vams"]`, matching the tasks.md note that it's here, not in `piperine-project` |

All 12 tasks genuinely done. No blocked/partial tasks.

---

## Spec-Anchored Acceptance Criteria

| Criterion | Spec-defined outcome | `file:line` + evidence | Result |
| --- | --- | --- | --- |
| LSPMIG-01: `.vams`/`.va` activates extension | `documentSelector` matches `language: vams`; watcher covers `**/*.{vams,va}` | `editors/vscode/package.json:22-24` (`id:"vams"`, `extensions:[".vams",".va"]`); `editors/vscode/extension.js:53-58` (`documentSelector` both schemes = `"vams"`; watcher `"**/*.{vams,va}"`) | ✅ PASS |
| LSPMIG-02: `source.vams` grammar applies | `scopeName: source.vams`, grammar covers keywords/comments/strings/`` ` ``-directives | `editors/vscode/package.json:30-32` (grammar registration); `editors/vscode/syntaxes/vams.tmLanguage.json` — valid JSON (`node -e JSON.parse` exit 0), `scopeName:"source.vams"`, repository entries for comments/strings/keywords/directives confirmed by fork | ✅ PASS |
| LSPMIG-03: dead `phdl` id fully removed | No `phdl` language id registered/activated anywhere | `grep -in phdl editors/vscode/{package.json,extension.js}` → zero hits; `phdl.tmLanguage.json` deleted (`git show 8a69ef17`, -162 lines) | ✅ PASS (see Code Quality note on orphaned `snippets/phdl.json`) |
| LSPMIG-04: embedded `.vsix` binary auto-discovery fixed | Embedded `bin/` candidate matches `build-vsix.sh`'s plain-name output | `editors/vscode/extension.js:32,97` — `path.join(context.extensionPath, "bin", serverBaseName)` / `cliBaseName`, no suffix; live `build-vsix.sh` run → `unzip -l` shows `extension/bin/piperine-lang-server`, `extension/bin/piperine` (plain names) | ✅ PASS |
| LSPMIG-05: local dev build regression guard | `target/release`/`target/debug` candidates unchanged | `editors/vscode/extension.js:23-28,88-93` — byte-identical to pre-feature version (fork diff-confirmed) | ✅ PASS |
| LSPMIG-06: `.vsix` rebuilt current | `build-vsix.sh` exits 0, `bin/` fresh, `package.json` shows new id/keys | Live run: exit 0; `unzip -p ... package.json` shows `"id":"vams"` + `piperine.server.path`/`piperine.cli.path`/`piperine.trace.server` | ✅ PASS |
| LSPMIG-07: 11 test files migrated to real VAMS | Every fixture parses, preserves original scenario kind | Fork spot-check: `diagnostics.rs` cross-file scenario retargeted (discipline-mismatch → undefined-module-instantiation, same "cross-file error fans out to right URI" kind, documented in commit); `hover.rs` extern-operator target retargeted to closest surviving construct, documented. All 11 files confirmed parseable (tests run past parse step) | ✅ PASS |
| LSPMIG-08: PHDL-syntax-only failures now pass | Per-file deltas match audit baseline | Live `cargo test -p piperine-lang-server` confirms: hover 3/8, completion_and_symbols 3/5, diagnostics 8/8, protocol 6/12, symbol_resolution 2/5, project_index 1/2, references_and_rename 2/5 (file grew to 5 tests via T10's new test — not a defect, see note) | ✅ PASS |
| LSPMIG-09: `span:None`-only failures documented | Every such failure has a naming comment | Fork cross-referenced every live failure against in-file comments — complete coverage | ⚠️ Spec-precision gap (see below) |
| LSPMIG-10: previously-passing 9/53 still pass | document_highlight/import_goto/completion_suppression/instance_highlight unchanged | Live run confirms 0/1, 0/1, 1/3, 0/4 respectively — unchanged; no `#[ignore]`/dead-code suppression anywhere in the 11 files (`grep` empty); no test function deleted (name-by-name diff across all 11 files) | ✅ PASS |
| LSPMIG-11: rename error message text | "Verilog-AMS identifier", never "PHDL identifier" | `crates/piperine-lang-server/src/handlers/rename.rs:21` — `format!("\`{new_name}\` is not a valid Verilog-AMS identifier")`; killed by mutation test (see Sensor mutation 1) | ✅ PASS |
| LSPMIG-12: doc comments describe VAMS | crate doc says Verilog-AMS, not PHDL | `crates/piperine-lang-server/src/lib.rs:1`, `src/main.rs:1` — both say "LSP server for Verilog-AMS." | ✅ PASS |
| LSPMIG-13: Test Explorer dead scanner fixed/removed | Either real `*_tb.py` scanner or full removal | `git show 483d91fd` — full removal chosen (130 lines deleted: controller, resolve handler, didOpen/didChange listeners, run profile); `piperine.test` terminal command (unrelated, pre-existing) untouched and confirmed not bench-scanning | ✅ PASS |
| LSPMIG-14: `.phdl`/`.ppr` removed from extensions | List no longer contains them | `crates/piperine-lang-server/src/project.rs:160` — `const EXTENSIONS: [&str; 2] = ["va", "vams"]` | ⚠️ PASS with gap — doc comments at lines 50, 74, 82 in the same file still read "every `.phdl` file under..." (present-tense, describing current — now wrong — behavior, not historical/provenance text). Violates the spirit of the Success Criteria's "`grep -ri phdl` ... excluding historical/provenance comments is empty" bullet. |

**Status**: ⚠️ Issues present — 14/14 ACs functionally satisfied, but 2 flagged items (1 spec-precision gap, 1 stale-doc-comment gap) below.

### Flagged item 1 — Spec-precision gap on LSPMIG-09 / doc:None

During migration, a **second**, previously-undocumented gap was discovered: `crates/piperine-lang/src/elab/lower/vams.rs` hardcodes `doc: None` on every POM node (13+ sites), blocking doc-hover the same way `span: None` blocks navigation. This was surfaced honestly — `crates/piperine-lang-server/tests/hover.rs:42-44` documents it in a comment, explicitly distinguishing it from the `span: None` gap. However the comment says "see spec.md's Out of Scope" — but `spec.md`'s Out of Scope table (reviewed directly) lists only the `span: None` gap for `Module`/`Port`/`Behavior`, never `doc: None`. The spec document itself was never retroactively amended to add this second gap. The underlying engineering judgment call (leave it failing, document it, don't silently skip) is correct and matches the spec's own stated policy for exactly this situation — but the specific claim "see spec.md's Out of Scope" is not accurate as written, since spec.md doesn't actually name `doc: None`. Recommend: amend spec.md's Out of Scope table to add a `doc: None` row (small, low-risk fix), or soften the test comment's citation. Not a blocker — no test was silently weakened, and the gap is real and out-of-scope by any reasonable reading — but the spec-to-code traceability claim needs a one-line fix.

### Flagged item 2 — Stale doc comments in `project.rs`

`crates/piperine-lang-server/src/project.rs:50,74,82` still describe the file-discovery behavior as covering "every `.phdl` file" — these are current, present-tense doc comments (not historical/provenance context describing why something changed), left un-updated when T12 changed the actual `EXTENSIONS` constant. This is exactly the kind of hit the Success Criteria's `grep -ri phdl` bullet is meant to catch, and T12's own "Done when" checklist (which only checked the constant's value + build cleanliness) missed it. Minor — pure doc-comment inaccuracy, no functional impact — but should be fixed as a quick follow-up.

---

## Discrimination Sensor

All three mutations were injected into the real working tree and reverted immediately (`git checkout -- <file>`) after each check; `git status` confirmed a clean tree before and after.

| # | File:line | Description | Killed? |
| - | --------- | ------------ | ------- |
| 1 | `crates/piperine-lang-server/src/handlers/rename.rs:21` | Reverted the LSPMIG-11 string fix (`"Verilog-AMS identifier"` → `"PHDL identifier"`) | ✅ Killed — `references_and_rename.rs:63`'s `rename_to_invalid_identifier_reports_a_verilog_ams_error` failed with `got: \`1bad\` is not a valid PHDL identifier` |
| 2 | `crates/piperine-lang-server/src/project.rs:160` | Reverted the LSPMIG-14 fix (`EXTENSIONS` back to `["va","vams","phdl","ppr"]`) | ❌ Survived — full `cargo test -p piperine-lang-server` run produced identical pass/fail counts with and without the mutation. Confirms the Test Coverage Matrix's own stated coverage for this layer ("build gate only — no dedicated test found for this exact list"); this is a documented, accepted gap in the task's own coverage design, not a surprise regression. |
| 3 | `editors/vscode/extension.js:32` | Reverted T3's fix (appended a bogus `"-x64"` suffix back onto the embedded-bin candidate) | ❌ Survived (by design) — `node --check editors/vscode/extension.js` exits 0 regardless, since it only validates JS syntax, not logic. This is the JS-layer discrimination-sensor limitation flagged in the task brief: there is no functional test framework in this repo for `extension.js`, so a logic-level regression in the candidate-path construction cannot be mechanically caught by anything currently in the suite. Not a false PASS — the Test Coverage Matrix is explicit that this layer's ceiling is `node --check` + manual code inspection, and this validation's own manual inspection (see AC LSPMIG-04 evidence) is what actually caught the real bug/fix pair. |

**Sensor depth**: lightweight (3 targeted mutations, default tier)
**Result**: 1/3 mechanically killed, 2/3 survived — both survivals are **expected and pre-documented** limitations of the chosen coverage strategy (Test Coverage Matrix explicitly scopes T12 to "build gate only" and the whole JS/JSON layer to scripted syntax checks, not functional tests), not undisclosed weaknesses discovered during verification. Recorded as PASS for sensor purposes since both gaps were already named in tasks.md rather than being surprises, but flagged below as residual risk.

---

## Code Quality

| Principle | Status |
| --- | --- |
| No features beyond what was asked | ✅ — T3's OS-suffix removal and T11's Test Explorer removal are both the explicitly-left-open judgment calls the spec granted ("Pick the simpler no-suffix version unless...", "Choose removal unless..."), not scope creep |
| No abstractions for single-use code | ✅ |
| No unnecessary "flexibility" added | ✅ |
| Only touched files required for task | ✅ — all 22 changed files map directly to the 12 tasks + spec/tasks docs |
| Didn't "improve" unrelated code | ✅ — the `piperine.test` terminal command, unrelated to the Test Explorer removal, was correctly left untouched |
| Matches existing patterns/style | ✅ |
| Would senior engineer approve? | ✅ with the two minor follow-ups noted above |
| Tests map to ACs and are non-shallow | ✅ — spot-checked (diagnostics.rs, hover.rs) fixture rewrites preserve original scenario kind, not just "make it parse" |
| Spec-anchored outcome check | ✅ — see AC table |
| Per-layer Coverage Expectation met | ✅ — matches the Test Coverage Matrix's own stated (and honestly limited) expectations per layer |
| Every test maps to a spec AC/edge case/Done-when | ✅ — including the one net-new test (T10's rename string test) |
| Documented guidelines followed | `CLAUDE.md` (`cargo build --workspace` zero warnings; a target lives in the crate it exercises) — followed |

**Minor observation (not a gap)**: `editors/vscode/snippets/phdl.json` is an orphaned, pre-existing file (untouched by any commit in this diff range, confirmed via `git log` on it in-range being empty) with no `snippets` contribution point in `package.json` — so it's inert, but `build-vsix.sh` still silently bundles it into the shipped `.vsix` as `extension/snippets/phdl.json`. Its *contents* have no literal "phdl" string, so the spec's content-grep success criterion technically doesn't catch it, and it predates this feature — out of scope to fix here, but worth a follow-up ticket since a `.vsix` literally containing a file named `phdl.json` undercuts the feature's own "no PHDL survives" goal.

---

## Edge Cases

- [x] Workspace with both `.vams` and legacy `.phdl` files: no PHDL activation path exists (confirmed — `phdl` language id fully deregistered)
- [x] `getOsSpecificBinaryName`'s candidate list finding nothing on any platform: fallback to bare configured name unchanged (candidate-list structure preserved, only the embedded-bin entry's name logic changed)
- [x] Migrated fixture with no direct one-line VAMS equivalent: confirmed handled via documented substitutions (e.g. hover.rs's extern-operator retarget, diagnostics.rs's discipline-mismatch retarget)

---

## Gate Check

- **Gate command**: `cargo build --workspace` (Build gate, mandatory)
- **Result**: exit 0, zero warnings (confirmed via `touch` + rebuild + `grep -c warning` = 0)
- **`cargo test -p piperine-lang-server`**: ran clean, per-file counts match claimed deltas (one file's denominator grew 4→5 due to T10's added test, not a defect)
- **Skipped tests**: none (`#[ignore]` grep empty across all 11 migrated files)
- **Failures**: all remaining failures trace to the documented `span:None`/`doc:None` out-of-scope gaps, each with an in-file comment

---

## Fix Plans (minor, non-blocking)

### Fix 1: Stale `.phdl` doc comments in `project.rs`

- **Root cause**: T12's "Done when" checklist checked the `EXTENSIONS` constant value and build cleanliness but not the surrounding doc comments describing the function's behavior in prose.
- **Fix task**: Update `crates/piperine-lang-server/src/project.rs:50,74,82` to say "every Verilog-AMS (`.va`/`.vams`) file" instead of "every `.phdl` file."
- **Priority**: Minor

### Fix 2: spec.md Out-of-Scope table missing `doc:None`

- **Root cause**: A second gap (`doc: None` hardcoded in `elab/lower/vams.rs`) was discovered during T6 (hover.rs migration) and honestly documented in test comments, but spec.md's Out of Scope table was never amended to actually list it — the test comment's citation ("see spec.md's Out of Scope") is currently inaccurate.
- **Fix task**: Add a `doc: None` row to spec.md's Out of Scope table (mirroring the existing `span: None` row), or adjust the test comments to not claim spec.md coverage that doesn't exist yet.
- **Priority**: Minor

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
| --- | --- | --- |
| LSPMIG-01 | Pending | ✅ Verified |
| LSPMIG-02 | Pending | ✅ Verified |
| LSPMIG-03 | Pending | ✅ Verified |
| LSPMIG-04 | Pending | ✅ Verified |
| LSPMIG-05 | Pending | ✅ Verified |
| LSPMIG-06 | Pending | ✅ Verified |
| LSPMIG-07 | Pending | ✅ Verified |
| LSPMIG-08 | Pending | ✅ Verified |
| LSPMIG-09 | Pending | ⚠️ Verified with spec-precision gap (Fix 2) |
| LSPMIG-10 | Pending | ✅ Verified |
| LSPMIG-11 | Pending | ✅ Verified |
| LSPMIG-12 | Pending | ✅ Verified |
| LSPMIG-13 | Pending | ✅ Verified |
| LSPMIG-14 | Pending | ⚠️ Verified with doc-comment gap (Fix 1) |

---

## Summary

**Overall**: ⚠️ Issues (minor, non-blocking) — functionally the feature is complete and correct; two small documentation-accuracy gaps found, no functional/test regressions, no scope creep, no weakened/deleted tests.

**Spec-anchored check**: 14/14 ACs functionally satisfied; 2 flagged (LSPMIG-09 spec-precision gap, LSPMIG-14 stale-comment gap)
**Sensor**: 1/3 mechanically killed; 2/3 survived, both pre-documented, expected limitations of the chosen (and honestly-scoped) coverage strategy for those layers
**Gate**: `cargo build --workspace` clean, zero warnings; `cargo test -p piperine-lang-server` matches all claimed per-file deltas

**What works**: Extension activates for real `.vams`/`.va` files, grammar highlights correctly, binary auto-discovery fixed and verified end-to-end via a live `.vsix` build/unzip, all 11 lang-server test files migrated to real VAMS with no test loss, PHDL-era strings and dead code fully removed, `.phdl`/`.ppr` dropped from recognized extensions.

**Issues found**: (1) `project.rs` doc comments still describe `.phdl` scanning (Fix 1); (2) spec.md's Out of Scope table doesn't actually list the `doc:None` gap that test comments claim it does (Fix 2). Both are documentation-only, non-functional.

**Next steps**: Apply Fix 1 and Fix 2 (both small, ~5 min each); no re-verification cycle needed given their non-functional nature — could be folded into the next small commit touching either file.
