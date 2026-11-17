# Unknown Semantics Specification

The unknown-value rules of part I §3.3 and §22.1, as the spec writes them. This is the first of
three specs for one area. It covers the values and the merge. The other two are
`unknown-semantics-per-level` (S6, G6: per-level flags on integers, which changes the ABI) and
`unknown-semantics-events` (L51, L52, L53: events as `bool`, `settle`, functions that return
`Event`, and what they unblock). The criteria here hold whatever the user decides for per-level
flags. They use `bool`, enums, whole integers and struct fields, where one flag per value and
one flag per level give the same answer.

## Open questions (the user's, before approval)

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | `merge`, `same` and `cast` are generic functions of `std/values.msl` (§22.1). Generic functions are refused today (L7) and belong to `generics`. Do they wait for `generics`, or come first? | **A.** Wait. The three compile from their stdlib source once `generics` lands. US-03 ships after `generics`; resolved-nets (`Logic::wired` calls `merge`) waits too. **B.** Contract first. The checker types a call of `merge`, `same` or `cast` at its concrete type, and the elaborator lowers it by the normative contract of §22.1.2 to §22.1.4, as it already must for the `extern "intrinsic"` `matches_bits` and `logic`. The stdlib text stays as written. When `generics` lands, a differential test runs the source body against the contract lowering, and the special case is deleted. | **B.** `merge` is "§3.3.3 used as a function" (§22.1.2 rule 3) and adds no rule, so the contract is the language's own. B also unblocks `resolved-nets` without waiting for the whole of `generics`. Cost: three rows of special typing that live until `generics` lands, and one differential test. |

---

## Problem Statement

Part I §3.3 says an unknown condition takes every path and merges the effects level by level.
The compiled backend instead stops the run: `fault 3 in Top at tick 0: the condition of an if
is unknown` (L19). So a register with no reset cannot be modelled the way the spec says. Models
must write `known_or` in front of every condition, and `maybe = true` is never written on a
record. The elaborator also refuses forms that should simply give an unknown result: a math
function of a value that may be unknown, a `known_or` default that may be unknown, a right
operand of `&&` or `||` that can panic. The tools that read and build unknowns are refused too
(L18): `Unknown` as an initial value, `x.level(k)`, `Repr::WIDTH`, `merge`, `same`, `cast`,
`logic` and `matches_bits`.

These gaps limit what a model can say about `x`, the digital value a real circuit has before
reset. Resolved nets (`Logic::Z` is `Unknown`, `Logic::wired` calls `merge`) and every stdlib
monitor need them.

## Goals

- [ ] Unknown control runs as §3.3.3 says: every path that is not excluded is taken and the effects are merged. No run-time fault remains for an unknown condition, scrutinee, guard or short-circuit operand.
- [ ] Every form of L18 elaborates and runs on the compiled backend: `Unknown` as an initial value and in a const, `x.level(k)`, `T::WIDTH`, `merge`, `same`, `cast`, `logic`, `matches_bits`.
- [ ] Every elaboration refusal that L19 names is gone. Digital code computes with values that may be unknown, and `analog` refuses them with `E0150` naming the carrier, not with `P0001`.
- [ ] A record reached under unknown control carries `maybe = true`.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Per-level flags of an integer: `(x & 1) & 2`, `merge(0b10, 0b11)`, `logic("10xx")` on a `u4` (S6, G6) | `unknown-semantics-per-level`: an ABI decision of the user's |
| An event read as a `bool`, `settle`, functions returning `Event`, `limexp`, the Book's clipper and chapter-8 buck (L51, L52, L53) | `unknown-semantics-events` |
| `for`, `while`, `loop`, `break`, `let mut` (L5); arrays and indexing (L3, L24) | `composite-values`. This feature states the merge rule for loops and unknown indices (US-04 AC 11 and 12); those criteria are checked once the forms exist |
| Generic functions in general (L7), `impl` on a value type | `generics`. Q1 decides whether `merge`, `same` and `cast` wait for it |
| `finish()` and `fatal()` ending the analysis (S1) | `msl-fixes` (lane A). This feature only keeps a `finish` reached under unknown control from ending it (US-05 AC 3) |
| The interpreter backend | Frozen at the M1 analog subset (codegen context R1). Every criterion here runs on the Rust backend |
| Spec error codes that belong to no construct of this feature (E0102, E0105, E0503 to E0508, E0803 to E0806) | `params-and-io` (GAPS §10) |
| Resolved nets, `Logic`, `Wire` | `resolved-nets` |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Q1: `merge`, `same`, `cast` before `generics` | Pending. The criteria hold under A and under B; only the order of delivery changes | Open question at the top | n |
| The bound on loop unrolling under unknown control (§3.3.16) | 64 iterations, written in Appendix C.2's table and applied in every analysis | §3.3.16 makes the bound implementation-defined and says only that it MUST be documented. 64 covers every loop of the galleries and the stdlib. A larger bound keeps more levels known and changes nothing else (G.46) | n - planner |
| Where the merge runs | In the generated Rust of a reaction, a definition, a body `let` and a function body: the same rule everywhere (§3.3 opening sentence) | "One rule covers carriers, locals, function arguments, closures and library code alike" | n - planner |
| The fault of an unknown `if` | Removed, with no session option to restore it | §3.3.3 is normative, and a model that wants Verilog's behaviour writes `known_or(false)` (§3.3, last paragraph). An option would be a dialect | n - planner |
| How a record says `maybe` | A `bool` field `maybe` in the record's layout, read by the host like any field and printed only when true | §19 rule 1 makes it part of every record, and the name is reserved (§19 rule 11). The ABI needs nothing new: a record is a commit to a `RECORD` pin with the device's own layout (abi.md C5) | n - planner |
| A panic under unknown control (§1 rule 13) | The panicking path contributes unknown to every effect of that merge: each value it would have yielded or committed is `Unknown` | §1 rule 13 says "Every effect of that merge becomes unknown" | n - planner |
| The `analog` refusal of a value that may be unknown | `E0150` naming the carrier, as §3.3.5 and §14 say, in place of today's `P0001` | The spec gives the code; the check already exists | n - planner |

