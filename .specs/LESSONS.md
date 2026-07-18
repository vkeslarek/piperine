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

### L-002 — Docstring-walk gates must assert an object's own __doc__ (or __dict__ doc), never inspect.getdoc: Python 3.12+ getdoc inherits docstrings from documented non-object bases (Enum), so removing a subclass's own class docstring passes the gate.
- signal: `surviving_mutant` · recurrence: 1 feature(s) · scope: `crates/piperine-python/tests` · harmful: 0
- features: bench-removal
- evidence: facade_hygiene.rs:23 / mutant M6a (crates/piperine-python/tests)
- last seen: 2026-07-18T00:55:28Z

### L-003 — Vocabulary-removal features must grep string literals and error messages, not just identifiers/AST: 'bench root module not found' survived total bench removal in a reachable pub-API error because greps targeted code symbols only.
- signal: `ac_gap` · recurrence: 1 feature(s) · scope: `crates/piperine-lang` · harmful: 0
- features: bench-removal
- evidence: crates/piperine-lang/src/pom/design.rs:385 / BRM-02 (crates/piperine-lang)
- last seen: 2026-07-18T00:55:38Z

### L-004 — A logged SPEC_DEVIATION that changes user-visible CLI behavior still needs a test asserting the new behavior (the piperine run .phdl migration notice is implemented but unasserted).
- signal: `spec_deviation` · recurrence: 1 feature(s) · scope: `crates/piperine-cli` · harmful: 0
- features: bench-removal
- evidence: crates/piperine-cli/src/commands/run.rs:43-49 / SPEC_DEVIATION 0b952a4 (crates/piperine-cli)
- last seen: 2026-07-18T00:55:48Z

## Quarantined (failed when applied — ignore)

A confirmed lesson that recurred alongside failure. Kept for the maintainer to review.

_none_
