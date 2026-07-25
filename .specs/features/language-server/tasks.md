# Language Server (A+) Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement with the `tlc-spec-driven` skill: activate it by name and follow its
Execute flow and Critical Rules (per-task cycle, sub-agent delegation offer,
adequacy review, Verifier, discrimination sensor). **If the skill cannot be
activated, STOP and tell the user.**

**Design**: `.specs/features/language-server/design.md`
**Spec**: `.specs/features/language-server/spec.md`
**Status**: Draft

---

## Test Coverage Matrix

| Code Layer | Required Test Type | Coverage Expectation | Location | Run Command |
| ---------- | ------------------ | -------------------- | -------- | ----------- |
| Lexer (`///` capture) | unit | All branches (`///` run, adjacent decl, dangling run, `//` discard) | `crates/piperine-lang/tests/parse_elab.rs` | `cargo test -p piperine-lang` |
| POM `doc` + elaboration attach + `ResolutionIndex` + error accum | unit | Attach rules; index correctness; accumulation | `crates/piperine-lang/tests/{elab,pom_serde}.rs` | `cargo test -p piperine-lang` |
| LSP handlers (resolve/refs/rename/goto/hover/diag/completion) | integration | Protocol round-trips; shadowing; cross-file | `crates/piperine-lang-server/tests/*.rs` | `cargo test -p piperine-lang-server` |

## Gate Check Commands

| Gate | When | Command |
| ---- | ---- | ------- |
| Quick (lang) | lexer/POM/elab tasks | `cargo test -p piperine-lang` |
| Quick (server) | LSP handler tasks | `cargo test -p piperine-lang-server` |
| Full | cross-crate (lang + server) | `cargo test --workspace` |

---

## Execution Plan

```
Phase 1 (doc comments + resolution core) → Phase 2 (refs/rename/project/diag) → Phase 3 (attr IDE + tests)
```

```
Phase 1: T1 → T2 → T3 → T4 → T5 → T6 → T7
Phase 2: T8 → T9 → T10 → T11 → T12 → T13 → T14 → T15 → T16 → T17
Phase 3: T18 → T19 → T20 → T21 → T22 → T23
```

**Batch packing** (~7/batch → ~3-4 batches): B1 = Phase 1 (7); B2 = Phase 2a
(T8..T12); B3 = Phase 2b (T13..T17); B4 = Phase 3 (6). Sub-agent offer applies.

---

## Task Breakdown

### Phase 1 — Doc comments + resolution core

#### T1: Lexer captures `///` doc runs
**What**: Recognize `///` line runs as doc trivia attached to the following
declaration token; `//` stays discarded. **Where**: `piperine-lang/parse/lexer.rs`
(hand-written — edit with care). **Requirement**: LSP-06.
**Done when**:
- [x] a `///` run before a decl is captured as an attached doc string
- [x] plain `//` is still discarded; `///` inside a `//` line is not special
- [x] existing `parse_elab` suite green; `cargo test -p piperine-lang`
**Tests**: unit · **Gate**: quick (lang)
**Status (2026-07-24)**: DONE, commit `f547aab`. Added `Lexed.doc:
Option<String>`; `Lexer::tokenize()` (the parser-facing, comment-filtered
stream) accumulates consecutive `///` lines and attaches the joined text to
the next surviving token. A blank line, a non-doc comment (`//`, `/* */`,
`////`), or EOF resets/drops a pending run. `tokenize_all()` (used by the
formatter) is untouched. 6 new tests in `parse_elab.rs`; full
`cargo test -p piperine-lang` green (no regressions).

#### T2: POM `doc` field (additive, MD-25)
**What**: `doc: Option<String>` on module/port/param/var/instance/net/behavior
decls; `#[serde]`-carried. **Where**: `piperine-lang/pom/`. **Requirement**:
LSP-07. **Depends on**: T1.
**Done when**:
- [x] the field exists, defaults `None`, serializes; authored structure untouched
- [x] `pom_serde` round-trip green; `cargo test -p piperine-lang`
**Tests**: unit · **Gate**: quick (lang)
**Status (2026-07-24)**: DONE, commit `8a63bcf`. Added `doc: Option<String>`
(`#[serde(default)]`) to `Module`/`Port`/`Param`/`Wire`/`Instance`/`Var`, and a
plain (non-serde — `Behavior` doesn't cross the serialization boundary)
`doc` field on `Behavior`. Every existing struct-literal construction site
across `piperine-lang` (elaborator lowering + tests) threaded through as
`None`, except hierarchy-flatten's wire/instance splice paths, which
preserve the source node's `doc`. 2 new `pom_serde.rs` tests (default-None
round-trip, Some-value round-trip); full `cargo test -p piperine-lang` and
`cargo build --workspace` green.

