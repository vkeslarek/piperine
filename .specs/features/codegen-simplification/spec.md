# codegen-simplification Specification

**Vision:** `ideal.md` (sketched 2026-07-31). This spec supersedes it where they
disagree — `ideal.md` was written before `dv-tester` landed and three of its
claims are now stale (§0 below).

**Depends on:** `dv-tester` — DELIVERED 2026-08-01. `piperine-signal` exists
(3 486 lines, purity-guarded), the four families are extracted, and the
cross-check fixtures that hold the two copies in agreement are in place.

---

## 0. What changed since `ideal.md`

Three corrections, each found by reading the code rather than the sketch. They
are recorded because they change the scope, not as trivia.

1. **Capability parity is no longer this feature's deliverable — it landed.**
   `tests/host_capability_parity.rs` already twins four devices (a `transition`
   source, a pnjlim junction, a `table` nonlinearity, a `cross`-triggered
   element) in PHDL and as host devices and asserts equal *values*.
   `ideal.md` §2 presented this as the hard goal; it is now a regression to
   preserve. This feature's deliverable is narrower and more honest: **delete
   the duplicate copies**.

2. **The god-files do not shrink.** `codegen-architecture` (MD-23) already moved
   every extracted family into small files. The four families live in
   `device/analog/operators.rs` (242), `events.rs` (188), `limits.rs` (170) and
   `resolve/pom/analog_ops.rs` (358) — **not** in `compile.rs` (1 747),
   `instance.rs` (1 516), `flatten/analog.rs` (1 329) or `kernel.rs` (1 033).
   `ideal.md` §0's framing ("the four god-files shrink") is wrong.
   The measured targets in CGS-11 are set on the files that actually change.

3. **A fifth family exists, and it shipped with a bug.** A full sweep of
   codegen's 57 files (2026-08-03) found one family `ideal.md` never named:
   **digital edge detection**, duplicated across `device/digital.rs:286,329`
   and `piperine-api/src/sequencer/wait.rs:20` — and the two copies
   **disagree**. Codegen implements IEEE 1364 (`prev != 1 && cur == 1`, so
   `X→1` and `Z→1` are rising edges); the host sequencer implements only
   `Zero→One`. A host tester waiting on a clock's first edge — from the `X`
   every digital net starts at — misses it and stalls a cycle, silently. This
   is the exact failure mode duplication was predicted to cause, found before
   it was reported. It becomes P1 story 5 (user, 2026-08-03).

4. **The limiter is not "already pure" in codegen.** `device/analog/limits.rs`
   is slot bookkeeping and `vcrit` seeding; the pnjlim/fetlim/limvds **formulas
   are emitted as branchless Cranelift IR** (`emit/analog_expr.rs:332-465`,
   ~135 lines). `ideal.md` §1's table calls this move "already pure" — it is
   the one family whose delegation needs a JIT→Rust call. That call path
   **already exists**: `piperine-lang/src/math.rs::MATH_FNS` + the import loop
   at `kernel/analog/compile.rs:148-164`.

---

## Problem Statement

`piperine-codegen` (16 670 lines) carries a second copy of four runtime-semantic
families that `piperine-signal` now owns: the runtime operators, the event
detectors, the junction limiter, and `table`. The duplication is deliberate and
temporary — `dv-tester` extracted without rewiring so a correctness-critical
refactor stayed off its critical path (MD-36) — and it is held together only by
four cross-check fixtures. Every day both copies exist is a day they can drift,
and a drift turns "delete a copy" into "reconcile two behaviors".

## Goals

- [ ] Exactly **one** implementation of each of the **five** families in the
      tree; codegen calls `piperine-signal` and holds no copy. Four are
      delegation of `dv-tester`'s extractions; the fifth (digital edges) is a
      new extraction, and unifying it fixes a live divergence (§0 item 3).
- [ ] **Zero numeric change** — the full suite and the ngspice oracle produce
      identical results.
- [ ] The absence of a second copy is **mechanically guarded**, so one cannot
      quietly reappear.
- [ ] The four touched files hit stated line caps (CGS-11).

## Out of Scope

