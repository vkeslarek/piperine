# SOLVER_GAPS.md — open solver/engine gaps

Rewritten 2026-07-18: everything DONE was purged (git history keeps the
audit trail; the big deliveries — Element ABI, Net naming, TR-BDF2 + PI
controller, LTE stepping, gmin/source stepping, current-residual convergence,
`$limit`/pnjlim, flux companions, `@initial` seed, `SolverDomain` errors,
prelude, live params — are summarized in `CLAUDE.md` and `.specs/STATE.md`).
This file is the **open-items audit** feeding ROADMAP P1/P2. Status:
**PARTIAL** (works in some cases) / **MISSING** (absent).

Cross-validation harness: root `tests/ngspice_validation.rs` (+`tests/ngspice/`)
— `cargo test -p piperine ngspice` after any solver change.

---

## 1. Analyses

- [ ] **`.dc` sweep — MISSING as a native analysis.** Host-level compile-once
  restamp sweeps (MD-18) cover param sweeps; confirm they cover nested sweeps
  and *source* sweeps, or add the solver-side loop.
- [ ] **`.sens` (DC/AC sensitivity) — MISSING.** Reuse the symbolic-diff
  infrastructure. Medium value alone; high value as the optimizer feeder (P6).
- [ ] **PSS (periodic steady state) — MISSING.** Shooting method over the
  transient engine. New V1 item (switching converters).
- [ ] `.four` — belongs to the Python host (FFT post-processing), not the
  solver. Tracked in ROADMAP P3.
- [ ] `.pz`, `.disto`, `.sp` — MISSING, niche, post-V1.

## 2. Transient

- [ ] **Breakpoints — MISSING. Top transient priority.** ngspice lands a
  timepoint exactly on every source discontinuity; piperine's LTE-reject
  backtracking resolves pulse edges but thrashes (~40k steps at edges).
  Source-declared breakpoint schedule (codegen-extracted from the periodic
  phase trick, or a `$periodic_breakpoints` declaration). Design decision
  pending.
- [ ] **Output interpolation onto the `.step` print grid — MISSING.** The
  recorded waveform is the raw adaptive grid (correct but uneven);
  `Waveform::at` interpolates point queries and stats are dt-weighted, so
  this is presentation-layer.
- [ ] **Enforced UIC hold — PARTIAL.** `@initial` seeds t=0; ngspice UIC also
  *holds* the node through the first solve via a large-conductance clamp
  released after t=0.
- [ ] **Inductor flux TR-stage dual — PARTIAL.** The TR stage uses the
  pure-derivative form; previous-voltage tracking is the follow-up (no known
  regression).
- [ ] **Remove vestigial `IntegrationMethod`** (+ `suggest_transient_step`'s
  `method` param). TR-BDF2 is the sole scheme; ~34 references linger.

## 3. Convergence

- [ ] **Circuit-wide `gshunt` / user-raisable diagonal GMIN — PARTIAL.**
  Models add junction gmin; no global option. Low priority.
- [ ] **`fetlim`/`DEVlimvds` — PARTIAL.** Identity today; MOS converges via
  gmin stepping without them. May matter for exact ngspice parity.

## 4. Engine operator gaps (codegen, all fail loud)

- [ ] `table(x, xs, ys, mode)` — **not registered at all** (resolves as
  unknown fn, never reaches the fail-loud path). Register, then implement
  1-D interpolation.
- [ ] `transition`, `laplace_*`, `zi_*` — recognized in the resolved form; no
  companion models.
- [ ] `idt` AC `1/jω` admittance — contributes 0 in AC.
- [ ] Multiple `ac_stim` per contribution.
- [ ] `@initial` cannot force a branch (event bodies reject Force).
- [ ] `Trace.i` over time on devices reading runtime state/vars — per-step
  banks not recorded in `TransientAnalysisResult`.

## 5. Digital

- [ ] **Fused combinational-network JIT — BUILT, not integrated.**
  `NetworkComb`/`DigitalNetwork` tested standalone; wire into
  `circuit.rs::run_digital_at` (cone detection, per-device fallback for
  clocked/analog members), then fuse clocked members. See
  `piperine-codegen/docs/DIGITAL_JIT.md`.

## 6. Element ABI maturity (feeds ROADMAP P2)

OSDI/ngspice used as a checklist, not as the native ABI. The native contract
stays mixed-signal-first; OSDI wrappers are one client.

