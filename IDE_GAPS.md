# IDE_GAPS.md — Everything between today and a grade-A PHDL IDE experience

**Status: handoff draft (2026-07-03).** Full audit of `crates/piperine-lang-server/` (~1.2k LOC,
5 handlers) and `.vscode/` (extension, TextMate grammar, language configuration). The server
works as a demo: diagnostics, hover, completion, go-to-definition, and document symbols all
exist — but every one of them is built on string matching instead of real name resolution, and
the extension is explicitly a test harness (`"name": "piperine-lang-server-test"`).

Each gap: what it is, where it lives, why it matters, and a concrete fix sketch. Grouped in
tiers — **T0 foundations** (everything else builds on these), **T1 correctness** of the five
existing features, **T2 missing server features**, **T3 extension/UX**, **T4 packaging, tests,
CI**. Cross-cutting principle: the compiler already knows the answers (elaborated `Design`,
`predict_at_cursor`, the formatter) — the IDE layer must stop re-deriving them with regex.

---

## T0 — Foundations (fix these first; every feature inherits them)

### I1 — Spans: the AST barely has them, the POM has none

**Where:** `piperine-lang/src/parse/ast.rs` — only `ModuleStatement` carries
`span: Option<miette::SourceSpan>` (~18 mentions total). `Expr`, `Item`, `FnDecl`,
`BehaviorDecl`, `Port`/`Param`/`Wire` decls: no position. The POM (`pom/*`) drops source
positions entirely.

**Why it matters:** this is the root cause of half the gaps below. Without decl/use spans,
go-to-definition, references, rename, precise hover ranges, and correct document symbols are
all impossible — the handlers fall back to `source.find("mod Name")` textual search
(`goto_def.rs::find_decl`, `symbols.rs::find_decl_range`), which breaks on: the second
occurrence of a name, names appearing in comments/strings, `pub mod`, and shadowing.

**Fix sketch:**
1. Parser already tracks token positions (`Lexed` has offsets; `ParseError` carries
   `SourceSpan`). Thread a `span` field through the declaration AST nodes first (items,
   ports, params, wires, fns, behaviors, benches, enum variants, bundle fields) — decl spans
   unlock 90% of features; expression spans can come later for hover-on-expression.
2. POM nodes get `span: Option<SourceSpan>` copied at elaboration (`elab/lower/*` — the
   decl is in hand at construction time; one field per `pom::{Module, Port, Param, Wire,
   Instance, Behavior, Function}`).
3. Delete `find_decl`/`find_decl_range` string search; read spans off the POM.

**Size:** medium (mechanical but wide). **This is the single highest-leverage item.**

### I2 — No name-resolution index (every handler does its own ad-hoc, scope-less lookup)

**Where:** `hover.rs::lookup_hover_info` iterates *all* modules and returns the first
port/param/wire named `word` anywhere in the design — hovering `p` in module `B` can show
module `A`'s port. `goto_def.rs` same pattern. Completion doesn't offer scope names at all.

**Fix sketch:** one `symbol_index` module in the server (or better, in `piperine-lang` as a
query API over `Design` + AST): given `(document, byte_offset)` → the enclosing item
(module/behavior/bench/fn), its scope chain, and the resolved declaration for the identifier
under the cursor. The elaborator already resolves names (P5/P12 gave `LowerCtx` exact
name→id maps); the IDE needs the same resolution exposed as a *query* instead of a lowering
side effect. Concretely: `fn resolve_at(design, ast, offset) -> Option<Resolution>` where
`Resolution` = {kind, decl_span, type_info}. All five handlers then consume one function.

**Size:** medium-large. Depends on I1.

### I3 — UTF-16 position handling is wrong (and three different broken converters exist)

**Where:**
- `hover.rs::position_to_byte` returns a **byte** offset, then `word_at_position` indexes a
  `Vec<char>` with it — byte index into char vector: wrong for any non-ASCII document, and
  the loop logic itself (`col >= position.character`) is off-by-subtle.
- `completion.rs::position_to_offset` counts `char`s as columns.
- `goto_def.rs::word_at_position` is a third, separate reimplementation.
- LSP default position encoding is **UTF-16 code units** — none of the three handle
  surrogate pairs; a `μ` in a comment shifts every position after it.

