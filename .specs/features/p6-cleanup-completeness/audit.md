# P6 — Audit Record

The measurement this feature acts on. Produced by
`tools/audit_tests.py`; per-test verdicts live in `audit_verdicts.tsv`
(one row per test, the gate `--check` enforces).

**Measured:** 2026-07-25, commit `dde81c2` (workspace green: 1123 passed).

> **Correction (T3):** the dead-code total is **two** files, not one —
> `analog_jit.rs` is switched off exactly like `ppr_ir.rs`, and it is named in
> `CLAUDE.md` as a test of record. T5's scope covers both (38 tests).

---

## 1. Test inventory (CLN-01)

| crate | tests | inline | in `tests/` | hint: unit | hint: integration | unclear | dead | ignored |
|---|---|---|---|---|---|---|---|---|
| piperine (root) | 161 | 0 | 161 | 15 | 146 | 0 | 0 | 0 |
| piperine-api | 13 | 10 | 3 | 10 | 3 | 0 | 0 | 0 |
| piperine-cli | 24 | 0 | 24 | 2 | 22 | 0 | 0 | 0 |
| piperine-codegen | 164 | 6 | 158 | 6 | 158 | 0 | **38** | **27** |
| piperine-lang | 354 | 61 | 293 | 82 | 272 | 0 | 0 | 0 |
| piperine-lang-server | 69 | 6 | 63 | 6 | 63 | 0 | 0 | 0 |
| piperine-plugin | 50 | 1 | 49 | 27 | 23 | 0 | 0 | 0 |
| piperine-plugin-macros | 7 | 0 | 7 | 2 | 5 | 0 | 0 | 0 |
| piperine-project | 26 | 22 | 4 | 23 | 3 | 0 | 0 | 0 |
| piperine-python | 59 | 16 | 43 | 0 | 59 | 0 | 0 | 0 |
| piperine-solver | 232 | 100 | 132 | 169 | 62 | 1 | 0 | 0 |
| **TOTAL** | **1159** | **222** | **937** | 342 | 816 | 1 | 38 | 27 |

### Reconciliation with the gate (why 1159 ≠ 1123)

```
1159  #[test] functions found in the tree
 -38  two never-compiled files, each with #![cfg(any())] as its first line:
        ppr_ir.rs      27 tests (all also #[ignore]d)
        analog_jit.rs  11 tests — and CLAUDE.md lists it as a test of record
=1121 live #[test] functions
  +2  passing doctests (piperine-lang: parse/mod.rs, lib.rs)
=1123 exactly what `cargo test --workspace` reports
```

The other 4 doctests are `ignored` illustrative examples and are excluded from
the passing count. **ROADMAP's "~800 tests" and "28 `#[ignore]`d tests" are
both stale** — corrected in T24.

---

## 2. Verdicts (CLN-01, CLN-04)

`audit_verdicts.tsv` carries one row per test:
`file · test · verdict · target_placement · note`, with
`verdict ∈ {keep, move-inline, move-to-tests, regroup, delete}`.

**Default is `keep` at the current placement.** That is a decision, not a
shrug: MD-28 rule 2 makes a test that exercises a crate's public surface
across modules an *integration* test, and both suites the heuristic disputes
most (`piperine-solver`'s 132 `tests/` cases, the analysis modules' 60-odd
inline cases) satisfy that reading:

- A `tests/` case reaching the crate only through `abi`/`prelude` is
  cross-module public-surface work → integration → stays in `tests/`, even
  when it asserts one formula. Moving it inline would require making solver
  internals `pub` — the opposite of the rule's intent.
- An inline case that builds a two-node fixture to exercise **its own
  module's** subject (`analyses/pz.rs`'s RC pole, `analyses/sp.rs`'s
  S-parameter identity) is a unit test *of that module*; the tool flags it
  `integration` only because a fixture touches `CircuitBuilder`. Co-location
  with the implementation is exactly what MD-28 rule 1 asks for.

The heuristic's 282 hint/placement conflicts are therefore **evidence to
inspect, not violations to fix**; `--check` enforces the recorded verdicts, so
the remaining conflicts cannot be silently re-litigated later.

### Decided moves

