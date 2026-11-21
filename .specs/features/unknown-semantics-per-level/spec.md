# Unknown Semantics: Per-Level Flags Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option: Q1 = C, Q2 = b)
**Amended**: 2026-11-21 - approved under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13". The amendment deletes the story of option A (UL-08), marks the enum part of UL-04 as waiting, adds UL-09 (aggregates, waiting for `composite-values`) and UL-10 (the elaborator's folding, the reference the compiled code is checked against), and records the defaults of `design.md` (D1 to D16) that a spec reader sees.

The second spec of the unknown-value area (see `unknown-semantics`). It closes S6 and G6: an
integer's unknown levels are tracked one by one, as §3.3.2 says. The user chose option C on
2026-11-18: per-level flags are an additive flag of the device ABI, `KNOWN_LEVELS`; every
existing layout and device stays valid.

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How are the unknown levels of an integer (and of any value wider than one level) tracked? Today a carried value has one known byte (abi.md §3, `Field.known`: one byte per element), so `((x & 1) & 2).known_or(9)` gives 9 where §3.3.2 gives 0. | **A. Keep one flag per value; amend the spec.** Cost S. Loss: `logic("10xx")` on a `u4` is all unknown, `merge(0b1010, 0b1011)` is all unknown, `x.level(k)` is `None` for every `k` of a partly known value, a `match` on a partly known enum takes every arm. **B. Per-level flags for every leaf, a new ABI version** (`piperine_abi_2`, every device rebuilds). Cost L. **C. Per-level flags as an additive ABI flag.** A new `Field.flags` bit, `KNOWN_LEVELS`, says that `known` is the offset of a mask the width of the leaf, per element. A field without it keeps one known byte. `BIT`, `REAL`, `BYTE` and every existing device stay valid. Cost M to L. | **C** (chosen). It gives the spec's semantics, keeps every existing device and layout valid, and makes D1's relaxation exact. |
| Q2 | How does the host print a value with some levels known and some not? | **a.** The whole cell unknown. **b.** A binary pattern with `x` for each unknown level, `0b10xx`, in tables, records and JSON Lines; a fully known value prints as today. **c.** Like b in records and JSON Lines, unknown in tables and plots. | **b** (chosen). A plot draws a partly known value as unknown, as it draws unknown today. |

---

## Problem Statement

§3.3.2 says `&`, `|`, `^` and `!` work per level, and shifts, `cast`, field access and indexing
move levels with their flags. The compiled backend keeps one flag for the whole value. So a
partial mask loses known levels: with `x` unknown, `((x & 1) & 2).known_or(9)` gives 9 and the
spec gives 0 (S6). A README probe shows the same on a multiplexer: with `sel` unknown and inputs
that differ in some bits, the output is all unknown where §3.3.3 keeps the levels on which the
inputs agree. There is no diagnostic: the result differs from the spec text. `logic("10xx")`, a
level-wise `merge` of two integers and an enum `match` on a partly known tag cannot be expressed.
The ABI is why: a pin's field has one known byte per element. G6 records this as a provisional
ruling.

## Goals

