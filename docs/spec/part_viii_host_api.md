# Part VIII — Host APIs: Python and Rust

Driving a simulation is a **host** concern, not a language concern. PHDL
describes circuits (Parts I–II); hosts elaborate, compile, solve, and
measure. There are exactly two host surfaces, and they are one surface
(MD-22): same call shape, same names, same config/result types, enforced by
a parity test (`tests/host_parity.rs`).

- **Python** (`import piperine`) — the scripting host. Testbenches are plain
  Python files (`*_tb.py`), run by `piperine test`; scripts run with
  `piperine run script.py`; an interactive REPL is `piperine run -i`.
- **Rust** (`piperine-api`) — the same session/results/waveform plumbing the
  Python binding wraps (MD-20: `piperine-api` is the complete external view
  of the project; the root `piperine` crate is a thin re-export shell, so
  external hosts may equally write `use piperine::…`).
  `piperine_api::prelude` is the one-import face.

The in-language `bench` block was removed (2026-07-17): a `bench` block is a
plain syntax error, and the interpreted context no longer exists. Everything
it did — analyses, measurement, parameter sweeps, assertions — is done by a
host, in Python or Rust, with no new syntax.

## 1. The session model

There is exactly **one** session type in Rust — `Session` — and two ways to
reach an analysis, both hosts:

- **The staged workflow** — `Module` (Python) /
  `Session::builder(&design, module)` (Rust): configure the build (staged
  overrides, a device provider, lifecycle hooks), then `compile()`. Python's
  `Module` compiles one session per analysis call over a forked design with
  its staged overrides (`Module.set`) replayed. The right shape for one-shot
  runs and sweeps expressed as plain loops.
- **The compiled workflow** — `Session`, both hosts (HOST-01):
  `Module.compile()` / `Session::compile(&design, module)` elaborates and
  JITs **once**, then every subsequent analysis restamps the held circuit
  (`Session::set`/`schedule_set`, MD-18 — never re-elaborates, never
  re-JITs). This is the primitive optimization loops, sweeps, and live
  parameter studies are built on.

The staged and compiled workflows are two uses of one type, not two types:
staging is a build-time option (`SessionBuilder::stage`), because a write that
changes what gets *built* has to precede the build.

```python
import piperine as pip

design = pip.load("chip.phdl")           # -> Design (elaborated POM)
module = design["Amp"]                    # -> Module (reflected view + analyses; __getitem__)
op     = module.op()                      # -> OpResult   (staged: fresh build)

sim    = module.compile()                 # -> Session    (compiled once)
op2    = sim.op()                         # -> OpResult   (restamp, no re-JIT)
tr     = sim.tran(pip.TranConfig(stop=1e-3, step=1e-6))
wave   = tr.v("out")                      # -> Waveform (time axis)
wave.values                             # np.ndarray (real)
wave.axis                               # np.ndarray (time)
t_cross = wave.cross(2.5, pip.CrossDirection.Rising)  # float | None

acr    = sim.ac(pip.AcConfig(fstart=1.0, fstop=1e9))
mag    = acr.v("out").mag                 # ComplexWaveform -> Waveform (property)
ndb    = acr.v("out").db

nz     = sim.noise(pip.NoiseConfig(out="out", fstart=1.0, fstop=1e6))
psd    = nz.psd()                         # Waveform over frequency
total  = nz.total()                       # integrated RMS noise (float)
```

```rust
use piperine::prelude::*;

let design  = parse_and_elaborate(&src, &SourceMap::dummy())?;
let mut sim = Session::compile(&design, "Amp")?;          // compiled once
let op      = sim.op(&SolverConfig::default(), None)?;
let mid     = op.v("mid")?;                                 // impl NetSelector for &str
```

## 2. `load` and `Design`

`pip.load(path)` / `pip.load_str(src)` parse and elaborate a `.phdl`/`.ppr`
file (or inline source, no filesystem read) into a `Design` — or raise
`ElaborationError` (a `ValueError` subclass) with the diagnostic (never a
silent success). In Rust, `parse_and_elaborate`/`parse_and_elaborate_seeded`
(re-exported from `piperine_lang`) return `Result<Design, ElabError>`.

