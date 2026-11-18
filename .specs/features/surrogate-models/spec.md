# Surrogate Models Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

Priority: **later** (user, 2026-11-17: surrogate models are a "melhoria futura… bem depois"). It is specified now so that the gap has an owner; it is queued after `multi-rate` and `generics`.

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

Nothing below is decided. Each question has options and the planner's recommendation.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How does a run choose the buck's power stage? The spec already has two swap mechanisms: generation `if` on a structural param (§8.2.7, §8.1.2) and a template-typed param with an `Fn(P)` bound (§8.4) | **A.** A structural selector param on `Buck` (`averaged: bool = false`) and a generation `if` that places one stage or the other; a run sets it with `--param` before it starts. **B.** A template param `stage: S` where `S: Fn(Electrical, Electrical, Electrical, Electrical, Bit, Bit)`, bound by the bench (`Buck { stage: AveragedStage { .. } }`). **C.** A for the buck, and B proven on a small fixture | **C.** A works from the command line as it is today (a `bool` param). B is the general mechanism of §8.4 and must be proven once, but a host cannot write a template value (Q2) |
| Q2 | May a host bind a template-valued param from text (`--param Top.stage='AveragedStage { }'`)? | **A.** No: a host sets scalar params only; a template choice goes through a selector param in the model. **B.** Yes: a spec extension, the host's value is parsed as an MSL const expression (§8.4.6) | **A.** No spec change; §23.1 does not give hosts expression values today |
| Q3 | Does "surrogate" include models the tool derives? | **A.** Hand-written averaged models only. **B.** Also state-space averaging computed by the tool from the switching model (a new language or host construct). **C.** Also data-fitted surrogates through `table_model` | **A.** B is a research feature with no spec text; C is `table_model` (L57, `params-and-io`) once it runs |
| Q4 | May a run switch from the detailed stage to the averaged one partway (start detailed, continue averaged)? | **A.** No: the selector is structural, a `set` during a run is refused with `E0803` (§8.1.24), and a long run uses the averaged stage from time 0. **B.** A spec extension: a structural `set` at a continued transient (§16.4) re-elaborates and carries each `ddt` state whose instance path exists in both | **A.** B changes §16.4 and the jump rule; it is a feature of its own if wanted |
| Q5 | Where does the averaged stage live? | **A.** `examples/buck/src/averaged.msl`, beside the switching stage. **B.** A standard-library module (a change of §22). **C.** Both | **A.** It is the buck's model; §22 has no power-stage modules |
| Q6 | How close must the averaged start follow the switching one? | **A.** Each switching period's mean of the switching `vout` within 10 mV of the averaged `vout` at the period's end, and the means of the last 40 µs within 2 mV. **B.** Only the SPEC limits (overshoot, settling, ripple bound) on both runs | **A.** B would accept an averaged model that settles to the wrong value inside the band |

## Problem Statement

The buck's 600 µs start is the longest run that exists: about 675 000 accepted steps (`GAPS.md` V12). Scenarios that matter to the designer last milliseconds (load steps, setpoint ramps, `examples/buck/SPEC.md` §10). Even with `multi-rate`, the switching stage needs short steps around every switch edge, four per 2 µs period. Nothing lets a run trade the switching detail for speed (`GAPS.md` V13): there is no averaged model, and the swap mechanisms the spec gives (§8.2.7 generation, §8.4 templates) are refused today (`GAPS.md` L31, L32).

## Goals