| File | Verdict | Reason |
|---|---|---|
| `crates/piperine-plugin/tests/manifest.rs` (14 tests) | **move-inline** → `src/manifest.rs` | Pure `Manifest::parse` string→`Result` assertions: one module's own behavior, no cross-crate wiring (MD-28 rule 1) |
| `crates/piperine-plugin/tests/phase3.rs` (6 tests) | **regroup** | Named after a *plan phase*, not a functionality (MD-28 rule 2): split into a hooks suite + the existing `inject.rs` staging suite |
| `crates/piperine-cli/tests/cli_check.rs` (1 test) | **regroup** → `check_cmd.rs` | Header reads "Phase 3 — CLI integration tests"; the crate's own convention is `<command>_cmd.rs` (`add_cmd.rs`, `build_cmd.rs`) |
| `crates/piperine-cli/tests/run_examples.rs` (1 test) | **delete** | survivor: `tests/run_examples.rs::every_example_phdl_elaborates` — same layer, same assertion (elaborate every `examples/*.phdl`) |
| `crates/piperine-lang/tests/run_examples.rs` (1 test) | **delete** | survivor: same — this is the **third** copy of the same gate (root, cli, lang) |

Everything else: `keep`. Amendments made during T8–T16 are appended to §6 with
their reason (a verdict change is recorded, never silent).

### Shared file stems (root vs crate)

| Stem | Decision |
|---|---|
| `run_examples.rs` (root + cli + lang) | Same-layer triplicate → keep root, **delete** both crate copies (above) |
| `opvar_host.rs` (root + piperine-python) | **Keep both** — different hosts (Rust `OpResult` vs the Python facade); layer duplication is coverage, not redundancy (spec edge case) |
| `session_analyses.rs` (root + piperine-python) | **Keep both** — same reason (MD-22 parity is the point) |

---

## 3. Process-global state (CLN-10)

16 tests touch process-global state; every one already lives where its guard
is, and all 16 are `keep`:

| Location | Count | Global state | Guard that must survive |
|---|---|---|---|
| `crates/piperine-python/src/lib.rs` | 13 | `Python::with_gil` + `run_script` | GIL + the facade lock inside `embed::run_script` |
| `crates/piperine-python/src/live.rs` | 3 | `Python::with_gil` + full session rebuild | same |

No relocation touches an env-var or cwd-mutating test, so CLN-10 costs nothing
beyond keeping these `keep` — recorded so a later verdict change cannot lose
the guard silently.

---

## 4. Integration targets with no `//!` scope header (CLN-08)

The mechanical half of "grouped by functionality": 13 targets state no scope.
Each gets a header in its crate's migration task (T9–T16), except the two
deleted files.

```
crates/piperine-cli/tests/run_examples.rs          (deleted in T9)
crates/piperine-lang/tests/run_examples.rs         (deleted in T13)
crates/piperine-lang-server/tests/integration_test.rs   (T14 — also renamed)
crates/piperine-lang/tests/bundle_connections.rs   (T13)
crates/piperine-lang/tests/elab.rs                 (T13)
crates/piperine-lang/tests/parse_elab.rs           (T13)
crates/piperine-lang/tests/rfport.rs               (T13)
crates/piperine-lang/tests/type_casts.rs           (T13)
crates/piperine-solver/tests/abi_surface.rs        (T15)
crates/piperine-solver/tests/digital_topology.rs   (T15)
crates/piperine-solver/tests/mixed_signal.rs       (T15)
crates/piperine-solver/tests/prelude_surface.rs    (T15)
crates/piperine-solver/tests/solver_entry.rs       (T15)
```

---

## 5. Gate state at T2

`tools/audit_tests.py --check-all` → **16 violations**, which are exactly the
pending work items:

- 14 × `crates/piperine-plugin/tests/manifest.rs` — recorded `inline`, still in
  `tests/` (T10).
- 2 × `run_examples.rs` — recorded `delete`, tests still present (T9, T13).

`regroup` rows do not fail placement (the target stays `tests/`); regrouping is
verified by the per-crate `-- --list` diff and the `//!`-header guard (T6).

---

## 6. Verdict amendments during migration

*(appended by T8–T16; empty at T2)*

---

## 7. §16 failure-rule classification (CLN-16)

**The table has 16 rows, not 18** — `spec.md`/`tasks.md` said 18 from a
miscount; the measured row count (`awk` over the table in
`docs/spec/part_vii_solver.md:1122-1140`) is 16. Corrected here and in T24.