| Method (Python) | Returns | Notes |
|--------|---------|-------|
| `design.top` | `Module \| None` | property (HOST-24) |
| `design[name]` | `Module` | `__getitem__`, raises `UnknownModule` |
| `design.module(name)` | `Module` | raises `UnknownModule` |
| `design.modules()` | `list[Module]` | every elaborated module |
| `design.const(name)` | value or `None` | a global constant (HOST-24: `const`, not `const_`) |
| `design.select(path)` | `Selection` | Part IV selector path |
| `design.compile(module=None)` | `Session` | `None` compiles the unambiguous top |

A `Design` is read-only for the host: parameter overrides are staged per
`Module` and replayed onto a fork per analysis — the parent design is never
mutated. `Session::compile` (both hosts) forks once at compile time; further
overrides go through `Session::set`, not re-elaboration.

## 3. Analyses — the uniform set

Every analysis is a method on `Module` (compile-and-run, Python) and on
`Session` (on the already-compiled circuit), same signature on both.
The full set, both hosts: `op`, `dc`, `tran`, `ac`, `noise`, `tf`, `sens`,
`pss`, `pz`, `disto`, `sp`, plus `four` as post-processing on a `Trace`.
`tests/host_parity.rs`'s `ANALYSES` constant is the canonical, executable
list.

```python
op    = sim.op(pip.OpConfig(nodeset={"out": 5.0}, solver=pip.Solver(reltol=1e-4)))
dcs   = sim.dc("v1", "dc", [0.0, 1.0, 2.0])            # -> Trace  (swept over v1.dc)
tr    = sim.tran(pip.TranConfig(stop=1e-3, step=1e-6, ic={"out": 0.0}))
acr   = sim.ac(pip.AcConfig(fstart=1e3, fstop=1e9, points=100, scale=pip.Scale.Dec))
nz    = sim.noise(pip.NoiseConfig(out="out", fstart=1e3, fstop=1e9))
tfr   = sim.tf(output="out", input_source="v1")         # -> TfResult (gain, z_in, z_out)
sens  = sim.sens(outputs=["out"], params=[("r2", "r")])  # -> SensResult
pss   = sim.pss(period=1e-6)                            # -> PssResult (Trace over one period + PssStats)
harm  = tr.four(f0=1e3, harmonics=9)                    # -> FourierResult (post-processing on a Trace)
poles = sim.pz(input_source="v1", output="out")          # -> PoleZeroResult (poles, zeros)
dist  = sim.disto(f1=1e6, amplitude=0.1, output="out")   # -> DistoResult (HD2/HD3/IM2/IM3)
spar  = sim.sp(fstart=1e6, fstop=1e9, points=201)        # -> SpResult (S-matrix)
```

```rust
let op    = sim.op(&config, None)?;
let dcs   = sim.dc("v1", "dc", &[0.0, 1.0, 2.0], &config, None)?;   // -> Trace<Waveform>
let tr    = sim.tran(1e-3, Some(1e-6), 0.0, &config, None, false, &[])?;
let acr   = sim.ac(1e3, 1e9, 100, true, &config)?;                  // -> Trace<ComplexWaveform>
let nz    = sim.noise("out", "gnd", 1e3, 1e9, 100, true, &config)?;
let tfr   = sim.tf("out", None, "v1", &config)?;                    // -> TfResult
let sens  = sim.sens(&["out"], &[("r2".into(), "r".into())], 1e-6, &config)?;
let pss   = sim.pss(1e-6, 0.0, &config)?;
let poles = sim.pz("v1", "out", None, &config)?;
let dist  = sim.disto(1e6, None, 0.1, "out", None, &config)?;
let spar  = sim.sp(1e6, 1e9, 201, true, &config)?;
```

