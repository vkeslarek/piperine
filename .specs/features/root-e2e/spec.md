# Root E2E Specification

**Approved**: 2026-11-17 - user: "Aceito todas (Recommended)" (Q1-Q3)

Planned after `host-ii` (spec only; design and tasks follow its approval). It is part 2 of the user's lane-B scope of 2026-11-17, "Host II mais o wiring do root", split out because the two together exceed 30 tasks.

## Questions answered (user, 2026-11-17)

Answered "Aceito todas (Recommended)".

| # | Question | Decided |
| - | -------- | ------- |
| Q1 | What a behavioral claim of the Book is | (a) every `msl` block presented as a model to run, every listing, and every sentence that states a value or an outcome the binary produces (a record, a number, a refusal) |
| Q2 | Where the integrated tests live | (a) the root package, `tests/book/`, one target per Book chapter, through `piperine::` and the binary |
| Q3 | How `tests/ngspice/` moves | (a) one move to `tests/e2e/<scenario>/`, every reader updated in the same task, no symlink |

-------- | ------- | -------------- |
| Q1 | What is a "behavioral claim" of the Book that needs its own integrated test? | (a) every `msl` block that the Book presents as a model to run, every listing (already held by `piperine-cli/tests/book.rs`) and every sentence that states a value or an outcome the binary can produce (a record, a number, a refusal); (b) only blocks and listings; (c) (a) plus every prose sentence about semantics | **(a)**: a stated outcome is falsifiable through the entry point; (c) would test the spec, which `docs/spec/` guards own |
| Q2 | Where do the integrated tests live? | (a) the root package, `tests/book/*.rs`, one target per Book chapter, through `piperine::` and the binary; (b) `piperine-cli/tests/book_*.rs` beside `book.rs` | **(a)**: the user's words ("Root package `piperine` re-exports the new `piperine-api` and holds the integrated tests"); MD-28 puts the host surface at the root |
| Q3 | The move of `tests/ngspice/` under `tests/e2e/`: one move, or the old path kept as a link for a release? | (a) one move: `tests/e2e/<scenario>/` with `<name>.msl`, `<name>.cir` and its expected output; every reader (`piperine-parity`, `piperine-cli/tests/twins.rs`, the guards) updated in the same task; (b) a symlink kept | **(a)**: one home, as ROADMAP 4b says; a link would leave two paths for guards to read |

---

## Problem Statement

The Book (`docs/book/content.html`) has 97 `msl` blocks and 29 command-line listings. The listings are held to the binary (`crates/piperine-cli/tests/book.rs`); no block is run as a model unless a listing runs it, and the blocks the Book shows refused are not tested as refused (`GAPS.md` U5). The root package `piperine` re-exports `piperine-api` and holds no integrated test. The ngspice twins sit in `tests/ngspice/` and are compared by `piperine-parity`'s targets, one home for fixtures and another for scenarios (`GAPS.md` U10, H11).

## Goals

- [ ] Every Book example and behavioral claim has at least one integrated test at the root, run through `piperine-api` or the binary.
- [ ] A Book example that cannot run yet is asserted as refused with its diagnostic, and listed in `GAPS.md`.
- [ ] One root `tests/e2e/` holds every scenario with its ngspice twin, compared through the single `ToleranceContract`; `piperine-parity` stays the harness.
- [ ] The 7 twins (and the twins `host-ii` adds) print the same bytes after the move.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| New language or host features to make a Book example run | This feature tests what exists; a refused example is a `GAPS.md` entry |
| New ngspice twins beyond those that exist when it starts | The move and the scenario layout only; new twins come with the feature that needs them |
| Python and the language server | User: out of lane B |
| Changing a tolerance | MD-38; parity is top priority |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Q1 What a claim is | option (a) | recommended option | y - user: "Aceito todas (Recommended)" |
| Q2 Where tests live | root `tests/book/` | recommended option | y - user: "Aceito todas (Recommended)" |
| Q3 The move | one move | recommended option | y - user: "Aceito todas (Recommended)" |
| Inventory | a table generated from the Book (block id, chapter, kind: run, refused, fragment) is checked in and a guard fails when a block has no row or a row no test | the guard pattern of `parity_corpus` and `suite_hygiene` (MD-31) | n - planner |
| Fragments | a block that is not a whole model (a single expression, a signature) is tested inside the smallest model the Book builds around it, or marked `fragment` with the block that contains it | a fragment cannot run alone | n - planner |
| Starts after `host-ii` | yes | the Book changes in `host-ii` (new keys, analyses, `Blinky`) | n - planner |

