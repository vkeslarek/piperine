# Appendix C — Host API surface reference (Python + Rust)

Complete inventory of the two host surfaces as of 2026-07-24 (post
`host-library`, T1–T28). The Python side was extracted mechanically
(`__all__` in `crates/piperine-python/python/piperine/__init__.py` +
`_piperine.pyi`); the Rust side is the `piperine-api` public surface
(`crates/piperine-api/src/{session,results,waveform,units,error}.rs`, all
re-exported through `piperine_api::prelude`/`piperine::prelude`). Part VIII
is the conceptual guide; this appendix is the flat reference. The parity
oracle is executable: `tests/host_parity.rs`'s `ANALYSES` constant is
checked against both surfaces by `cargo test -p piperine host_parity`.

---

## 1. Python surface (`import piperine`)

**Module exports (`__all__`, ~55 names):** `load`, `load_str` (fns) +
`Design`, `Module`, `Port`, `Net`, `Instance`, `Param`, `Behavior`,
`Selection`, `Node` (reflection) + `InstanceView`, `Terminal`,
`ModelDescriptor`, `TerminalDescriptor`, `ObservableDescriptor`,
`ParamDescriptor`, `SolverStats`, `LimitingReport`, `NoiseContribution`
(introspection) + `Session`, `Sweep`, `SweepPoint`, `Grid` (compiled
session) + `OpResult`, `Trace`, `AcTrace`, `NoiseTrace`, `PssResult`,
`PssStats`, `SensResult`, `PoleZeroResult`, `SpResult`, `TfResult`,
`Waveform`, `ComplexWaveform`, `FourierComponent`, `FourierResult`
(results) + `Scale`, `CrossDirection`, `Direction`, `Solver`, `OpConfig`,
`TranConfig`, `AcConfig`, `NoiseConfig` (configs/enums) + `plot`, `bode`,
`extract`, `Hz`, `ns`, `mV`, `C` (host helpers) + `SimulationError`,
`ElaborationError`, `UnknownModule`, `UnknownNet`, `ConvergenceError`
(errors).

### Entry + reflection

| Object | Members |
|---|---|
| `load(path) -> Design` | Load + elaborate `.phdl`/`.ppr` (raises `ElaborationError`) |
| `load_str(src) -> Design` | Elaborate inline source, no filesystem read |
| `Design` | `compile(module=None) -> Session` · `const(name)` · `module(name) -> Module` (raises `UnknownModule`) · `__getitem__(name) -> Module` · `modules()` · `select(path) -> Selection` · `top -> Module \| None` (property) |
| `Module` | analyses `op(cfg=None)`, `sens(...)`, `pss(...)`, `pz(...)`, `disto(...)`, `sp(...)`, `tran(cfg)`, `ac(cfg)`, `noise(cfg)` · `compile() -> Session` · `set(label, param, value)` (staged override) · reflection properties `name`, `ports`, `params`, `nets`, `instances`, `behaviors` |
| `Selection` | `nodes() -> list[Node]` · `len()` · `is_empty()` |
| `Node` / `Instance` / `Net` / `Port` / `Param` / `Behavior` | plain reflected records (`name`, `kind`/`ty`/`direction`/`default`/`module`) |

### Compiled session

| Object | Members |
|---|---|
| `Session` | `op(cfg=None)` · `tran(cfg)` · `ac(cfg)` · `noise(cfg)` · `sens(...)` · `pss(...)` · `pz(...)` · `disto(...)` · `sp(...)` · `tf(...)` · `dc(label, param, values, nodeset=None, solver=None)` · `set(label, param, value)` (restamp, no re-JIT) · `schedule_set(t, label, param, value)` (mid-transient, breakpoint-exact) · `sweep(label, param, values) -> Sweep` · `sweep_grid({"label.param": values, ...}) -> Grid` · `rebuilds` (prop: auto structural rebuild count) |
| `Sweep` | iterable of `SweepPoint` (single-knob, HOST-18) |
| `SweepPoint` | a `Session` view at one sweep value — every `Session` method available |
| `Grid` | iterable of `SweepPoint` over the cartesian product of named axes (HOST-19) · `map(fn) -> numpy.ndarray` shaped like the axes · `shape()` |

