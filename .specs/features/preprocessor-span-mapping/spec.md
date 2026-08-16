# Preprocessor Span-Mapping Specification

## Problem Statement

`piperine-lang`'s preprocessor (`crates/piperine-lang/src/preprocessor.rs`)
runs LRM ch. 10 directives (`` `include ``, `` `define ``/macro expansion,
`` `ifdef ``/conditional compilation) by textual substitution: it consumes
the raw source string and returns one flat expanded `String`. Every `Span`
minted afterward (lexer, grammar, AST, elaboration errors) is a `{start,
end}` byte offset **into that flat expanded string** — it carries no
notion of which original file, or which macro invocation, those bytes
actually came from.

Every consumer that turns a `Span` into something a human reads assumes
the opposite — that `Span` offsets index directly into the **original,
unexpanded top-level file's own text**:

- `piperine-lang::lib.rs` (`parse_and_elaborate_seeded_with_context`)
  calls `.with_source_code(input.to_string())` — `input` is the raw
  top-level file, always, regardless of what actually got preprocessed.
- `piperine-lang-server::text_pos.rs` converts LSP `Position`↔byte-offset
  against the open document's own buffer.

The mismatch is silent and gradient: for a file with no directives, the
two texts are identical and nothing breaks. The moment any `` `include ``
or `` `define `` runs, offsets diverge from the original file's length and
content. Small divergences (comment stripping, short macro bodies)
produce **wrong but in-bounds** snippets — a diagnostic that points at the
wrong line, silently. Larger divergences (an `` `include ``d header is
longer than the file that includes it, the overwhelmingly common case)
push the offset **past the original file's length entirely** — the miette
renderer panics/reports `OutOfBounds` instead of showing anything.

Found via a real-world Verilog-AMS corpus sweep
(`/home/keslarek/Git/vams-corpus/REPORT.md`, "Diagnostics span-offset
crash" finding) — confirmed reproducible on files as short as a 5-line
module with two `` `include ``s.

This is not a rendering nit: any file using `` `include ``/`` `define ``
(the overwhelming majority of real Verilog-A — Annex D's own
`disciplines.vams`/`constants.vams` headers are `` `include ``d by nearly
every real design) gets diagnostics that range from mislocated to
crash-on-render, in both the CLI (`piperine check`) and the LSP server
(hover, go-to-definition, diagnostics — anywhere `text_pos.rs` or
`symbol_index.rs` resolves a `Span` against a document).

## Goals

- [ ] Every `Span` reaching a human-facing surface (CLI diagnostic, LSP
      diagnostic/hover/goto) resolves to the real originating file and a
      byte range that is valid (in-bounds) within that file's own text —
      zero `OutOfBounds`/panic renders, on any file in the
      `vams-corpus` sweep or the existing test corpus.
- [ ] A `Span` that falls inside macro-expanded text reports **both** the
      macro invocation site (primary location — what the module author
      wrote and can act on) **and** the macro definition site (secondary
      "expanded from here" note — for whoever debugs the macro itself),
      matching the rustc macro-backtrace convention.
- [ ] A `Span` that falls inside `` `include ``d content reports the
      included file's own path and local offset directly — no
      "somewhere in the top-level file" fallback.
- [ ] `Span` correctness holds from the moment a `Span` is minted (right
      after lexing) through grammar, AST, elaboration, and every existing
      consumer crate (`piperine-lang-server`, `piperine-codegen`) — not
      only at a final render-time patch-up. (User decision, see
      Assumptions: `Span` is not a purely internal flat-offset type
      anymone; every consumer resolving a `Span` to human-readable
      location goes through one shared, always-available resolution
      path, and that path is populated as the `Span` is created, not
      reconstructed after the fact from string-length heuristics.)
- [ ] Existing preprocessor *behavior* (what expands to what, conditional
      compilation semantics, macro rescan rules) is unchanged — this is a
      provenance/position-tracking retrofit, not a semantics rewrite.
- [ ] A macro invocation's actual argument text is recoverable from its
      `ExpansionFrame`, not just the invocation site — so a debugging
      session can see *what value* a macro parameter got, not only
      *where* the call happened (user's explicit ask, added after initial
      spec confirmation — same rationale as rustc's macro-backtrace
      showing the invocation's tokens).
- [ ] A new `piperine expand <file>` CLI command exists, showing the
      fully preprocessed (`` `include ``/`` `define ``-expanded) text a
      designer or `piperine-lang` developer can read directly — the
      Verilog-AMS-toolchain equivalent of `cargo expand`/
      `rustc -Zunpretty=expanded` (user's explicit ask). This command is
      developer/designer-facing debug tooling, not part of the compile
      pipeline itself.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---|---|
| New preprocessor *semantics* (new directives, changed macro-expansion rules) | This feature only adds position tracking to the existing, LRM-ch.10-scoped behavior; semantic changes are a separate feature |
| Fixing the other corpus-sweep gaps (`` `define attr(txt) (*txt*)`` idiom, unrecognized attribute schemas, residual Annex F cases) | Tracked separately in `/home/keslarek/Git/vams-corpus/REPORT.md`; unrelated to span provenance |
| `piperine-codegen`'s own `Span` usage beyond making sure it keeps compiling and its own error-reporting boundary resolves correctly | Codegen consumes already-elaborated POM spans; no new codegen-side position-tracking logic is being added, only the resolve-at-report-boundary call this feature introduces |
| Column/line caching or incremental re-lex performance work | Out of scope; this is a correctness fix, not a performance feature |
| Rewriting `lexer.rs`/`grammar/*.rs` internals beyond what's mechanically required by the `Span` shape change | Both are flagged in `CLAUDE.md` as sensitive, ripple-prone files — the design (next phase) must minimize touched call sites there, not restructure them |

