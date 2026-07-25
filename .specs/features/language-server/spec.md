# Language Server (A+) Specification

> **Refines ROADMAP P4** ("Language server 100%"). Grounded in a code audit of
> `crates/piperine-lang-server` (2026-07-23): the server advertises 17 LSP
> capabilities but the **depth** is thin — resolution is word-based global
> lookup, references/rename are text scans, there is no project-wide symbol
> index, elaboration stops at the first error, and PHDL has no doc comments.
> "A+" = the editor understands PHDL as well as the compiler does.

**Builds on:** MD-24 (declared surface — `extern` decls already carry
`decl_span`, consumed by `resolve_at`), MD-25 (POM navigability — the doc field
is additive, never overwrites authored structure).

## Problem Statement

The LSP surface is broad but shallow. Concretely, from the audit:

1. **Resolution is word-based global lookup** (`symbol_index.rs:53`, comment:
   *"Global lookup for now (until we build true scope resolution)"*). `resolve_at`
   returns the first declaration in any module whose name matches the word under
   the cursor — no scope, no shadowing, no cursor context, no use-vs-decl
   distinction. Every downstream feature (hover, goto, references, rename,
   highlight) inherits this weakness.
2. **References/rename are text scans** (`references.rs:23`, `rename.rs:29`):
   `resolve_at` only gates "is this a symbol", then `word_occurrences`
   (`state.rs:105`) returns every whole-word text match — including comments and
   strings, and across unrelated scopes. Renaming a `power` var in one module
   renames every `power` in the file. Rename is single-file
   (`changes.insert(uri, …)`).
3. **No project-wide symbol index** (`project.rs`): the project `SourceMap` is
   built for elaboration, but `ServerState.documents` is per-URI and `resolve_at`
   reads one document's design — cross-file goto/references/rename don't traverse
   other files.
4. **Elaboration stops at the first error** (`state.rs:80`): parse errors are
   tolerant (multiple), but `elaborate_with_context` returns a single `Err`, so
   the editor shows one elaboration error at a time. Diagnostics are ERROR-only
   with a generic `"parse-error"` code (`diagnostics.rs:69`).
5. **No doc comments**: comments are lexed-and-discarded (no token), POM
   declarations carry no documentation, and hover shows only type/kind
   (`hover.rs` / `lookup_hover_info`). The one missing *language* feature.

## Goals

- [ ] **Scope-aware resolution**: a real binding resolver (cursor context, scope,
      shadowing, use-vs-decl) replaces word-based global lookup — the engine every
      feature rides.
- [ ] **Binding-driven references/rename/highlight**: occurrence sets come from
      the resolver's binding identity, not text; comments/strings never match;
      rename spans files when the binding does.
- [ ] **Project-unit elaboration**: a project-wide symbol index; cross-file
      goto/references/rename; per-file diagnostic fan-out.
- [ ] **Error-accumulating elaboration**: multiple elaboration errors at once;
      warning severity + structured error codes.
- [ ] **PHDL `///` doc comments**: lexer → POM `doc` field → hover (+ completion
      detail) — the last missing language piece.
- [ ] **Attribute-schema IDE support**: `@schema` completion, argument
      validation, hover→fields, goto→`@attribute` decl, outline.
- [ ] **Protocol-level tests**: `Connection::memory()` round-trips covering the
      above (incl. scope-shadowing + doc-comment assertions).
- [ ] `cargo test --workspace` green; zero rustc warnings.

## Out of Scope

