# Piperine — Claude Code Instructions

## Project summary

Piperine compiles **Verilog-AMS** (LRM 2.4, `.vams`/`.va` — `piperine-lang` is the
parser+elaborator, written from `VAMS-LRM-2-4.md` Annex A only) straight into a **native in-house circuit
solver** (Cranelift-JIT analog devices + an event-driven digital interpreter). No external
SPICE dependency. Verilog-A device models also load as compiled OSDI (v0.4) shared
libraries through the **`piperine-osdi` plugin** (external repo — the solver core has no
OSDI/libloading dependency) for models Piperine's own `.va`/`.vams` stdlib doesn't cover.
**PHDL (`.phdl`/`.ppr`) has been retired entirely** (MD-44/MD-45, `verilog-ams-support`
feature) — Verilog-AMS is the sole frontend, zero `.phdl` sources remain in the tree
(`tests/suite_hygiene.rs::phdl_corpus_does_not_grow` is the regression guard). Rust
workspace, edition 2024.

## Pipeline (the spine)

```
Verilog-AMS (.vams/.va) ──parse_and_elaborate──► Design (POM)
                                        │
                                        ▼
                            piperine_codegen::resolve::lower_bodies
                            (Design ──► LoweredBody per module)
                                        │
                                        ▼
                            CircuitCompiler::new(&design, &bodies)
                                        │
                                        ▼
                            CompiledModule (AnalogKernel JIT + DigitalKernel)
                                        │
                                        ▼
                            PiperineDevice ──► solver
                                        │
                                        ▼
                            hosts: `piperine-api` lib (Rust) / `import piperine` (Python)
```

The POM (`Design`/`Module`, from `piperine-lang`) is the single object model;
`piperine_codegen::resolve` is codegen's own resolved form — expressions with
interned ids, symbolic differentiation (`resolve/diff.rs`), the POM→resolved pass
(`resolve/pom/`, `lower_bodies`). `resolve` stays `pub` (hosts/tests address it by deep path,
e.g. `resolve::pom::LoweredBody`) but nothing outside `piperine-codegen` depends on its shape.
A resolved body covers one module's *own* behavior; `CircuitCompiler` reads all instance
structure (connections, param overrides) straight from the POM
`Design`/`Module`/`Instance` at circuit-build time.
"100% coverage" means: every Verilog-AMS construct lowers to executable device code. When
something cannot be faithfully lowered, **fail loud** (`CodegenError::Unsupported`) — never
silently emit `0.0` or a no-op.

### UNBREAKABLE RULE — POM navigability mirrors the source

**The POM's navigability reflects the structure of the original code, never the internal
structure of elaboration.** A device author reads back their own modules, instances, and
hierarchy from the POM exactly as written; internal transforms (hierarchy flattening above
all) are codegen concerns they must never leak into `Design::modules`. Every elaboration pass
builds the POM from the immutable AST and only *adds* or *validates* — it never overwrites
authored structure. Monomorphization may *name* a concrete variant (`urc__5`) but must keep
the `instance → submodule → sub-instances` tree walkable. A transform that needs a
collapsed/flattened form (e.g. for codegen) produces a **separate side artifact**
(`Design::flat_modules`, `#[serde(skip)]`), leaving the authored hierarchy intact. See
`.specs/features/hierarchy-flattening/design.md`.

## Build and test

```sh
cargo build --workspace           # build all crates — zero warnings is the bar
cargo test  --workspace           # the whole suite (every crate's targets)
cargo test -p piperine-solver     # one crate
cargo test <name>                 # one test
cargo test -- --nocapture         # see solver output
```

**`cargo test` bare at the repo root only runs the root package** (root `Cargo.toml` is
both a package and the workspace) — always pass `--workspace`.

## Crate responsibilities

