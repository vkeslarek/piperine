# Piperine — Claude Code Instructions

## Project summary

Piperine is a PHDL (`.phdl`/`.ppr`) hardware-description language compiled through a shared
**IR** to a **native in-house circuit solver** (Cranelift-JIT analog devices + an
event-driven digital interpreter). No external SPICE dependency; Verilog-A device models
load as compiled OSDI (v0.4) shared libraries. A native Verilog-AMS frontend is being
reworked outside the workspace. Rust workspace, edition 2024.

## Pipeline (the spine)

```
PHDL (.phdl) ──parse_and_elaborate──► Design (POM) ──ppr_to_ir──► IrProgram
                                        │                            │
                                        │ (bench: interpreted)       ├──► CircuitCompiler
                                        ▼                            ▼
                                   BenchRunner ────────────► CompiledModule
                                   ($op/$tran/$ac/$noise)    (AnalogKernel JIT +
                                                              DigitalKernel)
                                                                     │
                                                                     ▼
                                                          PiperineDevice ──► solver
```

The **IR** (`crates/piperine-ir`, re-exported as `piperine_codegen::ir`) is the contract.
"100% coverage" means: every PHDL construct maps to IR, and every IR construct lowers to
executable device code. When something cannot be faithfully lowered, **fail loud**
(`CodegenError::Unsupported`) — never silently emit `0.0`. Same rule in the bench: an
unimplemented task is an elaboration error (`bench_task_implemented` allowlist), never a
silent no-op.

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
| `piperine-lang` | PHDL frontend: lexer/parser (`parse/`), elaboration → POM `Design` (`elab/`, `pom/`), POM → IR lowering (`lowering/`, `ppr_to_ir`), bench/const interpreter (`eval/`: `Interpreter`, `Host` trait, task allowlist in `eval/tasks.rs`). `parse_and_elaborate` is the entry point. |
| `piperine-ir` | The shared IR: `expr.rs` (`IrExpr` + symbolic `diff.rs`), `stmt.rs`, `symbols.rs`, `validate.rs` (SPEC §11 emit-and-validation contract). |
| `piperine-codegen` | IR → devices. `jit/`: `flatten.rs` (contribution splitting: resistive/charge/`ac_stim`, fn inlining), `analog.rs` (`AnalogKernel` — Cranelift residual/Jacobian/charge/force/noise rows), `emit.rs` (IrExpr → Cranelift), `digital/`. `device/`: `AnalogInstance` (MNA stamping, runtime operators, events), `DigitalInstance`, `CircuitCompiler` → `PiperineDevice`. |
| `piperine-solver` | Native solver: DC/AC/transient/noise/TF (`analysis/`), MNA/linear algebra (`math/`, faer), `Device` trait, OSDI loader (`osdi/`), digital topology. Does **not** depend on codegen. |
| `piperine-bench` | Bench runtime: `SimHost` (`host.rs`), `SimTask`s (`tasks.rs`), result objects (`objects.rs`, `waveform.rs`), solve plumbing (`session.rs`), `BenchRunner` (`runner.rs`). |
| `piperine-cli` | `piperine` CLI: `check`, `build`, `run`, `fmt`, `new`, `test`, `clean`, `add`, `remove`, `tree`. |
| `piperine-project` | `Piperine.toml` discovery, git dependency resolver. |
| `piperine-lang-server` | LSP server. Handlers share `RequestExt::parse`/`ConnectionExt::respond` (every request id gets a response), `DocumentState::{analyze,resolve_at,word_occurrences}`, `ProjectContext::discover`. |

## The analog device path

- `AnalogKernel::compile(module)` flattens the analog body (`jit/flatten.rs`) and JITs it:
  contributions split into resistive + charge `Q(V)` (`ddt` companion model) + `ac_stim`
  stimulus rows; the Jacobian is **symbolic differentiation** (`piperine-ir/src/diff.rs`),
  emitted like any other expression.
- `AnalogInstance` stamps MNA: `load_dc`/`load_transient` (Norton companion, `alpha·dQ/dV`),
  `load_ac` (`jω·dQ/dV`, force branch rows, `ac_stim` RHS `mag·e^{jφ}`),
  `noise_current_psd` (white + flicker `(1/f)^exp`), runtime operators (`delay`/`slew`/
  `idt`) and analog events serviced per accepted step.
- The OSDI device (`solver/src/osdi/device.rs`) is the reference for reactive/noise stamping.

## Known gaps (all fail loud — see ROADMAP.md)

- `transition`, `laplace_*`, `zi_*` — recognised in IR, no companion model yet.
- `ac_stim` in potential contributions is now supported (force-branch AC drive → voltage
  sources do AC); multiple `ac_stim` per contribution is still fail-loud.
- `$limit` (pnjlim/fetlim) is not lowered in the JIT — blocks junction devices from
  compiling through `CircuitCompiler` (works in the bench interpreter). See ROADMAP.
- `@initial` cannot force a branch (`V<-ic`); large analog bodies (mos1) exceed Cranelift's
  function-size limit; `Option<T>` is not a valid `param` type. See ROADMAP.
- `idt` contributes 0 in AC (no `1/jω` stamp).
- `$plot`, `extract`/`.attach`/`.meta` — bench tasks not yet implemented (allowlist-gated).

## Naming & conventions

- Ground net → MNA reference; gnd-family names: `gnd/GND/vss/VSS`.
- PHDL pre-folds param defaults during elaboration; `fn` default parameters are elaboration
  constants honored by both the interpreter and the IR inliner.
- No macro magic — data tables + plain helpers. Every helper method has an owner (struct
  method or extension trait), not loose module-level fns.

## Files not to edit casually

- `crates/piperine-lang/src/parse/` — hand-written recursive-descent parsers; changes
  ripple through all parsing.
- `crates/piperine-ir/src/` — the shared IR contract.
- `crates/piperine-codegen/src/jit/analog.rs` — the shared JIT residual/Jacobian skeleton.
- `headers/`, `tests/fixtures*` — frozen test corpora.

## Tests of record

- `piperine-codegen/tests/analog_jit.rs`, `digital_jit.rs` — kernel-level JIT behavior.
- `piperine-lang/tests/` — parse/elab (`parse_elab.rs`), POM→IR (`ppr_ir.rs`,
  `codegen_ir.rs`), end-to-end sim (`spec_simulation.rs`), bench gating (`bench.rs`),
  ngspice headers (`ngspice_*.rs`).
- `piperine-bench/tests/bench.rs` — bench e2e (has the `elab` helper + `CIRCUIT` fixture);
  `run_examples.rs` — every `examples/*.phdl` bench must stay green.
- `piperine-solver/tests/` — solver-level analyses, mixed-signal, OSDI, cosim.

## Documentation

- Language spec: `crates/piperine-lang/docs/SPEC.md` (Parts I–VI)
- Bench spec: `crates/piperine-bench/docs/SPEC.md` (update §11 status rows when closing gaps)
- IR spec: `crates/piperine-codegen/docs/SPEC.md`
- Digital network JIT + event interface: `crates/piperine-codegen/docs/DIGITAL_JIT.md`
  (the stable `DigitalEventModel` boundary in `solver/src/digital_interface.rs`; the fused
  Verilator-style cone compiler scaffold in `codegen/src/jit/digital/network.rs`)
- Open items: `ROADMAP.md`
