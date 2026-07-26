# Language Server Follow-up Bugs Tasks

**Spec**: `.specs/features/language-server-followup-bugs/spec.md`
**Design**: `.specs/features/language-server-followup-bugs/design.md`
**Status**: Complete — all 10 tasks / 13 requirements delivered, Verifier PASS

## Test Coverage Matrix

| Code Layer | Required Test Type | Location | Run Command |
|---|---|---|---|
| Resolver item-file tracking + Project query | unit | `crates/piperine-lang/tests/{elab,resolve}.rs` | `cargo test -p piperine-lang` |
| Extern doc capture (AST/parser/registries) | unit | `crates/piperine-lang/tests/elab.rs` | `cargo test -p piperine-lang` |
| Instance token-level spans (AST/POM/index) | unit | `crates/piperine-lang/tests/elab.rs` | `cargo test -p piperine-lang` |
| goto/hover/highlight/completion handlers | integration | `crates/piperine-lang-server/tests/*.rs` | `cargo test -p piperine-lang-server` |

## Gate Check Commands

| Gate | Command |
|---|---|
| Quick (lang) | `cargo test -p piperine-lang` |
| Quick (server) | `cargo test -p piperine-lang-server` |
| Full | `cargo test --workspace` |

## Execution Plan

```
T1 → T2 → T3 (BUG-1)
T4 → T5 → T6 (BUG-2)
T7 → T8 (BUG-3)
T9 (BUG-4)
T10 (full gate + Verifier)
```

**Batch packing**: 10 tasks > 8 → offer sub-agents. B1 = T1-T5 (BUG-1 full +
BUG-2 pipeline), B2 = T6-T10 (BUG-2 headers, BUG-3, BUG-4, close-out).

---

## Task Breakdown

#### T1 (LSB-01..03): Resolver item-file tracking
**What**: `Resolver.item_files: HashMap<String, PathBuf>`, populated in
`prelude_items()` (hardcoded `CARGO_MANIFEST_DIR`-relative paths for the 5
embedded headers) and `load_source()`/`expand_inner()` (the real path
already computed there). `take_item_files()` mirrors `take_origins()`.
**Where**: `crates/piperine-lang/src/resolve.rs`.
**Done when**:
- [x] every prelude item (incl. `ddt`, `Real`) maps to its real header path
- [x] every `use`-loaded item maps to its real on-disk path
- [x] `cargo test -p piperine-lang`
**Gate**: quick (lang)

**Status (2026-07-25)**: DONE, commit `96a0b1f`. Added `Resolver.item_files:
HashMap<String, PathBuf>` and `file_paths: HashMap<Vec<String>, PathBuf>`
(a resolved-path cache so `expand_inner` can tag items without
recomputing path resolution). The 5 embedded headers are tagged via
`concat!(env!("CARGO_MANIFEST_DIR"), "/headers/X.phdl")`; the dynamically
loaded `piperine::{capabilities,collections,prelude}` and every
`use`-loaded file are tagged from `file_paths` at the point `origins` is
already populated. `take_item_files()` mirrors `take_origins()`. Two new
tests in `tests/elab.rs` verify `ddt`/`Real` map to real, existing files
on disk, and a `use`-loaded item maps to its real tempdir path.

#### T2 (LSB-01..03): Thread item_files into Design/Project
**What**: `Project` gains `item_files: HashMap<String, PathBuf>` + a setter
+ `item_file(name) -> Option<&Path>` query, populated the same way
`origins`/`set_project_meta` already are.
**Where**: `crates/piperine-lang/src/pom/design.rs` (or wherever `Project`
lives), `elab/mod.rs` (elaboration entry points call the new setter).
**Depends on**: T1.
**Done when**:
- [x] `design.project().item_file("ddt")` returns the real header path
- [x] `cargo test -p piperine-lang`
**Gate**: quick (lang)

