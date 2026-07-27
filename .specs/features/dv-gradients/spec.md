# dv-gradients Specification — analytic sensitivity and the host policy driver

**Vision:** `.specs/features/design-verification/ideal.md` §3–§5. Decisions
D1–D16 there are binding; this spec cites them.
**Depends on:** `dv-core` (margins are what the gradients act on).
**Sibling:** `dv-tester`.
**Scope tier:** Large/Complex. **ROADMAP:** P7 (Optimizer) + the new P8.

The differentiator. The compiler already differentiates symbolically to build
the Jacobian; pointed at parameters it yields `∂F/∂p`, and one adjoint solve
turns that into the gradient of any scalar with respect to **every** parameter.
On top of it sits one host driver with three policies: optimize, center,
high-sigma.

## Problem Statement

The only sensitivity path today is `.sens` (`crates/piperine-solver/src/analyses/sens.rs:2`):
central finite difference over the restamp path, two full DC re-solves per
parameter. Fifty sizing knobs cost a hundred simulations, which puts
gradient-based sizing and worst-case-distance centering out of reach and leaves
black-box search as the only option. Meanwhile `resolve/diff.rs` already emits
symbolic derivatives for the Jacobian, and `analyses/noise.rs:240` already solves
a transposed system with a unit excitation — both halves of the adjoint method
exist in the tree and neither is wired to parameters.

## Goals

