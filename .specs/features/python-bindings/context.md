# Piperine Python Bindings Context

**Gathered:** 2026-07-15
**Spec:** `.specs/features/python-bindings/spec.md`
**Status:** Ready for design

---

## Feature Boundary

A Python library (PyO3) exposing the **identical** bench + POM surface (spec
§10 uniform host-neutral API): `load()` → `Design` → `module()` →
`op/tran/ac/noise` → results as numpy arrays, with `result["net"]` and
`result["instance.path"]` access. The CLI gains `piperine run script.py`
(embedded CPython, no pip install). A typed pure-Python facade gives IDE
autocomplete. Read-only POM reflection + param staging (no PHDL editing).

---

## Implementation Decisions

### Execution model — embed CPython

- **Decision:** `piperine run script.py` embeds CPython (PyO3
  `auto-initialize`), registers the piperine module as a built-in, and runs
  the script. `import piperine` works with **no pip install**.
- **Rationale:** self-contained — one binary, `piperine run foo.py` just works
  for real cases. The user explicitly wants this.

### Autocomplete — typed pure-Python facade

- **Decision:** The public API is a **pure-Python typed facade**
  (`piperine/__init__.py`) with dataclasses and full annotations, wrapping the
  native PyO3 extension (`_piperine` cdylib). The IDE reads the `.py`
  (autocomplete, docstrings); runtime forwards to native (negligible cost).
- **Rejected:** hand-maintained `.pyi` stub (drifts); un-annotated native-only
  module (no autocomplete).
- **Rationale:** the user requires IDE autocomplete; a typed facade is the gold
  standard and doubles as documentation. `import piperine` is explicit (user-
  confirmed).

### `result["name"]` — net AND instance

- **Decision:** `result["net_name"]` returns the net's primary array/scalar
  (== `result.v("net_name")`). `result["instance.path"]` returns a terminal
  sub-view (the instance's terminal voltages + branch current), resolved via
  the POM selector.
- **Rationale:** covers both patterns the user cited (`result["buck.resistor1"]`).

### Config bundles — typed dataclasses

- **Decision:** `Solver`, `OpConfig`, `TranConfig`, `AcConfig`, `NoiseConfig`
  as Python dataclasses (in the facade) mirroring the PHDL bundles. Construction
  by kwargs with declared defaults.
- **Rationale:** autocomplete + idiomatic + matches spec §10 `TranConfig { .stop=... }`.

### Numpy exposure — two separate arrays

- **Decision:** `Waveform.values` → real `np.ndarray`; `Waveform.axis` →
  time/freq `np.ndarray` (equal length). `ComplexWaveform.values` → complex
  `np.ndarray`. `Trace.axis()` returns the axis array.
- **Rationale:** idiomatic for `matplotlib.plot(trace.axis, trace.v("out").values)`.

### POM access — read-only reflection + staging

- **Decision:** `Design`/`Module`/`Port`/`Net`/`Instance`/`Param`/`Behavior`
  are read-only reflected views over the POM. The only mutation is
  `module.stage(label, param, value)` (the bench override). Sweeps are native
  Python `for` loops.
- **Rationale:** matches the bench capability; the bench never edits structure,
  only stages overrides.

---

## Specific References

- **Uniform API contract:** `docs/spec/part_iii_interpreted_context.md` §10
  (load/Design/Module/op/tran/ac/noise; "never a different shape").
- **Existing Rust surface to wrap:**
  - `piperine_lang::parse_and_elaborate(&str, &SourceMap) -> Design`
    (`piperine-lang/src/lib.rs:66`).
  - `Design` reflection (`pom/design.rs:155-292`): top/module/modules/select/
    const_/disciplines/bundles/enums/...
  - `Module` reflection (`pom/module.rs:210-264`): ports/nets(wires)/instances/
    params/behaviors + per-name lookups.
  - `piperine_bench::SimSession::run_op/run_tran/run_ac/run_noise`
    (`session.rs:155-270`) — the four analysis entry points.
  - `SimSession::stage(label, param, Value)` (`session.rs:123`) — override.
  - Result objects (`objects.rs`, `waveform.rs`): `OpResult`, `Trace`,
    `AcTrace`, `NoiseTrace`, `Waveform`, `ComplexWaveform` — methods today
    dispatched by name via `impl Object`; the binding writes thin typed
    wrappers.
  - `piperine_bench::NetRef { name: String }` (`objects.rs:17`) — the net
    handle result methods accept.
- **CLI pattern:** `piperine-cli/src/lib.rs` clap `Commands` enum +
  `commands/<name>.rs::execute()`; a `Run` subcommand already exists for bench
  entries — the Python `run` is a new path/flag.
- **Result numpy seam:** `Waveform.points: Vec<(f64, T)>` (`waveform.rs:21`) —
  split into two `np.ndarray`s via PyO3's numpy binding (`pyo3 + numpy`).

---

## Deferred Ideas

- **`pip install piperine` wheel distribution** (manylinux/macOS wheels via
  maturin + CI) — the embedded CLI path ships first; broad packaging later.
- **Plugin host control from Python** (load/exercise plugins) — Part VI follow-up.
- **PHDL structure editing from Python** (add modules/nets) — out of scope;
  POM stays read-only + staging.
- **`$write`/`$plot` task mirrors** — Python has csv/matplotlib natively.
