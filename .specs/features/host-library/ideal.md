# Host Library — the ideal surface (north star)

> **Purpose.** Design the *perfect* `import piperine` (and its Rust mirror)
> greenfield — the surface a low/medium-level designer would love — BEFORE
> measuring the delta against what ships today. This doc is the target; the
> delta (`delta.md`) maps ideal → current → gap, and the ROADMAP P3 refinement
> follows from it.
>
> **Locked design decisions (user 2026-07-23):**
> 1. Analyses take **kwargs directly** (`sim.tran(stop=1e-3, step=1e-6)`); a
>    reusable Config object is optional, never mandatory.
> 2. Numbers are **plain floats + explicit SI helpers** (`pip.Hz("10M")`,
>    `pip.ns(5)`) — no magic string-unit parsing (`"1m"` is NOT accepted).
> 3. The **compiled `Session` is the center of gravity** — `Module.<analysis>`
>    is a compile-and-run shortcut; sweeps, live edits, optimization, and any
>    repetition live on the compiled `Session`.
> 4. Waveform **measurements are methods on the waveform**
>    (`wf.bandwidth_3db()`), discoverable via autocomplete, chainable.
>
> **Governing rule MD-22:** the surface is identical in Python and Rust — same
> call shape, same names, same config/result types. Every Python snippet below
> has a 1:1 Rust mirror (§9).

---

## 0. Driving scenario — efficiency optimization via opvar

The end-to-end case the whole surface must serve: a designer defines a
*readout* variable (component power — a `var` that computes, contributes to no
branch), reads it from the host, and optimizes a derived figure (efficiency).

```phdl
// in the power-stage module — pure opvars (no branch contribution)
var p_out = V(out) * I(out)    @name("p_out") @unit("W")
var p_in  = V(vdd) * I(vdd)    @name("p_in")  @unit("W")
```

```python
sim = design["converter"].compile()

# DC point — instantaneous power
op  = sim.op()
eff = op["x1"].opvar("p_out") / op["x1"].opvar("p_in")

# transient (switching converter) — cycle-average power
tr  = sim.tran(stop=T, probe=["x1.p_out", "x1.p_in"])   # record named observables
eff = tr.opvar("x1.p_out").mean() / tr.opvar("x1.p_in").mean()

# optimize efficiency — knob bounds pulled from the param descriptors
def efficiency(s):
    t = s.tran(stop=T, probe=["x1.p_out", "x1.p_in"])
    return t.opvar("x1.p_out").mean() / t.opvar("x1.p_in").mean()

best = pip.optimize(sim, knobs=["l1.l", "c1.c"], objective=efficiency, maximize=True)
#                          └─ bounds auto-read from Param.bounds (§8.5); or knobs={"l1.l": (lo,hi)}
```

This scenario exercises: opvar authoring → **opvar host access** (`op[...].opvar`,
`trace.opvar`) → observable recording (`probe=`) → param-bound reflection →
`optimize`. Every arrow is a delta item — this is the north-star validation.

---

## 1. The feel — six principles

1. **One central object: the compiled `Session`.** Compile once, hang
   everything off it. `Module.<analysis>` is sugar that compiles-and-runs for
   the one-shot case.
2. **Analyses are uniform and complete.** Every analysis is one method with the
   same call shape and a typed result object. The full set ships on both hosts:
   `op`, `dc`, `tran`, `ac`, `noise`, `tf`, `sens`, `pss`, `four`, `pz`,
   `disto`, `sp`.
3. **Results are numpy-native and rich.** A waveform plots itself, measures
   itself, transforms itself. No reaching into internals.
4. **Sweeps and live edits are first-class**, not host gymnastics — compile-once
   restamp underneath (MD-18).
5. **Discoverable.** Typed `__init__`/kwargs, complete `.pyi` stubs, docstrings
   — autocomplete shows every field.
