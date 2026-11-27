# Review history of the device-ABI proposal

This file accompanies `proposal-v6.md`. Section and rule numbers in the first table are those of v5.

## A.1 The first review (versions 1 to 3)

A proposer and a critic, two rounds, both citing one snapshot of the code. Round 1 (22 findings, four
blocking) showed that the first version narrowed the where conditions, lost the loader's check of a
compiled shape, claimed proof for `force`, and took *carrier* and *signal* from the language with other
meanings. Round 2 (19 findings, none blocking) corrected the migration, the *hold*, the party that
checks a created shape, and the wording of `RESHAPE`, and settled from the code that analog code never
reads an array. The critic's closing judgement of v3's concept set: well placed, nearly orthogonal, not
small. The user rejected v3 on that last word.

## A.2 The second review

It scores each version on coverage (pass or fail against C1–C98 and S1), simplicity and elegance.

**Round 1, on v4.** Coverage failed narrowly; simplicity 6, elegance 6. Twenty-one findings. The answer
to each:

| Finding | Verdict | What v5 does |
| --- | --- | --- |
| R1-1 (Blocker) the three behaviours of a trace | accepted | Stated normatively (13.6, 15.4); a trace-record area in section 4; section 5 and row C8 corrected. The trace leaves the merged table: `traces` stands alone. |
| R1-2 (Blocker) flags on an event | accepted, as a refusal | Listed as refusal 2 with the critic's evidence; rows C30 and C34 say they are narrowed; 16.1 says "nets". |
| R1-3 the refusals are more than five | accepted | One complete list (section 6), from a walk of every rule case and every present field. Four of v4's refusals are removed by definition, so the list is six. |
| R1-4 what the where-condition flag drops | accepted | Said in 0.4 and 21.1. |
| R1-5 array checks at bind | accepted | Section 12.1 and the list at the end of section 17. |
| R1-6 `Measurement.unit` | accepted | Removed, with the struct. |
| R1-7 three misstatements | accepted | The sensitivity citation gives both call sites; section 4 says the function results are also where term values and strengths are read from, and lists the stored operating-point values; today's tags are counted as seven. |
| R1-8 the concept count fell by reclassification | accepted | The budget counts every defined noun and lists them (0.2). The count is 30, as in v4; the text says so and claims no reduction there. Tag values are defined inside the row of their tag. |
| R1-9 `MAY_RESHAPE` | accepted | Removed. 16.3 is today's two conditions. |
| R1-10 `function_kind` states what the writes state | accepted, and taken further | The tag is removed; the kind is a definition (14.2), and the table there lists the schedule, the wanted partials, the statuses and the error report, which v4 omitted. |
| R1-11 the step bound is not a measurement | accepted | `STEP_BOUND` is a written aspect of the call environment. |
| R1-12 strength aspects; rule V8 | accepted | One aspect `SMALL_SIGNAL_SOURCE_STRENGTH`; `RECORD_STREAM` is defined to track nothing and the rule is gone. The cost is refusal 3. |
| R1-13 merges judged unsound | accepted | Not reopened. |
| R1-14 `EVENT_FIRED` against D6 | accepted: the sentence of D6 is withdrawn | Duality is a stated rule (14.1): reading an aspect gives a facet's state and writing sets it. The second sentence of D6 is removed from the diagnosis, and D6 is now about a part whose meaning depends on the kind of its cell. |
| R1-15 idea 2 does not decide the tables | accepted | Replaced by "one table per obligation", tested on a complete sentence (0.3). It separates the crossing, the operating-point variable, the trace and the step bound without counting fields. |
| R1-16 *variable* | accepted | The three tables are of *pins*. *Variable* is used only in *operating-point variable*, with the language's meaning. |
| R1-17 the weak points of A.4 | accepted | `Linkage` is described as a tagged union; its key serves ports only; the name's two jobs are stated; `Term` and `Crossing` use `SESSION_DEFAULT`; the aspects stay at 24 (three fewer, three added by the split and the step bound); tags are three. |
| R1-18 idea 1's soft spot | accepted | The analog tables are "where the results of analog code go". |
| R1-19 *measurement* | accepted | The word is not used. |
| R1-20 no table of "where does X go" | accepted | Section 3.1, 27 rows. |
| R1-21 Part IV | accepted | Seven decisions; the names are one; the two exceptions to "code follows the specification" are marked. |

Nothing was refuted. Four things the critic may want to test in round 3, stated so that it starts from
them:

1. **Eleven tables, not ten.** The critic left open whether a trace could stay in a merged table if the
   tag were defended as more than a recipient. The proposer chose the split, because rule 1 as now
   stated demands it.
2. **Dropping `function_kind`** goes beyond what the critic asked. The defence is idea 1: digital code
   writes pins and analog code cannot, so the kind is a consequence and not a datum.
