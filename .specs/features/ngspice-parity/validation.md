# ngspice-parity Validation

**Date**: 2026-08-06
**Spec**: `.specs/features/ngspice-parity/spec.md`
**Diff range**: `bdf279c..HEAD` (`8a1c08c`..`29f00d9` — T16, T17, T19, T20, T21, T22, T23, T24, Fix A, Fix B, traceability sync)
**Verifier**: independent sub-agent (author ≠ verifier; re-derived from code+tests, not from the spec traceability table)
**Reference oracle**: ngspice-46 on PATH (`/usr/bin/ngspice`)

---

## Task Completion

| Task | Commit | Status | Notes |
| ---- | ------ | ------ | ----- |
| T16 (cap+diode UIC)        | `24f3ebd` | ✅ Done | cap+diode `@initial` wired; `ind` left as documented deviation (current-seed mechanism) |
| T17 (bjt+jfet+mos1/2/3 UIC)| `f5b9838` | ✅ Done | all five models wired with potential-seed `@initial` blocks |
| T18 (CKTsrcFact vsrc+isrc) | `bdf279c` (boundary) | ✅ Done | measured: both already honoured; falsifiable proof shipped as `homotopy_bias`/`homotopy_sink` |
| T19 (urc geometric taper)  | `6a0cbe5` | ✅ Done | K-taper landed; `urc_taper` AC fixture falsifies a uniform revert |
| T20 (urc ISPERL shunt)     | `cd3a4c0` | ✅ Done | diode-mode shunt landed; `urc_isperl` DC fixture falsifies a shunt drop |
| T21 (gap-3 array-net exp)  | `8a1c08c` | ✅ Done | `FlattenHierarchy::remap` expands const-indexed NetRef; 7 inline tests cover happy + 4 loud-fail paths |
| T22 (urc[N] generic)       | `a4246f8`, `94abc06` | ✅ Done | hand-unrolled `urc2`/`urc5`/`urc10` deleted; `.phdl` twins use `urc[N]`; `.cir` twins unchanged |
| T23 (urc FMAX → N)         | `4ca1c01` | ✅ Done | `URC_DEFAULT_N` const pre-computes ngspice's N-selection; `urc_fmax` fixture passes |
| T24 (ROADMAP rewrite)      | `30bca6e` | ⚠️ Partial | ROADMAP rewritten from measured status, BUT the urc row (line 94) drifted stale after Fix A/B + `94abc06` landed after T24 — see Code Quality |
| Fix A (array-wire nodes)   | `ba79501` | ✅ Done | `structure.rs` registers N element nodes per array wire; 2 inline tests |
| Fix B (structural-connection unify) | `8a8114b` | ✅ Done | `builder.rs` builds alias forest; 2 inline tests |
| Traceability sync          | `29f00d9` | ✅ Done | spec.md NGP-10..22 statuses all reflect measured state |
| Fix A (ind header comment) | — | ❌ Gap | `passives.phdl:255-257` ind deviation reason is stale — see Code Quality |
| Fix B (ROADMAP urc row)    | — | ❌ Gap | `ROADMAP.md:94` says NGP-18/19/22 are pending; they're closed — see Code Quality |

---

## Spec-Anchored Acceptance Criteria

The diff range covers NGP-10 (part), NGP-11 (boundary), NGP-15, NGP-16, NGP-17, NGP-18, NGP-19, NGP-20, NGP-21, NGP-22.
NGP-01..09, NGP-12..14 were verified by the pre-existing oracle (T1–T15, committed at or before `bdf279c`); the gate confirms they still pass.

