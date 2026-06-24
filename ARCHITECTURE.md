# Piperine Architecture

High-level map of what runs, in what order, and which crate owns each step.
For per-crate API detail see `docs/`. For contributor conventions see `CLAUDE.md`.

## The pipeline

Two entry points exist: the CLI (`src/main.rs`) and the Python extension
(`crates/piperine-python`). Both share the same elaboration path.

```
                  .ppr source
                      │
            ┌─────────▼──────────┐
            │  parse              │  piperine-parser
            │  → Document (AST)   │
            └─────────┬──────────┘
                      │
        ┌─────────────┴──────────────┐
        │                            │
┌───────▼─────────┐        ┌─────────▼────────────────┐
│  VA path         │        │  elaborate_circuit()      │  piperine-circuit
│  extract_va_*   │        │  → Circuit {              │
│  → compile .osdi│        │      spice_lines,         │
│    (OpenVAF)    │        │      soa_checks,          │
└───────┬─────────┘        │    }                      │
        │                  └─────────┬────────────────-┘
        │                            │
        │                   load_circuit(spice_lines + .end)
        └─────────────┬──────────────┘
                      │
            ┌─────────▼──────────┐
            │  ngspice (worker)  │  piperine-worker (subprocess)
            └─────────▲──────────┘
                      │ IPC: Command / Response
            ┌─────────┴──────────┐
            │  NgspiceSession    │  piperine-python (PyO3)
            │  .op() .ac() …     │
            │  .tran_async()     │  GIL released; Rust threads
            │  SimFuture.join()  │
            └────────────────────┘
```

`elaborate_circuit` produces a `Circuit` — a `Vec<String>` SPICE netlist plus
`Vec<SoaCheck>` compiled from `always @(step)` blocks. Devices format their own
SPICE line via `HardwareInstance::spice_lines()`; nothing builds a typed netlist.

## Crate responsibilities

| Crate | Owns | Does NOT |
|-------|------|----------|
| `piperine-parser` | Lexer, preprocessor, recursive-descent parser → AST | No semantics, no SPICE |
| `piperine-circuit` | `HardwareDefinition` trait, registry, `elaborate_circuit`, net/paramset/SOA resolution | No device impls, no IPC |
| `piperine-ngspice` | ngspice device impls, `register_hardware`, `NgspiceBackend`, `ngspice.ppr` | No FFI (that's the worker) |
| `piperine-python` | PyO3 native extension: `NgspiceSession`, `SimFuture`, `join_all` | No language/elaboration |
| `piperine-interpreter` | Runs `always @(step)` handlers during analysis; `SimulatorBackend` trait | No testbench logic (that's Python) |
| `piperine-openvaf` | Compiles VA modules → `.osdi`, caches by source hash | Not a simulator |
| `piperine-coordinator` | `ProcessPool` — spawns/owns worker subprocesses | No simulation logic |
| `piperine-worker` | Subprocess wrapping libngspice via FFI; answers IPC | No language knowledge |
| `piperine-common` | IPC message types (`Command`, `Response`, `EventAction`) | Nothing else |

## Key boundaries

**IPC boundary** (`piperine-common`). `NgspiceBackend` translates Python session
calls into `Command`s sent over `ipc-channel` to a separate `piperine-worker`
process, which alone touches libngspice. A worker crash cannot reach Python.

**GIL boundary** (`piperine-python`). Every blocking call (`op`, `ac`, `tran`,
`tran_async` join) releases the GIL via `py.allow_threads(...)`. Multiple sessions
run in parallel Rust threads — wall time equals the slowest worker.

## How an analysis runs

`NgspiceSession` methods use two paths:

- **`run_analysis_simple(cmd)`** — `op`, `tran`, `dc`. Start analysis, poll `Done`,
  harvest vectors. No event callbacks.
- **`run_ac_analysis(cmd)`** — AC sweep. Same start/poll, but fetches complex vectors
  via `GetVecComplex` (AC plots store all vectors as complex in ngspice).

`always @(step)` SOA blocks in hardware modules are compiled at elaboration to
`.meas tran` SPICE lines + `SoaCheck` entries. After a transient, Python calls
`sess.check_soa()` to read those measurement vectors and compare thresholds.

## SOA compilation

```
always @(step) begin
    if (V(c) > 30.0) $run_error("Vce_max");
end
```

`elaborate_circuit` walks this block, extracts the comparison operator and threshold,
and emits:

```spice
.meas tran _soa_0 MAX v(c)
```

The `SoaCheck { meas_name: "_soa_0", label: "Vce_max", threshold: 30.0, op: Gt }`
is stored in `Circuit.soa_checks`. `check_soa()` reads the measurement result and
raises on violation.

## Where to look first

- Add a device → `crates/piperine-ngspice/src/hardware.rs` + `ngspice.ppr`.
  See `docs/development/SPICE_COMPONENTS_IMPL.md`.
- Change elaboration → `crates/piperine-circuit/src/elaboration.rs`.
- Change the language → `crates/piperine-parser/src/grammar/` (hand-written; be careful).
- Add a Python API method → `crates/piperine-python/src/lib.rs`.
- Extend IPC → `piperine-common/src/lib.rs` + worker match arm + backend method.
