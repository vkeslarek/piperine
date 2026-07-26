# PHDL Introspection Attributes Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Design**: `.specs/features/phdl-introspection-attributes/design.md`
**Status**: Done — 8/8 tasks delivered, independent Verifier PASS (see `validation.md`)

**AC syntax reconciliation (SPEC_DEVIATION — recorded, user-approved 2026-07-23):**
The spec ACs write single-field attributes positionally (`@name("i_d")`, `@unit("S")`,
`@kind("State")`). PHDL's attribute grammar is **keyed-only**
(`crates/piperine-lang/src/parse/parser/attributes.rs:11-35`: after `(` it calls
`parse_ident()` then `expect(Assign)`). User decision 2026-07-23: keep the grammar
unchanged; single-field schemas declare a `value: String` field and authors write
`@name(value = "i_d")`, `@unit(value = "S")`, `@kind(value = "State")`. `@model` is
already keyed (`@model(type = "mos", version = "3")`). Every AC's positional text is
read as its keyed equivalent. No parser change; no positional form added.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec. Guidelines found:
> `AGENTS.md` (build/verify bar: `cargo build --workspace` zero warnings,
> `cargo test --workspace` green; fail-loud rule; test-placement table; hand-written
> parser rule), `CLAUDE.md`. Existing tests are the **floor**, never the ceiling;
> thoroughness target comes from the spec ACs.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| PHDL schema/header declaration (lang elaboration gate) | unit | Every new `extern attribute` schema parses + registers; LSP go-to-def lands on the textual declaration (MD-24); unknown field fails loud at elab | `crates/piperine-lang/tests/*.rs` (`elab.rs`, `fail_loud_type_attr_resolution.rs`, `extern_grammar.rs`) | `cargo test -p piperine-lang` |
| POM resolver (`Design::introspection_meta`) | unit | 1:1 to spec ACs for validation paths: `@kind` enum membership (var→ObservableKind, terminal→TerminalKind), duplicate `@name` collision, wrong-placement, attribute-absent fallback. Mirror `rfport.rs` depth | `crates/piperine-lang/tests/*.rs` (new `introspection_meta.rs` or extend `elab.rs`) | `cargo test -p piperine-lang` |
| Introspect bridge (`PiperineDevice`) | integration | Every bridge AC end-to-end via the `parse_and_elaborate` → `lower_bodies` → `CircuitCompiler::build_circuit` → `all_devices()[0]` harness; one assertion per spec-defined outcome; absent-attribute → today's default | `crates/piperine-codegen/tests/*.rs` (`model_descriptor.rs`, `opvar_bridge.rs`, `observable_catalog.rs`, `terminal_bridge.rs`) | `cargo test -p piperine-codegen` |
| Kernel limiter catalog (`AnalogKernel`) | integration | Per-slot `(name, reason)` catalog populated from `$limit` kind literal; `limit_catalog()` accessor returns it in slot order; `limiters.rs` JIT harness | `crates/piperine-codegen/tests/limiters.rs` | `cargo test -p piperine-codegen` |
| Device limiting report (`PiperineDevice::limiting_report`) | integration | `limiter_name`/`reason` come from the slot that clamped; two-limiters-no-cross-contamination; unannotated `$limit` sites unchanged (goldens green) | `crates/piperine-codegen/tests/limiters.rs` (+ existing `checkpoint_limiter.rs` stays green) | `cargo test -p piperine-codegen` |

## Gate Check Commands

> Generated from codebase (`AGENTS.md` build/verify bar + Cargo workspace layout).

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After tasks touching one crate only (lang OR codegen kernel-only) | `cargo test -p <crate>` (plus `cargo build -p <crate>` first) |
| Full | After tasks with cross-crate integration tests (codegen bridge, device report) | `cargo test -p piperine-lang -p piperine-codegen -p piperine-solver` |
| Build | After phase completion, config/header changes, or the last task | `cargo build --workspace && cargo test --workspace` (zero warnings is the bar) |

---

## Execution Plan

Phases are ordered and run sequentially — each phase completes before the next begins, and tasks within a phase execute in order.

```
Phase 1 (Foundation):      T1 ──→ T2
Phase 2 (P1 model id):     T3
Phase 3 (P2 var+terminal): T4 ──→ T5 ──→ T6
Phase 4 (P3 limiters):     T7 ──→ T8
```