- [ ] **Internal-unknown allocation — MISSING, the P2 blocker.** External
  models need auxiliary nodes/branches allocated pre-finalization. Blocks the
  `@device(plugin = "osdi", …)` PHDL seam (factory fails loud today).
- [ ] **Model/instance separation — MISSING.** `ModelHandle` (shared card) vs
  `ElementInstance` (terminals, instance params, state); gives sweeps a clean
  rebuild rule.
- [ ] **Explicit lifecycle — MISSING.** Ordered hooks: model setup → instance
  setup → temperature preprocess → load/evaluate → accept/commit → rollback →
  destroy. One chart per analysis.
- [ ] **Commit/rollback for all mixed-signal state — MISSING.** Rejected
  timesteps must restore every stateful participant (A2D crossings, D2A
  latches, co-sim state), not only the digital net array.
- [ ] **Unified event model — MISSING.** One queue for digital events, analog
  crossings, timers, breakpoints, `$bound_step` hints (kind, target, time,
  priority, source, rollback behavior). Pairs with §2 breakpoints.
- [ ] **Richer terminal descriptors** — domain, direction, required/optional,
  sign convention, external/internal/auxiliary.
- [ ] **Opvar catalog** — declared names/types/units/owner for `gm`, `vbe`,
  register state; uniform query path.
- [ ] **Noise metadata** — per-source names/types/terminal pairs; per-source
  contribution reporting (today total PSD only).
- [ ] **Temperature protocol** — nominal/instance/delta separation; declare
  whether a change means recompute constants, restamp, or rebuild.
- [ ] **Parameter invalidation rules** — partially landed
  (`ParamDescriptor::invalidation`); wire sweeps/optimizer to honor them.
- [ ] **Formal limiting API** — proposed/limited values, limiter name, active
  state, reason (today `limiting_active` bool).
- [ ] **Jacobian/stamp capability declaration** — analytic vs numeric vs
  missing; validation error for analyses that need what's absent.
- [ ] **Device-side bypass capability** (see §7).
- [ ] **Save/probe selection** — devices declare observables + cost; record
  only what the host asked.
- [ ] **`NewtonStrategy`/`StepperStrategy`** — fold Newton damping/limiting
  and transient step rejection into the `ConvergencePlan` composition
  (homotopy half is done).
- [ ] Introspect leftovers: model descriptor (type id/version), real
  opvar/terminal catalogs from the kernel (indices exist, names don't).

## 7. Performance

- [ ] **Device bypass — MISSING.** Skip re-evaluating nonlinear devices whose
  terminal voltages barely changed (`CKTbypass`). Matters for large circuits.
- [ ] **Matrix reuse — CHECK.** `self.linear_system = L::new(...)` per Newton
  iteration looks like a full rebuild; confirm faer symbolic factorization is
  actually reused.
- [ ] **Transient predictor — CHECK.** Confirm warm-start from previous step;
  evaluate a polynomial predictor for Newton seeding.
- [ ] **Temperature sweep — PARTIAL.** Models read `temp`/`dtemp`; confirm
  global `.temp` + `tnom` rescaling flows uniformly (analysis-level sweep is
  host-side).

## 8. Model-equation bugs (harness-surfaced, not solver bugs)

- [ ] **MOS1 drain current ~1.5× high** (`validation/nmos_load`: ngspice
  v(d)=3.0 V vs 1.92 V). Check `headers/spice/mos.phdl` β/`kp` path against
  `mos1load.c`.
- [ ] **JFET ~15 mV / ~1 % off** (`validation/jfet_bias`: 1.382 vs 1.397 V).

## 9. Minor refactor leftovers (§7-era)

- [ ] Split `digital/scheduler.rs` into topology/state/scheduler modules.
- [ ] `DcAnalysisResult::as_iv(&Netlist)` — analysis types shouldn't take
  `Netlist`; move or re-sign when the surface finalizes.
- [ ] Shared `Integrator` for noise trapezoid + future `.four`.
- [ ] `SignalBridge` extraction from
  `CircuitInstance::accept_and_run_digital` (three jobs in one method).
- [ ] `Context::default` must not `init_global`; `Solver::build` owns it.

---

## Priority order (recommended)

1. Breakpoints + unified event model (§2/§6) — transient efficiency gate.
2. Internal-unknown allocation (§6) — unblocks OSDI `@device`, P2.
3. `.sens` (§1) — optimizer feeder.
4. PSS (§1).
5. MOS1/JFET model fixes (§8) — parity credibility.
6. Bypass + matrix-reuse check (§7) — perf once correctness is solid.
7. Everything else on demand.
