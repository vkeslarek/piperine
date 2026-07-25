# Language Server Follow-up Bugs Tasks

**Spec**: `.specs/features/language-server-followup-bugs/spec.md`
**Design**: `.specs/features/language-server-followup-bugs/design.md`
**Status**: Draft

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
- [ ] every prelude item (incl. `ddt`, `Real`) maps to its real header path
- [ ] every `use`-loaded item maps to its real on-disk path
- [ ] `cargo test -p piperine-lang`
**Gate**: quick (lang)

#### T2 (LSB-01..03): Thread item_files into Design/Project
**What**: `Project` gains `item_files: HashMap<String, PathBuf>` + a setter
+ `item_file(name) -> Option<&Path>` query, populated the same way
`origins`/`set_project_meta` already are.
**Where**: `crates/piperine-lang/src/pom/design.rs` (or wherever `Project`
lives), `elab/mod.rs` (elaboration entry points call the new setter).
**Depends on**: T1.
**Done when**:
- [ ] `design.project().item_file("ddt")` returns the real header path
- [ ] `cargo test -p piperine-lang`
**Gate**: quick (lang)

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
- [ ] goto on `ddt` returns a `Location` at `headers/operators.phdl`
- [ ] goto on a same-file `extern` decl still works (no regression)
- [ ] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

#### T4 (LSB-04..06): extern doc field (AST + parser)
**What**: `doc: Option<String>` on `ExternSig` and `ExternDecl::{Type,
Attribute,Impl}`; each `parse_extern_*` captures `parser.current_doc()`.
**Where**: `crates/piperine-lang/src/parse/ast.rs`,
`parse/parser/extern_decl.rs`.
**Done when**:
- [ ] `extern operator ddt` preceded by `///` parses with `doc: Some(...)`
- [ ] no `///` → `doc: None`
- [ ] `cargo test -p piperine-lang`
**Gate**: quick (lang)

#### T5 (LSB-04..06): thread doc into registries + Resolution
**What**: `doc: Option<String>` on `ExternOperatorDecl`/`TypeDefKind::
Extern`/callable/schema registry structs, populated from the AST `doc`
during registration. `symbol_index.rs`'s extern arms read `.doc` instead of
hardcoded `None`.
**Where**: `crates/piperine-lang/src/elab/registry/{operators,types,
callables,schemas}.rs`, `crates/piperine-lang-server/src/symbol_index.rs`.
**Depends on**: T4.
**Done when**:
- [ ] hover on a `///`-documented `extern` shows the doc as Markdown
- [ ] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

#### T6 (LSB-04..06): author `///` docs on headers
**What**: Convert the existing `//` prose directly above `extern operator
ddt` (`headers/operators.phdl`) and the primitive types
(`headers/types.phdl`) to `///`.
**Where**: `crates/piperine-lang/headers/{operators,types}.phdl`.
**Depends on**: T5.
**Done when**:
- [ ] hover on `ddt` in a real PHDL file shows its authored doc
- [ ] every existing header/prelude test still elaborates (no `//`→`///`
      accidentally breaking a non-doc comment elsewhere in the file)
- [ ] `cargo test -p piperine-lang`
**Gate**: quick (lang)

#### T7 (LSB-07..10): instance token-level spans (AST/POM)
**What**: `label_span`/`type_span: Option<SourceSpan>` on
`ast::ModuleStatement::Instance`, mirrored on `pom::Instance`, captured at
the exact label/type-name token positions during parsing, threaded through
AST→POM lowering.
**Where**: `crates/piperine-lang/src/parse/ast.rs`,
`parse/parser/stmt.rs`, `pom/module.rs`, elaboration's instance-lowering.
**Done when**:
- [ ] `label_span` covers only the label token; `type_span` only the type
      token; both `None`→fallback-safe if genuinely absent
- [ ] `cargo test -p piperine-lang`
**Gate**: quick (lang)

#### T8 (LSB-07..10): symbol_index.rs + ResolutionIndex consistency
**What**: `resolve_in_module`'s Instance arm picks `label_span`/`type_span`
per the design's convention; `elab/resolution.rs::index_design` indexes
`BindingKind::Instance` by the *same* convention (byte-for-byte agreement
required for `occurrences_for_decl_span`'s exact match).
**Where**: `crates/piperine-lang-server/src/symbol_index.rs`,
`crates/piperine-lang/src/elab/resolution.rs`.
**Depends on**: T7.
**Done when**:
- [ ] highlighting the reported fixture's `"src"` returns a 3-byte range
- [ ] highlighting `"RampSource"` on that same instance returns a
      10-byte range (not the 56-byte whole statement)
- [ ] unlabeled instance highlight unchanged/still-correct
- [ ] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

#### T9 (LSB-11..13): completion suppression heuristic
**What**: `build_completions_predictive` suppresses `for`/`match`/`return`/
`when` and merges `add_top_level_completions` when both
`Punctuation(RBrace)` and a mod-body-only keyword (`param`/`wire`) are
present in `expected`.
**Where**: `crates/piperine-lang-server/src/handlers/completion.rs`.
**Done when**:
- [ ] completion right after `mod A(...) { param r: Real = 1.0; }`'s `}`
      never includes `"for"`, does include `"mod"`
- [ ] completion mid-behavior-body (genuine `for`-valid position) still
      includes `"for"` — no regression
- [ ] `cargo test -p piperine-lang-server`
**Gate**: quick (server)

#### T10: Full workspace gate + Verifier close-out
**What**: `cargo test --workspace`; update this file's Status notes;
dispatch the standalone-fallback Verifier (spec-anchored check across all
13 LSB requirements + discrimination sensor); write `validation.md`.
**Depends on**: T1-T9.
**Done when**:
- [ ] `cargo test --workspace` green (or pre-existing unrelated flakes
      confirmed, documented)
- [ ] `validation.md` written, PASS
**Gate**: full

---

## Requirement → Task Coverage

| Req | Task | Req | Task |
|---|---|---|---|
| LSB-01..03 | T1,T2,T3 | LSB-07..10 | T7,T8 |
| LSB-04..06 | T4,T5,T6 | LSB-11..13 | T9 |
