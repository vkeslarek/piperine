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
| mixed~68328637 | the buck start-up climbs by 0.0033 per period from `vout` near 0 | the tick fires at (0, 1) inside the operating point; its `hs <- true` is a round of the operating point (part V §16 step 4), so the transient starts from 11.8 V: 10 us reads vout 11.6 V, duty 0 | spec and Book disagree, code follows the spec | T36 | `disputed`, owner `user`: the Book's table and the sentence, or §16 step 4 for a reaction whose commit the solver reads, one of the two moves; no code is changed |
| cli~0549bfb1 (now cli~271fd4c4) | a `nodeset` on `ac` or `transient` is `P0010` | the seed is not ignored: it picks the root of a bistable node in both (host-ii `option_reach`, and `book_cli`), no `P0010` | stale Book (not a code divergence) | T40 | the Book is corrected to the spec (host-ii F6); `holds` (fixed in T40) |
| modules~1faf8c12 | `m` and `dtemp` are bound like any param | `E0320`, no param `m` | (2) | none | `refused` under L29, owner `params-and-io` |
| welcome~5b11a01a (now welcome~a17f566a) | `sin` and the math methods are refused with `P0001` | `sin` runs | stale Book | T40 | the Book is corrected; `holds` (fixed in T40) |
| mixed~22d4126e (now mixed~a6b3f8ed) | the math methods such as `sin` are refused with `P0001` | `sin` runs, `settle` is `P0001` | stale Book | T38 | the Book is corrected; `holds` (fixed in T38) |
| compiler~e9ad112c (now compiler~dc42a66e) | worst reverse-bias difference 5.9e-21 A | 4.98e-20 A measured (inside the 1 pA floor) | stale Book | T36 | the Book prints 5.0e-20 A; `holds` (fixed in T36) |

Also in scope of Phase 9 (not rows): the line numbers of the listing of compiler#3 (`devices.msl:5:1` and
`8:32` in the Book, `4:1` and `8:34` from the binary, T35) and `finish()` with `--stream` omitting `sim`
(today "ended before its stop", T40).

Not fixed here, listed for the user: none. No sentence of the spec itself looks wrong.

T35 (diagnostics): the compiler#3 listing positions are corrected in the Book to `devices.msl:4:1` and `8:34` (the binary's), asserted in `book_compiler`. The `E0163` secondary label is given for a top-of-namespace `use` clash only; the block-level clash (a `use` inside a block) keeps its primary label alone.

T36 (values and records): the buck row is not a code divergence. The code follows part V §16 (step 4: a commit the analog side reads starts another round of the operating point, and `every(0.0, T)` fires at (0, 1)); the Book's start-up from 0 V contradicts the spec. Left `disputed` for the user, with the test asserting today's values.

T37 (analog): none. No kind (1) row has the area analog.

T38 (digital and mixed): the four digital and mixed rows with a diagnostic (digital~ce04e7a4, digital~54ec4a1d, mixed~98e4dc1b) were fixed in T35 with the other diagnostics; the stale `sin` sentence of chapter 9 is corrected here.

T39 (packages and the library): none. No kind (1) row has the area.

T40 (host and command line): `nodeset` is verified to seed the operating point of ac and transient (a bistable node's result follows the seed: `book_cli::a_nodeset_seeds_the_operating_point_of_ac_and_transient`), so the Book sentence was stale and is corrected. A `finish()` with `--stream` omitting `sim` ended the run "before its stop": the finish was read from the filtered records only. Fixed in `piperine-api` (`Streams::finish_of`), test `digital_only::a_finish_ends_the_run_ok_when_the_sim_stream_is_not_kept`.