**Batch packing:** 8 tasks total → fits a single batch (≤ ~8). **No sub-agent dispatch —
execute inline in the main window.** (Per sub-agents.md: ≤~8 tasks = single batch inline.)

---

## Task Breakdown

### T1: Textual introspection attribute schemas + prelude wiring

**What**: Create `crates/piperine-lang/headers/introspection.phdl` declaring `@model`/
`@name`/`@unit`/`@description`/`@kind` as textual `extern attribute` schemas, and wire it
into the always-embedded prelude so every compilation unit registers them with `decl_span`
(LSP go-to-def, MD-24).
**Where**: `crates/piperine-lang/headers/introspection.phdl` (new); `crates/piperine-lang/
src/resolve.rs` (add a fifth `include_str!` block next to the `types.phdl`/`math.phdl`/
`tasks.phdl`/`operators.phdl` blocks at lines 107-134; MUST load after `types.phdl` since
fields reference `String`).
**Depends on**: None
**Reuses**: `@device`/`@port` textual header syntax (`crates/piperine-lang/headers/
device_port.phdl:16-23`); the existing `register_declared` textual path
(`crates/piperine-lang/src/elab/lower/register.rs:131-143`); the four existing prelude
`include_str!` blocks (`resolve.rs:107-134`).
**Requirement**: PIA-04

**Schema surface (keyed form, per user decision 2026-07-23):**
```phdl
extern attribute model { type: String, version: String }
extern attribute name { value: String }
extern attribute unit { value: String }
extern attribute description { value: String }
extern attribute kind { value: String }
```

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] `headers/introspection.phdl` exists with the five `extern attribute` declarations above
- [ ] `resolve.rs` loads it via `include_str!` after `types.phdl`; the schemas are present in every elaborated `SchemaRegistry` (with non-`None` `decl_span`)
- [ ] `@model(type = "mos", version = "3")` etc. parse + elaborate without error in a fresh project; an unknown field like `@model(bogus = 1)` fails loud at elaboration (`AttrSchemaField`, E2023) — covered by the existing `attrs.rs:84-93` path, asserted by a regression test
- [ ] No existing test regresses (the new schemas are additive; `@rfport` code-registration at `registry/mod.rs:56` stays untouched)
- [ ] Zero rustc warnings

**Tests**: unit (lang elaboration gate)
**Gate**: build (new header = prelude change; run the full workspace gate once)

**Commit**: `feat(lang): declare @model/@name/@unit/@description/@kind as textual extern attributes (PIA-04)`

---

### T2: `IntrospectionMeta` sidecar struct + `Design::introspection_meta` resolver

**What**: Define `IntrospectionMeta` (with `ModelId`/`VarMeta`/`TermMeta` per design
component 2) and a `Design::introspection_meta(module_name)` resolver that reads the new
attributes off `Module`/`Var`/`Port`/`Wire`, validates them (placement matrix, `@kind` enum
membership per placement, duplicate `@name`), and fails loud through
`ElabErrorKind::AttrSchemaField` — mirroring `Design::rfports()`.
**Where**: `crates/piperine-lang/src/pom/introspection.rs` (new module for the struct +
`ModelId`/`VarMeta`/`TermMeta`); `crates/piperine-lang/src/pom/design.rs` (add
`introspection_meta()` method next to `rfports()` at line 327); `crates/piperine-lang/src/
pom/mod.rs` (pub-mod the new module).
**Depends on**: T1 (schemas must be declared so attributes carry the right `schema()` names)
**Reuses**: `Design::rfports()` (`pom/design.rs:327-377`) as the iteration + `field_err`
template; `Attribute::schema()`/`field()` (`pom/module.rs:19-23`); `ObservableKind`
(`crates/piperine-solver/src/core/introspect.rs:213-229`) and `TerminalKind`
(`introspect.rs:175-183`) for enum membership.

**Placement matrix (resolver enforces; misplacement → `AttrSchemaField` fail-loud, PIA-03/19):**

| Schema | Module | Var | Port | Wire |
|--------|:------:|:---:|:----:|:----:|
| `@model` | ✓ | | | |
| `@name` | | ✓ | ✓ | ✓ |
| `@unit` | | ✓ | | |
| `@description` | | ✓ | ✓ | ✓ |
| `@kind` | | ✓ (→ObservableKind) | ✓ (→TerminalKind) | ✓ (→TerminalKind) |

