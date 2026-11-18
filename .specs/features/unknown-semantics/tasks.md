# Unknown Semantics Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/unknown-semantics/design.md`
**Status**: Approved
**Approved**: 2026-11-18 - user: "Eu gostaria que engatilhasse até o 12"
**Build**: `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run typesunknowns**: `cargo test -p piperine-lang --test types_unknowns`
**Run elabunknowns**: `cargo test -p piperine-lang --test elab_unknowns`
**Run sourceunknowns**: `cargo test -p piperine-parity --test source_unknowns`
**Run rt**: `cargo test -p piperine-transpile --lib rt::`
**Run reaction**: `cargo test -p piperine-transpile --test reaction`
**Run abilayouts**: `cargo test -p piperine-abi --test layouts`
**Run abicaller**: `cargo test -p piperine-abi --test caller`
**Run queue**: `cargo test -p piperine-digital-solver --test queue`
**Run instant**: `cargo test -p piperine-digital-solver --test instant`
**Run irdigital**: `cargo test -p piperine-ir --test digital`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run planshape**: `cargo test -p piperine-plan --test digital_shape`
**Run records**: `cargo test -p piperine-api --test records`
**Run verdict**: `cargo test -p piperine-api --lib verdict`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run docs**: `cargo test -p piperine-cli --test docs`
**Run book**: `cargo test -p piperine-cli --test book`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_commits --test buck_records --test buck_supervisor --test buck_time --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** The spec's Q1 is (b) (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"): `merge`, `same` and `cast` are typed and lowered by their §22.1 contract at the concrete type until `generics`; the loop bound under unknown control is 64, written in Appendix C.2. Every task follows them as written.

Rules every task follows:

- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-c CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-c` before any cargo command. Cargo with `-p <crate>`, never `--workspace`, never `cargo clean`. Commands run in the foreground; no polling loop. If `df /` passes 90 %, stop and report.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves. Every buck target passes after every task that touches the printer, the runtime or the solver (the buck targets above, `cargo test -p piperine-cli --test buck`, `cargo test -p piperine-api --test buck_long`).
- **Only design.md "Goldens, pins and tests that may move" may move,** under the task it names; each replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both and the spec line (the spec's assumption "The fault of an unknown `if`: removed, with no session option to restore it").
- **The code follows the spec.** Where today's behaviour contradicts the spec for a construct this feature touches, the task fixes it; nothing is left as an accepted divergence. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values come from the spec's criteria, §3.3, §1 rule 13, §19 and §22.1, each cited at its assertion. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (user rules, `.specs/features/code-clarity/spec.md`, and rule 8).** Readable at a glance; the module tree says where each thing is; each crate one purpose; no free function (every function has an owner type); one purpose per type, no abbreviated name; no god method and no needless abstraction; the data path evident. Every new type, method, field and variant carries a `///` doc comment; a non-evident block cites its spec rule. AGENTS.md idiom rules bind (MD-13): no macro, no `unwrap()`/`expect()` on a path user input reaches.
- **Conventions.** New IR `Expr` variants, plan `Source` variants and `Env`-like enums are appended last (msl-digital, msl-fixes). The interpreter refuses every new IR form by name (codegen context R1). Every new test target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` is appended to, never edited above; `GAPS.md` keeps its section numbering; `piperine-plan`, `piperine-api` and `piperine-cli` change only where a task says.
- Zero warnings on every crate the `**Build**` line names. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured), MD-57 (spec edits with `spec_text.rs`), MD-63 (the compiled backend).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Checker (`piperine-lang/src/types`) | integration | every AC the checker decides; each error code with its span and names | `crates/piperine-lang/tests/types_unknowns.rs` | `cargo test -p piperine-lang --test types_unknowns` |
| Elaborator (`piperine-lang/src/elab`) | integration | every elaboration-time AC; each error code | `crates/piperine-lang/tests/elab_unknowns.rs` | `cargo test -p piperine-lang --test elab_unknowns` |
| IR (`piperine-ir`) | integration | each new node typed, printed, checked, its dependencies | `crates/piperine-ir/tests/digital.rs` | `cargo test -p piperine-ir --test digital` |
| Interpreter refusals (`piperine-interp`) | integration | each new form refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Plan (`piperine-plan`) | integration | each layout and read change on a hand-built body | `crates/piperine-plan/tests/digital_shape.rs` | `cargo test -p piperine-plan --test digital_shape` |
| Runtime (`piperine-transpile/src/rt`) | unit | all branches; the tables of §3.3.2, §3.3.3, §22.1 case by case | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-transpile --lib rt::` |
| Printer (`piperine-transpile/src/print`) | integration | compiled reactions on hand-built IR; then source-level runs | `crates/piperine-transpile/tests/reaction.rs`, `crates/piperine-parity/tests/source_unknowns.rs` | `cargo test -p piperine-transpile --test reaction` |
| ABI (`piperine-abi`) | integration | the new entry and the merge, valid and refused | `crates/piperine-abi/tests/{layouts,caller}.rs` | `cargo test -p piperine-abi --test <target>` |
| Digital solver (`piperine-digital-solver`) | integration | the flags at push and at apply; the journal undoing them | `crates/piperine-digital-solver/tests/{queue,instant}.rs` | `cargo test -p piperine-digital-solver --test <target>` |
| Host library (`piperine-api`) | integration and unit | decode; the verdict rules | `crates/piperine-api/tests/records.rs`, inline `verdict.rs` | `cargo test -p piperine-api --test records`, `--lib verdict` |
| Command line (`piperine-cli`) | integration | the record line with and without `maybe` | `crates/piperine-cli/tests/run.rs` | `cargo test -p piperine-cli --test run` |
| Source-level runs (end to end) | integration | one test per run-time AC, from MSL source on the Rust backend | `crates/piperine-parity/tests/source_unknowns.rs`, fixtures `crates/piperine-parity/tests/unknowns/*.msl` | `cargo test -p piperine-parity --test source_unknowns` |
| Spec text (`docs/spec/`) | integration | one assertion per decided fact (MD-57) | `crates/piperine-lang/tests/spec_text.rs` | `cargo test -p piperine-lang --test spec_text` |
| Repository records (`GAPS.md`, `.specs/STATE.md`, abi.md, the Book) | integration | one assertion per removed or added line | `crates/piperine-cli/tests/{docs,book}.rs` | `cargo test -p piperine-cli --test docs --test book` |
| Contract re-check (T1) | none | - (build gate; its findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes the runtime, the printer, the plan or the solver | Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins`, `cargo test -p piperine-api --test buck_long`, `cargo test -p piperine-transpile`, `cargo test -p piperine-digital-solver`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Contract

The tree after the rebase onto whatever has merged (`host-ii`, `root-e2e`, `analog-structure`).

```
T1
```

### Phase 2: The checker

`level`, `WIDTH`, the five intrinsics and the flag-total rule, typed before anything lowers them.

```
T2 → T3
T4
```

### Phase 3: Commits under unknown control in the ABI and the solver

The additive ABI entry and what the solver does with it.

```
T5 → T6 → T7 → T8
```

### Phase 4: The runtime

What generated code calls: panics, value methods, the pure merge, the reaction's paths.

```
T9 → T10 → T11 → T12
```

### Phase 5: IR and plan

The new nodes, the `maybe` field, the reads a merge needs.

```
T13 → T14 → T15
```

### Phase 6: The host reads `maybe`

```
T16 → T17
```

### Phase 7: The printer

The merge in generated code, owner by owner.

```
T18 → T19 → T20 → T21 → T22
```

### Phase 8: The command line

```
T23
```

### Phase 9: Elaboration of values (US-01 to US-03)

```
T24 → T25 → T26 → T27 → T28 → T29
```

### Phase 10: Elaboration of control and of digital values (US-04 AC 8, US-06)

```
T30
T31
T32
```

### Phase 11: Documents

```
T33 → T34 → T35
```

---

## Task Breakdown

### T1: Contract: the design against the tree after the rebase

**What**: Rebase `feature/unknown-semantics` onto the current `main` (whatever of `host-ii`, `root-e2e`, `analog-structure` has merged). Re-read every anchor of design.md "What the code does today" and "Cross-lane touches" at the new `HEAD`: `Flagged::decide` and its callers, `ValuePrinter::faults`, the refusals of `elab/lower.rs`, `elab/known.rs`, `elab/contribute.rs`, `ConstEval`'s `Unknown` row, `CARRIER_METHODS`, `Commits`, `StatementPlan::of_reaction`, `RecordLayout`, `Source`, `Queue::push`, `Digital::apply_due`, the commit sink that the owner hands a reaction, `Host`, `React`, `Record`, `Verdict`, the record line of `piperine-cli`, the next free `MD-` number, and whether an `Option`-yielding `if` duplicates arms that commit (design Risks). Write every difference, and how each later task maps onto it, on the Handoff's `**Settled mid-build:**` line. No code changes.
**Where**: `.specs/features/unknown-semantics/tasks.md` (Handoff only)
**Depends on**: None
**Reuses**: design.md
**Requirement**: US-01, US-02, US-03, US-04, US-05, US-06, US-07
**Executor**: Sonnet

**Done when**:

- [x] The branch is rebased onto `main` with no conflict left; `git log --oneline main..HEAD` shows only this feature's commits
- [x] Every anchor is confirmed or its difference is written on the Handoff with the task it changes
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): unknown-semantics - contract re-checked after the rebase`

---

### T2: `x.level(k)` and `T::WIDTH` are typed

**What**: `Builtin::Level` (`x.level(k)`, receiver any type with `Repr`, `k: u32`, result `Option<bool>`) leaves `CARRIER_METHODS`'s refused names; the member `Repr::WIDTH` read through a concrete type (`u8::WIDTH`, `bool::WIDTH`, `Mode::WIDTH`, `f64::WIDTH`) is typed `u32` and is a constant expression. Create the target `types_unknowns.rs` with its `//!` header.
**Where**: `crates/piperine-lang/src/types/call.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Builtin`, `stdlib_builtin`, `Ty::has_repr`, `pom::builtin_member("Repr", "WIDTH")`
**Requirement**: US-02
**Executor**: Sonnet

**Done when**:

- [x] `level_is_typed_on_every_repr_type` (bool, `u8`, `i4`, `f64`, a unit enum; a receiver without `Repr` refused) and `width_is_typed_u32_on_a_concrete_type` pass under `**Run typesunknowns**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): level and WIDTH are typed on Repr values`

---

### T3: The intrinsics of §22.1 are typed at the concrete type

**What**: A new module `types/intrinsic.rs` with `Intrinsic::{Merge, Same, Cast, Logic, MatchesBits}`: a call whose callee is `std::values`'s `merge`, `same`, `cast`, `logic` or `matches_bits` is typed by its §22.1 contract at the concrete type (from the arguments, the turbofish or the expected type) instead of reaching `signature_parts` ("generic function", "`extern` function"). `cast` with `A::WIDTH != B::WIDTH` is `E0701` naming both types (§22.1.3 rule 3); a `logic`/`matches_bits` pattern that is not a string literal of `0`, `1`, `x` of length `T::WIDTH` is `E0702` at the pattern (§22.1.4 rule 3). A project's own function of the same name is not intercepted (§22 rule 3). The callee is recorded as `Callee::Intrinsic`.
**Where**: `crates/piperine-lang/src/types/intrinsic.rs`
**Depends on**: T2
**Reuses**: `Callee`, `check_as`, `coded`
**Requirement**: US-03
**Executor**: Sonnet

**Done when**:

- [x] `the_intrinsics_are_typed_at_the_concrete_type`, `cast_between_widths_is_e0701` and `a_bad_pattern_is_e0702` pass under `**Run typesunknowns**`
- [x] `types_values.rs`'s "generic function" test still passes for a project function
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): merge, same, cast, logic and matches_bits are typed by their contract`

---

### T4: Flag-total functions and the `E0150` exception

**What**: A new module `types/flag_total.rs` with `FlagTotal`: whether parameter `i` of a function is flag-total (§3.3.6: every use of it is the receiver of `level`, `known`, `known_or`, `is_known`, `high`, or the argument of a flag-total parameter of another function, followed with a cycle guard). `Checker::carrier_reads_in` (`types/result.rs`) accepts a carrier read that is the argument of a flag-total parameter; every other bare read stays `E0150` naming the carrier.
**Where**: `crates/piperine-lang/src/types/flag_total.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `carrier_reads_in`, `stdlib_builtin`
**Requirement**: US-06
**Executor**: Sonnet

**Done when**:

- [x] `a_flag_total_function_may_take_a_carrier_in_analog` and `a_function_that_reads_its_parameter_bare_is_not_flag_total` (`E0150` naming the carrier) pass under `**Run typesunknowns**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(types): a flag-total function may read a carrier in analog`

---

### T5: `layout::Merge` merges two values of a layout

**What**: `piperine_abi::layout::Merge`: `Merge::of(layout)`, `Merge::into(&self, settled: &mut [u8], committed: &[u8]) -> Result<(), MergeError>`: per field with `HAS_KNOWN`, per element, a value whose known byte is set in both and whose value bytes are equal is kept; every other is zeroed, value and known byte (P3, §3.3.3). A field without `HAS_KNOWN` is refused naming it.
**Where**: `crates/piperine-abi/src/layout.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Layout`, `Field`, `field_flag::HAS_KNOWN`, `BIT`, `BYTE`, `REAL`
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [x] `merge_keeps_equal_known_fields_and_clears_the_rest` (`BIT`, `BYTE`, `REAL`, a two-field record layout) and `merge_refuses_a_field_without_a_known_byte` (`F64`) pass under `**Run abilayouts**`
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): merge two values of a layout level by level`

---

### T6: `Host::commit_with` and the commit flags

**What**: `commit_flag::{MAYBE = 1, MERGE = 2}`; `Host` gains `commit_with: CommitWithFn` appended after `commit` (`Host.size` grows), `MINOR` becomes 1; `React::commit_with(statement, bytes, after, flags) -> Result<(), Fault>`, a `Host` whose `size` has no room for the entry giving a fault naming the statement; `caller.rs` builds the entry and hands the flags to the owner's commit sink (`Commit` gains `flags`, 0 for `commit`). The raw layout table test of `raw.rs` covers the new field.
**Where**: `crates/piperine-abi/src/raw.rs`
**Depends on**: T5
**Reuses**: `CommitFn`, `React::commit`, `caller.rs`'s `catch_unwind` around the sink
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [x] `a_commit_with_flags_reaches_the_owner` and `a_host_without_commit_with_is_a_named_fault` pass under `**Run abicaller**`
- [x] Every existing `piperine-abi` target passes (`cargo test -p piperine-abi`)
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(abi): commits carry MAYBE and MERGE through an appended host entry`

