# Part VIII — Host APIs: Python and Rust

Driving a simulation is a **host** concern, not a language concern. PHDL
describes circuits (Parts I–II); hosts elaborate, compile, solve, and
measure. There are exactly two host surfaces, and they are one surface:

- **Python** (`import piperine`) — the scripting host. Testbenches are plain
  Python files (`*_tb.py`), run by `piperine test`; scripts run with
  `piperine run script.py`; an interactive REPL is `piperine run -i`.
- **Rust** (the root `piperine` crate) — the same session/results/waveform
  plumbing the Python binding wraps (MD-19: the root crate is the complete
  external view of the project). `piperine::prelude` is the one-import face.

The in-language `bench` block was removed (2026-07-17): a `bench` block is a
plain syntax error, and the interpreted context no longer exists. Everything
it did — analyses, measurement, parameter sweeps, assertions — is done by a
host, in Python or Rust, with no new syntax.

## 1. The call graph (uniform shape)

```python
import piperine

design  = piperine.load("chip.phdl")     # -> Design (elaborated POM)
module  = design.module("Amp")           # -> Module (reflected view + analyses)
op      = module.op()                    # -> OpResult
v_out   = op.v("out")                    # -> float
i_src   = op.i("vin", "gnd")             # -> float (branch current a -> b)

trace   = module.tran(piperine.TranConfig(stop=1e-3))
wave    = trace.v("out")                 # -> Waveform (time axis)
wave.values                            # np.ndarray (real)
wave.axis                              # np.ndarray (time)
t_cross = wave.cross(2.5, "Rising")      # float | None

ac      = module.ac(piperine.AcConfig(fstart=1.0, fstop=1e9))
mag     = ac.v("out").mag()              # ComplexWaveform -> Waveform
ndb     = ac.v("out").db()

nz      = module.noise(piperine.NoiseConfig(out="out", fstart=1.0, fstop=1e6))
psd     = nz.psd()                       # Waveform over frequency
total   = nz.total()                     # integrated RMS noise (float)
```

The Rust shape is identical (`SimSession::run_op/run_tran/run_ac/run_noise`
returning `OpResult`/`Trace`/`AcTrace`/`NoiseTrace`, `Waveform` readouts).

## 2. `load` and `Design`

`piperine.load(path)` parses and elaborates a `.phdl`/`.ppr` file into a
`Design` — or raises `ValueError` with the diagnostic (never a silent
success).

| Method | Returns | Notes |
|--------|---------|-------|
| `design.top()` | `Module \| None` | the elaborated top module |
| `design.module(name)` | `Module` | `ValueError` if absent |
| `design.modules()` | `list[Module]` | every elaborated module |
| `design.const_(name)` | value or `None` | a global constant |
| `design.select(path)` | `Selection` | Part IV selector path |
| `design.compile(module=None)` | `LiveSession` | §5 |

A `Design` is read-only for the host: parameter overrides are staged per
`Module` and replayed onto a fork per analysis — the parent design is never
mutated.

## 3. `Module`: reflection and analyses

Reflection is read-only: `name`, `ports()`, `nets()`, `instances()`,
`params()`, `behaviors()`.

Each analysis takes a config dataclass (defaults mirror
`headers/prelude.phdl`):

```python
module.op(piperine.OpConfig(nodeset={"out": 5.0}, solver=piperine.Solver(reltol=1e-4)))
module.tran(piperine.TranConfig(stop=1e-3, step=0.0, start=0.0, ic={"out": 0.0}))
module.ac(piperine.AcConfig(fstart=1e3, fstop=1e6, points=100, scale=piperine.Scale.Dec))
module.noise(piperine.NoiseConfig(out="out", fstart=1e3, fstop=1e6))
```

- `step = 0.0` selects the adaptive stepper (a positive `step` is the initial
  `dt`); `start` is the earliest **recorded** time (the solver integrates
  from `t = 0` regardless).
