# dv-gradients Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and
follow its Execute flow and Critical Rules.** Do not search for skill files by
filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Spec**: `.specs/features/dv-gradients/spec.md` (DVG-01..23)
**Design**: `.specs/features/dv-gradients/design.md`
**Vision**: `.specs/features/design-verification/ideal.md` §3–§5 (D1–D16 binding)
**Depends on**: `dv-core` — margins are the functions being differentiated. Phases
1–3 here need only `dv-core` Phase 4 (margin lowering); Phase 4 onward needs
`dv-core` complete.
**Status**: Draft — awaiting approval

---

## Test Coverage Matrix

> Generated from codebase sampling and project guidelines — confirm before Execute.
> Guidelines found: `CLAUDE.md` (fail-loud rule, "zero warnings is the bar", the
> named numeric oracles), `AGENTS.md` (MD-13), `tests/suite_hygiene.rs`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `piperine-codegen` differentiation | integration | 1:1 to spec ACs; **the function-count bound is asserted, not inspected** | `crates/piperine-codegen/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-codegen` |
| `piperine-solver` ABI | integration | analytic and fallback paths both covered; capability-bit gating proven | `crates/piperine-solver/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-solver` |
| `piperine-solver` adjoint driver | integration | **cross-checked against `.sens` within FD tolerance**; linear-solve count asserted | `crates/piperine-solver/tests/*.rs` | same as above |
| `piperine-api` host surface + driver | integration | every policy: happy path, loud refusal, engine attribution | `crates/piperine-api/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-api` |
| `piperine-python` bindings | integration | every Python-visible name; parity with Rust | `crates/piperine-python/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-python` |
| Host surface / cross-crate (MD-28) | integration | optimize/center/MC end to end; compile-once across every loop | `tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` |
| `docs/spec/` (markdown) | none | build gate only | `docs/spec/*.md` | build gate only |

**Numeric oracles for this feature specifically:**

| Claim | Oracle |
|---|---|
| DC gradient correctness | `analyses/sens.rs` (central finite difference) — must agree within the FD's own error |
| Adjoint economy | counted linear solves: 20 parameters ⇒ **one** extra solve |
| Compile-once (MD-18) | `tests/compile_once_sweep.rs`, `tests/urc_compile_count.rs` — unchanged across a full optimize run |
| High-sigma estimate | a long plain Monte Carlo, agreeing within the reported confidence interval |
| Kernel-count bound | a `Mos1`-based circuit (~60 model-card params) compiles; this is the `.disto` wall (`ROADMAP.md` P1, `TryFromIntError` on MOS2/MOS3) |

## Gate Check Commands

> Generated from codebase — confirm before Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | Task touching exactly one crate's internals | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p <crate>` |
| Full | Task touching the host surface, Python, or more than one crate | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` |
| Build | Phase completion, doc-only tasks | `CARGO_PROFILE_DEV_DEBUG=0 cargo build --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo clippy --workspace --all-targets -- -D warnings && CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` |

Same three binding rules as `dv-core`: `CARGO_PROFILE_DEV_DEBUG=0` always (the
workspace test build is ~63 GB without it), the doc gate excludes
`piperine-python`/`piperine-cli` (pre-existing `numpy` rustdoc ICE), and
`--workspace` always.

---

## Execution Plan

### Phase 1: Parameter differentiation
```
T1 → T2 → T3
```

### Phase 2: ABI delivery
```
T4 → T5 → T6
```

### Phase 3: The DC adjoint driver
```
T7 → T8 → T9 → T10
```

### Phase 4: Host sensitivity surface
```
T11 → T12
```

### Phase 5: Statistical sampling
```
T13 → T14 → T15
```

### Phase 6: The policy driver
```
T16 → T17 → T18 → T19
```

### Phase 7: High-sigma and aging
```
T20 → T21
```

### Phase 8: The formal spec
```
T22 → T23
```

---

## Task Breakdown

### T1: Differentiate the resolved form with respect to parameters

**What**: An additive entry point in the symbolic differentiator that
differentiates by a *parameter* instead of an unknown.
**Where**: `crates/piperine-codegen/src/resolve/diff.rs`
**Depends on**: None
**Reuses**: the existing Jacobian differentiation — same pass, different
differentiation variable
**Requirement**: DVG-01
**Care**: `resolve/diff.rs` is the correctness-critical core (CLAUDE.md: "files not
to edit casually"). This task is **additive only** — the existing unknown-differentiation
path and its tests must be untouched and stay green.

**Done when**:
- [ ] a resolved expression can be differentiated w.r.t. a named parameter, returning a resolved expression
- [ ] chain rule through the existing operator set matches hand-derived derivatives on a table of cases
- [ ] the existing Jacobian tests pass unchanged, and no existing function's behavior moved
- [ ] a parameter the expression does not depend on yields exact zero (distinguishable from "not differentiable")

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): differentiate resolved expressions by parameter`

