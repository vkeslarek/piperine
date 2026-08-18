# Constraints on VAMS Specification

## Problem Statement

Piperine's design-verification surface — declared constraints, signed
margins, and the pass/fail channel built on them — is **dark**. MD-45 (user
decision, 2026-08-12) deliberately gutted PHDL's entire value layer at T21,
taking `constraint Mod { require/var/target/cover }` with it, and recorded
the plan to rebuild it later on Verilog-AMS. This feature is that rebuild.

The gutting was clean but asymmetric: the **frontend is gone** (no grammar
production, `constraint` is not a keyword, `pom::ConstraintBlock`/
`QuantityClass`/`Module::constraints()` deleted, `resolve/pom/body.rs` sets
`LoweredBody::constraints = None` permanently), while the **backend was
deliberately preserved for exactly this re-anchor**:

- `crates/piperine-codegen/src/resolve/pom/constraints.rs` — the margin and
  cover catalog types (`QuantityClass`, `MarginKind`, `LoweredMargin`,
  `ResolvedWindow`, …), kept, per its own module doc, "so that machinery
  doesn't need touching in this retarget"
- the kernel's constraint/coverage capabilities, already `Option`-gated on
  `LoweredBody::constraints`
- `piperine-api`'s complete margins surface — `ConstraintCatalog`,
  `MarginsResult`, `MarginEntry`, `is_satisfied`/`scalar`/`argmin`/
  `is_not_exercised`, the validation-findings channel, and the three
  postures — whose `ConstraintCatalog::compute()` today returns an empty
  catalog behind an explicit "**MD-45 regression**" doc comment.

The cost is measured, not hypothetical: the PHDL→VAMS fixture migration run
in this same session left **73 tests failing across `piperine-api` (66, over
20 targets) and `piperine-python` (3, plus singletons)** that are blocked on
precisely this surface and on nothing else. They cannot be fixed by fixture
translation, because the mechanism they exercise does not exist.

Meanwhile `ROADMAP.md` still advertises dv-core as **✅ delivered**, listing
`constraint Mod { require/var/target }` grammar among what "landed" —
grammar that no longer parses.

## Goals

- [ ] A designer can declare constraints in a `.vams`/`.va` module and get
      signed margins and loud, named pass/fail out of a real simulation
      run — the capability that has been dark since T21.
- [ ] The preserved backend is **reconnected, not rewritten**: the codegen
      catalog types, the kernel's `Option`-gated capabilities, and
      `piperine-api`'s margins surface all light up from a real producer.
- [ ] The semantics are anchored on Verilog-AMS's own assertion model
      (LRM §9.7.3) rather than re-derived, so the rejected-iteration rule
      comes from the standard instead of from a hand-rolled guess.
- [ ] `ROADMAP.md` stops claiming a delivered feature that cannot parse.

## Out of Scope