- Config bundles are dataclasses in Python (`OpConfig`/`TranConfig`/
  `AcConfig`/`NoiseConfig`, plus `Solver` attached to each); Rust takes
  positional args plus a `&SolverConfig`. `step = 0.0`/`None` selects the
  adaptive stepper (a positive `step` seeds the initial `dt`); `start` is
  the earliest **recorded** time (the solver always integrates from `t=0`).
  `nodeset`/`ic` are `{net_name: volts}` maps seeding the Newton guess /
  t=0 state. `Solver`/`SolverConfig` fields: `temperature`, `reltol`,
  `abstol`, `gmin`, `max_iter`, `dc_damp_tolerance` — identical field set on
  both hosts (HOST-20). Every config dataclass carries `.with_(**overrides)`
  (Python, `dataclasses.replace` — immutable copy) and is fully typed
  (`inspect.signature` shows every field).
- `tran(..., record_device_state=True)` records per-step device runtime
  banks; `Trace.i` recomputes branch currents from the solved terminal
  voltages — with recording on, that also works for devices reading runtime
  state (`delay`/`transition`/`idt`); with it off, such a read is a loud
  `Measurement`/error naming the opt-in.
- `tran(probe=[...])` (HOST-08) records named opvar observables per step;
  `trace.opvar("x1.p_out")` returns a `Waveform`. An unknown probe target
  fails loud at setup.
- **`dc(label, param, values)`** (HOST-05) is a compile-once sweep: restamp
  `label.param` on the one compilation (MD-18), returning a `Trace` over the
  swept axis — read the same way as `tran`/`pss` (`.v`/`.i`/`.axis`). SPEC
  DEVIATION from `ideal.md`'s `dc(src, points)`: no stdlib ideal source ships
  a canonical "value" param name, so the two-arg spelling would silently
  assume one; `dc` takes the same `(label, param, values)` triple as
  `sweep`/`set`.
- **`sens`** — `∂V(output)/∂(param)` at the operating point, central finite
  difference over the compile-once restamp path. Same shape both hosts: the
  result maps `(output, "label.param") → float`, with a `get(output, label,
  param)` reader. Unknown nets/elements/params and rebuild-class parameters
  fail loud.
- **`pss`** — single shooting: one converged period as a normal transient
  trace plus diagnostics. The drive period is user-supplied (driven
  circuits; autonomous period detection is out of scope). `r.trace.v("out")`
  is a `Waveform` over one period; `r.stats.shoot_iterations`/`.residual`/
  `.estimated_settle_time` are the shooting diagnostics (the last computed
  from the dominant monodromy eigenvalue, `None` when shooting needed no
  Jacobian). Non-convergence names the iteration count and residual; an
  orbit that does not repeat at `2T` (non-periodic drive) is rejected; a
  mixed-signal circuit whose digital state closes only after `k` periods
  reports "circuit period appears to be k·T".
- **`pz`/`disto`/`sp`** — pole-zero, small-signal distortion (Volterra),
  and N-port S-parameters, all documented per-method above; every one fails
  loud on an unaddressable net/source, a non-positive frequency/amplitude,
  or (S-parameters) a module with no `@rfport` attributes.

`module.set(label, param, value)` (Python) /
`Session::builder(..).stage(label, param, value)` (Rust) stages an override
consumed by the compilation it is staged on — sweeps expressed as plain
loops:

```python
for rl in [2e3, 1e3, 500.0]:
    m = design.module("DividerBoard")
    m.set("r_bot", "r", rl)
    assert abs(m.op().v("mid") - 5.0 * rl / (3e3 + rl)) < 1e-6
```

## 4. The compiled `Session` — center of gravity

`Session` (HOST-01, both hosts) is the compiled center: elaborate + JIT
**once**, hold the circuit, restamp on every write — the primitive
optimization loops and parameter studies want natively.

```python
sim = design["Amp"].compile()        # -> Session (owns the compiled circuit)

sim.set("m1.w", 4e-6)                                     # live restamp — no re-JIT
sim.schedule_set(t=5e-6, label="v1", param="dc", value=1.8) # breakpoint-exact
sim.rebuilds                                               # property: structural rebuild count

op = sim.op()
tr = sim.tran(pip.TranConfig(stop=1e-3))
```

```rust
let mut sim = Session::compile(&design, "Amp")?;
sim.set("m1.w", 4e-6)?;
sim.schedule_set(5e-6, "v1", "dc", 1.8);
sim.rebuilds();
```

