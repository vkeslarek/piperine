# Piperine Language Specification

Formal, single-document reference for the Piperine language **as implemented today**.
Ground truth: derived from the parser (`piperine-parser`), interpreter
(`piperine-interpreter`), and elaborator (`piperine-circuit`). Where a construct
parses but is not (fully) evaluated, it is marked **[parse-only]**. Where a feature
is planned but absent, it is in §15.

The other files in `docs/lang/` are tutorials; this is the normative reference.

Notation: grammar is EBNF-ish — `X*` zero-or-more, `X?` optional, `X | Y` choice,
`'kw'` literal keyword/token. Terminals from the lexer (§2) are in `code`.

---

## 1. Overview

Piperine is a Verilog-A/AMS **superset** for device descriptions plus a
SystemVerilog-style **procedural testbench** layer. A source file (`.ppr`) is a
sequence of top-level items (§4). Exactly one module with an `initial` block is the
**testbench**; it is elaborated to a SPICE netlist and its `initial`/`always` code
is run by a tree-walking interpreter against a simulator backend (ngspice).

Two evaluation worlds coexist and are distinguished syntactically (§10):
- **Procedural** — `initial`/`always`/`function` bodies, run by the interpreter.
  System calls use the `$` prefix (`$tran`, `$sin`) and evaluate immediately.
- **Analog** — expressions assigned to `parameter expr` device values, lowered to a
  continuous simulator expression (B-source). Bare calls (`V(a)`, `sin(x)`) live here.

---

## 2. Lexical structure

### 2.1 Whitespace & comments
- Whitespace and newlines separate tokens; newlines are not statement terminators
  (`;` is). Line-continuation `\` before a newline is consumed.
- Comments: `// … <eol>` and `/* … */`.

### 2.2 Identifiers
- `[A-Za-z_][A-Za-z0-9_$]*`. Case-sensitive.
- A leading `$` starts a **system function/task** name (`$name`).
- A leading `` ` `` starts a **preprocessor directive** (`` `define ``, `` `include ``,
  `` `ifdef ``/`` `ifndef ``/`` `else ``/`` `endif ``).

### 2.3 Keywords (reserved)
Contextual keyword set recognized by the grammar:
```
module macromodule endmodule  extern  class  endclass
discipline enddiscipline  nature endnature  paramset endparamset
typedef enum struct  function endfunction  analog  branch
parameter localparam genvar  initial always  aliasparam
begin end  if else  while for foreach repeat forever
case casex casez endcase default  break continue return
assert assert_run assert_warn
input output inout terminal
integer real string  inf
initial_step final_step step above cross
```
Net-type words (consumed where a net declaration is allowed):
`reg wreal wire uwire wand wor ground tri supply0 supply1`.

### 2.4 Numbers
| Literal | Form | Notes |
|---------|------|-------|
| Integer | `[0-9]+` | stored as 64-bit signed |
| Real (std) | `1.5`, `1e-9`, `2.5E3` | f64 |
| Real (SI) | digits + SI suffix | f64, suffix scales |
| `inf` | keyword | `+∞` |

**SI suffixes** (Verilog-A convention — *case-sensitive*, `M`=mega, `m`=milli):
`T`=1e12 `G`=1e9 `M`=1e6 `K`/`k`=1e3 `m`=1e-3 `u`=1e-6 `n`=1e-9 `p`=1e-12 `f`=1e-15
`a`=1e-18. Time suffixes: `ns us ms ps fs` (= 1e-9 … 1e-15). E.g. `10n`, `2.2k`, `5ns`.

### 2.5 Strings
`"…"` — stored with surrounding quotes in the token, stripped to the inner value at
evaluation. No escape processing beyond the raw bytes.

### 2.6 Operators & punctuation (tokens)
```
( ) [ ] { }  '{ (array-start)  (* *) (attribute)
, ; : . @ ? #
= <+ == != ! ~ < > <= >= << >>
+ - * / % **  & && | || ^ ^~ ~^
++ -- += -= *= /= %=
```

---

## 3. Source file

```
SourceFile = Directive* Item*
Item = DisciplineDecl | NatureDecl | ModuleDecl
     | 'extern' ('module' ExternModule | 'class' ExternClass)
     | 'typedef' ('enum' TypedefEnum | 'struct' TypedefStruct)
     | Paramset
