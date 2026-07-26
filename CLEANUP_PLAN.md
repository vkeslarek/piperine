# Cleanup Plan — architecture, placement, and readability

**Status:** proposal / working document. Nothing here is executed yet.
**Date:** 2026-07-26 · **Branch of record:** `feature/bench-removal`
**Governing rules:** MD-13 (Rust idiom rules), MD-17/MD-20/MD-23 (public
surfaces), MD-25 (POM navigability), MD-28 (test placement), MD-31 (a policy
invariant lives in the gate).

This is not a bug hunt. The question it answers is the one the user posed:

> *does every element of the project sit where it belongs, can I hold a mental
> picture of how it all works and know the home of each thing, are the
> implementations clean, are the files split sensibly?*

Every finding below is anchored to evidence gathered from the tree at
`6d7c35e` (plus the CI/examples fix in flight). Each has a verdict, a proposed
action, and a cost/risk note. Findings are numbered `CL-nn` so tasks can cite
them.

---

## 0. Measurements (the baseline this plan reasons about)

| Metric | Value |
|---|---|
| Workspace crates | 10 + root shell |
| `src` LOC (all crates) | 61 199 |
| Inline-test + integration LOC | ~26 300 |
| Test targets | 166 (`tests/*.rs` across the workspace) |
| Tests green | 1161 / 0 failed / 4 ignored (STATE.md, 2026-07-26) |
| Functions total | 2123 · `>100` lines: **20** · `>200` lines: **4** |
| Module-level free functions (MD-13 rule 2) | **332** |
| Blanket `#![allow(dead_code)]` files | **12** |
| Dead items those 12 files hide | **22** (measured, §2) |
| `mod.rs` files over 500 lines | 4 (largest: 1237) |
| `TODO`/`FIXME`/`HACK` in `src` | 0 ✅ |

Per-crate `src` size and free-function count — the free-function column is the
sharpest single indicator of which crates have had the MD-13 pass and which
have not:

| Crate | src LOC | module-level free `fn` |
|---|---:|---:|
| `piperine-lang` | 15 511 | 75 |
| `piperine-codegen` | 14 918 | **112** |
| `piperine-solver` | 13 629 | **5** ✅ |
| `piperine-python` | 5 206 | 11 |
| `piperine-lang-server` | 3 457 | 66 |
| `piperine-api` | 3 389 | 8 |
| `piperine-plugin` | 2 165 | 13 |
| `piperine-project` | 1 472 | 8 |
| `piperine-cli` | 1 222 | 30 (command entry points — legitimate) |
| `piperine-plugin-macros` | 230 | 4 |

`piperine-solver` is the reference implementation of the house style: 13.6k
LOC, 5 free functions, one long function in the whole crate. `codegen` and
`lang` never got that pass. **That gap — not any individual file — is the
main body of work in this plan.**

---

## 1. Architecture: where the seams are wrong

### CL-01 — `piperine-api` ships **two** host surfaces in one file (highest-value fix)

`crates/piperine-api/src/session.rs` is 1329 lines and defines **both**:

- `SimSession` (line 103) — `run_op`, `run_tran`, `run_ac`, `run_noise`,
  `run_sens`, `run_pss`, `run_pz`, `run_sp`, `run_disto`, `snapshot_*`, `stage`
- `Session` (line 629) — `compile`, `set`, `schedule_set`, `op`, `tran`, `ac`,
  `noise`, `sens`, `pss`, `pz`, `disto`, `sp`, `tf`, `dc`, `sweep`, `rebuilds`

Two names, two lifetimes, largely the same analysis menu. Consumers are split:

- `piperine-python` binds **`SimSession`** (`module.rs` 6 uses, `live.rs` 7)
- `piperine-api`'s own `Session` is used by `python/src/instance.rs` +
  `embed.rs` only
- 24 of the 37 root integration targets construct `SimSession`

MD-27 was adopted precisely to kill host drift ("api-canonical, parity-enforced"),
and the drift now lives *inside* the canonical crate. A newcomer asking "what is
the entry point of the Rust host?" has no answer from the tree.

**Verdict:** the single worst hit to the mental picture in the repo.
**Action:** decide one entry point and demote the other to a documented alias or
delete it. Then split the file by role: `session.rs` (the entry object),
`sweep.rs` (`Sweep`/`SweepPoint`/`Grid`/`Nested`), `config.rs`
(`SolverConfig`/`Scale`). Retarget `piperine-python` to the survivor in the same
change so parity is never one-sided (MD-27 §2).
**Cost:** medium-high (touches Python bindings + ~24 test targets).
**Risk:** mechanical but wide; `tests/host_parity.rs` is the safety net.

