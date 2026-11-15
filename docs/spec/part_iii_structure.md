# Part III. Structure

Structure is what elaboration turns into a fixed netlist: modules, their ports and params, signals,
instances, named expressions and combinational definitions. A definition `y = expr;` is structure:
it is a wire with a function on it. Behavior with time is Part IV.

**Terms defined here:** module, port, value input, result, param, structural param, const context,
named expression, discrete expression, continuous expression, definition, delayed definition,
anonymous definition, loop, relaxation, generation, instance, placement, direct dependence, template,
implicit param, effective multiplicity, extern module, quantity.

Error codes of this part: `E03xx`. Rule citations and error tables follow §0.2.

---

## 8. Modules

A **module** is ports, params and a netlist of signals and instances, fixed at elaboration. Its
time-dependent behavior is written in `analog for` and `digital for` blocks (§10).

**Syntax**

```
module_decl    ::= attrs 'pub'? ( 'extern' STRING )? 'module' IDENT generics?
                   '(' ( port ',' )* ')' ( '->' type )? ( 'where' expr )? '{' module_item* expr? '}'
port           ::= attrs dir? IDENT ':' ( sigtype | type ) ( '=' expr )?         // a value type: a value input
dir            ::= 'in' | 'out'
sigtype        ::= 'flip'? path generic_args? | '[' sigtype ';' expr ']'
module_item    ::= attrs 'pub'? ( param | signal_decl | 'let' pat '=' expr ';' )
                 | attrs place '=' expr ( 'after' expr )? ';'                    // a definition
                 | gen<module_item>
param          ::= IDENT ':' type ( '=' expr )? ( 'where' expr )? ';'
signal_decl    ::= 'signal' IDENT ':' sigtype ( '=' expr )? ';'
gen<X>         ::= attrs 'for' pat 'in' expr '{' X* '}'
                 | attrs 'if' expr '{' X* '}' ( 'else' ( '{' X* '}' | gen<X> ) )?
place          ::= path ( '[' expr ']' | '.' IDENT )*                           // a carrier, an element or a field of one
instance       ::= callee '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')' ( '{' ( IDENT ( ':' expr )? ',' )* '}' )?
callee         ::= path generic_args? | call | '(' expr ')'
template       ::= path generic_args? '{' ( IDENT ( ':' expr )? ',' )* '}'
```

| Form | Written | Defined in |
| --- | --- | --- |
| module | `module Name(ports) [-> T] { items [result] }` | §8, §8.1, §8.2 |
| extern module | `#[link(…)] extern "abi" module Name(ports) { params }` | §8.5 |
| template | `Name { params }`: a value of type `Name` | §8.4 |
| instance | `let x = Name(connections) { params };`, or a call inside an expression | §8.2, §8.3 |

**Static semantics**

- **§8 rule 1** A module declaration is an item at namespace scope, a file or an inline `mod` (§3.4). A behavior block for it may sit in any namespace of the same file (§3.4.3). It **MUST NOT** appear
  inside a module body, a behavior block or a function.
- **§8 rule 2** A module's name is a type (§8.4). It shares the namespace of types of its file.
- **§8 rule 3** A module declared with `-> T` **MUST** end its body with an expression of type `T`, the
  **result** (§8.3). A module declared without `-> T` **MUST NOT** end its body with an expression.
- **§8 rule 4** Generic parameters are types (`T: Repr`), natures (`S: Nature`, with an optional
  default), module bounds (`C: Fn(P)`, §8.4) and const generics. A `where` after the port list
  constrains const generics and **MUST** hold at elaboration.
- **§8 rule 5** A module and all its behavior blocks **MUST** be in one file (§3.4).
- **§8 rule 6** A module has no `impl`. What its blocks share is a named expression of its body (§8.2).

> **Resolution.** §8 rule 3 makes the correspondence between `-> T` and the trailing expression
> mandatory in both directions. The source design states only that the result is the last expression.

**Dynamic semantics**

- **§8 rule 7** A module does nothing by itself at run time. Its run-time effects are its definitions
  (§8.2), its instances, and its behavior blocks (Part IV).
- **§8 rule 8** Elaboration evaluates const generics, checks params (§8.1), runs generation (§8.2.7)
  and places every instance (§8.3). The result is fixed for the whole session unless the host
  re-elaborates.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0301` | module declared with `-> T` has no result expression, or the result is not of type `T` | elaboration |
| `E0302` | module declared without `-> T` ends its body with an expression | parse |
| `E0303` | module declaration nested in a module body, a block or a function | parse |
| `E0304` | the `where` after the port list is false, or is not a `bool` const expression | elaboration |

**Examples**

```rust
/// Ideal resistor.
module Resistor(p: Electrical, n: Electrical) {
    /// Resistance, in ohms.
    r: f64 where r > 0.0;
    tc1: f64 = 0.0;
    tnom: f64 = 300.15 where tnom > 0.0;
}

