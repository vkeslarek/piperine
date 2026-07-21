<div align="center">

<img src="assets/logo.svg" alt="Piperine Logo" width="50%"/>

**A modern hardware-description language (HDL) and simulator for analog and mixed-signal circuits.**

</div>

> ⚠️ **Work in progress — not production ready.** APIs, syntax, and behavior
> change without notice. Use it to explore and contribute, not for anything you
> depend on.
>
> 🛑 **Disclaimer:** PHDL is an EXPERIMENTAL language!

Piperine unifies continuous (Newton-Raphson) and discrete (event-driven) hardware into a
single, cohesive model. Its native language **PHDL** (`.phdl`) compiles through a shared
intermediate representation into a **pure-Rust, in-house solver** — analog devices are
JIT-compiled to native code via Cranelift, digital behavior runs on an event-driven
interpreter. No SPICE underneath. Industry-standard Verilog-A device models plug in as
compiled **OSDI** (v0.4) shared libraries; a native Verilog-AMS frontend targeting the same
IR is being reworked.

```
PHDL (.phdl) ──► elaborated design ──► Cranelift-JIT analog devices ──► native solver
                                             + event-driven digital interpreter
                                             + OSDI (.osdi) device models
hosts: Python (`import piperine`) and Rust (`piperine-api`) drive analyses and measurement
```

## Highlights

- **One mixed-signal model.** `analog` and `digital` blocks share the same module
  constructs; the boundary between them is explicit and type-checked. A digital register
  read inside an analog contribution is a feature, not a hack.
- **Native solver, no external SPICE.** DC operating point, adaptive transient, AC
  small-signal (including `ac_stim` stimuli), and noise analysis (white + flicker device
  PSDs), all over MNA with symbolic Jacobians — the derivative of your device model is
  computed symbolically and JIT-compiled next to its residual.
- **Python testbenches.** Verification is real code in a real language: `import piperine`
  gives you the elaborated design, `op`/`tran`/`ac`/`noise` analyses with typed config
  dataclasses, numpy waveforms (`values`/`axis`, `cross`, `rms`, `mag`/`db`), parameter
  sweeps as plain `for` loops, and a compile-once `LiveSession` for optimization loops.
  Run them with `piperine test` (`*_tb.py`) or `piperine run script.py`. No TCL, no
  `.measure` mini-language.
- **A real language.** Generics with const parameters, capabilities (traits), bundles,
  enums, default parameter values, `Map`/`Vec`/`Option` value types, SI-suffixed literals
  (`2.2u`, `10k`), typed attributes — resolved at elaboration by a pure evaluator, never a
  macro stage.
- **No-Magic philosophy.** Type conversions and domain crossings are explicit. Anything the
  toolchain cannot compile faithfully is a *named error*, never a silent zero.
- **Builtin SPICE device library.** `use spice::diode;` (or `bjt`, `mos`, `jfet`,
  `passives`, `sources`, `controlled`, `switches`) works in any project with no
  dependency — the ngspice-faithful PHDL models ship as stdlib headers
  (`crates/piperine-lang/headers/spice/`), translated line-by-line from the
  ngspice C sources.
- **OSDI support.** Loads `.osdi` v0.4 compiled device models (the standard Verilog-A
  compilation target) alongside JIT-compiled PHDL devices.
- **Tooling included.** A `piperine` CLI (project scaffolding, git dependencies, check,
  format, run, test), an LSP language server (diagnostics, hover, completion,
  go-to-definition, formatting, semantic tokens, rename),
  and a VS Code extension under `editors/vscode/`.

## What it looks like

### True mixed-signal: first-order delta-sigma modulator

A single component crosses the analog/digital boundary twice — the `analog` block handles
continuous integration while the `digital` block handles discrete clock edges:

```phdl
mod DeltaSigma ( input vin : Electrical, inout gnd : Ground, input clk : Bit, output dout : Bit ) {
    param c : Real = 1.0e-12;
    param r : Real = 1.0e3;
    param vref : Real = 1.0;

    wire intg : Electrical;            // integrator output
    var  q : Bit = 0;                  // quantizer register (held across clocks)
}

analog DeltaSigma {
    var vfb : Real = if (q == 1) { vref } else { -vref };   // digital state read in analog
    I(intg, gnd) <+ c * ddt(V(intg, gnd));                  // integrating capacitor
    I(intg, gnd) <+ (vfb - V(vin)) / r;                     // (feedback − input) drives the node
}

digital DeltaSigma {
    dout <- q;
    @ posedge(clk) { q = (V(intg) > 0.0); }                 // clocked 1-bit quantizer
}
```

### Parametric generation, resolved at compile time

Structural and behavioral scaling is native `for`-loop data generation with const
parameters `[N]` — no external netlist generators:

```phdl
mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }

// N-stage RC ladder
mod Ladder[N] ( inout bus : Electrical, inout gnd : Ground ) {
    param r : Real = 1.0e3;
    param cpar : Real = 5.0e-15;

    wire tap : Electrical[N];

    for i in 0..N {
        rseg[i] : Resistor ( bus, tap[i] )    { .r = r };
        rgnd[i] : Resistor ( tap[i], gnd )    { .r = r };
    }
}

analog Ladder {
    for i in 0..N {
        // Probe child instance nodes to add parasitics
        I(rseg[i].n, gnd) <+ cpar * ddt(V(rseg[i].n, gnd));
    }
}
```

### Verification is code, not an afterthought

A Python testbench (`*_tb.py`) drives analyses over the elaborated netlist. Measurement
goes through immutable result objects; configuration is a dataclass, never hidden state:

```python
import piperine

m = piperine.load("src/main.phdl").module("SwitchOpenTest")

r = m.op()
assert r.v("vsrc", "gnd") > 4.9, "voltage source should be active"
assert abs(r.i("vsrc", "gnd")) < 1e-8, "no current with the switch open"

r = m.ac(piperine.AcConfig(fstart=1.0, fstop=1e9, points=100))
assert r.v("out").db().at(1e3) > -3.0, "passband flat at 1 kHz"

curve = []
for rl in [1e3, 1e4, 1e5, 1e6]:   # a sweep is a loop, not a task
    m.set("load", "resistance", rl)
    r = m.op()
    curve.append((rl, r.v("out") / r.v("in_")))
```

Run it with `piperine test` — every `*_tb.py` in the project is discovered and run.

### Typed metadata (attributes)

No unstructured `PRAGMA` comments. Layout, routing, and floorplanning intent attach as
schema-validated attributes directly on netlist components:

```phdl
@layout(min_width = 2.0e-6, layer = "m3")
@route(priority = high)
wire clk : Electrical;
```

## Getting started

```sh

piperine new my_chip             # scaffold a project (Piperine.toml + src/)
piperine check src/main.phdl     # parse, elaborate, sanity-check
piperine fmt   src/main.phdl     # canonical formatting
piperine test                    # discover and run every *_tb.py testbench
piperine run script.py           # run a python script (embedded CPython)
piperine run -i src/main.phdl    # interactive REPL with the design loaded
piperine add <git-url>           # add a dependency (resolved via git)
piperine tree                    # show the dependency tree
```

The `examples/` directory holds a gallery of self-contained real-world designs with numerically
validated Python testbenches — voltage dividers, RC filters, diode clippers, DACs, a flash ADC, a
bang-bang thermostat, Johnson noise, a coulomb counter, PWM, an op-amp follower — every `.phdl`
elaborates and every `.py` runs green in CI (`tests/run_examples.rs`).

## IDE support

`piperine-lang-server` speaks LSP: diagnostics with real spans, hover, context-aware
completion (parser-predicted), go-to-definition, document symbols, formatting, semantic
tokens, references/rename, folding, inlay hints (SI-literal expansion: `10k` → `= 10000`).
The VS Code extension lives in `editors/vscode/`.

## Documentation

| Document | What it covers |
|----------|----------------|
| `docs/spec/` (Parts I–VII + appendices) | The formal PHDL specification |
| `docs/spec/part_viii_host_api.md` | The Python + Rust host APIs (load/Design/Module, analyses, LiveSession, CLI) |
| `ROADMAP.md` | Open work items |