- `set`/`schedule_set` address instances by their PHDL labels (bundle fields
  flatten to `{param}_{field}`, e.g. `model_is`); unknown names fail loud
  (Python: `KeyError`/`UnknownNet`; Rust: `Error::Measurement`), listing the
  element's parameters; out-of-bounds values fail loud with the solver's
  own message.
- `schedule_set` lands exactly on its timestamp (forced breakpoint);
  same-parameter sets apply in scheduling order (last write wins).
- **SPEC_DEVIATION** (both hosts document this differently — see T3's
  status note in `.specs/features/host-library/tasks.md`): Python's
  `Session.set` auto-rebuilds on a structural write and counts it in
  `rebuilds` (the ideal.md behavior). The Rust `Session::set` instead
  **fails loud** on a structural (`Invalidation::Rebuild`) write — `rebuilds()`
  stays part of the surface (currently always `0` from `set`) for a future
  auto-rebuild follow-up; a fresh `Session::compile` (or
  `Session::builder(..).stage(..).compile()`) is the workaround today. `Session::sweep`/`sweep_grid` (§5) *do* auto-rebuild on
  a structural knob, scoped to the sweep path only.

## 5. Sweeps — first-class, compile-once

```python
for pt in sim.sweep("r1", "r", [1e3, 1e4, 1e5]):
    print(pt.op().v("out"))

grid = sim.sweep_grid({"temp.value": [-40, 27, 125], "vdd.dc": [3.0, 3.3, 3.6]})
for pt in grid:
    print(pt.ac(pip.AcConfig(fstart=1e3, fstop=1e9)).v("out").db.max())

gains = grid.map(lambda s: s.ac(pip.AcConfig(fstart=1e3, fstop=1e9)).v("out").db.max())
# gains.shape == (3, 3), a numpy.ndarray
```

```rust
let mut it = sim.sweep("r1", "r", &[1e3, 1e4, 1e5]);
while let Some(pt) = it.next() {
    let pt = pt?;
    println!("{}", pt.op(&config, None)?.v("out")?);
}

let mut grid = sim.sweep_grid(&[("temp", "value", &[-40.0, 27.0, 125.0]), ("vdd", "dc", &[3.0, 3.3, 3.6])]);
let gains: Nested<f64> = grid.map(|pt| pt.ac(1e3, 1e9, 100, true, &config).unwrap().v("out").unwrap().max());
```

A `SweepPoint` IS a `Session` view at that operating point (Python: attribute
delegation; Rust: `Deref`/`DerefMut` to `Session`) — every analysis works on
it directly. `sweep_grid` (HOST-19) visits the cartesian product of named
axes in row-major order; `Grid.map(fn)` collects into an axis-shaped
`numpy.ndarray` (Python) / `Nested<R>` (Rust — a `Branch`/`Leaf` tree shaped
like `Grid::shape()`, generic over the mapped result type, since Rust has no
runtime ndarray dependency in `piperine-api`). A structural knob auto-
rebuilds mid-sweep and counts it in `rebuilds` (never a silently-wrong
restamp); values match per-point fresh builds. Axes are addressed
`"label.param"` (not bare kwargs — a dotted path is not a valid Python
identifier, and PHDL parameters are addressed by flat instance label
regardless).

## 6. Device introspection — the opvar/observable door

The `element-abi-maturity` catalogs (opvars, observables, terminals+kind,
model descriptor, limiting reports, per-source noise, param bounds) are
reachable from an `InstanceView`, obtained by indexing a solved `OpResult`
or `Trace`:

```python
op = sim.op()
inst = op["x1"]                      # InstanceView

inst.opvar("p_out")                  # one computed opvar, by name
inst.opvars()                        # [(name, value), ...] — every computed opvar
inst.model                           # ModelDescriptor -> .type_id, .version
inst.terminals                       # [TerminalDescriptor] -> .name, .kind, .domain, .direction
inst.observables()                   # [ObservableDescriptor] -> .name, .kind, .cost
inst.params()                        # [ParamDescriptor] -> .name, .bounds, .unit, .scope, .invalidation
inst.param("m1.w")                   # one ParamDescriptor, by name
inst.v("p", "n") · inst.i("p", "n")  # terminal quantities (pre-existing)
```