**Fix sketch:** one `text_pos` module: `fn position_to_byte(source, Position) -> usize` and
inverse, UTF-16-aware (or negotiate `positionEncoding: "utf-8"` with clients that support
it — VS Code does since LSP 3.17 — and keep byte math). Delete the three copies. Property
test: round-trip on sources with emoji/accents.

**Size:** small. Do it before anything touches positions again.

### I4 — `SourceMap::dummy()`: multi-file projects don't work in the editor at all

**Where:** `state.rs:70` — elaboration runs with `SourceMap::dummy()`. Any file with a real
`use` of a sibling file, or relying on project-level headers, errors in the IDE while
`piperine build` succeeds. The `piperine-project` dependency is declared in the server's
Cargo.toml and **never used**.

**Fix sketch:**
1. On document open: walk up from the file to `Piperine.toml`
   (`piperine_project::get_current_project_root`-style, but rooted at the file, not CWD),
   build the same `SourceMap` the CLI builds (`piperine-cli/commands/utils.rs::build_source_map`
   is the reference — consider moving it into `piperine-project` so both consume it).
2. Cache per workspace root; invalidate on `Piperine.toml` change (the extension already
   registers a file watcher — the server ignores `workspace/didChangeWatchedFiles`, see I18).
3. Cross-file design state: today `DocumentState.design` is per-file. A project should
   elaborate as one unit keyed by root, with per-file diagnostics fanned out
   (`PublishDiagnostics` per URI). This restructure (`ServerState.projects:
   HashMap<Root, ProjectState>`) is the prerequisite for cross-file goto/references.

**Size:** medium. Without it the IDE is single-file-only, which no real user survives.

### I5 — Synchronous single-threaded loop: no debounce, no cancellation, no async

**Where:** `main.rs` — one thread, requests handled inline in receive order.
`state.rs::upsert_document` re-runs full parse+**elaborate** on every keystroke
(`TextDocumentSyncKind::FULL`), before the next message is even read. A slow elaboration
(big design, ngspice headers) freezes hover/completion behind it. No `$/cancelRequest`
handling; `.expect()` on param deserialization **panics the whole server** on a malformed
message (`diagnostics.rs:20`, all handlers); `unwrap()` on channel sends.

**Fix sketch:**
1. Debounce: on `didChange`, stamp the version and schedule analysis ~150–300ms later on a
   worker thread; only publish if still the latest version. (Store `source` immediately so
   position-based requests use fresh text with stale analysis — standard LSP pattern.)
2. Threading: `lsp-server` supports responding out of order — move analysis to a worker,
   keep the loop non-blocking. rust-analyzer's `main_loop` shape is the model; a minimal
   version is one analysis thread + `crossbeam-channel` (already a dependency!).
3. Replace every `expect`/`unwrap` on protocol I/O with logged error + LSP error response.
4. Honor `$/cancelRequest` for in-flight analysis-dependent requests.
5. Consider `TextDocumentSyncKind::INCREMENTAL` once debounce exists (FULL + debounce is
   fine at PHDL file sizes; incremental is polish, not urgency).

**Size:** medium.

---

## T1 — Correctness gaps in the five features that exist

### I6 — Diagnostics: elaboration errors always land at 0:0, ranges are 1 char wide

**Where:** `state.rs::parse_and_collect_errors` — elab error pushed with
`byte_offset: None` even though `ElabError` **has** a `span: Option<SourceSpan>` field
(`pom/error.rs::with_span`); it's discarded via `e.to_string()`. Parse errors keep only
`span.offset()`, dropping length — `parse_error_to_diagnostic` fabricates a 1-char range.
`LowerErrors` (fallible lowering, P5) aren't surfaced at all — lowering isn't run by the
server, so "unresolved name" errors the CLI would catch never appear in-editor.

**Fix sketch:**
1. Keep the structured errors: `Vec<ParseError>` → carry `SourceSpan` (offset+len), map to
   full ranges.
2. Populate `ElabError.span` at the error construction sites that have a decl in hand
   (many pass `None` today — audit `elab/`); depends on I1 for decl spans.
