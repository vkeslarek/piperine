# dv-core Specification — the declaration and its runtime

**Vision:** `.specs/features/design-verification/ideal.md` (shared by the three
`dv-*` features). Decisions D1–D16 there are binding; this spec cites them.
**Siblings:** `dv-gradients` (analytic sensitivity + host policies),
`dv-tester` (programmatic tester + verification at scale).
**Scope tier:** Large/Complex. **ROADMAP:** feeds a new P8 (Design Verification).

This is the foundation the other two stand on: two grammar additions (`tol` on
`param`, the `constraint` block), the signed-margin convention, kernel
evaluation with provenance, and one findings channel on the `Element` ABI.

## Problem Statement

A Piperine user cannot say "this circuit is correct" anywhere except a Python
testbench. SOA/ERC knowledge cannot ship with a device model, spec compliance is
re-derived per testbench, and statistical variation has no declaration site at
all. Meanwhile the tree already holds the two hard parts: the compiler
differentiates symbolically (`resolve/diff.rs`) and the session restamps without
re-elaborating (MD-18). What is missing is a **declaration site** for
correctness intent that composes with hierarchy, a runtime-evaluated **margin**
with provenance, and a **findings channel** every element origin can speak
through — PHDL-compiled, plugin, or OSDI.

## Goals

- [ ] `tol` clause on `param`: distribution declared on the parameter, visible
      in the POM, inert at solve (nominal analyses bit-identical).
- [ ] `constraint` block: `require`/`var`/`target` attached to a module by name,
      authored structure in the POM like `analog`/`digital`.
- [ ] Signed-margin convention (`m ≥ 0` ⟺ satisfied), `tol`-normalized targets,
      worst margin + argmin (time / frequency / sweep coordinate + instance).
- [ ] The pointwise/reduced split (§2.2): pointwise lowers to a kernel, reduced
      is a host reduction (D7) — and a `require` over a reduced quantity inside a
      pointwise scope is a loud error.
- [ ] Three postures — `strict` (default), `collect`, `off` — on
      `Context`/`SolverConfig`, both hosts.
- [ ] `validation_reports()` on `Element`, gated by an `EMITS_VALIDATION`
      capability bit, shaped after `limiting_report()`.
- [ ] SOA `constraint` blocks on `headers/spice/` models, with limits **absent by
      default** so the gallery is unaffected by construction.
- [ ] Margins on result objects as their own channel (D3), identical on both hosts.

## Out of Scope

| Feature | Owner |
|---|---|
| `∂F/∂p` kernel, `param_stamps`, adjoint sensitivity | `dv-gradients` |
| Optimizer, Monte Carlo, centering, high-sigma, aging | `dv-gradients` |
| Statistical *sampling* of `tol` distributions | `dv-gradients` — this spec declares and stores only |
| The programmatic tester, monitors, `cover` | `dv-tester` |
| RNM, `behavioral` body kind | Dropped from V1 (D16); recorded as V2 with reasons |
| Implementing the declared `resolve sum\|avg\|max\|min` kinds | Not a `dv-*` feature — MD-24 declared-language debt, own ROADMAP item |
| Transient/AC adjoint of a margin | `dv-gradients` (DC only there; AC is a known gap) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Confirmed? |
|---|---|---|
| Margins are their own result channel | `MarginsResult`-shaped; represents pointwise (with argmin) and reduced (no argmin) without faking a `t = 0` | **y** (D3) |
| Unscoped `require` scope | Holds in every analysis; the default is a `Context` field | **y** (D2) |
| Non-smooth worst case | Differentiate at the located argmin, reported as such | **y** (D1) |
| Constraints under monomorphization | Variants carry the block like any body; one authored block per authored module | **y** (D4) |
| `tol` spelling shared by param distributions and target scales | Keep both — deliberate rhyme | **y** (user) |
| `var` names computed scalars, with mandatory type annotation | `var gain_db : Real = …`; no inference the rest of the language lacks | **y** (user, §2.1) |
| No `minimize` statement; objective is host-side per run | Keep host-side | **y** (user) |
| Reduced metrics computed host-side | Solver stays pointwise-pure | **y** (D7) |
| New keywords contextual except `constraint` | `target` is already an identifier in two frozen fixtures | **y** (measured, §2.6) |
| SOA limits on built-in models default absent | An unset limit makes its `require` vacuously satisfied | **y** (§2.8) |
| Findings polled only where §2.5 says | Never at a homotopy stage, never on a rejected step, skipped at `t = 0` under UIC | **y** (§2.5) |
| `tol` restricted to `Real` params | Discrete variation rejected loud | n (agent) |
| Distribution args may reference sibling params | Elaboration-constant, module param scope | n (agent) |

