# P6 — Audit Record

The measurement this feature acts on. Produced by
`tools/audit_tests.py`; per-test verdicts live in `audit_verdicts.tsv`
(one row per test, the gate `--check` enforces).

**Measured:** 2026-07-25, commit `dde81c2` (workspace green: 1123 passed).

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
 -38  crates/piperine-codegen/tests/ppr_ir.rs — first line is #![cfg(any())],
      so the whole file (38 tests, 27 of them also #[ignore]) never compiles
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

*(T3)*

## 8. Capability-flag verdict evidence (CLN-11/12/15)

*(T4)*

## 9. Guard proofs (CLN-08)

*(T7)*

## 10. Final accounting (CLN-02/04/09)

*(T17)*
