# PHDL Introspection Attributes Specification

> **Follow-up to** `element-abi-maturity` (delivered 2026-07-23). That feature
> built the **reflective** ABI/codegen bridge — `PiperineDevice` reads kernel
> data into `ModelDescriptor`/opvar/observable/terminal catalogs. This feature
> makes those catalogs **declarative**: the device author controls them from
> PHDL, instead of the codegen echoing derived defaults.

**Implements:** ROADMAP P2 "Follow-up: PHDL introspection attributes".
**Builds on:** MD-24 (declared language surface — every name is a textual
declaration; the `extern attribute` schema is the extension point), MD-25 (POM
navigability — attributes are additive POM data on authored declarations).

## Problem Statement

After `element-abi-maturity`, a JIT-compiled device surfaces a full introspection
catalog through the `Introspect` trait — but every field is a codegen-derived
default the author cannot control or annotate:

1. `ModelDescriptor.type_id` echoes the **module name**; there is no author-set
   model type/version (`{ type: "mos", version: "3" }`).
2. Opvars (`gm`, `vbe`) carry a name and value but **no units or description** —
   a host renders `gm = 1e-3` with no idea it is a transconductance in siemens.
3. Observables are named **positionally** (`ddt[0]`, `vold[1]`) while opvars are
   named by their `var` identifier — one device speaks two naming schemes for
   the same underlying state (the opvar-name vs observable-name inconsistency).
4. `TerminalKind` (External/Internal/Auxiliary) is **inferred** from port-vs-wire
   position; the author cannot mark a port as auxiliary or name a terminal.
5. `LimitingReport.limiter_name`/`reason` are **hardcoded** `"pnjlim"`/
   `VoltageStep` — a MOSFET running `limvds` alongside `fetlim` reports both as
   `"pnjlim"`, so host diagnostics cannot tell which limiter fired even though
   `$limit(kind: String)` already carries the distinguishing `kind` at the call
   site.

Each gap is a place where the ABI has the *slot* but the author has no *language
surface* to fill it.

### Design shape: atomic metadata attributes, not role bundles

The metadata is expressed with **small, single-purpose attributes** that compose
on any declaration — `@name`, `@unit`, `@description`, `@kind` — rather than
role-shaped bundles (`@opvar(...)`/`@observable(...)`/`@terminal(...)`) (user
decision 2026-07-23). Model identity is the one deliberate pair:
`@model(type, version)` carries both fields in one attribute (a model type is
meaningless without its version). Consequences:

- **The opvar/observable inconsistency dissolves at the root.** A `var` has ONE
  `@name`; both the opvar query catalog and the observable catalog read it.
  There is nothing to "unify" — a single declaration feeds both.
- **`@kind` is placement-resolved.** On a `var` it names an `ObservableKind`; on
  a port/wire it names a `TerminalKind`. One attribute, interpreted by what it
  annotates.
- **Reusable and extensible.** `@description` annotates a module, a var, or a
  terminal uniformly; adding `@unit` to a terminal later needs no new bundle.

`$limit`'s naming (item 5) is an operator argument, not an attribute — kept as
`$limit` optional args (user decision 2026-07-23).

## Goals

- [ ] `@model(type, version)` on a module populates `ModelDescriptor` from the
      attribute, not the module name.
- [ ] `@name`/`@unit`/`@description`/`@kind` on a `var` name and annotate its
      opvar query entry AND its observable catalog entry — one declaration, both
      catalogs, no positional `ddt[k]` naming, inconsistency gone.
- [ ] `@name`/`@description`/`@kind` on a port or internal wire classify the
      terminal (external/internal/auxiliary) and name it, feeding
      `TerminalDescriptor`.
- [ ] `$limit`'s `kind` argument (plus an optional reason) flows into
      `LimitingReport.limiter_name`/`reason` — no more hardcoded `"pnjlim"`.