**Open questions:** none blocking.

---

## User Stories

### P1: `tol` on `param` — declared, inert, POM-visible ⭐ MVP

**User Story**: As a model author, I want
`param vto : Real = 0.7 tol gauss(sigma = 0.005/sqrt(w*l));` so statistical
variation ships with the parameter it perturbs and any host can enumerate it
without simulating.

**Why P1**: Cheapest item in the vision and the anchor for every later
statistic. Pure declaration — no solve behavior.

**Acceptance Criteria**:

1. WHEN a `param` carries `tol <dist>(args) [global]` with a distribution
   declared in `headers/statistics.phdl` THEN elaboration SHALL succeed and the
   POM `Param` SHALL expose it (`Param.distribution()`), including the `global`
   (process) vs default (mismatch) marker.
2. WHEN a distribution name is not declared THEN elaboration SHALL fail loud
   naming it (MD-24; `extern_coverage_guard.rs` extends).
3. WHEN `tol` appears on a non-`Real` param THEN elaboration SHALL fail loud.
4. WHEN a nominal `op()`/`tran()`/`ac()` runs on a design with `tol` clauses THEN
   results SHALL be bit-identical to the same design without them.
5. WHEN distribution arguments reference sibling params THEN they SHALL evaluate
   as elaboration constants in module param scope.
6. WHEN a param carries both `tol` and a staged override THEN the override SHALL
   win for the nominal value and the distribution SHALL stay attached.

**Independent Test**: elaborate a `tol` fixture, read the distribution back off
the POM, diff `op()` against the untoleranced twin.

---

### P1: `constraint` block — grammar and POM ⭐ MVP

**User Story**: As a circuit author, I want `constraint Mod { … }` with
`require`/`var`/`target` attached to my module by name, so correctness intent is
authored structure walkable in the POM like `analog`/`digital`.

**Why P1**: The declaration site everything else reads.

**Acceptance Criteria**:

1. WHEN a `constraint` block names an existing module and contains
   `require <name> : <pred>;`, `var <name> : <Type> = <expr>;`, and
   `target <expr> <cmp> <level> [tol <scale>];` THEN parse + elaboration SHALL
   succeed and the module's POM node SHALL expose the block in authored form.
2. WHEN a constraint references an unknown net, port, instance path, or `var`
   THEN elaboration SHALL fail loud naming the reference and the use site.
3. WHEN a module is monomorphized (`Dac[8]` → `Dac__8`) THEN each variant SHALL
   carry the block as it carries its `analog`/`digital` body, and the POM SHALL
   hold exactly one authored block per authored module (D4, UNBREAKABLE).
4. WHEN a `require` predicate is not Boolean-typed, or a `target` comparison uses
   a non-`Real` operand, or a `var` omits its type annotation THEN elaboration
   SHALL fail loud.
5. WHEN a constraint reads a child instance's **port** net (`V(m1.d, m1.s)`) THEN
   it SHALL resolve through the authored instance tree — already supported, see
   `crates/piperine-lang/tests/examples/sar_adc.phdl:29`; WHEN it reads a
   child's **internal** node THEN elaboration SHALL fail loud (§2.8).
6. WHEN two statements in one block share a name THEN elaboration SHALL fail loud.
7. WHEN `constraint` is used as an identifier THEN that SHALL be a parse error;
   WHEN `target`/`tol`/`require`/`cover`/`global` appear as identifiers outside
   their grammar positions THEN they SHALL still parse as identifiers — the
   frozen `crates/piperine-lang/tests/examples/ring_oscillator.phdl:5` and
   `oscillator.phdl:16` must keep compiling.

**Independent Test**: `piperine check` on a constraint fixture; selector walk
showing the authored block on a monomorphized variant; one negative fixture per
fail-loud clause; both frozen `target` fixtures green.

---

### P1: Margins — the pointwise/reduced split and the signed convention ⭐ MVP

**User Story**: As a verification engineer, I want every comparison to become a
signed margin I can read, so a violation is a value with provenance rather than
a line in a transcript.

**Why P1**: The convention the whole vision rests on (§2.3).

**Acceptance Criteria**:

1. WHEN `a <= b` / `a >= b` / `a in [l,u]` lowers THEN the margin SHALL be
   `(b−a)/scale` / `(a−b)/scale` / `min(u−a, a−l)/scale`, `scale = 1` absent a
   `tol`, and `m ≥ 0` ⟺ satisfied.
2. WHEN a helper declared **pointwise** in `headers/constraints.phdl` is used
   THEN its `var`/`target` SHALL lower to a per-point kernel; WHEN a helper
   declared **reduced** is used THEN it SHALL be computed as a host reduction
   over the analysis's points (D7).
3. WHEN a `require` or `target` inside a pointwise scope reads a **reduced**
   quantity THEN elaboration SHALL fail loud ("a reduced quantity is checkable
   once per analysis, not per point").
4. WHEN a pointwise margin is reported THEN it SHALL carry the worst value and
   its argmin — time (transient), frequency (AC), swept coordinate (sweep) — plus
   the instance path; WHEN a reduced margin is reported THEN it SHALL carry one
   value and **no** argmin, and the channel SHALL state which kind it is.
5. WHEN a margin is non-finite at an evaluated point THEN it SHALL be reported
   loud as a violation, never silently clamped.
6. WHEN a `var` is read from an analysis where it is not defined THEN the read
   SHALL fail loud ("not defined in this analysis").

**Independent Test**: a fixture with one pointwise and one reduced `target`;
assert the pointwise argmin and the reduced margin's absence of one; negative
fixture for a reduced quantity inside `@ tran`.

---

### P1: Kernel evaluation, the three postures, and which points count ⭐ MVP

**User Story**: As a verification engineer, I want `require`s evaluated at the
solver's accepted points — failing loud in strict with instance, time, and value
— and zero cost when I turn them off.

**Why P1**: The runtime half of the declaration; unlocks SOA-on-models.

**Acceptance Criteria**:

1. WHEN a transient runs with `checks=strict` (default) and a `require` margin
   goes below zero at an evaluated point THEN the analysis SHALL fail loud naming
   the constraint, instance path, time, and margin value.
2. WHEN the same run uses `checks=collect` THEN it SHALL complete and record the
   worst margin per constraint with its argmin.
3. WHEN `checks=off` THEN constraint kernels SHALL not be called, and a circuit
   with no constraint block SHALL be bit-identical in cost and result to
   pre-feature behavior (Option sub-struct, as `forces.rs`/`limits.rs`).
4. WHEN a DC or OP analysis is solved THEN margins SHALL be evaluated **only at
   the final converged solution** — never at a gmin-stepping or source-stepping
   stage, whose intermediates are non-physical by construction (§2.5).
5. WHEN a transient step is **rejected** THEN no margin SHALL be evaluated and no
   finding emitted for it; WHEN `t = 0` state comes from UIC/`@initial` THEN that
   point SHALL be skipped.
6. WHEN a `sweep`/`sweep_grid` completes THEN the result SHALL carry the worst
   margin across swept points with the swept coordinate in its argmin.
7. WHEN the posture changes between analyses in one session THEN the next
   analysis SHALL honor it with **no** recompilation, and
   `compile_once_sweep.rs`-style counts SHALL be unchanged (MD-18).
8. WHEN the posture is read on either host THEN Python `Solver` and Rust
   `SolverConfig`/`Context` SHALL expose the identical field, alongside D2's
   unscoped-`require` scope default.

**Independent Test**: a divider fixture with a violated `require` — strict fails
naming instance/time/value, collect reports the worst margin with argmin, off is
clean and pays nothing; plus a gmin-stepping fixture that would violate
mid-homotopy and must not report.

---

### P1: `validation_reports()` — one findings channel ⭐ MVP

**User Story**: As a device author (PHDL, plugin, or OSDI), I want one ABI hook
for structured findings with severity, so constraint violations, model
self-checks, monitors, and testers all speak one language.

**Why P1**: Without it, PHDL constraints are privileged and plugin/OSDI checks
stay second-class (§2.9). It is also the **primary** mechanism for digital
verification, because margins are an analog notion (§2.10).

**Acceptance Criteria**:

1. WHEN `Element` grows `validation_reports() -> Option<ValidationReport>` THEN
   it SHALL be cross-cutting (not `AnalogDevice`-gated), SHALL follow the
   `limiting_report()` shape (`core/element.rs:173`), and each finding SHALL
   carry severity (`Warning` | `Error`), label, message, value, time, and
   instance path.
2. WHEN an element does not declare `EMITS_VALIDATION` THEN the solver SHALL NOT
   poll it — "costs nothing when unused" means *not called*, not *returns
   empty*. The bit joins `ElementCapabilities` (`u32`; highest used today is
   `NUMERIC_JACOBIAN = 1 << 14`).
3. WHEN a `require` margin crosses zero THEN the constraint kernel SHALL emit an
   `Error` finding through this channel (margins are values; findings are events).
4. WHEN the solver polls in `strict` and an `Error` finding exists THEN the
   analysis SHALL fail loud; in `collect` it SHALL land on the result
   (`r.violations`); in `off` the hook SHALL never be polled.
5. WHEN a `Warning` is emitted in strict posture THEN the analysis SHALL continue
   — only `Error` aborts.
6. WHEN a digital-only element emits a finding THEN it SHALL be collected
   identically, from the digital scheduler's accepted events (after event
   settling, not mid-delta-cycle).

