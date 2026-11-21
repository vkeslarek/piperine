# Unknown Semantics: Events Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)
**Amended**: 2026-11-20 - under the user's standing AFK rule ("always take the most reasonable default and tell later"): Out of Scope and UE-06 follow what `msl-fixes` (MD-67) and `host-ii` (MD-69) delivered; UE-02 AC 4 says what "committed" means (design.md D2); UE-05 AC 5 and UE-06 AC 4 add the ngspice parity the project puts first; UE-06 AC 1 counts the `clip` records as the Book does. Each change is a D-default of design.md.

The third spec of the unknown-value area (see `unknown-semantics`). It closes the event gaps of
§13 that remain: an event read as a `bool` (L51), `settle` (L52) and functions that return
`Event` (L53). It also delivers what they unblock: `limexp`, the Book's diode clipper and the
Book's chapter-8 buck, which are refused today only by `settle` and by the closure inside
`limexp`.

## Problem Statement

Three forms of §13 are refused with `P0001`. `if fref.rise() { .. }` inside a reaction, and a
guard that reads an event (`on d.changed() && en && !en.changed()`), are "an event read as a
bool". `on settle` is "built-in `settle`", so no model can sample its waveform once per time
point. The Book's clipper and its chapter-8 buck stop at that line. A function returning
`Event` (`fn handshake(req: Bit, ack: Bit) -> Event`) is refused at the method call on its
carrier argument. The stdlib's `limexp` is refused because the closure it passes to `limit` binds
names inside a module with a result, so the Book's diode cannot use it.

The solver already has the `SETTLE` global event pin and fires it once the microstep loop is
quiet (`piperine-digital-solver` `engine.rs`). The gap is in the type checker, the elaborator,
the IR and the generated Rust.

## Goals