- [ ] The buck has an averaged power stage with the switching stage's exact port list, and a run chooses either one with no edit of the model (Q1).
- [ ] The averaged start follows the switching start within the bounds of Q6, and takes at most one twentieth of its accepted steps under the same options.
- [ ] A 6 ms averaged run completes, with its memory bounded as `long-run` bounds it.
- [ ] The averaged stage reports, in the `checks` stream, when the run leaves the region where averaging is valid.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Generation `for`/`if` and templates in general (L31, L32), params of module type (L28) | `generics` and `params-and-io` own them; this feature uses them on the buck |
| Tool-derived or data-fitted surrogates | Q3 recommends hand-written only; `table_model` is L57 (`params-and-io`) |
| Switching implementations partway through a run | Q4 recommends refusal with `E0803` |
| Multi-rate stepping | `multi-rate` (V12); this feature's speed criterion depends on it |
| Host stimulus during a run (load steps from the host) | `host-iii` (H1) and `buck-scenarios` (U3). The long run here uses the bench's own stimulus |
| Averaged models of other converters or a library of them | Q5 recommends the buck's model only |
| A new run key | the selector is a param; `--param` and its run key exist (`msl-cli`) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The swap mechanism | Q1 C: a structural `averaged: bool = false` on `Buck` with a generation `if`; one fixture proves a template param | spec §8.2.7 and §8.4 already define both; awaiting Q1 | n - planner |
| What "the same ports" means | the averaged stage's port list equals `PowerStage(vin, vout, isense, isense_n, in hs_cmd, in ls_cmd)`: the same types, directions and order (§8.4.5); names may match too | The swap must not touch `Buck`'s connections | n - planner |
| How the averaged stage gets its duty | a reaction measures each period's high-side on-time from `hs_cmd` edges and commits the duty as a real carrier once per period; `analog` reads only that carrier, `vin` and its own nodes | Same ports, no new language; the analog side sees one change per period | n - planner |
| The current-sense outputs | carry the averaged inductor current, so cycle-by-cycle current limits cannot trip as in the switching stage | An averaged model has no peak current; SUR-05 reports the region where this matters | n - planner |
| Validity condition | continuous conduction: the averaged inductor current stays above half its computed peak-to-peak ripple; a `require` in the stage writes a failed `checks` record when it does not | DCM is where state-space averaging of a synchronous buck stops holding | n - planner |
| Accuracy bounds | Q6 A | awaiting Q6 | n - planner |
| Speed bound | the averaged start takes at most 1/20 of the switching start's accepted steps, both with multi-rate and the same options | Four switch edges per period cost the switching stage most of its steps; the averaged one changes once per period | n - planner |
| The long run | a new run `long_averaged` in `examples/buck/Piperine.toml`: the `Startup` bench, `averaged = true`, 6 ms, no `max_step`, `print_step = 5e-6` | ten starts' length; `max_step = 1e-8` of `startup` would cap it at 600 000 steps | n - planner |
| Mid-run `set` of the selector | refused with `E0803` (§8.1.24, §23.1 rule 7) | the spec's rule; awaiting Q4 | n - planner |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation: SUR-02 AC 2 (a stage whose ports differ is refused), SUR-06 AC 2 (`E0803`). Failure states: SUR-05 (a run outside the valid region reports it). Idempotency: N/A because a run is one run. Auth and rate limits: N/A, local tool. Concurrency: N/A, no new concurrency. Data lifecycle: SUR-04 AC 3 (memory bounded as `long-run` bounds it). Observability: SUR-03 AC 3 (steps and wall time printed). External dependency: N/A, no external tool. State transitions: SUR-06 (the selector cannot change during a run).

---

## User Stories

### P1: A run chooses the stage ⭐ MVP

**User Story**: As the buck's designer, I want to pick the switching or the averaged power stage per run, with the model untouched, so that a long scenario uses the fast stage and a detailed one the exact stage.

**Why P1**: The language mechanism is the gap's core.

**SUR-01** (selection)

**Acceptance Criteria**:
1. WHEN a run of `examples/buck` gives `--param Startup.buck.averaged=true` THEN the system SHALL place the averaged stage as `buck.power` and no switching stage.
2. WHEN a run gives no value for `averaged` THEN the system SHALL place the switching stage, and the run's output SHALL equal its output before this feature.
3. WHEN either stage is placed THEN the POM SHALL show `buck.power` as an instance of that stage's module, with its own children as written.
4. WHEN the selector is given THEN the system SHALL elaborate the design once, before the run, and SHALL NOT re-elaborate during it (MD-18).

**SUR-02** (the same ports)

**Acceptance Criteria**:
1. The system SHALL give the averaged stage the port list of `PowerStage`: the same types and directions in the same order.
2. IF a module placed through a template param does not satisfy its `Fn(P)` bound THEN the system SHALL refuse the design with `E0360` naming the module (§8.4.5).
3. WHEN a fixture binds a template param `stage: S` with `S: Fn(Electrical, Electrical)` once to a resistor and once to a resistor-capacitor pair, in its source THEN the system SHALL run both and give each its own operating point.