---

### T2: The `∂F/∂p` kernel — one function, parameter-indexed

**What**: Emit parameter derivatives as **one** JIT function taking a parameter
index, alongside the residual and Jacobian.
**Where**: `crates/piperine-codegen/src/kernel/analog/`
**Depends on**: T1
**Reuses**: the residual/Jacobian emission path; CSE
**Requirement**: DVG-02
**Critical**: `ROADMAP.md` P1 records that `.disto`'s per-combination kernels
overrun Cranelift with `TryFromIntError` on MOS2/MOS3. A per-parameter design walks
into the same wall — a `Mos1` model card carries ~60 parameters.

**Done when**:
- [ ] exactly **one** `∂F/∂p` function is emitted per module regardless of parameter count, taking the parameter index as an argument
- [ ] a `Mos1`-based circuit with its full model card compiles — the acceptance test for the wall
- [ ] the function count is **asserted numerically**, not inspected by eye
- [ ] emitted once at compile time; a later `set` does not re-emit

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): emit a parameter-indexed dF/dp kernel`

---

### T3: Admissibility and the compile-once proof

**What**: Only `Real`-typed restamp-class parameters are differentiable; everything
else is loud. And the whole thing respects MD-18.
**Where**: `crates/piperine-codegen/src/kernel/analog/`, `crates/piperine-codegen/tests/`
**Depends on**: T2
**Reuses**: the admissibility rule `analyses/sens.rs` already enforces — one rule,
not a second copy
**Requirement**: DVG-03

**Done when**:
- [ ] a rebuild-class parameter emits no derivative and a later request fails loud
- [ ] a discrete (`Integer`/`Natural`) parameter likewise
- [ ] `AnalogKernel::compile_count()` is unchanged across a `set` loop on a sensitivity-enabled circuit
- [ ] a circuit with **zero** differentiable parameters emits no `∂F/∂p` kernel at all (no cost)

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(codegen): enforce gradient admissibility and compile-once`

---

### T4: `param_stamps` into a caller sink

**What**: `AnalogDevice` grows the one method that hands parameter derivatives to
the solver, writing into a caller-provided sink.
**Where**: `crates/piperine-solver/src/core/element.rs`, `crates/piperine-codegen/src/device/element.rs`
**Depends on**: T2
**Reuses**: `Stamp` (`crates/piperine-solver/src/math/linear.rs:12`); `load_dc`'s
sink pattern — this call sits in the adjoint's inner loop, so no per-call `Vec`
**Requirement**: DVG-04

**Done when**:
- [ ] `param_stamps(param_handle, &mut sink)` on `AnalogDevice`, defaulted to writing nothing
- [ ] the PHDL device implements it by calling T2's kernel with the parameter index
- [ ] no allocation per call — asserted by shape, not by benchmark
- [ ] a parameter an element does not own writes nothing and is not an error

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): add param_stamps to the analog device ABI`

---

### T5: `HAS_SENSITIVITY`

**What**: The capability bit, declared by PHDL-JIT elements and clear for
plugin/OSDI.
**Where**: `crates/piperine-solver/src/core/element.rs`, `crates/piperine-codegen/src/device/element.rs`
**Depends on**: T4
**Reuses**: the `HAS_DISTO2 = 1 << 12` / `HAS_DISTO3 = 1 << 13` precedent
**Requirement**: DVG-05

**Done when**:
- [ ] the bit exists and PHDL-compiled elements set it when they carry a `∂F/∂p` kernel
- [ ] an element without a kernel does **not** set it, even if it is PHDL-compiled
- [ ] plugin/OSDI elements default clear
- [ ] the bit is queryable through the introspection surface

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): declare sensitivity capability on elements`

---

