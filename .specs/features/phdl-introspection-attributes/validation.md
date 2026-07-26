# phdl-introspection-attributes Validation

**Date**: 2026-07-23
**Spec**: `.specs/features/phdl-introspection-attributes/spec.md`
**Diff range**: `f34479a..388f7a7` (8 commits, feature-only; HEAD `b7109f8` excluded per scope)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status     | Notes |
| ---- | ---------- | ----- |
| T1 — Textual introspection attribute schemas + prelude wiring | ✅ Done | `headers/introspection.phdl` + `resolve.rs:136-147` |
| T2 — `IntrospectionMeta` sidecar + `Design::introspection_meta` resolver | ✅ Done | `pom/introspection.rs`, `pom/design.rs:403-538` |
| T3 — Plumb sidecar → `PiperineDevice` + `model_descriptor` reads it | ✅ Done | `device/mod.rs:122-137,565-583`, `circuit.rs:83,157-167`, `builder.rs:261,359` |
| T4 — `list_queries`/`read_opvars` read `@name`/`@unit`/`@description` | ✅ Done | `device/mod.rs:165-171,406-440` |
| T5 — `list_observables` reads `@name`/`@kind` (one name, both catalogs) | ✅ Done | `device/mod.rs:637-658` |
| T6 — `list_terminals` reads `@name`/`@kind` on ports + wires | ✅ Done | `device/mod.rs:503-559` |
| T7 — Kernel `Limits.catalog` + `limit_catalog()` accessor | ✅ Done | `kernel/analog/limits.rs:24-52`, `kernel/analog/compile.rs:486-503,676`, `kernel/analog/mod.rs:437-439` |
| T8 — Per-slot active tracking + `limiting_report` reads catalog | ✅ Done | `device/analog/limits.rs:13-67,131-169`, `device/analog/mod.rs:400-428` |

All 8 tasks delivered, each in its own commit with the documented PIA-mapped commit message.

---

## Spec-Anchored Acceptance Criteria

ACs use keyed attribute syntax (`@name(value = "i_d")`) per the recorded SPEC_DEVIATION (tasks.md:14-22, user-approved 2026-07-23) — the spec's positional text (`@name("i_d")`) is read as its keyed equivalent. This deviation is approved, not a gap.

