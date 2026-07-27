# dv-gradients Design

**Spec:** `spec.md` (DVG-01..22). **Vision:** `../design-verification/ideal.md` §3–§5.
**Depends on:** `dv-core` (margins are the functions being differentiated).

## A. The three layers

```
compile time ── codegen ─────────────────────────────────────────────────────
  resolve/diff.rs already differentiates w.r.t. *unknowns* (the Jacobian).
  Same pass, different differentiation variable → ∂F/∂p.
  Emitted ONCE as a single parameter-INDEXED kernel (§C1).
        │
run time ── solver ─────────────────────────────────────────────────────────
  AnalogDevice::param_stamps(param, &mut sink)  ← gated by HAS_SENSITIVITY
  fallback: two load_dc calls at the SAME operating point (no re-solve)
        │
  adjoint DC driver:  Jᵀλ = ∂f/∂x   (one solve)   then   ∂f/∂p = −λᵀ(∂F/∂p)
        │
host ── piperine-api ──────────────────────────────────────────────────────
  sensitivity(metric, param) / gradient(metric)
  ONE driver, THREE policies: optimize | center | high-sigma      (§C4)
```

## B. Component breakdown

| Component | Home | Notes |
|---|---|---|
| Parameter differentiation | `piperine-codegen/src/resolve/diff.rs` (extend) | correctness-critical file; additive entry point, no change to unknown-differentiation |
| `∂F/∂p` kernel emission | `piperine-codegen/src/kernel/analog/` | one function, parameter index as an argument (§C1) |
| `param_stamps` | `piperine-solver/src/core/element.rs` (`AnalogDevice`) | trait exists — `abi.rs`, impl at `codegen/src/device/element.rs:139` |
| `HAS_SENSITIVITY` | `piperine-solver/src/core/element.rs` | next to `HAS_DISTO2 = 1 << 12` / `HAS_DISTO3 = 1 << 13` |
| Adjoint DC driver | `piperine-solver/src/analyses/` | sibling of `sens.rs`, **not** a reuse of `noise.rs` (§C2) |
| Perturbation fallback | same driver | two `load_dc` at one operating point |
| Host sensitivity surface | `piperine-api/src/` | on the result objects |
| The policy driver | `piperine-api/src/` | optimize / center / high-sigma (§C4) |
| Sampling | `piperine-api/src/` | walks the authored instance tree (§C3) |

## C. The four decisions that shape the implementation

### C1. One kernel, parameter-indexed — enforced, not preferred

`ROADMAP.md` P1 records an open bug found 2026-07-26: the `.disto` 2nd/3rd
derivative kernels emit **one JIT function per ordered controlling-branch
combination**, and on MOS2/MOS3 that count overruns Cranelift with
`TryFromIntError`, making those devices uncompilable. A `Mos1` model card carries
~60 parameters. A `∂F/∂p` design that emits a function per parameter walks into
the identical wall with the identical symptom, one crate over.

So the kernel takes a parameter index and the acceptance test is a `Mos1`-based
circuit that would have exploded a per-parameter design (DVG-02). This is the one
design constraint in this feature with a live failure already in the tree.

### C2. The existing adjoint is the AC shape; DC is a sibling

`analyses/noise.rs:240` `solve_adjoint_system` transposes the system and solves
with a unit excitation — **complex, per frequency**. That is the AC shape.

A DC adjoint needs a real transpose solve: the same idea, a different code path.
The design must say "sibling of the noise adjoint", not "reuse of it", because the
optimistic reading turns into a surprise during implementation.

**DC is nevertheless first (D6)**, for one reason: `analyses/sens.rs:2` (central
finite difference over the restamp path, already refusing `Invalidation::Rebuild`)
is a verifiable oracle, and AC has none. An unverifiable gradient is worth less
than a slower verifiable one — and a plausible wrong gradient is exactly what
this project's fail-loud rule exists to prevent.

Accepted cost, stated in the spec: AC-metric gradients (gain, UGBW, phase margin)
are unavailable in this feature. They fail loud naming the gap; a host may choose
finite differences explicitly, but the driver never substitutes silently (DVG-12).

### C3. Sampling walks the authored instance tree

The mismatch/process distinction is the whole engine of analog yield, and it is a
*structural* question:

- `tol … global` (process) → **one** draw, shared by every instance.
- `tol …` (mismatch) → **independent** draw per instance.