An unknown opvar/param name raises `UnknownNet`/`Error::Measurement` — fail
loud, never `None`/`NaN`.

**Recorded observables over a transient** (HOST-08): `tran(probe=[...])`
records named opvars per step; `trace.opvar("x1.p_out")` returns a
`Waveform` you can `.mean()`/measure like any other:

```python
tr  = sim.tran(pip.TranConfig(stop=T), probe=["x1.p_out", "x1.p_in"])
eff = tr.opvar("x1.p_out").mean() / tr.opvar("x1.p_in").mean()
```

**Convergence diagnostics** (HOST-10) — why a Newton step limited:

```python
op.stats.limiting   # [LimitingReport] -> .device, .net, .proposed, .limited_value, .limiter_name, .reason
```

Empty when nothing limited (the common case at a converged operating
point) — the shipped `SolverStats.limiting: Vec<LimitingReport>` field,
collected at the end of the DC solve.

**Per-source noise** (HOST-11):

```python
nz = sim.noise(pip.NoiseConfig(out="out", fstart=1e3, fstop=1e9))
nz.total()          # integrated output noise (float)
nz.by_source()       # {"element/source": Waveform, ...} — per-source PSD
nz.contributions()   # [NoiseContribution] -> .element, .source, .kind, .integrated_sq
```

`sum(contribution.integrated_sq for ...)` reconciles with `total()²`
(conservation).

**Param reflection feeds a future optimizer** (HOST-12):

```python
p = op["x1"].param("m1.w")
p.bounds          # (lo, hi) | (None, None)
p.unit            # str | None
p.scope           # "instance" | "module" | ...
p.invalidation     # "restamp" | "rebuild" | ...
```

**SolverStats** — always present on every analysis result's `.stats`:
`converged`, `newton_iterations`, `homotopy_strategy`/`homotopy_levels`,
`steps_accepted`/`steps_rejected`, `dt_min`/`dt_max`/`dt_min_floor_hits`,
`bypass_hits`/`bypass_misses`, `assembly_time_ns`/`solve_time_ns`,
`limiting`.

**SPEC_DEVIATION** (ideal.md §6.5 vs. delivered, HOST-12): the ideal access
path is module-level (`amp.param("m1.w")`); the delivered path is
instance-scoped (`op.instance("m1").param("w")` /
`op["m1"].param("w")`) — `ParamDescriptor` is per-device and only available
after compilation, matching the shipped `Introspect::list_params` ABI and
the same instance-scoped pattern `opvar` already uses.

## 7. Return types — the nine-type taxonomy

*Separate a type when it changes the available operations; unify when only
the data values differ.* Nine types, identical set on both hosts (no host
has a type the other lacks):

| Type | Stands alone because |
|------|-----------------------|
| `Waveform` | real-signal ops: time-weighted `rms`/`mean`, `slew_rate`, `overshoot`, `cross` |
| `ComplexWaveform` | complex-only ops: `mag`/`phase`/`db`, `bandwidth_3db`, margins |
| `Trace<T>` | **one generic container** for every swept signal set — `tran`/`dc` → `Trace<Waveform>`, `ac` → `Trace<ComplexWaveform>`, `noise` → `Trace` + noise methods. Same `.v()`/`.i()`/`.axis()`/`.stats`/`.four()`; the sample type is the only difference. On the **Rust** side `AcTrace`/`NoiseTrace` are now genuine **type aliases** (`Trace<ComplexWaveform>`, `Trace<NoiseSample>` — `NoiseSample` a zero-sized discriminator, since noise has no per-net `v`/`i`, only `psd`/`total`/`by_source`/`contributions`), not separate types — `HOST-13` folded them in. **SPEC_DEVIATION** (Python, T7): `_AcTrace`/`_NoiseTrace` stay distinct native `#[pyclass]` types in `piperine-python` (PyO3 pyclasses cannot be generic over a Rust type parameter the way `Trace<T>` is), each wrapping the corresponding Rust `Trace<ComplexWaveform>`/`Trace<NoiseSample>` internally and exposing the same method surface a caller would see through a generic `Trace` — the *shape* is uniform even though the Python class identity is not literally one generic class. |
| `OpResult` | point values + `InstanceView` indexing + opvars |
| `TfResult` | `gain`/`z_in`/`z_out` scalars |
| `SensResult` | `(output, "label.param") → value` map |
| `PssResult`/`PoleZeroResult`(`PzResult`)/`DistoResult`/`SpResult`(`SParamResult`) | structured, analysis-specific fields — kept distinct rather than merged into a bag of `Optional` fields |

