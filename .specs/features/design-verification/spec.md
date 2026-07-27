# design-verification Specification (Tier 1)

Verification, optimization, and centering read **one declaration**. This
feature lands the Tier-1 foundation of `ideal.md` (the vision document in
this directory): two grammar additions — `tol` on `param` and the
`constraint` block — plus the kernel/ABI machinery that makes their margins
evaluated values with provenance, and the analytic-gradient path
(`∂F/∂p` kernel + adjoint DC sensitivity) that makes those margins usable by
a host-side optimizer.

## Problem Statement

Today a Piperine user cannot say "this circuit is correct" anywhere except a
Python testbench. SOA/ERC knowledge cannot ship with a device model, spec
compliance is re-derived per testbench, and the only sensitivity path
(`.sens`, Part VII §17) costs two full DC re-solves per parameter via finite
differences. Meanwhile the compiler already differentiates symbolically
(`resolve/diff.rs`) and the session already restamps without re-elaborating
(MD-18). The missing pieces are: a declaration site for correctness intent
that composes with hierarchy, a runtime-evaluated **margin** value with
provenance, a universal findings channel on the `Element` ABI, and parameter
derivatives delivered through that ABI.

## Goals

- [ ] `tol` clause on `param`: distribution declared on the parameter,
      visible in the POM, inert at solve (nominal analyses deterministic).
- [ ] `constraint` block: `require`/`var`/`target` statements compiled into
      the kernel, evaluated per accepted solver point.
- [ ] Signed-margin convention (`m ≥ 0` ⟺ satisfied) with `tol`-normalized
      targets; worst-margin + argmin (time, instance) reported per analysis.
- [ ] Three check postures — `strict` (default, fail loud), `collect`, `off`
      (zero cost) — as a `Solver`/`SolverConfig` knob on both hosts.
- [ ] `validation_reports()` on the `Element` ABI: one findings channel with
      `Warning`/`Error` severity, consumed by constraint kernels now and by
      monitor/OSDI/tester devices by construction.
- [ ] SOA constraint blocks on `headers/spice/` device models: every
      instantiated MOSFET carries oxide/bulk limits, checked for free.
- [ ] Analytic `∂F/∂p`: a third JIT kernel differentiated w.r.t. parameters,
      delivered via `param_stamps` + `HAS_SENSITIVITY`, consumed by an
      adjoint DC sensitivity driver reusing the noise solve shape, with a
      stamp-perturbation fallback for elements without the bit.
- [ ] Host reads: margins, per-constraint worst values, and adjoint
      gradients on result objects — identical on Python and Rust.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep; each item names its
owning follow-up (`ideal.md` §8).

| Feature | Reason |
|---------|--------|
| Temporal/scoped event blocks in constraints (`@ tran(after=…, dur=…)`) | Tier 2 item 9 — Tier 1 requires are unscoped or analysis-scoped only |
| `cover` bins and coverage closure | Tier 3 item 10 |
| Monitor modules as a documented pattern; `warn` statement kind | Tier 3 item 11 — the ABI channel already carries `Warning` severity, so no blocker |
| Host optimizer, `monte_carlo`, `center` drivers | Tier 2 items 6–8 — host code on the margins/gradient primitives delivered here |
| Statistical *sampling* of `tol` distributions | Tier 2 item 7 — Tier 1 declares and stores distributions only |
| Transient adjoint (backward-in-time) | `ideal.md` §3.4 — DC adjoint only here; transient adjoint needs trajectory checkpointing design |
| RNM, `behavioral` body kind, high-sigma, aging, tester library | Tier 3 items 12–15 |
| Renaming the target's `tol` keyword to `scale` | Deliberate rhyme (`ideal.md` §2.2); revisit only if review rejects it |

---

## Assumptions & Open Questions