---

## Assumptions & Open Questions

Every ambiguity is resolved or recorded here — nothing is left silently
unclear.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Macro-expansion error attribution | Report **both**: primary label at the invocation site, secondary "expanded from here" label at the macro's own `` `define `` body site | User explicitly chose this (rustc-style) over invocation-only or definition-only, accepting the extra implementation cost (multi-label miette diagnostic, an expansion-chain carried per `Span`) | y |
| `Span` scope of change | `Span` itself becomes resolvable to real file+offset **from the moment it is minted**, not only at a final render boundary; this is a first-class property of every `Span`, propagating through `piperine-lang-server` and `piperine-codegen`, not a bolt-on translation table only two crates ever call | User explicitly chose this over a narrower "resolve only at the CLI/LSP boundary" option, accepting wider blast radius, in exchange for every crate having one consistent, always-correct way to ask "where did this Span really come from" | y |
| How "resolvable from the moment it's minted" is achieved at the `Span` *type* level (does the struct itself gain a `source_id` field vs. stay `{start,end}` over a single global position space with a universal resolver) | Left to Design phase — both satisfy the goal above; Design phase picks based on blast-radius measurement (grammar/lexer already have 69 `Span::new(` call sites; `piperine-codegen` has 10 files touching `Span`) and presents the chosen mechanism with its call-site cost before implementation starts | Sizing this precisely needs the call-site inventory Design phase produces; deferring the concrete type shape avoids locking in a shape that turns out to force an unnecessarily large mechanical rewrite of hand-written parser code `CLAUDE.md` flags as sensitive | y (deferred to Design, not silently dropped) |
| Included-file re-inclusion (the same path `` `include ``d twice, e.g. guarded by `` `ifndef ``) | Same file path resolves to the same stable identity every time it's included in one parse run — a `Span` inside the second inclusion's (skipped, if guarded) content is still resolvable to that file, not a second synthetic copy | Matches how every C-family preprocessor and rustc's own `SourceMap` treat repeated `#include`s of the same file: one canonical file identity, many possible spans into it | y |
| A macro invocation whose expanded body itself invokes another macro (nested expansion) | The "expanded from here" chain supports more than one level — each expansion step remembers its own invocation site, so a doubly-nested macro error can show a 2-deep (or N-deep) chain, not just one hop | Falls out naturally from choosing "both" above; LRM macros can call other macros (CPP-style rescan is explicitly in the preprocessor's stated scope), so a single-level-only chain would silently truncate real cases | y |
| Behavior on a `Span` that straddles two different original files (e.g. an unterminated construct that starts before an `` `include `` and is only detected as unterminated after it) | Report using the **start** position's originating file or the identity as EOF is reached (mirrors how "unterminated X" errors already report by construction start, not end) — an explicit edge case, not silently undefined | Existing precedent: `ParseErrorKind`'s several "unterminated ..." variants already anchor on the construct's start; span-mapping doesn't change that convention, it only makes the *file* part of that anchor correct too | y |
| Existing public API stability of `piperine_lang::error::Span` and `piperine_lang::SourceMap` | A breaking change to `Span`'s shape is accepted for this feature (see the scope decision above); `SourceMap` (the existing include-namespace resolver, `source_map.rs`) is a different, unrelated type from whatever new source-identity registry this feature introduces — naming for the new type is a Design-phase decision that must not collide with the existing `SourceMap` name | Flagging now so Design doesn't accidentally shadow/confuse the two — `piperine_lang::SourceMap` today only maps *namespaces to search-path directories* for `` `include`` resolution; it has no concept of "the text of file X" or "position N in file X" | y |

**Open questions:** none — all resolved or logged above.

---

## User Stories

### P1: Correct file+line on any `` `include ``-touched diagnostic ⭐ MVP

**User Story**: As someone running `piperine check design.va` on a real
Verilog-A file that includes `disciplines.vams`/`constants.vams` (nearly
every real design), I want a parse or elaboration error to show *my*
file's real line number and a valid source snippet, so that I can fix the
actual mistake instead of hitting a crash or a mislocated pointer.

**Why P1**: This is the crash — `OutOfBounds` — the corpus sweep actually
hit. Nothing else in this feature matters if this case isn't fixed.

**Acceptance Criteria**:

1. WHEN a parse or elaboration error's `Span` falls inside text that
   originated from the top-level file (nothing expanded) THEN the CLI
   SHALL render the same file name, line, and snippet it renders today
   (no regression on the already-correct case).
2. WHEN a parse or elaboration error's `Span` falls inside text that
   originated from an `` `include ``d file THEN the CLI SHALL render that
   included file's own path and a byte range valid within that file's own
   text — never the top-level file's text, never an out-of-bounds slice.
3. WHEN the included file is longer than the top-level file that includes
   it (pushing the flat offset past the top-level file's own byte length)
   THEN the CLI SHALL still render a valid, correctly-located snippet —
   no panic, no `OutOfBounds` message.
4. WHEN the same file is `` `include ``d more than once in one parse run
   THEN every `Span` inside either inclusion SHALL resolve to that one
   file's identity and the correct local offset for that occurrence.

**Independent Test**: Run `piperine check` against
`/home/keslarek/Git/vams-corpus/sources/gnucap-modelgen-vams/vsine.vams`
(the file that reproduced the `OutOfBounds` crash) and confirm a valid,
correctly-located diagnostic renders instead of a crash.

---

### P2: Macro-expansion dual attribution

**User Story**: As a module author who invoked a macro that expands into
something invalid, I want the error to point at *my* invocation first, so
I know which line in my own file to look at without having to understand
the macro's internals.

**Why P2**: Real (not MVP-blocking, since P1 already makes this
non-crashing/resolvable to *a* correct location) but the dual-attribution
UX is what the user explicitly asked for and materially improves
debuggability for the `` `define attr(txt) (*txt*)`` -style macros the
corpus sweep found are common.

**Acceptance Criteria**:

1. WHEN a `Span` falls inside text produced by macro expansion THEN the
   diagnostic SHALL show a primary label at the macro invocation site
   (the `` `NAME(...) `` the author wrote).
2. WHEN a `Span` falls inside text produced by macro expansion THEN the
   diagnostic SHALL additionally show a secondary "expanded from here"
   label at the macro's own `` `define `` body location.
3. WHEN a macro invocation's expansion itself invokes another macro
   (nested expansion) and the error is inside the innermost expansion
   THEN the diagnostic SHALL show the full chain (innermost definition →
   … → outermost invocation), not just one hop.
4. WHEN a `Span` falls inside plain (non-macro, non-include) source text
   THEN no expansion-chain label SHALL be shown (no noise on the common
   case).

**Independent Test**: Craft a `` `define `` whose body is syntactically
invalid when expanded with certain arguments (e.g. the fixed
`scan_call_args` comma-in-string case from the earlier conformance pass,
inverted to reintroduce a real error) and confirm both labels render with
the right locations.

---

### P3: LSP server position resolution matches the CLI

**User Story**: As an editor user with the piperine LSP server active on
a file with `` `include ``s/macros, I want hover, go-to-definition, and
inline diagnostics to point at the right place in *my open document*, not
silently drift or throw once macros/includes are involved.

**Why P3**: Same underlying bug class as P1, in a second consumer
(`piperine-lang-server`), but not part of the CLI-focused corpus sweep
that motivated this feature — real, but validated after the CLI path is
solid.

**Acceptance Criteria**:

1. WHEN a diagnostic's `Span` originates from `` `include ``d content
   THEN the LSP SHALL either (a) locate it correctly if the client has
   the included file open, or (b) degrade to a clearly-labeled
   "originates in `<path>`, not this document" diagnostic — never a wrong
   in-document line or a crash.
2. WHEN hover/go-to-definition is requested at a position inside the
   *open document's own* (non-expanded) text THEN behavior SHALL be
   unchanged from today.

**Independent Test**: Open a `.vams` file with an `` `include`` in the
LSP test harness, trigger a diagnostic whose real cause is inside the
included file, confirm no crash and a correctly-labeled location.

---

### P2.5: `piperine expand` — see the expanded text and its macro args

**User Story**: As a designer or `piperine-lang` developer debugging why
a macro-heavy `.va` file misbehaves, I want a command that shows me the
fully expanded source (what the parser actually sees, `` `include ``s
and `` `define ``s resolved) and, for any macro invocation, the argument
values that were substituted — so I can debug expansion problems
directly instead of mentally simulating the preprocessor.

**Why P2.5**: Explicit user ask, added after the initial spec round.
Independent of P1/P2's diagnostic-correctness goal (this is a new debug
surface, not a fix to an existing one) but reuses the exact same
`SourceProvenance`/`ExpansionFrame` machinery P2 builds — sequenced right
after it rather than as a separate feature.

**Acceptance Criteria**:

1. WHEN a designer runs `piperine expand <file>` THEN the CLI SHALL print
   the fully preprocessed text (every `` `include `` resolved, every
   `` `define `` invocation expanded, conditional-compilation branches
   resolved) exactly as the lexer/grammar see it today.
2. WHEN `piperine expand <file>` is run with a verbose/trace flag THEN
   the output SHALL additionally annotate each macro-expanded region with
   the macro name, its argument list (name=value pairs, from the
   `ExpansionFrame` this feature's core work already records), and the
   invocation's real source location.
3. WHEN the target file (or something it `` `include ``s) fails to
   resolve THEN `piperine expand` SHALL report the same typed error
   `piperine check` would (reusing existing error plumbing, not a
   parallel error path).