analog for Resistor {
    i(p, n) +> v(p, n) / (r * (1.0 + tc1 * (temperature() - tnom)));
}
```

### 8.1 Ports and params

A **port** is a name in a module's port list. It is a carrier, a bundle of carriers, or a value
input. A **param** is a value fixed during a solve: the model reads it, the host sets it.

| Port | Written | Connected to |
| --- | --- | --- |
| carrier of a one-driver discrete nature | `in a: Bit<T>`, `out q: Bit<T> [= initial]` | a carrier; an `in` port also takes a discrete expression, which is an anonymous definition (§8.2.9) |
| carrier of any other nature | `p: Electrical`, `sda: Wire [= initial drive]` | a carrier (a net) |
| bundle | `tx: Stream<T>`, `rx: flip Stream<T>`, `in s: Stream<T>` | a bundle carrier or a bundle literal (§7) |
| value input | `x: f64` | an expression (§8.3) |

A **value input** is a port whose type is a value type. At each instance it names the expression
the parent wrote in that position (§8.3.1).

A param is written `name: Type [= default] [where condition];` in the body.

| Part | Example |
| --- | --- |
| no default: required | `r: f64;` |
| `Option`: may be left out (§3.2) | `rs: Option<f64>;` |
| `where`: a boolean over this param and earlier ones | `n: f64 = 1.0 where (0.1..=10.0).contains(&n);` `vmax: f64 where vmax > vmin;` |

A **const context** is a position evaluated at elaboration: a `const` item, a const generic
argument, a generation range or condition, an array length, a param default and a `where` condition.

**Static semantics: ports**

- **§8.1.4** The type of a port decides its kind. A nature or an array of natures makes a carrier
  port. A bundle type makes a bundle port. Any value type makes a value input.
- **§8.1.5** A carrier port of a one-driver discrete nature (§4) **MUST** have a direction, `in` or
  `out`. A carrier port of any other nature **MUST NOT** have one. A bundle port takes no direction,
  `flip` in its type, or `in` (§7). A value input **MUST NOT** have a direction.
- **§8.1.6** `in` is read. `out` is driven by this module and is read inside it like any of its
  carriers. A module **MUST NOT** write an `in` port (§5.2).
- **§8.1.7** `= expr` in a port is the first commit of this module's drive of that port (§5.2). It
  is allowed on an `out` port and on a port of a resolved nature. It is an error on an `in` port, a
  value input, a bundle port and a port of a continuous nature.
- **§8.1.8** Every port **MUST** be connected at every instance, except a trailing value input of
  type `Option<T>`, which reads `None` when left out (§3.2). A carrier port is never optional.
- **§8.1.9** Connections are positional. After them, optional value inputs **MAY** be passed by
  name (§3.2).
- **§8.1.10** A carrier port **MUST** be connected to a carrier of the same discrete nature, or
  of a compatible continuous nature (§6.1.3), with the same array shape, visible in the parent.
  `gnd` connects to a port of any continuous nature (§6).
- **§8.1.11** An `out` port **MUST** be connected to a carrier the parent may hand to one driver: a
  signal of the parent, an `out` port of the parent, or an element or bundle field of one. It takes
  no expression. The instance then holds the drive, and §5.2 forbids a second one.
- **§8.1.12** An `in` port takes a carrier or a discrete expression (§8.2.10). An expression there
  is an anonymous definition of the parent (§8.2.9). A continuous expression there is an error at
  the call: `transition(v(a), …)` is rejected.
- **§8.1.13** A value input takes any expression of its type, discrete or continuous. It **MAY**
  probe: `slew(v(a), …)` is accepted.
- **§8.1.14** The ports of the top module are connected by the host (§23.1).

> **Resolution.** §8.1.7 rejects `= expr` on a port of a continuous nature. A node's starting value
> is given where the node is declared (§6) or by the host's nodeset (§18), so that it is given once.

> **Resolution.** §8.1.11 lists what an `out` port accepts. The source design says only that a port
> connects to a carrier and that a one-driver carrier has one drive (§5.2).

**Static semantics: params**

- **§8.1.1** `where` is a `bool` expression over this param and params declared earlier in the
  body. It is checked at elaboration and on every host `set`. The host reads it as the param's
  bounds (§23.2).
- **§8.1.2** A param is **structural** when one of the following reads it: generation (§8.2.7), a
  const generic argument, an array length, a template (§8.4), or the trip count of a loop that
  places instances (§8.3.4). A param is also structural when it alone selects the kind of a branch
  (§11 rule 3). A structural param is fixed at elaboration.
- **§8.1.3** Units, aliases, Monte Carlo distributions, nodesets and "exclude from gradients" are
  not model text. `///` documents a param (§21). The rest belongs to the host session (§23).
- **§8.1.15** A param's type is a value type. A module's name is one (§8.4). A param is never a
  carrier and is never written by a model: there is no assignment form for it.
- **§8.1.16** A default is a const expression over constants, const generics and earlier params.
- **§8.1.17** A param with no default and a type other than `Option<T>` is required. Every instance
  and every template **MUST** bind it. For the top module the host binds it.
- **§8.1.18** At an instance, params are bound in a block, `Name(ports) { name: value }`, under the
  call rule of §3.2: a left-out `Option` param is `None`, and a `T` given for `Option<T>` is `Some`.
  A name **MUST** be a param of the module and **MUST NOT** repeat.
- **§8.1.19** A bound value is an expression over the parent's params, constants and const
  generics. It **MUST NOT** read a carrier, a probe, the environment (§14.4) or an instance result.
- **§8.1.20** Params **MUST** be declared at the top level of the body, never inside generation.
- **§8.1.21** The environment functions of §14.4 **MUST NOT** appear in a const context.

> **Resolution.** §8.1.2 adds array lengths and instance-placing trip counts to the structural
> params. Both change the elaborated netlist, as generation does.

> **Resolution.** §8.1.16, §8.1.19 and §8.1.20 are implied by "a value fixed during a solve" and by
> the host listing params from the body (§21). §8.1.21 follows from elaboration preceding analysis.

**Dynamic semantics**

- **§8.1.22** A param is known and constant during every solve (§16.2). Its value changes only by
  a host `set` (§9 rule 4).
- **§8.1.23** When a host `set` changes a param, every value bound from it (§8.1.19) is recomputed
  and every dependent `where` is checked again. A `set` that violates a `where` is refused (§23).
- **§8.1.24** A host `set` of a structural param during a run is refused. The host re-elaborates.
- **§8.1.25** A value input is evaluated as §8.3.1 states. It has no storage and no events.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0310` | wrong direction for the port's kind, or a missing one (§8.1.5) | elaboration |
| `E0311` | `= expr` on a port that takes no initial value (§8.1.7) | elaboration |
| `E0312` | a port left unconnected, or too many connections (§8.1.8) | elaboration |
| `E0313` | a carrier port connected to a carrier of another array shape (§8.1.10) | elaboration |
| §6.1 | a carrier port connected to a carrier of another kind (`E0250`) or of a different or incompatible nature (`E0251`) | elaboration |
| `E0314` | an expression, or a carrier the parent cannot hand over, connected to an `out` port (§8.1.11) | elaboration |
| `E0315` | an expression connected to a carrier port that is not an `in` port | elaboration |
| `E0316` | a continuous expression connected to an `in` port (§8.1.12) | elaboration |
| `E0317` | `where` is false at elaboration | elaboration |
| `E0318` | `where` is not `bool`, or reads a later param or anything that is not a param or constant | elaboration |
| `E0319` | a required param is not bound (§8.1.17) | elaboration |
| `E0320` | a param block names no param of the module, or names one twice | elaboration |
| `E0321` | a bound param value or a default is not an expression over params and constants | elaboration |
| `E0322` | a param declared inside generation | elaboration |
| `E0323` | an environment function in a const context (§8.1.21) | elaboration |
| §5.2 | a second drive of a one-driver carrier (`E0220`); a write to an `in` port (`E0222`) | elaboration |
| §23.1 | a host `set` that violates `where` (`E0802`), or that targets a structural param during a run (`E0803`) | host |

**Examples**

```rust
module AsyncFifo<T: Repr, const A: u32>(in wclk: Bit, in rclk: Bit, rx: flip Stream<T>, tx: Stream<T>)
    where A >= 1 { }   // the body is elided