**Enum string mapping (case-insensitive on the lowercased variant name):**
- Var `@kind(value)` ∈ {branchcurrent, charge, flux, state, var} → `ObservableKind`
- Port/Wire `@kind(value)` ∈ {external, internal, auxiliary} → `TerminalKind`
- Value outside the target enum → `AttrSchemaField { schema: "kind", field: "value", reason }` (PIA-09/13)

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] `IntrospectionMeta` struct (with `model: Option<ModelId>`, `vars: HashMap<String, VarMeta>`, `terminals: HashMap<String, TermMeta>`) is defined and exported from `piperine_lang::pom`
- [ ] `Design::introspection_meta(&self, module: &str) -> Result<IntrospectionMeta, ElabError>` resolves all five schemas per the placement matrix; attribute-absent → `None`/empty (additive, no regression)
- [ ] `@kind` value outside the placement's target enum fails loud (PIA-09 var, PIA-13 terminal)
- [ ] A schema on a wrong node kind fails loud (PIA-03 `@model` on a var; PIA-19 `@unit` on a module) — the resolver scans every node's attributes against the matrix
- [ ] Two vars with the same `@name(value)` fail loud (PIA-20 duplicate introspection name)
- [ ] `@unit`/`@description` on a non-opvar (non-`Var`) placement fails loud per the matrix
- [ ] Zero rustc warnings

**Tests**: unit (lang) — new `crates/piperine-lang/tests/introspection_meta.rs` covering: each schema resolves; each `@kind` enum branch (both placements); duplicate-name fail; misplacement fail (one per schema family); absent-attribute → empty meta. Mirror `rfport.rs` depth.
**Gate**: quick (`cargo test -p piperine-lang`)

**Commit**: `feat(lang): resolve introspection attributes into IntrospectionMeta sidecar (PIA-03,09,13,14,19,20)`

---

### T3: Plumb `IntrospectionMeta` through `CircuitCompiler` → `PiperineDevice` + `model_descriptor` reads it

**What**: Thread the sidecar from `CircuitCompiler` (which holds `&Design`) into
`PiperineDevice::new`, and make `Introspect::model_descriptor` prefer `meta.model` over
today's module-name echo. This is the P1 slice — the thinnest end-to-end proof of the
parse→POM→codegen→ABI path.
**Where**: `crates/piperine-codegen/src/device/circuit.rs` (resolve + cache per-module meta
alongside `kernels`, ~line 74-127); `crates/piperine-codegen/src/device/builder.rs` (pass
meta into `PiperineDevice::new` at both call sites lines 257, 354);
`crates/piperine-codegen/src/device/mod.rs` (add `meta: IntrospectionMeta` field +
constructor arg at lines 99-130; rewrite `model_descriptor` at lines 488-496 to prefer
`meta.model`).
**Depends on**: T2 (resolver produces the sidecar)
**Reuses**: `CircuitCompiler`'s existing `&Design` access (`module()`/`flat_module()` at
circuit.rs:114-127); `PiperineDevice::new` constructor (device/mod.rs:117); the test harness
in `crates/piperine-codegen/tests/model_descriptor.rs:16-25`.

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] `PiperineDevice` carries an `IntrospectionMeta` field; `new()` takes it (resolve `Default::default()` is NOT acceptable — every call site resolves real meta from `&Design`)
- [ ] `model_descriptor()` returns `ModelDescriptor { type_id, version }` from `meta.model` when present (PIA-01); else today's module-name echo with empty version (PIA-02 — no regression)
- [ ] Existing `model_descriptor.rs` tests stay green (no `@model` → module-name fallback unchanged)
- [ ] A stdlib-style module annotated `@model(type = "mos", version = "3")` yields exactly `ModelDescriptor { type_id: "mos", version: "3" }` from `model_descriptor()`
- [ ] Zero rustc warnings

**Tests**: integration (codegen) — extend `crates/piperine-codegen/tests/model_descriptor.rs`: add `model_descriptor_reads_at_model_attribute` (asserts both fields) and confirm the existing no-`@model` test still asserts the module-name echo.
**Gate**: full (`cargo test -p piperine-lang -p piperine-codegen -p piperine-solver`)

**Commit**: `feat(codegen): bridge @model attribute to ModelDescriptor via IntrospectionMeta sidecar (PIA-01,02)`