### Configs (dataclasses, mirror prelude bundles, HOST-20)

| Class | Fields | Notes |
|---|---|---|
| `Solver` | `temperature=300.15`, `reltol=1e-3`, `abstol=1e-12`, `gmin=1e-12`, `max_iter=100`, `dc_damp_tolerance=0.5` | canonical knob set, identical on both hosts |
| `OpConfig` | `solver`, `nodeset={}` | |
| `TranConfig` | `stop`, `step=0.0` (auto), `start=0.0`, `ic={}`, `solver`, `record_device_state=False` | |
| `AcConfig` | `fstart`, `fstop`, `points=100`, `scale=Scale.Dec`, `solver` | |
| `NoiseConfig` | `out`, `fstart`, `fstop`, `points=100`, `scale=Scale.Dec`, `solver` | |
| `Scale` | `Lin` / `Dec` / `Oct` | enum |
| `CrossDirection` | `Rising` / `Falling` / `Either` | enum (HOST-23) |
| `Direction` | `In` / `Out` / `Inout` | enum wrapping `Port`/`Terminal.direction`'s `str` (HOST-23) |

Every config class carries `.with_(**overrides) -> Self` (immutable copy,
`dataclasses.replace`) via a shared `_ConfigMixin`; every field is visible
to `inspect.signature(TranConfig)` (plain dataclass `__init__`, HOST-20).

### Results

| Object | Members |
|---|---|
| `OpResult` | `v(a, b=None)` · `i(a, b=None)` · `op["instance.path"] -> InstanceView` · `stats -> SolverStats` |
| `InstanceView` | `label` (prop) · `v(port_a, port_b=None)` · `i(port_a, port_b=None)` · `terminal_connections()` · `opvar(name) -> float` (HOST-07) · `opvars() -> list[(str, float)]` · `model -> ModelDescriptor` (prop) · `terminals -> list[TerminalDescriptor]` (prop) · `observables() -> list[ObservableDescriptor]` (HOST-09) · `params() -> list[ParamDescriptor]` · `param(name) -> ParamDescriptor` (HOST-12) |
| `ModelDescriptor` | `type_id`, `version` |
| `TerminalDescriptor` | `name`, `kind` (`"external"`/`"internal"`/`"auxiliary"`), `domain`, `direction` |
| `ObservableDescriptor` | `name`, `kind`, `cost` |
| `ParamDescriptor` | `name`, `bounds` (`(lo, hi)` tuple, either side `None`), `unit`, `scope`, `invalidation` |
| `LimitingReport` | `device`, `net`, `proposed`, `limited_value`, `limiter_name`, `reason` (HOST-10) |
| `NoiseContribution` | `element`, `source`, `kind`, `integrated_sq` (HOST-11) |
| `Trace` (tran/dc) | `v(a, b=None) -> Waveform` · `i(a, b=None) -> Waveform` · `axis()` · `stats` · `opvar(path) -> Waveform` (HOST-08) · `four(f0, harmonics) -> FourierResult` |
| `AcTrace` | `v(a, b=None) -> ComplexWaveform` · `axis()` — distinct native pyclass wrapping `Trace<ComplexWaveform>`, same method shape (SPEC_DEVIATION, T7: PyO3 pyclasses cannot be generic) |
| `NoiseTrace` | `psd() -> Waveform` · `total() -> float` · `by_source() -> dict[str, Waveform]` (HOST-11) · `contributions() -> list[NoiseContribution]` (HOST-11) — distinct native pyclass wrapping `Trace<NoiseSample>`, same SPEC_DEVIATION reason |
| `Waveform` | `values`/`axis` (numpy) · `at(x)` · `cross(level, dir=CrossDirection.Either)` (HOST-23 enum, legacy `str` still accepted) · `min()/max()/mean()/rms()/peak_to_peak()` (time-weighted) · `plot(**kwargs)` (HOST-17, matplotlib-guarded) · `len()`/`__len__` (HOST-24) |
| `ComplexWaveform` | `values`/`axis` (numpy complex) · `mag`/`phase`/`db` (properties → `Waveform`) · `at(x)` · `plot(**kwargs)` (Bode, HOST-17) · `len()`/`__len__` |
| `FourierComponent` / `FourierResult` | harmonic decomposition of a `Trace` (`.four(f0, harmonics)`) |
| `PssResult` | `.trace -> Trace` (one period) · `.stats -> PssStats` |
| `PssStats` | `shoot_iterations`, `residual`, `estimated_settle_time` |
| `SensResult` | `get(output, label, param) -> float \| None` · `items()` |
| `PoleZeroResult` | `poles: list[complex]`, `zeros: list[complex]` (`.pz`) |
| `SpResult` | `frequencies`, `s` (matrix), `z0`, `n_ports` (`.sp`) |
| `DistoResult` | `hd2`, `hd3`, `im2`, `im3` (`.disto`) |
| `TfResult` | `gain`, `z_in`, `z_out` (`.tf`) |
| `SolverStats` | `converged` · `newton_iterations` · `homotopy_strategy`/`homotopy_levels` · `steps_accepted`/`steps_rejected` · `dt_min`/`dt_max`/`dt_min_floor_hits` · `bypass_hits`/`bypass_misses` · `assembly_time_ns`/`solve_time_ns` · `limiting: list[LimitingReport]` (HOST-10) |

