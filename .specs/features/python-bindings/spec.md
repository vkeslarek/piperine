# Piperine Python Bindings Specification

**Implements:** docs/spec `part_iii_interpreted_context.md` §10 "The uniform
host-neutral API" — the Python host. The bench `SimHost` is one host; this is
another, presenting the **identical** operation set idiomatically.
**ROADMAP reference:** "The uniform API (was G12) — milestone".

## Problem Statement

Piperine's bench layer (`piperine-bench`) is the only way to run a simulation
and read results today, and it lives inside a `.phdl` source file. To use
Piperine on a real design — visualize waveforms, sweep parameters, feed
results into the scientific-Python stack (numpy/matplotlib/pandas) — there is
no path. This feature exposes the **complete bench + POM surface** as a Python
library so anything doable in a bench is doable from Python, with results as
numpy arrays. The CLI gains `piperine run script.py`, which embeds CPython and
runs the script with `import piperine` available.

## The uniform-API mandate (binding)

Per spec §10, "the complete operation set is modeled once and exposed
identically from every host." The Python host **must mirror** the bench/Rust
shape — `load()` → `Design` → `module()` → `op/tran/ac/noise` → results with
`.v(net)` — never a different shape. Python presents it idiomatically
(dataclasses, kwargs, `__getitem__`) but the types and call graph are the same
contract. This is a hard requirement (PY-15), not a nicety.

## Goals

- [ ] `piperine.load(path) -> Design` — load a `.phdl`/`.ppr` file into the POM.
- [ ] **Full POM reflection** — navigate `Design` → modules → ports/nets/
      instances/params/behaviors, plus `select(path)` (Part IV selector).
- [ ] **All four analyses** from a module — `.op()/.tran()/.ac()/.noise()` with
      typed config dataclasses mirroring the PHDL bundles.
- [ ] **Results as numpy arrays** — waveforms expose `.values` and `.axis` as
      `np.ndarray` (real or complex); point results expose scalars.
- [ ] **Name-based access** — `result["net"]` (== `.v(net)`) and
      `result["instance.path"]` (a terminal sub-view).
- [ ] **Param staging** — `module.stage(label, param, value)` (the bench
      override mechanism; sweeps are native Python `for` loops).
- [ ] **`piperine run script.py`** — the CLI embeds CPython, registers the
      piperine module, and runs the script; `import piperine` works with no
      `pip install`.
- [ ] **IDE autocomplete** — a typed pure-Pthon facade (`piperine/__init__.py`)
      wraps the native extension so PyCharm/Pylance/mypy see full annotations.
- [ ] Zero behavior change to the Rust workspace; the binding is a new crate.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Editing PHDL from Python (add modules/nets) | POM reflection is read-only; param staging is the only mutation (matches bench) |
| Plugin loading/control from Python | Plugin host is a Rust-side concern (Part VI); follow-up |
| `$write`/`$plot`/`$assert`/`$display` task mirrors | Python has csv/matplotlib/assert/print natively; sweeps are `for` loops |
| Output-grid interpolation, TR-BDF2 cleanups | Separate feature (`solver-trbdf2-engine`); explicitly deferred |
| Installing into arbitrary virtualenvs / manylinux wheels | Ship the wheel + the embedded-CLI path; broad packaging is a follow-up |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| `piperine run script.py` execution | **Embed CPython** (PyO3 auto-init; register `piperine` as built-in) | Self-contained — `piperine run foo.py` works with no pip install | y |
| `result["name"]` semantics | **Net AND instance** — `result["out"]` → net array; `result["buck.r1"]` → terminal sub-view | Covers both patterns the user cited | y |
| Config bundles | **Typed dataclasses** (`TranConfig(stop=..., step=...)`, `Solver(...)`) | Mirror PHDL bundles; autocomplete; idiomatic | y |
| Numpy exposure | **Two separate arrays** — `.values` + `.axis` | Idiomatic for plotting; AC → complex array | y |
| Autocomplete mechanism | **Typed pure-Python facade** wrapping native `_piperine` extension | Best IDE support (real annotations, no stub drift); negligible runtime cost | y |
| Explicit `import piperine` | Required in every script (standard Python) | User-confirmed | y |
| POM mutation | Read-only reflection + `stage()` for params only | Matches bench capability | y |

**Open questions:** none — all resolved above.

---

## User Stories

### P1: Load + reflect + run (the vertical slice) ⭐ MVP

**User Story**: As a circuit designer, I want to load a design, run an
operating point, and read a node voltage in Python, so I can script real
designs and feed results into numpy/matplotlib.

**Why P1**: This is the load→reflect→analyze→read loop; everything else
extends it.

**Acceptance Criteria**:

1. WHEN `piperine.load("chip.phdl")` is called THEN it SHALL return a `Design`
   object reflecting the elaborated POM (or raise on parse/elab error).
2. WHEN `design.module("Amp")` is called THEN it SHALL return that `Module`
   (or raise if absent); `design.modules()` SHALL list all modules;
   `design.top()` SHALL return the top module.
3. WHEN `module.op()` is called THEN it SHALL return an `OpResult`.
4. WHEN `op.v("out")` is called THEN it SHALL return the node voltage (float);
   `op.v("a", "b")` SHALL return the differential; `op.i("a", "b")` SHALL
   return the branch current.
5. WHEN `op["out"]` is called THEN it SHALL equal `op.v("out")`.

### P1: Transient/AC/Noise + numpy arrays ⭐ MVP

