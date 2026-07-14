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

**Feature:** `solver-trbdf2-engine` (TR-BDF2 sole integration scheme + PI controller + unified breakpoints + factorization reuse). Spec/context/design/tasks in `.specs/features/solver-trbdf2-engine/`.
**Branch:** `feature/plugin-architecture`
**Last commit:** `175bbf6`
**Working tree:** clean (T5 attempt reverted to green T4 state).

### Completed
- Specify + Design + Tasks phases done; committed (`17c0865`, `0b78b55`, `175bbf6`).
- TRB-20 baseline recorded (design.md): narrow-pulse charge pump under the 500-step budget — current arch gives `1ns≡10ns` (identical, wrong) + non-monotonic + 5–7× budget blowup. Pure-PHDL `Pulse` source written (100% PHDL, `if/else` on `$abstime`).
- **Phase 1 (seams) — T1–T4 DONE & committed:** `6fd9ed3` (TrBdf2 math), `4abf75b` (Element::next_breakpoints), `ea87b24` (BYPASS_OK), `7d3cb6c` (FaerSparseLinearSystem::reset). All additive, build zero warnings, 34+ lib tests green.

### In progress / blocked
- **T5 (two-phase driver) reverted — design discovery:** the kernel's reactive companion (`device/analog.rs::load_transient`) is pure-derivative `i_C = c0·Q+c1·Q_prev+c2·Q_prev2` (BDF style). The TR stage needs the trapezoidal companion `i_C = (2C/s)(V−V_n) − i_{C,n}` which tracks the **previous capacitor current** per reactive port — state the kernel does not hold. Without it the TR stage degrades to BE-over-half-step (measured `τ_eff≈1.55τ`). **New prerequisite task T5a** (kernel previous-current bank) inserted into tasks.md; must land before the merged T5/T6 two-phase driver.

### Next step
- Implement **T5a** (kernel trapezoidal companion support: per-reactive-port previous-current state bank, applied only in the TR phase; BDF2 phase unchanged). Verify with the RC-discharge case (`V(τ)` within 1% of `e⁻¹`). Then proceed to the merged T5+T6 (context phase + two-phase driver).

### Blockers
- None — the path is clear (T5a → T5/T6 merged → T7 PI → T8–T16). The kernel-enhancement scope of T5a is the only addition vs the original plan.

### Uncommitted files
- None.

### Test baseline
- `cargo build --workspace` — zero warnings.
- `cargo test --workspace` — green at T4 (Phase 1 complete). The two transient examples (`02_rc_lowpass` discharge, `12_opamp_follower` settle) currently pass on Gear-2; they will migrate to TR-BDF2 outcomes once the engine activates (after T5a+T5/T6).
