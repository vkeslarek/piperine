# Spec ABI Amendments Specification

## Problem Statement

The device ABI (`.specs/features/device-abi/abi.md`) and the kept solver algorithm disagree with ten places of `docs/spec/`. The user accepted seven ABI amendments (abi.md §15: 1, 2, 3a, 4, 6, 8, 9) and three solver ones (§15.2, Appendix C.2, §17.1.2 describe the solver's present discontinuity algorithm and tolerance mapping). None has been written. Until they are, the specification states a time-point rule, a jump solve and a residual test that no code will implement.

## Goals

- [ ] `docs/spec/` states the seven accepted ABI amendments, each guarded by one test in `crates/piperine-lang/tests/spec_text.rs` (MD-57).
- [ ] §15.2, Appendix C.2 and §17.1.2 describe what the solver does today, with no "gap" left on the jump row.
- [ ] `cargo test -p piperine-lang` stays green after every commit.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Amendment 3b (`extern "cosim"` with continuous ports) | user: "Not now (Recommended)" |
| Amendments 5 and 7 | withdrawn by the critic of the duel |
| Any change to solver code | the amendments describe present behavior; the ABI lands in `solver-abi-*` |
| Rewriting the per-primitive "Jump solve" columns of Parts IV and VII | they keep their meaning once §15 redefines the jump solve (SAA-11) |
| Moving `abi.md` into `docs/spec/` | not asked; the spec names the ABI by its symbol `piperine_abi_1` |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Which amendments land | 1, 2, 3a, 4, 6, 8, 9 of abi.md §15, plus §15.2, Appendix C.2, §17.1.2 | `codegen/context.md` "Device ABI decisions": "Accept (Recommended)", "Core fns gmin()/source_scale() (Recommended)", "Yes, all of them (Recommended)"; `solver-completeness/context.md` round 3: "Amend §15.2 to the solver (Recommended)" | y |
| Amendment 6 wording | two sentences only: all-zero bytes are the all-unknown value; an unknown level's value bytes are zero | abi.md §15 row 6, critic's trim, accepted with "Yes, all of them" | y |
| Amendment 8 shape | two core environment functions `gmin()` and `source_scale()`, in the class of `now()` and `freq()`; §14.4.6 keeps naming no `simparam` | user: "Core fns gmin()/source_scale() (Recommended)" | y |
| Source of truth for the solver amendments | the code: `crates/piperine-solver/src/analyses/transient.rs` (`predict_step`, `attempt_step`, `assess_step`, `accept_step`, `propose_dt`), `crates/piperine-analog-solver/src/tolerances.rs` (`residual_test`), `math/integration.rs` (`stage_coeffs`) | user, Q7: "eu manteria o algoritmo atual do solver"; Q9: "mantém Q7 e Q9" | y |
| The word "jump" | kept. A jump is still a change at one time of something `analog` reads; what changes is that it has no solve of its own | 170 mentions across seven pages keep their meaning; only §15.2 and its dependents are rewritten | y - follows from Q7 |
| Guard style | one `spec_text` test per requirement asserting a row, a code or a short phrase, never a sentence | MD-57 rule 1 | y - settled by MD-57 |
| `gmin`, `source_scale` in `piperine-lang` | the two rows are added to `crates/piperine-lang/src/pom/builtin.rs` in the commit of SAA-06 only if an existing guard ties that table to §14.4; otherwise the edit is spec-only | MD-57 rule 2: an edit travels with its implementation only when an existing guard reads it | y - settled by MD-57 |

**Open questions:** none - all resolved or logged above.

**Implicit-requirement dimensions.** Input validation, failure states, idempotency, auth, concurrency, data lifecycle, observability, external dependencies, state transitions: N/A because the feature edits prose and adds text guards; the one failure mode (a guard another page trips) is the gate of every task.

---

## User Stories

### P1: The accepted ABI amendments are in the specification ⭐ MVP

**User Story**: As a codegen author, I want the specification to state the time-point rule, the byte form and the extern interface the device ABI relies on, so that the ABI and the language agree before any device is generated.

**Why P1**: `solver-abi-*` and `msl-elab` cite these sentences.

**Acceptance Criteria**:

1. The second bullet of §17.2 rule 2 SHALL say that a counting reaction makes its scheduled time a time point when it can change, at that instant, a carrier that `analog` or a probing reaction reads, and that a commit whose delay always advances time by at least one tick changes nothing at the instant that makes it.
2. The definition of *time point* in §15 SHALL say that a time point is a time at which the analog solver produces an accepted solution and runs the time-point procedure, and that a simulator MAY accept solutions at other times, where `settle` does not fire.
3. §8.5.8 SHALL say that the C interface of an extern module is the device ABI, whose exported symbol is `piperine_abi_1`, that `"cosim"` is a module whose reaction is irreversible and that `"osdi"` names an adapter over that ABI.
4. The row of G.65 in Appendix G SHALL no longer read `Open`.
5. §8.5.10 and §23.1 rule 7 SHALL say that a `set` which would change the structure of an extern module is refused with `E0803` during a run, and that in a `Dc` sweep that point re-elaborates the instance.
6. §2 of Part I SHALL say that all-zero bytes are the all-unknown value of a `Repr` type.
7. §5.3 of Part II SHALL say that the value bytes of an unknown level are zero.
8. §14.4 SHALL list `gmin()` and `source_scale()` as core environment functions, and §14.4.6 SHALL still name no `simparam` session parameter.
9. Appendix C SHALL carry a note under the `emit!` row saying that records travel as commits to quiet pins and add no microstep.
10. WHEN the test suite of `piperine-lang` runs after any commit of this feature THEN every test SHALL pass.

**Independent Test**: `cargo test -p piperine-lang --test spec_text` passes with seven new tests, one per amendment.

---

### P1: The specification describes the solver's discontinuity algorithm and tolerances ⭐ MVP

**User Story**: As a solver author, I want §15.2, Appendix C.2 and §17.1.2 to describe what the solver does, so that the unit tests of `solver-abi-*` pin the specified behavior and not a deviation.

**Why P1**: MD-60 item 4 owes these amendments; ngspice parity depends on the described behavior staying as it is.

**Acceptance Criteria**:

1. §15.2 SHALL say that a jump has no solve of its own: a change due at microstep 0 of a time point is read by the step that arrives there, and a change made in a later microstep is read by the first step after it.
2. §15.2 SHALL say that the step landing on a jump is exempt from the truncation-error test and that the next step restarts the integrator: the trapezoidal stage drops its previous derivative and runs as backward Euler.
3. §15.2 SHALL NOT state the backward-Euler limit `h → 0`, the repeated solve or the impulsive flow as normative.
4. The row of `E0504` SHALL stay defined exactly once and SHALL name the failure of a transient step that reads a jump.
5. The rows "the jump" of Appendix C.1 and C.2 SHALL describe the kept algorithm and SHALL carry no `gap (G.6)` mark.
6. The row "restart" of Appendix C.2 SHALL say that the step size restarts at 10⁻³ of the last step after a host `set` only, and that every other jump keeps the controller's proposal.
7. The row of G.6 that lists "the jump as a zero-length backward-Euler solve" SHALL be removed from the gap list.
8. §17.1 rule 2 SHALL say that the residual of a node's row is tested against the potential `abstol` of that node and the residual of a branch row against the flow `abstol`, and SHALL say that this is the mapping ngspice parity was validated with.
9. The "Solves" table of §15 SHALL define the jump solve as the transient step that reads a jump, so that every "Jump solve" column of Parts IV and VII keeps a meaning.
10. The walkthrough of Appendix F.5 SHALL say that its right values are those of the first step that reads the jump.

**Independent Test**: `cargo test -p piperine-lang --test spec_text` passes with four new tests: the jump, the reference rows, the residual mapping, the dependents.

---

## Edge Cases

- IF an existing guard of `piperine-lang` (`msl_corpus`, `pom_corpus`, `pom_docs`, `msl_errors`, `spec_text`) fails on an amended page THEN the task SHALL fix the page, never the guard.
- IF an amendment needs a new error code THEN the task SHALL stop and report, because no accepted amendment adds one.
- WHEN a page cross-references §15.2.1 to §15.2.10 by rule number THEN the amended §15.2 SHALL keep a rule under each number that is still cited, or the citing sentence SHALL be updated in the same commit.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SAA-01 | P1: ABI amendments, AC 1 (amendment 1, `LATER`) | Tasks | In Tasks |
| SAA-02 | P1: ABI amendments, AC 2 (amendment 2, time point) | Tasks | In Tasks |
| SAA-03 | P1: ABI amendments, AC 3, 4 (amendment 3a, extern interface) | Tasks | In Tasks |
| SAA-04 | P1: ABI amendments, AC 5 (amendment 4, structural `set`) | Tasks | In Tasks |
| SAA-05 | P1: ABI amendments, AC 6, 7 (amendment 6, byte form) | Tasks | In Tasks |
| SAA-06 | P1: ABI amendments, AC 8 (amendment 8, continuation variables) | Tasks | In Tasks |
| SAA-07 | P1: ABI amendments, AC 9 (amendment 9, records) | Tasks | In Tasks |
| SAA-08 | P1: Solver amendments, AC 1 to 4 (§15.2) | Tasks | In Tasks |
| SAA-09 | P1: Solver amendments, AC 5 to 7 (Appendix C, G.6) | Tasks | In Tasks |
| SAA-10 | P1: Solver amendments, AC 8 (§17.1.2) | Tasks | In Tasks |
| SAA-11 | P1: Solver amendments, AC 9, 10 (dependents of the jump) | Tasks | In Tasks |
| SAA-12 | P1: ABI amendments, AC 10 (every commit green) | Tasks | In Tasks |

**Coverage:** 12 total, 12 mapped to tasks, 0 unmapped.

---

## Sources

- `.specs/features/device-abi/abi.md` §15: the amendment table, the critic's verdicts, the exact proposed wording.
- `.specs/features/codegen/context.md`, "Device ABI decisions (user, 2026-11-15)": which amendments are accepted.
- `.specs/features/solver-completeness/context.md`, rounds 1 to 3: Q7, Q9 and "Amend §15.2 to the solver".
- `.specs/STATE.md` MD-57 (how a spec edit lands), MD-60 item 4 (the amendments owed).
- `crates/piperine-solver/src/analyses/transient.rs`, `crates/piperine-analog-solver/src/tolerances.rs`, `crates/piperine-analog-solver/src/math/integration.rs`: the behavior the solver amendments describe.

---

## Success Criteria

- [ ] Eleven new tests in `spec_text.rs` pass, and each fails when its amendment is reverted.
- [ ] `grep -n "gap (G.6)" docs/spec/appendix_c_implementation.md` no longer matches a jump row.
- [ ] `cargo test -p piperine-lang` is green at every commit.
