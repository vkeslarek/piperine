# Project State — Piperine

## Macro Decisions (locked)

These are the architectural decisions that shape the solver. They are binding
and won't be relitigated per-PR. Feature specs reference them by ID.

### MD-01: One Element ABI, no downcast
The solver talks to every participant through one `Element` trait with
`ElementCapabilities` bitflags. No `Device` wrapper, no analog/digital facet
split, no downcast. A resistor, a gate, a comparator, and a plugin are the
same type to the solver.

**Status:** Done.

### MD-02: Net is the unified naming layer
`Net` (kind + dense index + label + optional `Arc<AnalogVariable>`) replaces
both `AnalogReference` and `DigitalNet` at the public boundary. Fast-path
aliases remain for hot loops. Result types answer `get_net(&Net)`.

**Status:** Done.

### MD-03: Per-analysis context, shared Context
`Context` carries only what every analysis shares (tolerances, integration
method, temperature, verbosity). Each analysis receives its own
`AnalysisContext` (`DcContext`, `AcContext`, `TransientContext`, etc.) with
analysis-specific tunables (`dt_min`, `dt_max`, `adaptive`, sweep config, …).

**Status:** Locked. Implementation pending.

### MD-04: Tolerances vs Policy
`Context` holds immutable `Tolerances`. Mutable `Policy` (homotopy scales,
step bounds, retry counters) is owned by the active `ConvergencePlan` and
its strategies — never by the shared `Context`.

**Status:** Done (solver-convergence-performance T11). `Context` is
tolerances-only; `Policy` lives on each analysis solver; time is explicit.

### MD-05: Strategy composition
The analysis state machine (setup→op→resume→accepted→rejected→restart) is
composed of three strategy traits: `NewtonStrategy`, `HomotopyStrategy`,
`StepperStrategy`. Each analysis picks the strategies it needs. No inline
if-else cascades in drivers.

**Status:** Locked. `HomotopyStrategy` done; Newton/Stepper pending.

### MD-06: init_global as Once
`tracing`/`faer` need one-time process init. `Context::default` does not
trigger it; `Solver::build()` does.

**Status:** Locked. Implementation pending.

### MD-07: Integration helpers in math/
`TrBdf2`/`TrBdf2Phase`, companion coefficients (`phase_coeffs`/`stage_coeffs`),
the Milne LTE estimate and `Integrator` (quadrature) all live in
`math/integration.rs`. The kernel calls the centralised formula — no
per-method branching in codegen. The vestigial `IntegrationMethod` enum, the
dead `TruncationError` trait and `Tolerances.integration` were removed
2026-07-18 (p1-solver-complete T22): TR-BDF2 is the sole scheme, there is no
method-selection surface.

**Status:** Done (amended 2026-07-18).

### MD-08: LTE drives timestep
After each accepted step, the stepper consults elements for LTE-based dt
suggestions. Takes the min, clamps to `[dt_min, dt_max]`. Non-reactive
circuits fall back to 2× growth. No allocation on hot path.

**Status:** Done.

### MD-09: SolverDomain enum
Error domain is a typed enum, not a free string. Typos are compile errors.

**Status:** Done.

### MD-10: Scheduler returns Result
Digital scheduler returns `Result<(), Error>` instead of `log::warn!`.
Caps live in `PlanLimits`.

**Status:** Done.

### MD-11: OSDI as checklist, not ABI
OSDI is a maturity checklist. Piperine's ABI is mixed-signal-first and
domain-uniform. OSDI wrappers are one client.

**Status:** Locked.

### MD-12: ABI vs solver policy classification
Element "is" or "knows" → ABI. Solver "decides" → solver policy. Per-device
convergence tests stay in ABI (element volunteers); solver gates the outer
loop on global convergence.

**Status:** Locked.

### MD-13: Rust idiom rules (binding)

These five rules govern every line of solver and codegen code. A PR that
violates any of them is not ready. They are also in `AGENTS.md` under
"Hard rules → Rust idiom rules".

1. **Contracts and capabilities first.** Think in traits, capability
   descriptors, and type-level contracts before algorithms and
   implementation. The code should read as a specification of *what* the
   solver does, not *how* it does it internally.

