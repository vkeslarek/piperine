# P6 Cleanup — Architecture & Readability · Pre-feature baseline

**Captured by:** T1 · **Requirement:** D9 (spec Assumptions)
**Commit measured:** `12e235e9b5ccdec5d34350a9bc007ccc8cbfe0be`
(`docs(p6-arch): spec, design, and tasks for the architecture cleanup`)
**Branch:** `feature/bench-removal` · **Date:** 2026-07-26
**Toolchain:** `rustc 1.94.1 (e408947bf 2026-03-25)`, `cargo 1.94.1 (29ea6fb6a 2026-03-24)`
**Tree state:** clean (no uncommitted source changes); `target/` was removed
(`cargo clean`) before measuring, so every number below comes from a cold build.

This file is the numeric oracle for every later task in the feature. A task
states its own counts and they must be **≥ the counts here**, with any decrease
itemized by name (MD-28.3 duplicate deletions and whole-file deletions of dead
code are the only legal decreases).

---

## 1. `cargo test --workspace`

```
1163 passed · 0 failed · 4 ignored · 190 test targets
```

Exit code `0`.

### Per-crate (measured independently with `cargo test -p <crate>`)

| Crate | Targets | Passed | Failed | Ignored |
|---|---:|---:|---:|---:|
| `piperine` (root shell + host suites) | 39 | 166 | 0 | 0 |
| `piperine-api` | 4 | 13 | 0 | 0 |
| `piperine-cli` | 7 | 23 | 0 | 0 |
| `piperine-codegen` | 26 | 152 | 0 | 0 |
| `piperine-lang` | 29 | 355 | 0 | 0 |
| `piperine-lang-server` | 15 | 69 | 0 | 0 |
| `piperine-plugin` | 12 | 49 | 0 | 1 |
| `piperine-plugin-macros` | 5 | 7 | 0 | 0 |
| `piperine-project` | 3 | 26 | 0 | 0 |
| `piperine-python` | 26 | 59 | 0 | 0 |
| `piperine-solver` | 24 | 244 | 0 | 3 |
| **Sum** | **190** | **1163** | **0** | **4** |

The per-crate sum reconciles exactly with the workspace run (190 targets, 1163
passed, 4 ignored). All four ignored items are **doc-tests**, owned by
`piperine-plugin` (1) and `piperine-solver` (3):

| Ignored doc-test | Source |
|---|---|
| `entry (line 121)` | `crates/piperine-plugin/src/lib.rs` |
| `analyses::ac::AcSweepAnalysisOptions::generate_frequencies (line 52)` | `crates/piperine-solver/src/analyses/ac.rs` |
| `core::builder::CircuitBuilder (line 61)` | `crates/piperine-solver/src/core/builder.rs` |
| `prelude (line 7)` | `crates/piperine-solver/src/prelude.rs` |

These are exactly the four ` ```ignore ` fences registered in
`tests/suite_hygiene.rs::every_ignored_doc_example_is_registered`
(`assert_eq!(seen.len(), 4)`). **Expected ignored count stays 4** for the whole
feature; a change means a doc fence was added or removed and the registry must
be updated in the same commit.

> **Counting note.** `CLEANUP_PLAN.md` records "1161 / 0 failed / 4 ignored"
> from `.specs/STATE.md`. The measured figure at this commit is **1163**; the
> two extra tests landed with commits after that snapshot. **1163 is the
> baseline for this feature.**

---

## 2. `cargo clippy --workspace --all-targets -- -D warnings`

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.17s
exit code 0
```

**Zero warnings, zero errors.** Every crate checked: `piperine-lang`,
`piperine-solver`, `piperine-codegen`, `piperine-api`, `piperine`,
`piperine-plugin`, `piperine-python`, `piperine-lang-server`,
`piperine-project`, `piperine-cli`, `piperine-plugin-macros`.

This is the strictest bar in the gate and it is currently green. Every later
task must keep it green.

---

## 3. `cargo doc --workspace --no-deps`

**Exit code `101` — the command FAILS at this baseline.** This is a
pre-existing condition, not introduced by this feature.

### 3.1 The failure is an upstream rustdoc ICE, not a Piperine defect

```
error: internal compiler error: src/librustdoc/passes/collect_intra_doc_links.rs:370:17:
  no resolution for "ToPyArray::to_pyarray" MacroNS DefId(232:648 ~ numpy[a39c]::convert)
   --> ~/.cargo/registry/.../numpy-0.23.0/src/convert.rs:151:5
error: could not document `piperine-python`
error: could not document `piperine-cli`
```