**Open questions:** none - all resolved or logged above. Q1 to Q3 were answered on 2026-11-17 (top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: N/A because no input surface is added. Failure states: RE-02 (a refused example asserted with its diagnostic). Idempotency: N/A because tests only read. Auth and rate limits: N/A, local tool. Concurrency: RE-05 AC 3 (each scenario in its own directory, no shared output). Data lifecycle: RE-04 (the move). Observability: RE-01 AC 3 (the inventory guard names the uncovered block). External-dependency failure: RE-05 AC 2 (ngspice absent: the harness's skip line). State-transition integrity: N/A because tests hold no state.

---

## User Stories

### P1: Every Book example runs or is refused, by test ⭐ MVP

**User Story**: As a reader of the Book, I want every example I read to be run by a test through the tool I use, so that the Book cannot drift from the binary.

**Why P1**: the user's rule: "tudo que está no livro tenha ao menos 1 teste integrado".

**Acceptance Criteria** (RE-01, the inventory):

1. The system SHALL hold an inventory with one row per `msl` block of the Book: its chapter, its index in the chapter and its kind (`run`, `refused`, `fragment`).
2. The system SHALL name, for each row, the root test that covers it.
3. IF a block of the Book has no row, or a row names no existing test THEN the inventory guard SHALL fail naming the block.

**Acceptance Criteria** (RE-02, runs and refusals):

1. WHEN a test covers a block of kind `run` THEN it SHALL run the block's model through `piperine-api` or the binary and assert the values or records the Book states about it.
2. WHEN a test covers a block of kind `refused` THEN it SHALL assert the diagnostic code and text the binary gives, and `GAPS.md` SHALL list the block under the gap that refuses it.
3. WHEN a refused block starts to run THEN its test SHALL fail, so that the row moves to `run` with the Book.

**Independent Test**: `cargo test --test book_inventory` at the root.

---

### P1: Behavioral claims ⭐ MVP

**User Story**: As a reader, I want each value or outcome the Book states to be checked.

**Why P1**: a wrong sentence about a result misleads as much as a wrong listing.

**Acceptance Criteria** (RE-03):

1. WHERE a sentence of the Book states a value, a record, an order of records or a refusal that the binary produces (Q1 (a)), the system SHALL hold a root test asserting it, citing the chapter and the sentence.
2. The system SHALL keep the list of such sentences in the inventory with the test that asserts each.

**Independent Test**: `cargo test --test book_claims`.

---

### P1: One home for the ngspice scenarios ⭐ MVP

**User Story**: As the owner of the parity claim, I want every scenario and its ngspice twin in one root directory, run through the user's entry point.

**Why P1**: ROADMAP 4b; GAPS U10.

**Acceptance Criteria** (RE-04, the move):

1. The system SHALL hold each twin under `tests/e2e/<scenario>/` with its `.msl`, its `.cir` and its expected output, and no `.msl` or `.cir` under `tests/ngspice/`.
2. WHEN each twin runs from the command line after the move THEN the system SHALL print the bytes it printed before the move, on each backend.
3. The system SHALL pass every `parity_*` target with its comparisons and `ToleranceContract` unchanged, reading the new paths.

**Acceptance Criteria** (RE-05, scenarios through the entry point):

1. WHEN a scenario runs THEN the system SHALL run it through the binary or `piperine-api` as a user does and compare its output with ngspice through `ToleranceContract`.
2. WHERE ngspice is absent or disabled the system SHALL run the scenario and print the harness's skip line.
3. The system SHALL run each scenario in a directory of its own.

**Independent Test**: `cargo test -p piperine-cli --test twins` and the `parity_*` targets after the move.

---

## Edge Cases

- WHEN the Book shows one model across several blocks THEN the inventory SHALL tie the blocks to the one test that runs the assembled model.
- IF a Book listing's output changes THEN `book.rs` SHALL fail before any root test, as today.
- WHEN a scenario needs a bench THEN its directory SHALL hold a `Piperine.toml` and `benches/`.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| RE-01 | P1: Every Book example | Specify | Pending |
| RE-02 | P1: Every Book example | Specify | Pending |
| RE-03 | P1: Behavioral claims | Specify | Pending |
| RE-04 | P1: One home for the ngspice scenarios | Specify | Pending |
| RE-05 | P1: One home for the ngspice scenarios | Specify | Pending |

**Coverage:** 5 total, 0 mapped to tasks (spec only), 5 unmapped until Design and Tasks.

---

## Sources

- The user, 2026-11-16: "Finalizando o cli + api na feature, pode fazer o wiring do root e criar os testes integrados. Eu recomendo que tudo que está no livro tenha ao menos 1 teste integrado"; 2026-11-17: "Host II mais o wiring do root".
- `ROADMAP.md` queue items 4 and 4b; `GAPS.md` H11, U5, U10.
- `crates/piperine-cli/tests/book.rs`, `crates/piperine-parity/tests/parity_corpus.rs`, `.specs/STATE.md` MD-28, MD-31, MD-38.

---

## Success Criteria

- [ ] The inventory guard passes with every Book block covered.
- [ ] `tests/ngspice/` holds no twin; every twin prints its old bytes from `tests/e2e/`.
- [ ] `cargo test` passes at the root.
