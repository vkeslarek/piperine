# Unknown Semantics: Per-Level Flags Specification

The second spec of the unknown-value area (see `unknown-semantics`). It closes S6 and G6: an
integer's unknown levels are tracked one by one, as §3.3.2 says, or the spec is amended to admit
one flag per value. Which of the two is the user's decision (Q1). On 2026-11-15 the orchestrator
ruled one flag per value as provisional (`ir-rust` F-B4), and noted that the user may revisit it.
This spec puts that ruling back in front of the user, with the cost of each option.

## Open questions (the user's, before approval)

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How are the unknown levels of an integer (and of any value wider than one level) tracked? Today a carried value has one known byte (abi.md §3, `Field.known`: one byte per element), so `((x & 1) & 2).known_or(9)` gives 9 where §3.3.2 gives 0. | **A. Keep one flag per value; amend the spec.** §3.3.2 and §22.1 gain a rule that an implementation MAY round partly known values up to all unknown, and G6 becomes a documented rounding, like G.46. Cost: S (about 4 tasks: spec text and guard, GAPS, Book). No ABI change. Loss: `logic("10xx")` on a `u4` is all unknown, `merge(0b1010, 0b1011)` is all unknown, `x.level(k)` is `None` for every `k` of a partly known value, a `match` on a partly known enum takes every arm. Results then differ from any per-level implementation in more than "how many levels stay known under a loop bound". **B. Per-level flags for every leaf, a new ABI version.** `Field.known` becomes, for every leaf, a mask the width of the leaf: bit `k` set means level `k` known. Every layout changes, including the shipped `BIT`, `REAL` and `BYTE`. The export becomes `piperine_abi_2` and every device rebuilds. Cost: L (about 16 tasks). **C. Per-level flags as an additive ABI flag.** A new `Field.flags` bit, `KNOWN_LEVELS`, says that `known` is the offset of a mask the width of the leaf, per element. A field without it keeps one known byte. The elaborator gives the mask to every leaf wider than one level (integers, enum tags, `f64` for `cast`). `BIT`, `REAL`, `BYTE` and every existing device stay valid. Cost: M to L (about 14 tasks): `piperine-abi` (constant, layout, `validate`), `piperine-plan` (carrier layouts), `piperine-transpile` (`rt::flag` keeps a mask), `piperine-digital-solver` (the relaxation of D1 clears the levels that move, not the whole field), `piperine-api` (decoding records and table cells). Runtime cost: one more word per carried value and one mask operation per bitwise operator. | **C.** It gives the spec's semantics, which the user's resolved nets, `logic` patterns and enum `match`es rely on. It keeps every existing device and layout valid. It also makes D1's relaxation exact: §8.2.3 step 4 says "the levels that move are unknown", which a per-value byte can only round to the whole field. A is cheap but makes the language's own examples (`logic("10xx")`, `Logic`'s level-wise `merge`) say something other than the text. |
| Q2 | How does the host print a value with some levels known and some not? (Only under B or C.) | **a.** As today, the whole cell is unknown. Exact levels are only in `same`, `level` and `matches_bits` inside the model. **b.** A binary pattern with `x` for each unknown level, `0b10xx`, in tables, records and JSON Lines; a fully known value prints as today, in decimal. **c.** Like b in records and JSON Lines, and the whole cell unknown in tables and plots. | **b.** It is the Verilog convention the users of `x` know, and it is the only form that shows what the model computed. A plot draws a partly known value as unknown, as it draws unknown today. |

---

## Problem Statement

§3.3.2 says `&`, `|`, `^` and `!` work per level, and shifts, `cast` and field access move levels
with their flags. The compiled backend keeps one flag for the whole value. So a partial mask loses
known levels: with `x` unknown, `((x & 1) & 2).known_or(9)` gives 9 and the spec gives 0 (S6).
There is no diagnostic: the result differs from the spec text. `logic("10xx")`, a level-wise
`merge` of two integers and an enum `match` on a partly known tag cannot be expressed. The ABI is
why: a pin's field has one known byte per element. G6 records this as a provisional ruling.

## Goals

- [ ] The user decides Q1, with each option's cost in front of them.
- [ ] Under B or C: every rule of §3.3 that moves or combines levels gives the spec's levels on the compiled backend. The probe of S6 gives 0.
- [ ] Under A: the spec states the rounding, and no result differs from the spec text without the text saying so.
- [ ] The decision and its trade-off are recorded in `.specs/STATE.md` as an MD entry.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The merge of unknown control, `Unknown`, `level`, `merge`/`same`/`cast`/`logic`/`matches_bits` over whole values | `unknown-semantics` |
| Events, `settle` | `unknown-semantics-events` |
| Per-level flags on the analog side | §3.3.5: `analog` has no unknowns |
| Per-level flags of a type without `Repr` | §3.3.1: one flag for the whole value |
| A four-state value type (`0`, `1`, `x`, `z`) | Not in the spec. High impedance is `Logic::Z`, a value of `resolved-nets` |
| The interpreter | Frozen at the M1 analog subset (codegen context R1) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Q1: how levels are tracked | Pending (recommended C) | The user's ruling of 2026-11-15 stands until they answer | n |
| Q2: how a partly known value prints | Pending (recommended b) | Host behaviour a user sees | n |
| Which story set ships | UL-01 to UL-07 under B or C; UL-08 under A. The stories of the option not chosen are deleted when the spec is approved | One spec, two possible outcomes, each testable | n - planner |
| Arithmetic and comparisons | All-or-nothing under every option (§3.3.2, §3.3.10) | The spec says so; per-level flags change only the operators that move levels | n - planner |
| Width of the mask under C | One bit per level of the leaf, so a mask the size of the value bytes, at the offset `known` names | Same alignment and size rules as the value; P3 (all zero is no value) still holds: mask 0 is every level unknown | n - planner |

**Open questions:** Q1 and Q2, at the top of this file.

**Implicit-requirement dimensions.** Input validation: UL-05 AC 3 (a malformed mask field fails bind validation). Failure states: UL-05 AC 4 (a relaxation that does not settle). Idempotency and retry: N/A because a run is deterministic (§15 I1). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: N/A because flags travel with values and change no ordering rule. Data lifecycle: N/A because nothing is stored. Observability: UL-06 (printing). External dependencies: UL-05 AC 2 (devices built against today's ABI keep working, under C). State transitions: UL-07 AC 2 (a level that goes between known and unknown is a change).

---

## User Stories

### P1: Bitwise operators and shifts keep known levels ⭐ MVP

**User Story**: As a model author, I want masks and shifts on a partly unknown integer to keep the levels they fix, so that a decoder over an unreset register gives the known bits the spec promises.

**Why P1**: S6 is a silent difference from the spec.

**Acceptance Criteria** (UL-01, under Q1 = B or C):

1. WHEN `x: u9` is unknown THEN `((x & 1) & 2).known_or(9)` SHALL be 0 (the probe of S6).
2. WHEN `&` is applied THEN the system SHALL make level `k` of the result known false when level `k` of either operand is known false, known `a & b` when both are known, and unknown otherwise.
3. WHEN `|` is applied THEN the system SHALL make level `k` known true when level `k` of either operand is known true, known `a | b` when both are known, and unknown otherwise.
4. WHEN `^` or `!` is applied THEN the system SHALL make level `k` known exactly when level `k` of every operand is known.
5. WHEN a value is shifted by a known amount THEN the system SHALL move each level with its flag and fill with known false, or for a signed `>>`, with the sign level and its flag (§3.3.11).
6. WHEN a value is shifted by an unknown amount, or an arithmetic operator, a comparison or `as` meets any unknown level THEN the system SHALL give a result with every level unknown (§3.3.2, §3.3.10).

**Independent Test**: A compiled run over a `u8` built from `merge` of two known values, printing `level(k)` for each `k` after each operator.

---

### P1: `merge` and `level` of integers, level by level ⭐ MVP

**User Story**: As a model author, I want `merge` of two integers to keep the levels they agree on, so that `merge` is §3.3.3 and not a coarser rule.

**Why P1**: The merge of unknown control is level by level; with whole-value flags every merged integer is lost.

**Acceptance Criteria** (UL-02, under Q1 = B or C):

1. WHEN `merge(0b1010u4, 0b1011u4)` is evaluated THEN levels 3, 2 and 1 SHALL be known with the values 1, 0, 1, and level 0 SHALL be unknown.
2. WHEN an `if` on an unknown condition commits `0b1010u4` in one arm and `0b1011u4` in the other THEN the system SHALL commit the value of AC 1.
3. WHEN `x.level(k)` reads a partly known value THEN the system SHALL give `Some(b)` for each known level and `None` for each unknown one.
4. WHEN `x.known()` or `x.known_or(d)` reads a partly known value THEN the system SHALL treat it as unknown (§22.1.1 rule 1: all-or-nothing).

**Independent Test**: The table of AC 1 checked with `same` against `logic("101x")` and printed with `level`.

---

### P1: Patterns with `x` ⭐ MVP

**User Story**: As a model author, I want `logic("10xx")` and `matches_bits` with `x` on wide values, as in a Verilog literal.

**Why P1**: §22.1.4's own example.

**Acceptance Criteria** (UL-03, under Q1 = B or C):

1. WHEN `logic::<u4>("10xx")` is evaluated THEN levels 3 and 2 SHALL be known 1 and 0, and levels 1 and 0 SHALL be unknown.
2. WHEN `matches_bits(s, "1x0x")` is evaluated on an `s` whose level 3 is known 1, level 1 known 0 and levels 2 and 0 unknown THEN the system SHALL give known `true`.
3. WHEN `matches_bits(s, "1100")` is evaluated on the same `s` THEN the system SHALL give known `false`.

**Independent Test**: Compiled runs of the two tables of §22.1.4 over a `u4`.

---

### P1: `cast`, enums and `match` follow the levels ⭐ MVP

**User Story**: As a model author, I want `cast` to keep each level's flag and a `match` on a partly known enum to take only the arms its known levels allow.

**Why P1**: §3.3.2 (`cast` moves flags) and §3.3.3 ("every path the known levels do not exclude").

**Acceptance Criteria** (UL-04, under Q1 = B or C):

1. WHEN `cast::<u8, i8>(x)` is evaluated on a partly known `x` THEN level `k` of the result SHALL be level `k` of `x` with its flag.
2. WHEN a `match` on a unit enum of four variants (tag levels 1 and 0) has level 1 known false and level 0 unknown THEN the system SHALL take the arms of the two variants with tag 0 and 1 only, and merge their effects.
3. WHEN `B::decode` of a partly known tag cannot decide a variant THEN the system SHALL give the level-wise merge over the variants the known levels allow.

**Independent Test**: A compiled run with a `Bit<Mode>` built by `merge` of two variants, a `match` whose arms each `emit!`, and a check that only two arms' records appear, each with `maybe = true`.

---

### P1: The ABI carries one flag per level ⭐ MVP

**User Story**: As the owner of the device ABI, I want per-level flags to be a property of a layout that a device declares, so that the solver moves them as bytes and the language decides what they mean.

**Why P1**: Every other story needs the flags to cross the ABI (abi.md §3, P3).

**Acceptance Criteria** (UL-05, under Q1 = C; under B, AC 2 is replaced by "the export is `piperine_abi_2` and every shipped layout is rewritten"):

1. WHERE a field's flags hold `KNOWN_LEVELS` the system SHALL read its `known` offset as one mask per element, the size of the leaf, bit `k` set when level `k` is known.
2. WHEN a device built against the current ABI, with one known byte per element, is bound THEN the system SHALL bind and run it unchanged.
3. IF a field holds `KNOWN_LEVELS` without `HAS_KNOWN`, or its mask does not fit the layout THEN the system SHALL refuse the bind with a validation rule naming the field.
4. WHEN a relaxation of resolved drives (abi.md §9 D1) ends with levels still moving in a `KNOWN_LEVELS` field THEN the system SHALL clear the mask bits and the value bits of exactly those levels, and write one record to the resolver's log pin (§8.2.3 step 4).
5. The system SHALL keep P3: a field whose bytes are all zero holds no value.

**Independent Test**: `piperine-abi` validation tests over the new flag; a `piperine-digital-solver` test of D1 with a two-driver loop whose top level keeps moving.

---

### P2: The host shows partly known values

**User Story**: As a user of `piperine run` and `piperine-api`, I want a partly known value printed so that I can see which levels are known.

**Why P2**: The values are right without it; this is what the user sees of them.

**Acceptance Criteria** (UL-06, under Q1 = B or C, written for Q2 = b):

1. WHEN a table cell, a record field or a JSON Lines field holds a value with some levels unknown THEN the system SHALL print it as `0b` followed by one character per level, highest first, `x` for each unknown level.
2. WHEN every level is known THEN the system SHALL print the value as before this feature.
3. WHEN every level is unknown THEN the system SHALL print the value as unknown, as before this feature.
4. WHEN a plot draws a partly known value THEN the system SHALL draw it as it draws an unknown one.

**Independent Test**: `piperine-api` record and table tests, and a `piperine-cli` golden of one run.

---

### P2: Changes are level changes

**User Story**: As a model author, I want `changed()` to see a level that becomes known or unknown, as §3.3 says.

**Why P2**: Follows from the flags; needed for monitors.

**Acceptance Criteria** (UL-07, under Q1 = B or C):

1. WHEN a commit changes only the flag of one level of a carrier THEN the system SHALL fire its `changed()` (§3.3, "changes").
2. WHEN a commit leaves every level and flag as it was THEN the system SHALL NOT fire `changed()`.

**Independent Test**: A compiled run committing `merge(0b1010, 0b1011)` and then `0b1010`.

---

### P1: The spec admits one flag per value (only under Q1 = A) ⭐ MVP

**User Story**: As a reader of the spec, I want the rounding to one flag per value stated as a rule, so that no result differs from the text without the text saying so.

**Why P1**: Under A, this is the whole feature.

**Acceptance Criteria** (UL-08, under Q1 = A):

1. WHEN this feature merges THEN §3.3 SHALL state that an implementation MAY track one flag per value, rounding a partly known value up to all unknown, and SHALL name the operators and functions whose results then differ (`&`, `|`, `^`, `!`, shifts, `cast`, `merge`, `logic`, `level`, `match`), with a `spec_text` guard (MD-57).
2. WHEN this feature merges THEN Appendix C.2 SHALL state that this implementation tracks one flag per value, and Appendix G SHALL record G6 as decided.
3. WHEN `logic::<u4>("10xx")` is evaluated THEN the system SHALL give a value with every level unknown.

**Independent Test**: `cargo test -p piperine-lang --test spec_text`, and the probe of S6 checked to give 9 by a test that names the rule.

---

## Edge Cases

- WHEN a `bool` carrier is involved THEN the system SHALL give the same results under every option, since a `bool` has one level.
- WHEN an integer of 64 levels is fully known THEN the system SHALL give the same results as before this feature.
- IF a value passes through an `extern fn` THEN the system SHALL treat it all-or-nothing (§3.3.2).
- WHEN a partly known value reaches `analog` through `known_or(d)` THEN the system SHALL give `d` (all-or-nothing, §22.1.1).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| UL-01 | P1: Bitwise operators and shifts keep known levels | - | Pending |
| UL-02 | P1: `merge` and `level` of integers | - | Pending |
| UL-03 | P1: Patterns with `x` | - | Pending |
| UL-04 | P1: `cast`, enums and `match` | - | Pending |
| UL-05 | P1: The ABI carries one flag per level | - | Pending |
| UL-06 | P2: The host shows partly known values | - | Pending |
| UL-07 | P2: Changes are level changes | - | Pending |
| UL-08 | P1: The spec admits one flag per value (A only) | - | Pending |

**Coverage:** 8 total, 0 mapped to tasks, 8 unmapped (no tasks yet).

### Gaps closed

| GAPS id | What it says | Closed by |
| ------- | ------------ | --------- |
| S6 | unknown integers are one flag per value, so `&` and `|` lose known bits; no diagnostic | UL-01, UL-02, UL-04, UL-07 under B or C; UL-08 under A (the difference becomes a stated rule) |
| G6 | the spec hole: per value, ruled provisional on 2026-11-15 | Q1 answered and recorded as an MD entry; then UL-05 (B or C) or UL-08 (A) |

## Dependencies

- `unknown-semantics`: `merge`, `logic`, `matches_bits`, `level` and the merge of unknown control (US-02, US-03, US-04). This spec refines their results on wide values.
- `resolved-nets` benefits from UL-05 AC 4 but does not wait for it.
- `.specs/features/device-abi/abi.md` §3 (`Field`), P3, §9 D1: amended under B or C.

## Size

- Q1 = A: **S**, about 4 tasks.
- Q1 = C: **M to L**, about 14 tasks: ABI 3, plan 1, transpile runtime 4, digital solver 1, host 2, documents 1, tests 2.
- Q1 = B: **L**, about 16 tasks, plus a rebuild of every shipped test device.

## Sources

- `GAPS.md` §3 S6, §5 G6.
- `.specs/features/ir-rust/spec.md` row "One known flag per scalar" and `verification.md` F-B4 (the orchestrator's ruling of 2026-11-15; "the user may revisit").
- `docs/spec/part_i_values.md` §3.3.1 to §3.3.4, §3.3.10, §3.3.11, §3.3.13; `part_vii_stdlib.md` §22.1.2 to §22.1.4; `part_iii_structure.md` §8.2.3.
- `.specs/features/device-abi/abi.md` §3 (`Field`, `HAS_KNOWN`), P3, §9 D1, §14 item 6, the amendment row 6.
- `crates/piperine-transpile/src/rt/flag.rs` (one flag per value today).

---

## Success Criteria

- [ ] Q1 and Q2 are answered and recorded.
- [ ] Under B or C, the probe of S6 gives 0 and the tables of §22.1.4 hold on a `u4`. Under A, the spec says what the implementation does.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.
