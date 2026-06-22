# Piperine

> ⚠️ **Work in progress — not production ready, not even close.** Piperine is an
> active experiment: APIs, syntax, and behavior change without notice, and plenty
> is half-built or missing. Use it to explore and contribute, not for anything
> you depend on.

## What it is

Piperine is a hardware-description language and simulator front-end for
analog and mixed-signal circuits. A single `.ppr` file holds two things that are
normally split across separate tools:

- **Device physics** — a superset of Verilog-A, compiled to OSDI device models.
- **Testbenches** — a SystemVerilog-style procedural layer: variables, loops,
  functions, math, and system tasks that drive the simulation and read results back.

Under the hood it elaborates your circuit to a SPICE netlist and runs it on
**ngspice**, which executes in an isolated worker process so a simulator crash
can never take your testbench down with it.

## What it's for

Analog verification usually means juggling three languages: Verilog-A for the
model, a SPICE deck for the netlist, and Tcl or Python to sweep parameters and
post-process. Piperine collapses that into one:

```verilog
`include "ngspice.ppr"

module rc_lowpass;
    // ── circuit ────────────────────────────────────────────────
    vsource #(.dc(0.0), .acmag(1.0)) Vin(.p(in), .n(gnd));
    res     #(.r(1e3))               R1 (.p(in), .n(out));
    cap     #(.c(159e-9))            C1 (.p(out), .n(gnd));

    // ── testbench ──────────────────────────────────────────────
    initial begin
        AcResult ac   = $ac("dec", 100, 10.0, 1e6);
        Signal   vout = ac.signal("v(out)");

        real f3db = vout.bandwidth_3db();
        $display("-3 dB bandwidth = %e Hz", f3db);

        assert (f3db > 900.0) else $error("corner too low: %e Hz", f3db);
    end
endmodule
```

Run it:

```sh
piperine rc_lowpass.ppr
```

The `initial` block *is* the test: it launches analyses (`$op`, `$tran`, `$ac`,
`$noise`, …), gets back typed result objects, and measures signals directly
(`.max()`, `.rms()`, `.bandwidth_3db()`, …). No external scripting layer.

## How to use it

### Install

```sh
cargo build --release
# binary at target/release/piperine
```

Requires Rust 1.85+, libngspice, and LLVM (for the OpenVAF-Reloaded model compiler).

### Run a file

```sh
piperine my_testbench.ppr
```

### Write a testbench

1. `` `include "ngspice.ppr" `` to get every built-in ngspice device
   (`res`, `cap`, `nmos`, `vsource`, `diode`, …).
2. Instantiate your circuit with `module #(.param(value)) name(.port(net), …);`.
3. Drive it from an `initial` block using analyses and measurements.

The full language and component references live in [`docs/`](docs/):

| Topic | Where |
|-------|-------|
| Language reference (types, statements, functions, stdlib) | [`docs/lang/`](docs/lang/) |
| ngspice components (every device + parameters) | [`docs/ngspice/`](docs/ngspice/) |
| Writing Verilog-A / OSDI device models | [`docs/openvaf/`](docs/openvaf/) |

New to the language? Start with [`docs/lang/overview.md`](docs/lang/overview.md).

## How it works

```
.ppr ──parse──▶ AST ──┬─ VA modules ──▶ OpenVAF ──▶ .osdi ─┐
                      │                                    ▼
                      └─ elaborate ──▶ SPICE netlist ──▶ ngspice (worker process)
                                       initial block ──▶ Interpreter ⇄ ngspice (IPC)
```

The interpreter knows nothing about ngspice specifically — simulators plug in
behind a `SimulatorBackend` trait. For the full picture (crate responsibilities,
the plugin and IPC boundaries, how an analysis runs) see
[**ARCHITECTURE.md**](ARCHITECTURE.md).

## Contributing

Piperine is a Rust workspace. Each crate has one job:

| Crate | Responsibility |
|-------|----------------|
| `piperine-parser` | Lexer + recursive-descent parser → AST |
| `piperine-circuit` | `HardwareDefinition` trait, elaboration, paramsets, net resolution |
| `piperine-interpreter` | Runs `initial`/`always` blocks; `Value`, `SystemTask`, `SimulatorBackend` traits |
| `piperine-ngspice` | ngspice device defs, system tasks, IPC backend, bundled `ngspice.ppr` |
| `piperine-openvaf` | Compiles Verilog-A modules → `.osdi` |
| `piperine-coordinator` | Spawns and owns worker subprocesses (`ProcessPool`) |
| `piperine-worker` | The subprocess that wraps libngspice via FFI |
| `piperine-common` | IPC message types shared by coordinator and worker |

### Build and test

```sh
cargo build                      # everything, including the worker binary
cargo test                       # full suite
cargo build -p piperine-worker   # rebuild just the worker after ngspice changes
```

Integration tests in `tests/` exercise parsing, elaboration, the interpreter, and
end-to-end IPC with a real worker.

### Where to start

Conventions and agent guidance live in [`CLAUDE.md`](CLAUDE.md) and
[`AGENTS.md`](AGENTS.md); deeper design notes in [`docs/development/`](docs/development/).
