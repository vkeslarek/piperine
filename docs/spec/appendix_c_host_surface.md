# Appendix C — Host API surface reference (Python + Rust)

Complete inventory of the two host surfaces as of 2026-07-18 (post
`api-crate`, pre `.sens`/PSS host bindings). The Python side was extracted
mechanically (introspection over `piperine.__all__` through the embedded
host); the Rust side is the `piperine-api` public surface. Part VIII is the
conceptual guide; this appendix is the flat reference — and the review sheet
for surface-design decisions (see §4).

---

## 1. Python surface (`import piperine`)

**Module exports (26):** `load` (fn) + `Design`, `Module`, `LiveSession`,
`Selection`, `Node`, `Instance`, `InstanceView`, `Terminal`, `Net`, `Port`,
`Param`, `Behavior` (reflection) + `OpConfig`, `TranConfig`, `AcConfig`,
`NoiseConfig`, `Solver`, `Scale` (configs) + `OpResult`, `Trace`, `AcTrace`,
`NoiseTrace`, `Waveform`, `ComplexWaveform`, `SolverStats` (results).

### Entry + reflection

| Object | Members |
|---|---|
| `load(path) -> Design` | Load + elaborate `.phdl`/`.ppr` |
| `Design` | `compile(module=None) -> LiveSession` · `const_(name)` · `module(name) -> Module` (raises `ValueError`) · `modules()` · `select(path) -> Selection` · `top() -> Module\|None` |
| `Module` | analyses `op(cfg=None)`, `tran(cfg)`, `ac(cfg)`, `noise(cfg)` · `compile() -> LiveSession` · `set(label, param, value)` (staged override) · reflection `name`, `ports()`, `params()`, `nets()`, `instances()`, `behaviors()` |
| `Selection` | `nodes() -> list[Node]` · `len()` · `is_empty()` |
| `Node` / `Instance` / `Net` / `Port` / `Param` / `Behavior` | plain reflected records (`name`, `kind`/`ty`/`direction`/`default`/`module`) |

### Live simulation

| Object | Members |
|---|---|
| `LiveSession` | `op(cfg=None)` · `tran(cfg)` · `ac(cfg)` · `noise(cfg)` · `set(label, param, value)` (live, no re-JIT) · `schedule_set(t, label, param, value)` (mid-transient, breakpoint-exact) · `rebuilds` (prop: auto structural rebuild count) |

### Configs (prelude bundles mirrored as classes)

| Class | Defaulted fields visible | Notes |
|---|---|---|
| `OpConfig` | *(none)* | nodeset/solver knobs live where? (see §4-R3) |
| `TranConfig` | `start=0.0`, `step=0.0` (auto) | plus `stop`, `ic` (constructor) |
| `AcConfig` | `points=100`, `scale=Scale.Dec` | plus `fstart`, `fstop` |
| `NoiseConfig` | `points=100`, `scale=Scale.Dec` | plus `out`, `ref`, `fstart`, `fstop` |
| `Solver` | `reltol=1e-3`, `abstol=1e-12`, `gmin=1e-12`, `max_iter=100`, `temperature=300.15` | attaches to configs |
| `Scale` | `Dec` / `Oct` / `Lin` | enum |

### Results

| Object | Members |
|---|---|
| `OpResult` | `v(a, b=None)` · `i(a, b=None)` · `op["instance.path"] -> InstanceView` · `stats` |
| `InstanceView` | `v(port_a, port_b=None)` · `i(port_a, port_b=None)` · `terminals()` · `label` |
| `Trace` (tran) | `v(a, b=None) -> Waveform` · `i(a, b=None) -> Waveform` · `axis()` · `stats` |
| `AcTrace` | `v(a, b=None) -> ComplexWaveform` · `axis()` |
| `NoiseTrace` | `psd() -> Waveform` · `total()` |
| `Waveform` | `values`/`axis` (numpy) · `at(x)` · `cross(level, dir="Either")` · `min/max/mean/rms/peak_to_peak` (time-weighted) · `len()`/`is_empty()` |
| `ComplexWaveform` | `values`/`axis` (numpy complex) · `mag`/`phase`/`db` (→ `Waveform`) · `at(x)` · `len()`/`is_empty()` |
| `SolverStats` | `converged` · `newton_iterations` · `homotopy_strategy`/`homotopy_levels` · `steps_accepted`/`steps_rejected` · `dt_min`/`dt_max`/`dt_min_floor_hits` · `bypass_hits`/`bypass_misses` · `assembly_time_ns`/`solve_time_ns` |