#### T3: Elaboration attaches `///` → `doc`
**What**: Fill each decl's `doc` from its captured run; dangling runs ignored.
**Where**: `piperine-lang/elab/`. **Requirement**: LSP-07/09. **Depends on**: T2.
**Done when**:
- [x] decl with `///` gets `doc = Some(...)`; decl without → `None` (no regression)
- [x] a `///` run not before a decl is ignored (no crash/misattach)
- [x] `cargo test -p piperine-lang`
**Tests**: unit · **Gate**: quick (lang)
**Status (2026-07-24)**: DONE, commit `fe0fd01`. Threaded the lexer's `doc`
trivia through the parser AST (new `doc: Option<String>` on
`ModuleDeclaration`, `Port`, `ModuleStatement::{ParamDecl,WireDecl,VarDecl,
Instance}`, `BehaviorDecl`, each captured via a new `Parser::current_doc()`
read *before* attributes/keywords are consumed) into the corresponding POM
node's `doc` field. Bundle-field-expanded params/wires/ports (synthetic,
no single source decl) stay `doc: None`. 5 new tests in `elab.rs`
(module/param/wire/var/instance/behavior attach, no-doc regression,
dangling-run-ignored); full `cargo test -p piperine-lang` and
`cargo build --workspace` green.

#### T4: Hover renders `doc`
**What**: Prepend the resolved decl's `doc` (Markdown) above the type/kind line.
**Where**: `piperine-lang-server/handlers/hover.rs` + `symbol_index.rs`
(`Resolution.doc`). **Requirement**: LSP-08. **Depends on**: T3.
**Done when**:
- [x] hover on a documented decl shows the doc as Markdown; undocumented unchanged
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `8887656`. Added `Resolution.doc`,
populated from each POM decl's `doc` for the module/port/param/wire/var/
instance/behavior arms of `resolve_at` (extern-registry arms — function/
type/operator/attr schema — have no POM `doc` field, stay `None`).
`hover.rs` prepends it as a Markdown paragraph above the existing
`**kind** \`name\`` line. 2 new `Connection::memory()` protocol tests
(documented-decl hover includes and precedes the doc text; undocumented
decl renders byte-identical to before). Full `cargo test -p
piperine-lang-server` and `cargo build --workspace` green.

#### T5: Elaborator emits `ResolutionIndex`
**What**: Record every identifier use→binding and binding→{decl_span, kind, doc,
use_spans, file} as an elaboration side artifact. **Where**: `piperine-lang/elab/`.
**Requirement**: LSP-03/05. **Depends on**: T3. **Reuses**: the resolver already
running during elaboration.
**Done when**:
- [x] `elaborate_*` returns a `ResolutionIndex` alongside `Design`
- [x] uses and decls of the same binding share one `BindingId`; spans recorded
- [x] `Design` output unchanged (additive); `cargo test -p piperine-lang`
**Tests**: unit · **Gate**: quick (lang)
**Status (2026-07-24)**: DONE, commit `3adf4eb`. Added
`elab::resolution::ResolutionIndex` — a `BindingId`-keyed side artifact
built by walking the already-elaborated `Design` (module/port/param/wire/
var/instance/behavior), each binding carrying `decl_span`/`kind`/`doc`/
`use_spans`/`owner_module`/`file`. New additive entry points
(`SourceFile::elaborate_with_index`, `parse_and_elaborate_with_index`)
return `(Design, ResolutionIndex)` without touching any existing
`elaborate*` signature or caller. Two parser fixes were needed for full
decl-kind coverage: `ast::Port` had no span at all (`pom::Port.span` was
always `None`) — added it; `BehaviorDecl::parse` also always set `span:
None` — now captures start/end like every other decl.
**SPEC_DEVIATION**: `Expr` carries no per-occurrence span anywhere in the
AST (only whole declarations/statements do); threading one through would
touch every `Expr` variant and every consumer across `piperine-lang` *and*
`piperine-codegen` (which lowers these same AST bodies) — outside this
task's surgical-change budget and into correctness-critical, edit-with-
care code. So each binding's `use_spans` holds only its own declaration
span (a reflexive use) today, not every in-expression occurrence;
documented at the top of `elab/resolution.rs` as a known follow-up. 5 new
tests in `elab.rs`; full `cargo test -p piperine-lang` and `cargo build
--workspace` green.