| Item | Reason |
|---|---|
| `Measure` ⟷ `dv-core` `constraint` unification | Crosses into `dv-core`'s territory; the two-definitions-of-peak debt stays managed (one written semantics + cross-check fixture + shared findings channel, per MD-36). ROADMAP'd, not built here. |
| `kernel/analog/noise.rs` PSD formula → `signal::noise` | **There is no formula there.** The file is 23 lines of three fields holding JIT `AnalogFn` handles; `S(f) = psd·(1/f)^exp` is emitted as IR (`kernel/analog/compile.rs:536`). `signal::noise` would receive nothing. `ideal.md` §1 read the filename, not the file. |
| `kernel/analog/reactive.rs` ddt companion → `signal::companion` | Same shape: 19 lines, two `AnalogFn` handles (`charge`, `charge_jacobian`). The interesting half (`IntegrationMethod::coeffs`) already lives in `piperine-solver`. |
| `cover` bin mapping (`device/analog/coverage.rs:24 bin_of`) | Pure, but **has no second copy** anywhere — the host merges bins that are already mapped. It is a capability *gap* (a host device cannot do `cover`), not drift. Nothing to delete; ROADMAP'd as a parity item. |
| Element LTE hint (`instance.rs:830`) vs solver `TrBdf2::milne_lte` | Not duplication — the ABI deliberately has both: the element supplies a per-charge hint, the solver estimates globally. |
| `device/analog/forces.rs` | Branch-unknown allocation is MNA-shaped. Stays permanently. |
| `flatten/analog.rs` marker-split rules | Lowering rules, not runtime semantics. Stay permanently. |
| Reshaping `compile.rs`/`instance.rs`/`kernel.rs`/`flatten/analog.rs` | `codegen-architecture` is delivered; re-architecting is `ideal.md` §5's named risk. This feature deletes and delegates only. |
| Any performance goal | `ideal.md` §3 is explicit: this buys none. CGS-10 caps the *regression*, it does not promise a gain. |
| Extending `HostElement`'s reach (noise, `.disto`, digital multi-phase) | Named as PHDL-only in `piperine-api/src/device/host_element.rs`; a scoped promise, unchanged here. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Feature scope | Delegation of the four proven families only | Smallest revertible unit; candidates ROADMAP'd | **y** (user) |
| `table` delegation shape | `signal::behavior::Table` owns validation + segment data; codegen builds its `If`-chain **from that validated struct** | One definition of the rules (strictly-increasing `xs`, end clamp, `"linear"`-only mode), two renderings. Symbolic differentiation over the `If`-chain is untouched | **y** (user) |
| Limiter delegation shape | **Full** — codegen's emitted IR is replaced by a JIT call to `signal::limit::{pnjlim,fetlim,limvds}` | Removes the duplication completely rather than guarding it | **y** (user) |
| Acceptance bar | Zero numeric change **+** a delegation-witness test per family | Guards against a copy reappearing later | **y** (user) |
| Line targets | Stated, on the files that actually change | See §0 item 2 — the god-file target is unreachable in this scope | **y** (user, retargeted after measurement) |
| Where the limiter symbols are registered | A **codegen-owned** `LIMIT_FNS` table (mirroring `MATH_FNS`'s shape), registered alongside `MATH_FNS` in `kernel/analog/compile.rs` | Putting them in `piperine-lang::math` would make the *frontend* depend on `piperine-signal`, which it has no reason to. Codegen is the only consumer | n — agent's call |
| Digital JIT registration | Analog JIT only | `$limit` is an analog-only construct; `kernel/digital/compile.rs` registers `MATH_FNS` for expression math, and adding unused imports there is noise | n — agent's call |
| `piperine-lang` header changes | None | `$limit` is already a declared `extern task`; `pnjlim`/`fetlim` are *kind string arguments*, not referenceable names, so MD-24 is unaffected | n — agent's call |
| Dev-dependency cycle | `piperine-codegen` → `piperine-signal` (normal dep) while `piperine-signal` → `piperine-codegen` (dev-dep) | Cargo permits cycles through dev-dependencies. **Must be proven building before any other task** (T1) — if it does not, the cross-check fixtures move out of `piperine-signal/tests/` rather than the delegation being abandoned | n — agent's call, verified in T1 |
| Which edge semantics wins | Codegen's (IEEE 1364: `X→1`/`Z→1` are rising edges) | It is the standard, and it is the one the compiled devices already obey — changing it would change every PHDL simulation; changing the host changes only a V1 host feature that is currently wrong | **y** (user, 2026-08-03) |
| The host's `baseline: None` arming rule | Stays in `piperine-api`, not in the shared detector | It is sequencing (when a wait was armed), not edge semantics. Pushing it down would give the shared detector a seeding rule codegen does not want — the same asymmetry trap `above`-vs-`cross` already documents | n — agent's call |
| Four-state type in `piperine-signal` | `signal` defines its own; both sides convert | Purity guard forbids depending on `piperine-solver` for `LogicValue`, and codegen's `Quad` is an `i64` JIT encoding that has no business crossing a crate boundary | n — agent's call |
| Fate of `cross_check_*.rs` | Deleted, all four | Their stated purpose (`dv-tester` design §C1, and each file's own header) is holding two copies in agreement. With one copy left they assert a function equals itself | n — follows from full delegation |
| `piperine-signal` purity | Unaffected | The purity guard inspects `[dependencies]` only; codegen depending on signal adds nothing to signal's own deps | n — agent's call |

**Open questions:** none — all resolved or logged above.

---

## User Stories

### P1: Runtime operators delegate ⭐ MVP

**User Story**: As a Piperine maintainer, I want `delay`/`slew`/`transition`/
`idt`/`idtmod` to have one implementation, so a fix to `transition`'s re-anchor
rule cannot land in one copy and miss the other.

**Why P1**: The largest single deletion (242 → ≤ 100) and the most mechanical —
`signal::ops::Operator` is codegen's type minus the `slot: usize` field.

**Acceptance Criteria**:

1. WHEN `AnalogInstance` constructs its operators from `RuntimeState` THEN it
   SHALL build `piperine_signal::ops::Operator` values, and
   `piperine-codegen` SHALL define no `Operator` enum of its own.
2. WHEN an operator is advanced on an accepted timestep THEN the state-array
   slot it writes SHALL be carried by codegen (a slot-to-operator mapping
   alongside the signal value), because `signal::ops::Operator` has no slot.
3. WHEN `idt`'s `ic` initial condition is applied at construction THEN the
   resulting integral SHALL equal today's value bit-for-bit
   (`instance.rs:243`'s `Operator::Integrate { slot, value, .. }` seeding path
   has no `signal` equivalent and is codegen's to keep).
4. WHEN a timestep is **rejected** THEN no operator state SHALL be mutated —
   the invariant `signal::ops`'s header declares to be the caller's
   responsibility.

**Independent Test**: `tests/ngspice_validation.rs` and the operator behavior
tests produce identical numbers; `grep` finds no `enum Operator` in
`piperine-codegen`.

---

### P1: Event detectors delegate ⭐ MVP

**User Story**: As a Piperine maintainer, I want one `cross`/`above`/`timer`
detector, so the `above`-vs-`cross` seeding asymmetry is stated once.

**Why P1**: Pure, small, and the asymmetry is exactly the kind of
learned-from-failure rule that a second copy loses.

**Acceptance Criteria**:

5. WHEN an analog event is evaluated at an accepted point THEN the fire decision
   SHALL come from `piperine_signal::events::EventDetector`, and
   `piperine-codegen` SHALL define no `EventDetector` of its own.
6. WHEN a `cross` detector sees its **first** sample THEN it SHALL NOT fire
   (arming only); WHEN an `above` detector sees its first sample and the value
   is `> 0` THEN it SHALL fire. The asymmetry is preserved exactly.
7. WHEN codegen maps its `CompiledTrigger` to `signal::events::Trigger` THEN
   `CompiledTrigger::Initial` SHALL remain codegen's (it is an
   `@initial` dispatch, not a detector) and SHALL NOT be given a `signal`
   counterpart.
8. WHEN a `timer` event's period and phase are known THEN `EventSystem` SHALL
   keep owning the period table and the `@initial`/fired-event variable-action
   dispatch — only the per-event detector delegates.

**Independent Test**: event behavior tests unchanged and passing; `grep` finds
no `struct EventDetector` in `piperine-codegen`.

---

### P1: The junction limiter delegates, formula included ⭐ MVP

**User Story**: As a Piperine maintainer, I want pnjlim/fetlim/limvds to exist
once — as Rust, called from JIT code — so the branchless-IR transcription of
ngspice `devsup.c` stops being a thing that can disagree with the Rust one.

**Why P1**: The only family with the formula duplicated across two *languages*
(Cranelift IR + Rust). It is also the one `ideal.md` mis-scoped, so getting it
right is what makes this feature honest.

**Acceptance Criteria**:

9. WHEN the analog JIT module is built THEN it SHALL register
   `pnjlim`/`fetlim`/`limvds` as imported symbols bound to
   `piperine_signal::limit`'s functions, using the same
   `jit_builder.symbol()` + `declare_function(Linkage::Import)` mechanism
   `MATH_FNS` uses.
10. WHEN `$limit` is emitted THEN it SHALL emit a **call** to the registered
    symbol, and `emit/analog_expr.rs` SHALL contain no `emit_pnjlim`,
    `emit_fetlim` or `emit_limvds`.
11. WHEN the same `$limit` expression is emitted more than once in one kernel
    THEN it SHALL still be CSE'd to a single call under `CseKey::Limit(slot)` —
    delegation SHALL NOT turn one clamp into two calls per Newton iteration.
12. WHEN `$limit` appears inside a body reached by the **parameter-Jacobian**
    kernel or by an inlined `fn` THEN it SHALL emit the same call — every
    emission site delegates, not only the primary residual path.
13. WHEN `$limit` is symbolically differentiated THEN it SHALL remain
    transparent (`d(vnew)/dV`, `resolve/diff.rs:264`) — the call is a value
    site, and the Jacobian pass SHALL be unchanged.
14. WHEN the limiter's per-slot state is advanced THEN the `active` /
    `active_slots` / `seeds` bookkeeping SHALL come from
    `piperine_signal::limit::Limiter`, and `piperine-codegen` SHALL define no
    `Limiter` of its own.
15. WHEN a `$limit` kind string is not one of `pnjlim`/`fetlim`/`limvds` THEN
    codegen SHALL still fail loud with `CodegenError::Unsupported` naming the
    kind — the existing error, unchanged.

**Independent Test**: `tests/ngspice_validation.rs`'s junction devices (dio,
bjt, mos1, jfet) produce identical DC operating points; `grep` finds no
`fn emit_pnjlim` in `piperine-codegen`.

---

### P1: `table` delegates its rules ⭐ MVP

**User Story**: As a Piperine maintainer, I want the `table` rules
(strictly-increasing `xs`, end clamp, `"linear"`-only) written once, so a PHDL
`table` and a host `Table` cannot interpolate differently at an endpoint.

**Why P1**: The one family that is *not* mechanical — codegen needs a
differentiable POM expression, `signal` has a runtime evaluator. Getting the
split right is the whole story.

**Acceptance Criteria**:

16. WHEN codegen lowers `table(x, xs, ys)` THEN it SHALL construct a
    `piperine_signal::behavior::table::Table` first and build its `If`-chain
    from that validated struct's segments — the validation and segment
    geometry SHALL have one definition.
17. WHEN `xs` is not strictly increasing, `xs`/`ys` lengths differ, or the
    interpolation mode is not `"linear"` THEN the failure SHALL be raised from
    `Table`'s own error type and reported as codegen's existing loud error with
    an unchanged user-facing message.
18. WHEN `x` falls outside `[xs[0], xs[n-1]]` THEN the lowered expression SHALL
    clamp to the end value (not extrapolate), matching `Table::value`.
19. WHEN the lowered expression is symbolically differentiated THEN the
    derivative SHALL be the segment slope read off the same piecewise
    structure — `resolve/diff.rs` is unchanged and no hand-written Jacobian is
    introduced.
20. WHEN `xs`/`ys` are not compile-time constant arrays THEN codegen SHALL fail
    loud exactly as today (`Table::const_array` returning `None` keeps its
    current error path) — delegation SHALL NOT make a non-constant table
    silently work or silently fail.

**Independent Test**: `tests/host_capability_parity.rs`'s
`table_nonlinearity_matches_between_phdl_and_a_host_device` still passes
unchanged, and it now compares two bindings of one implementation.

---

### P1: Digital edge detection unifies, on IEEE 1364 semantics ⭐ MVP

**User Story**: As someone writing a host tester, I want `wait(edge(clk, Rising))`
to fire on exactly the transitions PHDL's `@(posedge clk)` fires on, so a
sequencer cannot silently miss a clock edge that the device it is testing saw.

**Why P1**: This is the only family found with the two copies **already
divergent**, and the divergence is a live silent-stall bug in a shipped V1
feature. It is also the one family that is *extraction* rather than delegation —
`dv-tester` never extracted it, so `piperine-signal` has no counterpart yet.

**Acceptance Criteria**:

29. WHEN `piperine-signal` gains an edge detector THEN it SHALL implement IEEE
    1364 semantics — `Rising` fires on `0→1`, `X→1` and `Z→1`; `Falling` fires
    on `1→0`, `X→0` and `Z→0`; `Any` fires on any change — matching codegen's
    current rule, which is the correct one.
30. WHEN `device/digital.rs` evaluates a clocked block's trigger terms (both the
    sequential path at `:286` and the combinational path at `:329`) THEN the
    fire decision SHALL come from the `piperine-signal` detector, and codegen
    SHALL hold no inline edge comparison.
