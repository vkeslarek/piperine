# Piperine — Claude Code Instructions

## Project summary

Piperine is a Verilog-A superset language + SystemVerilog procedural testbench layer targeting ngspice. Source files are `.ppr`. The project is a Rust workspace.

## Build and test

```sh
cargo build                          # build all crates
cargo build -p piperine-worker       # rebuild worker after ngspice changes
cargo test                           # run all tests (11 integration tests)
cargo test <name>                    # run one test
```

Tests live in `tests/`. They use IPC and require a built worker binary — if tests fail with unexpected events, run `cargo build -p piperine-worker` first.

## Crate responsibilities

| Crate | Role |
|-------|------|
| `piperine-parser` | Lexer + recursive-descent parser → AST. Handles `extern module`, `paramset`, `initial`. |
| `piperine-circuit` | `HardwareDefinition` trait, `HardwareRegistry`, elaboration, net resolution, paramset expansion. |
| `piperine-interpreter` | Procedural interpreter: runs `initial` blocks, dispatches `$op`, `$tran`, etc. |
| `piperine-ngspice` | Plugin: all ngspice hardware defs + `NgspiceBackend` IPC + `ppr/ngspice.ppr` bundled declarations. |
| `piperine-coordinator` | `ProcessPool` — spawns/manages worker subprocesses. |
| `piperine-worker` | Subprocess: wraps libngspice, responds to IPC commands. |
| `piperine-openvaf` | Wraps OpenVAF-Reloaded — compiles `.ppr` VA modules → `.osdi` shared libraries. |
| `piperine-common` | Shared IPC message types (`SimulatorCommand`, `SimulatorEvent`). |

## Key types and patterns

### Hardware definitions

Every ngspice component is a Rust struct implementing `HardwareDefinition`:
- `name() -> &str` — bare module name, no prefix (`"res"`, `"cap"`, `"ind"`, not `"spice_res"`)
- `ports()` — returns `&[]` (ngspice doesn't enforce port order in Rust)
- `parameters()` — returns `&[]` (parameter parsing happens at elaboration)
- `instantiate()` — reads connections + parameters, returns `Box<dyn HardwareInstance>`
- `spice_lines()` — formats the SPICE element line(s)

### `spice_name(prefix, name)` helper

Prepends SPICE element prefix if not already present. `spice_name('R', "res1")` → `"Rres1"`, `spice_name('R', "R1")` → `"R1"`.

### Paramset

```verilog
paramset nmos_lvt nmos;
    .model = "NMOS_LVT";
    .w = 500e-9;
    .l = 180e-9;
endparamset
```

Elaborator emits `.model NMOS_LVT NMOS ...` + uses preset params for instances.

### Ground convention

Net named `gnd` → SPICE node `"0"` (done in elaboration, not in device code).

### IPC pattern

`SimulatorBackend::start_analysis()` / `poll_analysis()` returns owned `AnalysisEvent` — avoids borrow conflict with `respond_to_analysis_event()`.

## `ngspice.ppr` bundled file

Located at `crates/piperine-ngspice/ppr/ngspice.ppr`. Include path exposed via `piperine_ngspice::ppr_dir()`.

Use `` `include "ngspice.ppr" `` at the top of `.ppr` files to get all device declarations.

## Adding a new ngspice device

1. Add `extern module <name>(...)` to `ngspice.ppr`
2. Add Rust struct + `HardwareDefinition` impl in `crates/piperine-ngspice/src/hardware.rs`
3. Register in `register_hardware()` in `crates/piperine-ngspice/src/lib.rs`

Follow the pattern in `docs/development/SPICE_COMPONENTS_IMPL.md`.

## Naming conventions

- Module names: bare, lowercase (`res`, `cap`, `ind`, `nmos`, `jfet_n`)
- Rust structs: `Spice` + PascalCase (`SpiceResistor`, `SpiceNmos`, `SpiceJfetN`)
- No `spice_` prefix in module names

## Files not to edit casually

- `crates/piperine-parser/src/grammar/` — hand-written recursive-descent parser; changes here affect all parsing
- `crates/piperine-circuit/src/elaboration.rs` — net resolution + paramset expansion logic
- `crates/piperine-ngspice/ppr/ngspice.ppr` — extern declarations must match `hardware.rs` exactly

## Tests

Integration tests in `tests/e2e_phase2_interpreter_test.rs` cover:
- Parser round-trips for device names and paramsets
- IPC communication with ngspice worker
- `$op()`, `$tran()`, `$voltage()`, `$current()` system tasks
- Elaboration of all major device types

Run `cargo test -- --nocapture` to see simulator output during test runs.
