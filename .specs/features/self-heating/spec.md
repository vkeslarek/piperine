# Self-Heating Device Pattern Specification

> **Was** `internal-unknown-allocation`. Renamed because the original spec
> framed the problem as *missing allocation plumbing* for internal (non-port)
> analog unknowns — but that plumbing **already exists and ships in
> production** (the BJT model uses 3 internal nodes; see "Already Solved"). No
> new solver or codegen capability is needed for the PHDL path. The only
> remaining work is **authoring** a self-heating device model in PHDL and
> validating it. Surfaced during the BSIM-models port (BSIM4/BSIMSOI
> `rth0`/`cth0` self-heating); the ngspice `HAS_INTERNAL_UNKNOWNS`/plugin
> auxiliary-node path is a **separate** gap (ROADMAP P2), not this.

## Problem Statement

Piperine has no shipped device that models **dynamic self-heating**: a device
whose temperature rises with instantaneous dissipated power and feeds that
temperature back into its temperature-dependent parameters (ngspice/BSIM
`rth0`/`cth0` self-heating). Today's `mos.phdl`/`bjt.phdl` expose only a
**static** `dtemp` parameter (a fixed offset), not a solved `ΔT`.

This is **not** a capability gap — every primitive needed already works
(see below). It is a **modeling/authoring** gap: no header expresses the
thermal RC + power-source + temperature-feedback pattern, and no test
validates it.

## Already Solved (do not re-implement)

The original spec's entire premise — "no PHDL-authored device uses internal
unknowns; there is no allocation path" — is **false**. Verified in code:

| Capability | Where | Status |
| ---------- | ----- | ------ |
| Internal (non-port) analog node → fresh MNA unknown | `circuit.rs:419-435` — every kernel terminal not bound to a parent net gets a `NodeIdentifier::Anonymous`; the allocation is automatic | Works. |
| Multiple internal nodes per leaf device | `bjt.phdl` declares `wire cp/bp/ep : Electrical` (collector'/base'/emitter' series-R nodes) — 3 internal unknowns, ngspice-validated | Works in production. |
| Arbitrary flow contribution into any node | `diode.phdl:446` `I(pp, n) <+ cd;` (cd an arbitrary current expression) | Works. |
| `ddt` companion (thermal capacitance) | `diode.phdl:447` `I(pp, n) <+ ddt(qtotal);` | Works. |
| Reading a solved internal node voltage in a var | `diode.phdl:285` `var vd : Real = V(pp, n);` | Works. |
| Runtime-value temp-dependent params | `mos.phdl`/`bjt.phdl` temperature preprocessing (`tnom`, static `dtemp`) | Works. |

**`HAS_INTERNAL_UNKNOWNS` is a different mechanism** and is *not* on the PHDL
path: it flags solver-**native** elements (plugin/OSDI/composed — see
`parity_baseline.rs`, `composed_element.rs`, `builder.rs` `allocate_unknowns`)
that own extra MNA rows beyond their terminals. PHDL devices never set it —
their internal wires expand to anonymous nets at build time. The original spec
conflated the two; the PHDL half needs nothing.

The lesson: **do not build an allocation path — it exists.** Self-heating is
authorable *today* by composing the primitives above.

## Goals

- [ ] Ship a self-heating device model in PHDL that composes the existing
      primitives — internal thermal node (`wire`), `rth0` resistor, `cth0`
      `ddt` capacitor, `Pdiss` current source, and `V(thermal)` fed back into
      the temperature-dependent parameter evaluation — with no new solver or
      codegen capability.
- [ ] Validate it against the closed-form RC thermal step response and, where
      ngspice exposes the same `rth0`/`cth0` option, against ngspice.
- [ ] Self-heating off (`rth0 = 0`) reproduces today's static-`dtemp`-only
      behavior exactly, with **zero** allocated internal node and zero solver
      overhead (const-folded away at elaboration).

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| **Internal-unknown allocation plumbing** | Already exists (`circuit.rs:419`, BJT's 3 nodes). This feature consumes it. |
| **`HAS_INTERNAL_UNKNOWNS` on the PHDL path** | Not used by PHDL devices at all — it is the plugin/OSDI/native-element mechanism. Irrelevant here. |
| Variable-count internal nodes | `hierarchy-flattening` feature — fixed count here. |
| External/plugin (OSDI) internal-node allocation | ROADMAP P2 separate blocker — the plugin path, real follow-up, reuses solver-side numbering, not built here. |
| Multi-stage thermal ladder (BSIM6) | One lumped Rth/Cth stage covers BSIM4/BSIMSOI; multi-stage is parametric-structure (hierarchy-flattening) or a P3 follow-up. |
| A dedicated "thermal discipline" type | Solver is domain-uniform; the thermal node is just another real unknown. A `thermal` discipline alias in headers is cosmetic-only, optional. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | --------------- | --------- | ---------- |
| No new language/solver surface | Self-heating is authored entirely from existing `wire` + `I(x) <+ expr` + `ddt` + `V(x)` + `$temperature` | All five verified working in `diode.phdl`/`bjt.phdl`/`circuit.rs` | y (code) |
| `Pdiss` expression | Author writes the dissipated-power sum by hand (Σ I·V over the device's resistive branches), mirroring BSIM VA's explicit `Pwr` contribution — not a compiler-synthesized quantity | Matches the reference model; keeps the compiler free of a special "total dissipation" builtin | y (agent, from BSIM model shape) |
| Off-path const-folding | `rth0 == 0` guarded by the existing `StructuralIf`/const-fold so the thermal `wire` + stamps vanish at elaboration | The `if (rth0 > 0) { ... }` pattern already const-folds (module.rs `StructuralIf`); no runtime cost | y (code — StructuralIf exists) |
| Which device ships it first | A minimal dissipative element (resistor-like) for the clean analytic proof, then optionally wire into `mos`/`bjt` | Isolates the thermal-network correctness from full BSIM temperature physics for the first validation | n (Design/authoring choice) |
| Thermal node introspection | The anonymous internal node is already introspectable via `core/introspect.rs` like the BJT's `cp/bp/ep`; a debug label (`<instance>.dtemp`) is a nice-to-have | Consistent with existing internal-node introspection; no new path | y (agent, follows BJT) |

**Open questions:** none capability-level. The only decisions are authoring
choices (which device first, exact header naming) — Design/authoring phase.

---

## User Stories

### P1: Self-heating device model ⭐ MVP

**User Story**: As an analog/power designer, I want a device that heats up
with dissipated power and feeds temperature back into its parameters, so
high-dissipation simulation matches ngspice's `rth0`/`cth0` self-heating.

**Why P1**: The single deliverable. The internal-node machinery it stands on
already exists; this story is the model + its validation.

**Acceptance Criteria**:

1. WHEN a device model declares `rth0`/`cth0` and self-heating is enabled
   THEN the module SHALL, using only existing primitives, allocate one
   internal thermal `wire`, stamp `rth0` (resistor to ambient reference),
   `cth0` (`ddt` capacitor in parallel), and a current source equal to the
   device's own instantaneous dissipated power `Pdiss` (hand-written Σ I·V).
2. WHEN the thermal node solves to a temperature rise `ΔT = V(thermal)` THEN
   the device's temperature-dependent parameters SHALL be evaluated at
   `$temperature + dtemp_static + ΔT` — composing with (not replacing) the
   existing static `dtemp` param.
3. WHEN self-heating is disabled (`rth0 = 0`) THEN the thermal `wire` and its
   stamps SHALL const-fold away at elaboration (existing `StructuralIf`),
   reproducing today's static-`dtemp`-only behavior with zero allocated
   internal node and zero solver overhead.
4. WHEN a self-heating test circuit (dissipative element with `rth0`/`cth0`,
   stepped to constant `Pdiss`) is simulated `.tran` THEN the temperature-rise
   curve SHALL match the closed-form RC step response
   `ΔT(t) = Pdiss·Rth·(1 − e^{−t/(Rth·Cth)})` within the existing ngspice
   cross-check tolerance, and a temperature-dependent parameter SHALL visibly
   shift as `ΔT` rises.

**Independent Test**: A resistor-like dissipative element with `rth0`/`cth0`
and a step current forcing constant `Pdiss`; confirm the internal thermal
node's transient matches the closed-form RC step response, and that a tempco'd
parameter shifts as `ΔT` rises. A second assertion: with `rth0 = 0`, the built
circuit has no extra anonymous node (introspection node count unchanged) and
matches the static-`dtemp` baseline exactly.

---

## Edge Cases

- WHEN `rth0 = 0` THEN no thermal node is allocated (const-folded) — never a
  degenerate zero-resistance node leaving the thermal unknown floating.
- WHEN `cth0 = 0` but `rth0 > 0` (algebraic self-heating, no thermal mass)
  THEN the node SHALL still solve (resistive-only stamp, no `ddt` term) —
  fail loud only if genuinely singular, never silently.
- WHEN the thermal node is declared but `Pdiss` is never contributed (dead
  node) THEN elaboration SHALL fail loud (CLAUDE.md "never silently emit 0.0")
  — this is the *existing* unconnected/dead-node diagnostic, not new work.
- WHEN two instances of a self-heating device run in one circuit THEN each
  SHALL get its OWN anonymous thermal node (existing per-instance anonymous
  allocation, `circuit.rs:429` `next_anon++`) — no temperature leakage
  across instances.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SHEAT-01 | P1 Self-heating (thermal RC + Pdiss stamp) | Design | Pending |
| SHEAT-02 | P1 Self-heating (temperature feedback) | Design | Pending |
| SHEAT-03 | P1 Self-heating (rth0=0 const-fold, zero overhead) | Design | Pending |
| SHEAT-04 | P1 Self-heating (RC step-response validation) | Design | Pending |

**ID format:** `SHEAT-[NUMBER]`

**Coverage:** 4 total, 0 mapped to tasks yet (Design/authoring pending).

---

## Success Criteria

- [ ] A self-heating device model ships in PHDL, composed entirely from
      existing primitives (no new solver/codegen capability).
- [ ] Its transient `ΔT` matches the closed-form RC thermal step response
      (and ngspice where the same option is exposed).
- [ ] Self-heating off (`rth0=0`) reproduces the static-`dtemp` baseline
      exactly — zero extra node, zero solver overhead.
- [ ] `cargo test --workspace` green; zero rustc warnings.
