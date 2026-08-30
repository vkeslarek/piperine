# lrm-conformance-wave3 Validation

**Date**: 2026-08-30
**Spec**: `.specs/features/lrm-conformance-wave3/spec.md`
**Diff range**: `d34a20d8..a3e351a2` (55 tasks, 8 phases)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

All 55 tasks (T1–T55) have a matching commit in the diff range, verified by
exact `Commit:` subject-line match against `git log --oneline d34a20d8..HEAD`
and spot-checked diffs (T4, T9, T13, T49, T52, T55 read in full). No task is
blocked or missing a commit.

`tasks.md` has exactly **2** unchecked `- [ ]` boxes out of 254 total
checkboxes (confirmed by direct grep, not taken on trust):

| Task | Unchecked item | Disclosure verified |
|---|---|---|
| T12 | `$readmemh`/`$readmemb` into a declared memory | Commit `46b092d6` body + `tasks.md:401-413` state, in detail, that `SystemTaskKind::Readmemh` is recognized but `emit/stmt.rs::S::TaskCall` has no dispatch arm at all — a real, checkable claim (confirmed: `piperine-lang/src/eval/tasks.rs` has the enum variant, `emit/stmt.rs`'s dispatch has no `"readmemh"`/`"readmemb"` arm). Honest. |
| T14 | PLA task family (`$async$and$array` et al.) real implementation | Commit `218f03b5` body + `tasks.md:471-477` explain the loud-rejection choice was one of two spec-sanctioned outcomes (P3 AC2 explicitly allows either); ties the same missing-JIT-trampoline gap back to T12. `docs/spec/part_iii_conformance.md:93` carries the same reason. `crates/piperine-codegen/tests/vams_pla_tasks_rejected.rs` (99 lines) pins all 16 names failing loud, name-specific (not a blanket catch-all). Honest. |

Both are genuinely disclosed, not swept under the rug — confirmed by reading
the actual commit bodies and cross-checking the technical claim (no dispatch
arm exists) directly in source, not just trusting the prose.

---

## Spec-Anchored Acceptance Criteria

### P1: Vectors are a real data type (MVP)

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| AC1 vector declaration | single POM net, 4 addressable bits | `crates/piperine-lang/tests/vams_vector_declarations.rs:30-41` (`NetType::Array(_,4)`, single `Wire`); `piperine-codegen/src/resolve/pom/structure.rs:304-342` (per-bit nodes `bus[0]..bus[3]`) | ✅ PASS |
| AC2 literal-index contribution stamps only that bit | `crates/piperine-codegen/tests/vams_vector_analog_access.rs:72-116` — asserts `jac[bus0*n+bus0]==1e-3` and bus[1..3] rows/cross-terms are exactly zero | ✅ PASS |
| AC3 genvar loop, LRM §5.5.2 DAC fixture | each iteration → own bit | `crates/piperine-codegen/tests/vams_genvar_analog_access.rs:76-115` proves per-bit resolution for weights 1/2/4/8, but the file's own header discloses this is *adapted* from §5.9.3's for-loop example, not the verbatim §5.5.2 DAC fixture the AC names | ⚠️ Spec-precision gap (mechanism proven; exact fixture claim not literal) |
| AC4 `reg [3:0] q <= 4'b0101` exact IEEE bit order | `vams_vector_digital_access.rs:421-432` asserts q[0]=1,q[1]=0,q[2]=1,q[3]=0 | ✅ PASS |
| AC5 part-select/bit-select exact value | `vams_vector_digital_access.rs:238-253`, `b[7:4]=1011` checked bit-for-bit | ✅ PASS |
| AC6 memory indexed read/write | `vams_vector_digital_access.rs:604-618`, word round-trip, other words untouched | ✅ PASS |
| AC7 vector port bit-for-bit + width-mismatch citing §6.5.7.1 | `tests/vams_vector_port_binding_e2e.rs:123-139` (8-bit round trip) and `:147-168` (4-bit-on-8-bit fails citing 6.5.7.1) | ✅ PASS |
| Edge: non-constant range bounds fail citing §3.6.3 | `crates/piperine-lang/tests/vams_vector_declarations.rs:76-81` | ✅ PASS |
| Edge: out-of-range index names net/index/bounds | `vams_vector_declarations.rs:204-218` | ✅ PASS |
| Edge: zero-width/reversed range — bit order preserved + zero-width fails loud | `tasks.md:259-262` (T6) discloses the width formula `|msb-lsb|+1` can never yield 0, so no fail-loud path/test exists for "zero-width"; bit-order preservation as a distinct property is likewise untested (T1's own done-when: "order itself is not yet a distinct field on `NetType::Array`") | ❌ GAP (structurally moot for zero-width given the formula, but bit-order preservation is a real, distinct, untested claim) |

**Discrimination sensor for this story** (see below): the out-of-range
boundary check has a **surviving mutant** — see Sensor section. This
directly corroborates the edge-case gap above: nobody tests the boundary
(`idx == width`), only comfortably-out-of-range indices.

### P2 (axis A): Nothing parses and vanishes (specify/config)

| Criterion | `file:line` + assertion | Result |
|---|---|---|
| AC1 path delay → `PathDelay`, observable transition | `crates/piperine-codegen/tests/specify_path_delays.rs:42-84` | ✅ PASS |
| AC2 `$setup` → `TimingCheckTable`, violation reported | `crates/piperine-solver/tests/specify_timing_check_enforcement.rs:89+` | ✅ PASS |
| AC3 unlowerable specify fails loud, no unread span | `crates/piperine-lang/tests/vams_specify.rs:210-297` | ✅ PASS |
| AC4 `config` fails loud citing A.1.5 | `crates/piperine-lang/tests/vams_stop_vanishing.rs:18-35` | ✅ PASS |
| AC5 drive strengths reach `DriverTable`/Table 7-3 | **Not implemented.** `crates/piperine-lang/tests/t43_final_coverage_gaps.rs:58-61` pins `assign (strong1, weak0) w = r;` as a parse error (declared limitation), and `docs/spec/part_iii_conformance.md:44,155` lists it as an open gap. No task in T1–T55 (grep of tasks.md for "drive strength"/"DriverTable"/"Table 7-3" in the specify-related tasks) targets LRMW3-09 at all — it was never scheduled, only later swept into the "declared limitation" bucket by the coverage backfill. | ❌ GAP against the literal AC text (spec.md's AC5 states a bare SHALL with no fail-loud escape, unlike AC3's explicit either/or) |

### P2: Discipline, nature, and attribute surface

| Criterion | `file:line` + assertion | Result |
|---|---|---|
| AC1 §3.6.2.5 override, numerically observable | `discipline_attribute_override.rs:94-123` — same step accepted under loose abstol, rejected under tightened one | ✅ PASS |
| AC2 §3.6.2.6 nature derived from discipline binding | `nature_derived_from_discipline.rs:49-96` | ✅ PASS |
| AC3 §3.6.2.7 user attribute, not a parse error | `discipline_user_attributes.rs:45-70` | ✅ PASS |
| AC4 desc/units/op/multiplicity accepted + surface on host descriptor | Acceptance/POM half: `standard_attributes.rs:34-56` ✅. Host-descriptor half: **gap** — `crates/piperine-api/src/model/descriptors.rs:113-117`'s `Param` struct carries only `name`/`ty`/`default`, no `desc`/`units`/`op`/`multiplicity` fields, and no test exercises them through `piperine-api` | ⚠️ PARTIAL — half the AC unmet |
| AC5 §3.10 out-of-module precedence, conflict fails loud | `out_of_module_discipline.rs:72-133` | ✅ PASS |
| AC6 §5.5.3 `a.potential.abstol` read | `nature_attribute_access.rs:78-120` | ✅ PASS |

### P2: Branch and contribution completeness — all 5 ACs PASS

`port_branches.rs:42-90,175`; `hierarchical_contributions.rs:52-80,109-171`;
`hierarchical_probe_reads.rs:93-124`;
`indirect_direct_contribution_conflict.rs:47-95`.

### P2: Illegal analog constructs fail loud

| Criterion | Result |
|---|---|
| AC1 analog op under non-constant conditional, §4.5.15 | ⚠️ **Deliberate, documented deviation** — the `if`/`case`/`?:` conditional bullet is NOT enforced. `crates/piperine-lang/src/elab/typecheck.rs:123-139,211-232` carries a detailed `SPEC_DEVIATION (T33/LRMW3-17)` comment citing `headers/spice/mos.va:2005-2011` (`if (mode == 1.0) begin ... ddt(V(g,sp)) ... end` with `mode` a genuinely non-constant `real`) as the reason enforcement would break real, frozen compact models. Verified directly against the header file — the deviation is evidence-backed, not hand-waved. `limexp` is separately, similarly exempted with the same standard of evidence (`diode.va:350,359,377`). |
| AC2 op inside event-triggered/repeat/while/non-genvar-for/function | ✅ PASS — `analog_operator_placement.rs:39-145` |
| AC3 event control under non-constant conditional, §5.8.4 | ✅ PASS — `conditional_statement_restrictions.rs:30-105` |
| AC4 NaN/±inf contribution, typed error naming branch | ✅ PASS — `failure_rules.rs:160-179`, cites §7.3.2.1, names the device |

### P2 (axis B): Piperine's own surface is proven, not asserted

| Criterion | Result |
|---|---|
| AC1 every axis-B surface resolves to a verdict | ✅ PASS — `tests/spec_section_coverage.rs::covered_and_excluded_partition_the_spec`, 284 headings, 15 Covered + 269 DeclaredLimitation + 0 unverdicted, independently confirmed via `spec_coverage_exclusions.toml` entry count (`declared_limitation` = 269) plus 15 unique `SPEC-COVERS` ids scraped from 18 declaring files |
| AC2 gate fails if a claim's test is deleted | ✅ PASS — proven by discrimination sensor below (deleted `part_i#9`'s only declaration → gate goes red naming exactly that heading) |
| AC3 selector axis returns matches or fails loud | ✅ PASS — `constraint::` genuinely implemented (`eval.rs:255-269`, walks `Module::constraints()`); the other four axes independently confirmed (via `git show d34a20d8:...`) to have *already* raised `SelectorError::AxisNotImplemented` before this feature started — spec.md's original B1 claim ("share one arm that matches nothing") was itself stale, a fact this feature correctly discovered and corrected rather than uncritically re-implementing |
| AC4 host asymmetry declared **and** asserted | ✅ PASS — `tests/host_parity.rs` (T51): probes the Python facade for every declared Rust-only name, plus a synthetic-regression self-check (`Waveform.min` correctly reported "found") proving the probe discriminates |
| AC5 unreachable POM variant removed or proven unreachable | ✅ PASS — `pom_retired_variants_unreachable.rs` (T50): mechanical scan + live traversal for `Kind::{Enum,Bundle,Capability}`, `Value::{EnumVariant,Tuple,Record}`, 9 `ElabErrorKind` variants; also **corrects** spec.md's own B3 claim (`Value::List` is in fact reachable via the `'{...}` extension) rather than uncritically accepting it |
| AC6 new extension lands with test + spec line | ✅ PASS (gate, going forward) — `tests/spec_extension_gate.rs` (T48), a registry cross-checked against `part_iii_conformance.md` §6's live table, proven to fail on an orphaned row (mutation proof recorded in commit `a780c073`'s body) |

**Axis-B exclusion-reason quality** (spec.md explicitly asks this be
checked): all 269 `declared_limitation` entries in
`spec_coverage_exclusions.toml` use one **identical, verbatim** boilerplate
string ("Axis-B backfill pending... tracked as an honest axis-B gap, not
asserted implemented"). Investigated further: this is *not* a case of
throwing away available specificity — the surfaces spec.md's own B1–B10
table gave a specific reason for (B1 constraint axis, B2/B3/B4 retired
variants, B5 host asymmetry, B6 `piperine-ir` doc, B7 include aliases) were
all genuinely **promoted out of the exclusions file** to `Covered` by
T48–T52 (confirmed: `part_iv#9`, `part_iv#5`, `part_iv#10`, `appendix_c#3`,
`part_ix#1`, `part_i#9` are absent from `spec_coverage_exclusions.toml`).
The 269 remaining are genuinely generic-because-untouched, not
generic-in-place-of-known-specifics. Recorded as an informational finding,
not a gap — but worth watching in a future wave: a boilerplate reason is
only honest as long as no more specific reason is knowable, and 269 is a
lot of headings to re-verify that for.

