# Lang-Server + VS Code Extension VAMS Migration Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by
name and follow its Execute flow and Critical Rules.** Do not search for
skill files by filesystem path. The skill is the source of truth for the
full flow (per-task cycle, sub-agent delegation, adequacy review,
Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not
proceed without it.**

---

**Spec**: `.specs/features/lang-server-vams-migration/spec.md`
**Status**: Draft — Design phase skipped (no architectural decision:
wiring/regex/config fixes and a mechanical test-syntax migration, no new
component or pattern per the skill's own skip rule)

---

## Test Coverage Matrix

> Generated from codebase sampling (`crates/piperine-lang-server/tests/*.rs`,
> `editors/vscode/package.json`'s `scripts`) and `CLAUDE.md`. Guidelines
> found: `CLAUDE.md` (`cargo build --workspace` zero warnings; a target
> lives in the crate it exercises). No JS test framework is configured in
> `editors/vscode/package.json` (no `test` script, no jest/mocha
> devDependency) — extension.js/package.json/syntax-grammar changes are
> verified by scripted shell checks (`node -e` syntax/require checks,
> `grep`, unzip-and-inspect the packaged `.vsix`) rather than a unit-test
> harness, matching the spec's own "Independent Test" descriptions.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `editors/vscode/package.json`/`language-configuration.json` (language id, grammar registration, config keys) | scripted verification | `node -e` loads the JSON and asserts the new `id`/`extensions`/`scopeName`/config-key fields; no test framework in this repo for this layer | n/a (inline verification script per task, discarded after) | `node -e "..."` |
| `editors/vscode/extension.js` (`documentSelector`, file watcher, `getOsSpecificBinaryName`/candidate lists ×2) | scripted verification | Every changed function checked against the exact candidate-path shape `build-vsix.sh` actually produces; `node --check extension.js` for syntax validity | n/a | `node --check editors/vscode/extension.js` + inline candidate-path assertions |
| `editors/vscode/syntaxes/vams.tmLanguage.json` (new grammar) | scripted verification | Valid JSON, `scopeName` matches `source.vams`, covers keywords/comments/strings/`` ` ``-directives per spec AC2 | n/a | `node -e "JSON.parse(require('fs').readFileSync('editors/vscode/syntaxes/vams.tmLanguage.json'))"` |
| `.vsix` packaging (`build-vsix.sh` output) | scripted verification | Unzipped `bin/` filenames match what the fixed candidate list looks for; `package.json` inside the package shows the new language id | n/a | `build-vsix.sh` then `unzip -p <vsix> extension/package.json` + `unzip -l <vsix>` |
| `crates/piperine-lang-server/tests/*.rs` (11 files, fixture migration) | integration (Rust) | 1:1 — every currently-PHDL-syntax-blocked test now exercises the same scenario in valid VAMS; `span:None`-only failures stay failing with a naming comment (per spec P4 AC3) | same 11 existing files, in place | `cargo test -p piperine-lang-server --test <target>` |
| `crates/piperine-lang-server/src/{handlers/rename.rs,main.rs,lib.rs}` (user-facing strings) | none beyond existing coverage (string-only change) | Existing tests in `references_and_rename.rs` cover `rename.rs`'s error path — after T-migration lands, confirm the new string appears; no new test file needed for a string change alone | `crates/piperine-lang-server/tests/references_and_rename.rs` (existing) | `cargo test -p piperine-lang-server --test references_and_rename` |
| `editors/vscode/extension.js` Test Explorer (`parseTestsInDocument`) | scripted verification | Confirms the dead `bench`-regex/`.phdl` glob no longer runs (removed or repointed), no JS test framework available | n/a | `node --check editors/vscode/extension.js` + manual read-through |
| `crates/piperine-project/src/*.rs` (extensions list) | build gate only | No dedicated test found for this exact list in a quick sample; a build-gate check is the floor per the strong default for config/entity-only changes | n/a | `cargo build -p piperine-project` |

**Coverage Expectation values** — no coverage-threshold config found;
strong default applies to the one real Rust-test layer (integration,
1:1 to spec ACs); the JS/JSON layers get the strongest verification this
repo's actual tooling supports (no framework exists to add one without
scope creep beyond this feature's fix-focused size).

## Gate Check Commands

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | After a task touching only `piperine-lang-server`'s Rust test target(s) | `cargo test -p piperine-lang-server --test <target>` |
| Scripted | After a task touching `editors/vscode/*.js`/`*.json` | `node --check editors/vscode/extension.js` (when touched) + the task's own inline verification script (see Test Coverage Matrix) |
| Full | After the extension-activation phase (P1/P2) and after the repackage task | `build-vsix.sh` (must exit 0) + unzip-inspect the result |
| Build | After every task, always | `cargo build --workspace` (zero warnings) |

---

## Execution Plan

### Phase 1: Extension activation core (P1 + P2)

```
T1 → T2 → T3 → T4
```

### Phase 2: Repackage (P3)

```
T5
```

### Phase 3: Test-fixture migration (P4)

```
T6 → T7 → T8 → T9
```

### Phase 4: Cosmetic strings + dead scanners (P5)

```
T10 → T11 → T12
```

---

## Task Breakdown

### T1: `package.json` — language id, extensions, grammar registration, config keys

**What**: Replace the `phdl` language contribution with `vams` (id
`vams`, `aliases: ["Verilog-AMS", "vams"]`, `extensions: [".vams", ".va"]`,
same `language-configuration.json` reference), point `grammars` at the
new `source.vams`/`vams.tmLanguage.json` (T4 creates the file; this task
only updates the registration — if T4 hasn't landed yet in isolation,
this task's own verification just checks the JSON shape, not that the
grammar file exists yet, since tasks run in order and T4 comes after in
this phase), change `activationEvents` to `["onLanguage:vams"]`, rename
config keys `phdl.server.path`→`piperine.server.path`,
`phdl.cli.path`→`piperine.cli.path`, `phdl.trace.server`→`piperine.trace.server`
(keep descriptions, just the key prefix).
**Where**: `editors/vscode/package.json`
**Depends on**: None
**Reuses**: existing `language-configuration.json` (unchanged, generic bracket/comment rules)
**Requirement**: LSPMIG-01, LSPMIG-03

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] `languages[0].id === "vams"`, `extensions` includes both `.vams`/`.va`, no `phdl` id remains anywhere in the file
- [x] `grammars[0].language === "vams"`, `scopeName === "source.vams"`, `path` points at the new grammar filename
- [x] `activationEvents` is `["onLanguage:vams"]`
- [x] `configuration.properties` keys are `piperine.server.path`/`piperine.cli.path`/`piperine.trace.server`
- [x] `node -e "JSON.parse(require('fs').readFileSync('editors/vscode/package.json'))"` exits 0 (valid JSON)

**Tests**: scripted verification
**Gate**: scripted

---

### T2: `extension.js` — document selector, file watcher, config namespace

**What**: `documentSelector` entries change `language: "phdl"` → `language: "vams"`
(both `file` and `untitled` scheme entries); `createFileSystemWatcher("**/*.phdl")`
→ `createFileSystemWatcher("**/*.{vams,va}")`; every
`vscode.workspace.getConfiguration("phdl.server")`/`"phdl.cli"` call
becomes `getConfiguration("piperine.server")`/`"piperine.cli"` (matching T1's renamed keys).
**Where**: `editors/vscode/extension.js`
**Depends on**: T1 (config key names must match)
**Reuses**: existing `startClient`/`getPiperineCliPath` structure, unchanged otherwise
**Requirement**: LSPMIG-01

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] No occurrence of the string `"phdl"` remains in `documentSelector`, the file watcher glob, or any `getConfiguration(...)` call
- [x] `node --check editors/vscode/extension.js` exits 0
- [x] `grep -n '"phdl"' editors/vscode/extension.js` — every remaining hit (if any) is outside these three spots and tracked separately by T10/T11 (Test Explorer), not silently left in the activation path

**Tests**: scripted verification
**Gate**: scripted

---

### T3: `extension.js` — fix binary auto-discovery candidate lists

**What**: Both `getOsSpecificBinaryName`-using candidate-list blocks
(`startClient`'s LSP server resolution, `getPiperineCliPath`'s CLI
resolution) currently only apply the OS-suffixed name to the embedded
`bin/` candidate — but `build-vsix.sh` copies plain, unsuffixed
`piperine-lang-server`/`piperine` into `bin/`. Change the embedded-`bin/`
candidate to use the plain `baseName` (matching what `build-vsix.sh`
actually produces), dropping the OS-suffix logic entirely per the spec's
logged assumption (single-platform-per-`.vsix` packaging makes the
suffix redundant) — or, if kept for future multi-platform packaging,
gate it behind checking the suffixed name FIRST and falling back to the
plain name, so today's actual `build-vsix.sh` output still resolves
either way. Pick the simpler no-suffix version unless it conflicts with
something found while implementing (state the choice made in the commit
message).
**Where**: `editors/vscode/extension.js` (`startClient`, `getPiperineCliPath`)
**Depends on**: T2
**Reuses**: existing candidate-list structure (workspace target dirs, embedded bin/, `~/.cargo/bin`) — only the embedded-bin/ entry's filename logic changes
**Requirement**: LSPMIG-04, LSPMIG-05

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] The embedded `bin/` candidate path for both functions matches exactly what `ls editors/vscode/bin/` shows after running `build-vsix.sh` (T5 confirms this end-to-end; this task's own check is a static read of the candidate-path construction against `build-vsix.sh`'s `cp` lines)
- [x] The workspace `target/release`/`target/debug` candidates (already correct per the audit) are unchanged — regression guard, LSPMIG-05
- [x] `node --check editors/vscode/extension.js` exits 0

**Tests**: scripted verification
**Gate**: scripted

---

### T4: New `source.vams` TextMate grammar

**What**: `editors/vscode/syntaxes/phdl.tmLanguage.json` → new
`vams.tmLanguage.json` (`scopeName: "source.vams"`), a minimal grammar
covering: line comments (`//`), block comments (`/* */`), string
literals, VAMS keywords (`module`/`endmodule`/`analog`/`discipline`/
`enddiscipline`/`nature`/`endnature`/`parameter`/`input`/`output`/
`inout`/`electrical`/`branch`/`function`/`endfunction`/control-flow
keywords per Annex B), `` ` ``-prefixed compiler directives
(`` `include``/`` `define``/`` `ifdef``/etc, ch. 10), and numeric
literals with LRM scale-factor suffixes. Delete the old
`phdl.tmLanguage.json` (dead language, matches the crate-side full
removal, not a soft-deprecate).
**Where**: `editors/vscode/syntaxes/vams.tmLanguage.json` (new), remove `editors/vscode/syntaxes/phdl.tmLanguage.json`
**Depends on**: T1 (registration references this filename)
**Reuses**: `VAMS-LRM-2-4.md` Annex A/B as the keyword/grammar source (same provenance rule as the Rust lexer — C7, never a third-party grammar)
**Requirement**: LSPMIG-02

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] `node -e "JSON.parse(require('fs').readFileSync('editors/vscode/syntaxes/vams.tmLanguage.json'))"` exits 0
- [x] `scopeName` is `source.vams`
- [x] Manual check: opening a sample `.vams` file (e.g. `examples/*.vams`) in a VS Code Extension Development Host shows distinct token colors for keywords, comments, strings, and `` ` ``-directives (documented as done in the task's own report — no automated screenshot check available; verified by pattern inspection against `examples/02_rc_lowpass.vams` instead since no VS Code instance is available in this environment)
- [x] `editors/vscode/syntaxes/phdl.tmLanguage.json` no longer exists

**Tests**: scripted verification + manual visual check (documented)
**Gate**: scripted

---

### T5: Rebuild and verify the `.vsix`

**What**: Run `build-vsix.sh`, then verify the packaged output actually
resolves per T3's fix: unzip the resulting `.vsix`, confirm
`extension/bin/piperine-lang-server` and `extension/bin/piperine` exist
under the exact names T3's candidate-list logic looks for, and confirm
`extension/package.json` shows the `vams` language id and renamed config
keys.
**Where**: `editors/vscode/*.vsix` (regenerated), no source changes beyond running the existing script
**Depends on**: T4
**Reuses**: `build-vsix.sh` as-is (no script changes needed unless T3's chosen fix requires the script to also emit an OS-suffixed copy — only if that path was chosen)
**Requirement**: LSPMIG-06

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] `build-vsix.sh` exits 0
- [x] `unzip -l <new vsix>` shows `extension/bin/piperine-lang-server` and `extension/bin/piperine`
- [x] `unzip -p <new vsix> extension/package.json` shows `"id": "vams"` and the renamed config keys
- [x] The stale `piperine-0.1.0.vsix` (or whatever old file remains) is removed/replaced, not left alongside as a second, confusing artifact

**Tests**: scripted verification (Full gate)
**Gate**: full

**Commit**: `fix(vscode): activate for real .vams/.va files, fix binary auto-discovery, repackage`

---

### T6: Migrate navigation test fixtures (hover, document_highlight, import_goto)

**What**: Rewrite every PHDL-syntax fixture string in these 3 files to
equivalent valid VAMS, preserving each test's exact scenario (same
hover/highlight/goto target kind). E.g. a PHDL
`discipline Electrical { potential v: Real; flow i: Real; }` /
`mod R (inout p: Electrical, inout n: Electrical) {}` pair becomes VAMS's
`` `include "disciplines.vams"``-based module using the builtin
`electrical` discipline, e.g.:
```
`include "disciplines.vams"
/// A two-terminal resistor.
module R(p, n);
  inout electrical p, n;
endmodule
```
(adjust line/character positions in each test's assertions to match the
new fixture text — this is the bulk of the task's actual work, not just
swapping keywords).
**Where**: `crates/piperine-lang-server/tests/{hover.rs,document_highlight.rs,import_goto.rs}`
**Depends on**: None (independent of Phase 1/2 — pure Rust-crate work)
**Reuses**: `crates/piperine-lang/tests/`'s existing VAMS fixture conventions (`` `include "disciplines.vams"``, `module`/`endmodule` shape) as the style reference
**Requirement**: LSPMIG-07, LSPMIG-08, LSPMIG-09, LSPMIG-10

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] Every fixture string in these 3 files is valid VAMS (parses without a `ParseError`) — verified by the test itself running past the parse step
- [x] Every test that was failing *only* due to PHDL syntax (not `span:None`) now passes
- [x] Any test still failing due to `span:None` has a one-line comment naming the gap and pointing at `.specs/features/lang-server-vams-migration/spec.md`'s Out of Scope entry
- [x] Previously-passing tests in these 3 files (1/8 in hover.rs per the audit) still pass unchanged
- [x] Gate check passes: `cargo test -p piperine-lang-server --test hover --test document_highlight --test import_goto`

**Tests**: integration (existing files, rewritten fixtures)
**Gate**: quick

---

### T7: Migrate completion/symbol test fixtures (completion_and_symbols, completion_suppression, symbol_resolution)

**What**: Same migration approach as T6, applied to these 3 files
(completion offering, schema-attribute completion, symbol/outline
resolution scenarios).
**Where**: `crates/piperine-lang-server/tests/{completion_and_symbols.rs,completion_suppression.rs,symbol_resolution.rs}`
**Depends on**: None
**Reuses**: same VAMS fixture conventions as T6
**Requirement**: LSPMIG-07, LSPMIG-08, LSPMIG-09, LSPMIG-10

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] Every fixture string in these 3 files is valid VAMS
- [x] Every PHDL-syntax-only failure now passes
- [x] Any `span:None`-only failure documented per T6's pattern
- [x] Previously-passing tests unchanged (completion_suppression 1/3)
- [x] Gate check passes: `cargo test -p piperine-lang-server --test completion_and_symbols --test completion_suppression --test symbol_resolution`

**Tests**: integration (existing files, rewritten fixtures)
**Gate**: quick

---

### T8: Migrate diagnostics + protocol test fixtures (heaviest pair)

**What**: Same migration approach, applied to `diagnostics.rs` (8 tests)
and `protocol.rs` (12 tests, the two largest files) — diagnostics
publishing scenarios and core protocol handshake/lifecycle scenarios.
**Where**: `crates/piperine-lang-server/tests/{diagnostics.rs,protocol.rs}`
**Depends on**: None
**Reuses**: same VAMS fixture conventions as T6
**Requirement**: LSPMIG-07, LSPMIG-08, LSPMIG-09, LSPMIG-10

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] Every fixture string in these 2 files is valid VAMS
- [x] Every PHDL-syntax-only failure now passes
- [x] Any `span:None`-only failure documented per T6's pattern
- [x] Previously-passing tests unchanged (diagnostics 3/8, protocol 3/12)
- [x] Gate check passes: `cargo test -p piperine-lang-server --test diagnostics --test protocol`

**Tests**: integration (existing files, rewritten fixtures)
**Gate**: quick

---

### T9: Migrate remaining test fixtures (instance_highlight, project_index, references_and_rename)

**What**: Same migration approach, applied to the last 3 files. This
task ALSO covers T11's rename-error-string update's own test evidence
(`references_and_rename.rs` already exercises the rename-validation
error path — after migrating its fixtures, confirm the new "Verilog-AMS
identifier" string, once T11 lands, appears where the old "PHDL
identifier" string used to — sequenced here since T9 runs before T11 in
the plan; if T11 hasn't landed yet when this task runs, note the
still-old string in a follow-up check rather than blocking on it, since
Phase 3/Phase 4 ordering is deliberate (fixture correctness first, string
polish after)).
**Where**: `crates/piperine-lang-server/tests/{instance_highlight.rs,project_index.rs,references_and_rename.rs}`
**Depends on**: None
**Reuses**: same VAMS fixture conventions as T6
**Requirement**: LSPMIG-07, LSPMIG-08, LSPMIG-09, LSPMIG-10

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] Every fixture string in these 3 files is valid VAMS
- [x] Every PHDL-syntax-only failure now passes
- [x] Any `span:None`-only failure documented per T6's pattern
- [x] Previously-passing tests unchanged (references_and_rename 1/4)
- [x] Gate check passes: `cargo test -p piperine-lang-server --test instance_highlight --test project_index --test references_and_rename`

**Tests**: integration (existing files, rewritten fixtures)
**Gate**: quick

---

### T10: Fix user-facing PHDL strings (`rename.rs`, `main.rs`, `lib.rs`)

**What**: `handlers/rename.rs`'s validation-error message changes
`` `{new_name}` is not a valid PHDL identifier `` → `` `{new_name}` is not
a valid Verilog-AMS identifier ``; `main.rs`/`lib.rs`'s crate-level doc
comment ("LSP server for PHDL") updated to describe Verilog-AMS.
**Where**: `crates/piperine-lang-server/src/{handlers/rename.rs,main.rs,lib.rs}`
**Depends on**: T9 (so the just-migrated `references_and_rename.rs` test can be re-run against the new string in the same gate check, catching a stale-assertion mismatch immediately)
**Reuses**: `references_and_rename.rs`'s existing invalid-identifier test case (assertion text updated to match, per the spec's explicit AC — this is the one test-string change this feature makes deliberately, not silent weakening)
**Requirement**: LSPMIG-11, LSPMIG-12

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] `grep -n "PHDL" crates/piperine-lang-server/src/handlers/rename.rs` returns nothing
- [x] `grep -rn "PHDL" crates/piperine-lang-server/src/{main.rs,lib.rs}` returns nothing
- [x] `references_and_rename.rs`'s invalid-identifier test asserts on the new string and passes
- [x] Gate check passes: `cargo test -p piperine-lang-server --test references_and_rename`

**Tests**: integration (existing test, assertion text updated)
**Gate**: quick

---

### T11: Fix or remove the dead Test Explorer bench scanner

**What**: `extension.js`'s `parseTestsInDocument` (bench-regex over
`**/*.phdl`) never matches anything (PHDL is gone). Per spec P5 AC3,
either repoint it at real Python testbenches (`*_tb.py`, matching
`piperine test`'s actual mechanism) or remove the dead Test Explorer
feature outright (controller registration, the file watcher for it,
associated `package.json` contribution points if any) rather than
leaving code that can never fire. Choose removal unless wiring the real
`*_tb.py` discovery turns out to be a small, contained change once in the
code (state the choice made and why in the commit message — this is a
judgment call explicitly left open by the spec, not a spec violation
either way).
**Where**: `editors/vscode/extension.js` (Test Explorer section), `package.json` if any related contribution point needs removing
**Depends on**: T3 (same file, avoid clobbering)
**Reuses**: N/A
**Requirement**: LSPMIG-13

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [x] No code path scans for `bench NAME { fn ... }` over `.phdl` files
- [x] Either a working `*_tb.py`-based Test Explorer exists, or the dead feature is fully removed (no orphaned controller/watcher left registered doing nothing)
- [x] `node --check editors/vscode/extension.js` exits 0

**Tests**: scripted verification
**Gate**: scripted

---

### T12: Remove `.phdl`/`.ppr` from `piperine-project`'s recognized extensions

**What**: `EXTENSIONS = ["va", "vams", "phdl", "ppr"]` → `["va", "vams"]`.
**Where**: `crates/piperine-project/src/project.rs` (or wherever the constant the audit found lives)
**Depends on**: None
**Reuses**: N/A
**Requirement**: LSPMIG-14

**Tools**: MCP: NONE / Skill: NONE

**Done when**:
- [ ] The extensions list no longer contains `"phdl"`/`"ppr"`
- [ ] `cargo build -p piperine-project` is clean
- [ ] `cargo test -p piperine-project` (existing suite) shows no new failure

**Tests**: build gate only (per Test Coverage Matrix — no dedicated test for this exact list found)
**Gate**: build

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1:  T1 ──→ T2 ──→ T3 ──→ T4
Phase 2:  T5
Phase 3:  T6 ──→ T7 ──→ T8 ──→ T9
Phase 4:  T10 ──→ T11 ──→ T12
```

