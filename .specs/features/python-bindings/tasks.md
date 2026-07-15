# Piperine Python Bindings Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** If the skill cannot be activated, STOP and tell the user.

---

**Design**: `.specs/features/python-bindings/design.md`
**Status**: Draft

---

## Test Coverage Matrix

> Generated from `AGENTS.md` (Test placement) + spec. Guidelines: `AGENTS.md` (Hard rules, Test placement), spec §10. **Note:** the `piperine-python` crate requires CPython dev headers + `numpy` installed; a Rust test embeds the interpreter for the Python smoke.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Native PyO3 wrappers (load/Design/Module/results) | unit (Rust, embedded-Python where needed) | 1:1 to spec ACs; uniform-shape vs bench | `crates/piperine-python/src/*.rs` (`#[cfg(test)]`) | `cargo test -p piperine-python` |
| End-to-end Python (load→op→tran→numpy) | integration (embedded script) | The success-criteria script; autocomplete not asserted by test (manual) | `crates/piperine-python/tests/smoke.rs` | `cargo test -p piperine-python --test smoke` |
| CLI `run` embedding | integration | `piperine run foo.py` executes; errors propagate | `crates/piperine-cli/tests/` (if present) or manual | `cargo test -p piperine-cli` |
| Facade (pure Python) | none (build gate; runtime exercised by smoke) | — | `crates/piperine-python/python/piperine/__init__.py` | build gate |

## Gate Check Commands

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Crate | After native-wrapper tasks | `cargo test -p piperine-python` |
| Python | After facade / embedding tasks | `cargo test -p piperine-python --test smoke` |
| Build | Every task | `cargo build --workspace` (zero warnings) |
| Full | Phase end | `cargo test --workspace` |

---

## Execution Plan

Phases are ordered and run sequentially.

```
Phase 1 (scaffold+seams) → Phase 2 (POM reflection) → Phase 3 (analyses+results+numpy)
                                                        ↓
                          Phase 4 (facade+autocomplete) → Phase 5 (piperine run + tests)
```

### Phase 1: Scaffold + bench readout seams

```
P1 → P2 → P3
```

### Phase 2: POM reflection

```
P4 → P5
```

### Phase 3: Analyses + results + numpy

```
P6 → P7 → P8 → P9
```

### Phase 4: Facade + autocomplete

```
P10
```

### Phase 5: piperine run + tests

```
P11 → P12 → P13
```

---

## Task Breakdown

### P1: Create `piperine-python` crate (PyO3 scaffold)

**What**: New workspace member `crates/piperine-python`. `Cargo.toml` with `pyo3` (feature `extension-module` optional, off by default) + the `numpy` crate + `piperine-lang`, `piperine-bench`, `piperine-project`. `lib.rs` with `#[pymodule] fn _piperine(...)` (empty) + `#[pyfunction] fn load(path) -> PyResult<_Design>` stub. Add to the workspace `Cargo.toml`. Verify `cargo build --workspace` is green (Python headers must be present).
**Where**: `crates/piperine-python/{Cargo.toml,src/lib.rs}`, root `Cargo.toml`
**Depends on**: None
**Requirement**: PY-01 (scaffold)

**Done when**:
- [ ] `cargo build --workspace` zero warnings (pyo3 resolves Python)
- [ ] `_piperine` module + `load` stub compile
**Tests**: none (scaffold)
**Gate**: build
**Commit**: `feat(python): piperine-python crate scaffold (PyO3)`

---

### P2: Expose bench result readouts (`pub` thin wrappers)

**What**: The result-object methods (`OpResult::v/i`, `Trace::v/i/axis`, `Waveform` accessors) are today `impl Object` dispatch, not `pub fn`. Add thin `pub fn v(&self, net: &NetRef) -> Result<f64,...>` etc. (or `pub(crate)`) so the PyO3 wrappers can call them. Surgical — move the dispatch bodies into named public methods, keep the `impl Object` dispatch delegating to them.
**Where**: `crates/piperine-bench/src/{objects.rs,waveform.rs}`
**Depends on**: None
**Requirement**: PY-06/07/08 (enabler)

