# MSL Parser Specification

**Approved**: 2026-11-13 - user: "Ok, manda bala"

## Problem Statement

`docs/spec/` defines the Piperine mixed-signal language (MSL), but no code reads it: the spec's 105 `rust` code blocks and its 17 galleries have never been machine-checked. A parser that turns MSL source text into a spanned AST, reports syntax errors with the spec's codes, and prints the AST back is the first executable artifact of the language and the base for elaboration later.

## Goals

- [ ] Every `rust` code block in `docs/spec/` that the spec presents as valid parses with zero diagnostics.
- [ ] Every parse-phase error the spec lists (17 rows marked `parse`) is reported with its code at the offending span.
- [ ] `parse(print(ast))` equals `ast` (ignoring spans) for every corpus file.

## Out of Scope

| Feature | Reason |
| ----------- | -------------- |
| Name resolution, type checking, elaboration, any error of phase `elaboration`/`run`/`host` | "apenas o parser"; next features |
| Lowering MSL to the POM, codegen, solver | later feature |
| Removing or changing the VAMS lexer/parser/elaborator | MSL lives beside it in `piperine-lang` for now |
| Resolving `include_str!` or reading other files | parse records the call; reading files is elaboration |
| CLI (`piperine check`/`fmt`) and LSP wiring | separate integration feature |
| Formatter style options (width, wrapping) | the printer emits one canonical form |
| Keeping other workspace crates building | user: other crates may break |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | --------------- | --------- | ---------- |
| Where the parser lives | new module `piperine_lang::msl` (`lexer`, `ast`, `parser`, `print`, `diag`) | user: "Dentro do piperine-lang" | y |
| Parsing technique | `logos` lexer + `chumsky` (0.10) parser combinators with Pratt expressions and error recovery; diagnostics rendered through `miette` (already a `piperine-lang` dependency) | user: "O que for mais completo, exibir mensagens completas e profissionais. Não precisa ser na mão" (user delegated the library) | y - user delegated |
| Error behavior | recover at `;`, `}` and item boundaries; report every error in the file | user: "Recupera e lista vários" | y |
| Scope | lexer + AST + parse-phase errors + canonical pretty-printer with round-trip | user: "… + pretty-printer" | y |
| Grammar authority | `docs/spec/appendix_b_grammar.md` B.3 for the additions; Rust 2024 reference for the inherited value grammar, minus the forms `docs/spec` excludes (§1 rule 2, App. A dropped token classes) | spec index §0.4 | y - settled by the spec |
| Which code blocks are "valid" | blocks fenced ` ```rust `; a block that illustrates an error is re-fenced ` ```rust,compile_fail ` in `docs/spec/` by this feature (docs edit only, no wording change) | the spec has no marker today; the corpus test needs one | y - approved with spec |
| Pace | one task per commit, sub-agents: Sonnet implements, Opus verifies | user: "Lança sempre Sonnet para implementação, Opus para verifier"; "Vamos devagar" | y |
| Branch | `feature/msl-parser`, from `feature/msl-spec`, worktree `../piperine-msl-spec` | keeps the user's dirty main tree untouched | y - approved with spec |
| Recursion bound | nesting deeper than 256 levels is a diagnostic, never a stack overflow | a parser must not crash on input | y - approved with spec |

**Open questions:** none - all resolved or logged above.

---

## User Stories

### P1: Lex MSL source ⭐ MVP

**User Story**: As a language implementer, I want MSL text turned into spanned tokens per Appendix A so that the parser and diagnostics share exact source positions.

**Why P1**: every later stage depends on it.

**Acceptance Criteria**:

1. WHEN a source contains an SI-suffixed literal listed in Appendix A (e.g. `50m`, `1p`, `2.2u`) THEN the lexer SHALL produce one float literal token carrying the scaled value and the original text.  <!-- event-driven -->
2. IF a source contains a malformed SI literal (suffix with exponent, with type suffix, or on a non-decimal literal) THEN the lexer SHALL report `E0902` at that literal's span.  <!-- unwanted -->
3. IF a source contains a token of a dropped class (char literal, byte literal, byte string, lifetime other than `'static`) THEN the lexer SHALL report `E0905` at that token's span.  <!-- unwanted -->
4. WHEN a source contains `+>` or `<-` THEN the lexer SHALL produce a single operator token for each.  <!-- event-driven -->
5. The lexer SHALL classify every keyword and contextual keyword listed in Appendix B.1 and Appendix A as the spec defines it (reserved vs contextual).  <!-- ubiquitous -->
6. The lexer SHALL attach doc comments (`///`, `//!`) to the token stream so the parser can attach them to items.  <!-- ubiquitous -->
7. IF a block comment or string literal is not terminated THEN the lexer SHALL report one diagnostic at its opening position and continue to end of file.  <!-- unwanted -->

**Independent Test**: lex fixture strings and compare token kinds, values and spans.

---

### P1: Parse every construct into a spanned AST ⭐ MVP

**User Story**: As a language implementer, I want every MSL construct parsed into a typed AST with source spans so that elaboration can be built on it.

**Why P1**: the deliverable of the feature.

**Acceptance Criteria**:

