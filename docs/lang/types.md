# Types

Piperine has a small set of types used in parameter declarations and expressions.

## Scalar types

| Type | SPICE equivalent | Notes |
|------|-----------------|-------|
| `real` | floating-point | Voltages, resistances, times, etc. |
| `integer` | integer | Counts, flags, enumerated values |
| `string` | quoted string | Model names, element references |

## Parameter types in `extern module`

```verilog
extern module example(
    inout p, inout n;
    parameter real r,           // required real
    parameter integer noisy = 1, // optional integer with default
    parameter string model = ""  // optional string with default
);
```

Required parameters have no default. Optional parameters have `= <expr>`.

## Special parameter kinds

Beyond scalar types, extern parameters can use two special forms:

### `parameter expr`

Used for behavioral source expressions. The value is a raw expression string passed as-is to ngspice:

```verilog
parameter expr V   // in bsource_v: V = V(a,b)*gain + offset
```

### `parameter ref`

Used to reference another instance by name. The elaborator resolves the instance name to its SPICE element identifier:

```verilog
parameter ref l1   // resolves to "La" if instance l1 is inductor La
```

Note: the `mutual` inductor currently uses `parameter string` instead of `parameter ref` — the user passes the SPICE element name directly as a string.

## Net type

Nets are implicitly typed as analog signals. All nets are real-valued voltage nodes in SPICE. No explicit type declaration is needed:

```verilog
// nets vcc, vout, gnd are inferred from connections
res #(.r(1e3)) R1(.p(vcc), .n(vout));
```

## Expressions

See [expressions.md](expressions.md) for the full expression syntax.

## Type coercion

- Integer literals work where `real` is expected (e.g., `parameter real r = 1000`)
- String values passed to non-string parameters cause elaboration errors
- Engineering notation is supported: `1e3`, `1.5e-9`, `100e-12`