**Independent Test**: `cargo test -p piperine-api --test surrogate` (selection, POM and the template fixture), `cargo test -p piperine-cli --test buck`.

---

### P1: The averaged start follows the switching start ⭐ MVP

**User Story**: As the designer, I want the averaged stage to give the same start as the switching stage, within stated bounds, so that a long averaged run says something true about the converter.

**Why P1**: A surrogate that is fast and wrong is worse than none.

**SUR-03** (accuracy and speed of the start)

**Acceptance Criteria**:
1. WHEN `startup` runs once with each stage on a 10 ns grid THEN, for every 2 µs switching period after `en` rises, the mean of the switching run's `buck.vout` rows over the period SHALL be within 10 mV of the averaged run's `buck.vout` at the period's end.
2. WHEN both runs complete THEN the means of `buck.vout` over their last 40 µs SHALL be within 2 mV of each other, and each run SHALL meet the overshoot and settling limits of `long-run` LR-09 and LR-10.
3. WHEN both runs complete THEN the test SHALL print each run's accepted steps and wall time on standard error.
4. WHEN both runs complete with multi-rate and the same options THEN the averaged run SHALL have accepted at most one twentieth of the switching run's steps.
5. WHEN the averaged run completes THEN every record of its `checks` stream SHALL have `pass` true and its last row SHALL hold `buck.ctl.reference` 128, `mode` 2 and `pgood` true.

**Independent Test**: `cargo test -p piperine-api --test surrogate_start` (both runs, one shared each).

---

### P1: A long averaged run ⭐ MVP

**User Story**: As the designer, I want to run the buck for milliseconds, so that slow behaviour (a long ramp, thermal-length settling) can be seen at all.

**Why P1**: "For runs longer than a start" is the gap's wording.

**SUR-04** (the long run)

**Acceptance Criteria**:
1. WHEN `piperine run long_averaged` runs in `examples/buck` THEN the system SHALL print 1 201 rows, from 0 to 6 ms on its 5 µs grid.
2. WHEN that run completes THEN its last row's `buck.vout` SHALL be within 0.8 V ± 10 mV and every `checks` record SHALL have `pass` true.
3. WHEN that run completes through `piperine-api` THEN its peak live heap SHALL be at most 1 GiB, measured as `long-run` LR-12 measures it.
4. WHEN that run completes THEN it SHALL have needed fewer accepted steps than the transient's default step budget.

**Independent Test**: `cargo test -p piperine-cli --test buck` (the rows) and `cargo test -p piperine-api --test surrogate_long` (heap, steps).

---

### P2: The averaged stage says when it is not valid

**User Story**: As the designer, I want the averaged stage to flag a run that leaves the region where averaging holds, so that I never trust a number it cannot give.

**Why P2**: The start never leaves the region; light loads and current limits do.

**SUR-05** (validity)

**Acceptance Criteria**:
1. IF the averaged inductor current falls below half its computed peak-to-peak ripple THEN the averaged stage SHALL write a `checks` record with `pass` false naming discontinuous conduction.
2. IF the averaged inductor current exceeds the current-limit threshold of `examples/buck/SPEC.md` §8.2 THEN the averaged stage SHALL write a `checks` record with `pass` false naming the cycle-by-cycle limit it does not model.
3. WHEN the load of a run keeps the converter in continuous conduction below the limit THEN the averaged stage SHALL write no failed record.

**Independent Test**: a bench with a 100 Ω load (discontinuous) and one with the nominal 6.6 Ω, run with the averaged stage.

---

### P2: The selector is fixed during a run

**User Story**: As a host author, I want a `set` of the selector during a run refused by name, so that a run never mixes two implementations silently.

**Why P2**: The spec already refuses it; the host code for `E0803` is `host-ii` H9.

**SUR-06** (refusal)

**Acceptance Criteria**:
1. The system SHALL treat `averaged` as a structural param (§8.1.2).
2. IF a host `set` of `averaged` is scheduled during a run THEN the system SHALL refuse it with `E0803` naming the param, and the run SHALL not start.

