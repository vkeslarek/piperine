# Validation Report — `hierarchy-flattening`

**Date**: 2026-07-22
**Verifier**: Independent (re-derived from spec; not the author)
**Spec**: `.specs/features/hierarchy-flattening/spec.md`
**Design**: `.specs/features/hierarchy-flattening/design.md`
**Tasks**: `.specs/features/hierarchy-flattening/tasks.md`
**Diff range**: `0169455..HEAD` (commits `7c018c2`..`a78db31`, 10 commits, 22 files, +2250/-34)
**Reading mode**: evidence-or-zero — every claim cites a `file:line` or is marked GAP.

---

## Task Completion

| Task | Commit | Status | Evidence |
| ---- | ------ | ------ | -------- |
| T1 `Design::flat_modules` + accessor + serde skip | `7c018c2` | ✅ | `crates/piperine-lang/src/pom/design.rs:101,206,604`; serde test `pom_serde.rs:85` |
| T2 driver + leaf test + cycle guard | `826656e` | ✅ | `flatten.rs:38-101` (driver), `flatten.rs:49-51` (leaf), `flatten.rs:68-72` (cycle); tests `flatten.rs:313-417` |
| T3 inline + remap + fail-loud guards | `4059ac3` | ✅ | `flatten.rs:124-204`; tests `flatten.rs:438-735` |
| T4 wire `FlattenHierarchy` into `PASSES` | `7b974cd` | ✅ | `passes.rs:35`; tests `elab.rs:737-813` |
| T5 codegen reads `flat_module(root)` | `4d74065` | ✅ | `circuit.rs:125-127,166`; test `flatten_hierarchy.rs:57-75` |
| T6 `with_overrides_applied` retargets | `8108b59` | ✅ | `design.rs:215-221,485`; tests `flatten_hierarchy.rs:151-252` |
| T7 author `urc.phdl` | `c15048d` | ⚠️ spec-precision gap | `headers/spice/urc.phdl` (fixed-N modules, not parametric `urc[N]`) |
| T8 ngspice cross-check `urc` ≥3 lumps | `a8ef83a` | ⚠️ spec-precision gap | `ngspice_validation.rs:602-615` (passes, but cross-checks a discrete-R/C twin not ngspice's URC model) |
| T9 monomorph compile_count guards | `d5d44ec` | ⚠️ spec-precision gap | `urc_compile_count.rs:73-179` (checks leaf-kernel count stability, not the spec's "distinct `urc__N` kernels") |
| T10 docs ROADMAP/STATE | `a78db31` | ✅ | `ROADMAP.md`, `.specs/STATE.md` updates |

---

## Spec-Anchored Acceptance Criteria

| ID | WHEN X THEN Y (spec outcome) | `file:line` + assertion expression | Result |
| -- | ---------------------------- | ---------------------------------- | ------ |
| **FLAT-01 / P1.1** | WHEN top instantiates a mid-level module THEN the pass inlines its instances recursively so `Design` handed to `CircuitCompiler` contains only leaf devices — the nested-hierarchy guard becomes unreachable for well-formed input | `crates/piperine-codegen/tests/flatten_hierarchy.rs:57-75` (`mid_level_module_builds_through_codegen` — `Seg` builds, labels `["v1","x.r1","x.r2","rl"]`); `crates/piperine-lang/tests/elab.rs:751-769` (`flatten_pass_populates_flat_modules_after_elaboration` — flat Top has `["x.r1","x.r2"]`); guard at `crates/piperine-codegen/src/device/builder.rs:156` (spec said `circuit.rs:389` — stale ref, actual location corrected) | **PASS** |
| **FLAT-02 / P1.2** | WHEN a sub-instance port connects to a parent net (and its internal wire only within the sub-module) THEN flattening binds the port to the parent net and lifts the wire to a fresh collision-free top-level net | `crates/piperine-lang/src/elab/lower/flatten.rs:462-473` (`assert_eq!(r1.ports[0].net, "a", …)` and `assert_eq!(r1.ports[1].net, "x.mid", …)`); `crates/piperine-codegen/tests/flatten_hierarchy.rs:71-74` (`info.nets.contains_key("x.mid")`); ground passthrough `flatten.rs:593-623`; fail-loud `flatten.rs:626-702` (dangling, gap-3 indexed) | **PASS** |
| **FLAT-03 / P1.3** | WHEN two instances of the same sub-module coexist THEN their inlined labels SHALL be distinct + deterministic and remain addressable for host overrides | `flatten.rs:477-509` (`inline_two_siblings_produce_distinct_labels_and_wires` — labels `["x.r1","x.r2","y.r1","y.r2"]`, both `x.mid`/`y.mid` lifted); `flatten.rs:511-555` (3-level composition `seg0.rc.r1`); `flatten_hierarchy.rs:151-220` (`override_on_spliced_label_restamps_flat_instance` — `x.r1` restamp shifts v(out)); `flatten_hierarchy.rs:225-252` (unknown label fails loud) | **PASS** |
| **FLAT-04 / P1.4** | WHEN `urc` is authored as `urc[N]` + `StructuralFor` over N segments and simulated THEN the result SHALL match ngspice's `urc` device within tolerance at ≥3 lump values | `tests/ngspice_validation.rs:602-615` (urc_lump2/5/10 PASS vs ngspice at 4.167V/3.333V/2.500V); `crates/piperine-lang/tests/urc_flatten.rs:41-143` (3-level inlining for fixed-N modules) — **but** `urc.phdl` uses fixed-N `urc2`/`urc5`/`urc10` not parametric `urc[N]`, and the ngspice twin is a hand-built R/C ladder not ngspice's URC code model | **PASS (outcome) / GAP (spec-precision)** — see §Spec-Precision Gaps |
| **FLAT-05 / P2.1** | WHEN a circuit instantiates `urc[3]` and `urc[7]` THEN codegen compiles TWO distinct kernels (`urc__3`,`urc__7`) — `compile_count` delta of 2 | `tests/urc_compile_count.rs:97-101` asserts `delta_5 == delta_10` (same leaf-kernel count) — NOT the spec's "two distinct monomorph kernels" because urc5/urc10 are non-generic modules | **GAP (spec-precision)** — invariant held is weaker than spec asked |
| **FLAT-06 / P2.2** | WHEN a circuit instantiates `urc[3]` twice with different non-structural params THEN codegen compiles ONE shared kernel and restamps both | `urc_compile_count.rs:115-128` verifies the restamp path is loud on a flattened label; `urc_compile_count.rs:147-153` verifies `per_build == delta_5` (one build's worth of kernels per build) | **PASS (weaker form)** — the shared-kernel invariant is checked via per-build stability, not the spec's `urc__3`-named twin |
| **FLAT-07 / P2.3** | WHEN a sweep varies a non-structural param across many points THEN it restamps with zero additional compiles | `urc_compile_count.rs:130-167` (`sweep_compiles == per_build` for a 20-point sweep, NOT 20·per_build); `urc_compile_count.rs:170-178` (every sweep point matches staged-single-build reference within 1e-3 reltol) | **PASS** |

---

## Edge Cases Checklist

| Edge case (spec §Edge Cases) | Test | Result |
| ---------------------------- | ---- | ------ |
| `N = 0` fails loud | none in feature tests — spec defers to upstream const-eval/mono (out of MVP scope) | **DEFERRED** (upstream) |
| `N` negative / non-const fails loud | none in feature tests — spec defers to upstream const-eval | **DEFERRED** (upstream) |
| 3+ level nesting recurses correctly | `flatten.rs:511-555` (`inline_nesting_composes_through_three_levels` — `seg0.rc.r1`, `seg0.rc.mid`) | **PASS** |
| Label collision disambiguated | `flatten.rs:477-509` (x.* vs y.*); `urc_flatten.rs:71-83` (u1.s0.r1 … u1.s4.c1) | **PASS** |
| Unconnected port preserved | No explicit test — `flatten.rs:132-136` silently skips absent ports; codegen `builder.rs:283` allocates a fresh internal node. Spec says "preserve the existing diagnostic"; none exists pre-feature. | **MINOR GAP** (no test, but no regression either) |
| Recursive module cycle fails loud | `flatten.rs:345-389` (self-cycle + transitive cycle) | **PASS** |
| Dangling net fails loud | `flatten.rs:626-658` | **PASS** |
| Indexed NetRef (gap-3) fails loud | `flatten.rs:660-702` | **PASS** |

---

## Discrimination Sensor

Mutations applied to `crates/piperine-lang/src/elab/lower/flatten.rs` in scratch state (file backup → mutate → test → restore). Backup verified byte-identical on restore.

| # | Mutation | Tests run | Result | Killed by |
| - | -------- | ---------- | ------ | --------- |
| M1 | `is_leaf(_) := true` (skip inlining, keep non-leaf instances) | lib flatten + codegen flatten_hierarchy + urc_flatten | **KILLED** (15 failures) | `inline_*` lib tests (8) + `mid_level_module_builds_through_codegen`, `flattened_ladder_simulates_correctly`, `override_on_spliced_label_restamps_flat_instance`, `two_mid_level_instances_produce_distinct_spliced_labels` (4) + all 3 `urc_flatten` tests. Codegen message surfaces verbatim: `"nested hierarchy: \`Seg\` instantiates further modules"` — `builder.rs:156` defensive guard catches the regression loudly. |
| M2 | Reverse port binding in `inline()` (`mutated_i = len-1-i`) — ports bind to wrong parent nets | lib flatten + codegen flatten_hierarchy + urc_flatten | **KILLED at unit, SURVIVED at integration** | Lib unit tests `inline_binds_ports_and_lifts_wires`, `inline_ground_nets_pass_through_remap_unchanged`, `inline_splices_child_connections_through_remap` (3) — explicit net-name assertions. **Codegen sim tests `flattened_ladder_simulates_correctly` and all `urc_flatten` tests PASS** — the test ladder uses equal-valued resistors, so swapping p/n is electrically symmetric. **Coverage observation**: codegen sim tests should use asymmetric segment values (e.g. r1≠r2) to discriminate port-binding bugs. |
| M3 | Remove the cycle guard (`in_progress` check deleted) | not run (would hang) | **KILLED by inspection** | `flatten.rs:345-389` asserts `.err().expect("self-cycle must fail loud")` — without the guard the recursion infinite-loops; the test would either stack-overflow or hang. Either way: failure. |
| M4a | Dangling-net guard returns `NetRef::simple("gnd")` instead of failing | lib flatten | **KILLED** (1 failure) | `inline_dangling_net_fails_loud` (`flatten.rs:626-658`) — only the unit test exercises this path; integration tests use well-formed inputs. |
| M4b | Gap-3 indexed-NetRef guard silently drops index | lib flatten | **KILLED** (1 failure) | `inline_indexed_netref_fails_as_gap3_deferred` (`flatten.rs:660-702`). |

**Sensor summary**: 5 mutations injected, **5 killed** (M3 by inspection — would hang otherwise). 1 partial coverage gap (M2: codegen sim tests are symmetric and would miss a port-binding regression that the lib unit tests catch).

---

## Gate Check

```
cargo build --workspace   → Finished, 2 warnings (pre-existing piperine-cli Python .so notices, unrelated to feature)
cargo test  --workspace   → 705 passed, 0 failed, 5 ignored (doctests)
```

Zero feature-introduced warnings. Zero test failures. The two `piperine-cli` warnings pre-date this feature (verified: present on stash of unrelated unstaged changes; build of the feature's own diff is clean).

Feature-specific suite (all PASS):
- `piperine-lang --lib flatten`: 14/14
- `piperine-lang --test urc_flatten`: 3/3
- `piperine-lang --test elab` (flatten cases): 2/2
- `piperine-lang --test pom_serde` (flat_modules skip): 1/1 (4/4 total in file)
- `piperine-codegen --test flatten_hierarchy`: 5/5
- `piperine --test urc_compile_count`: 1/1
- `piperine --test ngspice_validation` (urc_lump2/5/10): 3/3 (ngspice IS installed at `/usr/bin/ngspice`)

---

## LOCKED Invariant Verification (MD-25 / design.md §UNBREAKABLE RULE)

> `Design::modules` is NEVER mutated by the flatten pass.

**Evidence**:
- `crates/piperine-lang/src/elab/lower/flatten.rs` — only write is `design.flat_modules.insert(...)` at line 99. Lines 39/65/73 are read-only (`design.modules.keys()/.get()` and `design.flat_modules.get()`).
- No `modules_map_mut()` or `self.modules.` write anywhere in `flatten.rs`.
- Non-destructive test assertions in every flatten unit test (`snapshot_modules` deep-equal before/after): `flatten.rs:319,339,352,361,388,455,496,541,582,618,657,701,727`.
- Integration non-destructive test: `elab.rs:783-810` (`flatten_pass_leaves_authored_modules_untouched` — modules deep-equal across re-elaborations; authored Top still has `Seg` as a direct instance).

**Verdict**: invariant upheld.

---

## Code Quality

| Check | Result |
| ----- | ------ |
| No features beyond what was asked | ✅ Pass is single-purpose (flatten); gaps 2/3 deferred per spec, fail-loud not built. |
| No unnecessary abstractions | ✅ One pass struct + 4 helper methods (`flatten_design`, `flatten_module`, `inline`, `remap`); no extra trait/type. |
| Only task-required files touched | ✅ Diff is bounded: 1 new pass file, 1 POM field, 1 codegen indirection, 1 override retarget, 1 header, tests, docs. |
| Matches existing patterns | ✅ Follows the `ElabPass` trait + `PASSES` array convention; `Design::flat_module` mirrors `Design::module`; comments reference design.md. |
| Tests map to ACs, non-shallow | ✅ Each AC has ≥1 test with explicit value assertions (not just "builds OK"); ngspice tests use the established tolerance contract. |
| No `unwrap()`/`expect()` on user-input paths | ✅ All `expect()` calls in test code only (which is the project convention); production paths use `?` and `ok_or_else`. |
| No macros | ✅ Zero `macro_rules!` or proc-macros introduced. |
| Zero warnings | ✅ Build is clean (modulo pre-existing unrelated piperine-cli notices). |
| MD-13 Rust idiom rules | ✅ Trait-owned (`ElabPass`); methods on `FlattenHierarchy`; module named by function (`flatten.rs`); flat over nested; early returns. |

---

## Spec-Precision Gaps

### GAP-1 (FLAT-04): Fixed-N modules instead of parametric `urc[N]`

**Spec P1 AC4**: "`urc` authored as `urc[N]` (existing monomorphization) with a `StructuralFor` over `N` series segments".

**Implementation** (`headers/spice/urc.phdl:46-104`): ships three separately-authored non-generic modules `urc2`/`urc5`/`urc10`, each with N explicit `urc_seg` instances. The natural `urc[N]` + `StructuralFor` + `wire tap : Electrical[N+1]` shape is blocked by gap-3 (array-net → flat-net expansion deferred — `remap` fails loud on `tap[i]`).

**Severity**: MEDIUM.
- The spec OUTCOME is met (3 lump values ngspice-validated, each within tolerance — `4.167V/3.333V/2.500V`).
- The spec MECHANISM is NOT used (no `urc[N]`, no monomorphization consumed, no `StructuralFor`). The 3-level hierarchy exercise (Top → urcN → urc_seg → res/cap) IS still real and goes through the same flatten path a parametric `urc[N]` would.
- The deviation is explicitly acknowledged in `urc.phdl:10-23` and is consistent with the spec's MVP boundary (`spec.md:73-78` — gap-3 deferred, "pure-structural route with generated scalar wires").
- **Net assessment**: acceptable deviation for the MVP. Flag for the fast-follow once gap-3 lands: re-author as `urc[N]` + `StructuralFor` and re-run the same ngspice cross-check.

### GAP-2 (FLAT-04): ngspice twin is a discrete R/C ladder, not ngspice's URC code model

**Spec P1 AC4**: "the result SHALL match ngspice's `urc` device".

**Implementation** (`tests/ngspice/urc_lump{2,5,10}.cir`): the `.cir` files build the SAME lumped RC ladder topology by hand from discrete R/C devices, rather than instantiating ngspice's native `Uxxxx` URC device (`ngspice/src/spicelib/devices/urc/`). The cross-check therefore validates "piperine's flatten-built ladder == a hand-built ngspice ladder", not "piperine's flatten-built ladder == ngspice's URC model".

**Severity**: LOW (for the MVP).
- At DC (`.op`), capacitors are open and the URC code model reduces to exactly the same series-R + Rload ladder ngspice would solve internally. The DC operating point is therefore identical whether ngspice uses its URC code model or a hand-built discrete ladder — the cross-check IS meaningful for the flatten pass's structural correctness at DC.
- A `.tran` cross-check (where the URC code model's capacitance distribution matters) would be more discriminating, and the spec mentions `.op`/`.tran`. The shipped tests are `.op` only.
- **Net assessment**: acceptable for the MVP's DC proof. Flag for a `.tran` cross-check against ngspice's native `U` device once transient analysis of the flattened ladder is in scope.

### GAP-3 (FLAT-05): compile_count test does not verify distinct per-shape monomorph kernels

**Spec P2 AC1**: "WHEN a circuit instantiates `urc[3]` and `urc[7]` THEN codegen SHALL compile two distinct kernels (`urc__3`, `urc__7`) — verified via `AnalogKernel::compile_count` delta of 2".

**Implementation** (`urc_compile_count.rs:97-101`): asserts `delta_5 == delta_10` — i.e. both builds compile the SAME leaf-kernel count (res+cap+vsrc). This is the correct invariant for the fixed-N authoring (urc5/urc10 are non-generic shells that flatten to the same leaf set), but it is NOT the spec's invariant. The spec wanted proof that monomorphization produces a distinct kernel per shape (`urc__3` vs `urc__7`); the implementation cannot test that without a parametric `urc[N]`.

**Severity**: MEDIUM.
- The shipped test verifies a real invariant (flatten doesn't invent or drop leaf kernels per shape) but it is weaker than the spec's.
- FLAT-06 (`urc_compile_count.rs:147-153`) verifies per-build kernel-count stability; FLAT-07 (`urc_compile_count.rs:156-167`) verifies the 20-point sweep compiles one build's worth of kernels. Both hold.
- **Net assessment**: blocked by GAP-1. Once `urc[N]` is re-authored, this test should be retargeted to two distinct monomorph shapes (the spec's exact `urc[3]` + `urc[7]` → delta 2).

---

## Fix Plans for Gaps

1. **GAP-1 + GAP-3 (blocker for full FLAT-04/05 compliance)** — when gap-3 (array-net expansion) lands:
   - Re-author `headers/spice/urc.phdl` as `mod urc[N]` + `StructuralFor i in 0..N` + `wire tap : Electrical[N+1]`.
   - Update `tests/urc_flatten.rs` to instantiate `urc[2]`/`urc[5]`/`urc[10]` and assert `flat_modules["urc__2"/"urc__5"/"urc__10"]` exists.
   - Update `tests/urc_compile_count.rs` to instantiate `urc[3]` + `urc[7]` and assert `compile_count` delta == 2 (distinct `urc__3`/`urc__7` kernels).
2. **GAP-2 (low severity)** — add a `.tran` ngspice cross-check using ngspice's native `Uxxxx urc` device as the golden side (not a hand-built ladder). Validating capacitance distribution through the flatten pass.
3. **M2 coverage observation** — in `flatten_hierarchy.rs:80-104`, change the test ladder to use asymmetric segment values (e.g. r1=1k, r2=2k) so the sim result depends on correct port binding; the current symmetric ladder (r1=r2=1k) hides port-swap bugs at the integration level. Unit tests already cover this; the integration test should too.
4. **Unconnected-port test** — add a unit test in `flatten.rs` confirming an instance with fewer nets than the child has ports does not panic and that codegen allocates a fresh internal node (preserves the existing unconnected-port behavior; no new silent dangling net).

---

## Summary

**Verdict: PASS ✅ (with 3 spec-precision gaps flagged, none blocking the MVP)**

The hierarchy-flattening feature meets every P1 acceptance criterion's OUTCOME: a mid-level module builds and simulates through codegen (FLAT-01), port/wire binding is correct and fail-loud on typos (FLAT-02), labels are collision-free and host-addressable (FLAT-03), and a 3-segment `urc`-shape ladder matches ngspice at 3 lump values (FLAT-04 outcome). The LOCKED non-destructive invariant (MD-25) is upheld by construction and asserted by 13 deep-equal tests. The cycle, dangling-net, and gap-3 fail-loud edges are all covered. 5/5 discrimination-sensor mutations were killed.

Three spec-precision gaps are flagged (all rooted in the same cause: gap-3 blocks the parametric `urc[N]` route):
- FLAT-04 ships fixed-N `urc2/5/10` instead of `urc[N]` (acceptable per spec MVP boundary).
- FLAT-04 ngspice twin is a discrete R/C ladder at `.op` (correct at DC; `.tran` vs ngspice's native URC device is a fast-follow).
- FLAT-05 tests same leaf-kernel count, not distinct `urc__3`/`urc__7` monomorph kernels (blocked by FLAT-04 authoring choice).

None of these regress the spec's stated MVP correctness bar; they narrow the proof's mechanism. All three should close together once gap-3 (array-net expansion) lands — at which point the urc authoring flips to `urc[N]` and these tests retarget to the spec's exact invariants.

**Lessons**: `scripts/lessons.py` does not exist in this repo — no lesson recorded. The M2 partial-coverage observation (symmetric codegen sim tests miss port-binding bugs) is captured above in Fix Plan #3 and would be the natural lesson content if the lessons layer is added.