- [x] The user decides Q1, with each option's cost in front of them (2026-11-18: C).
- [ ] Every rule of §3.3 that moves or combines levels gives the spec's levels on the compiled backend, for `bool`, integers, `f64` and unit enums first, and for aggregates once `composite-values` has laid them out. The probe of S6 gives 0.
- [ ] The elaborator's constant folding gives the same levels as the compiled code on the same expressions.
- [ ] The decision and its trade-off are recorded in `.specs/STATE.md` as an MD entry, and GAPS S6 and G6 are closed.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The merge of unknown control, `Unknown`, `level`, `merge`/`same`/`cast`/`logic`/`matches_bits` over whole values | `unknown-semantics` (merged, MD-70) |
| Events, `settle` | `unknown-semantics-events` |
| Per-level flags on the analog side | §3.3.5: `analog` has no unknowns |
| Per-level flags of a type without `Repr` | §3.3.1: one flag for the whole value |
| A four-state value type (`0`, `1`, `x`, `z`) | Not in the spec. High impedance is `Logic::Z`, a value of `resolved-nets` |
| The interpreter | Frozen at the M1 analog subset (codegen context R1). It refuses each new IR form by name |
| Resolved nets (`Wire`, resolvers) | `resolved-nets` (ROADMAP item 12). UL-05 AC 4 makes D1's relaxation per level below them, tested on a hand-built shape |
| Aggregate types, their layouts and the value-semantics surface | `composite-values` and `value-semantics`. UL-09 gives their leaves per-level flags once they are on `main` |
| `piperine-lang-server` | Outside the default build, deferred by the user |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Q1: how levels are tracked | C: the additive ABI flag `KNOWN_LEVELS` | User, 2026-11-18 | y |
| Q2: how a partly known value prints | b: `0b` and one character per level, `x` where unknown | User, 2026-11-18 | y |
| Which story set ships | UL-01 to UL-07, UL-09, UL-10. UL-08 (option A) is deleted | The spec said the stories of the option not chosen go when it is approved | y - user's answer |
| Arithmetic and comparisons | All-or-nothing (§3.3.2, §3.3.10) | The spec says so; per-level flags change only the operators that move levels | y - spec |
| Width of the mask | One bit per level of the leaf, so a mask the size of the leaf's value bytes, at the offset `known` names, per element | Same alignment and size rules as the value; P3 still holds: mask 0 is every level unknown (design.md D1) | y - AFK default |
| Which leaves get `KNOWN_LEVELS` | Every carried leaf with more than one level: integers, unit-enum tags of two or more levels, `f64`. A `bool` leaf and a one-level tag keep the plain known byte, whose bytes are the same | §2 gives every level a flag, `Float<N>` included; a one-level mask is the known byte, so `BIT` carriers do not change (D2) | y - AFK default |
| ABI version | `MINOR` becomes 2. A library that declares a minor below 2 and sets `KNOWN_LEVELS` is refused by validation | The precedent of `commit_with` (MD-70: minor 1) (D3) | y - AFK default |
| Pins of one net with and without `KNOWN_LEVELS` | Unequal layouts (P4): a bind error naming both pins | P4 compares per field; a device built for the plain known byte cannot read a mask (D4) | y - AFK default |
| How a partly known value prints in each host format | Tables and CSV: `0b10xx`. Record lines: `name=0b10xx`. JSON Lines: the string `"0b10xx"`. A fully unknown value prints as today (`x`, `null`). A plot draws it as unknown | Q2 = b, carried to the two formats the question did not name (D5) | y - AFK default |
| A partly known `f64` | Printed as its 64-level pattern, highest level first | Q2 = b has no exception for reals; a partly known real only arises from `merge`, `cast` and `logic` (D6) | y - AFK default |
| The reference the compiled code is checked against | The elaborator's constant folding (`elab/consts.rs`), made per level (UL-10). The interpreter stays frozen | The interpreter runs no digital code; the folder already evaluates the same rules at elaboration (D7) | y - AFK default |
| Aggregates | UL-09 waits for `composite-values` phases A–B on `main`; scalar work goes first | `composite-values` T56/T57 lay out composite pins and T36 rewrites `Carried`; per-level flags of their leaves are built on that layout (D8) | y - AFK default |
| Enum `match` and `cast` to an enum on a partly known tag | UL-04 AC 2 and AC 3 wait with UL-09 | `composite-values` T48/T50 turn every enum into an IR enum and T22 rewrote pattern tests (D9) | y - AFK default |
| A partly known param value | Refused at elaboration as an unknown param is today | §3.3.1: params are known (D10) | y - spec |
| Test models | Written in the language both `main` and the value-semantics amendment accept: no `let mut`, no `&`, no `Clone`/`Copy` derive | The amendment (G.71) is on `main`; its implementation is on a branch (D11) | y - AFK default |
| Spec error codes | None. `crates/piperine-lang/tests/support/code_owners.tsv` assigns no code to this feature, and the feature adds none to `docs/spec/` | Checked 2026-11-21; a malformed mask is an ABI validation rule, not a language code | y - checked |

**Open questions:** none - Q1 and Q2 answered 2026-11-18; every default since is recorded above and in design.md "Decisions taken (user AFK)".

