# p1-solver-complete Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** If the skill cannot be
activated, STOP. **Do not start until `api-crate` is DONE** (Verifier PASS) —
host surfaces (`run_sens`/`run_pss`) are born in `piperine-api`.

---

**Design**: `.specs/features/p1-solver-complete/design.md`
**Status**: In Progress — T1–T15 + T17 done; T16 BLOCKED (codegen gap, see
below); T18 next, 2026-07-18
**Baseline**: whatever `api-crate` closes at (≥449 passed / 5 ignored)

---

## Test Coverage Matrix

> Guidelines found: `CLAUDE.md` (zero warnings; always `--workspace`; ngspice
> harness lives on the root/api crate, live-or-SKIP).

| Code Layer | Test Type | Coverage Expectation | Location | Run Command |
|---|---|---|---|---|
| solver analyses (sens/pss/transient seam) | unit + integration | 1:1 to SC-01..07 ACs incl. loud-error paths; analytic references (divider, driven RC) | `piperine-solver/tests/` + unit in-module | `cargo test -p piperine-solver` |
| codegen operators | unit (kernel) + integration (circuit) | per-operator kernel test + circuit case per SC-08..12; Jacobian asserted where spec'd | `piperine-codegen/tests/analog_jit.rs`, `piperine-lang/tests/spec_simulation.rs` | `cargo test -p piperine-codegen -p piperine-lang` |
| digital integration | integration | bit-equality vs per-device path on every digital suite + examples 17–20 | `piperine-solver/tests/digital_topology.rs`, root run_examples | full |
| spice models | integration (golden) | new ngspice golden per model/region, live-or-SKIP | root/api `tests/ngspice*` | `cargo test -p piperine ngspice` |
| host/python surface | e2e | `run_sens`/`run_pss` parity Rust↔Python; docstrings (facade hygiene walk keeps passing) | `piperine-api/tests/`, python tests | full |
| hygiene refactors | none (behavior-preserving) | full suite green, byte-identical results | — | build |

## Gate Check Commands

| Gate | Command |
|---|---|
| Quick | `cargo test -p <crate>` |
| Full | `cargo test --workspace` |
| Build | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

---

## Execution Plan

```
Phase 1 (analyses):   T1 → T2 → T3 → T4 → T5 → T6
Phase 2 (operators):  T7 → T8 → T9 → T10 → T11
Phase 3 (digital):    T12
Phase 4 (models):     T13 → T14 → T15 → T16 → T17 → T18
Phase 5 (parity):     T19 → T20 → T21 → T22
Phase 6 (hygiene):    T23 → T24 → T25 → T26 → T27
```

Batching at Execute (~7/batch, whole phases): batch 1 = Phase 1 (6), batch 2
= Phases 2+3 (6), batch 3 = Phase 4 (6), batch 4 = Phases 5+6 (9 — fat but
one cohesive cleanup train; splitting 5/6 is acceptable if preferred).

---

## Task Breakdown

### T1: ✅ DONE — `.dc` host-proof tests (commit 2b0d846)
**What**: Extend `compile_once_sweep.rs`: nested two-param sweep (source ×
resistor) + source-only sweep; every point compared to an independent
fresh-build solve (exact voltage equality); compile count asserted = 1.
**Where**: root/api `tests/compile_once_sweep.rs`
**Depends on**: None · **Requirement**: SC-07
**Done when**: both cases green; equality exact; compile count 1; gate quick.
**Tests**: integration · **Gate**: quick
**Commit**: `test(api): nested + source sweeps prove host-level .dc`