| Crate | Role |
|-------|------|
| `piperine-lang` | Parser **and** elaborator for Verilog-AMS (LRM 2.4) in one crate (MD-47 — merged from the former `piperine-vams` + `piperine-lang` split, MD-43's purity-gate boundary retired as a cost with no independent consumer). Lexer/preprocessor/grammar (`` `define``/`` `include``/`` `ifdef`` in `preprocessor.rs`, the Annex A grammar in `grammar/`) into `ast::SourceFile`, then the elaborator over that AST: source → POM `Design` (`elab/`, `pom/`), a const evaluator over `ast::Expr` directly (`elab/const_eval.rs` — no PHDL-era tree-walking `Interpreter`/`Host`, see `eval/mod.rs`'s own `//!`), and pure ch. 9 system-task/VPI-registration helpers in `eval/tasks.rs`. `parse_and_elaborate` is the entry point. Grammar/semantics come from `VAMS-LRM-2-4.md` Annex A only, never a third-party Verilog-A compiler. Builtin stdlib headers in `headers/` (Annex D: `constants.vams`, `disciplines.vams`) and `headers/spice/` (the ngspice-faithful `.va` device models — `` `include "spice/<file>.va" `` resolves in any project with no dependency, via the builtin `spice` namespace `SourceMap` seeds). |
| `piperine-codegen` | POM → devices, one module per pipeline stage: `pom::Design ─▶ resolve ─▶ flatten ─▶ emit ─▶ kernel ─▶ device`. `resolve/` (resolved form: `expr.rs`/`stmt.rs`/`symbols.rs`, `diff.rs` symbolic differentiation, `pom/` `lower_bodies`). `flatten/` (resolved analog → `FlatAnalog`, crate-private). `emit/` (Cranelift emission machinery: `Builder`, `Codegen` trait, CSE, `SimCtx` ABI, crate-private). `kernel/`: `analog/` (`AnalogKernel`, capability sub-structs behind `Option`), `digital/`. `device/`: `analog/` (`AnalogInstance`, capability files `forces.rs`/`limits.rs`/`operators.rs`/`events.rs`), `digital.rs`, `circuit.rs`/`builder.rs`/`fusion.rs`/`plugin.rs` (`CircuitCompiler`) → `PiperineDevice` (implements `Element`). Public surface is a single `lib.rs` façade (MD-23) — `resolve`/`kernel`/`device` stay `pub`, `emit`/`flatten`/`error` are crate-private. |
| `piperine-solver` | Native solver: DC/AC/transient/noise/TF and the rest of the analysis family (`analyses/`), MNA/linear algebra (`math/`, faer), `Element` trait + `ElementCapabilities` (`core/element.rs`), `Net` naming layer (`core/net.rs`), OSDI-style introspection (`core/introspect.rs`), `ConvergencePlan` + `HomotopyStrategy` (`analyses/convergence.rs`), `TrBdf2` + LTE (`math/integration.rs`), `prelude.rs`. Does **not** depend on codegen. OSDI is an external plugin. |
| `piperine-api` | The library face (MD-20): the one host entry point `Session` + `SessionBuilder`, the sweep drivers, and `SolverConfig` (`session/`), result objects (`results.rs`, `waveform.rs`, `fourier.rs`, `units.rs`), `SimHooks` lifecycle trait (`hooks.rs`), `prelude` re-exports. |
| `piperine` (root) | Thin re-export shell over `piperine-api` (`pub use piperine_api::*`) — external Rust hosts keep `use piperine::…`; the tests of record live here as the shell's parity proof. The `piperine` binary target lives in `piperine-cli`. |
| `piperine-plugin` | Plugin SDK + host (v2): native-dlopen + embedded-Python backends only, three shapes (`Pure` / scripted / device, `PluginShape`, inferred from the manifest keys) — a pure-code plugin is a `.vams`/`.va` library resolved via `` `include``, no PHDL-specific meaning survives the frontend swap — TOFU trust + permissions consent, `@device` loading, release-asset device binaries, CLI scripts, and (T53/VAMS-21) native routing for VPI-associated system tasks (`PluginHost::register_system_task`). Plugins contribute **no** attribute schemas and no textual declaration surface. |
| `piperine-plugin-macros` | The `#[pip::device]` / `#[pip::script]` / `#[pip::hook(phase)]` proc-macros — declaration-coupled contributions (depend on it as `pip` so it spells like the Python `@pip.…`). |
| `piperine-cli` | `piperine` CLI (+ the binary target): `check`, `build`, `run` (python scripts / REPL), `fmt`, `new`, `test` (`*_tb.py` runner), `clean`, `add`, `remove`, `tree`, `plugin`. |
| `piperine-project` | `Piperine.toml` discovery, git dependency resolver (a dependency that declares contributions **is** a plugin — MD-30), `SourceMap` recipe, GitHub-release device-binary fetch + cache (`release.rs`), plugin lockfile. |
| `piperine-lang-server` | LSP server. Handlers share `RequestExt::parse`/`ConnectionExt::respond` (every request id gets a response), `DocumentState::{analyze,resolve_at,word_occurrences}`, `ProjectContext::discover`. |
| `piperine-signal` | Pure mechanism crate for the `dv-tester` host device library (MD-36): no `Element`, no MNA/matrix, no Cranelift, **no in-tree dependency** — a purity guard (`tests/purity.rs`) proves it mechanically. Extracted-from-codegen families, semantics intact (codegen keeps its own copies until `codegen-simplification` delegates to this crate): runtime operators (`ops.rs`: `delay`/`slew`/`transition`/`idt`/`idtmod`), event detectors (`events.rs`: `cross`/`above`/`timer`), `table` (`behavior/table.rs`), the junction limiter (`limit.rs`: pnjlim/fetlim). Native (new) families: `Wave` (`wave.rs`, value-from-time descriptors + per-kind DC values), `Behavior` (`behavior/kinds.rs`, value-from-state with closed-form Jacobians, `BehaviorKind` trait), `Measure` (`measure/`, streaming reductions with published error bounds, `MeasureKind` trait), tier-2 crossing prediction (`events.rs::predict_crossing`), and `verify_derivatives` (`verify.rs`, the FD oracle every `BehaviorKind` is checked against). Consumed by `piperine-api`'s host device library (Part VII §20, Part VIII §17 in `docs/spec/`). |

## The analog device path

- `AnalogKernel::compile(module)` flattens the analog body (`flatten/analog.rs`) and JITs it:
  contributions split into resistive + charge `Q(V)` (`ddt` companion model) + `ac_stim`
  stimulus rows; the Jacobian is **symbolic differentiation** (`resolve/diff.rs`),
  emitted like any other expression.
- `AnalogInstance` stamps MNA via `Element::load_dc`/`load_transient` (Norton companion,
  coefficients from `IntegrationMethod::coeffs`), `load_ac` (`jω·dQ/dV`, force branch rows,
  `ac_stim` RHS), `noise_current_psd` (white + flicker), runtime operators (`delay`/`slew`/
  `idt`), analog events, and `suggest_transient_step` (LTE). Implements `Element` through
  `PiperineDevice`.
- The OSDI device (external `piperine-osdi` repo) wraps compiled OSDI v0.4 models as
  `Element` implementations.

## Solver architecture (current state)

- **One ABI:** `Element` trait (`core/element.rs`) with `ElementCapabilities` bitflags
  (`ANALOG`, `DIGITAL`, `SAMPLES_ANALOG`, `LOADS_DC/AC/TRAN`, `EMITS_NOISE`,
  `DEPENDS_ON_DIGITAL`, `HAS_INTERNAL_UNKNOWNS`, `SUPPORTS_ROLLBACK`, `BYPASS_OK`).
  No `Device` wrapper, no downcast.
- **Naming:** `Net` (`core/net.rs`) unifies analog nodes, branch currents, digital nets,
  and pseudo variables under one public identity with stable labels.
- **Convergence:** `ConvergencePlan` (`analyses/convergence.rs`) composes `HomotopyStrategy`
  (gmin stepping, source stepping), `NewtonStrategy`, `StepperStrategy`, and `PlanLimits`
  (caps extracted from magic numbers).
- **Integration:** `TrBdf2` (`math/integration.rs`) — the sole transient scheme (a
  Trapezoidal stage over `γh` then a BDF2 stage over `(1−γ)h`, γ = 2−√2); no
  method-selection surface. LTE-driven timestep via
  `Element::suggest_transient_step` + the plan's PI controller.
- **Errors:** `SolverDomain` enum — typed domains, no free strings.
- **Scheduler:** Returns `Result<(), Error>` instead of `log::warn!`.
- **Prelude:** `prelude.rs` exports the host-facing surface.

## Known gaps (all fail loud — see `ROADMAP.md`)

- `.disto` on MOS2/MOS3 — the 2nd/3rd-derivative kernels emit one JIT function per
  *ordered* controlling-branch combination and overrun Cranelift (`TryFromIntError`).
  They are opt-in (`SessionBuilder::disto(true)`, default off) and `Session::disto`
  fails loud without them. Open in `ROADMAP.md` P1 engine-operator gaps.
- Everything else in that family is implemented and has a ROADMAP entry with its commit:
  `transition` and the other runtime operators (`device/analog/operators.rs`), `table`,
  `$limit`/pnjlim/fetlim (`kernel/analog/limits.rs` + `device/analog/limits.rs`),
  `idt`'s AC `1/jω` admittance, `laplace_*`/`zi_*` (registered in
  `resolve/pom/analog_ops.rs`'s `AnalogOpRegistry`), and multiple `ac_stim` per
  contribution (phasor sum, `flatten/analog.rs::split_ac_stim`).
- Solver ABI work in progress — see `.specs/STATE.md` for the decision log and
  `.specs/features/` for feature specs.

## Naming & conventions

- Ground net → MNA reference; gnd-family names: `gnd/GND/vss/VSS`.
- Elaboration pre-folds `parameter` defaults; `analog function`/`function` default
  arguments are elaboration constants honored by codegen's fn inliner
  (`flatten/analog.rs`).

## Declared language surface (MD-24, re-spelled for VAMS — see MD-42)

**PHDL-era mechanism, gone (MD-45, not retargeted):** every referenceable name used to
resolve to a textual `extern type`/`fn`/`task`/`operator`/`attribute`/`impl` declaration
in `crates/piperine-lang/headers/*.phdl`. That mechanism has no Verilog-AMS Annex A
production and was deleted in full with `piperine-lang/src/parse/` (T21) — there is no
`extern` keyword in VAMS, and `ElabContext` lost every sub-registry
(`callables`/`types`/`operators`/`constraint_helpers`/`impl_methods`) that backed it.

**What's real today:** primitive types, math/libm functions, system tasks, and runtime
operators are LRM builtins the parser/elaborator/codegen recognize natively (Annex A
grammar productions, ch. 4/9 tables) — no textual declaration step, because Verilog-A
never had one either. The **only** surviving declared-surface mechanism is
`@device`/`@port`: `ElabContext::schemas` (T35, VAMS-12/13) is a name-only registry,
seeded before elaboration by `piperine_lang::seed_device_port_schemas`/
`PluginHost::seed_schemas` (the plugin *system's* own schema authority, MD-21/D2, so two
device plugins never collide), and consulted by `elab::lower::vams`'s `(* device = ...
*)`/`(* port = ... *)` attribute-instance lowering — an unrecognized schema still fails
loud (`ElabErrorKind::UnknownAttrSchema`), MD-24's "no silent fallback" rule carried over
verbatim even though its declaration vehicle changed. **Plugins contribute no
declarations** — no attribute schemas, no per-plugin stub (the PHDL-era `extern.phdl`
stub mechanism is deleted; `crates/piperine-plugin/tests/extern_stub.rs` is the guard
proving a plugin shipping such a file is inert).

Known gap: four analog operators (`idtmod`, `absdelay`, `table`, `ac_stim`) resolve
through codegen's own operator registry rather than a language-level declaration —
tracked in `docs/spec/part_v_builtins.md` §2 and `ROADMAP.md`.

Regression guard (rewritten for VAMS, T36, design R9 "rewritten, never deleted"):
`crates/piperine-lang/tests/extern_coverage_guard.rs`.

## Files not to edit casually

- `crates/piperine-lang/src/{grammar,lexer.rs,preprocessor.rs,parser.rs}` — the
  hand-written, LRM-Annex-A-only recursive-descent Verilog-AMS parser; changes ripple
  through all parsing. Provenance rule (C7): grammar and semantics come from
  `VAMS-LRM-2-4.md` only — never a third-party Verilog-A compiler, and the deleted
  `piperine-ams` is off-limits as a source.
- `crates/piperine-codegen/src/resolve/` — the resolved expression/statement form and its
  symbolic differentiation; the correctness-critical core.
- `crates/piperine-codegen/src/emit/analog_expr.rs` — `emit_analog`, the shared JIT
  residual/Jacobian skeleton emission.
- `headers/`, `tests/fixtures*` — frozen test corpora.

## Where the tests are

A hand-maintained list of test files goes stale silently (P6 found this file
naming a target that had been switched off with `#![cfg(any())]`). So there is
no list here — the tree is the list, and a guard keeps it honest.