---

### T4: `list_queries` / `read_opvars` read `@name` / `@unit` / `@description` on vars

**What**: Make the opvar-query catalog honor `@name`/`@unit`/`@description` on a `var`: the
query name, unit, and description come from the sidecar when present; the opvar *value* read
stays sourced from the kernel by the var's kernel id. Absent attributes → today's
`QueryDescriptor::opvar(name)` default.
**Where**: `crates/piperine-codegen/src/device/mod.rs` (`list_queries` lines 365-376,
`read_opvars` lines 353-357).
**Depends on**: T3 (sidecar is on the device)
**Reuses**: `QueryDescriptor` fields (`crates/piperine-solver/src/core/introspect.rs:132-151`:
`name`/`kind`/`unit`/`description`); `VarMeta` from T2; `AnalogInstance::eval_opvars`
(`device/analog/mod.rs:518-538`) for value lookup keyed by the kernel var name.

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] `list_queries()` emits `QueryDescriptor` with `unit`/`description` filled from `meta.vars[kernel_name]` when present (PIA-05); bare `QueryDescriptor::opvar(n)` when absent (PIA-08, no regression)
- [ ] `read_opvars()` pairs use the `@name(value)` label when present, else the kernel var id (one source of truth — feeds PIA-07's "one name, both catalogs" alongside T5)
- [ ] The kernel var id is still used to look up the *value* (renaming the label must not break value fetch)
- [ ] Existing `opvar_bridge.rs` tests stay green (no attributes → today's names)
- [ ] Zero rustc warnings

**Tests**: integration (codegen) — extend `crates/piperine-codegen/tests/opvar_bridge.rs`: `query_descriptor_carries_unit_and_description`; `read_opvars_uses_at_name_label`; absent-attribute fallback.
**Gate**: quick (`cargo test -p piperine-codegen`)

**Commit**: `feat(codegen): bridge @name/@unit/@description on vars to QueryDescriptor (PIA-05,08)`

---

### T5: `list_observables` reads `@name` / `@kind` on vars — one name feeds both catalogs

**What**: Make the observable catalog honor `@name`/`@kind` on a `var`: the observable name
is the `@name(value)` (not a positional `ddt[k]`/`var[k]`), and the kind is the placement-
resolved `ObservableKind`. The SAME `meta.vars[v].name` read in T4 is read here — one
declaration, both catalogs (the inconsistency is structurally gone).
**Where**: `crates/piperine-codegen/src/device/mod.rs` (`list_observables` lines 535-567).
**Depends on**: T3 (sidecar); coordinates with T4 (shared `VarMeta.name` source — run after T4)
**Reuses**: `ObservableDescriptor`/`ObservableKind` (`introspect.rs:213-246`); the kernel
state-slot iteration already in `list_observables`.

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] A `var i_d @name(value = "i_d") @kind(value = "State")` surfaces in `list_observables()` as `ObservableDescriptor { name: "i_d", kind: State, .. }` — NOT `ddt[k]`/`var[k]` (PIA-06)
- [ ] The same `@name(value)` is the name in BOTH `list_queries()` (T4) and `list_observables()` — one declaration, two consistent catalogs (PIA-07 — structural, no unification code)
- [ ] A var with no attributes keeps today's positional/derived naming (PIA-08, no regression)
- [ ] `@kind(value = "Bogus")` on a var never reaches codegen — T2's resolver rejects it at elab (assert the elab error is raised, complementing T2's unit test)
- [ ] Existing `observable_catalog.rs` tests stay green
- [ ] Zero rustc warnings

**Tests**: integration (codegen) — extend `crates/piperine-codegen/tests/observable_catalog.rs`: `observable_named_by_at_name_not_positional`; `observable_kind_from_at_kind`; cross-catalog name consistency assertion (query name == observable name for the same var).
**Gate**: quick (`cargo test -p piperine-codegen`)

**Commit**: `feat(codegen): bridge @name/@kind on vars to ObservableDescriptor — one name, both catalogs (PIA-06,07,09)`

---

### T6: `list_terminals` reads `@name` / `@kind` on ports + wires (placement-resolved TerminalKind)

