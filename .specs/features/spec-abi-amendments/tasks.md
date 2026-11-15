# Spec ABI Amendments Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: none - the feature edits prose; the decisions are in `spec.md` (Assumptions) and abi.md §15.
**Status**: Draft
**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run spec**: `cargo test -p piperine-lang --test spec_text`
**Run lang**: `cargo test -p piperine-lang`

Executors: Sonnet sub-agents, one commit per task. Verifier: Opus. Work in `/home/keslarek/Git/piperine-solver-completeness`, branch `feature/solver-completeness`. Never pass `--workspace`, never run `cargo clean`, do not set `CARGO_TARGET_DIR`, do not push.

Rules every task follows:

- **MD-57.** The prose is written with its guard in the same commit: one test in `crates/piperine-lang/tests/spec_text.rs` that asserts a row, a code or a short phrase inside the named section (use the file's `spec` and `section` helpers), never a whole sentence. No existing guard is relaxed.
- **The wording source.** ABI amendments take the wording of `.specs/features/device-abi/abi.md` §15 (column "Proposed change", as trimmed by the critic's verdict). Solver amendments describe the code named in the task; read the functions before writing.
- **Normative style.** Keep the page's own voice: numbered rules, `SHALL`/`MAY` in bold where the page uses it, `> **Resolution.**` notes for decided ambiguities. An amended rule keeps its number.
- **No solver code changes.** If the prose cannot be made true of the present solver, stop and report.
- A new test has exactly the name its check cites, no `#[ignore]`, no macro.
- Commit message as given per task, with the trailer `Co-Authored-By:` the session gives.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `.specs/STATE.md` MD-57 (a spec edit lands with its `spec_text` guard), MD-31 (an invariant lives in a gate), `CLAUDE.md` ("zero warnings is the bar").

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Specification prose (`docs/spec/*.md`) | unit | one `spec_text` test per requirement, asserting the decided phrase inside its section | `crates/piperine-lang/tests/spec_text.rs` | `cargo test -p piperine-lang --test spec_text` |
| Existing spec guards (`msl_corpus`, `pom_corpus`, `pom_docs`, `msl_errors`, `suite_hygiene`) | unit | all pass unchanged after every commit | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | never alone: every task edits a page other guards read | `cargo test -p piperine-lang --test spec_text` |
| Full | every task | `cargo test -p piperine-lang` |
| Build | last task of each phase | Full, plus `cargo build -p piperine-lang --all-targets` with zero warnings |

---

## Execution Plan

Phases are ordered and run sequentially. Two batches, one per phase (7 and 4 tasks).

### Phase 1: The accepted ABI amendments

```
T1 → T2 → T3 → T4 → T5 → T6 → T7
```

### Phase 2: The solver amendments

```
T8 → T9 → T10 → T11
```

---

## Task Breakdown

### T1: Amendment 1: a commit that always advances time is not a time point

**What**: Rewrite the second bullet of §17.2 rule 2 with the wording of abi.md §15 row 1 and add its guard.
**Where**: `docs/spec/part_v_execution.md` (§17.2 rule 2; test in `crates/piperine-lang/tests/spec_text.rs`)
**Depends on**: None
**Reuses**: `spec`, `section` and `all_specs` helpers of `crates/piperine-lang/tests/spec_text.rs`; the wording of abi.md §15
**Requirement**: SAA-01, SAA-12

**Done when**:

- [x] The bullet contains the phrases `at that instant` and `by at least one tick`, and says the due time of such a commit is a scheduled time tested on its own
- [x] A1 of the advance procedure and §15 "Event times that are not time points" do not contradict the bullet (read them; edit only if they do)
- [x] Test `later_commit_is_not_a_time_point` asserts both phrases inside the section `### 17.2`
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): a commit that always advances time is not a time point (ABI amendment 1)`

---

### T2: Amendment 2: a time point runs the time-point procedure

**What**: Rewrite the definition of *time point* in §15 with the wording of abi.md §15 row 2.
**Where**: `docs/spec/part_v_execution.md` (§15 definition; test in `spec_text.rs`)
**Depends on**: T1
**Reuses**: `spec`, `section` and `all_specs` helpers of `crates/piperine-lang/tests/spec_text.rs`; the wording of abi.md §15
**Requirement**: SAA-02, SAA-12

**Done when**:

- [x] The definition says a time point is a time at which the analog solver produces an accepted solution `and runs the time-point procedure`
- [x] It says a simulator `MAY accept solutions at other times`, that those are not time points and that `settle` does not fire there
- [x] §17.2 rule 4 still says instance state advances on every accepted step, and the glossary row of `index.md` for *time point* matches the new definition
- [x] Test `time_point_runs_the_procedure` asserts both quoted phrases inside `## 15.`
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): a time point is an accepted solution that runs the procedure (ABI amendment 2)`

---

### T3: Amendment 3a: the C interface of an extern module is the device ABI

**What**: State in §8.5.8 and its contract table that an extern module is reached through the device ABI, and close G.65.
**Where**: `docs/spec/part_iii_structure.md` (§8.5.8, contract table, the note that cites G.65; `appendix_g_scope.md` G.65; test in `spec_text.rs`)
**Depends on**: T2
**Reuses**: `spec`, `section` and `all_specs` helpers of `crates/piperine-lang/tests/spec_text.rs`; the wording of abi.md §15
**Requirement**: SAA-03, SAA-12

**Done when**:

- [x] §8.5.8 names the device ABI and its exported symbol `piperine_abi_1`; `"cosim"` is a module whose reaction is irreversible; `"osdi"` names an adapter over the same ABI
- [x] §8.5.11, §8.5.15 and §8.5.16 are unchanged (3b is not accepted): `"cosim"` ports stay discrete-only
- [x] The G.65 row of Appendix G no longer reads `Open`; its section says what closed it, in the status vocabulary the appendix already uses
- [x] Test `extern_c_interface_is_the_device_abi` asserts `piperine_abi_1` inside §8.5 and that the G.65 table row does not contain `Open`
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): the extern C interface is the device ABI (ABI amendment 3a)`

