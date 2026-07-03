<div align="center">

# Piperine

<img src="assets/logo.svg" alt="Piperine Logo" width="180"/>

**A modern hardware-description language (HDL) and simulator for analog and mixed-signal circuits.**

</div>

> ⚠️ **Work in progress — not production ready.** APIs, syntax, and behavior
> change without notice. Use it to explore and contribute, not for anything you
> depend on.

It aims to unify continuous (Newton-Raphson) and discrete (event-driven) hardware into a single, cohesive model. Piperine supports both industry-standard **Verilog-A/AMS** (`.va`, `.vams`) and our new native language called **PHDL** (`.phdl`), compiling them into an intermediate representation that runs on a pure-Rust solver.

## Key Features

- **One Mixed-Signal Model:** Analog (`analog` block) and digital (`digital` block) behaviors share the same module constructs. The boundary between them is explicit and type-checked.
- **No-Magic Philosophy:** Type conversions and domain crossings are explicit. No implicit driver resolutions that lead to hidden bugs.
- **Scriptable Verification (`bench`):** A built-in effectful scripting layer runs tests, measurements, and parameter sweeps using an interactive Uniform API.
- **Parametric & Generics:** Build flexible, reusable components with type generics and compile-time evaluated constants.
- **OSDI Support:** Fully integrates with `.osdi` version 0.4 device models.

## What it looks like (PHDL)

Forget boilerplate-heavy connect-rules. In PHDL, analog and digital behaviors sit elegantly side-by-side. 

### 1. True Mixed-Signal: First-Order Delta-Sigma Modulator
Here, a single component crosses the analog/digital boundary twice. Notice how the `analog` block handles the continuous integration, while the `digital` block safely handles discrete clock edges:

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

### 2. Parametric Generics & Compile-Time Evaluation
Forget writing endless scripts just to generate hardware. In PHDL, structural and behavioral scaling is resolved natively at compile time via pure data generation using `for` loops and parameter bounds `[N]`:

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
        // Easily probe the child instance nodes to add parasitics!
        I(rseg[i].n, gnd) <+ cpar * ddt(V(rseg[i].n, gnd));
    }
}
```

### 3. Scriptable Verification: Parameter Sweeps
Testing isn't an afterthought. PHDL includes an effectful `bench` block with a built-in Uniform API that evolves the traditional `.measure` statements into pure, natively integrated code:

```phdl
bench AmpSweep {
    fn dc_gain_vs_load() {
        var curve : Vec<(Real, Real)> = [];
        
        // Native loops for parameter sweeps!
        for rl in [1e3, 1e4, 1e5, 1e6] {
            load.resistance = rl;
            var r = $op(); // Run Operating Point analysis
            curve.push((rl, r.v(out) / r.v(in_)));
        }
        
        $write("gain_vs_load.csv", curve);
    }
}
```

### 4. Typed Metadata (Attributes)
Say goodbye to using messy, unstructured `PRAGMA` comments to pass physical design intent to your tools. PHDL introduces typed, schema-validated attributes that cleanly attach layout, routing, and floorplanning intent directly to your netlist components:

```phdl
// The compiler validates `min_width` and `layer` against your project's `layout` schema plugin!
@layout(min_width = 2.0e-6, layer = "m3") 
@route(priority = high) 
wire clk : Electrical;
```

*(Note: Scriptable verification via `bench` blocks and the Uniform API is currently in design and will be added in future releases).*

## Usage

Use the `piperine` command line interface to work with your circuit designs.

### Verifying a design

You can parse, elaborate, and sanity-check your PHDL or Verilog-A files:

```sh
piperine check path/to/circuit.phdl
```

### Formatting

Keep your code clean with the built-in formatter:

```sh
piperine fmt path/to/circuit.vams
```

*(Note: The other CLI subcommands like `build`, `run`, `test`, and `clean` are currently under development.)*