---

### T7: The queue converts instead of removing under `MAYBE`

**What**: `Pending` gains `flags`; `Queue::push` of a `MAYBE` commit sets `MERGE` on the pending commits of the same statement it would remove (C3, §12.1.4) instead of removing them; the journal records the conversion (`Undo::Converted`) so `rewind` restores it (HYG-14).
**Where**: `crates/piperine-digital-solver/src/queue.rs`
**Depends on**: T6
**Reuses**: `same_statement`, `overlaps`, `Journal`
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [x] `a_maybe_commit_converts_the_pending_it_would_remove` and `a_rewind_undoes_a_conversion` pass under `**Run queue**`
- [x] `cargo test -p piperine-digital-solver` passes
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(digital-solver): a maybe commit converts the pending commits it would remove`

---

### T8: A `MERGE` commit merges with the value then

**What**: `Digital::apply_due` applies a `MERGE` pending commit as `layout::Merge` of its bytes into the settled bytes at that point of the C2 order (the net's layout from `DigitalPlan::net_layout`); a refused merge is a run error naming the pin. The commit sink that receives `commit_with` (located by T1) carries the flags into `Pending`.
**Where**: `crates/piperine-digital-solver/src/instant.rs`
**Depends on**: T7
**Reuses**: `Settled`, `write_net`, `layout::Merge`
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_merge_commit_merges_with_the_value_then` (a plain commit before it in C2 order is merged with, one after it wins) and `a_merge_commit_equal_to_the_value_fires_no_change` pass under `**Run instant**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(digital-solver): a merge commit merges with the value the net holds then`