6. **Fail loud, typed.** A `piperine.SimulationError` hierarchy hosts can catch;
   never a silent zero.

---

## 2. Entry + reflection

```python
import piperine as pip

design = pip.load("amp.phdl")          # load + elaborate .phdl/.ppr
design = pip.load_str("module amp {…}") # inline source (REPL/tests/docs)

amp  = design["amp"]                    # __getitem__ → Module (raises pip.UnknownModule)
top  = design.top                       # property, Module | None
mods = design.modules                  # property → list[Module]
gm0  = design.const("GM0")             # a global constant

# reflection (plain records, all properties)
amp.name; amp.ports; amp.params; amp.nets; amp.instances; amp.behaviors
design.select("x1.m*").nodes           # selector → nodes
```

`const` not `const_`; `design[name]` not `design.module(name)`; reflection is
properties, not zero-arg methods (§8 consistency).

---

## 3. Analyses — one shape, kwargs-first

Every analysis is a method on both `Module` (compile-and-run shortcut) and
`Session` (on the already-compiled circuit). Same signature on both.

```python
op    = amp.op()                                   # -> OpResult
dcs   = amp.dc("v1", pip.linspace(0, 5, 51))       # -> Trace[Waveform]  (swept over v1)
tr    = amp.tran(stop=1e-3, step=1e-6, ic={"out": 0.0})   # -> Trace[Waveform]
acr   = amp.ac(fstart=1e3, fstop=1e9, points=100, scale="dec")  # -> Trace[ComplexWaveform]
nz    = amp.noise(out="out", ref="in", fstart=1e3, fstop=1e9, points=100)  # -> Trace (noise)
tfr   = amp.tf(out="out", src="v1")                # -> TfResult (gain, Zin, Zout)
sens  = amp.sens(out="out")                        # -> SensResult
pss   = amp.pss(period=1e-6)                       # -> PssResult (Trace over one period + PssStats)
harm  = tr.four(f0=1e3, harmonics=9)               # -> FourierResult (post-processing on a Trace)
poles = amp.pz(kind="pz")                          # -> PzResult (poles, zeros)
dist  = amp.disto(f1=1e6, f2=1.1e6)                # -> DistoResult (HD2/HD3/IM2/IM3)
spar  = amp.sp(fstart=1e6, fstop=1e9, points=201)  # -> SParamResult (S-matrix)

solver = pip.Solver(reltol=1e-4, temperature=pip.C(27))   # optional, reusable
tr = amp.tran(stop=1e-3, solver=solver, nodeset={"out": 1.2})
```

**Config object is optional, for reuse:**

```python
base = pip.TranConfig(stop=1e-3, step=1e-6)        # typed, IDE-visible fields
tr1  = sim.tran(base)
tr2  = sim.tran(base.with_(stop=2e-3))             # immutable copy-with
```

**SI helpers (no string-unit magic):**

```python
pip.Hz, pip.kHz, pip.MHz, pip.GHz      # frequency
pip.ns, pip.us, pip.ms                 # time
pip.mV, pip.uA, pip.pF, pip.kOhm       # electrical
pip.C(27) -> 300.15                    # Celsius → Kelvin
sim.tran(stop=pip.ms(1), step=pip.us(1))
sim.ac(fstart=pip.kHz(1), fstop=pip.GHz(1))
```

---

## 4. The compiled `Session` — center of gravity

```python
sim = amp.compile()          # -> Session (owns the compiled circuit)

# live edits — restamp, no re-JIT (structural edits auto-rebuild, counted)
sim.set("m1.w", 4e-6)
sim.schedule_set(t=pip.us(5), label="v1", param="dc", value=1.8)   # breakpoint-exact
sim.rebuilds                 # property: structural rebuild count

# every analysis, on the compiled circuit
op = sim.op()
tr = sim.tran(stop=1e-3)
sim.last_stats               # SolverStats from the most recent run, always present
```

---

