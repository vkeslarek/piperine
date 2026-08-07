# ngspice-parity Specification

**Reference:** ngspice C sources at `~/Git/ngspice/src/spicelib/devices/`.
**Piperine models:** `crates/piperine-lang/headers/spice/` (4190 lines, 13 models).
**Supersedes:** ROADMAP.md "SPICE model completeness" (audited 2026-07-28).

## Problem Statement

Piperine ships 13 ngspice-faithful device models, but the *faithfulness claim
is largely unverifiable*. The cross-validation harness
(`tests/ngspice_validation.rs`, 26 golden checks) exercises **DC operating
points, DC sweeps, `.disto`, `.four` and `.pz` only** — it never runs AC,
`.noise`, `.tran`, or a temperature sweep against ngspice. Every "✅ exact
noise" and "✅ full tlev/tlevc" row in the ROADMAP table is therefore an
assertion no test can falsify.

Meanwhile the same audit names concrete, known-wrong behaviour in exactly the
domains the harness does not cover: MOSFET/JFET channel noise uses a
saturation-only `gm` estimate, the switch has no thermal noise at all, `ic`/UIC
is declared but dead on six models, and `urc` is a uniform ladder where ngspice
tapers geometrically.

Refining parity without first extending the oracle would trade one set of
unverified claims for another. This feature extends the oracle, then closes the
deviations the oracle measures across the 13 shipped models — leaving a proven
foundation the BSIM ports are specified against separately.

**This feature adds no new device.** `asrc` and `vbic` were considered and
rejected on merit (see Out of Scope); BSIM is split into its own features. The
scope is: make parity measurable, then make it true.

## Goals

- [ ] Every parity claim in the ROADMAP model table is backed by a golden test
      that fails when the claim breaks — no unfalsifiable ✅.
- [ ] Cross-validation covers AC, `.noise`, `.tran` and temperature, each with
      an explicit, documented numeric tolerance contract.
- [ ] The named deviations in the 13 existing models are closed, or recorded
      as a deliberate documented deviation with a reason.
- [ ] `urc` is faithful to ngspice's geometric taper.
- [ ] `gap-3` is closed, so a model can be authored once generically instead of
      hand-unrolled per lump count — unblocking the BSIM follow-on features.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Legacy BSIM versions (`bsim3v0`/`v1`/`v32`, `bsim4v5`/`v6`/`v7`, `bsim1`, `bsim2`) | ngspice keeps them only for netlist back-compat against pinned model cards. The current `bsim3`/`bsim4` directories are the live models; porting six frozen forks multiplies the work without adding capability. |