**Status (2026-07-25)**: DONE, commit `06468c5`. Added `Project.item_files`
+ `Project::item_file(name) -> Option<&Path>` (mirrors `origin_of`'s
`Dac__8`→`Dac` monomorphized-name fallback). `Design::set_item_files`
called alongside `set_origins` at all three elaboration entry points in
`elab/mod.rs`. One new test verifies `design.project().item_file("ddt")`
resolves to the real, existing `headers/operators.phdl`.

#### T3 (LSB-01..03): goto_def.rs extern cross-file resolution
**What**: `Resolution.file: Option<PathBuf>` populated in `symbol_index.rs`'s
extern arms from `design.project().item_file(word)`. `goto_def.rs::
cross_file_location` gains a branch for `resolution.file.is_some()` that
reads the target file directly and returns its `Location` — checked before
the existing Module/Instance branch.
**Where**: `crates/piperine-lang-server/src/symbol_index.rs`,
`handlers/goto_def.rs`.
**Depends on**: T2.
**Done when**:
- [x] goto on `ddt` returns a `Location` at `headers/operators.phdl`
- [x] goto on a same-file `extern` decl still works (no regression)
- [x] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

**Status (2026-07-25)**: DONE, commit `4b4a5ae`. Added `Resolution.file:
Option<PathBuf>`, populated in `symbol_index.rs`'s extern-registry arms
from `design.project().item_file(&word)`. `goto_def.rs::
cross_file_location` gains a branch, checked before the existing
Module/Instance logic: when `resolution.file` is set and (after
canonicalization) differs from the current document, read that file's
text directly and return its `Location`; when it's the same file, falls
through to the existing same-file fallback (no regression). Verified:
`goto_definition_on_ddt_lands_on_operators_header` opens the real
`crates/piperine-lang/headers/operators.phdl` and asserts the returned
range's byte offset exactly matches `header_text.find("extern operator
ddt")` — confirmed against the actual file on disk, not just "code
compiles."

#### T4 (LSB-04..06): extern doc field (AST + parser)
**What**: `doc: Option<String>` on `ExternSig` and `ExternDecl::{Type,
Attribute,Impl}`; each `parse_extern_*` captures `parser.current_doc()`.
**Where**: `crates/piperine-lang/src/parse/ast.rs`,
`parse/parser/extern_decl.rs`.
**Done when**:
- [x] `extern operator ddt` preceded by `///` parses with `doc: Some(...)`
- [x] no `///` → `doc: None`
- [x] `cargo test -p piperine-lang`
**Gate**: quick (lang)

**Status (2026-07-25)**: DONE, commit `bab93d5`. Added `doc:
Option<String>` to `ExternSig` and `ExternDecl::{Type,Attribute,Impl}`;
each `parse_extern_*` function (including each individual method inside
`extern impl`) captures `parser.current_doc()` at the same point every
other decl parser does. `register.rs` and `piperine-plugin`'s
extern-attribute scanner pattern-match the new field with `..`/`doc: _`
for now (doc threading into the registries is T5). 8 new tests in
`tests/extern_grammar.rs` cover all 6 `extern` forms plus the
block-vs-per-method doc attachment edge case from spec.md.

#### T5 (LSB-04..06): thread doc into registries + Resolution
**What**: `doc: Option<String>` on `ExternOperatorDecl`/`TypeDefKind::
Extern`/callable/schema registry structs, populated from the AST `doc`
during registration. `symbol_index.rs`'s extern arms read `.doc` instead of
hardcoded `None`.
**Where**: `crates/piperine-lang/src/elab/registry/{operators,types,
callables,schemas}.rs`, `crates/piperine-lang-server/src/symbol_index.rs`.
**Depends on**: T4.
**Done when**:
- [x] hover on a `///`-documented `extern` shows the doc as Markdown
- [x] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