2. **No loose functions.** Every function has an owner — a trait method or a
   struct method. `pub(crate) fn` or `pub fn` at module level is a defect.
   If a helper doesn't belong to a trait or struct, it means the abstraction
   is missing.

3. **Clean and simple.** Bat the eye and understand what the code is doing.
   If a reader needs to trace three files to understand a single operation,
   the code is too clever. Prefer explicit over implicit, flat over nested,
   early-return over deep match.

4. **Modules organized by system function.** Files are named after what they
   do in the system (`solver.rs`, `integration.rs`, `circuit.rs`), not after
   language constructs (`traits.rs`, `models.rs`, `utils.rs`). The golden
   rule: glance at the file tree and know where every struct and trait
   belongs.

5. **No macros.** No `macro_rules!`, no `paste!`, no proc-macro codegen.
   Data tables + plain helpers. If a pattern repeats, extract a trait or a
   struct method — never a macro.

**Status:** Locked. Enforced in AGENTS.md.

### MD-14: TF voltage-source-only
TF keeps explicit error for current-source input. Documented limit, not a
gap.

**Status:** Done.

### MD-15: No piperine-math crate
The math dispatch table was absorbed into `piperine-lang` / `piperine-codegen`
directly. There is no standalone `piperine-math` crate in the workspace.

**Status:** Done.

### MD-16: Crate-level docs removed
Per-crate documentation (`crates/*/docs/`) was removed. The formal spec lives
in `docs/spec/` (Parts I–VII). Solver gaps and feature tracking live in
`SOLVER_GAPS.md` and `.specs/`.

**Status:** Done.

### MD-17: Two-tier public surface — `prelude` + `abi`

Delivered by `solver-abi` feature. Public surface of `piperine-solver` is
exactly two modules: `prelude` (host audience: bench, python, CLI) and `abi`
(device-author audience: codegen, plugins, test doubles). Every other module
is `pub(crate)`. Re-exports in `prelude`/`abi` elevate item visibility without
reopening internal module paths.

- `prelude`: `CircuitBuilder`, `CircuitInstance`, `Solver`, `Context`, `Policy`,
  `Tolerances`, analysis results + options, `Net`, `Error`, `SolverStats`, …
- `abi`: `Element`, `ElementCapabilities`, `UnknownAllocator`, `Stamp`,
  `AnalogReference`, `Netlist`, `Noise`, `NoiseKind`, digital interface, …
- `lib.rs`: `pub mod abi; pub mod prelude;`, all others `pub(crate)`.

**Status:** Done (2026-07-16).

### MD-18: Elaboration fixes devices; simulation never re-JITs

Elaboration/JIT happens once per design+staging; after that, a simulation —
including parameter sweeps — runs entirely on the solver. Re-running
elaborate/compile inside a simulation loop (e.g. per sweep point) is an
architecture defect, not a perf tweak. Swept parameters go through a
solver-level restamp/staging path on the already-compiled circuit.

**Status:** Locked (user, 2026-07-16). Implementation: spice-stdlib T12.

### MD-19: Root crate is the library face (lib-only; bin in cli)

The root `piperine` crate is the complete external Rust view of the project:
`src/lib.rs` hosts the session/results/waveform plumbing plus a `prelude`
re-exporting the lang/codegen/solver public faces. The root is **lib-only** —
the `piperine` binary target lives in `piperine-cli` (`[[bin]] name =
"piperine"`) because root(bin)→cli→python→root(lib) would close a cargo
package cycle. Dependency flow: `root(lib) → {lang, codegen, solver}`;
`python → root(lib)`; `cli → {python, root(lib), project}` + bin.
`cargo install` targets `crates/piperine-cli`.

**Status:** Locked (user, 2026-07-17 — bench-removal topology option B).
Implementation: bench-removal T1. **Superseded by MD-20 (2026-07-18).**

### MD-20: `piperine-api` is the library face; root is a thin re-export shell

A dedicated `crates/piperine-api`, pure Rust: the host API
(session/results/waveform/hooks/error/prelude). `piperine-python` is a thin
binding layer over it. The root `piperine` package becomes a **thin
re-export shell** (`pub use piperine_api::*`) so Rust hosts keep
`use piperine::…` — no code of its own in root `src/`. The `piperine` binary
stays in `piperine-cli` (amended 2026-07-18: user chose re-export shell over
root-absorbs-CLI). Supersedes MD-19. Dependency flow:
`api → {lang, codegen, solver}`; `python → api`; `root(shell) → api`;
`cli → {python, api, project}` + bin — no cycle. Device/plugin ABI-contract
consolidation into the api crate is deferred to the P2/P5 features.