Every ambiguity is resolved or recorded here — nothing is left silently
unclear. Items marked **n** are the review surface for the next pass.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Margins travel as their own result channel, not waveform rows (`ideal.md` §9.3) | `MarginsResult`-shaped data on `OpResult`/`Trace`: per-constraint worst value + argmin (time, instance) | Margins are per-step scalars with provenance — closer to introspection than to a waveform; the nine-type taxonomy rule says separate when operations differ | n (agent) |
| Unscoped `require` default (`ideal.md` §9.2) | Holds in **every** analysis; noise absorbed by `collect`/`off` postures and by analysis-scoped event blocks (Tier 2) | Default-off lets a real violation hide; default-on is loud but never silently wrong | n (agent) |
| Non-smooth worst-case differentiation (`ideal.md` §9.1) | Differentiate at the located argmin; documented in the result object | Standard practice, correct a.e.; softmin would silently change what the optimizer believes about feasibility | n (agent) |
| Constraints under monomorphization (`ideal.md` §9.4) | POM holds the authored `constraint` block (one per module, UNBREAKABLE); evaluation form (per-instance margin kernels) is a codegen artifact | Authored form stays walkable; `urc__5`-style variants must not multiply the declaration | n (agent) |
| `tol` spelling shared by param distributions and target scales | Keep both | Deliberate rhyme — both express "how much slack exists" (`ideal.md` §2.2) | y (user, in ideal.md) |
| `var` (not `measure`) names computed scalars in constraint blocks | Keep `var` | Same keyword, same meaning; zero new vocabulary (user decision) | y (user) |
| Objective lives host-side (`optimize(objective="idd")`), no `minimize` statement | Keep host-side | Objective is per-run intent, not model truth; optimization scripts run once (user decision) | y (user) |
| Distribution arguments may reference sibling params (`sigma = 0.005/sqrt(w*l)`) | Allow, as elaboration-constant expressions in module param scope | Matches how parameter defaults already evaluate | n (agent) |
| `tol` restricted to `Real` params | Reject discrete params loud | Discrete variation is a different mechanism (`ideal.md` §3.4) | n (agent) |
| Findings channel polls after each accepted tran point, converged DC point, and AC frequency point | Poll-based (like `limiting_report()`), not sink-based | Precedent exists; `off` posture = never polled = zero cost | n (agent) |
| Adjoint driver lives in `piperine-solver` as a DC analysis variant | New driver reusing the noise adjoint solve shape (Part VII §12) | Solver never depends on codegen; `∂F/∂p` arrives through the element ABI | n (agent) |
| Differentiable = `Real`-typed + restamp-class invalidation | Same admissibility rule as `.sens` (Part VII §17); rebuild-class/discrete → loud error | One rule, already enforced in the tree | y (follows existing rule) |

**Open questions:** none blocking — the five `ideal.md` §9 questions are
answered above as logged assumptions awaiting review confirmation; the RNM
`behavioral` question belongs to Tier 3 and is out of scope.

---

## User Stories

### P1: `tol` on `param` — declared, inert, POM-visible ⭐ MVP

**User Story**: As a model author, I want to write
`param vto : Real = 0.7 tol gauss(sigma = 0.005/sqrt(w*l));` so that
statistical variation ships with the parameter it perturbs and any host can
enumerate it without simulating.

**Why P1**: Cheapest item in the vision and the anchor for all later
statistics (MC, centering). Pure declaration — no solve behavior.

**Acceptance Criteria**:

1. WHEN a `param` declaration carries `tol <dist>(args) [global]` with a
   distribution declared in `headers/statistics.phdl` THEN elaboration SHALL
   succeed and the POM `Param` node SHALL expose the distribution
   (`Param.distribution()`), including the `global` (process) vs default
   (mismatch) marker.
2. WHEN a distribution name is not declared in `headers/statistics.phdl`
   THEN elaboration SHALL fail loud naming the distribution (MD-24 — no
   Rust-side magic names; the `extern_coverage_guard` extends).
