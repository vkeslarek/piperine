# Language Server Specification

**Priority: low.** User, 2026-11-16: "O lang-server é o menos importante, vai ficar para bem depois." This spec is written so that `GAPS.md` B3 has an owner; it is scheduled after every other open feature unless the user moves it.

**Size:** M, about 20 tasks.

## Open questions (user decisions, asked 2026-11-17)

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | Port or rewrite `crates/piperine-lang-server` (6 447 lines aimed at the retired Verilog-AMS front end)? | (a) rewrite on the MSL front end: keep the transport (`lsp-server`, `dispatch.rs`, `text_pos.rs`) and replace every handler and the state, deleting the Verilog-AMS code; (b) port handler by handler, keeping their structure | **(a)**. Every handler reads a front end that no longer exists; the transport is the only part that still fits |
| Q2 | Which editor features in this feature? The old server had 16 handlers (completion, rename, semantic tokens, inlay hints, ...) | (a) diagnostics, hover, go to definition, formatting and document symbols now; references, rename, completion and semantic tokens as a later feature; (b) all 16 now | **(a)**. Diagnostics first, as the user's order goes; the rest reuses the same resolution and comes cheap later. With (b) the size is L |
| Q3 | Editor packaging | (a) the server binary only, with a documented configuration for VS Code, Neovim and Helix; (b) also a VS Code extension in the repository (client and a TextMate grammar) | **(a)**. Any LSP client starts the binary; an extension is a release artifact of its own |
| Q4 | Which items get type diagnostics? The type checker checks only what a run's top reaches (`GAPS.md` S3, owned by `generics`) | (a) parse and name diagnostics for every file of the package, type diagnostics for what the package's `[run]` tops reach, and every item once S3 is closed; (b) wait for S3 before starting | **(a)**. The server gives what `piperine check` gives, no more and no less, and gains the rest when S3 lands |

## Problem Statement

`piperine-lang-server` does not build (`GAPS.md` B3): it targets the deleted Verilog-AMS front end and sits outside `default-members`. An editor has no diagnostics, hover, navigation or formatting for MSL; `piperine check` and `piperine fmt` give the same diagnostics and formatting from the command line meanwhile.

## Goals

- [ ] `piperine-lang-server` builds in the default members and speaks LSP over standard input and output.
- [ ] An editor shows the diagnostics `piperine check` gives, as the user types.
- [ ] Hover, go to definition, document symbols and formatting work on MSL, formatting through `msl::format` (the comment-preserving formatter).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| References, rename, completion, semantic tokens, inlay hints, code actions, signature help, folding, selection ranges, workspace symbols | Q2 (a): a later feature |
| A VS Code extension | Q3 (a) |
| Type diagnostics for items no top reaches | `generics` (S3); Q4 (a) gives them when S3 lands |
| Verilog-AMS | Retired front end |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Rewrite | keep the transport, replace the rest (Q1 (a)) | Q1 recommendation | n - open (Q1) |
| Features | diagnostics, hover, definition, formatting, symbols (Q2 (a)) | Q2 recommendation | n - open (Q2) |
| Packaging | binary and documented client configuration (Q3 (a)) | Q3 recommendation | n - open (Q3) |
| Diagnostic scope | what `piperine check` checks (Q4 (a)) | Q4 recommendation | n - open (Q4) |
| When diagnostics are recomputed | on open, on save, and on change after 300 ms without a keystroke, the whole package re-checked from the open buffers | a whole-package check is simple and the package is small; incremental analysis is a later optimisation if measured slow | n - planner |
| Unsaved buffers | the open buffer's text wins over the file on disk for every check | an editor shows the buffer | n - planner |
| Position encoding | UTF-16 code units, the LSP default, converted from the front end's byte spans | the LSP specification | y - LSP |

**Open questions:** Q1 to Q4 at the top of this file, awaiting the user.

