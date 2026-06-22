# Writing Verilog-A Device Models

This guide covers writing Verilog-A models that compile successfully with OpenVAF-Reloaded.

## Module structure

```verilog
// my_diode.va (or inside a .ppr file)
`include "constants.vams"    // optional: standard constants (q, k, eps0, etc.)
`include "disciplines.vams"  // optional: electrical discipline

module my_diode(a, c);
    inout a, c;
    electrical a, c;

    // Parameters
    parameter real is  = 1e-14 from (0:inf];
    parameter real n   = 1.0   from [0.5:10];
    parameter real vj  = 0.7;

    // Internal nodes (optional)
    // electrical internal_node;

    // Analog behavior
    analog begin
        I(a, c) <+ is * (exp(V(a, c) / (n * $vt)) - 1.0);
    end
endmodule
```

## Discipline declarations

Piperine's bundled headers provide the `electrical` discipline. If using a standalone `.va` file, include the standard disciplines file or declare it manually:

```verilog
`include "disciplines.vams"
// or manually:
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline
```

## Parameters

```verilog
parameter real r    = 1e3;               // real, no range
parameter real r    = 1e3 from (0:inf];  // real, positive only
parameter real n    = 1.0 from [1:10];   // real, closed interval
parameter integer nf = 1 from [1:100];   // integer with range
parameter string model_type = "NFET";    // string (no range)
```

Range constraints are checked at simulation time. Use `(` for exclusive, `[` for inclusive bounds.

## Branch contributions

```verilog
analog begin
    // Voltage contribution (series branch)
    V(p, n) <+ R * I(p, n);       // Ohm's law

    // Current contribution (shunt/parallel branch)
    I(a, c) <+ is * (exp(V(a,c) / vt) - 1.0);

    // Temperature-voltage product
    real vt;
    vt = $vt;     // kT/q at current temperature
    // or: vt = `P_K * $temperature / `P_Q;
end
```

## Temperature dependence

```verilog
parameter real tnom = 27.0;    // nominal temperature (°C)

analog begin
    real t_abs, t_nom_abs, ratio;
    t_abs     = $temperature;               // K (current temp)
    t_nom_abs = tnom + `P_CELSIUS0;        // convert °C to K

    ratio = t_abs / t_nom_abs;
    // ... use ratio for temperature scaling
end
```

Standard constants (available after `` `include "constants.vams" ``):
- `` `P_K `` — Boltzmann's constant (1.3806503e-23 J/K)
- `` `P_Q `` — electron charge (1.6021918e-19 C)
- `` `P_CELSIUS0 `` — 273.15 (0°C in Kelvin)

## Derivatives and convergence

OpenVAF automatically computes Jacobian entries from your analog expressions. To help convergence for exponential characteristics, use `limexp`:

```verilog
I(a, c) <+ is * (limexp(V(a,c) / (n * $vt)) - 1.0);
```

`limexp(x)` behaves like `exp(x)` for small `x` but limits growth for large `x` to prevent overflow during Newton iterations.

## Time derivatives

```verilog
analog begin
    // Capacitor: I = C * dV/dt
    I(p, n) <+ c * ddt(V(p, n));

    // Inductor: V = L * dI/dt
    V(p, n) <+ l * ddt(I(p, n));
end
```

## Events

```verilog
analog begin
    @(initial_step) begin
        // Runs at first simulation time point
        saved_value = V(in);
    end

    @(final_step) begin
        // Runs at last simulation time point
    end
end
```

## Noise contributions

```verilog
analog begin
    // Thermal noise: 4kT/R
    I(p, n) <+ white_noise(4.0 * `P_K * $temperature / r, "thermal");

    // Flicker noise: KF * Id^AF / Cox / f
    I(p, n) <+ flicker_noise(kf * pow(abs(I(d,s)), af), 1.0, "flicker");
end
```

## Internal nodes

Internal nodes are used for distributed models or hidden nodes:

```verilog
module bjt_with_rb(c, b, e);
    inout c, b, e;
    electrical c, b, e;
    electrical b_int;   // internal base node

    parameter real rb = 100.0;

    analog begin
        V(b, b_int) <+ rb * I(b, b_int);   // base resistance
        // ... rest of BJT equations using b_int
    end
endmodule
```

## Multi-terminal devices

```verilog
module nmos_simple(d, g, s, b);
    inout d, g, s, b;
    electrical d, g, s, b;

    parameter real vth = 0.5;
    parameter real kp  = 50e-6;
    parameter real w   = 10e-6;
    parameter real l   = 1e-6;

    analog begin
        real vgs, vds, vbs, ids;
        vgs = V(g, s);
        vds = V(d, s);
        vbs = V(b, s);

        if (vgs - vth <= 0) begin
            ids = 0.0;
        end else if (vds < vgs - vth) begin
            // Linear region
            ids = kp * (w/l) * ((vgs-vth)*vds - vds*vds/2.0);
        end else begin
            // Saturation
            ids = 0.5 * kp * (w/l) * (vgs-vth)*(vgs-vth);
        end

        I(d, s) <+ ids;
    end
endmodule
```

## Common pitfalls

- **Division by zero**: Guard against `V(p,n)/0` — use `max(val, epsilon)` or conditional branches
- **Discontinuities**: Abrupt if/else transitions cause convergence issues — use smooth functions like `limexp`, `tanh`, or `min`/`max`
- **Missing Jacobian**: Every branch contribution needs a well-defined partial derivative — avoid `abs()` at zero without smoothing
- **Parameter ranges**: Always specify `from` ranges — helps the solver avoid nonphysical operating points
