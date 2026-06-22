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

For B-sources, the expression string is passed verbatim to ngspice. Any valid ngspice expression is allowed:

```verilog
bsource_v #(.V("V(a,b)*2.0 + 0.5")) Bv1(.p(out), .n(gnd));
```

The expression uses ngspice node voltage `V(node)` and current `I(element)` syntax, not Piperine net names.
