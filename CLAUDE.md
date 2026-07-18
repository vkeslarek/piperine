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
                            piperine_codegen::ir::lower_bodies
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
                            hosts: root `piperine` lib (Rust) / `import piperine` (Python)
```

There is **no separate IR crate**. The POM (`Design`/`Module`, from `piperine-lang`) is the
single object model; `piperine_codegen::ir` (formerly the standalone `piperine-ir` crate,
now `piperine-codegen/src/lower/`) is codegen's **private** resolved form — expressions with
interned ids, symbolic differentiation (`lower/diff.rs`), the POM→resolved pass
(`lower/pom/`, `lower_bodies`). Nothing outside `piperine-codegen` depends on its shape.
`CircuitCompiler` walks the POM `Design`/`Module`/`Instance` directly for structure
(connections, param overrides) — there is no `IrModule`/`IrInstance`/`IrProgram` structural
twin. "100% coverage" means: every PHDL construct lowers to executable device code. When
something cannot be faithfully lowered, **fail loud** (`CodegenError::Unsupported`) — never
silently emit `0.0` or a no-op.

## Build and test

```sh
cargo build --workspace           # build all crates — zero warnings is the bar
cargo test  --workspace           # the whole suite — 51 green targets
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
| `piperine-codegen` | POM → devices. `lower/` (codegen-private resolved form: `expr.rs`/`stmt.rs`/`symbols.rs`, `diff.rs` symbolic differentiation, `pom/` `lower_bodies`). `jit/`: `flatten.rs`, `analog.rs` (`AnalogKernel`), `emit.rs`, `digital/`. `device/`: `AnalogInstance`, `DigitalInstance`, `CircuitCompiler` → `PiperineDevice` (implements `Element`). |
| `piperine-solver` | Native solver: DC/AC/transient/noise/TF (`solver/`), MNA/linear algebra (`math/`, faer), `Element` trait + `ElementCapabilities` (`core/element.rs`), `Net` naming layer (`core/net.rs`), OSDI-style introspection (`core/introspect.rs`), `ConvergencePlan` + `HomotopyStrategy` (`solver/convergence.rs`), `IntegrationMethod` + LTE (`math/integration.rs`), `prelude.rs`. Does **not** depend on codegen. OSDI is an external plugin. |
| `piperine` (root) | The library face (MD-19, lib-only): `SimSession`/`SolverConfig` (`session.rs`), result objects (`results.rs`, `waveform.rs`), `SimHooks` lifecycle trait (`hooks.rs`), `prelude` re-exports. The `piperine` binary target lives in `piperine-cli`. |
| `piperine-plugin` | Plugin SDK + host: native/WASM/process backends, TOFU trust, `@device` loading, attribute schemas, CLI scripts. |
| `piperine-plugin-wasm` | WASM guest SDK (re-exports `pom::wire` for `wasm32-unknown-unknown`). |
| `piperine-cli` | `piperine` CLI (+ the binary target): `check`, `build`, `run` (python scripts / REPL), `fmt`, `new`, `test` (`*_tb.py` runner), `clean`, `add`, `remove`, `tree`, `plugin`. |
| `piperine-project` | `Piperine.toml` discovery, git dependency resolver, plugin lockfile. |
| `piperine-lang-server` | LSP server. Handlers share `RequestExt::parse`/`ConnectionExt::respond` (every request id gets a response), `DocumentState::{analyze,resolve_at,word_occurrences}`, `ProjectContext::discover`. |

## The analog device path

- `AnalogKernel::compile(module)` flattens the analog body (`jit/flatten.rs`) and JITs it:
  contributions split into resistive + charge `Q(V)` (`ddt` companion model) + `ac_stim`
  stimulus rows; the Jacobian is **symbolic differentiation** (`codegen/src/lower/diff.rs`),
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
  `DEPENDS_ON_DIGITAL`, `HAS_INTERNAL_UNKNOWNS`, `SUPPORTS_ROLLBACK`, `SUPPORTS_QUERIES`).
  No `Device` wrapper, no downcast.
- **Naming:** `Net` (`core/net.rs`) unifies analog nodes, branch currents, digital nets,
  and pseudo variables under one public identity with stable labels.
- **Convergence:** `ConvergencePlan` (`solver/convergence.rs`) composes `HomotopyStrategy`
  (gmin stepping, source stepping) and `PlanLimits` (caps extracted from magic numbers).
  `NewtonStrategy`/`StepperStrategy` are the next phase (see `.specs/`).
- **Integration:** `IntegrationMethod` (`math/integration.rs`) — Trapezoidal and Gear/BDF
  with unified `coeffs(dt, dt_prev, order)`. LTE-driven timestep via
  `Element::suggest_transient_step`.
- **Errors:** `SolverDomain` enum — typed domains, no free strings.
- **Scheduler:** Returns `Result<(), Error>` instead of `log::warn!`.
- **Prelude:** `prelude.rs` exports the host-facing surface.

## Known gaps (all fail loud — see `SOLVER_GAPS.md` and `ROADMAP.md`)

- `transition`, `laplace_*`, `zi_*` — recognised in the resolved form, no companion model yet.
- `ac_stim` in potential contributions is now supported; multiple `ac_stim` per contribution
  is still fail-loud.
- `$limit` (pnjlim/fetlim) is not lowered in the JIT.
- `idt` contributes 0 in AC (no `1/jω` stamp).
- Solver ABI refactor in progress — see `.specs/STATE.md` for macro decisions and
  `.specs/features/` for feature specs.

## Naming & conventions

- Ground net → MNA reference; gnd-family names: `gnd/GND/vss/VSS`.
- PHDL pre-folds param defaults during elaboration; `fn` default parameters are elaboration
  constants honored by both the interpreter and codegen's fn inliner (`jit/flatten.rs`).

## Files not to edit casually

- `crates/piperine-lang/src/parse/` — hand-written recursive-descent parsers; changes
  ripple through all parsing.
- `crates/piperine-codegen/src/lower/` — the resolved expression/statement form and its
  symbolic differentiation; codegen-private, but the correctness-critical core.
- `crates/piperine-codegen/src/jit/analog.rs` — the shared JIT residual/Jacobian skeleton.
- `headers/`, `tests/fixtures*` — frozen test corpora.

## Tests of record

- `piperine-codegen/tests/`: `analog_jit.rs`, `digital_jit.rs` (kernel-level JIT);
  `codegen_ir.rs`, `codegen_api.rs`, `from_ir.rs`, `silent_bugs.rs` (POM→resolved + circuit).
- `piperine-lang/tests/`: `parse_elab.rs`, `spec_simulation.rs`, `elab.rs`,
  `bundle_param.rs`, `bundle_connections.rs`, `prelude.rs`, `type_casts.rs`, `pom_serde.rs`,
  `bench_removed.rs` (the bench keyword is a syntax error).
- `tests/` (root, host API): `session.rs`, `ngspice_validation.rs` (+`ngspice/`),
  `spice_smoke.rs` (+`spice/`), `compile_once_sweep.rs`, `run_examples.rs` (every
  `examples/*.phdl` elaborates + every `examples/*.py` runs).
- `piperine-solver/tests/`: `digital_topology.rs`, `mixed_signal.rs`.
- `piperine-plugin/tests/`: `e2e.rs`, `native_smoke.rs`, `phase3.rs`, `process_smoke.rs`,
  `wasm_smoke.rs`, `trust.rs`, `manifest.rs`.

## Documentation

- Formal spec: `docs/spec/` (Parts I–VII + appendices A/B)
- Solver gaps + ABI refactor plan: `SOLVER_GAPS.md`
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`
