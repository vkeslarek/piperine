# MSL POM Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/msl-pom/design.md`
**Status**: Approved
**Approved**: 2026-11-13 - user: "Coloca o Fable para planejar essa feature, depois roda ela usando Sonnet impl) e Opus (verifier)"
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^warning')`
**Run design**: `cargo test -p piperine-lang --test pom_design`
**Run tree**: `cargo test -p piperine-lang --test pom_tree`
**Run body**: `cargo test -p piperine-lang --test pom_body`
**Run corpus**: `cargo test -p piperine-lang --test pom_corpus`
**Run resolve**: `cargo test -p piperine-lang --test pom_resolve`
**Run classify**: `cargo test -p piperine-lang --test pom_classify`
**Run reflect**: `cargo test -p piperine-lang --test pom_reflect`
**Run docs**: `cargo test -p piperine-lang --test pom_docs`

Executors: Sonnet sub-agents, one commit per task. Verifier: Opus. Work in `/home/keslarek/Git/piperine-msl-spec`, branch `feature/msl-pom`. Do not set `CARGO_TARGET_DIR` (the environment provides it). Never pass `--workspace`: the other crates do not build and are out of scope. `CLAUDE.md` still describes the retired Verilog-AMS pipeline; for this feature the facts are in `spec.md` and `design.md`.

Rules every task follows:

- A new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- No macro is written to generate ids, handles or accessors: data and plain functions. The standard derives (`Debug, Clone, PartialEq, Serialize, Deserialize`) go on every storage type.
- Only lowering (`src/pom/lower/`) pushes to the arenas. A resolver pass computes `(slot, value)` pairs, then applies them.
- No POM function panics on any input.
- A test asserts the spec's outcome on a source text written in the test (or on a spec fence); it never restates the implementation.
- `docs/spec/` is not edited by this feature.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` ("Where the tests are": a target lives in the crate it exercises, one concern per target, `//!` header), `tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope `allow`; it cannot run today because the root crate does not build, so its rules are applied by hand), the existing `crates/piperine-lang/tests/msl_*.rs` targets (integration tests over source text, corpus from `docs/spec/`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Design, builder, sources, loader (`pom/design.rs`, `pom/source.rs`) | integration | POM-01 ACs 1:1; empty source; disk trees in temp dirs | `crates/piperine-lang/tests/pom_design.rs` | run design |
| Declaration lowering and its handles (`pom/decl.rs`, `lower/items.rs`, `lower/hardware.rs`, `meta.rs`) | integration | POM-02..05 and POM-07 ACs 1:1; every declaration kind has a fixture | `crates/piperine-lang/tests/pom_tree.rs` | run tree |
| Body lowering and `to_ast` (`lower/body.rs`, `to_ast.rs`) | integration | every expression, statement, pattern and type form; error nodes; depth 256 | `crates/piperine-lang/tests/pom_body.rs` | run body |
| Whole-corpus properties | integration | every ```` ```rust* ```` fence of `docs/spec/*.md`: round trip, no panic, invariants | `crates/piperine-lang/tests/pom_corpus.rs` | run corpus |
| Scopes, imports, names (`resolve/scopes.rs`, `imports.rs`, `names.rs`, `builtin.rs`) | integration | POM-08 ACs 1:1; every lookup level of the scope order; each diagnostic code with its span | `crates/piperine-lang/tests/pom_resolve.rs` | run resolve |
| Behavior binding, classification, members (`resolve/behaviors.rs`, `classify.rs`, `members.rs`) | integration | POM-09, POM-10, POM-12 ACs 1:1; each class and each `Deferred` case | `crates/piperine-lang/tests/pom_classify.rs` | run classify |
| Reflection queries, reverse index, serialization (`reflect/`, `index.rs`, serde derives) | integration | POM-11 and POM-15 ACs 1:1; galleries of Appendices D, E, F; JSON round trip | `crates/piperine-lang/tests/pom_reflect.rs` | run reflect |
| Reference docs (`docs/pom/`, `mkdocs.yml`) | integration + doctest | POM-13 ACs 1:1: type, method and production coverage; `msl` fences; `rust` fences executed | `crates/piperine-lang/tests/pom_docs.rs`, doctests of `pom::guide` | run docs; C84's command |
| Data-only definitions (ids, node enums, resolution enums) | none | build gate only | - | build |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | after a task that touches one target | that task's `Run <name>` line(s) |
| Full | after a task that changes lowering or the resolver (earlier targets may move) | `cargo test -p piperine-lang` |
| Build | every task, before its commit | the `**Build**` line above |

---

## Execution Plan

Phases are ordered and run sequentially; tasks within a phase execute in order.

### Phase 1: Foundation

```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2: Value items, namespaces, source files

```
T6 → T7 → T8 → T9 → T10 → T11
```

### Phase 3: Hardware structure

```
T11 → T12 → T13 → T14 → T15 → T16 → T17
```

### Phase 4: Losslessness and robustness

```
T17 → T18 → T19 → T20 → T21
```

### Phase 5: Name resolution

```
T21 → T22 → T23 → T24 → T25 → T26 → T27 → T28
```

### Phase 6: Classification and members

```
T28 → T29 → T30 → T31 → T32 → T33
```

### Phase 7: Reflection and serialization

```
T33 → T34 → T35 → T36 → T37
```

### Phase 8: Reference documentation

```
T37 → T38 → T39 → T40 → T41
```

---

## Task Breakdown

### T1: Ids, `Design`, `SourceFile`, builder

**What**: The seven id types, the owning `Design` with its (still empty) arenas, `DesignBuilder::source/build` (one in-memory root), `SourceFile` (`name`, `path`, `text`, `ast`), `DesignDiagnostic`, parse diagnostics copied in; `pub mod pom;` in `lib.rs`; `serde` as a dependency (`serde_json`, `tempfile` as dev-dependencies), `Diagnostic.code` as `Cow<'static, str>`, serde derives on `Diagnostic`, `Span`, `Loc` and the reused AST leaf enums, `SourceFileRepr` (the AST is rebuilt from text). Shared test helper `tests/support/pom.rs` (`build(src)`, `codes(&design)`).
**Where**: `crates/piperine-lang/src/pom/design.rs` (+ `ids.rs`, `mod.rs`, one line in `src/lib.rs`, `Cargo.toml`, derive lines in `msl/diag.rs`, `msl/span.rs`, `msl/ast.rs`, `tests/support/pom.rs`, `tests/pom_design.rs`)
**Depends on**: None
**Reuses**: `msl::parser::parse`, `msl::Diagnostic`, `msl::Span`
**Requirement**: POM-01

**Done when**:

- [x] Test `build_retains_name_text_and_ast` passes in `pom_design.rs` (C1: POM-01 AC 1)
- [x] Test `parse_diagnostics_pass_through_with_file` passes in `pom_design.rs` (C2: POM-01 AC 2)
- [x] Test `design_is_send_and_sync` passes in `pom_design.rs` (C4: POM-01 AC 4)
- [x] `Design` has no public `&mut self` method and no `Cell`/`RefCell`/`Rc` field
- [x] Ids are hand-written newtypes over `u32`: no macro
- [x] From this task on, every storage type derives `Debug, Clone, PartialEq, Serialize, Deserialize`, and side tables are `BTreeMap` or `Vec`
- [x] The five existing `msl_*` targets still pass after the `Diagnostic.code` change (`cargo test -p piperine-lang`)
- [x] Gate passes: run design; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): design, source files and builder`

---

### T2: Declaration tree core

**What**: `DeclData`, `Name`, `DeclKind` with every variant of the design (payload structs start empty; later tasks add their fields), the root `Namespace` per file, one child declaration per `ast::Item` with kind, name, span and `public` (inline `mod` recursively), the generic handle `Decl`, the trait `Reflect` (`name`, `span`, `file`, `text`, `is_pub`, `declared_in`), `Design::root`, `find`, `dump`, `check`.
**Where**: `crates/piperine-lang/src/pom/decl.rs` (+ `lower/mod.rs`, `reflect/mod.rs`, `tests/pom_tree.rs`)
**Depends on**: T1
**Reuses**: T1 arenas
**Requirement**: POM-01, POM-02

**Done when**:

- [x] Test `two_builds_dump_identical` passes in `pom_design.rs` (C3: POM-01 AC 3)
- [x] Test `root_children_mirror_items_in_source_order` passes in `pom_tree.rs` (C7: POM-02 AC 1)
- [x] Test `inline_mod_is_a_namespace` passes in `pom_tree.rs` (C11: POM-02 AC 5)
- [x] Test `empty_source_has_empty_root` passes in `pom_design.rs`
- [x] `children` is in source order; ids are assigned in source order
- [x] `dump()` prints one line per declaration: indent, kind, name, `start..end`; no `HashMap` iteration
- [x] Gate passes: run design, run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): declaration tree, Decl handle, Reflect`

---

### T3: Body node types

**What**: Data only: `ExprNode`/`ExprKind` (33 variants), `StmtNode`/`StmtKind`, `PatNode`/`PatKind`, `TypeNode`/`TypeKind`, `PathNode`/`PathSegment`, `GenericArgsNode`, `Arg`, `FieldInit`, `Arm`, `ClosureParam`, `Block`, and `resolution.rs` (`Resolution`, `DeferReason`, `CallClass`, `StructClass`, `PortKind`, `NatureKind`); re-export of `ast::{UnOp, BinOp, Lit, LitKind, Dir, Quantity, BehaviorKind}`.
**Where**: `crates/piperine-lang/src/pom/body.rs` (+ `resolution.rs`)
**Depends on**: T2
**Reuses**: `msl/ast.rs` as the variant list
**Requirement**: POM-06

**Done when**:

- [x] One variant per AST variant, child nodes by id, nothing desugared
- [x] Every slot starts `Unresolved` / `Unclassified` / `Unknown`
- [x] Build line passes with no warning

**Tests**: none
**Gate**: build
**Commit**: `feat(lang/pom): body node and resolution types`

---

### T4: Body lowering and `const` items

**What**: Lowering of paths, types, generic arguments, patterns (one `Binding` declaration per bound identifier), expressions and statements into the arenas, each node with its span and owner; `Const` payload (`ty`, `value`) as the first consumer; handles `Expr`, `Stmt`, `Pat`, `Type`, `Path` (`kind`, `span`, `text`, `enclosing`) and `Const`.
**Where**: `crates/piperine-lang/src/pom/lower/body.rs` (+ `reflect/body.rs`, `reflect/items.rs`, `tests/pom_body.rs`)
**Depends on**: T3
**Reuses**: T3 node types
**Requirement**: POM-04, POM-06

**Done when**:

- [x] Test `const_type_and_value` passes in `pom_tree.rs` (C26: POM-04 AC 6)
- [x] Test `every_expression_form_has_its_kind` passes in `pom_body.rs` (C34: POM-06 AC 3)
- [x] Test `statement_pattern_and_type_forms_lower` passes in `pom_body.rs`
- [x] `Contribution`, `Commit`, `Signal` and `Reaction` statements are lowered here; their tests come with the behavior tasks
- [x] Error expressions become `ExprKind::Error`
- [x] Gate passes: run tree, run body; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): body lowering and const items`

---

### T5: Functions, generics, items in bodies

**What**: `Fn` payload (`is_const`, `abi`, `ret`, `where_`, `body`), `FnParam` and `GenericParam` declarations, items written inside a block (`StmtKind::Item`) parented to the enclosing declaration; handles `Function`, `FnParam`, `GenericParam`.
**Where**: `crates/piperine-lang/src/pom/lower/items.rs` (+ `reflect/items.rs`)
**Depends on**: T4
**Reuses**: T4 body lowering
**Requirement**: POM-02, POM-04

**Done when**:

- [x] Test `item_in_fn_body_is_declared_in_the_fn` passes in `pom_tree.rs` (C12: POM-02 AC 6)
- [x] Test `fn_abi_params_ret_body` passes in `pom_tree.rs` (C23: POM-04 AC 3)
- [x] A generics helper shared by every later item kind
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): functions, generic params, nested items`

---

### T6: Docs and attributes

**What**: `Meta`, `Doc`, `AttrNode`, `AttrInputNode` lowered for every declaration through one helper; file `//!` docs on the root namespace; `Reflect::docs`, `attrs`, `attr(name)`.
**Where**: `crates/piperine-lang/src/pom/meta.rs` (+ `reflect/mod.rs`)
**Depends on**: T5
**Reuses**: T4 expression lowering (for `#[doc = expr]`)
**Requirement**: POM-07

**Done when**:

- [x] Test `doc_comments_reach_docs` passes in `pom_tree.rs` (C35: POM-07 AC 1)
- [x] Test `attribute_arguments_are_exposed` passes in `pom_tree.rs` (C36: POM-07 AC 2)
- [x] `doc_comments_reach_docs` starts with file, `mod`, fn and const docs and is extended in T7, T13 and T14 for field, variant, module, port, param, signal
- [x] Attribute paths are stored, not resolved
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): doc comments and attributes`

---

### T7: Structs and enums

**What**: `Struct`, `Field`, `Enum`, `Variant` payloads and declarations (named, tuple and unit forms; `dir`, `pub`, discriminant); handles `Struct`, `Field`, `Enum`, `Variant`.
**Where**: `crates/piperine-lang/src/pom/lower/items.rs` (modify; + `reflect/items.rs`)
**Depends on**: T6
**Reuses**: T5 generics helper, T6 meta helper
**Requirement**: POM-04

**Done when**:

- [x] Test `struct_fields_and_enum_variants` passes in `pom_tree.rs` (C22: POM-04 AC 2)
- [x] `Struct.bundle` slot stays `None` until T29
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): structs and enums`

---

### T8: Traits and impls

**What**: `Trait` (supertraits, items) and `Impl` (generics, trait path, self type, items) payloads; handles `Trait`, `Impl`.
**Where**: `crates/piperine-lang/src/pom/lower/items.rs` (modify; + `reflect/items.rs`)
**Depends on**: T7
**Reuses**: T5 function lowering
**Requirement**: POM-04

**Done when**:

- [x] Test `impl_and_trait_parts` passes in `pom_tree.rs` (C24: POM-04 AC 4)
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): traits and impls`

---

### T9: `use` items

**What**: `Use` payload: the tree as written (`UseTreeNode`) and the flattened `imports` (path, alias, glob); handle `Use` with `imports()`.
**Where**: `crates/piperine-lang/src/pom/lower/items.rs` (modify; + `reflect/items.rs`)
**Depends on**: T8
**Reuses**: T4 path lowering
**Requirement**: POM-04

**Done when**:

- [x] Test `use_tree_flattens_to_imports` passes in `pom_tree.rs` (C25: POM-04 AC 5)
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): use items and imports`

---

### T10: `mod` files in memory, prelude sources

**What**: `source.rs`: `SourceProvider`, `MemorySources`, `mod_candidates` (Rust's layout with `.msl` and `mod.msl`); `DesignBuilder::source(path, text)` for further virtual files and `prelude(name, text)`; stage 0 follows every `mod b;` (also inside inline modules), loads the one readable candidate under an `External` namespace, or keeps it empty with `E0161` (none, both); `Namespace.origin`; handle `Namespace` (`items`, `submodules`, `origin`, `source_file`); `SourceFile::path`.
**Where**: `crates/piperine-lang/src/pom/source.rs` (+ `design.rs`, `lower/mod.rs`, `reflect/items.rs`)
**Depends on**: T9
**Reuses**: T2 namespace lowering
**Requirement**: POM-01

**Done when**:

- [x] Test `mod_loads_sibling_file_or_mod_file` passes in `pom_design.rs` (C5: POM-01 AC 5)
- [x] Test `mod_without_file_is_empty_namespace_e0161` passes in `pom_design.rs` (C6: POM-01 AC 6)
- [x] Test `mod_with_two_candidates_is_e0161` passes in `pom_design.rs` (C89: POM-01 AC 7)
- [x] Test `nested_mod_files_follow_rust_layout` passes in `pom_design.rs` (C90: POM-01 AC 8)
- [x] Files are numbered prelude sources first, then the root, then each loaded file in the order its `mod` item is met
- [x] `find("b::M")` walks namespaces across files
- [x] `#[path]` is given no meaning
- [x] Gate passes: run design; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): mod files by Rust's layout, prelude sources`

---

### T11: Loading from disk

**What**: `DiskSources`, `DesignBuilder::root_file(path)`, `Design::load(path)`; the chain of canonical paths being loaded and the cycle error; an unreadable root as an empty root with `E0161`; invalid UTF-8 through `parse_bytes`.
**Where**: `crates/piperine-lang/src/pom/source.rs` (modify; + `design.rs`)
**Depends on**: T10
**Reuses**: T10 provider and candidates, `tempfile`
**Requirement**: POM-01

**Done when**:

- [x] Test `mod_cycle_is_e0161_and_not_reloaded` passes in `pom_design.rs` (C91: POM-01 AC 9)
- [x] Test `load_from_disk_equals_in_memory_build` passes in `pom_design.rs` (C92: POM-01 AC 10)
- [x] Test `non_utf8_file_is_a_parse_diagnostic` passes in `pom_design.rs`
- [x] Tests build their trees in `tempfile::tempdir()`; the cycle test makes a directory symlink (`std::os::unix::fs::symlink`) and is `#[cfg(unix)]`
- [x] Nothing but `std::fs` and `std::path` is used by the loader
- [x] Gate passes: run design; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): load a source tree from disk`

---

### T12: Natures

**What**: `Nature` payload (fields in source order, `kind` computed from the declared fields), `Quantity` declarations with their attribute list; handles `Nature` (`kind`, `quantities`, `potential`, `flow`, `value_type`, `resolve_fn`) and `Quantity` (`role`, `access`, `units`, `abstol`).
**Where**: `crates/piperine-lang/src/pom/lower/hardware.rs` (+ `reflect/hardware.rs`)
**Depends on**: T11
**Reuses**: T5 generics helper
**Requirement**: POM-04

**Done when**:

- [x] Test `nature_quantities_and_kind` passes in `pom_tree.rs` (C21: POM-04 AC 1)
- [x] A repeated or mixed field gives `NatureKind::Invalid`; the fields stay in the list as written
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): natures and quantities`

---

### T13: Module heads

**What**: `Module` payload (`abi`, `ret`, `where_`, `result`), `Port` declarations (`dir`, `ty`, `default`); handles `Module` (`generics`, `ports`, `port`, `result`, `result_type`, `where_clause`, `is_extern`, `abi`) and `Port` (`index`, `direction`, `ty`, `default`).
**Where**: `crates/piperine-lang/src/pom/lower/hardware.rs` (modify; + `reflect/hardware.rs`)
**Depends on**: T12
**Reuses**: T5 generics helper, T6 meta helper
**Requirement**: POM-03

**Done when**:

- [x] Test `ports_in_order_with_direction_type_default` passes in `pom_tree.rs` (C13: POM-03 AC 1)
- [x] Test `extern_module_abi` passes in `pom_tree.rs` (C18: POM-03 AC 6)
- [x] Test `module_result_and_result_type` passes in `pom_tree.rs` (C19: POM-03 AC 7)
- [x] Test `module_generics_and_where` passes in `pom_tree.rs` (C20: POM-03 AC 8)
- [x] Extends `doc_comments_reach_docs` with module and port docs
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): module heads and ports`

---

### T14: Module body items

**What**: `Param`, `Signal`, `Let` (+ its `Binding` children, origin `BodyLet`) and `Definition` declarations; handles `Param`, `Signal`, `Let`, `Binding`, `Definition`; `Module::{params, param, signals, signal, lets, named, definitions, items, member}`; `Reflect::module()`.
**Where**: `crates/piperine-lang/src/pom/lower/hardware.rs` (modify; + `reflect/hardware.rs`)
**Depends on**: T13
**Reuses**: T4 pattern bindings
**Requirement**: POM-02, POM-03

**Done when**:

- [x] Test `signal_is_declared_in_its_module_only` passes in `pom_tree.rs` (C8: POM-02 AC 2)
- [x] Test `params_type_default_where_required` passes in `pom_tree.rs` (C14: POM-03 AC 2)
- [x] Test `signal_pub_type_init` passes in `pom_tree.rs` (C15: POM-03 AC 3)
- [x] Test `body_let_pattern_names_each_component` passes in `pom_tree.rs` (C16: POM-03 AC 4)
- [x] Test `definition_place_value_after` passes in `pom_tree.rs` (C17: POM-03 AC 5)
- [x] `is_required()` is syntactic: no default and a type that is not a path ending in `Option`
- [x] Extends `doc_comments_reach_docs` with param and signal docs
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): params, signals, lets, definitions`

---

### T15: Generation in module bodies

**What**: `Generation` declarations for `gen<module_item>` (`For` with its pattern bindings, `If` with `then`/`else` arms, nested generation); handles `Generation` and `GenArm`; `Module::generations`; `signals()`, `lets()`, `definitions()` become deep; `Signal::generation()`.
**Where**: `crates/piperine-lang/src/pom/lower/hardware.rs` (modify; + `reflect/hardware.rs`)
**Depends on**: T14
**Reuses**: T14 item lowering
**Requirement**: POM-02

**Done when**:

- [x] Test `generation_for_holds_its_signal_once` passes in `pom_tree.rs` (C9: POM-02 AC 3)
- [x] Test `generation_if_chain_exposes_three_arms` passes in `pom_tree.rs` (C10: POM-02 AC 4)
- [x] Items are held once, exactly as written: nothing is unrolled
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): generation in module bodies`

---

### T16: Analog behavior blocks

**What**: `Behavior` declarations at file scope for `analog` (kind, `names`, `target` path, `body` statements); handles `Behavior` (`kind`, `names`, `target`, `statements`, `contributions`) and `Contribution` (`lhs`, `rhs`, `stmt`).
**Where**: `crates/piperine-lang/src/pom/lower/hardware.rs` (modify; + `reflect/hardware.rs`, `reflect/body.rs`)
**Depends on**: T15
**Reuses**: T4 statement lowering
**Requirement**: POM-05

**Done when**:

- [x] Test `nested_contribution_is_listed` passes in `pom_tree.rs` (C28: POM-05 AC 2)
- [x] Test `behavior_generic_names` passes in `pom_tree.rs` (C31: POM-05 AC 5)
- [x] Test `analog_block_is_a_file_item` passes in `pom_tree.rs`
- [x] `contributions()` walks every nested block (`if`, `match`, `for`, plain blocks)
- [x] The `module` link is filled in T27
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): analog behavior blocks`

---

### T17: Digital behavior blocks

**What**: `Behavior` for `digital`, `Reaction` declarations (also for `StmtKind::Reaction`), `Generation` for `gen<digital_item>`; handles `Reaction` (`event`, `body`, `commits`) and `Commit` (`place`, `value`, `after`, `stmt`); `Behavior::{reactions, generations}`.
**Where**: `crates/piperine-lang/src/pom/lower/hardware.rs` (modify; + `reflect/hardware.rs`, `reflect/body.rs`)
**Depends on**: T16
**Reuses**: T15 generation lowering, T16 behavior lowering
**Requirement**: POM-05

**Done when**:

- [x] Test `reactions_and_deep_commits` passes in `pom_tree.rs` (C29: POM-05 AC 3)
- [x] Test `reaction_in_digital_generation` passes in `pom_tree.rs` (C30: POM-05 AC 4)
- [x] `reactions()` is deep: it includes reactions written inside generation, in source order
- [x] Gate passes: run tree; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): digital blocks, reactions, commits`

---

### T18: Error nodes, spans, depth

**What**: `Error` nodes for every AST error variant (`ItemKind::Error`, `ModuleItemKind::Error`, `DigitalItemKind::Error`, `FnParam::Error`, `ExprKind::Error`); span and `enclosing()` audit of every body node kind; the 256-level case.
**Where**: `crates/piperine-lang/tests/pom_body.rs` (modify; + fixes in `crates/piperine-lang/src/pom/lower/`)
**Depends on**: T17
**Reuses**: T4-T17 lowering
**Requirement**: POM-06, POM-14

**Done when**:

- [x] Test `node_text_is_authored_text_and_enclosing_is_nearest_decl` passes in `pom_body.rs` (C33: POM-06 AC 2)
- [x] Test `error_nodes_are_kept` passes in `pom_body.rs` (C87: POM-14 AC 2)
- [x] Test `depth_256_builds_on_default_stack` passes in `pom_body.rs` (C88: POM-14 AC 3)
- [x] If the depth test overflows, wrap the recursive body-lowering functions in `stacker::maybe_grow` and add `stacker` to `Cargo.toml`; otherwise add no dependency
- [x] Gate passes: run body; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `test(lang/pom): error nodes, spans, nesting depth`

---

### T19: `to_ast` for bodies, `const` and `fn`

**What**: `SourceFile::to_ast()` rebuilding `ast::File` from the arenas only: paths, types, patterns, expressions, statements, metas, `const`, `fn`.
**Where**: `crates/piperine-lang/src/pom/to_ast.rs`
**Depends on**: T18
**Reuses**: T3 node types
**Requirement**: POM-06

**Done when**:

- [x] Test `value_bodies_round_trip` passes in `pom_body.rs`
- [x] Never reads the retained `ast::File`
- [x] Spans are written back into every `Loc`
- [x] Gate passes: run body; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): to_ast for bodies, consts and fns`

---

### T20: `to_ast` for every item, and the corpus round trip

**What**: `to_ast` for structs, enums, traits, impls, `use`, `mod`, natures, modules, generation, behavior blocks and error items; the corpus target.
**Where**: `crates/piperine-lang/src/pom/to_ast.rs` (modify; + `tests/pom_corpus.rs`)
**Depends on**: T19
**Reuses**: `tests/support/spec_corpus.rs`
**Requirement**: POM-06

**Done when**:

- [x] Test `every_spec_block_round_trips_to_its_ast` passes in `pom_corpus.rs` (C32: POM-06 AC 1)
- [x] A `rust,fragment` fence is included when `parse` accepts it as a file
- [x] The test prints how many fences it compared and fails on zero
- [x] Gate passes: run corpus; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): lossless to_ast over the spec corpus`

---

### T21: Invariants over the corpus

**What**: `Design::check()` complete: parent/child symmetry, every id in range, every owner valid, every `Resolution::Decl` in range, no declaration reachable twice; run over every fence class.
**Where**: `crates/piperine-lang/src/pom/design.rs` (modify; + `tests/pom_corpus.rs`)
**Depends on**: T20
**Reuses**: T20 corpus target
**Requirement**: POM-14

**Done when**:

- [x] Test `every_spec_block_builds_with_consistent_invariants` passes in `pom_corpus.rs` (C86: POM-14 AC 1)
- [x] A fence that panics fails the test with its file and line (`std::panic::catch_unwind`)
- [x] Gate passes: run corpus; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): design invariants checked over the corpus`

---

### T22: Built-in table

**What**: `BuiltinInfo`, `BuiltinKind`, `BuiltinId`, `BUILTINS` (plain rows: primitive types, `Some`/`None`, bounds, prelude constants of §1, every row of the §14 table), `builtin(name)` with the `uN`/`iN`/`fN` helper.
**Where**: `crates/piperine-lang/src/pom/builtin.rs` (+ `tests/pom_resolve.rs`)
**Depends on**: T21
**Reuses**: `docs/spec/part_iv_behavior.md` §14, `part_i_values.md` §1
**Requirement**: POM-08

**Done when**:

- [x] Test `builtin_table_has_every_core_primitive` passes in `pom_resolve.rs`
- [x] Data + plain helpers, no macro
- [x] `u1`..`u128`, `i1`..`i128`, `f16`/`f32`/`f64`/`f128` match; `u0`, `u129`, `f8` do not
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): built-in name table`

---

### T23: Namespace and module scopes

**What**: Stage 2: per-namespace type and value tables, per-module member table; `E0457` for a namespace item named like a core primitive, `E0330` for a repeated member name.
**Where**: `crates/piperine-lang/src/pom/resolve/scopes.rs` (+ `resolve/mod.rs`)
**Depends on**: T22
**Reuses**: T22 table
**Requirement**: POM-08

**Done when**:

- [x] Test `core_name_redefined_is_e0457` passes in `pom_resolve.rs` (C49: POM-08 AC 13)
- [x] Test `duplicate_member_name_is_e0330` passes in `pom_resolve.rs` (C50: POM-08 AC 14)
- [x] A module name enters both tables (§3.4.5)
- [x] Lookup of a repeated name returns the first declaration
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): namespace and module scopes`

---

### T24: Imports, qualified paths, prelude

**What**: Stage 3: `use` to a fixpoint (simple, alias, group, glob), `self`/`super`/`crate`, visibility (`E0161` private), prelude sources visible everywhere; the fixture prelude `tests/fixtures/pom/prelude.msl` (natures `Bit`, `Wire`, `Electrical`, `Thermal`, `VSignal`, `impl Bit` with `rise`/`fall`, modules `idt`, `above`, `transition`, copied from Part VII and §4).
**Where**: `crates/piperine-lang/src/pom/resolve/imports.rs` (+ `tests/fixtures/pom/prelude.msl`)
**Depends on**: T23
**Reuses**: T23 tables, T10 prelude sources
**Requirement**: POM-08

**Done when**:

- [x] Test `use_and_qualified_paths_resolve` passes in `pom_resolve.rs` (C43: POM-08 AC 7)
- [x] Test `prelude_source_items_are_visible_everywhere` passes in `pom_resolve.rs` (C44: POM-08 AC 8)
- [x] Test `private_item_links_and_reports_e0161` passes in `pom_resolve.rs` (C46: POM-08 AC 10)
- [x] Test `local_item_wins_over_glob` passes in `pom_resolve.rs`
- [x] The fixture must build with zero diagnostics once T28 lands; until then only its own tests read it
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): imports, qualified paths, prelude sources`

---

### T25: Names in types and signatures

**What**: Stage 5, part 1: the scope stack; resolution of every type path, bound, supertrait, impl trait and self type; generics in scope of their declaration; `Self`; built-ins as the last lookup.
**Where**: `crates/piperine-lang/src/pom/resolve/names.rs`
**Depends on**: T24
**Reuses**: T24 imports
**Requirement**: POM-08

**Done when**:

- [x] Test `type_paths_resolve_to_declarations` passes in `pom_resolve.rs` (C41: POM-08 AC 5)
- [x] Test `builtin_names_resolve_to_table_entries` passes in `pom_resolve.rs` (C42: POM-08 AC 6)
- [x] Test `self_type_resolves` passes in `pom_resolve.rs` (C48: POM-08 AC 12)
- [x] Compute `(PathId, segment, Resolution)` pairs, then apply them
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): type and signature name resolution`

---

### T26: Names in bodies

**What**: Stage 5, part 2: block locals (sequential, shadowing), binders (closure, arm, `for`, `if let`/`while let`, fn params), items of function bodies, expression and pattern paths; `E0161` for an unresolved path; diagnostics sorted by file, offset, code.
**Where**: `crates/piperine-lang/src/pom/resolve/names.rs` (modify)
**Depends on**: T25
**Reuses**: T25 scope stack
**Requirement**: POM-08

**Done when**:

- [x] Test `shadowed_local_resolves_to_previous_binding` passes in `pom_resolve.rs` (C39: POM-08 AC 3)
- [x] Test `binders_scope_their_names` passes in `pom_resolve.rs` (C40: POM-08 AC 4)
- [x] Test `unresolved_path_is_one_e0161` passes in `pom_resolve.rs` (C45: POM-08 AC 9)
- [x] Test `diagnostics_are_ordered` passes in `pom_resolve.rs` (C51: POM-08 AC 15)
- [x] A pattern path that names a constant or variant resolves; a plain identifier pattern is a binding, not a reference
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): body name resolution, E0161`

---

### T27: Module scope and behavior binding

**What**: Stage 4 and its use in stage 5: behavior target resolved to a module of the same file (`E0401`, `E0402`), `names` checked against the generics (`E0403`), `Behavior.module` and `Module.behaviors` filled; module members visible in the whole body, in generation arms and in the module's blocks; `Behavior::module`, `Module::{behaviors, analog_blocks, digital_blocks}`.
**Where**: `crates/piperine-lang/src/pom/resolve/behaviors.rs` (+ `resolve/names.rs`, `reflect/hardware.rs`, `tests/pom_classify.rs`)
**Depends on**: T26
**Reuses**: T23 member tables, T26 walk
**Requirement**: POM-05, POM-08, POM-09

**Done when**:

- [x] Test `analog_block_is_file_item_linked_to_module` passes in `pom_tree.rs` (C27: POM-05 AC 1)
- [x] Test `block_reads_resolve_to_module_members` passes in `pom_resolve.rs` (C37: POM-08 AC 1)
- [x] Test `body_names_are_order_independent` passes in `pom_resolve.rs` (C38: POM-08 AC 2)
- [x] Test `behavior_without_module_is_e0401` passes in `pom_classify.rs` (C58: POM-09 AC 7)
- [x] Test `behavior_for_extern_module_is_e0402` passes in `pom_classify.rs` (C59: POM-09 AC 8)
- [x] Test `behavior_names_mismatch_is_e0403` passes in `pom_classify.rs` (C60: POM-09 AC 9)
- [x] Test `several_analog_blocks_in_order` passes in `pom_tree.rs`
- [x] Test `behavior_for_module_in_other_source_is_e0401` passes in `pom_classify.rs`
- [x] A block's generic names resolve to the module's generic params by position
- [x] Gate passes: run tree, run resolve, run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): module scope, behavior blocks bound to modules`

---

### T28: Associated paths

**What**: `Type::name`: enum variants, associated consts and fns of the `impl`s of the type, `Self::name`.
**Where**: `crates/piperine-lang/src/pom/resolve/names.rs` (modify)
**Depends on**: T27
**Reuses**: T27 resolver
**Requirement**: POM-08

**Done when**:

- [x] Test `associated_paths_resolve_to_variant_const_fn` passes in `pom_resolve.rs` (C47: POM-08 AC 11)
- [x] Test `fixture_prelude_builds_clean` passes in `pom_resolve.rs`
- [x] An `impl` is found by the declaration its self type resolves to
- [x] Gate passes: run resolve; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): associated path resolution`

---

### T29: Declared types, port kinds, bundles

**What**: Stage 6, part 1: `declared_type`, `nature_of` (through arrays and `flip`), `PortKind`, `Struct.bundle` (memoized, cycle-safe); `Port::{kind, nature}`, `Signal::nature`, `Struct::is_bundle`, `Nature::impls`.
**Where**: `crates/piperine-lang/src/pom/resolve/classify.rs` (+ `reflect/hardware.rs`, `tests/pom_reflect.rs`)
**Depends on**: T28
**Reuses**: T25 type resolution
**Requirement**: POM-09, POM-11

**Done when**:

- [x] Test `port_kinds` passes in `pom_classify.rs` (C52: POM-09 AC 1)
- [x] Test `struct_with_carrier_field_is_bundle` passes in `pom_classify.rs` (C53: POM-09 AC 2)
- [x] Test `carrier_nature_through_array_and_flip` passes in `pom_reflect.rs` (C71: POM-11 AC 4)
- [x] Test `self_referential_struct_bundle_check_ends` passes in `pom_classify.rs`
- [x] A generic param bounded by `Nature` makes a carrier port
- [x] Gate passes: run classify, run reflect; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): port kinds, bundles, carrier natures`

---

### T30: Instances, templates, literals

**What**: Stage 6, part 2: `CallClass` (module callee, template-typed param or value input, `Fn(..)`-bounded generic, fn returning a module type, built-in modules), `StructClass`, names of `Arg` and `FieldInit` linked to ports, params and fields, `Module.instances`, `E0130`; handle `Instance`; `Module::{instances, instance}`, `Binding::instance`, `Expr::as_instance`.
**Where**: `crates/piperine-lang/src/pom/resolve/classify.rs` (modify; + `reflect/hardware.rs`)
**Depends on**: T29
**Reuses**: T29 declared types
**Requirement**: POM-09, POM-12

**Done when**:

- [x] Test `named_instance_with_connections_and_params` passes in `pom_classify.rs` (C54: POM-09 AC 3)
- [x] Test `anonymous_instance_in_analog_and_plain_call` passes in `pom_classify.rs` (C55: POM-09 AC 4)
- [x] Test `struct_expression_classes` passes in `pom_classify.rs` (C56: POM-09 AC 5)
- [x] Test `instance_through_template_value` passes in `pom_classify.rs` (C57: POM-09 AC 6)
- [x] Test `impl_on_module_is_e0130` passes in `pom_classify.rs` (C62: POM-09 AC 11)
- [x] Test `generation_body_is_not_unrolled` passes in `pom_classify.rs` (C77: POM-12 AC 1)
- [x] Test `self_placement_under_generation_if` passes in `pom_classify.rs` (C78: POM-12 AC 2)
- [x] Test `unresolved_callee_is_plain_call` passes in `pom_classify.rs`
- [x] Classification creates no node: an `Instance` is a view over a `Call` expression
- [x] The instance name is the binding of a body `let` whose whole value is the call and whose pattern is one identifier
- [x] Gate passes: run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): instances, templates, literal classes`

---

### T31: Access aliases

**What**: Probe calls: a callee that is the `access` name of a quantity resolves by the nature of the first argument; one candidate in the whole design resolves directly; otherwise `Deferred(AmbiguousAccess)`; `CallClass::Probe`.
**Where**: `crates/piperine-lang/src/pom/resolve/classify.rs` (modify)
**Depends on**: T30
**Reuses**: T29 `nature_of`
**Requirement**: POM-09

**Done when**:

- [x] Test `access_alias_resolves_by_first_argument` passes in `pom_classify.rs` (C61: POM-09 AC 10)
- [x] A declaration with the same name in scope wins over an alias (G.16 stays open in the spec)
- [x] Gate passes: run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): access aliases resolve to nature quantities`

---

### T32: Field members

**What**: Stage 7, part 1: `e.name` on an instance (`pub` items; `E0343`), on a bundle carrier, on a value of a declared struct type; `Deferred(ReceiverTypeUnknown)` otherwise.
**Where**: `crates/piperine-lang/src/pom/resolve/members.rs`
**Depends on**: T31
**Reuses**: T29 declared types, T30 instances
**Requirement**: POM-10

**Done when**:

- [x] Test `instance_pub_member_resolves` passes in `pom_classify.rs` (C63: POM-10 AC 1)
- [x] Test `instance_private_member_is_e0343` passes in `pom_classify.rs` (C64: POM-10 AC 2)
- [x] Test `bundle_field_resolves` passes in `pom_classify.rs` (C65: POM-10 AC 3)
- [x] Test `unknown_receiver_is_deferred_without_diagnostic` passes in `pom_classify.rs` (C67: POM-10 AC 5)
- [x] The receiver is a chain of paths, fields, indexes and parens whose root has a declared type
- [x] Gate passes: run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): field member resolution, E0343`

---

### T33: Method members

**What**: Stage 7, part 2: `e.m()` in the order of §3.1.7: carrier built-ins, the `impl`s of the nature or bundle, the `impl`s of the value type.
**Where**: `crates/piperine-lang/src/pom/resolve/members.rs` (modify)
**Depends on**: T32
**Reuses**: T32 receiver typing, T22 table
**Requirement**: POM-10

**Done when**:

- [x] Test `methods_resolve_to_impl_fn_or_builtin` passes in `pom_classify.rs` (C66: POM-10 AC 4)
- [x] Gate passes: run classify; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): method member resolution`

---

### T34: Lookups, names, casts

**What**: `Design::{modules, module, natures, nature, functions, structs, namespaces}`, `Reflect::qualified_name`, every `Decl::as_*` cast, handle equality, `Instance::{connections, connection, param_bindings}`.
**Where**: `crates/piperine-lang/src/pom/reflect/mod.rs` (modify; + `reflect/hardware.rs`)
**Depends on**: T33
**Reuses**: T2 `find`
**Requirement**: POM-11

**Done when**:

- [x] Test `lookups_by_name_and_path` passes in `pom_reflect.rs` (C68: POM-11 AC 1)
- [x] Test `qualified_names` passes in `pom_reflect.rs` (C69: POM-11 AC 2)
- [x] Test `casts_match_kind_only` passes in `pom_reflect.rs` (C70: POM-11 AC 3)
- [x] Test `instance_connection_by_port` passes in `pom_reflect.rs` (C72: POM-11 AC 5)
- [x] Test `handle_equality` passes in `pom_reflect.rs` (C73: POM-11 AC 6)
- [x] `module(name)` searches every non-prelude namespace in file order and returns the first match
- [x] Gate passes: run reflect; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `feat(lang/pom): lookups, qualified names, casts`

---

### T35: Reverse index

**What**: Stage 8: `references`, `writers`, `instantiated_by` tables; `Reflect::references`, `Signal::writers`, `Port::writers`, `Module::instantiated_by`.
**Where**: `crates/piperine-lang/src/pom/index.rs` (+ `reflect/`)
**Depends on**: T34
**Reuses**: T30 instances, T32 members
**Requirement**: POM-11

**Done when**:

- [x] Test `references_in_source_order` passes in `pom_reflect.rs` (C74: POM-11 AC 7)
- [x] Test `writers_are_definitions_and_commits` passes in `pom_reflect.rs` (C75: POM-11 AC 8)
- [x] Test `instantiated_by_lists_instances` passes in `pom_reflect.rs` (C76: POM-11 AC 9)
- [x] The root of a place is found by stripping `Field`, `Index` and `Paren`
- [x] Gate passes: run reflect; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `feat(lang/pom): references, writers, instantiated_by`

---

### T36: Node-count invariant and galleries

**What**: The with/without-prelude comparison; reflection over Appendices D, E and F: every signal of every gallery module is declared in its module; `SarAdc` (F.1) instances name their modules.
**Where**: `crates/piperine-lang/tests/pom_reflect.rs` (modify)
**Depends on**: T35
**Reuses**: `tests/support/spec_corpus.rs`, the fixture prelude
**Requirement**: POM-12, POM-11

**Done when**:

- [x] Test `resolution_creates_no_node` passes in `pom_reflect.rs` (C79: POM-12 AC 3)
- [x] Test `gallery_signals_are_declared_in_their_module` passes in `pom_reflect.rs`
- [x] Test `sar_adc_instances_name_their_modules` passes in `pom_reflect.rs`
- [x] A name error the POM finds in a gallery is reported to the user in the task summary; `docs/spec/` is not edited
- [x] Gate passes: run reflect; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `test(lang/pom): node-count invariant, gallery reflection`

---

### T37: Serde round trip

**What**: The compile-time assertion that `Design` and each storage type are `Serialize + DeserializeOwned`; the `SarAdc` gallery design through `serde_json` and back; determinism of the JSON; any derive or `BTreeMap` fix the tests force.
**Where**: `crates/piperine-lang/tests/pom_reflect.rs` (modify; + derive fixes under `crates/piperine-lang/src/pom/`)
**Depends on**: T36
**Reuses**: T36 gallery fixture, `serde_json`
**Requirement**: POM-15

**Done when**:

- [x] Test `design_and_storage_types_implement_serde` passes in `pom_reflect.rs` (C93: POM-15 AC 1)
- [x] Test `gallery_design_round_trips_through_json` passes in `pom_reflect.rs` (C94: POM-15 AC 2)
- [x] Test `json_is_deterministic` passes in `pom_reflect.rs` (C95: POM-15 AC 3)
- [x] The retained AST is not in the JSON: the test checks that the serialized text holds no `"ast"` key and that the deserialized `ast()` equals the original's
- [x] Gate passes: run reflect; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: full
**Commit**: `test(lang/pom): serde round trip of a gallery design`

---

### T38: `docs/pom/index.md` and `model.md`

**What**: The overview (what the POM is, the unbreakable rule, the build call, a quick tour on `Front`, what it does not do) and the storage reference: one ``### `Name` `` heading with a field table for every public struct, enum and trait of `src/pom/`; the coverage test.
**Where**: `docs/pom/model.md` (+ `docs/pom/index.md`, `crates/piperine-lang/tests/pom_docs.rs`)
**Depends on**: T37
**Reuses**: the rustdoc comments of `src/pom/`
**Requirement**: POM-13

**Done when**:

- [x] Test `every_public_pom_type_has_a_heading` passes in `pom_docs.rs` (C80: POM-13 AC 1)
- [x] The test scrapes `pub struct|enum|trait` (not `pub(crate)`) from every `.rs` under `src/pom/`
- [x] MSL examples use ```` ```msl ```` fences
- [x] Gate passes: run docs; no test removed (the count of passing tests in each target does not drop)
- [x] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(pom): overview and model reference`

---

### T39: `docs/pom/reflection.md` and doctests

**What**: Every handle and every method: signature, result, example; `pom::guide` including each `docs/pom/*.md` through `#[doc = include_str!(..)]` so the `rust` fences run.
**Where**: `docs/pom/reflection.md` (+ `crates/piperine-lang/src/pom/guide.rs`, `crates/piperine-lang/tests/pom_docs.rs`)
**Depends on**: T38
**Reuses**: T38 test scaffolding
**Requirement**: POM-13

**Done when**:

- [ ] Test `every_reflection_method_is_documented` passes in `pom_docs.rs` (C81: POM-13 AC 2)
- [ ] C84 holds: its command exits 0 (POM-13 AC 5)
- [ ] Each `rust` example is complete (its own `use piperine_lang::pom::Design;`, its own source string, its own `assert`s): no hidden `#` lines, they would show in mkdocs
- [ ] Gate passes: run docs; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(pom): reflection reference, executable examples`

---

### T40: `docs/pom/mapping.md` and `resolution.md`

**What**: The construct → POM table (one row per B.3.2 production and per inherited Rust item: source form, node, handle, query) with worked `msl` + `rust` pairs; the resolution reference (scope order, built-in table, B.3.3 classes, `Deferred`, every diagnostic code).
**Where**: `docs/pom/mapping.md` (+ `docs/pom/resolution.md`, `crates/piperine-lang/tests/pom_docs.rs`)
**Depends on**: T39
**Reuses**: `docs/spec/appendix_b_grammar.md` B.3.2
**Requirement**: POM-13

**Done when**:

- [ ] Test `every_b3_production_is_mapped` passes in `pom_docs.rs` (C82: POM-13 AC 3)
- [ ] Test `msl_fences_parse_clean` passes in `pom_docs.rs` (C83: POM-13 AC 4)
- [ ] The test reads the production names from B.3.2 (`^name ::=` lines) at run time
- [ ] Gate passes: run docs; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(pom): construct mapping and resolution reference`

---

### T41: Navigation and crate docs

**What**: An "Object model" section in the `nav` of `mkdocs.yml` listing every `docs/pom/*.md`; the crate doc of `src/lib.rs` and the module doc of `src/pom/mod.rs` rewritten to describe the POM.
**Where**: `mkdocs.yml` (+ `crates/piperine-lang/src/lib.rs`, `crates/piperine-lang/src/pom/mod.rs`, `crates/piperine-lang/tests/pom_docs.rs`)
**Depends on**: T40
**Reuses**: the existing nav
**Requirement**: POM-13

**Done when**:

- [ ] Test `mkdocs_nav_lists_every_pom_page` passes in `pom_docs.rs` (C85: POM-13 AC 6)
- [ ] Gate passes: run docs; no test removed (the count of passing tests in each target does not drop)
- [ ] Build line passes with no warning

**Tests**: integration
**Gate**: quick
**Commit**: `docs(pom): mkdocs navigation, crate docs`

---

## Checks

**C1** - `files()` holds one `SourceFile` whose name is `top.msl`, whose text is the input and whose `ast()` equals `parse(text).file` (POM-01, AC 1)
Proof: run design | test build_retains_name_text_and_ast ... ok

**C2** - a source with a syntax error still yields a `Design`, and `diagnostics()` lists the parse diagnostic with its code, span and file id (POM-01, AC 2)
Proof: run design | test parse_diagnostics_pass_through_with_file ... ok

**C3** - two builds of the same sources give byte-identical `dump()` texts (POM-01, AC 3)
Proof: run design | test two_builds_dump_identical ... ok

**C4** - `Design` is `Send + Sync` (POM-01, AC 4)
Proof: run design | test design_is_send_and_sync ... ok

**C5** - with `b.msl` beside the root (or `b/mod.msl` instead) and `mod b;` in the root, `find("b::M")` is the module of that file and `M.file().path()` is that file (POM-01, AC 5)
Proof: run design | test mod_loads_sibling_file_or_mod_file ... ok

**C6** - `mod b;` with neither candidate keeps an empty namespace `b` and reports `E0161` at the `mod` item naming both candidate paths (POM-01, AC 6)
Proof: run design | test mod_without_file_is_empty_namespace_e0161 ... ok

**C7** - `root().children()` holds one declaration per item in source order, with kind, name, the item's AST span and `declared_in()` the root (POM-02, AC 1)
Proof: run tree | test root_children_mirror_items_in_source_order ... ok

**C8** - `module("abc").signal("w").declared_in()` is `abc`, and no other module lists `w` (POM-02, AC 2)
Proof: run tree | test signal_is_declared_in_its_module_only ... ok

**C9** - `for k in 0..4 { signal s: Bit; }` yields exactly one signal `s` whose parent is a `Generation` whose parent is the module (POM-02, AC 3)
Proof: run tree | test generation_for_holds_its_signal_once ... ok

**C10** - `if a {X} else if b {Y} else {Z}` exposes three arms in order, each with its condition (none for the last) and its own items (POM-02, AC 4)
Proof: run tree | test generation_if_chain_exposes_three_arms ... ok

**C11** - `mod a { module M() {} }` makes `find("a::M")` return `M` with `declared_in()` the namespace `a` (POM-02, AC 5)
Proof: run tree | test inline_mod_is_a_namespace ... ok

**C12** - an item declared in a function body has that function as `declared_in()` (POM-02, AC 6)
Proof: run tree | test item_in_fn_body_is_declared_in_the_fn ... ok

**C13** - `ports()` lists the ports in order with name, direction, type, default and 0-based `index()` (POM-03, AC 1)
Proof: run tree | test ports_in_order_with_direction_type_default ... ok

**C14** - params expose type, `default()`, `where_clause()` and `is_required()` (`r: f64 where r > 0.0` required; `tc1: f64 = 0.0` and an `Option<T>` param not) (POM-03, AC 2)
Proof: run tree | test params_type_default_where_required ... ok

**C15** - `pub signal sn: Electrical = 0.0;` exposes `is_pub()`, its type and its initial value (POM-03, AC 3)
Proof: run tree | test signal_pub_type_init ... ok

**C16** - `let (r, f) = (a, b);` exposes two named expressions sharing one `Let` item whose `value()` is the tuple (POM-03, AC 4)
Proof: run tree | test body_let_pattern_names_each_component ... ok

**C17** - `q = !(r | qn) after tpd;` is one definition with `place()`, `value()` and `after()` (POM-03, AC 5)
Proof: run tree | test definition_place_value_after ... ok

**C18** - an `extern "osdi" module` reports `is_extern()` and `abi() == "osdi"` (POM-03, AC 6)
Proof: run tree | test extern_module_abi ... ok

**C19** - a module with `-> T` and a trailing expression exposes `result_type()` and `result()`; without them both are none (POM-03, AC 7)
Proof: run tree | test module_result_and_result_type ... ok

**C20** - `<T: Repr, const A: u32>` and `where A >= 1` are exposed as two generic params in order and a `where_clause()` (POM-03, AC 8)
Proof: run tree | test module_generics_and_where ... ok

**C21** - a nature exposes its quantities with role, name and `access`/`units`/`abstol`, and `kind()` is `Conservative`, `SignalFlow`, `Discrete`, `Resolved` or `Invalid` by its fields (POM-04, AC 1)
Proof: run tree | test nature_quantities_and_kind ... ok

**C22** - struct fields expose name, direction, visibility and type; enum variants expose name, fields and discriminant (POM-04, AC 2)
Proof: run tree | test struct_fields_and_enum_variants ... ok

**C23** - `extern "C" fn f(x: f64) -> f64;` exposes `abi() == "C"`, one param, the return type and no body; a method's first param is `self` (POM-04, AC 3)
Proof: run tree | test fn_abi_params_ret_body ... ok

**C24** - each `impl` exposes its self type, its trait and its items; a trait exposes its supertraits and items (POM-04, AC 4)
Proof: run tree | test impl_and_trait_parts ... ok

**C25** - `use a::{b, c as d, e::*};` exposes three imports: `a::b`, `a::c` aliased `d`, and a glob of `a::e` (POM-04, AC 5)
Proof: run tree | test use_tree_flattens_to_imports ... ok

**C26** - `const N: u32 = 8;` exposes its type and value expression (POM-04, AC 6)
Proof: run tree | test const_type_and_value ... ok

**C27** - `analog for M {}` is a `Behavior` of kind `Analog` declared in the file namespace, `module()` is `M`, and `M.behaviors()` lists the blocks in source order (POM-05, AC 1)
Proof: run tree | test analog_block_is_file_item_linked_to_module ... ok

**C28** - a contribution nested under `if` is returned by `contributions()` with its `lhs()` and `rhs()` (POM-05, AC 2)
Proof: run tree | test nested_contribution_is_listed ... ok

**C29** - `reactions()` lists reactions in source order with `event()`, `body()` and every nested commit's `place()`, `value()`, `after()` (POM-05, AC 3)
Proof: run tree | test reactions_and_deep_commits ... ok

**C30** - a reaction inside a `for` of a `digital` block is declared in that `Generation`, which is declared in the block (POM-05, AC 4)
Proof: run tree | test reaction_in_digital_generation ... ok

**C31** - `analog<N> for Short<N>` exposes `names() == ["N"]` (POM-05, AC 5)
Proof: run tree | test behavior_generic_names ... ok

**C32** - for every `rust` and file-parsable `rust,fragment` fence of `docs/spec/*.md`, `file.to_ast()` equals the parsed `ast::File` (POM-06, AC 1)
Proof: run corpus | test every_spec_block_round_trips_to_its_ast ... ok

**C33** - each body node's `text()` is the authored text of that node and its `enclosing()` is the nearest declaration (POM-06, AC 2)
Proof: run body | test node_text_is_authored_text_and_enclosing_is_nearest_decl ... ok

**C34** - each of the 31 expression forms yields one node of the matching `ExprKind` (POM-06, AC 3)
Proof: run body | test every_expression_form_has_its_kind ... ok

**C35** - `///` and `/** */` comments on a module, port, param, signal, field and variant are returned by `docs()` in order; `//!` comments are the file namespace's `docs()` (POM-07, AC 1)
Proof: run tree | test doc_comments_reach_docs ... ok

