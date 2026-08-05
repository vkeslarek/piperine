# ngspice-parity Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Design**: `.specs/features/ngspice-parity/design.md`
**Status**: Draft

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec — confirm before Execute.
> Guidelines found: `AGENTS.md` (§Build and test, §Test placement, §Hard rules),
> `CLAUDE.md` (§Build and test, §Where the tests are), `tests/suite_hygiene.rs`
> (the mechanical enforcement of MD-28/MD-31).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Test-support module (`WrData`, `ToleranceContract`, `NgspiceHarness`) | unit | All branches; every parse/tolerance edge case in the spec's Edge Cases list (malformed line, empty export, N-column shapes, zero shared coordinates) | `tests/support/*.rs` (`#[cfg(test)] mod tests`) | `cargo test -p piperine --test ngspice_validation` |
| Golden cross-check target (per domain) | integration | 1:1 to spec ACs for that domain: happy path + tolerance-miss path + ngspice-absent SKIP + vacuous-comparison guard | `tests/ngspice_*.rs` | `cargo test -p piperine --test ngspice_<domain>` |
| Coverage gate (`ParityMatrix`) | integration | Exhaustiveness over model × domain; every `Covered` row's fixture exists; **guard proven able to fail** (MD-31) | `tests/ngspice_parity_matrix.rs` | `cargo test -p piperine --test ngspice_parity_matrix` |
| PHDL model bodies (`headers/spice/*.phdl`) | integration | Every closed deviation covered by a golden test that fails when the fix is reverted (NGP-15, MD-38) | `tests/ngspice/*.{cir,phdl}` + the domain target | `cargo test --workspace` |
| Flattener (`crates/piperine-lang/src/elab/lower/flatten.rs`) | unit | All branches: const index expands, non-const index still fails loud, existing fail-loud paths unchanged | `crates/piperine-lang/src/elab/lower/flatten.rs` (`#[cfg(test)] mod tests`) | `cargo test -p piperine-lang` |
| Docs (`AGENTS.md`, `CLAUDE.md`, `ROADMAP.md`) | none | Build gate only | — | build gate only |

**Note:** The strong default applies to the model layer — the repo's existing
ngspice checks are the **floor**, not the ceiling. A fix task's fixture must
exercise the region where the model was wrong (e.g. the linear region for
`gm_est`), or it proves nothing.

## Gate Check Commands

> Generated from codebase (`AGENTS.md` §Build and test, `CLAUDE.md`) — confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After tasks touching one target or the support module | `cargo test -p piperine --test <target>` |
| Full | After tasks touching models, the flattener, or more than one target | `cargo test --workspace` |
| Build | After phase completion | `cargo build --workspace && cargo test --workspace` (zero rustc warnings is the bar) |

---

## Execution Plan

Phases are ordered and run sequentially — each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Oracle foundation (4 tasks)

Shared machinery every domain target consumes. No behaviour change to existing checks.

```
T1 → T2 → T3 → T4
```

### Phase 2: Domain comparators + the gate (6 tasks)

One target per domain, then the MD-31 coverage gate.

```
T5 → T6 → T7 → T8 → T9 → T10
```

### Phase 3: Model fixes (8 tasks)

Each fix ships with the golden fixture that fails when it is reverted.

```
T11 → T12 → T13 → T14 → T15 → T16 → T17 → T18
```

### Phase 4: urc faithfulness, gap-3-independent (2 tasks)

```
T19 → T20
```

### Phase 5: gap-3 and the generic urc (3 tasks)

```
T21 → T22 → T23
```

### Phase 6: Close (1 task)

```
T24
```

---

## Task Breakdown

### T1: Extract `NgspiceHarness` into a shared support module