**Open questions:** Q1, at the top of this file. No criterion depends on its answer.

**Implicit-requirement dimensions.** Input validation: US-01 AC 4, US-03 AC 8 and 9 (`E0151`, `E0701`, `E0702`). Failure states: US-04 AC 7 (a panic under unknown control is not a fault), US-02 AC 4 (a panic on a known out-of-range index). Idempotency and retry: N/A because elaboration and a run are deterministic (§15 I1). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: US-04 AC 2 and the snapshot rule I2, which this feature does not change. Data lifecycle: N/A because nothing is stored. Observability: US-05 (`maybe`). External dependencies: N/A because `rustc` is the build's own concern (MD-63). State transitions: US-01 AC 2 (unknown to unknown is no change)

---

## User Stories

### P1: `Unknown` is a value a model can write ⭐ MVP

**User Story**: As a model author, I want to write `Unknown` as an initial value, a committed value and in a const, so that a register with no reset and a high-impedance constant say what they are.

**Why P1**: `Logic::Z` is a const built with `Unknown`, and every stdlib net builds on it.

**Acceptance Criteria** (US-01):

1. WHEN a signal or an `out` port of a discrete nature is declared `= Unknown` THEN the system SHALL elaborate it and start the carrier with every level unknown at `(0, 0)`.
2. WHEN a carrier declared `= Unknown` is observed at `(0, 0)` THEN the system SHALL NOT fire its `changed()` there, since unknown to unknown is no change (§3.3, "changes").
3. WHEN a const's expression uses `Unknown` (`const Z: Option<bool> = Some(Unknown);`) THEN the system SHALL accept it and give every read of the const the same unknown levels (§3.3.9).
4. IF `Unknown` decides structure (a generation condition, a generation loop bound, a const generic, an array length or a `where` condition) THEN the system SHALL report `E0151` at elaboration, naming the decision.
5. WHEN a reaction commits `q <- Unknown;` THEN the system SHALL make every level of `q` unknown at the microstep the commit applies.

**Independent Test**: An elaboration test for `E0151`, and a compiled run that prints `s.is_known()` and the record of `on s.changed()` for a `Bit<u8> = Unknown`.

---

### P1: Reading the flag ⭐ MVP

**User Story**: As a model author, I want `x.level(k)` and `T::WIDTH`, so that I can write my own flag-aware functions, as the stdlib does.

**Why P1**: `known`, `same` and `matches_bits` are defined on `level` (§22.1). A project that replaces one (§22 rule 3) needs it.

**Acceptance Criteria** (US-02):