| SOI family (`bsimsoi`, `bsim3soi_*`, `soi3`) | Distinct device physics (buried oxide, floating body), not a version of the bulk model. Own feature. |
| Surface-potential MOS (`hisim2`, `hisimhv1`), `mos6`, `mos9` | ROADMAP marks Low priority: niche academic/foundry models. |
| Numerical device solvers (`numd`, `numd2`, `numos`, `nbjt`, `nbjt2`) | 1D Poisson solvers — a different engine, not a compact model. |
| `adms` (Verilog-A loader), `ndev` (shared-lib C device) | Served by the external OSDI/plugin path (`piperine-osdi`), which is deliberately out of the solver core. |
| Compound-semiconductor FETs (`mes`, `mesa`, `hfet1`, `hfet2`, `jfet2`) | ROADMAP Medium priority; no measured user demand. Deferred pending the P1 oracle proving the porting method scales. |
| `asrc` (the behavioural `B` element) | **Redundant by construction.** The `B` element exists because a SPICE netlist has no expression language; PHDL does — `V(p,n) <+ <expr>` in an `analog` block *is* the behavioural source, differentiated symbolically like any other contribution. An imported deck's `B` line is transcribed into a PHDL body by the SPICE→PHDL importer (Part VI §9), so nothing is blocked. Porting it as a native device would reimplement the language's own premise as a model. |
| `vbic` | Foundry-grade RF BJT. Outside the ROADMAP north star (hobbyists, independent professionals, small teams without Cadence-class tooling); the Gummel-Poon `bjt` serves that user. Revisit only on measured demand. |
| `ltra` / `txl` / `cpl` (lossy + coupled lines) | ROADMAP Medium; `urc` covers the practical distributed-RC case, which P3 makes faithful. Own feature if scheduled. |
| `.disto` on MOS2/MOS3 (Cranelift function-count explosion) | Separate open P1-backlog defect with its own fix (indexed function instead of ordered cross-product). Parity work must not absorb it. |
| Rewriting the solver's numerical algorithms | Parity is a *model* concern. A deviation traced to the solver becomes a named defect report, not a fix inside this feature. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Scope of "new devices" | **None.** `asrc` and `vbic` are out of scope on merit (see Out of Scope), not deferred for size; BSIM is split into its own features. This feature touches only the 13 existing models plus the `gap-3` prerequisite. | Resolved in review: `asrc` duplicates PHDL's own behavioural capability, `vbic` sits outside the project's stated user. | **y** |
| BSIM belongs in separate features | `ngspice-bsim3`, then `ngspice-bsim4` | Measured size (34k LOC C, bsim4 alone > mos1+mos2+mos3 combined) and coupling: keeping them here would hold P1's oracle and P2's fixes hostage to the two largest ports. | **y** |
| BSIM port targets (for those features) | `bsim3` (BSIM3v3.3) and `bsim4` (BSIM4.8) — ngspice's current directories | These are the live models; the `vN` directories are pinned legacy forks (see Out of Scope). | n |
| `gap-3` (array-net → flat-net expansion) is a prerequisite for **BSIM** and for `urc`'s generic re-authoring — but **not** for the `urc` taper | Delivered as P4, before the BSIM features; P3's taper and `ISPERL` ship without it | The blocker is the array net (`wire tap : Electrical[N+1]` indexed as `tap[i]`), not the `[N]` const-param: `remap` fails loud on an indexed `NetRef`. A taper changes segment *values*, so it fits the existing fixed-N modules. Corrected after review — the first draft over-coupled all of P3 to the prerequisite. | n |
| ROADMAP's "`ic`/UIC blocked on `@initial` branch-forcing in the JIT" is **stale** | Treat UIC wiring as unblocked ordinary work | The mechanism landed: `flatten/analog.rs:876` collects `@initial { V(p,n) <- ic; }` as UIC seeds and `analyses/transient.rs:228` implements the hold clamps. The models were simply never wired to it. | n |
| ngspice binary version used as the golden reference | Whatever is on `PATH`, with the version recorded in the failure message | Matches the existing harness contract (absent binary ⇒ SKIP). Pinning a version is a CI concern, not a spec concern. | n |
| A measured deviation that traces to ngspice being *wrong* (not piperine) | Documented as a deliberate deviation with the physics reason; the test asserts piperine's value | ngspice carries known historical bugs (e.g. `badMos3`); blind bug-for-bug matching is not the goal. Fidelity means matching the model, not the defect. | n |

**Open questions:** none. The two scope questions (which new devices; where
BSIM lives) were resolved in review and are marked confirmed above; every
remaining item is logged with a chosen default and rationale.

---

## User Stories

### P1: Measured parity — extend the golden oracle ⭐ MVP

**User Story**: As a device-model author, I want the ngspice cross-check to
cover AC, noise, transient and temperature so that a parity claim is something
a test can falsify rather than something a table asserts.

**Why P1**: Every later story depends on this. Without it, "we fixed the noise
model" is unprovable, and the existing ✅ rows stay unfalsifiable.

**Acceptance Criteria**:

1. WHEN the harness runs an AC fixture THEN it SHALL compare piperine and
   ngspice **per frequency point**, on both magnitude and phase, and fail
   naming the node, frequency, both values and the delta.