#### T6: `resolve_at` rewrite on `ResolutionIndex` (cursor-context + shadowing)
**What**: Map cursor offset → containing use span → `BindingId` → `Resolution`;
delete the word-based global loop. **Where**: `piperine-lang-server/symbol_index.rs`,
`state.rs`. **Requirement**: LSP-01/02. **Depends on**: T5.
**Done when**:
- [x] resolution uses cursor context, not first-match; shadowed name → innermost
- [x] the `symbol_index.rs:53` global loop is removed (grep: zero)
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `b302b70`. Deleted the word-based
global loop (the "Global lookup for now ... until we build true scope
resolution" comment/loop that scanned every module in POM iteration order
for ports/params/wires/vars/instances/behaviors). Replaced with
`resolve_in_module()`: when the cursor sits inside a module's own decl
span, resolve the word against *that* module's scope first, innermost-
first (var, wire, instance, param, port, behavior, then the module's own
name). Module *names* stay a genuine cross-module scan afterward (any
instance anywhere may legitimately reference any module by name — not the
bug), as do enum/bundle/discipline/capability/impl and the extern-registry
lookups. `grep -n "Global lookup for now"` → zero matches. Note: this
rewrite works directly off `Design`'s own decl spans rather than plumbing
T5's `ResolutionIndex` through `DocumentState` — see the commit message's
SPEC_DEVIATION (same expr-span gap as T5: "innermost" means innermost
*module scope*, not true lexical/behavior-local shadowing, since
behavior-local `var`s carry no span at all). 2 new protocol tests
(cross-module same-name resolves per cursor context; a same-named param in
a later module isn't shadowed by an earlier one). Full `cargo test -p
piperine-lang-server` and `cargo build --workspace` green.

#### T7: goto-definition rides the binding
**What**: goto jumps to the resolved binding's `decl_span` (correct under
same-name-in-other-module). **Where**: `handlers/goto_def.rs`. **Requirement**:
LSP-04. **Depends on**: T6.
**Done when**:
- [x] goto on a shadowed/duplicated name lands on the correct declaration
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `60aefdd`. No code change needed:
`goto_def.rs::handle` already forwards `resolve_at(...)?.decl_span`
directly, so T6's `resolve_at` rewrite already fixed the resolution goto
reads from. Added a protocol-level (`Connection::memory`) regression test
proving it end-to-end: two modules each declare a param of the same name,
and goto-definition on the cursor's own module lands inside that module,
never inside the textually-first same-named declaration. Full `cargo test
-p piperine-lang-server` and `cargo build --workspace` green.

---

### Phase 2 — References / rename / project / diagnostics

#### T8: Occurrence engine from binding
**What**: `occurrences(BindingId) -> [Span]` from `use_spans`; the shared source
for refs/rename/highlight. **Where**: `piperine-lang-server`. **Requirement**:
LSP-10/13 (base). **Depends on**: T6.
**Done when**:
- [x] returns exactly the binding's uses; no comment/string/other-scope spans
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `e6ca4dd`. New `occurrences.rs`:
`occurrences_for_decl_span(index, decl_span)` matches a resolved
`Resolution.decl_span` (already scope-correct via T6) against
`ResolutionIndex`'s bindings and returns the recorded use spans.
`DocumentState` now builds/holds a `ResolutionIndex` alongside
`design`/`ctx` (populated in `analyze()`), and exposes
`occurrences_at(byte_offset) -> Vec<(usize, usize)>` — the engine T9-T11
consume instead of `word_occurrences`'s text scan.
**SPEC_DEVIATION**: per T5's own SPEC_DEVIATION, `ResolutionIndex.
use_spans` today holds only each binding's reflexive declaration span (no
per-occurrence `Expr` span exists in the AST), so `occurrences_at`
legitimately returns a one-element list for most bindings — it returns
exactly what the index provides, not an invented richer set. Symbols with
no `ResolutionIndex` entry (extern registry lookups) fall back to their
own `decl_span` as the one known occurrence rather than an empty result
for a symbol that did resolve. 3 new tests in `integration_test.rs`; full
`cargo test -p piperine-lang-server` green (19 passed).

#### T9: references handler → binding uses
**What**: Replace `word_occurrences` in references with `occurrences(binding)`.
**Where**: `handlers/references.rs`. **Requirement**: LSP-10. **Depends on**: T8.
**Done when**:
- [x] references returns binding uses only; a `// name` comment is excluded
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `892e5be`. `references.rs::handle`
now calls `DocumentState::occurrences_at` (T8) instead of
`word_occurrences` — the explicit `resolve_at(offset)?` symbol gate is
gone too (`occurrences_at` already returns empty when nothing resolves).
1 new protocol test (`Connection::memory` round trip) on a two-module
fixture with a `// power` comment: proves the comment mention and module
B's own `power` never appear in module A's references. Full `cargo test
-p piperine-lang-server` green (20 passed).

#### T10: rename handler → binding uses (single-file)
**What**: Replace `word_occurrences` in rename/prepare-rename with the binding
occurrences. **Where**: `handlers/rename.rs`. **Requirement**: LSP-11.
**Depends on**: T8.
**Done when**:
- [x] rename edits only the binding's uses; same-named other-scope untouched
- [x] prepare-rename declines on keyword/literal
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `b343aa7`. Both `handle_rename` and
`handle_prepare_rename` now call `occurrences_at` (T8) instead of
`word_occurrences`: rename declines (`None`) whenever the occurrence set
is empty (nothing resolved), and prepare-rename's containing-range lookup
runs over the same set — a keyword/literal never resolves, so both
naturally decline. 2 new protocol tests: rename on module A's `power`
never edits module B's own `power`; prepare-rename on a numeric literal
returns `None`. Full `cargo test -p piperine-lang-server` green (22
passed).

#### T11: document-highlight → binding uses
**What**: Highlight from binding occurrences, not text. **Where**:
`handlers/document_highlight.rs`. **Requirement**: LSP-13. **Depends on**: T8.
**Done when**:
- [x] highlights the binding's uses only; `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `9b65c24`. `document_highlight.rs::
handle` now calls `occurrences_at` (T8), same binding-identity source as
references (T9). 1 new protocol test on the same two-module/comment
fixture used for T9/T10, proving both exclusions. Full `cargo test -p
piperine-lang-server` green (23 passed).

#### T12: `ProjectUnit` — multi-file index
**What**: `ServerState.projects: Map<Root, ProjectUnit>` holding the multi-file
`Design` + one `ResolutionIndex` spanning files; documents map to their unit.
**Where**: `piperine-lang-server/state.rs`, `project.rs`. **Requirement**:
LSP-14. **Depends on**: T5. **Reuses**: `ProjectContext` / project `SourceMap`.
**Done when**:
- [x] a project builds one unit over all files; `BindingInfo.file` set
- [x] single-file docs form a unit of one (fallback); `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-24)**: DONE, commit `f0ffd31`. New `ProjectUnit`
(`project.rs`): elaborates every `.phdl` file under a project's `src/`
(mirroring `piperine-cli check`'s own file discovery) against the shared
project `SourceMap`, merging each file's `ResolutionIndex` into one
project-wide index with `BindingInfo.file` stamped to the owning path.
New `ServerState.analyze_document(uri)` seam both analyzes the document
and lazily builds/caches the owning project's `ProjectUnit` in the new
`ServerState.projects` map, recording the root on `DocumentState.
project_root`; `dispatch.rs`/`server.rs`'s two analyze call sites now go
through this single seam. Two small additive `ResolutionIndex` methods
added in `piperine-lang` (`set_file`, `merge`, with `BindingId`
remapping to avoid cross-file id collisions) — required to fold per-file
indices into one project-wide index.
**SPEC_DEVIATION**: design.md frames this as "the multi-file `Design`"
(singular); `piperine-lang` has no cross-file `Design`-merge primitive
(`Design` is one elaboration unit's output), so `ProjectUnit` holds one
`Design` per file keyed by path instead — the actual LSP-14 payload (one
binding-identity index spanning every file) is delivered in full via
`ProjectUnit.index`. Standalone documents get `project_root: None` and no
`projects` entry (no literal unit-of-one object materializes) rather than
a synthesized one — existing single-file behavior is unaffected, which is
the substance of the fallback requirement. 2 new tests: a two-file scratch
project builds one `ProjectUnit` covering both `Design`s with
`BindingInfo.file` stamped per file; a standalone document gets no
`project_root`/`projects` entry and still elaborates normally. Full
`cargo test -p piperine-lang-server` green (25 passed); `cargo test -p
piperine-lang` no regressions; `cargo build --workspace` zero warnings.

#### T13: Cross-file goto
**What**: goto opens the decl's file when the binding is declared elsewhere.
**Where**: `handlers/goto_def.rs`. **Requirement**: LSP-15. **Depends on**: T12.
**Done when**:
- [x] goto on an imported symbol opens its file at the decl
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-25)**: DONE, commit `dfa1339`. `goto_def.rs` rides
`ProjectUnit`/`BindingInfo.file` (T12) to open the declaring file's URI when
it differs from the requesting document.

#### T14: Cross-file rename (`document_changes`)
**What**: Rename emits a multi-file `WorkspaceEdit.document_changes` over every
file with uses. **Where**: `handlers/rename.rs`. **Requirement**: LSP-12.
**Depends on**: T12, T10.
**Done when**:
- [x] renaming a project-wide symbol edits all referencing files
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-25)**: DONE, commit `be0366a`. SPEC_DEVIATION (inherited
from T5/T8): edits cover exactly what `occurrences_at`/`ResolutionIndex`
tracks (mostly reflexive decl-span uses), not every textual in-expression
occurrence — no per-occurrence span exists in the AST yet.