3. WHEN a `tol` clause appears on an `Integer`/`Natural` (non-`Real`) param
   THEN elaboration SHALL fail loud.
4. WHEN a nominal `op()`/`tran()`/`ac()` runs on a design with `tol` clauses
   THEN results SHALL be bit-identical to the same design without them
   (inert by default — no secret resampling).
5. WHEN distribution arguments reference sibling params THEN they SHALL
   evaluate as elaboration-constant expressions in module param scope.

**Independent Test**: elaborate a module with `tol` params; reflect the POM
and read the distribution back; run `op()` and compare against the untoleranced
twin.

---

### P1: `constraint` block — grammar and POM ⭐ MVP

**User Story**: As a circuit author, I want a `constraint Mod { … }` block
with `require`/`var`/`target` statements attached to my module by name, so
that correctness intent is authored structure, walkable in the POM like
`analog`/`digital` bodies.

**Why P1**: The declaration site everything else reads.

**Acceptance Criteria**:

1. WHEN a `constraint` block names an existing module and contains
   `require <name> : <pred>;`, `var <name> = <expr>;`, and
   `target <expr> <cmp> <level> [tol <scale>];` statements THEN parse +
   elaboration SHALL succeed and the module's POM node SHALL expose the
   block (`Module::constraints`) in authored form.
2. WHEN a constraint references an unknown net, port, instance path, or
   `var` THEN elaboration SHALL fail loud naming the reference and use site
   (never a silent `0.0`).
3. WHEN a module is monomorphized (`Dac[8]` → `Dac__8`) THEN the POM SHALL
   still hold exactly one authored constraint block per module — variants
   must not multiply the declaration (UNBREAKABLE rule).
4. WHEN a `require` predicate is not Boolean-typed, or a `target`
   comparison uses a non-`Real` operand THEN elaboration SHALL fail loud.