1. WHEN `x.level(k)` is evaluated with a known `k` in `0..T::WIDTH` THEN the system SHALL return the known value `Some(b)` when level `k` of `x` is known and holds `b`, and `None` when it is unknown.
2. WHEN `x.level(k)` is evaluated with a known `k` THEN the system SHALL give a known result, whatever the flags of `x` (§3.3.14).
3. WHEN `T::WIDTH` is read for a concrete `Repr` type `T` THEN the system SHALL give its level count as a const `u32`: 1 for `bool`, `N` for `UInt<N>` and `Int<N>`, the tag width of §2 for a unit enum.
4. IF `k` is known and not less than `T::WIDTH` THEN the system SHALL raise the panic of §1 rule 11, a run fault naming the expression and `k`.
5. WHEN `k` is unknown THEN the system SHALL give the merge of `x.level(j)` over every `j` that the known levels of `k` do not exclude (§3.3.13).

**Independent Test**: A compiled run that prints `level` over a known `u8`, an unknown `u8` and a `bool`, and a fault test for `k = 8` on a `u8`.

---

### P1: The stdlib functions of §22.1 ⭐ MVP

**User Story**: As a model author, I want `merge`, `same`, `cast`, `logic` and `matches_bits`, so that I can combine, compare, reinterpret and pattern-match values with their flags.

**Why P1**: `Logic::wired` calls `merge`; the monitors of §22.5 and the galleries call `same` and `matches_bits`.

**Acceptance Criteria** (US-03):

1. WHEN `merge(a, b)` is evaluated on two known equal values THEN the system SHALL give that value, known.
2. WHEN `merge(a, b)` is evaluated on two `bool`s, two unit enum values or two whole-known integers that differ THEN the system SHALL give a result whose differing levels are unknown, and SHALL give `merge(a, b)` and `merge(b, a)` the same levels and flags.
3. WHEN `same(a, b)` is evaluated THEN the system SHALL give a known `true` exactly when every level of `a` and `b` has the same flag and, where known, the same value; `same(Unknown, Unknown)` SHALL be `true` and `Unknown == Unknown` SHALL be unknown (§3.3.10).
4. WHEN `cast::<A, B>(a)` is evaluated with `A::WIDTH == B::WIDTH` THEN the system SHALL give the value whose level `k` is level `k` of `a` with its flag.
5. WHEN `B::decode` of the levels of `a` returns `None` (a tag with no variant) THEN `cast` SHALL give `Unknown`.
6. WHEN `logic::<T>(p)` is evaluated with a pattern of only `0` and `1` THEN the system SHALL give the known value whose leftmost character is level `T::WIDTH - 1`; WHEN every character is `x` THEN the system SHALL give a value with every level unknown.
7. WHEN `matches_bits(s, p)` is evaluated THEN the system SHALL give a known `true` exactly when each level matches its character: `x` matches any level, `0` and `1` match only a known level of that value.
8. IF `cast` is called with `A::WIDTH != B::WIDTH` THEN the system SHALL report `E0701` naming both types.
9. IF the pattern of `logic` or `matches_bits` has a character other than `0`, `1`, `x`, or a length other than `T::WIDTH` THEN the system SHALL report `E0702` at the pattern.

**Independent Test**: One compiled test per function with the tables of §22.1.2 to §22.1.4, and elaboration tests for `E0701` and `E0702`.

---

### P1: Unknown control is a merge ⭐ MVP

**User Story**: As a model author, I want an unknown condition to take every path and merge the effects, so that an unreset register stays unknown instead of stopping the run.

**Why P1**: The heart of §3.3 (L19). Without it every digital model must guard every decision by hand.

**Acceptance Criteria** (US-04):