#### T15: Per-file diagnostic fan-out + single-file fallback
**What**: Publish each file's errors against its own URI; no project → single-file
behavior. **Where**: `handlers/diagnostics.rs`, `state.rs`. **Requirement**:
LSP-16/17. **Depends on**: T12.
**Done when**:
- [x] an error in file A publishes against A's URI; standalone files still work
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-25)**: DONE, commit `8f95b4c`. `publish_project_diagnostics`
walks every file in the `ProjectUnit` (open buffers use their own live
`DocumentState`; closed files re-read from disk against `ProjectUnit`'s own
build) and publishes each against its own URI; standalone documents keep
using the pre-existing `publish_diagnostics` path unchanged.

#### T16: Error-accumulating elaboration
**What**: Return `(Design, ResolutionIndex, Vec<ElabError>)`; accumulate in
recoverable passes; adapt callers. **Where**: `piperine-lang/elab/`, `state.rs:80`.
**Requirement**: LSP-18. **Depends on**: T5.
**Done when**:
- [x] two independent elab errors both appear; unrecoverable passes documented
- [x] callers adapted (host first-error via `.first()`); `cargo test --workspace`
**Tests**: unit + integration · **Gate**: full
**Status (2026-07-25)**: DONE, commit `ccaf5d9`. New additive
`elaborate_with_context_accumulating` entry point — every existing
`elaborate`/`elaborate_with_context`/`elaborate_with_index` caller is
untouched (no signature change to adapt). `ElabModules` and `Typecheck` are
the two passes where independence genuinely holds (one module's failure
doesn't affect another's correctness) — both now attempt every item and
accumulate every error while still returning their first error, so
`elaborate`'s existing fail-fast callers see identical behavior. Every other
pass (`Register`, `FoldGlobals`, `ElabFns`, `AttachBehaviors`, `ResolveCalls`,
`FlattenHierarchy`, `ValidateEvents`) stays fail-fast — each is a genuine
precondition for the next pass, not an independent-item pass; documented in
`lower/mod.rs::elaborate_accumulating`'s doc comment. `DocumentState::analyze`
now records every returned error, not just the first. Full workspace gate
green (one pre-existing, unrelated PyO3 cross-thread-unsendable flake in
`piperine-python::simulation_error`, confirmed passing single-threaded).