```

`` `include "file" `` textually includes another `.ppr` (resolved against the include
dirs, e.g. the bundled `ngspice.ppr`). `discipline`/`nature` are accepted
(Verilog-AMS) but carry no runtime semantics in the testbench path.

---

## 4. Top-level items

### 4.1 `extern module` — device declarations
```
ExternModule = 'extern' 'module' name '(' PortList? ';' ExternParam (',' ExternParam)* ')' ';'
ExternParam  = 'parameter' ('real'|'integer'|'string') name ('=' Expr)?   -- Typed
             | 'parameter' 'expr' name                                     -- Expr (AST passthrough)
             | 'parameter' 'ref'  name                                     -- Ref (sibling instance name)
```
Declares a device the elaborator can instantiate (every ngspice primitive lives in
`ngspice.ppr`). Parameter kinds:
- **Typed** — value coerced to real/integer/string; `= Expr` gives a default
  (no default ⇒ mandatory, except see §11.4).
- **`expr`** — the argument is captured as an **unevaluated AST** (`ParameterValue::Ast`)
  for behavioral lowering (§10).
- **`ref`** — the argument must be a bare sibling instance name; resolved to that
  instance's SPICE element name (used by `mutual`'s coupled inductors).

### 4.2 `paramset` — named device preset (emits a `.model` card)
```
Paramset = 'paramset' name base_module ';' ( '.' name '=' Expr ';' )* 'endparamset'
```
Top-level (outside any module). Binds preset parameters to a base module, producing a
new module name usable like any device. Entries are `.<name> = <value>;` assignments.
At elaboration each paramset emits a `.model <model> <TYPE> (…)` card (TYPE from the
base device's model class) and instances reference it. Example:
```verilog
paramset nmos_svt nmos;
    .model = "NMOS_SVT";
    .w = 1e-6;
endparamset
```

### 4.3 `typedef enum` / `typedef struct`
```
TypedefEnum   = 'typedef' 'enum' BaseType? '{' name ('=' Expr)? (',' …)* '}' name ';'
TypedefStruct = 'typedef' 'struct' '{' (Type name ';')* '}' name ';'
```
Enums register named integer variants; structs register a field list. See §5.4.

### 4.4 `module` — structural + behavioral
```
ModuleDecl = 'module' name PortList? ';' ModuleItem* 'endmodule'
```

---

## 5. Modules, types & values

### 5.1 Module items
```
ModuleItem = PortDecl ';'              -- input/output/inout/terminal ports
           | NetDecl ';'               -- wire/electrical/… (see 5.2)
           | ParamDecl ';'             -- parameter / localparam
           | Function                  -- user function (§9)
           | Instance ';'              -- device/sub-module instantiation
           | InitialBlock              -- 'initial' Stmt
           | AlwaysBlock               -- 'always' '@' '(' Event ')' Stmt
           | 'analog' …                -- Verilog-A analog block (device side)
           | 'branch' …                -- VA branch decl
```

### 5.2 Nets are implicit
Any identifier used as a port connection is a **net** (a SPICE node). Net
declarations (`wire n;`, `electrical a,b;`) are accepted and matter for
*sub-module-internal* nets (they get hierarchical name-mangling, §11.3), but
**top-level nets need no declaration**. The net named `gnd` maps to SPICE node `0`.
(There is no strict mode yet; see §15.)

### 5.3 Scalar types
| Type | Runtime | Zero value |
|------|---------|-----------|
| `integer` | 64-bit signed | `0` |
| `real` | f64 | `0.0` |
| `string` | UTF-8 string | `""` |
| *custom name* `T x;` | parses; `T` is a type name | `void` |

Only `integer`, `real`, `string` have first-class runtime semantics. Other type
words (`time`, `realtime`, `logic`, `bit`, a `typedef` name, …) parse as a **custom
type** and zero-initialize to `void` unless assigned. Variables are dynamically typed
at runtime regardless of declared type — the declaration sets the initial value;
assignment can change the held kind.

### 5.4 Runtime value model
The interpreter's `Value` is exactly:
```
Real(f64) | Integer(i64) | String | Void
| RealVec(f64[])                       -- a bare numeric vector
| Complex(re, im)                      -- a complex scalar
| Enum{type_id, variant}               -- a typedef-enum value
| Struct{type_id, fields}              -- a typedef-struct value
| ExternObject(dyn ExternClass)        -- handle objects (§12)
```
Truthiness: `0`/`0.0`/`""`/`void` are false; everything else (incl. objects/vectors)
is true. Mixed real/integer arithmetic promotes to real.

### 5.5 Arrays / queues
Created by an **array literal** `'{…}` or `{…}` (§6.6). Backed by `ArrayObj`, an
ExternObject with **reference (handle) semantics**: `b = a;` shares storage. Indexed
with `a[i]` (read) and `a[i] = v` (write); iterated with `foreach` (§7). Methods §12.4.

---

## 6. Expressions

```
Expr = Ternary
Ternary = BinExpr ('?' Expr ':' Expr)?            -- only at statement/top level
BinExpr = Unary (BinOp Unary)*  |  Unary 'inside' Set
Unary   = ('-' | '!' | '~' | '+')* Postfix
Postfix = Atom ('[' Expr ']' | '[' Expr ':' Expr ']')*
Atom    = Literal | Path | '(' Expr ')' | ArrayLit | Call | PortFlow
```

### 6.1 Operator precedence (loosest → tightest)
| Rank | Operators | Assoc |
|------|-----------|-------|
| 0 | `?:` (ternary) | right; only when not nested in a higher-bp context |
| 1 | `\|\|` | left |
| 2 | `&&` | left |
| 3 | `\|` | left |
| 4 | `^` | left |
| 5 | `^~` `~^` (xnor) | left |
| 6 | `&` | left |
| 7 | `==` `!=` · `inside` | left |
| 8 | `<` `<=` `>` `>=` | left |
| 9 | `<<` `>>` | left |
| 10 | `+` `-` | left |
| 11 | `*` `/` | left |
| 12 | `%` | left |
| 13 | `**` | right |
| — | prefix `-` `!` `~` `+` | binds tighter than all binary |
| — | postfix `[]` `[:]` | tightest |

Relational/equality/logical operators yield `integer` `1`/`0`. `**` on integers uses
integer power; any real operand promotes to real.

### 6.2 `inside` — set membership
```
Set = '{' SetElem (',' SetElem)* '}'
SetElem = Expr | '[' Bound ':' Bound ']'        Bound = Expr | '$'
```
`x inside {a, [lo:hi], [lo:$]}` desugars (at parse time) to an OR-chain of `==` and
range tests (`x>=lo && x<=hi`); `$` is an open bound. Result is `1`/`0`.

### 6.3 Index & part-select
- `a[i]` — index. On an ExternObject it calls `get(i)` (array element; or a DataFrame
  column when `i` is a string — see §15). On any other value: type error.
- `a[msb:lsb]` — **[parse-only]**: the interpreter rejects part-selects at runtime.

### 6.4 Function / system calls
```
Call = '$' name ArgList?            -- system task/function (procedural, eval-now)
     | Path     ArgList?            -- user function | object method | analog primitive
ArgList = '(' (Arg (',' Arg)*)? ')'
Arg = Expr | name '=' Expr          -- positional | named
```
- `$name(...)` — looked up in the system-task registry (§11); evaluated now.
- `name(...)` with a dotted path `obj.method(...)` — method on an ExternObject (§12)
  **or** a device operating-point read when `obj` is a device handle (§11.5).
- bare `name(...)` matching a user `function` — called (§9).
- bare `name(...)` in an **analog** (`parameter expr`) position — an analog primitive
  (`V`, `I`, `ddt`, `idt`, math) lowered to the simulator (§10); not a procedural call.

Named arguments (`name = val`) are supported after positional ones; tasks read them
positionally-or-by-name.

### 6.5 Paths & port-flow
- `Path` — `a` or `a.b.c` (dotted). A bare path is a variable, enum variant,
  physical constant (§11.6), or — when dotted onto a device/object — a property/method.
- `< net >` — **port-flow** syntax; **[parse-only]** in procedural code (errors).

### 6.6 Array literal
`'{e0, e1, …}` or `{e0, e1, …}` builds an `ArrayObj` (§5.5). `'{}` is the empty array.
(At statement position `{ … }` is a block, not an array — §7.)

---

## 7. Statements

```
Stmt = ';'                                            -- empty
     | Block | If | While | For | Foreach | Repeat | Forever | Case
     | 'break' ';' | 'continue' ';' | 'return' Expr? ';'
     | Assert | Event | AssignOrExpr
Block = ('begin' (':' name)? | '{') BlockItem* ('end' | '}')
BlockItem = VarDecl | ParamDecl | Stmt
If      = 'if' '(' Expr ')' Stmt ('else' Stmt)?
While   = 'while' '(' Expr ')' Stmt
For     = 'for' '(' ForAssign ';' Expr ';' ForAssign ')' Stmt
Foreach = 'foreach' '(' array '[' index ']' ')' Stmt
Repeat  = 'repeat' '(' Expr ')' Stmt
Forever = 'forever' Stmt
Case    = ('case'|'casex'|'casez') '(' Expr ')' CaseItem* 'endcase'
CaseItem= (Expr (',' Expr)* | 'default') ':' Stmt
Assert  = ('assert'|'assert_run'|'assert_warn') '(' Expr ')' ('else' Expr)? ';'
Event   = '@' '(' Expr ')' Stmt
```

- **Blocks** use `begin`/`end` **or** `{`/`}`, interchangeably; a `begin` block may
  carry a `: label`. They do not introduce a new scope (§8).
- **`for`** init/incr accept `=`, `<+`, compound assigns, and `++`/`--` (§7.1).
- **`repeat(n)`** runs the body `n` times; **`forever`** loops until `break`/`return`.
- **Loop control:** `break` exits the innermost loop; `continue` skips to the next
  iteration (the increment in a `for`); `return [expr]` exits the enclosing
  function/block. Implemented via a `Flow{Normal,Break,Continue,Return(v)}` signal
  propagated out of blocks and loops (not exceptions).
- **`case`/`casex`/`casez`** — currently all compare by structural equality (no
  X/Z wildcard handling); first matching item wins, else `default`.
- **Assertions:** `assert` failure ⇒ fatal halt; `assert_run` ⇒ run-failure
  (stops the current handler, recorded as a run error); `assert_warn` ⇒ prints a
  warning and continues. The `else` expression is the message.
- **`@(Event)`** — event-controlled statement; meaningful for `always` handlers
  (§8.3). Inside `initial` it is collected, not executed.

### 7.1 Assignment, compound, increment
```
AssignOrExpr = Lvalue AssignOp Expr ';'  |  ('++'|'--') Lvalue ';'  |  Lvalue ('++'|'--') ';'  |  Expr ';'
AssignOp = '=' | '<+' | '+=' | '-=' | '*=' | '/=' | '%='
```
- `=` and `<+` both assign (the interpreter treats `<+` as a plain assignment in the
  testbench path). Lvalue is a variable name or `arr[i]`.
- `+= -= *= /= %=` combine with the current value.
- `++`/`--` (prefix or postfix) desugar to `+= 1` / `-= 1`.

---

## 8. Procedural semantics

### 8.1 Scope
A single flat variable map per execution context (`initial` body, or one function
call). Blocks do **not** create nested scopes; a variable declared in a nested block
is visible for the rest of the context. `set(name, value)` creates or overwrites.

### 8.2 Variable & param declarations
`VarDecl` (`real x = e;`) initializes from `e` or the type zero value (§5.3).
`ParamDecl`/`localparam` initialize from their default expression. Both just seed
scope variables at runtime.

### 8.3 `always` handlers (analog events)
The elaborator collects `always @(event) Stmt` into handler sets keyed by event:
`initial_step`, `step`, `final_step`, `above(expr)`, `cross(expr, dir)`. During an
analysis driven by `Interpreter::run_analysis`, the simulator streams events and the
interpreter runs the matching handler body; the body's `Flow`/error maps to a
backend action (continue / run-error / halt). Plain `$tran(...)` etc. that have no
handlers use the simpler non-streaming path.

---

## 9. Functions

```
Function = 'function' Type? name ('(' FnArg (',' FnArg)* ')')? ';' FnItem* 'endfunction'
FnArg  = Direction? Type? name           -- SV-style parenthesized args
FnItem = FunctionArg('input …;')         -- Verilog-A-style arg decls
       | VarDecl | ParamDecl | Stmt
```
- Declared at module level; callable from procedural code and other functions
  (recursion supported).
- Arguments are **by value**, bound positionally. SV-style `(input real a, …)` and
  Verilog-A-style `input real a;` body declarations both work.
- A call runs the body in a fresh flat scope (args + locals + params). The return
  value is the value of an explicit `return expr;`, else — Verilog-A convention — the
  final value of a variable named after the function.

---

## 10. Behavioral / analog expressions

A device parameter declared `parameter expr` (§4.1) captures its argument as an
**unevaluated AST**, lowered to ngspice B-source syntax at elaboration.

### 10.1 `$X()` vs `X()` (the procedural/analog split)
| Form | Meaning | Evaluator |
|------|---------|-----------|
| `$X(...)` | procedural, eval-now, returns a value | interpreter |
| bare `X(...)` (in `parameter expr`) | continuous analog expression | simulator, per timestep |

A `$`-system task inside an analog expression is a **hard error**
("system tasks cannot appear in a behavioral expression"). A bare analog primitive
used in procedural code errors ("call to unknown function").

### 10.2 Analog vocabulary (serialized to ngspice)
- `V(n)`, `V(n1,n2)` → `v(n)`, `v(n1,n2)`; `I(branch)` → `i(branch)`.
- `ddt(x)`, `idt(x[,ic])` — derivative / integral.
- math: `abs sqrt exp ln log log10 sin cos tan asin acos atan sinh cosh tanh
  atan2 pow floor ceil`.
- operators `+ - * / % **`, relational, `&&`/`||`, ternary `?:`.
- `$time`, `$temper` → simulator vars `time`, `temper`.
- bare identifiers pass through (treated as ngspice `.param` names / circuit params).
- a string literal is an **escape hatch**: its content is emitted verbatim
  (`.v("v(a)*2")`).
- A user `function` referenced in an analog expression is **inlined** (its
  `return EXPR` body, args substituted) — equivalent to ngspice `.func`.

### 10.3 Emission
`bsource_v`/`bsource_i` emit `B<name> p n V=<expr>` / `I=<expr>` (bare, no braces, per
ngspice). Expression-valued passives (R/C/L) and behavioral E/G emit `KEY={<expr>}`.

---

## 11. Elaboration semantics

### 11.1 Instances → netlist
```
Instance = ModuleType ('#' '(' (name '(' Expr ')' | Expr ',' …)* ')')? name Range? '(' Connections ')' ';'
Connections = '.' port '(' net ')' (',' …)*    -- named
            | net (',' net)*                    -- positional
```
The elaborator resolves connections to flat SPICE nodes and asks the device to emit
its SPICE line(s). `#(.param(value))` overrides parameters; the same `#(...)` form
applies parameter values to a `paramset`/sub-module.

### 11.2 Ground & nets
`gnd` → `0`. Other net names pass through at top level. A device's SPICE element name
is `spice_name(prefix, instance_name)` — the prefix letter is prepended unless the
name already starts with it (`R1`→`R1`, `load`→`Rload`).

### 11.3 Hierarchical sub-modules
A Piperine module instantiated structurally is **flattened inline**: its internal
nets are renamed `{instance}_{net}` so ngspice's flat namespace stays collision-free;
its ports bind to the parent's nets.

### 11.4 Parameter resolution
Named overrides set values; missing mandatory params (no default) error — **except**
`ref` and `expr` params, which are skipped in the mandatory check (the device decides:
a required behavioral value errors at emission, an optional one falls back to a
numeric form).

### 11.5 Device operating-point access — `inst.param`
Every elaborated instance is bound in the interpreter scope as a **device handle**
under its *piperine* name. After an analysis, `M1.gm` / `M1.gm()` reads the simulator
vector `@<spice_name>[<param>]` (the SPICE name is the first token of the device's
emitted line — ground truth). The elaborator statically scans the testbench AST for
such accesses and emits the required `.save @<spice>[<param>]` cards. `@dev[param]`
never appears in source.

### 11.6 Physical constants
Predefined read-only identifiers (resolve to `real`): `M_PI`, `M_TWO_PI`, `M_E`,
`BOLTZMANN`/`P_K` (1.380649e-23), `ECHARGE`/`P_Q` (1.602176634e-19), `P_CELSIUS0`
(273.15), `P_EPS0`, `P_U0`, `P_H`, `P_C`. A user variable of the same name shadows
the constant.

---

## 12. Standard objects (ExternObject handles)

Method call syntax `obj.method(args)`. Type names below are the runtime `type_name()`.

### 12.1 Analysis result (`OpResult`/`TranResult`/`AcResult`/…)
Returned by every analysis task (§13.4). Methods:
`signal(name)` → Signal · `scale()` → Signal (the index axis) · `ok()` → `1` if no
run errors · `dataset()` → result-set id string.

### 12.2 `Signal` — a named vector
`values()` → real[] · `len()` → integer · `max()` `min()` `mean()` `rms()`
`peak_to_peak()` · `integral()` (trapezoidal over the scale) · `bandwidth_3db()`
`phase_margin()` (AC) · `at(x)` (interpolate at scale value `x`).

### 12.3 `Complex`
`real()` `imag()` `magnitude()` `phase()` (deg) `phase_rad()` `db20()` `db10()`
`conjugate()`.

### 12.4 `Array` (queue) — handle semantics
`size()`/`len()` · `push_back(v)`/`push(v)` · `push_front(v)` · `pop_back()`/`pop_front()`
· `get(i)`/`set(i,v)` · `insert(i,v)` · `delete(i)`/`delete()` · `clear()` ·
`first()`/`last()` · `reverse()` · `sum()`/`product()`/`mean()` · `min()`/`max()` ·
`values()`.

### 12.5 `Device` — see §11.5 (resolved by the interpreter, not via `call_method`).

---

## 13. System task & function library

Registered automatically (`SystemTaskRegistry::default()` + the ngspice plugin). All
are invoked with `$` prefix.

### 13.1 I/O & severity (stdlib)
`$display(fmt, …)` `$write(fmt, …)` (no newline) · `$warning` `$error` `$fatal`
(halt) `$run_error` (run-failure) · `$sformatf(fmt, …)` → string.
Format specifiers: `%d %f %e %g %s %b %o %h %%` with width/precision (`%8.3f`, `%0d`).

### 13.2 Math (stdlib)
Scalar: `$abs` `$min(a,b)` `$max(a,b)`. Unary real: `$sqrt $ln $log10 $exp $sin $cos
$tan $asin $acos $atan $sinh $cosh $tanh $floor $ceil`. Binary real: `$pow(x,y)`
`$atan2(y,x)` `$hypot(x,y)`. Integer: `$clog2(n)`.

### 13.3 Randomization (stdlib)
`$srandom(seed)` (reseed; thread-local generator) · `$random([seed])` (signed 32-bit)
· `$urandom([seed])` (unsigned 32-bit) · `$urandom_range(max[,min])` · `$dist_uniform
(seed,start,end)` → integer · `$dist_normal(seed,mean,std)` → **real** · `$dist_exponential
(seed,mean)` → real. `$dist_*` take seed first (non-zero reseeds).

### 13.4 Analyses (ngspice) — return a result object (§12.1)
`$op()` · `$tran(tstep, tstop[, tstart[, tmax]][, uic=…])` · `$ac(spacing, points,
fstart, fstop)` · `$dc(src, start, stop, step[, src2,…])` · `$noise(out, in_src,
spacing, points, fstart, fstop[, ptspersum])` · `$tf(outvar, in_src)` · `$sens(outvar)`
· `$sens_ac(outvar, spacing, points, fstart, fstop)` · `$pz(in+, in-, out+, out-,
vol|cur, pol|zer|pz)` · `$disto(spacing, points, fstart, fstop[, f2overf1])` ·
`$pss(fguess, stabtime, points, harmonics)` · `$sp(spacing, points, fstart, fstop)`.
(`spacing` is `"dec"|"oct"|"lin"`.)

### 13.5 Measurements (ngspice) — return real
`$meas(analysis, name, spec)` (raw `.meas` passthrough) · `$meas_find_at` ·
`$meas_when` · `$meas_trig_targ` · `$meas_rms` · `$meas_avg` · `$meas_min` ·
`$meas_max` · `$meas_max_at` · `$meas_integral`.

### 13.6 Probes & vectors (ngspice)
`$V("n")` / `$V("a","b")` (differential) → real · `$I("branch")` → real ·
`$get_vec("v(out)")` → real[] (the whole series).

### 13.7 Circuit control (ngspice) — return void
`$set_option(key, val)` · `$set_temp(t)` · `$set_tnom(t)` · `$alter(inst, param, val)`
· `$altermod(model, param, val)` · `$alterparam(param, val)`.

---

## 14. Preprocessor directives
`` `define NAME val `` · `` `NAME `` (macro use) · `` `ifdef/`ifndef/`else/`endif `` ·
`` `include "file" ``. Resolved before parsing. (Policy directives like
`` `default_nettype `` are **not** adopted — see §15 and ROADMAP.)