6. WHEN `module.tran(TranConfig(stop=1e-3, step=1e-6))` is called THEN it SHALL
   return a `Trace`.
7. WHEN `trace.v("out")` is called THEN it SHALL return a `Waveform` whose
   `.values` is a real `np.ndarray` and `.axis` is the time `np.ndarray`
   (equal length); `trace.axis()` SHALL return the time array.
8. WHEN `module.ac(AcConfig(fstart=1, fstop=1e6, points=100))` is called THEN
   `ac.v("out")` SHALL return a `ComplexWaveform` whose `.values` is a
   complex `np.ndarray`; `.mag`/`.phase`/`.db` SHALL return real `Waveform`s.
9. WHEN `module.noise(NoiseConfig(out="out", fstart=1, fstop=1e6))` is called
   THEN `noise.psd()` SHALL return a `Waveform` and `noise.total()` a float.
10. WHEN a `Waveform` is indexed `trace["out"]` THEN it SHALL return the
    `.values` array (== `trace.v("out").values`).

### P1: Param staging + sweeps ⭐ MVP

11. WHEN `module.stage("r1", "r", 2e3)` is called THEN the next analysis on
    that module SHALL use `r = 2e3` for instance `r1` (the bench override
    semantics); staging is pure — the held `Design` is not mutated.
12. WHEN a user writes a Python `for` loop staging a param and calling
    `.op()` each iteration THEN each result SHALL reflect that iteration's
    staged value (sweeps are native loops, no special API).

### P2: Instance name-based access

13. WHEN `op["buck.r1"]` (or `trace["buck.r1"]`) is called for an instance
    path THEN it SHALL return an instance sub-view exposing that instance's
    terminal quantities (terminal voltages and the branch current), resolved
    through the POM hierarchy / `select(path)`.

### P2: POM reflection depth

14. WHEN `module.ports()/nets()/instances()/params()/behaviors()` are called
    THEN each SHALL return a list of typed objects (`Port`, `Net`/`Wire`,
    `Instance`, `Param`, `Behavior`) with their attributes (name, type,
    direction, default value, connected nets).
15. WHEN `design.select("buck.r1.p")` is called THEN it SHALL return the
    resolved selection (Part IV selector), navigable to the node.

### P2: `piperine run script.py`

16. WHEN `piperine run foo.py` is invoked THEN the CLI SHALL embed CPython,
    register the `piperine` module as a built-in, and execute `foo.py` with
    `import piperine` resolving to the embedded module — **no `pip install`
    required**.
17. WHEN the embedded script raises THEN the CLI SHALL propagate the Python
    traceback to stderr and exit non-zero (no silent swallow).

### P2: IDE autocomplete

18. WHEN a user edits a script in PyCharm/Pylance THEN autocomplete SHALL
    offer `piperine.load`, `Design.module`, `Module.op`, `TranConfig(stop=)`,
    `Waveform.values`, etc. — driven by the typed pure-Python facade
    (`piperine/__init__.py` with full annotations + dataclasses).

---

## Edge Cases

- WHEN `load()` is given a nonexistent path THEN it SHALL raise a Python
  `FileNotFoundError`/`ValueError` with the parse/elab message.
- WHEN `.v(net)` names a net not in the module THEN it SHALL raise
  `KeyError`/`ValueError` (fail loud, never silent NaN).
- WHEN `.v(net)` names a digital net (in an OpResult) THEN it SHALL return
  0/1 (matching the bench's Bit-net read).
- WHEN `result["instance.path"]` does not resolve THEN it SHALL raise
  `KeyError`.
- WHEN a waveform is empty THEN `.values` SHALL be an empty `np.ndarray`
  (not None).
- WHEN an analysis fails to converge THEN it SHALL raise a Python exception
  carrying the solver error domain + message.

---

## Requirement Traceability

| ID | Story | Status |
|----|-------|--------|
| PY-01 | P1 load | Pending |
| PY-02 | P1 Design reflection | Pending |
| PY-03 | P1 Module reflection | Pending |
| PY-04 | P1 analyses (op/tran/ac/noise) | Pending |
| PY-05 | P1 config dataclasses | Pending |
| PY-06 | P1 OpResult access | Pending |
| PY-07 | P1 Trace + numpy | Pending |
| PY-08 | P1 Waveform numpy + stats | Pending |
| PY-09 | P1 ComplexWaveform (AC) | Pending |
| PY-10 | P1 NoiseTrace | Pending |
| PY-11 | P1 net-name access | Pending |
| PY-12 | P1 param staging + sweeps | Pending |
| PY-13 | P2 instance-name access | Pending |
| PY-14 | P2 select(path) | Pending |
| PY-15 | P2 piperine run script.py | Pending |
| PY-16 | P2 IDE autocomplete (facade) | Pending |
| PY-17 | uniform shape (§10) — binding invariant | Pending |

**Coverage:** 17 total, 0 mapped to tasks ⚠️.

**Status values:** Pending → In Design → In Tasks → Implementing → Verified

---

## Success Criteria

- [ ] A Python script: `load → module → tran → trace.v("out").values` returns
      a numpy array that matches the bench's `$tran` + `Trace.v(out)` for the
      same circuit (the uniform-shape proof).
- [ ] `piperine run foo.py` executes with `import piperine`, no pip install.
- [ ] IDE autocomplete works on the full surface (typed facade).
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` green;
      a Python smoke test (pytest or embedded) exercises load+op+tran+numpy.