---

### T9: `rt::Panic` and `rt::Attempt`

**What**: A new runtime file `rt/control.rs` (listed by `Runtime::files` and the `rt.rs` root) with `Panic::raise(message) -> !` (`std::panic::resume_unwind`, no hook output) and `Attempt::of(body) -> Option<T>` (`catch_unwind` with `AssertUnwindSafe`). Every `panic!` of `rt/int.rs` and `rt::Unwrap` raises through `Panic`, message unchanged. Grep the test suites for a check of the stderr text "panicked at" and report any on the Handoff.
**Where**: `crates/piperine-transpile/src/rt/control.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `rt/int.rs` messages, `thunk.rs::panic_message` (unchanged)
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [ ] `rt::control::tests::a_raised_panic_is_caught_by_an_attempt` passes under `**Run rt**`, and every existing `rt::int` panic test passes unchanged
- [ ] A compiled division by zero still faults with its message (`cargo test -p piperine-transpile --test reaction`)
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): runtime panics are raised without the hook and can be caught`

---

### T10: `Flagged` value methods of §3.3 and §22.1

**What**: In `rt/flag.rs`: `trait Merge { fn merge(self, other) -> Self; fn unknown() -> Self; }` for `Flagged<T: Carried>` and `()`; `Flagged::same`, `Flagged::level(k, origin) -> (Flagged<bool>, Flagged<bool>)` (a known `k` out of range raises the §1 rule 11 panic naming the origin and `k`; an unknown `k` merges over every level, §3.3.13), `Flagged::recast::<U>(tags)` (§22.1.3: the bit pattern under the other type, `Unknown` for a tag outside `tags`), `Flagged::known_or_flagged(d)` (§22.1.1 rule 4). One flag per value; `known` stays private.
**Where**: `crates/piperine-transpile/src/rt/flag.rs`
**Depends on**: T9
**Reuses**: `zip`, `map`, `Carried`, `Panic`
**Requirement**: US-02, US-03, US-06
**Executor**: Sonnet

**Done when**:

- [ ] `rt::flag::tests::merge_follows_the_level_rule`, `rt::flag::tests::same_compares_flags`, `rt::flag::tests::level_reads_a_level_and_merges_an_unknown_index`, `rt::flag::tests::recast_keeps_levels_and_decodes_tags` and `rt::flag::tests::known_or_flagged_gives_the_default_with_its_flag` pass under `**Run rt**`
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(transpile): merge, same, level and recast on flagged values`

---

### T11: `Decision` and `choose`, the merge of pure code

**What**: In `rt/control.rs`: `Decision::of(c)` (`Known(bool)` or `Unknown`) and `Flagged<bool>::choose(then, otherwise)`: a known condition runs one arm; an unknown one runs both under `Attempt` and returns their `Merge::merge`, or `Merge::unknown()` when an arm panicked (§3.3.3, §1 rule 13). `decide` stays until T19.
**Where**: `crates/piperine-transpile/src/rt/control.rs`
**Depends on**: T10
**Reuses**: `Attempt`, `Merge`
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [ ] `rt::control::tests::choose_runs_one_arm_or_merges_both` and `rt::control::tests::a_panicking_arm_makes_the_choice_unknown` pass under `**Run rt**`
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(transpile): choose merges the arms of an unknown condition`

---

### T12: `Commits` with paths, `Due`, `maybe` and `finish`