3. **Refusal 3** (one writer for the two parts of a phasor) is the price of the single strength aspect.
4. **`pin` is the present word.** The proposer's own diagnosis D1 attacked `Pin`; the claim here is
   that D1's findings are about the struct, and all of them are answered by the three tables and the
   linkage.

## A.3 What every version since v3 keeps

- every where condition of every instance is read after the cone, for writes to parameter nets and to
  session nets (16.4);
- the default shape, and the loader's check of it against the plan (9.2, C90);
- the caller, not the device's thunk, checks a created shape against the default shape (15.2);
- `hold_potential` is the language's hold (11.1);
- the language's vocabulary, and the rule for the two pairs of words (2.1, 3.2);
- analog code never reads an array, and the guard is added (14.3);
- the alignment promise follows the code (15.1.3);
- the inventory C1–C98 and S1 (section 22), with `force` marked as not implemented; one row, C8, is
  corrected in this version on the critic's evidence;
- the staged migration, with the validator's tests kept until step D and the one commit that crosses
  three crates named (section 7).

## A.4 Facts re-verified in the code for v4 and v5

| Fact | Where |
| --- | --- |
| Ports are taken in the order of the `PORT` pins, whatever their kind | `sv/core/bind.rs:623`–`:631` |
| A continuous port left unconnected is bound to a branch unknown, as a private one is | `sv/core/bind.rs:638`, `:643` |
| An event fires on every applied write; a discrete net changes only when its bytes differ | `dg/run.rs:348`–`:351`; `dg/run.rs:102`–`:122` |
| A function may write discrete pins and event pins together; only one that reads `CHANGED` is restricted to events | `abi/validate.rs:547`–`:571` |
| The two consumers decide a function's kind by the same predicate on its writes | `an/plan.rs:236`; `dg/plan.rs:269` |
| A trace's buffer is zeroed before each call; a function whose only writes are traces is never needed | `an/assembly.rs:712`–`:714`, `:192`–`:193` |
| Trace records are kept through a restore | `sv/protocol/sink.rs:1`–`:8`, `:22` |
| The two parts of a stimulus are written by one function in the generator | `plan/plan.rs:808` |
| `HAS_CHECK` is read by one consumer, which reads only the named pin | `dg/engine.rs:554`–`:565` |
| `LATER` is kept only when `min_after` is at least one tick, so a zero promises nothing | `dg/plan.rs:291` |
| `STRUCTURAL` is folded into the net from any discrete or event pin | `sv/core/bind.rs:500` |
| `RESHAPE` is accepted only in a walk of the cone | `dg/run.rs:328` |
| The owner identifies a parameter by instance path and name | `sv/core/bind.rs:721` |
| The sensitivity analysis evaluates analog functions and digital functions through the one entry | `an/assembly.rs:587`; `sv/analyses/sensitivity.rs:333` |

## Round 2 of the second review, on v5, and what v6 does

| Finding | Verdict | What v6 does |
| --- | --- | --- |
| R2-1 the obligation rule does not decide terms, functions, linkage | accepted | Section 0.3 presents twelve one-sentence contracts, claims only that different contracts mean different tables, and lists the three merges made by judgement with what differs and why they stay merged. No "predicts". |
| R2-2 D2 still stands | accepted | D2 is reworded to the defect removed (two predicates). Section 6 lists "the kind is read from the writes" under what is not fixed. Decision 4 is the choice between derived and declared. 14.2 says what is checked of a function that writes nothing and that a trace-only function is never called. |
| R2-3 *pin* and D1 | accepted | The "no name of their own" clause is removed from D1. Decision 2 states the cost of the broad *pin*. *Net* and *unknown* are defined in section 3 and in the glossary. |
| R2-4 thirty nouns is the floor | accepted | Sections 0.1 and 3 are a core of nine kinds and four closed catalogues. Section 0.2 and section 6 say that this helps orientation and not mastery, and that the count cannot fall without conflation. |
| R2-5 one writer of a strength | accepted | A bind check, beside the existing one for a term's value (13.3, section 17); refusal 3 is reworded to cover a second writer of the whole strength. |
| R2-6 `STEP_BOUND` and the call environment | accepted | The call environment is the seven received values. The step bound is a result with no table (14.1). |
| R2-7 Part IV | accepted in part | The derived kind is its own decision (4); the cost of *pin* is in decision 2. Decision 1 is kept as a decision: the user rejected v3 as a whole and never ruled on it. |
| R2-8 smaller disagreements | accepted | Section 6 lists D2 and the first half of D5 as not fixed; 11.2 says `RECORD_STREAM` is a strict superset of `UNTRACKED`. |
| R2-9 three gaps | accepted | The two-writers check is in section 17; 14.2 covers a function with no writes; 9.5 says why `create` takes an index. |
| R2-10 the lowering sentence | accepted | Section 7 says write indices are translated where a strength is split. |
| R2-11 length | accepted | The review history is this file. Sections 0.4 and 1.5 and the long appendix are gone from the proposal. |