**Status (2026-07-25)**: DONE, commit `623c7e5`. Added `CallableDef::doc()`
default method (mirrors `decl_span()`), overridden on `ExternFnDecl`/
`ExternOperatorDecl` to read `sig.doc`. Added `doc` to
`TypeDefKind::Extern` and a `docs` store + `SchemaRegistry::doc()` to
`SchemaRegistry`, populated in `register.rs` (and `piperine-plugin`'s
extern-attribute-stub scanner) from the AST `doc` field captured in T4.
`symbol_index.rs`'s 5 extern-registry `Resolution` arms now read
`.doc()`/`.doc` instead of the hardcoded `None`. Verified with a
synthetic `///`-documented `extern operator` fixture end-to-end through
`Connection::memory()`: hover renders the doc as Markdown; an
undocumented sibling still renders unchanged. Authoring the real
`ddt`/`Real` header docs (`//` → `///`) is T6, not in this batch.

#### T6 (LSB-04..06): author `///` docs on headers
**What**: Convert the existing `//` prose directly above `extern operator
ddt` (`headers/operators.phdl`) and the primitive types
(`headers/types.phdl`) to `///`.
**Where**: `crates/piperine-lang/headers/{operators,types}.phdl`.
**Depends on**: T5.
**Done when**:
- [x] hover on `ddt` in a real PHDL file shows its authored doc
- [x] every existing header/prelude test still elaborates (no `//`→`///`
      accidentally breaking a non-doc comment elsewhere in the file)
- [x] `cargo test -p piperine-lang`
**Gate**: quick (lang)

**Status (2026-07-25)**: DONE, commit `5d51952`. Converted the `//` block
directly above `extern operator ddt` (5 lines, no blank-line break) and
the `//` block directly above `extern type Real` (6 lines) to `///` —
both blocks sit with no blank line between the comment and the
declaration, so the lexer's attach rule picks them up whole. New test
`test_ddt_doc_comes_from_the_real_header_content` elaborates a real
`ddt(...)`-using analog body, looks up `ctx.operators.lookup("ddt")
.doc()`, and asserts the returned string contains prose read straight
from `headers/operators.phdl` on disk (not a hardcoded fixture string) —
proving the pipeline end-to-end. Full `cargo test -p piperine-lang`
green (61→64 tests across the suite, no regressions).

#### T7 (LSB-07..10): instance token-level spans (AST/POM)
**What**: `label_span`/`type_span: Option<SourceSpan>` on
`ast::ModuleStatement::Instance`, mirrored on `pom::Instance`, captured at
the exact label/type-name token positions during parsing, threaded through
AST→POM lowering.
**Where**: `crates/piperine-lang/src/parse/ast.rs`,
`parse/parser/stmt.rs`, `pom/module.rs`, elaboration's instance-lowering.
**Done when**:
- [x] `label_span` covers only the label token; `type_span` only the type
      token; both `None`→fallback-safe if genuinely absent
- [x] `cargo test -p piperine-lang`
**Gate**: quick (lang)

**Status (2026-07-25)**: DONE, commit `c29fcb6`. Captured both spans in
`parser/stmt.rs`'s instance-parsing function at the point each identifier
is read (`current_span_start()`/`previous_span_end()` around
`parse_ident()`), for both the labeled (`label : Type`) and unlabeled
(bare `Type`) forms — unlabeled leaves `label_span: None`, `type_span`
set to the single identifier's span. Mirrored onto `pom::Instance`
(`#[serde(skip)]`, matching `span`), threaded through
`elab/lower/module.rs::lower_instance`. Synthetic `Instance` construction
sites with no source tokens (hierarchy-flattening splice, runtime-staged
instances, and existing test fixtures in `flatten.rs`/`typecheck.rs`/
`selector/eval.rs`) set both fields to `None`. New tests confirm exact
3-byte/10-byte token spans on spec.md's reported fixture shape.

