# Constraint Statements — var / target / cover Specification

## Problem Statement

The `constraint begin … end` block (MD-49, `constraints-on-vams`) accepts
exactly `require`. The dv-core surface it re-anchors had three more
statement kinds — constraint-local **vars** (computed quantities, pointwise
or reduced), **targets** (unlabeled soft goals), and **cover** points /
crosses (verification coverage counters) — and their entire evaluation
backend still sits alive and `Option`-gated:

- codegen: `MarginKind::{Target,Var}` (solver abi), `QuantityClass`,
  `LoweredCoverPoint`/`LoweredCoverCross` types, the kernel's cover
  compile arm, `AnalogKernel::eval_cover`, `CoverTracker`
- solver: `CoverageReport`, `ReportedCoverPoint/Cross`, the `evaluate_
  constraints_at`/`coverage_reports` polls
- api: `CoverageDatabase` (merge across runs), `MarginValue::Reduced`,
  the `Session::ac` → `ReducedEvaluation` → `with_reduced_margins`
  plumbing (the module survives as a shell — its reduction math was
  deleted at T21 as unreachable dead code, recoverable from `34c0c632`)

What is missing is the **producer**: grammar, POM nodes, codegen lowering,
the api catalog walk for the new kinds, and the reduced-math restoration.

Measured cost: the present-and-red tests carried since the migration —
`margin_lowering` ×2 (var/target), `cover_bin_mapping` ×7,
`reduced_metrics` ×5, `coverage_merge` ×4, `margins_channel` reduced ×1,
`violations_channel` reduced/digital ×2, python `margins_surface` ×1 —
are blocked on precisely this surface.

## Goals

- [ ] A designer can declare constraint-local computed quantities, soft
      targets, and coverage points inside `constraint begin`, and each
      flows to its preserved channel: margins (var/target), the coverage
      database (cover/cross), reduced AC metrics included.
- [ ] Reconnect, not rewrite: the kernel/solver/api consumers light up
      unchanged (the only api-side rebuild is `reduced.rs`'s deleted math,
      restored from `34c0c632`).
- [ ] `target#<n>` naming stays statement-index-based, matching the
      original dv-core convention the blocked tests assert.

## Out of Scope

| Feature | Reason |
|---|---|
| Analysis-scoped event blocks (`@ dc`/`@ tran`/`@ ac`) + temporal windows | Separate deferred surface (`event_window`/`scoped_require_scope` tests); statement-independent. |
| Parameter distributions / Monte-Carlo | The random-variable catalogue feature (user-approved next). |
| New reduced helpers beyond the original three (`ac_unity_gain_freq`/`ac_phase_margin`/`ac_gain_margin`) | Restore the deleted math; extensions follow with demand. |
| AC adjoint gradients of reduced metrics | dv-gradients follow-up (unchanged classification). |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| `var` syntax | `var <name> = <expr>;` — no type annotation | Margins/quantities are real; the PHDL `: Real` annotation carried no information. Simplest grammar that matches the original semantics. | Agent's discretion |
| `var` typing | Always real (pointwise or reduced) | The whole margin machinery is real-valued. | Agent's discretion |
| Classification | An expression containing a reduced helper (`ac_unity_gain_freq`/`ac_phase_margin`/`ac_gain_margin`) is `QuantityClass::Reduced` with `analyses: ["ac"]`; everything else Pointwise | The original rule (DVC-09), re-derived from the preserved types. | y (pre-MD-45 shape) |
| `target` label | `target#<stmt-index>` — the index across ALL the module's constraint statements, 0-based, authored order | What the blocked tests assert (`reduced_metrics`: vars 0..2, `target#3`). | y |
| `cover` bins forms | `bins [lo:step:hi]` and `bins in {e1, e2, …}` — literal-f64 after const-eval | The preserved `ResolvedCoverBins::{Range,Set}` shapes. | y |
| `cover cross` syntax | `cover cross <name> : <p1>, <p2>;` over named cover points | The blocked fixture's own form. | y |
| Reduced var on non-AC | `not_defined` catalog entry → `MarginsResult::get` fails loud "not defined in this analysis" | The preserved channel distinction (DVC-18). | y |

**Open questions:** none.

## User Stories

### P1: Constraint vars — computed quantities ⭐

**Story**: declare a named quantity once, read it in requires/targets.

