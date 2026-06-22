# Piperine Language Overview

Piperine (`.ppr`) is a hardware description language for analog/mixed-signal circuit simulation. It has two layers:

1. **Structural layer** — `module` declarations that instantiate devices and wire nets
2. **Procedural layer** — `initial` blocks (SystemVerilog-style) that drive simulation

## File structure

A typical `.ppr` file looks like:

```verilog
`include "ngspice.ppr"

// Optionally define model presets
paramset nmos_lv nmos;
    .model("NMOS_LV"), .w(2e-6), .l(180e-9);
endparamset

// Main circuit module
module inverter;
    // Port declarations (optional for top-level)
    inout vdd, gnd, in, out;

    // Device instances
    nmos_lv #() Mn(.d(out), .g(in), .s(gnd), .b(gnd));
    pmos   #(.model("PMOS"), .w(4e-6), .l(180e-9)) Mp(.d(out), .g(in), .s(vdd), .b(vdd));
    vsource #(.dc(1.8)) Vdd(.p(vdd), .n(gnd));

    // Testbench
    initial begin
        $op();
        $display("Vout = %f V", $voltage(out));
    end
endmodule
```

## Execution model

When Piperine runs a `.ppr` file:

1. Parse the file (and any included files)
2. Find the top-level module (only one may have an `initial` block)
3. Elaborate the circuit: resolve all device instances → SPICE netlist
4. Load the netlist into ngspice
5. Execute the `initial` block line by line
6. System tasks like `$op()`, `$tran()` trigger actual simulations in ngspice

## Key concepts

### Nets

Nets are named wires connecting component ports. Every identifier used in a port connection is a net. The net named `gnd` automatically maps to SPICE node `0`.

### Instances

A device instance is:
```verilog
<module_name> #(<parameter_list>) <instance_name>(<port_connections>);
```

Parameters and connections both use `.name(value)` syntax.

### `extern module`

Declares a device implemented outside Piperine (in Rust, backed by ngspice). All ngspice components are declared this way in `ngspice.ppr`:

```verilog
extern module res(
    inout p, inout n;
    parameter real r,
    parameter real tc1 = 0.0
);
```

### `paramset`

A `paramset` binds default parameter values to an existing module, creating a named variant:

```verilog
paramset my_nfet nmos;
    .model("NMOS_65N"), .w(1e-6), .l(65e-9);
endparamset
```

### `initial` block

SystemVerilog-style procedural code. Runs once after the circuit is loaded:

```verilog
initial begin
    $tran(.tstep(1e-9), .tstop(100e-9));
    $display("peak = %f", $voltage(out));
end
```

## Language pages

- [Modules and instances](modules.md)
- [Types](types.md)
- [Expressions](expressions.md)
- [Statements](statements.md)
- [Analog (Verilog-A) modules](analog.md)
- [Paramset](paramset.md)
- [Includes](includes.md)