**What**: Move `NgspiceHarness` (detection, SKIP seam, fixture paths, source map, DC compare) from `tests/ngspice_validation.rs` into `tests/support/ngspice.rs`; `ngspice_validation.rs` declares `mod support;` and keeps every existing test body unchanged.
**Where**: `tests/support/ngspice.rs` (new), `tests/ngspice_validation.rs` (modify)
**Depends on**: None
**Reuses**: Every existing method body — this is a move, not a rewrite
**Requirement**: NGP-01..07 (enabler)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `tests/support/ngspice.rs` exists with a `//!` header stating its scope
- [x] `ngspice_validation.rs` compiles against it with **zero changes to existing assertions**
- [x] `tests/support/` is not treated as a target (verified: `suite_hygiene::integration_targets` matches top-level only)
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: the 26 existing ngspice checks + suite_hygiene still pass (no silent deletions)

**Tests**: integration (existing, unchanged) · **Gate**: full
**Commit**: `refactor(tests): extract NgspiceHarness into tests/support`

---

### T2: Generalize the `wrdata` parser to N columns

**What**: Replace the strict two-float `parse_wrdata` with a `WrData` type parsing N columns, exposing `pairs()` (2-col) and `complex()` (3-col AC `freq real imag`), preserving the existing strictness contract.
**Where**: `tests/support/ngspice.rs`
**Depends on**: T1
**Reuses**: `parse_wrdata`'s strictness rules (malformed line ⇒ loud, empty ⇒ loud)
**Requirement**: NGP-01, NGP-02, NGP-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `WrData::parse(circuit, content, columns)` handles 2- and 3-column exports
- [x] `complex()` reconstructs `Complex64` from the `real imag` pair (format verified against ngspice-46 in design.md)
- [x] Unit tests: valid 2-col, valid 3-col, malformed line fails loud naming the line, empty content fails loud, wrong column count fails loud
- [x] Existing DC sweep checks still pass through the new type
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥5 new unit tests + 26 existing checks pass

**Tests**: unit · **Gate**: full
**Commit**: `test(ngspice): parse wrdata exports of arbitrary column count`

---

### T3: Add the per-domain `ToleranceContract`

**What**: A `ToleranceContract { reltol, abstol, rationale }` data table with the seven domain constants from design.md, `holds()`, and `rationale()` surfaced in failure text.
**Where**: `tests/support/ngspice.rs`
**Depends on**: T1
**Reuses**: `within_tolerance`'s `|a−b| ≤ abstol + reltol·max(|a|,|b|)` form
**Requirement**: **NGP-07**, NGP-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Seven constants defined (`DC`, `AC_MAG`, `AC_PHASE`, `NOISE_PSD`, `NOISE_TOTAL`, `TRAN`, `TEMP`), each with a non-empty `rationale`
- [x] No numeric tolerance literal remains in any test body (NGP-07: the contract is the only source)
- [x] DC constants are byte-identical to today's values (no baseline drift)
- [x] Unit tests: boundary at exactly the tolerance, just inside, just outside
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥3 new unit tests

**Tests**: unit · **Gate**: full
**Commit**: `test(ngspice): declare the per-domain tolerance contract`

---

### T4: Add the multi-export scratch-dir runner

**What**: `NgspiceHarness::run_export(circuit, exports)` — run a `.cir` in a scratch dir keyed `{circuit}-{domain}-{pid}` and read back N `wrdata` files.
**Where**: `tests/support/ngspice.rs`
**Depends on**: T2
**Reuses**: The existing `ngspice_sweep` scratch-dir pattern
**Requirement**: NGP-01..04 (enabler); mitigates the design's parallelism concern

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Scratch key includes the domain, so two domains for one circuit cannot collide
- [x] A missing expected export file fails loud naming circuit + path
- [x] Scratch dir is removed on both success and failure paths
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥2 new unit tests (missing file; scratch isolation)

**Tests**: unit · **Gate**: full
**Commit**: `test(ngspice): run a fixture and read back N wrdata exports`

---

### T5: AC cross-check target + fixture