```

```rust,fragment
let ramp = transition(vref * (code as f64) / 256.0, 0.0, tedge, tedge);   // in port: a discrete expression
let lim  = slew(v(a), 1M, 1M);                                           // value input: may probe
```

### 8.2 The body

The body is the netlist. Its items appear in any order. Every name is visible in the whole body
and in the module's blocks.

| Item | Example |
| --- | --- |
| param | `c: f64 = 1p where c > 0.0;` |
| signal | `signal mid: Electrical;` `signal q: Bit<u8> = 0;` |
| instance | `let r1 = Resistor(a, b) { r: 1k };` |
| named expression | `let half = r / 2.0;` `let drift = v(p, n) * 1e-3;` `let acc = idt(v(a));` |
| definition | `y = a & b;` `busy = st != Tx::Idle;` `q = !(r \| qn) after tpd;` |
| generation | `for k in 0..N { … }`, `if cond { … }` |

**`let` names an expression.** A **named expression** is a body item `let pat = expr;`. It writes
nothing and has no events. Wherever the name is read, the expression is evaluated there.

**`=` defines a discrete carrier.** A **definition** is a body item `place = expr;`. It makes this
module's drive of the carrier a combinational function of what the expression reads. A **delayed
definition** carries `after d`. An **anonymous definition** is an expression connected to an `in`
port (§8.1.12): the parent drives an unnamed carrier with it.

A **discrete expression** reads only values, params, discrete carriers, the environment that is
constant during an analysis (§14.4), and results that are themselves discrete expressions. A
**continuous expression** is any other: it reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()`, `analysis(Static)`, or
a result that does.

**Static semantics: names and `let`**

- **§8.2.11** Ports, params, signals and `let` names of one module share one namespace. A name
  **MUST** be unique in it, across generation arms too.
- **§8.2.12** A body `let` **MUST NOT** bind `mut`. `let mut` is a value-level local of a block or
  a function.
- **§8.2.13** A `let` with a pattern names each component: `let (r, f) = (…);` names two
  expressions.
- **§8.2.14** A `let` whose expression is the placement of a module without a result names the
  instance. The name is then used only as the head of `inst.name` (§8.2.8). Its pattern **MUST** be
  an identifier.
- **§8.2.15** A `let` whose expression contains placements places each of them once, at the `let`,
  not once per read (§8.3.3).
- **§8.2.16** A `let` **MUST NOT** read its own name, directly or through other `let`s. The one
  exception is feedback through an instance, under §8.3.5.
- **§8.2.17** The type of a `let` is the type of its expression. Where it may be read follows from
  what it reads: a continuous expression is read only where §3 allows a probe; an `Event` only in
  event positions (§13); an expression that calls `emit!` only where `emit!` is allowed (§19).
- **§8.2.18** Inside a block or a function, an immutable `let` is the same rule read in one place.

> **Resolution.** §8.2.11 requires unique names even across the arms of one generation `if`. A
> module variant is selected with a template (§8.4), not with two items of one name.

> **Resolution.** §8.2.14: when a module has a result, the `let` name reads the result, and member
> access applies to that value. The `pub` items of such an instance are not reachable from the
> parent's text. The host still reaches them by path (§21; G.48).

**Static semantics: definitions**

- **§8.2.5** A definition is recomputed only when a carrier or param it reads changes. Its
  expression, and its delay, **MUST** be discrete expressions: no probe, no `now()`, no `freq()`, no `gmin()`, no `source_scale()`, no
  `analysis(Static)`, directly or through a `let`, a function or a result. It **MAY** read
  `temperature()`, `simparam` and `analysis` of an analysis kind. An analog value is sampled in a
  reaction (§12.1).
- **§8.2.6** A definition between carriers of the same width and different types is written with
  the stdlib `cast` (§22.1): `byte = cast(sh);`. There is no implicit conversion.
- **§8.2.9** An anonymous definition obeys every rule of a definition without `after`. Its carrier
  belongs to the parent, has no name, and is read only by the port it is connected to.
- **§8.2.10** The target `place` **MUST** name a discrete carrier, or an element or field of one,
  that this module may drive under §5.2:

  | Target | A definition or commit by module `M` |
  | --- | --- |
  | a discrete signal declared in `M` | allowed |
  | an `out` port of `M`; a bundle field `M` produces (§7) | allowed |
  | a port or signal of a resolved nature that `M` sees, a `pub` one of its instances included | allowed: `M` holds its own drive |
  | an `in` port of `M`; a bundle field `M` consumes | error |
  | a one-driver carrier of an instance of `M`, `pub` or not | error: a parent never writes a child's drive |
  | a carrier already handed to an instance's `out` port (§8.1.11) | error (§5.2) |
  | a node, a param, a value input, a `let` name | error |

- **§8.2.19** The expression's type **MUST** be the value type of the levels the `place` names.
- **§8.2.20** Within one module, a carrier is driven by definitions or by commits of reactions,
  never both (§5.2). Several definitions **MAY** target disjoint levels of one carrier. Two
  definitions that share a level are an error. A level no definition names stays unknown.
- **§8.2.21** A carrier whose drive is a definition without `after` **MUST NOT** have an initial
  value on the levels defined. A delayed definition **MAY** have one: `out q: Bit = false` with
  `q = !(r | qn) after tpd;`.