```python
wf: Waveform
  .values / .axis                        # numpy arrays
  .at(x) · .cross(level, dir)             # point/edge queries (CrossDirection enum or legacy str)
  .min() / .max() / .mean() / .rms() / .peak_to_peak()   # time-weighted reductions
  # transforms -> new Waveform (Rust-only today, HOST-15; see §9)
  .fft() · .resample(grid) · .clip(t0, t1) · .derivative() · .integral()
  # measurements (Rust-only today, HOST-14; see §9)
  .rise_time() · .fall_time() · .slew_rate()
  .overshoot() · .settling_time(tol) · .delay(other, level)
  .plot(ax=None)                          # matplotlib, one line (Python, HOST-17)
  len(wf) / __len__                        # HOST-24

cw: ComplexWaveform
  .mag · .phase · .db                     # -> Waveform (properties)
  .bandwidth_3db() · .gain_margin() · .phase_margin() · .unity_gain_freq()   # Rust-only today
  .at(x) · .plot()

tr: Trace[T]                              # one generic container, T in {Waveform, ComplexWaveform}
  .v(a, b=None) -> T · .i(a, b=None) -> T    # sample type follows the analysis
  .axis() · .stats · .four(f0, harmonics) · .opvar(path) -> Waveform   # HOST-08
  # noise trace = Trace + noise-specific views (same container, extra methods):
  .psd() -> Waveform · .total() -> float · .by_source() -> dict[str, Waveform] · .contributions()

op: OpResult
  .v(a, b=None) · .i(a, b=None)
  op["x1"] -> InstanceView                 # .v/.i/.opvar/.opvars/.model/.terminals/.observables/.params
  .stats

sens: SensResult    -> .get(output, label, param) -> float | None
tf:   TfResult      -> .gain · .z_in · .z_out
dist: DistoResult   -> .hd2 · .hd3 · .im2 · .im3
spar: SpResult      -> .s (matrix) · .z0 · .frequencies · .n_ports  (Rust: SParamResult.s(k, i, j))
pss:  PssResult     -> .trace (one period, a Trace) · .stats (PssStats: shoot_iterations/residual/estimated_settle_time)
```

Unknown nets/opvars/params raise `UnknownNet`/`Error::Measurement` —
measurement failures are loud, never a silent `0.0` or NaN. Digital nets
read their logic value (0/1, X/Z as NaN) directly from `OpResult.v`/
`Trace.v`.

## 8. Configs, units, `Solver`

Config bundles (`OpConfig`/`TranConfig`/`AcConfig`/`NoiseConfig`) are typed
Python dataclasses with `.with_(**overrides)` (immutable copy,
`dataclasses.replace`); every field is autocomplete-visible
(`inspect.signature(TranConfig)`). `Solver` (Python dataclass) and
`SolverConfig` (Rust struct) carry the identical canonical knob set
(HOST-20): `temperature`, `reltol`, `abstol`, `gmin`, `max_iter`,
`dc_damp_tolerance`.

SI helpers (HOST-21, Python) — never magic-parse a raw `float`/`int`, only a
`str` argument:

```python
pip.Hz(1e6)      # == 1e6 (bare number, already Hz)
pip.Hz("10M")    # == 1e7 (SI prefix)
pip.Hz("10MHz")  # == 1e7 (SI prefix + unit-name suffix)
pip.ns(10)       # == 10e-9
pip.mV(300)      # == 0.3
pip.C(27)        # == 300.15  (Celsius -> Kelvin, for Solver.temperature)
```

