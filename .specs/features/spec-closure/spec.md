# Spec Closure Specification

Opened 2026-11-17 from `GAPS.md` (audit of 2026-11-17) at the user's request: "gostaria que
abrisse feature e escrevesse a spec para todos esses gaps [...] Preciso que TODOS os gaps estejam
endereçados ok? Principalmente esses que tu listou que limitam a expressividade da linguagem ok?"
Spec only: no design, no tasks yet.

The specification owes sentences where the code already decided (G4, G5), describes a solver
that MD-62 replaced (X1) and a conformance state that has moved (X2), and the buck asked for two
language additions nobody has ruled on (G1, G2). The repository still carries documents of the
Verilog-AMS and PHDL eras (H13). This feature closes all seven.

## Questions for the user (open)

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | G1: does the language gain a fixed-point type? The buck's Q4 PID writes `i32` carriers with the scale in their names (`KP_Q4`) and a named shift (`Q_SHIFT`) (`examples/buck/GAPS.md` G1). | (a) **Adopt a core type**: `SFix<N, F>` and `UFix<N, F>`, `N` total bits, `F` fraction bits, stored as `Int<N>`/`UInt<N>` (so it has `Repr` and an integer ABI leaf); `+`, `-` and comparisons need equal `F` (else `E0106`); `*` gives the sum of widths and of fractions; `as` converts between fixed types, integers and `f64` under Part I §1 rule 1 (SC-04). (b) **A stdlib type** after `composite-values` (structs) and `generics` (`impl` on a value type): methods (`a.mul(b)`) instead of operators, since operator traits are not in Appendix H. (c) **Decline**: Appendix G closes G1 naming the scaled-integer idiom | **(a).** Fixed point is the arithmetic of every digital controller in a mixed-signal loop, which is this language's purpose; (b) reads worse than the `i32` it replaces; the representation reuses the integer path, so the cost is the type checker and `as` |
| Q2 | G2: does the language gain a ranged integer? The buck wants the setpoint code limited to 0..=168 (`examples/buck/GAPS.md` G2); it writes `d.min(SETPOINT_MAX)` at the one writer and nothing checks the readers. | (a) **Adopt a ranged type** `Ranged<LO, HI>` over the smallest integer that holds the range, whose arithmetic result outside the range is a run fault. (b) **Adopt `where` on a signal**: `signal code: Bit<u8> = 0 where code <= 168;`, the same form as a param's `where` (§8.1.1), checked at every commit; a commit that fails it is a run fault with a new code `E0230` naming the carrier, the value and the writer (SC-06). (c) **Decline**: Appendix G closes G2 naming the idiom (`min` at the writer, a `require` monitor at the readers) | **(b).** One form the language already has for params, applied to carriers; it checks every writer, which is what the buck's complaint is about; no new type, no new arithmetic rule |
| Q3 | G5: what does a shift by a negative amount do? The provisional rule covers amounts `>= N`; a negative `Int<M>` amount panics in compiled code today (ir-rust F-B9). | (a) **A panic**: `E0105` during a run under known control, `E0104` in a const context. (b) **The other direction**: `x << -k` is `x >> k`. (c) **The amount's bits read as unsigned**: a negative amount is a large one, so the result is that of a shift by `>= N` | **(a).** It is what the code does; a negative shift is a bug in every language that defines it, and (b) hides it |
| Q4 | H13: what happens to the documents of earlier eras? | (a) **Delete** `SPEC_DEVIATIONS.md`, `CLEANUP_PLAN.md`, `docs/audit/`, `docs/reference/` and `VAMS-LRM-2-4.md`; git history keeps them. (b) **Move** them to `docs/archive/`, each with a one-line banner naming its era. (c) **(a), except `VAMS-LRM-2-4.md`**, which moves to `docs/reference/vams-lrm-2-4.md` with a banner: the MSL spec cites it eleven times (`VAMS 6.3.6`, `VAMS 9.21`, `VAMS 4.5.x`) | **(c).** The citations stay resolvable; everything else has no reader |

---

## Problem Statement

