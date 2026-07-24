# Host Library Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement with the `tlc-spec-driven` skill: activate it by name and follow its
Execute flow and Critical Rules (per-task cycle, sub-agent delegation offer,
adequacy review, Verifier, discrimination sensor). **If the skill cannot be
activated, STOP and tell the user.**

**Design**: `.specs/features/host-library/design.md`
**Spec**: `.specs/features/host-library/spec.md`
**North star / gap**: `ideal.md` / `delta.md`
**Status**: Draft

---

## Test Coverage Matrix

| Code Layer | Required Test Type | Coverage Expectation | Location | Run Command |
| ---------- | ------------------ | -------------------- | -------- | ----------- |
| `piperine-api` surface (Session, results, Trace<T>, units, errors, introspection) | integration | All ACs; happy + fail-loud + edge | `tests/*.rs` (root host tests) | `cargo test -p piperine` |
| `piperine-python` wrappers + ergonomics | integration | Parity with api; kwargs/errors/naming | `crates/piperine-python/tests/*.rs` + `*_tb.py` | `cargo test -p piperine-python` / `piperine test` |
| Cross-host parity | integration | Both public surfaces identical | `tests/host_parity.rs` | `cargo test -p piperine host_parity` |
| Docs (Part VIII / appendix_c) | none | Completeness check | `docs/spec/` | build/review gate |

## Gate Check Commands

| Gate | When | Command |
| ---- | ---- | ------- |
| Quick (api) | api-only tasks | `cargo test -p piperine` |
| Quick (python) | python-only tasks | `cargo build -p piperine-python --features extension-module && cargo test -p piperine-python` |
| Parity | after any surface change | `cargo test -p piperine host_parity` |
| Full | cross-crate / phase end | `cargo test --workspace` |

---

## Execution Plan

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6
```

- **Phase 1 — Foundation + uniform analyses** (HOST-01..06, 13): `Trace<T>` +
  `ComplexWaveform`, all result types, `Session`, uniform analyses, `tf`,
  `dc`→Trace, Python wrappers, parity scaffold. *Reshape-once: consolidation
  here, not later.*
- **Phase 2 — Introspection door** (HOST-07..12): the highest-leverage cluster.
- **Phase 3 — Rich Waveform** (HOST-14..17).
- **Phase 4 — Sweeps** (HOST-18..19).
- **Phase 5 — Configs/units/errors/naming/discoverability** (HOST-20..26).
- **Phase 6 — Docs** (HOST-27..28).

```
Phase 1: T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8
Phase 2: T9 → T10 → T11 → T12 → T13 → T14 → T15
Phase 3: T16 → T17 → T18 → T19
Phase 4: T20 → T21
Phase 5: T22 → T23 → T24 → T25 → T26 → T27 → T28
Phase 6: T29 → T30
```

**Batch packing** (~7/batch → ~5 batches): B1 = Phase 1; B2 = Phase 2;
B3 = Phase 3+4; B4 = Phase 5 (7 tasks); B5 = Phase 6. Sub-agent offer applies
(>8 tasks total).

---

## Task Breakdown

### Phase 1 — Foundation + uniform analyses

#### T1: `Trace<T>` generic + `ComplexWaveform` in api (fold `AcTrace`/`NoiseTrace`)
**What**: Make `Trace` generic over sample type; add canonical `ComplexWaveform`;
fold `AcTrace` (→ `Trace<ComplexWaveform>`) and `NoiseTrace` (→ `Trace` + noise
methods). **Where**: `piperine-api/waveform.rs`. **Requirement**: HOST-13.
**Reuses**: existing `Trace`/`AcTrace`/`NoiseTrace`/`Waveform`.
**Done when**:
- [x] `Trace<T>` with `v`/`i`/`axis`/`stats`/`four`; `T ∈ {Waveform, ComplexWaveform}`
- [x] `ComplexWaveform` is a canonical api struct (`mag`/`phase`/`db`)
- [x] noise = `Trace` + `psd`/`total` (contributions in T14); `AcTrace`/`NoiseTrace` removed
- [x] all existing call sites compile; `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE, commit `aa2f563`. `AcTrace`/`NoiseTrace` are now
type aliases of the same generic `Trace<T>` (`NoiseTrace = Trace<NoiseSample>`,
a zero-sized discriminator — noise has no per-net v/i, only psd/total, so
sharing `T=Waveform` with the transient backend would let a caller call
`.v()` on a noise trace nonsensically). `Trace<Waveform>` also gained a
DC-sweep backend (used by T6).

