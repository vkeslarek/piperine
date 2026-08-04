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
- **Design note** (both hosts document this differently — see T3's
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

**Design decision** (ideal.md §6.5 vs. delivered, HOST-12): the ideal access
path is module-level (`amp.param("m1.w")`); the delivered path is
instance-scoped (`op.instance("m1").param("w")` /
`op["m1"].param("w")`) — `ParamDescriptor` is per-device and only available
after compilation, matching the shipped `Introspect::list_params` ABI and
the same instance-scoped pattern `opvar` already uses.

## 7. Return types — the taxonomy

*Separate a type when it changes the available operations; unify when only
the data values differ.* Eight rows below, eleven named types (the fourth-from-last
row groups the four structured analysis-specific results, which share one rationale).
The set is identical on both hosts — no host has a type the other lacks:

| Type | Stands alone because |
|------|-----------------------|
| `Waveform` | real-signal ops: time-weighted `rms`/`mean`, `slew_rate`, `overshoot`, `cross` |
| `ComplexWaveform` | complex-only ops: `mag`/`phase`/`db`, `bandwidth_3db`, margins |
| `Trace<T>` | **one generic container** for every swept signal set — `tran`/`dc` → `Trace<Waveform>`, `ac` → `Trace<ComplexWaveform>`, `noise` → `Trace` + noise methods. Same `.v()`/`.i()`/`.axis()`/`.stats`/`.four()`; the sample type is the only difference. On the **Rust** side `AcTrace`/`NoiseTrace` are now genuine **type aliases** (`Trace<ComplexWaveform>`, `Trace<NoiseSample>` — `NoiseSample` a zero-sized discriminator, since noise has no per-net `v`/`i`, only `psd`/`total`/`by_source`/`contributions`), not separate types — `HOST-13` folded them in. **Implementation note** (Python, T7): `_AcTrace`/`_NoiseTrace` stay distinct native `#[pyclass]` types in `piperine-python` (PyO3 pyclasses cannot be generic over a Rust type parameter the way `Trace<T>` is), each wrapping the corresponding Rust `Trace<ComplexWaveform>`/`Trace<NoiseSample>` internally and exposing the same method surface a caller would see through a generic `Trace` — the *shape* is uniform even though the Python class identity is not literally one generic class. |
| `OpResult` | point values + `InstanceView` indexing + opvars |
| `TfResult` | `gain`/`z_in`/`z_out` scalars |
| `SensResult` | `(output, "label.param") → value` map |
| `PssResult`/`PoleZeroResult`(`PzResult`)/`DistoResult`/`SpResult`(`SParamResult`) | structured, analysis-specific fields — kept distinct rather than merged into a bag of `Optional` fields |
| `MarginsResult` | the margins channel (D3, dv-core DVC-18) — pointwise and reduced margins live here, never as waveform rows. Separate because it carries an argmin-or-not shape a waveform cannot model without inventing a `t = 0` for the reduced kind |

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

# Every analysis result that carries margins exposes the same channel
# (dv-core DVC-15/18, design §C3): OpResult, Trace (tran/dc sweep), AcTrace.
r.margins                  -> MarginsResult
r.margins["head"]          -> MarginEntry  (bare name if unique, else "inst.path.label")
r.margins["x1.head"]       -> MarginEntry  (qualified form disambiguates)
r.margins.get(name)        -> MarginEntry  # loud: unknown/ambiguous/not-defined/digital
r.margins.requires_ok      -> bool         # every require satisfied AND exercised
r.margins.first_violation  -> MarginEntry | None
r.margins.first_not_exercised -> MarginEntry | None   # scoped require whose analysis never ran
r.violations               -> list[ValidationFinding]  # severity, label, message, value, time, instance_path
r.first_violation          -> MarginEntry | None       # the common "show me what failed" read

# MarginEntry shapes (D3): pointwise carries worst + argmin; reduced carries
# one value and NO argmin — the channel states which kind it is.
entry.shape                -> MarginShape  # Pointwise | Reduced
entry.scalar()             -> float        # worst (pointwise) | value (reduced)
entry.argmin()             -> float | None # time/freq/swept coord (pointwise) | None (reduced)
entry.is_satisfied()       -> bool         # m >= 0  ⟺  satisfied
entry.is_not_exercised()   -> bool         # scoped require whose analysis never ran
```

Unknown nets/opvars/params raise `UnknownNet`/`Error::Measurement` —
measurement failures are loud, never a silent `0.0` or NaN. Digital nets
read their logic value (0/1, X/Z as NaN) directly from `OpResult.v`/
`Trace.v`.

The margins channel is identical on both hosts (`r.margins`, `r.violations`,
`r.requires_ok`, `r.first_violation`) — see Appendix C for the full surface
and `tests/host_parity.rs` for the executable proof. Reduced margins
(`ac_unity_gain_freq`, `ac_phase_margin`, …) are computed host-side as
reductions over the points the solver returns (D7) — the solver stays
pointwise-pure, which is the reason it can.

## 8. Configs, units, `Solver`

Config bundles (`OpConfig`/`TranConfig`/`AcConfig`/`NoiseConfig`) are typed
Python dataclasses with `.with_(**overrides)` (immutable copy,
`dataclasses.replace`); every field is autocomplete-visible
(`inspect.signature(TranConfig)`). `Solver` (Python dataclass) and
`SolverConfig` (Rust struct) carry the identical canonical knob set
(HOST-20): `temperature`, `reltol`, `abstol`, `gmin`, `max_iter`,
`dc_damp_tolerance`. Two posture knobs (dv-core DVC-12) ride on the same
config and thread into the solver's `Context`:

- **`checks: CheckPosture`** (`Strict | Collect | Off`) — how an analysis reacts when a
  validation finding fires or a `require` margin crosses zero. Default
  `Strict` (a silent pass is never the default posture). `Collect` records
  the worst margin + argmin and always completes — the optimizer's
  inner-loop posture, where an infeasible iterate is a margin to climb out
  of rather than an exception. `Off` is zero-cost: the constraint kernels
  are not called and the findings hook is not polled.
- **`require_scope: RequireScope`** — which analyses an unscoped `require` holds in. The
  default is `EveryAnalysis` (D2); a `require` inside `@ dc { … }` is
  scoped by the block, not by this knob (it is "DC only" regardless).

Both spell identically on Python `Solver` (as the strings `"strict"`/`"collect"`/
`"off"` and `"every_analysis"`) and on Rust `SolverConfig` (as the `CheckPosture`
and `RequireScope` enums); Part VII §3.3 receives them on `Context`.

Changing the posture between analyses in one session triggers no
recompilation (MD-18) — the posture fields are pure run data on `Context`.
The same two fields exist with identical spelling on Python `Solver` and
Rust `SolverConfig`, and `host_parity.rs` covers them.

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
malformed SI string panics (`From` is infallible); `Freq::parse` /
`Time::parse` are the fallible alternatives (`Result<Freq, String>`).
**Implementation note**: the
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

The table covers the commands that *are* a host surface. The CLI's remaining
subcommands — `fmt`, `new`, `clean`, `add`, `remove`, `tree`, `plugin` — are
project/tooling concerns, not simulation hosting: `add`/`remove`/`plugin` are
specified in Part VI §5, the rest are outside this specification.

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

## 14. Sensitivity and gradients — the adjoint surface (dv-gradients)

`sensitivity(metric, param)` (one parameter) and `gradient(metric, params)`
(many parameters, one adjoint solve) sit on `Session` (Rust) and `Module`/
`Session` (Python) — identical names, shapes, and values on both hosts
(`tests/host_parity.rs`), per MD-22. `metric` resolves to either a declared
`dv-core` margin name or a solved analog net name, the same resolution order
`Session::gradient` uses everywhere; an unknown name fails loud listing the
candidates it tried against.

```python
s = sim.sensitivity("idd", ("r1", "r"))       # -> value, method, fallback_users
g = sim.gradient("idd", [("r1", "r"), ("r2", "r")])   # one adjoint solve, both params
print(g.method, g.d)                          # "adjoint", {"r1.r": ..., "r2.r": ...}
```

```rust
let s: Sensitivity = session.sensitivity("idd", &("r1".into(), "r".into()), &config)?;
let g: Gradient = session.gradient("idd", &[("r1".into(), "r".into()), ("r2".into(), "r".into())], &config)?;
assert_eq!(g.method, SensitivityMethod::Adjoint);
let d_r1 = g.get("r1", "r").unwrap();
```

Every result names the **engine** that produced it — `SensitivityMethod::Adjoint`
("adjoint": one transpose solve, Part VII §17.2) or
`SensitivityMethod::FiniteDifference` ("finite-difference": the `.sens`
baseline, Part VII §17.1) — and carries `fallback_users`, the set of element
labels that used stamp-perturbation instead of an analytic `∂F/∂p` kernel
(Part VII §3.2's `HAS_SENSITIVITY` note); a fully analytic circuit reports an
empty set. A gradient the solver genuinely cannot compute — an AC-defined
metric (Part VII §17.2's stated D6 gap), an event-detected metric, or a
non-differentiable reduction — fails loud naming the reason, never a
plausible wrong number.

## 15. The policy driver — optimize / center / high-sigma (dv-gradients)

`optimize`, `center`, and high-sigma yield estimation are **three policies of
one driver**, not three separate features (design §C4) — they share the same
restamp loop, the same `dv-core` margin channel, and the same gradient path
where one exists. Concretely, on the Rust side this is one type,
`piperine_api::driver::Driver`, built from one enum, `Policy` (unrelated to Part VII
§3.3's solver-side `Policy`, which names a driver's convergence tunables — two
different crates, two different concepts, one unfortunate spelling):

```rust
use piperine_api::{Driver, Policy, OptimizeSpec, CenterSpec, HighSigmaSpec, DriverOutcome};

let driver = Driver::new(Policy::Optimize(OptimizeSpec {
    objective: "idd".into(),
    minimize: true,
    over: vec![("m1".into(), "w".into())],
    ..Default::default()
}));
let outcome = driver.run(&mut session, &config)?;
let DriverOutcome::Optimize(result) = outcome else { unreachable!() };
println!("{:?} engines={:?}", result.params, result.engines);
```

`Driver::run` **always** solves in `CheckPosture::Collect`
(`Driver::posture()` reports it unconditionally), regardless of the
`checks` field on the `SolverConfig` passed in — an optimizer that aborted
on its own infeasible iterates could not search. `Driver::run` never
recompiles across the whole run (MD-18) — every step restamps the one
compiled circuit.

- **`Policy::Optimize(OptimizeSpec)`** — a feasibility phase (climb every
  `require` margin to ≥ 0) followed by an objective phase (descend/ascend
  `objective` while projecting back onto the feasible set on a rejected
  step). `OptimizeResult.engines` is a set of `Engine::Gradient`/
  `Engine::BlackBox` — a run that composed gradient feasibility with a
  black-box (Nelder–Mead) objective phase, because the objective was not
  analytically differentiable, names **both**; a purely gradient-driven run
  never spuriously reports `BlackBox`. `Engine::as_str()` gives the
  host-visible names `"gradient"`/`"black-box"`.
- **`Policy::Center(CenterSpec)`** — worst-case-distance centering: maximize
  `min_i (m_i / σ_i)` over `over`'s design parameters, where `σ_i` comes
  from an inner Monte Carlo run (§16) around the current search point.
  `CenterResult.sigma_scheme` (`SigmaScheme::FixedWithinOuterIteration`,
  the only scheme this delivery implements) names how `σ_i` was estimated
  — held fixed within one outer iteration, re-sampled between outer
  iterations — because the number means something different under another
  scheme. `CenterResult.worst_normalized_margin` is the centered point's
  worst-case distance, in sigmas.
- **`Policy::HighSigma(HighSigmaSpec)`** — mean-shift importance-sampling
  tail estimation (Part VII's noise-adjoint analogy does not apply here;
  this is a host-side sampling technique, not a solver analysis). Every
  `HighSigmaResult` carries `estimator`
  (`HighSigmaEstimator::MeanShiftImportanceSampling` — a yield without its
  estimator is not a yield), `ci: (f64, f64)` (the two-sided confidence
  interval at `confidence`), and `failing_indices` — the drawn samples that
  were a failure under the shifted distribution, each replayable (§16).

**Coverage note:** the driver (`Driver`/`Policy`/all three specs/results) is
Rust-only (`piperine-api`) as of this delivery — it has not yet been bound
into `piperine-python`, unlike `sensitivity`/`gradient` (§14), which are on
both hosts today. This mirrors §9's existing tracked-asymmetry pattern: a
Python binding is a follow-up task, not part of this feature's scope.

## 16. Monte Carlo sampling and aging drift (dv-gradients)

`piperine_api::SamplePlan` (Rust-only, same coverage note as §15) walks the
**authored** instance tree (never the flattened form — MD-25) to find every
`tol`-carrying parameter, and `SamplePlan::monte_carlo` restamps `n` draws on
the one compiled session (MD-18 — a 1000-sample run costs zero additional
compiles):

```rust
let plan = SamplePlan::collect(&design, "Top")?;
let mc: MonteCarloResult = plan.monte_carlo(&mut session, 1000, /* seed */ 7, &config)?;
println!("yield={} sigma={:?}", mc.yield_fraction(), mc.sigma("dut1.head"));
let (worst_index, worst_value) = mc.worst("dut1.head").unwrap();
```

- **Reproducibility contract.** A sample is fully determined by `(seed,
  index)`: the same pair produces byte-identical draws across runs and
  across processes, and a named failing sample (`mc.worst(name)`'s index, or
  a `HighSigmaResult::failing_indices` entry, §15) replays as an ordinary
  session — `plan.apply(&mut session, seed, index)` on a freshly compiled
  `Session` reproduces the exact original failure. **A sample index is NOT a
  stable name across design edits**: `SamplePlan::collect` re-derives the
  entry list from the design's current authored hierarchy every time it
  runs, so an edit that adds, removes, renames, or re-hierarchizes a
  `tol`-carrying instance changes the plan's entry set — the same `(seed,
  index)` against a plan collected from an edited design is not guaranteed
  to reproduce the same physical sample. Treat `(seed, index)` as valid only
  against the specific `SamplePlan` (and hence the specific design) it was
  drawn from.
- **Process vs. mismatch draws.** A `tol … global` declaration is process
  variation — one draw shared by every instance of the same
  `(module, param)` group; a plain `tol` declaration is mismatch
  variation — one independent draw per instance (`SampleScope::Global` /
  `SampleScope::Mismatch`).
- A sample that fails to converge is a reported `SampleOutcome::Failed`
  outcome, never a crash that aborts the run; `mc.failed_indices()` names
  them so a caller does not trust `yield_fraction`/`sigma`/`worst` blindly
  on a run with non-convergent samples. `SamplePlan::reset` (called
  automatically at the end of `monte_carlo`) returns the session to
  declared nominal values — sampling never leaves residue for a later
  nominal analysis.

**Aging as parameter drift.** `piperine_api::age(session, &AgingSpec,
&config)` (Rust-only, same coverage note) computes a scalar stress proxy from an
OP or transient run
(`StressSource::Op`/`StressSource::TranPeak`), drifts declared parameters
linearly with `stress · time` through the same restamp path
(`Session::set`), re-verifies at each scanned point, and returns an
`AgingResult` — no new result type: `AgingResult::delta(name)` reads the
existing margins channel (`baseline`/`aged`, both `MarginsResult`) rather
than carrying a dedicated delta type. `AgingResult.first_violation` names
which declared constraint went negative first and after how long, or `None`
if the scanned `lifetime` never crosses it. A drift that would carry a
parameter across a rebuild-class boundary fails loud (propagated straight
from `Session::set`'s existing `Invalidation::Rebuild` refusal — Part VII
§3.4), rather than silently re-elaborating. This is a placeholder linear
drift model, not a calibrated aging tool (MOSRA/RelXpert): the gap is the
per-device stress-equation *data*, which the shipped ngspice-faithful
models do not carry — the same documented gap `dv-core` states for SOA
limits.

## 17. The host device library (dv-tester)

A fifth way to get a device into a circuit, alongside PHDL, a compiled
plugin, an OSDI model, and (via §14–16) the driver/sampling surface: write
the device in the host language itself — Rust or Python, in-process, no
`dlopen`, no matched toolchain, no Verilog-A compiler. `HostElement` +
`HostAdapter<T>` (Part VII §20.1) is what makes the solver unable to tell a
host device from any other; this section is the library built on top of
it.

### 17.1 Three levels, one cost table

Where the code runs, not how it looks, is the axis. The three compose in
one device — a class may stamp directly *and* install a `Wave`/`Behavior`
*and* sequence time, all at once:

| Level | What it is | Home | Host calls |
|---|---|---|---|
| **0** | the raw solver ABI — stamps, unknowns, `Element` itself | `piperine-solver` (Rust) / `pip.Device` (Python) | one per **Newton iteration** in Python; native in Rust |
| **1** | native signal mechanism — `Wave`, `Behavior`, events, `Measure` | `piperine-signal` (Part IX) | **zero** — closed-form value + derivative, evaluated every Newton iteration for free |
| **2** | the sequencer — declares level 1, sequences time, checks | `piperine-api::sequencer` / `pip.Sequencer` | one per **declared wake** (`t.advance`/`t.wait`), never per Newton iteration |

The delivered cost model, concretely:

| What you write | Level | Host calls | Ceiling |
|---|---|---|---|
| `Wave.*` (any kind but `from_fn`) | 1 | **0** | unlimited |
| `Wave.sampled(...)` (precomputed, e.g. from numpy) | 1 | **0** | unlimited |
| `HostBehavior.*` (catalog or Rust-authored `BehaviorKind`) | 1 | **0** | unlimited |
| `Measure.*` accumulation | 1 | **0** (fed natively on accepted points) | unlimited |
| `Sequencer` wake (`t.advance`/`t.wait`) | 2 | 1 | ~10⁵ wakes per run is cheap |
| `Wave.from_fn` | 1→host | 1 per evaluation | large but not free |
| `load_dc`/`load_transient`/`update` in Python | 0 | 1 per Newton iteration / accepted step | expensive at scale |

A boundary device (a tester, a stimulus source) is a handful of elements —
cheap regardless of level. A bulk device (thousands of instances) written
at level 0 in Python is a bad idea the docs say so about, never silently
prevented; the same device in Rust, or reached for at level 1, has no
per-call cost at all.

### 17.2 `Wave` — value from time

`piperine_signal::wave::Wave` (Rust) / `pip.Wave` (Python, identical
spelling except `Wave.const` vs. `Wave.constant` in Rust — `const` is a
Rust keyword): `const`/`ramp`/`pulse`/`sin`/`exp`/`sigmoid`/`pwl`/`sampled`/
`from_fn`, plus `sum`/`scaled`/`shifted` for composition. Every kind but
`from_fn` evaluates natively with **zero** host calls; `sampled(t, v)`
accepts an arbitrary precomputed array (e.g. built with numpy) at the same
zero cost. A `Wave` with known discontinuities (a `pulse`'s edges, a
`pwl`'s breakpoints) publishes them through `pending_edges(from, end)` — the same
declared-breakpoint contract the runtime operators use (Part IX §2.3, §8.4), which
the device layer forwards to the solver as `Element::next_breakpoints` (Part VII
§15.9) — so the integrator lands on each edge instead of interpolating across it.

**Every `Wave` kind declares its own DC value** (SPICE convention, so a
reader arriving from that background is not surprised):

| Kind | DC value |
|---|---|
| `const(v)` | `v` |
| `ramp(v0, v1, dur)` | `v0` (the start) |
| `pulse(lo, hi, ...)` | `lo` (the initial level) |
| `sin(offset, ...)` | `offset` — **not** `value(0.0)` |
| `exp(v0, v1, ...)` | `v0` (the initial level) |
| `sigmoid(lo, hi, ...)` | `lo` |
| `pwl(points)` / `sampled(t, v)` | the first point |
| `from_fn(f)` | `f(0.0)` — this is the one kind whose DC value pays its own host call |

An installed `Wave` contributes exactly this value at the operating point;
a sequencer-bearing session's coroutine does not run at DC (`run()` begins
with the transient — Part VII's D13 companion rule), so DC is defined by
this table rather than by whatever the coroutine happened to install first.

### 17.3 `Behavior` — value from circuit state

`piperine_signal::behavior::kinds` (Rust) / `pip.HostBehavior` (Python —
**not** `pip.Behavior`; that name already names the pre-existing POM
`analog`/`digital`-block reflection type returned by `Module.behaviors()`,
so the host-device catalog is exposed under a different name to avoid
silently shadowing it, the same precedent `Wave.const`/`Design.const`
already set): `comparator`/`hysteresis`/`switch`/`limiter`/`vcvs`/`vccs`/
`vco`/`table`, plus `sum`/`scaled`/`composed`. Unlike `Wave`, a `Behavior`
is `f(v₁ … vₙ)` — it depends on the solution, so it contributes **cross
Jacobian entries** (`∂f/∂vᵢ`) from a closed form, reacting at every Newton
iteration for **zero** host calls. `comparator` is `tanh`-shaped and smooth
by default (differentiable, convergence-friendly); a genuinely
discontinuous kind (`switch`, `hysteresis`) registers a crossing instead of
faking a derivative across its own jump, so the integrator lands on the
edge (Part VII §20.5's tier-2 machinery).

`BehaviorKind` (Rust trait: `value`, `jacobian`, `crossings`) is the
extension point — closed in Python (`pip.HostBehavior.register_kind(...)`
fails loud, naming level 0 and the Rust route), open in Rust: implementing
the trait outside `piperine-signal` composes exactly like a shipped kind.
`piperine_signal::verify_derivatives` is the guard every hand-written
`jacobian` is checked against — a central-finite-difference comparison,
the same oracle pattern `dv-gradients` used for `.sens` — and a
deliberately wrong Jacobian is what proves the guard can fail.

### 17.4 `Measure` — reductions at the solver's rate, with a published bound

`piperine_signal::measure::kinds` / `pip.Measure` (+ `pip.DelayMeasure`,
the two-channel `delay(a, b, level)` kind, which ships its own inherent
`feed(t, a, b)` rather than conforming to the single-`(t, v)`
`MeasureKind` trait every other kind implements — propagation delay is
inherently a two-signal reduction): `peak`/`min`/`average`/`rms`/`ripple`/
`integral`/`cross`/`settle`/`count`/`delay`. Each is an **O(1) streaming
accumulator**, fed one `(t, v)` per **accepted** point, never storing a
waveform — the same accumulator reduces a stored `Waveform` post-run and a
live transient online, and both agree (the crate's own regression test).

**Every value travels with an error bound.** `Measure.peak` over accepted
points can miss the true peak when it falls between two of them — for
overshoot, the archetypal analog check, that would be a plausible wrong
number, exactly what this project's fail-loud rule exists to prevent. The
default mode reduces over accepted points and reports its own bound
(`peak ∈ [v, v + ε]`, `ε` from the local LTE); `refine=True` drives step
refinement near the extremum until `ε` is under a requested tolerance,
terminating **at the step floor with a report** rather than stalling
forever. `t.expect(...)` against a measure compares **against the bound**
(`value - error_bound` / `value + error_bound`, `SequencerControl::
expect_measure_at_least`/`expect_measure_at_most`), never the point
estimate alone — a borderline pass whose bound-adjusted worst case would
fail the threshold is still a failure.

Gating: `window=(t0, t1)` by time; an analog gate opens/closes on a
crossing predicate (the same tier-2 machinery, Part VII §20.5); a digital
gate opens/closes on the digital scheduler's **settled** events only,
never mid-delta-cycle — a mid-cycle value can be an artifact of DAG
evaluation order rather than a signal the circuit ever had (the identical
rule Part VII §20.2 states for the subscriber). A measure whose window
opens more than once accumulates across every open interval, not just the
first.

**Lifecycle.** `arm(kind)` creates a fresh accumulator starting at the arm
instant — arming the same measure twice yields two independent handles, a
measure is a handle, not a name in a global table. `.value`/
`.error_bound` before any accepted point has fed the accumulator — or when
a gated measure's window never opened — fails **loud** with one named
reason (`MeasureError::NeverSampled`, `"never sampled: no accepted point
has fed this measure yet"`), never `NaN`, zero, or a nominal. `disarm()`
freezes the accumulator: `.value` stays readable and stops moving.

> **SPEC_DEVIATION**: `ideal.md` §4.3 describes the "gate never opened"
> case as a distinct message from "never fed at all". The delivered
> `MeasureError` has exactly **one** variant, `NeverSampled`, covering
> both — per `ideal.md`'s own literal wording ("names both causes
> identically"), the two are the same fact from the accumulator's point of
> view: zero accepted points reached it, for whatever reason.

### 17.5 The `Sequencer`, one `wait`, three wake sources

A `Sequencer` (`piperine_api::sequencer::Sequencer<P>` / `pip.Sequencer`,
a `pip.Device` subclass) is a coroutine over simulated time: `yield
t.advance(dt)` reads as a straight-line program; `yield t.wait(...)`
expresses event-driven waiting without leaving that line. Rust has no
stable generators, so a Rust `Sequencer`'s program (`SequencerProgram`) is
a hand-written `resume` state machine; Python uses a real generator
(`def run(self, t): yield ...`). The **contract** — same wake sources,
same findings, same resume ordering — is identical; only the authoring
mechanism differs (Part VII §20.3 states why the resume site is safe).

`wait` has exactly one blocking verb and three forms. Two of the three ride
the solver's existing event kinds; the third does not — it is resumed by
the sequencer's own polling, not by an `EventKind`:

| The test writes | Wakes on | `EventKind` | Fidelity |
|---|---|---|---|
| `t.advance(dt)` | a time the test named | `Breakpoint` | `Exact` |
| `t.wait(edge(net, direction))` | a digital net settling to a matching transition | `Digital` | `Exact`, settled-only (Part VII §20.2) |
| `t.wait(above(net, level, direction))` | an analog crossing | none — resumed via tier-2 prediction, not `EventKind::Crossing` (see note) | tier 2, error reported (Part VII §20.5) |

> **Note — `WaitCrossing` does not use `EventKind::Crossing`.** The
> `EventQueue`/`EventKind` machinery exists for events the *solver*
> schedules and lands on (breakpoints, digital settling). A crossing wait
> is different: the sequencer itself predicts the crossing instant from
> the bracketing accepted pair (`piperine_signal::events::predict_crossing`,
> Part VII §20.5's tier 2) and polls for it every accepted step through
> `HostElement::accept_timestep`'s `StateView` (`Sequencer::accept_timestep`,
> `crates/piperine-api/src/sequencer/driver.rs`, `crossing.rs`) — it never
> reads or produces an `EventKind::Crossing` entry. `EventKind::Crossing`
> is what PHDL's `@cross` reads (Part VII §20.5's tier 1); the two crossing
> paths are independent, and only `@cross` touches the event queue.