**Done when**:
- [ ] `OpResult`, `Trace`, `Waveform`, `ComplexWaveform`, `AcTrace`, `NoiseTrace` have `pub` typed accessor methods callable from outside the crate
- [ ] `cargo test -p piperine-bench` green (behavior unchanged)
**Tests**: unit (existing bench tests cover behavior)
**Gate**: `cargo test -p piperine-bench`
**Commit**: `refactor(bench): pub typed accessors on result objects (python seam)`

---

### P3: `_piperine.load(path)` + `_Design` wrapper

**What**: Implement `load`: read file, project-aware `SourceMap` (via `piperine-project` when a project root exists, else dummy), `parse_and_elaborate`, wrap in `_Design` (owns `Rc<Design>`). `_Design` pyclass with `top()`, `module(name)`, `modules()`, `const_(name)` (read-only reflection starters). Errors → `PyValueError`.
**Where**: `crates/piperine-python/src/{lib.rs, design.rs}`
**Depends on**: P1
**Requirement**: PY-01, PY-02 (partial)

**Done when**:
- [ ] `load("path.phdl")` returns a `_Design`; `design.module("X")` returns the module; missing → error
- [ ] Unit test: load a tiny PHDL, assert module list
**Tests**: unit (embedded-Python via PyO3 in `#[cfg(test)]`)
**Gate**: crate
**Commit**: `feat(python): load() + Design reflection`

---

### P4: `_Module` reflection (ports/nets/instances/params/behaviors)

**What**: `_Module` pyclass (holds `(Rc<Design>, name)`, re-looks-up on each call). Methods `name/ports/nets/instances/params/behaviors` return lists of typed pyclasses (`_Port/_Net/_Instance/_Param/_Behavior`) with their attributes.
**Where**: `crates/piperine-python/src/module.rs`
**Depends on**: P3
**Requirement**: PY-03

**Done when**:
- [ ] All six reflection methods return typed lists with attributes
- [ ] Unit test: load, enumerate a module's nets/instances
**Tests**: unit
**Gate**: crate
**Commit**: `feat(python): Module POM reflection`

---

### P5: `select(path)` (Part IV selector)

**What**: `_Design.select(path)` → resolves a hierarchical path to a node selection, exposed as a typed object (the node + its kind). Instance-name access (PY-13) builds on this.
**Where**: `crates/piperine-python/src/design.rs`
**Depends on**: P3
**Requirement**: PY-14

**Done when**:
- [ ] `design.select("top.r1.p")` resolves; unresolved → error
- [ ] Unit test
**Tests**: unit
**Gate**: crate
**Commit**: `feat(python): Design.select(path) POM selector`

---

### P6: `_Module.op/tran/ac/noise` (wrap `SimSession`)

**What**: Native analysis methods on `_Module`. Each builds `SimSession::new(self.design.fork(), self.name)` and calls `run_op/run_tran/run_ac/run_noise` with positional args (mirror the `run_*` signatures). Return `_OpResult/_Trace/_AcTrace/_NoiseTrace`. Add `stage(label, param, value)` (PY-12).
**Where**: `crates/piperine-python/src/module.rs`
**Depends on**: P4
**Requirement**: PY-04, PY-12

**Done when**:
- [ ] `module.op()` returns `_OpResult`; `module.tran(stop, step, start, ic, solver)` returns `_Trace`; etc.
- [ ] `module.stage("r1","r",2e3)` overrides the next analysis
- [ ] Unit test: op on a divider, stage + re-op
**Tests**: unit
**Gate**: crate
**Commit**: `feat(python): Module.op/tran/ac/noise + stage`

---

### P7: `_OpResult` + `_Trace` + net-name `__getitem__`

**What**: `_OpResult.v(net)/.i(net_a,net_b)`, `_Trace.v(net)/.i(net)/.axis()`. `__getitem__("net")` → `OpResult` scalar / `Trace` → the Waveform (or its values). Build `NetRef` from `&str`.
**Where**: `crates/piperine-python/src/results.rs`
**Depends on**: P6, P2
**Requirement**: PY-06, PY-07, PY-11 (partial)

