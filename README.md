<div align="center">

<img src="assets/logo.svg" alt="Piperine Logo" width="50%"/>

**One language for analog, digital and mixed-signal circuits, and a native solver that runs it.**

</div>

> **Work in progress.** The language, the command line and the library still
> change. Use Piperine to explore and to contribute, not yet for work you depend on.

---

## What is Piperine?

Piperine is a circuit simulator built around **MSL**, the Piperine mixed-signal
language. You describe a circuit's parts and its test bench in MSL, and the
`piperine` command runs it on Piperine's own solver: operating point, DC sweep,
transient, AC, transfer function, poles and zeros, sensitivity and periodic
steady state. There is no SPICE underneath.

- **One language for every domain.** An `analog` block contributes currents and
  voltages (`i(p, n) +> v(p, n) / r;`), a `digital` block reacts to events
  (`on clk.rise() { q <- d; }`), and the two meet in the same module. Benches,
  sources and monitors are MSL too.
- **Typed and checked before it runs.** Params carry checks (`r: f64 where r > 0.0`),
  natures keep electrical and other quantities apart, and every mistake is
  reported with a code and the place it happened.
- **Your model's records are real.** `emit!` writes a record to a named stream;
  a record written during a rejected step never appears, and nothing in the
  language can read a stream back, so printing never changes the result.
- **Checked against ngspice.** The analog engine is cross-checked against ngspice
  on a corpus of twin circuits, under fixed tolerances.

## A first run

```sh
piperine new rc        # an RC low-pass filter with a step-response bench
cd rc
piperine run op        # the operating point
piperine run step      # a 5 µs transient
```

The resistor in the generated library is the whole model:

```rust
/// Ideal resistor.
pub module Resistor(p: Electrical, n: Electrical) {
    r: f64 where r > 0.0;
}

analog for Resistor {
    i(p, n) +> v(p, n) / r;
}
```

The bench places the parts and watches the output:

```rust
module Step() {
    signal vin: Electrical;
    signal vout: Electrical = 0.0;
    let v1 = Vsource(vin, gnd) { dc: 1.0 };
    let r1 = Resistor(vin, vout) { r: 1k };
    let c1 = Capacitor(vout, gnd) { c: 1n };
    let mon = Monitor(vout);
}
```

## Examples

- [`examples/rc`](examples/rc): the RC filter above.
- [`examples/buck`](examples/buck): a digitally controlled buck converter: 12 V
  in, a 200 MHz DPWM, an ADC, a PID loop and a supervisor with soft start,
  current limit and over-voltage protection, written to a specification
  ([`SPEC.md`](examples/buck/SPEC.md)) that its tests check.

## Documentation

- **The MSL Book** ([`docs/book/`](docs/book)): the user manual. Part 1 teaches
  the language, running simulations and the command line; Part 2 goes into the
  object model, the device ABI, the solver and how Piperine is organized.
- **The specification** ([`docs/spec/`](docs/spec)): the authoritative definition
  of MSL.
- **The documentation site** builds both with `mkdocs build`; the `Docs` workflow
  also produces the specification as a PDF.

## Building

Piperine is a Rust workspace (edition 2024). The compiled backend turns a design
into Rust, so running a model needs a Rust toolchain on the machine.

```sh
cargo build --release      # builds the piperine command
cargo test                 # the test suite
```

| Crate | Role |
| --- | --- |
| `piperine-lang` | MSL front end: parser, object model, name resolution, type checker, elaboration to the IR |
| `piperine-ir` | The elaborated design as an object model, with its printer and checks |
| `piperine-plan` | What the two backends share: the plan of each body |
| `piperine-transpile` | The compiled backend: a design printed as Rust and built with `rustc` |
| `piperine-interp` | The reference interpreter, a second backend for checking the first |
| `piperine-abi` | The device ABI between compiled models and the solver |
| `piperine-analog-solver` | The analog engine: assembly, Newton, TR-BDF2, step control |
| `piperine-digital-solver` | The digital engine: nets, events, microsteps, commits |
| `piperine-solver` | The analyses and the protocol that lets the two engines take turns |
| `piperine-api` | The host library: sessions, runs, results and streams |
| `piperine-cli` | The `piperine` command |
| `piperine-project` | Packages: `Piperine.toml`, dependencies and the lockfile |
| `piperine-parity` | The ngspice parity harness and the end-to-end scenarios |

## License

See [LICENSE](LICENSE).