Rust typed-unit `Into` (HOST-21): `Freq`/`Time` newtypes in
`piperine_api::units` — `Freq::from("10MHz") == Freq(1e7)`,
`Freq::from(1e7)` (bare `f64`, already Hz). `Session::ac`'s `fstart`/`fstop`
accept `impl Into<Freq>` as the representative demonstration (every
existing `f64` call site keeps compiling via the blanket `From<f64>`); a
malformed SI string panics (`From` is infallible). **SPEC_DEVIATION**: the
`Into<...>` retrofit is scoped to `Session::ac` today, not every
frequency/time-shaped argument across the analysis menu — the newtypes, SI-string parsing, and the Python `Hz`/`ns`/`mV`/
`C` helpers are fully delivered either way; widening the retrofit is a
separable mechanical follow-up.

## 9. Rust/Python coverage gaps (Phase 3 items still Rust-only)

HOST-14 (`Waveform` measurements: `slew_rate`/`rise_time`/`fall_time`/
`overshoot`/`settling_time`/`delay`) and HOST-16 (`ComplexWaveform` margins:
`bandwidth_3db`/`gain_margin`/`phase_margin`/`unity_gain_freq`) landed on
the Rust `piperine-api` `Waveform`/`ComplexWaveform` types only — their
respective tasks' gate was "quick (api)", not a Python-binding requirement.
HOST-15 (`fft`/`resample`/`derivative`/`integral`/`clip`) is likewise
Rust-only. These are the one intentional, tracked asymmetry left by this
feature (not a parity-test regression — `ANALYSES` covers *analyses*, not
every `Waveform` method); a native Python binding for these methods is a
follow-up task, not part of HOST-27/28's scope. `pip.extract` (HOST-25)
works over the native methods that already exist (`.max`/`.min`/`.cross`)
today.

## 10. Errors — `SimulationError` hierarchy

```python
class SimulationError(Exception): ...          # base — catch-all
class ElaborationError(SimulationError, ValueError): ...   # parse/elab, load()/load_str()
class UnknownModule(SimulationError, ValueError): ...       # Design.module()/__getitem__
class UnknownNet(SimulationError, KeyError): ...             # unaddressable net/opvar/param
class ConvergenceError(SimulationError, RuntimeError):       # .node, .iteration, .analysis
    ...

try:
    sim.tran(pip.TranConfig(stop=1e-3))
except pip.ConvergenceError as e:
    print(e.analysis, e.node, e.iteration)
```

