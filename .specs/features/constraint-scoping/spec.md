# Constraint Scoping — @ dc / @ tran / @ ac + temporal windows

## Problem
Six api tests (event_window ×2, scoped_require_scope ×3 counting strict_dc,
gradient @ ac/@ tran ×2) are blocked on the scoping grammar: `@ <analysis>
[(after = e[, dur = e])] { … }` around constraint statements. The backend
is live: `LoweredMargin.analyses`/`window`, `MarginInfo` carries both, the
device tracker filters by running analysis + time window (DVC-19/20), and
`MarginsResult` reports not-exercised.

## Goals
- `@ dc`/`@ tran`/`@ ac` scoping blocks parse inside `constraint begin`;
  contained requires/targets carry the analysis set.
- Optional `after`/`dur` (tran only; const-evaluated, open-ended `after`
  without `dur` = [after, ∞)) — carried as ResolvedWindow (dur = inf for
  open-ended).
- `target#<n>` indexing counts scoped statements too (all statements,
  authored order).
- Not-exercised semantics: a scoped margin whose analysis never ran (or
  whose window selects no point) reads worst=+inf/no argmin and gates
  `requires_ok` false (existing tracker behavior, asserted by tests).

## Out of scope
Cross-analysis reduced vars inside scopes (already classified by helper
scan); `&&`/`not` window algebra (dv-core had it; deferred — no test).

## Requirements
| ID | Requirement | Status |
|---|---|---|
| CSC-01 | Grammar: @ dc/@ tran/@ ac [(after, dur)] { stmts } parses; contextual | Verified |
| CSC-02 | POM carries scope; contained statements inherit analysis (+window) | Verified |
| CSC-03 | Lowering propagates analyses/window into LoweredMargin | Verified |
| CSC-04 | Kernel keeps pointwise scoped margins; tracker honors (existing) | Verified |
| CSC-05 | Fixture migration green: event_window ×2, scoped ×3, gradient ×2 | Verified |
| CSC-06 | Docs: Part I §12 scoping paragraph; ROADMAP still-dark shrink | Verified |

## Edge cases
- dur without after → loud; non-const after/dur → loud; unknown analysis
  name → loud.
- Nested scoping → loud (no semantics yet).