**What**: Make the terminal catalog honor `@name`/`@kind` on ports and internal wires: the
terminal name and `TerminalKind` come from the sidecar when present; absent → today's
position-inferred kind (port→External, non-port wire→Internal). `@kind("external")` on an
internal wire is legal (author declaration wins over inferred).
**Where**: `crates/piperine-codegen/src/device/mod.rs` (`list_terminals` lines 439-482 — the
analog block at 444-458 is the override site).
**Depends on**: T3 (sidecar); coordinates with T4/T5 (same device file — run after T5)
**Reuses**: `TerminalDescriptor.kind`/`TerminalKind` (`introspect.rs:175-183, 286-299`);
`TermMeta` from T2; `kernel.terminal_name(i)` for the fallback name.

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] A port with `@kind(value = "auxiliary")` → `TerminalDescriptor.kind == Auxiliary` (PIA-10)
- [ ] An internal `wire` with `@kind(value = "internal") @name(value = "cp")` → `Internal` named `"cp"` (PIA-11)
- [ ] A terminal with no `@kind` keeps the position-inferred kind (port→External, wire→Internal) (PIA-12, no regression)
- [ ] `@kind(value = "external")` on an internal wire is accepted (explicit wins over inferred, PIA — author-owned)
- [ ] `@kind(value = "bogus")` on a terminal never reaches codegen — T2 rejects it at elab (complement T2's unit test)
- [ ] Existing `terminal_bridge.rs` tests stay green
- [ ] Zero rustc warnings

**Tests**: integration (codegen) — extend `crates/piperine-codegen/tests/terminal_bridge.rs`: `port_at_kind_auxiliary`; `wire_at_kind_internal_named`; absent-kind fallback; explicit-external-on-wire-accepted.
**Gate**: full (last task of Phase 3 — `cargo test -p piperine-lang -p piperine-codegen -p piperine-solver`)

**Commit**: `feat(codegen): bridge @name/@kind on terminals to TerminalDescriptor (PIA-10..14)`

---

### T7: Kernel `Limits.catalog` + emit collects per-slot `(name, reason)` + `limit_catalog` accessor

**What**: Add a per-slot catalog `Vec<(&'static str, LimitReason)>` parallel to
`Limits.branches`, populated at emit (`emit_analog_limit` already knows `slot` + `kind`),
and expose it via `AnalogKernel::limit_catalog()`. Reason is inferred from kind
(`pnjlim`/`fetlim` → `VoltageStep`, `limvds` → `VdsStep`) — zero signature change to
`$limit` (MVP per design component 5).
**Where**: `crates/piperine-codegen/src/kernel/analog/limits.rs` (add `catalog` field to
`Limits` at line 8-22); `crates/piperine-codegen/src/emit/analog_expr.rs` (collect
`(kind, reason)` in `emit_analog_limit` at lines 298-330, threaded through the
emit-builder/compiler context); `crates/piperine-codegen/src/emit/builder.rs` (carry the
catalog alongside `limits`/`limit_base` at lines 50-53); `crates/piperine-codegen/src/
kernel/analog/compile.rs` (build the `Limits { catalog, .. }` literal ~line 653) and the
`AnalogCompiler` context (~lines 128/130); `crates/piperine-codegen/src/kernel/analog/
mod.rs` (add `limit_catalog()` accessor next to `limit_branches()` at lines 423-425).
**Depends on**: None (independent of the sidecar chain; the limiter path is intrinsic to
`$limit`, not metadata). Run after T6 for clean phase ordering.
**Reuses**: `emit_analog_limit`'s existing `match kind` (`analog_expr.rs:322-326`);
`LimitReason` (`crates/piperine-solver/src/core/element.rs:146-154`, already in
`piperine_solver::abi`); the `limiters.rs` JIT harness (`crates/piperine-codegen/tests/
limiters.rs:70-92`).

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] `AnalogKernel::limit_catalog()` returns `&[(&'static str, LimitReason)]` in slot order (parallel to `limit_branches()`)
- [ ] A device with `$limit("fetlim", ...)` and `$limit("limvds", ...)` populates the catalog as `[("fetlim", VoltageStep), ("limvds", VdsStep)]` in slot order; a `pnjlim` site yields `("pnjlim", VoltageStep)` (PIA-15)
- [ ] `$limit` signature is unchanged — no new required args (PIA-18 goldens unaffected)
- [ ] Unknown `$limit` kind still fails loud via the existing `CodegenError::unsupported` (`analog_expr.rs:326`)
- [ ] Existing `limiters.rs` + `checkpoint_limiter.rs` + `model_descriptor.rs` (vold-slot naming) stay green
- [ ] Zero rustc warnings