### CL-02 — the host **object model** exists only in Python

`piperine-api` has no `Design`/`Module`/`Instance` types; Python does
(`python/src/design.rs`, `module.rs`, `instance.rs`, 1436 LOC). MD-22 says the
Rust host "gains the object model Python already has" and MD-27 §1 says
capability lands in the api *first*. Today the flow is inverted: Python owns the
navigable model and the api owns only sessions and results.

**Verdict:** a locked decision that is not implemented; the tree contradicts
the docs.
**Action:** either lift `Design`/`Module`/`InstanceView` navigation into
`piperine-api` (making Python's three files pure delegation, matching what
`results.rs` already does well — every method is a one-line `self.inner.…`
forward), or amend MD-22 to record that the object model is Python-only.
Both are acceptable; the current silence is not.
**Cost:** high (option A) / trivial (option B, doc amendment).

### CL-03 — root `tests/` holds tests that belong to other crates

The root package is a "thin re-export shell" with 9 lines of `src` (MD-20) and
**5839 lines of tests** in 37 targets. Most are legitimate (the shell's parity
proof, per MD-20). Four are not — they never touch `piperine` at all:

| Target | Actually imports | Belongs in |
|---|---|---|
| `tests/sens.rs` (217) | codegen + solver + lang | `piperine-solver/tests/` |
| `tests/pss.rs` (195) | codegen + solver + lang | `piperine-solver/tests/` |
| `tests/transient_reentry.rs` (133) | codegen + solver + lang | `piperine-solver/tests/` |
| `tests/plugin_parity.rs` (258) | plugin + solver + lang | keep (cross-crate parity is a root concern) |

Also note the pairing smell: `sens.rs`/`pss.rs`/`pz.rs`/`sp.rs`/`disto.rs`
(engine-level) sit beside `pss_host.rs`/`host_*.rs` (host-level) in the same
directory with no naming rule distinguishing them.

**Verdict:** MD-28 violation, small but exactly the kind that erodes "know the
place of each thing".
**Action:** move the three solver-level targets; adopt one root naming rule —
`host_*.rs` for host-surface proofs, `<feature>.rs` for shell/integration
proofs — and enforce it in `tests/suite_hygiene.rs` (MD-31: the rule is a test).
**Cost:** low. **Risk:** none (test-only).

### CL-04 — `piperine-solver`'s `analog/` is a one-file directory; `result.rs` floats

`crates/piperine-solver/src/analog/` contains `mod.rs` (8 lines) + `netlist.rs`
(266) — a directory that exists to hold one file, next to `digital/` which
genuinely has five. And `result.rs` (445) sits at crate root while every other
domain concept lives under `core/`, `analyses/`, `math/`, `digital/`.

**Verdict:** minor, but MD-13 rule 4 is about the tree being self-describing at
a glance, and these two entries misdescribe it.
**Action:** collapse `analog/netlist.rs` → `analog.rs` (or move it under
`core/`, alongside `net.rs`, which is its naming sibling); move `result.rs` →
`core/result.rs`.
**Cost:** trivial. **Risk:** none.

### CL-05 — `Tolerances` lives in `analyses/mod.rs`, which says config lives elsewhere

`analyses/mod.rs` (269 lines) declares "the config home lives in `config.rs`"
in its own `//!` header — then defines `Tolerances`, `Context`, `Policy`, and
the `Once` init inline.

**Verdict:** a module doc contradicted by the module it documents; also the
only remaining `mod.rs` in the solver carrying types.
**Action:** move `Tolerances`/`Context`/`Policy` into `analyses/config.rs` (or a
new `analyses/context.rs` if `config.rs` should stay literals-only), leaving
`mod.rs` as declarations + re-exports. Fold the `Once` into `Solver::build`
per MD-06 while there.
**Cost:** low. **Risk:** none (pure move).

---

## 2. Dead and hidden surface

### CL-06 — 12 blanket `#![allow(dead_code)]` hide 22 dead items (measured)

Removing all twelve file-level allows and running `cargo check --workspace
--all-targets` yields exactly 22 warnings. The full list:

| Dead item(s) | Location |
|---|---|
| trait `DcAnalysis` | `analyses/dc.rs` |
| trait `AcAnalysis` | `analyses/ac.rs:85` |
| trait `TransientAnalysis` | `analyses/transient.rs:166` |
| trait `NoiseSource` | `analyses/noise.rs:59` |
| structs `AcContext`, `TransientContext`, `NoiseContext`, `TfContext` | `analyses/{ac,transient,noise,tf}.rs` |
| 8 constants: `PI`, `E`, `I`, `BOLTZMANN_CONSTANT`, `ELEMENTARY_CHARGE`, `PLANCK_CONSTANT`, `SPEED_OF_LIGHT`, `ABSOLUTE_ZERO_CELSIUS` | `math/constant.rs` (**the entire file**) |
| enum `Port` | `core/port.rs` (**the entire file**) |
| field `options` never read | `analyses/pz.rs:78` |
| field `bundle_name` never read | `resolve/pom/structure.rs:14` |
| methods `require_param_given`, `lookup_param`, `lookup_var`, `require_ident_as_param`, `require_var` | `resolve/pom/mod.rs:179` |
| functions `has_ddt_marker`, `contrib_branch` | `resolve/pom/stmt.rs:44,61` |

Two of these are architecturally load-bearing findings, not lint noise:

1. **The four `*Analysis` traits are dead.** The analysis abstraction the
   solver *reads* as trait-based is not: `analyses/*.rs` drivers are concrete.
   Anyone reading for the contract (MD-13 rule 1) is reading a fiction.
2. **The four `*Context` structs are dead** — i.e. **MD-03**
   ("per-analysis context, shared `Context`", status *Locked, implementation
   pending*) is half-built and rotting in the tree. The skeleton was laid, the
   wiring never happened, and a blanket allow silenced the evidence.

This is the MD-31 failure mode verbatim: dark surface reading as live surface,
already "documented", caught by no gate.

**Action, in order:**
1. Delete `math/constant.rs` and `core/port.rs` (whole files, zero consumers).
2. Delete the four dead traits and the two dead free functions; drop the two
   never-read fields; drop the five dead `resolve/pom` methods.
3. Decide MD-03: implement the per-analysis contexts **or** delete the four
   structs and mark MD-03 superseded. Do not leave the third option.
4. Remove all 12 `#![allow(dead_code)]` lines and add a hygiene guard that
   fails on any *new* file-scope `allow(dead_code)` (registry +
   exhaustiveness, the `capabilities_contract.rs` shape). Prove the guard can
   fail before landing it.

**Cost:** low for 1–2 and 4; medium for 3 (a real decision).
**Risk:** low — the compiler is the oracle.

### CL-07 — narrower `#[allow(dead_code)]` at item scope (7 sites)

`emit/builder.rs:146,150,154`, `lang/resolve.rs:43`, `lang-server/state.rs:74`,
`lang-server/handlers/diagnostics.rs:145`, `analyses/pz.rs:77`. Each needs the
same triage: consumer, or delete. Item-scoped allows are far less harmful than
file-scoped ones — but seven of them is a pattern, not an exception.

---

## 3. Comments that describe a codebase that no longer exists

### CL-08 — 16 references to the deleted IR architecture

`grep -rn "IrProgram\|IrModule\|IrExpr\|IrInstance\|piperine-ir"` over `src`
returns 16 hits, all in doc comments. Two kinds:

**Broken doc links (actively wrong):**
- `codegen/src/device/mod.rs:8` — "walks an [`crate::resolve::IrProgram`]'s top
  module"; that type does not exist. An intra-doc link to nothing.
- `lang/src/math.rs:2` — "the compile-time evaluator used by
  `IrExpr::eval_const`".

**Archaeology (defines code by what it used to be):**
`codegen/src/lib.rs:13`, `resolve/mod.rs:1,84`, `resolve/expr.rs:1`,
`resolve/stmt.rs:2`, `resolve/pom/mod.rs:3,40`, `device/circuit.rs:8,253`,
`device/builder.rs:86`, `flatten/analog.rs:2`, `lang/elab/lower/module.rs:212`.

Sentences of the form "formerly the standalone `piperine-ir` crate", "the old
`IrExpr` is gone", "there is no `IrModule` structural twin" require a reader to
know a history they cannot verify from the tree. Some carry real intent (the
*absence* of a structural twin is a deliberate invariant) — those belong as
positive statements ("`CircuitCompiler` reads structure from the POM directly")
or as an MD entry, not as a negation of a deleted name.

**Action:** fix the two broken links; rewrite the archaeology in the positive;
keep at most one historical note, in `codegen/src/lib.rs`, where the pipeline
is introduced. Add a hygiene grep guard for the dead identifiers (MD-31).
**Cost:** low. **Risk:** none.