**What**: `tests/ngspice_ac.rs` with an RC-lowpass fixture twin, comparing magnitude and phase per frequency point; phase compared only where magnitude clears the floor, and skips are counted.
**Where**: `tests/ngspice_ac.rs` (new), `tests/ngspice/ac_rc.{cir,phdl}` (new)
**Depends on**: T3, T4
**Reuses**: `WrData::complex`, `ToleranceContract::{AC_MAG, AC_PHASE}`, `Session::ac`
**Requirement**: **NGP-01**, NGP-05, NGP-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `//!` scope header present (suite_hygiene)
- [x] Magnitude and phase compared per frequency; failure names node, frequency, both values, Δ and the contract rationale
- [x] Phase skipped where magnitude < floor, and a fixture where *every* point skips fails as vacuous
- [x] ngspice-absent takes the SKIP branch (NGP-06)
- [x] The RC fixture's analytic response is verified independently (known −3 dB point), so the test is not merely self-consistent
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥4 tests (happy, vacuous guard, skip branch, tolerance-miss message)

**Tests**: integration · **Gate**: full
**Commit**: `test(ngspice): cross-check AC magnitude and phase per frequency`

---

### T6: Noise cross-check target + fixture + the unit reconciliation

**What**: `tests/ngspice_noise.rs` comparing output-noise spectrum per frequency and the integrated total. **Owns the V/√Hz → V²/Hz conversion in exactly one place**, with the derivation in-comment.
**Where**: `tests/ngspice_noise.rs` (new), `tests/ngspice/noise_rc.{cir,phdl}` (new)
**Depends on**: T3, T4
**Reuses**: `ToleranceContract::{NOISE_PSD, NOISE_TOTAL}`, `NoiseTrace::{psd, total}`
**Requirement**: **NGP-02**, NGP-05, NGP-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `//!` header present
- [x] ngspice `onoise_spectrum` (V/√Hz) squared to piperine's V²/Hz in one named site, derivation commented (design.md §Research)
- [x] **Unit-probe test**: a 1 kΩ resistor's ngspice spectrum equals √(4kTR) = 4.07e-9 V/√Hz within `NOISE_PSD` — this is what catches a future unit regression
- [x] `onoise_total` compared against `NoiseTrace::total()`
- [x] ngspice-absent SKIP branch
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥4 tests (per-point, integrated, unit probe, skip branch)

**Tests**: integration · **Gate**: full
**Commit**: `test(ngspice): cross-check noise PSD and integrated total`

---

### T7: Transient cross-check target + fixture

**What**: `tests/ngspice_tran.rs` comparing node voltages on ngspice's print grid, evaluating piperine's adaptive waveform via `Waveform::at(t)`.
**Where**: `tests/ngspice_tran.rs` (new), `tests/ngspice/tran_rc.{cir,phdl}` (new)
**Depends on**: T3, T4
**Reuses**: `Waveform::at`, `ToleranceContract::TRAN`, `Session::tran`
**Requirement**: **NGP-03**, NGP-05, NGP-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `//!` header present
- [x] Comparison is on ngspice's print times via interpolation — **never** raw adaptive samples
- [x] A non-convergent fixture reports "failed to converge" distinctly from a tolerance miss (spec Edge Cases)
- [x] ngspice-absent SKIP branch
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥4 tests

**Tests**: integration · **Gate**: full
**Commit**: `test(ngspice): cross-check transient on a common time grid`

---

### T8: Temperature cross-check target + fixture

**What**: `tests/ngspice_temp.rs` comparing the operating point at a non-nominal temperature (`.temp` card vs `SolverConfig.temperature`).
**Where**: `tests/ngspice_temp.rs` (new), `tests/ngspice/temp_diode.{cir,phdl}` (new)
**Depends on**: T3, T4
**Reuses**: `compare_op`, `ToleranceContract::TEMP`
**Requirement**: **NGP-04**, NGP-05, NGP-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `//!` header present
- [x] At least two temperatures compared (one below, one above nominal), so a model pinned to nominal fails
- [x] ngspice-absent SKIP branch
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥3 tests

**Tests**: integration · **Gate**: full
**Commit**: `test(ngspice): cross-check the operating point across temperature`

---

### T9: The parity-matrix coverage gate (MD-31)