### T6: Stamp-perturbation fallback, always reported

**What**: For elements without the bit, difference their stamps at the *same*
operating point — and say which elements needed it.
**Where**: `crates/piperine-solver/src/analyses/`
**Depends on**: T5
**Reuses**: the restamp path for the parameter write
**Requirement**: DVG-06, DVG-07

**Done when**:
- [ ] two `load_dc` calls at the same operating point per parameter — **no re-solve**, so the adjoint economy survives
- [ ] a test element without the bit coexists with PHDL devices in one circuit and both paths contribute
- [ ] the result names which elements used the fallback, by label
- [ ] the linear-solve count is identical whether an element took the analytic or the fallback path

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(solver): fall back to stamp perturbation without the bit`

---

### T7: The real transpose solve

**What**: A real-valued `Jᵀλ = b` solve — the sibling of the noise adjoint, which
is complex and per-frequency.
**Where**: `crates/piperine-solver/src/math/`, `crates/piperine-solver/src/analyses/`
**Depends on**: None (independent of Phases 1–2; ordered here)
**Reuses**: the already-factored Jacobian; `analyses/noise.rs:240`
`solve_adjoint_system` as the **shape** reference — not as code to share, since it
is complex
**Requirement**: DVG-08

**Done when**:
- [ ] a real transpose solve against a known small system matches a hand-computed λ
- [ ] the existing factorization is reused rather than refactored
- [ ] the noise adjoint is untouched and its tests stay green
- [ ] a singular/ill-conditioned system fails loud through the existing `SolverDomain` error family, never returning garbage

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): add a real transpose solve for adjoint sensitivity`

---

### T8: The adjoint DC driver

**What**: One solve, all parameters: `∂f/∂p = −λᵀ(∂F/∂p)`.
**Where**: `crates/piperine-solver/src/analyses/` (new driver beside `sens.rs`)
**Depends on**: T4, T6, T7
**Reuses**: T7's transpose solve; T4's `param_stamps`; `sens.rs`'s parameter-handle plumbing
**Requirement**: DVG-08

**Done when**:
- [ ] `∂f/∂x` is built for a scalar output from the converged DC solution
- [ ] **one** transpose solve serves every requested parameter
- [ ] a 20-parameter gradient costs exactly one extra linear solve — **counted**, not claimed
- [ ] the driver is a DC analysis variant; the solver still does not depend on codegen

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): add the adjoint DC sensitivity driver`

---

### T9: Cross-check against `.sens`

**What**: The oracle test that makes DC-first (D6) worth it.
**Where**: `crates/piperine-solver/tests/`
**Depends on**: T8
**Reuses**: `analyses/sens.rs` as the finite-difference baseline
**Requirement**: DVG-09, DVG-10

**Done when**:
- [ ] adjoint and `.sens` agree within the finite difference's own error tolerance on an RC and a divider fixture
- [ ] they agree on a **nonlinear** fixture (a diode or MOS circuit), not only linear ones
- [ ] the solve-count assertion from T8 is re-verified end to end
- [ ] the comparison is on values, not on "both ran without error"

**Tests**: integration · **Gate**: quick
**Commit**: `test(solver): cross-check the adjoint against finite differences`

---

### T10: Margin gradients, and the loud refusals

**What**: Gradients of `dv-core` margins — pointwise at the argmin, reduced only if
the reduction is differentiable, AC loud.
**Where**: `crates/piperine-solver/src/analyses/`, `crates/piperine-api/src/`
**Depends on**: T9
**Reuses**: `dv-core`'s margin channel and its pointwise/reduced classification
**Requirement**: DVG-11, DVG-12

**Done when**:
- [ ] a **pointwise** margin's gradient is taken at the located argmin (D1) and the result says so
- [ ] a **reduced** margin's gradient is delivered when the reduction is differentiable (a unity-gain frequency, via the implicit function theorem on `|gain(f)| = 1`) and **fails loud** otherwise
- [ ] an event-detected metric (settling time, overshoot instant) fails loud rather than returning a plausible number
- [ ] an **AC-defined** metric fails loud naming the AC-adjoint gap (D6) — never a silent finite-difference substitution

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): differentiate margins and refuse what it cannot`

---

### T11: `sensitivity` / `gradient` on result objects