**Status:** Locked (user, 2026-07-18; shell amendment same day).
Implementation: feature `api-crate`.

### MD-21: Plugin backends are native + Python only

The WASM (wasmtime) and process JSON-RPC plugin tiers are removed
(`piperine-plugin-wasm` deleted with them). Native dlopen stays — trusted,
fast, and the same mechanism as the low-level `libloading` device path (V1
P2). Python plugins run through the existing embedded-host isolation (same
surface as benches); the lifecycle registry must be exposed to Python so a
plugin self-registers (attribute schemas, hooks, scripts, devices)
transparently on load.

**Status:** Locked (user, 2026-07-18). Implementation: pending (ROADMAP P5).

### MD-22: Uniform host surface — Python and Rust are one API

The two host surfaces are the same API in two languages: identical call
shape, identical names, identical config/result types. Part VIII's "two
surfaces, one surface" is **normative**, not aspirational. Concretely:
Rust gains the object model Python already has (`load` → `Design` →
`Module` → analyses / `compile()` → `LiveSession`, `InstanceView`
indexing, bundle-shaped configs); Python gains every Rust-only knob
(nodeset, `dc_damp_tolerance`, …); naming divergences (`Solver` vs
`SolverConfig`, `const_`, string-typed `cross` direction) are resolved to
one form on both sides. New analyses (sens, PSS, …) land with the same
shape on both hosts in the same feature — never one-sided. The full
Rust-side alignment is the `uniform-host-api` feature (ROADMAP P3);
Appendix C §4 is the working review sheet.

**Status:** Locked (user, 2026-07-18 — "princípio da uniformidade").
Implementation: sens/PSS bindings immediately; full alignment in P3.

---

## Handoff Snapshot

**Last updated:** 2026-07-18 — `p1-solver-complete` DELIVERED (Verifier round 2
PASS), 509 green / 5 ignored.

### Feature E — `p1-solver-complete` (DELIVERED)

Verifier round 2 **PASS** (`.specs/features/p1-solver-complete/validation.md`):
spec-anchored evidence for all 25 active ACs; discrimination sensor 6/6 killed.
Round 1 FAIL gaps fixed: `list_params` Rebuild classification (`d3693a9`),
PSS hidden-digital-state round-trip (`5dc84a9` — new
`Element::digital_hidden_snapshot/restore` ABI; shots now deterministic from
the shot start per the mixed-signal contract), table slope discrimination
(`038d5f8`), transition zero-rise edge (`4c178b1`). SC-16 (urc) stays blocked
on the `codegen-parametric-devices` feature.

Spec/design/tasks in `.specs/features/p1-solver-complete/`. ROADMAP pillar
P1 closed: every item done or in the named backlog table (ROADMAP.md).
Delivered beyond the other AI's T1–T18:

- **T19 fetlim/limvds** (`81f36af`) — inherited uncommitted; validated against
  ngspice devsup.c, shallow test cases strengthened (all clamps/floors bite).
- **T20 temperature** (`5dfa04d`) — tnom audit uniform; `.temp` sweep test
  (−1.66 mV/K at 4.3 mA, theory −1.7).
- **T21 inductor TR dual** (`f76b4db`) — fix pre-landed `d400973`; added the
  missing coupled-LC transient regression; discrimination proven (mutant
  shifts first-transfer peak 1.36→1.69 µs, killed).
- **T22 IntegrationMethod removal** (`1d7e605`) — enum + dead `TruncationError`
  + `Tolerances.integration` gone; `suggest_transient_step` re-signed.
- **T23/T24** — pre-landed (`2403e29` scheduler split, `1857df5` SignalBridge);
  marked with evidence.
- **T25** (`81b9c1d`) — `Netlist::initial_values` (as_iv re-home), shared
  `Integrator::trapezoid`, init_global ownership proof test.
- **T26** (`e8f1ff4`) — `record_device_state` opt-in (uniform on both hosts,
  MD-22); `Trace.i` on stateful devices KCL-exact; loud error kept when off.