### P1 (oracle — verified pre-diff-range, gate confirms still green)

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
| --------- | -------------------- | ----------------------- | ------ |
| NGP-01 (AC mag+phase per point) | per-frequency mag AND phase; phase gated on mag floor | `tests/ngspice_ac.rs:38-78` (AcCase::run) + `tests/support/ngspice.rs:154` `ToleranceContract::holds` | ✅ PASS |
| NGP-02 (noise PSD + integrated) | per-point PSD AND integrated RMS; V/√Hz → V²/Hz reconciled in one site | `tests/support/ngspice.rs:85-92` (NOISE_PSD rationale records the squaring) | ✅ PASS |
| NGP-03 (tran common grid) | compare on ngspice's print times via interpolation, never raw adaptive samples | `tests/support/ngspice.rs:108-113` (TRAN rationale) | ✅ PASS |
| NGP-04 (temp-sweep OP) | at least two temperatures, one below and one above nominal | `tests/ngspice_temp.rs` (existing, pre-diff-range) | ✅ PASS |
| NGP-05 (loud tolerance failure) | names model, domain, coordinate, both values, Δ, contract rationale | `tests/support/ngspice.rs:412-419` (compare_op failure text) | ✅ PASS |
| NGP-06 (ngspice-absent SKIP) | existing SKIP seam, suite passes | `tests/support/ngspice.rs:287-289` `NgspiceHarness::detect` | ✅ PASS |
| NGP-07 (documented tolerance contract) | seven domains, each with rationale; no literal in a test body | `tests/support/ngspice.rs:27-163` (ToleranceContract data table) | ✅ PASS |

### P2 (model fixes — T16/T17 in scope; T11-T15 verified pre-diff-range, gate confirms still green)

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
| --------- | -------------------- | ----------------------- | ------ |
| NGP-08 (analytic gm in channel noise) | analytic `gm` (not `gm_est`) in linear+transition regions; fixture sweeps linear region | mos: `crates/piperine-lang/headers/spice/mos.phdl` (sites :471/:1209/:1768 per tasks.md — committed in `c39d35c`, pre-diff-range); fixture `tests/ngspice/noise_mos_linear.{cir,phdl}` (T11); jfet: `jfet.phdl:285` (`4868c91`); fixture `noise_jfet` (T12) | ✅ PASS (gate-confirmed green) |
| NGP-09 (switch ON-state thermal noise) | closed switch contributes ~16 pA/√Hz at ron=1Ω | `crates/piperine-lang/headers/spice/switches.phdl` (`1ac5fdc`); fixture `noise_switch` (T13) | ✅ PASS (gate-confirmed green) |
| NGP-10 part 1 (cap+diode UIC) | `ic` enforced under UIC matching ngspice `CKTsetIC`; default-ic=0 does NOT clamp every transient | cap: `passives.phdl:181-188` `@ initial { V(p,n) <- ic; }`; diode: `diode.phdl:474-481`; default-ic filter: `instance.rs:679-704` + `compile.rs:346-373`; UIC hold release: `transient.rs:738-748`; fixtures `uic_rc.{cir,phdl}`; assertion `tests/ngspice_tran.rs:154-200` — `(v0 - 2.0).abs() < 1.0e-3` | ✅ PASS (mutation-killed: see sensor) |
| NGP-10 part 2 (bjt+jfet+mos UIC) | `IC=VBE,VCE` / `IC=VDS,VGS` enforced under UIC | bjt: `bjt.phdl:133-135,568-577`; jfet: `jfet.phdl:63-68,322-331`; mos1/2/3: `mos.phdl:105-110,514-517` (+ mos2/mos3 same shape); fixture `uic_mos.{cir,phdl}`; assertion `tests/ngspice_tran.rs:202-249` — `(v0 - 2.0).abs() < 5.0e-2` | ✅ PASS (mutation-killed) |
| NGP-10 deviation `ind` | documented deviation: current-seed mechanism | spec.md:307 (correct); ROADMAP.md:84,113-116 (correct); **passives.phdl:255-257 STALE — cites "JIT cannot force branch" which T16 closed** | ⚠️ Spec-precision gap (see Code Quality) |
| NGP-10 deviation mos/bjt/jfet | documented deviation: external-node vs internal-state seed | spec.md:307 (correct); ROADMAP.md:116-119 (correct); bjt.phdl:569-571 ("internal vbe/vce"); mos.phdl:507-513 (imprecise — claims ngspice seeds external ports, contradicts spec framing); jfet.phdl:323-326 (terse) | ⚠️ Spec-precision gap (see Code Quality) |
| NGP-11 (CKTsrcFact on vsrc+isrc TRANOP) | vsrc forced value scales on MNA force row; isrc DC value scales through simparam; TRANOP runs as DC-analysis solve | `tests/ngspice_homotopy.rs:48-97` — two fixtures (`homotopy_bias` for vsrc, `homotopy_sink` for isrc); `assert_ladder_conducts` distinguishes from a vacuous match; commit `bdf279c` (boundary) | ✅ PASS |
| NGP-12 (cox=0 substitute) | cox=0 yields ngspice's 1e-7, not NaN/Inf | `mos.phdl` flicker site (`c2b4d9f`); fixture `noise_mos_cox0` (T14) | ✅ PASS (gate-confirmed green) |
| NGP-13 (jfet nlev=3) | nlev=3 computes its own form | `jfet.phdl:285` (`4868c91`); fixture `noise_jfet_nlev3` (T12) | ✅ PASS (gate-confirmed green) |
| NGP-14 (BJT VAF/VAR temp dependence) | VAF/VAR scale with temperature per ngspice `bjttemp.c` | `bjt.phdl` (`2eb614d`); fixture `temp_bjt` (T15) | ✅ PASS (gate-confirmed green) |
| NGP-15 (every closed deviation covered by golden test) | MD-31 falsifiability gate | `tests/ngspice_parity_matrix.rs:1-22,485-535` — 4 tests enforce exhaustiveness + fixture-existence + golden-target-reference + reason-stated | ✅ PASS |

