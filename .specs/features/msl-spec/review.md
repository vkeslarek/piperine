# Review of `docs/spec/` (MSL technical specification)

Reviewer pass over the 16 files of `docs/spec/` against `duel/spec_v5.md`, `STYLE.md` and
`WRITER_NOTES.md`. Method: full read of `index`, Parts I–VI, VIII, Appendices A, B, G; targeted
read of Part VII, Appendix C and the galleries; scripted checks of error codes, EBNF excerpts,
link anchors, rule references and bold definitions. Nothing in `docs/spec/` was edited.

## Verdict

**Not ready to publish as is; ready after one integration pass.** The design survived the
formalization: 11 concepts, 65 constructs (B.1.1 sums to 65, B.1.2 has 65 rows), no new keyword,
operator, attribute or macro. No Resolution contradicts a v5 rule outright. What is wrong is the
seam work between the five writers: one corrupted table, three places where two parts state
different rules, five rule-citation conventions, and error codes assigned twice to one condition.
About 154 `Resolution` notes exist; only 4 are surfaced in Appendix G.

| Count | BLOCKER | MAJOR | MINOR |
| --- | --- | --- | --- |
| Findings | 3 | 17 | 22 |

### Mechanical checks

| Check | Result |
| --- | --- |
| B.1 count | 65 in B.1.1 and in B.1.2. Pass. |
| Markdown links | 288 links, 0 broken anchors. But 11 of 16 files contain no link at all (Parts III–VIII, Appendices B–G use bare `§`). |
| Error codes | 176 defining rows. 0 referenced-but-undefined. 4 codes carry two conditions inside one part; ≥ 12 conditions carry two codes across parts (M4). |
| EBNF excerpts vs B.3 | 7 productions differ (M3). |
| Rule references | every `§a.b.c` / `§a rule k` target exists; 25 gallery references use a form index §0.2 forbids; ~50 references in Parts V, VI, VIII use a third form (M2). |
| Concept table (index §0.5) vs parts | corrupted at row 4 (B1). |
| History / duel mentions | none in normative text. Appendix G "Closed earlier" narrates design history (m20). |

### Scores

Elegance rubric of `duel/BRIEF.md`, applied to the specification as the document of the language.

| # | Axis | Score | Justification |
| --- | --- | --- | --- |
| 1 | Orthogonality | 8.0 | The concept set is untouched. Overlap was introduced only in the apparatus: one condition, two error codes; one term, two definitions. |
| 2 | One rule per concept | 6.5 | Rules were split into "static part / dynamic part" (§8.2.3, §11 rule 2, §13 rule 1), and several Resolutions add exceptions: a merged short keeps its flow unknown "only if probed", `p.changed()` exempt at `(0, 0)`, undelayed definition forbids an initial value, `finish` ignored under `maybe`, `E0415` with two phases. |
| 3 | Syntax economy | 8.5 | 65 held. B.3.5's ten amendments add no construct. One slip: `#[derivative]` takes a list in §3 and one path in B.3. |
| 4 | Stdlib-in-language | 7.5 | Every v5 definition is kept and is normative. 17 items are signature-plus-contract only, and three of those contracts leave observable behavior open (`normal`, `table_model` splines, `noise_table`). |
| 5 | Readability | 7.0 | Gallery walkthrough tables are the best part of the document. Five citation conventions, rules listed out of numeric order, and link-free parts cost a point each. |
| 6 | Semantic closure | 8.0 | Every core primitive and stdlib item has a per-solve table; §16.2 governs. Holes: which scheduled instants are time points (B2), anonymous instance names (B3), param history (M7), the two extern ABIs (M10), G.33–G.36. |
| | **Mean** | **7.6** | |

| Document quality | Score | Note |
| --- | --- | --- |
| Navigability | 6.0 | glossary index covers ~60 of ~130 defined terms and mis-points 6; 11 files unlinked; Appendix G cited without item ids in 8 places |
| Redundancy | 6.5 | the per-solve table is restated in §9, §11, §12.1, §13, §14.1–14.10 and each §22 entry; safe only because §14 rule 6 and §22 rule 5 say which copy governs |
| Normative clarity | 7.5 | RFC 2119 words used consistently in I–IV; Part V is the clearest; phase model broken by Part VIII (M5) |

---

## BLOCKER

**B1. `index.md` §0.5: the concept table is corrupted.**
Line 135 is `| 4 | discrete nature, continuous nature | [§4](…) |` (a glossary row, 3 cells) and
line 136 is the drive row without its number. The table renders broken and shows no concept 4.
Fix: delete line 135; prefix line 136 with `| 4 `. Add the lost row to §0.9:
`| discrete nature, continuous nature | [§4](part_ii_carriers.md#4-natures) |`.