**What**: `tests/ngspice_parity_matrix.rs` — a registry of every (model, domain) cell as `Covered { fixture }` or `NotApplicable { reason }`, with an exhaustiveness assert and a fixture-existence assert.
**Where**: `tests/ngspice_parity_matrix.rs` (new)
**Depends on**: T5, T6, T7, T8
**Reuses**: `capabilities_contract.rs`'s registry+exhaustiveness pattern
**Requirement**: **NGP-15**; delivers the spec's headline goal ("no unfalsifiable ✅")

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `//!` header present
- [x] Every one of the 13 models × 6 domains is `Covered` or `NotApplicable` with a written reason
- [x] A `Covered` row naming a missing `.cir`/`.phdl` twin fails loud
- [x] **Guard proven able to fail** (MD-31): add an unaccounted model row, observe the failure, revert — recorded in the commit body
- [x] Gate check passes: `cargo test --workspace`
- [x] Test count: ≥3 tests

**Tests**: integration · **Gate**: full
**Commit**: `test(ngspice): gate model × domain parity coverage`

---

### T10: Update the test-placement docs

**What**: Extend `AGENTS.md` §Test placement and `CLAUDE.md` §Where the tests are to name the new ngspice targets, so the docs match the tree.
**Where**: `AGENTS.md`, `CLAUDE.md`
**Depends on**: T9
**Reuses**: The existing table rows
**Requirement**: NGP-07 (discoverability of the contract)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Both tables name every new target and `tests/support/`
- [x] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: none (docs — matrix says build gate only) · **Gate**: build
**Commit**: `docs: name the per-domain ngspice targets in the test tables`

---

### T11: MOSFET channel noise uses the analytic `gm`

**What**: Replace `gm_est` (saturation-only `beta * vgst`) with the model's analytic `gm` in mos1/mos2/mos3 channel noise; fixture must sweep **into the linear region**.
**Where**: `crates/piperine-lang/headers/spice/mos.phdl` (:471, :1209, :1768), `tests/ngspice/noise_mos_linear.{cir,phdl}` (new)
**Depends on**: T9
**Reuses**: ngspice `mos1noi.c` (`MOS1gm`) as the reference
**Requirement**: **NGP-08**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] All three `gm_est` sites replaced with the analytic `gm`
- [x] Fixture biases the device in the **linear** region, where the old estimate was wrong — a saturation-only fixture does not satisfy this task
- [x] Reverting the change makes the new fixture fail (recorded in the commit body)
- [x] Parity matrix updated: mos1/2/3 × Noise → `Covered`
- [x] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): use the analytic gm for MOSFET channel noise`

---

### T12: JFET channel noise — analytic `gm` and `nlev = 3`

**What**: Same `gm_est` replacement for jfet, plus the missing `nlev = 3` noise form.
**Where**: `crates/piperine-lang/headers/spice/jfet.phdl` (:285), `tests/ngspice/noise_jfet.{cir,phdl}` (new)
**Depends on**: T11
**Reuses**: ngspice `jfetnoi.c`
**Requirement**: **NGP-08**, **NGP-13**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Analytic `gm` replaces `gm_est`; fixture exercises the linear region
- [x] `nlev = 3` computes its own form; a fixture sets `nlev=3` and matches ngspice
- [x] Reverting either change fails its fixture
- [x] Parity matrix updated: jfet × Noise → `Covered`
- [x] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): analytic gm and nlev=3 noise for the JFET`

---

### T13: Switch ON-state thermal noise

**What**: Add ON-state thermal noise to `sw`/`csw`, matching ngspice `swnoise.c`.
**Where**: `crates/piperine-lang/headers/spice/switches.phdl`, `tests/ngspice/noise_switch.{cir,phdl}` (new)
**Depends on**: T11
**Reuses**: ngspice `swnoise.c`
**Requirement**: **NGP-09**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Closed switch contributes thermal noise (~16 pA/√Hz at `ron = 1 Ω`, per the ROADMAP audit)
- [x] Open switch contributes the OFF-state value ngspice uses
- [x] Reverting fails the fixture
- [x] Parity matrix updated: sw/csw × Noise → `Covered`
- [x] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): stamp ON-state thermal noise for the switch`

---

### T14: Flicker noise with `cox = 0`

**What**: Substitute ngspice's `1e-7` when `cox = 0` in MOSFET flicker noise instead of dividing by zero.
**Where**: `crates/piperine-lang/headers/spice/mos.phdl`, `tests/ngspice/noise_mos_cox0.{cir,phdl}` (new)
**Depends on**: T11
**Reuses**: ngspice `mos1noi.c`
**Requirement**: **NGP-12**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `cox = 0` yields ngspice's substituted value, not NaN/Inf
- [x] Fixture uses a model card with no `tox`/`cox`, the case that triggers it
- [x] Reverting fails the fixture
- [x] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): substitute cox=0 in MOSFET flicker noise`