31. WHEN `piperine-api/src/sequencer/wait.rs`'s `EdgeWait::observe` runs THEN it
    SHALL delegate to the same detector, and `EdgeDirection::matches` SHALL be
    deleted.
32. WHEN a host `wait(edge(net, Rising))` is armed and the net's next settled
    transition is `X→1` THEN the wait SHALL be satisfied. This is a
    **deliberate behavior change** to the host side — the only one in this
    feature — and it SHALL have its own test asserting the previously-missed
    edge now fires.
33. WHEN a wait is armed and no prior settled sample exists THEN it SHALL NOT
    fire on the first observation (arming only) — `EdgeWait`'s existing
    `baseline: None` rule is host-side sequencing, SHALL be preserved, and
    SHALL NOT be pushed into the shared detector, which sees only
    `(before, after)` pairs.
34. WHEN codegen's `Quad` encoding (`0,1,2=X,3=Z`) is mapped to the detector's
    input THEN the mapping SHALL be codegen's, and the detector SHALL be stated
    over `LogicValue`-equivalent four-state values, not over codegen's `i64`
    encoding — `piperine-signal` SHALL NOT gain a dependency on
    `piperine-solver`'s `LogicValue` (its purity guard forbids it), so it
    defines its own four-state type and both sides convert.

**Independent Test**: a host sequencer waiting on `Rising` over a net that goes
`X → 1` is satisfied at that transition; the same net driven into a PHDL
`@(posedge)` block fires at the same point.

