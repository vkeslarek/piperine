# Piperine — Claude Code Instructions

## Project summary

Piperine is a PHDL (`.phdl`/`.ppr`) hardware-description language compiled straight into a
**native in-house circuit solver** (Cranelift-JIT analog devices + an event-driven digital
interpreter). No external SPICE dependency. Verilog-A device models load as compiled OSDI
(v0.4) shared libraries through the **`piperine-osdi` plugin** (external repo — the solver
core has no OSDI/libloading dependency). Verilog-AMS has been dropped entirely — PHDL is
the only frontend. Rust workspace, edition 2024.

## Pipeline (the spine)

```
PHDL (.phdl) ──parse_and_elaborate──► Design (POM)
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
"100% coverage" means: every PHDL construct lowers to executable device code. When
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
| `piperine-lang` | PHDL frontend: lexer/parser (`parse/`), elaboration → POM `Design` (`elab/`, `pom/`), const evaluator (`eval/`: `Interpreter`, `Host` trait, pure system tasks in `eval/tasks.rs`) — walks the POM/AST directly, no IR. `parse_and_elaborate` is the entry point. Builtin stdlib headers in `headers/` (prelude, disciplines, constants) and `headers/spice/` (the ngspice-faithful device models — `use spice::<file>;` works in any project, no dependency; a project package named `spice` shadows the builtin). |
| `piperine-codegen` | POM → devices, one module per pipeline stage: `pom::Design ─▶ resolve ─▶ flatten ─▶ emit ─▶ kernel ─▶ device`. `resolve/` (resolved form: `expr.rs`/`stmt.rs`/`symbols.rs`, `diff.rs` symbolic differentiation, `pom/` `lower_bodies`). `flatten/` (resolved analog → `FlatAnalog`, crate-private). `emit/` (Cranelift emission machinery: `Builder`, `Codegen` trait, CSE, `SimCtx` ABI, crate-private). `kernel/`: `analog/` (`AnalogKernel`, capability sub-structs behind `Option`), `digital/`. `device/`: `analog/` (`AnalogInstance`, capability files `forces.rs`/`limits.rs`/`operators.rs`/`events.rs`), `digital.rs`, `circuit.rs`/`builder.rs`/`fusion.rs`/`plugin.rs` (`CircuitCompiler`) → `PiperineDevice` (implements `Element`). Public surface is a single `lib.rs` façade (MD-23) — `resolve`/`kernel`/`device` stay `pub`, `emit`/`flatten`/`error` are crate-private. |
| `piperine-solver` | Native solver: DC/AC/transient/noise/TF and the rest of the analysis family (`analyses/`), MNA/linear algebra (`math/`, faer), `Element` trait + `ElementCapabilities` (`core/element.rs`), `Net` naming layer (`core/net.rs`), OSDI-style introspection (`core/introspect.rs`), `ConvergencePlan` + `HomotopyStrategy` (`analyses/convergence.rs`), `IntegrationMethod` + LTE (`math/integration.rs`), `prelude.rs`. Does **not** depend on codegen. OSDI is an external plugin. |
| `piperine-api` | The library face (MD-20): the one host entry point `Session` + `SessionBuilder`, the sweep drivers, and `SolverConfig` (`session/`), result objects (`results.rs`, `waveform.rs`, `fourier.rs`, `units.rs`), `SimHooks` lifecycle trait (`hooks.rs`), `prelude` re-exports. |
| `piperine` (root) | Thin re-export shell over `piperine-api` (`pub use piperine_api::*`) — external Rust hosts keep `use piperine::…`; the tests of record live here as the shell's parity proof. The `piperine` binary target lives in `piperine-cli`. |
| `piperine-plugin` | Plugin SDK + host (v2): native-dlopen + embedded-Python backends only, three shapes (pure-PHDL / scripted / device) inferred from the manifest keys, TOFU trust + permissions consent, `@device` loading, release-asset device binaries, CLI scripts. Plugins contribute **no** attribute schemas and no `extern`. |
| `piperine-plugin-macros` | The `#[pip::device]` / `#[pip::script]` / `#[pip::hook(phase)]` proc-macros — declaration-coupled contributions (depend on it as `pip` so it spells like the Python `@pip.…`). |
| `piperine-cli` | `piperine` CLI (+ the binary target): `check`, `build`, `run` (python scripts / REPL), `fmt`, `new`, `test` (`*_tb.py` runner), `clean`, `add`, `remove`, `tree`, `plugin`. |
| `piperine-project` | `Piperine.toml` discovery, git dependency resolver (a dependency that declares contributions **is** a plugin — MD-30), `SourceMap` recipe, GitHub-release device-binary fetch + cache (`release.rs`), plugin lockfile. |
| `piperine-lang-server` | LSP server. Handlers share `RequestExt::parse`/`ConnectionExt::respond` (every request id gets a response), `DocumentState::{analyze,resolve_at,word_occurrences}`, `ProjectContext::discover`. |

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
- **Integration:** `IntegrationMethod` (`math/integration.rs`) — Trapezoidal and Gear/BDF
  with unified `coeffs(dt, dt_prev, order)`. LTE-driven timestep via
  `Element::suggest_transient_step`.