`GAPS.md` §5 lists spec holes with a provisional rule and no sentence: `f64 as u10` outside the
range saturates and maps NaN to 0 (G4), and a shift by `N` or more bits gives the mathematical
result truncated to `N` bits (G5); the spec says neither. Part V §15 and §15.2 describe a solver
that runs the microsteps of `t` before solving `t`, and say the device ABI's order "is adopted:
`solver-abi` implements it and rewrites §15 when it changes the code" (`part_v_execution.md:316`).
MD-62 changed the code (solve on the left value, then react, then deliver the change with a
restart); §15, §15.1, §15.2 and the C.2 rows naming `run_digital_at`, `predict_step` and
`settle_digital` were not rewritten, and nothing compares what a reaction at `(t, 0)` reads with
the text (X1). Appendix C.2.3 and G.38 say "nothing is executed", but `transition` runs (X2). The
buck asked for a fixed-point type and a ranged integer and nobody ruled (G1, G2). The first 270
lines of `ROADMAP.md`, the `STATE.md` handoffs older than `solver-restructure`, `README.md`,
`docs/index.md` and five Verilog-AMS-era files describe a product that no longer exists (H13).

## Goals

- [ ] Part I states what `as` from a float to an integer type and a shift by any amount give; the compiled backend and the const evaluator agree with it on every probe of `GAPS.md`.
- [ ] Part V §15, §15.1, §15.2 and the C.2 rows this feature owns describe the order the owner protocol runs, and tests from source pin what a reaction and a step read.
- [ ] Appendix C.2.3 and G.38 say, per item, what runs and which feature owns what does not.
- [ ] G1 and G2 are ruled by the user, and the ruling is in the spec (and, if adopted, in the language).
- [ ] No current document of the repository presents Verilog-AMS, PHDL, Cranelift or a Python bench as Piperine's present.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| §15.2 rule 6 (the restart step), §16.4, §17.2's Resolution on where the truncation error is measured, the `E0503` to `E0508` rows, and the C.2 rows "convergence", "truncation error", "step control" and "restart", C.2.1, C.2.2 item 4 | `solver-hygiene` (lane C, in flight) rewrites them with its numerics (its design: "Only those sentences"); this feature leaves them to it |
| G6, unknown integers per value (S6) | `unknown-semantics` |
| X3, X4, X7, X8, X9 | `host-ii` (in flight) |
| X5 | `params-and-io` |
| X6 | `generics` |
| Making `zi_nd`, `inertial` and `Tran` run (C.2.3's other items) | `analog-operators`, `generics`, `resolved-nets`; this feature makes C.2.3 say so |
| Rewriting the Macro Decisions MD-01 to MD-49 of `STATE.md` | They are a decision log; H13 names the handoff only |
| The galleries type-checked or run (U4) | `conformance` |
| Adding this round's features to `ROADMAP.md`'s queue | The orchestrator does it once for every feature of the round |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| G4's rule | `f as UInt<N>` / `Int<N>`: round toward zero, then saturate to the type's range; NaN gives 0; `+inf` and `-inf` saturate. Integer to integer: truncate to the low `N` bits, or extend (sign for `Int`, zero for `UInt`) | The provisional rule of `ir-rust` (2026-11-15), as Rust does for its own widths; the integer half restates "truncates or extends" exactly | y - orchestrator (provisional rule written as spec) |
| G5's rule for amounts `>= N` | `<<` gives 0; `>>` gives 0 on `UInt<N>` and the sign fill (0 or -1) on `Int<N>` | The provisional rule of `ir-rust`; the mathematical result truncated to `N` bits | y - orchestrator |
| The shift amount's type | Any integer type, independent of the shifted operand's | As in Rust | n - planner |
| X1's direction | The spec is rewritten to the order the code runs, not the reverse | MD-60 item 4 ("Appendix C.2, §17.1.2 and §15.2 are amended to describe the kept algorithm, not the reverse") and MD-62 item 2 (user: "solve on the left value, then react") | y - user (MD-60, MD-62) |
| Where X1's facts are proven | New tests from source in `crates/piperine-api/tests/` (a transient through `Session`), plus `spec_text.rs` for each sentence | MD-57; a sentence about execution order needs a run that would fail if it were false | n - planner |
| C.2.3's form | A table with one row per item: the item, what it exercises, "runs" with the test that runs it or "refused" with the owning feature and `GAPS.md` id; each later feature edits its own row | X2 asks for the truth per item; ownership keeps it true | n - planner |
| ROADMAP's first 270 lines | Rewritten in MSL terms: the north star kept in substance; each open item kept as a line naming its `GAPS.md` id or feature, or dropped when it has no MSL counterpart; items paused by a decision (the plugin gallery, MD-60 item 5) kept as one "later" line naming the decision | H13; nothing the user still wants is lost silently | n - planner |
| STATE handoffs | Every handoff below `solver-restructure` (from `lrm-conformance-wave3` down) is removed; `.specs/features/<name>/` keeps each feature's record | They describe the deleted VAMS front end; the feature folders and git hold the history | n - planner |
| `README.md` and `docs/index.md` | Rewritten for MSL: what the language is, how to run `examples/rc` and `examples/buck`, where the spec, the Book and `GAPS.md` are | "VAMS-era docs" of H13; both open on Verilog-AMS today | n - planner |

**Open questions:** Q1 to Q4 at the top of this file wait for the user.

**Implicit-requirement dimensions.** Medium scope: input validation (SC-01, SC-02: the edge
values NaN, infinities, amounts `>= N` and negative) and state-transition integrity (SC-07,
SC-08: what each phase of an instant reads) are covered; remaining dimensions N/A for this
scope, which edits documents and pins rules the code already follows.

---

## User Stories

### P1: The spec says what `as` and a shift give ⭐ MVP

**User Story**: As a model author, I want the spec to say what a float-to-integer `as` and a
large shift give, so that I can rely on the result without reading the transpiler.

**Why P1**: G4 and G5: the code decided, the spec is silent, and two readings of "truncates" exist.

**SC-01. Float to integer `as` (G4).**

**Acceptance Criteria**:
1. WHEN this feature merges THEN Part I §1 rule 1 SHALL state G4's rule (round toward zero, saturate, NaN to 0, infinities saturate, integer `as` truncates or extends), asserted in `spec_text.rs`.
2. WHEN a reaction computes `5000.7 as u10`, `-3.9 as u10`, `f64::NAN as i8`, `-200.5 as i8`, `1e300 as i64` and `-0.5 as u8` THEN the compiled backend SHALL give 1023, 0, 0, -128, 9223372036854775807 and 0.
3. WHEN the same casts are param defaults THEN the const evaluator SHALL give the same values.
4. WHEN `300u16 as u8` and `-1i8 as u4` and `-1i4 as i8` are computed THEN the system SHALL give 44, 15 and -1.

**SC-02. Shifts (G5).**

**Acceptance Criteria**:
1. WHEN this feature merges THEN Part I §1 SHALL state G5's rule and Q3's answer, asserted in `spec_text.rs`.
2. WHEN `1u8 << 9`, `200u8 >> 8`, `-100i8 >> 9` and `100i8 >> 9` are computed in a reaction THEN the system SHALL give 0, 0, -1 and 0.
3. WHEN the same shifts are param defaults THEN the const evaluator SHALL give the same values.
4. WHERE Q3 is answered (a), IF a shift amount is negative under known control during a run THEN the system SHALL fail the analysis with `E0105` naming the shift, the instance and the time.
5. WHERE Q3 is answered (a), IF a shift amount is negative in a const context THEN the system SHALL report `E0104` naming the shift.

**Independent Test**: One `piperine-transpile` test per probe through a reaction and one
elaboration test per probe through a param default.

---

### P1: §15 describes the solver that runs ⭐ MVP

**User Story**: As a reader of the spec, I want §15 to say in which order an instant runs, so
that I can predict what a reaction and a step read.

**Why P1**: X1. The text contradicts the code on when a change is read, which decides results.

**SC-07. The text of §15 and C.2.**

**Acceptance Criteria**:
1. WHEN this feature merges THEN §15's time-point procedure SHALL state the owner protocol's order: land on `t` solving on the left value, accept, fire crossings located at `t`, run the instant at `t` whose reactions read the solution at `t`, run the settle phase when `t` is a time point, then deliver what changed with the restart the next step starts from.
2. WHEN this feature merges THEN invariants I5 and I6, the settle-commit rules S1 to S7, the table "What each kind of code reads" and the trace of §15.1 SHALL agree with that order.
3. WHEN this feature merges THEN §15.2 SHALL state which step reads a change made at `t` under that order, and the note that begins "This section and §15 describe the present solver" SHALL be gone.
4. WHEN this feature merges THEN the C.2 rows "microstep loop", "the jump", "time points" and "rollback" SHALL name only functions and types that exist in the solver crates.
5. The system SHALL hold each sentence SC-07 changes with an assertion in `spec_text.rs`.
6. The system SHALL leave unedited by SC-07 every sentence the Out of Scope table gives to `solver-hygiene`.

**SC-08. Tests from source pin the order.**

**Acceptance Criteria**:
1. WHEN a reaction runs at `(t, 0)` on an `at(t)` and reads a node THEN it SHALL read the solution at `t`, solved with the discrete values before `t`'s commits.
2. WHEN a commit due at `t` changes a value `analog` reads THEN the first step after `t` SHALL read it, restarted, and the step that arrives at `t` SHALL NOT.
3. WHEN a reaction on `settle` runs at a time point THEN it SHALL read the final discrete state of `t` and the solution at `t`.
4. WHEN the trace of §15.1 (`Front` with a crossing at 10 ns on a clock edge) runs from source THEN its records SHALL list the instants, the applied values and the reactions in the order §15.1's table gives.
5. WHEN the charge-sharing example of Appendix F.5 runs from source THEN both capacitors SHALL hold 0.25 V at the first step that reads the switch, to the accuracy of that step.

**Independent Test**: `crates/piperine-api/tests/instant_order.rs` (new), one test per AC, each
asserting values that differ under the other order.

---

### P1: The conformance set says what runs ⭐ MVP

**User Story**: As a reader of Appendix C, I want C.2.3 and G.38 to say what runs today, so that
I know which guarantees are proven.

**Why P1**: X2. "Nothing is executed" is false.

**SC-09. C.2.3 and G.38.**

**Acceptance Criteria**:
1. WHEN this feature merges THEN C.2.3 SHALL give, for each of `transition`, `zi_nd`, `inertial`, `Tran` on `Wire`s and the trace of §15.1, either "runs" with the test that runs it or "refused" with the owning feature and `GAPS.md` id.
2. WHEN this feature merges THEN C.2.3 SHALL say "runs" for `transition` and for the §15.1 trace (SC-08 AC 4), and "refused" for `zi_nd` (`analog-operators`, `generics`), `inertial` (`generics`) and `Tran` (`resolved-nets`).
3. WHEN this feature merges THEN G.38 SHALL say that the library and the galleries are parsed and resolved, which stdlib items run (the list of `GAPS.md` §1), and point at C.2.3 for the conformance set.
4. The system SHALL hold each row's status with an assertion in `spec_text.rs` that names the test or the feature.

---

### P2: The buck's language proposals are ruled

**User Story**: As the designer of the buck, I want a ruling on a fixed-point type and a ranged
integer, and the language to have them if adopted, so that the PID and the setpoint say what
they mean.

**Why P2**: G1 and G2 are language proposals, not defects; the user named expressiveness first,
so they are specified here rather than parked.

**SC-03. The ruling is recorded.**

**Acceptance Criteria**:
1. WHEN the user answers Q1 and Q2 THEN Appendix G SHALL record G1 and G2 as items with the buck's need, the answer and its status (adopted with the section that defines it, or declined with the idiom).
2. WHEN the user answers THEN `.specs/STATE.md` SHALL record the answers as one `MD-xx` decision.

**SC-04. A fixed-point type, if adopted.**

**Acceptance Criteria**:
1. WHERE Q1 is answered (a), WHEN a model declares `const KP: SFix<16, 4> = 2.5;` THEN the system SHALL store it as the `Int<16>` value 40.
2. WHERE Q1 is answered (a), WHEN two fixed values of equal `F` are added THEN the result SHALL be the sum of their integer values with the same `F`, wrapping as `Int<N>` does (§1 rule 6).
3. WHERE Q1 is answered (a), IF two fixed values of different `F` are added or compared THEN the system SHALL report `E0106` naming both types.
4. WHERE Q1 is answered (a), WHEN `SFix<N1, F1>` is multiplied by `SFix<N2, F2>` THEN the result SHALL be `SFix<N1 + N2, F1 + F2>` holding the exact product.
5. WHERE Q1 is answered (a), WHEN a fixed value is converted with `as` to another fixed type, an integer type or `f64` THEN the result SHALL follow Part I §1 rule 1 (SC-01) on the scaled value.
6. WHERE Q1 is answered (a), WHEN the buck's PID is rewritten with fixed-point types and the run `startup` runs THEN every record of its `checks` stream and every duty word SHALL equal the `i32` version's, bit for bit.

**SC-05. The ranged-integer ruling.**

**Acceptance Criteria**:
1. WHERE Q2 is answered (c), WHEN this feature merges THEN Appendix G SHALL close G2 naming the idiom of `examples/buck` (`min` at the writer, a `require` at the readers).

**SC-06. A `where` on a signal, if adopted.**

**Acceptance Criteria**:
1. WHERE Q2 is answered (b), WHEN a model declares `signal code: Bit<u8> = 0 where code <= 168;` and a reaction commits 100 THEN the carrier SHALL hold 100.
2. WHERE Q2 is answered (b), IF a commit gives the carrier a value for which its `where` is false under known control THEN the system SHALL fail the analysis with `E0230` naming the carrier, the value and the reaction that wrote it.
3. WHERE Q2 is answered (b), IF the initial value fails the `where` THEN the system SHALL report `E0230` at elaboration naming the carrier and the initial value.
4. WHERE Q2 is answered (b), IF a signal's `where` reads anything but this carrier, params and constants THEN the system SHALL report `E0318` naming what it reads.
5. WHERE Q2 is answered (a), WHEN a `Ranged<0, 168>` value is computed outside its range under known control THEN the system SHALL fail the analysis with `E0230` naming the expression.

**Independent Test**: The buck's PID in fixed point against its `i32` run; a ranged setpoint
written above 168 from a bench.

---

### P2: The repository documents say what is true

**User Story**: As a newcomer, I want the README, the docs index, the roadmap and the state file
to describe MSL and the current solver, so that I do not learn a retired product first.

**Why P2**: H13; no behavior depends on it, and each stale page misleads.

**SC-10. ROADMAP.md.**

**Acceptance Criteria**:
1. WHEN this feature merges THEN the part of `ROADMAP.md` above "Out of agent scope" SHALL present no PHDL, no Verilog-AMS front end, no Cranelift JIT and no Python bench as current, checked by a test that greps it.
2. WHEN this feature merges THEN each open item of those lines SHALL be either kept as a line naming its `GAPS.md` id or feature, kept as a "later" line naming the decision that paused it, or dropped, and the commit message SHALL list the dropped items.

**SC-11. STATE.md.**

**Acceptance Criteria**:
1. WHEN this feature merges THEN `.specs/STATE.md`'s Handoff Snapshot SHALL hold no handoff of a feature older than `solver-restructure`, and SHALL name `.specs/features/` and git as where they are.

**SC-12. Files of earlier eras.**

**Acceptance Criteria**:
1. WHERE Q4 is answered (a) or (c), WHEN this feature merges THEN `SPEC_DEVIATIONS.md`, `CLEANUP_PLAN.md`, `docs/audit/` and the two Annex ports of `docs/reference/` SHALL be absent.
2. WHERE Q4 is answered (a), WHEN this feature merges THEN `VAMS-LRM-2-4.md` SHALL be absent and each `VAMS x.y` citation of `docs/spec/` SHALL name the LRM edition and section without a file link.
3. WHERE Q4 is answered (c), WHEN this feature merges THEN the LRM SHALL sit at `docs/reference/vams-lrm-2-4.md` under a banner saying it is the informative source of the spec's `VAMS x.y` citations.
4. WHERE Q4 is answered (b), WHEN this feature merges THEN the five items SHALL sit under `docs/archive/`, each opening with a banner naming its era.
5. WHEN this feature merges THEN `README.md` and `docs/index.md` SHALL describe MSL, `examples/rc`, `examples/buck`, the spec, the Book and `GAPS.md`, and SHALL present no Verilog-AMS or PHDL front end as current, checked by the test of SC-10 AC 1.
6. WHEN this feature merges THEN no file of the repository SHALL link to a removed path, checked by a test that resolves every relative Markdown link of the root documents and `docs/`.

**SC-13. Records.**

**Acceptance Criteria**:
1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list G1, G2, G4, G5, X1, X2 and H13, and `ROADMAP.md` SHALL no longer list G1, G2, G4 and G5 under "MSL spec gaps".

---

## Edge Cases

- WHEN `0.0 as UInt<1>` and `1.9999 as UInt<1>` are computed THEN the system SHALL give 0 and 1.
- WHEN `-0.0 as i8` is computed THEN the system SHALL give 0.
- WHEN a shift amount equals `N - 1` THEN the system SHALL give the ordinary shift.
- WHEN a shift amount is of a wider type than the operand (`1u8 << 300u16`) THEN the system SHALL give 0.
- IF `solver-hygiene` merges after this feature and rewrites a C.2 row this feature also touched THEN that feature's text SHALL win for its rows (this feature edits none of them).

---

## Gaps closed

| GAPS id | Closed by | How |
| ------- | --------- | --- |
| G1 fixed-point type | SC-03, SC-04 | ruled by the user (Q1); adopted and implemented, or declined in Appendix G |
| G2 ranged integer | SC-03, SC-05, SC-06 | ruled by the user (Q2); a `where` on a signal or a ranged type, or declined with the idiom |
| G4 float to `UInt<N>`/`Int<N>` outside the range | SC-01 | the provisional rule becomes Part I §1 rule 1 |
| G5 shift by `N` or more | SC-02 | the provisional rule and the negative amount (Q3) become spec text |
| X1 §15 describes a solver that is gone | SC-07, SC-08 | rewritten to MD-62's order, except the sentences `solver-hygiene` owns; pinned by tests from source |
| X2 C.2.3 and G.38 say "nothing is executed" | SC-09 | per-item status with owner |
| H13 stale repository documents | SC-10, SC-11, SC-12 | rewritten, removed or moved (Q4) |

No error code of `GAPS.md` §10 belongs here. Two codes may be added: `E0230` (Q2 (a) or (b)) and
the use of `E0106` (from `params-and-io` Q2) for fixed-point mismatches.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SC-01 | P1: `as` and shifts | - | Pending |
| SC-02 | P1: `as` and shifts | - | Pending |
| SC-03 | P2: the buck's proposals | - | Pending |
| SC-04 | P2: the buck's proposals | - | Pending |
| SC-05 | P2: the buck's proposals | - | Pending |
| SC-06 | P2: the buck's proposals | - | Pending |
| SC-07 | P1: §15 describes the solver that runs | - | Pending |
| SC-08 | P1: §15 describes the solver that runs | - | Pending |
| SC-09 | P1: the conformance set says what runs | - | Pending |
| SC-10 | P2: the repository documents | - | Pending |
| SC-11 | P2: the repository documents | - | Pending |
| SC-12 | P2: the repository documents | - | Pending |
| SC-13 | P2: the repository documents | - | Pending |

**Coverage:** 13 total, 0 mapped to tasks (spec only).

---

## Dependencies and size

- **Depends on:** `solver-hygiene` (in flight) for the Part V and C.2 sentences it owns; SC-07
  should land after it to avoid two rewrites of one section. `params-and-io` for `E0106` (SC-04
  AC 3). `host-ii` (in flight) for a digital-only transient if SC-08 AC 4 runs `Front` without an
  analog node (`Front` has no `analog` block: L56).
- **Touches:** `docs/spec/` Part I §1, Part V §15 to §15.2, Appendix C.2 and C.2.3, Appendix G;
  `crates/piperine-lang/tests/spec_text.rs`; a new `crates/piperine-api/tests/instant_order.rs`;
  `ROADMAP.md`, `.specs/STATE.md`, `README.md`, `docs/index.md`, `mkdocs.yml`; the removed or
  moved files of Q4. Under Q1 (a) and Q2 (a) or (b): `piperine-lang` (types, elaboration),
  `piperine-ir`, `piperine-transpile`.
- **Size:** S if Q1 and Q2 are declined (about 10 tasks: the two rules, §15, C.2.3, documents).
  M if both are adopted (about 22 tasks: fixed point about 8, signal `where` about 4).

## Sources

- `GAPS.md` §5 (G1, G2, G4, G5, X1, X2), §7 (H13); `examples/buck/GAPS.md` G1, G2, G4, G5.
- `ROADMAP.md` "MSL spec gaps (spec text owed)".
- `docs/spec/part_i_values.md` §1; `part_v_execution.md` §15, §15.1, §15.2 (the note at line
  316); `appendix_c_implementation.md` C.2, C.2.3; `appendix_g_scope.md` G.38.
- `crates/piperine-solver/src/protocol/transient.rs` lines 1 to 17: the protocol's order.
- `.specs/STATE.md` MD-57, MD-60 item 4, MD-62 item 2.
- `.specs/features/solver-hygiene/spec.md` and `design.md` (branch of lane C): the sentences it
  rewrites.

## Success Criteria

- [ ] The six G4 probes and four G5 probes give the spec's values on the compiled backend and in the const evaluator.
- [ ] `instant_order.rs` fails if the order of §15 is swapped, and passes as written.
- [ ] `spec_text.rs` asserts every changed sentence.
- [ ] A grep of the current documents finds no PHDL, Verilog-AMS front end, Cranelift or Python bench presented as current.