---

### P1: The delegation is proven and guarded ⭐ MVP

**User Story**: As a Piperine maintainer, I want the "one implementation" claim
to be enforced by the test suite, so it survives the next person who is in a
hurry.

**Why P1**: Without this the feature is a refactor that decays. It is the
difference between deleting a copy and preventing one.

**Acceptance Criteria**:

21. WHEN `cargo test --workspace` runs after delegation THEN it SHALL pass with
    the same test count modulo the four deleted `cross_check_*` targets, zero
    rustc/clippy warnings, and **zero numeric differences** in
    `tests/ngspice_validation.rs`. The **sole** intended behavior change in
    this feature is AC 32's host edge fix; any other observable difference is a
    defect, not a result.
22. WHEN the delegation-witness test for a family runs THEN it SHALL assert
    mechanically that the type/function in use originates in
    `piperine-signal` — one witness per family (operators, events, limiter,
    table, digital edges), **five** in total, the fifth covering both
    `piperine-codegen` and `piperine-api` since that family had copies in two
    crates.
23. WHEN a future change reintroduces a codegen-local copy of a delegated
    family THEN a witness SHALL fail. The witness SHALL be a source-level check
    in the style of `tests/suite_hygiene.rs` (which already walks the repo's
    own sources), not a comment.
24. WHEN delegation is complete THEN
    `crates/piperine-signal/tests/cross_check_{operators,events,limiter,table}.rs`
    SHALL be deleted, and `tests/host_capability_parity.rs` SHALL be left
    untouched (it compares two *bindings*, not two implementations — its own
    header says so).