| Feature | Reason |
|---------|--------|
| **New LSP capabilities** (call hierarchy, type hierarchy, code lens, …) | The 17 advertised capabilities cover "100%"; this feature deepens them, not widens. Net-new capabilities are post-V1. |
| **Incremental/streaming elaboration** | The full re-analyze on change is fast enough today (`state.rs:71`); an incremental engine is a perf follow-up, not a correctness gap. |
| **Doc-comment rendering beyond hover** (doc-gen site, `@schematic`) | The `doc` field enables them (P3 host reflection reads it), but generating a doc site is separate. |
| **Formatter changes** | `formatting.rs` exists and is out of scope here. |
| **VS Code extension packaging** | User-owned (ROADMAP "Out of agent scope"). |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Scope of "P4 refine" | All of P4: resolution, navigation, diagnostics, doc comments, attribute IDE, protocol tests | User 2026-07-23 ("todo o P4... A plus") | y (user) |
| Doc comments span crates | The feature includes the **language** work (lexer + POM `doc`) *and* the LSP hover — one feature, since the payoff is hover | User bundled "refina P4 e adiciona phdl docs" | y (user) |
| MVP anchor | P1 = scope-aware resolution core + doc comments (the engine + the user-emphasized missing piece) | Everything rides the resolver; doc comments is high-value/independent | n (Design) |
| Resolver source | Expose the elaborator's existing name→id/binding maps as a query, rather than re-implementing scoping in the server | The elaborator already resolves scopes at elaboration; the server should read that, not duplicate it | n (Design) |
| Doc-comment syntax | `///` line doc comments attach to the following declaration; `//` stays discarded. `/** */` block form deferred unless trivial | Rust-style, matches user ask; block form is additive later | n (Design) |
| `doc` field placement | Additive `doc: Option<String>` on POM module/port/param/var/instance/net/behavior decls (MD-25 — never overwrites authored structure; `#[serde]`-carried) | POM is the single object model; hosts (P3) + LSP read one field | y (design intent) |
| Cross-file rename shape | `WorkspaceEdit.document_changes` spanning every file that references the binding | Project-unit index makes it possible; single-file rename is a bug for shared symbols | n (Design) |
| Error accumulation depth | Elaboration collects multiple `ElabError`s where the pass can continue; passes that genuinely cannot continue still stop (documented) | Not every error is recoverable; accumulate where safe | n (Design) |

**Open questions:** the `n (Design)` rows — resolver query shape, doc-comment
block form, cross-file edit mechanics, accumulation depth. HOW-shape for Design.

---

## User Stories

### P1: Scope-aware resolution core ⭐ MVP

**User Story**: As an editor, I resolve the identifier under the cursor to its
**actual binding** — respecting scope and shadowing — so hover, goto, and every
navigation feature points at the right declaration.

**Why P1**: The engine. `symbol_index.rs:53`'s word-based global lookup is the
root cause of every shallow feature. Fixing it upgrades hover/goto immediately
and enables binding-driven references/rename (P2).

**Acceptance Criteria**:

1. WHEN the cursor is on an identifier THEN resolution SHALL use cursor context
   (the enclosing module/scope), not a global first-match, returning the binding
   in scope at that position.
2. WHEN a name is shadowed (a local/param shadowing an outer/global of the same
   name) THEN resolution SHALL return the **innermost** binding in scope, not the
   first module-order match.
3. WHEN the cursor is on a declaration vs a use of the same binding THEN both
   SHALL resolve to the same binding identity (a stable key), enabling occurrence
   grouping (P2).
4. WHEN an identifier resolves THEN `goto-definition` SHALL jump to that
   binding's `decl_span` (correct even when another module declares the same
   name).
5. WHEN resolution is exposed THEN it SHALL come from the elaborator's binding
   information (a query), not a re-implementation of scoping in the server.

**Independent Test**: A source with two modules each declaring `x`, plus a local
`x` shadowing a param in one. Goto/hover on each `x` lands on the correct
declaration; the shadowed inner `x` resolves to the local, not the param.

---

### P1: PHDL `///` doc comments

**User Story**: As a device author, I document a module/port/param/var with `///`
comments and see that documentation on hover — the last missing language feature.

**Why P1**: User-emphasized ("a única coisa que falta no nosso carinha"),
high-value, and largely independent of the resolver. Delivers visible hover
value fast and seeds the P3 host reflection (`Module.doc`).

**Acceptance Criteria**:

1. WHEN a run of `///` lines precedes a declaration THEN the lexer SHALL capture
   it and elaboration SHALL attach it to that declaration's POM `doc` field;
   ordinary `//` comments SHALL stay discarded.
2. WHEN a declaration has a `doc` THEN hover SHALL render it as Markdown above the
   type/kind line (`lookup_hover_info`).
3. WHEN a declaration has no `///` THEN `doc` SHALL be `None` and hover SHALL be
   unchanged (no regression).
4. WHEN the POM is written THEN `doc` SHALL be additive (MD-25 — authored
   structure never overwritten) and `#[serde]`-carried so hosts read it.
5. WHEN a `///` run is not immediately followed by a declaration THEN it SHALL be
   ignored (no crash, no misattachment).

**Independent Test**: `/// A two-terminal resistor.` above `module res(...)`;
hover on `res` shows the sentence as Markdown; a plain `// note` does not appear
in hover; `res` POM node carries `doc = Some("A two-terminal resistor.")`.

---

### P2: Binding-driven references / rename / highlight

**User Story**: As an editor, "find references", "rename", and "highlight" act on
the **binding**, not the text — every real use, no comment/string false matches,
across files.