2. WHEN the harness runs a `.noise` fixture THEN it SHALL compare output noise
   PSD per frequency point **and** the integrated RMS, failing with the same
   named detail.
3. WHEN the harness runs a `.tran` fixture THEN it SHALL compare node voltages
   on a **common time grid** (interpolating piperine's adaptive grid onto
   ngspice's print points, never comparing raw adaptive samples).
4. WHEN the harness runs a fixture at a non-nominal temperature THEN it SHALL
   compare the resulting operating point, exercising each model's temperature
   path.
5. WHEN a comparison exceeds its domain's declared tolerance THEN the test
   SHALL fail loud naming model, domain, coordinate and both values — never a
   silent pass and never a widened tolerance.
6. WHEN `ngspice` is absent from `PATH` THEN every new golden test SHALL take
   the existing SKIP branch, exactly as the DC checks already do.
7. WHEN the tolerance contract is read THEN each domain (DC, AC magnitude, AC
   phase, noise PSD, transient, temperature) SHALL have an explicit documented
   value with a stated rationale — no undocumented constant in a test body.

**Independent Test**: Add one AC fixture for an RC lowpass whose analytic
response is known; the test passes against ngspice and fails when the tolerance
is tightened past the true agreement.

---

### P2: Close the measured deviations in existing models

**User Story**: As a circuit designer, I want the shipped models to agree with
ngspice in AC, noise and temperature so that a simulation I trust for DC is
equally trustworthy elsewhere.

**Why P2**: This is the feature's actual payload; P1 only makes it verifiable.

**Acceptance Criteria**:

1. WHEN a MOSFET or JFET channel-noise PSD is computed THEN it SHALL use the
   model's **analytic `gm`**, not the saturation-only `gm_est`, and SHALL match
   ngspice in the linear and transition regions, not only in saturation.
2. WHEN a closed switch (`sw`/`csw`) is in an operating point THEN it SHALL
   contribute ON-state thermal noise matching `swnoise.c`.
3. WHEN a `.tran` runs with UIC and a model declares `ic` THEN the declared
   initial condition SHALL be enforced (cap, ind, diode, bjt, jfet, mos1/2/3),
   matching ngspice's `CKTsetIC` behaviour.
4. WHEN a DC solve uses source stepping THEN `vsrc` SHALL honour the
   source-stepping scale (`CKTsrcFact`), and `isrc` SHALL honour it in the
   TRANOP case.
5. WHEN a MOSFET's flicker noise is computed with `cox = 0` THEN it SHALL
   substitute ngspice's `1e-7` rather than dividing by zero.
6. WHEN a JFET's noise is requested with `nlev = 3` THEN it SHALL compute the
   `nlev=3` form rather than silently using another level.
7. WHEN a BJT is evaluated at a non-nominal temperature THEN `VAF`/`VAR` SHALL
   follow their temperature dependence rather than being pinned to identity.
8. WHEN a deviation is closed THEN a golden test from P1 SHALL cover it, and
   that test SHALL fail if the fix is reverted.

**Independent Test**: Revert the `gm_est` → analytic `gm` change and watch the
linear-region noise fixture fail; restore and watch it pass.

---

### P3: `urc` faithful to ngspice's geometric taper

**User Story**: As a designer modelling a distributed RC line, I want `urc` to
taper the way ngspice does so that the lumped approximation converges to the
same answer.

**Why P3**: Isolated and currently marked ❌ NOT faithful. Only its *last*
criterion depends on P4 — see the split below.

**What does and does not need `gap-3`.** The blocker is **not** the `[N]`
const-parameter (monomorphizing `mod urc[N]` works exactly as `mod Dac[N]`
does). It is the array net the natural shape needs alongside it: `wire tap :
Electrical[N+1]` indexed as `tap[i]` reaches `FlattenHierarchy::remap` as an
indexed `NetRef`, and array-net → flat-net expansion is gap-3, so `remap` fails
loud rather than dropping the index silently (`flatten.rs`
`inline_indexed_netref_fails_as_gap3_deferred`; the choice is documented at
`urc.phdl:10-23`). Consequently:

- The **geometric taper** changes per-segment R/C *values*, not topology — it
  lands on today's `urc2`/`urc5`/`urc10` with no language change.
- **`ISPERL`** adds a shunt branch inside the existing `urc_seg` — likewise
  independent.
- Only **`FMAX` → `N` auto-selection** genuinely needs a generic `urc[N]`,
  because there `N` becomes computed rather than chosen.

**Acceptance Criteria**:

1. WHEN `urc` is elaborated THEN segment values SHALL follow ngspice's
   **geometric taper** (the `K` parameter), not a uniform ladder. *(No `gap-3`
   dependency.)*
2. WHEN `ISPERL` is set THEN the diode-mode shunt path SHALL exist. *(No
   `gap-3` dependency.)*
3. WHEN `gap-3` (P4) has landed THEN `urc` SHALL be re-authored as a single
   generic `urc[N]`, replacing the hand-written `urc2`/`urc5`/`urc10` triple,
   with the existing golden checks unchanged and still passing.
4. WHEN the generic form exists AND `urc` is given `FMAX` THEN it SHALL
   auto-select `N` by ngspice's rule rather than requiring a pre-chosen lump
   count. *(Depends on AC-3.)*

**Independent Test**: The existing `urc_lump2/5/10` golden checks keep passing,
and a new tapered fixture matches ngspice where the uniform ladder does not.

---

### P4: `gap-3` — array-net expansion (prerequisite)

**User Story**: As a model author, I want `wire tap : Electrical[N]` with
`tap[i]` to elaborate so that I can write one generic model instead of
hand-unrolled copies.

**Why P4**: A hard prerequisite for P3's generic re-authoring and for the BSIM
follow-on features' multi-segment structures. Currently `flatten.rs` fails loud
on an indexed `NetRef`.

**Acceptance Criteria**:

1. WHEN a module declares an array net and indexes it with an
   elaboration-constant index THEN flattening SHALL expand it to scalar nets
   rather than failing loud.
2. WHEN the index is not an elaboration constant THEN it SHALL still fail loud,
   naming the index expression.
3. WHEN `gap-3` lands THEN the existing hand-unrolled `urc2`/`urc5`/`urc10`
   SHALL be replaceable by `urc[N]` with the golden checks unchanged and still
   passing (this is P3's NGP-18/19).

**Independent Test**: Re-author `urc` generically; the three existing lump
golden checks pass without edits to their `.cir` counterparts.

---

---

## Follow-on features (split out of this spec)

BSIM was originally drafted here as two further priorities and was **split into its own
features** during specification. The reason is size and coupling, both
measured:

| Port | ngspice LOC | Comparison |
| ---- | ----------- | ---------- |
| `bsim3` (BSIM3v3.3) | 11,442 | ≈ mos1 (7,017) + dio (3,504) |
| `bsim4` (BSIM4.8) | 22,564 | **larger than mos1+mos2+mos3 combined** (23,272 — which was itself a whole feature) |
| both | 34,006 | vs 37,925 for *every* model already ported |

Keeping them inside `ngspice-parity` would mean the feature does not close
until both ports close, holding P1's oracle and P2's fixes — the parts with
immediate value — hostage to the two largest items.

| Feature | Depends on | Notes |
| ------- | ---------- | ----- |
| `ngspice-bsim3` | this feature's **P1** (measured oracle) and **P4** (`gap-3`) | Port first; it proves the method and the internal-node authoring route. |
| `ngspice-bsim4` | `ngspice-bsim3` | Reuses the proven method; largest single item in the backlog. |

Both are **opened as scaffolds** — `.specs/features/ngspice-bsim3/spec.md` and
`.specs/features/ngspice-bsim4/spec.md` — carrying the measurements taken here,
the inherited scope decisions, and an explicit list of open questions. Each is
marked `NOT YET SPECIFIED`: no user stories, no acceptance criteria, no
requirement IDs until refined. They record what is known so it is not
re-derived; they do not design by anticipation.

---

## Edge Cases

- WHEN `ngspice` is absent THEN every golden test SKIPs, and the suite still
  passes (the binary is never a hard dependency).
- WHEN ngspice's output is unparseable THEN the harness fails loud rather than
  reporting zero compared nodes as success.
- WHEN a fixture shares zero nodes between the two runs THEN it is a contract
  violation, not a vacuous pass (the existing rule, extended to every new
  domain).
- WHEN piperine fails to converge on a parity fixture THEN the failure names
  the analysis and the fixture — it is never reported as a tolerance miss.
- WHEN a noise PSD is compared near zero THEN the comparison SHALL use the
  domain's absolute floor rather than a relative test that divides by ~0.
- WHEN ngspice and piperine disagree because ngspice carries a known defect
  THEN the deviation is documented with its reason and the test asserts
  piperine's value.
- WHEN a model parameter exists in ngspice but has no PHDL counterpart THEN
  that absence SHALL be visible (a named gap), not silently accepted.

---

## Implicit-Requirement Dimensions Sweep

| Dimension | Resolution |
| --------- | ---------- |
| Input validation & bounds | Covered: ngspice's own clamps are explicit ACs — `cox = 0` → `1e-7` (NGP-12), and the `limexp`/`vcrit` junction limiters are already exercised by the existing DC corpus; an out-of-range array index fails loud (NGP-21). |
| Failure / partial-failure states | Covered: non-convergence on a fixture is a named failure, distinct from a tolerance miss (Edge Cases). |
| Idempotency / retry / duplicate handling | **N/A** — the harness is a deterministic pure comparison; there is no mutable state to retry against and no dedup key. |
| Auth boundaries & rate limits | **N/A** — no network, no multi-user surface; ngspice runs as a local subprocess. |
| Concurrency / ordering | Covered: golden tests spawn ngspice subprocesses and may run in parallel under the test harness; fixtures must not share output paths. |
| Data lifecycle / expiry | **N/A** — `tests/ngspice/` is a frozen corpus (CLAUDE.md "files not to edit casually"); fixtures are added, never expired. |
| Observability | Covered: P1-05 requires every failure to name model, domain, coordinate and both values — the diagnostic *is* the deliverable. |
| External-dependency failure | Covered: ngspice absent ⇒ SKIP (P1-06); unparseable output ⇒ loud (Edge Cases); version recorded in the failure message (Assumptions). |
| State-transition integrity | Covered by proxy: MOSFET/JFET region transitions are exactly where `gm_est` is wrong (P2-01); the linear/transition/saturation crossing is an explicit test target. |

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| NGP-01 | P1: AC per-point magnitude + phase | P1 | Verified (T5, `1221978`) |
| NGP-02 | P1: noise PSD per point + integrated RMS | P1 | Verified (T6, `7ce96f6`) |
| NGP-03 | P1: transient on a common time grid | P1 | Verified (T7, `a0a1900`) |
| NGP-04 | P1: temperature-sweep operating point | P1 | Verified (T8, `e1b8429`) |
| NGP-05 | P1: loud, fully-named tolerance failure | P1 | Verified (T3 + T5–T8; rationale surfaced via `ToleranceContract::rationale()`) |
| NGP-06 | P1: ngspice-absent SKIP branch preserved | P1 | Verified (T5–T8 — each domain target takes the existing SKIP seam) |
| NGP-07 | P1: documented per-domain tolerance contract | P1 | Verified (T3, `46620e2`; T10 docs) |
| NGP-08 | P2: analytic `gm` in channel noise | P2 | Verified (T11 mos `c39d35c`; T12 jfet `4868c91`) |
| NGP-09 | P2: switch ON-state thermal noise | P2 | Verified (T13, `1ac5fdc`) |
| NGP-10 | P2: `ic`/UIC enforced on six models | Design | Verified (cap, diode — T16; bjt, jfet, mos1/2/3 — T17). `ind` is a documented deviation: ngspice's `ind.IC` is a **current** seed (`indload.c` flux form `L·I`), but `collect_initial_conditions` only collects `Potential` seeds — a current-seed mechanism is a separate codegen feature. A second narrower deviation: mos/bjt/jfet ICs pin the external node (piperine `@initial`), whereas ngspice seeds the device's internal state during UIC init — the two agree with no series resistance (`uic_mos`'s case) and diverge only when RD/RS are nonzero. |
| NGP-11 | P2: `CKTsrcFact` on vsrc + isrc TRANOP | P2 | Verified (T18, `bdf279c` — measured: both already did; delivered falsifiable fixtures `homotopy_bias`/`homotopy_sink`) |
| NGP-12 | P2: flicker noise `cox = 0` substitution | P2 | Verified (T14, `c2b4d9f`) |
| NGP-13 | P2: JFET `nlev = 3` noise | P2 | Verified (T12, `4868c91`) |
| NGP-14 | P2: BJT `VAF`/`VAR` temperature dependence | P2 | Verified (T15, `2eb614d`) |
| NGP-15 | P2: every closed deviation covered by a golden test | Design | Verified — T11..T20 + T16/T17 each ship a golden fixture (`noise_mos_linear`, `noise_jfet`, `noise_switch`, `noise_mos_cox0`, `temp_bjt`, `uic_rc`, `uic_mos`, `homotopy_bias`/`homotopy_sink`, `urc_taper`, `urc_isperl`) that fails when the fix is reverted; the parity-matrix gate (`tests/ngspice_parity_matrix.rs`) is the MD-31 falsifiability guard. |
| NGP-16 | P3: `urc` geometric taper (`K`) — no `gap-3` dependency | - | Verified |
| NGP-17 | P3: `urc` `ISPERL` diode shunt path — no `gap-3` dependency | - | Verified |
| NGP-18 | P3: `urc` re-authored generically after P4 | - | Verified |
| NGP-19 | P3: `urc` `FMAX` → `N` auto-selection (depends on NGP-18) | - | Verified |
| NGP-20 | P4: array-net expansion with const index | - | Verified |
| NGP-21 | P4: non-const index still fails loud | - | Verified |
| NGP-22 | P4: `urc[N]` replaces the hand-unrolled triple | - | Verified |

**Coverage:** 22 total, 22 mapped to tasks, 0 unmapped. ✅ All verified or documented-deviation. NGP-10 carries two documented deviations (`ind` current-seed; mos/bjt/jfet external-node vs device-state-seed) — both recorded with reasons in the relevant model headers, `ROADMAP.md`, and the validation report.

---

## Success Criteria

- [ ] No row in the ROADMAP model table carries a ✅ that no test exercises.
- [ ] The golden corpus covers all six domains (DC, AC, noise, transient,
      temperature, spectral) across the 13 existing models.
- [ ] Every deviation named in the 2026-07-28 audit is either closed or
      recorded as a deliberate documented deviation with its reason.
- [ ] `urc` matches ngspice on a tapered fixture where the uniform ladder does
      not.
- [ ] `urc[N]` exists generically; the three hand-unrolled modules are gone and
      their golden checks still pass unedited.
- [ ] ROADMAP's "SPICE model completeness" section is rewritten from measured
      results, and its stale `ic`/UIC blocker claim is corrected.
- [ ] `cargo test --workspace` green with zero rustc warnings; the parity
      baselines that existed before this feature remain bit-identical.