- [ ] Every new attribute is a **textual** `extern attribute` declaration
      (MD-24), resolvable by LSP go-to-definition; no code-only registration.
- [ ] `cargo test --workspace` green; zero rustc warnings.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| **Reflective ABI bridge itself** | Delivered in `element-abi-maturity`. This feature only adds the declarative *inputs* that override the derived defaults; the catalogs, `Introspect` methods, and codegen bridge already exist. |
| **`@rfport` migration to textual** | `@rfport` is a pre-existing code-registered stdlib-reserved schema (`registry/mod.rs:56`). Migrating it to a textual header is an MD-24 cleanup tracked separately; this feature only mirrors its *registration* pattern for the new prelude-level schemas (which ARE textual). |
| **`@port`/`@device` reuse for terminals** | `@port` is the plugin device-wiring schema, only in scope once a plugin loads (`device_port.phdl:7`). Terminal classification is a stdlib/introspection concern for every project — a distinct purpose, so distinct (atomic) attributes, never `@port` (user decision 2026-07-23). |
| **New limiter *algorithms*** | `fetlim`/`limvds` already ship (ROADMAP P1, `81f36af`). This feature only makes the *report* name the limiter that actually fired; it adds no new limiting math. |
| **Host-side rendering of the new metadata** | Units/description/kind reaching the Python/LSP UI is a P3/P4 host-surface task (MD-22); this feature stops at the ABI surface carrying the data. |

---

## Assumptions & Open Questions

