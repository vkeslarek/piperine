# Part II. Carriers

A **carrier** exists over time and can be connected. Every port and every internal signal is a
carrier, and its type is a nature. A **signal** is a carrier declared in a module body with
`signal`; a port is a carrier declared in a port list
([§8.1](part_iii_structure.md#81-ports-and-params)).

Error codes of this part: `E02xx`.

**Terms defined here:** carrier, signal, nature, `Nature`, nature quantity, discrete nature,
continuous nature, compatible natures, discrete carrier, drive, one-driver carrier, resolved
carrier, change, node, branch, probe, `gnd`, signal-flow node, bundle, view.

## 4. Natures

**A nature says what its carriers hold and how the drivers of one carrier combine.**

**Syntax**

```
nature_decl    ::= attrs 'pub'? 'nature' IDENT generics? '{' nature_field ( ',' nature_field )* ','? '}'
nature_field   ::= attrs ( 'potential' | 'flow' ) ':' IDENT ( '{' ( IDENT ':' expr ',' )* '}' )?
                 | attrs 'value' ':' type
```

A nature takes trait `impl`s ([§3.1.9](part_i_values.md#31-impl)); `impl Resolve<X> for N` is the one
that lets a discrete carrier take several drives (rule 8). There is no `resolve` field and no
keyword `resolve`.

**Static semantics**

There are four kinds of nature. A **discrete nature** declares `value`. A **continuous
nature** declares `potential`.

|  | discrete | discrete that implements `Resolve` | continuous, with `flow` | continuous, no `flow` |
| --- | --- | --- | --- | --- |
| declared | `nature N { value: T }` | `nature N { value: T }` and `impl Resolve<X> for N` | `nature N { potential: …, flow: … }` | `nature N { potential: … }` |
| holds | a `T: Repr`, level by level | the same | a real potential and flow; complex in small-signal analyses | a real potential |
| drivers combine | one driver | `resolve([X; N]) -> T` over all `N` drives | flows sum to zero (KCL) | one driver |
| port direction | `in` or `out` | none | none | none |
| computed by | the event engine | the event engine | the analog solver | the analog solver |
| written by | `=`, `<-` | `=`, `<-` | `+>` | `+>` |
| past | `prev()` ([§5.3](#53-history)) | the same | located events, modules with state | the same |

- **§4 rule 1.** A discrete nature declares `value`. It may implement `Resolve` (rule 8). A continuous
  nature declares `potential` and optionally `flow`. Discrete and continuous carriers never
  connect ([§6.1](#61-crossing-between-kinds)).
- **§4 rule 2.** `resolve` is a function of the drives, which it receives as an array of plain
  values in the order of their instance paths. The result is a function of the source even
  when the function is not exactly associative, as a floating-point sum is not. It is called
  with an empty array for a carrier nobody drives. High impedance, strengths, wired logic and
  summing nets are value types and `impl Resolve` of natures that a model declares, never
  language features and never library natures
  ([§22.3](part_vii_stdlib.md#223-strengths-and-ready-made-net-modules)).
- **§4 rule 3.** `access` names the probe. It is an alias of the generic probes `potential`
  and `flow` ([§6](#6-continuous-carriers)), which read the fields of the same names. `units`
  is metadata, never checked. `abstol` is the tolerance of that quantity's unknowns and
  residuals ([§17.1](part_v_execution.md#171-convergence)), and the default `etol` of
  crossings ([§17.3](part_v_execution.md#173-locating-crossings)).
- **§4 rule 4.** Each `abstol` **SHOULD** be chosen below the smallest difference that
  matters. A user state is best normalized to order one (stdlib nature `State`,
  [§22.2](part_vii_stdlib.md#222-analog-functions-and-modules)).
- **§4 rule 5.** Quantity names are labels: two natures whose potentials share a name may
  connect. A high-voltage variant of `Electrical` is a second nature with
  `Voltage { abstol: 1m }`. There is no inheritance between natures.
- **§4 rule 6.** A nature declares each field at most once. `value` does not mix with
  `potential` and `flow`. `flow` requires `potential`. `Resolve` is implemented for a discrete
  nature only.
- **§4 rule 7.** The type of `value` **MUST** implement `Repr`
  ([§2](part_i_values.md#2-repr)).
- **§4 rule 8.** `Resolve` is the capability of taking several drives
  ([§3.7.13](part_i_values.md#37-traits)):

  ```rust
  pub trait Resolve<X: Repr, V: Repr = X> {
      fn resolve<const N: usize>(drives: [X; N]) -> V;
  }
  ```

  `X` is the type a module drives into the carrier; `V` **MUST** be the nature's `value` type,
  what a reader sees (`E0203`). An `impl` whose `V` is another type, or for a continuous
  nature, is `E0203`. The drives are an array, passed by value like any other
  ([§1.4.6](part_i_values.md#14-typing-inference-and-conversion)); elaboration instantiates
  the function with `N` the number of drives a call receives, zero included
  ([§8.2.3](part_iii_structure.md#82-the-body)). The function **MUST** be of class pure
  ([§3](part_i_values.md#3-functions)). A nature may implement `Resolve` for several `X`; a
  drive picks the `impl` by the type it drives ([§5.2.15](#52-drives)).

  ```rust
  pub nature AndWire { value: bool }
  impl Resolve<bool> for AndWire {
      fn resolve<const N: usize>(drives: [bool; N]) -> bool { drives.iter().all(|d| d) }
  }
  ```
- **§4 rule 9.** A **nature quantity** is the `IDENT` after `potential:` or `flow:` with its
  attribute block. The attributes are closed:

| Attribute | Type | Default when left out |
| --- | --- | --- |
| `access` | an identifier | none: only the generic probe reads the quantity |
| `units` | a string literal | `""` |
| `abstol` | a const `f64` expression, `> 0` | the simulator's default for a potential or a flow ([Appendix C.2](appendix_c_implementation.md)) |

- **§4 rule 10.** The two `access` names of one nature **MUST** differ. Different natures may
  use the same name. A call `v(a, b)` is resolved by the nature of `a`.
- **§4 rule 11.** **`Nature`** is the bound that every nature satisfies. A module or function
  generic over `N: Nature` is checked at each instantiation: a probe of a field that `N` does
  not declare is an error there.
- **§4 rule 12.** A nature may have generic parameters with defaults:
  `nature Bit<T: Repr = bool>`. `Bit` alone is `Bit<bool>`.
- **§4 rule 13.** Two discrete natures connect only when they are the same nature with the
  same arguments. Two continuous natures are **compatible natures** when their potential
  quantities have the same name and, if both declare `flow`, their flow quantities have the
  same name.

> **Resolution.** Rules 6–10 complete the declaration: required fields, the closed attribute
> set and its defaults, and how an `access` alias shared by several natures (`v` on
> `Electrical` and on `Servo`) is resolved.

> **Resolution.** Rule 11: the design uses `N: Nature` for both generic probes and generic
> state (`Short<N>`, `idt<S>`) and never restricts it. Every nature satisfies the bound, and
> misuse surfaces when the generic is instantiated.

> **Resolution.** Rule 13 adds the flow-name condition to rule 5. Without it, KCL at one node
> would sum flows of different quantities. A nature with `flow` and one without connect on
> the potential name alone; the node is then conservative ([§6](#6-continuous-carriers) rule 2).
> This narrows rule 5 ([G.49](appendix_g_scope.md)).

> **Resolution.** The order of rule 2 is the lexicographic order of the hierarchical paths of
> the instances that hold the drives, compared segment by segment: names by code point, array
> indices numerically. The host's drive of a top-level carrier comes first. An instance with
> no `let` of its own has the name [§21](part_vi_io.md#21-metadata) rule 3 gives it. The
> drives one instance holds on one net (two of its carriers connected to it) are ordered,
> after the instance path, by the declaration order of those carriers in the instance's
> module. *(AFK default 2026-11-24, pending user review.)*

**Dynamic semantics**

- **§4 rule 14.** A nature has no behavior of its own. What it decides is which engine
  computes its carriers and how drives combine (table above).
- **§4 rule 15.** `resolve` is called in the microstep in which any drive of the carrier
  changes ([§5.2](#52-drives)). It receives every present drive as a plain value of type `X`.
  A drive that relaxation has not yet produced is absent from what it receives
  ([§8.2.3](part_iii_structure.md#82-the-body)); absence is not a value of the type.
- **§4 rule 16.** In every analysis a discrete nature's carriers are constant during a solve,
  and a continuous nature's carriers are unknowns of the solve
  ([§16.2](part_v_execution.md#162-every-primitive-in-every-solve)).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0201` | a nature that repeats a field, mixes discrete and continuous fields, or lacks `value` or `potential` | elaboration |
| [§2](part_i_values.md#2-repr) | a `value` type that does not implement `Repr` (`E0112`) | elaboration |
| `E0203` | an `impl Resolve<X, V>` whose `V` is not the nature's `value` type, one for a continuous nature, or one whose `resolve` is not a pure function with one const parameter `N: usize` and the signature `fn([X; N]) -> V` | elaboration |
| `E0204` | a quantity attribute other than `access`, `units`, `abstol`; one of the wrong type; an `abstol` that is not positive | elaboration |
| `E0205` | the two `access` names of one nature are equal | elaboration |
| `E0206` | a probe that the nature of its first argument does not declare | elaboration |

**Examples**

```rust
pub nature Bit<T: Repr = bool> {
    value: T,
} // prelude; Bit is Bit<bool>
pub nature Sda { value: Driven }                                         // a model's own net (§22.3, Appendix F.4)
impl Resolve<Driven> for Sda {
    fn resolve<const N: usize>(drives: [Driven; N]) -> Driven { Driven::strongest(drives) }
}
pub nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow: Current { access: i, units: "A", abstol: 1p },
}

pub nature Thermal {
    potential: Temperature { access: temp, units: "K", abstol: 1m },
    flow: Power { access: pwr, units: "W", abstol: 1n },
}

pub nature VSignal {
    potential: Voltage { access: v, units: "V", abstol: 1u },
} // signal-flow
```

## 5. Discrete carriers

A **discrete carrier** is a carrier of a discrete nature. It holds a value of its nature's
type and changes only at events. `Bit<T>` is the common one.

**Syntax**

```
signal_decl    ::= 'signal' IDENT ':' sigtype ( '=' expr )? ';'
sigtype        ::= 'flip'? path generic_args? | '[' sigtype ';' expr ']'
```

### 5.1 Declaring and reading

**Static semantics**

- **§5.1.4** `signal x: N;` declares one carrier of nature `N`. `signal x: [N; K];` declares
  `K` carriers; `K` is a const expression.
- **§5.1.5** The initial value `= expr` has the nature's `value` type, or for an array an
  array of it. It is an expression over params and constants; it reads no carrier and no
  probe. A carrier with no initial value starts at `Default::default()` of its value type
  ([§3.3.4](part_i_values.md#33-uncertainty-and-absence)); a type without `Default` requires
  the initial value (`E0187`).
- **§5.1.6** Where a discrete carrier may be read is the placement matrix
  ([Appendix B.2](appendix_b_grammar.md)). Inside `analog` a read is an ordinary expression
  ([§3.3.6](part_i_values.md#33-uncertainty-and-absence)).
- **§5.1.7** `changed`, `prev` and `others` apply to one carrier. On an array of carriers
  they are written on an element: `lines[k].changed()`.

> **Resolution.** Rule 5: an initial value is committed at `(0, 0)` before any solve, so it
> can depend on nothing that exists over time. Rule 7 keeps the carrier methods per carrier;
> the design never applies one to an array.

**Dynamic semantics**

- **§5.1.1** Reading a carrier yields its value
  ([§3.3](part_i_values.md#33-uncertainty-and-absence)). The carrier has a value from `(0, 0)`
  on: the initial value is committed there, `= value` or `Default::default()`
  ([§16](part_v_execution.md#16-the-operating-point)).
- **§5.1.2** A discrete carrier is constant during every Newton solve. It has no derivative
  and no small-signal response around an operating point
  ([§11.1](part_iv_behavior.md#111-one-block-every-analysis)). What a commit of a real value
  carries along a trajectory is [§23.3.4](part_viii_host.md#233-gradients-across-events).
- **§5.1.3** An array of carriers reads as an array value. Storage and history of an element
  exist from its first commit, so `[Bit<u8>; 1 << 32]` costs what is written.
- **§5.1.8** An element that was never committed reads as the array's initial value for that
  element, and its `prev()` is the same value. An implementation **MUST NOT** require storage
  proportional to the declared length.
- **§5.1.9** The initial expression is evaluated with the param values in force when the
  analysis starts. A later `set` does not commit it again
  ([§9](part_iii_structure.md#9-quantities) rule 4).
- **§5.1.10** When a carrier is read, and what value a reader sees within an instant, is
  [§15](part_v_execution.md#15-time-and-the-instant): a reaction sees the values settled at its
  microstep; `analog` sees them constant for the whole solve.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0210` | an initial value of a signal or node of the wrong type, or one that reads a carrier or a probe | elaboration |
| `E0211` | a carrier method applied to an array of carriers or to a bundle | elaboration |
| [§3.7](part_i_values.md#37-traits) | a signal, an `out` port or a drive with no initial value whose value type does not implement `Default` and whose driver gives none (`E0187`, naming the carrier and `Default`; §3.3.4) | elaboration |

**Examples**

```rust,fragment
signal en:    Bit = false;               // a bool has no default: the initial value is written
signal last:  Bit<Option<u8>>;           // starts at None: Option has a default
signal count: Bit<u8> = 0;
signal level: Bit<f64> = 0.0;            // real-number modeling: piecewise constant
signal st:    Bit<Tx> = Tx::Idle;
signal lines: [Bit; 8] = [false; 8];     // eight carriers, each with its own events
signal bus:   Bit<Logic<4>> = logic("10xx");  // four-state, level by level; starts xxxx without `= ..`
signal sda:   Sda;                       // resolved: any number of drivers, no initial value of its own
```

### 5.2 Drives

A **drive** is what one module writes to one discrete carrier. Each module that writes a
carrier holds one drive of it: either one `=` definition
([§8.2](part_iii_structure.md#82-the-body)) or the commits of its reactions
([§12.1](part_iv_behavior.md#121-reactions)), never both. The carrier's value is what its
nature makes of the drives.

A **one-driver carrier** is a carrier of a discrete nature that does not implement `Resolve`.
A **resolved carrier** is a carrier of a discrete nature that does
([§4](#4-natures) rule 8).

**Static semantics**

- **§5.2.1 One driver.** One module holds the drive: the one that declares the carrier, or
  the one that has it as an `out` port. A second drive is `E0220`, and its diagnostic names
  the nature and `Resolve`.
- **§5.2.2 Resolved.** Any module that sees the carrier may hold a drive, and the carrier
  takes any number of them, zero included. Its ports take no direction.
- **§5.2.3** `= value` on a signal or port is the first commit of the declaring module's
  drive ([§16](part_v_execution.md#16-the-operating-point) step 2). On a resolved signal it is
  a drive of type `X`. A carrier's initial value is given once, by its driver. For a top-level
  `in` port the driver is the host ([§23.1](part_viii_host.md#231-testbenches)). A resolved
  carrier with no drive of the declaring module needs no initial value of its own: its value
  is `resolve` of its drives.
- **§5.2.4** A module **MUST NOT** hold both a definition and commits for one carrier. An
  element of an array of carriers is one carrier for this rule.
- **§5.2.5** A module never writes a carrier it has as an `in` port.
- **§5.2.6** When a module passes a one-driver carrier it declares to an `out` port of an
  instance, the instance holds the drive. The declaring module then holds none, and its
  `signal` **MUST NOT** have an initial value. An array of carriers is judged per element, as
  §8.2.21 does for definitions: the instance holds the drive of the elements handed to it only,
  so an initial value is the value of the elements no `out` port holds, and is `E0223` only once
  every element is handed. *(AFK default 2026-11-24, pending user review.)*
- **§5.2.7** `s.others()` is defined on a resolved carrier only. It gives a `V`.
- **§5.2.8** A one-driver carrier with no drive is allowed. It holds its initial value in every
  analysis; its value type implements `Default` or it is `E0187`.

> **Resolution.** Rule 6 states who gives the initial value when the declaring module is not
> the driver: the `out` port does (`signal up: Bit;` with `out up: Bit = false`). Rule 8
> follows from "a module that never writes a carrier has no drive of it".

**Dynamic semantics**

- **§5.2.9** On a one-driver carrier the value is the drive.
- **§5.2.10** On a resolved carrier the value is always `resolve` of its present drives,
  recomputed in the microstep any drive changes, also when there is one or none. Readers,
  `changed()` and history see the resolved value.
- **§5.2.11** `s.others()` reads `resolve` of every drive but this module's own. This is how
  a bidirectional element looks through itself
  ([§22.3](part_vii_stdlib.md#223-resolved-nets)). In a module that holds no drive of `s`, it
  equals the value of `s`.
- **§5.2.12** A drive with no initial value starts at `Default::default()` of the driven type;
  a type without it is `E0187` (§5.1 errors). It is a drive from `(0, 0)` on, and `resolve`
  receives it. A module that never writes a carrier has no drive of it.
- **§5.2.13** A commit to some levels of a drive leaves its other levels as they were
  ([§12.1](part_iv_behavior.md#121-reactions)).
- **§5.2.14** In which microstep a drive takes a new value, and how commits to one level are
  ordered, is [§12.1](part_iv_behavior.md#121-reactions) and
  [§15](part_v_execution.md#15-time-and-the-instant). Where time does not advance, drives
  behave as [§16.1](part_v_execution.md#161-analyses-without-time) states.
- **§5.2.15** A drive picks its `impl` of `Resolve` by the type of the value it drives. Every
  drive of one carrier has the same `X`: two driven types on one carrier are `E0225`, naming
  both modules and both types. A drive whose type no `impl` accepts is `E0180`, naming the
  nature and the driven type; a literal that several `impl`s accept is `E0181`.

On a node, the counterpart of a drive is the module's branches ([§6](#6-continuous-carriers)).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0220` | a second drive of a one-driver carrier; the diagnostic names the nature and `Resolve` | elaboration |
| `E0221` | a definition and a commit to the same carrier in one module | elaboration |
| `E0222` | a definition or commit whose target the module may not drive ([§8.2.10](part_iii_structure.md#82-the-body)): an `in` port, a carrier it neither declares nor has as a port, a child's one-driver carrier, or anything that is not a discrete carrier | elaboration |
| `E0223` | an initial value given by a module that does not hold the drive, or given twice: for an array, over every element handed to `out` ports | elaboration |
| `E0224` | `others()` on a one-driver carrier | elaboration |
| `E0225` | two drives of one resolved carrier whose driven types differ | elaboration |

### 5.3 History

Every discrete carrier, and every param, keeps one previous value.

| Method | Gives |
| --- | --- |
| `s.changed()` | `Event`: `s` changed in this microstep |
| `s.prev()` | the value before the most recent change (on a resolved carrier, the previous settled value); before any change after `(0, 0)`, the initial value of a carrier and the param's own value on a param |

**Static semantics**

- **§5.3.1** `changed()` and `prev()` apply to a discrete carrier and to a param. On a param,
  a change is a host `set` ([§9](part_iii_structure.md#9-quantities)).
- **§5.3.2** A node has no `prev()`: a value one step ago would depend on the step size. Its
  past is reached through located events and modules with state.
- **§5.3.3** Any longer memory is a signal written by a reaction. The time of the last change
  is the stdlib `since(s)` ([§22.1](part_vii_stdlib.md#221-values-and-bit)).

**Dynamic semantics**

- **§5.3.4** A carrier undergoes a **change** in a microstep when its value there differs from
  its value in the previous microstep in at least one level.
- **§5.3.5** A commit of the value the carrier already holds is not a change.
- **§5.3.6** A `Bit<f64>` changes when its levels change: `0.0` to `-0.0` is a change, the
  same NaN is not ([G.9](appendix_g_scope.md)).
- **§5.3.7** `prev()` is the whole value of the carrier before its most recent change. It is
  constant between changes.
- **§5.3.8** The initial value, written or `Default::default()`, is committed at `(0, 0)` and
  that commit is a change ([§13](part_iv_behavior.md#13-events) rule 6). Before any later
  change `prev()` equals the initial value, as a param's does.
- **§5.3.9** On a resolved carrier, a change and `prev()` refer to the resolved value after
  the microstep's structure has settled
  ([§8.2.3](part_iii_structure.md#82-the-body)). Intermediate relaxation passes are not
  changes.
- **§5.3.10** No change happens in a small-signal sweep
  ([§16.2](part_v_execution.md#162-every-primitive-in-every-solve), second table).
- **§5.3.11** A param has a value from elaboration and no first commit: binding it, at elaboration or
  by a `set` made before the run, is not a change
  ([§13](part_iv_behavior.md#13-events) rule 16). Before the first `set` during a run,
  `p.prev()` equals `p`.

> **Resolution.** Rules 4, 5 and 7 define "changed" level by level, as the stdlib's `rise` and
> `fall` assume: a commit of the value a carrier holds is not a change, and a carrier that
> starts high has not risen at `(0, 0)`. Whether one previous value is enough stays open
> ([G.19](appendix_g_scope.md)).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0230` | `prev()` or `changed()` on a node | elaboration |

## 6. Continuous carriers

A **node** is a carrier of a continuous nature; the solver computes its value. A **branch** is
a pair of nodes of one module; the module contributes to it and probes it
([§11](part_iv_behavior.md#11-analog)). A **probe** is a function that reads the potential or
the flow of a branch.

**Syntax.** A node is declared by `signal_decl` ([§5](#5-discrete-carriers)) or as a port.

| Probe | Reads |
| --- | --- |
| `potential(a, b)`, `flow(a, b)` | the generic probes, for code generic over a nature |
| `v(a, b)`, `v(a)` | the nature's alias of `potential`; `b` is an `Option`, `gnd` when left out |
| `i(a, b)`, `i(a)` | the alias of `flow`, from `a` to `b` through the module |
| `p.inflow()` | flow entering the module at its port `p` (VAMS `I(<p>)`) |

**Static semantics**

- **§6 rule 1.** **`gnd`** is the reference node of every continuous nature. It is not a
  global net: a model reaches any other node only through its ports.
- **§6 rule 2.** A **signal-flow node** is a node no nature of which declares `flow`. It has
  exactly one potential driver, checked at elaboration.
- **§6 rule 3.** A module has one branch between two given nodes: every probe and contribution
  on `(a, b)` is that branch, and `(b, a)` is the same branch reversed. A second, independent
  branch between the same nodes goes through an internal node.
- **§6 rule 4.** Every probe has the signature
  `fn potential<N: Nature>(a: N, b: Option<N>) -> f64`.
- **§6 rule 5.** `i(p)` is the flow of this module's branch `(p, gnd)`. `p.inflow()` is the
  total flow of all this module's branches at `p`.
- **§6 rule 6.** The initial value `= expr` of a node is an `f64` expression over params and
  constants. Its meaning is [§16](part_v_execution.md#16-the-operating-point). A port of a
  continuous nature takes no `= expr` ([§8.1.7](part_iii_structure.md#81-ports-and-params)).
- **§6 rule 7.** A module probes and contributes only on its own nodes: its ports, its
  signals, `gnd`, and the `pub` nodes of its instances
  ([§8.2](part_iii_structure.md#82-the-body)). Where probes may be written is
  [Appendix B.2](appendix_b_grammar.md).
- **§6 rule 8.** Both nodes of a branch have the same nature, the one `N` of rule 4. `gnd`
  takes the nature of the other node. Compatible natures meet only through a connection
  ([§6.1.3](#61-crossing-between-kinds)).
- **§6 rule 9.** A branch is identified by the names of its nodes in the module, not by the
  nets they are connected to. A contribution to `(a, a)` is an error; a probe of it reads 0.
- **§6 rule 10.** `inflow()` applies to a port of a nature with `flow`. `flow` and its alias
  apply to a nature with `flow`.
- **§6 rule 11.** `gnd` is a core name (§14 rule 1). It may be passed as a connection and named in a
  branch. It has no initial value, no flow probe of its own and no `inflow()`.

> **Resolution.** `gnd`, `Nature` and `str` are core names: the language provides them and no declaration in the language can define them (`E0457`, §14 rule 1). The prelude (§3.4.2) holds the standard library and is replaceable (§22).

> **Resolution.** Rule 2 counts a node as signal-flow only when every nature meeting there
> lacks `flow`. Its one potential driver is one module that contributes potential to
> `(n, gnd)`; a flow contribution to such a node is `E0413`
> ([§11](part_iv_behavior.md#11-analog) rule 11).

> **Resolution.** Rule 9: two ports of a module may be connected to one net by the parent
> (`Resistor(x, x)`). The branch still exists; a potential contribution on it then fails as a
> singular solve naming the branch, not at elaboration.

> **Resolution.** "This module's branches" in rule 5 includes the branches of the module's
> instances that end on `p`. This makes `p.inflow()` the flow entering the module, as the
> table and VAMS `I(<p>)` state ([G.59](appendix_g_scope.md)).

**Dynamic semantics**

- **§6 rule 12.** `potential(a, b)` is the potential of `a` minus that of `b`. The potential
  of `gnd` is 0. `potential(b, a) == -potential(a, b)`.
- **§6 rule 13.** `flow(a, b)` is the flow from `a` to `b` through this module's branch
  `(a, b)`: the sum of its flow contributions, or the branch unknown when it has potential
  contributions. A branch with no contribution in a solve has zero flow
  ([§11](part_iv_behavior.md#11-analog) rule 4). `flow(b, a) == -flow(a, b)`.
- **§6 rule 14.** What a probe returns in each kind of solve is the first row of
  [§16.2](part_v_execution.md#162-every-primitive-in-every-solve).
- **§6 rule 15.** In a reaction a probe reads the solution at the instant its event fired
  ([§12.1.5](part_iv_behavior.md#121-reactions)). In an event expression it is read by
  the solver ([§13](part_iv_behavior.md#13-events)).
- **§6 rule 16.** A probe is a plain `f64` ([§3.3.1](part_i_values.md#33-uncertainty-and-absence)).
- **§6 rule 17.** No probe is scaled by a multiplicity: the run's multiplicity is read by the model
  ([§8.5](part_iii_structure.md#85-run-settings-and-extern-modules)).
- **§6 rule 18.** A node's value is continuous in time except at a jump, where it has a left
  and a right value ([§15.2](part_v_execution.md#152-the-jump)).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0240` | a probe or contribution whose two nodes have different natures | elaboration |
| `E0241` | a contribution to a branch `(a, a)` | elaboration |
| `E0242` | a signal-flow node with no potential driver, or with more than one | elaboration |
| `E0243` | `inflow()` on something that is not a port of a nature with `flow` | elaboration |
| `E0244` | a probe or contribution on a node the module cannot reach under rule 7 | elaboration |
| [§5.1](#51-declaring-and-reading) | a node initial value of the wrong type, or one that reads a carrier or a probe (`E0210`) | elaboration |

**Examples**

```rust,fragment
signal mid:  Electrical;          // internal node
signal vout: Electrical = 0.0;    // where Newton starts; held there through a transient's operating point (§16)
```

### 6.1 Crossing between kinds

A discrete carrier is never connected to a node, and no converter is inserted implicitly.

**Static semantics**

- **§6.1.1** Crossing is a module: the stdlib `D2A`, `A2D` and `Bidir`
  ([§22.4](part_vii_stdlib.md#224-converters)).
- **§6.1.2** Inside a module, behavior crosses freely: `analog` reads discrete carriers
  ([§3.3.6](part_i_values.md#33-uncertainty-and-absence)), and events and reactions probe.
- **§6.1.3** A carrier connects to a port only when their natures are the same discrete
  nature or compatible continuous natures ([§4](#4-natures) rule 13).

**Dynamic semantics**

- **§6.1.4** A discrete carrier that `analog` reads is a constant of each solve. Its change
  is a jump ([§15.2](part_v_execution.md#152-the-jump)).
- **§6.1.5** A reaction that probes makes its instant a time point
  ([§17.2](part_v_execution.md#172-steps)).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0250` | a discrete carrier connected to a port of a continuous nature, or the reverse | elaboration |
| `E0251` | a carrier connected to a port of a different discrete nature, or of an incompatible continuous nature | elaboration |

## 7. Bundles

A `struct` whose fields are carriers is a **bundle**: a carrier type, not a value. Fields of a
one-driver nature have directions, written from the producer's side; other fields have none. A
bundle has no behavior with time, and may have functions ([§3.1](part_i_values.md#31-impl)).

**Syntax**

```
struct_field   ::= attrs 'pub'? dir? IDENT ':' ( type | sigtype )               // carrier fields make a bundle
dir            ::= 'in' | 'out'
sigtype        ::= 'flip'? path generic_args? | '[' sigtype ';' expr ']'
```

**Static semantics**

A **view** is the way one module sees a bundle: which of its one-driver fields it drives.

| Use | Written |
| --- | --- |
| producer view | `tx: Stream<T>` |
| consumer view, every direction reversed | `rx: flip Stream<T>` |
| observer view, every field an input | `in s: Stream<T>` |
| internal bundle | `signal a: Stream<T>;` |
| connection | a bundle carrier, or a literal `Cell { wl: wl[r], bl: bl[c] }` |

- **§7 rule 1.** Every field of a bundle is a carrier, an array of carriers, or a bundle. A
  `struct` that mixes carrier fields and value fields is an error.
- **§7 rule 2.** A field of a one-driver nature (a discrete nature that does not implement
  `Resolve`) **MUST** have `in` or `out`. A field of any other nature, a resolved one
  included, **MUST NOT** have a direction.
- **§7 rule 3.** In the producer view the module drives the `out` fields and reads the `in`
  fields. `flip` swaps `in` and `out` on every one-driver field, nested bundles included. The
  observer view makes every one-driver field an input.
- **§7 rule 4.** Fields without a direction are the same in every view.
- **§7 rule 5.** A field whose type is a bundle is written plain, or with `flip` to reverse
  it. It takes no `in` and no `out`.
- **§7 rule 6.** `flip` applies to a bundle type only. `out` on a bundle port is an error.
- **§7 rule 7.** `signal a: Stream<T>;` declares every carrier of the bundle. The declaring
  module drives none of its one-driver fields unless it writes them; it connects a producer
  to a consumer.
- **§7 rule 8.** A bundle literal names every field once. Each field is given a carrier of
  that field's nature, or a nested literal. The literal is written only as a connection.
- **§7 rule 9.** A bundle is not a value. It **MUST NOT** be a param type, the type argument
  of a nature, a function result, or the operand of a value operator. It does not implement
  `Repr`.
- **§7 rule 10.** A bundle signal or port has no initial value. A field's initial value comes
  from its driver ([§5.2.3](#52-drives)), or from the field's own declaration: an `in` or `out`
  carrier field may carry `= value` after its type, `out valid: Bit = false`,
  `in ready: Bit = false`. It is the initial value of the carrier the field becomes on the side
  that drives it: the producer's port leaf for an `out` field, the `flip` consumer's port leaf
  for an `in` field, and a bundle signal's leaf for either (the signal carries the field whoever
  drives it). The leaf on the reading side is an input and takes none. The value is the field
  declaration's, not the port's, so it is no initial value on a bundle port. A field of a value
  `struct`, a nested bundle field (its own fields carry theirs) and a field with no direction
  take none (`E0261`). A side that commits a `Bit` field it drives needs one, as it needs one on
  any `out` port of a type with no `Default` (`E0187`, whose help names the field with the
  direction its `struct` writes). *(AFK default 2026-11-24, pending user review.)*
- **§7 rule 11.** A bundle may be generic, and may be an array element:
  `[flip Stream<T>; N]`.
- **§7 rule 12.** Fields are reached with `.`: `tx.valid`, `c.bl`. Each field is a carrier
  with the rules of [§5](#5-discrete-carriers) or [§6](#6-continuous-carriers). Carrier
  methods apply to a field, not to the bundle.
- **§7 rule 13.** A function takes a bundle as a handle
  ([§3.1](part_i_values.md#31-impl)). Through a handle every field is read-only, whatever
  the view.

> **Resolution.** Rules 1, 5, 6, 8 and 10 close the cases the use table leaves open: mixed
> structs, directions on nested bundles, what a literal's fields may be, and initial values.
> A bundle that needs extra ports is an open item ([G.4](appendix_g_scope.md)).

**Dynamic semantics**

- **§7 rule 14.** A bundle has no semantics of its own in any analysis. Each field behaves as
  the carrier it is. Passing a bundle to a port connects each field to the field of the same
  name.
- **§7 rule 15.** The one-driver rule ([§5.2.1](#52-drives)) is checked field by field after
  views are applied: each one-driver field has at most one module that sees it as `out`.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0260` | a `struct` that mixes carrier fields and value fields | elaboration |
| `E0261` | a one-driver field without a direction, or a direction on any other field; an initial value on a field that is no `in` or `out` carrier of a bundle | elaboration |
| `E0262` | `flip` on something that is not a bundle; `out` on a bundle port; `in` or `out` on a nested bundle field | elaboration |
| `E0263` | a bundle literal with a missing, repeated or unknown field, or a field that is not a carrier of the field's nature | elaboration |
| `E0264` | a bundle used as a value, or given an initial value | elaboration |

**Examples**

```rust
struct Stream<T: Repr> { out valid: Bit, in ready: Bit, out data: Bit<T> }
struct Diff { p: Electrical, n: Electrical }
struct Cell { in wl: Bit, bl: Electrical }

impl<T> Stream<T> {
    pub fn beat(self, clk: Bit) -> Event { clk.rise() && self.valid && self.ready }
}
impl Diff {
    pub fn vd(self) -> f64 { v(self.p, self.n) }
}
```