5. WHEN a `var` expression calls an analysis-specific helper (e.g.
   `ac_gain`) THEN the `var` SHALL be marked defined only in that analysis;
   a host read from another analysis SHALL fail loud ("not defined in this
   analysis").

**Independent Test**: `piperine check` on a fixture with a constraint block;
selector walk `//Ota` showing the authored block; negative fixtures for
unknown refs and type errors.

---

### P1: Margin evaluation in the kernel, with provenance ⭐ MVP

**User Story**: As a verification engineer, I want every `require` and
`target` evaluated at each accepted solver point — failing loud in strict
posture with the instance, time, and value — so that a violation is a typed
event, not a line in a transcript.

**Why P1**: The runtime half of the declaration; unlocks SOA-on-models.

**Acceptance Criteria**:

1. WHEN a comparison `a <= b` / `a >= b` / `a in [l,u]` lowers THEN the
   emitted margin SHALL be the signed, scale-normalized form of `ideal.md`
   §2.2 (`scale = 1` without `tol`), and `m ≥ 0` ⟺ satisfied.
2. WHEN a transient runs with `checks=strict` (default) and a `require`
   margin crosses below zero at an accepted point THEN the analysis SHALL
   fail loud naming the constraint, the instance path, the time, and the
   margin value.
3. WHEN the same run uses `checks=collect` THEN the analysis SHALL complete
   and the result SHALL record the worst margin per constraint with its
   argmin (time + instance path).
4. WHEN `checks=off` THEN constraint kernels SHALL not be called (zero
   per-point cost — verified by a circuit without constraints paying exactly
   nothing, matching the `forces.rs`/`limits.rs` Option-sub-struct pattern).
5. WHEN a margin is evaluated in DC or at an AC frequency point THEN the
   result SHALL carry the constraint value at that point (no time argmin;
   the point identity instead).
6. WHEN the `checks` knob is read on either host THEN Python `Solver` and
   Rust `SolverConfig` SHALL expose the identical field (HOST-20 parity).

**Independent Test**: a divider fixture with `require vout_low :
V(out,gnd) >= 0.1;` — strict run fails naming instance/time/value; collect
run reports `worst = -0.05` with argmin; off run is clean.

---

### P1: `validation_reports()` — the universal findings channel ⭐ MVP

**User Story**: As a device author (PHDL, plugin, or OSDI), I want one ABI
hook through which any element reports structured findings with severity, so
that constraint violations, model self-checks, and future monitor/tester
devices all speak the same language.

**Why P1**: Without it, PHDL constraints are a privileged path and
plugin/OSDI device checks stay second-class (`ideal.md` §2.6).

**Acceptance Criteria**:

1. WHEN `Element` grows `validation_reports()` (default empty) THEN it SHALL
   be cross-cutting (not `AnalogDevice`-gated) and SHALL return structured
   findings: severity (`Warning` | `Error`), label, message, value, time,
   instance path.
2. WHEN a `require` margin crosses zero THEN the constraint kernel SHALL
   emit an `Error` finding through this channel (margins are values;
   findings are events — one feeds the other).
3. WHEN the solver polls findings in `strict` posture and an `Error` finding
   exists THEN the analysis SHALL fail loud; in `collect` it SHALL record
   (`r.violations`); in `off` the hook SHALL never be polled.
4. WHEN a digital-only element emits a finding THEN it SHALL be collected
   identically (the channel serves the digital scheduler's accepted points).
5. WHEN an element declares no validation capability THEN its default empty
   report SHALL cost nothing (no allocation, no call overhead beyond the
   poll, consistent with defaulted `Element` methods).

**Independent Test**: a recording test element emitting a `Warning` and an
`Error` at known times; assert strict aborts on the `Error`, collect records
both with provenance, off sees nothing.

---

### P1: SOA constraints on `headers/spice/` models ⭐ MVP

**User Story**: As a designer who never writes a `constraint` block, I want
every MOSFET I instantiate to carry its own oxide and junction limits, so
SOA checking is free in every design.

**Why P1**: Highest industrial value per line of code in the vision
(`ideal.md` §2.5) — the foundry-deck placement, but with first-class
margins instead of warning text.

**Acceptance Criteria**:

1. WHEN `headers/spice/mos.phdl` gains a `constraint Mos1` block with
   `vgs_ox`/`vds_ox`/bulk-junction requires reading `vgs_max`/`vds_max` from
   the model card THEN every existing fixture and example using `Mos1`
   SHALL still elaborate and simulate unchanged in default posture (the
   shipped limits are consistent with the gallery's operating points).
2. WHEN a fixture drives a `Mos1` past `vds_max` in strict posture THEN the
   transient SHALL fail loud naming the device instance and the violated
   constraint.
3. WHEN `m1.region` is referenced (constraint or host) THEN the device SHALL
   expose an operating-region opvar (cutoff/triode/saturation) through the
   existing introspection surface — an opvar addition, not a language
   change.
4. WHEN other `headers/spice/` models gain their blocks (diode, bjt) THEN
   the pattern SHALL be identical: limits as model-card params, requires in
   the model's own constraint block.

**Independent Test**: gallery fixture swept into SOA violation under
`collect`; assert the expected margins go negative on the named instances
while a legal fixture reports all-clear.

---

### P1: Analytic `∂F/∂p` — kernel, ABI delivery, adjoint DC driver ⭐ MVP

**User Story**: As an optimization-loop author, I want
`r.sensitivity("gain_db", "w1")` to cost one adjoint solve — not two DC
re-solves per parameter — so 50 sizing knobs cost one extra solve, not 100
simulations.

**Why P1**: The defensible differentiator (`ideal.md` §3); the whole
centering story collapses without it. Delivered in the smallest possible ABI
delta: one method, one capability bit, one documented fallback.

**Acceptance Criteria**:

1. WHEN codegen compiles a circuit with differentiable params (`Real`-typed,
   restamp-class) THEN it SHALL emit a `∂F/∂p` kernel alongside residual and
   Jacobian — at compile time, once (MD-18: a later `set` never re-emits).
2. WHEN the solver asks an element for parameter stamps THEN
   `AnalogDevice::param_stamps(param)` SHALL return them only if the element
   declares `HAS_SENSITIVITY`; PHDL-JIT elements declare it, plugin/OSDI
   elements default clear.
3. WHEN the adjoint DC driver computes `∂f/∂p` for a scalar output THEN it
   SHALL solve `Jᵀλ = ∂f/∂x` once (reusing the noise adjoint solve shape)
   and combine with `∂F/∂p` for **all** requested parameters from that one
   solve.
4. WHEN an element lacks `HAS_SENSITIVITY` THEN the driver SHALL fall back
   to stamp perturbation (two `load_dc` calls at the **same** operating
   point per parameter — no re-solve) and SHALL report which elements used
   the fallback (never a silent accuracy change).
5. WHEN a gradient is requested through a rebuild-class or discrete
   parameter, or for a metric that cannot be differentiated through the
   solver THEN the request SHALL fail loud (same admissibility rule as
   `.sens`, Part VII §17) — never a plausible wrong number.
6. WHEN adjoint and `.sens` (finite-difference baseline) both compute a DC
   sensitivity on a differentiable circuit THEN they SHALL agree within the
   finite-difference's own error tolerance.
7. WHEN the compile-once guard (`compile_once_sweep.rs`-style) runs on a
   constraint+sensitivity-enabled circuit THEN the compile count SHALL be
   unchanged by sensitivity requests and `set` loops.

**Independent Test**: RC/divider fixture — adjoint gradient of `V(out)` w.r.t.
two params matches `.sens` within FD tolerance, at one adjoint solve (count
linear solves), with the fallback reported on a non-PHDL test element.

---

### P2: Host surface — margins and gradients on result objects

**User Story**: As a Python (or Rust) host author, I want
`r.margins["headroom"]`, `r.violations`, `r.sensitivity(metric, param)`, and
`r.gradient(metric)` with identical names and shapes on both hosts, so the
verification currency is a query, not log scraping.

**Why P2**: Useless without the P1 machinery; the P1 machinery is verifiable
without it (solver-level tests).

**Acceptance Criteria**:

1. WHEN an analysis completes in `collect` or `strict` posture THEN the
   result SHALL expose every constraint's worst margin + argmin and every
   finding, with identical member names on Python and Rust (parity enforced
   in the `host_parity.rs` style).
2. WHEN `sensitivity(metric, param)` / `gradient(metric)` are called THEN
   they SHALL trigger the adjoint path and return typed values; unknown
   metric/param names SHALL fail loud listing candidates (`UnknownNet` /
   `Error::Measurement` family).
3. WHEN a `var` is read from an analysis where it is not defined THEN the
   read SHALL fail loud ("not defined in this analysis").

**Independent Test**: one fixture driven through both hosts asserting
identical margins/violations/gradient values.

---

### P2: Analysis-scoped event blocks (`@ dc` / `@ tran` / `@ ac`)

**User Story**: As a constraint author, I want `@ dc { require … }` scoping
via the existing `EventBlock` production, so a DC-only headroom rule does
not trip during a transient startup ramp.

**Why P2**: The unscoped-default assumption (above) is livable without this,
but noisy; window args (`after=`/`dur=`) stay Tier 2 regardless.

**Acceptance Criteria**:

1. WHEN a constraint body contains `@ dc { require h : …; }` THEN the
   require SHALL be evaluated only in DC analyses; `@ tran`/`@ ac` likewise;
   `@ (dc | tran)` in both.
2. WHEN a `require` appears at top level (no block) THEN it SHALL hold in
   every analysis (per the logged default).
3. WHEN an event-block-scoped require's analysis never runs THEN the host
   SHALL see "not exercised" for that constraint — never a silent vacuous
   pass.

**Independent Test**: fixture whose `@ dc` rule is violated during tran
startup but legal at DC — strict tran passes, strict dc fails.

---

## Edge Cases

- WHEN a constraint references a hierarchical path (`u3.m1.d`) THEN
  resolution SHALL go through the authored instance tree (UNBREAKABLE), and
  an unknown path SHALL fail loud at elaboration.
- WHEN a `target` omits `tol` THEN `scale = 1` (unnormalized signed
  distance) — no implicit normalization.
- WHEN two constraints share a name in one block THEN elaboration SHALL
  fail loud (duplicate label).
- WHEN a margin expression is non-finite (NaN/Inf) at an accepted point
  THEN it SHALL be treated as a violation-worth loud report, never silently
  clamped (mirrors §VII-15.10 linear-solver safety).
- WHEN `checks` posture flips mid-session THEN the next analysis SHALL
  honor it with no recompilation (posture is runtime config, not compiled).
- WHEN a circuit has zero constraint blocks THEN every analysis SHALL be
  bit-identical to today (Option sub-struct — no constraints, no cost).
- WHEN a param carries both `tol` and a staged override THEN the override
  SHALL win for the nominal value and the distribution SHALL stay attached
  (POM staging rules unchanged).
- WHEN a plugin element emits a `Warning` finding in strict posture THEN
  the analysis SHALL continue (warnings never abort; only `Error` does).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| VER-01 | P1: `tol` grammar clause + parse | - | Pending |
| VER-02 | P1: `headers/statistics.phdl` + POM `Param.distribution()` | - | Pending |
| VER-03 | P1: `tol` inert at solve (deterministic nominal) | - | Pending |
| VER-04 | P1: `constraint` block grammar (`require`/`var`/`target`) | - | Pending |
| VER-05 | P1: POM `Module::constraints`, authored, monomorph-safe | - | Pending |
| VER-06 | P1: margin lowering (signed, normalized) | - | Pending |
| VER-07 | P1: kernel evaluation per accepted point + argmin provenance | - | Pending |
| VER-08 | P1: postures strict/collect/off, both hosts | - | Pending |
| VER-09 | P1: `validation_reports()` ABI + `ValidationFinding` | - | Pending |
| VER-10 | P1: findings on result objects (`violations`, strict abort) | - | Pending |
| VER-11 | P1: spice-model SOA blocks + region opvar | - | Pending |
| VER-12 | P1: `∂F/∂p` kernel (resolve/diff w.r.t. params) | - | Pending |
| VER-13 | P1: `param_stamps` + `HAS_SENSITIVITY` | - | Pending |
| VER-14 | P1: adjoint DC sensitivity driver | - | Pending |
| VER-15 | P1: stamp-perturbation fallback + reporting | - | Pending |
| VER-16 | P2: host margins/gradient surface, both hosts | - | Pending |
| VER-17 | P2: analysis-scoped event blocks in constraints | - | Pending |
| VER-18 | cross-cutting: fail-loud catalog (edge cases above) | - | Pending |
| VER-19 | cross-cutting: compile-once guard unchanged (MD-18) | - | Pending |

**Coverage:** 19 total, 0 mapped to tasks (tasks phase not started — spec
under review), 19 unmapped ⚠️ (expected at this stage)

---

## Success Criteria

- [ ] A MOSFET-driven fixture violates `vds_max` and fails loud in strict
      posture naming instance + time + margin — with **zero** user-written
      constraint code (SOA ships with the model).
- [ ] The same fixture under `collect` reports normalized margins for every
      `require`/`target` with argmin provenance.
- [ ] `r.sensitivity(...)` on a differentiable circuit matches `.sens` within
      FD tolerance at one adjoint solve; a 20-parameter gradient costs one
      extra linear solve, not 40 re-solves.
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace`
      green, including new fail-loud and parity guards.
- [ ] A circuit with no constraints and no `tol` is bit-identical in cost
      and results to pre-feature behavior.
