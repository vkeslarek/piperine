# LESSONS — auto-maintained by scripts/lessons.py

> Machine-owned. Do NOT hand-edit. Changes are overwritten on the next `lessons.py` write.
> Canonical state lives in `.specs/lessons.json`. Edit lessons only via the script.
> promote_threshold=2 distinct features · window_days=45 · quarantine_threshold=2

## Confirmed (load these at Specify/Design)

Corroborated across multiple features. Safe to apply as guidance.

_none_

## Candidates (under observation — do NOT load as guidance yet)

Seen once or not yet corroborated. Tracked, not trusted.

### L-001 — Numeric-coefficient fixes (restart/discontinuity conventions) need coefficient-level unit tests: integration-level suites mask O(h) errors whenever restarts begin at tiny steps (1e-3*dt), so assert the exact coefficient tuple.
- signal: `surviving_mutant` · recurrence: 1 feature(s) · scope: `solver/math` · harmful: 0
- features: solver-live-params
- evidence: piperine-solver/src/math/integration.rs:197 stage_coeffs backward-Euler degradation (solver/math)
- last seen: 2026-07-17T19:20:56Z

## Quarantined (failed when applied — ignore)

A confirmed lesson that recurred alongside failure. Kept for the maintainer to review.

_none_