**Why P2**: Rides the P1 resolver. `references.rs:23`/`rename.rs:29`'s
`word_occurrences` text scan is incorrect for shadowed names and matches
comments/strings.

**Acceptance Criteria**:

1. WHEN "find references" runs THEN it SHALL return exactly the uses of the
   resolved binding (from the resolver), excluding same-spelled identifiers in
   other scopes and any occurrence inside comments or strings.
2. WHEN "rename" runs THEN it SHALL edit exactly those binding uses; a same-named
   identifier in an unrelated scope SHALL NOT be edited.
3. WHEN a binding is referenced across files THEN rename SHALL produce a
   multi-file `WorkspaceEdit.document_changes` covering every file.
4. WHEN "document highlight" runs THEN it SHALL highlight the binding's uses in
   the current file (same source as references), not text matches.
5. WHEN prepare-rename runs on a non-renameable token (keyword, literal) THEN it
   SHALL decline (not offer a rename).

**Independent Test**: Two modules with a `power` var each; rename one → only that
module's `power` changes; a `// power` comment is untouched; a cross-file
reference renames in both files.

---

### P2: Project-unit elaboration + cross-file navigation

**User Story**: As an editor in a multi-file project, goto/references/rename and
diagnostics work across the whole project, not just the open buffer.

**Why P2**: `project.rs` discovers the project but the server holds no
project-wide symbol index; cross-file navigation is impossible today.

**Acceptance Criteria**:

1. WHEN a document belongs to a project (`Piperine.toml`) THEN the server SHALL
   hold a project unit: the multi-file elaborated design + a symbol index keyed
   by binding, spanning all files.
2. WHEN goto-definition targets a symbol declared in another file THEN it SHALL
   open that file at the declaration.
3. WHEN references/rename run on a project-wide symbol THEN they SHALL span every
   file that uses it.
4. WHEN one file changes THEN diagnostics SHALL fan out per file (each file's
   errors published against its own URI), not collapsed onto the edited buffer.
5. WHEN no project root is found THEN the server SHALL fall back to single-file
   behavior (no regression for standalone files).

**Independent Test**: A two-file project where `b.phdl` instantiates a module from
`a.phdl`; goto on the instance type opens `a.phdl`; rename of the module updates
both files; an error in `a.phdl` publishes against `a.phdl`.

---

### P2: Error-accumulating elaboration + richer diagnostics

**User Story**: As a developer, the editor shows all my errors at once, with
warnings and meaningful codes — not one error at a time.

**Why P2**: `state.rs:80` stops at the first `ElabError`; `diagnostics.rs:69`
emits ERROR-only with a generic code.

**Acceptance Criteria**:

1. WHEN elaboration hits multiple errors in recoverable passes THEN it SHALL
   accumulate and report them all; a genuinely unrecoverable pass MAY stop
   (documented which).
2. WHEN a diagnostic is a warning (not a hard error) THEN it SHALL carry
   `WARNING` severity, not ERROR.
3. WHEN a diagnostic has a known error class THEN it SHALL carry a structured
   code (e.g. `E2021`), not a blanket `"parse-error"`.
4. WHEN diagnostics are published THEN each SHALL have an accurate span (no
   `0:0..0:1` fallback for errors that have a span).

**Independent Test**: A file with two independent elaboration errors → both
appear; a warning-class diagnostic shows as a warning; codes are specific.

---

### P3: Attribute-schema IDE support

**User Story**: As an author using `@rfport`/`@device`/`@model`/… attributes, the
editor completes schema names, validates arguments, hovers the schema fields, and
navigates to the `@attribute` declaration.

**Why P3**: The `extern attribute` schemas (`@rfport` today; `@model`/`@name`/…
from `phdl-introspection-attributes`) already carry `decl_span`
(`symbol_index.rs:221`). The IDE affordances are the missing layer.

**Acceptance Criteria**:

1. WHEN typing `@` in an attribute position THEN completion SHALL offer the
   in-scope schema names.
2. WHEN an attribute argument is wrong (unknown field, bad type, missing
   required) THEN the editor SHALL show a diagnostic at that argument.
3. WHEN hovering a schema name or field THEN hover SHALL show the schema's fields
   (name/type/required).
4. WHEN goto-definition targets a schema name THEN it SHALL jump to the
   `extern attribute` declaration.
5. WHEN the outline is requested THEN attribute instances SHALL appear as outline
   entries on their declarations.

**Independent Test**: `@rfport(nu|` completes to `num`; `@rfport(num = "x")` (bad
type) shows a diagnostic; hover on `@rfport` lists `num`/`z0`; goto opens the
schema declaration.

