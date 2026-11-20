# Unknown Semantics: Events Design

**Spec**: `.specs/features/unknown-semantics-events/spec.md`
**Status**: Approved (2026-11-20, under the user's standing AFK rule; see "Decisions taken (user AFK)")
**Base**: `main` at `494a7f75`. Batches B1 to B4 start from it now; B5 starts once
`composite-values` phases A–B and `generics` phases A–B are on `main` (D12).

---

## What the code does today (`main` at `494a7f75`)

| Area | Today | Where |
| ---- | ----- | ----- |
| `settle` in the checker | a `Resolution::Builtin` with no type: `P0001` "built-in `settle`" | `crates/piperine-lang/src/types/expr.rs:553` (the `Resolution::Builtin(id)` arm), `pom/builtin.rs:104` (`row("settle", BuiltinKind::Event, "§14.9")`) |
| An event read as a `bool` | `P0001` "an event read as a `bool`" in a reaction body and in a guard; `E0441` when committed (`q <- up`), because the checker treats a commit's value as "committed" (`self.committing`) | `types/event.rs` (`event_out_of_place`, `negated_event`) |
| `fired(e)` | already in the IR: `Expr::Fired(u32)`, the `CHANGED` byte of event `e`'s pin, built for a split head (`(fired(e₁) && c₁) \|\| ..`); planned (`Source::Fired`) and printed (`event{k}_fired`) | `piperine-ir/src/model.rs:449`, `elab/events.rs:130`, `piperine-plan/src/plan.rs:137,786`, `piperine-transpile/src/print/value.rs:644` |
| Event sources | `Changed`, `Cross`, `At`, `ParamChanged`; no `Settle` | `piperine-ir/src/model.rs:227` |
| `GLOBAL` event pins | `START` (`Digital::start_pin`, for `at`); `SETTLE` is named by the ABI and printed by `ShapePrinter` but no body asks for it | `piperine-plan/src/digital.rs:505`, `piperine-transpile/src/print/shape.rs:31` |
| The settle phase | `Mode::settle` fires the `SETTLE` nets at `(at, n + 1)` and runs reactions; it does **not** run the discrete functions after firing, so an event function reading `SETTLE` never runs; a record of the phase is stamped `n + 1` | `piperine-digital-solver/src/instant.rs:234` |
| Settle commits | a commit with no advancing delay from a reaction whose head pin is a `SETTLE` net is refused: `DigitalError::Unsupported { what: "settle commit" }` | `piperine-digital-solver/src/run.rs:577` |
| The owner | runs the settle phase at every time point of a transient (`InstantMode { settle }`) and once at the operating point, and already delivers the operating point's settle boundary to the analog side (`Restart::None`) | `piperine-solver/src/protocol/{transient.rs:348,operating_point.rs:61}` |
| `limexp` | `pub module limexp(x: f64) -> f64 { limit(x, \|new, old\| deltalim(new, old, 1.0)).exp() }`; the closure's two parameters are `DeclKind::Binding` children of the module, and the module item walk refuses every unlisted child kind: `P0001` "binding inside a module" | `crates/piperine-lang/std/analog.msl:19`, `types/check.rs:558` |
| Functions returning `Event` | no function takes a carrier (L8); `handshake(req, ack)` is `P0001` "method `rise` on this receiver" | `types/event.rs:126` |
| The Book | `analog#3` (clipper), `mixed#10` (buck check), `cheatsheet#5` print the `settle` refusal; `digital#6`, `mixed#6`, `mixed#15` the bool read; `values#2` the event function | `tests/book/inventory.tsv`, `tests/book/{analog,mixed,digital,values,cheatsheet,welcome}.rs`, `crates/piperine-cli/tests/book.rs:530` |

---

## Architecture Overview

Nothing new crosses the device ABI: `SETTLE` is already a `GLOBAL` event pin (abi.md §4), and
abi.md §10 step 4 already says where a settle commit is due. The work is one IR event source,
one plan pin, one printed event function, the settle phase run as the spec says, and the
owner protocol landing the held commits.

```
MSL ──types/event.rs──► Ty::Event / Ty::Bool (rule 2, rule 9)
     ──elab/events.rs──► EventSource::Settle, Expr::Fired(k)   (IR)
     ──plan/digital.rs──► GLOBAL SETTLE event pin, Source::Settle
     ──print/digital.rs──► event function: fired_ = settle_changed && guard
     ──digital-solver──► settle phase: fire SETTLE, run functions, run reactions (on_settle by phase)
                         a no-delay commit there is held (SettleCommits) and reported as settle_boundary
     ──solver protocol──► deliver settle_boundary to analog now (restart, next attempt exempt);
                          land the held commits at (t_next, 0) once t_next is accepted; drop at the stop
```

### How `settle` lowers

`settle` is a core `Event` value (§13 rule 10). It lowers like `c.changed()`: an event
`Event { source: EventSource::Settle, guard }` on the body, its pin written by an event
function that reads the `CHANGED` byte of the body's `GLOBAL` `SETTLE` pin and the guard
(D5). `settle || ev` and `settle && g` follow the disjunctive form `elab/events.rs` already
builds. In the settle phase only `SETTLE` has fired (§15 Resolution), so a disjunct of another
source reads false there, and a guard reading `x.changed()` reads false unless `settle` makes
it fire.

### The settle phase (§15 step 4, §19 rule 8)

At the end of an instant that is a time point the engine fires the `SETTLE` nets, **runs the
discrete functions of their cone** (today it does not, so a guarded `settle` could never fire),
then runs the reactions whose event pins fired. The phase records under `(t, n)`, `n` the index
of the instant's last microstep (§19 Resolution; today `n + 1`, divergence V1). A reaction that
runs in the settle phase is `on_settle` whatever its head names (§13 rule 14: the phase decides),
replacing today's test "one of its head pins is a `SETTLE` net" (D4).

### Settle commits (§15 S1 to S7)

```
time point t:   … microsteps … │ settle phase: reaction commits x <- 1 (no delay)
                                │   engine: SettleCommits holds it; Outcome::settle_boundary = [(x, bytes)]
owner:          deliver settle_boundary to the analog copy, Restart::Integrator; next attempt exempt (S2, S3)
step t → t_c:   discrete instants in (t, t_c) run on the old x (S4); a rejected step restores the
                checkpoint, which holds SettleCommits (S5)
accept t_c:     if t_c is a time point: Digital::land_settle_commits(t_c) → due at (t_c, 0); run the
                instant: x changes, changed() fires at microstep 0 (S1)
stop:           Digital::drop_settle_commits() (S6); a commit with an advancing delay is ordinary (S7)
```

`SettleCommits` is the engine's list of held commits (statement, bytes, net, replace flag), a
field of `Checkpoint` so a restore brings it back. The landing tick is decided by the owner,
which alone knows when the next time point is accepted: a forced landing that is not a time
point (abi.md R6) does not land them. In an operating point or a `Dc` point the next time point
is the next sweep point (S6): the protocol lands the held commits at the start of the next
`operating_point()` call on the same engine, and the transient's first time point lands the
operating point's commits (O4). The analog side reads a settle commit from the solve that
arrives at `t_next` (S2), which is the immediate delivery: between `t` and `t_next` there is no
other solve.

### Events read as `bool` (§13 rules 2, 9)

In a reaction body, an `Event` subexpression where a `bool` is expected types as `bool` (the
checker records it as an *event read*). It lowers to `Expr::Fired(k)`: `k` is the event of the
body equal to it (the head's own events first, by structural equality of source and guard), or
a new event appended to the body's events and not to any reaction's `on` (D7). Its pin is
written by its event function every microstep, so `fired(k)` is whether it fired in this
microstep. In a guard the same holds (rule 9): the guard's event subexpressions lower to
`fired(j)`, and the event function of the guarded event reads event `j`'s pin; the plan orders
`j`'s function before it (an event cannot read itself: its guard is not part of `j`) (D8).

`q <- up` with `q: Bit` commits the `bool` rule 2 reads (D2): the "committed" of rule 8 is an
`Event` where a type other than `bool` is expected.

### `limexp`

A closure parameter is a `DeclKind::Binding` child of the module that holds the closure. The
module item walk of `types/check.rs` skips it, as it skips `GenericParam`: the binding is typed
with its closure, where `limit` already takes it (D9). The module with a result is hoisted as
MD-64/MD-71 hoist any module with a result; `limit` makes one `LIMIT` input per use (§22.2.4
rule 4).

### Functions returning `Event` (B5, after `generics` B)

A `fn` whose result type is `Event` has its body's result in an event position (§13). It is
never an IR function (IR functions are pure values): every call is inlined where it stands,
through `generics`' `HandleBinding` (`elab/handles.rs`, its T49), its handles bound to the
caller's carriers, and its result lowered by `elab/events.rs` into the caller's head, `let`,
guard or body read (D14).

---

## Code Reuse Analysis

### Existing Components to Leverage

| Component | Location | How to Use |
| --------- | -------- | ---------- |
| `Expr::Fired`, `Source::Fired` | `piperine-ir/src/model.rs`, `piperine-plan/src/plan.rs` | the body read of an event and the guard read |
| The disjunctive head | `elab/events.rs` (`Disjunct`) | `settle` is one more source; a guard's event read is one more conjunct form |
| `Digital::start_pin` | `piperine-plan/src/digital.rs:505` | the model of `settle_pin` |
| The settle phase and `InstantMode::settle` | `instant.rs:234`, `protocol/sides.rs:42` | run as today, plus the function cone and the held commits |
| `Restart::Integrator`, `Attempt` exemption | `protocol/transient.rs` | the landing of S3 |
| `HandleBinding`, `Scope::handles` | `elab/handles.rs` (generics T49) | the inlining of an `Event` function |
| `Refusal`, `Chapter`, `Commands` | `tests/book/support/` | the Book rows |
| The twin corpus and `TranRun` | `tests/e2e/twins/`, `crates/piperine-parity/tests/parity_tran.rs` | `tran_clipper` |
| `OperatingPoint` compare | `crates/piperine-parity/tests/parity_op.rs` | the `limexp` diode against `diode_series.cir` |

### Integration Points

| System | Integration Method |
| ------ | ------------------ |
| `piperine-ir` | `EventSource::Settle` appended last (msl-digital convention); `Ir::check` and the printer take it |
| `piperine-interp` | refuses `EventSource::Settle` by name (codegen context R1) |
| `piperine-plan` | `Digital::settle_pin`, `Source::Settle` |
| `piperine-transpile` | `print/digital.rs` event function |
| `piperine-digital-solver` | `instant.rs` settle phase; new `settle.rs` (`SettleCommits`); `run.rs` drops the refusal |
| `piperine-solver` | `protocol/transient.rs`, `protocol/operating_point.rs` land and drop |
| `piperine-lang` | `types/event.rs`, `types/check.rs`, `elab/events.rs` |

---

## Components

### C1. IR `EventSource::Settle`
- **Purpose**: the event `settle` of a body.
- **Location**: `crates/piperine-ir/src/model.rs` (appended last), `check.rs`, `print.rs`.
- **Interfaces**: `EventSource::Settle`; printed `settle`.
- **Reuses**: the printing of `Changed`.

### C2. Plan `settle_pin` and `Source::Settle`
- **Purpose**: the `GLOBAL` `SETTLE` event pin of a body with a `Settle` event, and the read of its `CHANGED` byte by the event function.
- **Location**: `crates/piperine-plan/src/digital.rs`, `plan.rs`.
- **Interfaces**: `Digital::settle_pin(s, body) -> Option<u32>`; `Source::Settle` appended last.

### C3. Printer
- **Purpose**: the event function of a `Settle` event: `fired_ = settle_changed && guard`.
- **Location**: `crates/piperine-transpile/src/print/digital.rs`.

### C4. The settle phase
- **Purpose**: fire, run the function cone, run reactions, stamp `(t, n)`; `on_settle` by phase.
- **Location**: `crates/piperine-digital-solver/src/instant.rs`, `run.rs`.

### C5. `SettleCommits`
- **Purpose**: the commits made in the settle phase with no advancing delay, held until the owner lands or drops them.
- **Location**: `crates/piperine-digital-solver/src/settle.rs` (new); fields on `Digital`, `Checkpoint`, `Outcome::settle_boundary`.
- **Interfaces**: `Digital::land_settle_commits(&mut self, tick: Tick)`, `Digital::drop_settle_commits(&mut self)`, `Digital::holds_settle_commits(&self) -> bool`.

### C6. Owner protocol
- **Purpose**: S1 to S6 at the owner: deliver the settle boundary with a restart, exempt the next attempt, land at the next accepted time point, drop at the stop and after the last sweep point.
- **Location**: `crates/piperine-solver/src/protocol/transient.rs`, `operating_point.rs`; `DiscreteSide` gains `land_settle_commits`/`drop_settle_commits` (`sides.rs`, adapter in `analyses/adapters.rs`).

### C7. Checker
- **Purpose**: `settle` typed `Event`; an event read as `bool` in a reaction body and a guard; `-> Event` functions (B5).
- **Location**: `crates/piperine-lang/src/types/event.rs` (all new methods there), dispatch arm in `types/expr.rs`; closure bindings skipped in `types/check.rs`.

### C8. Elaborator
- **Purpose**: `EventSource::Settle`; `Expr::Fired` for an event read; the inlined `Event` function (B5).
- **Location**: `crates/piperine-lang/src/elab/events.rs`; a new `elab/event_reads.rs` (`EventReads`, the lookup of an event read's index and the append of a new event) when `events.rs` would pass 600 lines.

---

## Data Models

```rust
/// The commits a reaction made in the settle phase with no delay that advances time: held until
/// the next time point is accepted (§15 S1), then due at its microstep 0 (abi.md §10 step 4).
pub(crate) struct SettleCommits {
    /// Each held commit, in the order the reactions made them (§12.1.2).
    held: Vec<Planned>,
}

/// What running an instant did (existing), one field added:
pub struct Outcome {
    // …
    /// The nets the analog side reads and the bytes the held settle commits will give them: the
    /// solve that arrives at the next time point reads these (§15 S2).
    pub settle_boundary: Vec<(NetId, Vec<u8>)>,
}
```

---

## Error Handling Strategy

| Error Scenario | Handling | User Impact |
| -------------- | -------- | ----------- |
| `settle` outside an event position, a guard or a reaction body | `E0441` (existing text) | as for any `Event` |
| `!ev` outside a guard or a reaction body; `bool && Event` in an event position | `E0440` (existing) | unchanged |
| An `Event` where a type other than `bool` is expected | `E0441` | unchanged text |
| A `fn -> Event` called outside an event position, a guard or a body | `E0441` | B5 |
| A settle commit at the stop time | dropped (S6), no diagnostic | none |
| A held settle commit whose bytes fail the canonical check when landed | the existing `check_canonical` fault at `(t_next, 0)` | the run fails as any commit would |

---

## Goldens, pins and tests that may move

Only these move, each under the task named; a replaced test is replaced by the acceptance test
of the criterion that supersedes it, and the commit body names both and the spec line.

| Pin | Moves in | Why |
| --- | -------- | --- |
| `crates/piperine-digital-solver/tests/instant.rs::a_settle_commit_with_no_delay_is_refused` | T7 | S1: the commit is held, not refused (replaced by `a_settle_commit_is_held_until_it_is_landed`) |
| `tests/book/welcome.rs` the `on settle` line of `welcome~a17f566a` | T6 | the form runs; the line asserts it runs |
| `tests/book/cheatsheet.rs` (cheatsheet#5, cheatsheet#7's `settle` half) | T6 | `settle` runs; cheatsheet#7 keeps L49 |
| `tests/book/analog.rs::the_clipper_is_refused_for_limexp_and_settle` and the clipper listing | T6 (settle half), T21 (runs) | L52, then `limexp` |
| `crates/piperine-cli/tests/book.rs::the_clipper_refusal_no_longer_names_sin` | T6 (no `settle`), T21 (runs) | the refusal shrinks, then goes |
| `tests/book/mixed.rs` mixed#10, mixed~a6b3f8ed, the buck listing | T6 | the check runs |
| `crates/piperine-lang/tests/elab_digital.rs::a_head_the_ir_cannot_hold_is_p0001` | T14 | rule 2 (replaced by `an_event_read_as_a_bool_in_a_body_lowers_to_fired`) |
| `tests/book/mixed.rs` mixed#6, mixed#15 | T16 | rule 2, D2 |
| `tests/book/digital.rs` digital#6 | T16 (only `let mut` left, D15), T26 (runs) | |
| `tests/book/values.rs` values#2, values~c389077a | T30 | L53 |
| `crates/piperine-cli/tests/docs.rs` rows of L52, L58, L51, L53 | T23, T31 | the rows leave `GAPS.md` |
| `tests/book/inventory.tsv` rows of the Book blocks above | the task that changes the block | `inventory_guard` prints the corrected row |

The 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/`, every
`ToleranceContract` and every parity target stay byte-identical. `tran_clipper` is a new twin
with no `.command` file, so the CLI twin count of `crates/piperine-cli/tests/twins.rs` stays
sixteen (D11).

---

## Cross-lane touches

| Lane | Shared files | Rule |
| ---- | ------------ | ---- |
| `composite-values` (in flight) | `types/event.rs` (no), `types/check.rs` (the module walk), `elab/events.rs` (no) | T17 touches one arm; B5 rebases |
| `generics` (in flight) | `types/handles.rs`, `elab/handles.rs` | read only, from B5 |
| `analog-operators` (sibling, item 10) | `tests/book/analog.rs`, `tests/book/cheatsheet.rs`, `inventory.tsv` (L49 rows next to the clipper) | rows are edited per row, never reordered; `analog#7` and `cheatsheet#7`'s L49 half stay theirs |
| every lane | `spec_text.rs`, `docs.rs`, `GAPS.md` | appended to, never edited above; `GAPS.md` keeps its numbering |

---

## Risks & Concerns

| # | Risk | Mitigation |
| - | ---- | ---------- |
| R1 | The Book's buck may not hold `\|vout − 3.3\| < 50 mV` at every time point after 10 ms (ripple) | T22 runs it; a failing check is a finding, the row becomes `diverges` with the measured margin, and the Book's model and the check's bound are not changed (D16) |
| R2 | `limexp` hoisted as a module with a result inside `analog` may meet a lowering gap behind the binding refusal | T17's test elaborates `limexp`; anything new it surfaces stops the task and is reported |
| R3 | An event function reading another event pin (D8) could reorder the discrete functions of the buck | every buck target after T15; the plan only adds edges for a guard that reads an event |
| R4 | The `Pfd3` test needs `let mut` (composite-values A) | it is in B5 (D13) |
| R5 | The settle phase now runs functions: a design whose functions read `SETTLE` changes | no design reads it today (the checker refused `settle`) |

---

## Divergences found (current code vs docs/spec in this area), each a fix task

| # | Divergence | Spec | Fix |
| - | ---------- | ---- | --- |
| V1 | A record of the settle phase is stamped `(t, n + 1)` | §19 Resolution: the index of the last microstep | T4 |
| V2 | The settle phase does not run the discrete functions after firing `SETTLE` | §13 rule 1 (a guard is evaluated in the microstep) and rule 14 | T4 |
| V3 | `on_settle` is decided by the head's pins | §13 rule 14: by the phase | T4 |
| V4 | `q <- up` is `E0441` | §13's own `Comparator`; rule 2 | T12 with the §13 Resolution (D2) |
| V5 | A settle commit is refused | §15 S1 | T7 to T9 |

---

## Decisions taken (user AFK)

The user said "always take the most reasonable default and tell later". Each default follows
the approved spec, `docs/spec/` (the authority) and the active MD decisions; each is reported
back.

| # | Open point | Default taken | Why |
| - | ---------- | ------------- | --- |
| D1 | The spec's UE-06 AC 1 waited on L14 and L55, and counted "two `clip` records per period" | the WHERE clause is dropped (`msl-fixes` delivered both, MD-67); the count is one per period, two in the 2 ms run, as the Book says ("two `clip` lines, one per period") | the Book is the source of UE-06 |
| D2 | `on up \|\| dn { q <- up; }`: rule 8 says an `Event` is never committed; §13's `Comparator` commits `up` | in a reaction body an `Event` where a `bool` is expected reads as a `bool`, a commit's value included; `E0441` is an `Event` where another type is expected. §13 gains a Resolution saying so, guarded by `spec_text` (T12) | the spec's own example; the Book reads it so (mixed#6); amending the text explicitly, not silently |
| D3 | The stamp of a settle-phase record | `(t, n)`, `n` the last microstep run (V1) | §19 Resolution |
| D4 | What makes a reaction "on settle" for the due of its commits | the phase it runs in | §13 rule 14 |
| D5 | How `settle` reaches a body | an event pin written by an event function reading the `GLOBAL` `SETTLE` pin's `CHANGED` byte, like `changed()`; no reaction lists `SETTLE` directly | one path for every source; a guard on `settle` needs the function anyway |
| D6 | Where a settle commit waits and who decides `t_next` | the engine holds it (`SettleCommits`, in the checkpoint); the owner lands it once a time point is accepted and drops it at the stop and after the last sweep point; the analog side gets its bytes at once, with `Restart::Integrator`, and the next attempt is exempt | abi.md §10 step 4 ("`D` on a shadow"), §15 S2/S3/S5/S6; only the owner knows when a time point is accepted |
| D7 | An event read as a `bool` that is not in the reaction's head | appended to the body's events (its pin and function run every microstep) and read with `fired(k)`; an equal event already on the body is reused | §13 rule 2 has no head restriction; `fired` is the IR's existing read |
| D8 | A guard that reads an event | `fired(j)` of a body event; the plan orders `j`'s function first | rule 9 says a guard reads as a body does |
| D9 | `limexp` (the spec put it here; L9 is `composite-values`') | the module item walk skips closure bindings; nothing else of closures changes here | narrow; `composite-values` does not touch this arm |
| D10 | Proof of UE-05 AC 3 and 4 (no Newton trace exists from `analog` until L49) | through the compiled device's ABI call: the argument after `limit` given `old = 0`, `new = 30` is 1.0; the partial of the current is `is · limexp(x) / nvt` | the device is the unit that limits; L49 is `analog-operators`' |
| D11 | ngspice parity of what this feature unblocks | the `limexp` diode at `diode_series.cir`'s operating point under `DC_VOLTAGE`; a new transient twin `tran_clipper.cir` under `TRAN_VOLTAGE`, with no `.command` (the CLI twins stay sixteen); no tolerance moves | the user's rule: parity first, never retune |
| D12 | Order against the in-flight lanes | B1 to B4 start from `main` at `494a7f75` now; B5 starts after `composite-values` A–B and `generics` A–B are on `main` (T24 rebases) | the caller's split; ROADMAP item 10 follows item 9 for L53 only |
| D13 | `Pfd3` (UE-01 AC 3) needs `let mut` | its run and Book row are in B5 | `let mut` is `composite-values` phase A |
| D14 | How an `Event` function lowers | inlined at every call through `generics`' `HandleBinding`, also when it takes no handle | an IR function is a pure value; an event is not |
| D15 | A Book row this feature unblocks that another gap still refuses | the row's gap and owner are re-pointed to that gap, with the refusal it now gives | the inventory names the gap that explains a row |
| D16 | The Book's buck check fails | the row is `diverges` with the measured margin and the finding goes to the user; the model and the 50 mV bound are not changed | parity and the Book's claim are measured, never tuned |
| D17 | The decision record | one `MD-` entry with the next free number at merge (siblings renumber, ROADMAP item 4), written at T23 and completed at T31 | STATE practice |
| D18 | Approval lines | `**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"` on tasks.md; the spec keeps its 2026-11-18 approval and gains an `**Amended**` line | the siblings' practice (generics D16) |
| D19 | Test placement | settle and bool reads from source in a new `crates/piperine-parity/tests/source_events.rs` (models in `tests/events/`), `limexp` in a new `source_limexp.rs`; protocol tests on stubs in the existing `protocol_transient.rs`/`protocol_operating_point.rs` | MD-28: a target lives in the crate it exercises; one concern per target |

---

## Tech Decisions

| Decision | Choice | Rationale |
| -------- | ------ | --------- |
| The ABI | unchanged | `SETTLE` and the settle commit's due instant are already abi.md §4 and §10 step 4 |
| IR | one variant, `EventSource::Settle`, appended last | no new `Expr`: `Fired` exists |
| The `MD-` entry | "`settle` runs as §15 says: held settle commits landed by the owner; an event read as a `bool` is `fired(k)`" | D17 |