**What**: The host reads, with engine attribution.
**Where**: `crates/piperine-api/src/`
**Depends on**: T10
**Reuses**: the result-object family; the `UnknownNet`/`Error::Measurement` error family
**Requirement**: DVG-13

**Done when**:
- [ ] `sensitivity(metric, param)` and `gradient(metric)` return typed values
- [ ] unknown metric or parameter names fail loud **listing candidates**
- [ ] every result names the engine that produced it (adjoint / fallback / finite difference)
- [ ] a metric read from the wrong analysis fails loud, reusing `dv-core`'s "not defined in this analysis"

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): expose sensitivity and gradient on results`

---

### T12: Python parity for the sensitivity surface

**What**: Identical names and values on both hosts, guarded.
**Where**: `crates/piperine-python/src/`, `tests/host_parity.rs`
**Depends on**: T11
**Reuses**: `host_parity.rs`'s enumeration mechanism
**Requirement**: DVG-13

**Done when**:
- [ ] every name added in Phase 4 exists on both hosts with identical spelling and shape
- [ ] one fixture through both hosts asserts identical gradient values **and** identical engine attribution
- [ ] `host_parity.rs` enumerates the new surface

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(python): bind the sensitivity surface`

---

### T13: Sample `tol` distributions over the authored instance tree

**What**: Process draws shared, mismatch draws independent per instance — the
distinction that makes analog yield mean anything.
**Where**: `crates/piperine-api/src/`
**Depends on**: `dv-core` T4 (`Param.distribution()`)
**Reuses**: the authored POM hierarchy walk; the restamp write path
**Requirement**: DVG-16
**Critical**: walk the **authored** instance tree, never `flat_modules` (UNBREAKABLE rule).

**Done when**:
- [ ] a `global` parameter gets **one** draw shared by every instance
- [ ] a plain (mismatch) parameter gets an **independent** draw per instance
- [ ] a two-instance fixture proves both behaviors in one run
- [ ] draws are written through the ordinary restamp path — no new mutation route
- [ ] nothing here reads the flattened form

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): sample declared distributions over the instance tree`

---

### T14: `(seed, index)` reproducibility and replay

**What**: Any sample is fully determined by its seed and index, and replayable as an
ordinary session.
**Where**: `crates/piperine-api/src/`
**Depends on**: T13
**Reuses**: the session/staging path
**Requirement**: DVG-17

**Done when**:
- [ ] the same `(seed, index)` produces byte-identical parameter values across runs and across processes
- [ ] a named failing sample can be replayed as an ordinary session and reproduces its failure exactly
- [ ] a nominal analysis after sampling returns to declared values — sampling leaves no residue
- [ ] sample index is stable under an unrelated design edit? **No** — document that it is not, so nobody treats an index as a permanent name

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): make every Monte Carlo sample replayable`

---

### T15: `monte_carlo` — yield, spread, worst sample

**What**: The reporting surface, and the compile-once proof for the sampling loop.
**Where**: `crates/piperine-api/src/`, `tests/`
**Depends on**: T14, `dv-core` Phase 6 (the margin channel)
**Reuses**: `dv-core`'s margins; the sweep drivers
**Requirement**: DVG-18

**Done when**:
- [ ] yield = fraction of samples with all margins ≥ 0
- [ ] per-metric `σ_i` reported (the input centering needs)
- [ ] the worst sample per constraint is named and replayable
- [ ] 10³ samples cost **one** compile — `compile_once_sweep.rs`-style count asserted
- [ ] a sample that fails to converge is reported as a sample outcome, not an analysis crash, and the run continues

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): report Monte Carlo yield, spread, and worst samples`

---

### T16: The driver skeleton and its policies

**What**: One driver, three policies — the structure that keeps optimize, center,
and high-sigma from becoming three engines that drift.
**Where**: `crates/piperine-api/src/`
**Depends on**: T11, T15
**Reuses**: the restamp loop; `dv-core`'s margin channel
**Requirement**: DVG-14

**Done when**:
- [ ] a policy-parameterized driver exists, with `optimize`/`center`/`high_sigma` as its three policies
- [ ] the driver runs in `collect` posture by construction — an optimizer that aborted on its own infeasible iterates could not search
- [ ] it lives in `piperine-api` so both hosts share one implementation (MD-22)
- [ ] a fixture proves the posture is `collect` even when the session default is `strict`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add the policy-driven optimization driver`