25. WHEN `piperine-signal`'s purity guard (`tests/purity.rs`) runs THEN it SHALL
    still pass — `piperine-signal` gains no dependency.

**Independent Test**: `cargo test --workspace` green; deliberately re-adding a
local `enum Operator` to codegen makes a witness fail.

---

### P2: Measured shrinkage

**User Story**: As a Piperine maintainer, I want the deletion sized in advance
so "it got simpler" is checkable rather than asserted.

**Why P2**: The correctness stories stand alone; this makes the readability
claim falsifiable. It is P2 because a target missed by a few lines must not
block a correct delegation — it must be *renegotiated in the open* instead.

**Acceptance Criteria**:

26. WHEN delegation is complete THEN these caps SHALL hold:

    | File | Now | Cap |
    |---|---|---|
    | `device/analog/operators.rs` | 242 | ≤ 100 |
    | `device/analog/events.rs` | 188 | ≤ 140 |
    | `device/analog/limits.rs` | 170 | ≤ 130 |
    | `emit/analog_expr.rs` | 618 | ≤ 500 |
    | `resolve/pom/analog_ops.rs` | 358 | ≤ 330 |

27. WHEN the crate is measured THEN `piperine-codegen` SHALL have **≥ 300 net
    lines removed** against its 16 670-line baseline, counting the code this
    feature adds (the `LIMIT_FNS` table, the slot mappings, the witnesses).