Every subclass **also** inherits the matching builtin exception type
(`ValueError`/`KeyError`/`RuntimeError`) it previously surfaced as, so any
existing `except KeyError`/`except ValueError` call site keeps working
unchanged — purely additive. A raw native failure that doesn't fit a more
specific subclass propagates as its original builtin type unchanged (never
silently swallowed). Every `Module`/`Session` analysis and `set` method is
wrapped with a decorator that reclassifies by message content ("Failed to
converge" → `ConvergenceError`; "is not addressable"/"is not a solved
analog net" → `UnknownNet`) and otherwise re-raises unchanged.

The Rust `Error` enum (`piperine_api::error::Error`) mirrors the same
taxonomy as typed variants: `Elaboration`, `Lowering`, `Codegen`, `Solver`,
`Measurement(String)` (unaddressable nets/opvars/params, convergence
failures — `piperine-solver`'s typed `SolverDomain` carries the structured
detail), `Plugin(String)`.

## 11. Naming, discoverability, `NetRef` ergonomics (HOST-23/24/26)

- **Rust**: `v`/`i` on `OpResult`/`Trace<Waveform>`/`Trace<ComplexWaveform>`
  take `impl NetSelector` — `op.v("out")`, `op.v(("out", "in"))`, or a bare
  `NetRef` all work; `NetRef` implements `From<&str>`/`From<String>`/
  `From<&String>`/`From<&NetRef>`, so no bare `NetRef { name: ... }`
  construction is needed at any call site. `CrossDirection`
  (`Rising`/`Falling`/`Either`) replaces `Waveform::cross`'s `dir: &str`
  (still `impl From<&str>` for legacy strings); `Scale` (`Lin`/`Dec`/`Oct`)
  is the sweep-geometry enum, `impl From<Scale> for bool` for
  `logarithmic`-typed slots.
- **Python**: `CrossDirection`/`Scale`/`Direction` are proper `Enum`
  classes (`Direction` wraps `Port`/`Terminal.direction`'s plain `str`
  reflection field for symbolic comparison — the field itself stays `str`,
  a native `#[pyclass]` field with no Python-level wrapper to intercept).
- `design[name]` (`__getitem__`), `design.top`/`amp.ports`/`amp.nets`/
  `amp.instances`/`amp.params`/`amp.behaviors` as **properties** (data,
  not action — HOST-24 consistency rule), `pip.load_str(src)`,
  `design.const(name)` (not `const_`), `len(wf)` (`__len__`).
- `pip.extract(source, {name: fn})` (HOST-25) applies every named
  measurement function to `source`, collecting a `{name: value}` dict —
  the removed PHDL bench's measurement shape, host-side.
- Complete `.pyi` stubs (HOST-26): `python/piperine/_piperine.pyi` types
  the native `_piperine` extension (no compiled type info of its own); the
  pure-Python facade (`__init__.py`) carries inline type hints + docstrings
  for every locally-defined class/function. `py.typed` (PEP 561) marks the
  package. A dedicated test (`pyi_stub.rs`) cross-checks every declared
  stub member against the real runtime module via `hasattr`.

## 12. The CLI as host

| Command | Behavior |
|---------|----------|
| `piperine check [file]` | parse + elaborate |
| `piperine build [file]` | elaborate + JIT-compile |
| `piperine run script.py` | run a Python script with `import piperine` available (embedded CPython — no pip install) |
| `piperine run -i [design.phdl]` | interactive REPL; with a file, pre-loads it as `design` |
| `piperine test [file]` | discover and run `**/*_tb.py` under the project root (skipping `.venv`/`target`); per-file PASS/FAIL with tracebacks, per-file timeout (default 300 s, `PIPERINE_TEST_TIMEOUT_SECS`), exit 1 on any failure, exit 0 with a notice when none exist |

A testbench is plain Python with asserts:

```python
# divider_tb.py
import piperine

m = piperine.load("src/main.phdl").module("DividerBoard")
r = m.op()
assert abs(r.v("mid") - 2.0) < 1e-6, "divider ratio is R2/(R1+R2)"
```

## 13. The Rust host — `piperine::prelude`

```rust
use piperine::prelude::*;

let design = parse_and_elaborate(&src, &SourceMap::dummy())?;
let mut session = Session::compile(&design, "Divider")?;
let op = session.op(&SolverConfig::default(), None)?;
assert!((op.v("mid")? - 2.0).abs() < 1e-9);
```

`piperine::prelude` (== `piperine_api::prelude`) re-exports: `Error`;
`FourierComponent`/`FourierResult`; `SimHooks`; the result types
(`DistoResult`, `NetRef`, `NetSelector`, `OpResult`, `PssResult`,
`PzResult`, `SParamResult`, `SensResult`, `TfResult`); the session types
(`Grid`, `Nested`, `Scale`, `Session`, `SessionBuilder`, `SolverConfig`,
`Sweep`, `SweepPoint`); the unit newtypes (`Freq`, `Time`); the waveform
types (`AcTrace`, `ComplexWaveform`, `CrossDirection`, `NoiseTrace`,
`Trace`, `Waveform`); plus `piperine-codegen`'s `CircuitBuildInfo`/
`CircuitCompiler`/`DeviceProvider`, `piperine-lang`'s `Design`/`SourceMap`/
`parse_and_elaborate[_seeded]`, and `piperine-solver::prelude::*` in full
(introspection types: `Bounds`, `Invalidation`, `ModelDescriptor`,
`ObservableDescriptor`, `ObservableKind`, `ParamDescriptor`, `ParamScope`,
`TerminalDescriptor`, `TerminalKind`, `NoiseContribution`).