---

### T17: The optimize policy

**What**: Feasibility phase, then objective phase, on gradients.
**Where**: `crates/piperine-api/src/`
**Depends on**: T16
**Reuses**: T11's gradients; T16's driver
**Requirement**: DVG-14

**Done when**:
- [ ] feasibility phase climbs until every `require` margin ≥ 0; objective phase descends while projecting onto the feasible set
- [ ] an objective naming an unknown `var` fails loud at the host boundary
- [ ] a two-parameter fixture with a known optimum converges in ~10¹–10² solves
- [ ] the compile count across the entire optimization is **1** (MD-18)
- [ ] step control tolerates a gradient discontinuity where the argmin switches (D1) rather than stalling

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add the gradient-based optimize policy`

---

### T18: Black-box fallback and engine attribution

**What**: A derivative-free engine for the parts the compiler cannot differentiate,
composed with the gradient path — and always attributed.
**Where**: `crates/piperine-api/src/`
**Depends on**: T17
**Reuses**: T10's loud refusals as the signal for *when* to fall back
**Requirement**: DVG-15

**Done when**:
- [ ] a derivative-free engine (CMA-ES or Bayesian) handles a non-differentiable objective
- [ ] gradient and black-box compose — gradient inside a discrete shell — on a mixed fixture
- [ ] every result names which engine produced it; a mixed run names both
- [ ] scipy/BoTorch, if used on the Python side, are optional accelerators and never the source of truth (the Rust host gets the same answers)

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add a derivative-free fallback engine`

---

### T19: The center policy

**What**: Maximize `min_i (m_i / σ_i)` — worst-case distance, in sigmas.
**Where**: `crates/piperine-api/src/`
**Depends on**: T15, T17
**Reuses**: `dv-core`'s normalized margins; T15's `σ_i`; T17's search
**Requirement**: DVG-19

**Done when**:
- [ ] `center` returns the centered parameters and the worst-case distance in sigmas
- [ ] `σ_i` is held fixed within an outer iteration and re-sampled between them, and the result **states which scheme it used** (the number means different things under each)
- [ ] the `min` over constraints differentiates at the active constraint (D1)
- [ ] on a fixture with two competing constraints: the centered point raises the worst normalized margin **and** a Monte Carlo at that point shows higher yield than nominal

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): add worst-case-distance design centering`

---

### T20: High-sigma importance sampling

**What**: The third policy — tail estimation without 10⁸ samples.
**Where**: `crates/piperine-api/src/`
**Depends on**: T19
**Reuses**: T16's driver; T13's sampling; the continuous margin (what makes a
classifier or an extrapolation possible at all)
**Requirement**: DVG-20 (P3)

**Done when**:
- [ ] an importance-sampling estimator (statistical blockade or scaled-sigma) is implemented as a driver policy
- [ ] every yield number travels with **its estimator and a confidence interval** — a yield without its estimator is not a yield
- [ ] a tail sample is replayable from `(seed, index)`
- [ ] on a bitcell-shaped fixture, the estimate agrees with a long plain Monte Carlo within the reported interval

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): estimate high-sigma yield by importance sampling`

---

### T21: Aging as parameter drift

**What**: Two runs of the same margins, reporting the delta — no new result type.
**Where**: `crates/piperine-api/src/`
**Depends on**: T20
**Reuses**: the restamp path; `dv-core`'s margin channel
**Requirement**: DVG-21 (P3)

**Done when**:
- [ ] the host computes stress from an OP or transient run, restamps drifted parameters, and re-verifies
- [ ] the reported artifact is the **margin delta** — which constraint goes negative first, and after how long
- [ ] a drift that would cross a rebuild-class boundary fails loud rather than silently re-elaborating
- [ ] documented honestly: the gap versus MOSRA/RelXpert is the model *data* (stress equations per device), not the mechanism — the shipped ngspice-faithful models carry none

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): add aging drift and margin-delta reporting`

---

### T22: `docs/spec/` — solver ABI and the adjoint driver

**What**: Document the ABI delta and the driver, including the AC gap as a stated
limitation.
**Where**: `docs/spec/part_vii_solver.md`
**Depends on**: T21
**Reuses**: Part VII's numbered-section structure; §17 (`.sens`) and §12 (noise) as
the neighbours
**Requirement**: DVG-23

**Done when**:
- [ ] `param_stamps` and `HAS_SENSITIVITY` documented in the ABI section
- [ ] the adjoint DC driver documented beside `.sens`, stating explicitly that it is a **sibling** of the noise adjoint (§12) and not a reuse — the optimistic reading is what this line exists to prevent
- [ ] the stamp-perturbation fallback and its reporting contract documented
- [ ] the differentiability admissibility rule stated **once**, shared with `.sens`, not duplicated
- [ ] the **AC-gradient gap is documented as a limitation with its reason** (D6) — a gap the spec names is a decision; a gap it omits is a bug report waiting

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document the sensitivity ABI and adjoint driver`

