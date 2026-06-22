# Statements

Statements appear inside `initial` blocks. They follow SystemVerilog procedural syntax.

## Variable declaration

```verilog
real voltage_out;
integer count;
string model_name;
```

Variables must be declared before use. They can be declared anywhere in a block.

## Assignment

```verilog
voltage_out = $voltage(out);
count = count + 1;
model_name = "NMOS_SVT";
```

## `if` / `else`

```verilog
if (voltage_out > 0.9) begin
    $display("logic high");
end else begin
    $display("logic low");
end
```

Single-statement bodies don't need `begin`/`end`:
```verilog
if (voltage_out > threshold)
    $display("above threshold");
```

## `for` loop

```verilog
integer i;
for (i = 0; i < 10; i++) begin
    $display("iteration %d", i);
end
```

The increment clause accepts `i++`, `++i`, `i--`, `--i`, and compound forms like
`i += 2` (see [expressions](expressions.md#increment-and-compound-assignment)).

## `while` loop

```verilog
while (count < 100) begin
    count += 1;
end
```

## `foreach` loop

Iterate an index variable over the elements of an array:

```verilog
real total = 0.0;
foreach (samples[i]) total += samples[i];
```

`break` and `continue` work inside it. See [Array](stdlib.md#array).

## `repeat` loop

Run a body a fixed number of times:

```verilog
repeat (16) begin
    $tran(1e-9, 1e-6);
end
```

## `forever` loop

Loop until `break`, `return`, or `$fatal`:

```verilog
forever begin
    count++;
    if (count >= max_iters) break;
end
```

## `break` / `continue` / `return`

- `break` exits the innermost loop.
- `continue` skips to the next iteration (the increment clause in a `for`).
- `return` (optionally `return expr;`) exits the enclosing block/function.

```verilog
for (i = 0; i < n; i++) begin
    if (skip[i]) continue;
    if (done)    break;
end
```

## User-defined functions

Declare functions at module level; call them from `initial`/`always` blocks or
from other functions (recursion is allowed). Arguments are passed by value.

```verilog
module tb;
    function real db20(input real ratio);
        return 20.0 * $log10(ratio);
    endfunction

    // Verilog-A style also works: assign to a variable named after the function.
    function integer fact(input integer n);
        if (n <= 1) fact = 1;
        else        fact = n * fact(n - 1);
    endfunction

    initial begin
        real g = db20(100.0);   // 40 dB
        integer f = fact(5);    // 120
    end
endmodule
```

A function returns either the value of an explicit `return expr;` or, following
the Verilog-A convention, the final value of a variable named after the function.

## System task calls

System tasks drive simulation and display output:

```verilog
$op();                              // operating point analysis
$tran(.tstep(1e-9), .tstop(1e-6)); // transient analysis
$display("Vout = %f", $voltage(out)); // print to stdout
$fatal("error: %s", msg);           // abort with error
```

See [system tasks](#system-tasks) below.

## Blocks: `begin`/`end` or `{ }`

Group multiple statements. Two interchangeable syntaxes — use whichever reads
better; a `begin` is closed by `end`, a `{` by `}` (no mixing a `begin` with `}`):

```verilog
initial begin
    $op();
    real v;
    v = $voltage(out);
    $display("V = %f", v);
end
```

```verilog
initial {
    $op();
    real v;
    v = $voltage(out);
    $display("V = %f", v);
}
```

The brace form nests anywhere a block is allowed (loop bodies, `if` branches).
Only the `begin` form takes a `: label`.

```verilog
for (i = 0; i < n; i++) {
    if (v[i] > vmax) { vmax = v[i]; }
}
```

## System tasks

### Simulation control

| Task | Description |
|------|-------------|
| `$op()` | DC operating point analysis |
| `$tran(.tstep(dt), .tstop(T))` | Transient analysis |

### Probing

| Function | Returns | Description |
|----------|---------|-------------|
| `$voltage(net)` | `real` | Voltage at net (relative to node 0) |
| `$voltage(net1, net2)` | `real` | Differential voltage |
| `$current(vsource_name)` | `real` | Current through named voltage source |

### Output

| Task | Description |
|------|-------------|
| `$display(fmt, ...)` | Print formatted string with newline |
| `$write(fmt, ...)` | Print without trailing newline |
| `$fatal(fmt, ...)` | Print error and abort |
| `$error(fmt, ...)` | Print error (non-fatal) |
| `$warning(fmt, ...)` | Print warning |

Format specifiers: `%f` (real), `%d` (integer), `%s` (string), `%e` (scientific), `%g` (shortest).

## Format strings

```verilog
$display("V(%s) = %.4f V at t = %e s", net_name, voltage, time);
```

Standard C-style printf formatting is supported.
