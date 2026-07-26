# Host Library — delta (ideal → current → gap)

> Maps the north-star (`ideal.md`) against what ships today, so the ROADMAP P3
> refinement and the feature spec are grounded in the real gap, not the stale
> `docs/spec/appendix_c_host_surface.md` (dated 2026-07-18, pre-`.sens`/PSS/
> spectral).
>
> **Current surface, verified 2026-07-23:**
> - **Rust host** (`piperine-api/session.rs`) analyses: `run_op`,
>   `run_op_sweep`, `run_tran`, `run_ac`, `run_noise`, `run_sens`, `run_pss`,
>   `run_pz`, `run_sp`, `run_disto`. **No `run_tf`.**
> - **Python classes** registered: `load`, `_Design`, `_Module`,
>   `_LiveSession`, `_Selection`, `_Node`/`_Instance`/`_Net`/`_Port`/`_Param`/
>   `_Behavior`/`_Terminal`, `_OpResult`, `_InstanceView`, `_Trace`,
>   `_AcTrace`, `_NoiseTrace`, `_FourierResult`/`_FourierComponent`,
>   `_Waveform`, `_ComplexWaveform`, `_SolverStats`.
>   **Missing typed classes: `PssResult`, `PzResult`, `SensResult`,
>   `DistoResult`, `SpResult`, `TfResult`** — the newer analyses have no typed
>   Python result object (return untyped dicts/tuples or aren't bound).
> - **Waveform** (`api/waveform.rs`): `at`, `axis`, `cross`, `min`, `max`,
>   `mean`, `rms`, `peak_to_peak`, `mag`, `phase`, `db`, `psd`, `total`, `v`,
>   `i`, `stats`. **No measurements, no fft/resample, no plot.**

**Classification legend:** KEEP (ships, matches) · RENAME (exists, wrong name) ·
RESHAPE (exists, wrong shape/ergonomics) · BUILD (exists one host, missing the
other — MD-22 gap) · NEW (doesn't exist anywhere).

---

## 1. Entry + reflection

| Ideal (§2) | Current | Class | Host |
|-----------|---------|-------|------|
| `pip.load(path)` | `load` | KEEP | B |
| `pip.load_str(src)` | Rust `parse_and_elaborate`; Python none | BUILD | P |
| `design[name]` | `design.module(name)` (raises) | RESHAPE (`__getitem__`) | B |
| `design.top` (prop) | `top()` method | RENAME (method→prop) | B |
| `design.modules` (prop) | `modules()` method | RENAME | B |
| `design.const(name)` | `const_(name)` | RENAME (drop `_`) | B |
| reflection as properties (`amp.ports`) | zero-arg methods (`ports()`) | RESHAPE | B |
| `design.select(path).nodes` | `select().nodes()` | RENAME | B |

## 2. Analyses — the uniform set

| Ideal (§3) | Current | Class | Host |
|-----------|---------|-------|------|
| `op` | `run_op` / `Module.op` | KEEP | B |
| `dc(src, pts) -> Trace` | `run_op_sweep -> Vec<OpResult>` | RESHAPE (Trace-returning) | B |
| `tran` | `run_tran` | KEEP (reshape kwargs) | B |
| `ac` | `run_ac` | KEEP (reshape kwargs) | B |
| `noise` | `run_noise` | KEEP (reshape kwargs) | B |
| `tf` | **solver only** (`analyses/mod.rs`), no host binding | BUILD/NEW (host) | B |
| `sens` | `run_sens` (Rust); Python typed class? **no** | BUILD (Python typed) | P |
| `pss` | `run_pss` (Rust); Python **no typed class** | BUILD (Python) | P |
| `pz` | `run_pz` (Rust); Python **no typed class** | BUILD (Python) | P |
| `disto` | `run_disto` (Rust); Python **no typed class** | BUILD (Python) | P |
| `sp` | `run_sp` (Rust); Python **no typed class** | BUILD (Python) | P |
| `four` (on a Trace) | `_FourierResult` exists | KEEP | B |
| **kwargs-first call shape** | Config classes (opaque) + Rust positional args | RESHAPE | B |

> **Biggest single finding:** the post-2026-07-18 analyses (`sens`/`pss`/`pz`/
> `disto`/`sp`) exist Rust-host-side but have **no typed Python result class** —
> a live MD-22 uniformity breach. `tf` exists only in the solver.

## 3. Configs + units

| Ideal (§3) | Current | Class | Host |
|-----------|---------|-------|------|
| kwargs on the analysis (`stop=`, `points=`) | `TranConfig`/`AcConfig`/... classes; Rust positional | RESHAPE | B |
| optional reusable `Config` + `.with_()` | Config classes exist but opaque (`inspect` shows no fields) | RESHAPE (typed `__init__`) | P |
| `Solver` knob set canonical | Rust `SolverConfig` has `dc_damp_tolerance`; Python `Solver` doesn't; nodeset asymmetry | RESHAPE (align) | B |
| `Solver` vs `SolverConfig` one name | two names | RENAME | B |
| SI helpers `pip.Hz/ns/mV/C(27)` | none | NEW | B |
| **Rust typed-unit `Into`** (`Freq: From<&str>` so `"10MHz"→1e7`, also `f64`, also helpers) | none | NEW (Rust ergonomics; Python mirrors by accepting `str` in the same slots) | B |

## 4. Session (center of gravity) + sweeps + live

| Ideal (§4–5) | Current | Class | Host |
|-------------|---------|-------|------|
| `Session` = compiled center | Python `LiveSession`; Rust none (only `run_op_sweep`) | RENAME (Py) / BUILD (Rust) | B |
| `Module.<analysis>` = compile-and-run sugar | Python `Module.op/tran/...`; Rust `SimSession` staged | RESHAPE | B |
| `session.set` / `schedule_set` / `rebuilds` | `LiveSession` has them (Py); Rust on `TransientSolver` | BUILD (Rust Session) | B |
| `sweep(knob, pts)` fluent, nested, `.map()->ndarray` | `run_op_sweep` single-knob `Vec` only | RESHAPE/NEW | B |
| `SweepPoint` is a `Session` view | none | NEW | B |

## 5. Results + measurements

| Ideal (§6) | Current | Class | Host |
|-----------|---------|-------|------|
| `Trace[T]` generic (one container) | separate `_Trace` + `_AcTrace` + `_NoiseTrace` | RESHAPE (consolidate) | B |
| noise = `Trace` + `.psd/.total/.by_source` | `_NoiseTrace` separate | RESHAPE | B |
| `Waveform` reductions (`min/max/mean/rms/ptp`) | exist | KEEP | B |
| `Waveform` measurements (`slew_rate`, `overshoot`, `settling_time`, `rise/fall_time`, `delay`) | none | NEW | B |
| `Waveform` transforms (`fft`, `resample`, `derivative`, `integral`, `clip`) | none | NEW | B |
| `ComplexWaveform` measures (`bandwidth_3db`, `gain/phase_margin`, `unity_gain_freq`) | `mag/phase/db` only | NEW | B |
| `Waveform.plot` / `pip.plot` / `pip.bode` | none | NEW | B |
| `OpResult["x1"] -> InstanceView` | Python yes; Rust no | BUILD (Rust) | R |
| `OpResult.opvar("gm")` / `.opvars()` | element-abi opvar bridge shipped; **no host surface** | NEW | B |
| typed `SensResult`/`PzResult`/`DistoResult`/`SParamResult`/`PssResult`/`TfResult` | Rust returns structs; Python untyped/absent | BUILD/NEW | B |
| `__len__`, property-vs-method consistency | inconsistent (`values` prop, `len()` method, no `__len__`) | RESHAPE | P |

## 5.5 Device introspection reflection (element-abi door — mostly missing)

The `element-abi-maturity` catalogs (shipped 2026-07-23) have almost no host
door. High-value, low-risk (read-only bridges over data that already exists).

| Ideal (§6.5) | Current | Class | Host |
|-------------|---------|-------|------|
| `inst.opvar(name)` / `inst.opvars()` | ABI `read_opvars` shipped; `InstanceView` has no opvar | BUILD/NEW | B |
| `trace.opvar("x1.p_out")` (recorded) | `ProbeSelection` shipped; no host `probe=`/`trace.opvar` | NEW | B |
| `inst.model` (`ModelDescriptor`) | ABI shipped; no host accessor | NEW | B |
| `inst.terminals` w/ `TerminalKind` | ABI `list_terminals`+kind shipped; host `terminals()` lacks kind | RESHAPE/BUILD | B |
| `inst.observables()` (discover probeable) | ABI `list_observables`+cost shipped; no host door | NEW | B |
| `op.stats.limiting` (`LimitingReport`) | ABI shipped; no host access | NEW | B |
| `nz.by_source()` / `.contributions()` (`NoiseContribution`) | ABI per-source shipped; `NoiseTrace` has `total` only | BUILD/NEW | B |
| `SolverStats` recent fields exposed | Rust struct has them; Python `_SolverStats` parity? verify | RESHAPE | B |
| `Param.bounds/.unit/.scope/.invalidation` | `ParamDescriptor` shipped; host `params()` exposes name/value only | BUILD | B |
| optimizer reads knob bounds from `Param.bounds` | none | NEW | B |

> **Theme:** the engine computes a rich introspection catalog; the host exposes
> a sliver (`v`/`i`). This is the single highest-leverage cluster — read-only
> bridges that unlock opvars (efficiency/power optimization — §0 driving
> scenario), convergence debugging, probe discovery, and auto knob bounds.

## 6. Errors, validation, optimization, plugins, discoverability

| Ideal (§7–8, §10) | Current | Class | Host |
|------------------|---------|-------|------|
| `pip.SimulationError` hierarchy (`ConvergenceError` w/ `.node/.iteration`) | Python `ValueError`/`KeyError` strings; Rust typed `Error` enum | NEW (Py hierarchy; align Rust) | B |
| `NetRef` ergonomics (`impl From<&str>`, `v(impl Into<NetRef>)`) | bare `NetRef { name }` (Rust) | RESHAPE | R |
| `cross`/`scale`/`dir` enums both sides | `cross(dir: &str)` string; `Scale` enum exists | RESHAPE | B |
| `pip.extract(trace, {...})` | died with the bench | NEW | B |
| `pip.optimize(...)` (design centering) | none (P6 under study) | NEW (P6-gated) | B |
| Python plugin scripting `@pip.device/@pip.hook` | none (P5 lifecycle-registry-to-Python) | NEW (P5-adjacent) | P |
| complete `.pyi` stubs + docstrings | facade docstringed (bench-removal); stubs incomplete | RESHAPE/NEW | P |
| **parity test locking both surfaces** | none | NEW | B |

---

## 7. Rollup — what the P3 refinement becomes

The gap clusters into workstreams (candidate feature/story boundaries):

1. **Uniform analysis surface (MD-22 core).** Bind `sens`/`pss`/`pz`/`disto`/
   `sp` as typed result objects on **Python** (Rust has them); add `tf` on both.
   Kwargs-first call shape. Reshape `dc` to Trace-returning. — *closes the
   biggest breach.*
2. **`Session` as center.** Rename `LiveSession`→`Session`; build the Rust
   `Session` equivalent (owns compiled circuit, `set`/`schedule_set`/analyses).
3. **First-class sweeps.** Fluent `sweep()`, nested/named, `SweepPoint`-as-view,
   `.map()->ndarray`.
4. **Rich Waveform.** Measurements + transforms + `plot`; `ComplexWaveform`
   margins/bandwidth; `pip.plot`/`bode`.
4b. **Device introspection reflection (highest leverage).** Host door over the
   shipped element-abi catalogs: `inst.opvar`/`opvars`/`observables`/`model`/
   `terminals`(+kind), `trace.opvar` via `probe=`, `op.stats.limiting`, noise
   `by_source`/`contributions`, `Param.bounds`/`unit`/`scope`. Unlocks the §0
   efficiency/opvar driving scenario, convergence debugging, probe discovery,
   and auto knob-bounds for the optimizer — all read-only bridges, low risk.
5. **Return-type consolidation.** `Trace[T]` generic; fold `AcTrace`/`NoiseTrace`.
6. **Configs + units.** Typed kwargs/`__init__`, canonical `Solver` knobs, SI
   helpers, Rust typed-unit `Into`.
7. **Errors + discoverability.** `SimulationError` hierarchy, `NetRef` ergonomics,
   enums for `cross`/`dir`, `.pyi` stubs, property/method consistency, the
   **parity test**.
8. **Host helpers.** `extract` (validation); `optimize` (P6-gated); Python plugin
   scripting (P5-adjacent).
9. **Naming cleanup.** `const_`→`const`, method→property reflection,
   `design[name]`, `load_str`.

**Reality check vs the roadmap's current P3 bullets:** the existing P3 list
(`uniform-host-api`, `plot`, `fft`, `resample`, `extract`, ergonomics,
`HookInput.solve`, packaging) is a *subset* — it misses the Python-lags-Rust
analysis-binding breach (#1), the `Session`-center + Rust `Session` (#2),
first-class sweeps (#3), Waveform measurements + margins (#4), **the entire
element-abi introspection door — opvars/observables/limiting/noise-by-source/
param-bounds (#4b, highest leverage)**, the return-type consolidation (#5), SI
units (#6), the typed error hierarchy + parity test (#7). The refinement
replaces the flat bullet list with these workstreams.
