# SIMPLIFICATION.md — Architecture Simplification Plan

**Status: reviewed and approved (2026-07-03), expanded.** Decisions from review:
the wide-but-defaulted `Device` trait stays as-is (idiomatic, room to grow); the solver's
Newton/analysis layering stays as-is; `piperine-ams` is **deleted**, not archived.

Each item carries an **owner tag**:
- `[structural]` — cross-crate, order-sensitive, high blast radius. Done by the primary
  agent (Claude), suite green after every step.
- `[delegate]` — mechanical, locally verifiable, safe for a smaller model or a quick
  manual pass. Each has explicit acceptance criteria.

**Goal.** One representation per stage, one owner per concept, one error story, no silent
fallbacks — so that reading one end-to-end flow (source text → solved waveform) fits in
one sitting.

---

## 1. The pipeline today (what a value actually flows through)

```
.phdl text
  │  parse::Lexer → Parser                              [piperine-lang/parse]
  ▼
ast::SourceFile        Stmt / BehaviorStmt / ModuleStatement / Expr   (3 stmt enums)
  │  SourceFile::elaborate  →  Elaborator (god struct, 5 files)       [piperine-lang/elab/lower]
  │  const-eval: eval::Interpreter<ConstHost>  (values: ConstVal)
  ▼
pom::Design            Module / Instance / pom::BehaviorStmt (4th)    (values: pom::Value)
  │  lowering::ppr_to_ir  ("the other lowering")        [piperine-lang/lowering]
  │  infallible — errors become silent Real(0.0)/ParamId(0)!
  ▼
ir::IrProgram          IrStmt (5th) / IrExpr            (defined in piperine-codegen,
  │                                                      but piperine-lang depends on it)
  │  CircuitCompiler → flatten → Cranelift JIT          [piperine-codegen/device+jit]
  ▼
CircuitInstance        Box<dyn Device>                                [piperine-solver]
  │  dc/transient/ac/noise/tf analyses
  ▼
DcAnalysisResult / TransientAnalysisResult / …          (values keyed by NodeIdentifier —
                                                         a second node-id type)
  │  bench: eval::Interpreter<SimHost> reads it back    [piperine-bench]
  ▼
OpResult / Trace / Waveform                             (values: eval::Value)
```

Side casts along the way: `ConstVal ↔ pom::Value ↔ eval::Value ↔ IrExpr literals`
(16 conversion sites), `NodeId → NodeIdentifier`, net-name strings → ids → names again.