---

### T4: Amendment 4: a structure-changing `set` on an extern module is refused

**What**: Extend §8.5.10 and §23.1 rule 7 so that `E0803` covers a `set` that would change the structure of an extern module.
**Where**: `docs/spec/part_iii_structure.md` (§8.5.10; `part_viii_host.md` §23.1 rule 7 and the `E0803` row; test in `spec_text.rs`)
**Depends on**: T3
**Reuses**: `spec`, `section` and `all_specs` helpers of `crates/piperine-lang/tests/spec_text.rs`; the wording of abi.md §15
**Requirement**: SAA-04, SAA-12

**Done when**:

- [x] §8.5.10 says a `set` that would change the structure of an extern module is refused like a structural param
- [x] §23.1 rule 7 and the `E0803` row say the same, and that in a `Dc` sweep that point re-elaborates the instance
- [x] `E0803` is still defined once (the existing code guard of `spec_text.rs` passes)
- [x] Test `structure_changing_set_is_refused` asserts `E0803` and `extern` inside §23.1 rule 7's section and `re-elaborates` in the same section
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): a structure-changing set on an extern module is refused (ABI amendment 4)`

---

### T5: Amendment 6: the canonical byte form of a value

**What**: Add the two accepted sentences on the byte form: one in Part I §2 (`Repr`), one in Part II §5.3.
**Where**: `docs/spec/part_i_values.md` (§2; `part_ii_carriers.md` §5.3.4 to §5.3.6; test in `spec_text.rs`)
**Depends on**: T4
**Reuses**: `spec`, `section` and `all_specs` helpers of `crates/piperine-lang/tests/spec_text.rs`; the wording of abi.md §15
**Requirement**: SAA-05, SAA-12

**Done when**:

- [x] Part I §2 says `all-zero bytes` are the all-unknown value of a `Repr` type
- [x] Part II §5.3 says the value bytes of an unknown level are zero
- [x] Neither page says where the known flags live ("one known byte per level" was rejected)
- [x] Test `zero_bytes_are_the_unknown_value` asserts `all-zero bytes` in `part_i_values.md` and `value bytes` in the §5.3 section of `part_ii_carriers.md`
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): all-zero bytes are the unknown value (ABI amendment 6)`

---

### T6: Amendment 8: `gmin()` and `source_scale()` are core environment functions

