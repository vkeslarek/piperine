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

### L-005 — Verifying a CLI notice by asserting its text is not enough: drive the command with adversarial inputs (wrong positional arg, broken input file) — 'piperine run other.phdl' silently ignored the arg and a broken design still printed 'elaborates' exit 0 through three verifier rounds.
- signal: `ac_gap` · recurrence: 1 feature(s) · scope: `crates/piperine-cli` · harmful: 0
- features: bench-removal
- evidence: crates/piperine-cli/src/commands/run.rs:30-49 / BRM-07 round 4 (crates/piperine-cli)
- last seen: 2026-07-18T01:58:47Z

### L-006 — Interpolation or ratio tests must use non-uniform sample spacing so a dropped division or normalization is discriminated
- signal: `surviving_mutant` · recurrence: 1 feature(s) · scope: `codegen` · harmful: 0
- features: p1-solver-complete
- evidence: crates/piperine-codegen/src/lower/pom/analog_ops.rs:180 (mutant M4) (codegen)
- last seen: 2026-07-19T00:20:01Z

### L-007 — Every fail-loud validation branch needs its own negative test that trips it; unknown-name tests alone leave guard clauses undiscriminated
- signal: `surviving_mutant` · recurrence: 1 feature(s) · scope: `solver` · harmful: 0
- features: p1-solver-complete
- evidence: crates/piperine-solver/src/solver/sens.rs:59 (mutant M8, SC-02) (solver)
- last seen: 2026-07-19T00:20:01Z

### L-008 — A Done-when that names a specific diagnostic case must have a test asserting that diagnostic message before the task is marked done
- signal: `ac_gap` · recurrence: 1 feature(s) · scope: `solver` · harmful: 0
- features: p1-solver-complete
- evidence: SC-05, crates/piperine-solver/src/solver/pss.rs:294 verify_digital_periodicity (solver)
- last seen: 2026-07-19T00:20:01Z

### L-009 — When the spec names an independent reference method, assert against that reference, not a self-consistency proxy
- signal: `spec_precision_gap` · recurrence: 1 feature(s) · scope: `solver` · harmful: 0
- features: p1-solver-complete
- evidence: SC-01, tests/sens.rs:111 (solver)
- last seen: 2026-07-19T00:20:12Z

### L-010 — A spec claim of golden-file validation requires a checked-in fixture pair; grep the fixtures directory before closing the task
- signal: `spec_precision_gap` · recurrence: 1 feature(s) · scope: `spice` · harmful: 0
- features: p1-solver-complete
- evidence: SC-15, tests/ngspice/ (no tline .cir pair) (spice)
- last seen: 2026-07-19T00:20:12Z

### L-011 — When a task audits 'every module has a //! contract', enumerate modules from the filesystem (all src/**/*.rs), not from the design's layer table — top-level modules outside the table (error.rs, result.rs) were missed.
- signal: `ac_gap` · recurrence: 1 feature(s) · scope: `piperine-solver` · harmful: 0
- features: solver-simplification
- evidence: SS-14 / crates/piperine-solver/src/error.rs:1,result.rs:1 (piperine-solver)
- last seen: 2026-07-19T16:34:44Z

### L-012 — When a metadata sidecar is resolved at the POM/lang layer and consumed at the codegen layer, attributes attached to POM nodes that do not surface in codegen's runtime catalogs (e.g. a var shadowed/never-assigned so the kernel omits it from opvar_names) must be cross-checked at the codegen boundary — silently dropping the orphan metadata violates fail-loud.
- signal: `ac_gap` · recurrence: 1 feature(s) · scope: `piperine-lang/pom, piperine-codegen/device, fail-loud` · harmful: 0
- features: phdl-introspection-attributes
- evidence: .specs/features/phdl-introspection-attributes/validation.md Edge Cases bullet 3 (orphan @unit/@description on shadowed var); pom/design.rs:469-500; device/mod.rs:428-440 (piperine-lang/pom, piperine-codegen/device, fail-loud)
- last seen: 2026-07-24T02:29:17Z

### L-013 — A guard that only scans a clean tree passes even when its detector is broken: extract each check into a named predicate and test it against fixture inputs (must-flag and must-not-flag), or the guard is verified only by a human remembering to inject a violation.
- signal: `surviving_mutant` · recurrence: 1 feature(s) · scope: `policy guards` · harmful: 0
- features: p6-cleanup-completeness
- evidence: tests/suite_hygiene.rs / mutants M3+M4 (policy guards)
- last seen: 2026-07-26T03:33:18Z

### L-014 — Before writing 'delete the unenforceable rule' into a spec, check whether the rule is unreachable or merely unenforced — an unenforced-but-reachable rule needs its status stated in place, since deleting it trades documentation drift for a silent capability regression.
- signal: `spec_deviation` · recurrence: 1 feature(s) · scope: `docs/spec` · harmful: 0
- features: p6-cleanup-completeness
- evidence: CLN-18 / audit.md §7b (docs/spec)
- last seen: 2026-07-26T03:33:18Z