- **T27** — ROADMAP P1 closed + named backlog; spec traceability 25 done /
  1 blocked; docs (part VIII, appendix C) carry `record_device_state`.
- **T16 urc — BLOCKED** on `codegen-parametric-devices` (hierarchy flattening,
  const-args-into-behavior, array-node expansion) — logged as its own feature.

**Baseline at close:** `cargo test --workspace` 504 green / 5 ignored, zero
warnings; ngspice live (27/27). MD-07 amended (IntegrationMethod removed).
**Branch:** `feature/bench-removal` (all p1 work landed here).

### Feature D — `bench-removal` (DELIVERED)

Spec/design/tasks in `.specs/features/bench-removal/`. The in-language
`bench` is gone (keyword = syntax error, `piperine-bench` deleted) and the
host surfaces consolidated:

- **Root `piperine` crate is the library face (MD-19, lib-only)**: `src/`
  hosts `SimSession`/`SolverConfig`, result objects (`results.rs`,
  `waveform.rs`), `Error`, `SimHooks` (plugin lifecycle — preserved per user
  decision), and a `prelude` re-exporting lang/codegen/solver. The `piperine`
  binary target lives in `piperine-cli` (`[[bin]]`).
- **Tests of record migrated** to root `tests/`: ngspice harness (19, live
  green), spice smoke (7, fixtures ported to session API with identical
  assertions), compile-once sweep, run_examples dual contract (24 `.phdl`
  elaborate + 26 `.py` run), session suite incl. 4 behaviors ported from the
  deleted bench.rs (tran start, nodeset, `Trace::i`, digital op readback).
- **`piperine test`** runs `*_tb.py` testbenches (subprocess isolation,
  per-file timeout via `PIPERINE_TEST_TIMEOUT_SECS`, exit codes).
- **Plugin bench-task surface removed** (SDK, wire protocol, WASM ABI v3);
  manifests declaring `bench_tasks` fail loud. Lifecycle hooks preserved via
  root `SimHooks`; plugin gates ported to the root session.
- **Python sanitized**: `Module.stage` → `Module.set`, facade fully
  docstringed (hygiene gate walks it), native sub-views surfaced
  (`InstanceView`/`Terminal`/`SolverStats`).
- **Docs**: `docs/spec/part_viii_host_api.md` (new host-API part); Part III
  tombstoned; index/Parts I/II/V/VI/appendices swept; AGENTS.md/CLAUDE.md/
  README/ROADMAP updated.
- **Deviation logged**: `piperine run <file>.phdl` no longer executes bench
  entry points (it elaborates and points at `*_tb.py`/the REPL).

### Feature A — `solver-trbdf2-engine` (DELIVERED — cleanups deferred)

Spec/context/design/tasks in `.specs/features/solver-trbdf2-engine/`.
**Delivered & green:** TR-BDF2 (γ=2−√2) two-phase sole scheme; trapezoidal
companion fix (`i_{C,n}` re-derived from prior BDF2); **PI controller
always-adaptive** (Milne LTE over node voltages, with asymmetric-difference
discontinuity exclusion); **`@timer(period, phase)`** + **unified
analog/digital breakpoints**; breakpoint discontinuity handling (skip LTE at
edges, reset prev_h). `docs/spec/` Parts I/II/III/V/VII + ROADMAP updated.
**Subsumed:** `solver-breakpoints` and `solver-unified-events` specs deleted
(both fully delivered by this engine).
**Deferred cleanups (user: "ignore for now"):** (1) remove vestigial
`IntegrationMethod` enum + `TruncationError` trait + dead
`suggest_transient_step` + `Tolerances.integration`; (2) inductor flux
TR-stage companion (dual previous-voltage); (3) T15/T16 permanent
discrimination test + ngspice parity; (4) `bp_dt`.

### Feature B — `python-bindings` (DELIVERED)

All 17 requirements (PY-01..PY-17) verified. Spec/context/design/tasks in
`.specs/features/python-bindings/`. **Delivered:**
- Crate `piperine-python` (PyO3) — `_piperine` native + typed pure-Python facade.
- `load → Design → Module → op/tran/ac/noise → results.v(net)` matching the
  bench shape exactly (PY-17 uniform-shape proof via embedded smoke test).