12 tasks total → 2 batches at the ~7-task budget: Phase 1+2 = 5 tasks,
Phase 3+4 = 7 tasks — offered to the user at Execute time, not assumed
here.

---

## Task Granularity Check

| Task | Scope | Status |
|---|---|---|
| T1: package.json language/config rename | 1 file, 1 cohesive JSON edit | ✅ Granular |
| T2: extension.js selector/watcher/config | 1 file, 3 related call sites | ✅ Granular (cohesive concern) |
| T3: extension.js binary discovery fix | 1 file, 2 near-identical functions | ✅ Granular |
| T4: new TextMate grammar | 1 new file + 1 deletion | ✅ Granular |
| T5: rebuild/verify .vsix | 1 script run + verification | ✅ Granular |
| T6: migrate 3 nav test files | 3 files, 1 cohesive concern (navigation fixtures) | ✅ Granular (2-3 related files = OK) |
| T7: migrate 3 completion/symbol test files | 3 files, 1 cohesive concern | ✅ Granular |
| T8: migrate 2 heaviest test files | 2 files, 1 cohesive concern | ✅ Granular |
| T9: migrate 3 remaining test files | 3 files, 1 cohesive concern | ✅ Granular |
| T10: user-facing string fixes | 3 files, 1 cohesive concern (string text) | ✅ Granular |
| T11: Test Explorer dead scanner | 1 file (extension.js section) | ✅ Granular |
| T12: extensions list | 1 file, 1 constant | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
|---|---|---|---|
| T1 | None | (start of Phase 1) | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T1 | (start of Phase 1 — cross-dep to T1, backward, allowed) | ✅ Match |
| T5 | T4 | (start of Phase 2, after Phase 1) | ✅ Match |
| T6 | None | (start of Phase 3, after Phase 2 — independent, sequenced by phase order not data dep) | ✅ Match |
| T7 | None | T6 → T7 (sequencing only, no data dep) | ✅ Match |
| T8 | None | T7 → T8 (sequencing only) | ✅ Match |
| T9 | None | T8 → T9 (sequencing only) | ✅ Match |
| T10 | T9 | (start of Phase 4, after Phase 3) | ✅ Match |
| T11 | T3 | T10 → T11 (diagram shows sequencing; body's real dependency is T3, cross-phase backward, allowed) | ✅ Match |
| T12 | None | T11 → T12 (sequencing only) | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
|---|---|---|---|---|
| T1 | package.json | scripted verification | scripted verification | ✅ OK |
| T2 | extension.js (selector/watcher/config) | scripted verification | scripted verification | ✅ OK |
| T3 | extension.js (binary discovery) | scripted verification | scripted verification | ✅ OK |
| T4 | vams.tmLanguage.json | scripted verification | scripted + manual visual check | ✅ OK |
| T5 | .vsix packaging | scripted verification (full gate) | scripted verification, full | ✅ OK |
| T6 | 3 test files | integration | integration | ✅ OK |
| T7 | 3 test files | integration | integration | ✅ OK |
| T8 | 2 test files | integration | integration | ✅ OK |
| T9 | 3 test files | integration | integration | ✅ OK |
| T10 | rename.rs/main.rs/lib.rs strings | existing test coverage (references_and_rename) | integration (existing test) | ✅ OK |
| T11 | extension.js Test Explorer | scripted verification | scripted verification | ✅ OK |
| T12 | project.rs extensions list | build gate only | build gate only | ✅ OK |

No ❌ violations.