**What**: Move `Commits` to a new runtime file `rt/commits.rs` (root re-export kept) and give it: `controlled(carriers, resolution)`; `branch(c, then, otherwise)` (paths marked, `maybe` depth raised, arms under `Attempt`, joined by design.md "Joining two paths": per key `(carrier, Due)` the last commit, both paths `merge(a, b)` with `MAYBE` through each path's statement, one path `MAYBE | MERGE`, a panicked path `Unknown` for every key and unknown record fields with `maybe` kept); `run(definite, body)`; `maybe() -> Flagged<bool>`; `record(statement, bytes, maybe_at)`; `finish(value)` and `flow()` (a `finish` under `maybe` never ends the run, §19 Resolution); `flush` sends flagged commits through `React::commit_with`. `Due::of(after, resolution)` follows abi.md C1 as `Resolution::due` does. `Commits::new()`, `commit`, `arm` keep their behaviour and text.
**Where**: `crates/piperine-transpile/src/rt/commits.rs`
**Depends on**: T11
**Reuses**: `Flagged::encode`, `Merge`, `Attempt`, `React::commit_with`
**Requirement**: US-04, US-05
**Executor**: Sonnet

**Done when**:

- [ ] `rt::commits::tests::a_join_merges_keys_by_carrier_and_due`, `rt::commits::tests::a_panicked_path_makes_every_effect_unknown`, `rt::commits::tests::a_maybe_finish_never_ends_the_run` and `rt::commits::tests::due_rounds_as_the_solver_does` pass under `**Run rt**`
- [ ] The moved `Commits` tests pass unchanged
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(transpile): the commits of a reaction join across unknown paths`

---

### T13: The IR nodes of `level`, `merge`, `same` and `cast`

**What**: Appended last to `Expr`: `Level { value, index, part: LevelPart }`, `Merge { a, b }`, `Same { a, b }`, `LevelCast { expr, to, tags }`; typing (`Merge` flagged, `Same` plain `bool`, `Level` flagged when `index` is, `LevelCast` flagged when its operand is or `tags` is given), `check`, `deps`, `print`, `statements`; `Math` and `KnownOr` accept a flagged operand and are flagged when it is. `Expr::If`'s doc states that a `Flagged` condition is the merge of §3.3.3. The interpreter refuses the four forms by name (`walk.rs`, R1).
**Where**: `crates/piperine-ir/src/model.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Typing::flag`, the exhaustive matches of `check.rs`, `deps.rs`, `print.rs`, `statements.rs`
**Requirement**: US-02, US-03, US-06
**Executor**: Sonnet

**Done when**:

- [ ] `the_unknown_nodes_type_print_and_check` passes under `**Run irdigital**`
- [ ] `the_unknown_nodes_are_refused_by_name` passes under `**Run interp**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(ir): level, merge, same and cast nodes; flagged math and known_or`

---

### T14: Every record ends with `maybe`

**What**: `RecordLayout::of` appends a field `maybe`, laid out as a `bool` carrier (`HAS_KNOWN`), after the stream's value fields; its offset is exposed (`RecordLayout::maybe_offset`). `Stream` in the IR is unchanged.
**Where**: `crates/piperine-plan/src/digital.rs`
**Depends on**: T13
**Reuses**: `CarrierLayout::of`, `abi_fields`
**Requirement**: US-05
**Executor**: Sonnet

**Done when**:

- [ ] `every_record_layout_ends_with_maybe` passes under `**Run planshape**`
- [ ] Every buck target passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): every record carries maybe`

---

### T15: The reads a merge needs

**What**: A reaction whose head has an event with a `Flagged` guard reads each guard's sources and `Fired(k)` of each event of its head; a reaction holding an `If` with a `Flagged` condition reads `Source::Resolution`, appended last, bound to the `GLOBAL` `RESOLUTION` pin. Reactions with neither keep their reads, pins and statements.
**Where**: `crates/piperine-plan/src/plan.rs`
**Depends on**: T14
**Reuses**: `ReadSet`, `Source::Fired`, `Source::Analysis` (a global pin read)
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_reaction_on_a_flagged_guard_reads_the_guard_and_fired` and `a_reaction_with_an_unknown_decision_reads_the_resolution` pass under `**Run planshape**`, and a body with neither has the shape it had (asserted in the same tests)
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(plan): a reaction reads what its merge needs`

---

### T16: `Record::maybe`

**What**: `Record` gains `maybe: bool`, decoded by `Streams::decode` from `RecordLayout::maybe_offset` and kept out of `fields`; struct literals in tests gain `maybe: false` (no assertion changes).
**Where**: `crates/piperine-api/src/record.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Streams::decode`, `RecordLayout`
**Requirement**: US-05
**Executor**: Sonnet

**Done when**:

- [ ] `maybe_is_decoded_beside_the_fields` passes under `**Run records**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): a record says whether it is a maybe`

---

### T17: The verdict reads `maybe`

**What**: `Verdict`: a `checks` record with `maybe = true` fails as an unknown check (US-05 AC 4, §23.2); a `sim` record with `failed` known true and `maybe = true` is not the fatal one (§19 Resolution, G.55).
**Where**: `crates/piperine-api/src/verdict.rs`
**Depends on**: T16
**Reuses**: `Verdict::record`
**Requirement**: US-05
**Executor**: Sonnet

**Done when**:

- [ ] `verdict::tests::a_maybe_check_fails_as_unknown` and `verdict::tests::a_maybe_fatal_is_not_fatal` pass under `**Run verdict**`
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): a maybe check fails and a maybe fatal ends nothing`

---

### T18: Pure code merges with `choose`

**What**: `ValuePrinter::if_else` prints an `if` on a `Flagged` condition as `c.choose(|| { A }, || { B })` in every owner without commits (functions, drives, event functions, arming, `cross` arguments, body `let`s); functions stop returning `Result<T, Fault>` (`faults()` and the `?` after calls go); reactions keep `decide` until T19. The dpwm snippet test of `value.rs` prints `.choose(..)`. Measure the buck's build time before and after, in the commit body.
**Where**: `crates/piperine-transpile/src/print/value.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `arm`, `block_lines`, `typed`
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_function_and_a_definition_merge_an_unknown_if` passes under `**Run sourceunknowns**` (new target with its `//!` header and fixtures in `tests/unknowns/`)
- [ ] Every buck target passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): functions and definitions merge an unknown if`

---

### T19: Reactions merge with `commits_.branch`

**What**: In a reaction, an `if` on a `Flagged` condition prints as `commits_.branch(c, |commits_| { A }, |commits_| { B })`; a reaction holding one builds `rt::Commits::controlled(&CARRIERS_, resolution_)` with its statement-to-carrier table; `finish` prints `commits_.finish(value)` and the reaction returns `commits_.flow()`. `decide`, `Flagged::UNKNOWN_CONTROL` and their unit test half are deleted. `an_unknown_condition_faults_and_commits_nothing` is replaced by `an_unknown_condition_merges_both_arms` (design.md table; spec US-04 AC 1).
**Where**: `crates/piperine-transpile/src/print/reaction.rs`
**Depends on**: T18
**Reuses**: `Commits`, `StatementPlan`, `ValuePrinter::committed`
**Requirement**: US-04, US-05
**Executor**: Sonnet

**Done when**:

- [ ] `an_unknown_condition_merges_both_arms` passes under `**Run reaction**`
- [ ] Under `**Run sourceunknowns**`: `an_unknown_if_runs_both_arms_without_a_fault`, `both_arms_committing_commit_their_merge`, `one_arm_committing_merges_with_the_value_then`, `equal_known_commits_stay_known`, `an_if_expression_and_an_if_statement_commit_the_same`, `an_unknown_match_merges_every_arm`, `a_panic_under_unknown_control_makes_the_merge_unknown`, `definitions_lets_and_functions_merge_alike`, `an_unknown_delay_under_unknown_control_commits_unknown_next`, `a_maybe_finish_does_not_end_the_run`, `neither_arm_committing_commits_nothing`, `delays_one_tick_apart_are_one_key`, `the_pfd3_reaction_keeps_state_unknown_until_a_known_commit` pass
- [ ] `grep -rn "decide(" crates/piperine-transpile/src` finds nothing
- [ ] Every buck target passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): an unknown if in a reaction merges its arms`

---

### T20: The new nodes in generated code

