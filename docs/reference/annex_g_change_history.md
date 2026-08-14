# Annex G — Change History (informative)

Ported from `VAMS-LRM-2-4.md` (Accellera Verilog-AMS LRM 2.4, Annex G) as
reference material, per the `verilog-ams-support` feature's T57 (VAMS-26).
Informative, not normative — no behavior in Piperine implements or enforces
anything on this page; it documents the LRM's own evolution for readers
cross-checking a construct against an older Verilog-A/Verilog-AMS revision.

## G.1 Changes from previous LRM versions

This section highlights key differences between OVI Verilog-A v1.0 and OVI
Verilog-AMS v2.0 syntax/semantics. Per the LRM's own text: "The syntax and
semantics of this document supersede any syntax, semantics, or
interpretations of previous revisions."

| Feature | OVI Verilog-A v1.0 | OVI Verilog-AMS v2.0 | Change type |
|---|---|---|---|
| Analog time | `$realtime` | `$abstime` | new |
| Ceiling operator | N/A | `ceil(expr)` | new |
| Floor operator | N/A | `floor(expr)` | new |
| Circular integrator | N/A | `idtmod(expr)` | new |
| Expression looping | N/A | `genvar` | new |
| Distribution functions | `$dist_*()` (integer-based) | `$rdist_*()` (real-value equivalents) | new |
| Empty discipline | predefined as type `wire` | type not defined | default definition |
| Implicit nodes | `` `default_nodetype `` discipline default: `wire` | default type: empty discipline, no domain type | default definition |
| `initial_step` | default = `TRAN` | default = `ALL` | default definition |
| `final_step` | default = `TRAN` | default = `ALL` | default definition |
| Analog `ground` | no definition | now a declaration statement | definition |
| `$realtime` | timescale = 1 sec | timescale = `` `timescale `` def = 1n, see `$abstime` | definition |
| Array setting | `aa[0:1] = {2.1 = (1), 4.5 = (2)}` | `aa[0:1] = {2.1, 4.5}` | syntax |
| Discontinuity function | `discontinuity(x)` | `$discontinuity(x)` | syntax |
| Limiting exponential function | `$limexp(expression)` | `limexp(expression)` | syntax |
| Port branch access | `I(a,a)` | `I(<a>)` | syntax |

**Piperine cross-check**: `$abstime`, `idtmod`, `genvar`/`generate`, `ground`,
`$discontinuity`, `limexp`, and port-branch access (`I(<port>)`) all target
the v2.0 (current, LRM 2.4) forms above — Piperine was built against
`VAMS-LRM-2-4.md` from the start (C7), so no v1.0-syntax compatibility path
exists or is needed.

## G.2 Obsolete functionality

The following are not supported in the current (2.4) LRM; noted here only for
backward-compatibility awareness when reading older Verilog-A/Verilog-AMS
source.

### G.2.1 `forever`

The `forever` statement is no longer supported.

### G.2.2 `NULL`

The `NULL` statement is no longer supported as a standalone construct.
(Certain constructs — `case`, conditionals, the event statement — do still
allow a null/empty statement per their own syntax productions.)

### G.2.3 `generate` (the old form)

An older, distinct `generate` looping construct existed, unrolled at
elaboration time, syntax:

```
generate_statement ::= generate index_identifier ( start_expr , end_expr [ , incr_expr ] ) statement
```

It has been superseded by the `generate for`/`genvar` form the current LRM
(and Piperine's `piperine-vams` parser + `UnrollGenerate` elaboration pass,
T25/VAMS-04) implements. The old form's worked example (a continuously
running unclocked ADC using `generate i (bits-1, 0) begin ... end`) is
historical only — Piperine has no production for it and does not need one,
since the current `generate for` form covers the same use case.

### G.2.4 `` `default_function_type_analog ``

This compiler directive is no longer supported. It used to let user-defined
functions be treated as `analog function`s in Verilog-A even without the
`analog` keyword in their definition. Piperine's grammar has no production
for it (the `analog` keyword on a function declaration is always required).