#### T2: api result types — `TfResult`/`PzResult`/`DistoResult`/`SParamResult`
**What**: Add the missing typed result structs, wrapping the solver returns.
**Where**: `piperine-api/results.rs` (+ new modules). **Requirement**: HOST-02/04.
**Depends on**: T1. **Reuses**: solver analysis results.
**Done when**:
- [x] `TfResult { gain, z_in, z_out }`, `PzResult { poles, zeros }`,
      `DistoResult { hd2, hd3, im2, im3 }`, `SParamResult { s(i,j), z0 }`
- [x] each constructed from the solver driver output; no untyped tuples
- [x] `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE, commit `c6be104`. `run_pz`/`run_disto`/`run_sp`
now return the api types (`From<solver type>` conversions); `TfResult`
constructed from T5.

#### T3: `Session` in api (compiled center)
**What**: `Session` owning the compiled circuit; `Module::compile() -> Session`;
`set`/`schedule_set`/`rebuilds`. **Where**: `piperine-api/session.rs`.
**Requirement**: HOST-01. **Depends on**: T1. **Reuses**: `CircuitInstance`,
`SimSession` staged logic, `run_op_sweep` restamp.
**Done when**:
- [x] `Session` holds the built circuit; `set(label, value)`/`schedule_set(t,…)`/`rebuilds`
- [~] `Module::compile()` returns it; `SimSession` staged surface folded in (no dup concept)
- [x] `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE (scoped), commit `bf9a26a`. `Session::compile(&design,
module)` plays `Module::compile()`'s role (no api-level `Module` reflection
wrapper exists yet). `SimSession` is kept as a distinct staged/fresh-build type
rather than folded — see the SPEC_DEVIATION note above `Session` in
`session.rs` for the reasoning; flagged for the Verifier.

#### T4: Uniform analyses on `Session`, kwargs/builder, typed returns
**What**: Every analysis a method on `Session` with a builder mirroring kwargs,
returning its typed result. **Where**: `piperine-api/session.rs`.
**Requirement**: HOST-02. **Depends on**: T2, T3.
**Done when**:
- [x] `op/dc/tran/ac/noise/sens/pss/pz/disto/sp` on `Session`, builder args
- [x] each returns the typed result (`Trace<T>`/`OpResult`/`*Result`)
- [x] `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE, commit `f33d0c0` (`dc` lands in T6; `tf` in T5).
Builder shape: positional `Option`-typed args, matching the existing
`SimSession::run_*` convention (Rust has no kwargs) rather than a new
builder-struct-per-analysis abstraction.

#### T5: `tf` binding (both hosts)
**What**: Bind the existing solver transfer-function analysis: `Session::tf` →
`TfResult`. **Where**: `piperine-api/session.rs`. **Requirement**: HOST-03.
**Depends on**: T4. **Reuses**: solver `tf` driver.
**Done when**:
- [x] `session.tf(out, src) -> TfResult`; no new solver math
- [x] `cargo test -p piperine` asserts gain/z_in/z_out on a known divider
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE (Rust side), commit `c70ace8`. Python binding
lands in T7 (its own Done-when lists `tf` among the typed-class results).

#### T6: `dc(src, points)` → `Trace<Waveform>`
**What**: Reshape the DC sweep to return a swept `Trace`, not `Vec<OpResult>`.
**Where**: `piperine-api/session.rs`. **Requirement**: HOST-05. **Depends on**:
T4. **Reuses**: `run_op_sweep` restamp (MD-18).
**Done when**:
- [x] `session.dc("v1", pts) -> Trace<Waveform>` over the swept axis
- [x] compile-once (one compilation, restamped); `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE, commit `211c023`. SPEC_DEVIATION: signature is
`dc(label, param, values)` (matches the existing `run_op_sweep` triple),
not literally `dc(src, points)` — no stdlib ideal source ships a fixed
canonical "value" param name, so the two-arg spelling would silently pick
an unstated convention (`"dc"`) rather than being genuinely generic across
any swept element/param.