**Independent Test**: a recording test element emitting a `Warning` and an `Error`
at known times — strict aborts on the `Error`, collect records both with
provenance, off sees nothing, and an element without the bit is never polled.

---

### P1: SOA constraints on `headers/spice/` models ⭐ MVP

**User Story**: As a designer who never writes a `constraint` block, I want every
MOSFET I instantiate to carry its own oxide and junction limits, so SOA checking
is free in every design.

**Why P1**: Highest industrial value per line of code in the vision — the
foundry-deck placement, but with first-class margins instead of warning text.

**Acceptance Criteria**:

1. WHEN `headers/spice/mos.phdl` gains `constraint Mos1` with
   `vgs_ox`/`vds_ox`/bulk requires reading `vgs_max`/`vds_max` from the model card
   THEN those limits SHALL default to **absent**, and an absent limit SHALL make
   its `require` vacuously satisfied — so every existing fixture and example SHALL
   elaborate and simulate unchanged in default (strict) posture.
2. WHEN a model card **sets** `vds_max` and a fixture drives past it in strict
   posture THEN the analysis SHALL fail loud naming the device instance and the
   violated constraint.
3. WHEN `m1.region` is referenced THEN the device SHALL expose an
   operating-region opvar through the existing introspection surface, typed by a
   declared `enum Region { Cutoff, Triode, Saturation }` — PHDL already has enums
   (`headers/prelude.phdl:13`) — not integer codes.
4. WHEN diode/BJT models gain their blocks THEN the pattern SHALL be identical:
   limits as absent-by-default model-card params, requires in the model's own block.
5. WHEN the ngspice cross-check suite runs THEN its numerics SHALL be unchanged —
   no invented limit may alter a faithful model's behavior.

**Independent Test**: gallery fixture swept into violation under a model card that
sets limits; the same gallery with stock cards reports all-clear;
`tests/ngspice_validation.rs` unchanged.

---

### P2: Host surface — margins and findings on result objects

**User Story**: As a Python (or Rust) host author, I want `r.margins[...]`,
`r.violations`, and `r.requires_ok` with identical names and shapes on both hosts,
so verification currency is a query, not log scraping.

**Acceptance Criteria**:

1. WHEN an analysis completes in `collect` or `strict` THEN the result SHALL
   expose every constraint's worst margin + argmin (or the reduced form) and every
   finding, with identical member names on both hosts (`host_parity.rs` style).
2. WHEN margins are carried THEN they SHALL travel as their own channel (D3), not
   folded into waveform rows.
3. WHEN an unknown constraint or `var` name is queried THEN the read SHALL fail
   loud listing candidates (`UnknownNet`/`Error::Measurement` family).

**Independent Test**: one fixture through both hosts asserting identical margins,
argmins, and violations.

---

### P2: Analysis-scoped event blocks (`@ dc` / `@ tran` / `@ ac`)

**User Story**: As a constraint author, I want `@ dc { require … }` scoping via
the existing `EventBlock` production, so a DC-only rule does not trip during a
transient ramp.

**Acceptance Criteria**:

1. WHEN a constraint body contains `@ dc { … }` THEN those requires SHALL be
   evaluated only in DC analyses; `@ tran`/`@ ac` likewise; `@ (dc | tran)` in both.
2. WHEN a `require` appears at top level THEN it SHALL hold in every analysis,
   subject to D2's `Context` default.
3. WHEN a scoped require's analysis never runs THEN the host SHALL see "not
   exercised" for it — never a silent vacuous pass.