- **§8.2.22** The delay of a delayed definition is an `f64` discrete expression. It **MAY** read
  the new value: `y = a & b after if a & b { tr } else { tf };`.

> **Resolution.** §8.2.10 lets a parent hold its own drive of a `pub` resolved carrier of a child.
> §5.2 lets any module that sees a resolved carrier drive it, and §8.2.8 lets a parent attach its
> own branches to a public node.

> **Resolution.** §8.2.20 reads "one `=` definition" per level, so two definitions of different
> fields compose, as two reactions that commit different fields do (§12.1.2).

> **Resolution.** §8.2.21: an initial value under an undelayed definition would be replaced in the
> microstep that commits it. Rejecting it keeps `prev()` and `changed()` at `(0, 0)` unambiguous
> (G.51).

**Static semantics: generation, visibility**

- **§8.2.7** **Generation** is `for` and `if` in a body or a `digital` block. It runs at
  elaboration. Its range or condition **MUST** be a const expression over params, constants and
  const generics. A module **MAY** place itself under an `if`; elaboration unrolls it. A recursion
  that does not end is an error.
- **§8.2.23** An item declared in a generation `for` makes an array, one element per iteration in
  iteration order, indexed from 0. Nested `for`s nest the array. Inside the loop body the name
  denotes the current iteration's element.
- **§8.2.24** An item declared in a generation `if` exists only when its arm is elaborated. A
  reference to an item that does not exist is an elaboration error.
- **§8.2.8** `pub` on an item makes it visible to the parent as `inst.name`. A parent **MAY** read
  a public carrier, a public param or a public named expression, probe a public node, and attach its
  own branches to a public node. A parent never writes a child's drive and never contributes to a
  child's branch. Without `pub`, an item is invisible to other models. The host observes everything
  (§21).

> **Resolution.** §8.2.23 indexes generated arrays by iteration ordinal, not by the loop value:
> `for k in 2..5` makes elements 0 to 2. F.3 reads `bank.cells[3][5]` this way.

> **Resolution.** §8.2.24 keeps every name visible in the whole body, as for other items, and moves
> the existence check to elaboration, where the condition's value is known.

**Dynamic semantics: `let`**

- **§8.2.25** A read of a `let` name evaluates its expression in the reader's context, with what
  that context reads (§15, "What each kind of code reads"). In `analog` the expression is
  differentiated (§11 rule 5). Read by another `let`, a value input or a result, it is evaluated
  wherever that one is read.

- **§8.2.26** A `let` over params and constants only is a constant. It is recomputed when a host
  `set` changes a param it reads (§9 rule 4).
- **§8.2.27** A `let` keeps no value between reads. Two reads in one context yield the same value,
  because the context fixes everything the expression can read.
- **§8.2.28** A value read through a `let` keeps its known flags (§3.3). An `analog` block that
  reads a discrete carrier through a `let` is still bound by §3.3.5.

**Dynamic semantics: definitions**

- **§8.2.1** When an input changes, the drive is recomputed in the same microstep, before that
  microstep's reactions run (§15 step 3.2). Definitions and `resolve` functions are evaluated in
  dependency order, so a chain settles within one microstep.
- **§8.2.2** A change of the carrier's value fires its `changed()` in that microstep.
- **§8.2.29** A definition is first evaluated at `(0, 0)`, when initial values are committed and
  the structure settles (§16 step 2). A host `set` of a param it reads recomputes it (§9 rule 4).
- **§8.2.3** **Loops.** *Static:* a drive that depends on itself with no reaction and no `after`
  in between is a **loop**. Dependencies are tracked per drive: `s.others()` depends on the other
  drives of `s`, not on this module's own. A loop that passes through a one-driver carrier is an
  elaboration error. A loop among drives of resolved carriers only is legal. *Dynamic:* a legal
  loop, one strongly connected component of the drive dependency graph, settles by
  **relaxation**. In each microstep where an input of the loop changed:
  1. every drive of the loop starts as not driving: it is absent from what `resolve` receives;
  2. all drives of the loop are re-evaluated together, each from the values the previous pass left;
  3. passes repeat until no drive changes, at most as many passes as the loop has drives, plus one;
  4. if drives still move after that, the levels that move are unknown, and the simulator writes a
     record naming the loop's drives on the `log` stream of the first drive's instance (§19), once
     per loop and analysis.

  Starting from nothing every time, a loop cannot latch. Only the settled value is observable:
  `changed()` compares it with the value before the microstep.
- **§8.2.4** `y = expr after d;` is the commit `y <- expr after d` (§12.1), made each time the
  value of `expr` changes, the first time at `(0, 0)`. The commit is made when the structure
  settles (§15 step 3.2) in the microstep in which `expr` changed. Everything else is the commit's rule: delays are transport per
  statement (§12.1.4); `after 0` is the next microstep; where time does not advance, a positive
  delay never comes due (§16.1.3).
- **§8.2.30** The delay is evaluated when the commit is made. A later change of what the delay
  reads does not move a pending commit. An unknown delay commits `Unknown` at the next microstep
  (§3.3.3).
- **§8.2.31** Definitions run at instants only. Per analysis they follow the second table of
  §16.2: at the microsteps of `(0, ·)` and of each sweep point, at every instant of a transient, and
  never in a small-signal sweep.
- **§8.2.32** A carrier driven by a definition is constant during every solve, like any discrete
  carrier (§5.1). Its change, when `analog` reads it, is a jump (§15.2).

> **Resolution.** Relaxation step 1 defines "not driving" as absent from the slice passed to
> `resolve`, which is nature-independent. Step 2 defines "together" as evaluation from the previous
> pass's values, so the outcome does not depend on an evaluation order.