#### T7: Python wrappers — `_Session` + all result types + `_ComplexWaveform`
**What**: Wrap the new api surface: rename `_LiveSession`→`_Session`, add
`_TfResult`/`_PzResult`/`_DistoResult`/`_SpResult`/`_ComplexWaveform`, make
`_Trace` return the right waveform per analysis. **Where**:
`piperine-python/{session,results,instance}.rs`, `lib.rs`. **Requirement**:
HOST-02/04/13 (python). **Depends on**: T4, T6.
**Done when**:
- [x] every api analysis callable from Python with kwargs; returns the typed wrapper
- [~] `sens/pss/pz/disto/sp/tf` return typed classes (not dicts); no `_AcTrace`/`_NoiseTrace`
- [x] `cargo build -p piperine-python …` + `cargo test -p piperine-python`
**Tests**: integration · **Gate**: quick (python)
**Status (2026-07-24)**: DONE (scoped), commit `ed7052a`. `_LiveSession`→
`_Session`/`LiveSession`→`Session` renamed; `Session` gained
`sens/pss/pz/disto/sp/tf/dc`; facade returns typed dataclasses (not dicts)
for all of them, `tf` via a new native `_TfResult`. SPEC_DEVIATION:
`_AcTrace`/`_NoiseTrace` kept as distinct native pyclasses — see the note in
the commit / `piperine-python/src/live.rs` region; flagged for the Verifier.

#### T8: Parity test scaffold
**What**: `tests/host_parity.rs` enumerating both public surfaces; assert same
analyses + result types. **Where**: root `tests/`. **Requirement**: HOST-06.
**Depends on**: T7.
**Done when**:
- [x] test lists api public analyses/result types + Python `__all__`; asserts parity
- [x] fails loud on a synthetic drift (one host missing an analysis)
- [x] `cargo test -p piperine host_parity`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `1e8db21`. `ANALYSES` constant is the
canonical parity oracle; Rust side proven at compile time (calls every
name — a removal fails to compile), Python side at runtime (`hasattr`
probe through the embedded facade, mirroring `facade_hygiene.rs`'s
technique). Synthetic-drift test proves the probe discriminates. `cargo
test --workspace`: 0 failed (root cause of an earlier stall was the
environment's `/home` partition being full, not a code issue — resolved
by `cargo clean`, unrelated to this task's scope).

---

### Phase 2 — Introspection door

#### T9: `CircuitInstance` introspection accessor (enabling seam)
**What**: Read-only accessor to reach per-device `Introspect` by instance label.
**Where**: `piperine-solver/core/circuit.rs`. **Requirement**: HOST-07 (support).
**Depends on**: T3. **Reuses**: shipped `Element::Introspect`.
**Done when**:
- [x] `CircuitInstance::device_introspect(label) -> Option<&dyn Introspect>` (or equiv)
- [x] no solver-math change; `cargo test -p piperine-solver`
**Tests**: integration · **Gate**: quick (solver)
**Status (2026-07-24)**: DONE, commit `c244992`. Read-only accessor by
instance label, no solver-math change.

#### T10: `InstanceView::opvar`/`opvars` (both hosts)
**What**: opvar host access. **Where**: `piperine-api/results.rs`,
`piperine-python/instance.rs`. **Requirement**: HOST-07. **Depends on**: T9.
**Reuses**: `read_opvars`.
**Done when**:
- [x] `op["x1"].opvar("gm")`/`opvars()` return values (api + python)
- [x] unknown opvar → fail loud (`UnknownNet`-class), not None/NaN
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `3fc21ef`. `OpResult` snapshots each
device's `read_opvars()` eagerly at solve time (mirrors the existing
digital-net snapshot); `OpResult::instance(label) -> InstanceView` exposes
`.opvar(name)`/`.opvars()`. Python `_InstanceView` gains matching methods
(the `Trace.opvar` recorded-over-time case is T11's scope, deferred there
by design).