---

## 15. Not yet implemented (boundary)

Parses but **not** evaluated, or absent entirely:
- **Part-selects** `a[lo:hi]` at runtime (interpreter errors). Slicing is planned.
- **Operator overloading on objects** — `signalA + signalB`, `signal > x`. Binary ops
  on an `ExternObject` are a type error today (planned for DataFrame/Signal math).
- **String indexing** `df["col"]` works mechanically (routes to `get`) but the
  **DataFrame** type that consumes it is not built yet.
- **Lambdas / `with (item …)`** iterator clauses — no closures.
- **Associative arrays** `T a[string]`, **dynamic-array `new[]`**, **multi-dim arrays**.
- **`package`**, **classes/OOP**, **`$cast`**, **`$sformat` (write-to-var)**.
- **Concurrent SVA**, **covergroups**, **clocking blocks**, **fork/join**,
  **interfaces**, **DPI**, **generate** — out of scope (see `SYSTEMVERILOG_FEATURES.md`).
- **`#![...]` file/module options** (strict nets, ground name) — planned (ROADMAP).
- **time/realtime/logic/bit** as distinct types — parse as custom (void) types.
- **`casex`/`casez` wildcards** — compared as plain equality.

Roadmap for the planned items: `docs/development/ROADMAP.md`.