## 5. Sweeps — first-class, vectorized, compile-once

```python
# single-knob
for pt in sim.sweep("r1.r", pip.logspace(1e3, 1e5, 10)):
    print(pt.value, pt.op().v("out"))

# nested, named
grid = sim.sweep(temp=[-40, 27, 125], vdd=[3.0, 3.3, 3.6])
for pt in grid:
    print(pt.temp, pt.vdd, pt.ac(fstart=1e3, fstop=1e9).v("out").db().max())

# vectorized collect (numpy out)
gains = grid.map(lambda s: s.ac(fstart=1e3, fstop=1e9).v("out").db().max())
# gains.shape == (3, 3)
```

A `SweepPoint` IS a `Session` view at that operating point — every analysis
works on it. `grid.map(fn)` returns an ndarray shaped like the sweep axes.

---

## 6. Results + measurements (methods on the object)

**Return-type taxonomy — the rule:** *separate a type when it changes the
available operations; unify when only the data values differ.* This holds
identically in both hosts (MD-22). Nine result types survive the cut:

| Type | Why it stands alone (its own operations) |
|------|-------------------------------------------|
| `Waveform` | real-signal ops: time-weighted `rms`/`mean`, `slew_rate`, `overshoot`, `cross` |
| `ComplexWaveform` | complex-only ops: `mag`/`phase`/`db`, `bandwidth_3db`, margins |
| `Trace[T]` | **one generic container** for every swept signal set — tran/dc → `Trace[Waveform]`, ac → `Trace[ComplexWaveform]`, noise → `Trace` + noise methods. Same `.v()`/`.i()`/`.axis()`; the sample type is the only difference (kills the old `AcTrace`/`NoiseTrace` split) |
| `OpResult` | point values + `InstanceView` indexing + opvars |
| `TfResult` | `gain`/`z_in`/`z_out` scalars |
| `SensResult` | `(out, param) → value` map |
| `PssResult` | one-period `Trace` + `settle_time` + `PssStats` |
| `PzResult` | pole list + zero list |
| `DistoResult` | `hd2`/`hd3`/`im2`/`im3` vs frequency |
| `SParamResult` | S-matrix of `ComplexWaveform` + `z0` |

(`SParamResult`/`PssResult` shown once here; the trio collapse is the only
consolidation — the structured results stay distinct because merging them would
produce a bag of `Optional` fields, not a shared surface.)

```python
wf: Waveform
  .values / .axis                      # numpy arrays
  .at(x) · .cross(level, "rising")     # point/edge queries (enum-or-str dir)
  .min / .max / .mean / .rms / .ptp    # time-weighted reductions
  # transforms → new Waveform
  .fft() · .resample(grid) · .clip(t0, t1) · .derivative() · .integral()
  # measurements (methods, chainable, discoverable)
  .rise_time() · .fall_time() · .slew_rate()
  .overshoot() · .settling_time(tol=0.01) · .delay(other, level)
  .plot(ax=None)                       # matplotlib, one line

cw: ComplexWaveform
  .mag · .phase · .db                  # → Waveform
  .bandwidth_3db() · .gain_margin() · .phase_margin() · .unity_gain_freq()
  .at(x) · .plot()

tr: Trace[T]                           # one generic container, T ∈ {Waveform, ComplexWaveform}
  .v(a, b=None) -> T · .i(a, b=None) -> T   # sample type follows the analysis
  .axis()  · .stats  · .four(f0, harmonics)
  # noise trace = Trace + noise-specific views (same container, extra methods):
  .psd() -> Waveform · .total() -> float · .by_source() -> dict[str, Waveform]

op: OpResult
  .v(a, b=None) · .i(a, b=None)
  op["x1.m1"] -> InstanceView          # .v/.i/.opvar("gm")/.terminals/.label
  .opvars() · .stats

sens: SensResult    → sens[out, "r1.r"] -> float   (typed indexing)
tf:   TfResult      → .gain · .z_in · .z_out
dist: DistoResult   → .hd2 · .hd3 · .im2 · .im3
spar: SParamResult  → spar.s(2, 1) -> ComplexWaveform · .z0
pss:  PssResult     → .waveform (one period, a Trace) · .settle_time · .stats
```

