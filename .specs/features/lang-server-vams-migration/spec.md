# Lang-Server + VS Code Extension VAMS Migration Specification

## Problem Statement

`crates/piperine-lang-server` and `editors/vscode` were never migrated
off PHDL when Verilog-AMS became the sole frontend (MD-44/45). The
extension's `activationEvents`/`documentSelector`/file-watcher only
recognize the dead `phdl` language id — opening a real `.vams`/`.va` file
in VS Code activates nothing at all: no syntax highlighting, no LSP, no
diagnostics. Separately, the packaged `.vsix`'s embedded binary can never
be found by the extension's own auto-discovery (a name-suffix mismatch
between `build-vsix.sh` and `extension.js`), and the lang-server crate's
own integration-test suite (44/53 tests) exercises PHDL fixture syntax
that the VAMS parser rejects outright — so today nothing signals when
hover/goto/rename/highlight/completion regress for real `.vams` content.

Full audit: prior-turn report in this session (also summarized in
`.specs/STATE.md`'s Handoff at time of writing).

## Goals

- [ ] Opening a real `.vams`/`.va` file in VS Code activates the
      extension: syntax highlighting, LSP client connects, diagnostics
      appear.
- [ ] The LSP server binary the extension needs is actually found —
      both from a local dev build (`cargo build`) and from the packaged
      `.vsix`'s embedded binary — without requiring the user to hand-edit
      `PATH` or a settings override.
- [ ] The packaged `.vsix` is rebuilt against the current source (not the
      stale 2026-07-25 build) and its embedded binaries actually resolve
      per the above.
- [ ] `crates/piperine-lang-server`'s integration-test fixtures are real,
      parseable Verilog-AMS (not PHDL) — the suite's pass/fail is a
      trustworthy signal for hover/goto/rename/highlight/completion
      regressions again.
- [ ] User-facing PHDL-era strings (a rename-validation error message the
      editor actually displays, doc comments, dead bench-syntax Test
      Explorer scanning, a dead `.phdl`/`.ppr` project-file glob) are
      updated to VAMS-accurate text/behavior.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---|---|
| Multi-error diagnostics (VAMS parser/elaborator is fail-fast, only the first error per file becomes a `Diagnostic`; PHDL's old tolerant multi-error mode is gone) | Real, known gap — but an architectural change to the parser/elaborator's error-recovery strategy, not a lang-server/extension wiring fix. Separate follow-up feature. |
| `Module`/`Port`/`Behavior::span` always `None` in `elab/lower/vams.rs` (blocks hover/goto/rename/highlight from ever resolving on those node kinds, even once everything in this feature is fixed) | Real, known gap (surfaced during `preprocessor-span-mapping`'s T13) — an elaborator change, not a lang-server/extension wiring fix. Separate follow-up feature. This feature's test-fixture migration (below) will surface exactly which hover/goto/rename cases stay dark because of it — expected, not a regression this feature introduces. |
| New LSP capabilities (semantic tokens, inlay hints, code actions, etc.) not already implemented | Scope is fixing what's wired but broken/stale, not adding new capability surface |
| Windows/macOS binary packaging validation (can't build/test those targets in this environment) | The Linux x86_64 path is what's verifiable here; the Windows/macOS suffix logic in `extension.js` is fixed by the same code change (one shared function) but not independently tested on those platforms |
| Publishing the rebuilt `.vsix` anywhere (marketplace, release asset) | Out of scope — rebuilding and validating locally only; publishing is a user decision outside this feature |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| New language id | `vams`, covering both `.vams` and `.va` extensions under one id (matching how the rest of the codebase treats them as one frontend, e.g. `CLAUDE.md`'s own "Verilog-AMS (LRM 2.4, `.vams`/`.va`)" phrasing) | Two extensions, one real language — no reason for two ids/two grammars | y (user said "manda bala," no blocking ambiguity here — logged per Large-tier closure gate) |
| TextMate scope name | `source.vams`, grammar file renamed `vams.tmLanguage.json` | Matches the new language id; `phdl.tmLanguage.json`'s existing rules are PHDL syntax (different keywords/grammar entirely) and cannot be reused as-is — this feature writes a new, minimal grammar covering VAMS keywords/comments/strings/directives (Annex A shape), not a full faithful port of every LRM 2.4 construct (that is its own scoped effort if ever wanted) | y |
| Config setting namespace | Renamed `phdl.server.path`/`phdl.cli.path`/`phdl.trace.server` → `piperine.server.path`/`piperine.cli.path`/`piperine.trace.server` | The extension has never worked for real files (0.1.0, audit confirms zero functional activation today) — there are no real users depending on the old key names; keeping `phdl.*` naming permanently would be confusing given PHDL is fully retired (MD-44/45) | y |
| Binary auto-discovery fix shape | Fix the embedded-`bin/`-candidate lookup to match what `build-vsix.sh` actually produces (unsuffixed names, since each `.vsix` build is already platform-specific — the OS suffix on the *candidate filename* was solving a problem the single-platform embedded-binary approach doesn't have); keep the OS-suffix concept only if a future multi-platform single-vsix packaging scheme is adopted (not now) | Matches `build-vsix.sh`'s actual behavior (copies plain `piperine-lang-server`/`piperine`) rather than inventing a second renaming step in the build script — smaller diff, one source of truth for the filename | y |
| Test-fixture migration scope | Every PHDL-syntax fixture string across the 11 test files gets rewritten to equivalent, minimal, parseable VAMS (`module`/`electrical`/`` `include "disciplines.vams"``-style, matching the project's own existing VAMS test-fixture conventions elsewhere in the repo, e.g. `crates/piperine-lang/tests/`) — preserving each test's original intent (same hover target, same rename scenario, etc.), not just making them compile | The whole point of Goal 4 is a trustworthy signal; a fixture that "parses" but no longer tests the same behavior would be worse than an honest skip | y |
| Tests currently failing *only* because of the known out-of-scope `span: None` gap | Left failing, but each such failure gets a one-line `// SPEC_DEVIATION`-style comment naming the gap and the follow-up feature responsible, rather than silently deleted/weakened/skipped | Matches the project's explicit test-integrity rules (never delete/weaken/skip to force a pass) and this feature's own Out-of-Scope table | y |

**Open questions:** none — all resolved or logged above.

---

## User Stories

### P1: `.vams`/`.va` files activate the extension ⭐ MVP

**User Story**: As a designer opening a real `.vams` file in VS Code, I
want syntax highlighting and the language server to activate, so the
editor is usable at all for my actual work.

**Why P1**: This is goal #1 from the audit — total, always-on impact.
Nothing else in this feature matters if the extension never activates.

**Acceptance Criteria**:

1. WHEN a `.vams` or `.va` file is opened in VS Code THEN the extension
   SHALL activate (language id `vams` recognized, `documentSelector`
   matches, file-system watcher covers `**/*.{vams,va}`).
2. WHEN a `.vams`/`.va` file is opened THEN VS Code SHALL apply the
   `source.vams` TextMate grammar (visible tokenization — keywords,
   comments, strings, `` ` ``-directives distinguished from plain text).
3. WHEN no `.phdl`/`.ppr` file exists anywhere in a workspace THEN
   nothing PHDL-specific SHALL be registered/activated (the dead language
   id is removed outright, not left dormant alongside the new one).

**Independent Test**: Open `examples/*.vams` (any file already in the
repo) in a VS Code instance with this extension installed; confirm
highlighting renders and the output channel shows the LSP client
starting.

---

### P2: The LSP server binary is actually found

**User Story**: As a user who installed the packaged extension (or is
running it from a local dev build), I want the language server to start
without manually setting a `PATH` entry or a settings override.

**Why P2**: Goal #2 — even with P1 fixed, a client that can't launch the
server shows highlighting but no diagnostics/hover/etc.

**Acceptance Criteria**:

1. WHEN the extension is packaged via `build-vsix.sh` and installed THEN
   the extension's own binary auto-discovery SHALL find the embedded
   `bin/piperine-lang-server` (and `bin/piperine`) it just packaged,
   without requiring `PATH`.
2. WHEN running from a local dev checkout with a default
   (`<workspace>/target/`) Cargo build THEN auto-discovery SHALL find the
   workspace's own `target/release/`or `target/debug/` binary (this path
   already works today per the audit's corrected finding — this AC is a
   regression guard, not a new fix).
3. WHEN no candidate resolves THEN the extension SHALL fall back to the
   bare configured name (today's existing behavior) rather than failing
   silently with no explanation — the output channel already logs the
   resolved `serverPath`; this stays true after the fix.

**Independent Test**: Run `build-vsix.sh`, inspect the resulting
`bin/` contents' filenames against exactly what the fixed
`getOsSpecificBinaryName`/candidate-list logic would look for on this
platform (Linux x86_64) — they must match.

---

### P3: The packaged `.vsix` is current

**User Story**: As someone installing the extension from the repo, I
want the `.vsix` to reflect today's `piperine-lang-server`/`piperine`
binaries and today's manifest, not a build from three weeks ago.

**Why P3**: Goal #3 — depends on P1/P2 landing first (repackaging a
still-broken extension just ships the same brokenness with a newer
timestamp).

**Acceptance Criteria**:

1. WHEN `build-vsix.sh` is run after P1/P2's changes THEN it SHALL
   produce a `.vsix` whose `package.json` reflects the new language
   id/config keys and whose embedded `bin/` binaries are freshly built
   from the current source tree.
2. WHEN the new `.vsix` is inspected (unzipped) THEN its `bin/` binary
   filenames SHALL match what the fixed auto-discovery logic (P2)
   actually looks for.

**Independent Test**: Run `build-vsix.sh`, unzip the resulting `.vsix`,
confirm `extension/bin/` contents and `extension/package.json`'s
language id match the P1/P2 changes.

---

### P4: Lang-server integration tests exercise real VAMS

**User Story**: As a `piperine-lang-server` developer, I want the
integration-test suite to actually test hover/goto/rename/highlight/
completion against valid Verilog-AMS, so a real regression shows up as a
test failure instead of being masked by fixtures that were already
broken for an unrelated reason (dead PHDL syntax).

**Why P4**: Goal #4 — the trustworthiness of the whole suite going
forward; doesn't block P1-P3's user-facing fix but is the safety net for
everything after this feature ships.

**Acceptance Criteria**:

1. WHEN each of the 11 test files under `crates/piperine-lang-server/tests/`
   is migrated THEN every fixture string SHALL be valid, parseable
   Verilog-AMS that preserves the original test's specific scenario
   (same construct kind being hovered/renamed/highlighted/completed).
2. WHEN the full suite is run after migration THEN every test that was
   failing *only* because of dead PHDL syntax SHALL now pass.
3. WHEN a test was failing (or, after migration, newly fails) because of
   the out-of-scope `span: None` gap THEN it SHALL be left failing with a
   comment naming the gap — never deleted, weakened, or skipped to force
   a pass.
4. WHEN the migration is done THEN the previously-passing subset (9/53)
   SHALL still pass unchanged (no regression from the rewrite itself).

**Independent Test**: `cargo test -p piperine-lang-server`; diff the
before/after failure list against the audit's documented per-file
breakdown (`completion_and_symbols` 0/5, `completion_suppression` 1/3,
`diagnostics` 3/8, `document_highlight` 0/1, `hover` 1/8, `import_goto`
0/1, `instance_highlight` 0/4, `project_index` 0/2, `protocol` 3/12,
`references_and_rename` 1/4, `symbol_resolution` 0/5).

---

### P5: PHDL-era user-facing strings and dead scanners

**User Story**: As a user, I never want to see the word "PHDL" in an
error message the editor shows me for a `.vams` file; as a developer, I
don't want the Test Explorer silently scanning for syntax that can never
match.

**Why P5**: Goal #5 — cosmetic/low-impact per the audit's own ranking,
sequenced last.

**Acceptance Criteria**:

1. WHEN a rename fails validation THEN the error message SHALL say
   "Verilog-AMS identifier" (or equivalent), never "PHDL identifier"
   (`handlers/rename.rs`).
2. WHEN reading `main.rs`/`lib.rs`'s crate-level doc comment THEN it SHALL
   describe the server as being for Verilog-AMS, not PHDL.
3. WHEN the VS Code Test Explorer scans a workspace THEN it SHALL look
   for `` `include``-able Python testbenches (`*_tb.py`, matching
   `piperine test`'s actual mechanism per `CLAUDE.md`'s "PHDL bench will
   die; Python is the host surface") instead of the dead
   `bench NAME { fn ... }` PHDL syntax over `**/*.phdl` files — or, if
   wiring a real Python-testbench Test Explorer view is judged out of
   this feature's small-fix size, the dead scanner/glob is removed
   outright rather than left silently matching nothing forever.
4. WHEN `piperine-project`'s recognized-extensions list is read THEN it
   SHALL no longer include `.phdl`/`.ppr` (dead, matches
   `tests/suite_hygiene.rs::phdl_corpus_does_not_grow`'s own "zero
   `.phdl` sources remain" invariant).

**Independent Test**: `grep -ri phdl crates/piperine-lang-server/src/`
returns nothing outside historical/comment context explaining the
migration itself (if any is kept for provenance, it must not appear in
any user-visible string or active glob/regex).

---

## Edge Cases

- WHEN a workspace has both `.vams` and legacy `.phdl` files (shouldn't
  happen per MD-44/45, but not enforced by the extension) THEN only the
  `.vams`/`.va` files activate anything — no PHDL activation path exists
  to accidentally trigger.
- WHEN `getOsSpecificBinaryName`'s candidate list finds nothing on any
  platform THEN behavior is unchanged from today (fall back to the bare
  configured name) — this feature fixes *which* candidates are checked,
  not the fallback behavior itself.
- WHEN a migrated test fixture's original PHDL source used a construct
  with no direct one-line VAMS equivalent THEN the migrating task uses
  the smallest valid VAMS construct that preserves the same *kind* of
  target (e.g. a PHDL `discipline` block's hover target becomes a VAMS
  `` `include "disciplines.vams"``'s `electrical` discipline reference,
  since VAMS disciplines are declared in Annex D headers, not inline
  per-file the way the PHDL fixtures assumed) — documented per-file if a
  non-obvious substitution was needed.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| LSPMIG-01 | P1: `.vams`/`.va` activates the extension | Tasks | Pending |
| LSPMIG-02 | P1: `source.vams` grammar applies | Tasks | Pending |
| LSPMIG-03 | P1: dead `phdl` language id fully removed | Tasks | Pending |
| LSPMIG-04 | P2: embedded `.vsix` binary auto-discovery fixed | Tasks | Pending |
| LSPMIG-05 | P2: local dev build auto-discovery regression guard | Tasks | Pending |
| LSPMIG-06 | P3: `.vsix` rebuilt current | Tasks | Pending |
| LSPMIG-07 | P4: all 11 test files migrated to real VAMS fixtures | Tasks | Pending |
| LSPMIG-08 | P4: PHDL-syntax-only failures now pass | Tasks | Pending |
| LSPMIG-09 | P4: `span:None`-only failures left failing, documented | Tasks | Pending |
| LSPMIG-10 | P4: previously-passing 9/53 still pass | Tasks | Pending |
| LSPMIG-11 | P5: rename error message text | Tasks | Pending |
| LSPMIG-12 | P5: doc comments describe VAMS | Tasks | Pending |
| LSPMIG-13 | P5: Test Explorer dead scanner fixed/removed | Tasks | Pending |
| LSPMIG-14 | P5: `.phdl`/`.ppr` removed from recognized extensions | Tasks | Pending |

**ID format:** `LSPMIG-NN`

**Status values:** Pending → In Design → In Tasks → Implementing →
Verified

**Coverage:** 14 total, 0 mapped to tasks (Tasks phase not yet run), 14
unmapped ⚠️ (expected at end of Specify)

---

## Success Criteria

- [ ] Opening any `examples/*.vams` file in a VS Code instance with the
      rebuilt extension shows highlighting + a live LSP connection
      (output channel confirms server start) with zero manual `PATH`
      setup.
- [ ] `cargo test -p piperine-lang-server` shows every PHDL-syntax-caused
      failure now passing; every remaining failure traces to the
      documented out-of-scope `span:None` gap, named in a comment.
- [ ] `grep -ri phdl` across `crates/piperine-lang-server/src/` and
      `editors/vscode/` (excluding historical/provenance comments) is
      empty.