1. WHEN the condition of an `if` in a reaction is unknown THEN the system SHALL run both arms and SHALL NOT fault.
2. WHEN both arms of that `if` commit to carrier `q` THEN the system SHALL commit `merge` of the two values, level by level, at the microstep the commits are due.
3. WHEN only one arm commits to `q` THEN the system SHALL commit the merge of that value and the value `q` would hold then (§3.3.3).
4. WHEN both arms commit the same known value under an unknown condition THEN the system SHALL commit that value known.
5. WHEN `q <- if c { a } else { b };` and `if c { q <- a; } else { q <- b; }` run with the same `c`, `a` and `b` THEN the system SHALL make the same commit.
6. WHEN the scrutinee of a `match` is unknown THEN the system SHALL take every arm that the known levels do not exclude and merge their effects; with one flag per value that is every arm.
7. WHEN a path taken under unknown control panics THEN the system SHALL NOT fault, and SHALL make unknown every effect of that merge: each value it yields and each commit it makes (§1 rule 13).
8. WHEN the left operand of `&&` or `||` is unknown THEN the system SHALL evaluate the right operand under the merge and give `unknown && false == false`, `unknown || true == true` and unknown otherwise (§3.3.12), also when the right operand can panic or emit.
9. WHEN a guard `ev && c` fires with `c` unknown THEN the system SHALL run the reaction and merge each of its effects with not running it (§13 rule 1).
10. WHEN a definition, a body `let`, an `if` expression or a function body decides on an unknown value THEN the system SHALL apply the same merge as in a reaction.
11. WHERE loops exist (`composite-values`, L5) the system SHALL treat a loop whose exit is unknown as its unrolling merged exit by exit, cut at the bound written in Appendix C.2, after which every local its body writes is unknown (§3.3.3, §3.3.16).
12. WHERE arrays exist (`composite-values`, L3, L24) the system SHALL read an unknown index as the merge of the elements its known levels do not exclude, and write through it by merging the value into each of them (§3.3.13).
13. WHEN a commit's delay is unknown THEN the system SHALL commit `Unknown` at the next microstep, as today.

**Independent Test**: The example of §3.3 (`Pfd3` with `state` unreset) and the `if`/`match`/guard probes of L19 run to their stop time, each commit checked level by level through `same` in an `emit!`.

---

### P1: What a record says about unknown control ⭐ MVP

**User Story**: As a reader of a run's records, I want each record to say whether it was reached under unknown control, so that I can tell "it happened" from "it may have happened".

**Why P1**: §19 rule 1; without it a merged `emit!` is indistinguishable from a real one.

**Acceptance Criteria** (US-05):

1. WHEN an `emit!` runs on a path taken under unknown control THEN the system SHALL deliver its record with `maybe = true`.
2. WHEN an `emit!` runs under known control THEN the system SHALL deliver its record with `maybe = false`, and the command line SHALL print no `maybe` field for it.
3. WHERE `finish()` and `fatal()` end the analysis (`msl-fixes`, S1) the system SHALL NOT end the analysis on a `sim` record whose `maybe` is true (§19, Resolution).
4. WHEN a `require` is reached under unknown control THEN the system SHALL deliver its `checks` record with `maybe = true`, and `piperine test` SHALL judge it as `piperine test` judges an unknown check.
5. The system SHALL keep each field's known flags in the record: an unknown field is delivered and printed as unknown.

**Independent Test**: A compiled run with one `emit!` under an unknown `if` and one under a known `if`, read through `piperine-api` and printed by `piperine run`.

---

### P1: Unknown values flow through digital code ⭐ MVP

**User Story**: As a model author, I want math functions and `known_or` to accept values that may be unknown in digital code, and `analog` to refuse them with the spec's code, so that the only refusals left are the spec's.

**Why P1**: The three elaboration refusals of L19 are `P0001` for forms the spec defines.

**Acceptance Criteria** (US-06):

1. WHEN a math function or method (`abs`, `sqrt`, `exp`, ...) is applied in a reaction, a definition or a function to a value that may be unknown THEN the system SHALL elaborate it and give an unknown result when an argument is unknown (§3.3.2, all-or-nothing).
2. WHEN `x.known_or(d)` is evaluated in digital code with a `d` that may be unknown THEN the system SHALL elaborate it, and give `x` when every level of `x` is known and otherwise `d` with its flags (§22.1.1).
3. IF `analog` reads a value that may be unknown other than through `level` or a flag-total function, in a contribution, a `known_or` default, a math function or an operator argument (`ddt`, `delay`, `limit`, `noise`), THEN the system SHALL report `E0150` naming the carrier, and SHALL NOT report `P0001`.
4. WHEN a function's every use of a parameter passes through `level` THEN the system SHALL treat the function as flag-total for that parameter, and `analog` SHALL accept a carrier read passed to it (§3.3.6).

**Independent Test**: Elaboration tests over the three refused forms of L19 (accepted in a reaction, `E0150` in `analog`), and one compiled run of `(x.sqrt()).known_or(-1.0)` with `x` unknown then known.

---

### P2: The documents say what is true

**User Story**: As a reader of `GAPS.md`, the Book and the spec, I want the unknown rules described as they run.

**Why P2**: No behaviour depends on it; the Book's listings are held to the binary.