4. WHEN `dc`/`tran`/`ac` resolve as event terms THEN they SHALL go through the
   existing `EventRegistry`, not a new one.

**Independent Test**: fixture whose `@ dc` rule is violated during transient
startup but legal at DC — strict tran passes, strict dc fails.

---

### P3: Event windows (`after =` / `dur =`) and the window algebra

**User Story**: As a constraint author, I want
`@ tran(after = cross(V(en,gnd), rise), dur = 5e-6) { require settle : … }` so a
settling spec is expressible.

**Acceptance Criteria**:

1. WHEN `EventTerm` grows named args THEN `after` SHALL accept a time or an event
   term and `dur` SHALL close the window, mirroring how `timer` already carries a
   second argument.
2. WHEN scopes compose with `|`/`&`/`not` THEN they SHALL behave as set union /
   intersection / complement over evaluation points.
3. WHEN a window selects no points THEN the host SHALL see "not exercised".

**Independent Test**: a settling fixture whose window opens on a crossing; the
margin is evaluated only inside it, and a misspelled trigger reports
not-exercised rather than passing.

---

## Edge Cases

- Duplicate constraint label in one block → loud.
- `target` without `tol` → `scale = 1`, no implicit normalization.
- Non-finite margin → loud violation, never clamped (mirrors §VII-15.10).
- Posture flip mid-session → honored with no recompilation.
- Zero constraint blocks → every analysis bit-identical to today.
- A `require` over a `Bit`/`Quad` → no margin exists; it rides the findings
  channel only (§2.10), and asking for its margin is loud.
- An element declaring `EMITS_VALIDATION` that always returns `None` → legal; the
  bit promises capability, not content.
- A constraint on a module that is never instantiated → not exercised, reported
  as such, never a vacuous pass.

---

## Requirement Traceability

| ID | Story | Status |
|---|---|---|
| DVC-01 | `tol` grammar clause + parse | Pending |
| DVC-02 | `headers/statistics.phdl` + POM `Param.distribution()` | Pending |
| DVC-03 | `tol` inert at solve; staging interaction | Pending |
| DVC-04 | `constraint` grammar (`require`/`var`/`target`) | Pending |
| DVC-05 | POM `Module::constraints`, authored, monomorph-carried (D4) | Pending |
| DVC-06 | Contextual keywords; frozen `target` fixtures keep compiling | Pending |
| DVC-07 | Instance-port refs resolve; internal-node refs loud | Pending |
| DVC-08 | Margin lowering (signed, normalized) | Pending |
| DVC-09 | Pointwise/reduced classification + loud misuse (D7) | Pending |
| DVC-10 | Kernel evaluation + argmin provenance (time/freq/sweep) | Pending |
| DVC-11 | Evaluation points: no homotopy stage, no rejected step, no UIC `t=0` | Pending |
| DVC-12 | Postures strict/collect/off + D2 scope default, both hosts | Pending |
| DVC-13 | `validation_reports()` + `ValidationFinding` shape | Pending |
| DVC-14 | `EMITS_VALIDATION` gates polling | Pending |
| DVC-15 | Findings on results; strict aborts, collect records, warning continues | Pending |
| DVC-16 | Spice-model SOA blocks, limits absent by default | Pending |
| DVC-17 | Region opvar as a declared `enum Region` | Pending |
| DVC-18 | Host margin/violation surface, both hosts (D3) | Pending |
| DVC-19 | Analysis-scoped event blocks | Pending |
| DVC-20 | Event windows + window algebra + not-exercised | Pending |
| DVC-21 | cross-cutting: fail-loud catalog (Edge Cases) | Pending |
| DVC-22 | cross-cutting: MD-18 compile-once unchanged | Pending |

**Coverage:** 22 total, 0 mapped (tasks phase not started).

---

## Success Criteria

- [ ] A MOSFET fixture whose model card sets `vds_max` fails loud in strict
      posture naming instance + time + margin, with **zero** user-written
      constraint code.
- [ ] The stock gallery is untouched — same numerics, same ngspice cross-checks,
      no example edited — because absent limits mean inert requires.
- [ ] `collect` reports normalized margins with argmin for pointwise and without
      for reduced, and never invents a `t = 0`.
- [ ] A circuit with no constraints and no `tol` is bit-identical in cost and
      results to pre-feature behavior.
- [ ] Both frozen fixtures using `target` as an identifier still compile.
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` green.