**B2. `part_v_execution.md` §17.2 rule 2 vs `appendix_d_gallery_analog.md` intro Resolution: which scheduled instants are time points is stated two ways.**
Part V: decided "from the static dependencies", and its Resolution adds "conservative: a time
point may occur at which no probe is read". Gallery D Resolution: "its reactions" means the
reactions whose event can fire given the commit collected there, so a falling `clk` edge is not a
time point (D.5, F.1, F.2 "(guarded instant)"), and ends "Part V owns the final wording". Part V
never took it up. The difference is observable: `settle` firings, settle commits
(`LaggedResistor`, `zi_nd` seed), record order. "May occur" also gives implementations latitude
in an observable. The same question applies to "the instant of a reaction that writes `sim`"
(static: every clocked monitor with a `fatal()` makes each edge a time point).
Fix: decide in §17.2 rule 2 and delete the gallery Resolution (a normative note may not live in an
informative appendix). Recommended text, consistent with §17.3 rule 6 and v5 D.5: "A reaction
counts at a scheduled time only if its event can fire there: its guard, evaluated on the discrete
state settled at `(t, 0)`, is not known false. Reactions reached in later microsteps count
statically." Replace "a time point may occur" by "the set of scheduled time points is exactly
this set". State that the `sim` clause is dynamic: the instant at which a `finish` record is
written.

**B3. `part_vi_io.md` §21 rule 3 + `part_ii_carriers.md` §4 rule 2 Resolution: `resolve` order depends on an implementation-chosen name.**
§4 fixes the order of drives as the lexicographic order of instance paths. §21 rule 3 leaves the
spelling of an anonymous instance's name "to the implementation". A drive held by an instance
placed inside an expression (`y = inertial(…)` in `TribufDelayed`, any converter placed in an
argument) then sorts differently per implementation; a non-associative `resolve` (`SumNet`) and
the record order of §19 rule 8 differ with it. This breaks §0.1 rule 1.
Fix in §21 rule 3: "An instance with no `let` of its own is named `<module>#<k>`, `k` the
0-based ordinal of the placement among the placements of that module name in the enclosing
module, in source order after generation unrolling." Any fixed scheme will do; it must be
normative.

---

## MAJOR

### Cross-part contradictions

**M1. `part_iii_structure.md` §8.1.10 vs `part_ii_carriers.md` §4 rule 13, §6.1.3.**
§8.1.10: a carrier port "MUST be connected to a carrier of the same nature". §4 rule 13 and
§6.1.3: compatible continuous natures connect. `E0313` and `E0251` then disagree.
Fix §8.1.10: "…a carrier of the same discrete nature, or of a compatible continuous nature
(§4 rule 13), with the same element type and array shape." Drop the nature clause from `E0313`
and cite `E0250`/`E0251`.

**M2. Rule citations: five conventions.**
(a) index §0.2 and Parts I–IV: `§8.2.3`, `§11 rule 3`. (b) Parts V, VI, VIII: `§15.2 rule 1`,
`§17.2 rule 2` (≈50 uses) for what (a) writes `§15.2.1`. (c) Part V letter handles `T1–T6`,
`I1–I8`, `S1–S7`, `O1–O5`, `H1–H7`. (d) Part VI `19.S1–19.S6`. (e) Galleries: `§13.6`, `§13.4`,
`§2.2`, `§4.2` (25 uses), which §0.2's own Resolution forbids ("the dotted form always names a
subsection"); galleries also restate a different convention in their headers.
Fix: keep (a). Rewrite (b) to `§15.2.1`; rewrite (e) to `§13 rule 6`; delete the "Reference
convention" paragraphs of D, E, F and the "Handles" paragraph of Part V. Admit (c) in §0.2 as
"lettered lists of §15–§16, cited `§15 T2`", and renumber (d) as §19 rules continuing v5's
(rules 10–15).

**M3. Syntax excerpts do not match B.3 (STYLE: "verbatim").**

| File, line | Production | Excerpt lacks |
| --- | --- | --- |
| `part_i_values.md` 191 | `fn_decl` | `'const'?`, `fn_param` (so `self` does not parse) |
| `part_i_values.md` 288 | `impl_block` | `impl_item*` (so `pub const Z` does not parse) |
| `part_ii_carriers.md` 21 | `nature_decl` | `attrs 'pub'?` |
| `part_iii_structure.md` 33 | `module_decl` | `'pub'?` |
| `part_iii_structure.md` 43, `part_iv_behavior.md` 36 | `gen<X>` | `else gen<X>` |
| `part_iii_structure.md` 45 | `instance` | `callee` |

And the reverse: `part_i_values.md` §3 rule 11 defines `#[derivative(f, …)]` with one function per
`f64` parameter; B.3 has `attr_derivative ::= '#' '[' 'derivative' '(' path ')' ']'`.
Fix: copy the seven productions from B.3.2; change B.3 to `'(' ( path ',' )* ')'`.
Part VI has no Syntax excerpt for `emit` / `include_str`; add both from B.3.2.