**C36** - `attr("link")` returns the positional argument `"bsim4.osdi"` and the named argument `model = "bsim4"` (POM-07, AC 2)
Proof: run tree | test attribute_arguments_are_exposed ... ok

**C37** - in a block of `M`, a read of a port, param, signal, body `let` or generic of `M` resolves to that declaration (POM-08, AC 1)
Proof: run resolve | test block_reads_resolve_to_module_members ... ok

**C38** - a body item reading a name declared later in the body resolves to it (POM-08, AC 2)
Proof: run resolve | test body_names_are_order_independent ... ok

**C39** - in `let x = 1; let x = x + 1;` the right-hand `x` resolves to the first local (POM-08, AC 3)
Proof: run resolve | test shadowed_local_resolves_to_previous_binding ... ok

**C40** - closure params, `match` arm, `for`, `if let` and fn-param bindings resolve inside their scope and not outside it (POM-08, AC 4)
Proof: run resolve | test binders_scope_their_names ... ok

**C41** - a type path naming a nature, struct, enum, trait, module or generic param resolves to that declaration (POM-08, AC 5)
Proof: run resolve | test type_paths_resolve_to_declarations ... ok

**C42** - `f64`, `u8`, `Option`, `ddt`, `temperature` and `K_B` resolve to `Builtin` entries carrying their spec section, unless shadowed (POM-08, AC 6)
Proof: run resolve | test builtin_names_resolve_to_table_entries ... ok