**Acceptance Criteria** (US-07):

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L18 or L19, and SHALL keep S6 and G6 unless `unknown-semantics-per-level` has merged.
2. WHEN this feature merges THEN Appendix C.2 SHALL state the loop bound of §3.3.16, with a `spec_text` guard (MD-57).
3. The system SHALL keep every command-line listing of the Book equal to the binary's output, the Book's chapter on unknown values edited where its examples now run.

**Independent Test**: `cargo test -p piperine-lang --test spec_text` and `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN an `if` condition is unknown and neither arm commits to `q` THEN the system SHALL commit nothing to `q`.
- WHEN an unknown condition guards an `at` re-arm THEN the system SHALL treat the armed time as unknown, and an unknown time does not fire (§13 rule 4).
- WHEN a `match` on an unknown enum is exhaustive THEN the system SHALL merge all its arms, including a wildcard arm.
- WHEN `merge(a, a)` is evaluated with `a` partly unknown THEN the system SHALL give the levels and flags of `a`.
- IF a function called under unknown control emits a record THEN the system SHALL deliver it with `maybe = true`.
- WHEN a merged commit equals the carrier's current value, levels and flags THEN the system SHALL NOT fire `changed()`.
- IF an unknown condition reaches a decision the IR fixes at elaboration (structure) THEN the system SHALL report `E0151` and SHALL NOT run.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| US-01 | P1: `Unknown` is a value a model can write | - | Pending |
| US-02 | P1: Reading the flag | - | Pending |
| US-03 | P1: The stdlib functions of §22.1 | - | Pending |
| US-04 | P1: Unknown control is a merge | - | Pending |
| US-05 | P1: What a record says about unknown control | - | Pending |
| US-06 | P1: Unknown values flow through digital code | - | Pending |
| US-07 | P2: The documents say what is true | - | Pending |

**Coverage:** 7 total, 0 mapped to tasks, 7 unmapped (no tasks yet).

### Gaps closed

| GAPS id | What it says | Closed by |
| ------- | ------------ | --------- |
| L18 | `Unknown` as an initial value; `x.level(k)`; `merge`, `same`, `cast`, `logic`, `matches_bits`; `Repr::WIDTH` | US-01, US-02, US-03 (partial-level cases of `merge` and `logic` on integers: `unknown-semantics-per-level` UL-02, UL-03) |
| L19 | unknown control is a fault, not a merge; `maybe` never written; math function, contribution and `known_or` default refused | US-04, US-05, US-06 |
| S6, G6 | per-level flags of integers | `unknown-semantics-per-level` |
| L51, L52, L53 | events as `bool`, `settle`, `Event` functions | `unknown-semantics-events` |

## Dependencies

- `msl-fixes` (lane A, in flight): S1 (`finish`, `fatal`) for US-05 AC 3.
- `composite-values`: L5 loops and L3/L24 arrays for US-04 AC 11 and 12 only; the rest does not wait.
- `generics`: only under Q1 = A, for US-03 AC 1 to 5.
- Downstream: `unknown-semantics-events` (the guard merge of US-04 AC 9), `resolved-nets` (`Logic::Z`, `merge`), `unknown-semantics-per-level`.

## Size

**L.** About 26 tasks: US-01 3, US-02 4, US-03 7, US-04 7, US-05 3, US-06 2. Phases: values (US-01, US-02), the merge in generated code (US-04, US-06), records (US-05), the stdlib functions (US-03), documents.

## Sources

- `GAPS.md` §4.1 L18, L19 (probes and diagnostics); §3 S6 and §5 G6 for the boundary with the per-level spec.
- `docs/spec/part_i_values.md` §1 rule 11, §1 rule 13, §3.3 (rules 1 to 16, the interactions and errors tables).
- `docs/spec/part_vii_stdlib.md` §22.1.1 to §22.1.4.
- `docs/spec/part_iv_behavior.md` §12.1.19, §13 rule 1; `docs/spec/part_vi_io.md` §19 rules 1 and 11 and the `finish` resolution.
- `.specs/features/ir-rust/spec.md` row "One known flag per scalar"; `crates/piperine-transpile/src/rt/flag.rs` (`decide`, the fault); `crates/piperine-lang/src/elab/known.rs`, `lower.rs:186`, `lower.rs:256`, `contribute.rs:226` (today's refusals).
- `.specs/STATE.md` MD-57, MD-63, MD-64; codegen context R1.

---

## Success Criteria

- [ ] No `P0001` of the L18 and L19 rows remains, and no run faults on an unknown condition.
- [ ] The §3.3 example `Pfd3` runs unreset and its `state` stays unknown until a known commit.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.
