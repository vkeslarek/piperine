# Declared Language Surface Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill files
by filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Spec**: `.specs/features/declared-language-surface/spec.md`
**Context**: `.specs/features/declared-language-surface/context.md`
**Design**: `.specs/features/declared-language-surface/design.md`
**Status**: Draft — awaiting task approval.

**Phase order below is IMPLEMENTATION order, not spec priority order** —
grammar must exist before the fail-loud rule has anything to look up, and the
mechanism (overload-aware registries + rule) must exist before any P4
migration sub-phase can flip enforcement for its category. Spec's P1/P2/P3/P4
labels are user-story priority; this file sequences by dependency.

**Progressive enforcement invariant (every P4 task):** the fail-loud rule for
a given category only flips ON once that category's `extern` declarations
land in the same task — never a global switch, never a category enforced
before it's declared (per design.md's flag-day risk mitigation).

---

## Test Coverage Matrix

> Generated from codebase + guidelines. Guidelines found: `CLAUDE.md`
> (§Build and test, §Tests of record), `.specs/STATE.md` (MD-13 idiom rules).
> Baselines measured directly: `cargo test -p piperine-lang` today passes
> 184 tests across its suites; `cargo test -p piperine-lang-server` passes 10;
> `cargo test --workspace` (whole repo, post `codegen-architecture`) passes
> 582. These are the floors every task's gate must meet or exceed (net-new
> tests raise the workspace total; a category-migration task must never drop
> below the pre-task workspace count).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------- | --------------------- | ----------------- | ------------ |
| Parser/grammar (`extern` forms) | unit | Every `extern` form (type/fn/task/operator/attribute/impl) has a parse-success test + a parse-error (body-on-extern) test — 1:1 to spec ACs | `crates/piperine-lang/src/parse/` `#[cfg(test)]` or `crates/piperine-lang/tests/extern_grammar.rs` (new) | `cargo test -p piperine-lang` |
| Registry / overload resolution | unit | Every registry gains extern-aware tests; overload resolution has a dedicated fixture test covering 1-candidate, N-candidate-disjoint-types, 0-match, ambiguous-match paths (design.md's isolation requirement) | `crates/piperine-lang/src/elab/registry/` `#[cfg(test)]` or `crates/piperine-lang/tests/overload_resolution.rs` (new) | `cargo test -p piperine-lang` |
| Fail-loud resolution rule | unit + integration | Every spec Edge Case + P1 AC has a dedicated test (undeclared name, extern-with-missing-binding, duplicate non-overload, shadowing rejection) | `crates/piperine-lang/tests/` (existing pattern: `elab.rs`, `parse_elab.rs`) | `cargo test -p piperine-lang` |
| LSP `symbol_index`/`goto_def` | integration | Each of the 6 relocated `extern` forms (+ the cast replacement) has a goto-def resolution test; the "undeclared → no location" regression case has one test | `crates/piperine-lang-server/tests/integration_test.rs` | `cargo test -p piperine-lang-server` |
| Stdlib headers (per P4 sub-phase) | integration (existing suite) | The pre-existing full suite (`piperine-lang`, `piperine-codegen`, root `tests/`, `piperine-solver`) stays green at ≥ its pre-task count after every sub-phase — this IS the regression test, no new assertions needed per relocated name (spec: "zero behavior change") | whole workspace | `cargo test --workspace` |
| Regression guard (native table ↔ extern decl) | integration | One test per native table (`MATH_FNS`, `Task` registry, operator match arms, schema registrations) asserting every entry has a matching `extern` declaration | `crates/piperine-lang/tests/extern_coverage_guard.rs` (new) | `cargo test -p piperine-lang extern_coverage_guard` |
| Docs (CLAUDE.md, STATE.md, `docs/spec/`) | none | build gate only | `CLAUDE.md`, `.specs/STATE.md`, `docs/spec/` | build gate |

## Gate Check Commands

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | Task touching only `piperine-lang` internals (grammar, registries, resolution rule) | `cargo test -p piperine-lang` |
| LSP | Task touching `piperine-lang-server` | `cargo test -p piperine-lang-server` |
| Full | Task touching cross-crate behavior, any P4 migration sub-phase, or anything codegen-facing | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |
| Build | Docs-only tasks | `cargo build --workspace` (zero warnings) |

> **Zero rustc warnings is a hard gate on every task** (CLAUDE.md). Every Full
> gate must report a workspace test count ≥ the count before that task
> (582 baseline, rising as new tests land — never dropping).

---

## Execution Plan

Phases ordered, run sequentially; tasks within a phase run in order.

### Phase 1: Grammar — the six `extern` forms
```
T1 → T2 → T3 → T4 → T5 → T6
```
### Phase 2: Overload-aware registries (data-model change, isolated per design.md)
```
T7 → T8 → T9 → T10
```
### Phase 3: Fail-loud declared-first resolution rule
```
T11 → T12 → T13
```
### Phase 4: LSP indexing
```
T14 → T15
```
### Phase 5: P4 sub-phase — primitive types
```
T16
```
### Phase 6: P4 sub-phase — casts (proves overload + impl-method table together)
```
T17 → T18
```
### Phase 7: P4 sub-phase — math functions
```
T19
```
### Phase 8: P4 sub-phase — system tasks
```
T20 → T21
```
### Phase 9: P4 sub-phase — runtime operators
```
T22
```
### Phase 10: P4 sub-phase — `@device`/`@port`
```
T23
```
### Phase 11: P4 sub-phase — plugin-contributed schemas
```
T24 → T25
```
### Phase 12: P4 sub-phase — remaining `extern impl` native capability methods
```
T26
```
### Phase 13: Regression guard + docs
```
T27 → T28 → T29
```

---

## Task Breakdown

### T1: `extern` keyword + `extern type` grammar

**What**: Lexer gains the `extern` keyword; parser gains `extern type Name;`
producing a body-less type-declaration AST node with `decl_span`.
**Where**: `crates/piperine-lang/src/parse/lexer.rs`, `crates/piperine-lang/src/parse/parser/*.rs`, `crates/piperine-lang/src/parse/ast.rs`
**Depends on**: None
**Reuses**: existing `bundle`/`discipline` declaration parsing shape
**Requirement**: DLS-08
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `extern type Real;` parses into a distinct AST node with correct `decl_span`.
- [x] `extern type Real { }` (a body) is a parse error naming the declaration.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): extern keyword + extern type grammar (DLS-08)`
**Status**: ✅ Complete — commit `cfa1859`

---

### T2: `extern fn` grammar

**What**: `extern fn name(params) -> RetType;` — signature-only function
declaration, no body.
**Where**: `crates/piperine-lang/src/parse/parser/*.rs`, `ast.rs`
**Depends on**: T1
**Reuses**: existing `FnDecl`/`FnParam` shapes (signature portion only)
**Requirement**: DLS-09
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `extern fn sin(x: Real) -> Real;` parses with correct `decl_span`.
- [x] A body on `extern fn` is a parse error naming the declaration.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): extern fn grammar (DLS-09)`
**Status**: ✅ Complete — commit `c92e53c`

---

### T3: `extern task` grammar

**What**: `extern task $name(params) -> RetType;` — preserves `$`-prefixed
system-task name form.
**Where**: `crates/piperine-lang/src/parse/parser/*.rs`, `ast.rs`
**Depends on**: T2
**Reuses**: T2's signature-only shape; existing `$name` lexing for system tasks
**Requirement**: DLS-10
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `extern task $temperature() -> Real;` parses with correct `decl_span`.
- [x] A body is a parse error.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): extern task grammar (DLS-10)`
**Status**: ✅ Complete — commit `4283c6b`

---

### T4: `extern operator` grammar

**What**: `extern operator name(params) -> RetType;` — the runtime-operator
declaration form (`ddt`, `delay`, …).
**Where**: `crates/piperine-lang/src/parse/parser/*.rs`, `ast.rs`
**Depends on**: T2
**Reuses**: T2's signature-only shape
**Requirement**: DLS-11
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `extern operator ddt(x: Real) -> Real;` parses with correct `decl_span`.
- [x] A body is a parse error.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): extern operator grammar (DLS-11)`
**Status**: ✅ Complete — commit `c7d8708`

---

### T5: `extern attribute` grammar

**What**: `extern attribute name { field: Type, ... }` — attribute-schema
declaration, fields shaped like bundle fields, each field carrying its own
`decl_span`.
**Where**: `crates/piperine-lang/src/parse/parser/*.rs`, `ast.rs`
**Depends on**: T1
**Reuses**: existing bundle-field parsing
**Requirement**: DLS-12
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `extern attribute device { plugin: String, type: String }` parses;
      each field has its own `decl_span`.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): extern attribute grammar (DLS-12)`
**Status**: ✅ Complete — commit `6de509a`

---

### T6: `extern impl` grammar

**What**: `extern impl TypeName { fn method(self, ...) -> Ret; ... }`
(optionally `extern impl Capability for TypeName { ... }`) — each method
signature-only with its own `decl_span`; the block itself carries a
`decl_span`.
**Where**: `crates/piperine-lang/src/parse/parser/*.rs`, `ast.rs`
**Depends on**: T2
**Reuses**: existing `ImplDecl` shape (`impl [Capability for] TypeRef`)
**Requirement**: DLS-13, DLS-14
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `extern impl Real { fn from(x: Integer) -> Real; }` parses; block and
      each method carry correct, distinct `decl_span`s.
- [x] `extern impl Capability for TypeName { ... }` parses.
- [x] A body on any method inside `extern impl` is a parse error naming that
      method.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): extern impl grammar (DLS-13,14)`
**Status**: ✅ Complete — commit `4e2abfe`

---

### T7: `TypeRegistry` gains `extern` variant

**What**: `TypeDefKind::Extern { name, decl_span }`; `Register` pass indexes
`extern type` items into `TypeRegistry` alongside plain types.
**Where**: `crates/piperine-lang/src/elab/registry/types.rs`, `elab/lower/register.rs` (or wherever `Register` walks top-level items)
**Depends on**: T1
**Reuses**: `TypeRegistry::register`/`lookup` (unchanged signature — types are not overloadable)
**Requirement**: DLS-01 (groundwork)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] An `extern type` item registers into `TypeRegistry` with its `decl_span`.
- [x] Duplicate `extern type`/plain-type same-name is a duplicate-declaration error.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): TypeRegistry extern variant (DLS-01 groundwork)`
**Status**: ✅ Complete — commit `8c81ef2`

---

### T8: `CallableRegistry` becomes overload-aware (`Vec` storage)

**What**: `callables: HashMap<String, Vec<Box<dyn CallableDef>>>` — `register`
appends; `CallableDef::validate_call` gains a real implementation (full
param-type match, not arity-only, per design.md's resolved Open Item #2).
**Where**: `crates/piperine-lang/src/elab/registry/callables.rs`
**Depends on**: None (independent data-model change, isolated per design.md's
risk mitigation — lands before any grammar/registry integration depends on it)
**Reuses**: existing `CallableDef` trait, `FnDecl`'s current registration path
**Requirement**: DLS-06
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `CallableRegistry::register` appends to a `Vec` per name; existing
      single-`fn`-per-name callers (today's only path) behave identically
      (a `Vec` of length 1 resolves exactly as the old single value did).
- [x] `validate_call` performs full param-type matching against a candidate's
      declared signature.
- [x] Quick gate passes; existing `piperine-lang` suite (184 baseline) unchanged.
**Tests**: unit · **Gate**: quick
**Commit**: `refactor(lang): CallableRegistry overload-aware storage (DLS-06)`
**Status**: ✅ Complete — commit `1a018a9`

---

### T9: Overload resolution algorithm + fixture tests

**What**: `CallableRegistry::resolve(name, arg_types) -> Result<&dyn CallableDef, ElabError>` — arity filter → exact param-type filter → 0 match/1 match/ambiguous handling, per design.md's algorithm. Dedicated synthetic-fixture tests prove all four paths **before** any P4 category depends on it (design.md's isolation requirement).
**Where**: `crates/piperine-lang/src/elab/registry/callables.rs`, new `crates/piperine-lang/tests/overload_resolution.rs`
**Depends on**: T8
**Reuses**: `ValueType` for structural matching
**Requirement**: DLS-07
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] 1-candidate resolves normally.
- [x] N-candidate-disjoint-types resolves the matching one by arg type.
- [x] 0-match fails loud naming the call site and every candidate signature tried.
- [x] Ambiguous-match (two structurally identical signatures — defensive path)
      fails loud naming every matching candidate.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): overload resolution algorithm + fixture tests (DLS-07)`
**Status**: ✅ Complete — commit `f53f4f2`

---

### T10: `OperatorRegistry` + `extern impl` per-type method table

**What**: New `OperatorRegistry` (mirrors `ComponentRegistry` shape,
overload-aware per design.md); new per-`(type_name, method_name)`
impl-method table, also overload-aware, both living in `ElabContext`.
`SchemaRegistry::register_declared` gains `decl_span`; `AttrField` gains
`decl_span` per field.
**Where**: `crates/piperine-lang/src/elab/registry/mod.rs` (new file for
`OperatorRegistry` + impl-method table), `elab/registry/schemas.rs`
**Depends on**: T4, T5, T6, T9 (reuses the resolution algorithm)
**Reuses**: `ComponentRegistry`'s `register`/`lookup` template; T9's resolution helper
**Requirement**: DLS-01 (groundwork), DLS-13/14 (impl-method table home)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] `OperatorRegistry::register`/`resolve` work identically to
      `CallableRegistry`'s overload path (shared algorithm, different map).
- [x] Impl-method table keyed by `(type_name, method_name)` resolves
      correctly, including overloaded methods on the same type.
- [x] `SchemaRegistry`'s `AttrField`s carry `decl_span`.
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): OperatorRegistry + extern impl method table (DLS-01,13,14)`
**Status**: ✅ Complete — commit `e8aa80d`

---

### T11: Fail-loud resolution rule — calls

**What**: `resolve_calls_in_expr` (or its successor) now does exactly one
lookup chain for `Expr::Call(Ident/Path, args)`: user `fn`/`impl` method →
`CallableRegistry`/`OperatorRegistry` (plain or extern, resolved via T9's
algorithm) → **fail loud**. No branch reaches `math.rs`/`eval/tasks.rs`/
codegen's operator match without passing through this.
**Where**: `crates/piperine-lang/src/elab/resolve.rs`
**Depends on**: T9, T10
**Reuses**: `ElabError`/`ElabErrorKind` fail-loud convention
**Requirement**: DLS-02, DLS-03, DLS-04
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] A plain-declaration call resolves exactly as before this task (no
      behavior change — DLS-02).
- [x] An `extern`-declared call dispatches with signature validated against
      the `extern` decl (DLS-03).
- [x] A call to a name with no declaration anywhere fails loud, naming the
      identifier and use site (DLS-04).
- [x] Full gate passes — **enforcement is not yet flipped for any category**
      (no `extern` declarations exist yet outside test fixtures), so this
      task's own tests use small synthetic PHDL, not the stdlib.
**Tests**: unit · **Gate**: full
**Commit**: `feat(lang): declared-first fail-loud call resolution (DLS-02,03,04)`
**Status**: ✅ Complete — commit `3b7f289`. Scope note: bare-identifier
calls with no `CallableRegistry` entry are left untouched (deferred to each
P4 sub-phase per design.md's per-category progressive enforcement); DLS-04
is proven via `Type::method(...)` (`Expr::Path`) calls, a currently-unused
production surface, so the fail-loud rule is 100% safe and immediately
enforced there. Also wired `extern fn/task/operator/attribute/impl`
registration (`elab/lower/register.rs`), a Phase 2 gap this task depended on.

---

### T12: Fail-loud resolution rule — types and attribute schemas

**What**: Type-reference resolution (`resolve_type`) and `@attr(...)`
resolution go through the same declared-first, fail-loud discipline as T11.
**Where**: `crates/piperine-lang/src/elab/lower/resolve.rs` (type resolution),
attribute-resolution call sites
**Depends on**: T7, T10
**Reuses**: T11's fail-loud pattern
**Requirement**: DLS-01, DLS-04 (extended to types/attributes)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] An undeclared type name fails loud, naming it and the use site.
- [x] An undeclared `@attr` schema name fails loud identically.
- [x] Full gate passes with the same "not yet globally enforced" caveat as T11.
**Tests**: unit · **Gate**: full
**Commit**: `feat(lang): declared-first fail-loud type/attribute resolution (DLS-01,04)`
**Status**: ✅ Complete — commit `8915255`. Both lookup paths already were
declared-first/fail-loud before this task (no Rust-side fallback existed);
work was completing `extern attribute` registration (done alongside T11)
and adding the previously-nonexistent `UnknownAttrSchema` test coverage.

---

### T13: `extern` with missing registry binding — distinct fail-loud

**What**: When an `extern` declaration resolves but has no matching native
implementation entry, fail loud with a message distinct from T11/T12's
"no declaration" case — names the `extern` decl and the missing binding.
**Where**: `elab/resolve.rs` (dispatch-to-registry step)
**Depends on**: T11
**Reuses**: `ElabErrorKind`, adds a new variant if needed
**Requirement**: DLS-05
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] A synthetic `extern fn` with no backing Rust table entry fails loud
      with a message distinguishable from the "undeclared" error (different
      `ElabErrorKind` variant or clearly distinct text, asserted in the test).
- [x] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): distinct fail-loud for extern w/ missing native binding (DLS-05)`
**Status**: ✅ Complete — commit `8c343ae` (test only; the
`ElabErrorKind::ExternMissingBinding` mechanism itself landed with T11's
commit `3b7f289`, since it's the natural consequence of extern candidates
entering the T11 lookup chain — this task's own contribution is the
dedicated proof asserting on the distinct variant).

---

### T14: `symbol_index` resolves `extern` declarations

**What**: `resolve_at` gains match arms for `TypeRegistry`/`CallableRegistry`/
`OperatorRegistry`/`SchemaRegistry`/impl-method-table entries — a use-site
identifier resolved through any of these returns that entry's `decl_span`.
**Where**: `crates/piperine-lang-server/src/symbol_index.rs`
**Depends on**: T11, T12, T13
**Reuses**: existing `Resolution { decl_span, kind }` shape; `goto_def.rs`
needs zero changes (confirmed in design.md)
**Requirement**: DLS-15
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] A use site of a name resolved via any of the 5 registries returns a
      `Resolution` with the correct `decl_span`.
- [x] LSP gate passes (baseline 10 tests, must not drop).
**Tests**: integration · **Gate**: LSP
**Commit**: `feat(lang-server): symbol_index resolves extern declarations (DLS-15)`
**Status**: ✅ Complete — commit `74d504e`. Required plumbing beyond the
listed file: `SourceFile::elaborate_with_context` (new, `piperine-lang`)
since `Design` alone carries no registry state; `DocumentState` now keeps
the returned `ElabContext`; `SchemaRegistry` gained a schema-name-level
`decl_span` (previously field-level only); `ImplMethodTable` gained
`find_by_method_name`. LSP suite: 11 tests (baseline 10 + 1 net after the
T15 split), zero dropped.

---

### T15: `goto_def` regression test — undeclared name still returns no location

**What**: Explicit regression test proving the previously-magic surfaces now
behave like ordinary declarations (T14), while a genuinely undeclared name
still correctly returns no location (unchanged `None` behavior).
**Where**: `crates/piperine-lang-server/tests/integration_test.rs`
**Depends on**: T14
**Reuses**: existing `find_definition`/`goto_def` test harness
**Requirement**: DLS-16
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [x] Test asserts `None` for a use site of a name with zero declaration.
- [x] LSP gate passes.
**Tests**: integration · **Gate**: LSP
**Commit**: `test(lang-server): undeclared name still returns no goto-def location (DLS-16)`
**Status**: ✅ Complete — commit `8acd259`. Used `DocumentState` directly
(the real `analyze()`/`resolve_at()` path) rather than the legacy
`find_definition` test helper, which is unused elsewhere and stayed
untouched (still takes no `ElabContext`, per design.md's "goto_def.rs
needs zero changes").

---

### T16: Migrate primitive types → `extern type` (flip enforcement: types)

**What**: `ElabContext::new()`'s hardcoded `prims` list (`Real`, `Natural`,
`Integer`, `Complex`, `Boolean`, `Quad`, `String`) is replaced by parsing
`extern type` declarations from a stdlib header; **flip fail-loud enforcement
for the type category** (T12's rule now has real declarations to find).
**Where**: `crates/piperine-lang/headers/prelude.phdl` (or a new
`headers/types.phdl`), `crates/piperine-lang/src/elab/registry/mod.rs`
(`ElabContext::new()`)
**Depends on**: T7, T12
**Reuses**: T7's `TypeRegistry::Extern` variant
**Requirement**: DLS-17
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] The 7 primitives resolve via parsed `extern type` decls, not the
      hardcoded list (grep-verified `prims` removed).
- [ ] Full gate passes; workspace test count ≥ 582 (baseline), same suite green.
**Tests**: regression (existing suite) · **Gate**: full
**Commit**: `feat(lang): migrate primitive types to extern type headers (DLS-17)`

---

### T17: Author cast `extern impl` blocks; delete `resolve.rs` special case

**What**: `extern impl Real { fn from(x: Integer) -> Real; fn from(x:
Boolean) -> Real; fn from(x: Quad) -> Real; }`-shaped blocks per target
primitive in a stdlib header; **delete** `elab/resolve.rs:83-95`'s bare-name
cast rewrite entirely (not migrated — removed, per spec P4-AC7).
**Where**: stdlib header (new `extern impl` blocks), `crates/piperine-lang/src/elab/resolve.rs`
**Depends on**: T10, T16 (needs `Real`/etc as declared types), T9 (overload)
**Reuses**: T9's overload resolution — this is its first real consumer
**Requirement**: DLS-23
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] `real(x)`-shaped rewrite in `resolve.rs` is gone (grep-verified).
- [ ] `Real::from(x)` (and siblings for other primitives) resolves via
      overload by argument type.
- [ ] Quick gate passes on `piperine-lang` alone (call-site migration is T18).
**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): extern impl cast functions; delete bare-cast special case (DLS-23)`

---

### T18: Migrate the 4 known bare-cast call sites

**What**: Update `crates/piperine-codegen/tests/analog_jit.rs`,
`digital_fusion.rs`, `crates/piperine-lang/tests/examples/sar_adc.phdl`, and
`crates/piperine-lang/tests/type_casts.rs` from `real(x)`/`bit(x)`-shaped
calls to `Real::from(x)`/`Bit::from(x)`-shaped calls (enumerated in
`context.md`), in the same commit as T17's deletion lands cleanly workspace-wide.
**Where**: the 4 files above
**Depends on**: T17
**Reuses**: nothing — this is the call-site update T17's own gate deferred
**Requirement**: DLS-23 (completion)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] `rg 'real\(|int\(|bit\(|Boolean\(|Quad\('` over `headers/`, `tests/`,
      `examples/` finds zero PHDL bare-call-cast sites.
- [ ] Full gate passes; workspace test count ≥ prior baseline, same suite green.
**Tests**: regression (existing suite, updated in place — not new assertions,
per spec's "zero behavior change") · **Gate**: full
**Commit**: `test: migrate bare-cast call sites to Type::from (DLS-23)`

---

### T19: Migrate math functions → `extern fn` (flip enforcement: math)

**What**: `math.rs`'s `MATH_FNS` table (`sin` … `limexp`) gets a matching
`extern fn` declaration per entry in a stdlib header (e.g. `headers/
math.phdl`); flip fail-loud enforcement for math-function calls.
**Where**: new `crates/piperine-lang/headers/math.phdl` (or added to
`prelude.phdl`), `math.rs` (implementation table, unchanged logic)
**Depends on**: T11, T16
**Reuses**: `MATH_FNS` table verbatim as the implementation backing
**Requirement**: DLS-18
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] Every `MATH_FNS` entry has a matching `extern fn` declaration.
- [ ] `sin(x)` (etc.) in PHDL resolves via the declaration, dispatches to the
      same Rust implementation, identical numeric output.
- [ ] Full gate passes; workspace test count ≥ prior baseline.
**Tests**: regression (existing suite) · **Gate**: full
**Commit**: `feat(lang): migrate math functions to extern fn headers (DLS-18)`

---

### T20: Migrate system tasks → `extern task`; collapse the two hardcoded lists

**What**: Every `eval/tasks.rs` `Task` impl AND `resolve.rs:25`'s
`valid_diagnostics` hardcoded list (`write`/`strobe`/`display`/…) get a
single matching `extern task` declaration each — the two disconnected
sources found in Design collapse into one; flip fail-loud enforcement for
system tasks.
**Where**: stdlib header (`extern task` declarations), `eval/tasks.rs`
(implementation, unchanged), `elab/resolve.rs` (delete `valid_diagnostics`
hardcoded list, replaced by the registry lookup)
**Depends on**: T11, T16
**Reuses**: `eval/tasks.rs`'s `Task` trait impls as implementation backing
**Requirement**: DLS-19
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] Every system task (`$assert`, `$display`, `$info`/`$warn`/`$error`/
      `$fatal`, `$temperature`, `$simparam`, `$abstime`, `$mfactor`,
      `$bound_step`, …) has one `extern task` declaration.
- [ ] `resolve.rs`'s `valid_diagnostics` list is gone (grep-verified);
      diagnostic validity now comes from the same registry as every other
      system task.
- [ ] Full gate passes; workspace test count ≥ prior baseline.
**Tests**: regression (existing suite) · **Gate**: full
**Commit**: `feat(lang): migrate system tasks to extern task; collapse duplicate validation (DLS-19)`

---

### T21: Regression test — system-task self-check fixture

**What**: A dedicated test proving the T20 collapse didn't silently drop a
previously-valid diagnostic name (each of the old `valid_diagnostics` entries
still resolves post-migration).
**Where**: `crates/piperine-lang/tests/` (existing pattern)
**Depends on**: T20
**Reuses**: —
**Requirement**: DLS-19 (completion)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] Each of the 9 original `valid_diagnostics` names round-trips through
      the new `extern task` path.
- [ ] Quick gate passes.
**Tests**: unit · **Gate**: quick
**Commit**: `test(lang): system-task migration completeness fixture (DLS-19)`

---

### T22: Migrate runtime operators → `extern operator` (flip enforcement: operators)

**What**: `ddt`, `delay`, `slew`, `transition`, `idt`, `cross`, `above`,
`timer`, `white_noise`, `flicker_noise`, `ddx`, `$limit` each get an `extern
operator` declaration; this is the first time these names have any
`piperine-lang`-level presence. Codegen's structural handling
(`resolve/pom/analog_ops.rs`, `flatten/analog.rs`) is untouched — only the
name/arity/type existence check moves upstream to elaboration.
**Where**: stdlib header (`extern operator` declarations), `elab/resolve.rs`
(operator identifiers now go through T11's lookup chain before reaching
codegen)
**Depends on**: T10, T11, T16
**Reuses**: `OperatorRegistry` from T10; codegen's existing operator emission
(zero change to *what* it computes, per design.md)
**Requirement**: DLS-20
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] Every listed runtime operator has an `extern operator` declaration.
- [ ] A `ddt(qtotal)` call in a stdlib model resolves through elaboration's
      `OperatorRegistry` lookup before reaching codegen; codegen's emission
      output is byte-identical to before this task.
- [ ] Full gate passes; workspace test count ≥ prior baseline (this is the
      highest-risk sub-phase per design.md — the widest existing-suite
      surface, since nearly every reactive/dynamic stdlib model uses `ddt`).
**Tests**: regression (existing suite) · **Gate**: full
**Commit**: `feat(lang): migrate runtime operators to extern operator (DLS-20)`

---

### T23: Migrate `@device`/`@port` → `extern attribute` (flip enforcement: plugin attrs)

**What**: `piperine-plugin/src/host.rs`'s hardcoded
`register_declared("device", …)`/`register_declared("port", …)` calls are
replaced by parsing `extern attribute device {...}`/`extern attribute port
{...}` from a stdlib header.
**Where**: stdlib header (`extern attribute` declarations), `crates/piperine-plugin/src/host.rs`
**Depends on**: T5, T10, T12, T16
**Reuses**: T10's `SchemaRegistry` extension
**Requirement**: DLS-21
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] `host.rs`'s hardcoded `register_declared("device"/"port", …)` calls
      are gone (grep-verified), replaced by header parsing.
- [ ] `@device(plugin = …, type = …)` and `@port(...)` still validate
      identically to before.
- [ ] `@rfport` (already textual, unconditional per existing code comments)
      is unaffected — still resolves without any plugin loaded.
- [ ] Full gate passes; workspace test count ≥ prior baseline (plugin suite:
      `e2e.rs`, `native_smoke.rs`, `phase3.rs`, `process_smoke.rs` must stay green).
**Tests**: regression (existing suite) · **Gate**: full
**Commit**: `feat(plugin): migrate @device/@port to extern attribute headers (DLS-21)`

---

### T24: Plugin extern-stub auto-import mechanism

**What**: A loaded plugin's published `extern.phdl`-style stub is parsed into
the project's `ElabContext` automatically on plugin load — no explicit `use`
required (per design.md's Tech Decision, mirroring `headers/spice/`
availability).
**Where**: `crates/piperine-project/src/` (header/plugin resolution), plugin-load path in `piperine-plugin`
**Depends on**: T5, T23
**Reuses**: existing header-loading mechanism for `headers/spice/`
**Requirement**: DLS-22 (groundwork)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] A test fixture plugin publishing an `extern.phdl` stub has its
      declarations available in the project without an explicit `use`.
- [ ] Full gate passes.
**Tests**: integration · **Gate**: full
**Commit**: `feat(project): auto-import plugin extern.phdl stubs (DLS-22 groundwork)`

---

### T25: One real plugin exercises the extern-stub path end-to-end (flip enforcement: plugin schemas)

**What**: At least one real plugin (a fixture or the stdlib's own
OSDI-adjacent example) publishes an `extern.phdl` stub for its
contributed attribute schema; flip fail-loud enforcement for
plugin-contributed schemas — a plugin loaded without a published stub now
fails loud (spec Edge Case), never silently falls back to dynamic registration.
**Where**: a plugin fixture under `crates/piperine-plugin/tests/` (or
equivalent), plugin-loading code path
**Depends on**: T24
**Reuses**: T24's auto-import mechanism
**Requirement**: DLS-22
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] The fixture plugin's custom attribute schema resolves via its
      `extern.phdl` stub, ctrl+click-able like any other `extern attribute`.
- [ ] A plugin loaded without a stub fails loud naming the missing stub
      (not a silent revert to dynamic registration).
- [ ] Full gate passes; workspace test count ≥ prior baseline.
**Tests**: integration · **Gate**: full
**Commit**: `test(plugin): extern-stub schema path end-to-end (DLS-22)`

---

### T26: Migrate remaining native `extern impl` capability methods

**What**: Any remaining native method on a primitive type without a textual
`impl` block today (candidates confirmed at Design: capability impls like
`Add`/`Sub`/`Eq` for `Real`/`Integer`/etc, if the typechecker special-cases
them structurally) gain `extern impl` declarations — the last, riskiest
sub-phase per design.md's ordering (most codegen/typecheck-entangled).
**Where**: stdlib header (`extern impl` blocks), `elab/typecheck.rs`
(operator-dispatch consumer, if applicable)
**Depends on**: T9, T10, T17 (impl-method table proven on casts first)
**Reuses**: the impl-method table + overload resolution, now proven twice
(casts, this task)
**Requirement**: DLS-24
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] Every native capability method found (or a documented "none found,
      operators are structurally special-cased and out of this feature's
      reach" finding if the investigation turns up nothing — never fabricate
      a migration target) has an `extern impl` declaration.
- [ ] Full gate passes; workspace test count ≥ prior baseline.
**Tests**: regression (existing suite) · **Gate**: full
**Commit**: `feat(lang): migrate remaining native type methods to extern impl (DLS-24)`

---

### T27: Regression guard — native table ↔ extern declaration self-check

**What**: A dedicated integration test asserting every native implementation
table (`MATH_FNS`, `eval/tasks.rs`'s `Task` registry, the operator match
arms, `SchemaRegistry` entries) has a matching `extern` declaration —
catches the mechanism silently regressing back into "magic" (design.md's
Error Handling Strategy, resolved as a permanent test per Open Design Item #3).
**Where**: new `crates/piperine-lang/tests/extern_coverage_guard.rs`
**Depends on**: T16, T18, T19, T21, T22, T23, T25, T26 (all P4 sub-phases complete)
**Reuses**: each registry's iteration surface
**Requirement**: DLS-25
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] Test enumerates every native table entry and asserts a matching
      `extern` declaration exists; fails loud (test failure) naming any
      orphan native entry.
- [ ] Full gate passes; workspace test count ≥ prior baseline.
**Tests**: integration · **Gate**: full
**Commit**: `test(lang): extern coverage regression guard (DLS-25)`

---

### T28: Docs — CLAUDE.md, `docs/spec/`, MD-NNN decision

**What**: `CLAUDE.md` gains a note on the `extern` mechanism (mirroring the
`codegen-architecture` refactor's doc-update discipline); `docs/spec/` (the
formal PHDL language spec) documents the `extern` grammar and the
declared-first resolution rule — not just an internal refactor invisible to
the language's own documentation (spec Success Criteria); append an
`MD-NNN` to `.specs/STATE.md` recording "every native/registry-backed name
must have a textual `extern` declaration, checked by a permanent regression
test" as a binding project convention (per design.md's flagged candidate).
**Where**: `CLAUDE.md`, `docs/spec/` (relevant Part), `.specs/STATE.md`
**Depends on**: T27
**Reuses**: —
**Requirement**: DLS-25 (documentation closure)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] CLAUDE.md references the `extern` mechanism where it discusses PHDL's
      language surface.
- [ ] `docs/spec/` documents `extern fn`/`task`/`type`/`operator`/
      `attribute`/`impl` grammar and the declared-first/fail-loud rule.
- [ ] `MD-NNN` appended to STATE.md.
- [ ] Build gate passes.
**Tests**: none · **Gate**: build
**Commit**: `docs: extern language surface mechanism + convention (MD-NNN)`

---

### T29: Final full-workspace regression sweep

**What**: One last `cargo test --workspace` + `cargo build --workspace`
sweep after all 28 prior tasks, confirming the workspace test count and
zero-warning bar hold end-to-end — the closing gate before the Verifier runs.
**Where**: whole workspace (no file changes expected; if this task finds a
regression, it becomes a fix task, not a silent pass)
**Depends on**: T28
**Reuses**: —
**Requirement**: DLS-25 (final confirmation)
**Tools**: MCP: NONE · Skill: NONE
**Done when**:
- [ ] `cargo build --workspace` zero warnings.
- [ ] `cargo test --workspace` all green, count ≥ 582 baseline (higher, given
      new tests added across T1–T27).
**Tests**: regression (existing + all new suite) · **Gate**: full
**Commit**: `chore: final regression sweep for declared-language-surface` (only if a fix was needed — otherwise this task closes with no commit, noted in the batch summary)

---

## Phase Execution Map

```
Phase 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 13

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6
Phase 2:  T7 → T8 → T9 → T10
Phase 3:  T11 → T12 → T13
Phase 4:  T14 → T15
Phase 5:  T16
Phase 6:  T17 → T18
Phase 7:  T19
Phase 8:  T20 → T21
Phase 9:  T22
Phase 10: T23
Phase 11: T24 → T25
Phase 12: T26
Phase 13: T27 → T28 → T29
```

Batch packing (~7 tasks/worker, whole phases): **Batch 1** = P1+P2 (10 →
slightly over budget, but P1/P2 are one tight dependency chain — grammar
must fully land before registries integrate it — legitimate oversized single
batch per sub-agents.md's "tight dependency chain" exception) — **alternative
split**: P1 alone (6) as Batch 1, P2+P3+P4 (9) as Batch 2. **Batch 3** =
P5+P6+P7+P8 (6) · **Batch 4** = P9+P10+P11+P12 (5) · **Batch 5** = P13 (3).
→ 5 workers if sub-agents accepted (using the alternative split). Final
packing offered to the user at Execute time per the skill's offer-then-confirm
rule — this table is provisional guidance, not a locked assignment.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1–T6 | 1 grammar form each | ✅ Granular |
| T7 | 1 registry variant | ✅ Granular |
| T8 | 1 data-model change (1 file) | ✅ Granular |
| T9 | 1 algorithm + its dedicated tests | ✅ Granular |
| T10 | 2 new registries, cohesive (built together, same PR-sized change per design.md) | ⚠️ OK if cohesive — both are the same "new registry" shape stamped twice |
| T11–T13 | 1 resolution-rule surface each | ✅ Granular |
| T14–T15 | 1 LSP change + its dedicated regression test | ✅ Granular |
| T16 | 1 category migration | ✅ Granular |
| T17–T18 | 1 deletion + its call-site migration, split for atomic commits | ✅ Granular |
| T19 | 1 category migration | ✅ Granular |
| T20–T21 | 1 category migration + its completeness fixture | ✅ Granular |
| T22 | 1 category migration (largest single category, but one cohesive header + one rule wiring) | ✅ Granular |
| T23 | 1 category migration | ✅ Granular |
| T24–T25 | 1 mechanism + its first real exercise | ✅ Granular |
| T26 | 1 category migration (open-ended investigation, bounded by "document if none found") | ✅ Granular |
| T27–T29 | 1 regression guard + 1 docs pass + 1 final sweep | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
| ---- | ------------------ | -------------- | ------ |
| T1 | None | phase start | ✅ Match |
| T2 | T1 | T1→T2 | ✅ Match |
| T3 | T2 | T2→T3 | ✅ Match |
| T4 | T2 | (T2→T4, parallel-capable, run in order) | ✅ Match |
| T5 | T1 | (T1→T5, parallel-capable, run in order) | ✅ Match |
| T6 | T2 | (T2→T6, parallel-capable, run in order) | ✅ Match |
| T7 | T1 | (P1→P2 boundary) | ✅ Match |
| T8 | None (isolated) | independent within P2, runs first in order | ✅ Match |
| T9 | T8 | T8→T9 | ✅ Match |
| T10 | T4, T5, T6, T9 | converges from P1 tail + T9 | ✅ Match |
| T11 | T9, T10 | P2→P3 | ✅ Match |
| T12 | T7, T10 | (T7 from P2 start, T10 from P2 tail) | ✅ Match |
| T13 | T11 | T11→T13 | ✅ Match |
| T14 | T11, T12, T13 | P3→P4 | ✅ Match |
| T15 | T14 | T14→T15 | ✅ Match |
| T16 | T7, T12 | P4→P5 | ✅ Match |
| T17 | T10, T16, T9 | P5→P6 | ✅ Match |
| T18 | T17 | T17→T18 | ✅ Match |
| T19 | T11, T16 | (P6→P7, uses P3+P5 outputs) | ✅ Match |
| T20 | T11, T16 | P7→P8 | ✅ Match |
| T21 | T20 | T20→T21 | ✅ Match |
| T22 | T10, T11, T16 | P8→P9 | ✅ Match |
| T23 | T5, T10, T12, T16 | P9→P10 | ✅ Match |
| T24 | T5, T23 | P10→P11 | ✅ Match |
| T25 | T24 | T24→T25 | ✅ Match |
| T26 | T9, T10, T17 | P11→P12 | ✅ Match |
| T27 | T16,T18,T19,T21,T22,T23,T25,T26 | P12→P13, converges all P4 sub-phases | ✅ Match |
| T28 | T27 | T27→T28 | ✅ Match |
| T29 | T28 | T28→T29 | ✅ Match |

All dependencies point backward or within-phase. ✅

---

## Test Co-location Validation

| Task | Code Layer Modified | Matrix Requires | Task Says | Status |
| ---- | -------------------- | ----------------- | ---------- | ------ |
| T1–T6 | Parser/grammar | unit | unit | ✅ OK |
| T7 | Registry | unit | unit | ✅ OK |
| T8 | Registry (data model) | unit | unit | ✅ OK |
| T9 | Overload resolution | unit | unit | ✅ OK |
| T10 | Registry (new) | unit | unit | ✅ OK |
| T11–T13 | Fail-loud rule | unit | unit | ✅ OK |
| T14 | LSP `symbol_index` | integration | integration | ✅ OK |
| T15 | LSP regression | integration | integration | ✅ OK |
| T16, T19, T20, T22, T23, T26 | Stdlib header migration | regression (existing suite) | regression | ✅ OK |
| T17 | `extern impl` + deletion | unit | unit | ✅ OK |
| T18 | Call-site migration | regression (existing suite) | regression | ✅ OK |
| T21 | Completeness fixture | unit | unit | ✅ OK |
| T24, T25 | Plugin stub mechanism | integration | integration | ✅ OK |
| T27 | Regression guard | integration | integration | ✅ OK |
| T28 | Docs | none | none | ✅ OK |
| T29 | Final sweep | regression | regression | ✅ OK |

No ❌. "Tested in another task" is not used anywhere as a justification —
every task's own gate is self-sufficient.

---

## Requirement → Task Map

| Requirement | Tasks |
| ------------ | ----- |
| DLS-01 (declared-first lookup) | T7, T10, T12 |
| DLS-02 (plain-declaration unchanged) | T11 |
| DLS-03 (extern dispatches to registry) | T11 |
| DLS-04 (no declaration = fail loud) | T11, T12 |
| DLS-05 (extern w/ missing binding = distinct fail-loud) | T13 |
| DLS-06 (overload set accepted) | T8 |
| DLS-07 (overload resolved by arg type) | T9 |
| DLS-08 (`extern type` grammar) | T1 |
| DLS-09 (`extern fn` grammar) | T2 |
| DLS-10 (`extern task` grammar) | T3 |
| DLS-11 (`extern operator` grammar) | T4 |
| DLS-12 (`extern attribute` grammar) | T5 |
| DLS-13 (`extern impl` grammar) | T6, T10 |
| DLS-14 (extern + body = parse error) | T6 |
| DLS-15 (goto-def resolves extern decl_span) | T14 |
| DLS-16 (undeclared name = no location) | T15 |
| DLS-17 (primitive types migrated) | T16 |
| DLS-18 (math functions migrated) | T19 |
| DLS-19 (system tasks migrated) | T20, T21 |
| DLS-20 (runtime operators migrated) | T22 |
| DLS-21 (`@device`/`@port` migrated) | T23 |
| DLS-22 (plugin schema stub path) | T24, T25 |
| DLS-23 (bare-name casts removed) | T17, T18 |
| DLS-24 (native type methods → extern impl) | T26 |
| DLS-25 (zero regression / final closure) | T27, T28, T29 |

**Coverage**: 25 requirements, all mapped across 29 tasks.