#### T8 (LSB-07..10): symbol_index.rs + ResolutionIndex consistency
**What**: `resolve_in_module`'s Instance arm picks `label_span`/`type_span`
per the design's convention; `elab/resolution.rs::index_design` indexes
`BindingKind::Instance` by the *same* convention (byte-for-byte agreement
required for `occurrences_for_decl_span`'s exact match).
**Where**: `crates/piperine-lang-server/src/symbol_index.rs`,
`crates/piperine-lang/src/elab/resolution.rs`.
**Depends on**: T7.
**Done when**:
- [x] highlighting the reported fixture's `"src"` returns a 3-byte range
- [x] highlighting `"RampSource"` on that same instance returns a
      10-byte range (not the 56-byte whole statement)
- [x] unlabeled instance highlight unchanged/still-correct
- [x] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

**Status (2026-07-25)**: DONE, commit `38617ad`. `symbol_index.rs`'s
Instance resolve arm now picks `label_span.or(i.span)`/`type_span.or
(i.span)` per which word matched; `elab/resolution.rs::index_design`
indexes `BindingKind::Instance` by the identical convention. Also fixed
`symbol_index::instance_module_type_at` (previously matched only against
the whole-statement `i.span`, now stale for a token-tight `decl_span`) to
match against `span`/`label_span`/`type_span` — required to keep
cross-file goto-definition on an instance's type name working (caught by
the pre-existing `cross_file_goto_opens_the_declaring_file` regression
test, which failed until this was fixed). New `instance_highlight.rs`
integration tests reproduce spec.md's exact fixture. **Mandatory
discrimination check performed as instructed**: reverted just
`index_design`'s half of the fix (kept `i.span`), confirmed the new
`resolved_decl_span_has_a_matching_binding_in_the_resolution_index` test
(and only that one) failed, then restored the real fix and reconfirmed
green — proving the two sides must actually agree, not that either fix
alone happens to pass by coincidence.

#### T9 (LSB-11..13): completion suppression heuristic
**What**: `build_completions_predictive` suppresses `for`/`match`/`return`/
`when` and merges `add_top_level_completions` when both
`Punctuation(RBrace)` and a mod-body-only keyword (`param`/`wire`) are
present in `expected`.
**Where**: `crates/piperine-lang-server/src/handlers/completion.rs`.
**Done when**:
- [x] completion right after `mod A(...) { param r: Real = 1.0; }`'s `}`
      never includes `"for"`, does include `"mod"`
- [x] completion mid-behavior-body (genuine `for`-valid position) still
      includes `"for"` — no regression
- [x] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

**Status (2026-07-25)**: DONE, commit `a2324f7`. Added the post-pass
exactly per design.md's pseudocode: when `expected` contains both
`Punctuation(Tok::RBrace)` and a `Keyword("param")`/`Keyword("wire")`,
drop `for`/`match`/`return`/`when` from `items` and merge in
`add_top_level_completions`. `if`/`var` deliberately untouched. New
`completion_suppression.rs` tests reproduce spec.md's exact fixture and
confirm the mid-behavior-body case (a genuine `for`-valid position) is
unaffected.

#### T10: Full workspace gate + Verifier close-out
**What**: `cargo test --workspace`; update this file's Status notes;
dispatch the standalone-fallback Verifier (spec-anchored check across all
13 LSB requirements + discrimination sensor); write `validation.md`.
**Depends on**: T1-T9.
**Done when**:
- [x] `cargo test --workspace` green (or pre-existing unrelated flakes
      confirmed, documented)
- [x] `validation.md` written, PASS
**Gate**: full

**Status (2026-07-25)**: DONE. B2's batch worker committed T6-T9 cleanly
and paused (session limit) before finishing this task's own gate/report;
the orchestrator completed it as a standalone Verifier pass:
`cargo test --workspace` confirmed green except the pre-existing,
unrelated `piperine-plugin::process_smoke::dead_guest_is_a_loud_error`
flake (passes single-threaded). Ran 2 independent discrimination-sensor
mutations (BUG-1's `item_file` lookup, BUG-4's suppression condition —
both killed) on top of T8's own inline sensor check, then wrote
`validation.md` — **PASS**, all 13 LSB requirements evidence-checked.

---

## Requirement → Task Coverage

| Req | Task | Req | Task |
|---|---|---|---|
| LSB-01..03 | T1,T2,T3 | LSB-07..10 | T7,T8 |
| LSB-04..06 | T4,T5,T6 | LSB-11..13 | T9 |