- [ ] A `∂F/∂p` kernel emitted at compile time — **one kernel taking a parameter
      index**, never one function per parameter (§3.1's `.disto` lesson).
- [ ] `AnalogDevice::param_stamps(param, &mut sink)` + `HAS_SENSITIVITY` — the
      whole ABI delta: one method, one bit, one documented fallback.
- [ ] A **DC** adjoint sensitivity driver (D6), verified against `.sens`.
- [ ] Stamp-perturbation fallback for elements without the bit, always reported.
- [ ] Host reads: `sensitivity(metric, param)` and `gradient(metric)`, identical
      on both hosts.
- [ ] One host driver with three policies — optimize / center / high-sigma —
      over the restamp loop, plus Monte Carlo sampling of `tol` declarations.
- [ ] Aging as parameter drift over a declared lifetime: two runs of the same
      margins, reporting the delta.

## Out of Scope

| Feature | Reason / owner |
|---|---|
| `tol` grammar, `constraint` grammar, margins, postures, findings channel | `dv-core` |
| The programmatic tester, monitors, `cover` | `dv-tester` |
| **AC adjoint** | Known gap (D6): DC first because `.sens` is the only finite-difference oracle. AC-metric gradients are finite-differenced or treated as feasibility filters until it lands |
| **Transient adjoint** (backward-in-time) | Needs trajectory checkpointing design; §3.4 |
| RL/GNN sizing | Dominated by the gradient path for continuous sizing |
| Topology optimization | Discrete search outside this feature |
| Foundry aging model *data* (stress equations per device) | Model-authoring work; the built-in ngspice-faithful models have none, same reasoning as `dv-core`'s absent SOA limits |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Confirmed? |
|---|---|---|
| DC adjoint before AC | DC — `.sens` is a verifiable oracle; an unverifiable gradient is worth less than a slower verifiable one | **y** (D6) |
| Non-smooth worst case | Differentiate at the located argmin; step control must tolerate a gradient discontinuity where the argmin switches | **y** (D1) |
| Differentiable admissibility | `Real`-typed + restamp-class `Invalidation` — the same rule `.sens` already enforces; rebuild-class or discrete fails loud | **y** (existing rule) |
| One kernel, parameter-indexed | Mandatory, not preferred — the ordered cross-product is exactly how `.disto` overruns Cranelift on MOS2/MOS3 | **y** (§3.1) |
| Stamps into a caller sink | `load_dc`'s pattern; this call is in the adjoint's inner loop over parameters | **y** (§3.2) |
| Optimize / center / high-sigma are one driver | Three policies, one engine — the alternative is three engines that drift | **y** (§5) |
| Driver lives in `piperine-api` | So both hosts get one implementation (MD-22); scipy/BoTorch stay optional accelerators, never the source of truth | n (agent) |
| Adjoint driver lives in `piperine-solver` | A DC analysis variant; the solver never depends on codegen, so `∂F/∂p` arrives through the element ABI | n (agent) |
| Monte Carlo reproducibility | Every sample replayable from `(seed, index)` as an ordinary session | n (agent) |
| `σ_i` in centering | Held fixed within an outer iteration, re-sampled between them | n (agent) |

**Open questions:** none blocking. The AC adjoint is a recorded gap, not an
open question — the decision was taken (D6) and its cost is stated.

---

## User Stories

### P1: The `∂F/∂p` kernel ⭐ MVP

**User Story**: As the compiler, I want to emit parameter derivatives alongside
the residual and Jacobian, so the solver can be handed exact `∂F/∂p` instead of
differencing stamps.

**Why P1**: Everything else in this spec consumes it.

**Acceptance Criteria**:

1. WHEN codegen compiles a circuit with differentiable params (`Real`-typed,
   restamp-class) THEN it SHALL emit a `∂F/∂p` kernel alongside residual and
   Jacobian, **once**, at compile time — a later `set` SHALL NOT re-emit (MD-18).
2. WHEN the kernel is emitted THEN it SHALL be **one function taking a parameter
   index**, not one function per parameter. A model card with ~60 params SHALL
   compile; the acceptance test is a `Mos1`-based circuit whose parameter count
   would have exploded a per-parameter design.
3. WHEN a parameter is rebuild-class or discrete THEN no derivative SHALL be
   emitted for it and a later request SHALL fail loud (same rule as `.sens`).
4. WHEN the compile-once guard runs on a sensitivity-enabled circuit THEN the
   compile count SHALL be unchanged by sensitivity requests and `set` loops.

**Independent Test**: compile a MOSFET circuit, assert one `∂F/∂p` function
exists and is index-driven, assert `AnalogKernel::compile_count()` is unchanged
across a `set` loop, and assert a rebuild-class parameter request is loud.

---

### P1: ABI delivery — `param_stamps` + `HAS_SENSITIVITY` ⭐ MVP

**User Story**: As the solver, I want to ask an element for its parameter stamps
without knowing whether it came from PHDL, a plugin, or OSDI.

**Why P1**: The solver never sees PHDL; this is the only seam.

**Acceptance Criteria**:

1. WHEN `AnalogDevice` grows `param_stamps(param_handle, &mut sink)` THEN stamps
   SHALL be written into the caller's sink at the current operating point — no
   `Vec` returned per call (`Stamp` already exists at
   `crates/piperine-solver/src/math/linear.rs:12`).
2. WHEN an element declares `HAS_SENSITIVITY` THEN `param_stamps` SHALL deliver
   analytic stamps; PHDL-JIT elements SHALL declare it and plugin/OSDI elements
   SHALL default clear. The bit joins the existing
   `HAS_DISTO2 = 1 << 12`/`HAS_DISTO3 = 1 << 13` precedent.
3. WHEN an element lacks the bit THEN the driver SHALL fall back to **stamp
   perturbation** — two `load_dc` calls at the **same** operating point per
   parameter, no re-solve — so the adjoint economy (one transpose solve for all
   parameters) survives and only `∂F/∂p` becomes approximate.
4. WHEN the fallback is used THEN the result SHALL name which elements used it —
   never a silent accuracy change.

**Independent Test**: a test element without the bit in a circuit with PHDL
devices; assert analytic and fallback paths coexist, that the fallback is
reported by element label, and that the linear-solve count is unchanged by which
path an element takes.

---

### P1: The DC adjoint driver ⭐ MVP

**User Story**: As an optimization-loop author, I want
`r.sensitivity("idd", "w1")` to cost one adjoint solve, so fifty knobs cost one
extra solve instead of a hundred simulations.

**Why P1**: The defensible differentiator; the centering story collapses without it.

**Acceptance Criteria**:

1. WHEN a DC gradient of a scalar output is requested THEN the driver SHALL solve
   `Jᵀλ = ∂f/∂x` **once** and combine with `∂F/∂p` for **all** requested
   parameters from that one solve.
2. WHEN the driver is built THEN it SHALL be a DC analysis variant in
   `piperine-solver`; note that `analyses/noise.rs:240`'s adjoint is complex and
   per-frequency (the AC shape), so this transpose solve is a **sibling** path,
   not a reuse — the spec must not claim otherwise.
3. WHEN adjoint and `.sens` both compute a DC sensitivity on a differentiable
   circuit THEN they SHALL agree within the finite difference's own error tolerance.
4. WHEN the number of linear solves is counted THEN a 20-parameter gradient SHALL
   cost **one** extra solve, not 40 re-solves.
5. WHEN a gradient is requested for a metric that cannot be differentiated
   through the solver (event-detected settling time, overshoot instant) THEN the
   request SHALL fail loud — never a plausible wrong number.
6. WHEN a gradient of a **pointwise** margin is requested THEN it SHALL be taken
   at the located argmin (D1) and the result SHALL say so; WHEN a **reduced**
   margin's gradient is requested THEN it SHALL be delivered only if the
   reduction is differentiable (a unity-gain frequency differentiates through the
   implicit function theorem on `|gain(f)| = 1`) and fail loud otherwise.
7. WHEN an AC-defined metric's gradient is requested THEN it SHALL fail loud
   naming the AC-adjoint gap (D6), never silently finite-difference behind the
   caller's back — the host may choose finite differences explicitly.

**Independent Test**: RC/divider fixture — adjoint gradient of `V(out)` w.r.t.
two params matches `.sens` within FD tolerance at one counted adjoint solve;
negative tests for an event-detected metric and an AC metric.

---

### P1: Host sensitivity surface

**User Story**: As a host author, I want `sensitivity(metric, param)` and
`gradient(metric)` with identical names on Python and Rust.

**Acceptance Criteria**:

1. WHEN `sensitivity`/`gradient` are called THEN they SHALL trigger the adjoint
   path and return typed values; unknown metric/param names SHALL fail loud
   listing candidates.
2. WHEN the same call is made on both hosts THEN names, shapes, and values SHALL
   be identical (`host_parity.rs` style).
3. WHEN `.sens` and the adjoint are both available THEN the result SHALL report
   which engine produced it.

**Independent Test**: one fixture through both hosts asserting identical gradient
values and identical engine attribution.

---

### P2: The driver — optimize policy

**User Story**: As a designer, I want `ota.optimize(objective="idd", over={…})`
to walk the restamp loop with gradients and give me a sized design.

**Acceptance Criteria**:

1. WHEN `optimize` runs THEN it SHALL structure the search as a feasibility phase
   (climb until every `require` margin ≥ 0) followed by an objective phase
   (descend the objective while projecting onto the feasible set), with
   constraints supplied as `{m_i ≥ 0}` from `dv-core`'s margin channel.
2. WHEN `optimize` runs THEN it SHALL use `collect` posture — an optimizer that
   aborted on its own infeasible iterates could not search.
3. WHEN the objective or a constraint is not differentiable THEN the driver SHALL
   fall back to a black-box engine (CMA-ES or Bayesian optimization) for that
   part, compose it with the gradient path (gradient inside a discrete shell),
   and **report which engine produced the result**.
4. WHEN an objective names an unknown `var` THEN the call SHALL fail loud at the
   host boundary.
5. WHEN the loop runs THEN it SHALL restamp, never re-elaborate — the compile
   count SHALL be unchanged across the whole optimization (MD-18).

**Independent Test**: a two-parameter fixture with a known optimum; assert
convergence in ~10¹–10² solves, zero recompiles, and engine attribution.

---

### P2: Monte Carlo over `tol` declarations

**User Story**: As a designer, I want `m.monte_carlo(n=500, seed=7)` to sample
every declared distribution and give me yield and the failing sample.

**Acceptance Criteria**:

1. WHEN sampling runs THEN it SHALL walk the **authored instance tree** to decide
   draw counts: a `global` (process) parameter gets one draw shared by every
   instance; a plain (mismatch) parameter gets an independent draw per instance.
2. WHEN a sample is drawn THEN it SHALL be reproducible from `(seed, index)` and
   replayable as an ordinary session.
3. WHEN the run completes THEN it SHALL report yield (fraction with all margins
   ≥ 0), per-metric spread `σ_i`, and the worst sample per constraint.
4. WHEN 10³ samples run THEN they SHALL be 10³ restamps on one JIT, never 10³
   elaborations (MD-18).
5. WHEN a nominal analysis runs afterwards THEN it SHALL return to declared
   values — sampling SHALL NOT leave the session perturbed.

**Independent Test**: a two-instance fixture with one `global` and one mismatch
`tol`; assert the global draw is shared and the mismatch draws differ, assert
`(seed, index)` replay reproduces a named failing sample exactly, assert the
compile count is 1.

---

### P2: The driver — center policy

**User Story**: As an analog designer, I want `m.center(...)` to push my design
away from every boundary and tell me the worst-case distance in sigmas.

**Why P2**: The single most valuable output in the vision for a real designer;
needs the gradients and the sampling first.

**Acceptance Criteria**:

1. WHEN `center` runs THEN it SHALL maximize `min_i (m_i / σ_i)` over the design
   parameters, using `dv-core`'s normalized margins and this spec's `σ_i`.
2. WHEN `σ_i` is estimated THEN it SHALL be held fixed within an outer iteration
   and re-sampled between iterations — and the result SHALL state which scheme it
   used, since `σ_i` depends on the design point.
3. WHEN the `min` over constraints switches its active member THEN
   differentiation SHALL follow D1 (at the active constraint) and the step
   control SHALL tolerate the resulting gradient discontinuity.
4. WHEN `center` completes THEN it SHALL report the centered parameters and the
   worst-case distance in sigmas.

**Independent Test**: a fixture with two competing constraints and known
geometry; assert the centered point increases the worst normalized margin versus
the nominal, and that a Monte Carlo at the centered point shows higher yield.

---

### P3: High-sigma sampling and aging

**User Story**: As a memory designer, I want a 5–6σ yield estimate without 10⁸
samples; as a reliability engineer, I want to know which constraint goes negative
first over a declared lifetime.

**Acceptance Criteria**:

1. WHEN high-sigma estimation runs THEN it SHALL be the third policy of the same
   driver, using importance sampling (statistical blockade or scaled-sigma) over
   the restamp loop.
2. WHEN a yield number is reported THEN it SHALL travel with the estimator that
   produced it and a confidence interval — a yield without its estimator is not a
   yield.
3. WHEN a tail sample is found THEN it SHALL be replayable from `(seed, index)`.
4. WHEN aging is requested THEN the host SHALL compute stress from an
   operating-point or transient run, restamp the drifted parameters, re-verify,
   and report the **margin delta** — which constraint goes negative first, and
   after how long — with no new result type.

**Independent Test**: a bitcell-shaped fixture where importance sampling and a
long plain Monte Carlo agree within the reported confidence interval; an aging
fixture whose margin delta names the first constraint to fail.

---

## Edge Cases

- A circuit with zero differentiable params → gradient requests fail loud, and no
  `∂F/∂p` kernel is emitted (no cost).
- A metric that is constant w.r.t. every parameter → gradient of exactly zero is
  a legitimate answer, distinguishable from "not differentiable".
- An element with `HAS_SENSITIVITY` whose `param_stamps` returns nothing for a
  parameter it does not own → not an error; the parameter simply has no
  contribution from that element.
- Mixed analytic/fallback in one circuit → allowed, reported, and the transpose
  solve count is unchanged.
- `optimize` over a parameter that also carries `tol` → legal; the optimizer
  moves the nominal, the distribution stays attached (`dv-core` AC).
- A sampled draw that makes a circuit fail to converge → reported as a sample
  outcome, not as an analysis crash; the run continues and the sample is named.
- Aging drift that crosses a rebuild-class boundary → loud, not silently re-elaborated.

---

## Requirement Traceability

| ID | Story | Status |
|---|---|---|
| DVG-01 | `∂F/∂p` kernel, emitted once at compile time | Pending |
| DVG-02 | One kernel, parameter-indexed (no per-parameter explosion) | Pending |
| DVG-03 | Differentiability admissibility + loud refusal | Pending |
| DVG-04 | `param_stamps` into a caller sink | Pending |
| DVG-05 | `HAS_SENSITIVITY` capability bit | Pending |
| DVG-06 | Stamp-perturbation fallback, no re-solve | Pending |
| DVG-07 | Fallback reported by element | Pending |
| DVG-08 | DC adjoint: one transpose solve for all parameters | Pending |
| DVG-09 | Adjoint vs `.sens` agreement within FD tolerance | Pending |
| DVG-10 | Solve-count proof (20 params = 1 extra solve) | Pending |
| DVG-11 | Pointwise gradient at argmin (D1); reduced only if differentiable | Pending |
| DVG-12 | AC-metric gradient fails loud naming the gap (D6) | Pending |
| DVG-13 | Host `sensitivity`/`gradient`, both hosts, engine attribution | Pending |
| DVG-14 | Optimize policy: feasibility then objective, `collect` posture | Pending |
| DVG-15 | Black-box fallback composed and attributed | Pending |
| DVG-16 | Monte Carlo: process vs mismatch draws over the instance tree | Pending |
| DVG-17 | `(seed, index)` reproducibility and replay | Pending |
| DVG-18 | Yield, `σ_i`, worst sample reporting | Pending |
| DVG-19 | Center policy: `min_i (m_i/σ_i)`, σ scheme stated | Pending |
| DVG-20 | High-sigma importance sampling + estimator and CI | Pending |
| DVG-21 | Aging: drift, re-verify, margin delta | Pending |
| DVG-22 | cross-cutting: MD-18 compile-once across every loop | Pending |

**Coverage:** 22 total, 0 mapped (tasks phase not started).

---

## Success Criteria

- [ ] `r.sensitivity(...)` on a differentiable circuit matches `.sens` within FD
      tolerance at **one** counted adjoint solve; a 20-parameter gradient costs
      one extra linear solve, not 40 re-solves.
- [ ] A `Mos1`-based circuit with ~60 model-card parameters compiles its `∂F/∂p`
      kernel without approaching the Cranelift function-count wall that `.disto`
      hits on MOS2/MOS3.
- [ ] An optimize run converges in ~10¹–10² solves with **zero** recompiles and
      reports which engine produced the result.
- [ ] A centered design shows a higher worst normalized margin and higher Monte
      Carlo yield than the nominal, with the σ scheme stated.
- [ ] Every gradient the tool cannot compute exactly fails loud — no plausible
      wrong numbers, including for AC metrics until the AC adjoint lands.
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` green.