- **Errors:** `SolverDomain` enum — typed domains, no free strings.
- **Scheduler:** Returns `Result<(), Error>` instead of `log::warn!`.
- **Prelude:** `prelude.rs` exports the host-facing surface.

## Known gaps (all fail loud — see `ROADMAP.md`)

- `laplace_*`, `zi_*` — **not declared at all** (`headers/operators.phdl` has no
  `extern operator` for either), so MD-24 stops a call at elaboration; they never reach
  codegen. Language backlog, tracked in `ROADMAP.md`.
- `.disto` on MOS2/MOS3 — the 2nd/3rd-derivative kernels emit one JIT function per
  *ordered* controlling-branch combination and overrun Cranelift (`TryFromIntError`).
  They are opt-in (`SessionBuilder::disto(true)`, default off) and `Session::disto`
  fails loud without them. Open in `ROADMAP.md` P1 engine-operator gaps.
- Everything else in that family is implemented and has a ROADMAP entry with its commit:
  `transition` and the other runtime operators (`device/analog/operators.rs`), `table`,
  `$limit`/pnjlim/fetlim (`kernel/analog/limits.rs` + `device/analog/limits.rs`),
  `idt`'s AC `1/jω` admittance, and multiple `ac_stim` per contribution (phasor sum,
  `flatten/analog.rs::split_ac_stim`).
- Solver ABI work in progress — see `.specs/STATE.md` for the decision log and
  `.specs/features/` for feature specs.

## Naming & conventions

- Ground net → MNA reference; gnd-family names: `gnd/GND/vss/VSS`.
- PHDL pre-folds param defaults during elaboration; `fn` default parameters are elaboration
  constants honored by both the interpreter and codegen's fn inliner (`flatten/analog.rs`).

## Declared language surface (MD-24)

Every referenceable PHDL name resolves to a **textual declaration** in
`crates/piperine-lang/headers/` (or a plugin's published `extern.phdl`
stub). Name lookup that finds no declaration is a fail-loud compile error,
never a silent fallback into a Rust-native registry. The seven `extern`
forms cover every previously-magic surface:

- `extern type Real;` — primitive value types (`headers/types.phdl`).
- `extern fn sin(x: Real) -> Real;` — libm intrinsics (`headers/math.phdl`).
- `extern task $display() -> Unit;` — system tasks (`headers/tasks.phdl`).
- `extern operator ddt(x: Real) -> Real;` — runtime operators
  (`headers/operators.phdl`).
- `extern attribute device { plugin: String, type: String }` — plugin
  system's own `@device`/`@port` schemas (`headers/device_port.phdl`).
- `extern impl Real { fn from(x: Integer) -> Real; ... }` — type methods
  (T17's cast-replacement surface; overload-resolved by argument type).
- `extern impl Capability for TypeName { ... }` — capability impls
  (reserved for future native capability dispatch on primitives; binary
  operators are pure grammar, not dispatched through capabilities today).

A loaded plugin publishes its own attribute schemas via an `extern.phdl`
stub auto-imported at load time; a schema-contributing plugin that
publishes no stub fails loud at `PluginHost::load_for_project`
(`PluginError::MissingExternStub`).

Permanent regression guard: `crates/piperine-lang/tests/extern_coverage_guard.rs`.

## Files not to edit casually

- `crates/piperine-lang/src/parse/` — hand-written recursive-descent parsers; changes
  ripple through all parsing.
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
  them: root `tests/ngspice_validation.rs` (+`tests/ngspice/`) cross-checks
  against ngspice; root `tests/run_examples.rs` is the **only** copy of the
  "every `examples/*.phdl` elaborates and every `examples/*.py` runs" gate.

## Documentation

- Formal spec: `docs/spec/` (Parts I–VII + appendices A/B)
- Solver gaps + ABI plan: merged into `ROADMAP.md` (P1/P2)
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`
