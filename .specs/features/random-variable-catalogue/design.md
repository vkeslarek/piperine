# Random-Variable Catalogue — Design

**Spec**: `.specs/features/random-variable-catalogue/spec.md`
**Status**: Draft

---

## Architecture Overview

The same frontend-dark pattern as the constraint surface: **the entire host
machinery survives** — `api/src/sampling.rs`'s `SamplePlan::collect/apply/
reset` (draw through `Session::set`, `(seed, index)` replay via
`RandomState`, global/mismatch scoping), `monte_carlo.rs`'s reporting
(worst sample, yield, sigma), and the POM carrier
`Param::distribution: Option<Distribution>` (`kind`/`args`/`global`) — all
alive, reading a frontend that writes `None` unconditionally. The feature
is the missing producer (grammar + elab) plus fixture migration.

**Catalogue shape** (the recorded direction, honored without a second POM
surface): the ordered catalogue is `SamplePlan::collect`'s walk over
authored modules × params — deterministic, hence a stable index for
replay; scoping lives on the entry (`global` flag); draws apply through
the restamp path (MD-18 compile-once for free); enumeration is the walk.
A parallel `Design.random_vars` list would duplicate that walk with no
new fact — the *cross-parameter* operations all live in the plan, which
is the catalogue the direction asks for.

**Sampling math**: `sampling.rs` keeps its own SplitMix64+Box-Muller stream
— the dv-core test suite pins its values; re-anchoring onto lang's
§9.13.2 `Distributions` would change every pinned draw and is a separate
user call. The spec's "one PRNG shape" goal is deviated here
(SPEC_DEVIATION: same determinism contract, different stream).

## Components

### 1. Grammar (`grammar/item.rs`)
`parse_param_decl_body`'s declarator gains, after the default value and
AFTER any `from`/`exclude` clauses (declaration order is fixed — ranges,
then tol): `tol <kind>( <keyed-args> )
[global]` — `kind` an identifier (`gauss`/`uniform`), keyed args
`name = expr` comma-separated. `tol`/`global` contextual keywords (Annex
B untouched). AST: `Declarator` already carries `init`; the param
declarator is `ParamAssignment { name, dims, value }` — the clause lands
on the ParamDecl level: `ParamDecl.tols: Vec<TolClause>` or into the
existing `ranges` sibling — new field `tol: Option<TolClause>` where
`TolClause { kind, args: Vec<(String, Expr)>, global }` (at most one —
two `tol` clauses on one param fail loud as duplicates).

### 2. Elab (`elab/lower/vams.rs`)
`eval_and_define_param`'s callers thread the clause: keyed args
const-eval against the module env (the `Distribution` doc's own example,
`sigma = 0.005 / sqrt(w*l)`, is a module-scope constant); build
`Distribution { kind, args, global }`; write `Param::distribution`.
Validation: unknown kind fails loud naming it; a second `tol` on the
same declarator fails loud.

### 3. api (no producer change)
`sampling.rs`/`monte_carlo.rs` already walk + draw + report. Only the
blocked fixtures migrate; `high_sigma_sampling` stays red (estimator
machinery — named).

### 4. Draw semantics (recorded here, implemented in the surviving walk)
`gauss(sigma = s)` → nominal + s·z; `gauss(sigma_rel = r)` →
nominal·(1 + r·z); `uniform(rel = u)` → nominal·(1 + U(−u, u)) — z
normal, U uniform, both from `Distributions`. Out-of-range draws (a
`from`-declared param pushed past its range by a draw) are **reported,
never clamped** (sampling explores; range checks guard authored values).

## Risks & Concerns

| Concern | Mitigation |
|---|---|
| `tol` also names `require`'s scale clause — same word, two grammars | Disjoint production sites (param declarator vs constraint stmt); contextual keywords allow this by design |
| `Param::distribution` was the "vestige" the direction warned about | The warning was about per-param Option as the *sampling* surface; the plan (cross-param) is the catalogue. Authored-site mirror kept deliberately — `param("r").distribution` is the introspection read. |
| `tol_inert`'s "staged override wins for nominal, distribution stays attached" | The surviving staged-override machinery keeps `distribution` on the POM — verified by that test's own assertions |

## Tech Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Catalogue = walk, not a new POM list | `SamplePlan::collect` | Stable order = replay; no dual surface |
| PRNG | lang's public `Distributions` | One convention, already tested |
| Out-of-range draws | report, never clamp | Sampling explores |