**What**: List the two continuation variables in §14.4 as core environment functions in the class of `now()` and `freq()`.
**Where**: `docs/spec/part_iv_behavior.md` (§14.4 table and rules; every table that enumerates the core environment functions; test in `spec_text.rs`)
**Depends on**: T5
**Reuses**: `spec`, `section` and `all_specs` helpers of `crates/piperine-lang/tests/spec_text.rs`; the wording of abi.md §15
**Requirement**: SAA-06, SAA-12

**Done when**:

- [x] §14.4 lists `gmin()` and `source_scale()` with their class (they change between solves, like `now()` and `freq()`, so they are not constant during an analysis: §14.4.1 is not contradicted), their value at rest (the session `gmin`; 1.0) and the solves in which they differ from it
- [x] §14.4.6 still names no `simparam` session parameter
- [x] Every other list of the core environment functions in `docs/spec/` (grep `now()` and `freq()`: Part VII, Appendix H, `index.md`) carries the two names, and the construct-count guard of `spec_text.rs` passes
- [x] If a guard of `piperine-lang` ties `crates/piperine-lang/src/pom/builtin.rs` to that list, the two rows are added there in this commit; otherwise `builtin.rs` is untouched
- [x] Test `continuation_variables_are_core_functions` asserts `gmin()` and `source_scale()` inside the `### 14.4` section
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): gmin() and source_scale() are core environment functions (ABI amendment 8)`

**Follow-up (B2, own commit, no new task id)**: `gmin` and `source_scale` added as built-in `Fn` rows (§14.4) in `crates/piperine-lang/src/pom/builtin.rs` and `docs/pom/resolution.md`; tests `continuation_variables_are_builtin_functions` and `continuation_variable_redefined_is_e0457` in `pom_resolve.rs` (red first, then green).

---

### T7: Amendment 9: records travel as commits to quiet pins

**What**: Add the implementation note on records to Appendix C.1.
**Where**: `docs/spec/appendix_c_implementation.md` (C.1 notes on single rows; test in `spec_text.rs`)
**Depends on**: T6
**Reuses**: `spec`, `section` and `all_specs` helpers of `crates/piperine-lang/tests/spec_text.rs`; the wording of abi.md §15
**Requirement**: SAA-07, SAA-12

**Done when**:

- [x] A numbered note of C.1 says an `emit!` record travels as a commit to a quiet pin, is delivered stamped with the instant it was made, and adds no microstep
- [x] §19 rule 1 is unchanged
- [x] Test `records_travel_as_quiet_commits` asserts `quiet` and `no microstep` inside the `## C.1` section
- [x] Gate: build (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: build
**Commit**: `docs(spec): records travel as commits to quiet pins (ABI amendment 9)`

---

### T8: §15.2 describes the kept discontinuity algorithm

**What**: Rewrite §15.2 so that a jump has no solve of its own, as the solver behaves today.
**Where**: `docs/spec/part_v_execution.md` (§15.2, step 3.4 of the time-point procedure, the `E0504` row; test in `spec_text.rs`)
**Depends on**: None (phase order: starts after T7 closes the previous phase)
**Reuses**: `spec` and `section` helpers of `spec_text.rs`; the solver sources named in `spec.md` (Assumptions)
**Requirement**: SAA-08, SAA-12

**Done when**:

- [x] §15.2 states, from `transient.rs`: the solver lands exactly on the time of the change (`predict_step`); what is due at microstep 0 is applied before the solve, so the step that arrives there reads it (`attempt_step`); a change made after the step's solve is read by the next, ordinary step and a host `set` applies after acceptance and is read by the next, restarting step (verification F2/F3); the landing step is exempt from the truncation-error test (`assess_step`); the next step restarts the integrator, the trapezoidal stage dropping its previous derivative and running as backward Euler (`accept_step`, `TrBdf2::stage_coeffs`)
- [x] The backward-Euler limit `h → 0`, the repeated solve (old rule 4) and the impulsive flow are gone as normative text; the two `Resolution` notes that depended on them are removed or rewritten
- [x] States under `ddt` are said to be carried by the integrator's history across the jump to the accuracy of the step, not as an exact limit at one time
- [x] Every rule number of §15.2 that another page cites (grep `§15.2.` in `docs/spec/`) still exists or its citing sentence is updated in this commit
- [x] Step 3.4 of the time-point procedure no longer requires a jump solve inside the instant
- [x] `E0504` is defined once and names the failure of a transient step that reads a jump
- [x] Test `jump_has_no_solve_of_its_own` asserts, inside `### 15.2`: the phrase `no solve of its own` is present, and `h → 0` and `repeated once` are absent
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): the jump is the kept discontinuity algorithm (section 15.2)`

---

### T9: Appendix C rows describe the kept jump and restart

**What**: Rewrite the jump and restart rows of Appendix C.1, C.2 and C.2.1 and drop the jump from the G.6 gap list.
**Where**: `docs/spec/appendix_c_implementation.md` (C.1 row "the jump", C.2 rows "the jump" and "restart", C.2.1 row "restart step after a jump"; `appendix_g_scope.md` G.6; test in `spec_text.rs`)
**Depends on**: T8
**Reuses**: `spec` and `section` helpers of `spec_text.rs`; the solver sources named in `spec.md` (Assumptions)
**Requirement**: SAA-09, SAA-12

**Done when**:

- [x] C.1 and C.2 rows "the jump (§15.2)" describe the algorithm of T8 and carry no `gap (G.6)`
- [x] C.2 row "restart" says: the previous derivative is dropped for one trapezoidal stage after every jump; the step size restarts at 10⁻³ of the last step, floored at the minimum step, after a host `set` only; any other jump keeps the controller's proposal (`propose_dt`, `sets_just_applied`)
- [x] C.2.1 row "restart step after a jump" says the same condition
- [x] G.6 no longer lists "the jump as a zero-length backward-Euler solve"; the "Highest impact" line of Appendix G no longer names the jump solve
- [x] Test `reference_rows_describe_the_kept_jump` asserts: no line of `appendix_c_implementation.md` that starts with `| the jump` contains `gap`, and the `| restart` row contains `set`
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): Appendix C describes the kept jump and restart`

