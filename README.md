# Piperine

Piperine is a hardware description language and simulator frontend for analog/mixed-signal circuit simulation. It combines a Verilog-A superset (for describing analog device physics) with a SystemVerilog-like procedural layer (for testbenches) and targets ngspice as its simulation engine.

## What it is

- **Language**: `.ppr` files contain `module` definitions, `extern module` declarations, `paramset` bindings, and `initial` blocks
- **Backend**: ngspice via bilateral IPC (worker subprocess wraps libngspice)
- **Device models**: Verilog-A modules compiled to OSDI shared libraries via OpenVAF-Reloaded
- **Built-in library**: `ngspice.ppr` — pre-declared extern modules for every ngspice component

## Quick start

```sh
cargo build --release
./target/release/piperine examples/rc_filter.ppr
```

A `.ppr` file runs its `initial` block, which drives simulation via system tasks like `$op()`, `$tran()`, `$voltage()`, and `$current()`.

## Example

```verilog
`include "ngspice.ppr"

module rc_filter;
    res #(.r(1e3)) R1(.p(in), .n(mid));
    cap #(.c(100e-12)) C1(.p(mid), .n(gnd));
    vsource #(.dc(1.0)) Vsrc(.p(in), .n(gnd));

    initial begin
        $op();
        $display("Vmid = %f", $voltage(mid));
    end
endmodule
```

## Repository layout

```
src/                        # piperine binary (main entry point)
crates/
  piperine-parser/          # .ppr lexer + parser → AST
  piperine-circuit/         # HardwareDefinition trait, elaboration, paramset
  piperine-interpreter/     # Procedural interpreter ($op, $tran, $voltage …)
  piperine-ngspice/         # ngspice plugin: hardware defs + IPC backend
    ppr/ngspice.ppr         # bundled extern declarations for all ngspice devices
  piperine-coordinator/     # Worker process pool manager
  piperine-worker/          # Subprocess wrapping libngspice
  piperine-openvaf/         # Compiles Verilog-A → OSDI via OpenVAF-Reloaded
  piperine-common/          # Shared IPC message types
docs/
  lang/                     # Piperine language reference
  ngspice/                  # ngspice component reference
  openvaf/                  # OpenVAF/OSDI device model guide
  development/              # Internal design docs and implementation plans
```

## Documentation

| Topic | Location |
|-------|----------|
| Language reference | `docs/lang/` |
| ngspice components | `docs/ngspice/` |
| Verilog-A / OSDI models | `docs/openvaf/` |
| Development internals | `docs/development/` |

## Building

```sh
# Build everything (including worker binary)
cargo build

# Run tests
cargo test

# Build worker separately (needed if tests use IPC)
cargo build -p piperine-worker
```

Requires: Rust 1.85+, libngspice, LLVM (for OpenVAF-Reloaded).

## Architecture overview

```
.ppr file → parser → AST
                        ↓
              elaboration (circuit)
                        ↓
              SPICE netlist lines
                        ↓
           NgspiceBackend (IPC) → piperine-worker → libngspice
                        ↑
              Interpreter runs initial block
              (calls $op, $tran, $voltage, etc.)
```

See `docs/lang/overview.md` for the full language walkthrough.
