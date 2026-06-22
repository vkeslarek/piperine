# Paramset

A `paramset` binds a set of default parameter values to a base module, creating a named module variant. It's the primary way to create named technology-specific device presets.

## Syntax

```verilog
paramset <name> <base_module>;
    .<param1>(<value1>),
    .<param2>(<value2>);
endparamset
```

The parameter list ends with a semicolon. Multiple parameters are comma-separated.

## Basic example

```verilog
paramset nmos_svt nmos;
    .model("NMOS_SVT"),
    .w(1e-6),
    .l(180e-9);
endparamset
```

After this declaration, `nmos_svt` is a module that acts like `nmos` with those defaults pre-filled.

## Usage

Use a paramset exactly like a regular module:

```verilog
// Uses nmos_svt defaults; w and l come from paramset
nmos_svt #() M1(.d(drain), .g(gate), .s(src), .b(src));

// Override w at instance
nmos_svt #(.w(2e-6)) M2(.d(drain), .g(gate), .s(src), .b(src));
```

Instance-level parameters override paramset defaults. Parameters not in the paramset use the base module's defaults.

## Elaboration

When the elaborator encounters an instance of a paramset:

1. Looks up the paramset definition → gets base module name + preset params
2. Merges: instance params → paramset params → base module defaults
3. If base module has `spice_model_type()` (e.g., `"NMOS"`), emits a `.model` line:
   ```
   .model NMOS_SVT NMOS <model_params>
   ```
4. Emits the instance line referencing the model

## Diode example

```verilog
paramset 1n4148 d;
    .model("1N4148"),
    .area(1.0);
endparamset
```

Elaborates to:
```spice
.model 1N4148 D
D<name> <anode> <cathode> 1N4148
```

## MOSFET example

```verilog
paramset pmos_hv pmos;
    .model("PMOS_HV"),
    .w(4e-6),
    .l(500e-9);
endparamset
```

Elaborates to:
```spice
.model PMOS_HV PMOS
M<name> d g s b PMOS_HV W=4e-6 L=5e-7
```

## Nesting paramsets

A paramset can reference another paramset as its base (if the base is registered). This allows multi-level presets.

## Multiple instances of same paramset

All instances of the same paramset share a single `.model` line in the netlist. The elaborator emits the `.model` line once.

## Paramsets vs `extern module`

| | `extern module` | `paramset` |
|---|---|---|
| Defined by | Rust `HardwareDefinition` | `.ppr` source |
| Purpose | Declare device interface | Create named device presets |
| SPICE `.model` | Can emit via `spice_model_type()` | Emits via base module's `spice_model_type()` |
| Instance syntax | Same as module | Same as module |