Also in the repo but **outside** this pipeline: `piperine-ams` (6.4k LOC Verilog-A
frontend, excluded from the workspace, zero dependents, doesn't build in CI).

---

## 2. Diagnosis — where the complexity actually is

| # | Symptom | Evidence |
|---|---------|----------|
| D1 | Value-type triplication | `ConstVal`, `pom::Value`, `eval::Value` + 16 conversion sites |
| D2 | Statement-enum quintuplication | `ast::Stmt`, `ast::BehaviorStmt`, `ast::ModuleStatement`, `pom::BehaviorStmt`, `IrStmt` — the first two are ~90% identical, the fourth is a re-typed clone of the second |
| D3 | Hand-rolled AST walkers | `subst_const`, `collect_syscalls`, `resolve_calls_in_*`, `scan_noise`, typecheck, eval, to-IR lowering, formatter, `predict.rs` — 9 independent recursions over the same `Expr`; every new variant (e.g. `Tuple`) means fixing all of them by compiler error |
| D4 | Two "lowerings" | `elab/lower/` (AST→Design) and `lowering/` (Design→IR) — same word, different phases |
| D5 | Silent fallbacks in Design→IR | 11 sites: `unwrap_or(ParamId(0))`, `IrExpr::Real(0.0)` for unknown names; `ppr_to_ir` is infallible by signature so it *cannot* fail loud — direct violation of the project's own fail-loud rule (this is what hid the digital-read-in-analog bug) |
| D6 | Backwards dependency | the IR lives in `piperine-codegen`, so the *frontend* depends on the *backend*; `eval` reaches into `codegen::jit::math` for pure math |
| D7 | God structs | `Elaborator` (self-documented as "one god struct", 5 files of methods), `LowerCtx`, `InstanceBuilder` |
| D8 | Legacy solver entry | `Circuit` builder + `CircuitInstance::instantiate` used only by tests; production always goes through `from_devices_and_netlist` |
| D10 | Dead weight | `piperine-ams` (0 dependents), 25 compiler warnings in piperine-lang alone, unreachable selector axes, `parse_ident_as`, `insert_module`, `lookup_instance_port`, `discipline_name` |
| D11 | Error-type sprawl | 9 error types, 4 styles: thiserror enums, miette-layered, solver's hand-rolled `Error` struct, bare `String` (selector, project, scattered) |
| D12 | Two node-id types | codegen `NodeId` vs solver `NodeIdentifier`, plus name-string round-trips in bench (`CircuitBuildInfo.nets`) |
| D13 | Doc drift | `lib.rs` documents a `runtime` module that doesn't exist; ARCHITECTURE.md was deleted with the old IPC architecture and never replaced; CLAUDE.md pipeline diagram shows AMS as active |

*(D9 — the 22-method `Device` trait — was reviewed and kept: defaulted methods with two
real implementors is idiomatic Rust and leaves room for device growth. Likewise the
solver's `analysis/*` + `solver/*` layering stays; it reads fine at its current size.)*

---

## 3. The plan, expanded

### P1 — Extract `piperine-ir`: the IR becomes the contract crate
**[structural — Claude]** *(fixes D6; enables P2, P12; partial D12)*

New crate `crates/piperine-ir`: pure data, no Cranelift, no solver.

1. Move `piperine-codegen/src/ir/{mod,expr,stmt,symbols,validate}.rs` →
   `piperine-ir/src/`, preserving module paths (`piperine_ir::{IrProgram, IrExpr, …}`).
   `IrExpr::eval_const` comes along (it is pure).
2. `piperine-codegen` re-exports `pub use piperine_ir as ir;` during the transition so
   downstream churn is a dependency edit, not an import rewrite; direct imports migrate
   opportunistically.
3. `piperine-lang` dependency flip: `piperine-codegen` (and `piperine-solver`) move from
   `[dependencies]` to `[dev-dependencies]` — the library only needs `piperine-ir`.
   The one production reach-in, `eval::tasks` → `codegen::jit::math::eval_const_math`,
   is replaced by a small pure dispatch inside `eval` (plain `f64` methods; only the JIT
   needs linkable symbols — bit-identical results still guaranteed for the shared subset
   because both call the same libm-backed `f64` intrinsics).
4. `piperine-bench`/CLI unchanged (they legitimately span frontend+backend).

Acceptance: workspace builds; `cargo tree -p piperine-lang -e normal` shows **no**
piperine-codegen/piperine-solver; full suite green.

Deliberately *not* in this step: making piperine-solver consume `piperine-ir` node ids
(the `NodeIdentifier` unification, D12). That is follow-up work once ir exists — tracked
under P12b below.

### P2 — One `Value` type
**[structural — Claude]** *(fixes D1)*

`eval::Value` is already the superset (scalars + tuple/list/record/option + closures +
host objects). Make it *the* value type of the frontend:

1. Move it to `piperine_lang::value::Value` (re-exported at crate root; `eval` and `pom`
   both use it).
2. Delete `ConstVal`: `ConstEnv` stores/returns `Value`; the narrowing that `ConstVal`
   encoded becomes one helper (`Value::as_const_scalar() -> Result<…>`) used by the few
   sites that must reject collections (array dims, discriminants, param folds).
3. Delete `pom::Value`: `Param.default`, `Instance.params`, `Design.consts`, staging all
   store `Value`. `Complex` scalars join the unified enum (POM had it, eval didn't).
4. `PartialEq`: manual impl — scalar variants compare by value, collections structurally,
   closures/objects always `false` (documented).
5. The 16 conversion impls collapse to ~2 (`Value ↔ IrExpr` literal at the lowering
   boundary).

Acceptance: `grep -rn "ConstVal" crates/` → only git history; conversion-impl count ≤ 3;
suite green.

### P3 — One statement enum for behavior bodies
**[structural — Claude, last]** *(fixes D2)*

- Merge `ast::BehaviorStmt` into `ast::Stmt` (add `Event`; `Bind` already shared).
  Context validation (already in `elab/validate`) decides legality, not type duplication.
- Delete `pom::BehaviorStmt`: POM stores the resolved AST statements plus the side
  tables it genuinely adds (resolved types), instead of a 1:1 re-typed clone.
- End state: `ast::Stmt` (surface) → `IrStmt` (executable); `ModuleStatement` stays
  (structural items genuinely differ).

Biggest churn; scheduled after P4 so only one walker needs porting.

### P4 — A real walk/fold on the AST
**[structural — Claude]** *(fixes D3)*

Plain inherent methods, no macros:

```rust
impl Expr {
    pub fn walk(&self, f: &mut impl FnMut(&Expr));
    pub fn walk_mut(&mut self, f: &mut impl FnMut(&mut Expr));
}
impl Stmt { pub fn walk_exprs(&self, f: &mut impl FnMut(&Expr)); /* + _mut */ }
```

Port `subst_const`, `collect_syscalls`, `resolve_calls_in_*`, `scan_noise`, and the
typecheck scan onto these (each becomes a ~10-line closure). The eval interpreter, the
to-IR lowering, and the formatter keep their own matches — they *transform*, they don't
*walk*.

Acceptance: adding a dummy `Expr` variant requires touching exactly: `walk`,
`walk_mut`, eval, to-IR, formatter, predict — and nothing else compiles-and-misses.

### P5 — `ppr_to_ir` returns `Result`; delete every silent fallback
**[structural — Claude]** *(fixes D5)*

`ppr_to_ir(&Design) -> Result<IrProgram, LowerError>`. The 11 `unwrap_or(ParamId(0))` /
`IrExpr::Real(0.0)` sites become typed errors naming symbol + module. Callers (bench
session, tests, CLI) already sit behind `Result` plumbing.

Acceptance: `grep -n "unwrap_or(ParamId(0))\|unwrap_or(VarId(0))" crates/piperine-lang/src`
→ zero; a test proving an unknown name in a contribution *errors* instead of stamping 0.

### P6 — Elaborator: god struct → pass list
**[structural — Claude]** *(fixes D7)*

Data in a plain `ElabCtx` (symbol tables + Design under construction); logic as ordered
passes, one file each:

```rust
let passes: [&dyn ElabPass; N] =
    [&Register, &ResolveTypes, &ElabModules, &AttachBehaviors, &AttachBenches, &Validate];
```

`elaborate()` becomes a readable loop; elaboration *order* becomes visible instead of
buried in a 280-line driver.

### P7 — Delete the legacy `Circuit`/`instantiate` solver entry
**[delegate]** *(fixes D8 — reduced scope per review: Newton/analysis layering and the
`Device` trait stay as they are)*

Remove `Circuit` builder + `CircuitInstance::instantiate` + the OSDI-from-`Circuit`
wiring in `piperine-solver/src/circuit.rs`; port their tests to
`from_devices_and_netlist` (the production path).

Acceptance: `grep -rn "instantiate\b" crates/piperine-solver` → gone; solver + osdi test
files still pass.

### P8 — Delete `piperine-ams`
**[delegate]** *(fixes part of D10, D13 — review decision: delete, don't archive)*

`git rm -r crates/piperine-ams`; drop the `exclude` from the root `Cargo.toml`; update
CLAUDE.md / AGENTS.md / docs/GAPS.md so the pipeline is PHDL-only (the `.va` frontend is
a future milestone, to be rebuilt against `piperine-ir` when it returns — git history
keeps the old code).

Acceptance: `grep -rni "piperine.ams\|\.vams\|ams_to_ir" --include="*.md" --include="*.toml"`
→ only historical notes explicitly marked as removed; workspace builds.

### P9 — One error story
**[delegate, per-crate increments]** *(fixes D11)*

- One public thiserror enum per crate, `#[from]` chaining.
- Solver's hand-rolled `Error{title, detail, cause}` → thiserror enum (keep the display
  format; the struct's shape becomes variant fields).
- `String` errors → typed: `pom/selector` (parse+eval), `piperine-project`, CLI `check`.
- `miette` only at the presentation boundary (CLI + LSP); internal code never builds
  miette reports.

Acceptance per increment: crate's public API exposes exactly one error type; no
`Result<_, String>` in its `src/`.

### P10 — Naming and docs honesty
**[delegate, after P1/P6 land]** *(fixes D4, D13)*

- `lowering/` → `to_ir/` (module rename + re-export shim for one release).
- `elab/lower/` naming disappears naturally with P6's passes.
- Fix `lib.rs` module table (ghost `runtime`, phantom `from_ir`).
- Recreate a short `ARCHITECTURE.md` — §1's diagram, kept current, is the template.
- CLAUDE.md: pipeline diagram updated (AMS removed, bench layer added).

### P11 — Dead-code and hygiene sweep
**[delegate — good first task]** *(fixes D10)*

Fix the 25 warnings in piperine-lang (unused imports, unreachable selector-axes arm,
`parse_ident_as`, `insert_module`, `lookup_instance_port`, `discipline_name`, unused
locals in lowering/structure.rs); then the stragglers in cli. Split `jit/digital.rs`
(1299 lines, largest file in the repo) into `layout` / `abi` / `compile` modules —
move-only, no logic change. CI gets `RUSTFLAGS="-D warnings"` once the count is zero.

Acceptance: `cargo build --workspace 2>&1 | grep -c warning` → 0; digital.rs ≤ ~400
lines per module; suite green.

### P12 — Symbol resolution once, ids afterwards
**[structural — Claude, rides on P5]**

Resolve every name exactly once at the start of Design→IR lowering into id-keyed maps
(`HashMap<String, NodeId>` built once per module, `instance_ports` keyed by
`(InstanceId, PortId)` instead of `"label.port"` strings); downstream code touches only
ids. Names reappear only in diagnostics and bench's user surface.

### P12b — Solver keyed by ir ids (follow-up to P1)
**[structural — Claude, optional this pass]** *(finishes D12)*

`piperine-solver` takes a dependency on `piperine-ir` and `NodeIdentifier` becomes a
newtype over (or alias of) the ir node id; `CircuitBuildInfo.nets`' name→id map shrinks
to the bench-facing surface only. Do only if P1 lands cleanly with appetite left —
independently shippable.

---

## 4. Target picture

```
                       ┌────────────────────┐
   .phdl ──────────────▶   piperine-lang    │  parse → elaborate(passes) → Design
                       │  (frontend only)   │  Design → IrProgram (to_ir, fallible)
                       └─────────┬──────────┘
                                 │ IrProgram                (piperine-ir: shared data contract,
                                 ▼                           Value at the lang boundary, validate)
                       ┌────────────────────┐
                       │  piperine-codegen  │  IrProgram → JIT kernels → Devices
                       └─────────┬──────────┘
                                 │ CircuitInstance
                                 ▼
                       ┌────────────────────┐
                       │  piperine-solver   │  analyses as-is; legacy Circuit path deleted
                       └─────────┬──────────┘
                                 │ results
                                 ▼
                       ┌────────────────────┐
                       │  piperine-bench    │  eval::Interpreter<SimHost>, results OM
                       └────────────────────┘
   piperine-cli / piperine-lang-server: presentation boundary (miette lives here)
   piperine-ams: deleted (git history keeps it)
```

One value type, two statement enums, one error enum per crate, every arrow fallible and
loud, dependency arrows = pipeline arrows.

---

## 5. Execution order and ownership

| Step | Items | Owner |
|------|-------|-------|
| 0 | P11 dead code · P7 legacy Circuit · P8 delete ams | **delegate** (parallel, independent) |
| 1 | **P1** piperine-ir extraction | **Claude** ✅ done |
| 2 | **P2** one Value | **Claude** ✅ done |
| 3 | P4 walk/fold | Claude |
| 4 | P5 fallible to_ir + P12 resolve-once | Claude |
| 5 | P6 elab passes | Claude |
| 6 | P3 stmt merge | Claude (last, biggest churn) |
| any | P9 errors (per-crate) · P10 docs (after P1/P6) | delegate |
| opt | P12b solver ids | Claude, if appetite |

Every step ships with the full suite green (the two pre-existing failures —
`a5_neg_in_digital_still_works` and the lib.rs doctest — are tracked separately and
predate this work; P10 fixes the doctest as a side effect).

## 6. Non-goals (explicitly out)

- No new DSL/macros for visitors or devices (project rule: data + plain helpers).
- No solver-internals refactor (review decision): Newton/analysis layering and the
  `Device` trait are fine at current size.
- No performance work — this is a legibility pass.
- No bench/SPEC_BENCH feature work here — the conformance gaps are *inventoried* in
  Appendix A below, but implementing them is milestone-2 work, not simplification.
- No rewrite of the hand-written parsers; P3/P4 touch AST *types*, the recursive-descent
  structure stays.

---

## Appendix A — SPEC_BENCH.md conformance audit (as of 2026-07-03)

What the spec promises vs. what `piperine-bench` + the `bench` frontend actually do.
Legend: ✅ conforms · ⚠️ implemented but diverges from the spec's shape · ❌ missing
(and, per the fail-loud rule, rejected at elaboration — never a silent no-op).

### A.1 Working and conforming

| Spec | Status |
|------|--------|
| §2 `bench ModName { fn … }`, attach by name, zero-arg fns are entry points, helpers callable | ✅ |
| §3 bare-name resolution: nets/wires/ports → `NetRef`, instances → `InstanceRef`, `inst.port`/`inst.param`, `gnd`/ground family, global consts, enum variants | ✅ (see A.3 for the params gap) |
| §4/§6 `OpResult.v(a, b)` and `.v(a)` (defaulted second arg); `.i(a, b)` instance-port form and unique-two-terminal form, force-branch + DC-residual readout | ✅ |
| §5 `$op()` (no-arg) | ✅ |
| §6 `Trace.v/.axis`, `Waveform.{at,min,max,mean,rms,peak_to_peak,len,points,cross}` | ✅ |
| §7 bare-name staging (`sw.ctrl = 1`) → override → next analysis re-elaborates | ✅ |
| §9 determinism/isolation: fresh `Design::fork()` per entry point, overrides accumulate until next analysis, results are immutable snapshots | ✅ (tested) |
| §11 fail-loud availability: calling any unimplemented task is an **elaboration error** | ✅ |
| §12.1 open/closed-switch example; §12.4 sweep-as-`for` (minus `$write`) | ✅ (bench crate tests) |
| `piperine test` discovers and runs entry points, per-fn pass/fail, nonzero exit | ✅ |

### A.2 Implemented but divergent (⚠️)

| Spec | Divergence |
|------|-----------|
| §5 `$tran(cfg: TranConfig)` | takes **positional** `(stop, step)`; no config bundle, no adaptive-step "auto", no `ic:` map |
| §5/§5.1 `$op(cfg: OpConfig = OpConfig {})` | cfg argument not accepted; always default `Solver` settings — the stdlib `Solver`/`OpConfig`/`TranConfig`/`AcConfig`/`NoiseConfig` bundles do not exist in the prelude at all |
| §6 `Trace.i(a, b)` | force-device (ideal-source) branches only — no reactive/residual readout over time |
| §6 `i(a: Net, b: Net = gnd)` | `.i` requires exactly 2 args (no defaulted-`gnd` one-arg form, unlike `.v`) |
| §6 `cross(level, dir: CrossDir = Either)` | accepts an enum-variant *or a string* (`"Rising"`); no stdlib `CrossDir` enum is declared, so spec-style bare `Rising` only works if the user defines the enum |
| §3 "generics appear in concrete form" | a `bench` naming a generic/monomorphized module is an elaboration **error** (milestone-1 restriction, documented) |

### A.3 Missing (❌)

| Spec | Notes | Suggested owner |
|------|-------|-----------------|
| §3 bench-module **params** by bare name | `SimHost::lookup` resolves wires/ports/instances/consts/enums but **not** the bench module's own `param`s — spec §3 item 2 lists params explicitly | delegate (small, `host.rs`) |
| §5 `$ac(cfg)` → `Trace` of Complex | solver AC exists (`AcSolver::solve_sweep`); needs `Waveform<Complex>` + `mag/phase/db` | structural |
| §5 `$noise(cfg)` → `NoiseTrace.{psd,total}` | solver noise analysis exists; thin wrapper | structural |
| §5.1 config bundles as **stdlib prelude** (`Solver`, `OpConfig`, …) + analyses reading them | interpreter already evaluates bundle literals (`Value::Record` with field defaults); missing: prelude declarations + task-side field readout | structural |
| §6 `Waveform.{map, fft, rise_time, fall_time}` and the Complex methods `mag/phase/db` | `rise_time`/`fall_time` are listed in the spec's core set, not the deferred algebra | delegate (pure Rust on `waveform.rs`, no cross-crate surface) |
| §5.1 `Map<K, V>` value type (`nodeset`/`ic` maps) | no literal syntax, no `Value` variant — also flagged in SPEC.md §6.1 as reserved | structural (language) |
| §7 `select("//…")` **from a bench** (bulk staging / bulk measurement) | `Design::select` exists in Rust; not exposed as a bench callable, selector writes not wired to staging | structural |
| §8 uniform API: `Design::op/tran/ac/noise`, `Module` handles, `load()`, Python/Rust parity | entirely absent — deliberately deferred (library/bindings surface) | structural, milestone 3 |
| §10 default parameter values on **user-defined** fns (`fn foo(x: Real = 1.0)`) | Part I §9 language addition; built-in methods fake it by arity today | structural (parser + interpreter) |
| §11 `$plot(waveform, title)` / `$write(path, …)` | elaboration-rejected today; `$write` needed by spec example §12.4 | delegate (SimTask + fs/csv emit) |
| §11 `extract` / `.attach` / `.meta` | extensibility spec not started | out of scope until that spec lands |
| §2 `piperine run` executing flows | still a `TODO` stub (only `piperine test` is wired); spec names both | delegate (mirror of test.rs) |
| §12.2 / §12.3 worked examples | blocked on `TranConfig` bundle and `$ac`+`db()` respectively | follows the items above |

### A.4 Spec-side cleanups owed

- §11's status markers (added during milestone 1) must be kept in sync as A.3 items land.
- When config bundles land, delete the "positional `(stop, step)`" divergence note and
  the §14 "default-argument ordering" question gets a concrete test case.
- `CrossDir`, `Scale` enums: decide stdlib-prelude vs. built-in — today neither exists.