| Criterion (WHEN X THEN Y) | Spec-defined outcome | `file:line` + assertion expression | Result |
| ------------------------- | -------------------- | ---------------------------------- | ------ |
| **PIA-01**: WHEN `@model(type, version)` on a module THEN `model_descriptor()` returns `{type_id, version}` from the attribute | `{ type_id: "mos", version: "3" }` | `crates/piperine-codegen/tests/model_descriptor.rs:58-79` — `assert_eq!(descriptor.type_id, "mos")` + `assert_eq!(descriptor.version, "3")` + `assert_ne!(descriptor.type_id, "Mos")`. POM side: `introspection_meta.rs:41-51`. | ✅ PASS |
| **PIA-02**: WHEN no `@model` THEN module-name echo, empty version (no regression) | `type_id == "RCap"`, `version == ""` | `crates/piperine-codegen/tests/model_descriptor.rs:32-51` — `assert_eq!(descriptor.type_id, "RCap")` + `assert_eq!(descriptor.version, "")`. POM side: `introspection_meta.rs:53-65`. | ✅ PASS |
| **PIA-03**: WHEN `@model` misplaced (e.g. on a var) THEN elaboration fails loud | placement error naming `model` + `var` | `crates/piperine-lang/tests/introspection_meta.rs:186-200` — `assert!(err.contains("model"))` + `assert!(err.contains("var"))`. Codegen propagation: `model_descriptor.rs:84-107`. | ✅ PASS |
| **PIA-04**: WHEN ctrl+click `@model` THEN resolves to textual `extern attribute` declaration (MD-24) | textual declaration with `decl_span` registered in prelude | `crates/piperine-lang/tests/introspection_attrs.rs:26-34,38-49` — `elaborate(src).expect("@model must elaborate via the prelude schema")` (use site elaborates with no per-project declaration; `decl_span` is threaded by `register_declared` per header comment). Unknown-field rejection: `introspection_attrs.rs:54-65`. | ✅ PASS (textual schema + registration verified; LSP go-to-def execution itself not directly tested in this PR — relies on the shared `SymbolKind::AttrSchema` LSP arm) |
| **PIA-05**: WHEN `var gm` carries `@unit`/`@description` THEN `QueryDescriptor` carries them | `unit == "S"`, `description == "transconductance"` | `crates/piperine-codegen/tests/opvar_bridge.rs:182-208` — `assert_eq!(q.unit.as_deref(), Some("S"))` + `assert_eq!(q.description.as_deref(), Some("transconductance"))`. POM side: `introspection_meta.rs:69-85`. | ✅ PASS |
| **PIA-06**: WHEN `var i_d` carries `@name`/`@kind` THEN observable named `"i_d"`, kind `State`, NOT positional `ddt[k]` | `name == "i_d"`, `kind == State`, no `var[k]` | `crates/piperine-codegen/tests/observable_catalog.rs:188-217` — `assert_eq!(id_obs.kind, ObservableKind::State)` + `assert!(!observables.iter().any(\|o\| o.name.starts_with("var[")))`. | ✅ PASS |
| **PIA-07**: WHEN `var` carries `@name` THEN both query catalog AND observable catalog use that name (inconsistency dissolved) | `gm` in BOTH catalogs; kernel id `g` in NEITHER | `crates/piperine-codegen/tests/observable_catalog.rs:223-253` — `assert!(query_has_gm && observable_has_gm)` + `assert!(...all \|q\| q.name != "g")` + `assert!(...all \|o\| o.name != "g")`. Opvar-side label remap: `opvar_bridge.rs:215-251`. | ✅ PASS |
| **PIA-08**: WHEN `var` has no attrs THEN today's default (opvar by var id; observable positional) | `name == "g"`, no unit/description | `crates/piperine-codegen/tests/opvar_bridge.rs:256-276` — `assert!(q.unit.is_none() && q.description.is_none())`. Observable positional fallback: `observable_catalog.rs:52-82` (`device_with_vars_declares_var_observables`). POM side: `introspection_meta.rs:87-99`. | ✅ PASS |
| **PIA-09**: WHEN `@kind` on a var names non-`ObservableKind` THEN elaboration fails loud | error naming offending value | `crates/piperine-lang/tests/introspection_meta.rs:143-167` — `assert!(err.contains("auxiliary"))` (terminal value on var) + `assert!(err.contains("Bogus"))` (unknown value). | ✅ PASS |
| **PIA-10**: WHEN a port carries `@kind("auxiliary")` THEN `TerminalDescriptor.kind == Auxiliary` | `kind == Auxiliary` for `t`, `External` for un-annotated `s` | `crates/piperine-codegen/tests/terminal_bridge.rs:282-312` — `assert_eq!(by_name.get("t"), Some(&(TerminalKind::Auxiliary, Domain::Analog)))` + un-annotated `s` stays `External`. | ✅ PASS |
| **PIA-11**: WHEN an internal `wire` carries `@kind("internal")` + `@name("cp")` THEN descriptor is `Internal` named `"cp"` | `name == "cp"`, `kind == Internal`; source id `mid` does NOT surface | `crates/piperine-codegen/tests/terminal_bridge.rs:317-345` — `assert!(...any \|n,k,_\| n == "cp" && *k == TerminalKind::Internal)` + `assert!(!...any \|n,_,_\| n == "mid")`. POM side: `introspection_meta.rs:275-288`. | ✅ PASS |
| **PIA-12**: WHEN a terminal has no `@kind` THEN position-inferred kind (port→External, wire→Internal) | `port == External`, `wire == Internal` | `crates/piperine-codegen/tests/terminal_bridge.rs:43-84` (`analog_kernel_terminals_bridge_names_and_kinds`) + `:93-140` (BJT three external + three internal). The un-annotated `s` port in `:307-311` also confirms. | ✅ PASS |
| **PIA-13**: WHEN `@kind` on a terminal names non-`TerminalKind` THEN elaboration fails loud | error naming offending value | `crates/piperine-lang/tests/introspection_meta.rs:171-182` — `assert!(err.contains("state"))` (var value on terminal). | ✅ PASS |
| **PIA-14**: WHEN `@kind` is placed (var vs terminal) THEN target enum selected by placement | var→ObservableKind, terminal→TerminalKind (validated at resolve) | Indirect via PIA-09 + PIA-13: a `var`-kind value (`"state"`) on a terminal fails (`introspection_meta.rs:171-182`); a terminal-kind value (`"auxiliary"`) on a var fails (`introspection_meta.rs:143-155`). The placement matrix is enforced uniformly. | ✅ PASS |
| **PIA-15**: WHEN `$limit(kind = "limvds")` clamps THEN `limiter_name == "limvds"`, NOT `"pnjlim"` | `limiter_name == "limvds"`, `reason == VdsStep` | Kernel catalog: `crates/piperine-codegen/tests/limiters.rs:155-180` — `has_fetlim` + `has_limvds` + `assert!(...all \|n,_\| *n != "pnjlim")`. Device report: `checkpoint_limiter.rs:172-205` — `assert_eq!(report.limiter_name, "limvds")` + `assert_eq!(report.reason, LimitReason::VdsStep)`. Two-limiter report: `limiters.rs:206-254`. | ✅ PASS |
| **PIA-16**: WHEN `$limit` carries optional reason THEN reason mapped onto `LimitReason`; omitted defaults to `VoltageStep` | omitted reason → `VoltageStep` (today's behavior) | ⚠️ **Partial coverage.** Default-to-VoltageStep: `limiters.rs:186-200` — `assert_eq!(catalog[0].1, LimitReason::VoltageStep)`. **However**, the "carries an optional reason" half is NOT IMPLEMENTED: the design chose the documented MVP (infer reason from `kind`, no `$limit` reason arg added — design.md:186-190, tasks.md T7). The grammar carries no `reason` argument today. | ⚠️ Spec-precision gap (design-approved MVP deferral) |
| **PIA-17**: WHEN two limiters fire in one iteration THEN each names itself independently (no cross-contamination) | each report carries its own limiter name | Catalog: `limiters.rs:155-180` proves `[fetlim/VoltageStep, limvds/VdsStep]` recorded separately in slot order. Device: `limiters.rs:206-254` asserts `report.limiter_name == "fetlim" \|\| "limvds"` (whichever clamped) + `report.limiter_name != "pnjlim"`. | ✅ PASS (catalog-level; the device-level assertion is conditional on a slot clamping in the test drive, which the comment acknowledges) |
| **PIA-18**: WHEN unchanged stdlib `$limit` sites compile THEN reports correct, goldens green | parity goldens still pass; `$limit` signature unchanged | `crates/piperine-solver/tests/parity_baseline.rs::parity_diode_dc_point` (full workspace gate green); the `fetlim_matches_ngspice_reference` + `limvds_matches_ngspice_reference` golden comparisons (`limiters.rs:96-145`) still pass value-for-value. | ✅ PASS |
| **PIA-19**: WHEN an atomic attribute is misplaced (e.g. `@unit` on a module) THEN elaboration fails loud | placement error naming schema + node kind | `crates/piperine-lang/tests/introspection_meta.rs:202-228` — `at_unit_on_module_fails_loud` (`err.contains("unit") && err.contains("module")`) + `at_unit_on_port_fails_loud` + `any_introspection_attr_on_param_fails_loud` (loops all 5 schemas). | ✅ PASS |
| **PIA-20**: WHEN two `var`s declare the same `@name` THEN elaboration fails loud | error flagging the duplicate + colliding value | `crates/piperine-lang/tests/introspection_meta.rs:256-270` — `assert!(err.contains("duplicate"))` + `assert!(err.contains("\"i\"") \|\| err.contains('\`'))`. Implementation: `pom/design.rs:490-497`. | ✅ PASS |

**Status**: ⚠️ 19/20 ACs cleanly PASS + 1 spec-precision gap (PIA-16 optional-reason half — design-approved MVP deferral, documented in design.md:186-190).

---

## Discrimination Sensor

| # | Mutation | File:line | Description | Killed? |
| - | -------- | --------- | ----------- | ------- |
| M1 (PIA-01) | `crates/piperine-codegen/src/device/mod.rs:565-583` | Replaced the `if let Some(model) = &self.meta.model { return ... }` block with `let _ = &self.meta.model;` (always falls through to module-name echo). Ran `model_descriptor_reads_at_model_attribute`. | ✅ Killed — `left: "Mos", right: "mos"` |
| M2 (PIA-15) | `crates/piperine-codegen/src/device/analog/mod.rs:419-420` | Replaced `let (limiter_name, reason) = catalog[slot];` with hardcoded `let limiter_name = "pnjlim"; let reason = LimitReason::VoltageStep;`. Ran `piperine_device_produces_limiting_report_when_clamping`. | ✅ Killed — `left: "pnjlim", right: "limvds"` |
| M3 (PIA-06/07) | `crates/piperine-codegen/src/device/mod.rs:643-652` | Replaced the sidecar-lookup chain with `let (name, kind) = (format!("var[{k}]"), ObservableKind::Var);` (always positional, ignores `@name`/`@kind`). Ran `observable_named_and_kinded_by_at_name_and_at_kind`. | ✅ Killed — `panicked ... observable named \`i_d\` via @name` |

**Sensor depth**: lightweight (3 behavior-level mutations on the highest-risk new code — model-identity bridge, limiter-naming bridge, observable-naming bridge)
**Result**: 3/3 killed — PASS ✅

All mutations ran in a scratch state (file backup + restore via `cp` to/from `/tmp/opencode`). The real working tree was verified clean (`git status --short` shows no `crates/` modifications) and all three targeted tests pass on the restored tree.

---

## Code Quality

Per `AGENTS.md` (binding MD-13 Rust idiom rules + fail-loud rule + no-unwrap-on-user-paths).

| Principle | Status | Notes |
| --------- | ------ | ----- |
| Minimum code — no features beyond what was asked | ✅ | No unrequested additions. The `reason` arg (PIA-16) is correctly deferred, not half-built. |
| Surgical changes — only touched files required for the task | ✅ | 23 files in the diff surface, all directly accounted for by T1-T8. The two non-test edits in `codegen_api.rs`/`temperature_dnf.rs` (4 lines each) are mechanical updates from the `PiperineDevice::new` signature change. |
| No abstractions for single-use code | ✅ | `ModelId`/`VarMeta`/`TermMeta` are all multi-consumer (POM resolver + codegen bridge + serde round-trip). `catalog_entry_for_kind` is a single helper on `Limits` (owner-bound per MD-13 rule 2). |
| No unnecessary "flexibility" added | ✅ | The `@kind` enum mapping uses plain `match` arms (no macros per MD-13 rule 5). The sidecar is sparse — empty entries are dropped (`has_any` gate), not eagerly allocated. |
| Didn't "improve" unrelated code | ✅ | The `chore:` commit `b7109f8` at HEAD is by the user (build-speed/docs), explicitly excluded from this review scope. |
| Matches existing patterns/style | ✅ | The resolver (`Design::introspection_meta`) mirrors `Design::rfports` exactly (`pom/design.rs:330-380`): same iteration shape, same `field_err`/`place_err` split, same `AttrSchemaField` fail-loud path. The `Limits.catalog` field parallels `Limits.branches`. |
| Spec-anchored outcome check (asserted values match spec) | ✅ | Every test asserts the spec-defined value precisely (`"mos"`/`"3"`, `"limvds"`/`VdsStep`, `TerminalKind::Auxiliary`, etc.) — not just "an assertion exists". One nuance: PIA-16 — the implementation infers `VdsStep` for `limvds`, while the spec AC literally says "default VoltageStep" universally. The design (design.md:225) approved this as the inferred-reason behavior; the literal AC wording is slightly looser than the implementation, but the implementation matches the design and is behaviorally correct. |
| Per-layer coverage expectation met | ✅ | Domain layer (lang POM resolver) has 1:1 AC mapping; integration layer (codegen bridge) covers every spec-defined outcome end-to-end through the `parse_and_elaborate → lower_bodies → CircuitCompiler → all_devices()[0]` harness; fail-loud paths covered at both layers. |
| Every test in scope maps to a spec AC/edge/Done-when (no unclaimed tests) | ✅ | Each new test names its PIA in a header comment and asserts the spec-defined outcome. The two pre-existing tests touched by T8 (`fetlim_matches_ngspice_reference`, `limvds_matches_ngspice_reference`) remain golden-value comparisons unrelated to this feature — they are scoped-out regression guards, not unclaimed new tests. |
| Documented guidelines followed: `AGENTS.md` | ✅ | Fail-loud rule honored (`CodegenError::Invalid` at `circuit.rs:163`; `ElabErrorKind::AttrSchemaField`/`Other` at `design.rs:415-426`). No `unwrap()`/`expect()` on user-input paths in the new code. MD-13 rule 2 (no loose functions) — every helper is a struct/trait method. MD-13 rule 4 — files named after system function (`introspection.rs`, `limits.rs`). MD-13 rule 5 — no `macro_rules!`/proc-macros. |

---

## Edge Cases

- [x] **Wrong-placement** (`@model` on a var, `@unit` on a module): Handled — `introspection_meta.rs:186-228` covers `@model` on var, `@unit` on module/port, all five schemas on param.
- [x] **Duplicate `@name` on vars**: Handled — `introspection_meta.rs:256-270` (`duplicate_at_name_on_vars_fails_loud`); implementation at `pom/design.rs:490-497`.
- [ ] **`@unit`/`@description` on a non-opvar (shadowed/internal-only) var**: **NOT HANDLED.** A scratch probe (`var shadowed : Real = 0.0;` with `@unit(value = "S")` and no analog-body assignment) showed the resolver SILENTLY ACCEPTS the metadata (`vars={"shadowed": VarMeta { unit: Some("S"), .. }}`). At codegen, the kernel's `opvar_names()` does not include the shadowed var, so `list_queries` never reads the entry — orphan metadata is silently dropped, exactly what the spec Edge Case forbids. Spec wording: "elaboration SHALL fail loud, not silently attach orphan metadata." See **Ranked Gap #1**.
- [x] **Plugin (non-PHDL) device unaffected**: Structurally guaranteed — plugins do not flow through `PiperineDevice::new` (`builder.rs:257,355` are the only constructors, both PHDL-only). `crates/piperine-plugin` has zero references to `IntrospectionMeta`/`introspection_meta`. A plugin `Element` sets its own descriptors directly. No explicit test, but the isolation is structural.
- [x] **`@kind` where neither enum applies** (e.g. on a module or param): Handled via the placement matrix — `@kind` on module is rejected at `design.rs:449`; `@kind` on param is rejected at `design.rs:459-461`. Covered by `any_introspection_attr_on_param_fails_loud` (loops `@kind` among the five schemas on a param); module-level `@kind` rejection shares the path of `at_unit_on_module_fails_loud` (uniform placement matrix — same code arm).

---

## Gate Check

- **Gate command**: `cargo build --workspace && cargo test --workspace` (per `AGENTS.md` build/verify bar + tasks.md Gate Check Commands "Build" tier)
- **Result**: **849 passed, 0 failed, 5 ignored** (all 5 ignored are pre-existing doctests/fixture entries annotated with `ignored` + reason — none skipped by this feature)
- **Test count before feature**: 818 (baseline supplied)
- **Test count after feature**: 849
- **Delta**: **+31 new tests** (positive — no silent test deletion)
  - `introspection_attrs.rs`: +4 (T1)
  - `introspection_meta.rs`: +18 (T2)
  - `model_descriptor.rs`: +2 (T3 — `model_descriptor_reads_at_model_attribute`, `at_model_on_param_fails_loud_at_build`)
  - `opvar_bridge.rs`: +3 (T4 — `query_descriptor_carries_at_unit_and_description`, `read_opvars_uses_at_name_label`, `query_descriptor_absent_attrs_keeps_default`)
  - `observable_catalog.rs`: +2 (T5 — `observable_named_and_kinded_by_at_name_and_at_kind`, `at_name_feeds_both_query_and_observable_catalogs`)
  - `terminal_bridge.rs`: +3 (T6 — `port_at_kind_auxiliary_classifies_terminal`, `internal_wire_at_kind_internal_named_cp`, `at_kind_external_on_wire_wins_over_inferred`)
  - `limiters.rs`: +3 (T7+T8 — `limit_catalog_records_kind_per_slot`, `limit_catalog_pnjlim_infers_voltage_step`, `limiting_report_names_the_actual_limiter_not_hardcoded_pnjlim`)
- **Skipped tests**: 5 (pre-existing doctests in `piperine_plugin`, `piperine_plugin_wasm`, `piperine_solver` — each annotated `ignored` with a reason; none introduced by this feature)
- **Failures**: none
- **rustc warnings**: zero (`cargo build --workspace` emits only the unrelated `piperine-cli` build-script note about `piperine-python .so` — not a rustc warning)

**Test-integrity check on `checkpoint_limiter.rs` assertion change**: The updated assertion at `checkpoint_limiter.rs:190` (`assert_eq!(report.limiter_name, "limvds")` + `:192` `assert_eq!(report.reason, LimitReason::VdsStep)`) replaces a previously generic `"pnjlim"`/`VoltageStep` assertion. The PHDL device under test was also updated to `$limit("limvds", ...)` (line 27). The new assertion is **strictly MORE specific**: it pins both the limiter name AND the inferred reason (VdsStep, not the default VoltageStep), proving the catalog threading end-to-end. This is the intentional PIA-15 strengthening noted in the validation brief — not a weakening.

---

## Fix Plans

### Fix 1: Reject `@unit`/`@description` on a non-opvar (shadowed/internal-only) var

- **AC/Edge**: Spec "Edge Cases" bullet 3 (the `@unit`/`@description` on non-opvar case); not in the PIA-NN traceability table but is a documented spec requirement.
- **Root cause**: `Design::introspection_meta` (`pom/design.rs:469-500`) accepts `@unit`/`@description` on any POM `Var` without checking whether the var will surface as an opvar. The lang layer cannot know this alone (opvar-ness is decided by codegen's `lower_bodies`, which omits shadowed vars from `module_var_temps`). Today the metadata silently attaches and is then silently dropped at `list_queries` (`device/mod.rs:428-440`), violating the fail-loud rule.
- **Fix task**: Either (a) move/duplicate the var-opvar check to the codegen boundary (when `CircuitCompiler::introspection_meta` builds the sidecar, cross-check `meta.vars` keys against the compiled `LoweredBody`'s opvar names and fail loud on orphans), or (b) extend the lang resolver to consult a "is this var referenced in the analog body" predicate. Approach (a) is the lower-risk, design-conforming option — it matches how `@rfport` validation surfaces at the consumer boundary.
- **Priority**: Minor (the metadata is dropped, never mis-rendered; no correctness or solver-behavior impact; only the fail-loud guarantee is weakened for this narrow case).

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
| ----------- | --------------- | ---------- |
| PIA-01 | Implementing | ✅ Verified |
| PIA-02 | Implementing | ✅ Verified |
| PIA-03 | Implementing | ✅ Verified |
| PIA-04 | Implementing | ✅ Verified (textual schema + prelude registration; LSP go-to-def relies on shared arm) |
| PIA-05 | Implementing | ✅ Verified |
| PIA-06 | Implementing | ✅ Verified |
| PIA-07 | Implementing | ✅ Verified |
| PIA-08 | Implementing | ✅ Verified |
| PIA-09 | Implementing | ✅ Verified |
| PIA-10 | Implementing | ✅ Verified |
| PIA-11 | Implementing | ✅ Verified |
| PIA-12 | Implementing | ✅ Verified |
| PIA-13 | Implementing | ✅ Verified |
| PIA-14 | Implementing | ✅ Verified |
| PIA-15 | Implementing | ✅ Verified |
| PIA-16 | Implementing | ⚠️ Partial — default-to-VoltageStep half verified; optional-reason half is a design-approved MVP deferral (design.md:186-190) |
| PIA-17 | Implementing | ✅ Verified |
| PIA-18 | Implementing | ✅ Verified |
| PIA-19 | Implementing | ✅ Verified |
| PIA-20 | Implementing | ✅ Verified |

---

## Summary

**Overall**: ✅ Ready (with one Minor fix task logged for an edge-case fail-loud gap unrelated to any PIA-traceability AC)

**Spec-anchored check**: 19/20 ACs match spec outcome; 1 spec-precision gap flagged (PIA-16 optional-reason half — design-approved MVP deferral, not a behavioral bug).
**Sensor**: 3/3 mutations killed.
**Gate**: 849 passed, 0 failed (+31 over baseline 818).

**What works**:
- The full P1/P2/P3 vertical slice: parse textual `extern attribute` schemas → POM `IntrospectionMeta` sidecar → `CircuitCompiler` plumbing → `Introspect` bridge prefers declared over derived. Every catalog (`model_descriptor`, `list_queries`/`read_opvars`, `list_observables`, `list_terminals`, `limiting_report`) honors its declarative source and falls back cleanly.
- The "one `@name`, both catalogs" structural invariant (PIA-07) is real — `var_display_name` and the observable-name lookup read the same `meta.vars[kernel_name].name`. The opvar-vs-observable naming inconsistency is genuinely gone at the source.
- Limiter naming (PIA-15/17) is correct end-to-end: the per-slot catalog is collected at emit from the `$limit` call-site `kind`, and `rebuild_limit_report` reads it by clamping slot. Hardcoded `"pnjlim"` is gone.
- Three discrimination mutations on the highest-risk bridges were all killed — the tests genuinely detect regressions, not just shape-match the implementation.
- Zero rustc warnings; zero test regressions; the `checkpoint_limiter.rs` assertion update is strictly more specific than what it replaced.

**Issues found**:
1. **Edge-case gap (Minor)**: `@unit`/`@description` on a shadowed/non-opvar var is silently accepted by the lang resolver and silently dropped by codegen. Spec Edge Case bullet 3 requires fail-loud. See Fix Plan #1 — codegen-boundary cross-check is the lower-risk fix.
2. **PIA-16 partial (design-approved deferral, not a bug)**: The optional `$limit` reason argument is not implemented; the design (design.md:186-190) records this as the MVP, with inference-from-kind satisfying the default-to-VoltageStep half. The spec AC's "carries an optional reason" half remains unimplemented. Documented here for traceability; no behavioral regression.

**Next steps**:
- Optional: implement Fix #1 (codegen-boundary orphan-metadata check) to close the spec Edge Case. Low risk; one new test in `introspection_meta.rs` or `opvar_bridge.rs`.
- Optional: if a future stdlib model needs a non-default reason for a junction limiter, add the `$limit` trailing `reason` arg (PIA-16 second half) — design.md:186-190 already scopes this as an additive follow-up.