**C43** - names imported by simple, aliased, grouped and glob `use`, and `self::`/`super::`/`crate::` paths, resolve to the named item (POM-08, AC 7)
Proof: run resolve | test use_and_qualified_paths_resolve ... ok

**C44** - the `pub` items of a prelude source resolve from another source without a `use` (POM-08, AC 8)
Proof: run resolve | test prelude_source_items_are_visible_everywhere ... ok

**C45** - a path naming nothing is `Unresolved`, reports exactly one `E0161` at its span, and the build completes (POM-08, AC 9)
Proof: run resolve | test unresolved_path_is_one_e0161 ... ok

**C46** - a private item reached from another namespace is linked and reports `E0161` with a message saying it is private (POM-08, AC 10)
Proof: run resolve | test private_item_links_and_reports_e0161 ... ok

**C47** - `Type::name` resolves to the enum variant, or to the associated const or fn of an `impl` of the type (POM-08, AC 11)
Proof: run resolve | test associated_paths_resolve_to_variant_const_fn ... ok

**C48** - `Self` resolves to the `impl`'s self-type declaration, or to the trait (POM-08, AC 12)
Proof: run resolve | test self_type_resolves ... ok

**C49** - a namespace-level item named like a §14 core primitive reports `E0457` at its name (POM-08, AC 13)
Proof: run resolve | test core_name_redefined_is_e0457 ... ok

