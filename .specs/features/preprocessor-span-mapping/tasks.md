# Preprocessor Span-Mapping Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by
name and follow its Execute flow and Critical Rules.** Do not search for
skill files by filesystem path. The skill is the source of truth for the
full flow (per-task cycle, sub-agent delegation, adequacy review,
Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not
proceed without it.**

---

**Design**: `.specs/features/preprocessor-span-mapping/design.md`
**Status**: Draft

---

## Test Coverage Matrix

> Generated from codebase sampling (`crates/piperine-lang/tests/corpus.rs`,
> `crates/piperine-lang/tests/preprocessor.rs`,
> `crates/piperine-lang/tests/errors.rs`,
> `crates/piperine-lang-server/tests/diagnostics.rs`) and
> `CLAUDE.md` ("Build and test", "Where the tests are"). Guidelines found:
> `CLAUDE.md` — `cargo build --workspace` zero warnings is the bar; a
> target lives in the crate it exercises (MD-28); the `corpus.rs`
> env-var-gated real-model sweep pattern is the existing precedent for
> "run against real-world files without vendoring them."

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `SourceProvenance`/`Span`/`ExpansionFrame` (new domain types + forward `resolve()`) | unit/integration | All branches; 1:1 to SPANMAP-01/02/03/07 | `crates/piperine-lang/tests/source_provenance.rs` (new) | `cargo test -p piperine-lang --test source_provenance` |
| `Span`↔`PreprocessedSpan` mechanical rename (lexer/grammar/AST/pom) | none (build gate only) | Zero behavior change — existing suite is the regression gate, no new tests written for the rename itself | n/a | `cargo build --workspace` + full existing `piperine-lang` suite |
| `piperine-codegen` `Span` rename | none (build gate only) | Same — pure rename | n/a | `cargo build -p piperine-codegen` |
| `Preprocessor::run` provenance recording (paired `push_str`+record, no-gap invariant) | unit/integration | Every existing `preprocessor.rs` test still passes + new no-gap-in-segment-table assertion | `crates/piperine-lang/tests/preprocessor.rs` (extend) | `cargo test -p piperine-lang --test preprocessor` |
| `lib.rs` render boundary (`NamedSource` via resolved `Span`, replacing `.with_source_code(input.to_string())`) | integration | Deterministic scratch case (SPANMAP-02's exact crash shape) always runs, no env var; extends the existing opt-in `PIPERINE_VAMS_CORPUS` sweep with a zero-`OutOfBounds`/zero-panic assertion | `crates/piperine-lang/tests/span_resolution.rs` (new) + `corpus.rs` (extend) | `cargo test -p piperine-lang --test span_resolution` / `cargo test -p piperine-lang --test corpus` |
| Macro-expansion dual attribution (`ExpansionFrame` chain) | unit/integration | 1:1 to SPANMAP-04/05/06 incl. the nested (N-deep) case | `crates/piperine-lang/tests/macro_expansion_span.rs` (new) | `cargo test -p piperine-lang --test macro_expansion_span` |
| LSP reverse lookup + diagnostics/hover position resolution | integration | 1:1 to SPANMAP-08 | `crates/piperine-lang-server/tests/diagnostics.rs`, `hover.rs` (extend) | `cargo test -p piperine-lang-server --test diagnostics` / `--test hover` |
| `piperine expand` CLI command (default + `--trace`) | integration | 1:1 to SPANMAP-09/10/11 | `crates/piperine-cli/tests/expand_cmd.rs` (new, matching `check_cmd.rs`'s existing convention) | `cargo test -p piperine-cli --test expand_cmd` |

**Coverage Expectation values** — no dedicated coverage-threshold config
found in this repo (no `.nycrc`/coverage gate in CI configs); the strong
default applies: domain/business logic (the new `source_provenance`
module, the preprocessor's recording logic, the render boundary) maps
1:1 to spec ACs with every listed edge case covered; pure mechanical
renames get build-gate-only coverage (a rename has no new behavior to
assert against).

## Gate Check Commands

> Generated from `CLAUDE.md`'s documented commands — confirm before
> Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | After a task that only touches one `piperine-lang` test target | `cargo test -p piperine-lang --test <target>` |
| Full | After a task spanning `piperine-lang` + a consumer crate (codegen/lang-server) | `cargo test -p piperine-lang && cargo test -p piperine-codegen && cargo test -p piperine-lang-server` (scoped to targets that already compiled before this feature — the ~22 pre-existing dead-PHDL-debt `piperine-lang` targets and ~6 pre-existing `piperine-codegen` targets, per MD-47's own A/B record, are known-broken independent of this feature and are excluded from the pass/fail gate, exactly as MD-47's own verification did) |
| Build | After every task, always | `cargo build --workspace` (zero warnings — the project bar) |

---

## Execution Plan

Phases are ordered and run sequentially — each phase completes before
the next begins, and tasks within a phase execute in order.

### Phase 1: Foundation — new types, no rename yet

```
T1 → T2
```

### Phase 2: Mechanical rename (Span → PreprocessedSpan)

```
T3 → T4 → T5
```

### Phase 3: Preprocessor provenance + P1 render-boundary fix (MVP)

```
T6 → T7 → T8
```

### Phase 4: Macro-expansion dual attribution (P2)

```
T9 → T10
```

### Phase 5: LSP integration (P3)

```
T11 → T12 → T13
```

### Phase 6: `piperine expand` CLI command (P2.5)

```
T14 → T15
```

---

## Task Breakdown

### T1: Define `SourceId`, `Span`, `ExpansionFrame` types

**What**: New module `crates/piperine-lang/src/source_provenance.rs` with
the `SourceId`, `Span { source_id, start, end, expansion_chain }`, and
`ExpansionFrame { invocation, definition }` structs from the design's
Data Models section — no `SourceProvenance` registry logic yet, just the
plain data types plus their obvious derives (`Debug`, `Clone`,
`PartialEq`).
**Where**: `crates/piperine-lang/src/source_provenance.rs` (new), wired
into `crates/piperine-lang/src/lib.rs`'s module list
**Depends on**: None
**Reuses**: None (new module) — mirrors `error.rs`'s existing `Span`
derive style
**Requirement**: SPANMAP-04, SPANMAP-05, SPANMAP-06 (data shape only)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `SourceId`, `Span`, `ExpansionFrame` compile with the fields the design specifies
- [x] `cargo build -p piperine-lang` is clean (new module, unused-code warnings expected until T2 uses it — allow that, don't suppress with lint attributes per MD-33)

**Tests**: none (pure data shape, exercised by T2's tests)
**Gate**: build

---

### T2: `SourceProvenance` registry + forward `resolve()`

**What**: `SourceProvenance` struct (`sources`, `segments`), `Segment`
struct, a builder API the preprocessor will call in Phase 3
(`SourceProvenance::Builder::record(...)`), and
`SourceProvenance::resolve(&self, span: PreprocessedSpan) -> Span` —
binary-search segment lookup + `expansion_chain` construction by
following `expanded_from` links. Also `text()`/`name()` accessors and the
documented "empty/test" constructor from the design's Risks table (so
hand-built test `Design`s degrade gracefully instead of hitting the
loud-fail path).
**Where**: `crates/piperine-lang/src/source_provenance.rs` (extend T1's
file)
**Depends on**: T1
**Reuses**: T1's types
**Requirement**: SPANMAP-01, SPANMAP-02, SPANMAP-03, SPANMAP-07

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `resolve()` correctly maps a segment-covered offset to its real `(source_id, local_offset)`
- [x] `resolve()` on a zero-length `PreprocessedSpan` returns a valid zero-width `Span` (spec Edge Case)
- [x] `resolve()` on a span that straddles two segments resolves using the segment covering the span's *start* (spec Edge Case, matches existing "unterminated X anchors on start" convention)
- [x] The empty/test constructor's `resolve()` degrades to a synthetic `<test>` source name rather than the loud-fail path
- [x] Gate check passes: `cargo test -p piperine-lang --test source_provenance`
- [x] Test count: at least one test per Done-when bullet above (no silent deletions)

**Tests**: unit/integration (`source_provenance.rs`, per matrix)
**Gate**: quick

---

### T3: Rename `Span` → `PreprocessedSpan` in `piperine-lang`

**What**: Pure mechanical rename across `error.rs` (the type
definition itself), `lexer.rs` (31 `Span::new(` sites), `grammar/{expr,
stmt,item}.rs` (69 combined sites), `ast/`, `elab/`, `pom/error.rs`, and
every `use ...::Span` import in this crate. No re-export/alias left
behind (design's Risks mitigation: a stale `Span` name must be a hard
compile error, not a silent pass). Zero logic changes — if a diff touches
anything beyond a type/identifier rename, that's scope creep, revert it.
**Where**: `crates/piperine-lang/src/{error.rs,lexer.rs,grammar/*.rs,ast/**,elab/**,pom/error.rs}`
**Depends on**: T2 (so `PreprocessedSpan` and the new `Span` coexist
correctly from the moment the rename lands — no window where both mean
the old thing)
**Reuses**: N/A (rename only)
**Requirement**: N/A (mechanical prerequisite for SPANMAP-01..08, not itself spec-mapped)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `grep -rn "\bSpan::new(" crates/piperine-lang/src/{lexer.rs,grammar}` returns zero matches (all renamed to `PreprocessedSpan::new(`)
- [x] No `pub use ... as Span` alias exists anywhere in `piperine-lang` pointing at the old type
- [x] `cargo build -p piperine-lang` is clean (zero warnings)
- [x] Full existing `piperine-lang` test suite (targets that compiled before this task) passes byte-identical to pre-rename, per MD-47's own A/B'd-via-git-stash verification method — this task explicitly re-runs that method

**Tests**: none new (rename — existing suite is the regression gate)
**Gate**: build + full existing suite (targeted, excluding the pre-existing dead-PHDL-debt targets per the Gate Check Commands table)

---

### T4: Rename `Span` → `PreprocessedSpan` in `piperine-codegen`

**What**: Same mechanical rename across the 10 files identified during
Design (`resolve/pom/{expr,structure,body,analog_ops}.rs`,
`resolve/diff.rs`, `kernel/digital/udp.rs`, `kernel/analog/compile.rs`,
`emit/analog_expr.rs`, `flatten/analog.rs`, plus their test file
`tests/vams_udp.rs`).
**Where**: `crates/piperine-codegen/src/**`, `crates/piperine-codegen/tests/vams_udp.rs`
**Depends on**: T3
**Reuses**: N/A (rename only)
**Requirement**: N/A (mechanical prerequisite)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `cargo build -p piperine-codegen` is clean
- [x] `cargo test -p piperine-codegen` — existing passing targets (excluding the ~6 pre-existing dead targets per MD-47's record) still pass byte-identical

**Tests**: none new (rename)
**Gate**: build + full existing suite (scoped, per Gate Check Commands)

---

### T5: Rename `Span` → `PreprocessedSpan` in `piperine-lang-server`

**What**: Same rename across `symbol_index.rs` and any other file
importing `piperine_lang::error::Span`. `text_pos.rs`'s own internal
`Position`/byte-offset logic is untouched here (it doesn't import `Span`
today) — its resolve-boundary integration is T11-T13's job, not this
task's.
**Where**: `crates/piperine-lang-server/src/symbol_index.rs` (and any
other hit)
**Depends on**: T4
**Reuses**: N/A (rename only)
**Requirement**: N/A (mechanical prerequisite)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] `cargo build -p piperine-lang-server` is clean
- [x] `cargo test -p piperine-lang-server` — existing passing targets still pass byte-identical

**Tests**: none new (rename)
**Gate**: build + full existing suite

---

### T6: `Preprocessor::run` records provenance alongside its output

**What**: Change `Preprocessor::run`'s return type to
`Result<(String, SourceProvenance), ParseError>`. Every existing
`out.push_str(...)` call site (plain pass-through, `` `include ``
expansion in `handle_include`, macro-body substitution in the
rescan-expansion path — ~9 sites) gets a paired
`SourceProvenance::Builder::record(...)` call recording exactly the bytes
just pushed. No change to *what* gets pushed (existing directive
semantics untouched, per spec's stated scope).
**Where**: `crates/piperine-lang/src/preprocessor.rs`
**Depends on**: T2
**Reuses**: T2's `SourceProvenance`/builder
**Requirement**: SPANMAP-01, SPANMAP-02, SPANMAP-03

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [x] Every existing `crates/piperine-lang/tests/preprocessor.rs` test still passes with the new return shape (test call sites updated to destructure the tuple, no assertions changed) — 24/25 pass; `timescale_and_resetall_pass_through_as_directive_text` is a proven pre-existing failure (byte-identical at true HEAD via worktree A/B check), unrelated to this feature
- [x] New test: for a source with a plain include + a macro expansion, every byte in the returned flat buffer is covered by exactly one segment (no-gap invariant, design's Risks mitigation)
- [x] New test: the same file path `` `include ``d twice resolves both inclusions to the same `SourceId` (SPANMAP-03)
- [x] Gate check passes: `cargo test -p piperine-lang --test preprocessor`

**Tests**: unit/integration (extend `preprocessor.rs`, per matrix)
**Gate**: quick

---

### T7: Thread `SourceProvenance` through `Parser`/`SourceFile`/`Design`

**What**: `Parser::parse` carries the `SourceProvenance` T6's
`Preprocessor::run` now returns through to the returned `SourceFile`;
elaboration (`Elaborator::elaborate`) carries it onto `Design` as a
`#[serde(skip)]` field, mirroring the existing `flat_modules`
side-artifact precedent (design's Tech Decisions).
**Where**: `crates/piperine-lang/src/parser.rs`, `crates/piperine-lang/src/pom/design.rs` (or wherever `Design` is defined), `crates/piperine-lang/src/elab/mod.rs`
**Depends on**: T6
**Reuses**: `flat_modules`'s existing `#[serde(skip)]` pattern on `Design`
**Requirement**: SPANMAP-01, SPANMAP-02

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] `Design::source_provenance` field exists, populated on every successful `parse_and_elaborate*` call
- [ ] `Design`'s existing `serde` round-trip tests (`pom_serde.rs`/`pom_vams_shape.rs`) still pass unchanged (the field is skipped, not serialized — no round-trip diff)
- [ ] `cargo build -p piperine-lang` clean

**Tests**: covered by existing `Design` serde tests (no new test file needed — this task only threads a field, T2/T6 already cover the type's own behavior)
**Gate**: quick

---

### T8: Fix the render boundary — `lib.rs` uses resolved `Span`, not raw `input`

**What**: Replace both
`.with_source_code(input.to_string())` call sites in
`parse_and_elaborate_seeded_with_context` with logic that resolves the
failing error's `PreprocessedSpan` via the available `SourceProvenance`
(from `Preprocessor::run`'s output for a pure parse failure before a
`Design` exists; from `design.source_provenance` for an elaboration
failure) and attaches a `NamedSource` built from the *real* originating
file's own text plus the resolved local span — this is the fix for the
crash the whole feature exists to close.
**Where**: `crates/piperine-lang/src/lib.rs`
**Depends on**: T7
**Reuses**: T2's `Span::to_named_source_span` helper
**Requirement**: SPANMAP-01, SPANMAP-02, SPANMAP-03, SPANMAP-07

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New deterministic test (no env var, always runs): a short top-level file `` `include ``ing a header longer than itself, with an error inside the included content — asserts the rendered `miette::Report`'s source name/snippet is the *included* file's, and asserts no panic/`OutOfBounds` (this is the committed repro of the exact crash `vsine.vams` hit)
- [ ] New deterministic test: a file with zero directives still renders byte-identical to today's output (spec's stated zero-regression edge case)
- [ ] Extend `corpus.rs`'s opt-in `PIPERINE_VAMS_CORPUS` test: assert no failure message contains `OutOfBounds` and no test-process panic occurs across the full sweep (run manually against `/home/keslarek/Git/vams-corpus/sources` as the local verification evidence for this task, not committed as a hard CI dependency — matches the existing opt-in pattern)
- [ ] Gate check passes: `cargo test -p piperine-lang --test span_resolution` and `cargo test -p piperine-lang --test corpus`
- [ ] Manual verification: `cargo run -q -p piperine-cli -- check /home/keslarek/Git/vams-corpus/sources/gnucap-modelgen-vams/vsine.vams` no longer crashes/prints `OutOfBounds`

**Tests**: integration (new `span_resolution.rs` + extend `corpus.rs`, per matrix)
**Gate**: full

**Commit**: `fix(lang): resolve real file+line for `include`/macro-expanded diagnostics`

---

### T9: Record macro-expansion `ExpansionFrame`s (incl. argument spans)

**What**: When macro-body substitution writes expanded text into the
buffer (the rescan-expansion path in `preprocessor.rs`), the paired
`SourceProvenance::Builder::record(...)` call from T6 additionally
carries the macro's name, the invocation `PreprocessedSpan` (the
`` `NAME(...) `` text), the macro's own `` `define `` body
`PreprocessedSpan`, and — SPANMAP-09, user's explicit ask — each declared
parameter paired with the resolved `Span` of the actual argument text
passed at this invocation. This requires `scan_call_args` to additionally
return each argument's raw `(start, end)` offset (today it discards
position and keeps only the trimmed text) — `substitute_params` itself is
unchanged, only the recording side needs the offsets. `resolve()` builds
the `expansion_chain` from these frames; nested macro-of-macro expansion
carries the chain through correctly (each hop records its own immediate
invocation, `resolve()` walks all of them).
**Where**: `crates/piperine-lang/src/preprocessor.rs` (`scan_call_args`'s return type, the macro-expansion path), `crates/piperine-lang/src/source_provenance.rs` (`resolve()`'s chain-walking, if T2 stubbed it)
**Depends on**: T8
**Reuses**: T2's `ExpansionFrame`, T6's recording plumbing
**Requirement**: SPANMAP-04, SPANMAP-05, SPANMAP-06, SPANMAP-09

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New test: an error inside a single-level macro expansion resolves with a one-frame `expansion_chain` carrying the correct invocation and definition spans (SPANMAP-04/05)
- [ ] New test: an error inside a doubly-nested macro expansion (a macro invoking another macro) resolves with a two-frame chain, innermost first (SPANMAP-06)
- [ ] New test: an error in plain (non-macro) text has an empty `expansion_chain` (spec Edge Case — no noise on the common case)
- [ ] New test: a multi-argument macro invocation (e.g. a 4-param `` `MPRnb(name,default,units,desc)``-shaped macro) records `args` with the correct param name → argument-text `Span` pairing, and the argument's text is recoverable via `SourceProvenance::text()` at that span (SPANMAP-09)
- [ ] Gate check passes: `cargo test -p piperine-lang --test macro_expansion_span`

**Tests**: unit/integration (new `macro_expansion_span.rs`, per matrix)
**Gate**: quick

---

### T10: Render the dual-label diagnostic

**What**: Where `lib.rs` (T8) builds the `miette::Report`, when the
resolved `Span`'s `expansion_chain` is non-empty, attach the chain as
miette related/secondary labels ("expanded from here" pointing at each
frame's `definition`), primary label still at the invocation/error site,
per the rustc-backtrace convention the spec locked in.
**Where**: `crates/piperine-lang/src/lib.rs` (or wherever the `Report` is finally assembled — same site T8 touched)
**Depends on**: T9
**Reuses**: T8's render-boundary code, T9's `expansion_chain`
**Requirement**: SPANMAP-04, SPANMAP-05, SPANMAP-06

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New test: rendering a macro-expansion error's `Report` (via `format!("{:?}", report)` or miette's own rendered-string API, matching how `errors_lrm.rs`/`vams_deviation_resolutions.rs` already assert on rendered output) contains both the invocation location and an "expanded from here"-labeled definition location
- [ ] Gate check passes: `cargo test -p piperine-lang --test macro_expansion_span`

**Tests**: integration (extend `macro_expansion_span.rs`)
**Gate**: quick

---

### T11: `SourceProvenance::segments_covering` reverse lookup

**What**: The reverse query from the design's `SourceProvenance`
interface — given `(source_id, local_range)`, return every
`PreprocessedSpan` in the flat buffer whose segment covers that range
(needed so the LSP can go from "cursor position in the open document" to
"which AST-space span is under the cursor").
**Where**: `crates/piperine-lang/src/source_provenance.rs`
**Depends on**: T2 (can start any time after Foundation; sequenced last only because it's P3/lowest priority per spec)
**Reuses**: T2's segment table
**Requirement**: SPANMAP-08

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New test: a known `(source_id, local_offset)` inside a plain-text region returns the expected `PreprocessedSpan`(s)
- [ ] New test: a `(source_id, local_offset)` at a macro invocation site returns the span(s) of the resulting expansion
- [ ] Gate check passes: `cargo test -p piperine-lang --test source_provenance`

**Tests**: unit (extend `source_provenance.rs`)
**Gate**: quick

---

### T12: Wire LSP diagnostics to resolved `Span`

**What**: `piperine-lang-server`'s diagnostic-publishing path (wherever
it currently turns a `piperine_lang` error into an LSP `Diagnostic`)
resolves the `PreprocessedSpan` via `Design::source_provenance` before
building the `Range`; when the resolved `Span`'s `source_id` is not the
open document's own file, degrade to the clearly-labeled
"originates in `<path>`, not this document" diagnostic from spec P2
AC1(b) instead of guessing a wrong in-document range.
**Where**: `crates/piperine-lang-server/src/` (diagnostics-publishing module — locate via existing `diagnostics.rs` test's target)
**Depends on**: T5, T11
**Reuses**: T11's reverse lookup, T2's `Span`
**Requirement**: SPANMAP-08

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New test in `diagnostics.rs`: a diagnostic whose real cause is inside `` `include ``d content renders the "originates in `<path>`" degraded form, no crash, no wrong-line pointer into the open document
- [ ] Existing `diagnostics.rs` tests for plain (non-directive) files still pass unchanged
- [ ] Gate check passes: `cargo test -p piperine-lang-server --test diagnostics`

**Tests**: integration (extend `diagnostics.rs`, per matrix)
**Gate**: quick

---

### T13: Wire LSP hover/goto to resolved `Span`

**What**: `text_pos.rs`'s position↔offset conversion is unchanged (it
already works correctly for the open document's own coordinate space);
`hover.rs`/`symbol_index.rs`'s span-matching logic gains the
`resolve()`/`segments_covering()` calls needed so hover/goto still work
correctly when the AST node under the cursor was produced by
macro-expanded or `` `include ``d text.
**Where**: `crates/piperine-lang-server/src/{occurrences.rs,symbol_index.rs}` (per Design's Integration Points)
**Depends on**: T12
**Reuses**: T11's reverse lookup
**Requirement**: SPANMAP-08

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New test in `hover.rs`: hover over a macro invocation site resolves correctly (not into the macro's expanded-but-unrelated body text)
- [ ] Existing `hover.rs`/`symbol_resolution.rs` tests for plain files still pass unchanged
- [ ] Gate check passes: `cargo test -p piperine-lang-server --test hover` and `--test symbol_resolution`

**Tests**: integration (extend `hover.rs`, per matrix)
**Gate**: full (closes the feature — run the complete scoped suite across all three crates)

**Commit**: `feat(lang-server): resolve real positions for include/macro-expanded spans`

---

### T14: `piperine expand <file>` — default mode

**What**: New `piperine expand <file>` CLI command. Reuses
`super::utils::build_source_map()` (same as `check`/`build`), runs
`piperine_lang::Parser`'s preprocessing stage (`Preprocessor::run`) and
prints the resulting flat, fully-expanded text verbatim to stdout — the
Verilog-AMS-toolchain equivalent of `cargo expand`. Errors (unresolved
`` `include ``, malformed directive) print via the same typed-error path
`check` uses, not a parallel one.
**Where**: `crates/piperine-cli/src/commands/expand.rs` (new),
`crates/piperine-cli/src/lib.rs` (new `Commands::Expand` variant + dispatch)
**Depends on**: T8 (needs the render-boundary error path already fixed, so `expand`'s own errors resolve correctly too)
**Reuses**: `commands::utils::build_source_map()`, `check.rs`'s error-printing pattern
**Requirement**: SPANMAP-10

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New test: `piperine expand` on a file with zero directives prints output byte-identical to the input file (SPANMAP-10 AC4)
- [ ] New test: `piperine expand` on a file with `` `include ``s/`` `define ``s prints the fully resolved/expanded text
- [ ] New test: `piperine expand` on a file with an unresolvable `` `include `` reports the same typed error `piperine check` would (SPANMAP-10 AC3)
- [ ] Gate check passes: `cargo test -p piperine-cli --test expand_cmd` (new file, matching the existing `check_cmd.rs`/`build_cmd.rs` convention)

**Tests**: integration (new `crates/piperine-cli/tests/expand_cmd.rs`, matching `check_cmd.rs`'s pattern)
**Gate**: quick

---

### T15: `piperine expand --trace` — macro annotation mode

**What**: Adds the `--trace`/`-v` flag from the design: walks
`SourceProvenance`'s segments in buffer order, and at each segment
boundary whose `expanded_from` is `Some`, prints an annotation line
(macro name, `param=value` pairs from `ExpansionFrame::args` resolved via
`SourceProvenance::text()`, and the invocation's real file:line) before
the expanded text it produced — the debug view the user explicitly asked
for ("os argumentos da expansion para poder debugar").
**Where**: `crates/piperine-cli/src/commands/expand.rs` (extend T14)
**Depends on**: T14, T9 (needs `ExpansionFrame::args` to exist)
**Reuses**: T9's `ExpansionFrame`, T2's `SourceProvenance::text()`
**Requirement**: SPANMAP-11, SPANMAP-09 (surfaced)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] New test: `piperine expand --trace` on a multi-argument macro invocation shows the macro name, every `param=value` pair, and the real invocation file:line in the annotation
- [ ] New test: `piperine expand --trace` on a plain (non-macro) file shows no annotations (matches the "no noise on the common case" convention from T9)
- [ ] Gate check passes: `cargo test -p piperine-cli --test expand_cmd`

**Tests**: integration (extend `expand_cmd.rs`)
**Gate**: full (closes Phase 6)

**Commit**: `feat(cli): add \`piperine expand\` for previewing preprocessed output`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6

Phase 1:  T1 ──→ T2
Phase 2:  T3 ──→ T4 ──→ T5
Phase 3:  T6 ──→ T7 ──→ T8
Phase 4:  T9 ──→ T10
Phase 5:  T11 ──→ T12 ──→ T13
Phase 6:  T14 ──→ T15
```

Execution is strictly sequential — there is no intra-phase parallelism.
15 tasks total → 2 batches at the ~7-task budget (Phases 1-3 = 8 tasks,
Phases 4-6 = 7 tasks), packed on phase boundaries — offered to the user
at Execute time, not assumed here.

---

## Task Granularity Check

| Task | Scope | Status |
|---|---|---|
| T1: Define new span/provenance types | 1 new module, 3 types | ✅ Granular |
| T2: `SourceProvenance` registry + `resolve()` | 1 module, 1 cohesive component | ✅ Granular |
| T3: Rename in `piperine-lang` | 1 mechanical transformation, compiler-verified | ✅ Granular (mechanical, not logic) |
| T4: Rename in `piperine-codegen` | 1 mechanical transformation | ✅ Granular |
| T5: Rename in `piperine-lang-server` | 1 mechanical transformation | ✅ Granular |
| T6: Preprocessor provenance recording | 1 file, 1 cohesive change (paired recording at existing splice sites) | ✅ Granular |
| T7: Thread `SourceProvenance` onto `Design` | 1 field, 3 files it passes through | ✅ Granular |
| T8: Fix `lib.rs` render boundary | 1 file, 2 call sites, the feature's core deliverable | ✅ Granular |
| T9: Record `ExpansionFrame`s | 1 file (macro-expansion path specifically) | ✅ Granular |
| T10: Dual-label diagnostic rendering | 1 file, 1 rendering change | ✅ Granular |
| T11: Reverse lookup | 1 method, 1 file | ✅ Granular |
| T12: LSP diagnostics wiring | 1 module | ✅ Granular |
| T13: LSP hover/goto wiring | 2 related files, 1 cohesive concern (position resolution) | ✅ Granular (2-3 related things in same concern = OK) |
| T14: `piperine expand` default mode | 1 new file, 1 CLI command | ✅ Granular |
| T15: `piperine expand --trace` | 1 file (extend T14), 1 new flag | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
|---|---|---|---|
| T1 | None | (start of Phase 1) | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | (start of Phase 2, after Phase 1) | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | T2 | (start of Phase 3, after Phase 1 — T2 is in Phase 1) | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T8 | (start of Phase 4, after Phase 3) | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |
| T11 | T2 | (start of Phase 5 — cross-phase dep on T2, sequenced late by priority not by data dependency) | ✅ Match (dependency points backward across phases, which is allowed — only forward-pointing deps are disallowed) |
| T12 | T5, T11 | (start of Phase 5, after T11) | ✅ Match |
| T13 | T12 | T12 → T13 | ✅ Match |
| T14 | T8 | (start of Phase 6, after Phase 3 — cross-phase dep, backward-pointing, allowed) | ✅ Match |
| T15 | T14, T9 | T14 → T15 (T9 dep is cross-phase backward, allowed) | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
|---|---|---|---|---|
| T1 | New types (data shape only) | none (exercised by T2) | none | ✅ OK |
| T2 | `SourceProvenance`/`Span` domain logic | unit/integration | unit/integration | ✅ OK |
| T3 | Rename (`piperine-lang`) | none (build gate) | none | ✅ OK |
| T4 | Rename (`piperine-codegen`) | none (build gate) | none | ✅ OK |
| T5 | Rename (`piperine-lang-server`) | none (build gate) | none | ✅ OK |
| T6 | Preprocessor provenance recording | unit/integration | unit/integration | ✅ OK |
| T7 | `Design` field threading | covered by existing serde tests | (references existing tests, not deferral — the field itself has no new logic beyond what T2/T6 already test) | ✅ OK |
| T8 | Render boundary | integration | integration | ✅ OK |
| T9 | `ExpansionFrame` recording | unit/integration | unit/integration | ✅ OK |
| T10 | Dual-label rendering | integration | integration | ✅ OK |
| T11 | Reverse lookup | unit | unit | ✅ OK |
| T12 | LSP diagnostics wiring | integration | integration | ✅ OK |
| T13 | LSP hover/goto wiring | integration | integration | ✅ OK |
| T14 | `piperine expand` default mode | integration | integration | ✅ OK |
| T15 | `piperine expand --trace` | integration | integration | ✅ OK |

No ❌ violations. T7's "covered by existing tests" is not test deferral
in the anti-pattern sense the rule warns about: T7 adds a plain
pass-through field with no new logic of its own (the logic it carries
was already fully tested by T2/T6) — the existing `Design` serde tests
are the correct regression gate for "does adding this field break
anything," which is the only new risk T7 introduces.