**Implicit-requirement dimensions.** Input validation: UL-05 AC 3 (a malformed mask field fails bind validation) and AC 6 (a library too old for the flag). Failure states: UL-05 AC 4 (a relaxation that does not settle). Idempotency and retry: N/A because a run is deterministic (§15 I1). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: N/A because flags travel with values and change no ordering rule. Data lifecycle: N/A because nothing is stored. Observability: UL-06 (printing). External dependencies: UL-05 AC 2 (devices built against the earlier ABI keep working). State transitions: UL-07 (a level that goes between known and unknown is a change).

---

## User Stories

### P1: Bitwise operators and shifts keep known levels ⭐ MVP

**User Story**: As a model author, I want masks and shifts on a partly unknown integer to keep the levels they fix, so that a decoder over an unreset register gives the known bits the spec promises.

**Why P1**: S6 is a silent difference from the spec.

**Acceptance Criteria** (UL-01):

1. WHEN `x: u9` is unknown THEN `((x & 1) & 2).known_or(9)` SHALL be 0 (the probe of S6).
2. WHEN `&` is applied THEN the system SHALL make level `k` of the result known false when level `k` of either operand is known false, known `a & b` when both are known, and unknown otherwise.
3. WHEN `|` is applied THEN the system SHALL make level `k` known true when level `k` of either operand is known true, known `a | b` when both are known, and unknown otherwise.
4. WHEN `^` or `!` is applied THEN the system SHALL make level `k` known exactly when level `k` of every operand is known.
5. WHEN a value is shifted by a known amount THEN the system SHALL move each level with its flag and fill with known false, or for a signed `>>`, with the sign level and its flag (§3.3.11).
6. WHEN a value is shifted by an unknown amount, or an arithmetic operator, a comparison or `as` meets any unknown level THEN the system SHALL give a result with every level unknown (§3.3.2, §3.3.10).

**Independent Test**: A compiled run over a `u8` built from `merge` of two known values, recording `level(k)` for each `k` after each operator.

---

### P1: `merge` and `level` of integers, level by level ⭐ MVP

**User Story**: As a model author, I want `merge` of two integers, and an `if` on an unknown condition, to keep the levels they agree on, so that a multiplexer with an unknown select is §3.3.3 and not a coarser rule.

**Why P1**: The merge of unknown control is level by level; with whole-value flags every merged integer is lost.

**Acceptance Criteria** (UL-02):

1. WHEN `merge(0b1010u4, 0b1011u4)` is evaluated THEN levels 3, 2 and 1 SHALL be known with the values 1, 0, 1, and level 0 SHALL be unknown.
2. WHEN an `if` on an unknown condition commits `0b1010u4` in one arm and `0b1011u4` in the other THEN the system SHALL commit the value of AC 1.
3. WHEN a multiplexer `if sel { a } else { b }` has `sel` unknown, `a = 0b1100_0101u8` and `b = 0b1100_1001u8` THEN the system SHALL give levels 7, 6, 5, 4, 1 and 0 known with the values 1, 1, 0, 0, 0, 1, and levels 3 and 2 unknown.
4. WHEN `x.level(k)` reads a partly known value THEN the system SHALL give `Some(b)` for each known level and `None` for each unknown one.
5. WHEN `x.known()`, `x.known_or(d)` or `x.is_known()` reads a partly known value THEN the system SHALL treat it as unknown (§22.1.1 rule 1: all-or-nothing).
6. WHEN `same(a, b)` compares two partly known values THEN the system SHALL give known `true` exactly when every level has the same flag and, where known, the same value.

**Independent Test**: The table of AC 1 checked with `same` against `logic("101x")` and recorded with `level`; the multiplexer of AC 3 run from source.

---

### P1: Patterns with `x` ⭐ MVP

**User Story**: As a model author, I want `logic("10xx")` and `matches_bits` with `x` on wide values, as in a Verilog literal.

**Why P1**: §22.1.4's own example.

**Acceptance Criteria** (UL-03):