---

### T15: BJT `VAF`/`VAR` temperature dependence

**What**: Replace the hardcoded identity temperature scaling of `VAF`/`VAR` with ngspice's dependence.
**Where**: `crates/piperine-lang/headers/spice/bjt.phdl`, `tests/ngspice/temp_bjt.{cir,phdl}` (new)
**Depends on**: T8
**Reuses**: ngspice `bjttemp.c`
**Requirement**: **NGP-14**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `VAF`/`VAR` scale with temperature per ngspice
- [x] Fixture compares at two temperatures away from nominal
- [x] Reverting fails the fixture
- [x] Parity matrix updated: bjt × Temperature → `Covered`
- [x] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): temperature-scale BJT VAF and VAR`

---

### T16: Wire `ic`/UIC on cap, ind and diode

**What**: Connect the declared-but-dead `ic` params to `@initial { V(p,n) <- ic; }` so UIC is enforced.
**Where**: `headers/spice/passives.phdl` (:129, :210), `headers/spice/diode.phdl` (:100), `tests/ngspice/uic_rc.{cir,phdl}` (new)
**Depends on**: T7
**Reuses**: **The mechanism already exists** — `flatten/analog.rs:876` collects `@initial` as UIC seeds, `transient.rs:228` clamps them. ROADMAP's "blocked" claim is stale.
**Requirement**: **NGP-10** (part 1), NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `ic` on cap/ind/diode drives the t=0 state under UIC
- [ ] Fixture runs `.tran … uic` and matches ngspice's `CKTsetIC` behaviour
- [ ] A run *without* UIC is unaffected (no regression on existing transient behaviour)
- [ ] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): enforce ic on cap, ind and diode under UIC`

---

### T17: Wire `ic`/UIC on bjt, jfet and mos1/2/3

**What**: The same wiring for the remaining five models.
**Where**: `headers/spice/{bjt,jfet,mos}.phdl`, `tests/ngspice/uic_mos.{cir,phdl}` (new)
**Depends on**: T16
**Reuses**: T16's pattern
**Requirement**: **NGP-10** (part 2), NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `ic` enforced on all five models under UIC
- [ ] Fixture covers at least one junction device and one MOSFET
- [ ] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): enforce ic on bjt, jfet and mos1/2/3 under UIC`

---

### T18: Source-stepping scale on `vsrc` and `isrc`

**What**: Apply the source-stepping scale (`CKTsrcFact`) in `vsrc`, and cover the TRANOP case in `isrc`.
**Where**: `crates/piperine-lang/headers/spice/sources.phdl`, `tests/ngspice/homotopy_bias.{cir,phdl}` (new)
**Depends on**: T9
**Reuses**: `DcAnalysisState`'s source-stepping scale (Part VII §15.6)
**Requirement**: **NGP-11**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] `vsrc` multiplies its value by the source-stepping scale; `isrc` covers TRANOP
      — **measured: both already did**, by two different routes. `vsrc`'s forced
      value is scaled on its MNA force row (`device/analog/forces.rs`), `isrc`'s
      DC value through `$simparam("sourceScaleFactor")` in its own body. TRANOP
      runs as a DC-analysis solve (`load_dc` pins `Analysis::Dc`, and
      `DcSolver::set_analysis_kind` only tags margin evaluation), so `isrc`'s
      non-`tran` branch — the one carrying the scale — is what a transient's
      t = 0 operating point takes. No model edit was needed; the deliverable is
      the falsifiable proof the claim previously lacked (NGP-15)
- [x] Fixture is a bias circuit that **needs** source stepping to converge (plain Newton and gmin both fail) — otherwise the task proves nothing
- [x] Reverting makes the fixture fail to converge
- [x] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): honour the source-stepping scale in vsrc and isrc`