4. WHEN the file has no directives at all THEN `piperine expand`'s output
   SHALL be byte-identical to the input file.

**Independent Test**: Run `piperine expand` against a file with a
multi-argument macro (e.g. the corpus's `` `MPRnb(name, default, units,
desc)`` idiom) and confirm the argument values are visible in the
trace-mode output.

---

## Edge Cases

- WHEN an `` `include `` target cannot be resolved THEN the existing
  `IncludeNotFound` error SHALL keep reporting the *including* file's
  directive span exactly as it does today (this path never reaches
  expanded text, so it's unaffected by this feature — stated for
  completeness, not because it changes).
- WHEN a macro is defined but never invoked THEN nothing about its body
  text participates in span resolution at all (dead code, never emitted
  into the flat buffer).
- WHEN an `` `ifdef ``-guarded (inactive) branch contains malformed
  syntax THEN it is already skipped textually by the preprocessor before
  span-mapping is relevant (existing behavior, unaffected).
- WHEN the top-level input has zero directives (no `` `include ``/`` `define ``) THEN the resolved `Span` SHALL be identical (same file, same
  offsets) to today's un-mapped behavior — this is the majority of the
  existing test suite and must show zero diff in rendered output.
- WHEN a `Span` covers zero-length text (some existing error variants
  build these deliberately, e.g. `from_vams_parse_error_handles_zero_length_span`) THEN resolution SHALL still produce a valid, in-bounds
  location, not a divide-by-zero/panic in the segment lookup.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| SPANMAP-01 | P1: Correct file+line on include-touched diagnostics | Tasks (T8, fix `cef91386`) | ✅ Verified |