---

### T10: §17.1.2 states the present residual mapping

**What**: Rewrite rule 2 of §17.1 and its `Resolution` to the mapping of `Tolerances::residual_test`.
**Where**: `docs/spec/part_v_execution.md` (§17.1 rule 2 and its Resolution; test in `spec_text.rs`)
**Depends on**: T9
**Reuses**: `spec` and `section` helpers of `spec_text.rs`; the solver sources named in `spec.md` (Assumptions)
**Requirement**: SAA-10, SAA-12

**Done when**:

- [x] Rule 2 says: a node's row is tested against the potential `abstol` of that node (the smallest potential `abstol` of the natures meeting there, default 1 µV), a branch row against the flow `abstol` (default 1 pA), each plus `reltol · scale`
- [x] A sentence says this is the mapping the reference implementation's ngspice parity was validated with
- [x] Rule 1 (the update test) is compared with `Tolerances::has_converged` and corrected only where it states something the code does not do
- [x] C.2.1's tolerance row agrees with the rule
- [x] Test `residual_test_uses_the_present_mapping` asserts, inside `### 17.1`, that the sentence of rule 2 about a node's row contains `potential` and the one about a branch row contains `flow`
- [x] Gate: full (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: full
**Commit**: `docs(spec): the residual test uses the present tolerance mapping (section 17.1.2)`

---

### T11: The dependents of the jump follow §15.2

**What**: Redefine the jump solve in the "Solves" table of §15 and update the glossary and the F.5 walkthrough.
**Where**: `docs/spec/part_v_execution.md` ("Solves" table; `index.md` glossary rows; `appendix_f_gallery_mixed.md` F.5; test in `spec_text.rs`)
**Depends on**: T10
**Reuses**: `spec` and `section` helpers of `spec_text.rs`; the solver sources named in `spec.md` (Assumptions)
**Requirement**: SAA-11, SAA-12

**Done when**:

- [x] The "Solves" table defines the jump solve as the transient step that reads a jump (the step arriving at its time point, or the first one after it), so the "Jump solve" columns of Parts IV and VII need no edit
- [x] `index.md`: the rows for *jump, right value*, *impulsive flow* and *state (of the jump rule)* match the new §15.2 (a row whose term no longer exists is removed)
- [x] F.5 says its right values (0.25 V on both capacitors) are those of the first step that reads the jump, and drops the `h → 0` derivation as normative
- [x] No other sentence of `docs/spec/` says a jump is solved at the same `t` (grep `jump solve`, `right value`, `second pass`)
- [x] Test `jump_solve_is_a_transient_step` asserts the "Solves" table row that starts with `| **jump solve**` contains `transient step`
- [x] Gate: build (`cargo test -p piperine-lang` green; test count is the previous count plus 1)

**Tests**: unit
**Gate**: build
**Commit**: `docs(spec): the dependents of the jump follow section 15.2`

---

## Checks

**C1** - The second bullet of §17.2 rule 2 carries `at that instant` and `by at least one tick` (SAA-01, AC 1)
Proof: run spec | test later_commit_is_not_a_time_point ... ok

**C2** - §15 defines a time point as an accepted solution that runs the time-point procedure, and allows accepted solutions elsewhere (SAA-02, AC 2)
Proof: run spec | test time_point_runs_the_procedure ... ok

**C3** - §8.5.8 names the device ABI by `piperine_abi_1` and the G.65 row is not `Open` (SAA-03, AC 3 and 4)
Proof: run spec | test extern_c_interface_is_the_device_abi ... ok

**C4** - §23.1 rule 7 refuses a structure-changing `set` on an extern module with `E0803` and re-elaborates in a `Dc` sweep (SAA-04, AC 5)
Proof: run spec | test structure_changing_set_is_refused ... ok

**C5** - Part I says all-zero bytes are the all-unknown value and Part II says an unknown level's value bytes are zero (SAA-05, AC 6 and 7)
Proof: run spec | test zero_bytes_are_the_unknown_value ... ok

**C6** - §14.4 lists `gmin()` and `source_scale()` (SAA-06, AC 8)
Proof: run spec | test continuation_variables_are_core_functions ... ok

**C7** - Appendix C.1 notes that records are commits to quiet pins and add no microstep (SAA-07, AC 9)
Proof: run spec | test records_travel_as_quiet_commits ... ok

**C8** - §15.2 says a jump has no solve of its own and no longer states `h → 0` or a repeated solve (SAA-08, AC 1 to 4)
Proof: run spec | test jump_has_no_solve_of_its_own ... ok

**C9** - No jump row of Appendix C carries a gap mark and the restart row names the host `set` (SAA-09, AC 5 to 7)
Proof: run spec | test reference_rows_describe_the_kept_jump ... ok

**C10** - §17.1 rule 2 tests a node's row against the potential `abstol` and a branch row against the flow `abstol` (SAA-10, AC 8)
Proof: run spec | test residual_test_uses_the_present_mapping ... ok

**C11** - The "Solves" table defines the jump solve as a transient step (SAA-11, AC 9 and 10)
Proof: run spec | test jump_solve_is_a_transient_step ... ok

**C12** - Every guard of `piperine-lang` passes on the amended pages (SAA-12, AC 10)
Proof: `cargo test -p piperine-lang`

---

## Handoff

**Settled mid-build:** none
**Dispatched:** B1 at dd805f01 - returned (T1-T7 ae9ae5ae..90693bba; gap: gmin/source_scale not in POM builtin table); B2 (T8-T11, Sonnet) at 7ff301da - returned (T8-T11 4cf94794..1446533b + gmin/source_scale builtin 7572e5e3; gap: cross caused by a jump wording vs solver); Verifier (Opus) at b419ac9c - returned (FAIL F1-F7: spec text vs solver code); Fix pass (Opus: delicate spec semantics, small) at a5ea8aa3 - returned (fix pass 63d43683..f0cfb5a1, F1-F7 closed; AC text aligned by orchestrator)
**Settled mid-build:** gmin()/source_scale() become POM built-in rows (builtin.rs + docs/pom/resolution.md) in B2 so a model calling them resolves and redefining them is E0457 (orchestrator)
**Walked:** not applicable - no screen

---

## Phase Execution Map

```
Phase 1 → Phase 2

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6 → T7
Phase 2:  T8 → T9 → T10 → T11
```

Batches: Phase 1 (7 tasks), Phase 2 (4 tasks). Phase 2 starts after T7 because both phases commit to `part_v_execution.md` and `spec_text.rs`.

## Task Granularity Check

Each task is one amendment: one decided wording, one guard, one commit. T3, T4, T5, T9 and T11 touch two or three pages because the amendment itself spans them; splitting would leave the specification inconsistent between commits.

## Test Co-location Validation

Every task edits specification prose (matrix: unit, `spec_text`) and adds its own guard in the same commit. No task says `Tests: none`.