> **Resolution.** §8.2.30 fixes when the delay is read. The source design says only that the delay
> may depend on the new value.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0330` | a name declared twice in one module (§8.2.11) | elaboration |
| `E0331` | `let mut` in a module body | parse |
| `E0332` | a `let` reads itself outside §8.3.5 (§8.2.16) | elaboration |
| `E0333` | the name of an instance without a result used as a value (§8.2.14) | elaboration |
| `E0335` | two definitions share a level (§8.2.20) | elaboration |
| `E0336` | a definition, its delay or an anonymous definition is a continuous expression (§8.2.5) | elaboration |
| `E0337` | a definition's type differs from its target's (§8.2.19, §8.2.6) | elaboration |
| `E0338` | an initial value on levels driven by an undelayed definition (§8.2.21) | elaboration |
| `E0339` | a loop through a one-driver carrier (§8.2.3) | elaboration |
| `E0340` | a generation range or condition is not a const expression (§8.2.7) | elaboration |
| `E0341` | generation recursion does not end | elaboration |
| `E0342` | a reference to an item whose generation arm was not elaborated, or an index outside a generated array (§8.2.24) | elaboration |
| `E0343` | `inst.name` names an item that is not `pub` (§8.2.8) | elaboration |
| §5.2 | a definition target this module may not drive (§8.2.10): a second drive of a one-driver carrier (`E0220`), any other target (`E0222`); a carrier with both a definition and commits in one module (`E0221`) | elaboration |
| — | a loop of resolved drives that does not settle is not an error: its levels are unknown and a `log` record is written (§8.2.3) | run |

**Examples**

```rust
/// Clock buffer and two flip-flops: the buffer settles before either reaction runs.
module Shift2(in clk: Bit, in d: Bit, out q2: Bit) {
    signal clk2: Bit;
    signal q1: Bit;
    clk2 = clk;
}
digital for Shift2 {
    on clk.rise()  { q1 <- d; }
    on clk2.rise() { q2 <- q1; }      // reads the old q1: two stages, as in hardware
}

/// Shift register, LSB first: the register is the array, the byte a cast of it.
module UartShift(in clk: Bit, in rx: Bit, out byte: Bit<u8>) {
    signal sh: [Bit; 8];
    byte = cast(sh);
}
digital for UartShift { on clk.rise() { sh[7] <- rx; for k in 0..7 { sh[k] <- sh[k + 1]; } } }

/// Bidirectional pass gate: a legal loop when two gates join the same wires (§22.3).
module Tran(a: Wire, b: Wire, in en: Bit) {
    a = if en { b.others() } else { Logic::Z };
    b = if en { a.others() } else { Logic::Z };
}
```

`SrLatch` (E.2) shows the other side of §8.2.3: without `after`, `q = !(r | qn);` and
`qn = !(s | q);` form a loop through one-driver carriers, `E0339`.

### 8.3 Instances inside expressions

A module may take value inputs and have a result; then it can be placed inside an expression. Every
operator with state is written this way: derivatives, integrators, filters, crossings.

An **instance** is one elaborated copy of a module, with its own signals, state and params. A
**placement** is a syntactic occurrence of the `instance` production: a module name, or a template
value (§8.4), called with connections.

**Static semantics**

- **§8.3.1** **A value input is a `let` written by the caller.** `x: f64` in a port list names, at
  each instance, the expression the caller wrote there. The module reads it under the rule of `let`
  (§8.2.25).
- **§8.3.2** **The result is a `let` read by the caller.** It is the last expression of the body.
  It **MAY** have any value type, `Event` included. It sees every name of the body.
- **§8.3.3** **Every placement is one instance**, per elaborated copy of the enclosing module and
  per unrolled iteration. A body **MAY** name one, `let acc = idt(v(a));`; then `acc` reads its
  result in every block, and every read is the same instance. Two placements with equal arguments
  are two instances.
- **§8.3.4** **Where.** A placement is allowed wherever a module writes an expression over time:

  | Position | Placement | Notes |
  | --- | --- | --- |
  | a body `let` | yes | placed once, by the `let` |
  | a definition, an anonymous definition | yes | the result **MUST** be a discrete expression (§8.2.5) |
  | a connection or value input of another placement | yes | an `in` port argument is an anonymous definition |
  | an `analog` block | yes |  |
  | the event of an `on` | yes | one instance per reaction |
  | a `fn`, a method, a closure | no | a function places no instance (§3 rule 4) |
  | a reaction body | no | a reaction reads a named result instead |
  | a loop whose trip count is not a constant | no | loops over params and constants are unrolled, one instance each |
  | a const context (§8.1); a bound param value | no | a result is not fixed during a solve |

  `m`, `dtemp` and a param block apply to a placement as to any instance:
  `zi_nd(v(inp), 1u) { num: [0.5, 0.5], den: [] }`.
- **§8.3.5** **Feedback.** A result has a **direct dependence** on a value input when its
  expression reads that input other than through a carrier of the module. A result that reads only
  the module's own carriers goes through unknowns of the module, so an instance **MAY** read its own
  result in its arguments: `let y = idt(f(y));` is the differential equation `y' = f(y)`. A cycle of
  named expressions, value inputs and results in which every result has a direct dependence on the
  input it is entered by closes a loop with no unknown in it: an elaboration error.
- **§8.3.6** **The core is modules too.** `ddt`, `delay`, `limit`, `noise`, `cross` and `at` are
  modules with a result implemented by the simulator (§14). They follow §8.3.1–§8.3.5 and no others.
  The result of each has a direct dependence on its value inputs.
- **§8.3.7** Arguments follow §3.2. Connections and value inputs are positional, in port-list
  order. Trailing `Option` value inputs **MAY** be left out or passed by name.
- **§8.3.8** A name in an argument expression resolves in the caller's scope. The environment it
  reads (§14.4) is the caller's: `temperature()` in an argument is the caller's temperature.
- **§8.3.9** A value input **MAY** be read in the callee's body `let`s, result, `analog` blocks,
  event expressions and reaction bodies. Each read is checked as if the caller's expression were
  written there: a continuous argument read by a definition of the callee is `E0336` at the call.
- **§8.3.10** A result is read where its expression could be written: a continuous result where
  probes are allowed (§3), an `Event` result in event positions (§13).
- **§8.3.11** As in Rust, a `{` after the head of `on`, `if`, `match`, `while` or `for` opens the
  block. A placement with a param block is parenthesized there.

> **Resolution.** §8.3.5 states the feedback rule over cycles of any length, with "direct
> dependence" as the test. The source design states it for one instance reading its own result.

> **Resolution.** §8.3.6 classifies every core module's result as directly dependent on its inputs.
> `let y = ddt(f(y));` and `let y = delay(f(y), td);` are therefore errors: the equation is written
> on a node, as `idt` does (G.56).

