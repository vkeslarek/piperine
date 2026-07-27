# P6 Cleanup — Architecture & Readability Validation (Phase 6: T31–T36, object-model lift)

**Date**: 2026-07-27
**Spec**: `.specs/features/p6-cleanup-architecture/spec.md`
**Diff range**: `7dc1bed` (T31) .. `3b98407` (T36); trailing `bc97ae2` is the batch log (docs only). Interleaved `docs(dv)` commits excluded.
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T31 (descriptors) | ✅ Done | 7 tests, `model/mod.rs` = 29 lines |
| T32 (Design) | ✅ Done | 11 tests incl. MD-25 proof |
| T33 (Module) | ✅ Done | 7 tests incl. staging isolation + live compile |
| T34 (InstanceView) | ✅ Done | one `InstanceView` in tree (`rg "pub struct InstanceView"` → `model/instance.rs:50` only); `results.rs:206` re-exports |
| T35 (python delegation) | ✅ Done | grep-verified (below) |
| T36 (Rust proof + parity) | ✅ Done | 3 root tests + parity guard; MD-31 failure proof reproduced by this verifier |

---

## Spec-Anchored Acceptance Criteria

### CLA-17 — the object model is api-canonical (lift)

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| AC1: api exposes design load + `top`/`module`/`modules`/`const_`/`select` | `top` infers unique root `"Top"`; `module("Nope")` errors `module `Nope` not found`; `const_("VDD")` = `Real(3.3)`; `select("/m1")` → 1 node kind `"instance"` | `crates/piperine-api/tests/model_design.rs:48` — `assert_eq!(top.name()..., "Top")`; `:83` — `assert_eq!(format!("{err}"), "module `Nope` not found")`; `:98` — `assert_eq!(design.const_("VDD"), Some(Value::Real(3.3)))`; `:108` — `assert_eq!(selection.nodes()[0].kind(), "instance")` | ✅ PASS |
| AC1: load fails loud (parse + I/O) | `Error::Model`; message names the unreadable path verbatim | `model_design.rs:60-63` — `matches!(err, Error::Model(_))`; `:71-74` — `msg.contains("failed to read `/nonexistent/piperine/model_design_fixture.phdl`")` | ✅ PASS |
| AC1: ambiguous root leaves top unset | `top().is_none()` for two candidate roots | `model_design.rs:54` — `assert!(design.top().is_none(), ...)` | ✅ PASS |
| AC1: module navigation (`name`/`ports`/`nets`/`instances`/`params`/`behaviors`) | exact authored declarations (port order `a,z,bias` + typed `Direction`; divider nets `gnd,mid,vin`; instance pairs `(label, module)`) | `crates/piperine-api/tests/model_descriptors.rs:47` — `assert_eq!(names, vec!["a","z","bias"])`; `:50-54` typed `Direction::Input/Output/Inout`; `crates/piperine-api/tests/model_module.rs:74-78` — `assert_eq!(pairs, vec![("r_bot","Resistor"),("r_top","Resistor"),("src","VoltageSource")])` | ✅ PASS |
| AC1: analysis menu + `compile` on api `Module` | divider `mid` = 2.0 V (op/tran), 10 AC points, 5 noise PSD samples; `compile()` → session restamps, `rebuilds() == 0` (MD-18) | `model_module.rs:87` — `(mid - 2.0).abs() < 1e-9`; `:98` tran last point; `:106` — `ac.axis().len() == 10`; `:114` — `noise.psd().len() == 5`; `:150` — `assert_eq!(session.rebuilds(), 0)` | ✅ PASS |
| AC1: `InstanceView` full surface | `terminal_connections` in port-declaration order `[("p","vin"),("n","mid")]`; `v(p)=5.0`, `v(p,n)=3.0`, `i(p,n)=1e-3`; `opvar("cond")=1/2e3`; catalogs reflect device | `crates/piperine-api/tests/model_instance.rs:78` — `assert_eq!(pairs, vec![("p","vin"),("n","mid")])`; `:88,:91,:94` scalars; `:103-104` opvar; `:107-123` model/terminals/observables/param | ✅ PASS |
| AC6 (MD-25): authored hierarchy, never `flat_modules` | model shows Top's **1** authored instance where the flat form splices **2**; descent into Mid yields `la`/`lb` | `model_design.rs:145-150` — `flat_top.instances().len() == 2` (discriminator); `:155` — `top_instances.len() == 1`; `:157` — `module() == "Mid"`; `:164` — `leaves == ["la","lb"]`. Structural: `rg "flat_module" crates/piperine-api/src/` → **empty** (no accessor can read the flat map) | ✅ PASS |
| Staging isolation (the `_Module` guarantee) | staged 2.5 V vs fresh view 2.0 V on the same design | `model_module.rs:125` vs `:133`; root twin `tests/host_object_model.rs:89` vs `:94` | ✅ PASS |

### CLA-18 — python model files are pure delegation

| Criterion | Spec-defined outcome | Evidence | Result |
|---|---|---|---|
| AC2: no POM traversal / `CircuitCompiler` in `design.rs`/`module.rs`/`instance.rs` | zero hits | `rg "piperine_lang::pom\|CircuitCompiler" crates/piperine-python/src/{design,module,instance}.rs` → **empty**; delegation shape confirmed at `design.rs:53-69` (forward + `PyValueError` map), `module.rs:92-130` | ✅ PASS |
| AC3 (D6): names/signatures/defaults/returns/messages byte-identical | python suite unchanged at 59; parity green | `cargo test -p piperine-python` → **59 passed / 0 failed**; `tests/host_parity.rs:274-287` green | ✅ PASS |
| AC4: parity enumerates the lifted model both sides | canonical lists; Rust compile-time + Python runtime `missing:`/`extra:` both directions | `tests/host_parity.rs:30-38` (lists), `:177-216` (`call_every_rust_model_method`), `:242-250` (both-direction probe), `:275` test | ✅ PASS |