`t.expect(cond, msg)` records an `Error` finding with time and instance
provenance through `dv-core`'s channel when `cond` is false — never a
print; `t.warn(msg)` always records a `Warning`, never aborts. Posture
(`strict`/`collect`, §8) governs whether a recorded `Error` aborts the run
the same way it governs any other finding.

**`install` semantics (D — `ideal.md` §4.2).** A `Wave` installed mid-run
has its time origin at the **install instant**, not absolute zero — a ramp
installed at `t = 5 µs` ramps over `[5 µs, 5 µs + dur]`. Replacing an
active wave takes effect **immediately** and its discontinuity is
registered as an `Exact` breakpoint — never smoothed. Continuity is
available, but explicit: the caller reads the current value and bakes it
into the replacement wave's own parameters.

**Drive** goes through the same declared-impedance path Part VII §20.4
describes; a digital drive is queued (`SequencerControl::drive_digital`)
and forwarded into the real `EventSink`, never touching the MNA.

**Delivered Rust/Python asymmetry beyond the closed/open catalog.** In
Rust, `t` (`SequencerControl`) itself carries only `drive_digital`/
`findings`/`expect`/`warn`/`expect_measure_at_least`/
`expect_measure_at_most`/`last_crossing` — `install`/`drive_voltage`/
`drive_current`/`arm` are **free functions** (`sequencer::install::
InstallSite`, `sequencer::drive::*`) that take `&mut SequencerControl`
plus the device's own struct fields (its install sites, its `Measure`
handles) explicitly, because those sites belong to the device, not to the
sequencer. **In Python, `t` (`SeqCtl`) exposes only `advance`/`wait`/
`expect`/`warn`/`.t`** as of this delivery — `install`/`drive_*`/`arm`/
reading a solved net back through `t` are not yet bridged into the Python
`SeqCtl` surface. A Python tester today installs level-1 descriptors once,
at construction, through `build(self, b)` (`b.wave(p, n, wave, rout)` /
`b.behavior(force_p, force_n, controls, behavior)`, §17.3's zero-host-call
path); reading circuit state back into the running coroutine goes through
the device's own instance attributes, set from `load_dc`/`accept_timestep`
— exactly the pattern `ideal.md` example G composes, and the honest
description of what a Python tester can do today rather than the aspirational
`t.arm`/`t.install`/`t.read_v` sketch in `ideal.md` §4/§9's examples E and
F. Extending the Python `SeqCtl` bridge to cover `arm`/`install`/`drive`/
read-back is a natural, named follow-up, not part of this delivery.