**Tests**: integration (codegen) — extend `crates/piperine-codegen/tests/limiters.rs`: `limit_catalog_names_slots_in_kind_order` (assert catalog contents for a two-limiter device built via the JIT harness).
**Gate**: quick (`cargo test -p piperine-codegen`)

**Commit**: `feat(codegen): collect per-slot limiter (name, reason) catalog on AnalogKernel (PIA-15)`

---

### T8: Device per-slot active tracking + `limiting_report` reads the catalog

**What**: Track which limit slot clamped (per-slot active, not a single bool — keeping the
aggregate `active()` as the OR for the existing Newton veto), and rebuild
`LimitingReport.limiter_name`/`reason` from `kernel.limit_catalog()[slot]` instead of the
hardcoded `"pnjlim"`/`VoltageStep`.
**Where**: `crates/piperine-codegen/src/device/analog/limits.rs` (`Limiter` struct line 13
— add per-slot active mask alongside the aggregate `active: bool`); `crates/piperine-codegen/
src/device/analog/mod.rs` (`rebuild_limit_report` lines 399-424 — read catalog at the
clamping slot; `Limiter::update` lines 116-149 — record the clamping slot).
**Depends on**: T7 (kernel exposes `limit_catalog()`)
**Reuses**: `Limits.update`/`seed`/`vnew` machinery (kernel/analog/limits.rs);
`LimitingReport`/`LimitReason` (`crates/piperine-solver/src/core/element.rs:131-154`);
existing `limiting_report()` delegation (`device/mod.rs:178-182`).

**Tools**:
- MCP: NONE
- Skill: NONE

**Done when**:
- [ ] `PiperineDevice::limiting_report()` returns `limiter_name`/`reason` from the slot that actually clamped (PIA-15 device-side); a `fetlim` clamp reports `"fetlim"`, a `limvds` clamp reports `"limvds"` — never hardcoded `"pnjlim"` (PIA-17 — two limiters name themselves independently, no cross-contamination)
- [ ] An omitted reason defaults to the kind-inferred `LimitReason` (VoltageStep for pnjlim/fetlim, VdsStep for limvds) — today's `VoltageStep` behavior preserved for unannotated `pnjlim` sites (PIA-16/18)
- [ ] The aggregate `Limiter::active()` (OR of per-slot) still drives the Newton convergence veto — existing MOS/diode limiting behavior unchanged (PIA-18 goldens green)
- [ ] Existing `limiters.rs` + `checkpoint_limiter.rs` + `crates/piperine-solver/tests/limiting_report.rs` stay green
- [ ] Zero rustc warnings

**Tests**: integration (codegen) — extend `crates/piperine-codegen/tests/limiters.rs`: `limiting_report_names_the_clamping_limiter` (drive a two-limiter PHDL device through `PiperineDevice::limiting_report()`, assert each name; assert a no-clamp iteration returns `None` or unchanged). Complement (do not duplicate) `crates/piperine-solver/tests/limiting_report.rs` (solver-side consumption stays as-is).
**Gate**: build (last task — `cargo build --workspace && cargo test --workspace`, zero warnings)