**What**: `ValuePrinter` prints `Level` (`x.level(k, "<origin>")`, the pair's part), `Merge`, `Same`, `LevelCast` (`recast` with its tags), and `Math`/`KnownOr` over a flagged operand (`x.map(f64::exp)`, `x.known_or_flagged(d)`), in digital and in analog printers.
**Where**: `crates/piperine-transpile/src/print/value.rs`
**Depends on**: T19
**Reuses**: `Flagged` methods of T10
**Requirement**: US-02, US-03, US-06
**Executor**: Sonnet

**Done when**:

- [ ] `level_merge_same_and_recast_compute_their_tables` (a hand-built reaction committing each node's result) passes under `**Run reaction**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): level, merge, same and cast in generated code`

---

### T21: Records write `maybe`

**What**: The record printer writes `commits_.maybe()` at `RecordLayout::maybe_offset` and calls `commits_.record(statement, &record_, maybe_offset)`; functions inlined into a reaction write against the same `commits_`.
**Where**: `crates/piperine-transpile/src/print/value.rs`
**Depends on**: T20
**Reuses**: the record printer (`EmitField` loop)
**Requirement**: US-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_record_under_unknown_control_is_maybe`, `a_record_field_keeps_its_flag` and `a_maybe_check_is_judged_as_unknown` (a `require` under an unknown `if`, judged by `Verdict::judge`) pass under `**Run sourceunknowns**`
- [ ] `buck_records` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): a record reached under unknown control says maybe`

---

### T22: An unknown guard runs the reaction as a maybe

**What**: The event function fires its pin when the source fired and the guard is not known false (`print/digital.rs`); a reaction on an event with a `Flagged` guard computes `definite_` (an event of its head fired with a known or absent guard) and runs its body as `commits_.run(definite_, |commits_| { .. })` (§13 rule 1).
**Where**: `crates/piperine-transpile/src/print/digital.rs`
**Depends on**: T21
**Reuses**: `ValuePrinter::condition` (its `decide` branch removed), `Commits::run`
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [ ] `an_unknown_guard_runs_the_reaction_as_a_maybe` and `a_known_false_guard_does_not_run` pass under `**Run sourceunknowns**`
- [ ] Every buck target passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(transpile): an unknown guard runs its reaction under the merge`

---

### T23: The command line prints `maybe` when true

**What**: The record line ends with `maybe = true` (and a JSON Lines record with `"maybe": true`) when `Record::maybe` holds; nothing is printed for `false`, so no existing listing or golden moves.
**Where**: `crates/piperine-cli/src/render.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: the record line layout
**Requirement**: US-05
**Executor**: Sonnet

**Done when**:

- [ ] `a_maybe_record_prints_maybe_and_a_known_one_does_not` passes under `**Run clirun**`
- [ ] `cargo test -p piperine-cli` passes
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(cli): a maybe record says so`

---

### T24: `ConstEval` folds `Unknown` by the level rules

**What**: `ConstEval` evaluates to an internal `Folded::{Known(Constant), Unknown}`; `Unknown` folds, operators follow §3.3.2 as `rt::Flagged` does (all-or-nothing; a known all-false `&` operand and a known all-true `|` operand decide; a shift by `N` or more of an unknown value is known 0), `known_or`, `is_known`; `value()` returns `ConstFault::Unknown { expr }` when the result is unknown. `T::WIDTH` folds to the level count (§2 table, rule 5). Create the target `elab_unknowns.rs`.
**Where**: `crates/piperine-lang/src/elab/consts.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ConstEval`, `EnumLayout`
**Requirement**: US-01, US-02
**Executor**: Sonnet

**Done when**:

- [ ] `unknown_folds_by_the_level_rules` and `width_is_the_level_count_of_each_repr_type` pass under `**Run elabunknowns**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): constants fold Unknown by the level rules`

---

### T25: `Unknown` deciding structure is `E0151`

**What**: A `ConstFault::Unknown` at a decision that fixes structure (a `where` condition today; the rule sits where `ConstEval` answers a structural question, so generation, const generics and array lengths inherit it) is `E0151` naming the decision (§3.3.8).
**Where**: `crates/piperine-lang/src/elab/body.rs`
**Depends on**: T24
**Reuses**: `coded`, `const_fault`
**Requirement**: US-01
**Executor**: Sonnet

**Done when**:

- [ ] `unknown_deciding_structure_is_e0151` passes under `**Run elabunknowns**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(elab): Unknown that decides structure is E0151`

---

### T26: `Unknown` as an initial value and in a const

**What**: A carrier declared `= Unknown` (or with an initial value that folds to `Unknown`) gets `Carrier::init = None`; a scalar const whose fold is `Unknown` reads as `Expr::Unknown`; an `Option` const reads its initializer inline (`lower_option`), so every read has the same levels (§3.3.9). The tests that pinned the refusal "this literal in a carrier's initial value" become these acceptance tests.
**Where**: `crates/piperine-lang/src/elab/carriers.rs`
**Depends on**: T25
**Reuses**: `lower_path`, `lower_option`
**Requirement**: US-01
**Executor**: Sonnet

**Done when**:

- [ ] `an_unknown_initial_value_starts_every_level_unknown`, `an_unknown_initial_value_fires_no_change_at_zero`, `a_const_holding_unknown_reads_unknown_everywhere` and `committing_unknown_makes_every_level_unknown` pass under `**Run sourceunknowns**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): Unknown as an initial value and in a const`

---

### T27: `x.level(k)` lowers to its pair

**What**: `level` lowers through `lower_option` to the pair `Level { part: Is }`, `Level { part: Value }` over the lowered receiver and index, flagged when the index may be unknown.
**Where**: `crates/piperine-lang/src/elab/compound.rs`
**Depends on**: T26
**Reuses**: `Pair`, `lower_option`, `lower_flag_read`
**Requirement**: US-02
**Executor**: Sonnet

**Done when**:

- [ ] `level_reads_each_level_of_a_known_value`, `level_with_a_known_index_is_known`, `level_out_of_range_is_a_panic_naming_k` and `level_with_an_unknown_index_merges_every_level` pass under `**Run sourceunknowns**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): level lowers to the pair of its option`

---

### T28: `merge`, `same` and `cast` lower by their contract

**What**: A new module `elab/intrinsics.rs` lowers `Callee::Intrinsic` calls: `merge` to `Expr::Merge` (flagged), `same` to `Expr::Same`, `cast` to `Expr::LevelCast` with the target enum's tags (`EnumLayout`) or none. The module's `//!` says it is deleted when `generics` compiles `std/values.msl` (spec Q1 (b)).
**Where**: `crates/piperine-lang/src/elab/intrinsics.rs`
**Depends on**: T27
**Reuses**: `lower_call`, `EnumLayout`
**Requirement**: US-03
**Executor**: Sonnet

**Done when**:

- [ ] `merge_of_equal_known_values_is_that_value`, `merge_of_different_values_is_unknown_and_commutative`, `merge_of_a_value_with_itself_is_that_value`, `same_compares_flags_and_values`, `cast_keeps_each_level_and_its_flag` and `cast_to_a_tag_with_no_variant_is_unknown` pass under `**Run sourceunknowns**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): merge, same and cast lower by their contract`

---

### T29: `logic` and `matches_bits`

**What**: `Elaboration::lower_levels(levels, ty)` (in `elab/intrinsics.rs`) lowers a parsed pattern: all `0`/`1` a literal, any `x` `Expr::Unknown` (one flag per value; partial levels are `unknown-semantics-per-level` UL-03); `ConstEval` folds `logic` the same way, so `Bit<u4> = logic("1010")` is an initial value. `matches_bits(s, p)` lowers to the conjunction over the levels `p` cares about of `level(k) == Some(p_k)`, `true` for an all-`x` pattern (§22.1.4 rule 2).
**Where**: `crates/piperine-lang/src/elab/intrinsics.rs`
**Depends on**: T28
**Reuses**: T27's `level` lowering, `ConstEval`
**Requirement**: US-03
**Executor**: Sonnet

**Done when**:

- [ ] `logic_builds_known_and_unknown_values` and `matches_bits_follows_its_table` pass under `**Run sourceunknowns**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): logic and matches_bits by their contract`

---

### T30: `&&` and `||` with a right operand that can panic or emit

**What**: An `&&`/`||` with an operand that may be unknown and a right operand that `partial` reports lowers to `If { cond: lhs, then: rhs, otherwise: false }` (`&&`) or `If { cond: lhs, then: true, otherwise: rhs }` (`||`); a pure right operand stays `&`/`|`. The refusal and its tests become the acceptance test.
**Where**: `crates/piperine-lang/src/elab/lower.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Elaboration::partial` (now a selector)
**Requirement**: US-04
**Executor**: Sonnet

**Done when**:

- [ ] `short_circuit_operands_follow_the_merge` (`unknown && false`, `unknown || true`, the unknown cases, a panicking and an emitting right operand) passes under `**Run sourceunknowns**`
- [ ] `elab_values.rs`'s DIG-08 AC 6 test (`&`/`|` for a pure right operand) passes unchanged
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): a short-circuit operand that can panic runs under the merge`

---

### T31: Math functions and `known_or` over values that may be unknown

**What**: In digital code, a math function (`exp`, `ln`, `sqrt`, `abs`) of a value that may be unknown lowers to a flagged `Expr::Math`; `known_or(d)` with a `d` that may be unknown lowers to a flagged `Expr::KnownOr` (§22.1.1 rule 4). The two `P0001`s and their tests become the acceptance tests.
**Where**: `crates/piperine-lang/src/elab/known.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Flag::wrap`
**Requirement**: US-06
**Executor**: Sonnet

**Done when**:

- [ ] `a_math_function_of_an_unknown_value_is_unknown` (`(x.sqrt()).known_or(-1.0)` with `x` unknown, then known) and `known_or_with_an_unknown_default_gives_that_default` pass under `**Run sourceunknowns**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(elab): math and known_or compute with values that may be unknown`

---

### T32: `analog` refuses a value that may be unknown with `E0150`

**What**: The elaborator's backstops in `analog` (a contribution, a `known_or` default, a math function, the argument of `ddt`, `delay`, `limit`, `noise`) report `E0150` naming the value's source, never `P0001`; the checker's `E0150` names the carrier first.
**Where**: `crates/piperine-lang/src/elab/contribute.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `coded`, the checker's `analog_reads`
**Requirement**: US-06
**Executor**: Sonnet

**Done when**:

- [ ] `analog_reading_a_maybe_unknown_value_is_e0150` (one source per form, each naming the carrier) and `no_analog_refusal_of_an_unknown_is_p0001` pass under `**Run elabunknowns**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(elab): analog refuses a value that may be unknown with E0150`

---

### T33: Appendix C.2 states the loop bound

**What**: Appendix C.2 gains a row "unknown control (§3.3.3)": the merge in generated code, the loop bound of 64 iterations applied in every analysis; C.2.1 gains the default "loop iterations under unknown control: 64 (§3.3.16)". Two `spec_text.rs` functions appended (MD-57).
**Where**: `docs/spec/appendix_c_implementation.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `spec_text.rs` helpers (`section`, `table_rows`)
**Requirement**: US-07, US-04
**Executor**: Sonnet

**Done when**:

- [ ] `appendix_c_states_the_loop_bound_under_unknown_control` and `appendix_c_lists_the_loop_bound_among_the_defaults` pass under `**Run spectext**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): Appendix C states the loop bound under unknown control`

---

### T34: abi.md and the decision record

**What**: `.specs/features/device-abi/abi.md`: `Host` with `commit_with`, the constants table (`commit_flag` `MAYBE` 1, `MERGE` 2; `MINOR` 1) and a rule C7 "Commits under unknown control" (merge at the due microstep in C2 order; transport converts under `MAYBE`). `.specs/STATE.md`: the next free `MD-` entry (number from T1) recording the merge in generated code, the additive ABI entry, records' `maybe`, and Q1 (b)'s deletion point; it extends MD-62 and MD-64 and supersedes nothing.
**Where**: `.specs/STATE.md`
**Depends on**: T33
**Reuses**: MD-67's layout
**Requirement**: US-07
**Executor**: Sonnet

**Done when**:

- [ ] `state_records_the_unknown_semantics_decision` and `abi_states_commits_under_unknown_control` pass under `**Run docs**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(specs): the unknown-semantics decision and the ABI's C7`

---

### T35: `GAPS.md` and the Book say what runs