**Done when**:
- [ ] `.v/.i` return floats; `["net"]` matches `.v("net")`
- [ ] Unknown net → `PyKeyError`
- [ ] Unit test
**Tests**: unit
**Gate**: crate
**Commit**: `feat(python): OpResult + Trace result access`

---

### P8: `_Waveform` + numpy arrays

**What**: `_Waveform` pyclass with `.axis` and `.values` properties returning `np.ndarray` (built via `PyArray1::from_vec` from `points`). Stats methods (`.at/.rms/.mean/.min/.max/.peak_to_peak/.len`) delegating to the bench Waveform.
**Where**: `crates/piperine-python/src/waveform.rs`
**Depends on**: P7
**Requirement**: PY-08, PY-14

**Done when**:
- [ ] `.values`/`.axis` are `np.ndarray` of equal length; complex for AC
- [ ] Stats return correct floats (rms matches bench)
- [ ] Unit test: tran, assert `.values` length + a known value
**Tests**: unit (numpy assertions)
**Gate**: crate
**Commit**: `feat(python): Waveform → numpy arrays`

---

### P9: `_ComplexWaveform` + `_AcTrace` + `_NoiseTrace`

**What**: AC + noise result wrappers. `_ComplexWaveform.values` → complex `np.ndarray`; `.mag/.phase/.db` → `_Waveform`. `_AcTrace.v(net)/.axis()`. `_NoiseTrace.psd()/.total()`.
**Where**: `crates/piperine-python/src/waveform.rs`
**Depends on**: P8
**Requirement**: PY-09, PY-10

**Done when**:
- [ ] AC `.values` complex array; `.mag/.phase/.db` real Waveforms
- [ ] Noise `.psd()` Waveform, `.total()` float
- [ ] Unit test
**Tests**: unit
**Gate**: crate
**Commit**: `feat(python): ComplexWaveform + AcTrace + NoiseTrace`

---

### P10: Typed pure-Python facade (`piperine/__init__.py`)

**What**: Write `crates/piperine-python/python/piperine/__init__.py` — typed re-exports of the native classes, the config dataclasses (`Solver/OpConfig/TranConfig/AcConfig/NoiseConfig`), and `__getitem__`/instance-sub-view sugar. `from __future__ import annotations` + docstrings. The facade imports `_piperine`.
**Where**: `crates/piperine-python/python/piperine/__init__.py`
**Depends on**: P9
**Requirement**: PY-05, PY-13, PY-16

**Done when**:
- [ ] Facade loads (`import piperine` works when `_piperine` is importable)
- [ ] Dataclasses construct via kwargs with defaults
- [ ] `result["instance.path"]` returns a terminal sub-view
- [ ] Manual autocomplete check (IDE sees annotations)
**Tests**: none (build gate; exercised by smoke P13)
**Gate**: build + manual autocomplete
**Commit**: `feat(python): typed pure-Python facade + dataclasses`

---

### P11: `piperine run script.py` (embed CPython)

**What**: CLI `run` path for `.py` scripts. A `piperine_python::run_script(path)` helper: `append_to_inittab(_piperine)` → `prepare_freethreaded_python` → `PyModule::from_code(facade_src,…)` registered as `piperine` in `sys.modules` → `py.run(user_script)`. Wire into `piperine-cli` (detect `.py` on `run`, or a flag). Errors propagate to stderr + exit 1.
**Where**: `crates/piperine-python/src/embed.rs`, `crates/piperine-cli/src/commands/run.rs`
**Depends on**: P10
**Requirement**: PY-15

**Done when**:
- [ ] `piperine run foo.py` runs with `import piperine` resolving (no pip install)
- [ ] Script exception → traceback to stderr, non-zero exit
**Tests**: integration
**Gate**: full (CLI builds + runs)
**Commit**: `feat(cli): piperine run script.py (embedded CPython)`

---

### P12: Python smoke test (embedded)

**What**: A Rust integration test `tests/smoke.rs` that embeds Python and runs the success-criteria script: `load → module → tran → trace.v("out").values` is a numpy array matching the bench `$tran` for the same circuit (uniform-shape proof, PY-17). Also covers op/ac/noise + `["net"]` + `["instance.path"]`.
**Where**: `crates/piperine-python/tests/smoke.rs`
**Depends on**: P11
**Requirement**: PY-17 (uniform shape)