**C50** - two equal member names in one module report `E0330` at the second with a secondary label at the first (POM-08, AC 14)
Proof: run resolve | test duplicate_member_name_is_e0330 ... ok

**C51** - `diagnostics()` is ordered by file, then start offset, then code (POM-08, AC 15)
Proof: run resolve | test diagnostics_are_ordered ... ok

**C52** - a port's `kind()` is `Carrier`, `Bundle`, `ValueInput` or `Unknown` by what its type resolves to (POM-09, AC 1)
Proof: run classify | test port_kinds ... ok

**C53** - a struct with a field whose type resolves to a nature, an array of natures or a bundle reports `is_bundle()` (POM-09, AC 2)
Proof: run classify | test struct_with_carrier_field_is_bundle ... ok

**C54** - `let r1 = Resistor(a, b) { r: 1k };` is an `Instance` named `r1`, of module `Resistor`, with two connections and one param binding `r` (POM-09, AC 3)
Proof: run classify | test named_instance_with_connections_and_params ... ok

**C55** - in `analog`, `idt(...)` (a module) is an anonymous `Instance` declared in the block and `windup(...)` (a fn) is not an instance (POM-09, AC 4)
Proof: run classify | test anonymous_instance_in_analog_and_plain_call ... ok

**C56** - `Name { .. }` is classified `Template`, `BundleLiteral` or `StructLiteral` by what `Name` resolves to (POM-09, AC 5)
Proof: run classify | test struct_expression_classes ... ok