#### T11: `probe=` + `Trace.opvar` (recorded observable over time)
**What**: `tran(probe=[…])` sets `ProbeSelection`; `Trace.opvar(path)` returns a
`Waveform`. **Where**: `piperine-api/session.rs`+`waveform.rs`, python.
**Requirement**: HOST-08. **Depends on**: T10. **Reuses**: `ProbeSelection`,
`record_device_state`, `eval_opvars`.
**Done when**:
- [ ] `tran(probe=["x1.p_out"])` records it; `trace.opvar("x1.p_out") -> Waveform`
- [ ] unknown probe target fails loud at setup (ABI-35 path)
- [ ] `.mean()` on the recorded opvar matches a static DC opvar at a held point
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T12: `inst.model`/`terminals`(+kind)/`observables()`
**What**: Surface model descriptor, terminals with kind, observable catalog.
**Where**: `piperine-api/results.rs`, python. **Requirement**: HOST-09.
**Depends on**: T9.
**Done when**:
- [ ] `inst.model` (type/version), `inst.terminals` (with `TerminalKind`),
      `inst.observables()` (name/kind/cost)
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T13: `op.stats.limiting` (`LimitingReport`)
**What**: Expose limiting diagnostics on stats. **Where**: `piperine-api`
results/stats, python. **Requirement**: HOST-10. **Depends on**: T9.
**Reuses**: `LimitingReport`.
**Done when**:
- [ ] `op.stats.limiting -> [LimitingReport]` (device/net/proposed/limited/name/reason)
- [ ] empty when nothing limited; `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T14: Noise `by_source`/`contributions`
**What**: Per-source noise. **Where**: `piperine-api/waveform.rs` (noise Trace),
python. **Requirement**: HOST-11. **Depends on**: T1. **Reuses**:
`NoiseContribution`.
**Done when**:
- [ ] `nz.by_source() -> {name: Waveform}`, `nz.contributions() -> [NoiseContribution]`
- [ ] sum of contributions reconciles with `total()` (conservation)
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T15: `Param.bounds`/`unit`/`scope`/`invalidation` reflection
**What**: Surface the shipped `ParamDescriptor` fields on the host `Param`.
**Where**: `piperine-api`, python. **Requirement**: HOST-12. **Depends on**: T3.
**Reuses**: `ParamDescriptor`.
**Done when**:
- [ ] `amp.param("m1.w").bounds`/`unit`/`scope`/`invalidation` readable both hosts
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

---

### Phase 3 — Rich Waveform

#### T16: Real `Waveform` measurements
**What**: `slew_rate`/`rise_time`/`fall_time`/`overshoot`/`settling_time`/`delay`.
**Where**: `piperine-api/waveform.rs`. **Requirement**: HOST-14. **Depends on**: T1.
**Done when**:
- [ ] each returns the measured value; edge cases (flat signal) defined
- [ ] step-response fixture yields plausible values; `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)

#### T17: `Waveform` transforms
**What**: `fft`/`resample`/`derivative`/`integral`/`clip` → new waveform.
**Where**: `piperine-api/waveform.rs`. **Requirement**: HOST-15. **Depends on**: T1.
**Done when**:
- [ ] `fft` round-trips a known tone; `resample(grid)` interpolates; others correct
- [ ] `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)

#### T18: `ComplexWaveform` margins/bandwidth
**What**: `bandwidth_3db`/`gain_margin`/`phase_margin`/`unity_gain_freq`.
**Where**: `piperine-api/waveform.rs`. **Requirement**: HOST-16. **Depends on**: T1.
**Done when**:
- [ ] each returns the value on an AC fixture (known -3dB corner)
- [ ] `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)