**M4. One condition, two error codes (and one code, two conditions).**
STYLE gives each part a prefix; writers each coded the conditions they touch. Parts III/IV already
use the right device elsewhere (a row keyed by `§5.2` instead of a code). Apply it to these:

| Condition | Codes | Keep |
| --- | --- | --- |
| branch from a node to itself | `E0241`, `E0412` | `E0241` |
| contribution/probe on a node the module cannot use | `E0244`, `E0411` | `E0244` |
| flow contribution on a nature without `flow` | `E0242` (part), `E0413` | `E0413`; trim `E0242` |
| port connected to a carrier of the wrong nature | `E0250`/`E0251`, `E0313` | `E025x` |
| a function places an instance | `E0120`, `E0350` | `E0350` |
| behavior block not in the module's file | `E0160`, `E0401` | `E0401` |
| stdlib parser rejects the text | `E0614`, `E0706` | `E0706` |
| `+>` / `<-` outside its statement form | `E0907`, `E0404` | split: `E0907` lexical position, `E0404` wrong block |
| write to a carrier the module may not drive | `E0220`, `E0222`, `E0334`, `E0431` | `E0220` (second drive), `E0222` (not drivable); `E0334`/`E0431` cite them |
| non-`Repr` type carried | `E0112`, `E0202` | `E0112` |
| primitive or `emit!` used where not allowed | `E0122`, `E0450`, `E0601` | state precedence: `E0601` for `emit!`, `E0450` for core, `E0122` for the calling `fn` |
| initial value of the wrong form | `E0210`, `E0245` | one code |

Same code, two conditions: `E0334` ("target may not drive", §8.2; "target is a param", §9),
`E0430` (two wordings, §12.1 and §13), `E0503` ("a solve of any kind is singular", §15.2;
"singular equilibrium solve (§15.2)", §16, wrong back-reference). Make the second row of each a
pointer. `E0455` is unassigned; say so or renumber.

**M5. `part_viii_host.md` errors vs `index.md` §0.3 rule 3 and Part V's header.**
§0.3: a run-phase condition "MUST fail the analysis". Part V: "A run error ends the analysis".
But `E0802`, `E0803`, `E0806` are "refused, changes nothing", and `E0807`–`E0809` "do not end the
analysis"; `E0801` has phase "elaboration or run". Part III already writes "run (host)".
Fix: add a fourth phase to §0.3: "**host**: a host operation is refused or answered with a
diagnostic; the analysis continues." Reclassify `E0801`–`E0809`. Likewise decide single phases
for `E0101` ("parse or elaboration") and `E0415` ("elaboration … else run": split into two codes).

**M6. `part_iii_structure.md` §8.2.4 uses "settle phase" for step 3.2.**
"The commit is made in the settle phase of the microstep in which `expr` changed." Part V defines
**settle phase** as step 4 (`on settle`), and a commit made there is a settle commit, due at the
next time point (§15 S1). Read with Part V's definition, every delayed definition (`SrLatch`,
`TwoPhase`, `transition`'s `xd`) would lag one analog step.
Fix: "The commit is made when the structure settles (§15 step 3.2) in the microstep in which
`expr` changed."

**M7. Params in `changed()` / `prev()` are stated three ways.**
§13 rule 16 (Resolution): `p.changed()` does not fire at `(0, 0)`. §16 step 2 and §23.1 rule 4:
a `set` made before the run is committed at `(0, 0)` "like the model's own" initial values, and
"`changed()` fires". §15 step 3.2: "every carrier or param whose value changed … fires". And
§5.3's table gives `prev()` as "unknown before the first [change]", while §3.3.1 says params are
always known; no text gives `p.prev()` before the first `set`.
Fix: in §16 step 2 add "A `set` made before the run binds the param; it is not a change
(§13 rule 16)." In §5.3 add a rule: "Before the first `set` during a run, `p.prev()` equals `p`."

**M8. Stale cross-writer statements left in normative text.**
(a) `part_vi_io.md` §19 Resolution: "Appendix B.3 carries no production for `emit!`" — false,
B.3.2 has `emit` and `include_str`. (b) `part_iv_behavior.md` §10 rule 8 Resolution: "Appendix B.3
should say so" — B.3.4 does. (c) `appendix_d_gallery_analog.md`: "Part V owns the final wording"
(see B2). Delete the three sentences; in (a) cite B.3.2.

### Terms

**M9. Terms with two definitions.**