**Delivered**: two fixtures, not one — `homotopy_bias` (stiff ladder, falsifies
the `vsrc` half) and `homotopy_sink` (softer ladder + 25 mA sink, falsifies the
`isrc` half). Neither alone falsifies both: the two sources break the ramp in
different ways, and a ladder tuned to expose one survives the other's revert.
They live in their own target `tests/ngspice_homotopy.rs` because adding two
more JIT'd designs to `ngspice_validation.rs` tipped that binary over a
Cranelift code-region limit (`TryFromIntError`, the known MOS3 function-count
defect) in `ngspice_nmos3_id_vds_sweep`.

---

### T19: `urc` geometric taper

**What**: Replace the uniform ladder with ngspice's geometric taper (the `K` parameter) in the existing fixed-N modules.
**Where**: `crates/piperine-lang/headers/spice/urc.phdl`, `tests/ngspice/urc_taper.{cir,phdl}` (new)
**Depends on**: T9
**Reuses**: ngspice `urc/urcsetup.c`; the existing `urc_seg` submodule
**Requirement**: **NGP-16**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [x] Segment R/C follow the geometric taper, not a uniform split
- [x] New tapered fixture matches ngspice where a uniform ladder does not
- [x] The three existing `urc_lump2/5/10` checks still pass
- [x] Parity matrix: urc × DcOp stays `Covered`, and its ROADMAP ❌ is retired
- [x] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): taper urc segments geometrically like ngspice`

---

### T20: `urc` `ISPERL` diode-mode shunt path

**What**: Add the diode-mode shunt branch selected by `ISPERL`.
**Where**: `crates/piperine-lang/headers/spice/urc.phdl`, `tests/ngspice/urc_isperl.{cir,phdl}` (new)
**Depends on**: T19
**Reuses**: `urc_seg`; `spice::diode`
**Requirement**: **NGP-17**, NGP-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `ISPERL` set ⇒ diode shunt path exists; unset ⇒ capacitive path unchanged
- [ ] Fixture covers both
- [ ] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(spice): add the urc ISPERL diode-mode shunt path`

---

### T21: `gap-3` — expand array nets with a constant index

**What**: In the flattener's `remap`, expand an indexed `NetRef` whose index is an elaboration constant into its scalar net; a non-constant index still fails loud.
**Where**: `crates/piperine-lang/src/elab/lower/flatten.rs`
**Depends on**: T9
**Reuses**: The existing fail-loud path (`inline_indexed_netref_fails_as_gap3_deferred` becomes a *non-const* test)
**Requirement**: **NGP-20**, **NGP-21**

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `tap[i]` with a const `i` expands to the scalar net
- [ ] A non-const index still fails loud, naming the index expression
- [ ] Every existing flatten fail-loud test (dangling net, etc.) still passes unchanged
- [ ] Unit tests cover: const expands, non-const loud, out-of-range loud
- [ ] Gate check passes: `cargo test --workspace`
- [ ] Test count: ≥3 new unit tests; existing flatten tests unchanged

**Tests**: unit · **Gate**: full
**Commit**: `feat(lang): expand array nets with a constant index in flatten`

---

### T22: Re-author `urc` as a single generic `urc[N]`

**What**: Replace `urc2`/`urc5`/`urc10` with one `urc[N]` using `wire tap : Electrical[N+1]` and a `StructuralFor`; delete the hand-unrolled triple and the authoring-choice comment that explained it.
**Where**: `crates/piperine-lang/headers/spice/urc.phdl`
**Depends on**: T21
**Reuses**: T21's expansion; the existing `urc_seg`
**Requirement**: **NGP-18**, **NGP-22**

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] One generic `urc[N]` replaces the three fixed-N modules
- [ ] The three existing `urc_lump2/5/10` golden checks pass **with their `.cir` twins unedited**
- [ ] The stale `urc.phdl:10-23` gap-3 comment is removed (MD-35: no dead-architecture identifiers)
- [ ] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `refactor(spice): author urc generically now that gap-3 is closed`