**Done when**:
- [ ] Smoke test passes: numpy array length + a known value within tolerance vs the bench
- [ ] Covers op/tran/ac + name access
**Tests**: integration (the gate itself)
**Gate**: `cargo test -p piperine-python --test smoke`
**Commit**: `test(python): embedded smoke test (uniform shape vs bench)`

---

### P13: Docs + workspace green

**What**: A short `crates/piperine-python/README.md` (or a section) with the Python quickstart mirroring spec §10's example. Confirm `cargo build --workspace` zero warnings + `cargo test --workspace` green.
**Where**: `crates/piperine-python/README.md`
**Depends on**: P12
**Requirement**: —

**Done when**:
- [ ] Quickstart doc with the load/module/op/tran example
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` green
**Tests**: build gate
**Gate**: full
**Commit**: `docs(python): quickstart; workspace green`

---

## Phase Execution Map

```
Phase 1:  P1 ──→ P2 ──→ P3
Phase 2:  P4 ──→ P5
Phase 3:  P6 ──→ P7 ──→ P8 ──→ P9
Phase 4:  P10
Phase 5:  P11 ──→ P12 ──→ P13
```

Execution is strictly sequential — one task at a time, in order.

---

## Task Granularity Check

| Task | Scope | Status |
|------|-------|--------|
| P1 scaffold | 1 crate + workspace | ✅ |
| P2 bench pub accessors | 2 files, surgical | ✅ |
| P3 load + Design | native wrappers | ✅ |
| P4 Module reflection | 1 pyclass family | ✅ |
| P5 select | 1 method | ✅ |
| P6 analyses + stage | 1 module (op/tran/ac/noise/stage) | ✅ |
| P7 OpResult + Trace | 2 result pyclasses | ✅ |
| P8 Waveform + numpy | 1 pyclass + numpy | ✅ |
| P9 AC/Noise results | 2 result pyclasses | ✅ |
| P10 facade | 1 .py file | ✅ |
| P11 piperine run | embed + CLI wiring | ✅ |
| P12 smoke test | 1 test file | ✅ |
| P13 docs | 1 README | ✅ |

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
|------|-------------------|---------------|--------|
| P1 | None | Phase 1 start | ✅ |
| P2 | None | Phase 1 | ✅ |
| P3 | P1 | P1 → P3 | ✅ |
| P4 | P3 | P3 → P4 | ✅ |
| P5 | P3 | P4 → P5 (P3 via P4) | ✅ |
| P6 | P4 | Phase 3 start ← P4 | ✅ |
| P7 | P6, P2 | P6 → P7 (P2 satisfied Phase 1) | ✅ |
| P8 | P7 | P7 → P8 | ✅ |
| P9 | P8 | P8 → P9 | ✅ |
| P10 | P9 | Phase 4 ← P9 | ✅ |
| P11 | P10 | P10 → P11 | ✅ |
| P12 | P11 | P11 → P12 | ✅ |
| P13 | P12 | P12 → P13 | ✅ |

---

## Test Co-location Validation

| Task | Code Layer | Matrix Requires | Task Says | Status |
|------|-----------|-----------------|-----------|--------|
| P1 | scaffold | none | none | ✅ |
| P2 | bench result objects | unit | unit (existing) | ✅ |
| P3 | native load/Design | unit | unit | ✅ |
| P4 | native Module | unit | unit | ✅ |
| P5 | native select | unit | unit | ✅ |
| P6 | native analyses | unit | unit | ✅ |
| P7 | native results | unit | unit | ✅ |
| P8 | native Waveform+numpy | unit | unit | ✅ |
| P9 | native AC/Noise | unit | unit | ✅ |
| P10 | facade (pure Python) | none (build) | none (smoke P12) | ✅ |
| P11 | CLI embed | integration | integration | ✅ |
| P12 | smoke (uniform shape) | integration | integration | ✅ |
| P13 | docs | build | build | ✅ |

All co-located; no test deferral. ✅