### CL-09 — `CLAUDE.md` and `ROADMAP.md` carry stale counts

- `CLAUDE.md`: "the whole suite — **51 green targets**". Actual: **166** test
  targets, 1161 tests.
- `CLAUDE.md` "Tests of record" lists files by name — a list that has already
  drifted once (P6 found `analog_jit.rs` listed there while switched off with
  `#![cfg(any())]`).
- `.specs/STATE.md` still shows `solver-simplification` as **IN PROGRESS —
  batch 6 remaining** (Part VII canonical rewrite T33–T35) while later entries
  read as delivered work on top of it.

**Verdict:** MD-31's exact lesson — a number in prose is not a gate.
**Action:** delete the count from `CLAUDE.md` (or have `suite_hygiene.rs` assert
it); replace the hand-maintained "tests of record" list with a pointer to the
guard that enumerates targets; reconcile `solver-simplification`'s status.
**Cost:** trivial. **Risk:** none.

### CL-10 — `docs/manual/` is an empty shell

`docs/manual/` contains `index.md` and nothing else, while `docs/spec/` has 12
substantive parts, and two mkdocs configs (`mkdocs.yml`, `mkdocs-spec.yml`)
build two sites. Either the manual is a real deliverable with a tracked
backlog, or the directory should go until it is.

---

## 4. MD-13 rule 2 (no loose functions) — the largest single debt

332 module-level free functions, concentrated in three crates:
**codegen 112, lang 75, lang-server 66**. Sampling codegen shows the character:

```
flatten/analog.rs:267  fn select        flatten/analog.rs:1028 fn collect_branch_current_pairs
flatten/analog.rs:275  fn binary        flatten/analog.rs:1049 fn isolate_branch_coeff
flatten/analog.rs:279  fn lit           flatten/analog.rs:1073 fn zero_branch_currents
flatten/analog.rs:283  fn not_expr      flatten/analog.rs:1089 fn has_branch_current
flatten/analog.rs:287  fn and_guards    flatten/analog.rs:1106 fn has_branch_access
kernel/analog/compile.rs:28 fn collect_limits, :51 limit_branch, :69 limit_branches_into,
                            :103 ident_of, :111 expr_eq
emit/cse.rs:83 fn expr_structural_eq, :122 blocks_eq, :132 stmts_eq
```

