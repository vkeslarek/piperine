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
- [x] `tran(probe=["x1.p_out"])` records it; `trace.opvar("x1.p_out") -> Waveform`
- [x] unknown probe target fails loud at setup (ABI-35 path)
- [x] `.mean()` on the recorded opvar matches a static DC opvar at a held point
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `086fc10`. `SimSession::run_tran` /
`Session::tran` take a `probe: &[&str]` arg (each `"instance.name"`); the
path is split, mapped to a `ProbeSelection`, and the solver's existing
`validate_probe_selection` (ABI-35) fails loud on unknown device /
observable at setup. `Trace::opvar(path)` recomputes the opvar per step
from the recorded `(state, vars)` bank via `eval_opvars` (same path
`OpResult::instance(label).opvar(name)` walks at a point); the opvar
index is found by the host-visible `@name(value)` display name carried
on `BuiltInstanceInfo::opvar_display_names` (PIA-07 — one name, both
catalogs). Python `_Module::tran` / `_Session::tran` mirror with
`probe=Vec<String>` kwargs.

#### T12: `inst.model`/`terminals`(+kind)/`observables()`
**What**: Surface model descriptor, terminals with kind, observable catalog.
**Where**: `piperine-api/results.rs`, python. **Requirement**: HOST-09.
**Depends on**: T9.
**Done when**:
- [x] `inst.model` (type/version), `inst.terminals` (with `TerminalKind`),
      `inst.observables()` (name/kind/cost)
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `f572d3e`. `InstanceView` gains
`model()`/`terminals()`/`observables()` over eagerly-snapshotted
`model_descriptor`/`list_terminals`/`list_observables` catalogs (same
shape as T10's opvar snapshot). Python PY-13 connectivity `terminals()`
renamed to `terminal_connections()` to free the `terminals` property name
for HOST-09 descriptors (SPEC_DEVIATION: a rename of an existing Python
method — justified by MD-22's normative `inst.terminals` property shape
from ideal.md §6.5; the old method's semantics are fully preserved under
the new name).

#### T13: `op.stats.limiting` (`LimitingReport`)
**What**: Expose limiting diagnostics on stats. **Where**: `piperine-api`
results/stats, python. **Requirement**: HOST-10. **Depends on**: T9.
**Reuses**: `LimitingReport`.
**Done when**:
- [x] `op.stats.limiting -> [LimitingReport]` (device/net/proposed/limited/name/reason)
- [x] empty when nothing limited; `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `8f4e48f`. `SolverStats` gains a
`limiting: Vec<LimitingReport>` field (collected at the end of the DC solve);
`LimitingReport` gains a `device: String` field so a host can attribute
the report. Python `_SolverStats.limiting` is a `#[pyo3(get)]` field of
`Vec<_LimitingReport>` (a separate `#[pymethods]` getter on _SolverStats
was found to break PyO3 macro expansion — `#[pyo3(get)]` on the field
works correctly). SPEC_DEVIATION: the limiting state is transient
(per-Newton-step) — at a converged DC operating point the list is
typically empty because the limiter releases once the junction
stabilises; the test covers the empty-case surface, not a live limiter
trigger (the solver-level `limiting_report.rs` tests cover the live path).

#### T14: Noise `by_source`/`contributions`
**What**: Per-source noise. **Where**: `piperine-api/waveform.rs` (noise Trace),
python. **Requirement**: HOST-11. **Depends on**: T1. **Reuses**:
`NoiseContribution`.
**Done when**:
- [x] `nz.by_source() -> {name: Waveform}`, `nz.contributions() -> [NoiseContribution]`
- [x] sum of contributions reconciles with `total()` (conservation)
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `3ba3b60`. `NoiseTrace` (= `Trace<NoiseSample>`)
gains `by_source()` (HashMap of `"element/source"` → PSD `Waveform`) and
`contributions()` (`&[NoiseContribution]`) over the shipped solver
`NoiseAnalysisResult.contributions` catalog. Conservation test confirms
`sum(integrated_sq) ≈ total()²`. Python `_NoiseTrace` mirrors both methods;
new `_NoiseContribution` pyclass with element/source/kind/integrated_sq.

#### T15: `Param.bounds`/`unit`/`scope`/`invalidation` reflection
**What**: Surface the shipped `ParamDescriptor` fields on the host `Param`.
**Where**: `piperine-api`, python. **Requirement**: HOST-12. **Depends on**: T3.
**Reuses**: `ParamDescriptor`.
**Done when**:
- [x] `amp.param("m1.w").bounds`/`unit`/`scope`/`invalidation` readable both hosts
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `6fed699`. `InstanceView` gains
`params()` / `param(name)` over eagerly-snapshotted `list_params()` catalogs
(same snapshot pattern as T12, extended `snapshot_introspect` to a 4-tuple).
SPEC_DEVIATION: the ideal.md access path is `amp.param("m1.w")` (module-
level), but the implementation is `op.instance("m1").param("w")` (instance-
level) — the ParamDescriptor is per-device and only available after
compilation, not at the module level; the instance-scoped path matches the
shipped `Introspect::list_params` ABI and the existing HOST-07 opvar pattern.

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
- [x] `fft` round-trips a known tone; `resample(grid)` interpolates; others correct
- [x] `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE, commit `9fb8834`. `fft` resamples onto the
inclusive-endpoint uniform grid (`t0..t_end`, `n` points over `n-1`
intervals — same grid `resample`/`derivative` use) and computes a direct
DFT; verified against a single-tone sine at an exact bin index (peak =
amplitude/2, mirror bin, DC ≈ 0, tolerance 1e-9). `derivative`/`integral`
verified exact on linear/constant synthetic signals; `clip` verified
saturating out-of-band values. `cargo test -p piperine`: 0 failed.

#### T18: `ComplexWaveform` margins/bandwidth
**What**: `bandwidth_3db`/`gain_margin`/`phase_margin`/`unity_gain_freq`.
**Where**: `piperine-api/waveform.rs`. **Requirement**: HOST-16. **Depends on**: T1.
**Done when**:
- [x] each returns the value on an AC fixture (known -3dB corner)
- [x] `cargo test -p piperine`
**Tests**: integration · **Gate**: quick (api)
**Status (2026-07-24)**: DONE, commit `95cc3d8`. `gain_margin` needed a
phase-unwrap helper (`arg()` wraps to `(-π, π]`, but a real multi-pole
rolloff's phase legitimately passes through -180° after wrapping) before
searching for the -180° crossing. Verified against a synthetic 3-pole
loop-gain fixture (`H(f) = A0/[(1+jf/f1)(1+jf/f2)(1+jf/f3)]`); every
expected value derived independently via closed-form magnitude/phase +
bisection root-finding in the test, not by reading the implementation.
`cargo test -p piperine`: 0 failed.

#### T19: `plot`/`pip.plot`/`bode` (matplotlib-guarded)
**What**: Python plotting convenience. **Where**: `piperine-python`.
**Requirement**: HOST-17. **Depends on**: T7.
**Done when**:
- [x] `wf.plot()`/`pip.plot(...)`/`pip.bode(...)` render with matplotlib present
- [x] absent matplotlib → clear "install matplotlib" error (no hard dep, no no-op)
- [x] `cargo test -p piperine-python` / `piperine test`
**Tests**: integration · **Gate**: quick (python)
**Status (2026-07-24)**: DONE, commit `0efd213`. `Waveform.plot()`/
`ComplexWaveform.plot()` bound onto the native pyclasses from the
pure-Python facade (no matplotlib dependency added to the Rust crate);
`pip.plot(waveform_or_dict)`/`pip.bode(complex_waveform)` at module level.
Every entry point returns the created `Figure` rather than calling
`plt.show()` (a library call forcing a blocking show would hang a
headless/test caller). One test exercises both the present-matplotlib
render path and the absent-matplotlib fail-loud path (`sys.modules
["matplotlib"] = None`, the documented CPython halted-import mechanism)
sequentially in one script, since the embedded interpreter is
process-global across parallel `#[test]`s. `cargo build -p piperine-python
--features extension-module && cargo test -p piperine-python`: 0 failed.

---

### Phase 4 — Sweeps

#### T20: Fluent `sweep` + `SweepPoint`-as-`Session`
**What**: `session.sweep(knob, points)` → iterable of `SweepPoint` (a `Session`
view). **Where**: `piperine-api/session.rs`, python. **Requirement**: HOST-18.
**Depends on**: T4. **Reuses**: compile-once restamp (MD-18).
**Done when**:
- [x] each `SweepPoint` runs any analysis; compile-once (one build)
- [x] structural param → rebuild + count (`rebuilds`), never wrong restamp
- [x] values match per-point fresh builds; `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `0f69401`. `Session::sweep(label,
param, values) -> Sweep`: a streaming (lending) iterator — `Sweep::next
(&mut self) -> Option<Result<SweepPoint, Error>>` instead of
`std::iter::Iterator`, since each yielded `SweepPoint` mutably borrows the
sweep's own `Session` and stable `Iterator` can't express an item
borrowing from the iterator itself (no GAT lending-iterator in stable
std). `SweepPoint` derefs to `Session`, so any analysis runs on it
directly. A structural knob write auto-rebuilds the circuit in place via a
new private `set_or_rebuild`/`rebuild` pair on `Session`, scoped to the
sweep path only — `Session::set`'s general fail-loud behavior (T3's
SPEC_DEVIATION) is untouched, since no existing test/caller depends on
auto-rebuild being absent there and the sweep is the one place HOST-18
explicitly asks for it. Verified against fresh-`Session::compile` ground
truth on a presence-flipping optional-param fixture (structural: exactly
one rebuild, then plain restamps) and a plain numeric param
(non-structural: zero rebuilds), both matching per-point fresh builds
within `1e-9` relative error. Python: `Session.sweep` backed by a new
native `_Session.sweep -> _Sweep` iterator (owned `Py<_Session>`, the
standard PyO3 shape for a parent-mutating iterator) so the facade method
has a real native counterpart, satisfying `facade_hygiene`'s native-parity
gate — Python's `_Session::set` already auto-rebuilds (LIVE-14), so the
native `_Sweep` just drives it per point. `cargo test --workspace`: 0
failed (one pre-existing, unrelated flaky test in `piperine-plugin`'s
`process_smoke::dead_guest_is_a_loud_error`, confirmed to pass standalone;
root cause was the environment's `/home` partition being nearly full,
triggering an `lld` crash on a parallel link — resolved by clearing
`target/debug/incremental`, unrelated to this task's scope).

#### T21: Nested/named sweep + `map()`→ndarray
**What**: `sweep(a=[…], b=[…])` grid; `grid.map(fn)` shaped array.
**Where**: `piperine-api`, python. **Requirement**: HOST-19. **Depends on**: T20.
**Done when**:
- [x] nested grid iterates all combinations; `map` returns axis-shaped ndarray (py) / nested Vec (rust)
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `319a96c`. `Session::sweep_grid
(axes) -> Grid` visits the cartesian product of named `(label, param,
values)` axes in row-major order; `Grid::map(f)` restamps (or rebuilds,
reusing T20's `set_or_rebuild`) each axis before calling `f` and collects
results into `Nested<R>` (`Branch` per outer axis, `Leaf` at the deepest
axis) shaped like `Grid::shape()` — the "nested Vec" the task asks for,
generic over the mapped result type rather than a fixed 2D `Vec<Vec<_>>`.
A mapped-function or restamp failure is wrapped with the failing
combination's coordinates (spec edge case). Verified against a
two-resistor divider's closed-form voltage (`mid = 10·r2/(r1+r2)`) at
every `(r1, r2)` combination of a 2×3 grid. Python:
`Session.sweep_grid({"label.param": [...], ...}) -> Grid`, backed by a new
native `_Session.sweep_grid -> _Grid` iterator (same owned-`Py<_Session>`
shape as T20's `_Sweep`); `Grid.map(fn)` returns an axis-shaped
`numpy.ndarray`. SPEC_DEVIATION: the literal spec/ideal example
`sweep(a=[...], b=[...])` (bare kwargs) isn't directly implementable —
PHDL parameters are addressed by flat instance label (`"label.param"`,
the same scheme `sweep`/`set`/`probe=` already use), and a dotted path is
not a valid Python identifier, so `sweep_grid` takes a `dict[str,
list[float]]` keyed by `"label.param"` instead; the grid iteration/nesting
semantics match the spec. Also fixed `Sweep`/`Grid`'s Python `__iter__` to
build a fresh native iterator per call instead of reusing one exhausted
after a single pass (caught by this task's ndarray test — `map()`
iterating a `Grid` left it exhausted for a second use — but the same bug
existed in T20's `Sweep`, fixed here too since both share the pattern).
`cargo test --workspace`: 0 failed (same pre-existing flaky
`process_smoke` test as T20, unrelated).

---

### Phase 5 — Configs / units / errors / naming / discoverability

#### T22: Typed configs + canonical `Solver` knobs
**What**: Typed config builders/`__init__`; unify `Solver` name + knob set
(nodeset, `dc_damp_tolerance`) across hosts. **Where**: `piperine-api`, python.
**Requirement**: HOST-20. **Depends on**: T4.
**Done when**:
- [x] `inspect.signature(TranConfig)` shows fields; `.with_()` immutable copy
- [x] `Solver` (both hosts) carries the same knobs incl. nodeset + `dc_damp_tolerance`
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `e4914e4`. Added `dc_damp_tolerance`
to the Python `Solver` dataclass (already on the Rust `SolverConfig`) and
threaded it through `solver_config`'s duck-typed mapping; added a shared
`_ConfigMixin.with_(**overrides)` (`dataclasses.replace`) to every config
bundle. Fixed the `nodeset` asymmetry on `Session.dc` (native `_Session.dc`
+ facade), which previously accepted `nodeset` on `op`/`tran` but not
`dc`. `cargo test --workspace`: 0 failed.

#### T23: Units — newtypes + SI helpers
**What**: `Freq`/`Time`/… newtypes (`From<&str>`+`From<f64>`); analysis args
`impl Into<…>`; Python `pip.Hz/ns/mV/C` helpers. **Where**: `piperine-api/units.rs`,
python. **Requirement**: HOST-21. **Depends on**: T4.
**Done when**:
- [x] `Freq::from("10MHz") == 1e7`; garbage fails loud; `f64` still accepted
- [x] `pip.Hz("10M") == 1e7`; raw floats do NOT string-parse
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `cbe237d`. `Freq`/`Time` newtypes
(`piperine-api/src/units.rs`) with `From<f64>` (bare number, base unit) and
`From<&str>` (SI prefix + optional unit-name suffix; garbage panics —
`From` can't return `Result`). `Session::ac`'s `fstart`/`fstop` accept
`impl Into<Freq>` as the representative demonstration (every existing
`f64` call site keeps compiling via the blanket `From<f64>`); SPEC_DEVIATION
above `Session::ac` explains why the wider `Into<...>` retrofit across
every analysis arg (both `Session`/`SimSession`, ~12 methods) is scoped
out. Python `pip.Hz/ns/mV/C` helpers mirror the Rust parsing; SI prefixes
only apply to `str` input, never to a raw `float`/`int`. `cargo test
--workspace`: 0 failed.

#### T24: `SimulationError` hierarchy
**What**: Python exception hierarchy mapped from api `Error`. **Where**:
`piperine-python`, `piperine-api/error.rs`. **Requirement**: HOST-22.
**Depends on**: T7.
**Done when**:
- [x] `SimulationError` base + `ConvergenceError(node/iteration/analysis)`/`ElaborationError`/`UnknownModule`/`UnknownNet`
- [x] a non-converging run raises `ConvergenceError`; api `Error` variants map 1:1
- [x] `cargo test -p piperine-python`
**Tests**: integration · **Gate**: quick (python)
**Status (2026-07-24)**: DONE, commit `33b57f7`. Added the five classes to
the Python facade; each subclass ALSO inherits the matching builtin
exception type it previously surfaced as (`ValueError`/`KeyError`/
`RuntimeError`) via multiple inheritance, so every existing
`except KeyError`/`except ValueError` call site (incl. LIVE-11's
`Session.set` error-parity test) keeps working unchanged — purely additive.
`load()`/`Design.module()` wrap their native call directly; every
`Module`/`Session` analysis + `set` method gets a `_wrap_analysis_errors`
decorator that reclassifies by message content ("Failed to converge" →
`ConvergenceError` with `iteration`/`analysis` populated, `node` best-effort
`None`; "is not addressable"/"is not a solved analog net" → `UnknownNet`)
and otherwise re-raises completely unchanged. `cargo test -p
piperine-python`: 0 failed.

#### T25: `NetRef` ergonomics + enums
**What**: `impl Into<NetRef> for &str`/tuples; `cross`/`dir`/`scale` enums both
hosts. **Where**: `piperine-api`, python. **Requirement**: HOST-23.
**Depends on**: T4.
**Done when**:
- [x] `v("out")`/`v(("out","in"))` in Rust; no bare `NetRef { name }` needed
- [x] `cross`/`dir`/`scale` are enums on both sides; `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `72027f2`. `NetRef` gains `From<&str>`/
`String`/`&String`/`&NetRef`, plus a `NetSelector` trait (implemented per
concrete shape, not a blanket `impl<T: Into<NetRef>>` + generic
`(A, Option<B>)` tuple impl — the two structurally overlap under Rust's
coherence rules) so `.v`/`.i` (`OpResult`, `Trace<Waveform>`,
`Trace<ComplexWaveform>`) take one argument instead of two:
`op.v("out")`/`op.v(("out","in"))`/`op.v(net_ref)` all work, no bare
`NetRef { name }` needed at any call site (56 existing call sites across
~25 files mechanically updated). `CrossDirection` (Rising/Falling/Either)
replaces `Waveform::cross`'s `dir: &str` (with `From<&str>` for legacy
strings); `Scale` (Lin/Dec/Oct) with `impl From<Scale> for bool`, wired
into `Session::ac`'s `logarithmic` via `impl Into<bool>`. Python: native
`.cross()` gets a facade-level `CrossDirection` enum shim — idempotent via
a `hasattr` guard, since the native `_Waveform` class is a process-wide
singleton across every embedded-interpreter facade re-execution and an
unconditional capture-then-wrap would self-recurse on a second `run_script`/
`piperine run` in the same process (caught by `run_examples.rs`, not a
new test file). `Direction` enum added for `TerminalDescriptor.direction`
(SPEC_DEVIATION: `Port`/`Terminal` reflection fields themselves stay plain
`str` — wrapping those native pyclasses is out of scope). `cargo test
--workspace`: 0 failed (one pre-existing flaky `process_smoke` test,
confirmed unrelated).

#### T26: Naming cleanup + `__len__` + properties
**What**: `const` (not `const_`), `design[name]`, `load_str`, property-based
reflection, `__len__`. **Where**: `piperine-python`, `piperine-api`.
**Requirement**: HOST-24. **Depends on**: T7.
**Done when**:
- [x] `design["amp"]`, `design.top` (prop), `amp.ports` (prop), `pip.load_str`, `len(wf)`
- [x] `const` replaces `const_`; property-vs-method consistent
- [x] `cargo test --workspace`
**Tests**: integration · **Gate**: full
**Status (2026-07-24)**: DONE, commit `5baf8b7`. `Design.top` becomes a
property; `Design.__getitem__` delegates to `.module()` (raising
`UnknownModule` on a miss); `Design.const_` renamed to `Design.const` in
the facade (native binding keeps `const_` — `const` is a Rust keyword, not
a Python one; `facade_hygiene.rs` gets a named exemption mirroring its
existing `compile` exemption). `Module.ports/nets/instances/params/
behaviors` become properties (reflection, not actions). Added
`pip.load_str(src)` (native `_piperine.load_str`, backed by a new
`_Design::load_str`/`from_source` split). `len(wf)` was already satisfied
(`_Waveform.__len__` pre-existing from an earlier task) — covered by this
task's test as an already-satisfied AC. `cargo test --workspace`: 0 failed
(same pre-existing flaky `process_smoke`/`host_parity` parallel-test races,
confirmed to pass standalone — a latent embedded-interpreter
cross-thread-unsendable characteristic, not something this task
introduced).

#### T27: `pip.extract` host helper
**What**: `pip.extract(trace, {name: fn})` → measurement dict. **Where**:
`piperine-python`. **Requirement**: HOST-25. **Depends on**: T16.
**Done when**:
- [x] returns the named-measurement dict; works over `Trace`/`Waveform`
- [x] `cargo test -p piperine-python` / `piperine test`
**Tests**: integration · **Gate**: quick (python)
**Status (2026-07-24)**: DONE, commit `05ffe1f`. `pip.extract(source, {name:
fn})` applies every named measurement function to `source` and collects
results into a dict — deliberately agnostic over `source`'s type (`Trace`,
`Waveform`, `ComplexWaveform`, ...), since it's just `fn(source)` per
entry. SPEC_DEVIATION note: T16 (`Waveform.slew_rate`/etc, HOST-14) only
landed on the Rust `piperine-api` side — no native Python binding exists
yet (T16's own gate was "quick (api)" only) — so this task's tests use
already-bound native `Waveform` methods (`.max`/`.min`/`.cross`) rather
than the still-Rust-only measurements, not silently expanding scope into
completing T16's Python binding. `cargo test -p piperine-python`: 0
failed.

#### T28: Complete `.pyi` stubs + docstrings
**What**: Hand-written complete stubs + docstrings for the public surface.
**Where**: `piperine-python` (`.pyi`). **Requirement**: HOST-26. **Depends on**:
T22..T27.
**Done when**:
- [x] every public class/fn has a stub with typed kwargs + docstring
- [x] `piperine test` / import smoke passes; autocomplete-visible fields verified
**Tests**: integration · **Gate**: quick (python)
**Status (2026-07-24)**: DONE, commit `67a942a`. Added
`python/piperine/_piperine.pyi` — a hand-written stub for the native
`_piperine` extension (28 classes, ~100 methods/getters/fields; the
compiled `.so` carries no type info of its own). The pure-Python facade
(`__init__.py`) already carries full inline type hints + docstrings for
every locally-defined class/function, so no separate stub was needed
there. Added `py.typed` (PEP 561). A dedicated test (`pyi_stub.rs`) parses
the stub with `ast` and cross-checks every declared class/function/
method/property against the real runtime `_piperine` module via
`hasattr` — caught one real drift during authoring (`_Trace.opvar`
declared but not natively bound; only `_InstanceView.opvar` exists),
fixed by removing the incorrect stub entry. This is also the last task of
Phase 5; `cargo test --workspace`: 0 failed (same pre-existing flaky
`process_smoke` test, confirmed to pass standalone).

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