1. WHEN `logic::<u4>("10xx")` is evaluated THEN levels 3 and 2 SHALL be known 1 and 0, and levels 1 and 0 SHALL be unknown.
2. WHEN `matches_bits(s, "1x0x")` is evaluated on an `s` whose level 3 is known 1, level 1 known 0 and levels 2 and 0 unknown THEN the system SHALL give known `true`.
3. WHEN `matches_bits(s, "1100")` is evaluated on the same `s` THEN the system SHALL give known `false`.
4. WHEN a carrier is declared `signal bus: Bit<u4> = logic("10xx");` THEN the system SHALL start it with levels 3 and 2 known 1 and 0 and levels 1 and 0 unknown.

**Independent Test**: Compiled runs of the two tables of §22.1.4 over a `u4`, and the initial value of `bus` read with `level`.

---

### P1: `cast` keeps each level's flag ⭐ MVP

**User Story**: As a model author, I want `cast` to keep each level's flag, and a `match` on a partly known enum to take only the arms its known levels allow.

**Why P1**: §3.3.2 (`cast` moves flags) and §3.3.3 ("every path the known levels do not exclude").

**Acceptance Criteria** (UL-04):

1. WHEN `cast::<u8, i8>(x)` is evaluated on a partly known `x` THEN level `k` of the result SHALL be level `k` of `x` with its flag.
2. WHEN a `match` on a unit enum of four variants (tag levels 1 and 0) has level 1 known false and level 0 unknown THEN the system SHALL take the arms of the two variants with tags 0 and 1 only, and merge their effects. (Waits with UL-09, D9.)
3. WHEN `cast::<u2, E>(x)` meets a partly known `x` THEN the system SHALL give the levels of `x` with their flags when every tag the known levels allow names a variant of `E`, and a value with every level unknown when one of them names no variant (§22.1.3 rule 2 under §3.3.3). (Waits with UL-09, D9.)

**Independent Test**: A compiled run of AC 1 over every level of a `u8`; for AC 2 a `match` whose arms each `emit!`, and a check that only two arms' records appear, each with `maybe = true`.

---

### P1: The ABI carries one flag per level ⭐ MVP

**User Story**: As the owner of the device ABI, I want per-level flags to be a property of a layout that a device declares, so that the solver moves them as bytes and the language decides what they mean.

**Why P1**: Every other story needs the flags to cross the ABI (abi.md §3, P3).

**Acceptance Criteria** (UL-05):

1. WHERE a field's flags hold `KNOWN_LEVELS` the system SHALL read its `known` offset as one mask per element, the size of the leaf, bit `k` set when level `k` is known.
2. WHEN a device built against minor 1 of the ABI, with one known byte per element, is bound THEN the system SHALL bind and run it unchanged.
3. IF a field holds `KNOWN_LEVELS` without `HAS_KNOWN`, or its mask does not fit the layout THEN the system SHALL refuse the bind with a validation rule naming the field.
4. WHEN a relaxation of resolved drives (abi.md §9 D1) ends with levels still moving in a `KNOWN_LEVELS` field THEN the system SHALL clear the mask bits and the value bits of exactly those levels, and write one record to the resolver's log pin (§8.2.3 step 4).
5. The system SHALL keep P3 in every `KNOWN_LEVELS` field: all-zero bytes hold no value, and the value bit of every unknown level is zero.
6. IF a library declaring a minor below 2 sets `KNOWN_LEVELS` on a field THEN the system SHALL refuse the bind with a validation rule naming the field.
7. WHEN a `MERGE` commit (abi.md C7) lands on a `KNOWN_LEVELS` field THEN the system SHALL keep each level that both values hold known and equal, and clear the others.

**Independent Test**: `piperine-abi` validation and merge tests over the new flag; a `piperine-digital-solver` test of D1 with a two-driver loop whose top level keeps moving.

---

### P2: The host shows partly known values

**User Story**: As a user of `piperine run` and `piperine-api`, I want a partly known value printed so that I can see which levels are known.

**Why P2**: The values are right without it; this is what the user sees of them.

**Acceptance Criteria** (UL-06):