**Acceptance Criteria**:
1. `var ap1 = a + 1.0;` parses; a later `require via_var : ap1 <= b;`
   substitutes the var's expression (authored order) — the lowered margin
   is `b − (a + 1)` (asserted on values).
2. A var referencing an earlier var substitutes transitively.
3. An unknown name in a var fails loud naming it.

### P2: Targets — soft goals ⭐

**Story**: track headroom without failing the run.

**Acceptance Criteria**:
1. `target <expr> <=|>= <level> [tol <scale>];` lowers to a signed,
   scale-normalized margin named `target#<index>` (statement index across
   all constraint statements), kind `Target`.
2. A violated target NEVER produces a finding (soft goal — the tracker
   records worst/argmin only); `requires_ok` ignores targets.
3. A reduced-class target (over a reduced helper) reaches the host's
   reduced channel on AC with one value and no argmin.

### P3: Reduced metrics on AC ⭐

**Story**: ugbw/pm/gm as first-class margins over a completed AC sweep.

**Acceptance Criteria**:
1. `var ugbw = ac_unity_gain_freq(V(out, gnd)); target ugbw >= 1e4 tol 1e3;`
   — on an AC result, `margins.get("ugbw")` returns kind `var`, shape
   `reduced`, value ≈ the analytic unity-gain frequency; `target#N`'s
   margin = (ugbw − 1e4)/1e3.
2. Reading a reduced quantity from a non-AC result fails loud
   "not defined in this analysis".
3. `ac_phase_margin`/`ac_gain_margin` reduce over the same sweep.

### P4: Cover points and crosses

**Story**: bin coverage counters over a run, merged across runs.

**Acceptance Criteria**:
1. `cover c1 : V(p, n) bins [0.0:1.0:3.0];` — a hit increments the bin
   (nearest-edge mapping); values outside every bin count nothing.
2. `cover c1 : V(p, n) bins in {…};` — set membership.
3. `cover cross joint : c1, c2;` — the joint bin = the same instant's
   per-point bins; joint count = product of point bin counts.
4. `CoverageDatabase::merge_run` sums (never replaces) across runs;
   empty bins name themselves.
5. `cover=off` never calls the coverage kernel (existing posture test
   shape).

## Edge Cases

- A var read before its declaration → loud (authored-order substitution).
- A target over an unresolved name → loud at lowering.
- `requires_ok` with only violated targets → still true.
- A cover point's expression non-finite at a point → no bin hit (never a
  crash/NaN counter).
- Duplicate labels across vars/requires/covers → E4008 (existing rule,
  now module-wide across statement kinds).

## Requirement Traceability

| ID | Story | Phase | Status |
|---|---|---|---|
| CONSTMT-01 | P1: vars parse, substitute (transitively), fail loud | Design | Pending |
| CONSTMT-02 | P2: targets lower to target#N margins, kind Target | Design | Pending |
| CONSTMT-03 | P2: targets never emit findings; requires_ok ignores them | Design | Pending |
| CONSTMT-04 | P3: reduced vars/targets on AC — ugbw/pm/gm values | Design | Pending |
| CONSTMT-05 | P3: reduced read outside AC fails loud not-defined | Design | Pending |
| CONSTMT-06 | P4: cover bins range+set, nearest-edge, out-of-bin | Design | Pending |
| CONSTMT-07 | P4: cover cross joint counting | Design | Pending |
| CONSTMT-08 | P4: CoverageDatabase merge sums across runs | Design | Pending |
| CONSTMT-09 | Edge: duplicate labels across kinds E4008 | Design | Pending |
| CONSTMT-10 | Present-and-red suite migrated green (margin_lowering ×2, cover_bin_mapping ×7, reduced_metrics, coverage_merge, margins_channel, violations_channel, python ×1) | Design | Pending |
| CONSTMT-11 | Docs: Part I §12 statement surface updated; ROADMAP still-dark list shrunk | Design | Pending |

## Success Criteria

- [ ] Every named present-and-red test above green; no assertion weakened.
- [ ] `LoweredBody::constraints/cover` produced from real `var`/`target`/
      `cover` sources (no longer require-only).
- [ ] `reduced.rs`'s "MD-45 regression" doc gone; `Session::ac` returns
      reduced entries for designs that declare them.
- [ ] Zero warnings; every remaining red test names a deferred mechanism.