1. WHEN a source uses any production of Appendix B.3 THEN the parser SHALL produce the AST node for that production with a span covering its text.  <!-- event-driven -->
2. WHEN a source uses an inherited Rust 2024 value form not excluded by `docs/spec` (items `fn`/`struct`/`enum`/`impl`/`const`/`use`/`mod`, expressions, patterns, types, generics, `where`, closures, attributes) THEN the parser SHALL produce its AST node.  <!-- event-driven -->
3. The parser SHALL parse every ` ```rust ` code block in `docs/spec/*.md` with zero diagnostics.  <!-- ubiquitous -->
4. The parser SHALL report at least one diagnostic for every ` ```rust,compile_fail ` code block in `docs/spec/*.md`.  <!-- ubiquitous -->
5. WHEN a `contribution` or `commit` appears at any statement depth (inside `if`/`match`/`for` blocks) THEN the parser SHALL accept it as a statement.  <!-- event-driven -->
6. The parser SHALL attach outer doc comments and attributes to the item, field or port that follows them.  <!-- ubiquitous -->

**Independent Test**: run the corpus test over `docs/spec/`; inspect AST snapshots for one fixture per B.3 production.

---

### P1: Report parse errors professionally ⭐ MVP

**User Story**: As a model author, I want syntax errors reported with the spec's code, the exact location and a fix hint so that I can correct my model without guessing.

**Why P1**: user: "exibir mensagens completas e profissionais".

**Acceptance Criteria**:

1. IF a source triggers a condition listed as phase `parse` in any `docs/spec` error table THEN the parser SHALL report a diagnostic with that row's code (e.g. `E0121`, `E0901`, `E0903`, `E0907`, `E0908`).  <!-- unwanted -->
2. Every diagnostic SHALL carry a code, a one-line message, a primary label at the offending span, and, where the spec names a correct form, a `help` line naming it.  <!-- ubiquitous -->
3. IF a source contains several independent syntax errors THEN the parser SHALL report each of them in one run, in source order, and still return a partial AST.  <!-- unwanted -->
4. IF an unexpected token is found THEN the diagnostic SHALL name the token found and the tokens or construct expected.  <!-- unwanted -->
5. WHEN diagnostics are rendered THEN the output SHALL be `miette` graphical reports showing the source line, the span underline and the code.  <!-- event-driven -->
6. IF nesting exceeds 256 levels THEN the parser SHALL report a diagnostic and stop descending, without a stack overflow.  <!-- unwanted -->
7. The parser SHALL parse 256 nesting levels in a debug build on the caller's thread with an 8 MiB stack, without spawning a larger-stack thread.  <!-- ubiquitous -->

**Independent Test**: one fixture per parse-phase error row asserting code and span; snapshot of rendered output for 3 fixtures.

---

### P2: Print the AST back

**User Story**: As a tool author, I want a canonical printer so that tools can rewrite MSL and so that AST completeness is tested by round-trip.

**Why P2**: user asked for it; it also proves the AST loses nothing.

**Acceptance Criteria**:

1. WHEN an AST parsed from a corpus file is printed and re-parsed THEN the second AST SHALL equal the first, ignoring spans.  <!-- event-driven -->
2. WHEN printed output is parsed and printed again THEN the second output SHALL be byte-identical to the first.  <!-- event-driven -->
3. The printer SHALL preserve doc comments and attributes.  <!-- ubiquitous -->

**Independent Test**: round-trip test over the corpus.

---

## Edge Cases

- WHEN the source is empty THEN the parser SHALL return an empty file AST and no diagnostics.
- IF the source is not valid UTF-8 THEN the parser SHALL report one diagnostic and no AST.
- WHEN `Unknown` is used as a value THEN the parser SHALL accept it; IF it is used as a binding, item or field name THEN it SHALL report `E0908`.
- WHEN `potential`/`flow`/`value`/`resolve` appear outside a nature THEN the parser SHALL treat them as identifiers (contextual keywords).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----------- | ------ | ------- |
| MSLP-01 | P1: Lex (AC 1–7) | Design | Pending |
| MSLP-02 | P1: Parse AST (AC 1–2, 5–6) | Design | Pending |
| MSLP-03 | P1: Parse corpus (AC 3–4) | Design | Pending |
| MSLP-04 | P1: Errors (AC 1–7) | Design | Pending |
| MSLP-05 | P2: Printer (AC 1–3) | Design | Pending |
| MSLP-06 | Edge cases | Design | Pending |

**Coverage:** 6 total, 0 mapped to tasks.

---

## Sources

- `docs/spec/appendix_a_lexical.md` - tokens, SI literals, keywords, `E09xx`
- `docs/spec/appendix_b_grammar.md` - B.1 constructs, B.3 complete EBNF of the additions
- `docs/spec/index.md` §0.4 - Rust 2024 inheritance
- `docs/spec/part_i_values.md` §1 rule 2, §3 rule 6 - excluded forms (`E0121`)

---

## Success Criteria

- [ ] Corpus test green: all valid spec blocks parse, all `compile_fail` blocks diagnose.
- [ ] 17 parse-phase error codes each covered by a fixture.
- [ ] Round-trip green over the corpus.