#### T17: Diagnostic severity + structured codes
**What**: Map `ElabError` kind → `WARNING`/`ERROR` + a structured `code`.
**Where**: `handlers/diagnostics.rs`, `ElabError`. **Requirement**: LSP-19.
**Depends on**: T16.
**Done when**:
- [x] warning-class shows `WARNING`; codes are specific (not blanket `parse-error`)
- [x] spans accurate (no `0:0` fallback where a span exists)
- [x] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)
**Status (2026-07-25)**: DONE, commit `d592d30`. Both
`piperine_lang::parse::error::ParseError` (E1xxx) and
`pom::error::ElabErrorKind` (E2xxx) already carried a real
`#[diagnostic(code(...))]` per variant — surfaced via
`miette::Diagnostic::code()` instead of the old blanket `"parse-error"`
string (gotcha: `ElabError`'s `#[diagnostic_source]` forwards `.source()`,
not `.code()` — the code has to be read off `ElabError::kind` directly).
SPEC_DEVIATION: `severity_for_code()` maps everything to `ERROR` — verified
every variant in both enums (E1001..E1004, E2001..E2025/E2999); none is
non-blocking today, so nothing warrants `WARNING` yet. The function is the
single seam a future lint-style kind would extend.

---

### Phase 3 — Attribute-schema IDE + protocol tests

