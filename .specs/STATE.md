# Project State — Piperine

## Macro Decisions (locked)

These are the architectural decisions that shape the solver. They are binding
and won't be relitigated per-PR. Feature specs reference them by ID.

### MD-01: One Element ABI, no downcast
The solver talks to every participant through one `Element` trait with
`ElementCapabilities` bitflags. No `Device` wrapper, no analog/digital facet
split, no downcast. A resistor, a gate, a comparator, and a plugin are the
same type to the solver.

**Status:** Done (amended 2026-07-19, solver-simplification batch 3):
`Element` is the conjunction of concern-scoped supertraits (`AnalogDevice`
+ `DigitalDevice` + `Introspect`). The object is not split — still one
`Element` ABI, still no `Any`/downcast; only its surface is grouped so
each concern is separately legible, and capability flags still gate which
concern runs. Rationale beyond legibility: a downcast-based facet split
would block the future C-style FFI ABI — supertraits keep the object flat
across the boundary.

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

**Status:** Done (2026-07-19, solver-simplification). `HomotopyStrategy`
(gmin/source stepping), `NewtonStrategy` (`DampedNewton`, wired in
`analyses/dc.rs` + the transient kernel), and `StepperStrategy`
(`PiController`, wired in `analyses/transient.rs`) all shipped and wired;
homotopy/stepper literals live in `analyses/config.rs`.

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

**Last updated:** 2026-07-19 — `solver-simplification` batches 1–5 (P0–P8)
DONE, 520 green / 5 ignored, 0 rustc warnings.

### Feature — `solver-simplification` (IN PROGRESS — batch 6 remaining)

Spec/design/tasks in `.specs/features/solver-simplification/`.
Behavior-preserving refactor of `piperine-solver`; the oracle is the P0
parity baselines (bit-identical) plus the unchanged 520-test suite.

- **Batch 1 (P0+P1)** ✅ — parity baselines pinned; dead surface removed
  (`LINEAR`, `ANALYTIC_JACOBIAN`, `STAMPS_CHARGE` + producers/asserts,
  phantom rollback doc); `SignalBridge` folded into `CircuitInstance`.
- **Batch 2 (P2+P3)** ✅ — `math/unit.rs` removed (`f64` inline, `Second`
  off the ABI surface); config home `analyses/config.rs`
  (`GminSchedule`/`SourceSchedule`/`StepperGains`/`TraceFlags`, defaults
  == former literals) wired into homotopy, `PiController`, trace path.
- **Batch 3 (P4)** ✅ — `Element` = `AnalogDevice + DigitalDevice +
  Introspect` conjunction (MD-01 amended 2026-07-19); codegen
  `PiperineDevice` + test doubles regrouped into the four blocks;
  composed-surface contract test (`composed_element.rs`).
- **Batch 4 (P5+P6)** ✅ — `CircuitInstance` five contracted sections;
  `solver/` + `analysis/` collapsed into `analyses/` (Scheme B, data +
  driver co-located); per-module `//!` layer contracts.
- **Batch 5 (P7+P8)** ✅ — transient `solve()` decomposed into named
  phase methods (`predict_step` / `attempt_step` / `assess_step` /
  `accept_step` / `settle_digital` / `record_step` / `propose_dt` /
  `reject_lte_step` / `reject_step`, plus `begin_run` / `finish_run` and
  the `TimeLoop` state struct — no driver method > 60 lines); STATE.md
  refreshed (MD-05 done, MD-01 amendment, this snapshot); module `//!`
  contract audit.

**Baseline at batch-5 close:** `cargo test --workspace` 520 green /
5 ignored, 0 rustc warnings; parity baselines bit-identical through every
batch.
**Remaining:** batch 6 (P9) — Part VII canonical rewrite (T33–T35), then
the feature Verifier.
**Branch:** `feature/bench-removal`.

### Previously delivered features (summary)

- **`p1-solver-complete`** (DELIVERED 2026-07-18, Verifier round 2 PASS) —
  25/25 active ACs, sensor 6/6; ROADMAP pillar P1 closed. Details in
  `.specs/features/p1-solver-complete/validation.md` and git history.
- **`bench-removal`** (DELIVERED) — in-language `bench` gone; root
  `piperine` crate is the library face (MD-19, superseded by MD-20);
  tests of record in root `tests/`; `piperine test` runs `*_tb.py`.
- **`solver-trbdf2-engine`** (DELIVERED) — TR-BDF2 sole scheme, PI
  controller always-adaptive, unified analog/digital breakpoints.
- **`python-bindings`** (DELIVERED) — `piperine-python` (PyO3) +
  pure-Python facade; PY-01..PY-17 verified.
- **`solver-convergence-performance`** (DELIVERED) — `SolverStats` wired,
  zero-alloc Newton, device bypass, `ConvergenceHint`, Tolerances/Policy
  split (MD-04 done).