| Term | Defined at | Problem | Fix |
| --- | --- | --- | --- |
| quantity | §4 rule 9 (a nature's `potential`/`flow` item) and §9 (param, discrete carrier, node) | two meanings; both listed as "defined here" | §4: "**nature quantity**"; keep §9's |
| operating-point variable | §11 rule 6 and §21 rule 6 | bold twice | unbold §21 |
| host | index §0.1 and §23 | bold twice | unbold §0.1 |
| jump, solve, settle | each bold twice or three times inside Part V (lines 71/89/307, 103/234, 108/316) | step labels look like definitions | use italics for step labels |
| instance | glossary says §8; bold is in §8.3 | | fix pointer |

**M10. Glossary index (§0.9) is incomplete and mis-points.**
Wrong targets: *equilibrium solve* (bold in §15, listed §16.2), *time point* (§15, listed §17.2),
*instance* (§8.3, listed §8), *result* (§8 rule 3, listed §8.3). No bold definition exists for
*limiting* and *analysis*. About 70 terms that parts list under "Terms defined here" are absent:
solve, transient step, jump solve, small-signal sweep, tick, state, settle phase, settle commit,
held node, hold, round, sweep point, scheduled time, located crossing, nodeset, session, host
action, `set`, `force`, sensitivity, gradient, saltation correction, guard, event position,
selector, branch kind, transport delay, placement, const context, discrete/continuous expression,
named expression, delayed/anonymous definition, loop, relaxation, direct dependence, implicit
param, effective multiplicity, extern module, site, trace, reduction, path, gradient param,
strength, high impedance, converter, monitor, inertial delay.
Fix: generate §0.9 from the bold occurrences.

### Completeness

**M11. `part_vii_stdlib.md` §22.2.17 `normal`: the algorithm is "the one in the standard library's source".**
`ClockGen` (D.2) then produces different edges on two conforming implementations, against §0.1
rule 1 and §22 rule 4 ("the contract is normative"). Same class, smaller: `table_model`
interpolation `2` and `3` name "quadratic spline" / "cubic spline" with no end conditions
(§22.2.15), and its `E0705` is a run error raised by a "pure `fn`" with no stated mechanism
(it is a panic, §1 rule 11: say so).
Fix: specify the generator and the splines, or mark each **implementation-defined, MUST be
documented** and add a G item (as G.46 does for the loop bound). Do not leave it silent.

**M12. `part_iii_structure.md` §8.5: the two extern ABIs have no interface.**
Constructs 55, 56 and 51 (`"osdi"`, `"cosim"`, `#[link]`) are placed, typed and given per-solve
behavior, but: no OSDI version is cited; the `"cosim"` step function, its descriptor, how it
returns "the next time it wants to run", and its behavior under unknown inputs are unspecified;
`#[link]` keys other than `model` are not enumerated. An implementer cannot build either from
the text. `simparam` (§14.4) likewise names no standard parameter.
Fix: cite OSDI 0.4 normatively for `"osdi"`; add a `"cosim"` C interface section or an explicit
G item "cosim ABI: open"; state that unknown levels on `"cosim"` inputs are passed as a mask or
make the outputs unknown.

**M13. Appendix G does not surface the Resolutions that narrow or extend the design.**
G's "Flagged" list has 4 entries for ~154 Resolutions. WRITER_NOTES lists a dozen "Review:" items
that never reached G. At minimum add flagged entries for the items in the "Resolutions" section
below marked **G**. Also: G.17 is "Provisional" while §23.1 says the top module "is open";
eight places cite "Appendix G" with no item id (Part V lines 47, 190, 390, 493, 639; Part VI 100;
Part VIII 98; Part IV 317) — add the ids (G.10, G.34, G.32, G.33–G.35, G.6, G.35, G.17,
G.33/G.36). Part VII already cites by id.

**M14. `part_iv_behavior.md` §14.6.2, §13 rule 11 vs stdlib `zi_nd` / `every`.**
`delay` without `max` requires `td` "constant during an analysis: an expression over params,
constants and the environment"; `at` requires a discrete argument. `zi_nd` calls
`delay(x, k as f64 * period)` and `every(period, period)` with `period` a **value input**, which
by kind is a continuous expression. §13 rule 11 says "checked per instance, after value inputs
are substituted"; §14.6.2 does not, and §22.2.12 is silent. As written `zi_nd` fails `E0452`.
Fix: add to §14.6.2 the same sentence as §13 rule 11 (or cite §8.3.9), and add to §22.2.12 a
static rule: "`period` MUST be a discrete expression constant during an analysis at each call."

### Structure of the text

**M15. Rule numbers are out of order and three rules are split in two.**
To keep v5 numbers while grouping static before dynamic, Parts I–IV list e.g. §8.1.4–§8.1.14,
then §8.1.1–§8.1.3, then §8.1.15…; §3.3.5 precedes §3.3.1; and "§8.2.3 (static part)" /
"(dynamic part)", "§11 rule 2 (static/dynamic part)", "§13 rule 1 (static/dynamic part)" each
appear twice. A reader looking for rule 2 finds half of it. Part V has the opposite problem:
§17.2 holds two numbered lists (rules 1–6, advance procedure 1–5), so `§17.2.4` is ambiguous;
§15 likewise.
Fix: list rules in numeric order with a `[static]` / `[dynamic]` tag per rule, each rule once;
letter the §17.2 advance procedure (`A1–A5`) like the other Part V lists.

**M16. `part_i_values.md` §3.1 lead vs its own Resolution.**
Lead: "`impl X { … }` … holds functions only". Resolution and B.3 `impl_item`: associated `const`
items are allowed on value types. Fix the lead: "holds functions, and on a value type associated
constants; it has no time."

**M17. `part_vii_stdlib.md` §22 rule 3 vs §3.3.7 and §14 rule 1.**
"A project MAY replace any item by declaring an item of the same name." §3.3.7 obliges an
implementation to treat "`known`, `known_or`, `is_known`, `high` of the stdlib" as flag-total; a
replacement of the same name is then either trusted without analysis or silently rejected in
`analog`. Fix §3.3.7: "…the stdlib's own definitions; a replacement is checked under §3.3.6 like
any function." State in §22 rule 3 that core names (§14 rule 1) are excluded.

---

## MINOR

| # | Where | Problem | Fix |
| --- | --- | --- | --- |
| m1 | Parts III–VIII, App. B–G | no hyperlinks; Parts I–II and index have 288 | link `§` references (STYLE left this to the reviewer); a script over the heading list suffices |
| m2 | §6 rule 17 | "divided by the instance multiplier `m`" | "by the effective multiplicity (§8.5.3)" as §8.5.4, §14.1.6 |
| m3 | §14.2.1, §14.3.1 vs B.2.2 note ² | Part IV: `ddx`, `ac_stim` "in `analog` only"; B.2.2 allows them in a `let` or value input read only by `analog` | add B's sentence to §14.2.1 and §14.3.1 |
| m4 | §22.2.3 `noise_table` | value input of type `&[(f64, f64); N]`: `&` outside a function parameter (§1 rule 2, `E0102`) | make `table` a param `[(f64, f64); N]` |
| m5 | §22.2.13, §22.2.14 | listings declare `-> f64` with a body that has no result: `E0301` by §8 rule 3 | mark as signatures: replace bodies by `{ … }` and say "signature only" |
| m6 | §22 code comments | bare v5 references: "(3.3 rule 5)", "(12.1 rule 2)", "(8.3 rule 1)", "(15)" | prefix `§` and use the index convention |
| m7 | B.3.3 item 1 | repeats B.3.1 item 1 | delete |
| m8 | B.1.2 rows 35–46 | "Defined in §14" for primitives that now have §14.1–§14.10; `bound_step` cites §17.3, defined in §17.2 rule 5 | cite §14.x; fix `bound_step` |
| m9 | `E0132`, `E0303` | phase "parse" for checks that need name resolution (is the `impl` target a nature?) | "elaboration" for `E0132` |
| m10 | §6 rule 9 vs §11 rule 10, §14.1.2 | probe of `(a, a)` reads 0 (§6) but §14.1.2 points only at §11 rule 9 | add "a probe of `(a, a)` reads 0 (§6 rule 9)" to §14.1 |
| m11 | §6 rule 8 | "both nodes of a branch have the same nature" ignores compatible natures inside one module | "the same or compatible natures (§4 rule 13)" or state the restriction is intended |
| m12 | §8.1.7 / B.3.3 item 13 / §6 rule 6 | initial value on a continuous **port**: III forbids, B says "a carrier the module drives", II is silent | add to §6 rule 6: "A port of a continuous nature takes no `= expr` (§8.1.7)." |
| m13 | §11 rule 3, third bullet | "keeps a flow unknown only if the module probes that flow" — and `p.inflow()`? | add "directly or through `inflow()`" |
| m14 | §12.1.4 | "pending commits … for times `≥ T`" does not say how microsteps of one tick compare | "for instants at or after the new commit's due instant" |
| m15 | §16 `E0503` row | "(§15.2)" back-reference in the operating-point table | "(§16 H4, H5; §8.3)" |
| m16 | §19 rule 5 Resolution | `finish` ignored when unknown or `maybe`: also applies to `fatal()` | say so in §22.6.2; add to G (see R11) |
| m17 | §21 rule 6 | opvar "at the last accepted solution" — in a `Dc` sweep, of which point? in `Ac`? | "of each sweep point; for `Ac`/`Noise`, of the operating point" |
| m18 | §23 options table | bounds on microsteps and rounds are session options, so `E0501`/`E0506` depend on the session | already implied by I7; say "raising a bound never changes a result that did not hit it" |
| m19 | A.1 attribute Resolution | "an attribute not in this table is an error" also rejects `#[allow]`, `#[cfg]`, `#[doc]` | list `#[doc]` (it is what `///` is); state that `#[cfg]` is rejected on purpose |
| m20 | Appendix G "Closed earlier", "Closed in this specification" | design history ("earlier revisions of the design") against STYLE's tone rule | keep the table, drop the provenance sentence |
| m21 | §9 rule 4.2 vs §5.1.9 | initial value "evaluated with the param values in force when the analysis starts" / "not committed again" after a `set` | consistent; cross-reference each other |
| m22 | Redundancy | per-solve rows restated in 6 places plus every §22 entry | keep, but make each copy end with "(§16.2)" as §14 rule 6 does; today §9 rule 6 and §12.1.21 do not say which governs |

---

## Resolutions: wrong, or changing the design

**Wrong (must change).**

| # | Resolution | Why |
| --- | --- | --- |
| R1 | §17.2 "Rule 2 is static, so it is conservative … a time point may occur" **and** Appendix D "time points at guarded scheduled instants" | contradict each other and v5 D.5 "falling edges cost nothing" needs the second (B2) |
| R2 | §21 rule 3: anonymous instance names spelled by the implementation | makes §4 rule 2's drive order and §19 rule 8's record order implementation-defined (B3) |
| R3 | §22.2.17: `normal`'s algorithm is whatever the library source is | results not a function of the source (M11) |
| R4 | §19: "B.3 carries no production for `emit!`" | false (M8) |

**Narrow or extend v5; acceptable, but each needs a Flagged entry in G (M13).**

| # | Resolution | Effect on the design |
| --- | --- | --- |
| R5 **G** | §4 rule 13: compatible natures also need equal flow names | v5 rule 5 tests potential names only; a `Servo` node (flow named `Voltage`) can no longer meet an `Electrical` one |
| R6 **G** | §13 rule 11: the argument of `at` MUST be discrete (no probe) | v5 says only "`now()` is not an input"; `at(v(x))` was not excluded |
| R7 **G** | §8.2.21: an undelayed definition forbids an initial value on its levels | new static error; no v5 model is hit |
| R8 **G** | §13 rule 16: `p.changed()` never fires at `(0, 0)` | v5 16 step 2 commits pre-run `set`s there (M7) |
| R9 **G** | §8.4.4: a placement through a template value takes no param block | follows from "no struct update", but removes `cell(c) { m: 2.0 }` |
| R10 **G** | §8.5.3: `m` and `dtemp` are readable names inside a module, with effective values | v5 has them as implicit params only |
| R11 **G** | §19: a `finish` record ends the analysis only if known true and not `maybe` | a `fatal()` under unknown control does not stop the run |
| R12 **G** | §8.3.6: every core result has a direct dependence on its inputs (`let y = delay(f(y), td)` is an error) | consistent with v5 8.3 rule 5; excludes delay-feedback written directly |
| R13 **G** | §3.3.6–7: the `analog` check is syntactic over "flag-total functions"; four stdlib names are mandatory | new term carrying a conformance rule; interacts with stdlib replacement (M17) |
| R14 **G** | §11 rule 3: the selector rule also binds the flow contributions of a potential-capable branch; a probed short is not merged | extension of rule 3 with an exception clause |
| R15 **G** | §6: `p.inflow()` includes branches of child instances ending on `p` | matches VAMS `I(<p>)`; v5 rule 5 says "this module's branches" |
| R16 **G** | §15 T2 ties round to the later tick; §15 T3 a positive delay that rounds to no advance is the next microstep | observable at half-tick delays |
| R17 **G** | Appendix A: `Unknown` is a reserved literal; unknown attributes are errors; `char` and lifetimes dropped | narrows "close to Rust" |
| R18 **G** | §1: `usize` is 64 bits on every platform | needed for determinism; differs from Rust |
| R19 **G** | §23.1: `force` on a node is a potential source to `gnd`; a host action answering an event is due at `(t, n + 1)` | new host semantics |

**Gallery models changed (allowed by STYLE; confirm).** `BehavioralCell` store guarded by
`c.wl.prev().is_known()` (F.3); `ShareCharge` ports `a`, `b` became `pub signal`s with initial
values (F.5); `where` added on const generics (`Pwl`, `Delay`, `Divider`); integer literals as
`f64` defaults rewritten (`RSwitch.ron`, `Dac8.rsw`, `D2A`/`Bidir.rout`); `zi_nd`'s `out` renamed
`vout`. All five are corrections of v5 models under v5's own rules; none changes the language.

**Judged correct and worth keeping as written:** §8.2.3 relaxation steps 1–2; §12.1.14 statement
identity; §12.1.2 "then by execution order"; §15 event table and the step list for event times
that are not time points; §15.2 "the limit is normative, a finite `h` must agree within
tolerance"; §16.1 rule 2 (crossing between sweep points fires at microstep 0 of the later point);
§17.1 `scale`; §18 rules 6–8; §23.3 rules 4a–4h; B.3.4 (contribution and commit at any depth);
B.3.5 amendments 1–10; A.2 rules 3–6.

---

## Completeness by construct (B.1), exceptions only

Every one of the 65 rows has a defining section, a type, a B.2 placement and, where it runs, a
per-solve or per-instant table. Rows with a gap:

| # | Construct | Gap |
| --- | --- | --- |
| 28 | `prev` | on a param before the first `set` (M7) |
| 34 | `settle` | which instants are time points (B2); in an analysis at a transient's last instant (G.34, open) |
| 45 | `simparam` | no standard names |
| 46 | `analysis` | `Static` at a transient's last instant (G.33, open); periodic flags (G.36, open) |
| 49 | `#[derivative]` | grammar vs §3 rule 11 (M3) |
| 51, 55, 56 | `#[link]`, `"osdi"`, `"cosim"` | no ABI (M12) |
| 57 | `sim` | static or dynamic time point (B2); continuation after `finish` (G.35, open) |
| 62 | value inputs and a result | per-instance checks stated for `at`, not for `delay` (M14) |

Open items of v5 (changelog "Left open", critique_v4 "Residual open items"): all present as
G.33–G.38; none was silently resolved.

---

## Resolution log

Integration pass over `docs/spec/`. The design is unchanged: 11 concepts, 65 constructs (B.1.1 and
B.1.2 untouched), no new keyword, operator, attribute or macro. Checks after the pass: 169 error
codes, each defined in one row, none referenced undefined; every Syntax excerpt line is a line of
B.3.2; no citation outside the forms of index §0.2; 0 broken anchors.

| Finding | Action | Files |
| --- | --- | --- |
| B1 | fixed: concept row 4 restored; the lost glossary row is in the regenerated §0.9 | `index.md` |
| B2 / R1 | fixed: §17.2.2 is the single statement. A reaction counts at a scheduled time when its event fires at `(t, 0)` on the settled discrete state (dynamic), or is reachable in a later microstep (static). "May occur" removed; the `finish` clause is dynamic. Gallery Resolution deleted; D and F cite §17.2.2; one row added to the §17.2 situation table | `part_v_execution.md`, `appendix_d_gallery_analog.md`, `appendix_f_gallery_mixed.md` |
| B3 / R2 | fixed: §21 rule 3 names an anonymous instance `<module>#<k>` (source order of the callee, then unrolling order); §4's order note cites it | `part_vi_io.md`, `part_ii_carriers.md` |
| M1 | fixed: §8.1.10 admits compatible continuous natures; `E0313` is array shape only, nature mismatches are `E0250`/`E0251` | `part_iii_structure.md` |
| M2 | fixed: one convention, tabulated in index §0.2 (lettered lists of Part V admitted, `§17.2 A1–A5` added). about 50 `§a.b rule k` rewritten to `§a.b.k`; 25 gallery `§13.6` forms rewritten to `§13 rule 6`; `19.S1–S6` are §19 rules 10–15; the "Reference convention", "Handles" and "Rule numbers are identifiers" paragraphs are deleted; bare numbers in code comments carry `§` | all parts, D, E, F, `index.md` |
| M3 | fixed: the seven excerpts copied from B.3.2; `attr_derivative` takes a list; `emit` and `include_str` excerpts added to §19 and §20 | `part_i_values.md`, `part_ii_carriers.md`, `part_iii_structure.md`, `part_iv_behavior.md`, `part_vi_io.md`, `appendix_b_grammar.md` |
| M4 | fixed: one code per condition, rule stated in index §0.2. Retired: `E0120`, `E0160`, `E0202`, `E0245`, `E0334`, `E0411`, `E0412`, `E0431` (each row is now a pointer to the owner). `E0242`, `E0313`, `E0614`, `E0601`, `E0450`, `E0122` trimmed; `E0907`/`E0404` split; `E0222` widened to every non-drivable target; the second rows of `E0430`, `E0503`, `E0317`, `E0319`, `E0323`, `E0336`, `E0350` are pointers. `E0455` is covered by "a gap is a retired code" | Parts I–VI, `appendix_a_lexical.md`, `index.md` |
| M5 | fixed: fourth phase *host* in index §0.3; `E0801`–`E0809` reclassified; `E0101` is parse (a heap type is `E0161`); `E0415` split into `E0415` (elaboration) and `E0418` (run) | `index.md`, `part_viii_host.md`, `part_i_values.md`, `part_iii_structure.md`, `part_iv_behavior.md` |
| M6 | fixed: §8.2.4 says "when the structure settles (§15 step 3.2)" | `part_iii_structure.md` |
| M7 | fixed: new §5.3.11 (binding a param is not a change; `p.prev()` equals `p` before the first `set`); §16 step 2 and §23.1.4 aligned | `part_ii_carriers.md`, `part_v_execution.md`, `part_viii_host.md`, `part_iv_behavior.md` |
| M8 / R4 | fixed: the three stale sentences are gone; §19 quotes the B.3.2 production | `part_vi_io.md`, `part_iv_behavior.md`, `appendix_d_gallery_analog.md` |
| M9 | fixed: "nature quantity" in §4; `host` bold only in §23, operating-point variable only in §11 rule 6; step labels of §15 and §16 in italics | `part_ii_carriers.md`, `index.md`, `part_vi_io.md`, `part_v_execution.md` |
| M10 | fixed: §0.9 regenerated from the bold occurrences, 99 rows, wrong targets corrected; *limiting* (§18) and *analysis* (§11.1) given defining occurrences. The per-part "Terms used from other parts" lists are deleted: §0.9 is the one map | `index.md`, `part_v_execution.md`, `part_iv_behavior.md`, Parts III–VIII |
| M11 / R3 | partially, deferred to G.64: the generator of `normal` and the spline end conditions are implementation-defined and **MUST** be documented; `E0705` is a panic. No generator was specified, to avoid inventing design | `part_vii_stdlib.md`, `appendix_g_scope.md` |
| M12 | partially, deferred to G.65 and G.66: OSDI 0.4 cited normatively (§8.5.8); `model` is the only `#[link]` key; §8.5.16 makes a `"cosim"` step all-or-nothing under unknown inputs; the C interface of `"cosim"` and the names of `simparam` stay open | `part_iii_structure.md`, `part_iv_behavior.md`, `appendix_g_scope.md` |
| M13 | fixed: G.49–G.63 added (below); G.17 and §23.1 agree; the eight bare "Appendix G" citations carry ids | `appendix_g_scope.md`, Parts IV, V, VI, VIII |
| M14 | fixed: §14.6.2 checks per instance after substitution; §22.2.12 states the requirement on `period` | `part_iv_behavior.md`, `part_vii_stdlib.md` |
| M15 | partially: each split rule appears once (§8.2.3, §11 rule 2, §13 rule 1, with *Static:* / *Dynamic:* inside the rule); the §17.2 advance procedure is `A1–A5`. Reordering into ascending numbers is rejected: STYLE fixes both the design's rule numbers and the Static-then-Dynamic grouping, and the two cannot hold together; index §0.2 now says so once | `part_iii_structure.md`, `part_iv_behavior.md`, `part_v_execution.md`, `index.md` |
| M16 | fixed | `part_i_values.md` |
| M17 | fixed: §3.3.7 covers the stdlib's own definitions; §22 rule 3 excludes the core names and gives a replacement no privilege | `part_i_values.md`, `part_vii_stdlib.md` |
| m1 | rejected: linking every `§` in eleven files adds about 60 kB of markup for no new content; better done by the site build | — |
| m2, m10, m11, m12 | fixed (m11: the restriction is intended, it is the one `N` of §6 rule 4) | `part_ii_carriers.md`, `part_iv_behavior.md` |
| m3 | fixed: §14.2.1 and §14.3.1 carry B.2.2 note ² | `part_iv_behavior.md` |
| m4, m5, m6, m16 | fixed: `noise_table`'s table is a param; root-form filters are marked "heads and params only"; comments carry `§`; `fatal()` under unknown control stated | `part_vii_stdlib.md` |
| m7, m8 | fixed: B.3.3 item 1 deleted and the items renumbered; B.1.2 rows 35–46 and 49 cite §14.x | `appendix_b_grammar.md` |
| m9 | fixed: `E0132` is elaboration | `part_i_values.md` |
| m13, m14 | fixed | `part_iv_behavior.md` |
| m15 | fixed with M4 | `part_v_execution.md` |
| m17 | fixed: per sweep point in `Dc`, the operating point in `Ac` and `Noise` | `part_vi_io.md` |
| m18 | fixed in §15 I7 | `part_v_execution.md` |
| m19 | fixed: `#[doc]` listed; `#[cfg]` and `#[allow]` rejected on purpose | `appendix_a_lexical.md` |
| m20 | fixed | `appendix_g_scope.md` |
| m21 | fixed: §5.1.9 cites §9 rule 4 | `part_ii_carriers.md` |
| m22 | fixed by deletion: §12.1.21, §13 rule 19, §9 rule 6, §8.2.25, §14.1.5, §14.12 and the per-solve tables of §14.2–§14.8 and §14.10 now cite §16.2 or §15; the `idt` table and listing of §8.3 cite §22.2.8; the end-of-part code tables of Parts V and VIII are deleted | `part_iii_structure.md`, `part_iv_behavior.md`, `part_v_execution.md`, `part_viii_host.md` |
| R5–R19 | deferred to G.49–G.63, status "Resolved provisionally", in order: R5 G.49, R6 G.50, R7 G.51, R8 G.52, R9 G.53, R10 G.54, R11 G.55, R12 G.56, R13 G.57, R14 G.58, R15 G.59, R16 G.60, R17 G.61, R18 G.62, R19 G.63. Each Resolution note cites its item | `appendix_g_scope.md` and the owning parts |
| Gallery models changed | not touched; still to be confirmed by the design owner | — |

Totals: 3 BLOCKER fixed; 17 MAJOR, of which 14 fixed and 3 partial (M11, M12, M15); 22 MINOR, of
which 21 fixed and 1 rejected (m1); 4 wrong Resolutions corrected; 15 narrowing Resolutions
recorded as G.49–G.63, plus G.64–G.66 opened. Not verified by this pass: Appendix C was not
re-read, and Part VII was edited only at the entries the findings name.