### 17.6 Closed in Python, open in Rust (§3.7's asymmetry, stated as a decision)

`BehaviorKind`/`MeasureKind` are Rust traits; the Python catalogs
(`pip.HostBehavior.*`/`pip.Measure.*`) are exactly what ships, and
attempting to register a new kind from Python fails loud, naming level 0
(raw stamps, priced per Newton iteration) and the Rust trait as the
escape hatch. This is deliberate, not an oversight: a hand-written
Jacobian or accumulator wants `verify_derivatives` and native speed, both
of which mean Rust. The **surface** is identical either way — same
spelling, same argument order, same semantics — the asymmetry is only in
who may add a new kind, and it is the one asymmetry `tests/host_parity.rs`
(§17.8) declares rather than silently tolerates.

### 17.7 Reach rules and posture interaction (D14)

Reading any net — internal or external — from a host device or sequencer
is free. **Driving an internal net is allowed, but always reported**
(Part VII §20.4): the sequencer's findings buffer records every
internal-net drive unconditionally, so a passing test that forces a node
is visibly doing so in the run's findings rather than passing silently.
Two devices driving the **same** net — regardless of which one is a
sequencer — is a loud conflict, never last-writer-wins, checked
host-side before the solver ever sees the stamps.

`checks=off` does not exempt a `Sequencer`'s own `expect`/`warn` findings
— those are unconditional (never gated by the `checks` posture, which
instead governs `require` margins, §8). A host device present with
`checks=off` still has its `expect` findings recorded and still aborts on
posture `strict`; the interaction is stated here explicitly rather than
left to be inferred from `dv-core`'s general posture rules.