---

### P3: Protocol-level test harness

**User Story**: As a maintainer, the LSP behavior is pinned by protocol round-trip
tests over an in-memory connection, so regressions are caught pre-merge.

**Why P3**: Locks the above; establishes the harness as first-class.

**Acceptance Criteria**:

1. WHEN the harness runs THEN it SHALL drive `Connection::memory()` through
   init → didOpen → request/response for hover, completion, goto, references,
   rename.
2. WHEN a scope-shadowing fixture is opened THEN a resolution test SHALL assert
   the correct (innermost) binding.
3. WHEN a doc-comment fixture is opened THEN a hover test SHALL assert the doc
   text appears.
4. WHEN a multi-file fixture is opened THEN a cross-file goto/rename test SHALL
   assert the cross-file result.

**Independent Test**: The harness itself — green protocol round-trips for each
feature above.

---

## Edge Cases

- WHEN the cursor is on a keyword/literal/comment THEN resolution SHALL return
  `None` and navigation features SHALL decline (no false symbol).
- WHEN a `///` run precedes another `///` run separated by blank lines THEN only
  the run immediately adjacent to the declaration SHALL attach (documented rule).
- WHEN a project has a cyclic import THEN the project unit SHALL fail loud (or
  degrade to single-file) without hanging.
- WHEN a symbol is renamed to a name that collides with an existing binding in
  the same scope THEN prepare-rename MAY warn (best-effort; not a hard block).
- WHEN a file outside any project is edited THEN all features SHALL work
  single-file (fallback path).
- WHEN elaboration fully fails THEN the last valid design SHALL keep serving
  navigation (existing `state.rs:87` resilience preserved).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| LSP-01 | P1 cursor-context resolution (not global) | Design | Pending |
| LSP-02 | P1 shadowing → innermost binding | Design | Pending |
| LSP-03 | P1 stable binding identity (use==decl) | Design | Pending |
| LSP-04 | P1 goto rides correct binding | Design | Pending |
| LSP-05 | P1 resolution from elaborator query, not re-impl | Design | Pending |
| LSP-06 | P1 lexer captures `///` doc runs | Design | Pending |
| LSP-07 | P1 POM `doc` field, elaboration attaches | Design | Pending |
| LSP-08 | P1 hover renders `doc` Markdown | Design | Pending |
| LSP-09 | P1 no-`///` → unchanged (no regression) | Design | Pending |
| LSP-10 | P2 references = binding uses, no comment/string | Design | Pending |
| LSP-11 | P2 rename = binding uses only | Design | Pending |
| LSP-12 | P2 cross-file rename (`document_changes`) | Design | Pending |
| LSP-13 | P2 highlight = binding uses | Design | Pending |
| LSP-14 | P2 project-unit symbol index | Design | Pending |
| LSP-15 | P2 cross-file goto | Design | Pending |
| LSP-16 | P2 per-file diagnostic fan-out | Design | Pending |
| LSP-17 | P2 single-file fallback (no project) | Design | Pending |
| LSP-18 | P2 error-accumulating elaboration | Design | Pending |
| LSP-19 | P2 warning severity + structured codes | Design | Pending |
| LSP-20 | P3 `@schema` completion | Design | Pending |
| LSP-21 | P3 attribute-argument validation | Design | Pending |
| LSP-22 | P3 hover→schema fields | Design | Pending |
| LSP-23 | P3 goto→`@attribute` decl | Design | Pending |
| LSP-24 | P3 attribute outline entries | Design | Pending |
| LSP-25 | P3 protocol-test harness (memory connection) | Design | Pending |
| LSP-26 | P3 scope-shadowing + doc-comment + cross-file protocol tests | Design | Pending |

**ID format:** `LSP-[NUMBER]`

**Coverage:** 26 total, 0 mapped to tasks (Design pending).

---

## Success Criteria

- [ ] Hover/goto resolve the correct binding under shadowing (LSP-01..05).
- [ ] `///` doc comments render on hover; POM carries `doc` (LSP-06..09).
- [ ] References/rename/highlight act on the binding, cross-file, no false text
      matches (LSP-10..13).
- [ ] Project-wide navigation + per-file diagnostics (LSP-14..17).
- [ ] All elaboration errors at once, with severities + codes (LSP-18..19).
- [ ] Attribute schemas complete/validate/hover/goto/outline (LSP-20..24).
- [ ] Protocol round-trip tests pin every feature (LSP-25..26).
- [ ] `cargo test --workspace` green; zero rustc warnings.