### Errors (HOST-22)

`SimulationError` (base, catch-all) → `ElaborationError(SimulationError,
ValueError)`, `UnknownModule(SimulationError, ValueError)`,
`UnknownNet(SimulationError, KeyError)`, `ConvergenceError(SimulationError,
RuntimeError)` (`.node`/`.iteration`/`.analysis`). Every subclass also
inherits the matching builtin type it previously surfaced as, so existing
`except KeyError`/`except ValueError` sites keep working unchanged.

### Host helpers

| Fn | Signature |
|---|---|
| `extract(source, {name: fn}) -> dict` | HOST-25 — applies every measurement fn to `source`, collects a dict |
| `plot(waveform_or_dict, **kwargs) -> Figure` | HOST-17, matplotlib-guarded, `ImportError` with install hint if absent |
| `bode(complex_waveform, **kwargs) -> Figure` | HOST-17, mag+phase pair |
| `Hz(value: float\|str) -> float` | HOST-21, SI prefix only on `str` |
| `ns(value: float\|str) -> float` | HOST-21 |
| `mV(value: float\|str) -> float` | HOST-21 |
| `C(value: float) -> float` | HOST-21, Celsius → Kelvin |

**CLI host commands:** `piperine run foo.py` · `piperine run -i
[design.phdl]` (REPL, pre-loads `design`) · `piperine test` (`*_tb.py`,
`--list`, explicit file, `PIPERINE_TEST_TIMEOUT_SECS`) · `piperine check`/
`piperine build`.

---

## 2. Rust surface (`piperine-api`; root `piperine` re-exports it)

### `session` — `SimSession`, `Session`, `SolverConfig`, `Scale`, sweeps