### P3: Paramset output variables, PLA tasks, coverage gate

| Criterion | Result |
|---|---|
| AC1 paramset output variable | ✅ PASS (not independently re-verified beyond task-commit audit; T28 commit `908b861e` matches its stated scope) |
| AC2 PLA tasks implemented or fail loud | ✅ PASS — loud-rejection chosen and disclosed (see Task Completion table); `vams_pla_tasks_rejected.rs` pins name-specific rejection |
| AC3 coverage gate — every LRM heading has a verdict | ✅ PASS — `tests/lrm_section_coverage.rs`, 612 headings, 588 Covered + 24 Excluded + 0 unverdicted (T55 commit's own numbers, reproduced independently by the full `cargo test --workspace` run: 3074 passed, 0 failed) |

**T37** (§9.5.9 rejected-iteration file-I/O rollback): declared limitation,
not implemented — `crates/piperine-codegen/src/emit/runtime_io.rs:340-358,477-510`
pins the current non-conformant behavior explicitly rather than silently
leaving it unaddressed. Matches `part_iii_conformance.md:43-44,153-154`.

**T40** (§7.8.5.1 gate-primitive connect-module naming): declared
limitation for gate terminals specifically (`tests/connect_module_naming_and_supply.rs:44-61`);
§7.8.6 (supply-sensitive connect module) *is* implemented and tested
(`:88+`). Matches the conformance doc.

**Status**: ❌ Gaps present (2 real: AC5 drive strengths unimplemented and
unscheduled; host-descriptor half of the standard-attributes AC unmet) +
1 confirmed-genuine deliberate deviation (§4.5.15 conditional) + 1
spec-precision gap (AC3's fixture) + 1 edge-case gap (bit-order
preservation untested) — none of these block the overall PASS verdict
below because all are honestly disclosed in `docs/spec/part_iii_conformance.md`
except the drive-strength AC's complete absence from the task breakdown,
which is a real planning gap (see Ranked Gaps).

---

## Discrimination Sensor

All three mutations were injected directly in the working tree, tested, and
reverted via `git checkout --` (confirmed empty diff afterward). No stash
was needed; the pre-existing unrelated dirty state in the repo (docs/spec
rewrites, `.specs/` bookkeeping — present before this session started) was
left untouched throughout.

| # | File:line | Mutation | Phase | Killed? |
|---|---|---|---|---|
| 1 | `crates/piperine-lang/src/elab/lower/vams.rs:2666` | `if idx >= width` → `if idx > width` (off-by-one on the vector out-of-range check) | Phase 1/2 — vector bounds | ❌ **SURVIVED** — `cargo test -p piperine-lang` (19/19 in `vams_vector_declarations.rs`, full crate) still green. No test exercises the exact boundary `idx == width` (e.g. `bus[4]` on a `[0:3]` net); every out-of-range test uses a comfortably-invalid index (`bus[9]`). |
| 2 | `crates/piperine-solver/src/digital/state.rs:190` | `if separation < limit.limit` → `if separation > limit.limit` (inverted timing-check violation condition) | Phase 3 — specify/timing-check enforcement | ✅ Killed — `cargo test -p piperine-solver --test specify_timing_check_enforcement`: 3 of 5 tests failed (compliant run now falsely reports a violation, the real violation goes unreported, strict-posture abort test fails) |
| 3 | `crates/piperine-lang/tests/vams_include_legacy_aliases.rs:11` | Deleted the file's only `SPEC-COVERS: part_i#9` declaration (simulating "a spec claim's test disappears") | Phase 6/7 — axis-B coverage gate | ✅ Killed — `cargo test -p piperine --test spec_section_coverage`: `covered_and_excluded_partition_the_spec` fails, naming exactly `part_i#9` as newly uncovered |

**Sensor depth**: lightweight (3 mutations, one per phase family as
requested).
**Result**: 2/3 killed, **1 survived** → this is a real, actionable gap in
the test suite, not a hypothetical one. It directly explains the missing
"zero-width... bit order preserved" edge-case coverage found in the AC
check above: the vector bounds-check family is undertested at its own
boundary.

---

## Code Quality

| Principle | Status |
|---|---|
| No features beyond what was asked | ✅ — commits stay within their stated task scope (e.g. T13's "wider than originally scoped" note in tasks.md is itself disclosed, not silent scope creep) |
| No abstractions for single-use code | ✅ |
| Matches existing patterns/style | ✅ — new gates (`spec_section_coverage.rs`) explicitly mirror `lrm_section_coverage.rs`'s shape; new element-node naming (T4, `032787a3`) is a single owning helper as designed |
| Spot-checked architecturally significant commits (T4, T13, T49, T52, T55) | ✅ — diffs match their commit-body claims exactly on file lists and behavior; T49/T52's factual claims about pre-existing code (`AxisNotImplemented` already present, `EventSystem` already `pub(super)`) were independently re-derived from `git show d34a20d8:...` and source reads, not taken on trust, and confirmed true |
| Spec-anchored outcome check | ✅ for the large majority; 3 gaps and 1 deviation cited above, all with evidence |
| Every test maps to a spec AC/edge case/Done-when | ✅ — no unclaimed test files found during sampling |
| Documented guidelines followed | CLAUDE.md's fail-loud contract (no construct reaches the POM as an unread span) — confirmed for `specify`/`config`; drive-strength stays a **parse error** (loud), not a silent accept, so the letter of fail-loud is intact even where the AC's positive behavior (reaching `DriverTable`) is not |

---

## Edge Cases

- [x] Non-elaboration-constant vector range bounds fail loud citing §3.6.3
- [x] Out-of-range vector index names net/index/bounds
- [ ] Zero-width/reversed range: bit order preservation is a real, distinct, untested claim (zero-width itself is structurally unreachable given the width formula, disclosed honestly in T6)
- [x] `specify` path naming a non-port terminal fails loud citing A.7.3 (not independently re-verified beyond task-commit audit)
- [x] Nature-attribute read of an undeclared attribute fails loud listing declared attributes (not independently re-verified beyond task-commit audit)
- [x] Two conflicting out-of-module discipline declarations fail loud naming both spans (`out_of_module_discipline.rs`)

---

## Gate Check

- **Build gate**: `cargo build --workspace` — **0 warnings**, exit 0.
- **Test gate**: `cargo test --workspace --no-fail-fast` — **3074 passed, 0 failed**, 472 test binaries, run independently by this Verifier (not carried forward from the T55 commit's own claim, though it matches exactly).
- Two pre-existing compiler warnings appeared during test compilation (`margin_tran_points.rs` dead_code, `soa_spice_models.rs` unused_mut) — confirmed via `git log -- <file>` that **neither file was touched in this feature's diff range** (`d34a20d8..HEAD`); both predate wave 3 and are not a regression from this work.
- No `#[ignore]`d test discovered in the diff; both previously-known-red gate targets (`lrm_section_coverage.rs`, and the newly-created `spec_section_coverage.rs`) are green.
- Test count before/after: not independently measured at `d34a20d8` (out of scope for a diff-scoped review of this size); the T55 commit body's own before/after framing ("3074 tests... across 472 test binaries") was independently reproduced bit-for-bit.

---

## Fix Plans

### Fix 1: Vector out-of-range boundary is untested (surviving mutant)

- **Root cause**: `validate_indexed_net_refs` (`crates/piperine-lang/src/elab/lower/vams.rs:2666`) uses `idx >= width`; every existing test uses a comfortably-out-of-range index (`bus[9]` on a 4-bit net), never the boundary `idx == width` (`bus[4]` on a `[0:3]` net).
- **Fix task**: Add a boundary-index test to `crates/piperine-lang/tests/vams_vector_declarations.rs` asserting `bus[4]` on `electrical [0:3] bus;` fails loud with the same message shape as the existing out-of-range test. Repeat for the analog/digital terminal-indexing resolvers (T7/T9's own separate resolution paths) if they have independent bounds checks with the same gap.
- **Priority**: Minor (the current behavior is already correct; only the regression-test coverage is missing).

### Fix 2: Drive-strength AC (LRMW3-09) was never scheduled

- **Root cause**: spec.md's P2 axis-A AC5 states a bare SHALL ("the strengths SHALL reach the DriverTable and participate in Table 7-3 resolution") with no fail-loud alternative, unlike its sibling AC3. No task in T1–T55 targets it; it surfaces only retroactively as a declared limitation via the coverage backfill (T43-era), which is an honest bookkeeping move but does not close the AC.
- **Fix task**: Either implement `assign (strong1, weak0) w = r;` → `DriverTable` wiring (LRMW3-09), or — if genuinely out of scope for a future wave — amend spec.md's AC5 itself to allow the fail-loud alternative (matching how AC3 is worded), so the written AC and the shipped behavior agree.
- **Priority**: Minor/Major depending on project intent — currently a documented-but-unplanned gap, not a silent one.

### Fix 3: Standard attributes don't reach the host descriptor

- **Root cause**: `crates/piperine-api/src/model/descriptors.rs:113-117`'s `Param` struct has no `desc`/`units`/`op`/`multiplicity` fields; T27 closed only the parse/POM half of the AC.
- **Fix task**: Add the four fields to the relevant descriptor type(s) and thread them from the POM attribute lookup already built by T27, with a test reading them back through `piperine-api`.
- **Priority**: Minor (host surface is additive, not correctness-affecting).

---

## Requirement Traceability Update

`spec.md`'s Requirement Traceability table (lines 412-455) was left with
every row still marked "Pending" — this feature's tasks.md/commits do the
real closure work, but nobody updated spec.md's own table to reflect it, so
the table itself understates completion. Recommended (not performed here,
out of a Verifier's read-only scope): update LRMW3-01 through -08,
-10 through -20 (except -09), -21 through -32, and PSW3-01 through -10 to
"✅ Verified"; leave **LRMW3-09** as "❌ Needs Fix" (drive strengths,
Fix 2 above) — every other row's evidence is in this report.

---

## Post-Fix Update (2026-08-30)

All 3 gaps below were routed as fix tasks and independently re-verified
(fresh sub-agent, author ≠ verifier) after implementation:

- **Fix 1** (vector index-out-of-range boundary, `idx == width`) — DONE,
  `c03b581c`. Re-verification independently reproduced the mutation
  (`idx >= width` → `idx > width`) against the new test and confirmed it
  is now killed.
- **Fix 2** (LRMW3-09, drive strengths reach `DriverTable`/Table 7-3) —
  DONE, `e3434412`. Re-verification confirmed the new test asserts actual
  resolved `LogicValue` outcomes, not just successful parsing, and
  independently verified the along-the-way bug fix
  (`PiperineDevice::init`/`comb_phase` was dropping strength on relay) is
  real. A separate, pre-existing, disclosed-not-fixed bug (bare
  `wire`/`reg` misclassifying as analog in the continuous-assign domain
  check, `elab/lower/vams.rs`) was confirmed genuine and out of this
  fix's scope — the new test correctly works around it with an explicit
  digital-discipline fixture rather than hiding it.
- **Fix 3** (standard attributes → host descriptor) — DONE, `e4613ba0`.
  Re-verification confirmed `desc`/`units`/`op`/`multiplicity` are
  populated end-to-end from an authored attribute-instance declaration
  through to `piperine-api`'s `Param` descriptor, asserted by exact
  string value (plus a control case asserting `None` when absent).

Full workspace (`cargo build --workspace` zero warnings,
`cargo test --workspace --no-fail-fast`) reproduced green on this HEAD by
both the fix implementer and the re-verifier independently.

**Revised overall verdict: PASS ✅** — all three ranked gaps closed with
evidence, zero known regressions.

---

## Summary

**Overall**: ✅ PASS (post-fix) — see "Post-Fix Update" above; the
original pass below reflects the state before the 3 gaps were closed.

**Spec-anchored check**: ~38/42 ACs across the sampled stories matched
their spec-defined outcome exactly; 3 real gaps (drive strengths
unimplemented+unscheduled, host-descriptor half of standard attributes,
bit-order-preservation edge case untested) + 1 spec-precision gap (AC3's
fixture) + 1 confirmed-genuine, evidence-backed deliberate deviation
(§4.5.15 conditional restriction).

**Gate**: cargo build 0 warnings; cargo test 3074 passed, 0 failed
(reproduced independently).

**Sensor**: 2/3 mutations killed, 1 survived (vector out-of-range boundary
check — see Fix 1).

**What works**: Vectors are real end to end (declaration, analog/digital
access, part-selects, memories, port binding) with strong evidence;
`specify`/`config` no longer parse-and-vanish; discipline/nature/branch/
contribution extensions all check out; both coverage gates (612 LRM
headings, 284 spec headings) are genuinely green, genuinely built from
scratch for axis B, and both were proven mutation-fallible during
implementation (reproduced independently for axis B in this review) and
by this review's own sensor; T49's and T52's more surprising claims
("already fails loud", "already pub(super)") were independently re-derived
from source/history and hold up.

**Issues found**: See Fix Plans 1–3 above. None are silent — all three
surfaces are either disclosed in `docs/spec/part_iii_conformance.md` or
(Fix 1) simply missing a regression test for already-correct behavior.

**Next steps**: Route Fix 1 (test-only, low effort) and Fix 3
(small, additive) as immediate follow-ups. Fix 2 (drive strengths) is a
product decision — implement, or reconcile spec.md's AC5 wording with the
declared-limitation outcome the team already shipped.
