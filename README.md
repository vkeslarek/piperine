# Piperine

> ⚠️ **Work in progress — not production ready.** APIs, syntax, and behavior change
> without notice. Use it to explore and contribute, not for anything you depend on.

## What it is

Piperine is a hardware-description language and simulator front-end for analog and
mixed-signal circuits. The model is simple:

- **`.ppr` files** — structural hardware description. Modules, components, paramsets,
  Verilog-A device models. No testbench code.
- **Python files** — testbenches. `import piperine` loads a native Rust extension
  (`piperine.so`) that gives you live ngspice sessions as first-class Python objects.

Hardware and its bench live together by design:

```
my_project/
  piperine.toml
  lpf/
    lpf.ppr       ← circuit description
    lpf.py        ← testbench (pure Python)
  amp/
    amp.ppr
    amp.py
```

## Quick start

```sh
cargo build --release
piperine new my_project
cd my_project
piperine run hello/hello.py
# V(vmid) = 5.000 V
```

`piperine setup` builds the `piperine.so` extension into a project-local `.venv`.
`piperine run` activates it and runs your Python bench.

## Testbench examples

### Simple — voltage divider OP

```python
import piperine as ppr

sess = ppr.NgspiceSession.from_file("divider/divider.ppr", module="divider")
op = sess.op()
print(f"V(mid) = {op['vmid']:.3f} V")
```

### AC sweep — RC low-pass filter

```python
import piperine as ppr
import numpy as np

sess = ppr.NgspiceSession.from_file("lpf/lpf.ppr", module="lpf_tb")
ac = sess.ac("dec", 50, 100.0, 100e3)

freq = ac["frequency"]
vout = ac["out"]                           # magnitude |V|
vout_db = 20 * np.log10(vout / vout[0])
idx = np.argmin(np.abs(vout_db + 3.0))
print(f"fc (-3 dB) ≈ {freq[idx]:.1f} Hz")
```

### Parallel Monte Carlo — 30 workers simultaneously

```python
import piperine as ppr
import numpy as np

N = 30
sessions = [ppr.NgspiceSession.from_file("lpf/lpf.ppr", module="lpf_tb")
            for _ in range(N)]

rs = np.random.normal(1000, 30, N)
cs = np.random.normal(100e-9, 3e-9, N)
for sess, r, c in zip(sessions, rs, cs):
    sess.alter("R1", "resistance", r)
    sess.alter("C1", "capacitance", c)

futures = [sess.tran_async("1n", "1u") for sess in sessions]
results = ppr.join_all(futures)           # wall time ≈ slowest worker
```

All 30 workers run in parallel. GIL released during simulation.

## Hardware description

`.ppr` files are structural hardware only. Paramsets create named device variants:

```verilog
`include "ngspice.ppr"

paramset lpf_r res; .r = 1000.0; endparamset
paramset lpf_c cap; .c = 100e-9;  endparamset

module lpf(in, out);
    inout in, out;
    lpf_r R1(.p(in), .n(out));
    lpf_c C1(.p(out), .n(gnd));
endmodule

// Testbench module — adds an AC source for frequency sweep.
module lpf_tb;
    wire in, out;
    vsource #(.dc(0), .acmag(1)) Vin(.p(in), .n(gnd));
    lpf DUT(.in(in), .out(out));
endmodule
```

SOA guards live in hardware modules as `always @(step)` blocks, compiled to
`.meas` at elaboration — no runtime interpreter:

```verilog
module bjt_amp;
    // ...
    always @(step) begin
        if (V(c) > 30.0) $run_error("Vce_max");
    end
endmodule
```

```python
sess.tran("1n", "1u")
sess.check_soa()   # raises RuntimeError if any limit was exceeded
```

## How it works

```
.ppr ──parse──▶ AST ──┬── VA modules ──▶ OpenVAF ──▶ .osdi ──┐
                      │                                       ▼
                      └── elaborate_circuit ──▶ SPICE netlist ──▶ ngspice (worker)
                                                                       ▲
import piperine ──▶ NgspiceSession (PyO3) ─────────────────────────────┘
                    .op() / .ac() / .tran() / .alter() / .tran_async() / …
```

ngspice runs in an isolated worker subprocess — a crash there cannot reach Python.
Multiple sessions run in parallel Rust threads; GIL released during simulation.

See [ARCHITECTURE.md](ARCHITECTURE.md) for more detail.
For the component reference see [`docs/`](docs/).

## Build and test

```sh
cargo build                      # all crates + worker binary
cargo build -p piperine-worker   # rebuild worker after ngspice changes
cargo test                       # full suite
```

Tests in `tests/` use IPC and require a built worker binary. If tests fail with
unexpected events, run `cargo build -p piperine-worker` first.

## Contributing

Conventions and agent guidance: [`CLAUDE.md`](CLAUDE.md), [`AGENTS.md`](AGENTS.md).
Design notes: [`docs/development/`](docs/development/).

| Crate | Role |
|-------|------|
| `piperine-parser` | Lexer + recursive-descent parser → AST |
| `piperine-circuit` | `HardwareDefinition` trait, elaboration, paramsets, net resolution |
| `piperine-ngspice` | ngspice device impls, IPC backend, bundled `ngspice.ppr` |
| `piperine-python` | PyO3 native extension — `NgspiceSession`, `SimFuture`, `join_all` |
| `piperine-coordinator` | `ProcessPool` — spawns/manages worker subprocesses |
| `piperine-worker` | Subprocess wrapping libngspice via FFI; answers IPC |
| `piperine-common` | IPC message types shared by coordinator and worker |
| `piperine-openvaf` | Compiles Verilog-A modules → `.osdi` via OpenVAF-Reloaded |
| `piperine-interpreter` | Runs `always @(step)` handlers during analyses |