These are not random: they cluster into three *missing abstractions*, which is
exactly what MD-13 rule 2 predicts ("if a helper doesn't belong to a trait or
struct, the abstraction is missing"):

1. **An expression constructor/builder** — `select`, `binary`, `lit`,
   `not_expr`, `and_guards`, `subst_expr`, `subst_block`, `subst_scope`,
   `substitute_marker`. Wants to be `ExprBuilder` (or inherent constructors on
   the resolved `Expr`).
2. **An expression *query* surface** — `has_branch_current`,
   `has_branch_access`, `has_marker`, `has_ddt_marker`, `expr_eq`,
   `expr_structural_eq`, `blocks_eq`, `stmts_eq`, `contrib_branch`,
   `isolate_branch_coeff`. Wants to be inherent methods on `Expr`/`Stmt` (or
   one `ExprQuery` trait) — the CSE structural-equality trio especially, which
   is one algorithm split across three free functions.
3. **A limits collector** — `collect_limits`/`limit_branch`/
   `limit_branches_into`/`ident_of` is one stateful walk wearing four hats.
   Wants to be `LimitCollector`.

**Action:** three focused refactors in `piperine-codegen`, in the order above,
each behavior-preserving with the existing codegen suite as oracle. Then the
same treatment for `lang` (parser helpers → parser methods) and `lang-server`
(handler helpers → `DocumentState`/`SymbolIndex` methods). Do **not** attempt
all 332 in one pass; land it crate by crate, one abstraction per commit.
**Cost:** high in aggregate, low per commit.
**Risk:** low — pure mechanical moves, strong test coverage
(codegen 5465 test LOC, lang 6447).

---

## 5. File splits and long functions

### CL-11 — `mod.rs` files carrying implementations

| File | Lines | Content |
|---|---:|---|
| `codegen/src/device/analog/mod.rs` | **1237** | `AnalogInstance` + stamping |
| `codegen/src/kernel/analog/mod.rs` | **864** | `AnalogKernel` |
| `codegen/src/device/mod.rs` | **859** | `CompiledModule` + `PiperineDevice` |
| `codegen/src/resolve/pom/mod.rs` | **615** | `LoweredBody` + `lower_bodies` |
| `solver/src/analyses/mod.rs` | 269 | see CL-05 |

MD-13 rule 4's golden rule ("glance at the file tree and know where every
struct and trait belongs") fails on `mod.rs`: the name says *nothing* about the
1237 lines inside. The sibling files in those same directories already do it
right (`forces.rs`, `limits.rs`, `operators.rs`, `events.rs`).

**Action:** rename the implementation out of every `mod.rs` — e.g.
`device/analog/mod.rs` → `device/analog/instance.rs` (`AnalogInstance` is
already the type name), `kernel/analog/mod.rs` → `kernel/analog/kernel.rs`,
`device/mod.rs` → `device/compiled.rs` + `device/element.rs` (the
`CompiledModule`/`PiperineDevice` split is a natural seam), `resolve/pom/mod.rs`
→ `resolve/pom/body.rs`. Leave each `mod.rs` as declarations + re-exports only.
Add a hygiene guard: no `mod.rs` over ~60 lines (MD-31).
**Cost:** low-medium (import churn only). **Risk:** none.

### CL-12 — the 4 functions over 200 lines

| Function | Lines |
|---|---:|
| `lang/src/parse/parser/expr.rs:96 fn parse_primary` | **316** |
| `codegen/src/resolve/pom/mod.rs:315 fn lower_bodies` | **253** |
| `lang/src/parse/parser/stmt.rs:22 fn parse_mod_stmt` | **212** |
| `lang-server/src/handlers/symbols.rs:46 fn extract_symbols` | **207** |

Plus 16 in the 100–200 band (`pss::solve` 158 — the solver's only offender —
`digital/compile.rs::compile` 141/123, `design.rs::introspection_meta` 136,
`flatten/analog.rs::walk` 134, `lexer.rs::lex_number` 133,
`resolve_expr` 132, `server.rs::new` 129, `tokenize_all` 124,
`resolve_call` 114, `interp.rs::eval_call` 107 / `eval_expr` 98,
`resolve.rs::prelude_items` 104, `parse_behavior_stmt` 101).

Nuance worth stating: a recursive-descent `parse_primary` is a dispatch table by
nature, and `parse/` is on the "do not edit casually" list. The honest target is
not "every function under 60 lines" but **one dispatch level per function** —
`parse_primary` should be a match arm per token kind, each arm one call.

**Action:** decompose the four >200 offenders into named phase methods, the
pattern `analyses/transient.rs` already proved (batch 5 of
`solver-simplification`: `predict_step`/`attempt_step`/`assess_step`/…, no
driver method over 60 lines). Take `lower_bodies` first — it is codegen's
entry point and not on the frozen list.
**Cost:** medium. **Risk:** medium for the parsers (frozen-corpus tests are the
oracle), low for `lower_bodies` and `extract_symbols`.

### CL-13 — inline tests dominating a façade file

`piperine-python/src/lib.rs` is 1186 lines, of which the `#[cfg(test)] mod
tests` block (from line 113) is the large majority, including three ~100-line
test functions with inline PHDL fixtures. The declarations + `#[pymodule]` are
~110 lines.

MD-28 rule 1 says unit tests live inline with their implementation — and these
tests exercise the *bindings across modules* (`_Design`, `_Module`, waveform
projections), which makes them integration tests of the crate's public surface,
belonging in `piperine-python/tests/`. The inline PHDL fixtures also want a
shared `tests/common/` (the `lang-server` split already established that
pattern).

**Action:** move the block to `crates/piperine-python/tests/bindings.rs` (split
by feature: `design.rs`, `waveform.rs`, `ac.rs`, `noise.rs`), with fixtures in
`tests/common/`. Leaves `lib.rs` as the ~110-line façade it should be.
**Cost:** low. **Risk:** none.

---

## 6. What is already good (do not "fix" these)

Stating this explicitly so a cleanup pass does not churn healthy code:

- **`piperine-solver`** is the house style working: 13.6k LOC, 5 free
  functions, one 158-line function, per-module `//!` layer contracts, one
  `Element` ABI with capability bits, `prelude`/`abi` two-tier surface (MD-17).
  It is the model the other crates should converge on.
- **`piperine-python`'s delegation layer** is genuinely thin where it counts —
  `results.rs`'s waveform methods are one-line `self.inner.…` forwards with no
  duplicated math. The volume is binding boilerplate, not logic drift.
- **Zero `TODO`/`FIXME`/`HACK`** in 61k lines of `src`. Unusual and worth
  keeping.
- **Zero macros** (MD-13 rule 5) outside the deliberate `piperine-plugin-macros`
  proc-macro crate, which exists for declaration-coupled contributions (MD-29).
- **The guard culture** (MD-31): `suite_hygiene.rs`,
  `capabilities_contract.rs`, `spec_failure_rules_guard.rs`,
  `extern_coverage_guard.rs`, `host_parity.rs`, `plugin_parity.rs`. Every new
  invariant in this plan should land as one more of these, not as a paragraph.
- **`piperine-cli`'s** one-file-per-command layout under `commands/` — its 30
  free functions are command entry points, which is the right shape.

---

## 7. Proposed phasing

Ordered by *mental-picture value per unit of risk*, not by size.

**Phase A — free wins, no design decisions (≈1 sitting, zero risk)**
1. CL-06 steps 1–2 + 4: delete `math/constant.rs`, `core/port.rs`, the four
   dead traits, two dead functions, two dead fields, five dead methods; remove
   all 12 file-scope allows; add the allow-guard.
2. CL-08: fix the two broken doc links, rewrite the 14 archaeology comments,
   add the dead-identifier grep guard.
3. CL-09: strip stale counts; reconcile `solver-simplification` status.
4. CL-04: collapse `analog/` → `analog.rs`; move `result.rs` → `core/`.
5. CL-05: move `Tolerances`/`Context`/`Policy` into the config home.

**Phase B — placement (low risk, test-only or import-only churn)**
6. CL-11: rename implementations out of the five oversized `mod.rs` files; add
   the `mod.rs` size guard.
7. CL-03: move the three solver-level root targets; adopt + enforce the root
   test naming rule.
8. CL-13: extract `piperine-python/src/lib.rs`'s test block into
   `tests/` + `tests/common/`.

**Phase C — the two real decisions (needs the user)**
9. CL-01: pick **one** of `SimSession` / `Session`; retarget Python; split
   `session.rs` by role.
10. CL-02: lift the object model into `piperine-api`, **or** amend MD-22.
11. CL-06 step 3: implement MD-03's per-analysis contexts, **or** delete them
    and mark MD-03 superseded.

**Phase D — the long grind (crate by crate, one abstraction per commit)**
12. CL-10 codegen: `ExprBuilder`, the expression-query surface,
    `LimitCollector` (kills ~40 of codegen's 112 free functions).
13. CL-12: decompose `lower_bodies`, then `extract_symbols`, then the two
    parser giants (frozen-corpus tests as oracle).
14. CL-10 for `lang` and `lang-server`.

---

## 8. Proposed new MD entries (for `.specs/STATE.md`)

If this plan is adopted, three invariants deserve locking so the cleanup does
not have to be redone:

- **MD-33: No file-scope lint suppression.** `#![allow(dead_code)]` (and
  friends) at file scope is forbidden; an unused item is deleted or given a
  consumer. Item-scope allows require a one-line justification comment.
  Enforced by a hygiene guard. *Rationale: 12 such lines hid 22 dead items,
  including a locked-but-unbuilt macro decision (MD-03).*
- **MD-34: `mod.rs` declares, never implements.** A `mod.rs` holds module
  declarations, re-exports, and the layer's `//!` contract — nothing else.
  Implementations live in a file named after the thing they implement.
  Enforced by a size guard. *Extends MD-13 rule 4 with its enforcement.*
- **MD-35: Comments describe the present.** No comment defines code by what it
  used to be. Deliberate absences are stated positively or recorded as an MD
  entry; dead identifiers are grep-guarded. *Rationale: 16 comments referenced
  an IR architecture deleted long ago, two as broken doc links.*

---

## 9. Explicit non-goals

- No behavior change anywhere in this plan. Every phase is refactor-only; the
  oracle is `cargo test --workspace` staying at 1161/0 (plus the ngspice
  cross-checks).
- No crate added or removed. The 10-crate topology is sound; the seams that are
  wrong (CL-01/CL-02) are *within* `piperine-api`, not between crates.
- No touching `parse/`'s grammar or `resolve/diff.rs`'s symbolic
  differentiation semantics (CLAUDE.md's "not to edit casually" list). CL-12
  decomposes parser *functions*; it does not change what they parse.
- No `.specs/` feature-doc rewriting beyond status reconciliation (CL-09).
- Not a V1 blocker. This is ROADMAP **P6**'s remaining half (the hygiene subset
  closed 2026-07-26; this is the architecture/readability subset) and can run
  interleaved with P1/P5/P7 work.