- numpy arrays (`.values`/`.axis`), `.cross()`, stats, `TranConfig.ic`.
- `piperine run foo.py` (embedded CPython, no pip install).
- `piperine run -i [design.phdl]` (interactive REPL with autocomplete).
- `piperine new` creates `.venv/` with bundled `_piperine.so` + facade (IDE
  autocomplete out of the box — no `target/` needed on the user's machine).
- 21 Python example scripts (one per `.phdl` in `examples/`) — 21/21 pass.

### Remaining solver specs (planning only — no code yet)

| Feature | What | Status |
|---------|------|--------|
| `solver-strategy-composition` | Extract `NewtonStrategy`/`StepperStrategy` traits; `Tolerances`/`Policy` split; `SignalBridge`; MD-13 cleanup | Spec + design + tasks; partially done (homotopy + PI controller delivered) |
| `solver-library-abi` | `Circuit` builder; `Solver::build()`; prelude-only public surface; scheduler split; `as_iv` decoupled | Spec + design + tasks |
| `solver-osdi-abi-completion` | Lifecycle hooks; terminal descriptors; internal unknowns; noise metadata; model/instance separation | Spec only |
| `solver-performance` | Device bypass; matrix reuse; predictor | Spec only |
| `solver-convergence-aids` | Circuit-wide `gshunt`; `fetlim`/`limvds` | Spec only |
| `solver-commit-rollback` | `Element::checkpoint/rollback/commit` lifecycle hooks | Spec only |

### Feature C — `solver-convergence-performance` (DELIVERED — 13/13 tasks)

Spec/design/tasks in `.specs/features/solver-convergence-performance/`.
**Delivered (all phases, 2026-07-16):**
- `SolverStats` fully wired: newton_iterations (plan total incl. homotopy),
  steps accepted/rejected, `dt_min_floor_hits`, dt range, bypass counters,
  `homotopy_strategy`/`homotopy_levels` (via `PlanOutcome`),
  `assembly_time_ns`/`solve_time_ns` (CP-01..03, CP-08)
- User tolerances reach the Newton loop (CP-04,05); Python `op.stats` /
  `trace.stats` (CP-09)
- Zero-alloc Newton loop: `reset()` + hoisted `residual`/`scale` fields +
  shared `compute_residual` (CP-06); Milne `node_indices` hoisted out of the
  step loop
- Device bypass (solution-delta stamp cache) hardened: cache invalidated on
  gmin/src_scale changes + digital settle; suppressed while limiting;
  build-into-cache buffer reuse (CP-11)
- `ConvergenceHint{net, limited_value}` + `Element::convergence_hint` — the
  solver applies structured limits to the guess pre-convergence-test (CP-12)
- `suggest_transient_step` consulted (CP-13); `gshunt` (CP-14)
- First-order Newton predictor: `set_predictor_ratio` one-shot seed, armed by
  the transient driver for the TR stage, gated off after rejections and
  breakpoint landings (CP-16)
- Tolerances/Policy split (MD-04 **done**): `Context = {Tolerances}` only;
  `Policy{max_iter, dc_damp_tolerance}` owned per driver (`pub policy` on
  Dc/Transient/Ac solvers); `Context.time` removed — time is an explicit
  argument (`accept_timestep(state, t, …)`, `accept_and_run_digital(sol, t)`)
  (CP-17)
- Dead code: `alpha`, `apply_limit` overrides, `Policy::damp_update`,
  `DcContext` stub, `util.rs` (`AsAny` + `map!` macro — MD-13 rules 4+5)
- Newton unsafe removed: `NonLinearSystem::netlist()` replaces the
  raw-pointer aliasing workaround in dc/transient; convergence math deduped
  onto `Tolerances::{has_converged, residual_test}`

**Known perf lever (not a regression):** DC midpoint damping
(`dc_damp_tolerance = 0.5`, global L2) costs ~4 extra Newton iterations on
trivial linear circuits (divider converges in 6, not 2). Next candidate:
damp only when limiting/oscillation is detected.

### Test baseline
- `cargo build --workspace` — zero warnings.
- `cargo test --workspace` — 391 green.
- 21/21 `examples/*.py` pass via `piperine run`.
- Stats validated on real runs: divider op ni=6 (plain Newton), clipper op
  ni=75, clipper tran 2 iters/step (1/phase — floor), timing/homotopy fields
  populated.