---

### T23: `urc` `FMAX` → `N` auto-selection

**What**: Derive `N` from `FMAX` by ngspice's rule when `N` is not given.
**Where**: `crates/piperine-lang/headers/spice/urc.phdl`, `tests/ngspice/urc_fmax.{cir,phdl}` (new)
**Depends on**: T22
**Reuses**: ngspice `urcsetup.c`'s N-selection formula
**Requirement**: **NGP-19**

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `FMAX` given without `N` selects the same lump count ngspice does
- [ ] Explicit `N` still wins over `FMAX`
- [ ] Fixture compares against ngspice with `FMAX` set
- [ ] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(spice): auto-select urc lump count from FMAX`

---

### T24: Rewrite the ROADMAP model table from measured results

**What**: Replace the 2026-07-28 audit table with measured status; retire the stale `ic`/UIC blocker claim; point the BSIM rows at their own features.
**Where**: `ROADMAP.md`
**Depends on**: T23
**Reuses**: The parity matrix as the source of truth
**Requirement**: Success criteria

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every ✅ in the table corresponds to a `Covered` cell in the parity matrix
- [ ] The stale "blocked on `@initial` branch-forcing" claim is corrected
- [ ] BSIM rows link to `.specs/features/ngspice-bsim{3,4}/`
- [ ] `asrc`/`vbic` rows record the MD-39 rejection rather than sitting as backlog
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: none (docs) · **Gate**: build
**Commit**: `docs(roadmap): rewrite SPICE model completeness from measured parity`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6

Phase 1:  T1 ──→ T2 ──→ T3 ──→ T4
Phase 2:  T5 ──→ T6 ──→ T7 ──→ T8 ──→ T9 ──→ T10
Phase 3:  T11 ─→ T12 ─→ T13 ─→ T14 ─→ T15 ─→ T16 ─→ T17 ─→ T18
Phase 4:  T19 ─→ T20
Phase 5:  T21 ─→ T22 ─→ T23
Phase 6:  T24
```

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1 | 1 module move | ✅ Granular |
| T2 | 1 type (`WrData`) | ✅ Granular |
| T3 | 1 type (`ToleranceContract`) | ✅ Granular |
| T4 | 1 method | ✅ Granular |
| T5–T8 | 1 target + 1 fixture each | ✅ Granular |
| T9 | 1 guard target | ✅ Granular |
| T10 | 2 doc tables (cohesive) | ⚠️ OK — same change, two files |
| T11 | 1 model family, 1 concern (noise gm) | ✅ Granular |
| T12 | 1 model, 2 cohesive noise fixes | ⚠️ OK — same file, same concern |
| T13–T15 | 1 model, 1 concern each | ✅ Granular |
| T16 | 3 models, 1 mechanical wiring | ⚠️ OK — identical one-line change per model |
| T17 | 5 models, same wiring | ⚠️ OK — same, split from T16 to bound the diff |
| T18 | 2 sources, 1 concern | ⚠️ OK — cohesive |
| T19–T20 | 1 model, 1 concern each | ✅ Granular |
| T21 | 1 function (`remap`) | ✅ Granular |
| T22 | 1 file re-author | ✅ Granular |
| T23 | 1 parameter rule | ✅ Granular |
| T24 | 1 doc section | ✅ Granular |