| SPANMAP-02 | P1: Include longer than including file (the crash case) | Tasks (T8) | ✅ Verified |
| SPANMAP-03 | P1: Repeated inclusion resolves to one stable file identity | Tasks (T6) | ✅ Verified |
| SPANMAP-04 | P2: Macro invocation site as primary label | Tasks (T9/T10) | ✅ Verified |
| SPANMAP-05 | P2: Macro definition site as secondary "expanded from here" label | Tasks (T9/T10) | ✅ Verified |
| SPANMAP-06 | P2: Nested macro expansion chain (N-deep) | Tasks (T9) | ✅ Verified |
| SPANMAP-07 | P1/P2: Zero regression on the no-directives case | Tasks (T8, fix `cef91386`) | ✅ Verified |
| SPANMAP-08 | P3: LSP position resolution for include/macro-touched spans | Tasks (T11/T12/T13) | ✅ Verified (T13 partial — see validation.md) |
| SPANMAP-09 | P2.5: macro invocation arguments recoverable from `ExpansionFrame` | Tasks (T9) | ✅ Verified |
| SPANMAP-10 | P2.5: `piperine expand <file>` prints fully preprocessed text | Tasks (T14) | ✅ Verified |
| SPANMAP-11 | P2.5: `piperine expand --trace` annotates macro name/args/location | Tasks (T15) | ✅ Verified |

**ID format:** `SPANMAP-NN`

**Status values:** Pending → In Design → In Tasks → Implementing →
Verified

**Coverage:** 11 total, 11 mapped to tasks, 0 unmapped. Verifier PASS
(2 fix→re-verify iterations) — see
`.specs/features/preprocessor-span-mapping/validation.md`.

---

## Success Criteria

- [ ] Zero `OutOfBounds`/panic diagnostic renders across the full
      `vams-corpus` sweep (86 files) and the existing `piperine-lang`
      test corpus.
- [ ] Every `.va`/`.vams` file in the existing test suite that already
      renders a correct diagnostic today still renders byte-for-byte the
      same rendered output (regression gate).
- [ ] A macro-expansion error demo shows both the invocation and
      definition locations in one rendered diagnostic.
