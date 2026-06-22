# Expressions

Piperine expressions appear in parameter defaults, instance parameter values, and `initial` block statements.

## Literals

```verilog
1.0          // real
1e3          // real (1000.0)
1.5e-9       // real (1.5 nanoseconds)
100e-12      // real (100 picofarads)
42           // integer
"NMOS_SVT"  // string
```

## Arithmetic operators

```verilog
a + b
a - b
a * b
a / b
a % b        // modulo
a ** b       // power
-a           // unary negation
```

## Increment and compound assignment

Statement-level shortcuts for updating a variable:

```verilog
i++;   ++i;       // i = i + 1
i--;   --i;       // i = i - 1

x += 2.5;         // x = x + 2.5
x -= 1.0;         // x = x - 1.0
x *= 3.0;         // x = x * 3.0
x /= 2.0;         // x = x / 2.0
n %= 8;           // n = n % 8
```

`++`/`--` and `+=`/`-=` are also valid in a `for` increment clause:

```verilog
for (i = 0; i < n; i++) ...
for (t = 0.0; t < tstop; t += dt) ...
```

## `inside` — set membership

Tests whether a value is in a set of scalars and/or ranges. `[lo:hi]` is an
inclusive range; `$` is an open bound. Returns 1 or 0.

```verilog
if (code inside {1, 5, 9})        ...   // any of these
if (v    inside {[0.0:1.8]})      ...   // within range
if (f    inside {[1e3:$]})        ...   // 1 kHz and up
if (n    inside {0, [10:20], 99}) ...   // mixed
```

## Array literals and indexing

```verilog
q = '{1.0, 2.0, 3.0};   // array literal ('{} is empty)
real a = q[0];          // indexed read
q[1] = 5.0;             // indexed write
```

See [Array](stdlib.md#array) for the full method set and handle semantics.

## Comparison operators (in procedural blocks)

```verilog
a == b
a != b
a < b
a <= b
a > b
a >= b
```

## Logical operators (in procedural blocks)

```verilog
a && b
a || b
!a
```

## Variable references

In `initial` blocks, variables declared with `real`, `integer`, or `string` can be read:

```verilog
initial begin
    real vout;
    vout = $voltage(out);
    $display("Vout = %f", vout);
end
```

## System function calls

System functions return simulation results and can be used in expressions:

```verilog
$voltage(net)          // voltage at net (real)
$voltage(net1, net2)   // differential voltage (real)
$current(vsource)      // current through voltage source (real)
```

## String expressions

Strings are used in parameter values for model names and element references:

```verilog
nmos #(.model("NMOS_SVT"), .w(1e-6)) M1(...);
```

String expressions do not support concatenation in parameter position.

## Parameter expressions

In instance parameter lists, expressions can reference module-level parameters or constants:

```verilog
module my_filter;
    parameter real R = 1e3;
    parameter real C = 1e-12;

    res #(.r(R)) R1(.p(in), .n(mid));
    cap #(.c(C)) C1(.p(mid), .n(gnd));
endmodule
```

## Behavioral expressions (`parameter expr`)

Piperine allows behavioral expressions to be passed as values for parameters declared as `parameter expr` (such as the value of a behavioral source or a nonlinear passive device).

The expression is a true Piperine expression, not a raw string. It is parsed, type-checked, and safely serialized to the underlying simulator.

### Procedural vs Analog evaluation (`$X()` vs `X()`)

Piperine enforces a strict semantic distinction between procedural host evaluation and continuous analog evaluation:

| Form | Meaning | Where | Who evaluates |
|------|---------|-------|---------------|
| `$X(...)` | **procedural, eval-now** — returns a value immediately | `initial`/`always`/functions | the **interpreter** |
| `X(...)` (bare) | **analog expression** — part of a continuous expression | behavioral params (`.v(...)`) | the **simulator**, every timestep |

```verilog
real f = $sin(1.0);                 // interpreter computes sin(1.0) now → 0.8414…
bsource_v #(.v( sin(6.28*1e3*time) + V(a)*V(b) )) B1(.p(out), .n(gnd));
//            └── bare sin / V(): lowered into the ngspice B-source, evaluated by
//                the simulator at each timestep. NOT computed by the interpreter.
```

- The `$` prefix marks an operation as "evaluate in the interpreter, right now, give me the number."
- A **bare** call in a behavioral-expression position is *never* evaluated by the interpreter; the whole expression AST is handed to the serializer and lowered to the simulator syntax. Bare `V()`, `I()`, `ddt()`, `idt()`, and the math names are **analog primitives** — they only have meaning in this context.

System tasks like `$voltage` or `$op` cannot appear inside a behavioral expression. Doing so will result in an elaboration error.

See [Analog (Verilog-A) Modules](analog.md) for more details on analog primitives.