### CLA-19 — Rust-side proof target

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| AC5: root target proves load → module → analysis → instance view → opvar → compile → live `set`, no Python | full path through root `piperine` crate; `mid` 2.0 → 2.5 V; `rebuilds()==0` | `tests/host_object_model.rs:48-79` — `:59` `(mid-2.0)<1e-9`, `:65` `cond≈1/2e3`, `:70` `v(p)≈2.0`, `:77` `(mid-2.5)<1e-9`, `:78` `assert_eq!(session.rebuilds(), 0)`; `//!` scope header at `:1-5` | ✅ PASS |
| T36 guard failure-proof (MD-31) | guard fails naming the drift | reproduced by this verifier: injecting `parity_probe_injected` into `MODEL_DESIGN_METHODS` failed with `the object-model surface drifted between hosts (MD-22 breach): missing:parity_probe_injected`; reverted | ✅ PASS |

**Status**: ✅ All Phase-6 ACs covered; no spec-precision gaps (spec defines precise outcomes and assertions match them).

---

## Discrimination Sensor

| Mutation | File:line | Description | Killed? |
|---|---|---|---|
| 1 | `crates/piperine-api/src/model/module.rs:122-124` | Dropped the staged-override replay loop in `session_with_disto` | ✅ Killed — `model_module.rs::staged_overrides_apply_without_mutating_the_parent_design` AND `host_object_model.rs::staged_overrides_stay_isolated_from_the_parent_design` both FAILED |
| 2 | `crates/piperine-api/src/model/design.rs:162-164` | `Design::select` returns `Ok` on an empty selection (fail-loud removed) | ✅ Killed — `model_design.rs::select_fails_loud_on_a_path_that_matches_nothing` FAILED |
| 3 | `crates/piperine-api/src/model/instance.rs:159` | `terminal_connections` reversed (declaration order dropped) | ✅ Killed — `model_instance.rs::terminal_connections_map_ports_to_parent_nets_in_declaration_order` FAILED |
| 4 | `tests/host_parity.rs:30` (scratch) | Bogus name `parity_probe_injected` added to the canonical list | ✅ Killed — guard failed naming `missing:parity_probe_injected` (reproduces the author's MD-31 proof verbatim) |

**Sensor depth**: lightweight (4 targeted mutations — 3 behavior + 1 guard)
**Result**: 4/4 killed — PASS ✅. All mutations applied to the working tree, reverted via `git checkout --`; `git status` clean after each.

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code / surgical changes | ✅ (diff surface confined to `piperine-api` model + tests, python delegation, root proof targets) |
| No file-scope `#![allow` | ✅ `rg "#!\[allow"` over all six commits' surfaces → empty |
| `model/mod.rs` ≤ 60 lines | ✅ 29 lines, declarations + re-exports + `//!` only |
| No `unwrap()`/`expect()` on user-input paths in model src | ✅ `rg "unwrap()\|expect(" crates/piperine-api/src/model/` → empty |
| Fail-loud (no silent 0.0/None) | ✅ select/opvar/param/label/port misses all typed `Error::Model`/`NotFound`/`Measurement` with names in messages; mutant 2 confirms the guard is load-bearing |
| Single `InstanceView` owner | ✅ one definition; `results.rs:206` re-export keeps call sites compiling |
| Tests map to ACs, non-shallow | ✅ every done-when criterion has ≥1 assertion at the spec-defined value |

---

## Edge Cases

- [x] MD-25 edge: "lift fails loud rather than surface the flat artifact" — no accessor exists; authored-vs-flat discriminator tested (`model_design.rs:139-168`)
- [x] Trace-bound view op-side accessors fail loud (`model_instance.rs:142-143`)
- [x] Introspection-only view readouts fail loud (`model_instance.rs:156-159`)
- [x] Unknown label/port fail loud with the name in the message (`model_instance.rs:163-174`)
- [x] Malformed selector vs zero-match are distinct error variants (`model_design.rs:124-131`)

---

## Gate Check

- **Gate command**: `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` (+ `-p piperine-python`, + `cargo clippy --workspace --all-targets -- -D warnings`)
- **Result**: **1204 passed, 0 failed, 4 ignored** (201 test-result lines aggregated); python **59/0/0**; clippy **clean** (exit 0)
- **Test count before batch**: 1169 → **after: 1204**, delta **+35** = +7 descriptors +11 design +7 module +6 instance +3 object-model +1 parity (matches the batch log itemization; no test dropped)
- **Skipped**: 4 ignored (3 pre-existing rustdoc ignores + 1 plugin doc ignore — pre-existing, not this batch)
- **Failures**: none

---

## Summary

**Overall**: ✅ Ready

**Spec-anchored check**: CLA-17/18/19 — all criteria matched to spec-defined outcomes with `file:line` evidence
**Sensor**: 4/4 mutations killed (incl. reproduced MD-31 guard proof)
**Gate**: 1204 passed / 0 failed / 4 ignored; python 59/0; clippy clean

**What works**: the full lifted object model on both hosts; authored-hierarchy navigation (MD-25 structurally enforced); staging isolation; compile-once through `Module::compile`; byte-stable Python surface (D6).

**Issues found**: none in scope. (The `OpResult::i` var-bearing-kernel limitation is pre-existing, documented in the fixture and the batch log, and recorded as a ROADMAP item — not a Phase-6 regression.)

**Next steps**: Phase 7 (T37–T41) may proceed.
