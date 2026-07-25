# Language Server Follow-up Bugs Specification

> Scope: Large. Four confirmed regressions/gaps in the just-shipped
> `language-server` feature (`.specs/features/language-server/`, 23/23 tasks
> DONE, Verifier PASS — these slipped the Verifier's 8-of-26 sample). Found
> via code audit + empirical repro tests (see each story's Root Cause).
> Reported by the user testing the VS Code extension directly.

## Problem Statement

`ddt`/`Real`/every `extern` name doesn't goto-definition or show hover docs;
labeled instances don't highlight correctly; completion suggests
behavior-only keywords (`for`) at true top level. All four block real IDE
use of the language server just shipped.

## Goals

- [ ] goto-definition works for every `extern` (type/operator/fn/attribute)
      name, jumping to its real declaring file (a stdlib header today).
- [ ] hover shows `///` docs for `extern` declarations, same as it already
      does for `mod`/`param`/etc.
- [ ] document-highlight/goto on a labeled instance targets the clicked
      token (label or type name), not the whole multi-line statement.
- [ ] completion never offers behavior-only statement keywords (`for`) when
      the cursor sits at a module's closing `}`.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Fully context-aware predictive-parser fix for BUG-4 | The parser's cursor/backtrack machinery (`piperine-lang/src/parse/`, hand-written — CLAUDE.md: "edit with care") would need real block-type tracking to know precisely *which* block is closing at any given `}`. The completion-layer heuristic in this spec fixes the reported symptom (true top-level) without touching that machinery. |
| Per-occurrence span for `Expr::Ident` (in-expression uses) | Pre-existing SPEC_DEVIATION from `language-server`'s T5/T8 — still out of scope here; not what these 4 bugs are about. |
| Doc-commenting every stdlib header exhaustively | BUG-2 requires enough `///` content to prove the pipeline (ddt + a few others); a full doc-comment pass over every header is a separate, larger content task. |
| `extern` decls declared inside a user's own project file (not a header) | Real, but the reported bugs are specifically about the *stdlib* surface; the fix (file-path tracking, span capture) is general and will also cover this case for free once landed — no separate work needed. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| BUG-1: how to give embedded (`include_str!`) headers a real path for goto | Hardcode the real on-disk path via `concat!(env!("CARGO_MANIFEST_DIR"), "/headers/X.phdl")` for the 5 `prelude_items()`-embedded files (types/math/tasks/operators/introspection.phdl) — same pattern `piperine-project/src/source_map.rs` already uses for its own header fallback. Valid because this is a source checkout running locally, not (yet) a distributed binary shipped without its source tree — matches the project's current pre-V1/dev-tool stage. | Simplest correct fix for the actual deployment shape today; documented as a SPEC_DEVIATION if/when the server ships standalone (a future concern, not a live requirement) | y (self-resolved from `piperine-project`'s existing precedent) |
| BUG-1: file path for `use spice::...`/`use piperine::...`-loaded externs | Use the real `file_path` already computed in `Resolver::load_source` — these already read from disk, just need the path threaded through | Reuses existing, already-correct file resolution — no new fallback needed | y |
| BUG-2: which headers get `///` authored in this pass | `headers/operators.phdl` (at least `ddt`) and `headers/types.phdl` (the primitive types) — the two the user actually hit | Proves the pipeline end-to-end on the exact reported case; exhaustive header docs are Out of Scope | y (user's own repro) |
| BUG-3: which span `Resolution`/`ResolutionIndex` index by for a labeled instance | The token span matching `i.name()`'s convention (label if present, else module type) — i.e. index by `label_span` when labeled, `type_span` when not — so `resolve_at` and `ResolutionIndex`'s indexing agree byte-for-byte (required for `occurrences_for_decl_span`'s exact-match `.find()` to succeed) | Keeps the existing `i.name()`-based binding-identity convention; the two sides must match exactly or occurrences silently break again | y (derived from confirmed root cause) |
| BUG-4: suppression heuristic scope | Suppress behavior-only keywords (`for`/`match`/`return`/`when`; keep `if`/`var` since those are common to both module- and behavior-body — see AC) only when `Punctuation(RBrace)` **and** a module-body-only keyword (`param`/`wire`) are *both* present in the same `expected` list (the repro's exact signature: mixed block-type ambiguity is itself the "we don't know, likely a block boundary" signal) | A principled fix needs block-type tracking the parser doesn't have; this heuristic is scoped to the exact confirmed repro, not a general claim about every `}` position | y (derived from repro; documented as a known-heuristic SPEC_DEVIATION) |

**Open questions:** none — all resolved above.

---

## User Stories

### P1: BUG-1 — goto-definition works for `extern` names

**User Story**: As a PHDL author, I want to Ctrl+click `ddt`/`Real`/any
`extern`-declared name and land on its real declaration, so that I can read
its exact signature the same way I already can for `mod`/`param`/etc.

**Root Cause**: `goto_def.rs::cross_file_location` only handles
`SymbolKind::Module`/`Instance`. `Type`/`Operator`/`Function`/`AttrSchema`
resolutions fall through to the same-file fallback (`goto_def.rs::handle`),
which applies the header file's byte-offset `decl_span` to the *current*
document's text — wrong file, so it lands on nonsense or nothing.
`ddt`/`Real`/etc. are declared in `crates/piperine-lang/headers/
operators.phdl`/`types.phdl`, embedded via `include_str!` in
`Resolver::prelude_items()` — no on-disk path is tracked anywhere for them
today (confirmed by reading `resolve.rs`, `symbol_index.rs`, `goto_def.rs`).

**Acceptance Criteria**:

1. WHEN a user requests goto-definition on `ddt` (or any name declared via
   `extern operator`/`extern type`/`extern fn`/`extern task`/
   `extern attribute`/`extern impl`) THEN the server SHALL return a
   `Location` whose URI is the real declaring file (a stdlib header) and
   whose range covers that declaration's own span in that file's text.
2. WHEN the extern declaration came from a `use`-imported package (e.g.
   `use spice::diode;`) THEN goto-definition SHALL still resolve to that
   real on-disk file — not just the 5 embedded prelude headers.
3. WHEN the extern name is declared in the *current* document (a user
   writing their own `extern` stub, e.g. a plugin's `extern.phdl`) THEN
   goto-definition SHALL still work as it does today (no regression).

**Independent Test**: goto on `ddt` inside any PHDL analog body opens
`crates/piperine-lang/headers/operators.phdl` at the `extern operator ddt`
line.

---

### P2: BUG-2 — hover shows `///` docs for `extern` declarations

**User Story**: As a PHDL author, I want hovering `ddt`/`Real`/any
`extern`-declared name to show its `///` doc comment, so hover is
consistently useful across the whole declared-language surface, not just
user-authored `mod`/`param` decls.

**Root Cause**: `ast::ExternSig`/`ExternDecl` carry no `doc` field at all —
T1-T3 of `language-server` only wired doc-comment capture into POM-level
decls (module/port/param/wire/instance/behavior). `symbol_index.rs`'s
extern-registry `Resolution` arms (`Type`/`Operator`/`Function`/
`AttrSchema`, ~lines 279-310) hardcode `doc: None`. Secondary: today's
headers use plain `//`, not `///`, on `ddt`/`Real` — even a fixed pipeline
has nothing to show until headers are authored with `///`.

**Acceptance Criteria**:

1. WHEN an `extern` declaration (any of the 6 forms) is preceded by a `///`
   run THEN hover on its use-site SHALL render that doc as Markdown, same
   rendering convention as an already-documented `mod`/`param`.
2. WHEN an `extern` declaration has no `///` run THEN hover SHALL render
   unchanged from today (no doc paragraph) — no regression.
3. WHEN a user hovers `ddt` (`headers/operators.phdl`) THEN the doc
   authored in this pass SHALL appear — the concrete, spec-anchored proof
   the pipeline works end-to-end, not just a synthetic fixture.

**Independent Test**: hover on `ddt` shows its authored `///` doc as
Markdown above the signature line.

---

### P3: BUG-3 — document-highlight/goto targets the clicked token on a labeled instance

**User Story**: As a PHDL author, I want document-highlight (and
goto/hover) on a labeled instance's label or type name to target just that
word, so the editor doesn't visually mark the entire multi-line
instantiation as if the whole statement were "the symbol."

**Root Cause**: Confirmed empirically — for
`src : RampSource(.p = vin, .n = gnd) { .slope = 4.0e5 };`, `resolve_at` on
either `"src"` or `"RampSource"` returns `Resolution.decl_span` covering the
entire 56-byte statement (offset 454, length 56). `occurrences_at` then
returns that same over-broad range as the sole highlight.
`pom::Instance`/`ast::ModuleStatement::Instance` (`parser/stmt.rs`) capture
only one span for the whole statement — no label-token or type-name-token
span exists. Confirmed: unlabeled instances have the *same* over-broad span
but it happens to start at the type name, so it looks less obviously wrong;
user confirms labeled instances visibly "mark the whole instance" while
unlabeled "work" (same bug, different visual symptom).

**Acceptance Criteria**:

1. WHEN a user clicks a labeled instance's **label** (e.g. `src`) THEN
   document-highlight SHALL return a range covering only the label token,
   not the whole instance statement.
2. WHEN a user clicks a labeled instance's **type name** (e.g.
   `RampSource`) THEN document-highlight SHALL return a range covering only
   the type-name token, not the whole instance statement.
3. WHEN a user clicks an **unlabeled** instance's type name THEN behavior
   SHALL be unchanged or improved (still resolves; span now tight to the
   type-name token instead of the whole statement) — no regression.
4. WHEN goto-definition is requested from either a label or type-name click
   THEN it SHALL continue to resolve the same *target* as before (this bug
   is about the highlighted/returned **range**, not about breaking
   resolution correctness already covered by `language-server`'s T7/T13).

**Independent Test**: for the exact reported fixture (`src :
RampSource(...) { ... };`), highlighting `"src"` returns a 3-byte range, not
a 56-byte range.

---

### P4: BUG-4 — completion doesn't suggest behavior-only keywords at true top level

**User Story**: As a PHDL author, I want completion right after closing a
module (`}`) to offer top-level declarations, not statement keywords like
`for` that were only ever valid *inside* a body — those belong to a removed
concept (the old bench `for`) in this exact position and are simply wrong
here regardless.

**Root Cause**: Confirmed empirically via `predict_at_cursor`: cursor
exactly at a module's closing `}` byte position returns `expected =
[Punctuation(RBrace), Keyword("param"), Keyword("wire"), Keyword("var"),
Keyword("for"), Keyword("if"), Ident(VariableName)]` — every alternative
the mod-body statement dispatcher tried before finding the `}`, because
`check_cursor()` (`parser/mod.rs`) intercepts `peek()` and returns `None`
the instant the cursor touches that token's boundary; every subsequent
`eat_ident`/`eat` attempt for that same token then *also* intercepts and
records its own `expected()` entry, snowballing into every possible
body-statement continuation with no signal that the block is actually
closing. True top-level (cursor separated from the previous `}` by
whitespace) correctly returns an empty list. The bug is specific to
cursor-immediately-after-a-closing-brace.

**Acceptance Criteria**:

1. WHEN the cursor sits immediately after a module's closing `}` AND the
   parser's `expected` list contains both `Punctuation(RBrace)` and a
   module-body-only keyword (`param`/`wire`) THEN completion SHALL NOT
   include behavior-only keywords (`for`/`match`/`return`/`when`) — those
   are never valid at this position regardless of which block is actually
   closing.
2. WHEN the same condition in AC1 holds THEN completion SHALL include the
   legitimate top-level declaration keywords (`mod`/`fn`/`discipline`/
   `bundle`/`enum`/`capability`/`impl`/`use`/`const`) as real candidates —
   not just suppress the wrong ones.
3. WHEN the cursor is genuinely inside a behavior body (`analog`/`digital`)
   mid-statement (not at a `}` boundary) THEN `for`/`match`/etc. SHALL
   still be offered — no regression to the legitimate case.

**Independent Test**: completion right after `mod A(...) { param r: Real =
1.0; }` (cursor at the closing `}`) never includes `"for"` in the returned
labels, and does include `"mod"`.

---

## Edge Cases

- WHEN an `extern` name has no textual declaration at all (a native-only
  registry entry, e.g. the built-in `rfport` schema per `language-server`'s
  T20 SPEC_DEVIATION) THEN goto-definition SHALL decline (no `Location`)
  rather than fabricate one — matches the existing precedent.
- WHEN two different files declare the same extern name (a project
  shadowing a stdlib header, e.g. the `spice` package-name-collision rule
  in `CLAUDE.md`) THEN goto resolves to whichever declaration actually won
  during elaboration (the same one the compiler uses) — not an arbitrary one.
- WHEN a `///` run precedes an `extern impl` block but not an individual
  method inside it THEN only the block-level doc SHALL attach — matches the
  same "attach to the immediately-following declaration" rule already used
  elsewhere (no per-method doc unless authored per-method).

---

## Requirement Traceability

| Requirement ID | Story | Status |
|---|---|---|
| LSB-01 | P1 (BUG-1) AC1 | Done (T3, commit `4b4a5ae`) |
| LSB-02 | P1 (BUG-1) AC2 | Done (T1, commit `96a0b1f`) |
| LSB-03 | P1 (BUG-1) AC3 | Done (T3, commit `4b4a5ae`) |
| LSB-04 | P2 (BUG-2) AC1 | Done (T5, commit `623c7e5`) |
| LSB-05 | P2 (BUG-2) AC2 | Done (T5, commit `623c7e5`) |
| LSB-06 | P2 (BUG-2) AC3 | Pending (T6 — real ddt/Real header docs not authored in this batch) |
| LSB-07 | P3 (BUG-3) AC1 | Pending |
| LSB-08 | P3 (BUG-3) AC2 | Pending |
| LSB-09 | P3 (BUG-3) AC3 | Pending |
| LSB-10 | P3 (BUG-3) AC4 | Pending |
| LSB-11 | P4 (BUG-4) AC1 | Pending |
| LSB-12 | P4 (BUG-4) AC2 | Pending |
| LSB-13 | P4 (BUG-4) AC3 | Pending |

**ID format:** `LSB-NN`. **Coverage:** 13 total, 0 mapped to tasks yet.

---

## Success Criteria

- [ ] goto on `ddt` opens `headers/operators.phdl` at the right line.
- [ ] hover on `ddt` shows its authored `///` doc.
- [ ] highlight on a labeled instance's label/type targets only that token.
- [ ] completion right after a module's `}` never offers `for`.
- [ ] `cargo test --workspace` green, zero new warnings.