---

### T23: `docs/spec/` — the host driver surface

**What**: Document the host reads and the three policies.
**Where**: `docs/spec/part_viii_host_api.md`, `docs/spec/appendix_c_host_surface.md`
**Depends on**: T22
**Reuses**: the host-API Part's structure
**Requirement**: DVG-23

**Done when**:
- [ ] `sensitivity`/`gradient` documented with engine attribution
- [ ] `optimize`/`center`/`monte_carlo`/high-sigma documented as **three policies of one driver**, not three features
- [ ] the reproducibility contract (`seed, index`) documented, including that a sample index is not stable across design edits
- [ ] the note that both these documents are absent from every mkdocs nav (`p6-cleanup-architecture` deferred) carried forward — if still open, this documentation is unpublished

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document the host optimization driver`

---

## Validation Tables

### Check 1: Task granularity

Each task is one deliverable: one differentiation entry point, one kernel, one ABI
method, one capability bit, one solve routine, one driver, one policy, or one pair
of spec documents. T10 spans solver and api because the refusal must be consistent
at both ends of one path — still a single concept.

### Check 2: Diagram ↔ `Depends on` cross-check

| Task | Diagram predecessor | `Depends on` | ✅ |
|---|---|---|---|
| T1 | — | None | ✅ |
| T2 | T1 | T1 | ✅ |
| T3 | T2 | T2 | ✅ |
| T4 | T3 (phase order) | T2 | ✅ |
| T5 | T4 | T4 | ✅ |
| T6 | T5 | T5 | ✅ |
| T7 | T6 (phase order) | None (independent; ordered for cohesion) | ✅ |
| T8 | T7 | T4, T6, T7 | ✅ |
| T9 | T8 | T8 | ✅ |
| T10 | T9 | T9 | ✅ |
| T11 | T10 | T10 | ✅ |
| T12 | T11 | T11 | ✅ |
| T13 | T12 (phase order) | `dv-core` T4 | ✅ |
| T14 | T13 | T13 | ✅ |
| T15 | T14 | T14, `dv-core` Phase 6 | ✅ |
| T16 | T15 | T11, T15 | ✅ |
| T17 | T16 | T16 | ✅ |
| T18 | T17 | T17 | ✅ |
| T19 | T18 | T15, T17 | ✅ |
| T20 | T19 | T19 | ✅ |
| T21 | T20 | T20 | ✅ |
| T22 | T21 | T21 | ✅ |
| T23 | T22 | T22 | ✅ |

**Cross-feature dependencies** (T13, T15) are on `dv-core` tasks by number. A batch
worker must confirm those landed before starting Phase 5.

### Check 3: Test co-location

| Task | Layer touched | Matrix requires | Task's `Tests` | ✅ |
|---|---|---|---|---|
| T1, T2, T3 | codegen differentiation | integration | integration | ✅ |
| T4, T5 | solver ABI + codegen device | integration | integration | ✅ |
| T6, T7, T8, T9 | solver adjoint | integration | integration | ✅ |
| T10 | solver + api | integration | integration | ✅ |
| T11 | api host surface | integration | integration | ✅ |
| T12 | python + host parity | integration | integration | ✅ |
| T13, T14, T15 | api sampling | integration | integration | ✅ |
| T16, T17, T18, T19 | api driver | integration | integration | ✅ |
| T20, T21 | api policies | integration | integration | ✅ |
| T22, T23 | `docs/spec/` markdown | none (build gate) | none | ✅ |

---

## Tools

**MCP**: none required. **Skill**: `tlc-spec-driven` (mandatory, per the Execution
Protocol above).