So the sampler walks `Design`'s authored hierarchy (never `flat_modules` — the
UNBREAKABLE rule) to count draws, then writes them through the ordinary restamp
path. `(seed, index)` fully determines a sample, so any sample is replayable as an
ordinary session — which is what makes `mc.worst("headroom")` a debuggable
artifact rather than an anecdote.

### C4. One driver, three policies

| Policy | Objective | Varies | Sampling |
|---|---|---|---|
| optimize | a host-named `var` | design params | none (nominal or corners) |
| center | `min_i (m_i / σ_i)` | design params | inner sampling for `σ_i` |
| high-sigma | failure probability | statistical params | importance-weighted, tail-focused |

All three walk the same restamp loop, read the same margin channel, and use the
same gradients where they exist. Building them as three features produces three
engines that drift; building one driver with a policy parameter produces one
tested engine. It lives in `piperine-api` so both hosts get one implementation
(MD-22) — scipy/BoTorch may accelerate the Python side but are never the source
of truth.

Two honest wrinkles carried into the design:

- **Non-smoothness.** `min` over time, corners, and constraints is non-smooth;
  D1 differentiates at the located argmin. The step control must therefore
  tolerate a gradient discontinuity where the argmin switches, rather than assume
  smoothness and stall.
- **`σ_i` moves with the design.** Centering's normalizer depends on the design
  point. Cheap scheme: hold `σ_i` fixed within an outer iteration, re-sample
  between iterations — and *report which scheme was used*, because the number
  means different things under each.

## D. Data flow: `sensitivity("idd", "w1")`

1. Host resolves `idd` (a `dv-core` `var`, pointwise, DC) and `w1` (a `Real`,
   restamp-class). Anything else → loud.
2. Solver takes the converged DC solution and builds `∂f/∂x` for `idd`.
3. One transpose solve: `Jᵀλ = ∂f/∂x`, reusing the already-factored Jacobian.
4. For each requested parameter, ask each element for `param_stamps` — analytic if
   it declares `HAS_SENSITIVITY`, perturbation otherwise — and accumulate
   `∂f/∂p = −λᵀ(∂F/∂p)`.
5. Return typed values, naming the engine and any element that used the fallback.

Cost: **one** extra linear solve for all parameters, versus `.sens`'s two full
re-solves per parameter. The solve count is an acceptance test (DVG-10), not a
claim.

## E. Risks

| Risk | Mitigation |
|---|---|
| `∂F/∂p` repeats `.disto`'s function-count explosion | §C1 — parameter-indexed kernel, with a ~60-param `Mos1` circuit as the acceptance test |
| "Reuse the noise adjoint" turns out to be a rewrite | §C2 names it a sibling up front; DC-first with `.sens` as oracle keeps the first delivery verifiable |
| Gradients silently wrong for a metric the solver cannot differentiate | Loud refusal is an AC (DVG-11/12); no silent finite-difference substitution |
| Editing `resolve/diff.rs`, the correctness-critical core | Additive entry point only; the existing Jacobian path and its tests are untouched and must stay green |
| Optimizer aborts on its own infeasible iterates | `collect` posture is an AC (DVG-14); `strict` in an optimizer loop is a category error |
| An optimization loop re-elaborates and destroys the economics | `compile_once_sweep.rs`-style count across a full optimize run (DVG-22) |
| Perturbation fallback silently degrades accuracy | Reported per element (DVG-07); absence of the bit changes speed and accuracy, never correctness, and never quietly |
| Centering reports a σ-dependent number without saying which scheme | Scheme reported with the result (§C4) |

## F. Test strategy

| Layer | Targets |
|---|---|
| Codegen | `piperine-codegen/tests/` — one indexed kernel, ~60-param compile, compile-once across `set` loops, loud refusal for rebuild-class/discrete |
| Solver | `piperine-solver/tests/` — adjoint vs `.sens` within FD tolerance, counted linear solves, fallback coexistence and attribution |
| Host | root `tests/` — `sensitivity`/`gradient` parity across hosts, optimize convergence with zero recompiles, MC process-vs-mismatch draw structure, `(seed, index)` replay, centering improves worst normalized margin and MC yield |
| Numeric oracle | `.sens` for DC gradients; a long plain Monte Carlo for the high-sigma estimator's confidence interval |