1. WHEN a table cell, a CSV cell or a record field holds a value with some levels unknown THEN the system SHALL print it as `0b` followed by one character per level, highest first, `x` for each unknown level.
2. WHEN a JSON Lines field holds a value with some levels unknown THEN the system SHALL write the string of AC 1, quoted.
3. WHEN every level is known THEN the system SHALL print the value as before this feature.
4. WHEN every level is unknown THEN the system SHALL print the value as unknown, as before this feature (`x` in text, `null` in JSON Lines).
5. WHEN a plot draws a partly known value THEN the system SHALL draw it as it draws an unknown one.

**Independent Test**: `piperine-api` record and table tests, and a `piperine-cli` e2e run of one model in every output format.

---

### P2: Changes are level changes

**User Story**: As a model author, I want `changed()` to see a level that becomes known or unknown, as §3.3 says.

**Why P2**: Follows from the flags; needed for monitors.

**Acceptance Criteria** (UL-07):

1. WHEN a commit changes only the flag of one level of a carrier THEN the system SHALL fire its `changed()` (§3.3, "changes").
2. WHEN a commit leaves every level and flag as it was THEN the system SHALL NOT fire `changed()`.

**Independent Test**: A compiled run committing `merge(0b1010, 0b1011)` and then `0b1010`.

---

### P1: Aggregates keep their levels (waits for `composite-values` A–B on `main`)

**User Story**: As a model author, I want a struct, a tuple, an array or an `Option` to keep each leaf's levels, so that field access, indexing and the merge of unknown control give the spec's levels on composite values too.

**Why P1**: §3.3.2 names field access and indexing among the operations that move levels; §3.3.3 merges every effect level by level, a composite one included.

**Acceptance Criteria** (UL-09):

1. WHEN a carrier of a derived struct, a tuple, an array or an `Option` is laid out THEN the system SHALL give each of its leaves of more than one level a `KNOWN_LEVELS` mask.
2. WHEN `merge` combines two structs that differ only in level 0 of one integer field THEN the system SHALL keep every other level of every field known.
3. WHEN a field is read, or an array is indexed by a known index, on a partly known composite THEN the system SHALL give the levels of that leaf with their flags.
4. WHEN an array is indexed by an unknown index THEN the system SHALL give the level-wise merge of the elements the known levels of the index allow (§3.3.13).
5. WHEN `cast` moves the levels of a composite to a type of the same width THEN level `k` of the result SHALL be level `k` of the source with its flag.

**Independent Test**: A compiled run over a carried `struct { a: u4, b: u4 }` built by `merge` of two values, recording `level(k)` of each field.

---

### P1: The elaborator folds per level ⭐ MVP

**User Story**: As the maintainer of the compiler, I want the elaborator's constant folding to give the same levels as the compiled code, so that a `const` and a run agree and the folder serves as the reference the compiled operators are checked against.

**Why P1**: `const B: u4 = logic("10xx");` is folded at elaboration; today the folder rounds it to all unknown (`elab/consts.rs`, `Folded`).

**Acceptance Criteria** (UL-10):

1. WHEN a `const` of an integer type folds `logic::<u4>("10xx")` THEN the system SHALL hold levels 3 and 2 known 1 and 0 and levels 1 and 0 unknown.
2. WHEN a constant expression applies `&`, `|`, `^`, `!`, a shift by a known amount, `merge`, `cast` or an `if` on an unknown condition THEN the system SHALL fold it by the rules of UL-01, UL-02 and UL-04 AC 1.
3. WHEN the same table of expressions over partly known `u8` values is folded at elaboration and run as compiled code THEN the system SHALL give equal levels and flags for every row.
4. IF a param would fold to a partly known value THEN the system SHALL refuse it as it refuses an unknown param today (§3.3.1).

**Independent Test**: A `piperine-lang` folding test over the S6 table, and a `piperine-parity` source test that records the same table from `const` items and from carriers and compares them.

---

## Edge Cases

