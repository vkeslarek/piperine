# Piperine Architecture

High-level map of what runs, in what order, and which crate owns each step.
For per-crate API detail see `docs/`. For contributor conventions see `CLAUDE.md`.

## The pipeline

A `.ppr` file goes through eight stages. `src/main.rs::run()` is the whole
pipeline in one function — the numbered comments there match the stages below.

```
                  .ppr source
                      │
            ┌─────────▼──────────┐
            │ 1. parse            │  piperine-parser
            │    → Document (AST) │
            └─────────┬──────────┘
                      │
        ┌─────────────┴──────────────┐
        │                            │
┌───────▼─────────┐        ┌─────────▼───────────┐
│ 2-3,5. VA path  │        │ 6. testbench path   │
│ extract_va_*    │        │ elaborate()         │  piperine-circuit
│ → compile .osdi │        │ → spice_lines       │
│   (OpenVAF)     │        │ → initial_statement │
└───────┬─────────┘        └─────────┬───────────┘
        │                            │
        │ pre_osdi <path>            │ 7. load_circuit(spice_lines)
        └─────────────┬──────────────┘
                      │
            ┌─────────▼──────────┐
            │ ngspice (worker)   │  piperine-worker  (subprocess)
            └─────────▲──────────┘
                      │ IPC: Command / Response
            ┌─────────┴──────────┐
            │ 8. Interpreter.exec│  piperine-interpreter
            │    runs initial{}  │
            │    $tasks ─────────┼──▶ SimulatorBackend ──▶ worker
            └────────────────────┘
```

Two things leave elaboration: **`spice_lines`** (the netlist, as `Vec<String>`)
and **`initial_statement`** (the procedural testbench the interpreter runs).
There is exactly one netlist representation — strings. Devices format their own
SPICE line via `HardwareInstance::spice_lines()`; nothing builds a typed netlist.

## Crate responsibilities

| Crate | Owns | Does NOT |
|-------|------|----------|
| `piperine-parser` | Lexer, preprocessor, recursive-descent parser → AST | No semantics, no SPICE |
| `piperine-circuit` | `HardwareDefinition` trait, registry, elaboration, net/paramset resolution | No device impls, no IPC |
| `piperine-ngspice` | ngspice device impls, `$tasks`, backend, `ngspice.ppr` | No FFI (that's the worker) |
| `piperine-interpreter` | Runs `initial`/`always` blocks, `Value`, `SystemTask`/`SimulatorBackend` traits | No ngspice specifics |
| `piperine-openvaf` | Compiles VA modules → `.osdi`, caches by source hash | Not a simulator |
| `piperine-coordinator` | `ProcessPool` — spawns/owns worker subprocesses | No simulation logic |
| `piperine-worker` | Subprocess wrapping libngspice via FFI; answers IPC | No language knowledge |
| `piperine-common` | IPC message types (`Command`, `Response`, `EventAction`) shared by coordinator+worker | Nothing else |

## Two boundaries worth knowing

**Plugin boundary** (`Plugin` trait, `piperine-interpreter`). The interpreter knows
nothing about ngspice. `NgspicePlugin` and `OpenVafPlugin` register hardware defs,
`$tasks`, and a `SimulatorBackend` into registries at startup (`src/main.rs` step 4).
Swapping simulators = swapping a plugin.

**IPC boundary** (`piperine-common`). The interpreter calls a `SimulatorBackend`
(in-process). `NgspiceBackend` translates those calls into `Command`s sent over an
`ipc-channel` to a separate `piperine-worker` process, which alone touches libngspice.
The worker is isolated so an ngspice crash cannot take down the interpreter.

## How an analysis runs

`$tran(...)` / `$ac(...)` etc. are `SystemTask`s. Two run paths exist by design:

- **`SimulatorBackend::run_analysis_simple(cmd)`** — most analyses. Start, poll to
  completion, harvest vectors. No event callbacks.
- **`Interpreter::run_analysis(cmd)`** — used only when the testbench has
  `always @(step)` / `above()` handlers. Same protocol, but dispatches handler bodies
  on each streamed event before responding to the worker.

Both return an `AnalysisResult` (`kind`, `plot_name`, `vectors`, `run_errors`), which
tasks wrap in an `AnalysisHandleObj` so `.ppr` code can call `.signal(...).max()` etc.

## Where to look first

- Add a regular device → `crates/piperine-ngspice/src/hardware.rs` (data table) +
  `ngspice.ppr`. See `docs/development/SPICE_COMPONENTS_IMPL.md`.
- Add an analysis/measurement → `crates/piperine-ngspice/src/tasks.rs`.
- Change the language → `crates/piperine-parser/src/grammar/` (careful: hand-written).
- Change net/paramset resolution → `crates/piperine-circuit/src/elaboration.rs`.
</content>
</invoke>