3. Run `ppr_to_ir` after successful elaboration and surface `LowerErrors` as diagnostics
   (they name module + symbol; with I1's spans they can point at the offending identifier).
4. Severity tiers: everything is `ERROR` today. Add `WARNING` plumbing now (unused wire,
   unconnected port are natural first warnings — lang-side work, but the channel should
   exist); `code` field per error kind (today always `"parse-error"`, even for elab errors)
   so the client can filter/link docs.
5. Multiple errors: parse is tolerant (`parse_str_tolerant` ✅) but elaboration stops at the
   first error (`Result<_, ElabError>`). Error-accumulating elaboration (mirror of P5's
   `LowerErrors` pattern) is what makes the editor show *all* problems, not one.

### I7 — Hover: global lookup, Debug-format leakage, no expression types, no docs

**Where:** `hover.rs`. Beyond the I2 scoping problem:
- `param.value_type()` and `port.direction()` printed with `{:?}` — users see Rust enum
  debug (`Real`, fine; but `Some(ConstVal::Real(1000.0))` for defaults — raw internals).
- No hover for: locals/`var`s, fn params, enum variants, bundle fields, disciplines'
  nature detail, builtin functions (`ddt`, `$op`, …), keywords, literals with SI-prefix
  expansion (hovering `1k` showing `1000.0` would be *chef's kiss* for an HDL).
- No doc-comment extraction: `///` comments above a decl should surface in hover (and
  completion detail). The lexer currently discards comments — needs comment attachment
  (formatter's `parse/format/comment.rs` already deals with comment tokens; reuse).
- `range: None` in the response — VS Code highlights nothing on hover.
- Builtin/task hover table should be *generated from* the registries
  (`lowering/syscalls.rs`, `eval/tasks.rs`, `bench_task_implemented`) — one source of truth,
  not a hand-copied list.

### I8 — Go-to-definition: textual search, single-file, no locals

**Where:** `goto_def.rs`. `find_decl(source, "mod ", word)` fails on: `pub mod X`,
`mod X` in a comment, first-occurrence-wins duplicates; the duplicated
`.or_else(|| find_decl(source, "mod ", word))` on line 105 is literally the same call twice
(dead code — symptom of the approach). Missing entirely: locals, fn params, enum variants
(`Mode::A`), bundle fields, instance labels, `use` targets (cross-file — needs I4), builtin
decls (jump into `headers/*.phdl` prelude/ngspice sources — very useful for device models).

**Fix:** subsumed by I1+I2 (spans + resolver). Add `textDocument/declaration` =
`definition`, and `typeDefinition` for `instance → mod decl`, `wire → discipline decl`.

### I9 — Completion: predictive base is good, everything on top is stale or missing

**Where:** `completion.rs`. `predict_at_cursor` (parser-driven expected-syntax) is the right
architecture — better than most first-pass LSPs. Gaps:
- **Scope identifiers absent**: in an `analog` body, the module's nets/params/vars — the
  things you actually type — are never offered. `IdentRole` has more roles than the handler
  consumes (`_ => {}` arm swallows them); wire discipline names, port names in instance
  connections (`.p = `), param names in `{ .r = }` blocks, enum variants after `::`.
- **Bench context missing entirely**: `$op/$tran/$ac/$noise/$write/$assert` not in the
  syscall list (it offers `$temperature/$abstime/$finish/$display` — the *analog* set —
  everywhere). Config bundle types (`OpConfig`, `TranConfig`, `Solver`…) and their fields
  after `.` inside a bundle literal. Result-object methods (`.v/.i/.at/.db/…`) — needs
  light type inference on `var r = $op();` chains, or at minimum offer the union of known
  result methods after `.` on any bench var.
- **Field/method completion after `.`**: trigger char `.` is registered but nothing handles
  member position (bundle fields, instance ports/params, waveform methods).
- Value-type list handwritten (drifts; `Natural` vs prelude names) — generate from one
  table shared with hover.
- Snippet completions for skeletons: `mod` with ports, `analog M { }`, `bench M { fn }` —
  the grammar knows the shapes; snippets are cheap wins.
- `is_incomplete: true` always — forces VS Code to re-query every keystroke; return `false`
  when the list is complete.
- Fallback `add_top_level_completions` is missing `bench` and `const`.

### I10 — Document symbols: string-match ranges, module range = whole file, missing kinds

**Where:** `symbols.rs`. `find_end_of_module` returns **EOF** for every module ("simple
heuristic" comment admits it) — so in a 3-module file all three "module" outline ranges span
the entire document, breaking breadcrumbs and sticky scroll. Instance lookup
`find_decl_range(source, label)` matches the label string *anywhere*. Missing symbol kinds:
`fn`s, `enum`s (+ variants as children), `bundle`s (+ fields), `discipline`s, `capability`s,
`impl` blocks, `bench`es (+ entry-point fns as children — pairs with the test runner, I20),
`const`s. Behaviors are listed under the module but `analog Foo` is a *sibling* item in
source — fine as a design choice, but the range math must come from spans (I1).

---

## T2 — Missing server features (the grade-A checklist)

### I11 — Formatting: the formatter exists and is not wired

`piperine-lang/src/parse/format/` (token-based formatter, used by `piperine fmt`) is
**done** — this is the cheapest big win in the whole document. Implement
`textDocument/formatting` (+ `rangeFormatting` if the formatter can take a token slice;
otherwise full-doc only and advertise accordingly): run `format_source`, diff against
current text, return minimal `TextEdit`s (or one whole-document edit — VS Code handles it).
Register `documentFormattingProvider: true`. Extension side: nothing needed;
format-on-save follows from user settings.

### I12 — References + rename

`textDocument/references` and `textDocument/rename` (+ `prepareRename`). Needs I1+I2 (all
use-sites must be resolvable). Rename is *the* trust feature of an IDE; scope it initially
to same-file module/port/param/wire/instance/fn names, cross-file after I4. Validate new
name against the lexer's ident rules; refuse renaming prelude/builtin names.

### I13 — Semantic tokens

TextMate regex can't distinguish `resistor` (instance) from `resistance` (param) from `out`
(net). `textDocument/semanticTokens/full` with token types: `namespace` (module),
`parameter` (params), `variable` (nets/wires — modifier `readonly` for ports), `property`
(bundle fields), `function` (fns + analog operators), `macro` (`$` tasks), `enumMember`,
`type` (disciplines/value types). The elaborated design + decl spans (I1) give exact
classifications; emit deltas later (`full` first is fine). This is what makes code *look*
grade-A in five seconds.

### I14 — Signature help

`textDocument/signatureHelp` on `(` and `,`: analog operators (`ddt(expr)`,
`transition(expr, td, rise, fall)`, `laplace_np(…)` — arities live in
`lowering/analog_ops.rs`), `$` tasks (from the task registries), user `fn`s (from POM),
instance port lists when typing `inst : Mod(<here>)` — port names + disciplines from the
design. Generated from the same single-source tables as hover/completion (I7/I9).

### I15 — Code actions / quick fixes

`textDocument/codeAction`. Highest-value fixes, each pairing with an existing diagnostic:
- unresolved name (LowerError) → "did you mean `X`" (edit) using levenshtein over the scope;
- unknown child-param override (`inst.rname`) → suggest nearest param;
- bench calling unimplemented task → link doc / remove call;
- unconnected required port (once that diagnostic exists) → insert `.port = ` template;
- missing `Piperine.toml` → "create project manifest" workspace edit.
Wire `Diagnostic.data` with a machine-readable code so actions match diagnostics robustly.

### I16 — Inlay hints

`textDocument/inlayHint`: inferred types on bench `var r = $op()` (`: OpResult`), SI-prefix
expansion (`1k` → `= 1000.0`), resolved param defaults on instances
(`{ .r = width * RSH }` → `= 2.4k`), port names in positional instance connections
(`R(a, b)` → `R(.p = a, .n = b)`). The last one is a genuine HDL-review superpower.

### I17 — Folding, selection range, workspace symbols, document highlight

- `foldingRange`: brace-based from the token stream (don't rely on indentation).
- `selectionRange`: expand cursor → ident → expr → stmt → block → item (needs expr spans).
- `workspace/symbol`: all modules/fns/benches across the project (needs I4).
- `documentHighlight`: all occurrences of the symbol under cursor (read/write distinction
  for `var` assignments vs reads) — falls out of I12's reference machinery.

### I18 — Protocol hygiene

- Handle `workspace/didChangeWatchedFiles` (extension already watches `**/*.phdl` —
  server logs "unhandled"): re-elaborate the project on external changes (git checkout!).
- Handle `workspace/didChangeConfiguration` (server has zero settings today; it will grow
  them — debounce ms, prelude path, diagnostics verbosity).
- Advertise + honor `workspace/workspaceFolders`; multi-root workspaces.
- `window/logMessage` instead of bare `eprintln!` so logs land in the client's output
  channel with severities.
- Respond with proper LSP errors (`ErrorCode::InvalidParams`) instead of silently returning
  (`hover.rs:19` just `return`s — the client waits forever for that request id; **this is a
  protocol violation** — every request id must get a response).

---

## T3 — Extension / VS Code UX

### I19 — The extension is a test harness, not a product

**Where:** `.vscode/extension/package.json` — `"name": "piperine-lang-server-test"`,
`"private": true`, `"publisher": "local"`, no repository/icon/license/README/categories/
keywords, no `vsce`/`@vscode/vsce` packaging script, plain JS (fine, but no bundler — the
`node_modules` ships raw), server path defaults to `target/debug/piperine-lang-server`
(debug build!). It lives under `.vscode/` inside the workspace — move to `editors/vscode/`
(the `.vscode/` directory is for *this repo's* editor settings, and marketplaces reject
publishing from there conceptually; also `.vscode/extension/package-lock.json` pollutes
repo-settings space).

**Fix sketch:** `editors/vscode/` with: proper manifest (name `piperine`, displayName
"Piperine PHDL", icon, categories `[Programming Languages]`), `npm run package` via
`@vscode/vsce`, esbuild bundle, release-build server resolution with clear error UX when
missing ("Build it with `cargo build -p piperine-lang-server --release`" + button), CI
artifact (T4).

### I20 — No bench/test runner integration

The killer app for this IDE: benches are *runnable* (`piperine test`, `BenchRunner`,
outcomes Passed/Failed/Error). Nothing surfaces them.
1. **CodeLens** (`textDocument/codeLens`, server-side): "▶ Run bench" / "▶ Run" above every
   bench entry-point `fn` and every `bench` block; command shells out to
   `piperine test --entry Mod::fn` (CLI already supports entry selection via `run`; align
   `test` flags).
2. **Test Explorer**: VS Code `TestController` in the extension — discover via a
   server-custom request (`piperine/benchEntries` returning module::fn + spans), run via
   CLI, map pass/fail back to gutter icons. Failure messages (`$assert` text) as
   `TestMessage` with location.
3. Later: stream `$info/$warn` logs to the test output channel; `$write`/`$plot` artifacts
   surfaced as links (pairs with SPEC_BENCH_GAPS G1).

### I21 — Grammar (TextMate) is stale and thin

**Where:** `.vscode/extension/syntaxes/phdl.tmLanguage.json`.
- **Missing keywords:** `bench` (!), `const`, `else` (control list has `if|else`? — no:
  `if|for|in|match|return|when` — **`else` is genuinely absent**), `as` (if cast syntax
  exists), `select`.
- **Missing types:** `Bit`, `Logic` (if disciplines), `Vec`, `Option`, `Map`, `Waveform`,
  config bundles (`OpConfig`, `TranConfig`, `AcConfig`, `NoiseConfig`, `Solver`), `Scale`,
  `CrossDir`.
- **Missing `$` tasks:** `$op`, `$tran`, `$ac`, `$noise`, `$assert`, `$write`, `$plot`,
  `$error` is present but `$warn` vs `$warning` mismatch with the real registry — generate
  this list from `eval/tasks.rs` + bench registry (a tiny `build.rs` or a checked-in
  generated file with a test asserting sync).
- **Missing operators:** `**` (power), `..`/`..=` (ranges), `=>` (match arms), `|` in
  comprehensions, `::` path separator scope, `@` event sigil.
- **No scopes for:** SI-prefixed literals (`1k`, `2.2u`, `10meg` — the most PHDL-specific
  token there is; regex `[0-9.]+(T|G|meg|k|m|u|n|p|f|a)\b`), attribute syntax, `param`
  bundle-override blocks (`{ .r = … }` — `.field` should scope as `variable.other.member`),
  doc comments (`///` distinct from `//`), instance declarations
  (`label : Mod(…)` — label as `entity.name`), string escapes.
- Long-term: semantic tokens (I13) supersede most of this, but the grammar is what users
  see in the first 100ms and on github.com — keep it accurate.

### I22 — language-configuration.json is minimal

Missing: `indentationRules` (increase after `{`, decrease on `}`), `onEnterRules`
(continue `///` doc comments, `*` in block comments), `folding.markers`
(`// region`/`// endregion`), `wordPattern` (include `$` so `$op` selects as one word —
double-click on `$assert` today selects only `assert`), auto-closing `<` **not** wanted
(contribution operator `<+` would fight it — verify current behavior with `<`).

### I23 — No snippets

`snippets/phdl.json` contribution: `mod` skeleton (ports + body), `analog`/`digital` block,
`bench` with entry fn + `$op` + `$assert`, `discipline`, instance-with-params, `for` sweep
staging loop (SPEC_BENCH §12.4 shape), config-bundle literals (`TranConfig { .stop = ${1} }`).
Ten snippets ≈ an afternoon, huge onboarding value.

### I24 — Extension activation/lifecycle polish

- `piperine.restartServer` command (server crashes today = reload window).
- Status bar item: server state + active project root.
- Output channel: pipe server stderr (LanguageClient does this if configured — name it
  "Piperine Language Server"; today `console.log` goes to the extension host log nobody
  opens).
- `phdl.server.path` setting exists ✅ but add `phdl.trace.server` (standard LSP trace
  setting `vscode-languageclient` honors automatically once declared).
- Auto-restart on server binary change (dev QoL: watch the path, prompt).
- Untitled/unsaved documents: `documentSelector` only matches `scheme: "file"` — new
  unsaved PHDL buffers get nothing; add `{ scheme: "untitled", language: "phdl" }`.

---

## T4 — Tests, CI, release

### I25 — Test suite tests helpers, not the protocol

`tests/integration_test.rs` (18 tests) unit-tests `byte_to_line_col`, `word_at_position`,
`find_definition`, hover-info string building — all good, none exercise LSP framing. Zero
coverage: completion handler, symbols handler, diagnostics publishing, position encoding on
non-ASCII, malformed params (which currently **panic**, I5). Fix: in-process harness with
`lsp_server::Connection::memory()` — spawn the server loop on a thread, drive
initialize → didOpen → hover/completion/definition round-trips with real JSON-RPC, assert
responses. This catches protocol violations (I18's missing responses) that unit tests
structurally cannot. Fixture files exist (`.vscode/test-fixtures/*.phdl`) — move next to the
tests they serve.

### I26 — Nothing builds/checks the extension in CI

No CI touches `.vscode/extension/`: no `npm ci`, no grammar JSON validation, no
`vsce package --no-publish` smoke, no extension integration test (`@vscode/test-electron`
runs headless VS Code — one smoke test: open fixture, expect diagnostics). Add a `just`/CI
job; also a Rust test asserting the grammar's keyword/task lists match the compiler's
registries (kills I21's drift class permanently).

### I27 — No release/versioning story

Server binary and extension version independently with no linkage. Define: extension bundles
a platform binary per target (vsix per platform, like rust-analyzer) **or** downloads a
GitHub release asset on first run, with `phdl.server.path` as the escape hatch. Version
handshake: extension sends its version in `initializationOptions`; server warns on mismatch.

---

## Suggested execution order

| Phase | Items | Outcome |
|-------|-------|---------|
| 1 | I3, I5, I18, I25 | server is *robust*: no panics, no protocol violations, correct positions, tested over real JSON-RPC |
| 2 | I1, I2, I6 | spans + resolver + real diagnostics — the foundation |
| 3 | I11, I8, I7, I10, I9 | formatting for free; the five features become *correct* |
| 4 | I4 | multi-file projects — the IDE works on real designs |
| 5 | I13, I12, I14, I20 | semantic tokens, rename/references, signature help, bench runner — the grade-A tier |
| 6 | I19, I21, I22, I23, I24, I26, I27 | extension productization + release |
| 7 | I15, I16, I17 | delight tier: quick fixes, inlay hints, workspace symbols |

Single-source-of-truth rule throughout: keyword lists, task tables, type names, operator
arities — generated from or test-pinned to the compiler's own registries
(`eval/tasks.rs`, `lowering/syscalls.rs`, `lowering/analog_ops.rs`, prelude bundles), never
hand-copied into handler/grammar/snippet files (three copies already drifted: completion's
syscall list, the TextMate task regex, hover's absence).