**C57** - a call through a template-typed param, a `Fn(..)`-bounded generic or a fn returning a module type is an `Instance` with `via_template()` and, when the type names one, its `module()` (POM-09, AC 6)
Proof: run classify | test instance_through_template_value ... ok

**C58** - a behavior block whose path names no module of its file reports `E0401`, stays in the tree, and has `module()` none (POM-09, AC 7)
Proof: run classify | test behavior_without_module_is_e0401 ... ok

**C59** - a behavior block for an `extern` module reports `E0402` at the path (POM-09, AC 8)
Proof: run classify | test behavior_for_extern_module_is_e0402 ... ok

**C60** - block `names` that differ from the module's generics, or path arguments that are not those names, report `E0403` (POM-09, AC 9)
Proof: run classify | test behavior_names_mismatch_is_e0403 ... ok

**C61** - `v(p, n)` resolves `v` to the quantity with `access: v` of `p`'s nature; with an undeterminable first argument and two candidate natures the callee is `Deferred` (POM-09, AC 10)
Proof: run classify | test access_alias_resolves_by_first_argument ... ok

**C62** - an `impl` whose self type resolves to a module reports `E0130` (POM-09, AC 11)
Proof: run classify | test impl_on_module_is_e0130 ... ok

**C63** - `r1.x` resolves to the `pub` item `x` of the instance's module (POM-10, AC 1)
Proof: run classify | test instance_pub_member_resolves ... ok

