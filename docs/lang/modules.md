# Modules and Instances

## Module declaration

```verilog
module <name>;
    // port declarations (optional)
    inout net1, net2;

    // device instances
    // ...

    // procedural block (at most one per file)
    initial begin
        // ...
    end
endmodule
```

## Extern module declaration

Declares a device that is implemented externally (in Rust, backed by ngspice or OSDI). The extern module declaration describes the interface:

```verilog
extern module <name>(
    <port_list>;
    <parameter_list>
);
```

- Port list: `inout p, inout n` (separated by commas, ended by `;`)
- Parameter list: `parameter <type> <name> [= <default>]` (comma-separated)
- No semicolon after the closing `)`? — The closing `)` ends the declaration; a `;` follows outside

Example:
```verilog
extern module cap(
    inout p, inout n;
    parameter real c,
    parameter real ic = 0.0,
    parameter real temp = 27.0
);
```

## Instance syntax

```verilog
<module_name> #(<parameters>) <instance_name>(<connections>);
```

Both parameters and connections use named `.key(value)` syntax:

```verilog
// Resistor with explicit resistance
res #(.r(1e3), .tc1(100e-6)) R1(.p(vcc), .n(vout));

// Capacitor with default temp
cap #(.c(10e-12)) C1(.p(vout), .n(gnd));
```

The `#(...)` parameter block is required even if empty (`#()`).

## Port directions

Ports are declared with `inout`. Piperine doesn't enforce directionality at the language level — ngspice determines signal flow.

```verilog
inout vdd, vss, inp, inn, out;
```

## Module hierarchy

Modules can instantiate other modules (structural hierarchy). Nets connect instances:

```verilog
module amp;
    inout in, out, vdd, gnd;

    res #(.r(10e3)) Rin(.p(in), .n(mid));
    cap #(.c(1e-12)) Cc(.p(mid), .n(out));
    nmos #(.model("NFET"), .w(10e-6), .l(250e-9)) M1(
        .d(out), .g(mid), .s(gnd), .b(gnd)
    );
    vsource #(.dc(3.3)) Vdd(.p(vdd), .n(gnd));
endmodule
```

## Ground net

The net named `gnd` is treated as SPICE node `0`. Using any other name for ground won't work unless explicitly connected to a node named `gnd`.

## Subcircuit instantiation

For pre-existing SPICE subcircuits, use the `subckt` extern module:

```verilog
subckt #(.ports("in out vdd gnd"), .subckt_name("OPAMP")) U1();
```

See [ngspice sources](../ngspice/sources.md) for details.