**Independent Test**: a `piperine-api` test scheduling the `set` at 10 µs.

---

### P3: The documents say what is true

**User Story**: As a reader of the buck's README, `GAPS.md` and the Book, I want the averaged stage and its limits described.

**Why P3**: No behaviour depends on it.

**SUR-07** (documents)

**Acceptance Criteria**:
1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list V13 and `ROADMAP.md` SHALL mark surrogate models delivered with its commit.
2. WHEN this feature merges THEN `examples/buck/README.md` SHALL describe both stages, the selector, the validity records and the bounds of SUR-03.
3. The system SHALL keep every command-line listing of the Book equal to the binary's output.

**Independent Test**: `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN `en` is low for the whole run THEN the averaged stage SHALL give `vout` 0 V and commit duty 0, as the switching stage does.
- WHEN no `hs_cmd` edge occurs in a period THEN the averaged stage SHALL commit the duty of that period as 0 or 1 from the level `hs_cmd` held.
- WHEN `hs_cmd` and `ls_cmd` are both low (dead time) THEN the averaged stage SHALL count that time as neither switch on.
- IF `hs_cmd` is unknown THEN the averaged stage SHALL commit an unknown duty, and the analog side SHALL read it under the rules of §3.3.5 as the switching stage reads an unknown gate.

---

## Gaps closed

| `GAPS.md` id | Requirement(s) | How |
| ------------ | -------------- | --- |
| V13 | SUR-01 to SUR-06 | SUR-01 and SUR-02 give the swap through the spec's own mechanisms, SUR-03 bounds the averaged start against the switching one and cuts its steps twentyfold, SUR-04 runs 6 ms, SUR-05 flags where the averaging fails, SUR-06 keeps `E0803` |

---

## Dependencies

- `generics` (L31 generation `if`, L32 templates and `Fn(P)` bounds) and `params-and-io` (L28, a param of module type): the swap mechanisms.
- `multi-rate` (V12): SUR-03 AC 4. Without it the 200 MHz `Dpwm` lands at every edge whichever stage is placed, because `count` reaches the probing ADC.
- `host-ii` (H9, `E0803`): SUR-06 AC 2.
- `long-run` (delivered): the streamed grid and the heap measure of SUR-04.

## Size

M: about 12 tasks. The averaged stage (3), selection and POM (2), the template fixture (1), the start comparison (2), the long run (2), validity records (1), documents (1).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SUR-01 | P1: a run chooses the stage | - | Pending |
| SUR-02 | P1: a run chooses the stage | - | Pending |
| SUR-03 | P1: the averaged start follows the switching start | - | Pending |
| SUR-04 | P1: a long averaged run | - | Pending |
| SUR-05 | P2: the averaged stage says when it is not valid | - | Pending |
| SUR-06 | P2: the selector is fixed during a run | - | Pending |
| SUR-07 | P3: the documents say what is true | - | Pending |

**Coverage:** 7 total, 0 mapped to tasks, 7 unmapped (spec stage).

---

## Sources

- The user, 2026-11-17: "gostaria que abrisse feature e escrevesse a spec para todos esses gaps… Preciso que TODOS os gaps estejam endereçados"; earlier, surrogate models are "melhoria futura… bem depois".
- `GAPS.md` §6 V13 and V12; §4 L28, L31, L32.
- `docs/spec/part_iii_structure.md` §8.1.2, §8.1.24, §8.2.7, §8.4 (templates and `Fn(P)`), `part_viii_host.md` §23.1 rule 7 (`E0803`).
- `examples/buck/src/power.msl` (`PowerStage`'s ports), `src/top.msl`, `src/dpwm.msl`, `SPEC.md` §2, §8.2, §10.
- `.specs/features/long-run/spec.md` LR-09 to LR-12; `ROADMAP.md` queue item 3c.

---

## Success Criteria

- [ ] `--param Startup.buck.averaged=true` places the averaged stage; the default run is unchanged.
- [ ] The averaged start stays within 10 mV of the switching start's period means and takes at most 1/20 of its steps.
- [ ] `piperine run long_averaged` prints 6 ms of the buck with every check passing.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.
