# Root E2E: the divergences, by cause

Triage of T34 over the inventory at `f32fddba` (`tests/book/inventory.tsv`): every row whose verdict is
`diverges` or `disputed`. Cause (1): an implemented construct gives another result, fixed in code by
Phase 9. Cause (2): not implemented, owned by an approved feature, `refused`. Cause (3): a code no source
raises, `unraised`. A stale Book sentence is corrected to the binary (rulings of the orchestrator,
2026-11-19, recorded on the `Dispatched` line of `tasks.md`).

Size: 8 cause (1) rows, 3 stale sentences, 1 reclassified to (2). The largest area holds 6 rows
(diagnostics), no area exceeds the threshold, so no `book-conformance` follow-up is written.

| Row | Sentence (short) | What the code gives | Cause | Area | Owner or status |
| --- | ---------------- | ------------------- | ----- | ---- | --------------- |
| errors#1 | the `E0150` diagnostic shows a help line | no help line | (1) | T35 diagnostics | fixed in T35 (`holds`) |
| errors#2 | the `E0163` diagnostic shows a secondary label on `mod util;` | no secondary label | (1) | T35 diagnostics | fixed in T35 (`holds`) |
| from-rust~64ee8d1d | `emit!` in a module body is `E0601` | `E0900` | (1) | T35 diagnostics | fixed in T35 (`holds`) |
| digital~ce04e7a4 | a latch without `after` is a loop, `E0339` | two `E0338`, no `E0339` | (1) | T35 diagnostics | fixed in T35 (`holds`) |
| digital~54ec4a1d | `count <- count + 1.0` is `E0432` | `P0002` at the addition | (1) | T35 diagnostics | fixed in T35 (`holds`) |
| mixed~98e4dc1b | `on every` inside a reaction is `E0350` | `E0404` | (1) | T35 diagnostics | fixed in T35 (`holds`) |
| mixed~68328637 | the buck start-up climbs by 0.0033 per period from `vout` near 0 | the operating point applies the first `hs <- true` commit: 11.8 V, duty 0 | (1) | T36 values and records | fixed in T36 |
| cli~0549bfb1 | a `nodeset` on `ac` or `transient` is `P0010` | accepted and ignored | (1) | T40 host and command line | the spec and host-ii F6 say `nodeset` applies to every analysis: the Book sentence is corrected, and the code is verified to seed the operating point of both |
| modules~1faf8c12 | `m` and `dtemp` are bound like any param | `E0320`, no param `m` | (2) | none | `refused` under L29, owner `params-and-io` |
| welcome~5b11a01a | `sin` and the math methods are refused with `P0001` | `sin` runs | stale Book | T40 | the Book is corrected to the binary; the row becomes `holds` |
| mixed~22d4126e | the math methods such as `sin` are refused with `P0001` | `sin` runs, `settle` is `P0001` | stale Book | T38 | the Book is corrected to the binary; the row becomes `holds` |
| compiler~e9ad112c | worst reverse-bias difference 5.9e-21 A | 4.98e-20 A measured (inside the 1 pA floor) | stale Book | T36 | the Book is corrected to the measurement; the row becomes `holds` |

Also in scope of Phase 9 (not rows): the line numbers of the listing of compiler#3 (`devices.msl:5:1` and
`8:32` in the Book, `4:1` and `8:34` from the binary, T35) and `finish()` with `--stream` omitting `sim`
(today "ended before its stop", T40).

Not fixed here, listed for the user: none. No sentence of the spec itself looks wrong.

T35 (diagnostics): the compiler#3 listing positions are corrected in the Book to `devices.msl:4:1` and `8:34` (the binary's), asserted in `book_compiler`. The `E0163` secondary label is given for a top-of-namespace `use` clash only; the block-level clash (a `use` inside a block) keeps its primary label alone.