**What**: `GAPS.md` drops L18 and L19 (S6 and G6 stay unless `unknown-semantics-per-level` merged); rows L5 and L3/L24 say that their merge criteria under unknown control (this spec's US-04 AC 11 and AC 12) travel with them; §9's coverage line moves the merge and `level` out of the gaps. The Book's chapter on unknown values runs its examples (`maybe` shown as printed) and the debounce listing is re-captured from the binary.
**Where**: `GAPS.md`
**Depends on**: T34
**Reuses**: `docs.rs` helpers, `book.rs`
**Requirement**: US-07, US-04
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_l18_and_l19` and `gaps_carries_the_unknown_index_merge_to_composite_values` pass under `**Run docs**`
- [ ] `every_cli_listing_of_the_book_is_what_the_binary_prints` passes under `**Run book**`
- [ ] Gate check passes: Build

**Tests**: integration
**Gate**: build

**Commit**: `docs: GAPS and the Book say what runs of unknown values`

---

## Checks

**C1** - A carrier declared `= Unknown` starts with every level unknown at `(0, 0)`. (US-01, AC 1; T26)
Proof: run sourceunknowns | test an_unknown_initial_value_starts_every_level_unknown ... ok

**C2** - A carrier declared `= Unknown` fires no `changed()` at `(0, 0)`. (US-01, AC 2; T26)
Proof: run sourceunknowns | test an_unknown_initial_value_fires_no_change_at_zero ... ok

**C3** - A const holding `Unknown` (`Some(Unknown)` included) gives every read the same unknown levels. (US-01, AC 3; T26)
Proof: run sourceunknowns | test a_const_holding_unknown_reads_unknown_everywhere ... ok

**C4** - `Unknown` deciding a `where` condition is `E0151` naming the decision. (US-01, AC 4; T25)
Proof: run elabunknowns | test unknown_deciding_structure_is_e0151 ... ok

**C5** - `q <- Unknown;` makes every level of `q` unknown at the microstep the commit applies. (US-01, AC 5; T26)
Proof: run sourceunknowns | test committing_unknown_makes_every_level_unknown ... ok

**C6** - `x.level(k)` of a known value gives `Some(b)` for each level `k` of a `u8` and a `bool`. (US-02, AC 1; T27)
Proof: run sourceunknowns | test level_reads_each_level_of_a_known_value ... ok

**C7** - With a known `k`, `level` is known whatever the flags of `x` (`None` for an unknown `x`). (US-02, AC 2; T27)
Proof: run sourceunknowns | test level_with_a_known_index_is_known ... ok

**C8** - `T::WIDTH` is 1 for `bool`, `N` for `uN` and `iN`, 64 for `f64`, the tag width of a unit enum, as a `u32`. (US-02, AC 3; T24)
Proof: run elabunknowns | test width_is_the_level_count_of_each_repr_type ... ok

**C9** - `x.level(8)` on a `u8` is a run fault naming the expression and `k = 8`. (US-02, AC 4; T27)
Proof: run sourceunknowns | test level_out_of_range_is_a_panic_naming_k ... ok

**C10** - An unknown `k` gives the merge of `x.level(j)` over every `j`. (US-02, AC 5; T27)
Proof: run sourceunknowns | test level_with_an_unknown_index_merges_every_level ... ok

**C11** - `merge` of two known equal values is that value, known. (US-03, AC 1; T28)
Proof: run sourceunknowns | test merge_of_equal_known_values_is_that_value ... ok

**C12** - `merge` of two differing `bool`s, enum values or integers is unknown, and `merge(a, b)` equals `merge(b, a)` by `same`. (US-03, AC 2; T28)
Proof: run sourceunknowns | test merge_of_different_values_is_unknown_and_commutative ... ok

**C13** - `same` is known `true` exactly when flags and known values agree; `same(Unknown, Unknown)` is `true` and `Unknown == Unknown` is unknown. (US-03, AC 3; T28)
Proof: run sourceunknowns | test same_compares_flags_and_values ... ok

**C14** - `cast::<A, B>` keeps each level with its flag between types of equal width. (US-03, AC 4; T28)
Proof: run sourceunknowns | test cast_keeps_each_level_and_its_flag ... ok

**C15** - `cast` to an enum whose tag names no variant gives `Unknown`. (US-03, AC 5; T28)
Proof: run sourceunknowns | test cast_to_a_tag_with_no_variant_is_unknown ... ok

**C16** - `logic` of `0`/`1` is the known value with the leftmost character as level `WIDTH - 1`; all `x` is every level unknown. (US-03, AC 6; T29)
Proof: run sourceunknowns | test logic_builds_known_and_unknown_values ... ok

**C17** - `matches_bits` is known `true` exactly when each level matches its character (`x` any, `0`/`1` a known level of that value). (US-03, AC 7; T29)
Proof: run sourceunknowns | test matches_bits_follows_its_table ... ok

**C18** - `cast` between types of different widths is `E0701` naming both types. (US-03, AC 8; T3)
Proof: run typesunknowns | test cast_between_widths_is_e0701 ... ok

**C19** - A pattern with a character other than `0`, `1`, `x`, or of the wrong length, is `E0702` at the pattern. (US-03, AC 9; T3)
Proof: run typesunknowns | test a_bad_pattern_is_e0702 ... ok

**C20** - An `if` in a reaction on an unknown condition runs both arms and the run does not fault. (US-04, AC 1; T19)
Proof: run sourceunknowns | test an_unknown_if_runs_both_arms_without_a_fault ... ok

**C21** - Both arms committing to `q` commit their level-wise merge at the microstep they are due. (US-04, AC 2; T19)
Proof: run sourceunknowns | test both_arms_committing_commit_their_merge ... ok

**C22** - One arm committing to `q` commits the merge of its value with the value `q` holds then, another reaction's commit in the same microstep included. (US-04, AC 3; T19)
Proof: run sourceunknowns | test one_arm_committing_merges_with_the_value_then ... ok

**C23** - Both arms committing the same known value commit it known. (US-04, AC 4; T19)
Proof: run sourceunknowns | test equal_known_commits_stay_known ... ok

**C24** - `q <- if c { a } else { b };` and `if c { q <- a; } else { q <- b; }` make the same commit. (US-04, AC 5; T19)
Proof: run sourceunknowns | test an_if_expression_and_an_if_statement_commit_the_same ... ok

**C25** - A `match` on an unknown scrutinee merges every arm, a wildcard arm included. (US-04, AC 6; T19)
Proof: run sourceunknowns | test an_unknown_match_merges_every_arm ... ok

**C26** - A panic on a path under unknown control does not fault and makes every value and commit of that merge unknown. (US-04, AC 7; T19)
Proof: run sourceunknowns | test a_panic_under_unknown_control_makes_the_merge_unknown ... ok

**C27** - `unknown && false == false`, `unknown || true == true`, unknown otherwise, also with a right operand that panics or emits. (US-04, AC 8; T30)
Proof: run sourceunknowns | test short_circuit_operands_follow_the_merge ... ok

**C28** - A guard `ev && c` with `c` unknown runs the reaction and merges each effect with not running it. (US-04, AC 9; T22)
Proof: run sourceunknowns | test an_unknown_guard_runs_the_reaction_as_a_maybe ... ok

**C29** - A definition, a body `let`, an `if` expression and a function body merge an unknown decision as a reaction does. (US-04, AC 10; T19)
Proof: run sourceunknowns | test definitions_lets_and_functions_merge_alike ... ok

**C30** - The loop bound under unknown control, 64, is written in Appendix C.2. (US-04, AC 11; T33)
Proof: run spectext | test appendix_c_states_the_loop_bound_under_unknown_control ... ok

**C31** - The merge of an unknown index travels with the `composite-values` rows L3/L24 in `GAPS.md`. (US-04, AC 12; T35)
Proof: run docs | test gaps_carries_the_unknown_index_merge_to_composite_values ... ok

**C32** - A commit whose delay is unknown, under unknown control, commits `Unknown` at the next microstep. (US-04, AC 13; T19)
Proof: run sourceunknowns | test an_unknown_delay_under_unknown_control_commits_unknown_next ... ok

**C33** - An `emit!` under unknown control is delivered with `maybe = true`. (US-05, AC 1; T21)
Proof: run sourceunknowns | test a_record_under_unknown_control_is_maybe ... ok

**C34** - A record under known control has `maybe = false` and its line prints no `maybe`; a maybe record's line ends with `maybe = true`. (US-05, AC 2; T23)
Proof: run clirun | test a_maybe_record_prints_maybe_and_a_known_one_does_not ... ok

**C35** - A `sim` record with `finish = true` and `maybe = true` does not end the run. (US-05, AC 3; T19)
Proof: run sourceunknowns | test a_maybe_finish_does_not_end_the_run ... ok

**C36** - A `require` under unknown control writes a `checks` record with `maybe = true` that `piperine test` judges as an unknown check. (US-05, AC 4; T21)
Proof: run sourceunknowns | test a_maybe_check_is_judged_as_unknown ... ok

**C37** - A record field that is unknown is delivered unknown. (US-05, AC 5; T21)
Proof: run sourceunknowns | test a_record_field_keeps_its_flag ... ok

**C38** - A math function of a value that may be unknown elaborates in a reaction and gives unknown when its argument is. (US-06, AC 1; T31)
Proof: run sourceunknowns | test a_math_function_of_an_unknown_value_is_unknown ... ok

**C39** - `x.known_or(d)` with `d` that may be unknown gives `x` when known, else `d` with its flags. (US-06, AC 2; T31)
Proof: run sourceunknowns | test known_or_with_an_unknown_default_gives_that_default ... ok

**C40** - `analog` reading a value that may be unknown in a contribution, a `known_or` default, a math function or an operator argument is `E0150` naming the carrier. (US-06, AC 3; T32)
Proof: run elabunknowns | test analog_reading_a_maybe_unknown_value_is_e0150 ... ok

**C41** - A function whose every use of a parameter passes through `level` may take a carrier read in `analog`. (US-06, AC 4; T4)
Proof: run typesunknowns | test a_flag_total_function_may_take_a_carrier_in_analog ... ok

**C42** - `GAPS.md` lists neither L18 nor L19, and keeps S6 and G6. (US-07, AC 1; T35)
Proof: run docs | test gaps_drops_l18_and_l19 ... ok

**C43** - Appendix C.2.1 lists the loop bound among the defaults, guarded by `spec_text`. (US-07, AC 2; T33)
Proof: run spectext | test appendix_c_lists_the_loop_bound_among_the_defaults ... ok

**C44** - Every command-line listing of the Book is what the binary prints. (US-07, AC 3; T35)
Proof: run book | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C45** - Neither arm committing to `q` commits nothing to `q`. (Edge case 1; T19)
Proof: run sourceunknowns | test neither_arm_committing_commits_nothing ... ok

**C46** - A merged commit equal to the net's value, levels and flags, fires no `changed()`. (Edge case 6; T8)
Proof: run instant | test a_merge_commit_equal_to_the_value_fires_no_change ... ok

**C47** - `merge(a, a)` has the levels and flags of `a`. (Edge case 4; T28)
Proof: run sourceunknowns | test merge_of_a_value_with_itself_is_that_value ... ok

**C48** - The §3.3 phase detector's reaction keeps `state` unknown until a known commit (the `if`-expression form §3.3 calls the same). (Success criterion 2; T19)
Proof: run sourceunknowns | test the_pfd3_reaction_keeps_state_unknown_until_a_known_commit ... ok

**C49** - A `MAYBE` commit turns the pending commits of its statement it would remove into `MERGE` commits. (design C7; T7)
Proof: run queue | test a_maybe_commit_converts_the_pending_it_would_remove ... ok

**C50** - Two delays that round to one tick are one key of a join. (design C5; T19)
Proof: run sourceunknowns | test delays_one_tick_apart_are_one_key ... ok

**C51** - An unknown condition runs one arm when known and merges both when unknown, in pure code. (design C3; T11)
Proof: run rt | test rt::control::tests::choose_runs_one_arm_or_merges_both ... ok

**C52** - A join keys commits by carrier and due instant: both paths a plain merge, one path a `MERGE` commit. (design C4; T12)
Proof: run rt | test rt::commits::tests::a_join_merges_keys_by_carrier_and_due ... ok

**C53** - A compiled reaction with an unknown condition commits the merge of its arms instead of faulting. (US-04, AC 1, compiled level; T19)
Proof: run reaction | test an_unknown_condition_merges_both_arms ... ok

**C54** - `layout::Merge` keeps equal known fields and zeroes the rest. (design C6; T5)
Proof: run abilayouts | test merge_keeps_equal_known_fields_and_clears_the_rest ... ok

**C55** - A `Host` without `commit_with` gives a fault naming the statement. (design C6; T6)
Proof: run abicaller | test a_host_without_commit_with_is_a_named_fault ... ok

**C56** - The IR types, prints and checks `Level`, `Merge`, `Same` and `LevelCast`. (design C9; T13)
Proof: run irdigital | test the_unknown_nodes_type_print_and_check ... ok

**C57** - The interpreter refuses the new forms by name (codegen context R1). (design C9; T13)
Proof: run interp | test the_unknown_nodes_are_refused_by_name ... ok

**C58** - Every record layout ends with a known `bool` field `maybe`. (US-05, AC 1, layout; T14)
Proof: run planshape | test every_record_layout_ends_with_maybe ... ok

**C59** - The host decodes `maybe` beside a record's fields, not among them. (US-05, AC 2, host side; T16)
Proof: run records | test maybe_is_decoded_beside_the_fields ... ok

**C60** - A `fatal` record with `maybe = true` is not the run's fatal. (US-05, AC 3, host side; T17)
Proof: run verdict | test verdict::tests::a_maybe_fatal_is_not_fatal ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** T1 (2026-11-18, B1): `origin/main` is `ec4d13b6`, an ancestor of this branch, so the rebase is a no-op (no merge of `host-ii`, `root-e2e`, `analog-structure` since design); `git log main..HEAD` holds only this feature's docs commits. Anchors confirmed at HEAD: `Flagged::decide` (`rt/flag.rs:260`), `ValuePrinter::faults` (`print/value.rs:343`), the refusals (`lower.rs:231` partial `&&`/`||`, `:304` math, `contribute.rs:223` `known_addend`, `known.rs:76` `partial`), `ConstEval` literal `Unknown` refusal (`consts.rs:172`), `Commits` (`rt/flag.rs:422`), `StatementPlan::of_reaction` (`piperine-plan/src/digital.rs:129`), `RecordLayout` (`digital.rs:250`), `Source` (`plan.rs:81`, no `Resolution` yet), `Queue::push` (`queue.rs:83`), `Digital::apply_due` (`instant.rs:44`), `Host` (`raw.rs:628`), `Record` (`piperine-api/src/record.rs:29`), `Verdict` (`verdict.rs:10`), the record line (`piperine-cli/src/render/record.rs`). Differences: (1) `level` is already in `CARRIER_METHODS` (`types/call.rs:14`) as a name that is not `E0161`; T2 types it (`Builtin::Level`) rather than removing a refusal there. (2) The commit sink the owner hands a reaction is `call_reaction`'s closure in `piperine-digital-solver/src/run.rs` (~l.520-602, building `Planned`) and `run_reactions` builds `Pending` at ~l.715, not `instant.rs`: T7 and T8 add `flags` to `Commit`, `Planned` and `Pending` there; `Writer::Host` pendings get flags 0. (3) `piperine-abi/tests/contract.rs:74` pins `assert_eq!(MINOR, 0)`; the spec mandates MINOR 1, so T6 changes that pinned value to 1 (the pin follows the spec, no tolerance or behaviour weakened); `tests/thunks.rs:202` and `caller.rs:242` build `Host` literals and gain the appended field. (4) The thunk's sink (`thunk.rs:362`) and `React::new` (`device.rs:261`, `rt/flag.rs:614`, `device.rs:343`) are the device-side path of `commit`; `React` gains a second sink for `commit_with`. (5) Next free MD number: MD-69 (MD-68 is the last). (6) An `Option`-yielding `if` is lowered as two `If`s over the same arms (`compound.rs:530`): confirmed, so an arm that commits or emits would run twice; T19 handles it as design Risks says. No other difference. (2) T2: `Repr::WIDTH` on an enum needed a POM change (`pom/resolve/imports.rs`: an enum member that is no variant or item resolves to the `Repr` member row); `types_subset.rs`'s 'known flag' row moved from `1.0.level(1.0)` (now a type error by spec) to `Some(1.0).level(1)` (still P0001 by name). `Ty::levels(design)` is the level count (§2 rule 5: tag `ceil(log2 V)`), reused by T24. T3: `Callee::Intrinsic(Intrinsic)`, `Checker.call_expected` hands the expected type to a call; `cast::<A,B>` needs a written or expected B else P0001. T4: flag reads are decided by `Checker::is_flag_read` (stdlib decl, or by name when the POM left the method unresolved). T5: `Merge::into` is named `merge_into` (an inherent `into(&self, ..)` loses to `Into::into`); later tasks call `merge_into`. T6: `commit_flag::{MAYBE,MERGE}`, `CommitWithFn`, `Host.commit_with` (Host 16 to 24 bytes, pinned in raw.rs's layout table), `MINOR` 1 (`tests/contract.rs` pin moved 0 to 1, as the spec mandates); device side `React::with_commit_with` + `React::commit_with` (a lens with no entry gives a fault naming the statement), thunk reads `Host` fields through the raw pointer and offers the entry only when `Host.size` reaches it; solver side `Commit.flags` (unknown bits refused with FAULT). The digital solver does not yet read `Commit.flags` (T7/T8). T7: `Pending.flags` (0 for host and plain commits; `Planned.flags` carries `Commit.flags` from the sink to the queue in `run.rs`); `Queue::push` and `Queue::cancel` of a `MAYBE` commit give `MERGE` to what a plain commit would remove (`Undo::Converted`, cleared by `rewind`); a pending commit already `MERGE` is not journaled as converted.
**Dispatched:** B1 (phases 1-3, T1-T8) - Sonnet worker, 2026-11-18.
**Walked:** none (no screen)

---

## Phase Execution Map

Visual representation of task ordering. Phases run in sequence, and tasks within a phase run in order:

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10 → Phase 11

Phase 1:  T1
Phase 2:  T2 ------→ T3
Phase 2:  T4
Phase 3:  T5 ------→ T6 ------→ T7 ------→ T8
Phase 4:  T9 ------→ T10 -----→ T11 -----→ T12
Phase 5:  T13 -----→ T14 -----→ T15
Phase 6:  T16 -----→ T17
Phase 7:  T18 -----→ T19 -----→ T20 -----→ T21 -----→ T22
Phase 8:  T23
Phase 9:  T24 -----→ T25 -----→ T26 -----→ T27 -----→ T28 -----→ T29
Phase 10: T30
Phase 10: T31
Phase 10: T32
Phase 11: T33 -----→ T34 -----→ T35
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet):

| Batch | Phases | Tasks |
| ----- | ------ | ----- |
| B1 | 1, 2, 3 | T1 to T8 |
| B2 | 4, 5 | T9 to T15 |
| B3 | 6, 7, 8 | T16 to T23 |
| B4 | 9 | T24 to T29 |
| B5 | 10, 11 | T30 to T35 |