```python
pip.plot(tr.v("out"), tr.v("in"), title="step response")   # multi-signal
pip.bode(acr.v("out"))                                      # mag+phase pair
```

---

## 6.5 Device introspection (reflection) — the door to the element-abi catalogs

The `element-abi-maturity` ABI (2026-07-23) built rich per-device catalogs the
host must surface — almost none has a host door today. The whole reflective
surface hangs off an instance view:

```python
inst = op["x1"]                      # InstanceView

inst.model                           # ModelDescriptor → .type_id, .version
inst.terminals                       # [Terminal] → .name, .kind (external/internal/auxiliary), .domain
inst.opvars()                        # {name: value} — computed op vars (gm, p_out, …)
inst.opvar("p_out")                  # one, by name  ·  inst["gm"] shorthand
inst.observables()                   # what CAN be probed → [Observable(name, kind, cost)]
inst.params                          # [Param] → .name, .value, .unit, .bounds, .scope
inst.v(a, b) · inst.i(a, b)          # terminal quantities (already exist)

# convergence diagnostics (LimitingReport) — why a step limited
op.stats.limiting                    # [LimitingReport] → .device, .net, .proposed, .limited_value, .limiter_name, .reason

# noise, per source (NoiseContribution)
nz = sim.noise(out="out", ref="in", fstart=1e3, fstop=1e9)
nz.total()                           # integrated output noise
nz.by_source()                       # {"r1/thermal": Waveform, "m1/flicker": Waveform, …}
nz.contributions()                   # [NoiseContribution] → .element, .source, .kind, .psd, .integrated_sq
```

**`SolverStats` — surface the recent fields** (perf/convergence deliverables):
`converged`, `newton_iterations`, `homotopy_strategy`/`homotopy_levels`,
`steps_accepted`/`steps_rejected`, `dt_min`/`dt_max`/`dt_min_floor_hits`,
`bypass_hits`/`bypass_misses`, `assembly_time_ns`/`solve_time_ns`, `limiting`.

### 8.5 Param reflection feeds the optimizer

`Param.bounds`/`.unit`/`.scope`/`.invalidation` come from the shipped
`ParamDescriptor`. The optimizer reads knob ranges straight from the model — the
user names knobs, bounds are inferred:

```python
p = amp.param("m1.w")
p.value · p.unit · p.bounds          # (lo, hi) → optimizer default range
p.invalidation                        # Restamp | Rebuild | Temperature (host knows what a set() costs)
pip.optimize(sim, knobs=["m1.w", "r1.r"], objective=...)   # bounds auto from Param.bounds
```

---

## 7. Validation + optimization (what the bench used to do)

```python
# spec assertions read like English
bw = acr.v("out").bandwidth_3db()
assert bw > pip.MHz(10), f"bandwidth too low: {bw/1e6:.1f} MHz"

# extract → dict of named measurements (test-bench shaped)
m = pip.extract(tr, {
    "slew":     lambda w: w.v("out").slew_rate(),
    "overshoot":lambda w: w.v("out").overshoot(),
})

# optimization (P6) — same live engine, design centering
result = pip.optimize(
    sim,
    knobs    = {"m1.w": (1e-6, 20e-6), "r1.r": (1e3, 1e5)},
    objective= lambda s: s.ac(fstart=1e3, fstop=1e9).v("out").bandwidth_3db(),
    maximize = True,
    method   = "worst_case",          # | "monte_carlo" | "nelder_mead"
)
result.best_params · result.history · result.apply(sim)
```

---

## 8. Errors + discoverability