**Commit**: `feat(codegen): name the fired limiter in LimitingReport via per-slot catalog (PIA-15..18)`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1:  T1 ──→ T2
Phase 2:  T3
Phase 3:  T4 ──→ T5 ──→ T6
Phase 4:  T7 ──→ T8
```

Execution is strictly sequential — there is no intra-phase parallelism. A single agent works
one task at a time, in order. **8 tasks total → single batch, inline execution (no
sub-agents).**

**Dependency rationale:**
- T2 depends on T1 (schemas must be declared so attributes resolve).
- T3 depends on T2 (consumes the sidecar the resolver produces).
- T4/T5/T6 each depend on T3 (sidecar on the device); sequenced within Phase 3 because they
  share `device/mod.rs` (avoid edit conflicts), not because of data dependency between them.
- T7 is independent of the metadata chain (limiter naming is intrinsic to `$limit`); placed
  in Phase 4 after the metadata work for clean phase boundaries.
- T8 depends on T7 (reads the kernel catalog).

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: textual schemas + prelude wiring | 1 new header + 1 wiring edit | ✅ Granular |
| T2: IntrospectionMeta + resolver | 1 new module + 1 method (mirrors `rfports`) | ✅ Granular |
| T3: sidecar plumbing + `model_descriptor` | 1 constructor signature + 1 bridge method (cohesive P1 slice) | ✅ Granular |
| T4: queries/opvars var metadata | 2 bridge methods, 1 concern (opvar query catalog) | ✅ Granular |
| T5: observables var metadata | 1 bridge method, 1 concern (observable catalog) | ✅ Granular |
| T6: terminals metadata | 1 bridge method, 1 concern (terminal catalog) | ✅ Granular |
| T7: kernel limiter catalog + emit | 1 kernel field + emit collection + accessor (one concern: catalog) | ✅ Granular |
| T8: device per-slot active + report | 1 struct extension + 1 report builder (one concern: naming the fired slot) | ✅ Granular |

**Granularity check:**
- ✅ 1 component / 1 function / 1 concern = Good — all tasks pass
- T3 spans 3 files but is one cohesive P1 slice (plumb + first consumer); splitting the
  plumbing from the read would produce untestable intermediate code (a sidecar with no
  consumer) — exactly the anti-pattern the Tasks guidance says to merge forward.

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | (Phase 1 root, no inbound arrow) | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | T3 (coordinates with T4 for sequencing) | T4 → T5 | ✅ Match (T5→T3 data dep + T4→T5 file sequencing both hold) |
| T6 | T3 (coordinates with T5 for sequencing) | T5 → T6 | ✅ Match |
| T7 | None (Phase 4 root) | (Phase 4 root, no inbound arrow from Phase 3) | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |

**Rules check:**
- Every `Depends on` has a corresponding diagram arrow: ✅
- Every diagram arrow has a corresponding `Depends on`: ✅
- No task depends on a task in a later phase: ✅

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1 | PHDL schema/header declaration (lang) | unit | unit (lang elaboration gate) | ✅ OK |
| T2 | POM resolver (lang) | unit | unit (new `introspection_meta.rs`) | ✅ OK |
| T3 | Introspect bridge (codegen) | integration | integration (extend `model_descriptor.rs`) | ✅ OK |
| T4 | Introspect bridge (codegen) | integration | integration (extend `opvar_bridge.rs`) | ✅ OK |
| T5 | Introspect bridge (codegen) | integration | integration (extend `observable_catalog.rs`) | ✅ OK |
| T6 | Introspect bridge (codegen) | integration | integration (extend `terminal_bridge.rs`) | ✅ OK |
| T7 | Kernel limiter catalog (codegen) | integration | integration (extend `limiters.rs`) | ✅ OK |
| T8 | Device limiting report (codegen) | integration | integration (extend `limiters.rs`) | ✅ OK |

**Rules check:**
- No `Tests: none` deferral: ✅
- Every task writes its required tests in-task (co-located): ✅
- Compilation dependencies resolved inline (T3 includes the first consumer so the sidecar is
  testable in-task; no forward-deferral): ✅

---

## Requirement Coverage (post-execution update)

| Requirement ID | Task | Status |
| -------------- | ---- | ------ |
| PIA-01 | T3 | Pending |
| PIA-02 | T3 | Pending |
| PIA-03 | T2 | Pending |
| PIA-04 | T1 | Pending |
| PIA-05 | T4 | Pending |
| PIA-06 | T5 | Pending |
| PIA-07 | T4 + T5 (structural) | Pending |
| PIA-08 | T4 (and T5/T6 by parity) | Pending |
| PIA-09 | T2 (elab reject) + T5 (codegen never sees it) | Pending |
| PIA-10 | T6 | Pending |
| PIA-11 | T6 | Pending |
| PIA-12 | T6 | Pending |
| PIA-13 | T2 (elab reject) + T6 | Pending |
| PIA-14 | T2 (placement-resolved mapping) | Pending |
| PIA-15 | T7 (kernel) + T8 (device) | Pending |
| PIA-16 | T8 (inferred default) | Pending |
| PIA-17 | T8 | Pending |
| PIA-18 | T7 + T8 (unchanged signature/goldens) | Pending |
| PIA-19 | T2 | Pending |
| PIA-20 | T2 | Pending |

**Coverage:** 20 requirements, all mapped to tasks.