Every ambiguity is resolved or recorded here — nothing is left silently unclear.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Feature scope | All 5 items ship as one coherent "declarative introspection" feature, P1/P2/P3 sliced internally | User 2026-07-23 — one spec, one review | y (user) |
| Attribute granularity | Atomic single-purpose attributes (`@name`/`@unit`/`@description`/`@kind`), not role bundles (`@opvar`/`@observable`/`@terminal`) | User 2026-07-23 — composable, DRY, dissolves the opvar/observable inconsistency at the root | y (user) |
| `@kind` context-resolution | The same `@kind` attribute is legal on a `var` (→ `ObservableKind`) and a port/wire (→ `TerminalKind`); the target enum is chosen by placement | Atomic attributes reused across declaration kinds; placement disambiguates cleanly | n (Design) |
| opvar vs observable naming | One `@name` on the `var` is the single naming truth; the opvar query and the observable both read it (no separate observable-name) | Atomic model means there is nothing to unify — one declaration, both catalogs | y (user, implied by granularity choice) |
| Named-limiter mechanism | Optional args on `$limit`: the existing `kind: String` becomes `limiter_name`; add an optional reason. JIT threads both into `LimitingReport` | User 2026-07-23 — the call site already knows which limiter it is; `$limit` is an operator, kept as args not an attribute | y (user) |
| Model identity form | One attribute `@model(type, version)` carrying both fields — the deliberate exception to the atomic style | User 2026-07-23 — a model type is meaningless without its version; the pair belongs together. `ModelDescriptor { type_id, version }` maps 1:1 | y (user) |
| New-schema declaration form | Textual `extern attribute` in a prelude header (e.g. `headers/introspection.phdl`), registered unconditionally like `@rfport` but WITH a textual source (MD-24) | MD-24 requires a textual declaration for LSP; `@rfport`'s code-only registration is the exception to avoid, not copy | n (Design) |
| `@kind`/enum-string domain | `@kind` values map onto the existing `ObservableKind`/`TerminalKind` enums; a value outside the target enum fails loud at elaboration | Reuse delivered enums; fail-loud per project bar | n (Design) |
| Attribute-absent behavior | Every attribute is **optional**: no `@model` → module-name echo; no `@name` on a var → var-id/positional default; no `@kind` on a terminal → position-inferred kind. Pure additive, zero regression | Backward compatibility — existing stdlib models compile unchanged | y (design intent) |
| `$limit` reason values | `reason` maps onto `LimitReason` (`VoltageStep`/`VdsStep`/`Other(&str)`); omitted reason defaults to `VoltageStep` (today's value) | Reuse the delivered enum; preserve current behavior when unannotated | n (Design) |

**Open questions:** the items marked `n (Design)` — header file name, `@kind`
placement-resolution mechanics, enum-string mapping, fail-vs-default on unknown
values. These are HOW-shape decisions the Design phase resolves; they do not
change WHAT the spec requires.

---

## User Stories

### P1: `@model(type, version)` — author-set model identity ⭐ MVP

**User Story**: As a device author, I declare my module's model type and version
in PHDL so a host reads `{ type: "mos", version: "3" }` from the introspection
surface instead of the raw module name.

**Why P1**: Thinnest vertical slice through the whole mechanism (parse attribute
→ POM → codegen reads it → `ModelDescriptor`). Proves the pipeline end to end and
delivers the highest-value catalog field (model identity is what a host renders
first). Every later story reuses this parse→POM→bridge path.

**Acceptance Criteria**:

1. WHEN a module carries `@model(type = "mos", version = "3")` THEN
   `Introspect::model_descriptor()` on its `PiperineDevice` SHALL return
   `ModelDescriptor { type_id: "mos", version: "3" }`.
2. WHEN a module carries no `@model` attribute THEN `model_descriptor()` SHALL
   fall back to today's default (module-name echo) — no regression.
3. WHEN `@model` is applied to a declaration kind it does not target (e.g. a
   `var`) THEN elaboration SHALL fail loud with a placement error.
4. WHEN `@model` is written with an unknown field THEN elaboration SHALL fail
   loud (schema validation).
5. WHEN a developer ctrl+clicks `@model` THEN it SHALL resolve to a textual
   `extern attribute model { type: String, version: String }` declaration
   (MD-24).

**Independent Test**: A stdlib MOS module annotated
`@model(type = "mos", version = "3")`; compile to `PiperineDevice`; assert
`model_descriptor()` reads the annotated values. A second module with no `@model`
asserts the module-name fallback.

---

### P2: `@name`/`@unit`/`@description`/`@kind` on vars — named, annotated device state

**User Story**: As a device author, I annotate an operating-point `var` with a
name, unit, description, and observable kind, so a host renders `gm = 1.2 mS
(transconductance)` and records `i_d` by name instead of `ddt[0]` — from ONE
declaration feeding both the query catalog and the observable catalog.

**Why P2**: Delivers the metadata that makes the catalogs useful, and dissolves
the opvar-name vs observable-name inconsistency: the atomic `@name` on the var is
the single naming source for both catalogs.

**Acceptance Criteria**:

1. WHEN a `var gm` carries `@unit("S")` and `@description("transconductance")`
   THEN its `QueryDescriptor` in `list_queries()` SHALL carry those unit and
   description (not empty metadata).
2. WHEN a `var i_d` carries `@name("i_d")` and `@kind("State")` THEN
   `list_observables()` SHALL return an `ObservableDescriptor` named `"i_d"` with
   kind `State` — NOT a positional `ddt[k]` name.
3. WHEN a `var` carries `@name` THEN both its opvar query entry AND its observable
   entry SHALL use that name — one declaration, two consistent catalogs (the
   inconsistency is structurally impossible).
4. WHEN a `var` carries none of these attributes THEN it keeps today's behavior
   (opvar named by var id; observable named positionally) — no regression.
5. WHEN `@kind("...")` on a var names a value outside `ObservableKind` THEN
   elaboration SHALL fail loud naming the offending value.

**Independent Test**: A device with `var gm @unit("S") @description(...)` and
`var i_d @name("i_d") @kind("State")`. Assert: `list_queries()` carries `gm`'s
unit/description; `list_observables()` returns `i_d` by name; a `ProbeSelection`
requesting `"i_d"` records it (and `"ddt[0]"` no longer names that slot).

---

### P2: `@name`/`@description`/`@kind` on terminals — author-classified terminals

**User Story**: As a device author, I mark a port or internal wire as
external/internal/auxiliary and name it in PHDL, so `list_terminals()` reports the
author-intended `TerminalKind` and name instead of position-inferred guesses.

**Why P2**: `TerminalKind` already exists on `TerminalDescriptor`
(`element-abi-maturity` T14), inferred from port-vs-wire position. Authors need
to override it (e.g. an auxiliary thermal port that is declared as a port but is
not an electrical external terminal). Reuses the same atomic `@kind`/`@name`
attributes — placement (a port/wire) selects `TerminalKind`.

**Acceptance Criteria**:

1. WHEN a port carries `@kind("auxiliary")` THEN its `TerminalDescriptor.kind` in
   `list_terminals()` SHALL be `TerminalKind::Auxiliary`.
2. WHEN an internal `wire` carries `@kind("internal")` and `@name("cp")` THEN its
   descriptor SHALL be `Internal` and named `"cp"` — author-declared and
   overridable.
3. WHEN a terminal carries no `@kind` THEN it keeps today's position-inferred
   kind (port → External, non-port wire → Internal) — no regression.
4. WHEN `@kind("...")` on a terminal names a value outside
   `{external, internal, auxiliary}` THEN elaboration SHALL fail loud.
5. WHEN `@kind("external")` is placed on an internal `wire` (contradicts
   position) THEN the author declaration wins (explicit over inferred) — legal
   and author-owned.

**Independent Test**: A device with an `@kind("auxiliary")` port and an
`@kind("internal") @name("cp")` wire. Assert `list_terminals()` reports the
declared kinds/name; a third un-annotated port keeps `External`.

---

### P3: Named limiters — `$limit` carries name + reason

**User Story**: As a MOSFET author using both `fetlim` and `limvds`, the
`LimitingReport` names which limiter actually fired, so a host diagnostic
distinguishes them instead of reading `"pnjlim"` for both.

**Why P3**: The report plumbing shipped in `element-abi-maturity` with a
hardcoded `"pnjlim"`/`VoltageStep`. `$limit(kind: String)` already carries the
distinguishing `kind` at the call site (`operators.phdl:61`); this story threads
it (plus an optional reason) through the JIT to the report. Nice-to-have because
limiting still *works* today — only the diagnostic label is wrong. It is an
operator argument, not an attribute — outside the atomic-attribute family.

**Acceptance Criteria**:

1. WHEN a `$limit(kind = "limvds", ...)` call clamps THEN the resulting
   `LimitingReport.limiter_name` SHALL be `"limvds"` — the call-site `kind`, not
   hardcoded `"pnjlim"`.
2. WHEN a `$limit` call carries an optional reason THEN `LimitingReport.reason`
   SHALL reflect it (mapped onto `LimitReason`); an omitted reason SHALL default
   to `VoltageStep` (today's value) — unannotated calls unchanged.
3. WHEN a device runs two different limiters in one Newton iteration THEN each
   active `LimitingReport` SHALL name its own limiter (no cross-contamination).
4. WHEN existing stdlib models (unchanged `$limit(kind)` sites) compile THEN
   their reports SHALL carry the correct `kind` with no behavioral change to the
   limiting math (goldens stay green).

**Independent Test**: A MOSFET DC sweep exercising both `fetlim` and `limvds`;
assert the reports carry `"fetlim"` and `"limvds"` respectively. The existing
MOS/diode goldens stay green (limiting math unchanged).

---

## Edge Cases

- WHEN an atomic attribute appears on a declaration kind it does not target (e.g.
  `@model` on a `var`, `@unit` on a module) THEN elaboration SHALL fail loud with
  a placement error.
- WHEN two `var`s declare the same `@name` THEN elaboration SHALL fail loud
  (duplicate introspection name) — the name is a key within its catalog.
- WHEN `@unit`/`@description` annotates a `var` that is not an operating-point var
  (shadowed/internal-only) THEN elaboration SHALL fail loud, not silently attach
  orphan metadata.
- WHEN a plugin (non-PHDL) device is introspected THEN it is unaffected — these
  attributes populate the PHDL→`PiperineDevice` path only; a plugin Element sets
  its own descriptors directly.
- WHEN `@kind` is placed on a declaration where neither `ObservableKind` nor
  `TerminalKind` applies THEN elaboration SHALL fail loud (no target enum).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| PIA-01 | P1 `@model`/`@version`: populate ModelDescriptor | Execute | ✅ Verified |
| PIA-02 | P1: module-name fallback when absent | Execute | ✅ Verified |
| PIA-03 | P1: fail loud on wrong-placement | Execute | ✅ Verified |
| PIA-04 | P1: textual extern declaration (MD-24) | Execute | ✅ Verified |
| PIA-05 | P2 `@unit`/`@description`: into QueryDescriptor | Execute | ✅ Verified |
| PIA-06 | P2 `@name`/`@kind`: named+kinded observable, not positional | Execute | ✅ Verified |
| PIA-07 | P2 `@name`: one name → both catalogs (inconsistency dissolved) | Execute | ✅ Verified |
| PIA-08 | P2 var attrs absent → today's default | Execute | ✅ Verified |
| PIA-09 | P2 `@kind` on var: fail loud on unknown ObservableKind | Execute | ✅ Verified |
| PIA-10 | P2 `@kind` on terminal → TerminalDescriptor.kind | Execute | ✅ Verified |
| PIA-11 | P2 `@kind`/`@name` on internal wire | Execute | ✅ Verified |
| PIA-12 | P2 terminal `@kind` absent → position-inferred | Execute | ✅ Verified |
| PIA-13 | P2 `@kind` on terminal: fail loud on unknown TerminalKind | Execute | ✅ Verified |
| PIA-14 | P2 `@kind` placement-resolution (var vs terminal enum) | Execute | ✅ Verified |
| PIA-15 | P3 `$limit`: kind → LimitingReport.limiter_name | Execute | ✅ Verified |
| PIA-16 | P3 `$limit`: optional reason → LimitReason (default VoltageStep) | Execute | ⚠️ Partial — default-half done via kind-inference (limvds→VdsStep, else VoltageStep); optional-reason-arg half deferred per design.md MVP (zero stdlib model needs a non-default reason today) |
| PIA-17 | P3 `$limit`: two limiters name themselves independently | Execute | ✅ Verified |
| PIA-18 | P3 `$limit`: unchanged stdlib sites, goldens green | Execute | ✅ Verified |
| PIA-19 | Edge: attribute on wrong declaration kind fails loud | Execute | ✅ Verified |
| PIA-20 | Edge: duplicate `@name` fails loud | Execute | ✅ Verified |

**ID format:** `PIA-[NUMBER]`

**Coverage:** 20 total — 19 verified, 1 partial (PIA-16 MVP deferral, design-approved).
Independent Verifier PASS: see `validation.md` (19/20 ACs spec-anchored, 3/3 mutations killed, gate 849 passed/0 failed).

---

## Success Criteria

- [ ] A device author sets model identity, var metadata (name/unit/description/
      kind), and terminal kinds from PHDL with small composable attributes — the
      introspection catalogs reflect the declarations, not codegen-derived
      defaults (PIA-01..14).
- [ ] The opvar-name vs observable-name inconsistency is structurally gone: one
      `@name` per var feeds both catalogs (PIA-07).
- [ ] `LimitingReport` names the limiter that actually fired (PIA-15..18).
- [ ] Every new attribute has a textual `extern attribute` declaration; LSP
      go-to-definition lands on it (PIA-04).
- [ ] Every attribute is optional — existing stdlib models compile unchanged
      (PIA-02, PIA-08, PIA-12, PIA-18).
- [ ] `cargo test --workspace` green; zero rustc warnings.
