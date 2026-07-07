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
PHDL (.phdl) ──► elaborated design ──► shared IR ──► Cranelift-JIT analog devices ──► native solver
                        │                            + event-driven digital interpreter
                        └──► bench (interpreted verification layer)      + OSDI (.osdi) device models
```

## Highlights

- **One mixed-signal model.** `analog` and `digital` blocks share the same module
  constructs; the boundary between them is explicit and type-checked. A digital register
  read inside an analog contribution is a feature, not a hack.
- **Native solver, no external SPICE.** DC operating point, adaptive transient, AC
  small-signal (including `ac_stim` stimuli), and noise analysis (white + flicker device
  PSDs), all over MNA with symbolic Jacobians — the derivative of your device model is
  computed symbolically and JIT-compiled next to its residual.
- **Built-in verification (`bench`).** An effectful, interpreted scripting layer runs
  right in your source file: `$op` / `$tran` / `$ac` / `$noise` analyses with typed config
  bundles, waveform post-processing (`fft`, `rise_time`, `db`, closures via `map`),
  parameter staging and sweeps as plain `for` loops, CSV export via `$write`. No TCL, no
  `.measure` mini-language.
- **A real language.** Generics with const parameters, capabilities (traits), bundles,
  enums, default parameter values, `Map`/`Vec`/`Option` value types, SI-suffixed literals
  (`2.2u`, `10k`), typed attributes — resolved at elaboration by a pure evaluator, never a
  macro stage.
- **No-Magic philosophy.** Type conversions and domain crossings are explicit. Anything the
  toolchain cannot compile faithfully is a *named error*, never a silent zero.
- **OSDI support.** Loads `.osdi` v0.4 compiled device models (the standard Verilog-A
  compilation target) alongside JIT-compiled PHDL devices.
- **Tooling included.** A `piperine` CLI (project scaffolding, git dependencies, check,
  format, run, test), an LSP language server (diagnostics, hover, completion,
  go-to-definition, formatting, semantic tokens, rename, code lenses that run your benches),
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

A `bench` block attaches to a module and runs analyses over the elaborated netlist.
Measurement goes through immutable result objects; configuration is a value, never hidden
state:

```phdl
bench SwitchOpenTest {
    fn test_open_circuit() {
        var r = $op();
        $assert(r.v(vsrc, gnd) > 4.9, "voltage source should be active");
        $assert(r.i(resistor.p, resistor.n) < 1e-8, "no current with the switch open");
    }

    fn test_bandwidth() {
        var r = $ac(AcConfig { .fstart = 1.0, .fstop = 1e9, .points = 100 });
        $assert(r.v(out).db().at(1e3) > -3.0, "passband flat at 1 kHz");
    }

    fn dc_gain_vs_load() {
        var curve : Vec<(Real, Real)> = [];
        for rl in [1e3, 1e4, 1e5, 1e6] {   // a sweep is a loop, not a task
            load.resistance = rl;           // stage an override
            var r = $op();                  // deterministic re-elaborate + solve
            curve.push((rl, r.v(out) / r.v(in_)));
        }
        $write("gain_vs_load.csv", curve);
    }
}
```

Run it with `piperine test` — every zero-argument bench `fn` is a discovered entry point.

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
piperine test                    # discover and run every bench entry point
piperine run --entry Amp::tune   # run one bench fn
piperine add <git-url>           # add a dependency (resolved via git)
piperine tree                    # show the dependency tree
```

The `examples/` directory holds a gallery of self-contained real-world designs with numerically
validated benches — voltage dividers, RC filters, diode clippers, DACs, a flash ADC, a bang-bang
thermostat, Johnson noise, a coulomb counter, PWM, an op-amp follower — all of
them run green in CI via `piperine-bench/tests/run_examples.rs`.

## IDE support

`piperine-lang-server` speaks LSP: diagnostics with real spans, hover, context-aware
completion (parser-predicted), go-to-definition, document symbols, formatting, semantic
tokens, references/rename, folding, inlay hints (SI-literal expansion: `10k` → `= 10000`),
and code lenses that run benches. The VS Code extension lives in `editors/vscode/`.

## Documentation

| Document | What it covers |
|----------|----------------|
| `crates/piperine-lang/docs/SPEC.md` | The PHDL language: types, modules, behavior, elaboration, grammar, reflection, selector, builtins |
| `crates/piperine-bench/docs/SPEC.md` | The `bench` block, analyses, result/waveform types, the uniform API |
| `crates/piperine-codegen/docs/SPEC.md` | The shared IR and lowering contract |
| `crates/piperine-cli/docs/CLI_TOOLS.md` | CLI commands and project management |
| `ROADMAP.md` | Open work items |
