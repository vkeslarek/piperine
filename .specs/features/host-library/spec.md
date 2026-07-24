# Host Library Specification

> **Refines ROADMAP P3** ("Python library polished"). Designed ideal-first:
> `ideal.md` is the north-star surface, `delta.md` maps it against today's
> shipping surface. This spec turns that delta into traceable requirements.
>
> **Governing:** MD-22 (uniform host surface — Python and Rust are one API,
> enforced by a parity test), MD-18 (compile-once / restamp), MD-20
> (`piperine-api` is the library face).

## Problem Statement

`import piperine` is meant to be the single host (benches, validation, plugin
scripting), but the surface is uneven: the engine ships analyses and a rich
introspection ABI the host barely exposes. Concretely (verified 2026-07-23,
`delta.md`): the Rust host runs `sens`/`pss`/`pz`/`sp`/`disto` but **Python has
no typed result class** for them; `tf` exists only in the solver; the whole
`element-abi-maturity` introspection catalog (opvars, observables, terminals+
kind, model descriptor, limiting reports, per-source noise, param bounds) has
**almost no host door** — so a designer cannot read a component's computed
`power` opvar, discover what is probeable, or feed param bounds to an optimizer.
Configs are opaque, return types carry a redundant `Trace`/`AcTrace`/
`NoiseTrace` trio, waveforms lack measurements/plot, and there is no typed error
hierarchy. The two hosts drift with no test locking them.

## Goals

- [ ] The compiled **`Session`** is the host center of gravity on both hosts
      (Python rename `LiveSession`→`Session`; build the Rust equivalent).
- [ ] Every analysis (`op`/`dc`/`tran`/`ac`/`noise`/`tf`/`sens`/`pss`/`pz`/
      `disto`/`sp`/`four`) ships on **both hosts** with a **typed result** and a
      **kwargs-first** call shape.
- [ ] The `element-abi` introspection catalogs get a host door: `opvar`(s),
      `observables`, `model`, `terminals`(+kind), `limiting`, noise
      `by_source`/`contributions`, `Param.bounds`/`unit`/`scope` — enough to run
      the §0 efficiency/opvar driving scenario end to end (minus `optimize`).
- [ ] Return types consolidate to the nine-type taxonomy (`Trace[T]` generic;
      `AcTrace`/`NoiseTrace` folded in).
- [ ] `Waveform`/`ComplexWaveform` gain measurements, transforms, and `plot`.
- [ ] Configs are typed/kwargs-first; SI helpers (`pip.Hz`…) exist; Rust adds
      typed-unit `Into` (`Freq: From<&str>`).
- [ ] A typed `SimulationError` hierarchy; `.pyi` stubs; consistent property/
      method conventions; naming cleanup.
- [ ] A **parity test** locks the two public surfaces in lockstep (MD-22).
- [ ] The normative host-API spec **`docs/spec/part_viii_host_api.md`** (and the
      flat reference `docs/spec/appendix_c_host_surface.md`) are updated to the
      delivered surface.
- [ ] `cargo test --workspace` + the Python test suite green; zero warnings.

## Out of Scope