### L-015 — Never plan against a quoted test count: grep the tree for #[test] and reconcile against the runner's number, because a #![cfg(any())] file contributes tests to the source count and zero to the gate.
- signal: `ac_gap` · recurrence: 1 feature(s) · scope: `test suite` · harmful: 0
- features: p6-cleanup-completeness
- evidence: audit.md §1 reconciliation / ROADMAP P6 (test suite)
- last seen: 2026-07-26T03:33:18Z

### L-016 — When one task's dispatcher must call into a sibling module the compiler requires to compile as a whole unit, ship that module's real implementation in the dispatcher's commit instead of a stub, and record it as a SPEC_DEVIATION rather than forcing an artificial per-task stub boundary.
- signal: `spec_deviation` · recurrence: 1 feature(s) · scope: `rust-module-structure` · harmful: 0
- features: dv-gradients
- evidence: crates/piperine-api/src/driver/entry.rs commit 3ed7501 (T16) (rust-module-structure)
- last seen: 2026-07-31T03:10:19Z

### L-017 — When an importance-sampling or distribution-shift estimator only shifts one distribution family (e.g. gauss with resolvable sigma), state the narrowed scope in the type's own doc comment and keep every other distribution unbiased-but-unshifted (likelihood ratio 1), rather than silently approximating or blocking on full multivariate support.
- signal: `spec_deviation` · recurrence: 1 feature(s) · scope: `sampling` · harmful: 0
- features: dv-gradients
- evidence: crates/piperine-api/src/driver/high_sigma.rs:22, crates/piperine-api/src/sampling.rs:288 (T20) (sampling)
- last seen: 2026-07-31T03:10:19Z

### L-018 — A settled-only/post-event fixture must use a net whose value at the mutated (wrong) dispatch point genuinely differs from its correct value — an initial-equals-final glitch fixture cannot discriminate stale-vs-settled dispatch.
- signal: `surviving_mutant` · recurrence: 1 feature(s) · scope: `test-design` · harmful: 0
- features: dv-tester
- evidence: crates/piperine-solver/tests/digital_settle_subscriber.rs (mutation: dispatch pre-settle snapshot instead of post-settle) (test-design)
- last seen: 2026-08-01T19:54:53Z

### L-019 — Any window/breakpoint-publishing helper needs an explicit boundary test placing an event exactly at the window's edge, not just interior placements — off-by-boundary mutations on inclusive ranges routinely survive interior-only test suites.
- signal: `surviving_mutant` · recurrence: 1 feature(s) · scope: `test-design` · harmful: 0
- features: dv-tester
- evidence: crates/piperine-signal/src/wave.rs push_in() window boundary (t <= end vs t < end) (test-design)
- last seen: 2026-08-01T19:54:53Z

### L-020 — When a spec AC asks for N properties to compose together, verify one test exercises all N simultaneously — tests proving N-1 of them plus a separate task boundary comment ('the Nth is a later phase's job') is not evidence the composition was ever closed.
- signal: `ac_gap` · recurrence: 1 feature(s) · scope: `spec-verification` · harmful: 0
- features: dv-tester
- evidence: DVT-30 / T18 done-when 'all three levels compose in one host device class' (spec-verification)
- last seen: 2026-08-01T19:54:53Z

### L-021 — When a fix-round test proves a spec's claimed mapping/behavior does not hold, also correct or SPEC_DEVIATION-annotate the spec doc that made the claim — a test-comment-only negative leaves the formal spec still asserting the false positive.
- signal: `spec_deviation` · recurrence: 1 feature(s) · scope: `spec-verification` · harmful: 0
- features: dv-tester
- evidence: docs/spec/part_viii_host_api.md:919, .specs/features/dv-tester/ideal.md:558-568 vs crates/piperine-api/src/sequencer/{driver.rs,crossing.rs} (spec-verification)
- last seen: 2026-08-01T20:57:53Z

### L-022 — A CSE/dedup claim (collapses to one call) needs an assertion that counts the actual emitted call or instruction sites, not just a numeric result that would be identical whether dedup ran once or twice.
- signal: `spec_precision_gap` · recurrence: 1 feature(s) · scope: `codegen` · harmful: 0
- features: codegen-simplification
- evidence: validation.md AC 11 (codegen)
- last seen: 2026-08-04T02:10:25Z

### L-023 — A clamp or boundary claim on a piecewise function needs a test that drives the input past both ends, not just one; a fixture that only ever increases from zero silently skips the low-end clamp.
- signal: `spec_precision_gap` · recurrence: 1 feature(s) · scope: `codegen` · harmful: 0
- features: codegen-simplification
- evidence: validation.md AC 18 (codegen)
- last seen: 2026-08-04T02:10:25Z

### L-024 — When a Tn task ships a docs rewrite (ROADMAP/spec) and later commits (Fix A/B, retire-triple) close items the docs already described as open, the docs MUST be re-synced in the same commit or the next docs commit — otherwise the traceability table and ROADMAP contradict the measured state.
- signal: `spec_precision_gap` · recurrence: 1 feature(s) · scope: `docs/traceability` · harmful: 0
- features: ngspice-parity
- evidence: ROADMAP.md:94; passives.phdl:255-257; mos.phdl:507-513 (docs/traceability)
- last seen: 2026-08-07T02:43:33Z

## Quarantined (failed when applied — ignore)

A confirmed lesson that recurred alongside failure. Kept for the maintainer to review.

_none_