#### T19: `plot`/`pip.plot`/`bode` (matplotlib-guarded)
**What**: Python plotting convenience. **Where**: `piperine-python`.
**Requirement**: HOST-17. **Depends on**: T7.
**Done when**:
- [ ] `wf.plot()`/`pip.plot(...)`/`pip.bode(...)` render with matplotlib present
- [ ] absent matplotlib → clear "install matplotlib" error (no hard dep, no no-op)
- [ ] `cargo test -p piperine-python` / `piperine test`
**Tests**: integration · **Gate**: quick (python)

---

### Phase 4 — Sweeps

#### T20: Fluent `sweep` + `SweepPoint`-as-`Session`
**What**: `session.sweep(knob, points)` → iterable of `SweepPoint` (a `Session`
view). **Where**: `piperine-api/session.rs`, python. **Requirement**: HOST-18.
**Depends on**: T4. **Reuses**: compile-once restamp (MD-18).
**Done when**:
- [ ] each `SweepPoint` runs any analysis; compile-once (one build)
- [ ] structural param → rebuild + count (`rebuilds`), never wrong restamp
- [ ] values match per-point fresh builds; `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T21: Nested/named sweep + `map()`→ndarray
**What**: `sweep(a=[…], b=[…])` grid; `grid.map(fn)` shaped array.
**Where**: `piperine-api`, python. **Requirement**: HOST-19. **Depends on**: T20.
**Done when**:
- [ ] nested grid iterates all combinations; `map` returns axis-shaped ndarray (py) / nested Vec (rust)
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

---

### Phase 5 — Configs / units / errors / naming / discoverability

#### T22: Typed configs + canonical `Solver` knobs
**What**: Typed config builders/`__init__`; unify `Solver` name + knob set
(nodeset, `dc_damp_tolerance`) across hosts. **Where**: `piperine-api`, python.
**Requirement**: HOST-20. **Depends on**: T4.
**Done when**:
- [ ] `inspect.signature(TranConfig)` shows fields; `.with_()` immutable copy
- [ ] `Solver` (both hosts) carries the same knobs incl. nodeset + `dc_damp_tolerance`
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T23: Units — newtypes + SI helpers
**What**: `Freq`/`Time`/… newtypes (`From<&str>`+`From<f64>`); analysis args
`impl Into<…>`; Python `pip.Hz/ns/mV/C` helpers. **Where**: `piperine-api/units.rs`,
python. **Requirement**: HOST-21. **Depends on**: T4.
**Done when**:
- [ ] `Freq::from("10MHz") == 1e7`; garbage fails loud; `f64` still accepted
- [ ] `pip.Hz("10M") == 1e7`; raw floats do NOT string-parse
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T24: `SimulationError` hierarchy
**What**: Python exception hierarchy mapped from api `Error`. **Where**:
`piperine-python`, `piperine-api/error.rs`. **Requirement**: HOST-22.
**Depends on**: T7.
**Done when**:
- [ ] `SimulationError` base + `ConvergenceError(node/iteration/analysis)`/`ElaborationError`/`UnknownModule`/`UnknownNet`
- [ ] a non-converging run raises `ConvergenceError`; api `Error` variants map 1:1
- [ ] `cargo test -p piperine-python`
**Tests**: integration · **Gate**: quick (python)

#### T25: `NetRef` ergonomics + enums
**What**: `impl Into<NetRef> for &str`/tuples; `cross`/`dir`/`scale` enums both
hosts. **Where**: `piperine-api`, python. **Requirement**: HOST-23.
**Depends on**: T4.
**Done when**:
- [ ] `v("out")`/`v(("out","in"))` in Rust; no bare `NetRef { name }` needed
- [ ] `cross`/`dir`/`scale` are enums on both sides; `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T26: Naming cleanup + `__len__` + properties
**What**: `const` (not `const_`), `design[name]`, `load_str`, property-based
reflection, `__len__`. **Where**: `piperine-python`, `piperine-api`.
**Requirement**: HOST-24. **Depends on**: T7.
**Done when**:
- [ ] `design["amp"]`, `design.top` (prop), `amp.ports` (prop), `pip.load_str`, `len(wf)`
- [ ] `const` replaces `const_`; property-vs-method consistent
- [ ] `cargo test --workspace`
**Tests**: integration · **Gate**: full