Verdicts: **E** = enforced (a test trips it), **U** = enforceable, untested
(the failure site exists; no test reaches it) → T21, **?** = no failure site
located → T22 decides remove-or-implement.

| # | § | Rule | Site | Test | Verdict |
|---|---|---|---|---|---|
| 1 | §2 | Element declares no capability | none found — `CircuitInstance` iterates by capability, so a no-flag element is silently inert rather than rejected | — | **?** |
| 2 | §3 | Unsupported analog behavior reaches the ABI | `CodegenError::Unsupported` (codegen, not solver) | `piperine-codegen/tests/disto_jit.rs:183-186` (`CodegenError::Unsupported` matched), `digital_codegen_gaps.rs` (three fail-loud gaps) — verified passing | **E** (enforced one layer earlier — the row's "device-load error" is codegen's fail-loud) |
| 3 | §4 | Digital boundary changes during an analysis | none found | — | **?** |
| 4 | §4 | Digital event targets a nonexistent net | none found (`digital/scheduler.rs` resolves nets by index; an unknown name cannot be constructed through the public surface) | — | **?** |
| 5 | §5 | Plugin cannot bind required terminals/params | `PluginError::DeviceNotRegistered` + the plugin/`@device` arity checks (`piperine-plugin/src/host.rs:471-484`) | `piperine-plugin/tests/e2e.rs::unregistered_type_fails_loud`, `::device_without_host_fails_loud` | **E** |
| 6 | §6 | Stamp references an unmapped variable | none found — `AnalogReference` is index-based, so an unmapped variable is unrepresentable | — | **?** |
| 7 | §8 | Analysis-time loading changes matrix dimension/sparsity | `analyses/dc.rs:328` `"solution not contiguous"` is the nearest guard | — | **U** |
| 8 | §9 | DC fails Newton + gmin + source stepping | `analyses/convergence.rs` plan exhaustion → `SolverDomain::Dc` | only the *scripted* driver test (`convergence.rs:658` inline, a fake driver) — no circuit-level test | **U** |
| 9 | §10 | Transient reaches the minimum timestep without converging | `analyses/transient.rs` `dt_min` floor (`dt_min_floor_hits`) | — | **U** |
| 10 | §11 | AC frequency point cannot solve its linear system | `math/faer.rs:120` → `SolverDomain::SpaceMatrix`, wrapped per point | — | **U** |
| 11 | §12 | Noise output/reference node unresolvable | `analyses/noise.rs:335,340` | — | **U** |
| 12 | §13 | Unsupported TF source form | `analyses/tf.rs:326` `"current-source input is not supported (D5)"` | `tests/session_tf.rs` asserts the message | **E** |
| 13 | §14 | Digital delta cycle does not settle | `digital/scheduler.rs:77` (+ `:221` DAG back-edge) | — | **U** |
| 14 | §15 | Linear solve returns NaN or infinity | `math/newton_raphson.rs:197,356` `!is_finite` guards | — | **U** |
| 15 | §17 | Sensitivity param unknown/unreadable/non-real/rebuild-strength | `analyses/sens.rs:85` + the rebuild-strength check | `tests/sens.rs` (rebuild-strength + unknown-param cases) | **E** |
| 16 | §18 | Non-positive period / negative pre-roll / non-periodic digital state | `analyses/pss.rs:105,367` | `tests/pss.rs` asserts the non-periodic case; **period/pre-roll validation is untested** | **partial E/U** |

**Totals:** 4 enforced (rows 2, 5, 12, 15), 1 partial (16), 7 enforceable-but-
untested (7, 8, 9, 10, 11, 13, 14 + the untested half of 16), 4 with no located
site (1, 3, 4, 6).

### Work lists

- **T21 (add tests):** rows 7, 8, 9, 10, 11, 13, 14, and 16's period/pre-roll
  half. Each asserts `SolverDomain` + the message fragment — the taxonomy is
  `Error::{Simple,WithCause}{domain, detail}` (`error.rs:57-70`), so
  domain+fragment *is* the typed assertion available.
- **T22 (decide remove-or-implement):** rows 1, 3, 4, 6. Each looks
  unreachable **by construction** rather than merely untested (index-based
  references, capability-driven iteration), which is the spec's
  "unenforceable" case — but T22 must attempt the trip before removing the
  row, and a row that turns out reachable becomes a T21 test instead.

### Note on "typed error"