### P3 (urc faithfulness)

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
| --------- | -------------------- | ----------------------- | ------ |
| NGP-16 (urc geometric taper) | segment R/C follow ngspice's K-taper, not uniform | `urc.phdl:140-144` (generic `urc[N]`) + `urc.phdl:178-191` (`urc_taper_5_k2` direct-leaf); fixture `tests/ngspice/urc_taper.{cir,phdl}`; test `tests/ngspice_ac.rs:178-189` (`ngspice_urc_taper`) | ✅ PASS (mutation-killed: see sensor) |
| NGP-17 (urc ISPERL diode shunt) | diode-mode shunt path exists when ISPERL set | `urc.phdl:222-256` (`urc_isperl_5_k2`); fixture `tests/ngspice/urc_isperl.{cir,phdl}`; test `tests/ngspice_validation.rs:128-141` (`ngspice_urc_isperl`) | ✅ PASS |
| NGP-18 (urc re-authored generically after P4) | single generic `urc[N]` using `wire tap : Electrical[N+1]` + structural `for`; codegen lifts array-wire elements and processes structural `connection`s | `urc.phdl:126-148` (generic urc[N]); Fix A `crates/piperine-codegen/src/resolve/pom/structure.rs:110-122` (one node per element); Fix B `crates/piperine-codegen/src/device/builder.rs:54-108,421-432,463-540` (alias forest); 4 inline tests across structure.rs + builder.rs | ✅ PASS (mutation-killed: see sensor) |
| NGP-19 (urc FMAX → N auto-selection) | derive N from FMAX by ngspice's rule when N not given | `urc.phdl:35-49` (URC_DEFAULT_N pre-computed from ngspice defaults); fixture `tests/ngspice/urc_fmax.{cir,phdl}` instantiates `urc[URC_DEFAULT_N]`; test `tests/ngspice_validation.rs:143-158` (`ngspice_urc_fmax`) | ✅ PASS |
| NGP-22 (urc[N] replaces triple) | three hand-unrolled modules gone; .cir twins unchanged | `urc.phdl` no longer contains `urc2`/`urc5`/`urc10` definitions (deleted in `94abc06`); `tests/ngspice/urc_lump{2,5,10}.phdl:15` use `urc[N]`; `.cir` twins unchanged (`git diff --stat -- tests/ngspice/urc_lump*.cir` = empty); `crates/piperine-lang/tests/urc_flatten.rs:39-69` updated for new labels | ✅ PASS |

