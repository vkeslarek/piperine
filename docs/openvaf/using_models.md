# Using Compiled OSDI Models

## Automatic loading

When Piperine runs a `.ppr` file with analog modules, OSDI compilation and loading are automatic. No extra steps are needed:

```verilog
// my_circuit.ppr
module my_bjt(c, b, e);
    inout c, b, e;
    electrical c, b, e;
    parameter real is = 1e-14;
    // ... analog behavior ...
    analog begin
        I(c, e) <+ is * exp(V(b, e) / 0.026);
    end
endmodule

module testbench;
    my_bjt #(.is(1e-14)) Q1(.c(col), .b(base), .e(emit));
    vsource #(.dc(5.0)) Vcc(.p(col), .n(emit));
    vsource #(.dc(0.7)) Vbe(.p(base), .n(emit));

    initial begin
        $op();
        $display("Ic = %f mA", $current(Vcc) * 1e3);
    end
endmodule
```

Run with: `piperine my_circuit.ppr`

## What happens internally

1. Parser finds `my_bjt` has an `analog` block → classified as VA module
2. `compile_va("my_circuit.ppr", cache_dir)` → `~/.cache/piperine/osdi/my_circuit.osdi`
3. `pre_load(osdi_path)` → sends `osdi /path/to/my_circuit.osdi` to ngspice worker
4. `elaborate()` → produces netlist with `Q1 col base emit MY_BJT IS=1e-14`
5. `load_circuit()` → sends netlist to ngspice (OSDI model already registered)
6. Interpreter runs `initial` block

## Using pre-compiled models

If you have an existing `.osdi` file (compiled with OpenVAF outside Piperine), you can load it manually using the `piperine-openvaf` crate:

```rust
use piperine_openvaf::LibraryCompiler;

LibraryCompiler.pre_load(&osdi_path, simulator.as_mut())?;
```

Or, write a wrapper `extern module` in Piperine that matches the model's ports/parameters, register a Rust `HardwareDefinition` that emits the correct `.model` + instance line, and the OSDI loads separately.

## Cache management

The OSDI cache lives at `~/.cache/piperine/osdi/`. Files are keyed by source path. To force recompilation:

```sh
rm -rf ~/.cache/piperine/osdi/
```

Or just touch the source file to update its mtime.

## Using external SPICE PDK models

For process design kits (PDKs) that provide SPICE models (not Verilog-A), use `paramset` with the built-in semiconductor devices:

```verilog
`include "ngspice.ppr"

// PDK provides .model NMOS_28NM NMOS ( ... )
// Load it via a separate .spi file or .control block

paramset nfet_28nm nmos;
    .model("NMOS_28NM"),
    .w(1e-6),
    .l(28e-9);
endparamset

module my_amp;
    nfet_28nm #() M1(.d(out), .g(in), .s(gnd), .b(gnd));
    // ...
endmodule
```

## Mixing VA models and ngspice built-ins

VA models and ngspice built-in devices work together in the same netlist:

```verilog
// my_model.ppr
module custom_res(p, n);   // VA model
    inout p, n;
    electrical p, n;
    parameter real r = 1e3;
    analog V(p,n) <+ r * I(p,n);
endmodule

module test;
    custom_res #(.r(2e3)) R1(.p(vcc), .n(mid));  // VA model
    cap #(.c(1e-12)) C1(.p(mid), .n(gnd));        // ngspice built-in
    vsource #(.dc(1.0)) Vs(.p(vcc), .n(gnd));

    initial begin
        $op();
        $display("Vmid = %f", $voltage(mid));
    end
endmodule
```

## OSDI ABI version

Piperine targets the OSDI ABI used by OpenVAF-Reloaded and supported by the bundled ngspice worker. The ABI version is set by the `piperine-openvaf` crate and the `piperine-worker` build. If you update ngspice or OpenVAF separately, ensure the OSDI versions match.

## Debugging model loading

Set `RUST_LOG=piperine_openvaf=debug` to see compilation output:

```sh
RUST_LOG=piperine_openvaf=debug piperine my_circuit.ppr
```

This shows the OpenVAF compiler invocation, output path, and any warnings. ngspice worker errors appear on stderr.