- `nodeset`/`ic` are `{net_name: volts}` maps seeding the Newton guess /
  t=0 state.
- `Solver` fields: `temperature`, `reltol`, `abstol`, `gmin`, `max_iter`.

`module.set(label, param, value)` stages an override consumed by the next
analysis on that module — sweeps are native Python loops:

```python
for rl in [2e3, 1e3, 500.0]:
    m = design.module("DividerBoard")
    m.set("r_bot", "r", rl)
    assert abs(m.op().v("mid") - 5.0 * rl / (3e3 + rl)) < 1e-6
```

## 4. Result objects

| Type | Readouts |
|------|----------|
| `OpResult` | `.v(net[, ref]) -> float`, `.i(a[, b]) -> float`, `.stats -> SolverStats`, `result["inst.path"] -> InstanceView` |
| `Trace` | `.v(net[, ref]) -> Waveform`, `.i(a[, b]) -> Waveform`, `.axis() -> Waveform`, `.stats` |
| `AcTrace` | `.v(net[, ref]) -> ComplexWaveform`, `.axis() -> Waveform` |
| `NoiseTrace` | `.psd() -> Waveform`, `.total() -> float` |
| `Waveform` | `.values`/`.axis` (numpy), `.at(x)`, `.min()/.max()/.mean()/.rms()/.peak_to_peak()`, `.cross(level[, dir])`, `len()` |
| `ComplexWaveform` | `.mag()/.phase()/.db() -> Waveform`, `.at(f)` |

Unknown nets raise `KeyError` ("not addressable") — measurement failures are
loud, never a silent `0.0` or NaN. Digital nets read their logic value
(0/1) directly from `OpResult.v`/`Trace.v`.

## 5. `LiveSession`: compile once, set, re-run

For optimization loops and parameter studies, elaboration and JIT must
happen **once** (MD-18); re-elaborating inside a simulation loop is an
architecture defect.

```python
live = design.compile("Fitter")          # or module.compile()
for guess in candidates:
    live.set("r_top", "r", guess)        # solver-level restamp — no re-JIT
    err = abs(live.op().v("out") - target)

live.schedule_set(5e-6, "r_top", "r", 2e3)   # applied mid-tran at t = 5 µs
trace = live.tran(piperine.TranConfig(stop=20e-6))
live.rebuilds                          # structural auto-rebuilds so far
```

- `set`/`schedule_set` address instances by their PHDL labels (bundle fields
  flatten to `{param}_{field}`, e.g. `model_is`); unknown names raise
  `KeyError` listing the element's parameters, out-of-bounds values raise
  `ValueError`.
- `schedule_set` lands exactly on its timestamp (forced breakpoint);
  same-parameter sets apply in scheduling order (last write wins).
- A **structural** set (one the restamp path cannot express, e.g. changing
  a parameter that alters the event topology) triggers an automatic
  re-elaboration with a notice, carrying net state by name; `rebuilds`
  counts these. A failed rebuild keeps the old circuit.

## 6. The CLI as host

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

## 7. The Rust host

```rust
use piperine::prelude::*;

let design = parse_and_elaborate(&src, &SourceMap::dummy())?;
let session = SimSession::new(design, "Divider".to_string());
let op = session.run_op(&SolverConfig::default(), None)?;
let mid = NetRef { name: "mid".into() };
assert!((op.v(&mid, None)? - 2.0).abs() < 1e-9);
```

`piperine::prelude` re-exports the session (`SimSession`, `SolverConfig`),
result objects (`OpResult`, `Trace`, `AcTrace`, `NoiseTrace`, `Waveform`),
the `SimHooks` lifecycle trait (SPEC Part VI §8), and the public faces of
`piperine-lang` (`parse_and_elaborate`, `Design`, `SourceMap`),
`piperine-codegen` (`CircuitCompiler`, `DeviceProvider`) and
`piperine-solver` (its `prelude`).
