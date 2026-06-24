# Python Bridge (PyO3)

## Core idea

`.ppr` files = hardware description only. No testbench code.  
Testbenches = Python files. `import piperine` loads a native Rust extension
(`piperine.so`) built with maturin.

Hardware and its bench live together by folder:

```
my_project/
  piperine.toml
  divider/
    divider.ppr     # circuit description
    divider.py      # testbench — pure Python
  lpf/
    lpf.ppr
    lpf.py
```

---

## API

### `NgspiceSession`

Primary entry point. Parses + elaborates the `.ppr` file, spawns a worker
subprocess, and loads the netlist — all in one call:

```python
import piperine as ppr

sess = ppr.NgspiceSession.from_file("lpf/lpf.ppr", module="lpf_tb")
```

`module` selects which top-level module to elaborate (optional when there is
only one non-extern module).

### Analyses

```python
op   = sess.op()                        # dict[str, float]
tran = sess.tran("1n", "1u")           # dict[str, np.ndarray]
ac   = sess.ac("dec", 50, 100.0, 1e5)  # dict[str, np.ndarray]
dc   = sess.dc("V1", 0, 5, 0.01)       # dict[str, np.ndarray]
```

AC vectors: `ac["frequency"]` (real), `ac["vout"]` (magnitude), plus
`ac["vout.re"]` and `ac["vout.im"]` for the complex components.

### In-run control

```python
sess.alter("R1", "resistance", 1500.0)
sess.altermod("BC548", "bf", 250.0)
sess.alterparam("temp", 27.0)
sess.set_option("reltol", 1e-4)
sess.set_temp(85.0)
```

### Async / parallel

Every blocking analysis has a `_async` twin that returns a `SimFuture`:

```python
futures = [sess.tran_async("1n", "1u") for sess in sessions]
results = ppr.join_all(futures)   # wall time ≈ slowest worker
```

`tran_async` spawns a Rust thread and returns immediately — GIL fully released
during simulation. `join_all` waits on all futures in wall-clock parallel.

---

## Testbench examples

### OP — voltage divider

```python
import piperine as ppr

sess = ppr.NgspiceSession.from_file("divider/divider.ppr", module="divider")
op = sess.op()
print(f"V(vmid) = {op['vmid']:.3f} V")
```

### AC sweep — RC low-pass filter

```python
import piperine as ppr
import numpy as np

sess = ppr.NgspiceSession.from_file("lpf/lpf.ppr", module="lpf_tb")
ac = sess.ac("dec", 50, 100.0, 100e3)

freq = ac["frequency"]
vout_db = 20 * np.log10(ac["out"] / ac["out"][0])
idx = np.argmin(np.abs(vout_db + 3.0))
print(f"fc (-3 dB) ≈ {freq[idx]:.1f} Hz")
```

### Parallel Monte Carlo — 30 workers

```python
import piperine as ppr
import numpy as np

N = 30
sessions = [ppr.NgspiceSession.from_file("lpf/lpf.ppr", module="lpf_tb")
            for _ in range(N)]

rs = np.random.normal(1000, 30, N)
cs = np.random.normal(100e-9, 3e-9, N)
for sess, r, c in zip(sessions, rs, cs):
    sess.alter("R1", "resistance", r)
    sess.alter("C1", "capacitance", c)

futures = [sess.tran_async("1n", "1u") for sess in sessions]
results = ppr.join_all(futures)
```

All 30 workers run in parallel. Wall time ≈ slowest worker, not sum.

---

## Build

```sh
# dev install (live-reloads the .so into the current venv)
cd crates/piperine-python
maturin develop

# or via piperine CLI (sets up project-local .venv)
piperine setup
piperine run lpf/lpf.py
```

`piperine setup` copies the worker binary to `.venv/bin/piperine-worker`.
`piperine run` sets `PIPERINE_WORKER` env var so `NgspiceSession` finds it
regardless of `current_exe()` returning a Python path.

---

## Crate: `piperine-python`

Type: `cdylib` (PyO3 native extension → `piperine.so`).

Dependencies: `piperine-parser`, `piperine-circuit`, `piperine-ngspice`,
`piperine-coordinator`, `pyo3`, `numpy` (pyo3-numpy).

Key types:

| Rust type | Python type | Role |
|-----------|-------------|------|
| `NgspiceSession` | `piperine.NgspiceSession` | Session per circuit; owns one worker |
| `SimFuture` | `piperine.SimFuture` | Handle to an in-flight async analysis |

`ppr.join_all(futures)` is a free function that drains all futures in parallel
(sequentially joining Rust thread handles that are already running in parallel).

---

## GIL accounting

| Call | GIL held? |
|------|-----------|
| `NgspiceSession.from_file(...)` | Yes (parse + elaborate + spawn worker) |
| `sess.op()` / `sess.tran()` / `sess.ac()` | Released via `py.allow_threads` |
| `sess.tran_async(...)` | Yes briefly (spawns thread only) |
| Inside the spawned thread | No (pure Rust IPC, no Python) |
| `future.join()` / `ppr.join_all(...)` | Released while waiting |

Multiple `NgspiceSession` instances run truly in parallel — worker crashes
are isolated to one session and cannot reach Python.

---

## `always @(step)` SOA guards

`always @(step)` blocks in hardware modules are **not** testbench constructs.
They compile to `.meas` SPICE lines at elaboration — no runtime interpreter.

```verilog
module bjt_stage;
    // ...
    always @(step) begin
        if (V(c) > 30.0) $run_error("Vce_max");
    end
endmodule
```

After a transient, call `sess.check_soa()` to raise if any limit was violated.
See `ARCHITECTURE.md §SOA compilation` for how the lowering works.
