# Language Server Follow-up Bugs — Design

## BUG-1: extern goto — file-path tracking

**New concept**: `Resolver` gains a second provenance map alongside its
existing `origins: HashMap<String, String>` (item name → package):

```rust
/// Item-name -> the real on-disk file it was declared in (BUG-1/LSB-01..03).
item_files: HashMap<String, PathBuf>,
```

Populated in two places:
- `prelude_items()`: each of the 5 `include_str!`-embedded headers
  (`types.phdl`/`math.phdl`/`tasks.phdl`/`operators.phdl`/
  `introspection.phdl`) gets a **hardcoded real path** via
  `concat!(env!("CARGO_MANIFEST_DIR"), "/headers/X.phdl")` — same pattern
  already used by `piperine-project/src/source_map.rs`'s own header
  fallback. After parsing each block, tag every item's name → that path.
- `load_source()`/`expand_inner()`: the real `file_path` is already computed
  there (reading from disk) — tag every loaded item's name → that path
  right after a successful load, alongside the existing `origins` tagging.

Exposed via `take_item_files(&mut self) -> HashMap<String, PathBuf>`
(mirrors `take_origins`), threaded into `Design`/`Project` the same way
`origins` already is (`Design::set_project_meta`-adjacent — add a new
`item_files: HashMap<String, PathBuf>` field on `Project`, a setter, and a
query method `Project::item_file(name: &str) -> Option<&Path>`).

**`Resolution` gains**: `pub file: Option<PathBuf>`. Populated in
`symbol_index.rs`'s extern-registry arms (`Type`/`Operator`/`Function`/
`AttrSchema`) from `design.project().item_file(&word)`. POM-level arms
(module/port/param/etc.) leave it `None` — T13's existing
`ProjectUnit`/`cross_file_location` machinery already handles those via a
different path; don't duplicate.

**`goto_def.rs::cross_file_location`**: add a branch checked *before* the
existing Module/Instance branch: if `resolution.file.is_some()` and differs
from the current document's own path, read that file's text from disk,
compute the `Location` directly from `decl_span` against it — no
`ProjectUnit`/module-search needed (extern items don't need cross-design
lookup, just the file + the same span already resolved).

## BUG-2: extern doc capture

**AST**: add `doc: Option<String>` to `ExternSig` (covers `extern fn`/
`extern task`/`extern operator`, and each method inside `extern impl`) and
to `ExternDecl::Type`/`Attribute`/`Impl`'s struct-variant fields (mirrors
`span`'s existing per-variant placement).

**Parser** (`parser/extern_decl.rs`): each `parse_extern_*` function reads
`parser.current_doc()` at the same point every other decl parser does
(before consuming the leading keyword/attribute), same convention as T3.

**Registries** (`elab/registry/{operators,types,callables,schemas}.rs`):
add `doc: Option<String>` to `ExternOperatorDecl`, `TypeDefKind::Extern`,
the callable struct(s), and the schema struct; populate from the AST's
`doc` field wherever these registries are built from `ExternDecl` (the
`Register`-pass-adjacent registration code — locate via existing
`decl_span`-population call sites, add `doc` alongside each).

**`symbol_index.rs`**: the extern-registry `Resolution` arms read `.doc`
instead of the hardcoded `None`.

**Headers**: convert the `///`-eligible lines directly above `extern
operator ddt` (`headers/operators.phdl`) and the primitive type decls
(`headers/types.phdl`) from `//` to `///`. Content: concise, one-paragraph,
matching the existing prose already there (rewritten as doc form, not
invented from scratch).

## BUG-3: instance token-level spans

**AST** (`ast::ModuleStatement::Instance`, `parser/stmt.rs`): add
`label_span: Option<miette::SourceSpan>` (captured right when the label
identifier is parsed, before the `:`) and `type_span: Option<miette::
SourceSpan>` (captured right when the module-type identifier is parsed —
for both the labeled `: Type` form and the unlabeled bare form, since
today's parser reuses the same `name`/`module_name` variables for both).

**POM** (`pom::Instance`): mirror both fields (parallel to the existing
`span`/`doc`), threaded through elaboration's AST→POM lowering the same way
`span` already is.

**`symbol_index.rs`**'s Instance resolve arm (`resolve_in_module`, ~line
73): when `i.label.as_deref() == Some(word)`, `decl_span: i.label_span.or
(i.span)`; when `i.module == word`, `decl_span: i.type_span.or(i.span)` —
`.or(i.span)` as a defensive fallback (should never actually be needed
once both are always captured).

**`elab/resolution.rs`**'s `index_design` (BUG-3's consistency
requirement, spec.md's BUG-3 Assumption row): index the `BindingKind::
Instance` entry by the *same* span convention — `i.label_span.unwrap_or
(i.type_span.unwrap_or(i.span))` when labeled, `i.type_span.unwrap_or
(i.span)` when not — so `occurrences_for_decl_span`'s exact offset+len
match against `symbol_index.rs`'s freshly-computed `decl_span` still
succeeds byte-for-byte. This is the one place a mismatch would silently
reintroduce the highlight bug — the discrimination sensor for this task
must specifically flip this convention and confirm the highlight test
fails.

## BUG-4: completion suppression heuristic

**`completion.rs`**, `build_completions_predictive`: after the existing
`for req in expected { match req { ... } }` loop populates `items`, add a
post-pass:

```rust
let has_rbrace = expected.iter().any(|e| matches!(e, ExpectedSyntax::Punctuation(Tok::RBrace)));
let has_mod_body_only = expected.iter().any(|e| matches!(e, ExpectedSyntax::Keyword(k) if k == "param" || k == "wire"));
if has_rbrace && has_mod_body_only {
    let behavior_only = ["for", "match", "return", "when"];
    items.retain(|it| !behavior_only.contains(&it.label.as_str()));
    add_top_level_completions(&mut items);
}
```

`if`/`var` are deliberately *not* suppressed — both are valid inside a
`mod{}` body too (a structural `if`/`var` default), so suppressing them
would be a new false negative, not a fix. Only `for`/`match`/`return`/
`when` are exclusively behavior-body constructs.

Re-sort/dedup already happens right after this point in the existing
function — no change needed there.

## Test strategy (per story)

- BUG-1: integration test via the existing `Connection::memory()` harness
  (`tests/protocol.rs` from `language-server`'s T22) — goto on `ddt` in a
  scratch project, assert the returned `Location.uri` points at
  `operators.phdl` and the range covers the real `extern operator ddt`
  text (read the real header file to build the expected range, not a
  hardcoded byte offset that would break if the header changes).
- BUG-2: `Connection::memory()` hover test — same pattern as
  `language-server`'s existing `hover_on_documented_module_renders_doc_as_markdown`.
- BUG-3: unit test in `piperine-lang-server` driving `DocumentState`
  directly (the pattern used during this bug's own repro) — assert exact
  byte ranges for label vs type clicks on the reported fixture.
- BUG-4: unit test calling `completions_at`/`build_completions_predictive`
  directly with the reported fixture, asserting `"for"` is absent and
  `"mod"` is present.