| Item | Signature |
|---|---|
| `SimSession::new` | `(Design, module: String) -> Self` |
| `set_device_provider` | `(Rc<dyn DeviceProvider>)` — plugin `@device` builds |
| `set_hooks` | `(Rc<dyn SimHooks>)` — lifecycle hooks |
| `design()` / `module()` | accessors |
| `stage` | `(&self, label, param, Value)` — staged override, consumed by the next analysis |
| `run_op` / `run_op_sweep` / `run_tran` / `run_ac` / `run_noise` / `run_sens` / `run_pss` / `run_pz` / `run_sp` / `run_disto` | one method per analysis, positional args + `&SolverConfig` — see Part VIII §3 for full signatures |
| `snapshot_digital` / `snapshot_opvars` / `snapshot_introspect` | pub for host reuse (same snapshot the Python live session builds) |
| `Session::compile` | `(&Design, module: &str) -> Result<Self, Error>` — **compiles once** (HOST-01) |
| `Session::{module, rebuilds}` | accessors |
| `Session::set` | `(&mut self, label, param, value: f64) -> Result<(), Error>` — restamp; fails loud on a structural write (SPEC_DEVIATION, see Part VIII §4) |
| `Session::schedule_set` | `(&mut self, t, label, param, value: f64)` |
| `Session::{op, tran, ac, noise, sens, pss, pz, disto, sp, tf, dc}` | one method per analysis on the held circuit (HOST-02/03/05) |
| `Session::sweep` | `(&mut self, label, param, values: &[f64]) -> Sweep` — lending iterator (HOST-18) |
| `Session::sweep_grid` | `(&mut self, axes: &[(&str, &str, &[f64])]) -> Grid` — named grid (HOST-19) |
| `Sweep` | `next(&mut self) -> Option<Result<SweepPoint, Error>>` (not `std::iter::Iterator` — each item mutably borrows the sweep's own `Session`) · `len()`/`is_empty()` |
| `SweepPoint` | `Deref`/`DerefMut` to `Session` |
| `Grid` | `shape()` · `len()`/`is_empty()` · `map<R>(fn) -> Nested<R>` |
| `Nested<R>` | `Branch(Vec<Nested<R>>)` / `Leaf(R)` tree shaped like `Grid::shape()` |
| `Scale` | `Lin`/`Dec`/`Oct` enum; `impl From<Scale> for bool` (`is_logarithmic`) |
| `SolverConfig` | `{ temperature, reltol, abstol, gmin, max_iter, dc_damp_tolerance }` + `to_context()` / `to_policy()` |

### `results` — `NetRef`, `NetSelector`, `OpResult`, `InstanceView`, structured results

| Item | Signature |
|---|---|
| `NetRef` | `{ name: String }`; `impl From<&str>`/`From<String>`/`From<&String>`/`From<&NetRef>` (HOST-23) |
| `NetSelector` | trait implemented for `&str`/`String`/`&String`/`NetRef`/`&NetRef` — the `.v`/`.i` argument bound |
| `OpResult::v` / `::i` | `(impl NetSelector) -> Result<f64, Error>` (digital nets: 0/1/NaN) |
| `OpResult::instance` | `(&self, label: &str) -> Result<InstanceView<'_>, Error>` (HOST-07) |
| `OpResult::stats` | `-> &SolverStats` |
| `InstanceView` | `label()` · `model() -> &ModelDescriptor` · `terminals() -> &[TerminalDescriptor]` · `observables() -> &[ObservableDescriptor]` (HOST-09) · `params() -> &[ParamDescriptor]` · `param(name) -> Result<&ParamDescriptor, Error>` (HOST-12) · `opvar(name) -> Result<f64, Error>` (HOST-07) · `opvars() -> Vec<(String, f64)>` |
| `TfResult` | `{ gain, z_in, z_out }` — `from_solver(...)` |
| `PzResult` | `{ poles, zeros }` — `From<PoleZeroResult>` |
| `DistoResult` | `{ hd2, hd3, im2, im3 }` — `From<solver DistoResult>` |
| `SParamResult` | `{ ... }` + `s(k, i, j) -> Complex64` — `From<solver SpResult>` |
| `SensResult` | `{ d: HashMap<(String, String), f64> }` + `get(output, label, param) -> Option<f64>` |
| `PssResult` | `{ trace: Trace<Waveform>, stats }` |

### `waveform` — `Waveform<T>`, `Trace<T>`, `ComplexWaveform`

| Item | Members |
|---|---|
| `Waveform<T = f64>` | `new(points)` · `points() -> &[(f64, T)]` · `len`/`is_empty` · (real) `at(x)` interp · `min`/`max`/`mean`/`rms`/`peak_to_peak` (dt-weighted) · `cross(level, CrossDirection)` |
| `CrossDirection` | `Rising`/`Falling`/`Either`; `impl From<&str>` for legacy callers (HOST-23) |
| `Waveform` (measurements, HOST-14) | `slew_rate()` · `rise_time()` · `fall_time()` · `overshoot()` · `settling_time(tol)` · `delay(other, level)` — each `Result<f64, Error>` |
| `Waveform` (transforms, HOST-15) | `resample(grid) -> Waveform` · `derivative() -> Result<Waveform, Error>` · `integral() -> Waveform` · `clip(lo, hi) -> Waveform` · `fft() -> Result<ComplexWaveform, Error>` |
| `ComplexWaveform` | `type ComplexWaveform = Waveform<num_complex::Complex64>`; `mag()`/`phase()`/`db() -> Waveform` · `at(x) -> Complex64` |
| `ComplexWaveform` (margins, HOST-16) | `bandwidth_3db()` · `unity_gain_freq()` · `phase_margin()` · `gain_margin()` — each `Result<f64, Error>` |
| `NoiseSample` | zero-sized discriminator type — `Trace<NoiseSample>` has no per-net `v`/`i`, only noise methods |
| `Trace<T>` | generic container (HOST-13): `v`/`i(impl NetSelector) -> Result<T, Error>` · `axis() -> Waveform` · `stats() -> &SolverStats` · `opvar(path) -> Result<Waveform, Error>` (HOST-08, `Trace<Waveform>` only) |
| `Trace<Waveform>` | `new(TransientAnalysisResult, info)` · `from_dc_sweep(...)` (HOST-05) |
| `Trace<ComplexWaveform>` = `AcTrace` | `new(AcAnalysisResult, info)` — type alias, not a separate type (HOST-13) |
| `Trace<NoiseSample>` = `NoiseTrace` | `new(NoiseAnalysisResult)` · `psd()` · `total()` · `by_source() -> HashMap<String, Waveform>` (HOST-11) · `contributions() -> &[NoiseContribution]` (HOST-11) — type alias, not a separate type |

### `units` — `Freq`, `Time` (HOST-21)

| Item | Signature |
|---|---|
| `Freq` | `pub struct Freq(pub f64)`; `From<f64>` (base unit), `From<&str>` (SI prefix + optional `Hz` suffix, panics on garbage) |
| `Time` | same shape, base unit seconds, optional `s` suffix |

### `error` — `Error` (mirrors the Python `SimulationError` taxonomy)

| Variant | Source |
|---|---|
| `Elaboration(ElabError)` | staging/elaboration |
| `Lowering(LowerErrors)` | POM → resolved-form lowering |
| `Codegen(CodegenError)` | circuit build |
| `Solver(solver::Error)` | analysis solve failure (typed `SolverDomain` inside) |
| `Measurement(String)` | unaddressable net/opvar/param, structural-write-on-`Session::set` |
| `Plugin(String)` | plugin/hook failure |

### `hooks`, `prelude`

| Item | Contents |
|---|---|
| `trait SimHooks` | `transform_design(&Design)` · `before_lower(&Design)` · `after_solve(analysis: &str, node_voltages: &[(String, f64)])` — all `Result<(), String>` |
| `prelude` | `Error`; `FourierComponent`/`FourierResult`; `SimHooks`; `DistoResult`/`NetRef`/`NetSelector`/`OpResult`/`PssResult`/`PzResult`/`SParamResult`/`SensResult`/`TfResult`; `Grid`/`Nested`/`Scale`/`Session`/`SimSession`/`SolverConfig`/`Sweep`/`SweepPoint`; `Freq`/`Time`; `AcTrace`/`ComplexWaveform`/`CrossDirection`/`NoiseTrace`/`Trace`/`Waveform`; plus `piperine-codegen`'s `CircuitBuildInfo`/`CircuitCompiler`/`DeviceProvider`, `piperine-lang`'s `Design`/`SourceMap`/`parse_and_elaborate[_seeded]`, and `piperine_solver::prelude::*` in full (introspection types `Bounds`/`Invalidation`/`ModelDescriptor`/`ObservableDescriptor`/`ObservableKind`/`ParamDescriptor`/`ParamScope`/`TerminalDescriptor`/`TerminalKind`, `NoiseContribution`, `CircuitInstance`, `Net`, `LogicValue`, analysis options/results) |

---

## 3. Cross-host parity — the executable proof

`tests/host_parity.rs` is the parity oracle (HOST-06):

- `ANALYSES = ["op", "tran", "ac", "noise", "sens", "pss", "pz", "disto",
  "sp", "tf", "dc"]` — the canonical uniform analysis-method list.
- **Rust side**: `call_every_rust_analysis` calls every name in `ANALYSES`
  on a live `Session` — a **compile-time** proof; removing a method fails
  the build, not just a test.
- **Python side**: an embedded script (`piperine_python::embed::run_script`)
  builds a `Session` on the same RLC fixture and asserts `hasattr(session,
  name)` for every name in `ANALYSES` — a **runtime** proof.
- `host_parity_probe_flags_a_synthetic_missing_analysis` proves the probe
  actually discriminates (a bogus name is correctly reported missing, not
  vacuously "all present").

`cargo test -p piperine host_parity` runs both checks; `crates/
piperine-python/tests/pyi_stub.rs` separately cross-checks every declared
`.pyi` stub member against the real runtime `_piperine` module.

**Intentional, tracked gap** (not a parity-test regression — `ANALYSES`
covers analyses, not every `Waveform` method): `Waveform` measurements
(HOST-14), transforms (HOST-15), and `ComplexWaveform` margins (HOST-16)
are Rust-only (`piperine-api`); no native Python binding exists yet. See
Part VIII §9.

---

## 4. Superseded — resolved asymmetries

The previous edition of this appendix (2026-07-18) carried a "§3 Known
asymmetries" and "§4 Surface review sheet" listing twelve open design
questions and three known implementation gaps. `host-library` (T1–T28)
closed all of them:

| Old item | Resolution |
|---|---|
| §3-1 `.sens` solver-level only | `Session::sens`/`Module.sens` — T4 (Phase 1) |
| §3-2 PSS not implemented | `Session::pss`/`Module.pss` — T4 (Phase 1) |
| §3-3 `Trace.i` fails loud on stateful devices | Unchanged by design — `record_device_state` opt-in documented, Part VIII §3 |
| §4-1 no Rust compiled session | `Session::compile` — T3 (HOST-01) |
| §4-2 opaque config constructors | Typed dataclasses + `.with_()` — T22 (HOST-20) |
| §4-3 `SolverConfig`/`Solver` knob asymmetry | Canonical knob set incl. `dc_damp_tolerance`/`nodeset` — T22 (HOST-20) |
| §4-4 `Solver` vs `SolverConfig` naming / `const_` | `Solver`/`SolverConfig` two names is accepted (Rust-idiomatic `Config` suffix); `const_`→`const` — T26 (HOST-24) |
| §4-5 string-typed `cross(dir: &str)` | `CrossDirection` enum both hosts — T25 (HOST-23) |
| §4-6 no inline-source load | `pip.load_str` — T26 (HOST-24) |
| §4-7 `Waveform` lacks `resample`/`fft`/`plot` | T16–T19 (HOST-14..17); Python binding tracked as the one remaining gap, §3 above |
| §4-8 `op["x1"]` Python-only | `OpResult::instance(label)` (Rust) — T9/T10 (HOST-07) |
| §4-9 bare `NetRef { name }` | `impl From<&str>`/tuples via `NetSelector` — T25 (HOST-23) |
| §4-10 no typed error hierarchy | `SimulationError` hierarchy (Python) + `Error` enum (Rust) — T24 (HOST-22) |
| §4-11 property/method inconsistency | `design.top`/`amp.ports`/etc. as properties, `__len__` — T26 (HOST-24) |
| §4-12 `sens`/`pss` shape undecided | Locked: `sens` keyed `(output, "label.param")`; `pss` returns `Trace` + `PssStats` — T2/T4 |

No open asymmetries remain in scope for this feature; the one documented
gap (Rust-only `Waveform` measurements/transforms/margins) is tracked in
Part VIII §9, not re-litigated here.