`SolverDomain` is the type; the detail is a string. A row's test therefore
asserts `(domain, fragment)`. Introducing a per-rule error enum would be an
ABI change well beyond P6's hygiene scope — recorded as a follow-up, not done
here.

## 8. Capability-flag verdict evidence (CLN-11/12/15)

Grep scope for every row below: `crates/**/*.rs` + `tests/**/*.rs` (whole
workspace, excluding `target/`).

### `SUPPORTS_QUERIES` (`1 << 10`) → **remove** (CLN-11)

| Question | Evidence |
|---|---|
| Who declares it? | **Nobody.** The only mentions are its own definition (`core/element.rs:77`) and the registry entry (`tests/capabilities_contract.rs:51`). No device, fixture, or test sets it. |
| Who reads it? | **Nobody.** No `contains(SUPPORTS_QUERIES)` anywhere. |
| What does the bit claim? | `core/element.rs:71-77`: "Reserved: a host hint that the model overrides `list_queries`/`query` with typed metadata beyond the `read_opvars` default. No solver path reads this flag today (audit SS-11)." |
| Why removal, not wiring? | `Introspect::list_queries` (`core/element.rs:382-388`) and `Introspect::query` (`:392-397`) both have working defaults derived from `read_opvars`, and consumers already call them unconditionally (`core/introspect.rs:425`, `piperine-codegen/tests/opvar_bridge.rs:169,244`). A hint bit adds nothing a host cannot ask for directly — there is no behavior to gate. |

### `BYPASS_OK` (`1 << 11`) → **wire** (CLN-12)

| Question | Evidence |
|---|---|
| Who declares it? | Only a **test** device: `piperine-solver/tests/live_params.rs:92` (its doc comment at `:12` says it declares the bit "so the DC device-bypass stamp cache applies to it"). No production device — `PiperineDevice::capabilities` never sets it. |
| Who reads it? | **Nobody.** `analyses/dc.rs:114-145` decides bypass from a *global* per-variable "solution barely moved" test plus a limiter check; its own comment at `:117` admits the situation: "audit P4 — BYPASS_OK declared but never consulted". |
| What does the bit claim? | `core/element.rs:78-85`: eligible for stamp bypass, "**Opt-in** — a model only sets this when its stamps are a pure function of terminal voltages". |
| Why wiring, not removal? | The implementation is *broader* than the contract: every device is bypassed, including ones whose stamps are not a pure function of terminal voltages. Removing the bit would bless that; wiring it makes the code match the documented contract. The existing suppression seams (`any_limiting_report`, `invalidate_bypass`) stay untouched. |
| Disqualifiers the codegen predicate must check (T19) | runtime operators (`delay`/`slew`/`idt`/`transition`), analog events, `$limit` limiters, `DEPENDS_ON_DIGITAL`/`SAMPLES_ANALOG`, history-dependent internal unknowns — each is already an `Option` capability sub-struct on `AnalogKernel`. |
| Risk accepted | Circuits containing a stateful device lose bypass hits (extra Newton evaluations). User decision, 2026-07-25: wire it. |

### `bound_step_hint` → **already enforced**, ROADMAP correction only (CLN-15)

ROADMAP P6 groups it with the dead flags ("same disposition as the already-
logged `bound_step_hint`"). That is stale — it has a producer, a consumer, and
a test:

| Role | Site |
|---|---|
| ABI default | `piperine-solver/src/core/element.rs:176` (`f64::INFINITY`) |
| Producer | `piperine-codegen/src/device/mod.rs:238-241` → `AnalogInstance::bound_step_hint` (`device/analog/mod.rs:1204`) |
| Consumer | `piperine-solver/src/analyses/events.rs:326` — folded into `EventEntry::step_hint` |
| Test | `piperine-solver/tests/event_adapters.rs:119` |

No code change; T24 corrects the ROADMAP text.

### Registry sweep

`documented_consumer` (`tests/capabilities_contract.rs:21-85`) has exactly two
entries that name no live consumer — the two above (`"reserved: host query-
metadata hint; no solver consumer today"`, `"reserved: solver-performance owns
stamp bypass"`). Every other flag's entry names a branch-gate, a loader, or a
driver. After T18–T20 the phrase "no consumer" cannot appear at all (T20 makes
that an assertion).

## 9. Guard proofs (CLN-08)

*(T7)*

## 10. Final accounting (CLN-02/04/09)

*(T17)*