- [ ] An event reads as a `bool` in a reaction body and in a guard, as §13 rules 2 and 9 say.
- [ ] `on settle` runs once per time point with the settle-commit rules S1 to S7 of §15.
- [ ] A function returning `Event` is accepted in every event position.
- [ ] `limexp` runs, and the Book's clipper and chapter-8 buck print what the Book says they print.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The merge of unknown control, `maybe`, `Unknown` | `unknown-semantics` |
| A function that takes a carrier or a node as a handle, in general (L8) | `generics`. UE-04 uses handles and waits for them |
| `emit!` in `analog` (L49, the Book's `newton` trace next to the clipper) | `analog-operators` |
| Closures outside the argument of `limit` (L9), `let mut` and assignment (L5) | `composite-values`. The Book's `Pfd3` needs `let mut`, so UE-01 AC 3 waits for its phase A |
| The stdlib monitors `Stable`, `Setup`, `Hold` | `generics` (L30, generic modules). They read events in guards; UE-02 proves the form on non-generic modules |
| Settle commits made in an analysis without time run at a transient's last instant | Open in the spec (G.34) |
| A `ttol` or `etol` given to `cross` (L54) | `msl-fixes` |
| The interpreter | Frozen at the M1 analog subset (codegen context R1) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| `settle` on the ABI | The global event pin `SETTLE` that the solver already fires, read by a reaction like any event pin | abi.md §4 lists `SETTLE` among the globals; `piperine-solver` binds it (`core/bind.rs`) | n - planner |
| Where `limexp` lives | In this feature, because the clipper needs it. The change is narrow: the closure argument of `limit` inside a module with a result. Closures elsewhere (L9) stay `composite-values`' | The orchestrator assigned `limexp` to this area; L9 already exempts the argument of `limit` | n - planner |
| A guard read with an unknown `bool` | Runs under the merge of `unknown-semantics` (US-04 AC 9) | §13 rule 1 | n - planner |
| `q <- up` in a reaction body, `up` an `Event` | Commits the `bool` §13 rule 2 reads, as the spec's own `Comparator` does; `E0441` stays for an `Event` where a type other than `bool` is expected, and outside reaction bodies, guards and event positions. `docs/spec` §13 gains a Resolution saying so | §13's example contradicts the literal "never committed" of rule 8; the Book (mixed#6) reads it as a `bool` | y - AFK default (design.md D2) |
| `sin` (L14), `on above(..) { emit!(..) }` (L55), a digital-only transient (L56) | Delivered on `main` (`msl-fixes`, `host-ii`); UE-06 AC 1 no longer waits on them | ROADMAP 3d, 3e | y - AFK default (design.md D1) |
| ngspice parity of what this feature unblocks | The `limexp` diode is compared with ngspice's diode at an existing twin's operating point (UE-05 AC 5); the Book's clipper gets a transient twin (UE-06 AC 4). No tolerance moves | the project's rule: parity first, tolerances never retuned | y - AFK default (design.md D11) |
| The clipper's and the buck's expected output | What the Book prints for them: the clipper's `clip` stream (two lines per period, `vin` near 0.72) and `wave` records, the buck's `regulated` check passing after 10 ms | The Book is the source; its listings are held to the binary (`piperine-cli/tests/book.rs`) | n - planner |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation: UE-02 AC 3 and 4 (`E0440`, `E0441`). Failure states: UE-03 AC 6 (a rejected step moves the settle commit). Idempotency and retry: N/A because a run is deterministic (§15 I1). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: UE-03 AC 2, 4 and 8 (order of the settle phase, its commits and its records). Data lifecycle: UE-03 AC 7 (a settle commit at the stop time is dropped). Observability: UE-03 AC 8 (records of the settle phase). External dependencies: N/A. State transitions: UE-03 AC 3 (settle commits are due at the next time point, never in the same one).

---

## User Stories

### P1: An event reads as a `bool` ⭐ MVP

**User Story**: As a model author, I want to ask inside a reaction which of its events fired, so that one reaction can handle two simultaneous edges, as the phase detector of §13 does.

**Why P1**: L51; `Pfd3`, the stdlib monitor `Stable` and every two-event reaction need it.

**Acceptance Criteria** (UE-01):

1. WHEN a reaction body reads an event as a `bool` (`if fref.rise() { .. }`) THEN the system SHALL give `true` exactly when that event fired in the reaction's microstep (§13 rule 2).
2. WHEN two events of one reaction's head fire in the same microstep THEN the system SHALL run the reaction once, with both read as `true` (§12.1.15, §13 rule 18).
3. WHEN the phase detector `Pfd3` of §13 runs with `fref` and `fvco` rising in the same microstep THEN `state` SHALL be unchanged after the reaction (one step up, then one down).
4. WHEN an event is read as a `bool` in the settle phase THEN the system SHALL give `true` only for an event that `settle` makes fire (§15, Resolution).

**Independent Test**: A compiled run of `Pfd3` from §13 with clocks that rise apart and together, printing `up` and `dn`.

---

### P1: Guards read events ⭐ MVP

**User Story**: As a model author, I want a guard to read an event as a `bool`, so that I can write "data changed while enable held still".

**Why P1**: §13 rule 9; the stdlib monitors `Stable`, `Setup` and `Hold` are written this way.

**Acceptance Criteria** (UE-02):

1. WHEN the head `on d.changed() && en && !en.changed()` is evaluated THEN the system SHALL fire it exactly in a microstep where `d` changed, `en` is true and `en` did not change.
2. WHEN a guard reads an event as a `bool` THEN the system SHALL evaluate it in the microstep of the head, reading what the reaction would read (§13 rule 1).
3. IF `!` is applied to an `Event` outside a guard or a reaction body, or `&&` has a `bool` on the left and an `Event` on the right in an event position THEN the system SHALL report `E0440`.
4. IF an `Event` value appears outside an event position, a guard or a reaction body, or where a type other than `bool` is expected (a commit onto a carrier whose type is not `bool`, a carried or stored value) THEN the system SHALL report `E0441`.
5. WHEN a reaction body commits an event read as a `bool` (`on up || dn { q <- up; }`, `q: Bit`) THEN the system SHALL commit whether `up` fired in that microstep (§13 rule 2, design.md D2).

**Independent Test**: A compiled run with `d` and `en` driven by timed commits through the four combinations, and elaboration tests for `E0440` and `E0441`.

---

### P1: `on settle` ⭐ MVP

**User Story**: As a model author, I want a reaction that runs once per time point after everything at that time has finished, so that I can sample a waveform and check a design, like an oscilloscope.

**Why P1**: L52. It is the language's way to print a waveform and to check at every time point; the Book's clipper and buck use it.

**Acceptance Criteria** (UE-03):

1. WHEN a transient reaches a time point and its microstep loop has gone quiet THEN the system SHALL run every reaction on `settle` once, reading the final discrete state and the solution at that time (§13 rule 17, §15 step 4).
2. WHEN an event time is not a time point THEN the system SHALL NOT run reactions on `settle` there (§15, T5).
3. WHEN a reaction on `settle` commits with no delay, or with a delay that does not advance time THEN the system SHALL make the commit due at microstep 0 of the next time point, where it fires `changed()` and is read by the step that arrives there (§15 S1, S2).
4. WHEN an event time lies strictly between the time point of a settle commit and the next time point THEN the system SHALL let it read the value from before the commit (§15 S4).
5. WHEN a reaction on `settle` commits with a delay that advances time THEN the system SHALL treat it as an ordinary delayed commit (§15 S7).
6. WHEN the step to the next time point is rejected THEN the system SHALL move the settle commit with the retried time point (§15 S5).
7. WHEN a settle commit is made at the stop time THEN the system SHALL drop it (§15 S6).
8. WHEN reactions on `settle` write records THEN the system SHALL order them after the records of every microstep of their time point, stamped with the time point's `t` and the index of its last microstep (§19 rules 8 and the Resolution).
9. WHEN an operating point, each point of a `Dc` sweep and the last instant of an analysis complete THEN the system SHALL run the reactions on `settle` there (§15, "What each kind of code reads"; S6).
10. WHEN `settle` is combined with another event by `||` or `&&` THEN the system SHALL fire the combination by §13 rule 1, and the phase the reaction ran in SHALL decide when its commits are due (§13 rule 14).

**Independent Test**: A compiled RC transient with `on settle { emit!(..) }`, compared with the run's accepted steps; a settle commit read by a `changed()` reaction at the next time point; a `dc` sweep with one record per point.

---

### P1: Functions that return `Event` ⭐ MVP

**User Story**: As a model author, I want to name a protocol event once as a function (`fn handshake(req: Bit, ack: Bit) -> Event`), so that every reaction that waits for it says so in one word.

**Why P1**: L53; §13 rule 7 makes every event other than a named `let` a function or a module.

**Acceptance Criteria** (UE-04):

1. WHEN a function returning `Event` is called in the head of `on`, in a body `let`, in the result of a module, in another such function's return value, or as an operand of `||` or `&&` in one of these THEN the system SHALL accept it and fire it whenever its body's event fires (§13, event positions).
2. WHEN `fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack.high() }` heads a reaction THEN the reaction SHALL run exactly in the microsteps where `req` rises with `ack` high.
3. WHEN such a function is read as a `bool` in a reaction body THEN the system SHALL give whether its event fired in that microstep (UE-01).
4. IF a function returning `Event` is called outside an event position, a guard or a reaction body THEN the system SHALL report `E0441`.

**Independent Test**: A compiled run with `handshake` in a head and in a body `let`, driven by timed commits.

---

### P1: `limexp` ⭐ MVP

**User Story**: As a device author, I want the stdlib's `limexp` in my diode, so that Newton's overshoot does not overflow the exponential.

**Why P1**: The Book's diode and clipper use it, and §22.2.4 makes it the standard cure.

**Acceptance Criteria** (UE-05):

1. WHEN a module calls `limexp(x)` in `analog` THEN the system SHALL elaborate the stdlib's definition, including the closure it passes to `limit`, and SHALL NOT report "binding inside a module".
2. WHEN a diode written with `limexp(v(a, k) / nvt)` and the same diode written with `(v(a, k) / nvt).exp()` and `pnjlim` limiting both converge THEN their operating points SHALL agree under `ToleranceContract::DC_VOLTAGE`.
3. WHILE Newton iterates, the system SHALL move the argument of each `limexp` instance by at most 1.0 per iteration (§22.2.4 rule 3).
4. The system SHALL give `limexp(x)` the derivative `limexp(x)` (§22.2.4 rule 5).
5. WHEN the operating point of the `diode_series` twin (`tests/e2e/twins/diode_series.cir`) is solved with its diode written with `limexp` THEN its node voltages SHALL agree with ngspice under `ToleranceContract::DC_VOLTAGE`.

**Independent Test**: An op of the Book's diode against the `exp` form, and a trace of the argument across iterations.

---

### P2: The Book's clipper and chapter-8 buck run

**User Story**: As a reader of the Book, I want its clipper and its chapter-8 buck to run as printed, so that the chapters on analog and mixed-signal behaviour show real output instead of a refusal.

**Why P2**: The language forms are P1 above; this is their proof on the Book's own sources. The clipper also waits for `msl-fixes` (L14, L55).

**Acceptance Criteria** (UE-06):

1. WHEN `piperine run clipper.msl --analysis transient --stop 2e-3 ...` runs with the Book's flags THEN the system SHALL print one `clip` record per period (two in the run) with `vin` within 50 mV of 0.72 V, and write one `wave` record per time point.
2. WHEN `piperine run buck.msl --analysis transient --stop 12e-3 ...` of chapter 8 runs with the `digital for Buck` check THEN the system SHALL complete with every `regulated` check record `pass = true`.
3. WHEN either run completes THEN the Book's listing SHALL be the binary's output, the refusal listings replaced, and `piperine-cli/tests/book.rs` SHALL pass.
4. WHEN the analog part of the Book's clipper (sine source, resistor, two `limexp` diodes) runs a 2 ms transient THEN `v(vout)` SHALL agree with its ngspice twin `tests/e2e/twins/tran_clipper.cir` under `ToleranceContract::TRAN_VOLTAGE`.

**Independent Test**: `cargo test -p piperine-cli --test book`.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md` and the Book, I want them to say that these forms run.

**Why P3**: No behaviour depends on it.

**Acceptance Criteria** (UE-07):

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L51, L52, L53, nor `limexp` in L58, and §1's "stdlib that runs" SHALL name `settle` and `limexp`.
2. WHEN this feature merges THEN the Book's sentences that say `settle` is not compiled SHALL be removed or rewritten to what runs.

**Independent Test**: `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN a time point has no reaction on `settle` THEN the system SHALL add no microstep and no record there.
- WHEN a settle reaction reads `x.changed()` THEN the system SHALL give `false` unless `settle` makes it fire (§15, Resolution).
- WHEN two time points are a jump's left and right values THEN the system SHALL run `settle` once per time point, reading the solution at that time point.
- WHEN a function returning `Event` calls another one THEN the system SHALL fire the outer when the inner fires under the outer's guard.
- IF a reaction on `settle` probes THEN the system SHALL read the solution at its time point and SHALL NOT make a new time point.
- WHEN a design has nothing analog THEN the system SHALL run `settle` at the operating point and at the stop time of a transient (§15, "What each kind of code reads"; L56 delivered by `host-ii`).
- WHEN a reaction body reads as a `bool` an event that is not in its head THEN the system SHALL give whether that event fired in the microstep (§13 rule 2).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| UE-01 | P1: An event reads as a `bool` | Tasks | In Tasks |
| UE-02 | P1: Guards read events | Tasks | In Tasks |
| UE-03 | P1: `on settle` | Tasks | In Tasks |
| UE-04 | P1: Functions that return `Event` | Tasks | In Tasks |
| UE-05 | P1: `limexp` | Tasks | In Tasks |
| UE-06 | P2: The Book's clipper and chapter-8 buck run | Tasks | In Tasks |
| UE-07 | P3: The documents say what is true | Tasks | In Tasks |

**Coverage:** 7 total, 7 mapped to tasks (tasks.md), 0 unmapped.

### Gaps closed

| GAPS id | What it says | Closed by |
| ------- | ------------ | --------- |
| L51 | an event read as a `bool`, in a reaction and in a guard | UE-01, UE-02 |
| L52 | `settle` | UE-03 |
| L53 | a function that returns `Event` | UE-04 (after `generics` delivers handles, L8) |
| L58, `limexp` only | `limexp` refused, "binding inside a module" | UE-05 |
| U5, the clipper and the chapter-8 buck rows | shown with their refusal | UE-06 (the rest of U5 is `root-e2e`'s) |
| Book rows (GAPS §11) | values#2, values~c389077a (L53); analog#3, analog~773a3851, mixed#10, mixed~9d07c4b9, cheatsheet#5 (L52); digital#6, mixed#6, mixed#15 (L51) | UE-01 to UE-06; a row another gap still refuses is re-pointed to that gap (design.md D15) |

## Dependencies

- `unknown-semantics`: US-04 AC 9 (a guard whose `bool` is unknown).
- `generics`: L8 handles (phase B), for UE-04 only.
- `composite-values`: `let mut` and assignment (phase A), for UE-01 AC 3 (`Pfd3`) only.
- `msl-fixes` and `host-ii`: delivered on `main` (L14, L55, L56).
- `piperine-digital-solver` already fires `SETTLE`; it refuses a settle commit today (`run.rs`, "settle commit"). This feature gives it the due instant of S1 to S6, and the owner protocol in `piperine-solver` lands it (design.md D6).

## Size

**M to L.** 31 tasks in five batches (tasks.md): four start from `main` now, the fifth waits for `composite-values` A–B and `generics` A–B.

## Sources

- `GAPS.md` §4.5 L51, L52, L53; L58 (`limexp`); §8 U5 (the clipper, the chapter-8 buck).
- `docs/spec/part_iv_behavior.md` §12.1.15, §12.1.17, §13 (rules 1, 2, 7 to 10, 14, 17, 18, errors), §14.9; `part_v_execution.md` §15 (steps 3 and 4, T5, S1 to S7, Resolution of the settle phase); `part_vi_io.md` §19 rule 8; `part_vii_stdlib.md` §22.2.4.
- `docs/book/content.html`: the clipper (chapter "Analog behavior") and the buck (chapter "Mixed-signal"), with their refusal listings.
- `.specs/features/device-abi/abi.md` §4 (global `SETTLE`); `crates/piperine-digital-solver/src/engine.rs`, `plan.rs`; `crates/piperine-lang/src/elab/known.rs:159`.

---

## Success Criteria

- [ ] No `P0001` remains for an event read as a `bool`, for `settle`, for an `Event` function or for `limexp`.
- [ ] The Book's chapter-8 buck and (with `msl-fixes`) its clipper run as printed.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.