**Implicit-requirement dimensions.** Input validation and bounds: LSV-02 AC 3 (a file outside a package). Failure and partial-failure states: LSV-02 AC 4 (a front-end panic does not kill the server), LSV-06 AC 2 (a source that does not parse is not formatted). Idempotency and retry: LSV-06 AC 3 (formatting a formatted file changes nothing). Auth and rate limits: N/A, a local process. Concurrency and ordering: LSV-02 AC 2 (a newer change cancels an older check). Data lifecycle: N/A, nothing is stored. Observability: LSV-01 AC 3 (the server logs to the client's log). External-dependency failure: N/A, no external service. State-transition integrity: LSV-01 AC 2 (shutdown then exit).

---

## User Stories

### P1: The server builds and starts ⭐ MVP

**User Story**: As an editor user, I want a server my editor can start.

**Why P1**: B3: it does not build.

**Acceptance Criteria**:

1. The system SHALL build `piperine-lang-server` as a member of the root's `default-members`, with no warning (LSV-01)
2. WHEN a client sends `initialize`, `initialized`, `shutdown` and `exit` THEN the system SHALL answer each as the LSP specification says and exit 0 (LSV-01)
3. WHEN the server logs THEN it SHALL send the message to the client's log (`window/logMessage`) and write nothing else on standard output (LSV-01)

**Independent Test**: `cargo test -p piperine-lang-server --test lifecycle`.

---

### P1: Diagnostics ⭐ MVP

**User Story**: As an MSL author, I want my errors in the editor as I type, the same as `piperine check`.

**Why P1**: the user's order: diagnostics first.

**Acceptance Criteria**:

1. WHEN a file of a package is opened, changed or saved THEN the system SHALL publish, for every file of the package, the diagnostics `piperine check` gives for the package with the open buffers' text, each with its code, message, range and related locations (LSV-02)
2. WHEN a change arrives while a check of an older version runs THEN the system SHALL publish only the newer version's diagnostics (LSV-02)
3. WHEN a lone `.msl` file outside a package is opened THEN the system SHALL check it as `piperine check <file>` does (LSV-02)
4. IF the front end panics on a source THEN the system SHALL publish one diagnostic naming the internal error at the start of the file and keep serving (LSV-02)
5. WHEN a diagnostic is fixed THEN the system SHALL publish an empty list for the file that had it (LSV-02)

**Independent Test**: opening `examples/buck` with one type error inserted gives the diagnostic `piperine check` prints, at the same line and column.

---

### P2: Hover and go to definition

**User Story**: As an MSL author, I want to see what a name is and jump to it.

**Why P2**: the navigation the user named.

**Acceptance Criteria**:

1. WHEN the client asks for hover on a name THEN the system SHALL give its kind, its declaration's signature as `msl::print` prints it (a module's ports and params, a function's parameters and result, a param's type and default, a nature's quantities) and its `///` docs (LSV-03)
2. WHEN the client asks for the definition of a name THEN the system SHALL give the range of the item it resolves to, across files and crates of the package, the embedded `std` included as a read-only document (LSV-04)
3. WHEN the name resolves to nothing THEN the system SHALL give no hover and no location (LSV-04)

**Independent Test**: hover on `Resistor` in `examples/rc` shows its ports, params and docs; go to definition opens its module.

---

### P2: Document symbols

**User Story**: As an MSL author, I want an outline of the file.

**Why P2**: cheap with the POM.

**Acceptance Criteria**:

1. WHEN the client asks for document symbols THEN the system SHALL give the file's items (modules, functions, natures, enums, constants) with their ranges, and each module's ports, params and instances as children, in source order (LSV-05)

**Independent Test**: the outline of `examples/buck/src/lib.msl` lists its modules in source order.

---

### P2: Formatting

**User Story**: As an MSL author, I want the editor to format my file as `piperine fmt` does.

**Why P2**: the formatter exists (`msl::format`, MD-65 item 4).

**Acceptance Criteria**:

1. WHEN the client asks to format a document THEN the system SHALL give one edit replacing the text with `msl::format`'s output, the same text `piperine fmt` writes (LSV-06)
2. IF the document does not parse, or the formatter would lose a comment THEN the system SHALL give no edit and send the reason to the client's log (LSV-06)
3. WHEN a formatted document is formatted again THEN the system SHALL give no edit (LSV-06)

**Independent Test**: formatting every file of `examples/buck` through the server equals `piperine fmt`'s output.

---

### P3: The documents

**User Story**: As an editor user, I want to know how to start the server in my editor.

**Why P3**: no behavior depends on it.

**Acceptance Criteria**:

1. WHEN this feature merges THEN the Book SHALL have a section on the language server with the client configuration of VS Code, Neovim and Helix (LSV-07)
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer list B3, and `CLAUDE.md` and `AGENTS.md` SHALL no longer say the server is outside the default build (LSV-07)

**Independent Test**: `cargo test -p piperine-cli --test docs`.

---

## Edge Cases

- WHEN a file is opened that is neither in a package nor `.msl` THEN the system SHALL publish no diagnostic for it.
- WHEN `Piperine.toml` itself has an error THEN the system SHALL publish the package error on the manifest.
- WHEN hover lands on a keyword or a literal THEN the system SHALL give no hover.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| LSV-01 | P1: The server builds and starts | - | Pending |
| LSV-02 | P1: Diagnostics | - | Pending |
| LSV-03 | P2: Hover and go to definition | - | Pending |
| LSV-04 | P2: Hover and go to definition | - | Pending |
| LSV-05 | P2: Document symbols | - | Pending |
| LSV-06 | P2: Formatting | - | Pending |
| LSV-07 | P3: The documents | - | Pending |

**Coverage:** 7 total, 0 mapped to tasks (spec only).

## Gaps closed

| Gap | What | Requirements |
| --- | ---- | ------------ |
| B3 | `piperine-lang-server` does not build | LSV-01 |
| B3 | an editor has no MSL diagnostics | LSV-02 |
| B3 | hover, navigation, outline, formatting | LSV-03 to LSV-06 |

## Dependencies

| Depends on | For |
| ---------- | --- |
| `piperine-lang` as it is | parse, POM resolution, type check, `msl::format` |
| `piperine-project` | package discovery |
| `generics` (S3) | type diagnostics of unreached items (Q4 (a)), not blocking |

## Sources

- `GAPS.md` B3, S3; the user, 2026-11-16 (quoted above).
- `.specs/STATE.md` MD-65 items 4 and 5; `CLAUDE.md` (the server outside `default-members`).
- `crates/piperine-lang-server/` (the transport kept); `crates/piperine-lang/src/msl/format.rs`.
- `.specs/features/language-server/` (the PHDL-era plan, not carried over).

---

## Success Criteria

- [ ] `cargo build` builds the server with no warning.
- [ ] The server's diagnostics equal `piperine check`'s on `examples/buck` with an inserted error.
- [ ] Formatting through the server equals `piperine fmt`.