> **Resolution.** §8.3.8 separates where a name resolves (the caller) from when the expression is
> evaluated (the callee's read, §8.3.12).

**Dynamic semantics**

- **§8.3.12** **Evaluation of a result.** A read of a placement, or of a name bound to one,
  evaluates the callee's result expression in the reader's context (§8.2.25). Each value input the
  result reads is evaluated in that same context, in the caller's scope. No value is copied at the
  instance boundary and no microstep or iteration separates the two sides.
- **§8.3.13** **Evaluation of a value input elsewhere.** A read of a value input in the callee's
  `analog` block evaluates the caller's expression at the current iterate and differentiates it
  (§11 rule 5). A read in a reaction of the callee evaluates it at the reaction's instant. A read in a
  located event is evaluated by the solver.
- **§8.3.14** **An instance exists whatever control flow surrounds its placement.** Its blocks run
  and its state advances as for any instance. The surrounding control flow decides only whether the
  result is used at that evaluation.
- **§8.3.15** Instance state advances only on accepted steps (§17.2). A rejected step leaves it
  unchanged.
- **§8.3.16** In `analog`, a result that reads the callee's nodes is a probe of the callee's
  unknowns. The caller's contribution gets Jacobian entries for those unknowns. The callee's rows
  get entries for whatever the value inputs probe in the caller.
- **§8.3.17** Per analysis, an instance in an expression is its module: its `analog` blocks follow
  the first table of §16.2, its definitions and reactions the second. The core modules are specified
  one by one in §14.
- **§8.3.18** A value input or result keeps its known flags (§3.3). §3.3.5 applies to an
  `analog` read through either.

`idt`, the stdlib integrator, is the worked case: its text and its behavior in every solve are
§22.2.8.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0350` | a placement in a `fn`, a method, a closure or a reaction body (§8.3.4) | elaboration |
| `E0351` | a placement in a loop whose trip count is not a constant (§8.3.4) | elaboration |
| `E0352` | a placement in a const context or a bound param value (§8.3.4) | elaboration |
| `E0353` | a feedback cycle with no unknown in it (§8.3.5) | elaboration |
| `E0354` | a result read where its expression could not be written (§8.3.10) | elaboration |
| §3.2 | a missing non-optional argument (`E0141`); a named argument that is not a trailing `Option` (`E0140`) | elaboration |
| §15.2 | an equilibrium solve that leaves an `idt` without `ic` undetermined (`E0503`) | run |

**Examples**

```rust
analog for Pi { v(outp) +> k1 * v(inp) + k2 * idt(v(inp), 0.0); }     // one instance of idt
```

In `Pi`, the placement makes one `idt` instance with `x` naming `v(inp)` of `Pi`. At each iterate
the instance's row reads `v(inp)` and gets its derivative. The result is the instance's unknown
`potential(y)`, so the contribution to `v(outp)` has the entry `k2` in that column.

```rust
fn windup(u: f64, lo: f64, hi: f64) -> f64 { u.clamp(lo, hi) - u }

/// PI with output limits and back-calculation anti-windup: the integral reads its own result.
module PiAw(e: VSignal, y: VSignal) {
    kp: f64 = 0.5; ki: f64 = 2k; kt: f64 = 10k;
    lo: f64 = 0.0; hi: f64 = 1.0 where hi > lo;
    let xi = idt(ki * v(e) + kt * windup(kp * v(e) + xi, lo, hi));
}
analog for PiAw { v(y) +> (kp * v(e) + xi).clamp(lo, hi); }
```

`xi` reads itself through `idt`, whose result reads only its own node: legal by §8.3.5.

### 8.4 Templates and swappable modules

**A module with its params bound is a value.** A **template** is the expression `Name { params }`.
Its type is `Name`. Calling it with connections places an instance.

**Static semantics**

- **§8.4.1** A template goes wherever a value goes: a param, a `const`, a function's result. It
  has no `Repr`, so it is never carried (§1).
- **§8.4.2** A template binds params under §8.1.18. Every required param **MUST** be bound in it.
  Params it leaves out take their defaults. `m` and `dtemp` (§8.5) **MAY** be bound in it.
- **§8.4.3** A template of a generic module names its arguments, `Name::<G> { … }`, or lets them be
  inferred. Its type is `Name<G>`.
- **§8.4.4** A placement whose head is a template value, `cell(connections)` or
  `nch(l, w)(d, g, s, b)`, takes connections only. It **MUST NOT** carry a param block.
- **§8.4.5** The bound `C: Fn(P)` holds for every module whose port list has the types and
  directions `P`, in order. Port names do not matter. A value of type `C` is a template of such a
  module.
- **§8.4.6** A template-valued expression is evaluated at elaboration. It **MUST** be a const
  expression over params and constants. Every param it reads is structural (§8.1.2), and so is a
  param of template type.
- **§8.4.7** Binning is a function returning a template. The function is ordinary: it places
  nothing (§3). The caller places the result.
- **§8.4.8** `inst.name` on an instance placed through a template-typed param is resolved at
  elaboration against the concrete module.

> **Resolution.** §8.4.2 and §8.4.4: a template is complete, and the call adds connections only.
> Overriding a template's params at the call would be a struct update, which the language lacks
> (G.53).

> **Resolution.** §8.4.5 is read literally: a module with a result also satisfies `Fn(P)`. The bound
> says nothing about a result, so a placement through `C` is not read as a value.

> **Resolution.** §8.4.8: elaboration is fully concrete, so `bank.cells[3][5].sn` (F.3) is checked
> against the module actually bound. A missing or non-`pub` item is `E0343`.

**Dynamic semantics**

- **§8.4.9** A template has no run-time existence. An instance placed from one is identical to an
  instance placed with `Name(connections) { params }`.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0360` | a module that does not satisfy the bound `Fn(P)` (§8.4.5) | elaboration |
| `E0361` | a param block on a placement whose head is a template value (§8.4.4) | elaboration |
| `E0362` | a template-valued expression that is not a const expression (§8.4.6) | elaboration |
| §8.1 | a template that leaves a required param unbound (§8.4.2; `E0319`) | elaboration |

**Examples**

```rust
/// One-transistor one-capacitor DRAM cell; Cell is the bundle of §7.
module Dram1T1C(c: Cell) {
    cs: f64 = 30f;
    ron: f64 = 5k;
    rleak: f64 = 1T;
    pub signal sn: Electrical = 0.0;     // storage node
}

module MemoryBank<C: Fn(Cell), const ROWS: u32, const COLS: u32>(in wl: [Bit; ROWS], bl: [Electrical; COLS]) {
    cell: C;
    for r in 0..ROWS { for c in 0..COLS { pub let cells = cell(Cell { wl: wl[r], bl: bl[c] }); } }
}
```

```rust,fragment
let bank = MemoryBank::<_, 64, 32>(wl, bl) { cell: Dram1T1C { cs: 30f } };   // in a parent's body
```

```rust
/// Binning: the function picks the params, the caller places the result.
pub fn nch(l: f64, w: f64) -> Bsim4 {
    let vth0 = if l < 0.18u { 0.42 } else if l < 1u { 0.45 } else { 0.47 };
    Bsim4 { l: l, w: w, vth0: vth0 }
}
```

```rust,fragment
let m1 = nch(0.13u, 2u)(d, g, s, b);                                       // likewise
```

### 8.5 Implicit params and extern modules

Every instance has two **implicit params**, `m` and `dtemp`, inherited by its children. An **extern
module** is implemented outside the language.

**Static semantics: implicit params**

- **§8.5.1** `m: f64 = 1.0 where m > 0.0` and `dtemp: f64 = 0.0` exist on every instance, core and
  extern included. They are bound in the param block like any param. Neither is structural.
- **§8.5.2** A module **MUST NOT** declare a port, param, signal or `let` named `m` or `dtemp`.
- **§8.5.3** The **effective multiplicity** of an instance is the product of `m` along its path.
  Its effective temperature offset is the sum of `dtemp` along its path. Inside a module, the names
  `m` and `dtemp` read the effective values.

> **Resolution.** §8.5.3 reads "inherited by its children" as VAMS 6.3.6 does for `$mfactor`:
> multiplicities multiply down the hierarchy. `dtemp` sums, as stated for `temperature()`. That
> a model reads the names `m` and `dtemp` at all is this note's addition (G.54).

**Dynamic semantics: implicit params**

- **§8.5.4** An instance behaves as `m` identical instances in parallel (VAMS 6.3.6). With `M` its
  effective multiplicity:

  | Quantity | Scaling |
  | --- | --- |
  | a flow contribution | multiplied by `M` |
  | a flow probe, `p.inflow()` | divided by `M`: the model sees one instance |
  | a potential contribution, a potential probe | unchanged |
  | noise power contributed to a flow | multiplied by `M` |
  | noise power contributed to a potential | divided by `M` |
  | discrete carriers, events, reactions | unaffected |

- **§8.5.5** `temperature()` inside an instance returns the analysis temperature plus its
  effective `dtemp` (§14.4).
- **§8.5.6** A host `set` of `m` or `dtemp` during a run is a param change (§9 rule 4).

> **Resolution.** The potential-noise row of §8.5.4 completes "noise power is multiplied by `m`"
> with the other half of VAMS 6.3.6: `m` sources in parallel average a series noise.

**Static semantics: extern modules**

- **§8.5.7** An extern module is written `#[link(…)] extern "abi" module Name(ports) { params }`.
  Its body lists params only: no signal, `let`, definition, generation or result. It has no value
  inputs and no behavior blocks.
- **§8.5.8** The ABI string is `"osdi"` or `"cosim"`. The set is closed. `"osdi"` is OSDI
  version 0.4: that document is normative for the descriptor and the calls of the ABI. The C
  interface of an extern module is the **device ABI**, whose exported symbol is
  `piperine_abi_1`. `"cosim"` is a module whose reaction is irreversible (it is never rolled
  back, §8.5 contract table). `"osdi"` names an adapter over the same ABI.
- **§8.5.9** `#[link("file", model = "name")]` is required. It names the foreign artifact and, when
  the artifact holds several, the model in it. `model` is the only key. A relative path is relative
  to the source file.
- **§8.5.10** Ports and params are checked against the foreign descriptor at elaboration: every
  listed param **MUST** exist there with a compatible type, and the ports **MUST** match its
  terminals in number and order. A descriptor param the body does not list keeps its foreign
  default and is visible to the host only (§21). A host `set` that would change the structure
  of an extern module is refused like a structural param (`E0803`, §23.1 rule 7); in a `Dc`
  sweep that point re-elaborates the instance.
- **§8.5.11** Port kinds follow the ABI:

  | ABI | Ports |
  | --- | --- |
  | `"osdi"` | nodes only |
  | `"cosim"` | carriers of one-driver discrete natures only, each `in` or `out` |

- **§8.5.12** An extern module is a module in every other respect: it is placed, templated
  (§8.4), bound by `Fn(P)`, and given `m` and `dtemp`.

> **Resolution.** §8.5.9 resolves a relative path against the source file, as `include_str!` does
> (§20). §8.5.10 lets the body list a subset of the descriptor's params.

**Dynamic semantics: extern modules**

| ABI | Contract |
| --- | --- |
| `"osdi"` | The model is an analog block the language did not write, reached through an adapter over the device ABI (§8.5.8). It is loaded at every Newton iteration. Its limiting vetoes convergence (§18). Its state advances on accepted steps. Its params, operating-point variables, noise sources and internal nodes appear in the metadata (§21) under the descriptor's names. `m` and `dtemp` are passed as the descriptor's multiplicity and temperature. |
| `"cosim"` | The foreign simulator is one module in the microstep loop (§15). At a microstep where an input changed, or at a time it asked for, it is given the inputs and returns commits and the next time it wants to run. The commits apply at the next microstep. The requested time is a scheduled time (§17.2). It never probes. It is never advanced into a step that can be rejected, so it needs no rollback. It is reached through the device ABI (§8.5.8). |

- **§8.5.13** An `"osdi"` instance follows the first table of §16.2 as an `analog` block does: it
  is evaluated in every kind of solve and linearized in a small-signal sweep.
- **§8.5.14** A `"cosim"` instance follows the second table of §16.2 as reactions do. Where time
  does not advance, a time it asks for never comes due (§16.1.3).
- **§8.5.15** A foreign model with analog pins sits behind `A2D` and `D2A` (§22.4).
- **§8.5.16** A `"cosim"` step is all-or-nothing, like an `extern` function (§3.3.2): when an
  input holds an unknown level the foreign simulator is not run, and every `out` port commits
  `Unknown`.

> **Resolution.** §8.5.16 is stated here. The C interface of `"cosim"` (the step function, its
> descriptor, how it returns the next time it wants to run) is the device ABI of §8.5.8 (G.65).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0370` | an extern module with a body item that is not a param, with a result, or with a value input (§8.5.7) | elaboration |
| `E0371` | an ABI string other than `"osdi"` or `"cosim"` | elaboration |
| `E0372` | `#[link]` missing, or its file or model cannot be loaded | elaboration |
| `E0373` | ports or params that do not match the foreign descriptor (§8.5.10) | elaboration |
| `E0374` | a port kind the ABI does not allow (§8.5.11) | elaboration |
| `E0375` | a model item named `m` or `dtemp` (§8.5.2) | elaboration |
| §8.1 | `m <= 0.0` (`E0317`) | elaboration |
| §10 | a behavior block for an extern module (`E0402`) | elaboration |

**Examples**

```rust
#[link("bsim4.osdi", model = "bsim4")]
extern "osdi" module Bsim4(d: Electrical, g: Electrical, s: Electrical, b: Electrical) {
    l: f64 = 1u;
    w: f64 = 1u;
    vth0: f64 = 0.4;
}

#[link("libuart_rx.so")]
extern "cosim" module UartRxRtl(in clk: Bit, in rx: Bit, out byte: Bit<u8>, out dv: Bit) {}
```

---

## 9. Quantities

Every **quantity** a model uses is one of three: a param, a discrete carrier or a node.

| Quantity | Declared | Changes | In the matrix | Derivative |
| --- | --- | --- | --- | --- |
| param | `r: f64 = 1k;` | only by the host's `set` | no | `∂/∂p` when it reaches a contribution (§21) |
| discrete carrier | `signal r: Bit<f64> = 1k;` | at events | no: constant in every solve | none |
| node | `signal n: Electrical;` | continuously | yes | `ddt` |

A named expression and a value input are not quantities: they are expressions over these three.

**Static semantics**

- **§9 rule 1** An expression in `analog` is in the matrix exactly when it depends on a node, through a
  probe or a result.
- **§9 rule 2** Models never write params. A param is the target of no definition, commit or
  contribution (§8.2.10, §12.1.7).
- **§9 rule 5** Only an `f64` param that reaches a contribution, a probe-reading expression or an event
  time has a gradient. The dependency analysis is §21.

**Dynamic semantics**

- **§9 rule 3** A change of a discrete carrier that `analog` reads is a jump (§15.2). The carrier is
  constant during every solve on both sides of it.
- **§9 rule 4** A host **MAY** `set` a non-structural param during a run. The `set` is collected at a
  scheduled instant and applies after the step to that instant is accepted (§15 step 5). When
  `analog` reads the param, that instant is a jump (§15.2, "Host `set`").
  At it:
  1. every value computed from params is recomputed: bound params, constant `let`s, definitions
     that read the param, armed `at` times (§13 rule 4);
  2. signals and instance state are kept: an initial value `= expr` that read the param is not
     committed again;
  3. `p.changed()` fires, so a reaction can follow the change: `on vref.changed() { … }`.
- **§9 rule 6** Per kind of solve, a param and a discrete carrier are constant: their row of
  §16.2. A node is an unknown of every solve. In a jump solve its states under `ddt` are kept
  where the equations allow (§15.2.1); in a small-signal sweep it is the complex response.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| §5.2 | a definition or commit whose target is a param (`E0222`) | elaboration |
| §23.1 | a host `set` of a structural param, or one that changes the structure of an extern module, during a run (`E0803`) | host |

**Examples**

Four ways to vary a resistance, one per row of the table and one for time:

```rust
// A. Set from outside: a Bit input; each change is a jump.
module DigiPot(p: Electrical, n: Electrical, in r: Bit<f64>) { r0: f64 = 1k; }
analog for DigiPot { i(p, n) +> v(p, n) / r.known_or(r0); }

// B. A function of a node: in the matrix, Newton sees the dependence.
module VcResistor(p: Electrical, n: Electrical, ctl: Electrical) { r0: f64 = 1k; k: f64 = 0.1; }
analog for VcResistor { i(p, n) +> v(p, n) / (r0 * (1.0 + k * v(ctl))); }

// C. Updated after each time point: an explicit lag of one step. No time point of its own, no restart (§15 S3).
module LaggedResistor(p: Electrical, n: Electrical, ctl: Electrical) {
    r0: f64 = 1k; k: f64 = 0.1;
    signal r: Bit<f64> = r0;
}
digital for LaggedResistor { on settle { r <- r0 * (1.0 + k * v(ctl)); } }
analog for LaggedResistor { i(p, n) +> v(p, n) / r.known_or(r0); }

// D. A function of time: no Jacobian entry; the corner is a carrier change, so the solver lands on it.
module SweptResistor(p: Electrical, n: Electrical) {
    r0: f64 = 1k; r1: f64 = 10k; tsweep: f64 = 1u;
    signal done: Bit = false;
}
analog for SweptResistor {
    let x = if done.high() { 1.0 } else { now() / tsweep };
    i(p, n) +> v(p, n) / (r0 + (r1 - r0) * x);
    bound_step(tsweep / 50.0);
}
digital for SweptResistor { on at(tsweep) { done <- true; } }
```

| Case | Quantity varied | Time point at the change | Jump | Jacobian entry |
| --- | --- | --- | --- | --- |
| A | discrete carrier | yes | yes | none |
| B | node | — (continuous) | no | yes |
| C | discrete carrier, committed from `settle` | no: due at the next one | no | none |
| D | `now()`, with a carrier change at the corner | at the corner | at the corner | none |

In case C, a host `set` of `r0` during the run does not re-commit `r`'s initial value (§9 rule 4 step 2);
the next `settle` commit reads the new `r0`.