```python
class SimulationError(Exception): ...          # base — catch-all
class ElaborationError(SimulationError): ...    # parse/elab
class ConvergenceError(SimulationError):        # .node, .iteration, .analysis
class UnknownModule(SimulationError): ...
class UnknownNet(SimulationError): ...

try:
    sim.tran(stop=1e-3)
except pip.ConvergenceError as e:
    print(e.analysis, e.node, e.iteration)
```

- Complete `.pyi` stubs → every kwarg (`stop=`, `step=`, `ic=`, `solver=`)
  autocompletes with type + default.
- Every public object docstringed; `help(pip.Session)` is the manual.
- Property-vs-method convention consistent: **data is a property**
  (`wf.values`, `design.top`, `sim.rebuilds`), **actions are methods**
  (`wf.rms()`, `sim.tran()`). `len(wf)` works (`__len__`).

---

## 9. Rust mirror (MD-22 — identical shape)

```rust
let design = pip::load("amp.phdl")?;
let amp    = &design["amp"];                       // Index → Module
let sim    = amp.compile()?;                        // Session, the center

let tr = sim.tran(Tran::new().stop(1e-3).step(1e-6))?;   // builder = kwargs
let v  = tr.v("out")?;                               // impl Into<NetRef> for &str
let bw = sim.ac(Ac::new().f(1e3, 1e9).points(100))?.v("out")?.bandwidth_3db();

for pt in sim.sweep("r1.r", &logspace(1e3, 1e5, 10)) {
    println!("{}", pt.op()?.v("out")?);
}
```

- `v("out")` / `v(("out","in"))` via `impl Into<NetRef> for &str`/tuples — no
  bare `NetRef { name }` construction.
- Builders mirror Python kwargs (`Tran::new().stop(..)`); `cross`/`scale`/`dir`
  are enums on both sides.
- `Session` is the Rust center too (owns the compiled circuit; `set`/
  `schedule_set`/`sweep`/analyses) — resolves the appendix §4-R1 asymmetry.
- **Same nine return types, same cut.** The consolidated taxonomy (§6) is the
  Rust taxonomy: `Trace<T>` is genuinely generic in Rust (`Trace<Waveform>` /
  `Trace<ComplexWaveform>`), and the same `Waveform`/`ComplexWaveform`/
  `OpResult`/`TfResult`/`SensResult`/`PssResult`/`PzResult`/`DistoResult`/
  `SParamResult` set stands on both hosts — no host has a type the other lacks.
- Same `SimulationError`-shaped taxonomy (typed `Error` enum ↔ Python
  hierarchy).

**Uniformity is a hard requirement, not a nicety.** The bar (MD-22): every
public name, call shape, kwarg/builder field, config, result type, enum, and
error variant exists on BOTH hosts with the same spelling and semantics. A
surface item that lands on only one host is a defect. "Rust too, in principle"
means the Rust mirror is designed *with* the Python surface — same feature, same
review — never bolted on after. A parity test enforces the two `__all__`/public
surfaces stay in lockstep.

---

## 10. What "polished" means (the P3 bar, restated from the ideal)

- Every analysis on both hosts, one uniform shape (§3).
- Compiled-`Session`-centric, live + sweep + optimize native (§4–7).
- Waveform is a rich, self-measuring, self-plotting numpy object (§6).
- Nine return types, cut by the separate-vs-unify rule, identical on both
  hosts (§6).
- SI helpers, kwargs-first, optional configs (§3).
- Typed error hierarchy, complete stubs, full docstrings, consistent
  property/method conventions (§8).
- **Byte-identical Python/Rust surface — a hard requirement enforced by a
  parity test, Rust designed with Python not after (§9, MD-22).**
- `piperine.plot`/`bode` conveniences; `extract`/`optimize` host helpers.
- Python plugin scripting (register device/attr/hook from `.py`) — P5-adjacent,
  surfaced here for the scripting story.
```
