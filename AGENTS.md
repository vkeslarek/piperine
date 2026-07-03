# Piperine — Agent Instructions

This file briefs AI coding agents on the Piperine codebase. Read it before making changes.

## What Piperine is

A hardware-description language and simulator for analog and mixed-signal circuits.
It has a frontend that lowers to an intermediate representation (IR); the IR
in turn compiles (via Cranelift JIT + a tree-walking digital interpreter) into the
solver's `Device` trait:

```
                                                ┌──────────────────┐
                                                │  piperine-codegen │
                                                │   (IR + lowering)  │
   .phdl / .ppr    ──►  piperine-lang  ──►      └────────┬─────────┘
   (PHDL / .ppr)     ◄─►   frontend     ◄─►               │
                                                            ▼
                                                  Vec<Box<dyn Device>>
                                                            │
                                                  ┌─────────┴─────────┐
                                                  ▼                   ▼
                                       ┌──────────────────┐  ┌──────────────────────┐
                                       │  piperine-solver   │  │  piperine-solver OSDI │
                                       │  (Newton-Raphson,  │  │  (.osdi shared libs,  │
                                       │   trapezoidal,     │  │   optional / future)  │
                                       │   mixed-signal)     │  │                         │
                                       └──────────────────┘  └──────────────────────┘
```

The solver does **not** depend on the codegen — the IR is the contract they share.

## Build and verify

Always build and run tests before declaring work done:

```sh
cargo build                  # build the workspace
cargo test                   # must pass, check tests-baseline.md for count
```

The current baseline is captured in `tests-baseline.md`.

## Workspace map

```
crates/
├── piperine-lang/          PHDL frontend
│   ├── src/{parse,elab,resolve,stdlib}/
│   └── tests/examples/      PHDL reference files (→ IR regression)
├── piperine-codegen/       IR central + lowering to Device
│   ├── src/ir.rs            IrProgram, IrExpr, IrStmt, IrEventKind, …
│   ├── src/from_ams.rs      ams_to_ir(...)
│   ├── src/from_ppr.rs      ppr_to_ir(...)
│   ├── src/from_ir.rs       from_ir(IrProgram, top) → CircuitInstance
│   ├── src/ir_analog_to_device.rs   ir_analog_to_device(IrProgram, module) → JitAnalogDevice
│   ├── src/ir_digital_to_interp.rs   ir_digital_to_interp(IrProgram, module) → DigitalInterpreter
│   ├── src/codegen/         Cranelift JIT + autodiff + expr emitter (was in piperine-lang)
│   ├── src/phdl_device.rs   PhdlDevice wraps Device for mixed-signal
│   └── tests/               IR unit + API pinning + E2E solver tests
├── piperine-solver/         Newton-Raphson, AC/DC/Tran/Noise/TF, OSDI loader
│   ├── src/{analog,digital,osdi}/   + solver/, math/, topology.rs
│   └── tests/{osdi_integration, cosim_integration, mixed_signal_tests, digital_topology_tests}.rs
│   └── tests/va/            canonical Verilog-A fixtures (resistor, cap, vsource, …)
├── piperine-cli/            clap-based subcommands (`check`, `build`, `run`, …)
├── piperine-project/        Piperine.toml reader
└── tools/OpenVAF-Reloaded/  external submodule (used only for OSDI tests)
```

## Translation pipeline (TDD-anchored)

| Step | Function | Test file |
|------|----------|-----------|
| AMS → IR | `ams_to_ir(doc)` | `tests/ams_ir_test.rs` (54) |
| PPR → IR | `ppr_to_ir(prog)` | `tests/ppr_ir_test.rs` (23) |
| IR → analog Device | `ir_analog_to_device(prog, name)` | `tests/ir_analog_to_device_tests.rs` |
| IR → digital interpreter | `ir_digital_to_interp(prog, name)` | `tests/ir_digital_to_interp_tests.rs` |
| IR → CircuitInstance | `from_ir(prog, top)` | `tests/from_ir_tests.rs` |
| AMS E2E | compile fixtures, drive solver | `tests/ams_ir_e2e_tests.rs` |
| PPR + AMS E2E | IR-built CircuitInstance → DcSolver | `tests/codegen_e2e_tests.rs` |

## Conventions

- **Panics:**  never `unwrap()`/`expect()` on user input paths; return `Result<String, ...>`.
- **Files in `tests/fixtures/`, `tests/fixtures_fmt/`, `tests/fixtures_ppr/`, `headers/`**:  do not edit; they are frozen test corpora.
- **Dependency direction:** `piperine-solver` does **not** depend on `piperine-codegen`.  The codegen depends on the solver (`Device`, `CircuitInstance`) because it lowers IR into it.  Breaking the arrow is a regression — `cargo metadata | grep -E '(name|path)'` if in doubt.
- **Numeric conventions:**  analog values are `f64`; digital is `LogicValue`; mixed-signal nets are anonymous `usize` indices.
- **Comments:** keep module-level `//!` docblocks updated when adding a new entry point; the test files in `piperine-codegen/tests/` describe the API surface via passing tests.

## Adding a new Verilog-A / PHDL device

1. Write a test in `crates/piperine-codegen/tests/codegen_e2e_tests.rs` that exercises the new device end-to-end through `from_ir` → DcSolver.
2. Translate the device's spec to IR via the `ams_to_ir` path (or `ppr_to_ir` if writing the PHDL form).  Use `ir_analog_to_device` / `ir_digital_to_interp` to lower.
3. If the lowering fails (uncommon constructs), extend `ir_expr_to_phdl` / `ir_stmt_to_phdl` in `crates/piperine-codegen/src/ir_analog_to_device.rs` and `ir_digital_to_interp.rs`.

## Testing gotchas

- After touching codegen, **always rerun `cargo test -p piperine-codegen`** — many crates import its API and a regression here cascades.
- If tests fail with `expected `&Path`, found `PathBuf`, add `&` before `Path::new(...)` — the solver's API uses `&Path`.
- For OSDI tests (`cargo test -p piperine-solver`), `OPENVAF_BIN` must be in the PATH.

## Documentation locations

- Language spec: `docs/piperine-hdl-spec.md`
- BNF AMS: `docs/BNF-AMS.md`
- IR system: `crates/piperine-codegen/IR-SYSTEM.md`
- Baseline test counts: `tests-baseline.md`
