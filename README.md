<div align="center">

<img src="assets/logo.svg" alt="Piperine Logo" width="50%"/>

**One language for analog, digital and mixed-signal circuits, and a native solver that runs it.**

</div>

> **Work in progress.** The language, the command line and the library still
> change. Use Piperine to explore and to contribute, not yet for work you depend on.

Piperine is a circuit simulator built around **MSL**, the Piperine mixed-signal
language. A device, a logic block, a converter with its control loop and the
bench that tests it are all written in MSL, and the `piperine` command runs them
on Piperine's own solver: there is no SPICE underneath. Every outcome of a
simulation is a function of the source and of what the host does, so two
reactions never race. The snippets below show the whole language; [Status](#status)
says which parts run from source today.

## The language in one module

A module declares ports, params and signals. An `analog` block gives it
continuous-time equations, a `digital` block gives it reactions to events, and one
module may have both, with no interface element in between:

```rust
/// Samples `vin` on each rising clock edge and holds it on `vout`.
module SampleHold(vin: Electrical, vout: Electrical, in clk: Bit) {
    signal held: Bit<f64> = 0.0;
}

digital for SampleHold { on clk.rise() { held <- v(vin); } }
analog  for SampleHold { v(vout) +> held; }
```

MSL has eleven concepts, each with one rule:

| Concept | What it is |
| --- | --- |
| value and `fn` | Data with no time: integers of any width, `f64`, structs, enums, arrays, `Option`, closures. A function keeps no state and writes no carrier. |
| nature | What a signal holds and how its drives combine: `Electrical` (potential and flow, conserved), `Bit` (one driver), `Wire` (resolved, with strengths), or your own. |
| carrier | A `signal` or a port of a nature. It exists over time and is connected by being passed to a port. A bundle is a `struct` of carriers. |
| drive | What one module writes to one carrier: a definition `y = expr`, a commit `q <- x`, an initial value, or a branch contribution on a node. |
| param | A value fixed during a solve. The model reads it, the host sets it; a `where` condition is checked on every set. |
| module | Ports, params and a netlist of signals and instances. Anything that keeps state is a module, `ddt`, `idt` and `cross` included. |
| `let` | Names an expression, or places an instance: `let r1 = Resistor(a, b) { r: 1k };` |
| `analog for M` | `probe +> x` adds `x` to the flow or the potential of a branch, at every Newton iteration. |
| `digital for M` | A reaction `on ev { q <- x; }` runs once when `ev` fires; its commits apply after every reaction of that instant. |
| event | A value of type `Event`: `rise`, `fall`, `changed`, `cross`, `above`, `at`, `every`, `init`, `settle`, combined with `\|\|` and guarded with `&&`. |
| instant | At one time, microsteps run until nothing is pending; an analog crossing is located inside the step and the reaction runs there. |

## Analog

Contributions, `ddt`, and a branch that is a short on one path and an open
circuit on the other:

```rust
analog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }
analog for Inductor  { v(p, n) +> ddt(l * i(p, n)); }
analog for Switch    { if ctl { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; } }
```

A junction diode: params with conditions, an optional series resistance, Newton
limiting, charge and noise. The same block serves every analysis.

```rust
module Diode(a: Electrical, k: Electrical) {
    is: f64 = 1e-14 where is > 0.0;
    n: f64 = 1.0 where (0.1..=10.0).contains(n);
    rs: Option<f64>;
    cj0: f64 = 0.0 where cj0 >= 0.0;
    signal ai: Electrical;
}
analog for Diode {
    match rs {
        Some(r) => { i(a, ai) +> v(a, ai) / r; }
        None    => { v(a, ai) +> 0.0; }          // no resistor: `ai` and `a` are one node
    }
    let nvt   = n * vt();
    let vcrit = nvt * (nvt / (2.0_f64.sqrt() * is)).ln();
    let vd    = limit(v(ai, k), |new, old| pnjlim(new, old, nvt, vcrit), vcrit);
    let id    = is * ((vd / nvt).exp() - 1.0);
    i(ai, k) +> id + ddt(cj0 * vd) + white_noise(2.0 * Q * id, "shot");
}
```

The library adds `idt`, `delay`, `slew`, `transition`, Laplace and sampled
filters, `table_model`, `ac_stim` and `ddx`. Nothing is specific to electricity:
a thermal port, or a nature that turns an equation into a row, is declared alike.

## Digital and values

A definition behaves like a gate and `after` gives it a delay
(`q = !(r | qn) after tpd;`). A carrier holds any value with a bit
representation, an enum with fields included:

```rust
#[derive(Repr, PartialEq)]
enum Tx { Idle, Start, Data { n: u3 }, Stop }

module UartTx(in clk: Bit, in baud_tick: Bit, in start: Bit, in byte: Bit<u8>,
              out txd: Bit = true, out busy: Bit) {
    signal st: Bit<Tx> = Tx::Idle;
    signal sh: Bit<u8> = 0;
    busy = st != Tx::Idle;
}
digital for UartTx {
    on clk.rise() {
        match st {
            Tx::Idle => if start { sh <- byte; st <- Tx::Start; },
            _ if !baud_tick => {}
            Tx::Start => { txd <- false; st <- Tx::Data { n: 0 }; }
            Tx::Data { n } => {
                txd <- (sh & 1) == 1;
                sh <- sh >> 1;
                st <- if n == 7 { Tx::Stop } else { Tx::Data { n: n + 1 } };
            }
            Tx::Stop => { txd <- true; st <- Tx::Idle; }
        }
    }
}
```

Models compute with ordinary typed values: integers wrap at their width and real
literals take SI suffixes.

```rust
const C_OUT: f64 = 2.2u;              // T G M k m u n p f a
const COUNTS: u9 = 400;               // nine bits, no more

fn dithered_width(duty: u12, frame: u3) -> u9 {
    let coarse = (duty >> 3) as u9;
    let fine = (duty & 7) as u3;
    if frame < fine { coarse + 1 } else { coarse }
}
```

**Values.** Every value is a value of its type, from the first instant: a carrier
starts at the initial value written or at the `Default` of its type (a number has
none, so it is written), and `analog` reads a carrier directly (`held`). What may be
absent is an `Option<T>`, and a net with four states is a `Logic<N>`.

## Structure

Modules take type and const parameters with conditions, `for` and `if` generate
structure when the design is built, and a `let` places an instance:

```rust
module Delay<T: Repr + Default, const N: usize>(in clk: Bit, in d: Bit<T>, out q: Bit<T>) where N >= 1 {
    signal r: [Bit<T>; N];
    q = r[N - 1];
}
digital<T, N> for Delay<T, N> {
    on clk.rise() { r[0] <- d; for k in 1..N { r[k] <- r[k - 1]; } }
}

module Tap(in clk: Bit, in d: Bit<u8>, out q: Bit<u8>) {
    let line = Delay::<u8, 4>(clk, d, q);
}
```

An instance may sit inside an expression, where its result is needed:
`let ph = idtmod(f0 + kvco * v(vctl), 0.0, 1.0, -0.5);` is a VCO's phase, and
`on cross(ph - 0.25) { clk <- false; }` turns it into a clock. A parent reads a
child's `pub` items by path, a module with its params bound is a value that can be
passed as a param, and an `extern` module reuses a Verilog-A model compiled to
OSDI or another simulator by cosimulation.

## Output and checks

`emit!` writes a typed record to a named stream, and checks are library functions
built on it. Printing never changes a result.

```rust
module Monitor(a: Electrical) {
    level: f64 = 0.5;
}
digital for Monitor {
    on cross(v(a) - level) {
        emit!("trip", v = v(a));
        require("crossed_upward", v(a) >= level);
    }
}
```

`require` must hold, `cover` records that a case was reached, `target` reports a
margin to maximize, and monitors (`Never`, `Setup`, `Hold`) are placed as instances.

## What you can run

The **host** is the `piperine` command or a program using the Rust library
`piperine-api`. It picks the top module, sets params, drives the top ports, asks
for an analysis and reads every node, carrier and stream by path.

Eight analyses: `op`, `dc` (a swept param), `ac`, `transient` (analog and digital
together), `tf`, `pz` (poles and zeros), `sens` and `pss` (periodic steady state).

```sh
piperine new rc                      # a project that runs
piperine check                       # parse, resolve, type-check, elaborate
piperine run step                    # a run declared in Piperine.toml
piperine run f.msl --analysis op     # a lone file; --param r1.r=2200 sets a param by path
piperine test                        # judge every run by its checks
```

Params may be set between solves without recompiling. Sweeps over many runs,
Monte Carlo and optimization are host loops by design: a model is the same file
whether it runs once or ten thousand times.

## Correctness

- **Checked before it runs.** Names, types, natures, directions and param
  conditions are checked when the design is built; every error has a code and a
  source place, and an unsupported form is refused by name (`P0001`).
- **Cross-checked against ngspice** on twin circuits under fixed tolerances,
  across all eight analyses.
- **A design held to its own specification.** [`examples/buck`](examples/buck) is a
  digitally controlled buck converter (200 MHz PWM, 10-bit ADC, fixed-point PID,
  soft start, protections; about 700 lines of MSL). It starts with its loop closed,
  regulates at 0.8 V within 3 mV, and its tests run its [`SPEC.md`](examples/buck/SPEC.md).

## A first run

```sh
piperine new rc        # an RC low-pass filter with a step-response bench
cd rc
piperine run step      # a 5 µs transient
```

The bench places a source, a resistor (`let r1 = Resistor(vin, vout) { r: 1k };`),
a capacitor and the `Monitor` above, whose record lands on the located crossing,
RC·ln 2. [`examples/rc`](examples/rc) is this project.

```console
run step: transient, top Step, backend rust
@6.931071e-7 Step.mon trip v=5.000000e-1

time         Step.vin    Step.vout
0.000000e0   1.000000e0  0.000000e0
1.000000e-6  1.000000e0  6.320928e-1
2.000000e-6  1.000000e0  8.646682e-1
```

## Status

The specification is complete and the front end parses and resolves all of it,
every snippet above included. Type checking, elaboration and the backends cover
a subset that grows feature by feature; [`GAPS.md`](GAPS.md) is the full list.

- **Runs today:** analog contributions, `ddt`, switch branches, `limit`,
  `ac_stim`, `transition`; reactions, commits, definitions, `after`, unknown
  values; every event above; mixed-signal modules; `emit!` and the checks;
  `f64`/`bool` params with `where`, hierarchy and packages; the eight analyses.
- **Specified, refused by name until implemented:** generic modules and
  generation, templates and `extern` modules; bundles, resolved natures and
  arrays of carriers; enums with fields, structs and closures as values; `idt`,
  `idtmod`, noise and the filters; Monte Carlo and optimization in the host.
- Of the snippets above, `SampleHold`, the passives, `Switch` and `Monitor` are
  in the first group.

## Documentation

- **The MSL Book** ([`docs/book/`](docs/book)): the user manual, language to solver.
- **The specification** ([`docs/spec/`](docs/spec/index.md)): the authoritative
  definition of MSL, with the grammar and three galleries of worked models.

## Building

```sh
cargo build --release      # builds the piperine command
cargo test                 # the test suite
```

Running a model needs a Rust toolchain on the machine: the compiled backend
builds each design with `rustc`. Crates: `piperine-lang` (front end),
`piperine-ir` and `piperine-plan` (the elaborated design), `piperine-transpile`
and `piperine-interp` (the two backends), `piperine-abi`,
`piperine-analog-solver`, `piperine-digital-solver` and `piperine-solver` (the
engines and analyses), `piperine-api` and `piperine-cli` (the host),
`piperine-project` (packages), `piperine-parity` (the ngspice harness).

## License

See [LICENSE](LICENSE).