### 17.8 Parity, and the guard that proves it

`tests/host_parity.rs` and `tests/host_capability_parity.rs` extend the
existing parity oracle (§7) to the device library: every name this feature
added exists on both hosts with identical spelling and shape, one fixture
runs through both hosts asserting identical findings **and** identical
measure values with identical bounds, and the closed-Python/open-Rust
catalog asymmetry (§17.6) is the **only** declared divergence — the guard
is proven able to fail (add a Rust-only name, watch it fail, revert) the
same way every other mechanical guard in this project is.
`tests/host_capability_parity.rs` additionally proves the harder claim:
four devices — a `transition`-driven source, a pnjlim junction, a `table`
nonlinearity, and a `cross`-triggered element — each written once in PHDL
and once as a host device, produce **identical** results on the same
fixture. Any PHDL capability not reachable from the host device library
is named there rather than silently missing — a scoped promise, not an
eroding one.

### 17.9 `CoverageDatabase` — merge and closure (dv-tester DVT-43)

A `cover`/`cover cross` point (Part I §10.6) produces per-bin hit counters, not
a margin. The solver returns one run's raw counters as a `CoverageReport`
(Part VII §3.5's `coverage_report()`); `piperine_api::CoverageDatabase` is the
host-side structure they merge into.

**Coverage persists across runs and seeds — margins do not.** A margin belongs
to one analysis and is discarded at the next (`reset_margin_tracking`, DVC-18);
a coverage database is state a caller deliberately keeps for an entire
regression, calling `merge_run` once per run:

```rust
let mut cov = CoverageDatabase::new();
for seed in 0..100 {
    let r = session.tran(...)?;
    cov.merge_run(r.cover_points(), r.cover_crosses());
}
assert!(cov.empty_bins().is_empty(), "unexercised bins: {:?}", cov.empty_bins());
```

| Member | Contract |
|---|---|
| `merge_run(points, crosses)` | Counters **sum**, never replace. A point not seen before starts from an all-zero vector sized to this run's report, so a run with zero accepted points still *creates* the entry — its bins are then reported as not-exercised rather than silently omitted. |
| `points()` / `crosses()` | The accumulated entries, keyed `"<instance_path>.<label>"` (bare `<label>` when the instance path is empty, matching `ValidationFinding`'s convention), so two instances of the same monitor module accumulate independently. |
| `empty_bins()` | Every never-hit bin across every declared `cover` point, named `"<key>[<bin index>]"`, sorted for determinism. |
| `CoverageEntry::fully_covered()` / `::empty_bin_indices()` | Per-entry closure, same rule. |

**An empty database is not "100% covered".** `empty_bins()` on a database with
no runs merged names nothing — because there is nothing to report yet, which is
a different fact from "ran and found no gaps". A caller checking closure reads
`points()` being empty as its own signal; `fully_covered()` is likewise `false`
for a zero-bin entry. The whole point is that a regression cannot pass coverage
by never having run.

`CoverageDatabase`/`CoverageEntry` are Rust-only as of this delivery, the same
tracked-asymmetry status as §15's driver surface.