### T2: ✅ DONE — Transient re-entry from arbitrary state (commit 99fd806)
**What**: `TransientSolver::with_initial_state(&[f64])` (+ digital snapshot
restore): start integration from a supplied full state vector instead of the
IC seed path. Standalone test: run RC 0→T, capture state, re-enter for T→2T,
result equals a single 0→2T run within reltol.
**Where**: `piperine-solver/src/solver/transient.rs`
**Depends on**: None · **Requirement**: SC-04 (enabler)
**Done when**: re-entry test green; existing transient suites untouched;
gate quick.
**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): transient re-entry from a supplied state`

### T3: ✅ DONE — `SensSolver` (FD central difference; SPEC_DEVIATION marked) (commit b2fdc60)
**What**: `analysis/sens.rs` options/result + `solver/sens.rs`: DC solve,
then per `(label,param)`: perturb via `set_element_param`, restamp-diff
`∂A/∂p, ∂b/∂p`, one LU-reused solve, `∂V(out)/∂p`. `Invalidation::Rebuild`
or unknown param → loud `SolverDomain::Element` error. Unit: divider
analytic `V·R1/(R1+R2)²` to 1e-6 rel; diode divider vs two-sided FD to 1e-3.
**Where**: `piperine-solver/src/{analysis,solver}/sens.rs`
**Depends on**: None · **Requirement**: SC-01, SC-02
**Done when**: analytic + FD cases green; loud-error test green; gate quick.
**Tests**: unit+integration · **Gate**: quick
**Commit**: `feat(solver): .sens DC sensitivity (FD direct method)`

### T4: ✅ DONE — sens host + python surface, uniform MD-22 (commit 9e1b0ee)
**What**: `piperine-api` `SimSession::run_sens` + result object;
`piperine-python` `module.sens(...)`/session binding with docstrings; part
VIII section. Rust↔Python value parity test.
**Where**: `piperine-api/`, `piperine-python/`, `docs/spec/`
**Depends on**: T3 · **Requirement**: SC-03
**Done when**: parity test green; facade hygiene walk still green; gate full.
**Tests**: e2e · **Gate**: full
**Commit**: `feat(api,python): sensitivity analysis surface`

### T5: ✅ DONE — `PssSolver` (single shooting + 2nd-period guard + k·T diagnostic) (commit d6f365d)
**What**: `analysis/pss.rs` (`PssAnalysisOptions {period, tstab,
max_shoot_iter, shoot_tol}`) + `solver/pss.rs`: optional tstab pre-roll,
shooting Newton on `g(x₀)=x(T)−x₀` (first Jacobian by FD columns, Broyden
updates after), inner runs via T2 re-entry. Shot state = analog + digital
nets + hidden banks (checkpoint/restore); Newton on continuous vars only;
digital periodicity is a post-convergence verification — mismatch → loud
`SolverDomain::Pss`, with the "period appears to be k·T" diagnostic
(k ≤ 4) for dividers. Tests: driven RC vs analytic phasor (1 %);
`|x(T)−x(0)| < shoot_tol`; period ≤ 0 loud; non-convergent case loud;
divider-by-2 case → k·T diagnostic.
**Where**: `piperine-solver/src/{analysis,solver}/pss.rs`, `error.rs`
**Depends on**: T2 · **Requirement**: SC-04, SC-05
**Done when**: all four tests green; gate quick.
**Tests**: unit+integration · **Gate**: quick
**Commit**: `feat(solver): periodic steady state via single shooting`

### T6: ✅ DONE — PSS host + python + rectifier validation + estimated_settle_time
**What**: `run_pss` in api + python (docstrings, part VIII); rectifier+RC
ripple vs settled-transient reference within 10·reltol; tstab equivalence
case (SC-06).
**Where**: `piperine-api/`, `piperine-python/`, solver tests
**Depends on**: T5 · **Requirement**: SC-06
**Done when**: rectifier + tstab cases green; parity Rust↔Python; gate full.
**Tests**: e2e · **Gate**: full
**Commit**: `feat(api,python): pss surface + rectifier validation`

### T7: ✅ DONE — `table` operator (commit fd2f83e)
**What**: Register `"table"` in `lower/pom/analog_ops.rs`; flatten/emit 1-D
linear interpolation with end clamp (read spec Part V §2 first — implement
the modes it defines); Jacobian = segment slope; non-monotonic xs / length
mismatch → loud codegen error. Kernel test (values + derivative) + circuit
case (table-driven resistor curve).
**Where**: `piperine-codegen/src/lower/pom/analog_ops.rs`, `jit/`, tests
**Depends on**: None · **Requirement**: SC-08
**Done when**: kernel + circuit + loud-error tests green; gate quick.
**Tests**: unit+integration · **Gate**: quick
**Commit**: `feat(codegen): table() 1-D interpolation operator`

### T8: ✅ DONE — `transition` operator (commit c66b2c7)
**What**: Companion using the runtime-operator state bank: state (start
value, target, t_change); linear ramp over rise/fall; breakpoints at ramp
start/end via `next_breakpoints`; rise/fall = 0 → instantaneous with
breakpoint; state survives rejected timesteps (commit/rollback path test).
**Where**: `jit/flatten.rs`, `device/analog.rs`, tests
**Depends on**: None · **Requirement**: SC-09
**Done when**: ramp-into-RC trace asserts the edge; rejected-step test
green; gate quick.
**Tests**: unit+integration · **Gate**: quick
**Commit**: `feat(codegen): transition() companion with breakpoints`

### T9: ✅ DONE — `idt` AC stamp (commit 6dedca1)
**What**: `load_ac` stamps `X/(jω)` for idt terms. Integrator circuit:
−20 dB/dec + −90° across 4 decades.
**Where**: `device/analog.rs`, `jit/analog.rs`, tests
**Depends on**: None · **Requirement**: SC-10
**Done when**: slope/phase asserted; DC/tran behavior unchanged; gate quick.
**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): idt 1/jω AC stamp`