| Feature | Reason |
|---------|--------|
| **`pip.optimize` (design centering)** | P6 pillar — under algorithm study (user 2026-07-23). This feature makes it *feedable* (`Param.bounds`, opvar objectives) but does not implement the loop. |
| **Python plugin scripting** (`@pip.device`/`@pip.hook`) | P5 pillar — the "lifecycle registry to Python" (MD-21). Surfaced in `ideal.md` for the scripting story; delivered under P5. |
| **New solver analyses / engine math** | The engine is complete (P1 CLOSED). This is host surface only — no new `.analysis`, no convergence/integration change. `tf` is a *binding* of an existing solver analysis, not new math. |
| **`.pyi` autogeneration tooling** | Ship hand-written complete stubs; a generator is a follow-up. |
| **matplotlib as a hard dependency** | `plot`/`bode` degrade gracefully (import-guarded); matplotlib stays optional. |
| **Packaging / PyPI publish** | Post-V1 (keep module layout PyPI-shaped, per current P3). |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Feature scope | Host-pure: workstreams #1–7 + #9; `optimize`→P6, plugin-scripting→P5, `extract` stays | User 2026-07-23 | y (user) |
| MVP anchor | P1 = compiled `Session` center + uniform analysis surface (#2 + #1); introspection (#4b) is P2 | User 2026-07-23 — structural base before the doors | y (user) |
| Config ergonomics | Kwargs-first on the analysis; optional reusable typed `Config` with `.with_()` | User 2026-07-23 | y (user) |
| Units | Plain floats + explicit SI helpers (`pip.Hz("10M")`); NO magic string-unit parse on raw floats. Rust adds typed-unit `Into` where a slot is typed (`Freq: From<&str>`) | User 2026-07-23 | y (user) |
| Return types | Nine-type taxonomy; `Trace[T]` generic folds `AcTrace`/`NoiseTrace`; noise = `Trace` + noise methods; structured results stay distinct | User 2026-07-23 (separate-if-ops-differ rule) | y (user) |
| Waveform measurements | Methods on the waveform (`wf.bandwidth_3db()`), not a `pip.measure` namespace | User 2026-07-23 | y (user) |
| `Session` naming | `Session` (drop `Live` prefix) is the compiled center on both hosts | Matches ideal §4; Rust builds the missing equivalent | y (user, implied) |
| `dc` result shape | `dc(src, pts)` returns `Trace[Waveform]` (swept axis), not `Vec<OpResult>` | Uniform with tran/ac; `run_op_sweep` becomes its restamp engine | n (Design) |
| Parity-test mechanism | A test enumerates both public surfaces (`__all__` + `piperine-api` exports) and asserts name/shape parity | MD-22 needs enforcement, not just intent | n (Design) |
| opvar addressing | `op["x1"].opvar("p_out")` (instance-scoped) + `trace.opvar("x1.p_out")` (path-scoped, recorded via `probe=`) | Matches the shipped ABI (`read_opvars` per device; `ProbeSelection` per observable) | n (Design) |

**Open questions:** the `n (Design)` rows (dc container details, parity-test
mechanism, opvar addressing surface) — HOW-shape decisions for Design; they do
not change WHAT.

---

## User Stories

### P1: Compiled `Session` + uniform analysis surface ⭐ MVP

**User Story**: As a host user (Python or Rust), I compile a module once into a
`Session` and run every analysis on it with the same kwargs-first call and a
typed result — identical in both languages.

**Why P1**: The structural base. Every later story hangs off `Session` and the
uniform analyses. It also closes the live MD-22 breach (Python missing typed
results for `sens`/`pss`/`pz`/`sp`/`disto`; `tf` absent from both hosts).

**Acceptance Criteria**:

1. WHEN a user calls `module.compile()` THEN both hosts SHALL return a `Session`
   owning the compiled circuit, exposing `set`/`schedule_set`/`rebuilds` and
   every analysis method (Rust builds the equivalent of Python's session).
2. WHEN any analysis (`op`/`dc`/`tran`/`ac`/`noise`/`tf`/`sens`/`pss`/`pz`/
   `disto`/`sp`) is called on either host THEN it SHALL accept kwargs directly
   (`tran(stop=1e-3, step=1e-6)` / builder-mirrored in Rust) and return a typed
   result object present on **both** hosts.
3. WHEN `tf` is requested THEN it SHALL run the existing solver transfer-function
   analysis and return a `TfResult` (`gain`/`z_in`/`z_out`) on both hosts (new
   binding — no new solver math).
4. WHEN `sens`/`pss`/`pz`/`disto`/`sp` are called from Python THEN each SHALL
   return a typed result class (`SensResult`/`PssResult`/`PzResult`/
   `DistoResult`/`SParamResult`), not an untyped dict — matching the Rust shape.
5. WHEN `dc(src, points)` is called THEN it SHALL return a `Trace[Waveform]`
   over the swept axis (compile-once restamp underneath, MD-18), not a bare list.
6. WHEN the parity test runs THEN it SHALL assert the Python public surface and
   the `piperine-api` public surface expose the same analyses + result types
   (fails loud on drift).

**Independent Test**: On both hosts, `sim = design["amp"].compile()`; run each
analysis; assert each returns its typed result; assert `tf` and the five
formerly-Python-untyped analyses now return typed objects; the parity test is
green.

---

### P2: Device introspection reflection — the opvar/observable door

**User Story**: As a designer, I read a component's computed operating-point
variables (e.g. `power`), discover what is probeable, record named observables
over a transient, and read param bounds — the shipped `element-abi` catalogs,
now reachable from the host.

**Why P2**: Highest-leverage cluster (`delta.md` #4b). Unlocks the §0
efficiency/opvar driving scenario, convergence debugging, and auto knob-bounds
for the future optimizer. All read-only bridges over data that already exists.

**Acceptance Criteria**:

1. WHEN an operating point is solved THEN `op["x1"].opvar("p_out")` and
   `op["x1"].opvars()` SHALL return the device's computed opvars from the shipped
   `read_opvars` bridge (both hosts).
2. WHEN a transient is run with `probe=["x1.p_out"]` THEN the observable SHALL be
   recorded per step and `trace.opvar("x1.p_out")` SHALL return a `Waveform`
   (backed by `ProbeSelection`); an unknown probe target SHALL fail loud.
3. WHEN an instance is introspected THEN `inst.model` (`ModelDescriptor`),
   `inst.terminals` (with `TerminalKind`), and `inst.observables()` (name/kind/
   cost catalog) SHALL be readable.
4. WHEN a solve limits THEN `op.stats.limiting` SHALL expose the
   `LimitingReport`s (`device`, `net`, `proposed`, `limited_value`,
   `limiter_name`, `reason`).
5. WHEN a noise analysis runs THEN `nz.by_source()` and `nz.contributions()`
   SHALL expose per-source `NoiseContribution` data (`element`/`source`/`kind`/
   `psd`/`integrated_sq`), beyond the scalar `total()`.
6. WHEN a param is reflected THEN `Param.bounds`/`unit`/`scope`/`invalidation`
   SHALL be readable from the shipped `ParamDescriptor`.

**Independent Test**: The §0 scenario minus `optimize`: author `var p_out`,
`op["x1"].opvar("p_out")` returns the DC value; `tran(probe=[...])` +
`trace.opvar(...).mean()` returns cycle-average; `amp.param("m1.w").bounds`
returns the range. Both hosts.

---

### P2: Return-type consolidation — `Trace[T]` generic

**User Story**: As a host user, swept results share one container type
regardless of real/complex sample, so I learn one `Trace` API.

**Why P2**: The redundant `Trace`/`AcTrace`/`NoiseTrace` trio is the one
consolidation the taxonomy demands (separate-if-ops-differ). Best done alongside
P1's typed-result work to avoid reshaping twice.

**Acceptance Criteria**:

1. WHEN `tran`/`dc` returns THEN it SHALL be a `Trace[Waveform]`; WHEN `ac`
   returns THEN a `Trace[ComplexWaveform]` — one generic container, same
   `v`/`i`/`axis`/`stats`/`four` surface (`AcTrace` removed).
2. WHEN `noise` returns THEN it SHALL be a `Trace` carrying the noise-specific
   `psd`/`total`/`by_source`/`contributions` methods (`NoiseTrace` folded in).
3. WHEN Rust is used THEN `Trace<T>` SHALL be genuinely generic over the sample
   type; the Python `_Trace` SHALL return the correct waveform type per analysis.
4. WHEN the parity test runs THEN neither host SHALL carry `AcTrace`/`NoiseTrace`
   as separate public types.

**Independent Test**: `type(sim.tran(...))` and `type(sim.ac(...))` are the same
container class on both hosts; `sim.ac(...).v("out")` is complex,
`sim.tran(...).v("out")` is real; no `AcTrace` symbol is exported.

---

### P3: Rich `Waveform` — measurements, transforms, plot

**User Story**: As a designer, a waveform measures itself (bandwidth, slew,
overshoot), transforms itself (fft, resample), and plots itself.

**Why P3**: High ergonomic value, but sits on the P1/P2 result types. Pure
additive methods.

**Acceptance Criteria**:

1. WHEN a real `Waveform` is measured THEN `slew_rate`/`rise_time`/`fall_time`/
   `overshoot`/`settling_time`/`delay` SHALL return the measured value (both
   hosts).
2. WHEN a `Waveform` is transformed THEN `fft`/`resample`/`derivative`/
   `integral`/`clip` SHALL return a new waveform.
3. WHEN a `ComplexWaveform` is measured THEN `bandwidth_3db`/`gain_margin`/
   `phase_margin`/`unity_gain_freq` SHALL return the value.
4. WHEN `wf.plot()` / `pip.plot(...)` / `pip.bode(...)` is called with
   matplotlib available THEN it SHALL render; absent matplotlib SHALL fail with a
   clear "install matplotlib" message (import-guarded, not a hard dep).

**Independent Test**: A step-response `Waveform` returns a plausible
`slew_rate()`/`overshoot()`; an AC `ComplexWaveform` returns a `bandwidth_3db()`;
`fft()` round-trips a known tone.

---

### P3: First-class sweeps

**User Story**: As a designer, I sweep one or many knobs fluently, treat each
point as a `Session`, and collect results as an ndarray.

**Why P3**: Depends on `Session` (P1). Elevates `run_op_sweep` to a general
compile-once sweep surface.

**Acceptance Criteria**:

1. WHEN `sim.sweep(knob, points)` is iterated THEN each `SweepPoint` SHALL be a
   `Session` view at that operating point exposing every analysis (compile-once
   restamp, MD-18).
2. WHEN `sim.sweep(a=[...], b=[...])` is used THEN it SHALL produce the nested
   named grid.
3. WHEN `grid.map(fn)` is called THEN it SHALL return an ndarray shaped like the
   sweep axes.
4. WHEN a sweep spans a structural (rebuild-invalidating) param THEN it SHALL
   rebuild and count it (`rebuilds`), never silently restamp wrong.

**Independent Test**: A 2-knob sweep `.map(lambda s: s.op().v("out"))` returns an
array of the sweep shape; values match per-point fresh builds.

---

### P3: Configs, units, errors, discoverability, naming

**User Story**: As a host user, configs are typed and IDE-visible, numbers read
in engineering units, failures are typed and catchable, and names are clean and
consistent.

**Why P3**: Ergonomics + polish. Independent, lands last.

**Acceptance Criteria**:

1. WHEN a `Config` (`TranConfig`…) is constructed THEN its fields SHALL be typed
   and visible to autocomplete (`inspect.signature`), with `.with_()` copy; the
   canonical `Solver` knob set (incl. `dc_damp_tolerance`, `nodeset`) SHALL be
   identical on both hosts; the class SHALL be named `Solver` on both.
2. WHEN a number is passed THEN SI helpers (`pip.Hz`/`ns`/`mV`/`C`…) SHALL
   construct it; Rust typed-unit slots SHALL also accept `&str` (`Freq::from
   ("10MHz")`) and `f64`; raw floats SHALL NOT magic-parse strings.
3. WHEN an analysis fails THEN it SHALL raise a typed `pip.SimulationError`
   subclass (`ConvergenceError` with `.node`/`.iteration`/`.analysis`,
   `ElaborationError`, `UnknownModule`, `UnknownNet`) — not a bare
   `ValueError`/`KeyError`; the Rust `Error` enum SHALL mirror the taxonomy.
4. WHEN the Rust host addresses a net THEN `v`/`i` SHALL accept `impl
   Into<NetRef>` (`&str`, tuples) — no bare `NetRef { name }`; `cross`/`dir`/
   `scale` SHALL be enums on both hosts.
5. WHEN names are used THEN `const` (not `const_`), `design[name]` indexing,
   `load_str`, property-based reflection (`design.top`, `amp.ports`), `__len__`,
   and `pip.extract(trace, {...})` SHALL be present; property-vs-method
   convention SHALL be consistent (data=property, action=method).

**Independent Test**: `inspect.signature(TranConfig)` shows `stop`/`step`;
`pip.Hz("10M") == 1e7`; a non-converging run raises `pip.ConvergenceError`;
`len(wf)` works; `pip.extract` returns the measurement dict.

---

### P3: Normative spec docs updated to the delivered surface

**User Story**: As a reader of the formal spec, `part_viii_host_api.md` describes
the surface that actually ships, so the doc is not a lie.

**Why P3**: Documentation gate. Lands last, after the surface stabilizes, so it
records the delivered shape (not an aspirational one).

**Acceptance Criteria**:

1. WHEN the feature completes THEN `docs/spec/part_viii_host_api.md` SHALL be
   updated to describe the delivered surface: `Session`-centric model, the
   uniform analysis set + typed results, the introspection door, the nine-type
   taxonomy, configs/units/errors — for **both** hosts (MD-22).
2. WHEN `part_viii` is updated THEN the flat reference
   `docs/spec/appendix_c_host_surface.md` SHALL be regenerated/refreshed to match
   (its §3 "known asymmetries" and §4 "review sheet" resolved or removed).
3. WHEN a reader cross-checks the doc against the code THEN every public host
   name in the doc SHALL exist on both hosts (the parity test is the executable
   counterpart of this doc claim).

**Independent Test**: `part_viii_host_api.md` mentions `Session`, `tf`, `opvar`,
`Trace[T]`, `SimulationError`, and no longer describes `LiveSession`/`AcTrace`;
appendix_c §3 asymmetries are gone.

---

## Edge Cases

- WHEN an analysis exists on one host only (regression) THEN the parity test
  SHALL fail loud (MD-22 guard).
- WHEN `probe=[...]` names an observable a device doesn't declare THEN setup
  SHALL fail loud (reuses the shipped `ProbeSelection` fail-loud, ABI-35).
- WHEN `opvar(name)` names an undeclared opvar THEN it SHALL raise
  `UnknownNet`-class error, not return `None`/`NaN` silently.
- WHEN `plot` is called without matplotlib THEN a clear install message SHALL be
  raised (no hard dependency, no silent no-op).
- WHEN a `Config` is reused across analyses THEN `.with_()` SHALL not mutate the
  original (immutable copy-with).
- WHEN a sweep point's analysis fails THEN the failure SHALL surface with the
  point's coordinates (which knob values), not a bare error.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| HOST-01 | P1 `Session` center, both hosts | Design | Pending |
| HOST-02 | P1 kwargs-first analyses, typed results, both hosts | Design | Pending |
| HOST-03 | P1 `tf` binding both hosts | Design | Pending |
| HOST-04 | P1 Python typed results for sens/pss/pz/disto/sp | Design | Pending |
| HOST-05 | P1 `dc` → `Trace[Waveform]` | Design | Pending |
| HOST-06 | P1 parity test locks both surfaces | Design | Pending |
| HOST-07 | P2 `opvar`/`opvars` host access | Design | Pending |
| HOST-08 | P2 `probe=` + `trace.opvar` recorded observable | Design | Pending |
| HOST-09 | P2 `inst.model`/`terminals`(+kind)/`observables()` | Design | Pending |
| HOST-10 | P2 `op.stats.limiting` (`LimitingReport`) | Design | Pending |
| HOST-11 | P2 noise `by_source`/`contributions` | Design | Pending |
| HOST-12 | P2 `Param.bounds`/`unit`/`scope`/`invalidation` | Design | Pending |
| HOST-13 | P2 `Trace[T]` generic; fold `AcTrace`/`NoiseTrace` | Design | Pending |
| HOST-14 | P3 `Waveform` measurements | Design | Pending |
| HOST-15 | P3 `Waveform` transforms (fft/resample/…) | Design | Pending |
| HOST-16 | P3 `ComplexWaveform` margins/bandwidth | Design | Pending |
| HOST-17 | P3 `plot`/`pip.plot`/`bode` (matplotlib-guarded) | Design | Pending |
| HOST-18 | P3 fluent `sweep` + `SweepPoint`-as-`Session` | Design | Pending |
| HOST-19 | P3 nested/named sweep + `map()`→ndarray | Design | Pending |
| HOST-20 | P3 typed configs + canonical `Solver` knobs | Design | Pending |
| HOST-21 | P3 SI helpers + Rust typed-unit `Into` | Design | Pending |
| HOST-22 | P3 `SimulationError` hierarchy (both hosts) | Design | Pending |
| HOST-23 | P3 `NetRef` ergonomics + `cross`/`dir`/`scale` enums | Design | Pending |
| HOST-24 | P3 naming cleanup (`const`, `design[]`, `load_str`, properties, `__len__`) | Design | Pending |
| HOST-25 | P3 `pip.extract` host helper | Design | Pending |
| HOST-26 | P3 complete `.pyi` stubs + docstrings | Design | Pending |
| HOST-27 | P3 `part_viii_host_api.md` updated to delivered surface | Design | Pending |
| HOST-28 | P3 `appendix_c_host_surface.md` refreshed to match | Design | Pending |

**ID format:** `HOST-[NUMBER]`

**Coverage:** 28 total, 0 mapped to tasks (Design pending).

---

## Success Criteria

- [ ] The §0 driving scenario runs end to end on both hosts (minus `optimize`):
      opvar authored → `op[...].opvar` / `trace.opvar` via `probe=` → param
      bounds readable (HOST-07..12).
- [ ] Every analysis returns a typed result on both hosts; `tf` bound; Python no
      longer lags Rust (HOST-01..05).
- [ ] The parity test is green and guards drift (HOST-06).
- [ ] Nine-type taxonomy; no `AcTrace`/`NoiseTrace` (HOST-13).
- [ ] Waveforms measure/transform/plot themselves (HOST-14..17).
- [ ] Sweeps are first-class and compile-once (HOST-18..19).
- [ ] Typed configs, SI units, typed errors, clean names, complete stubs
      (HOST-20..26).
- [ ] `part_viii_host_api.md` + `appendix_c` describe the delivered surface
      (HOST-27..28).
- [ ] `cargo test --workspace` + Python suite green; zero rustc warnings.