#### T27: `pip.extract` host helper
**What**: `pip.extract(trace, {name: fn})` → measurement dict. **Where**:
`piperine-python`. **Requirement**: HOST-25. **Depends on**: T16.
**Done when**:
- [ ] returns the named-measurement dict; works over `Trace`/`Waveform`
- [ ] `cargo test -p piperine-python` / `piperine test`
**Tests**: integration · **Gate**: quick (python)

#### T28: Complete `.pyi` stubs + docstrings
**What**: Hand-written complete stubs + docstrings for the public surface.
**Where**: `piperine-python` (`.pyi`). **Requirement**: HOST-26. **Depends on**:
T22..T27.
**Done when**:
- [ ] every public class/fn has a stub with typed kwargs + docstring
- [ ] `piperine test` / import smoke passes; autocomplete-visible fields verified
**Tests**: integration · **Gate**: quick (python)

---

### Phase 6 — Docs

#### T29: Rewrite `part_viii_host_api.md`
**What**: Update the normative host-API spec to the delivered surface (both
hosts). **Where**: `docs/spec/part_viii_host_api.md`. **Requirement**: HOST-27.
**Depends on**: T1..T28.
**Done when**:
- [ ] describes `Session`-centric model, uniform analyses + typed results,
      introspection door, nine-type taxonomy, configs/units/errors
- [ ] no stale `LiveSession`/`AcTrace`; mentions `Session`/`tf`/`opvar`/`Trace<T>`/`SimulationError`
- [ ] build/review gate
**Tests**: none · **Gate**: build

#### T30: Refresh `appendix_c_host_surface.md`
**What**: Regenerate the flat reference; resolve/remove §3 asymmetries + §4
review sheet. **Where**: `docs/spec/appendix_c_host_surface.md`. **Requirement**:
HOST-28. **Depends on**: T29.
**Done when**:
- [ ] flat inventory matches the delivered surface; §3 asymmetries gone
- [ ] cross-checks against the parity test; build/review gate
**Tests**: none · **Gate**: build

---

## Phase Execution Map

```
Phase 1:  T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8
Phase 2:  T9 → T10 → T11 → T12 → T13 → T14 → T15
Phase 3:  T16 → T17 → T18 → T19
Phase 4:  T20 → T21
Phase 5:  T22 → T23 → T24 → T25 → T26 → T27 → T28
Phase 6:  T29 → T30
```

Sequential; whole phases are the batch boundaries. 30 tasks → ~5 batches → offer
sub-agents at execution.

---

## Requirement → Task Coverage

| Req | Task | Req | Task |
|-----|------|-----|------|
| HOST-01 | T3 | HOST-15 | T17 |
| HOST-02 | T4,T7 | HOST-16 | T18 |
| HOST-03 | T5 | HOST-17 | T19 |
| HOST-04 | T2,T7 | HOST-18 | T20 |
| HOST-05 | T6 | HOST-19 | T21 |
| HOST-06 | T8 | HOST-20 | T22 |
| HOST-07 | T9,T10 | HOST-21 | T23 |
| HOST-08 | T11 | HOST-22 | T24 |
| HOST-09 | T12 | HOST-23 | T25 |
| HOST-10 | T13 | HOST-24 | T26 |
| HOST-11 | T14 | HOST-25 | T27 |
| HOST-12 | T15 | HOST-26 | T28 |
| HOST-13 | T1,T7 | HOST-27 | T29 |
| HOST-14 | T16 | HOST-28 | T30 |

All 28 requirements mapped.