- WHEN a `bool` carrier is involved THEN the system SHALL give the same results and the same pin bytes as before this feature, since a `bool` has one level.
- WHEN an integer of 64 levels is fully known THEN the system SHALL give the same results as before this feature, with every bit of its mask set.
- IF a value passes through an `extern fn` THEN the system SHALL treat it all-or-nothing (§3.3.2).
- WHEN a partly known value reaches `analog` through `known_or(d)` THEN the system SHALL give `d` (all-or-nothing, §22.1.1).
- WHEN an `i8` with level 7 unknown is shifted right by a known amount THEN the system SHALL make every vacated level unknown, as the sign level is (§3.3.11).
- WHEN a value is shifted by its width or more THEN the system SHALL give every level known false, or for a signed `>>`, every level the sign level with its flag (G5's provisional rule, carried per level).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| UL-01 | P1: Bitwise operators and shifts keep known levels | Tasks | In Tasks |
| UL-02 | P1: `merge` and `level` of integers | Tasks | In Tasks |
| UL-03 | P1: Patterns with `x` | Tasks | In Tasks |
| UL-04 | P1: `cast` keeps each level's flag (AC 2, 3 wait) | Tasks | In Tasks |
| UL-05 | P1: The ABI carries one flag per level | Tasks | In Tasks |
| UL-06 | P2: The host shows partly known values | Tasks | In Tasks |
| UL-07 | P2: Changes are level changes | Tasks | In Tasks |
| UL-09 | P1: Aggregates keep their levels (waits) | Tasks | In Tasks |
| UL-10 | P1: The elaborator folds per level | Tasks | In Tasks |

UL-08 (option A, "the spec admits one flag per value") is deleted: the user chose C.

**Coverage:** 9 total, 9 mapped to tasks, 0 unmapped.

### Gaps closed

| GAPS id | What it says | Closed by |
| ------- | ------------ | --------- |
| S6 | unknown integers are one flag per value, so `&` and `|` lose known bits; no diagnostic | UL-01, UL-02, UL-03, UL-04, UL-07, UL-10 (scalars), UL-09 (aggregates) |
| G6 | the spec hole: per value, ruled provisional on 2026-11-15 | Q1 answered (C) and recorded as an MD entry; UL-05 |

## Dependencies

- `unknown-semantics` (merged, MD-70): `merge`, `logic`, `matches_bits`, `level` and the merge of unknown control. This spec refines their results on wide values.
- `composite-values` phases A–B: UL-09 and UL-04 AC 2, 3 start once they are on `main` (D8, D9). Every other story starts from `main` at `856d2668`.
- `value-semantics`: test models use the subset both languages accept (D11).
- `resolved-nets` benefits from UL-05 AC 4 but does not wait for it.
- `.specs/features/device-abi/abi.md` §3 (`Field`), P3, P4, §9 D1, C7 and the version table: amended by this feature.

## Size

**L**, 33 tasks: 24 that start now (ABI, solver, plan, runtime, IR, elaborator, host, documents) and 9 that wait for `composite-values` A–B.

## Sources

- `GAPS.md` §3 S6, §5 G6.
- `.specs/features/ir-rust/spec.md` row "One known flag per scalar" and `verification.md` F-B4 (the orchestrator's ruling of 2026-11-15; "the user may revisit").
- `docs/spec/part_i_values.md` §2 (`Repr`, rules 8 to 11), §3.3.1 to §3.3.4, §3.3.10, §3.3.11, §3.3.13; `part_vii_stdlib.md` §22.1.1 to §22.1.4; `part_iii_structure.md` §8.2.3.
- `.specs/features/device-abi/abi.md` §3 (`Field`, `HAS_KNOWN`), P3, P4, §9 D1, C7, §14 item 6, the amendment row 6.
- `crates/piperine-transpile/src/rt/flag.rs` (one flag per value today), `crates/piperine-lang/src/elab/consts.rs` (`Folded`).
- `.specs/features/composite-values/tasks.md` on `feature/composite-values` (T36, T56, T57: composite `Carried` and layouts).

---

## Success Criteria

- [x] Q1 and Q2 are answered and recorded.
- [ ] The probe of S6 gives 0, the multiplexer of UL-02 AC 3 keeps six levels, and the tables of §22.1.4 hold on a `u4`.
- [ ] Folding and compiled code agree on the S6 table (UL-10 AC 3).
- [ ] Every `buck_*` target and every ngspice parity target pass, with no tolerance or contract changed; the buck assertions that move are exactly those design.md lists.
- [ ] `cargo build` has no warning and `cargo test` passes at the root.