**C64** - `r1.x` naming a non-`pub` item resolves to it and reports `E0343` at the member name (POM-10, AC 2)
Proof: run classify | test instance_private_member_is_e0343 ... ok

**C65** - `tx.valid` resolves to the field `valid` of `tx`'s bundle (POM-10, AC 3)
Proof: run classify | test bundle_field_resolves ... ok

**C66** - `clk.rise()` resolves to `fn rise` of `impl Bit`; `changed`, `prev`, `others`, `inflow`, `level` resolve to `Builtin` entries (POM-10, AC 4)
Proof: run classify | test methods_resolve_to_impl_fn_or_builtin ... ok

**C67** - a member whose receiver type is not readable from a declaration is `Deferred` and reports nothing (POM-10, AC 5)
Proof: run classify | test unknown_receiver_is_deferred_without_diagnostic ... ok

**C68** - `module(name)`, `nature(name)` and `find(path)` return the handle, and none for a name that matches nothing (POM-11, AC 1)
Proof: run reflect | test lookups_by_name_and_path ... ok

**C69** - `qualified_name()` of signal `w` of module `M` in `mod a` is `a::M.w`, and a `Generation` adds no segment (POM-11, AC 2)
Proof: run reflect | test qualified_names ... ok

**C70** - `as_module()`, `as_signal()` and the other casts return a handle only when the kind matches (POM-11, AC 3)
Proof: run reflect | test casts_match_kind_only ... ok

**C71** - `signal.nature()` and `port.nature()` return the nature through arrays and `flip` (POM-11, AC 4)
Proof: run reflect | test carrier_nature_through_array_and_flip ... ok

**C72** - `instance.connection(port)` returns the argument at that port's position (POM-11, AC 5)
Proof: run reflect | test instance_connection_by_port ... ok

**C73** - two handles are equal exactly when they come from the same `Design` and name the same node (POM-11, AC 6)
Proof: run reflect | test handle_equality ... ok

**C74** - `references()` returns every path and member resolved to the declaration, in source order (POM-11, AC 7)
Proof: run reflect | test references_in_source_order ... ok

**C75** - `writers()` returns every definition and commit whose place is rooted at the carrier, in source order (POM-11, AC 8)
Proof: run reflect | test writers_are_definitions_and_commits ... ok

**C76** - `instantiated_by()` returns every instance whose `module()` is the module (POM-11, AC 9)
Proof: run reflect | test instantiated_by_lists_instances ... ok

**C77** - `for k in 0..1000 { signal s: Bit; let u = Cell(s); }` holds one signal and one instance (POM-12, AC 1)
Proof: run classify | test generation_body_is_not_unrolled ... ok

**C78** - a module placing itself under a generation `if` builds, with one instance whose `module()` is the enclosing module (POM-12, AC 2)
Proof: run classify | test self_placement_under_generation_if ... ok

**C79** - the same text built with and without its prelude source has the same number and order of declarations, statements, expressions, patterns and types (POM-12, AC 3)
Proof: run reflect | test resolution_creates_no_node ... ok

**C80** - every `pub struct`, `pub enum` and `pub trait` under `src/pom/` has a heading ``### `Name` `` in `docs/pom/` (POM-13, AC 1)
Proof: run docs | test every_public_pom_type_has_a_heading ... ok

**C81** - every `pub fn` of `src/pom/reflect/` is mentioned as `` `name(` `` in `docs/pom/reflection.md` (POM-13, AC 2)
Proof: run docs | test every_reflection_method_is_documented ... ok

**C82** - every production name of Appendix B.3.2 is in a row of the mapping table of `docs/pom/mapping.md` that names a POM type (POM-13, AC 3)
Proof: run docs | test every_b3_production_is_mapped ... ok

**C83** - every `msl` fence of `docs/pom/*.md` builds with no parse diagnostic (POM-13, AC 4)
Proof: run docs | test msl_fences_parse_clean ... ok

**C84** - the `rust` fences of `docs/pom/*.md` pass as doctests of `piperine-lang`, and at least 10 run (POM-13, AC 5)
Proof: `cargo test -p piperine-lang --doc pom::guide 2>&1 | tee /dev/stderr | grep -qE 'test result: ok\. [1-9][0-9]+ passed'`

**C85** - `mkdocs.yml` lists every `docs/pom/*.md` file in its `nav` (POM-13, AC 6)
Proof: run docs | test mkdocs_nav_lists_every_pom_page ... ok

**C86** - every `rust`, `rust,fragment` and `rust,compile_fail` fence of `docs/spec/*.md` builds, and `check()` reports no broken invariant (POM-14, AC 1)
Proof: run corpus | test every_spec_block_builds_with_consistent_invariants ... ok

**C87** - an AST error node (item, module item, digital item, fn param, expression) is kept as an `Error` node with its span (POM-14, AC 2)
Proof: run body | test error_nodes_are_kept ... ok

**C88** - an expression nested 256 levels deep builds on a default 2 MiB test thread (POM-14, AC 3)
Proof: run body | test depth_256_builds_on_default_stack ... ok

**C89** - with both `b.msl` and `b/mod.msl` present, neither is loaded, `b` is empty and `E0161` names both paths (POM-01, AC 7)
Proof: run design | test mod_with_two_candidates_is_e0161 ... ok

**C90** - `mod c;` in `b.msl` and in `b/mod.msl` loads `b/c.msl` or `b/c/mod.msl`; inside an inline `mod a {}` of the root it loads `a/c.msl` or `a/c/mod.msl` (POM-01, AC 8)
Proof: run design | test nested_mod_files_follow_rust_layout ... ok

**C91** - a `mod` item that would load a file already on the loading chain (same canonical path) loads nothing and reports `E0161` saying the modules form a cycle (POM-01, AC 9)
Proof: run design | test mod_cycle_is_e0161_and_not_reloaded ... ok

**C92** - `Design::load(path)` on a temp directory gives the same `dump()` as the in-memory build of the same files; an unreadable root gives an empty root and one `E0161` naming the path (POM-01, AC 10)
Proof: run design | test load_from_disk_equals_in_memory_build ... ok

**C93** - `Design` and every public storage type of `pom` implement `Serialize` and `Deserialize` (POM-15, AC 1)
Proof: run reflect | test design_and_storage_types_implement_serde ... ok

**C94** - the `SarAdc` gallery design survives `serde_json` serialize + deserialize: `==` the original, same `dump()`, same `diagnostics()`, same `ast()` per file (POM-15, AC 2)
Proof: run reflect | test gallery_design_round_trips_through_json ... ok

