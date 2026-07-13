# Piperine — Claude Code Instructions

## Project summary

Piperine is a PHDL (`.phdl`/`.ppr`) hardware-description language compiled straight into a
**native in-house circuit solver** (Cranelift-JIT analog devices + an event-driven digital
interpreter). No external SPICE dependency. Verilog-A device models load as compiled OSDI
(v0.4) shared libraries through the **`piperine-osdi` plugin** (`~/Git/piperine-osdi` —
extracted from the solver core 2026-07-10; the core has no OSDI/libloading dependency).
Verilog-AMS has been dropped entirely — PHDL is the only frontend.
Rust workspace, edition 2024.

## Pipeline (the spine)

```
PHDL (.phdl) ──parse_and_elaborate──► Design (POM)
                                        │
                                        │ (bench: interpreted directly)
                                        ▼
                                   BenchRunner              piperine_codegen::ir::lower_bodies
                                   ($op/$tran/$ac/$noise)   (Design ──► LoweredBody per module)
                                                                      │
                                                                      ▼
                                                              CircuitCompiler::new(&design, &bodies)
                                                                      │
                                                                      ▼
                                                              CompiledModule (AnalogKernel JIT +
                                                              DigitalKernel)
                                                                      │
                                                                      ▼
                                                              PiperineDevice ──► solver
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
silently emit `0.0`. Same rule in the bench: an unimplemented task is an elaboration error
(`bench_task_implemented` allowlist), never a silent no-op.

## Build and test

```sh
cargo build --workspace           # build all crates
cargo test  --workspace           # the whole suite — 45 green targets, zero warnings
cargo test -p piperine-codegen    # one crate
cargo test <name>                 # one test
cargo test -- --nocapture         # see solver output
```

**`cargo test` bare at the repo root only runs the root package** (root `Cargo.toml` is
both a package and the workspace) — always pass `--workspace`.

## Crate responsibilities

| Crate | Role |
|-------|------|
| `piperine-lang` | PHDL frontend: lexer/parser (`parse/`), elaboration → POM `Design` (`elab/`, `pom/`), bench/const interpreter (`eval/`: `Interpreter`, `Host` trait, task allowlist in `eval/tasks.rs`) — walks the POM/AST directly, no IR. `parse_and_elaborate` is the entry point. Depends only on `piperine-math`; its dev-dep on `piperine-codegen` (for integration tests) does not create a real cycle — `piperine-codegen` depends on `piperine-lang` for POM types. |
| `piperine-codegen` | POM → devices. `lower/` (codegen-private, formerly `piperine-ir` + `piperine-lang::lowering`): `expr.rs`/`stmt.rs`/`symbols.rs` (resolved form), `diff.rs` (symbolic differentiation), `validate.rs` (SPEC §11), `pom/` (`lower_bodies`: POM `Module` → `LoweredBody`). `jit/`: `flatten.rs` (contribution splitting: resistive/charge/`ac_stim`, fn inlining), `analog.rs` (`AnalogKernel` — Cranelift residual/Jacobian/charge/force/noise rows), `emit.rs` (resolved expr → Cranelift), `digital/`. `device/`: `AnalogInstance` (MNA stamping, runtime operators, events), `DigitalInstance`, `CircuitCompiler` (walks POM `Design`/`Module`/`Instance` directly) → `PiperineDevice`. |
| `piperine-math` | Leaf crate: the builtin math name→fn-pointer dispatch table + compile-time evaluator, shared by the interpreter (`piperine-lang`) and the JIT/const-eval (`piperine-codegen`) so `$sqrt`-style builtins agree bit-for-bit. Not an IR — inert data, no expression/statement duplication. |
| `piperine-solver` | Native solver: DC/AC/transient/noise/TF (`analysis/`), MNA/linear algebra (`math/`, faer), `Device` trait, digital topology. Does **not** depend on codegen. OSDI lives in the external `piperine-osdi` plugin. |
| `piperine-bench` | Bench runtime: `SimHost` (`host.rs`), `BenchTask`s (`tasks.rs`), result objects (`objects.rs`, `waveform.rs`), solve plumbing (`session.rs`: `lower_bodies` + `CircuitCompiler::new(&design, &bodies)`), `BenchRunner` (`runner.rs`). |
| `piperine-cli` | `piperine` CLI: `check`, `build`, `run`, `fmt`, `new`, `test`, `clean`, `add`, `remove`, `tree`. |
| `piperine-project` | `Piperine.toml` discovery, git dependency resolver. |
| `piperine-lang-server` | LSP server. Handlers share `RequestExt::parse`/`ConnectionExt::respond` (every request id gets a response), `DocumentState::{analyze,resolve_at,word_occurrences}`, `ProjectContext::discover`. |

## The analog device path

- `AnalogKernel::compile(module)` flattens the analog body (`jit/flatten.rs`) and JITs it:
  contributions split into resistive + charge `Q(V)` (`ddt` companion model) + `ac_stim`
  stimulus rows; the Jacobian is **symbolic differentiation** (`codegen/src/lower/diff.rs`),
  emitted like any other expression.
- `AnalogInstance` stamps MNA: `load_dc`/`load_transient` (Norton companion, `alpha·dQ/dV`),
  `load_ac` (`jω·dQ/dV`, force branch rows, `ac_stim` RHS `mag·e^{jφ}`),
  `noise_current_psd` (white + flicker `(1/f)^exp`), runtime operators (`delay`/`slew`/
  `idt`) and analog events serviced per accepted step.
- The OSDI device (`piperine-osdi/src/device.rs`, external repo) is the reference for
  reactive/noise stamping.

## Known gaps (all fail loud — see ROADMAP.md)

- `transition`, `laplace_*`, `zi_*` — recognised in the resolved form, no companion model yet.
- `ac_stim` in potential contributions is now supported (force-branch AC drive → voltage
  sources do AC); multiple `ac_stim` per contribution is still fail-loud.
- `$limit` (pnjlim/fetlim) is not lowered in the JIT — blocks junction devices from
  compiling through `CircuitCompiler` (works in the bench interpreter). See ROADMAP.
- `@initial` cannot force a branch (`V<-ic`). See ROADMAP. (Large analog bodies no longer
  blow up — the flattener uses a shared-temporary tape, `jit/flatten.rs`; `dio` compiles and
  converges, `bjt`/`mos1` compile pending `$limit` multi-junction convergence.)
- `idt` contributes 0 in AC (no `1/jω` stamp).
- `$plot`, `extract`/`.attach`/`.meta` — bench tasks not yet implemented (allowlist-gated).

## Naming & conventions

- Ground net → MNA reference; gnd-family names: `gnd/GND/vss/VSS`.
- PHDL pre-folds param defaults during elaboration; `fn` default parameters are elaboration
  constants honored by both the interpreter and codegen's fn inliner (`jit/flatten.rs`).
- No macro magic — data tables + plain helpers. Every helper method has an owner (struct
  method or extension trait), not loose module-level fns.

## Files not to edit casually

- `crates/piperine-lang/src/parse/` — hand-written recursive-descent parsers; changes
  ripple through all parsing.
- `crates/piperine-codegen/src/lower/` — the resolved expression/statement form and its
  symbolic differentiation; codegen-private, but the correctness-critical core.
- `crates/piperine-codegen/src/jit/analog.rs` — the shared JIT residual/Jacobian skeleton.
- `headers/`, `tests/fixtures*` — frozen test corpora.

## Tests of record

- `piperine-codegen/tests/analog_jit.rs`, `digital_jit.rs` — kernel-level JIT behavior;
  `ppr_ir.rs`, `codegen_ir.rs`, `codegen_api.rs`, `from_ir.rs`, `silent_bugs.rs` — POM→resolved
  lowering and the `CircuitCompiler` structural path (moved here from `piperine-lang` when
  the IR crate was removed — codegen depends on `piperine-lang` now, not the reverse).
- `piperine-lang/tests/` — parse/elab (`parse_elab.rs`), end-to-end sim (`spec_simulation.rs`),
  bench gating (`bench.rs`), ngspice headers (`ngspice_*.rs`).
- `piperine-bench/tests/bench.rs` — bench e2e (has the `elab` helper + `CIRCUIT` fixture);
  `run_examples.rs` — every `examples/*.phdl` bench must stay green.
- `piperine-solver/tests/` — solver-level analyses, mixed-signal, OSDI, cosim.

## Documentation

- Language spec: `crates/piperine-lang/docs/SPEC.md` (Parts I–VI)
- Bench spec: `crates/piperine-bench/docs/SPEC.md` (update §11 status rows when closing gaps)
- Resolved-form spec: `crates/piperine-codegen/docs/SPEC.md`
- Digital network JIT + event interface: `crates/piperine-codegen/docs/DIGITAL_JIT.md`
  (the stable `DigitalEventModel` boundary in `solver/src/digital_interface.rs`; the fused
  Verilator-style cone compiler scaffold in `codegen/src/jit/digital/network.rs`)
- Open items: `ROADMAP.md`