### P4 (gap-3)

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
| --------- | -------------------- | ----------------------- | ------ |
| NGP-20 (const-index array-net expands) | flattening expands const-indexed NetRef to scalar net, not fail-loud | `flatten.rs:188-244` (`remap` + `expand_array_element`); test `flatten.rs:795-817` `inline_indexed_array_wire_expands_to_scalar_net` + end-to-end `flatten.rs:879-907` `array_net_element_survives_elaboration_end_to_end` | ✅ PASS |
| NGP-21 (non-const index still fails loud) | non-elaboration-constant index fails loud naming the index expression | `flatten.rs:910-926` `non_constant_array_index_fails_loud` — asserts msg contains "net ref index on `bus`" and "k" | ✅ PASS |
| NGP-21 corner (out-of-range, scalar, nested) | each loud case named | `flatten.rs:834-877` (`out_of_range`, `scalar_net`, `onto_an_indexed_binding`) | ✅ PASS |

### Success Criteria

| Criterion | Status | Evidence |
| --------- | ------ | -------- |
| No ✅ in ROADMAP unbacked by test | ✅ | `tests/ngspice_parity_matrix.rs:502-509` enforces every Covered cell names an existing fixture; `:514-523` enforces a golden target runs it |
| Golden corpus covers six domains across 13 models | ✅ | `tests/ngspice_parity_matrix.rs:85-88` MODELS axis = 13; `:30-37` Domain enum = 6 (DcOp, DcSweep, Ac, Noise, Transient, Temperature) |
| Every 2026-07-28 deviation closed or documented | ✅ | spec.md:296-321 traceability table; ROADMAP.md:96-119 cross-cutting issues; SPEC_DEVIATION notes in `urc.phdl:41-49` (compile-time taper propagation) |
| `urc` matches ngspice on tapered fixture | ✅ | `tests/ngspice_ac.rs:178-189` `ngspice_urc_taper` passes; mutation sensor killed (see below) |
| `urc[N]` exists; triple gone; .cir twins unchanged | ✅ | `urc.phdl` (no triple); `git diff --stat -- tests/ngspice/urc_lump*.cir` empty |
| ROADMAP "SPICE model completeness" rewritten from measured results; stale `ic`/UIC blocker claim corrected | ⚠️ Partial | ROADMAP.md:64-119 rewritten from measured results, stale `ic`/UIC claim corrected at lines 109-119; **but urc row line 94 drifted stale** (says NGP-18/19/22 pending; they're closed) |
| `cargo test --workspace` green with zero rustc warnings; baselines bit-identical | ✅ | 1841 passed, 0 failed, 0 rustc warnings (only pre-existing python `.so` build-script notice); ngspice DC/tran baselines preserved (the_rc_fixture, xfmr_coupled_lc_energy_transfer, ngspice_tran_rc unchanged) |

**Status**: ✅ All 22 ACs covered with `file:line` evidence and spec-matching assertions; ⚠️ two documentation-drift gaps (does not block close).

---

## Discrimination Sensor

Mutations applied in scratch state (file edit → test → revert), each chosen to target the highest-risk new code named in the prompt.

| # | Mutation site | Description | Killed by | Result |
| - | ------------- | ----------- | --------- | ------ |
| 1 | `crates/piperine-codegen/src/device/analog/instance.rs:699` | Flipped the default-ic filter: `== 0` → `!= 0` (default params trigger seeds; given ones don't) | `tests/ngspice_tran.rs::ngspice_uic_rc` FAILED, `ngspice_uic_mos` FAILED; baseline `ngspice_tran_rc`/`the_rc_fixture_*`/`xfmr_coupled_lc_energy_transfer` still PASS | ✅ Killed |
| 2 | `crates/piperine-solver/src/analyses/transient.rs:748` | Reverted `uic_hold = false` to the pre-T16 form `!self.system.uic_clamps.is_empty()` (hold through the first step) | `tests/ngspice_tran.rs::ngspice_uic_rc` FAILED (mos fixture passes — its slower τ makes the ~step/τ startup error invisible; RC's tight τ exposes it) | ✅ Killed (1 fixture suffices) |
| 3 | `crates/piperine-lang/headers/spice/urc.phdl:179-191` (`urc_taper_5_k2`) | Replaced every `pow(2.0, i)` with `1.0` (uniform ladder where the spec demands geometric K=2 taper) | `tests/ngspice_ac.rs::ngspice_urc_taper` FAILED — "71 of 41 point(s) out of tolerance" | ✅ Killed |
| 4 | `crates/piperine-codegen/src/resolve/pom/structure.rs:110-122` | Collapsed array-wire element registration back to the pre-Fix-A single-scalar form | `tests/ngspice_validation.rs::{ngspice_urc_lump2, ngspice_urc_lump5, ngspice_urc_lump10, ngspice_urc_fmax}` all FAILED; `crates/piperine-codegen/src/resolve/pom/structure.rs::tests::{array_wire_registers_one_node_per_element, lifted_array_wire_resolves_per_element_in_flat_parent}` FAILED; `ngspice_urc_isperl` passes (uses direct-leaf `urc_isperl_5_k2`, no array wire) | ✅ Killed (4 integration + 2 unit) |

**Sensor depth**: lightweight (default — 4 mutations on the four highest-risk sites named in the prompt)
**Result**: 4/4 killed — PASS ✅
**Post-restore gate**: 1841 passed, 0 failed (identical to pre-sensor count — clean restoration)

---

## Code Quality

| Principle | Status | Notes |
| --------- | ------ | ----- |
| Minimum code (no features beyond what was asked) | ✅ | No gold-plating; spec's "no new device" constraint honoured (`asrc`/`vbic` rejection recorded) |
| Surgical changes (only touched files required) | ✅ | 26 files in `git diff --stat bdf279c..HEAD`; each maps to a task |
| No scope creep | ✅ | Each task scoped to its concern |
| Matches existing patterns | ✅ | `ToleranceContract` mirrors `capabilities_contract.rs` registry; `wrdata` parser preserves existing strictness; flatten's `expand_array_element` mirrors existing fail-loud shape |
| MD-13 rule 2 (no loose functions) | ⚠️ Minor | `crates/piperine-codegen/src/device/builder.rs:480,509,520,536` introduce 4 private module-level helpers (`build_node_aliases`, `resolve_alias_target`, `union_root`, `canonicalize`) tightly coupled to `InstanceBuilder`. They are private (not `pub`/`pub(crate)`), so the rule's strict wording is not violated, but the rule's intent ("if a helper doesn't belong to a struct, the abstraction is missing") suggests extracting a `NodeAliasMap` struct that owns the alias forest and exposes `canonicalize(&self, node) -> NodeId`. Functional and tested; cosmetic improvement only. |
| MD-13 rule 5 (no macros) | ✅ | All new code is plain Rust; `ParityMatrix` registry is a `const` table, not a `macro_rules!` |
| MD-25 (non-destructive flatten) | ✅ | `flatten.rs:188-244` `expand_array_element` constructs new NetRefs; the existing `flatten_is_idempotent` and `flatten_writes_only_flat_modules` tests still pass; new `inline_indexed_array_wire_expands_to_scalar_net` asserts `modules untouched (non-destructive)` |
| MD-28 (test placement) | ✅ | Unit tests for support module + flattener + codegen structure/builder live with the code; integration tests for fixtures in `tests/ngspice_*.rs` |
| MD-31 (policy-as-test) | ✅ | `tests/ngspice_parity_matrix.rs` is the MD-31 gate; 4 mechanical checks |
| MD-38 (parity measured, not asserted) | ✅ | Every Covered cell names a fixture a golden target runs; the sensor proves the fixtures discriminate |
| Spec-anchored outcome check (asserted values match spec) | ✅ | uic_rc asserts `v(out)@0 = 2.0 V ±1e-3` (matches the IC seed); uic_mos asserts `v(out)@0 = 2.0 V ±5e-2`; urc_taper fails loud on uniform revert |
| Per-layer Coverage Expectation met | ✅ | Domain logic 1:1 to ACs; routes (golden targets) cover happy + edge + error paths |
| Every test maps to a spec requirement — no unclaimed tests | ✅ | Each new `#[test]` cites its NGP ID in its docstring |
| Documented guidelines followed | ✅ | `AGENTS.md` §Build and test, §Test placement, §Hard rules; `CLAUDE.md` §Build and test |

---

## Edge Cases (spec.md §Edge Cases)

- [x] ngspice absent → SKIP — `tests/support/ngspice.rs:287-289` `NgspiceHarness::detect` returns None; every new `#[test]` opens with the SKIP branch (verified in `ngspice_ac.rs:185-189`, `ngspice_tran.rs:158-161`, `ngspice_tran.rs:209-212`, `ngspice_validation.rs:130-141`)
- [x] ngspice output unparseable → loud — `tests/support/ngspice.rs:228-252` `WrData::parse` strict mode; unit tests `wrdata_malformed_line_fails_loud_*`, `wrdata_unparseable_number_fails_loud`, `wrdata_wrong_column_count_fails_loud`, `wrdata_empty_export_fails_loud`
- [x] Zero shared nodes → contract violation — `tests/support/ngspice.rs:406-411` `compare_op` returns "0 shared nodes between ngspice … and piperine — contract violation"
- [x] Piperine non-convergence → distinct from tolerance miss — `tests/ngspice_tran.rs:73-79` `convergence_failure` returns "piperine transient failed to converge"; test `:309-319` `a_convergence_failure_is_distinct_from_a_tolerance_miss` asserts the phrasing never overlaps with a tolerance-miss message
- [x] Noise PSD near zero → absolute floor — `tests/support/ngspice.rs:85-92` `NOISE_PSD.abstol = 1e-24` (squared amplitude floor); `NOISE_TOTAL.abstol = 1e-15`
- [x] ngspice defect → documented deviation — `urc.phdl:41-49` SPEC_DEVIATION note (compile-time taper propagation); ROADMAP.md:84,94,113-119 record both NGP-10 deviations with reasons
- [x] Model param exists in ngspice, no PHDL counterpart → named gap — ROADMAP.md:82-94 "Open" column per model (e.g. mos1 "mos3's `badMos3` alt mode intentionally absent"; diode "LEVEL=3 geometry path missing"; bjt "SPICE2-era temperature model only (~50 modern temp-coeff params absent)")

---

## Gate Check

- **Gate command**: `cargo build --workspace && cargo test --workspace` (AGENTS.md §Build and test)
- **Build**: green; zero rustc warnings (the two `piperine-cli` `warning:` lines are pre-existing build-script `println!` about the Python `.so`, not rustc diagnostics)
- **Test result**: **1841 passed, 0 failed, 4 ignored**
  - 4 ignored: 3 pre-existing piperine-solver doc-tests + 1 pre-existing piperine-plugin doc-test (none from this feature)
- **Test count before feature** (`git checkout bdf279c -- . && cargo test --workspace`): **1826 passed, 0 failed, 4 ignored**
- **Delta**: **+15 tests, 0 regressions, 0 new ignores**
- **Baseline parity preserved**: ngspice DC operating-point baselines (`divider`, `rdiode`, `bjt_ce`, `jfet_bias`, `nmos_*`, `urc_lump*`) and transient baselines (`ngspice_tran_rc`, `the_rc_fixture_matches_its_analytic_charging_curve`, `xfmr_coupled_lc_energy_transfer`) all bit-identical — the default-ic filter in `AnalogInstance::initial_conditions` (`instance.rs:679-704`) suppresses seeds whose backing params are at declared defaults, so models keeping `ic = 0.0` stay inert under UIC.

---

## Documented Deviations Audit

The prompt names two known deviations. Per the prompt's rule, each must be recorded in **all three sites**: (a) the relevant model header comment, (b) ROADMAP.md, (c) spec.md traceability.

### Deviation 1: `ind` (NGP-10 part — current-seed mechanism)

| Site | Location | Status |
| ---- | -------- | ------ |
| (a) Model header | `crates/piperine-lang/headers/spice/passives.phdl:255-257` | ⚠️ **STALE** — comment reads "Reserved until the JIT can force a branch inside `@ initial` (see piperine ROADMAP)". The JIT CAN force a branch (T16 just wired cap+diode that way); the actual blocker is the current-seed mechanism (`collect_initial_conditions` only collects `Potential` seeds, not `Flow` seeds). The deviation is recorded; the **reason** is stale. |
| (b) ROADMAP.md | `ROADMAP.md:84` (ind row Open column) + `:113-116` (cross-cutting issue #2) | ✅ Correct: "ngspice's `ind.IC` is a **current** seed (`indload.c` flux form `L·I`); `collect_initial_conditions` only collects `Potential` seeds — a current-seed mechanism is a separate codegen feature" |
| (c) spec.md traceability | `spec.md:307` (NGP-10 row) | ✅ Correct: matches (b) verbatim |

**Verdict**: deviation recorded in all three sites; **gap = stale reason in (a)**. Recommended fix: refresh `passives.phdl:255-257` to cite the current-seed blocker (the cap+diode wiring in T16 is the proof-of-existence that the JIT branch-forcing blocker is closed).

### Deviation 2: mos/bjt/jfet external-node vs device-state-seed (NGP-10 narrower)

| Site | Location | Status |
| ---- | -------- | ------ |
| (a) Model header | bjt: `bjt.phdl:569-571` "device's internal vbe/vce start from these values and the rest of the circuit solves against them" — ✅ precise; mos: `mos.phdl:507-513` "external d/s ports are, with the series resistances solved consistently" — ⚠️ imprecise (contradicts spec.md framing; claims external, spec says internal); jfet: `jfet.phdl:323-326` — ⚠️ terse (does not name the divergence case) | ⚠️ Spec-precision gap (1 of 3 precise; 2 of 3 imprecise/terse) |
| (b) ROADMAP.md | `ROADMAP.md:116-119` | ✅ Correct: "piperine's `@initial` pins the external node, whereas ngspice seeds the device's internal state during UIC init. The two agree with no series resistance (`uic_mos`'s case) and diverge only when RD/RS are nonzero" |
| (c) spec.md traceability | `spec.md:307` (NGP-10 row tail) | ✅ Correct: matches (b) verbatim |

**Verdict**: deviation recorded in all three sites; **gap = model-header imprecision in mos.phdl + jfet.phdl** (the divergence case "RD/RS nonzero" is named in spec/ROADMAP but not in the model headers). Recommended fix: align mos.phdl:507-513 and jfet.phdl:323-326 with bjt.phdl:569-571 (which is precise).

### Bonus drift gap: ROADMAP.md urc row (not in the prompt's named deviations but found in audit)

| Site | Location | Status |
| ---- | -------- | ------ |
| ROADMAP.md:94 | "ISPERL diode-mode shunt landed (T20); **NGP-18/19/22 documented deviations** — generic `urc[N]` authored but the codegen side does not yet lift array-wire elements or process structural `connections`, so `FMAX → N` auto-select and retirement of the hand-unrolled triple wait on that integration" | ❌ **STALE/WRONG** — Fix A (`ba79501` "register array-wire elements as distinct resolved nodes") landed; Fix B (`8a8114b` "unify nodes through structural connections in circuit build") landed; the hand-unrolled triple was retired (`94abc06`); FMAX → N works (`ngspice_urc_fmax` passes). The ROADMAP was rewritten in T24 (`30bca6e`) describing the state BEFORE Fix A/B; the traceability sync (`29f00d9`) updated spec.md but missed ROADMAP. spec.md:296-321 correctly marks NGP-18/19/22 as Verified. |

**Verdict**: **gap = ROADMAP.md contradicts the measured state for NGP-18/19/22**. Recommended fix: update ROADMAP.md:94 to reflect Fix A/B and the triple retirement.

---

## Requirement Traceability Update

Spec.md `## Requirement Traceability` already updated by `29f00d9` (the traceability sync). All 22 requirements marked Verified or documented-deviation. No change needed.

---

## Summary

**Overall**: ✅ Ready (with two minor documentation-drift gaps + one model-header imprecision — none blocking, all fixable in a single docs commit)

**Spec-anchored check**: 22/22 ACs matched spec outcome; 0 spec-precision gaps in the in-scope diff range. Three documentation-drift gaps flagged (do not affect AC coverage):
  1. `passives.phdl:255-257` ind deviation reason stale (cites closed blocker)
  2. `mos.phdl:507-513` + `jfet.phdl:323-326` external-node deviation imprecise (does not name the RD/RS-nonzero divergence case that spec.md/ROADMAP name)
  3. `ROADMAP.md:94` urc row drifted (says NGP-18/19/22 pending; they're closed by Fix A/B + `94abc06`)

**Sensor**: 4/4 mutations killed (UIC default-ic filter, UIC hold release, urc K-taper, array-wire element registration)
**Gate**: 1841 passed (delta +15 from baseline 1826), 0 failed, 0 rustc warnings, ngspice DC/tran baselines bit-identical

**What works**:
- All 22 NGP requirements verified against spec-defined outcomes with `file:line` evidence
- T16/T17 (NGP-10): UIC wiring on cap/diode/bjt/jfet/mos1/2/3 — default-ic filter prevents baseline regressions; sensor-killed
- T19/T20 (NGP-16/17): urc geometric taper + ISPERL shunt — sensor-killed
- T21 (NGP-20/21): gap-3 array-net expansion with 7 inline tests covering happy + 4 loud-fail paths
- T22 + Fix A/B (NGP-18/22): generic `urc[N]` retires the hand-unrolled triple; `.cir` twins unchanged — sensor-killed (4 integration + 2 unit tests)
- T23 (NGP-19): FMAX → N auto-selection via `URC_DEFAULT_N` pre-computed constant
- The MD-31 parity matrix gate (`tests/ngspice_parity_matrix.rs`) is the spec's headline deliverable: every Covered cell names a fixture a golden target actually runs

**Issues found**:
- **Issue 1 (Minor — docs)**: `passives.phdl:255-257` ind deviation comment cites the closed "JIT cannot force branch" blocker. Fix: refresh to cite the current-seed mechanism blocker per ROADMAP.md:84,113-116.
- **Issue 2 (Minor — docs)**: `mos.phdl:507-513` and `jfet.phdl:323-326` do not name the RD/RS-nonzero divergence case that spec.md:307 and ROADMAP.md:116-119 name explicitly. Fix: align with `bjt.phdl:569-571` (which is precise).
- **Issue 3 (Minor — docs)**: `ROADMAP.md:94` urc row contradicts measured state — says NGP-18/19/22 are pending codegen integration when Fix A/B + `94abc06` have closed them. Fix: update line 94 to reflect Fix A/B and the triple retirement.
- **Issue 4 (Cosmetic — code quality)**: `builder.rs:480,509,520,536` introduce 4 private module-level helpers tightly coupled to `InstanceBuilder`. MD-13 rule 2's strict wording (about `pub`/`pub(crate)`) is not violated, but the rule's intent suggests extracting a `NodeAliasMap` struct. Functional and tested; improvement only.

**Next steps**: a single docs commit closing Issues 1-3 (no code change, no test change, no baseline drift). Issue 4 is optional cosmetic refactoring.