**C95** - serializing the same design twice gives byte-identical JSON (POM-15, AC 3)
Proof: run reflect | test json_is_deterministic ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** B1 (Phase 1, T1–T6) Sonnet worker at f553b9c5 - returned (HEAD 8c3f84a8, 15 pom tests); B2 (Phase 2, T7–T11) Sonnet worker at 2bdfd477 - returned (HEAD 8cf88529); B3 (Phase 3, T12–T17) Sonnet worker at 8244631b - returned (HEAD 7160c941; PROCESS DEVIATION: T12–T17 in ONE commit, tests written after code — accepted by orchestrator, flagged for Verifier); B4 (Phase 4, T18–T21) Sonnet worker at 7e52e39d - returned (HEAD 4fb22caf, 4 commits); B5 (Phase 5, T22–T28) Sonnet worker at 380eb98c - returned (HEAD 93a81457, 7 commits); B6 (Phase 6, T29–T33) Sonnet worker at 64580b38 - returned (HEAD 57617671, 5 commits); B7 (Phases 7–8, T34–T41) Sonnet worker at 2949a3db
**B1 notes:** test helper `codes` in msl tests leaks a boxed str after `Diagnostic.code` became `Cow` (T1); shorthand struct-pattern binding points at the whole pattern; bare ident patterns (`None`) lowered as Binding until resolution refines; ExprKind has 31 variants (design said 33); `BuiltinId` lives in resolution.rs until T22; `Design::root()` returns `Decl` until T10 Namespace handle.
**B2 notes:** root file keeps children beside it like mod.msl (`is_root` param, SPEC_DEVIATION in source.rs); a loaded file's root is its `mod b;` namespace; non-UTF-8 files keep lossy text; `Design::root()` still `Decl` (use `.as_namespace()`); Import span of `a::{b}` leaf includes braces.
**B3 notes:** T12-T17 landed in ONE commit (the lowering/handles are interlinked; intermediate commits would not build warning-free). `ast::Quantity` is exported as `QuantityRole` (the handle `Quantity` takes the name; SPEC_DEVIATION in body.rs). `Binding::value()` is the whole `let` value (no tuple destructuring). `Module::{signals,params,lets,definitions}` deep; `generations()` direct only; `Generation::arms()` flattens an else-if chain (the else-if is a child Generation of the outer). `Reflect::module()` is itself-or-ancestor (a Behavior's link comes in T27). `Design::{module,modules,nature,natures}` added. T12 `NatureKind` rule: (1p,1f)=Conservative,(1p)=SignalFlow,(1v)=Discrete,(1v,1r)=Resolved, else Invalid. pom_design `two_builds_dump_identical` line indices updated (Module now has Port children; assertions strengthened). `Module.behaviors`/`instances` slots not yet added (T27/T30).
**B4 notes:** Doc has no inner/outer flag — loaded-file `//!` vs `mod b;` `///` told apart by text at span (heuristic); `check()` in pom/check.rs; arenas pub(super); NatureFieldNode::Value/Resolve carry spans; DeclData.generics_span.
**B5 notes:** VERIFIER LOOK: lowering tests in pom_tree/pom_body switched from `is_empty()` to `lowering_codes` (drops the 8 binding-error codes) once E0161 started firing on undeclared names in lowering fixtures; T24 prelude assertion narrowed to root file (whole fixture covered by T28 `fixture_prelude_builds_clean`); T26 impl written before tests (stash-red-restore). Gaps: `use` inside fn body unresolved silently; associated-item visibility unchecked; `f64::MAX` Deferred; E0403 head span includes attrs/docs.
**B6 notes:** ORCHESTRATOR DECISION: bundle slot is two-valued after classification — fill `Some(false)` for every classified non-bundle struct, remove the SPEC_DEVIATION in classify.rs, and change `pom_tree::struct_fields_and_enum_variants` from `bundle() == None` to `bundle() == Some(false)` (equally strict; `None` now means "not classified"). Instance connections/param_bindings landed in T30 (T34 adds `connection(port)`). Gaps: `Nature` bound detected by last path segment text; ports always readable by parent; missing field/method on known receiver stays Unresolved without diagnostic; carrier field access Deferred.
**Walked:** none (no screen; the stories' Independent Tests are the tests named in `## Checks`)

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6
Phase 2:  T6 → T7 → T8 → T9 → T10 → T11
Phase 3:  T11 → T12 → T13 → T14 → T15 → T16 → T17
Phase 4:  T17 → T18 → T19 → T20 → T21
Phase 5:  T21 → T22 → T23 → T24 → T25 → T26 → T27 → T28
Phase 6:  T28 → T29 → T30 → T31 → T32 → T33
Phase 7:  T33 → T34 → T35 → T36 → T37
Phase 8:  T37 → T38 → T39 → T40 → T41
```

Execution is strictly sequential; each phase diagram starts with the last task of the phase before it. Suggested batches of whole consecutive phases: Phase 1 (6 tasks) | Phase 2 (5) | Phase 3 (6) | Phase 4 (4) | Phase 5 (7) | Phase 6 (5) | Phases 7-8 (8).

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: Ids, `Design`, `SourceFile`, builder | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T2: Declaration tree core | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T3: Body node types | data types of one file pair, no logic | ✅ Granular |
| T4: Body lowering and `const` items | one mutually recursive lowering unit (expr ↔ block ↔ stmt ↔ pat ↔ type ↔ path cannot compile apart) + its first consumer | ⚠️ cohesive, cannot split |
| T5: Functions, generics, items in bodies | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T6: Docs and attributes | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T7: Structs and enums | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T8: Traits and impls | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T9: `use` items | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T10: `mod` files in memory, prelude sources | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T11: Loading from disk | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T12: Natures | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T13: Module heads | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T14: Module body items | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T15: Generation in module bodies | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T16: Analog behavior blocks | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T17: Digital behavior blocks | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T18: Error nodes, spans, depth | tests of one concern + the fixes they force | ✅ Granular |
| T19: `to_ast` for bodies, `const` and `fn` | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T20: `to_ast` for every item, and the corpus round trip | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T21: Invariants over the corpus | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T22: Built-in table | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T23: Namespace and module scopes | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T24: Imports, qualified paths, prelude | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T25: Names in types and signatures | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T26: Names in bodies | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T27: Module scope and behavior binding | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T28: Associated paths | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T29: Declared types, port kinds, bundles | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T30: Instances, templates, literals | one classification pass; its handle is the only way to observe it | ⚠️ cohesive, cannot split |
| T31: Access aliases | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T32: Field members | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T33: Method members | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T34: Lookups, names, casts | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T35: Reverse index | one lowering/resolver unit + the handles that expose it | ✅ Granular |
| T36: Node-count invariant and galleries | tests of one concern + the fixes they force | ✅ Granular |
| T37: Serde round trip | tests of one concern + the fixes they force | ✅ Granular |
| T38: `docs/pom/index.md` and `model.md` | one or two doc pages + their coverage test | ✅ Granular |
| T39: `docs/pom/reflection.md` and doctests | one or two doc pages + their coverage test | ✅ Granular |
| T40: `docs/pom/mapping.md` and `resolution.md` | one or two doc pages + their coverage test | ✅ Granular |
| T41: Navigation and crate docs | one or two doc pages + their coverage test | ✅ Granular |

A `Where` that lists secondary files names the handle file and the test target of the same unit: a lowering without its handle cannot be tested from an integration target, so they land together (merge-backward rule of test co-location).

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | phase start | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | T5 | T5 → T6 | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T8 | T8 → T9 | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |
| T11 | T10 | T10 → T11 | ✅ Match |
| T12 | T11 | T11 → T12 | ✅ Match |
| T13 | T12 | T12 → T13 | ✅ Match |
| T14 | T13 | T13 → T14 | ✅ Match |
| T15 | T14 | T14 → T15 | ✅ Match |
| T16 | T15 | T15 → T16 | ✅ Match |
| T17 | T16 | T16 → T17 | ✅ Match |
| T18 | T17 | T17 → T18 | ✅ Match |
| T19 | T18 | T18 → T19 | ✅ Match |
| T20 | T19 | T19 → T20 | ✅ Match |
| T21 | T20 | T20 → T21 | ✅ Match |
| T22 | T21 | T21 → T22 | ✅ Match |
| T23 | T22 | T22 → T23 | ✅ Match |
| T24 | T23 | T23 → T24 | ✅ Match |
| T25 | T24 | T24 → T25 | ✅ Match |
| T26 | T25 | T25 → T26 | ✅ Match |
| T27 | T26 | T26 → T27 | ✅ Match |
| T28 | T27 | T27 → T28 | ✅ Match |
| T29 | T28 | T28 → T29 | ✅ Match |
| T30 | T29 | T29 → T30 | ✅ Match |
| T31 | T30 | T30 → T31 | ✅ Match |
| T32 | T31 | T31 → T32 | ✅ Match |
| T33 | T32 | T32 → T33 | ✅ Match |
| T34 | T33 | T33 → T34 | ✅ Match |
| T35 | T34 | T34 → T35 | ✅ Match |
| T36 | T35 | T35 → T36 | ✅ Match |
| T37 | T36 | T36 → T37 | ✅ Match |
| T38 | T37 | T37 → T38 | ✅ Match |
| T39 | T38 | T38 → T39 | ✅ Match |
| T40 | T39 | T39 → T40 | ✅ Match |
| T41 | T40 | T40 → T41 | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: Ids, `Design`, `SourceFile`, builder | Design, builder, sources | integration | integration | ✅ OK |
| T2: Declaration tree core | Declaration lowering and its handles | integration | integration | ✅ OK |
| T3: Body node types | Data-only definitions | none | none | ✅ OK |
| T4: Body lowering and `const` items | Body lowering and `to_ast` | integration | integration | ✅ OK |
| T5: Functions, generics, items in bodies | Declaration lowering and its handles | integration | integration | ✅ OK |
| T6: Docs and attributes | Declaration lowering and its handles | integration | integration | ✅ OK |
| T7: Structs and enums | Declaration lowering and its handles | integration | integration | ✅ OK |
| T8: Traits and impls | Declaration lowering and its handles | integration | integration | ✅ OK |
| T9: `use` items | Declaration lowering and its handles | integration | integration | ✅ OK |
| T10: `mod` files in memory, prelude sources | Design, builder, sources | integration | integration | ✅ OK |
| T11: Loading from disk | Design, builder, sources | integration | integration | ✅ OK |
| T12: Natures | Declaration lowering and its handles | integration | integration | ✅ OK |
| T13: Module heads | Declaration lowering and its handles | integration | integration | ✅ OK |
| T14: Module body items | Declaration lowering and its handles | integration | integration | ✅ OK |
| T15: Generation in module bodies | Declaration lowering and its handles | integration | integration | ✅ OK |
| T16: Analog behavior blocks | Declaration lowering and its handles | integration | integration | ✅ OK |
| T17: Digital behavior blocks | Declaration lowering and its handles | integration | integration | ✅ OK |
| T18: Error nodes, spans, depth | Body lowering and `to_ast` | integration | integration | ✅ OK |
| T19: `to_ast` for bodies, `const` and `fn` | Body lowering and `to_ast` | integration | integration | ✅ OK |
| T20: `to_ast` for every item, and the corpus round trip | Whole-corpus properties | integration | integration | ✅ OK |
| T21: Invariants over the corpus | Whole-corpus properties | integration | integration | ✅ OK |
| T22: Built-in table | Scopes, imports, names | integration | integration | ✅ OK |
| T23: Namespace and module scopes | Scopes, imports, names | integration | integration | ✅ OK |
| T24: Imports, qualified paths, prelude | Scopes, imports, names | integration | integration | ✅ OK |
| T25: Names in types and signatures | Scopes, imports, names | integration | integration | ✅ OK |
| T26: Names in bodies | Scopes, imports, names | integration | integration | ✅ OK |
| T27: Module scope and behavior binding | Behavior binding, classification, members | integration | integration | ✅ OK |
| T28: Associated paths | Scopes, imports, names | integration | integration | ✅ OK |
| T29: Declared types, port kinds, bundles | Behavior binding, classification, members | integration | integration | ✅ OK |
| T30: Instances, templates, literals | Behavior binding, classification, members | integration | integration | ✅ OK |
| T31: Access aliases | Behavior binding, classification, members | integration | integration | ✅ OK |
| T32: Field members | Behavior binding, classification, members | integration | integration | ✅ OK |
| T33: Method members | Behavior binding, classification, members | integration | integration | ✅ OK |
| T34: Lookups, names, casts | Reflection queries and reverse index | integration | integration | ✅ OK |
| T35: Reverse index | Reflection queries and reverse index | integration | integration | ✅ OK |
| T36: Node-count invariant and galleries | Reflection queries and reverse index | integration | integration | ✅ OK |
| T37: Serde round trip | Reflection queries and reverse index | integration | integration | ✅ OK |
| T38: `docs/pom/index.md` and `model.md` | Reference docs | integration | integration | ✅ OK |
| T39: `docs/pom/reflection.md` and doctests | Reference docs | integration + doctest | integration | ✅ OK |
| T40: `docs/pom/mapping.md` and `resolution.md` | Reference docs | integration | integration | ✅ OK |
| T41: Navigation and crate docs | Reference docs | integration | integration | ✅ OK |