| Feature | Reason |
|---|---|
| `var` / `target` / `cover` statements | User chose "core first". They were part of dv-core and are the natural next feature, but each carries its own classification and catalog machinery. |
| Parameter distributions (PHDL's `tol gauss(…)`), Monte-Carlo, `monte_carlo_*` tests | Deferred by the user. Research recorded in `context.md`: the LRM's own §9.13.2 sampling functions (`$rdist_normal`/`$rdist_uniform`/`$random`) are **already implemented** in `eval/tasks.rs`, so the follow-up re-anchors onto them rather than inventing syntax. `context.md` additionally records the user's architectural direction for that follow-up — uncertainty must become a **first-class random-variable catalogue on the POM**, not the per-parameter `Param::distribution` vestige, because correlation, global-vs-mismatch scoping, reproducible replay, and MD-18 compile-once are all cross-parameter facts — together with why that catalogue is the joint between constraints and the design-centering optimizer. **Ordering consequence flagged there:** its shape should be settled before optimizer work hardens around a per-parameter view. |
| Analysis-scoped event blocks (`@ dc`/`@ tran`/`@ ac`) and temporal windows | dv-core surface, deferred with `var`/`target`/`cover`. `ResolvedWindow` already exists in the preserved types for whenever it lands. |
| Restoring SOA constraint blocks on the spice stdlib models (mos1/mos2/mos3/diode/bjt) | Offered as a scope option, user chose core first. First natural follow-up. |
| LSP support for constraint names (hover/goto/rename on a `require` label) | Follows naturally from POM spans once the block elaborates, but is not an acceptance criterion here. |
| Un-blocking the 73 currently-failing dv-core tests | Those exercise `var`/`target`/`cover`/`tol gauss`/event blocks as well as `require`. This feature un-blocks the `require`-only subset; the rest follow with their own features. **The count must not be read as this feature's success metric.** |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Block syntax | `constraint begin … end`, a module body item mirroring `analog begin … end` | User's explicit call: keep the block ergonomics of PHDL's `constraint Mod { … }` but spell it the way VAMS spells its other bodies. Inside the module, so ports/params/branches are simply in scope — no name-matching indirection. | y |
| Grammar provenance (`CLAUDE.md` C7: grammar from `VAMS-LRM-2-4.md` Annex A only) | `constraint begin … end` is a **sanctioned Piperine extension**, documented as such in Part I alongside the existing extensions | Annex A has no constraint production, so this is a deliberate deviation and must be recorded as one, not slipped in. Precedent exists and is documented: `(* device *)`/`(* port *)` attribute schemas (§8) and `///` doc comments (§1.1). Needs a new `MD-` decision entry. | y |
| Posture ↔ severity | Postures map onto LRM §9.7.3's assertion severity: `strict` → `$error`, `collect` → `$warning`, `off` → silent | User's call after the LRM research. Inherits the standard's rejected-iteration rule ("non-fatal severity tasks called during a rejected iteration shall have no effect") instead of re-deriving it — precisely where a naive checker would be wrong. | y |
| Behavior when a constraint references a quantity the running analysis cannot provide | Fail loud, naming the constraint, the quantity, and why it is unavailable in this analysis | User's call. Matches the project's fail-loud rule; the original dv-core already carried `MarginsResult::not_defined`/`digital` for exactly this distinction. | y |
| `tol` placement | `require <name> : <pred> [tol <scale>];` — an optional trailing clause on `require` | The user's chosen core explicitly includes tol-based normalization, but in PHDL `tol <scale>` was a clause on `target`, and `target` is deferred. Normalization is what makes margins comparable across constraints, so it moves onto `require` rather than being deferred with its old host. | y (assumption, logged) |
| Margin numeric convention | `m >= 0` ⟺ satisfied, as in dv-core | Unchanged from the original; the preserved backend types already assume it. | y |
| Where the check runs | At accepted solution points of the running analysis (DC operating point, each accepted transient step, each AC/sweep point) | Follows from the rejected-iteration rule; the exact hook is a Design-phase decision. | y (deferred to Design) |

**Open questions:** none — all resolved or logged above.

---

## User Stories

### P1: Declare a constraint and get a loud, named failure ⭐ MVP

**User Story**: As a designer, I want to declare a safe-operating-area limit
on my module and have the simulator tell me — loudly, by name — when a run
violates it, so a design error surfaces at simulation time instead of in
silicon.

**Why P1**: This is the capability that has been dark since T21 and the
whole point of the feature. Nothing else matters if a violation cannot be
detected and reported.

**Acceptance Criteria**:

1. WHEN a module contains `constraint begin require <name> : <pred>; end`
   THEN the module SHALL parse and elaborate, and the constraint SHALL be
   visible on the POM attached to that module.
2. WHEN a `require` predicate reads ports, parameters, or branch
   probes of its enclosing module (e.g. `abs(V(g,s)) <= vgs_max`) THEN
   those names SHALL resolve in the module's own scope with no
   qualification.
3. WHEN a run in `strict` posture reaches an accepted solution point at
   which a `require` predicate is false THEN the run SHALL fail loud with a
   typed error naming the constraint, the violating instance path, and the
   signed margin.
4. WHEN every `require` holds at every accepted solution point THEN the run
   SHALL complete normally and report each constraint as satisfied with its
   worst margin.
5. WHEN a `require` predicate is false only during a Newton iteration that
   the solver subsequently rejects THEN no violation SHALL be reported
   (LRM §9.7.3's rejected-iteration rule).

**Independent Test**: A two-module fixture where a resistor divider drives a
node past a declared limit; assert the run fails naming the constraint, and
that raising the limit makes the same fixture pass.

---

### P2: Signed margins, normalized and reduced

**User Story**: As a designer, I want the *distance* to violation, not just
a boolean, so I can see how much headroom a design has and compare
constraints against each other.

**Why P2**: The boolean is the MVP; the margin is what makes the surface
useful for optimization and centering — and it is what the preserved
`piperine-api` types already expose.

**Acceptance Criteria**:

1. WHEN a constraint is evaluated THEN it SHALL carry a signed margin with
   `m >= 0` ⟺ satisfied.
2. WHEN a `require` declares `tol <scale>` THEN its margin SHALL be
   normalized by that scale; absent, the scale SHALL be `1` with no implicit
   normalization.
3. WHEN a run sweeps time, frequency, or a parameter THEN each constraint
   SHALL report its **worst** margin over the run together with the argmin
   (the sweep coordinate at which the worst value occurred) and the instance
   path.
4. WHEN margins are read from the host THEN they SHALL arrive through the
   existing `piperine-api` surface (`MarginsResult`/`MarginEntry`,
   `is_satisfied()`, `scalar()`, `argmin()`) with no new parallel API.

**Independent Test**: Sweep a parameter across a limit and assert the worst
margin changes sign at the expected point, with `argmin()` naming that
sweep coordinate.

---

### P3: The three postures

**User Story**: As a designer, I want to choose whether a violation aborts
the run, is merely recorded, or is not checked at all, so verification cost
and strictness match what I'm doing right now.

**Why P3**: Real, and already designed into the surviving API — but the
default (`strict`) is what P1 delivers, so the other two are refinement.

**Acceptance Criteria**:

1. WHEN the posture is `strict` (the default) THEN a violation SHALL behave
   as LRM §9.7.3's `$error`: a run-time error that does not let the
   simulation proceed past initialization.
2. WHEN the posture is `collect` THEN a violation SHALL behave as
   `$warning`: recorded in the validation-findings channel, run continues,
   and every violation SHALL be readable afterwards.
3. WHEN the posture is `off` THEN no constraint SHALL be evaluated and the
   run SHALL pay no measurable cost for the declared constraints.
4. WHEN a constraint references a quantity the running analysis cannot
   provide THEN the failure SHALL be loud and SHALL name the constraint, the
   quantity, and why it is unavailable in this analysis — in every posture
   except `off`.

**Independent Test**: Run the same violating fixture under all three
postures; assert abort / recorded-and-continue / silent respectively.

---

## Edge Cases

- WHEN a module declares `constraint begin … end` but the module is never
  instantiated THEN nothing is evaluated and no diagnostic is produced.
- WHEN a module declares more than one `constraint begin … end` block THEN
  they SHALL compose (all requires from all blocks apply) — mirroring the
  LRM's own "multiple analog initial blocks are executed as if concatenated"
  rule, and mirroring the multiple-`always` fix landed in `f75beb4d`. A
  silent drop of any block is forbidden.
- WHEN a `require` name collides with another `require` name in the same
  module THEN elaboration SHALL fail loud naming the duplicate.
- WHEN a `require` predicate references a name that does not resolve THEN
  elaboration SHALL fail loud naming it — never a silent `false` or a
  vacuous pass.
- WHEN a constraint is declared on a module with no analog behavior (a
  purely digital or purely structural module) THEN asking for its margin
  SHALL be loud, matching the existing `MarginsResult::digital` distinction
  (margins are an analog notion).
- WHEN the same constrained module is instantiated many times THEN each
  instance SHALL report its own margin, distinguished by instance path.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| CONVAMS-01 | P1: `constraint begin … end` parses and elaborates onto the POM | Design | Implementing |
| CONVAMS-02 | P1: predicate names resolve in the enclosing module's scope | Design | Implementing |
| CONVAMS-03 | P1: violation in `strict` fails loud, naming constraint/instance/margin | Design | Implementing |
| CONVAMS-04 | P1: all-satisfied run completes and reports worst margins | Design | Implementing |
| CONVAMS-05 | P1: rejected-iteration violations are not reported (LRM §9.7.3) | Design | Implementing |
| CONVAMS-06 | P2: signed margin, `m >= 0` ⟺ satisfied | Design | Implementing |
| CONVAMS-07 | P2: `tol <scale>` normalization, default scale 1 | Design | Implementing |
| CONVAMS-08 | P2: worst margin + argmin + instance path over a sweep | Design | Implementing |
| CONVAMS-09 | P2: read through the existing `piperine-api` margins surface | Design | Implementing |
| CONVAMS-10 | P3: `strict` behaves as `$error` | Design | Implementing |
| CONVAMS-11 | P3: `collect` behaves as `$warning`, findings readable | Design | Implementing |
| CONVAMS-12 | P3: `off` evaluates nothing at no measurable cost | Design | Implementing |
| CONVAMS-13 | P3: unavailable-quantity reference fails loud, naming it | Design | Implementing |
| CONVAMS-14 | Edge: multiple `constraint` blocks compose, none dropped | Design | Implementing |
| CONVAMS-15 | Edge: duplicate/unresolved `require` names fail loud | Design | Implementing |
| CONVAMS-16 | Docs: `constraint begin` recorded as a sanctioned extension (Part I) + new `MD-` decision | Design | Implementing |
| CONVAMS-17 | Docs: `ROADMAP.md` corrected to stop claiming dv-core's PHDL grammar as delivered | Design | Implementing |

**ID format:** `CONVAMS-NN`

**Status values:** Pending → In Design → In Tasks → Implementing → Verified

**Coverage:** 17 total, 17 mapped to T1–T8 (see tasks.md Requirement fields), 0 unmapped

---

## Success Criteria

- [ ] A `.vams` module with a `constraint begin … end` block runs, and a
      violation aborts under `strict` naming the constraint, the instance
      path, and the margin.
- [ ] `ConstraintCatalog::compute()` no longer returns an empty catalog for
      a design that declares constraints, and its "MD-45 regression" doc
      comment is removed as no longer true.
- [ ] `LoweredBody::constraints` is populated from a real producer for a
      constrained module, instead of being permanently `None`.
- [ ] The `require`-only subset of the currently-dark dv-core tests is
      un-blocked; every test still failing does so for a mechanism this
      feature explicitly deferred (`var`/`target`/`cover`/`tol gauss`/event
      blocks), each named.
- [ ] `ROADMAP.md` describes the real state of dv-*.