**CLI host commands:** `piperine run foo.py` · `piperine run -i [design.phdl]`
(REPL, pre-loads `design`) · `piperine test` (`*_tb.py`, `--list`, explicit
file, `PIPERINE_TEST_TIMEOUT_SECS`).

---

## 2. Rust surface (`piperine-api`; root `piperine` re-exports it)

### `session` — `SimSession`, `SolverConfig`

| Item | Signature |
|---|---|
| `SimSession::new` | `(Design, module: String) -> Self` |
| `set_device_provider` | `(Rc<dyn DeviceProvider>)` — plugin `@device` builds |
| `set_hooks` | `(Rc<dyn SimHooks>)` — lifecycle hooks |
| `design()` / `module()` | accessors |
| `stage` | `(&self, label, param, Value)` — staged override, consumed by the next analysis |
| `run_op` | `(&SolverConfig, Option<&HashMap<String, f64>> /* nodeset */) -> Result<OpResult>` |
| `run_op_sweep` | `(label, param, &[f64], &SolverConfig, nodeset) -> Result<Vec<OpResult>>` — compile-once (MD-18) |
| `run_tran` | `(stop, step: Option<f64>, start, &SolverConfig, ic) -> Result<Trace>` |
| `run_ac` | `(fstart, fstop, points, logarithmic: bool, &SolverConfig) -> Result<AcTrace>` |
| `run_noise` | `(out, reference, fstart, fstop, points, logarithmic, &SolverConfig) -> Result<NoiseTrace>` |
| `snapshot_digital` | `(&CircuitBuildInfo, &CircuitInstance) -> HashMap<String, f64>` (pub for host reuse) |
| `SolverConfig` | `{ temperature, reltol, abstol, gmin, max_iter, dc_damp_tolerance }` + `to_context()` / `to_policy()` |

### `results` — `NetRef`, `OpResult`

| Item | Signature |
|---|---|
| `NetRef` | `{ name: String }` — the `.v`/`.i` argument type |
| `OpResult::v` | `(&NetRef, Option<&NetRef>) -> Result<f64>` (digital nets: 0/1/NaN) |
| `OpResult::i` | `(&NetRef, Option<&NetRef>) -> Result<f64>` (unique two-terminal match; sources read the MNA branch) |
| `OpResult::stats` | `-> &SolverStats` |

### `waveform` — traces + `Waveform<T>`

| Item | Members |
|---|---|
| `Waveform<T = f64>` | `new(points)` · `points() -> &[(f64, T)]` · `len`/`is_empty` · (real) `at(x)` interp · `min/max/mean/rms/peak_to_peak` (dt-weighted) · `cross(level, dir: &str)` |
| `ComplexWaveform` | `mag()`/`phase()`/`db() -> Waveform` · `at(x) -> Complex64` (nearest) |
| `Trace` | `v`/`i` `(&NetRef, Option<&NetRef>) -> Result<Waveform>` · `axis()` · `stats()` (stateful-device `i` fails loud — SC-25 opt-in recording pending) |
| `AcTrace` | `v(...) -> Result<ComplexWaveform>` · `axis()` |
| `NoiseTrace` | `psd() -> Waveform` · `total() -> f64` |

### `hooks`, `error`, `prelude`

| Item | Contents |
|---|---|
| `trait SimHooks` | `transform_design(&Design)` · `before_lower(&Design)` · `after_solve(analysis: &str, node_voltages: &[(String, f64)])` — all `Result<(), String>` |
| `enum Error` | `Elaboration(ElabError)` · `Lowering(LowerErrors)` · `Codegen(CodegenError)` · `Solver(solver::Error)` · `Measurement(String)` · `Plugin(String)` |
| `prelude` | the api types + `CircuitBuildInfo`/`CircuitCompiler`/`DeviceProvider` (codegen) + `Design`/`SourceMap`/`parse_and_elaborate[_seeded]` (lang) + `piperine_solver::prelude::*` |