`rustdoc` panics while resolving an intra-doc link **inside the `numpy 0.23.0`
dependency**. It takes down the two crates that depend on it
(`piperine-python`, and `piperine-cli` through it). The other **five** crates
document successfully.

**Consequence for later tasks:** any task whose gate is `build` (which includes
`cargo doc --workspace --no-deps`) cannot get exit `0` from that step until the
ICE is gone. Use the documenting subset as the doc gate:

```sh
cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli
```

and treat "no *new* warnings, ICE unchanged" as the doc criterion. **Do not
"fix" this by touching `numpy`, and do not report it as a regression.**

### 3.2 Warning census (49 rustdoc warnings across 6 crates)

| Crate | rustdoc warnings |
|---|---:|
| `piperine-lang` | 16 |
| `piperine-solver` | 18 |
| `piperine-codegen` | 10 |
| `piperine-api` | 3 |
| `piperine-python` | 1 |
| `piperine-cli` | 1 |
| **Total** | **49** |

By kind:

| Kind | Count |
|---|---:|
| `unresolved link to …` (broken intra-doc link) | **33** |
| `public documentation for X links to private item Y` | 13 |
| `redundant explicit link target` | 2 |
| `` `Error` is both an enum and a derive macro `` | 1 |

Plus 4 warnings that are not rustdoc-content warnings and will not change:

- `output filename collision at target/doc/piperine/index.html` — the
  `piperine-cli` bin target and the root `piperine` lib target share a name
  (known cargo bug rust-lang/cargo#6313).
- 3 × `piperine-cli@0.2.0: piperine-python .so not found …` build-script
  notices.

**The two broken links this feature must fix (CLA-06 / T8)** — both are in the
33-item unresolved-link set:

| Warning | Site |
|---|---|
| `unresolved link to `crate::resolve::IrProgram`` | `crates/piperine-codegen/src/device/mod.rs:8` |
| `unresolved link to `ppr_to_ir`` | `crates/piperine-codegen/src/resolve/pom/mod.rs:51` |

`crates/piperine-lang/src/math.rs:2`'s `IrExpr::eval_const` reference is prose
inside a `//!` header, not a link, so it produces no warning — it is caught by
CLA-06/CLA-08's grep, not by rustdoc.

The other 31 unresolved links are pre-existing and **out of scope** for this
feature (T8 targets only the two named above). Recorded here so a later task
cannot mistake them for new damage. Full list, by crate:

- `piperine-lang`: `validate`, `EventRegistry`, `ElabPass::run`, `ElabContext`
  (×2), `crate::elab::validate` (×2), `eat_ident`, `Lexer::tokenize`, `Stmt`,
  `Id`, `Value::PartialEq`
- `piperine-solver`: `NewtonRaphsonSolver`,
  `crate::analyses::dc::DcAnalysisResult`, `InitialValue`, `Net` (×2),
  `run_digital_at`, `Invalidation::Temperature`, `Invalidation::Restamp`,
  `CircuitBuilder::build`, `UnknownAllocator::branch`,
  `QueryKind::OperatingVariable`, `Introspect::list_state_slot_names`,
  `evaluate_until_stable`
- `piperine-codegen`: `FlatAnalog::read_bounds`, `DigitalKernel`,
  `crate::kernel::analog::AnalogCompiler::compile_jacobian`,
  `QueryKind::OperatingVariable`
- `piperine-api`: `CircuitInstance::set_element_param`
- `piperine-python`: `Self::extension-module`

---

## 4. `tests/ngspice_validation.rs` — the numeric oracle

33 tests, all passing. The suite prints a `PASS` line per case; the cases that
print numbers are the ones later phases (especially the Phase-5 `Session`
collapse and Phase-7/8 codegen work) must reproduce **unchanged**:

| Case | Printed comparison (piperine vs ngspice) |
|---|---|
| `pz_rc` | `.pz` pole `-5.000000e2` vs `-5.000000e2` |
| `four_diode` | `.four` THD `3.5379e-2` vs `3.5459e-2` |
| `disto_diode` | `.disto` HD2 `2.0356e-5` vs `2.0356e-5`, HD3 `6.5573e-6` vs `6.5574e-6` |

The remaining 30 cases assert against golden nodes / sweep points without
printing the values; their per-case shape is the second half of the oracle:

```
divider (2 golden nodes)          jfet_id_vds (26 sweep points)
jfet_bias (1 golden nodes)        bjt_ce (2 golden nodes)
bjt_mirror (2 golden nodes)       diode_iv (37 sweep points)
diode_series (2 golden nodes)     nmos3_fixed (2 golden nodes)
nmos2_id_vds (26 sweep points)    nmos2_fixed (2 golden nodes)
nmos3_id_vds (26 sweep points)    nmos2_id_vgs (26 sweep points)
nmos2_load (1 golden nodes)       nmos3_load (1 golden nodes)
nmos_fixed (2 golden nodes)       nmos3_id_vgs (26 sweep points)
nmos_id_vds (26 sweep points)     urc_lump10 (2 golden nodes)
urc_lump2 (2 golden nodes)        urc_lump5 (2 golden nodes)
rdiode (2 golden nodes)           nmos_id_vgs (21 sweep points)
nmos_load (1 golden nodes)
```

Reproduce with:
`cargo test -p piperine --test ngspice_validation -- --nocapture`

---

## 5. Census

### 5.1 Module-level `fn` count per crate (MD-13 rule 2)

Counting method — a `fn` declaration at **column 0** of a `src/**/*.rs` file:

```sh
grep -hE '^(pub )?(pub\(crate\) )?(pub\(super\) )?(const )?(async )?(unsafe )?(extern "C" )?fn '
```

| Crate | `src` LOC | Module-level `fn` |
|---|---:|---:|
| `piperine-lang` | 15 511 | 75 |
| `piperine-codegen` | 14 918 | **112** |
| `piperine-solver` | 13 629 | **5** ✅ reference |
| `piperine-python` | 5 206 | 11 |
| `piperine-lang-server` | 3 457 | 66 |
| `piperine-api` | 3 389 | 8 |
| `piperine-plugin` | 2 165 | 13 |
| `piperine-project` | 1 472 | 8 |
| `piperine-cli` | 1 222 | 30 |
| `piperine-plugin-macros` | 230 | 4 |
| `piperine` (root `src/`) | 9 | 0 |
| **Total** | **61 208** | **332** |

Reproduces `CLEANUP_PLAN.md` §0's table exactly. Phase 7 (T40) targets
`piperine-codegen`'s 112; Phase 9 (T47/T48) targets `piperine-lang`'s 75 and
`piperine-lang-server`'s 66.

### 5.2 `mod.rs` files over 60 lines (MD-34 / CLA-09)

Under `crates/*/src` and `src` (the guard's scope, T19):

| Lines | File |
|---:|---|
| 1237 | `crates/piperine-codegen/src/device/analog/mod.rs` |
| 864 | `crates/piperine-codegen/src/kernel/analog/mod.rs` |
| 859 | `crates/piperine-codegen/src/device/mod.rs` |
| 615 | `crates/piperine-codegen/src/resolve/pom/mod.rs` |
| 269 | `crates/piperine-solver/src/analyses/mod.rs` |
| 259 | `crates/piperine-lang/src/parse/parser/mod.rs` |
| 229 | `crates/piperine-lang/src/elab/lower/mod.rs` |
| 172 | `crates/piperine-lang/src/elab/mod.rs` |
| 148 | `crates/piperine-lang/src/parse/format/mod.rs` |
| 95 | `crates/piperine-lang/src/parse/mod.rs` |
| 90 | `crates/piperine-lang/src/elab/registry/mod.rs` |
| 90 | `crates/piperine-codegen/src/resolve/mod.rs` |

**12 offenders.** Two more `mod.rs` files exceed 60 lines but sit in `tests/`,
outside the rule's scope: `crates/piperine-lang-server/tests/common/mod.rs`
(783) and `crates/piperine-solver/tests/helpers/mod.rs` (147).

### 5.3 Functions over 200 lines (CLA-25 / T46)

Counting method — brace-balance scan from the `fn` signature line to the line
that closes its body, inclusive; `src` trees only, comments and string literals
stripped before counting braces. This is the same algorithm T46's
`no_function_over_200_lines` guard must implement, so the guard reproduces these
numbers by construction. **2673 functions total.**

| Lines | Site |
|---:|---|
| 581 | `crates/piperine-codegen/src/kernel/analog/compile.rs:218` — `fn compile` |
| 253 | `crates/piperine-lang/src/parse/parser/expr.rs:96` — `fn parse_primary` |
| 253 | `crates/piperine-codegen/src/resolve/pom/mod.rs:315` — `fn lower_bodies` |
| 223 | `crates/piperine-lang/src/elab/lower/module.rs:198` — `fn lower_mod_stmt` |
| 215 | `crates/piperine-lang-server/src/symbol_index.rs:183` — `fn resolve_at` |
| 212 | `crates/piperine-lang/src/parse/parser/stmt.rs:22` — `fn parse_mod_stmt` |
| 207 | `crates/piperine-lang-server/src/handlers/symbols.rs:46` — `fn extract_symbols` |

**7 functions over 200 lines**, not 4. `CLEANUP_PLAN.md` §0 and `design.md`
§C5 name only four (`lower_bodies`, `extract_symbols`, `parse_primary`,
`parse_mod_stmt`) and give `parse_primary` as 316 lines; both figures come from
a different counting method. The three additional offenders this measurement
finds — `kernel/analog/compile.rs::compile` (581), `elab/lower/module.rs::lower_mod_stmt`
(223), `symbol_index.rs::resolve_at` (215) — are **not** in any task's scope
today. T46 ("no function in `crates/*/src` exceeds 200 lines" + the guard)
cannot pass without them, so **T46 must either bring these three under the
ceiling or the guard's scope must be narrowed with a recorded reason.** Flagged
here so Phase 8 does not discover it late.

Functions in the 100–200 band (32, the aspirational tier — the ceiling is 200,
not 60):

| Lines | Site |
|---:|---|
| 169 | `codegen/src/device/builder.rs:143` — `add_instance` |
| 168 | `codegen/src/device/analog/mod.rs:878` — `load_ac` |
| 158 | `solver/src/analyses/pss.rs:168` — `solve` |
| 157 | `lang/src/pom/selector/parse.rs:8` — `from_str` |
| 144 | `lang/src/elab/lower/register.rs:19` — `register_items` |
| 141 | `codegen/src/kernel/digital/compile.rs:448` — `compile` |
| 139 | `codegen/src/device/analog/mod.rs:118` — `new` |
| 136 | `lang/src/pom/design.rs:449` — `introspection_meta` |
| 135 | `lang/src/pom/selector/eval.rs:35` — `eval_step` |
| 134 | `codegen/src/flatten/analog.rs:404` — `walk` |
| 133 | `solver/src/digital/scheduler.rs:98` — `evaluate_dag_ordered` |
| 133 | `lang/src/parse/lexer.rs:349` — `lex_number` |
| 132 | `codegen/src/resolve/pom/expr.rs:104` — `resolve_expr` |
| 131 | `python/src/live.rs:453` — `tran` |
| 129 | `lang-server/src/server.rs:49` — `new` |
| 124 | `codegen/src/kernel/analog/compile.rs:957` — `compile_disto3` |
| 123 | `codegen/src/kernel/digital/compile.rs:93` — `compile` |
| 122 | `lang/src/parse/lexer.rs:222` — `tokenize_all` |
| 118 | `lang/src/elab/lower/module.rs:425` — `lower_instance` |
| 118 | `codegen/src/device/fusion.rs:67` — `fuse_comb_cones` |
| 114 | `codegen/src/resolve/pom/expr.rs:400` — `resolve_call` |
| 113 | `python/src/lib.rs:703` — `instance_path_returns_terminal_subview` (test) |
| 112 | `lang/src/elab/lower/behavior.rs:80` — `lower_stmt_to_behavior` |
| 109 | `codegen/src/device/analog/mod.rs:605` — `load_transient` |
| 107 | `plugin/src/manifest.rs:151` — `parse` |
| 107 | `lang/src/eval/interp.rs:503` — `eval_call` |
| 106 | `python/src/lib.rs:1016` — `ac_returns_complex_waveform_with_projections` (test) |
| 105 | `codegen/src/kernel/analog/compile.rs:1267` — `build_fn` |
| 104 | `lang/src/resolve.rs:137` — `prelude_items` |
| 102 | `project/src/resolver.rs:191` — `resolve_deps` |
| 101 | `lang/src/parse/parser/stmt.rs:281` — `parse_behavior_stmt` |

### 5.4 File-scope lint suppression (CLA-01/02 · MD-33 · T6/T7)

`grep -rn '^#!\[allow(' crates/*/src src` → **12 hits**, all `dead_code`:

| File | Line |
|---|---:|
| `crates/piperine-codegen/src/resolve/pom/mod.rs` | 1 |
| `crates/piperine-solver/src/core/port.rs` | 1 |
| `crates/piperine-solver/src/math/constant.rs` | 1 |
| `crates/piperine-solver/src/analyses/ac.rs` | 4 |
| `crates/piperine-solver/src/analyses/noise.rs` | 4 |
| `crates/piperine-solver/src/analyses/tf.rs` | 4 |
| `crates/piperine-solver/src/analyses/transient.rs` | 5 |
| `crates/piperine-solver/src/analyses/dc.rs` | 6 |
| `crates/piperine-solver/src/analyses/events.rs` | 25 |
| `crates/piperine-solver/src/analyses/sp.rs` | 28 |
| `crates/piperine-solver/src/analyses/pz.rs` | 32 |
| `crates/piperine-solver/src/analyses/disto.rs` | 46 |

**Target after T6: 0.** T7's guard keeps it there.

### 5.5 Item-scope `#[allow(dead_code)]` (CLA-01 AC3 · T6)

**8 sites** (T6's brief lists 7; it omits `solver/src/digital/events.rs:86`,
which is inside a `#[cfg(test)]` module):

| Site | Item | Has a justification comment today? |
|---|---|---|
| `codegen/src/emit/builder.rs:146` | `fn layout` | no |
| `codegen/src/emit/builder.rs:150` | `fn ptrs` | no |
| `codegen/src/emit/builder.rs:154` | `fn reads` | no |
| `lang/src/resolve.rs:43` | `ResolveError::NotFound` variant | no |
| `lang-server/src/state.rs:74` | `fn dummy` | partial ("for testing") |
| `lang-server/src/handlers/diagnostics.rs:145` | `fn extract_error_range` | **yes** ("Test-support surface: the integration tests are the only consumers.") |
| `solver/src/analyses/pz.rs:77` | field `circuit` | no |
| `solver/src/digital/events.rs:86` | `struct MockInverter` (in `#[cfg(test)]`) | no |

Other item-scope allows in the tree (**not** `dead_code`, out of T6's scope):
`unused_imports` ×2 (`codegen/src/emit/mod.rs:11`, `solver/src/analyses/transient.rs:238`),
`clippy::too_many_arguments` ×10, `clippy::type_complexity` ×4,
`clippy::large_enum_variant` ×2, `clippy::mutable_key_type` ×2,
`clippy::should_implement_trait` ×1, `deprecated` ×9 (`lang-server` symbol
handlers, required by `lsp-types` struct literals).

### 5.6 Dead-architecture identifier hits (CLA-07/08 · MD-35 · T9/T10)

`grep -rEn 'IrProgram|IrModule|IrExpr|IrInstance|piperine[-_]ir' crates/*/src src`
→ **16 hits in 11 files**. Target after T9: **1** (in
`crates/piperine-codegen/src/lib.rs`).

| File:line | Text |
|---|---|
| `codegen/src/lib.rs:13` | `POM→resolved pass (formerly the standalone `piperine-ir` crate +` ← **the one survivor** |
| `codegen/src/resolve/mod.rs:1` | `The resolved lowering layer — formerly the standalone `piperine-ir`` |
| `codegen/src/resolve/mod.rs:84` | `` `device::circuit` … there is no `IrModule`/ `` |
| `codegen/src/resolve/mod.rs:85` | `` `IrInstance` structural twin. `` |
| `codegen/src/resolve/expr.rs:1` | `Operator types shared by digital and analog codegen. The old `IrExpr`` |
| `codegen/src/resolve/stmt.rs:2` | `These now carry POM `Expr` (not `IrExpr`) — the resolved-id form is gone.` |
| `codegen/src/resolve/pom/mod.rs:3` | `` [`LoweredBody`] — no separate IR crate, no `IrModule`/`IrProgram` `` |
| `codegen/src/resolve/pom/mod.rs:40` | `` `IrModule::new`). `` |
| `codegen/src/device/mod.rs:8` | `` [`CircuitCompiler`] — walks an [`crate::resolve::IrProgram`]'s top module `` ← also a rustdoc warning |
| `codegen/src/device/circuit.rs:8` | `` there is no `IrModule`/`IrInstance`/`IrProgram` structural twin `` |
| `codegen/src/device/circuit.rs:253` | `` `IrInstance.connections` structural twin. `` |
| `codegen/src/device/builder.rs:86` | `` do once for every module's `IrInstance.connections` `` |
| `codegen/src/emit/analog_expr.rs:26` | `` `Expr` instead of `IrExpr`. `` |
| `codegen/src/flatten/analog.rs:2` | `` Operates entirely on POM `Expr`/`Stmt` — no `IrExpr`. `` |
| `lang/src/math.rs:2` | `` the matching compile-time evaluator used by `IrExpr::eval_const`. `` |
| `lang/src/elab/lower/module.rs:212` | `` access into `IrExpr::Param("model_rsh")` — see `` |

T9's `Where` names 10 files; this list matches it, plus
`crates/piperine-codegen/src/resolve/expr.rs` (already in T9's list) — all 11
files are covered.

### 5.7 The 22 dead items T2–T5 delete

Measured by removing all 12 file-scope allows and running
`cargo check --workspace --all-targets`.

| # | Item | Site | Task |
|---:|---|---|---|
| 1–8 | `PI`, `E`, `I`, `SPEED_OF_LIGHT`, `ABSOLUTE_ZERO_CELSIUS`, `ELEMENTARY_CHARGE`, `BOLTZMANN_CONSTANT`, `PLANCK_CONSTANT` | `solver/src/math/constant.rs` (whole file) | T2 |
| 9 | `enum Port` | `solver/src/core/port.rs` (whole file) | T2 |
| 10 | `trait DcAnalysis` | `solver/src/analyses/dc.rs:63` | T3 |
| 11 | `trait AcAnalysis` | `solver/src/analyses/ac.rs:25` | T3 |
| 12 | `trait TransientAnalysis` | `solver/src/analyses/transient.rs:209` | T3 |
| 13 | `trait NoiseSource` | `solver/src/analyses/noise.rs:40` | T3 |
| 14 | `struct AcContext` | `solver/src/analyses/ac.rs:86` | T4 |
| 15 | `struct TransientContext` | `solver/src/analyses/transient.rs:167` | T4 |
| 16 | `struct NoiseContext` | `solver/src/analyses/noise.rs:60` | T4 |
| 17 | `struct TfContext` | `solver/src/analyses/tf.rs:55` | T4 |
| 18 | 5 methods: `require_param_given`, `lookup_param`, `lookup_var`, `require_ident_as_param`, `require_var` | `codegen/src/resolve/pom/mod.rs:180,228,233,282,294` | T5 |
| 19 | `fn has_ddt_marker` | `codegen/src/resolve/pom/stmt.rs:44` | T5 |
| 20 | `fn contrib_branch` | `codegen/src/resolve/pom/stmt.rs:61` | T5 |
| 21 | field `bundle_name` | `codegen/src/resolve/pom/structure.rs:14` | T5 |
| 22 | field `options` | `solver/src/analyses/pz.rs:79` | T5 |

Two corrections to the task briefs, recorded so the tasks do not fail on a
wrong line number:

- `pz.rs`'s dead field `options` is at **line 79**, not 78. Line 78 is the
  field `circuit`, which already carries its own `#[allow(dead_code)]` and is
  T6's item to triage.
- `codegen/src/emit/resolver.rs:33` carries a doc reference to
  `LowerCtx::require_param_given` ("keep the two in sync"). It is orphaned the
  moment T5 deletes that method, so T5 must also correct that line — one extra
  file beyond its declared `Where`.

`NoiseSource` is a **name collision**: `codegen/src/resolve/symbols.rs:182`
declares a live `pub struct NoiseSource` used across `resolve/`. Only the
**solver trait** at `analyses/noise.rs:40` is dead.

### 5.8 `TODO`/`FIXME`/`HACK` in `src`

0 — unchanged from `CLEANUP_PLAN.md` §0.

---

## 6. Gate commands as they actually behave at this baseline

| Gate | Command | Baseline result |
|---|---|---|
| Quick | `cargo test -p <crate>` | green, counts in §1 |
| Full | `cargo test --workspace` | `1163 passed · 0 failed · 4 ignored`, exit 0 |
| Build (build) | `cargo build --workspace` | exit 0, zero warnings |
| Build (lint) | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, zero warnings |
| Build (test) | `cargo test --workspace` | exit 0 |
| Build (doc) | `cargo doc --workspace --no-deps` | **exit 101 — upstream `numpy 0.23` rustdoc ICE (§3.1)**; use `--exclude piperine-python --exclude piperine-cli`, 49 pre-existing warnings |
