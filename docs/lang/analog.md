# Analog (Verilog-A) Modules

Piperine is a superset of Verilog-A. You can write analog device models directly in `.ppr` files alongside structural modules.

## Analog module syntax

An analog module has an `analog` block instead of (or in addition to) structural instances:

```verilog
module my_resistor(p, n);
    inout p, n;
    electrical p, n;
    parameter real r = 1e3;

    analog begin
        V(p, n) <+ r * I(p, n);
    end
endmodule
```

## Compilation

Analog modules in `.ppr` files are compiled via OpenVAF-Reloaded to OSDI shared libraries. This happens automatically when Piperine processes a file containing analog modules:

1. Parse the `.ppr` file
2. Detect modules with `analog` blocks (no `initial` block)
3. Compile to `.osdi` via OpenVAF
4. Load `.osdi` into ngspice with the `osdi` command before loading the netlist
5. Use the module like any other device

## Port declarations

```verilog
module va_device(port1, port2, port3);
    inout port1, port2;
    input port3;           // input-only port
    electrical port1, port2, port3;
```

## Parameter declarations

```verilog
parameter real saturation_current = 1e-14 from [0:inf];
parameter real emission_coefficient = 1.0 from [1:inf];
parameter integer nf = 1 from [1:100];
```

Range constraints (`from [lo:hi]`) are supported by Verilog-A and passed through to OpenVAF.

## Analog operators

```verilog
analog begin
    // Branch voltage and current
    V(p, n) <+ expr;        // contribute voltage across p-n
    I(p, n) <+ expr;        // contribute current from p to n

    // Node voltages
    real vp = V(p);          // voltage at p relative to ground
    real vpn = V(p, n);      // differential voltage

    // Derivatives
    ddt(x)                   // d/dt operator
    ddx(V(p,n), V(p,n))     // partial derivative (for Jacobian)

    // Time
    $abstime                 // simulation time
end
```

## Built-in functions

Standard Verilog-A math functions work in analog blocks:

```verilog
exp(x), log(x), log10(x)
sin(x), cos(x), tan(x)
sqrt(x), pow(x, y)
abs(x), min(x, y), max(x, y)
limexp(x)                    // exp with limiting (prevents overflow)
```

## Using analog modules

After compiling, analog modules are used exactly like ngspice components:

```verilog
// If my_bjt is defined as an analog module in the same file:
my_bjt #(.is(1e-14), .nf(1.0)) Q1(.c(collector), .b(base), .e(emitter));
```

The `paramset` mechanism works with analog modules too:

```verilog
paramset bc547 my_bjt;
    .is(6.734e-15), .bf(416.4), .nf(1.0);
endparamset
```

## Limitations

- Analog modules and `initial` blocks cannot coexist in the same module
- The `.ppr` file is compiled as a single unit — all VA modules in the file go to one `.osdi`
- OpenVAF-Reloaded supports a subset of the full Verilog-A standard; see `docs/openvaf/` for details

## OpenVAF integration

See [docs/openvaf/overview.md](../openvaf/overview.md) and [docs/openvaf/writing_models.md](../openvaf/writing_models.md) for the full Verilog-A / OSDI workflow.