No ❌ — no task spans multiple unrelated files or concerns.

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
| ---- | ----------------- | ------------- | ------ |
| T1 | None | (phase start) | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T1 | T2 → T3 (sequential within phase; T3's real dep is T1) | ✅ Match — intra-phase order is a superset of the dependency |
| T4 | T2 | T3 → T4 | ✅ Match — same |
| T5 | T3, T4 | T4 → T5 (phase boundary) | ✅ Match |
| T6 | T3, T4 | T5 → T6 | ✅ Match |
| T7 | T3, T4 | T6 → T7 | ✅ Match |
| T8 | T3, T4 | T7 → T8 | ✅ Match |
| T9 | T5, T6, T7, T8 | T8 → T9 | ✅ Match |
| T10 | T9 | T9 → T10 | ✅ Match |
| T11 | T9 | T10 → T11 (phase boundary) | ✅ Match |
| T12 | T11 | T11 → T12 | ✅ Match |
| T13 | T11 | T12 → T13 | ✅ Match |
| T14 | T11 | T13 → T14 | ✅ Match |
| T15 | T8 | T14 → T15 | ✅ Match — backward dep, earlier phase |
| T16 | T7 | T15 → T16 | ✅ Match — backward dep |
| T17 | T16 | T16 → T17 | ✅ Match |
| T18 | T9 | T17 → T18 | ✅ Match — backward dep |
| T19 | T9 | (phase 4 start) | ✅ Match |
| T20 | T19 | T19 → T20 | ✅ Match |
| T21 | T9 | (phase 5 start) | ✅ Match |
| T22 | T21 | T21 → T22 | ✅ Match |
| T23 | T22 | T22 → T23 | ✅ Match |
| T24 | T23 | (phase 6) | ✅ Match |

No task depends on a later phase. ✅

## Test Co-location Validation

| Task | Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | ---------------------- | --------------- | --------- | ------ |
| T1 | test-support module | unit | integration (existing, unchanged) | ✅ OK — a pure move; behaviour is covered by the 26 existing checks it must keep green |
| T2 | test-support module | unit | unit | ✅ OK |
| T3 | test-support module | unit | unit | ✅ OK |
| T4 | test-support module | unit | unit | ✅ OK |
| T5 | golden target | integration | integration | ✅ OK |
| T6 | golden target | integration | integration | ✅ OK |
| T7 | golden target | integration | integration | ✅ OK |
| T8 | golden target | integration | integration | ✅ OK |
| T9 | coverage gate | integration | integration | ✅ OK |
| T10 | docs | none | none | ✅ OK |
| T11 | PHDL model | integration | integration | ✅ OK |
| T12 | PHDL model | integration | integration | ✅ OK |
| T13 | PHDL model | integration | integration | ✅ OK |
| T14 | PHDL model | integration | integration | ✅ OK |
| T15 | PHDL model | integration | integration | ✅ OK |
| T16 | PHDL model | integration | integration | ✅ OK |
| T17 | PHDL model | integration | integration | ✅ OK |
| T18 | PHDL model | integration | integration | ✅ OK |
| T19 | PHDL model | integration | integration | ✅ OK |
| T20 | PHDL model | integration | integration | ✅ OK |
| T21 | flattener | unit | unit | ✅ OK |
| T22 | PHDL model | integration | integration | ✅ OK |
| T23 | PHDL model | integration | integration | ✅ OK |
| T24 | docs | none | none | ✅ OK |

No ❌ VIOLATION. No task defers its tests to another task.

---

## Requirement Coverage

| Requirement | Task(s) |
| ----------- | ------- |
| NGP-01 | T5 |
| NGP-02 | T6 |
| NGP-03 | T7 |
| NGP-04 | T8 |
| NGP-05 | T3, T5–T8 |
| NGP-06 | T5–T8 |
| NGP-07 | T3, T10 |
| NGP-08 | T11, T12 |
| NGP-09 | T13 |
| NGP-10 | T16, T17 |
| NGP-11 | T18 |
| NGP-12 | T14 |
| NGP-13 | T12 |
| NGP-14 | T15 |
| NGP-15 | T9 (the gate) + every fix task's revert check |
| NGP-16 | T19 |
| NGP-17 | T20 |
| NGP-18 | T22 |
| NGP-19 | T23 |
| NGP-20 | T21 |
| NGP-21 | T21 |
| NGP-22 | T22 |

**Coverage:** 22 requirements, 22 mapped, **0 unmapped**. ✅