### Solver-level extras a Rust host can reach (via prelude)

`CircuitInstance` (`dc`/`ac`/`tran`/`noise`/`sens` drivers, `set_element_param`,
`nets()`, `netlist()`), `TransientSolver::{schedule_set, with_initial_state}`,
`SensAnalysisOptions`/`SensResult` (new — no `SimSession`/Python surface yet),
`ConvergencePlan`, `Net`, `LogicValue`, analysis options/results.

---

## 3. Known asymmetries (implementation state, not design)

- `.sens` exists solver-level only (T4 pends: `SimSession::run_sens` +
  `module.sens(...)`).
- PSS not yet implemented (T5/T6).
- `Trace.i` on stateful devices fails loud on both hosts (SC-25).

---

## 4. Surface review sheet

> **Resolved 2026-07-18 (MD-22 — uniform host surface):** the user accepted
> the sheet and locked the governing principle: *the API is identical in
> both languages* — same call shape, same names, same config/result types.
> Every point below resolves under that rule; the Rust-side alignment is
> the `uniform-host-api` feature (ROADMAP P3). New analyses land on both
> hosts with the same shape in the same feature.

Candid list of places where the surface could be better. R = Rust-side,
P = Python-side, B = both.

1. **B — Live/staged duality.** Python has the clean split
   (`Module.op` staged vs `LiveSession` compiled-once); Rust has
   `SimSession` (staged) + only `run_op_sweep` for compile-once. A Rust
   `LiveSession` equivalent (own the compiled circuit, `set`/`schedule_set`/
   analyses) would mirror the Python story and is what the optimizer loop
   will want natively.
2. **P — Config constructors are opaque.** `inspect.signature` shows no
   `__init__` fields; required fields (e.g. `TranConfig.stop`,
   `AcConfig.fstart/fstop`) are invisible to autocomplete until you read
   docs. Typed `__init__` signatures (or dataclass-style stubs) would fix
   IDE ergonomics.
3. **B — Solver knobs asymmetry.** Rust `SolverConfig` carries
   `dc_damp_tolerance`; Python `Solver` doesn't. Python `OpConfig` has no
   visible nodeset; Rust `run_op(nodeset)` does. Decide the canonical knob
   set and mirror it.
4. **P — naming: `Solver` (config class) vs Rust `SolverConfig`.** Same
   thing, two names. Also `const_` (trailing underscore) is a keyword-dodge
   that reads poorly — `constant(name)`?
5. **R — string-typed `cross(dir: &str)`.** Python mirrors it
   (`"Rising"/"Falling"/"Either"`). An enum on both sides is cheap and
   IDE-friendly.
6. **B — no inline-source load.** `load(path)` only; a `load_str(source)`
   (Rust: exists as `parse_and_elaborate`) would help REPL/docs/tests on the
   Python side.
7. **P — `Waveform` lacks `resample(grid)` and `fft()`** (ROADMAP P3, agreed
   host-side items) and a `piperine.plot()` convenience.
8. **B — result indexing asymmetry.** `op["x1"] -> InstanceView` exists in
   Python only; Rust `OpResult` has no instance-view accessor.
9. **R — `NetRef { name }` is a bare public struct.** `op.v(&NetRef { name:
   "mid".into() }, None)` is verbose; `impl From<&str> for NetRef` + generic
   `v(impl Into<NetRef>)` would read like the Python side.
10. **B — error taxonomy.** Python raises `ValueError`/`KeyError` with
    stringified messages; Rust has the typed `Error` enum. Fine for now —
    but if plugins/scripts want to *catch* specific failures, a
    `piperine.SimulationError` hierarchy is the upgrade.
11. **P — `LiveSession.rebuilds` is a property while `len()`/`is_empty()`
    are methods** — small inconsistency in property-vs-method conventions
    across the facade (`Waveform.values` prop, `Waveform.len()` method,
    `__len__` unimplemented).
12. **B — upcoming surfaces to shape now:** `sens` (map keyed how? Python
    dict `{(out, "label.param"): float}` vs nested dicts) and `pss`
    (returns a `Trace` restricted to one period + `PssStats`?). Decide
    before T4/T6 land them.
