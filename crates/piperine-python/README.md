# piperine-python

Python bindings for the Piperine analog/mixed-signal simulator — the uniform
host-neutral API (spec §10). The same call graph the bench layer exposes
(`load → Design → module → op/tran/ac/noise → results.v(net)`), presented
idiomatically with typed dataclasses, `__getitem__`, and numpy arrays.

## Requirements

- **Python 3.9+** with **numpy** installed (the binding returns `np.ndarray`
  for waveforms; numpy is a hard dep).
- CPython dev headers (for building from source).

## Two ways to run

**`piperine run script.py`** (embedded CPython — no pip install):

```sh
piperine run my_analysis.py
```

The CLI embeds CPython, registers the `piperine` module, and runs the script.
`import piperine` just works.

**`pip install piperine`** (wheel via maturin — follow-up): the native
extension (`_piperine`) + the typed facade (`piperine/`) ship as a wheel.

## Quickstart

```python
import piperine
import numpy as np

design  = piperine.load("chip.phdl")        # -> Design
module  = design.module("Amp")               # -> Module
op      = module.op()                        # -> OpResult
v_out   = op.v("out")                        # -> float
trace   = module.tran(piperine.TranConfig(stop=1e-3, step=1e-6))  # -> Trace
wave    = trace.v("out")                     # -> Waveform
values  = wave.values                        # -> np.ndarray (real)
axis    = wave.axis                          # -> np.ndarray (time)
```

### The four analyses

```python
op     = module.op()                                              # DC op
trace  = module.tran(piperine.TranConfig(stop=1e-3, step=1e-6))   # transient
ac     = module.ac(piperine.AcConfig(fstart=1, fstop=1e6, points=100))  # AC sweep
noise  = module.noise(piperine.NoiseConfig(out="out", fstart=1, fstop=1e6))  # noise
```

### Reading results

```python
# OpResult — scalars
op.v("out")          # node voltage (float)
op.v("a", "b")       # differential a - b
op.i("a", "b")       # branch current a -> b
op["out"]            # == op.v("out")  (net-name access)

# Trace — waveforms over time
trace.v("out").values       # np.ndarray (real)
trace.v("out").axis         # np.ndarray (time)
trace["out"]                # == trace.v("out")  (net-name access)

# ComplexWaveform (AC) — complex arrays + projections
ac.v("out").values          # np.ndarray (complex128)
ac.v("out").mag             # -> Waveform (real, |H|)
ac.v("out").phase           # -> Waveform (real, arg(H))
ac.v("out").db              # -> Waveform (real, 20·log10|H|)

# NoiseTrace — PSD + integrated total
noise.psd().values          # np.ndarray (V²/Hz)
noise.total()               # float (RMS)
```

### Instance-path access

`result["instance"]` returns a terminal sub-view of that instance's quantities
(terminal voltages + branch current), resolved through the POM hierarchy:

```python
view = op["r1"]             # -> InstanceView (the r1 instance)
view.terminals()            # [(port, net), ...]
view.v("p")                 # voltage at r1's port p (the connected net)
view.v("p", "n")            # differential across r1
view.i("p", "n")            # branch current through r1
```

### Param staging + sweeps

Staging overrides the next analysis; sweeps are native Python `for` loops:

```python
for r in [1e3, 2e3, 5e3]:
    module.stage("r1", "r", r)   # override r1.r for the next analysis
    op = module.op()
    print(r, op.v("out"))
```

### Waveform stats

```python
wave.rms()              # time-weighted RMS
wave.mean()             # time-weighted mean
wave.min(), wave.max()  # extremes
wave.peak_to_peak()     # max - min
wave.at(2.5e-3)         # linear interpolation at t = 2.5 ms
```

## POM reflection

Read-only navigation of the elaborated design:

```python
design.modules()              # all modules
design.top()                  # the top module
module.ports()                # [(name, direction, type), ...]
module.nets()                 # wires
module.instances()            # submodule instances
module.params()               # params with defaults
module.behaviors()            # analog/digital blocks
design.select("/r1/port::p")  # Part IV selector — /-separated axis::name steps
```

## Architecture

A typed pure-Python facade (`piperine/__init__.py`) wraps a native PyO3
extension (`_piperine`). The facade provides IDE autocomplete + dataclasses;
runtime forwards to the native engine. Both ship together — there is no stub
drift.

```
foo.py  ──►  piperine (facade: dataclasses + annotations)
                  │
                  ▼
             _piperine (native PyO3)
                  │
                  ▼
        piperine_bench::SimSession + piperine_lang::pom::Design
```

## Build

The crate builds two ways via one Cargo feature:

- **Default (rlib)** — linked into the CLI's embedded interpreter + the test
  suite. PyO3 links libpython normally.
- **`extension-module` feature (cdylib)** — the importable `_piperine.so` for
  the maturin wheel. Enable only for the wheel build.