28. WHEN a cap is missed THEN the miss SHALL be reported with the actual number
    and the reason, and the cap renegotiated — never met by moving code to
    another file.

**Independent Test**: `wc -l` on the five files; `git diff --stat` on the crate.

---

## Edge Cases

- WHEN the `piperine-codegen` → `piperine-signal` dependency is added while
  `piperine-signal` keeps its dev-dependency on `piperine-codegen` THEN the
  workspace SHALL still build (Cargo permits dev-dependency cycles). If it does
  not, the cross-check fixtures relocate — the delegation does not stop.
- WHEN a `$limit` call is emitted inside a loop-unrolled or `fn`-inlined body
  THEN CSE SHALL still collapse structurally-equal `$limit` expressions to one
  call (AC 11 applied to the inliner's output).
- WHEN `pnjlim`'s branchless IR computed `ln` of a non-positive argument and
  discarded it via `select`, while the Rust version short-circuits and never
  calls `ln` THEN the returned value SHALL be identical — the discarded NaN was
  never observable. (This is the one place where "the same formula" needs an
  argument rather than an assertion.)
- WHEN `vte == 0` THEN `dv / vte` is `±inf` in both renderings and the guard
  `dv.abs() > 2.0 * vte` decides the result the same way.
- WHEN a device declares zero `$limit` slots THEN no limiter symbol lookup SHALL
  be forced and the zero-limit fast paths SHALL behave as today.
- WHEN a `table` has exactly two points THEN the single segment SHALL still be
  built and clamped at both ends.
- WHEN `@initial` runs before `Limiter::seed` THEN the construction order in
  `AnalogInstance::new` SHALL be preserved exactly (`limits.rs`'s header
  documents this two-step split as deliberate).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| CGS-01 | P1: Operators (AC 1–2) | Design | Pending |
| CGS-02 | P1: Operators (AC 3–4) | Design | Pending |
| CGS-03 | P1: Events (AC 5–6) | Design | Pending |
| CGS-04 | P1: Events (AC 7–8) | Design | Pending |
| CGS-05 | P1: Limiter symbols + call (AC 9–11) | Design | Pending |
| CGS-06 | P1: Limiter all emission sites + diff (AC 12–13) | Design | Pending |
| CGS-07 | P1: Limiter bookkeeping + loud kind (AC 14–15) | Design | Pending |
| CGS-08 | P1: Table validation + geometry (AC 16–18) | Design | Pending |
| CGS-09 | P1: Table diff + const-array failure (AC 19–20) | Design | Pending |
| CGS-10 | P1: Zero numeric change (AC 21) | Design | Pending |
| CGS-11 | P1: Witnesses + fixture deletion + purity (AC 22–25) | Design | Pending |
| CGS-12 | P2: Measured shrinkage (AC 26–28) | Design | Pending |
| CGS-13 | P1: Edge detector extraction, IEEE 1364 (AC 29–31, 34) | Design | Pending |
| CGS-14 | P1: Host edge fix + arming rule (AC 32–33) | Design | Pending |

**Coverage:** 14 requirements, 34 ACs. Mapping to tasks: pending Tasks phase.

---

## Success Criteria

- [ ] `grep` over `piperine-codegen` finds no local `Operator`, `EventDetector`,
      `Limiter`, `emit_pnjlim`/`emit_fetlim`/`emit_limvds`, `Table` validation
      or inline edge comparison; `piperine-api` has no `EdgeDirection::matches`
      — and a test enforces each.
- [ ] A host `wait(edge(net, Rising))` fires on `X→1`, and a test names that
      transition explicitly.
- [ ] `tests/ngspice_validation.rs` reports numbers identical to the
      pre-delegation run, junction devices included.
- [ ] `cargo test --workspace` green, zero warnings, four `cross_check_*`
      targets gone and nothing else removed.
- [ ] The five capped files are at or under their caps; ≥ 300 net lines removed.
- [ ] `piperine-signal/tests/purity.rs` still passes.
