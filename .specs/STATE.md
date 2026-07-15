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

**Status:** Locked. Homotopy state extracted; full split pending.

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

### MD-07: IntegrationMethod in math/
`IntegrationMethod`, companion coefficients (`coeffs(dt, dt_prev, order)`),
`TruncationError`, `BreakpointProvider` all live in `math/integration.rs`.
The kernel calls the centralised formula — no per-method branching in
codegen.

**Status:** Done.

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

---

## Handoff Snapshot

**Last updated:** 2026-07-15. Two features delivered this cycle; remaining
solver specs are planning-only (no code yet).

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

### Test baseline
- `cargo build --workspace` — zero warnings.
- `cargo test --workspace` — green (TR-BDF2 active, python-bindings active).
- 21/21 `examples/*.py` scripts pass via `piperine run`.