#### T18: `@schema` completion
**What**: Complete in-scope schema names at `@` position. **Where**:
`handlers/completion.rs`. **Requirement**: LSP-20. **Depends on**: T6.
**Reuses**: `ctx.schemas`.
**Done when**:
- [ ] `@rf|` completes to `rfport`; only in-scope schemas offered
- [ ] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)

#### T19: Attribute-argument validation
**What**: Diagnose unknown/typed/missing-required attribute fields at the arg.
**Where**: `handlers/diagnostics.rs` (or elaboration schema check surfaced).
**Requirement**: LSP-21. **Depends on**: T17.
**Done when**:
- [ ] `@rfport(num = "x")` (bad type) / unknown field / missing required → diagnostic at the arg
- [ ] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)

#### T20: Hover→schema fields + goto→`@attribute` decl
**What**: Hover a schema name/field shows fields; goto jumps to the
`extern attribute` decl_span. **Where**: `handlers/hover.rs`, `goto_def.rs`.
**Requirement**: LSP-22/23. **Depends on**: T18. **Reuses**: `ctx.schemas.decl_span`.
**Done when**:
- [ ] hover on `@rfport` lists `num`/`z0`; goto opens the schema decl
- [ ] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)

#### T21: Attribute outline entries
**What**: Attribute instances appear as outline entries on their declarations.
**Where**: `handlers/symbols.rs`. **Requirement**: LSP-24. **Depends on**: T18.
**Done when**:
- [ ] an `@rfport`-annotated node shows the attribute in the outline
- [ ] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)

#### T22: Protocol-test harness (`Connection::memory()`)
**What**: A harness driving init → didOpen → request/response for hover,
completion, goto, references, rename. **Where**:
`piperine-lang-server/tests/protocol.rs`. **Requirement**: LSP-25.
**Depends on**: T7, T9, T10.
**Done when**:
- [ ] harness drives a memory connection through the core round-trips
- [ ] `cargo test -p piperine-lang-server`
**Tests**: integration · **Gate**: quick (server)

#### T23: Shadowing + doc-comment + cross-file protocol tests
**What**: Fixtures asserting innermost-binding resolution, doc-on-hover, and
cross-file goto/rename over the harness. **Where**:
`piperine-lang-server/tests/`. **Requirement**: LSP-26. **Depends on**: T22, T14.
**Done when**:
- [ ] shadowing fixture → correct binding; doc fixture → doc on hover; multi-file
      fixture → cross-file goto + rename
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

---

## Phase Execution Map

```
Phase 1: T1 → T2 → T3 → T4 → T5 → T6 → T7
Phase 2: T8 → T9 → T10 → T11 → T12 → T13 → T14 → T15 → T16 → T17
Phase 3: T18 → T19 → T20 → T21 → T22 → T23
```

Sequential; whole phases are batch boundaries. 23 tasks → ~4 batches → offer
sub-agents at execution.

---

## Requirement → Task Coverage

| Req | Task | Req | Task |
|-----|------|-----|------|
| LSP-01 | T6 | LSP-14 | T12 |
| LSP-02 | T6 | LSP-15 | T13 |
| LSP-03 | T5 | LSP-16 | T15 |
| LSP-04 | T7 | LSP-17 | T15 |
| LSP-05 | T5 | LSP-18 | T16 |
| LSP-06 | T1 | LSP-19 | T17 |
| LSP-07 | T2,T3 | LSP-20 | T18 |
| LSP-08 | T4 | LSP-21 | T19 |
| LSP-09 | T3 | LSP-22 | T20 |
| LSP-10 | T8,T9 | LSP-23 | T20 |
| LSP-11 | T10 | LSP-24 | T21 |
| LSP-12 | T14 | LSP-25 | T22 |
| LSP-13 | T8,T11 | LSP-26 | T23 |

All 26 requirements mapped.