### T10: ✅ DONE — Multiple `ac_stim` per contribution (commit 660af1c)
**What**: Sum stimulus terms in flatten (mag/phase as complex sum);
superposition test vs equivalent two-source circuit.
**Where**: `jit/flatten.rs`, `jit/analog.rs`, tests
**Depends on**: None · **Requirement**: SC-11
**Done when**: superposition equality; single-stim behavior unchanged; gate
quick.
**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): multiple ac_stim per contribution`

### T11: ✅ DONE — `@initial` branch force + enforced UIC hold (commit b9f47af)
**What**: Extend `FlatAnalog.initial_conditions` to branch constraints
(`V(a,b) <- ic` in `@initial` no longer errors); t=0 clamp branch (large-G
`G·(v−ic)`, ngspice CKTsetIC) released after the first accepted step.
Pre-charged cap discharge matches `5·e^(−t/RC)` within 10·reltol.
**Where**: `jit/flatten.rs`, `solver/transient.rs`, tests
**Depends on**: None · **Requirement**: SC-12
**Done when**: discharge case green; existing @initial seed cases green;
gate full.
**Tests**: integration · **Gate**: full
**Commit**: `feat(solver): @initial branch force + UIC hold clamp`

### T12: ✅ DONE — Fused digital network integration (commit 4272f61)
**What**: `core/circuit.rs`: detect pure-comb cones (`DigitalTopology`),
build `DigitalNetwork` elements, per-device fallback for clocked/
`SAMPLES_ANALOG`. Fusion-active assertion (instrumentation counter or
capability check) + bit-equality differential vs per-device path on
`digital_topology.rs`, `mixed_signal.rs`, cross-module NBA case, examples
17–20. Clocked fusing: only if the seam stays clean — else log follow-up in
ROADMAP (assumption in spec).
**Where**: `piperine-solver/src/core/circuit.rs`, tests
**Depends on**: None · **Requirement**: SC-13
**Done when**: fusion proven active; all digital suites bit-identical; gate
full.
**Tests**: integration · **Gate**: full
**Commit**: `feat(solver): fused combinational digital network active`

### T13: ✅ DONE — MOS level 2
**What**: Port ngspice `mos2` load equations to `headers/spice/mos.phdl`
(new module, shared helpers with mos1 where clean); golden DC cases per
region (cutoff/linear/sat) in the ngspice harness, live-or-SKIP.
**Where**: `piperine-lang/headers/spice/mos.phdl`, `tests/ngspice*`
**Depends on**: None · **Requirement**: SC-14
**Done when**: goldens within harness tolerance (live run recorded); gate
full.
**Tests**: integration (golden) · **Gate**: full
**Commit**: `feat(spice): MOS level 2`

### T14: ✅ DONE — MOS level 3
**What**: Same shape as T13 for `mos3` (empirical short-channel).
**Where**: same
**Depends on**: T13 · **Requirement**: SC-14
**Done when**: goldens green; gate full.
**Tests**: integration (golden) · **Gate**: full
**Commit**: `feat(spice): MOS level 3`

### T15: ✅ DONE — Ideal lossless tline
**What**: `headers/spice/tline.phdl`: Branin model over the `delay` runtime
operator (two internal controlled sources, delayed cross-terms). Tests:
matched termination (< 1 % reflection), open termination (doubling at 2·td),
td/Z0 ≤ 0 loud.
**Where**: `piperine-lang/headers/spice/tline.phdl`, tests + ngspice golden
**Depends on**: None · **Requirement**: SC-15
**Done when**: three cases green; gate full.
**Tests**: integration (golden) · **Gate**: full
**Commit**: `feat(spice): ideal transmission line`

### T16: ⛔ BLOCKED — `urc` lumped RC line
**What**: RC-ladder expansion module (`param n`, geometric segmenting per
ngspice); step-response delay/rise vs ngspice golden.
**Where**: `headers/spice/tline.phdl` (same file), tests
**Depends on**: T15 · **Requirement**: SC-16
**Done when**: golden green; bad-params loud; gate full.
**Tests**: integration (golden) · **Gate**: full
**Commit**: `feat(spice): urc lumped RC line`

**BLOCKER (2026-07-18):** a parametric `urc[N]` needs codegen support that
does not exist. Three distinct gaps found while attempting it:
1. **No hierarchy flattening** — codegen (`device/circuit.rs:374`) rejects a
   submodule that itself instantiates devices ("nested hierarchy … flatten
   during elaboration"); only single-level (top → leaf) designs compile. A
   structural RC-ladder submodule cannot be simulated. `RcChain[N]` in
   `language_features.phdl` has the same latent problem.
2. **Const-args not substituted into analog behaviors** — a generic module's
   `analog` body sees `N` as undefined (`AttachBehaviors` clones the base
   behavior unsubstituted). A contained fix exists (subst const params per
   monomorphized variant via `Stmt::subst_const`) but is unused without gap 3.
3. **Array wires are not flat analog nets** — `wire node : Electrical[N+1]`
   stays one array-typed net; `node[i]` in `V()/I()` cannot resolve (the node
   table has no `node[0]`). Needs array-net expansion + index→net mapping in
   the flattener.
Each is a real codegen feature, not a stdlib-model task. **Recommend a
separate `codegen-parametric-devices` feature** (flatten pass OR array-node
support + the const-arg-into-behavior fix). See [[urc-codegen-gaps]].

### T17: ✅ DONE — Transformer block
**What**: `xfmr(l1, l2, k)` combined two-winding device in
`headers/spice/passives.phdl` over the mutual-flux engine. AC ratio ≈
`k·√(L2/L1)`; coupled-LC energy-transfer regression stays green.
**Where**: `headers/spice/passives.phdl`, tests
**Depends on**: None · **Requirement**: SC-17
**Done when**: ratio + regression green; gate full.
**Tests**: integration · **Gate**: full
**Commit**: `feat(spice): xfmr combined transformer`

### T18: ✅ DONE — Stdlib off sentinel params
**What**: Migrate `1e99`/`$param_given` sentinel encodings to `T?` +
`.get_or` across `headers/spice/`; behavior unchanged (all model suites +
goldens green; grep-clean for the sentinel patterns).
**Where**: `piperine-lang/headers/spice/*.phdl`
**Depends on**: T13, T14, T15, T16, T17 · **Requirement**: SC-18
**Done when**: grep-clean; full model suites green; gate full.
**Tests**: integration (existing suites) · **Gate**: full
**Commit**: `refactor(spice): optional params replace sentinels`

### T19: fetlim / limvds
**What**: Port `DEVfetlim`/`DEVlimvds` formulas into `emit_analog_limit`
(same slot machinery as pnjlim); unit tests against reference values
computed from the ngspice C source; MOS validation stays green.
**Where**: `codegen/src/codegen/analog_emit.rs`, tests
**Depends on**: None · **Requirement**: SC-19
**Done when**: formula unit tests + MOS goldens green; gate full.
**Tests**: unit+integration · **Gate**: full
**Commit**: `feat(codegen): real fetlim/limvds limiters`

### T20: Temperature uniformity + `.temp` sweep
**What**: Audit `tnom` rescaling per stdlib model (fix inconsistencies);
host-level `.temp` sweep test — diode forward drop ≈ −2 mV/°C.
**Where**: `headers/spice/`, api tests
**Depends on**: None · **Requirement**: SC-20
**Done when**: sweep test green; audit findings fixed or logged; gate full.
**Tests**: integration · **Gate**: full
**Commit**: `fix(spice): uniform temperature flow + .temp sweep proof`

### T21: Inductor TR-stage dual
**What**: Previous-voltage-tracking dual form for the TR stage flux
companion; coupled-LC + RL corners unchanged or tighter.
**Where**: `device/analog.rs`
**Depends on**: None · **Requirement**: SC-21
**Done when**: regressions green (bounds not loosened); gate quick.
**Tests**: integration · **Gate**: quick
**Commit**: `fix(solver): TR-stage flux dual form`

### T22: Remove `IntegrationMethod`
**What**: Delete the enum + `suggest_transient_step` `method` param;
migrate ~34 references (TR-BDF2 hardwired). Mechanical; zero behavior
change.
**Where**: `piperine-solver/src/` (math/solver), `codegen/device/`
**Depends on**: T21 · **Requirement**: SC-22
**Done when**: grep-clean `IntegrationMethod`; full suite green; gate build.
**Tests**: none (behavior-preserving) · **Gate**: build
**Commit**: `refactor(solver)!: TR-BDF2 is the only integration scheme`

### T23: Scheduler split
**What**: `digital/scheduler.rs` → `digital/{topology,state,scheduler}.rs`;
explicit re-exports; no public path breakage beyond module moves.
**Where**: `piperine-solver/src/digital/`
**Depends on**: T12 · **Requirement**: SC-23
**Done when**: full suite green; gate build.
**Tests**: none · **Gate**: build
**Commit**: `refactor(solver): scheduler split into topology/state/scheduler`

### T24: `SignalBridge` extraction
**What**: Extract the three jobs of
`CircuitInstance::accept_and_run_digital` into an internal `core/bridge.rs`
component; behavior byte-identical.
**Where**: `piperine-solver/src/core/`
**Depends on**: T23 · **Requirement**: SC-23
**Done when**: full suite green; gate build.
**Tests**: none · **Gate**: build
**Commit**: `refactor(solver): SignalBridge owns the mixed-signal handoff`

### T25: `as_iv` + shared `Integrator` + `init_global`
**What**: Re-home `DcAnalysisResult::as_iv`; noise trapezoid through a
shared `Integrator`; `Context::default` stops calling global init (first
solver build owns it). Identical numeric results.
**Where**: `piperine-solver/src/{analysis,math,solver}/`
**Depends on**: None · **Requirement**: SC-24, SC-26
**Done when**: noise results identical; init test (no global side effect on
`Context::default()`); gate build.
**Tests**: unit · **Gate**: build
**Commit**: `refactor(solver): analysis-layer seams (as_iv, Integrator, init)`

### T26: `Trace.i` state/var recording (opt-in)
**What**: Opt-in per-step state/var bank recording in
`TransientAnalysisResult` (off by default); `Trace.i` on state-reading
devices works when enabled, keeps the loud error when disabled.
**Where**: `piperine-solver/src/analysis/`, `piperine-api/src/waveform.rs`
**Depends on**: None · **Requirement**: SC-25
**Done when**: both paths tested (enabled = current values; disabled = loud);
gate full.
**Tests**: integration · **Gate**: full
**Commit**: `feat(api): opt-in state recording unlocks Trace.i on stateful devices`

### T27: Docs + ROADMAP closure
**What**: Part VII/VIII sections for sens/PSS/new operators/models; ROADMAP
P1 checkboxes closed or moved to named backlog lines (laplace/zi, LTRA,
autonomous PSS, AC sens, clocked fusing if deferred); traceability →
Verified.
**Where**: `docs/spec/`, `ROADMAP.md`, `.specs/`
**Depends on**: T1–T26 · **Requirement**: closure
**Done when**: gate build (zero warnings, full green, ngspice live).
**Tests**: none · **Gate**: build
**Commit**: `docs: solver P1 complete`

---

## Phase Execution Map

```
Phase 1: T1 → T2 → T3 → T4 → T5 → T6
Phase 2: T7 → T8 → T9 → T10 → T11
Phase 3: T12
Phase 4: T13 → T14 → T15 → T16 → T17 → T18
Phase 5: T19 → T20 → T21 → T22
Phase 6: T23 → T24 → T25 → T26 → T27
```

27 tasks → ~4 batches at Execute (sub-agent offer applies).

## Diagram-Definition Cross-Check

| Task | Depends (body) | Diagram | Status |
|---|---|---|---|
| T1 none · T2 none · T3 none · T4 T3 · T5 T2 · T6 T5 | backward/within phase | Phase 1 chain | ✅ |
| T7–T10 none · T11 none | independent, ordered | Phase 2 chain | ✅ |
| T12 none | — | Phase 3 | ✅ |
| T13 none · T14 T13 · T15 none · T16 T15 · T17 none · T18 T13–T17 | backward-only | Phase 4 chain | ✅ |
| T19 none · T20 none · T21 none · T22 T21 | backward-only | Phase 5 chain | ✅ |
| T23 T12 · T24 T23 · T25 none · T26 none · T27 all | backward-only | Phase 6 chain | ✅ |

## Test Co-location Validation

| Task | Layer | Matrix | Task Says | Status |
|---|---|---|---|---|
| T1 host tests | integration | integration | ✅ |
| T2/T3/T5 solver | unit+integration | unit+integration | ✅ |
| T4/T6 host+python | e2e | e2e | ✅ |
| T7–T11 operators | unit+integration | unit+integration | ✅ |
| T12 digital | integration | integration | ✅ |
| T13–T17 models | integration golden | integration | ✅ |
| T18 stdlib refactor | existing suites | integration | ✅ |
| T19 limiters | unit+integration | unit+integration | ✅ |
| T20/T26 | integration | integration | ✅ |
| T21 | integration | integration | ✅ |
| T22–T25 hygiene | none (behavior-preserving) | none | ✅ |
| T27 docs | none | none | ✅ |