- **Enumerate**: `ls crates/*/tests/*.rs tests/*.rs`, or `cargo test --workspace`
  (each `Running tests/<name>.rs` line is one target). Every target opens with a
  `//!` header saying what it covers — read that first.
- **Placement rule (MD-28)**: a target lives in the crate it exercises; root
  `tests/` is for the host surface and cross-crate proofs. Integration tests are
  grouped by functionality, one concern per target.
- **The enforcing guard**: `tests/suite_hygiene.rs` walks the repo's own sources
  and fails on switched-off test code, `#[ignore]`d tests, unregistered
  ```` ```ignore ```` doc fences, a target with no `//!` scope header, file-scope
  lint suppression (MD-33), and dead-architecture identifiers (MD-35).
- **The numeric oracles**, worth knowing by name because a change must reproduce
  them: root `tests/ngspice_{validation,ac,noise,tran,temp}.rs` cross-check
  against ngspice — one target per domain over the `tests/ngspice/` fixture
  twins, sharing `tests/support/ngspice.rs` (the harness, the `wrdata` parser
  and the per-domain `ToleranceContract`, which is the *only* place a numeric
  tolerance is written); `tests/ngspice_parity_matrix.rs` is the coverage gate
  that keeps a parity claim from being unfalsifiable. Root
  `tests/run_examples.rs` is the **only** copy of the "every `examples/*.vams`
  elaborates and every `examples/*.py` runs" gate.

## Documentation

- Verilog-AMS language reference: `VAMS-LRM-2-4.md` (LRM 2.4, the sole grammar/semantics
  source, C7) — Annex G (change history) and Annex H (glossary) are additionally ported as
  reference material under `docs/reference/` (T57).
- `docs/spec/` (Parts I–X + appendices A/B/C; `index.md` is the map) — Piperine's own
  formal specification of elaboration/solver/host-API/plugin/codegen architecture, written
  against PHDL originally; retained as historical/architectural reference (most of it
  describes backend concerns that outlived the frontend swap) but its concrete grammar
  text (Parts I–IV) is superseded by `VAMS-LRM-2-4.md` — see each file's own supersession
  notice.
- Solver gaps + ABI plan: merged into `ROADMAP.md` (P1/P2)
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`
