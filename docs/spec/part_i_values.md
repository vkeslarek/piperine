# Part I. Values

A **value** is data with no time. Everything a model computes with is a value; what it connects
to is a carrier ([Part II](part_ii_carriers.md)). The value world follows Rust's
([§0.4](index.md#04-inheritance-from-rust)), with the differences stated here.

Error codes of this part: `E01xx`.

**Terms defined here:** value, value type, level, `Repr`, function class, closure, handle,
known, unknown, known flag, merge, flag-total function, panic, optional parameters, namespace
module, prelude, crate, crate root, dependency name, package, manifest, dependency,
lockfile.

## 1. Value types

A **value type** is one of the kinds below. Only a type that implements `Repr`
([§2](#2-repr)) can be carried.

| Kind | Types | Notes |
| --- | --- | --- |
| boolean | `bool` |  |
| integer | `UInt<N>`, `Int<N>` (`u1` … `u128`, `i1` … `i128`) | any width, a const expression (`UInt<{A + 1}>`); arithmetic wraps |
| float | `Float<N>`, `N` in {16, 32, 64, 128} (`f16` … `f128`) | IEEE 754; analog code computes in `f64` |
| string | `&'static str` | literals only; never carried |
| composite | `[T; N]`, tuples, `struct`, `enum`, `Option<T>` | as in Rust |

**Static semantics**

- **§1 rule 1.** `as` truncates or extends, as in Rust. `checked_*` and `saturating_*` work as
  in Rust. The methods a model may call are those of [Appendix H](appendix_h_library.md).
- **§1 rule 2.** These are left out, because a model is fixed at elaboration: mutable
  references, heap types, `unsafe`, threads, `async`, `dyn`. `&` appears only as a shared,
  read-only borrow: of an array, slice or string in a function parameter, and of any value as
  a call argument (`(0.1..=10.0).contains(&n)`).
- **§1 rule 3.** Constants are `const` items. The prelude has `Q`, `K_B`, `EPS0`, `PI`, `E`
  and `inf` (table below).
- **§1 rule 4.** Only a type with `Repr` can be carried: it is the `T` of `Bit<T>` and the
  `value` of a nature ([§4](part_ii_carriers.md#4-natures)).
- **§1 rule 5.** `uN` and `iN` are aliases of `UInt<N>` and `Int<N>` for `N` in 1..=128. `fN`
  is an alias of `Float<N>`. The width of `UInt<N>` and `Int<N>` is any const expression
  `N >= 1`.
- **§1 rule 6.** Integer arithmetic wraps modulo `2^N` in every build. `Int<N>` is two's
  complement. There is no overflow error.
- **§1 rule 7.** An integer literal has an integer type and a float literal a float type, as
  in Rust. Neither converts implicitly ([§3.2.3](#32-parameters-and-calls)). A literal out of
  its type's range is `E0909` ([A.2](appendix_a_lexical.md#a2-si-suffixed-reals) rule 8).
- **§1 rule 8.** A string exists only as a literal, a `const`, or an argument. It has no
  `Repr`.
- **§1 rule 9.** `Event` ([§13](part_iv_behavior.md#13-events)), `Analysis`
  ([§11.1](part_iv_behavior.md#111-one-block-every-analysis)), closures
  ([§3](#3-functions) rule 3) and templates
  ([§8.4](part_iii_structure.md#84-templates-and-swappable-modules)) are values without
  `Repr`.

> **Resolution.** `usize` and `isize` are aliases of `UInt<64>` and `Int<64>` on every
> platform. A result may not depend on the machine that computes it
> ([§0.1.1](index.md#01-scope); [G.62](appendix_g_scope.md)).

> **Resolution.** `char` is not a value type: the table above is closed
> ([G.61](appendix_g_scope.md)). `String`, `Vec`, `Box` and every other heap type are covered
> by rule 2.

**Prelude constants**

| Name | Type | Value | Meaning |
| --- | --- | --- | --- |
| `Q` | `f64` | `1.602176634e-19` | elementary charge, C |
| `K_B` | `f64` | `1.380649e-23` | Boltzmann constant, J/K |
| `EPS0` | `f64` | `8.8541878128e-12` | vacuum permittivity, F/m |
| `PI` | `f64` | Rust's `f64::consts::PI` | π |
| `E` | `f64` | Rust's `f64::consts::E` | Euler's number |
| `inf` | `f64` | `f64::INFINITY` | positive infinity |

> **Resolution.** The design names the constants and not their values. `Q` and `K_B` are the
> exact SI values of 2019; `EPS0` is the CODATA 2018 value.

**Dynamic semantics**

- **§1 rule 10.** A value has no time. An expression over values is evaluated where it is
  written: at elaboration for a `const`, a param default and generation
  ([§8.2](part_iii_structure.md#82-the-body)); at an instant in a reaction; at every iterate
  in `analog`.
- **§1 rule 11.** A value-level failure is a **panic**: an out-of-range index, an integer
  division by zero, `unwrap` on `None`, an explicit `panic` path of a standard method. During
  elaboration it is an elaboration error. During a run it fails the analysis, naming the
  instance and the instant.
- **§1 rule 12.** A float division by zero does not panic. It yields an infinity or a NaN, as
  IEEE 754 defines. A non-finite value that reaches a solve fails it
  ([§17.1](part_v_execution.md#171-convergence)).
- **§1 rule 13.** A panic on a path taken under unknown control ([§3.3.3](#33-unknown-values))
  is not an error. Every effect of that merge becomes unknown.
- **§1 rule 14.** A plain identifier pattern, with no `mut`, `ref` or `@`, whose name resolves
  to a constant, a unit variant or a unit struct is a path to that item and binds nothing.
  Any other identifier pattern binds a new name, as in Rust.

> **Resolution.** Rule 14 is Rust's rule. `match o { None => 0, Some(x) => x }` tests for
> `None`; it does not bind a name `None`. A name that resolves to a local or a `fn` is a
> binding that shadows it.

> **Resolution.** Rules 11–13 are not in the design, which is silent on panics. Rule 13
> follows the unknown rule: whether the failing path runs is not known, so nothing it could
> have written is known. An unknown index selects only among existing elements
> ([§3.3.3](#33-unknown-values)), so it never panics.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0101` | a syntactic form left out by rule 2: a mutable reference, `unsafe`, `async`, `dyn`. A heap type or a thread API is a path that does not resolve (`E0161`) | parse |
| `E0102` | `&` other than a shared borrow of an array, slice or string in a function parameter, or of any value as a call argument | elaboration |
| `E0103` | an integer width that is not a const expression `>= 1`, or a float width outside {16, 32, 64, 128} | elaboration |
| `E0104` | a panic while evaluating a const expression, a param default, a `where` or generation | elaboration |
| `E0105` | a panic under known control during a run | run |

## 2. `Repr`

**`Repr`** lays a value type out on levels. It is the only bridge from values to carriers. A
**level** is one boolean position of that layout; each level has its own known flag
([§3.3](#33-unknown-values)).

```rust
pub trait Repr: Copy {
    const WIDTH: u32;                                   // levels occupied
    fn encode(self) -> [bool; Self::WIDTH];
    fn decode(levels: [bool; Self::WIDTH]) -> Option<Self>;
}
```

**Static semantics**

- **§2 rule 1.** The built-in layouts are those of the table below. Level 0 is the lowest.
- **§2 rule 2.** `#[derive(Repr)]` on a struct concatenates the fields, first field lowest. On
  an enum it lays out a tag, then the widest payload. `#[repr(uN)]` and explicit discriminants
  choose the tag. Any other encoding (one-hot, Gray) is a hand-written `impl Repr`.
- **§2 rule 3.** `#[derive(Repr)]` requires every field of the type to implement `Repr`.
- **§2 rule 4.** A hand-written `encode` and `decode` are ordinary functions. For every value
  `x`, `decode(encode(x))` **MUST** be `Some(x)`.

| Type | `WIDTH` | Layout |
| --- | --- | --- |
| `bool` | 1 | the value |
| `UInt<N>`, `Int<N>` | `N` | the bit pattern, least significant bit at level 0 |
| `Float<N>` | `N` | the IEEE 754 bit pattern, least significant bit at level 0 |
| `[T; N]` | `N · T::WIDTH` | elements concatenated, element 0 lowest |
| tuple | sum of the fields | fields concatenated, field `.0` lowest |
| `Option<T>` | `1 + T::WIDTH` | level 0 is the tag, true for `Some`; then `T` |
| derived struct | sum of the fields | fields concatenated, first field lowest |
| derived enum | tag width + widest payload | the tag lowest, then the payload of the variant |

- **§2 rule 5.** The tag of a derived enum without `#[repr(uN)]` has `ceil(log2(V))` levels for
  `V` variants, and zero levels for one variant. Discriminants count from 0 in declaration
  order unless written. With `#[repr(uN)]` the tag has `N` levels.
- **§2 rule 6.** A variant's payload is its fields concatenated, first field lowest. A payload
  narrower than the widest is padded with false levels by `encode`; `decode` ignores the
  padding. The payload of `None` is all false.
- **§2 rule 7.** For known values, behavior never depends on the encoding, except through
  `level`, `encode`, `decode` and `cast`. The encoding decides which levels a merge can keep
  ([§3.3.3](#33-unknown-values)).

> **Resolution.** Rules 5–6, and the element order of arrays and tuples, complete the layout
> the design gives by example (`Tx`: a 2-level tag and a 3-level payload).

**Dynamic semantics**

- **§2 rule 8.** The built-in and derived `encode` and `decode` move levels with their flags
  ([§3.3.2](#33-unknown-values)).
- **§2 rule 9.** A derived `decode` returns `None` for a tag that names no variant. A `None`
  from any `decode` reads as unknown: every level of the value is unknown
  ([§3.3.1](#33-unknown-values)).
- **§2 rule 10.** A hand-written `encode` or `decode` propagates flags through its body by the
  ordinary rules of [§3.3](#33-unknown-values). It has no rule of its own.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0110` | `#[derive(Repr)]` on a type with a field that does not implement `Repr` | elaboration |
| `E0111` | `#[repr(uN)]` with a discriminant that does not fit `N` levels, or two equal discriminants | elaboration |
| `E0112` | a type without `Repr` where a carried type is required | elaboration |

**Examples**

```rust
#[derive(Repr, Clone, Copy, PartialEq)]
enum Tx { Idle, Start, Data { n: u3 }, Stop }      // 2-level tag + 3-level payload

#[derive(Repr, Clone, Copy, PartialEq)] #[repr(u2)]
enum Op { Add = 0b00, Sub = 0b01, And = 0b10, Or = 0b11 }

#[derive(Repr, Clone, Copy, PartialEq)]
enum Pam4 { L0, L1, L2, L3 }                        // a PAM4 symbol is a value; its voltage is analog
```

## 3. Functions

A function is computation without time: it keeps no state, writes no carrier and places no
instance. What its body touches decides where it may be called.

**Syntax**

```
fn_decl        ::= 'const'? ( 'extern' STRING )? 'fn' IDENT generics? '(' ( fn_param ',' )* ')' ( '->' type )? ( 'where' expr )? ( block | ';' )
fn_param       ::= 'self' | IDENT ':' type
```

**Static semantics**

A **function class** is a property of a function, inferred from its body and from the classes
of the functions it calls. A function may belong to several classes; the restrictions add up.

| Class | The body… | Callable in |
| --- | --- | --- |
| pure | uses values, reads discrete carriers and the environment that is constant during an analysis ([§14](part_iv_behavior.md#14-core-primitives)) | anywhere |
| solve-reading | probes (`v(a)`, `i(a, b)`) or reads the environment of the solve (`now()`, `freq()`, `analysis(Static)`) | `analog`, event expressions, reaction bodies, body `let`s, value inputs of instances |
| event | returns `Event` ([§13](part_iv_behavior.md#13-events)) | event positions; as `bool` inside a reaction |
| emitting | calls `emit!` ([§19](part_vi_io.md#19-streams-and-checks)) | reactions, `analog` |

- **§3 rule 1.** Inside `analog`, calls are differentiated through
  ([§11](part_iv_behavior.md#11-analog) rule 5).
- **§3 rule 2.** `extern "C" fn` is implemented outside the language and has no body; a function
  that is neither `extern` nor a trait item has a body. Either mismatch is `E0125`. An
  `extern` function called inside `analog` needs `#[derivative(f)]`.
- **§3 rule 3.** A **closure** `|a, b| expr` is a value and may be passed to functions. It
  captures by value. Its class is that of its body.
- **§3 rule 4.** Something with state is a module with a result
  ([§8.3](part_iii_structure.md#83-instances-inside-expressions)), never a function. A
  function cannot call one: `fn f(x: f64) -> f64 { ddt(x) }` is an error. Write
  `module f(x: f64) -> f64 { ddt(x) }`.
- **§3 rule 5.** A function may call `emit!`: output never feeds back into the simulation.
- **§3 rule 6.** A function body contains no `signal`, no definition, no `<-`, no `+>`, no
  `bound_step` and no `on`; a closure inside a function follows the same rule. Such a form in a
  function body is `E0121`, never `E0404`. The placement matrix is
  [Appendix B.2](appendix_b_grammar.md).
- **§3 rule 7.** A parameter of a nature or bundle type is a handle
  ([§3.1](#31-impl)). A function reads a carrier only through a handle.
- **§3 rule 8.** `where expr` on a function is a boolean const expression over its const
  generics and associated consts. It is checked when the function is instantiated. Trait
  bounds are written in `generics`.
- **§3 rule 9.** A function that is generic over a type, a nature or a const is instantiated
  at elaboration. An error inside an instantiation is an elaboration error.
- **§3 rule 10.** The only ABI string a function accepts is `"C"`. An `extern "C" fn` takes
  and returns `bool`, integer and float scalars only. It **MUST** be a function of its
  arguments alone.
- **§3 rule 11.** `#[derivative(f, …)]` names one function for each `f64` parameter, in order.
  Each has the signature of the `extern` function and returns the partial derivative with
  respect to that parameter.
- **§3 rule 12.** Recursion is allowed. In a function called from `analog`, the recursion
  depth **MUST** be decided by params and constants, so that elaboration can unroll it.

> **Resolution.** The class is inferred, never declared: the design gives the table and no
> syntax for it. An `extern "C" fn` is pure by rule 10.

> **Resolution.** Rule 11 extends the one-argument example of the design to several
> arguments. Rule 10 restricts `extern` functions to scalars because no layout for composite
> values across the C ABI is defined. How the symbol is located is the toolchain's business.

> **Resolution.** Rule 12 mirrors the rule for a module that places itself
> ([§8.2](part_iii_structure.md#82-the-body)): `analog` code is straight-line after
> elaboration.

**Dynamic semantics**

- **§3 rule 13.** A call evaluates its arguments left to right, then the body, as in Rust.
  A value input of a module is not an argument in this sense: it is a `let`
  ([§8.3.1](part_iii_structure.md#83-instances-inside-expressions)).
- **§3 rule 14.** A function has the same meaning in every analysis. What differs between
  analyses is what the probes and the environment it reads return
  ([§16.2](part_v_execution.md#162-every-primitive-in-every-solve)).
- **§3 rule 15.** Known flags travel through arguments, locals and results
  ([§3.3](#33-unknown-values)). An `extern` function is all-or-nothing.
- **§3 rule 16.** A function of class event, used as a `bool` inside a reaction, says whether
  its event fired in this microstep ([§13](part_iv_behavior.md#13-events) rule 2).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| [§8.3](part_iii_structure.md#83-instances-inside-expressions) | a function places an instance: it calls a module with a result, core or not (`E0350`) | elaboration |
| `E0121` | a function body, a closure inside it included, contains a form excluded by rule 6. `E0121` is the code reported, never `E0404` | parse |
| `E0122` | a function is called where one of its classes is not allowed. A core primitive or `emit!` written directly in such a position is `E0450` or `E0601` | elaboration |
| `E0123` | an `extern` function without `#[derivative]` is called inside `analog` | elaboration |
| `E0124` | `#[derivative]` names a function with the wrong signature, or the wrong number of functions | elaboration |
| `E0125` | an ABI string other than `"C"` on a function, a non-scalar type in its signature, an `extern` function with a body, or a function without a body that is neither `extern` nor a trait item | elaboration |
| `E0126` | a recursion reached from `analog` that elaboration cannot unroll | elaboration |
| `E0127` | a `where` on a function that is false for an instantiation | elaboration |

**Examples**

```rust
pub fn diode_current(vd: f64, is: f64, nvt: f64) -> f64 { is * ((vd / nvt).exp() - 1.0) }
pub fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack }
#[derivative(erfcx_dx)] extern "C" fn erfcx(x: f64) -> f64;
```

### 3.1 `impl`

`impl X { … }` attaches functions to a type. It holds functions and, on a value type,
associated constants; it has no time.

**Syntax**

```
impl_block     ::= 'impl' generics? path generic_args? ( 'for' path generic_args? )? '{' impl_item* '}'
impl_item      ::= attrs 'pub'? ( fn_decl | const_item )
```

**Static semantics**

| `impl` on | `self` is | Typical use |
| --- | --- | --- |
| value type | the value | methods, trait implementations |
| a nature | one carrier, read through a handle | events: `rise`, `fall` |
| a bundle ([§7](part_ii_carriers.md#7-bundles)) | the bundle | events and queries over several carriers |

A **handle** is a read-only reference to one carrier or bundle, bound at elaboration. It is
the `self` of an `impl` on a nature or bundle, and any function parameter of such a type.

- **§3.1.1** On a nature or a bundle, `self` is a handle that reads, never a copy.
- **§3.1.2** Generic names follow `impl`; bounds come from the declaration
  (`impl<T> Stream<T>`).
- **§3.1.3** Names may not repeat a carrier method: `changed`, `prev`, `others`, `inflow`.
- **§3.1.4** There is no `impl` on a module. What a module's blocks share is a `let` in its
  body ([§8.2](part_iii_structure.md#82-the-body)).
- **§3.1.5** A handle may be read, probed, passed to another function, and have carrier
  methods called on it. It **MUST NOT** be returned, stored in a value, or written through.
- **§3.1.6** Reading a handle of a discrete carrier yields the carrier's value with its flags
  ([§5.1.1](part_ii_carriers.md#51-declaring-and-reading)). Through a handle, `others()` is
  taken from the module that made the call.
- **§3.1.7** A method call on a carrier expression is looked up in this order: the carrier
  methods ([§14](part_iv_behavior.md#14-core-primitives)), the `impl` blocks of its nature,
  then the methods of the value it reads.
- **§3.1.8** An `impl` on a nature with a defaulted type argument and no argument list is the
  `impl` of the default: `impl Bit` is `impl Bit<bool>`.
- **§3.1.9** `impl Trait for Type` is accepted for a value type only.

> **Resolution.** `level` is added to the names of rule 3: it is a method of every `Repr`
> value, and so of every carrier read ([Appendix B.1](appendix_b_grammar.md)).

> **Resolution.** The associated `const` items of a value type are Rust's (`Logic::Z`,
> [§22.3](part_vii_stdlib.md#223-resolved-nets)). On a nature or bundle an `impl` holds
> functions only (`E0132`).

> **Resolution.** Rules 5–9 define the handle and the lookup order, which the design uses
> (`clk.rise()`, `ipk.max(x)`, `w.s`) without stating.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0130` | an `impl` on a module | elaboration |
| `E0131` | a method named `changed`, `prev`, `others`, `inflow` or `level` | elaboration |
| `E0132` | an item other than a function in an `impl` on a nature or bundle | elaboration |
| `E0133` | a handle returned, stored in a value, or written through | elaboration |
| `E0134` | `impl Trait for` a nature or a bundle | elaboration |

**Examples**

```rust
impl Thermal {
    pub fn celsius(self) -> f64 { temp(self) - 273.15 }
}
```

### 3.2 Parameters and calls

One rule covers functions, methods, core primitives and instances. There is no overloading, no
variadic call and no default value in a signature.

**Syntax**

```
call           ::= path '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')'           // §3.2
```

**Static semantics**

- **§3.2.1** Arguments are positional.
- **§3.2.2** Trailing parameters of type `Option<T>` may be left out. A left-out parameter is
  `None`.
- **§3.2.3** An argument of type `T` passed for a parameter of type `Option<T>` is `Some` of
  it. This is the only implicit conversion in the language.
- **§3.2.4** After the positional arguments, optional parameters may be passed by name,
  `name: value`, which lets a call skip earlier ones.
- **§3.2.5** The callee decides what `None` means, usually with `unwrap_or`.
- **§3.2.6** Module params follow the same rule at an instance,
  `Name(ports) { name: value }`. A module param may also have a default, `r: f64 = 1k`,
  because the host lists and sets params ([§21](part_vi_io.md#21-metadata)).
- **§3.2.7** The **optional parameters** of a callee are the maximal trailing run of its
  parameters of type `Option<T>`. Only these may be left out or named.
- **§3.2.8** An `Option<T>` parameter that a required parameter follows is passed
  positionally. Rule 3 applies to it.
- **§3.2.9** A named argument **MUST** name an optional parameter that no earlier argument of
  the call has bound. Named arguments may come in any order.
- **§3.2.10** When the argument already has the parameter's type `Option<T>`, it is passed
  as it is. Rule 3 never wraps twice: `None` for an `Option<T>` parameter is `None`.
- **§3.2.11** A call through a closure or a generic `Fn` bound is positional only.

> **Resolution.** Rules 7–11 fix what the design leaves implicit: which parameters count as
> optional (`idtmod`'s `ic` does not), duplicate and misplaced names, and nested `Option`.

**Dynamic semantics**

- **§3.2.12** Arguments are evaluated in the order written, named ones included. A left-out
  parameter evaluates nothing.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0140` | a named argument that names no optional parameter, or one already bound | elaboration |
| `E0141` | too few positional arguments for the required parameters, or too many arguments | elaboration |
| `E0142` | a positional argument after a named one | parse |
| `E0143` | a default value or a variadic parameter in a function signature, or a second `fn` of the same name in one namespace (an overload) | parse |

**Examples**

```rust,fragment
cross(e)                    // rising, simulator tolerances
cross(e, 1p)                // with ttol
cross(e, etol: 1u)          // skips ttol
idt(x)                      // an integrator, no initial condition, no reset
idt(x, 1.0, reset: rst)
```

### 3.3 Unknown values

Every level of every value is **known** or **unknown**. Unknown is not a third level: it is
the record that the level is not established. The **known flag** of a level is that record.
One rule covers carriers, locals, function arguments, closures and library code alike, because
the flag travels with the value.

**Static semantics**

- **§3.3.5 `analog` has no unknowns.** The solver computes with known reals. Whatever an
  `analog` block reads from a discrete carrier, directly or through a `let` or a value input,
  passes through `level`: in practice through the stdlib's `known()`, `known_or(d)` or, for a
  `Bit`, `high()` ([§22.1](part_vii_stdlib.md#221-values-and-bit)), after field access and
  indexing at most. Anything else is an elaboration error naming the carrier. An expression
  bound to an `in` port is a definition
  ([§8.1](part_iii_structure.md#81-ports-and-params)): it is discrete and outside this rule.
  The model says what an unknown means electrically; the language does not guess.
- **§3.3.6** The check of rule 5 is syntactic. A carrier read is accepted when, after field
  access and indexing at most, it is the receiver of `level`, or the receiver or argument of a
  flag-total function. A **flag-total function** is one whose result is known for every
  argument, because every use of that parameter in its body passes through `level`.
- **§3.3.7** An implementation **MUST** treat the stdlib's own `known`, `known_or`,
  `is_known` and `high` as flag-total. A project's replacement of one
  ([§22](part_vii_stdlib.md#22-standard-library) rule 3) is checked under rule 6 like any
  function. An implementation **MUST NOT** waive rule 5 because a carrier is provably always
  known ([G.29](appendix_g_scope.md)).
- **§3.3.8** A decision that fixes structure **MUST** be known: a generation condition, a
  loop bound of generation, a const generic, an array length, a `where` condition.

> **Resolution.** Rule 6 turns "passes through `level`" into a check an implementation can
> run. Rule 7 names the functions every implementation must accept, so that conforming models
> do not depend on the strength of one implementation's analysis
> ([G.57](appendix_g_scope.md)).

> **Resolution.** Rule 8: a const may hold unknown levels (`Logic::Z`), so elaboration can
> meet one. Structure that depends on it has no meaning.

**Dynamic semantics**

- **§3.3.1 Sources.** Literals, params, constants, probes and the environment are known.
  Unknown enters only through a carrier with no initial value, the literal `Unknown` (all
  levels unknown, of any `Repr` type), or a `decode` that returns `None`. A value of a type
  without `Repr` has one flag for the whole value.
- **§3.3.2 Operators.** `&`, `|`, `^`, `!` work per level: `unknown & false == false`,
  `unknown | true == true`, otherwise unknown. Field access, indexing by a known index, shifts
  by a known amount, `cast` and the built-in and derived `encode`/`decode` move levels with
  their flags. Every other operator (arithmetic, comparisons, `as`, a shift by an unknown
  amount, an `extern fn`) is all-or-nothing: one unknown level in any operand makes the whole
  result unknown.
- **§3.3.3 Control flow.** When what decides control is unknown (an `if` or `while` condition,
  a `match` scrutinee, a guard, a short-circuit operand, a loop bound, an index), every path
  the known levels do not exclude is taken, and **every effect is merged level by level**:
  equal known levels are kept, the rest become unknown. This combination is the **merge**.
  Effects are the value yielded, each local written, and each commit. A path that does not
  make a commit counts as committing, for the same time, the value the carrier would hold
  then. A commit whose delay is unknown commits `Unknown` at the next microstep: when is not
  known, so from now on the value is not. A loop is its unrolling, merged exit by exit. A loop
  whose exit is still unknown after a bounded number of iterations is cut there, and what its
  body writes becomes unknown; this is the one place where the merge is rounded up. An `emit!`
  reached this way carries `maybe = true`.
- **§3.3.4 Reading the flag.** `x.level(k)` reads level `k` of any `Repr` value as a known
  `Option<bool>`: `None` when unknown. The stdlib builds `x.known() -> Option<T>` (all levels
  known, or `None`), `x.known_or(d)`, `x.is_known()`, `same(a, b)` and `merge(a, b)` on it
  ([§22.1](part_vii_stdlib.md#221-values-and-bit)).
- **§3.3.9** A const whose expression uses `Unknown` is the exception to rule 1's "constants
  are known". NaN is a known value: it is not a form of unknown.
- **§3.3.10** A comparison is all-or-nothing (rule 2): `a == b` is unknown when any level of
  either operand is unknown, even if known levels already differ. `same(a, b)` compares
  levels and flags and is always known.
- **§3.3.11** A shift by a known amount fills with known levels: false, or for an arithmetic
  right shift, the sign level with its flag.
- **§3.3.12** `&&` and `||` on `bool` are control (rule 3). The merge gives
  `unknown && false == false` and `unknown || true == true`, as rule 2 gives for `&` and `|`.
- **§3.3.13** An unknown index selects among the existing elements that its known levels do
  not exclude. A read is the merge of those elements. A write merges the written value into
  each of them ([Appendix E.5](appendix_e_gallery_digital.md)).
- **§3.3.14** In `x.level(k)`, `k` is an index: an out-of-range `k` is a panic
  ([§1](#1-value-types) rule 11), and an unknown `k` is an unknown index (§3.3.13). With a
  known `k` the result of `level` is known.
- **§3.3.15** Two values of a type without `Repr` merge to a known value only when both are
  known and equal. Otherwise the merge is unknown. Any operation on such an unknown value is
  all-or-nothing.
- **§3.3.16** The bound on loop iterations in rule 3 is implementation-defined. An
  implementation **MUST** document it and apply the same bound in every analysis. A larger
  bound only keeps more levels known.

> **Resolution.** Rules 9–16 spell out consequences and corner cases of rules 1–4. Under
> rule 16 two conforming implementations may differ in a result, but only in how many levels
> stay known ([G.46](appendix_g_scope.md)).

Rule 3 does not depend on how the code is written. `q <- if c { a } else { b };` and
`if c { q <- a; } else { q <- b; }` are the same. `if c { s += 1; }` on a local is the merge
of `s` and `s + 1`. An exhaustive `match` on an unknown enum merges all its arms.
`iter().filter(|d| d.oe).count()` over an unknown `oe` is unknown, because `filter` is written
with an `if`.

This is stricter than Verilog's `if`, which takes the `else` branch on an unknown condition
and hides the unknown. A model that wants that writes it: `if c.known_or(false) { … }`.

**Interactions**

| With | Rule | Defined in |
| --- | --- | --- |
| carriers | every level is unknown until the first commit; `q <- Unknown;` commits unknown | [§5.1.1](part_ii_carriers.md#51-declaring-and-reading) |
| changes | a level that goes between known and unknown has changed; unknown to unknown has not | [§5.3](part_ii_carriers.md#53-history) |
| events | an unknown guard runs the reaction under rule 3; an unknown `at` time and an unknown `cross` expression do not fire | [§13](part_iv_behavior.md#13-events), [§17.3](part_v_execution.md#173-locating-crossings) |
| `analog` and every solve | no unknown is read (rule 5) | this section |
| small-signal | a discrete carrier is a known constant of the linearization | [§11.1](part_iv_behavior.md#111-one-block-every-analysis) |
| output | a field keeps its flags; a record reached under unknown control has `maybe = true` | [§19](part_vi_io.md#19-streams-and-checks) rule 1 |
| elaboration | structure must be known (rule 8) | this section |

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0150` | `analog` reads a discrete carrier, directly or through a `let` or a value input, other than through `level` or a flag-total function; the diagnostic names the carrier | elaboration |
| `E0151` | a decision that fixes structure evaluates to unknown | elaboration |

**Examples**

```rust,fragment
on fref.rise() || fvco.rise() {          // state has no initial value here
    let mut s = state;                   // s: i2, all levels unknown
    if fref.rise() && s < 1 { s += 1; }  // condition unknown: s = merge(s, s + 1) = unknown
    state <- s;                          // stays unknown until something resets it
}
```

### 3.4 Files and imports

One file is one module of the namespace. `mod` declares submodules, `use` brings names into
scope, `pub` controls visibility, a package of models is a crate ([§3.5](#35-crates)), and
the package holds it ([§3.6](#36-packages)).

**Static semantics**

- **§3.4.1** A **namespace module** is a `mod` item or a file, as in Rust. "Module" without
  qualification always means a hardware module
  ([§8](part_iii_structure.md#8-modules)).
- **§3.4.2** The **prelude** is the set of names imported implicitly into every file: the
  `pub` items of `std::prelude`, which re-exports the standard library
  ([§22](part_vii_stdlib.md#22-standard-library)) ([§3.5](#35-crates) rule 9). The **core names** need no
  import and no declaration: `gnd`, `Nature`, `m`, `dtemp`, `str` and the core primitives
  ([§14](part_iv_behavior.md#14-core-primitives)).
- **§3.4.3** A hardware module and all its behavior blocks **MUST** be in one file. Each is an
  item at namespace scope, a file or an inline `mod`; a block may sit in any namespace of its
  module's file and names the module by a path (`a::M`, `super::a::M`).
- **§3.4.4** Natures, bundles, hardware modules, functions, constants and value types are
  items. They follow Rust's rules for paths, `use` and `pub`.
- **§3.4.5** A hardware module's name is in the type namespace, as the type of its templates,
  and in the value namespace, as a callable
  ([§8.4](part_iii_structure.md#84-templates-and-swappable-modules)).
- **§3.4.6** `pub` on an item of a module body has its own meaning: visibility to the parent
  instance ([§8.2](part_iii_structure.md#82-the-body)).
- **§3.4.7** A namespace has two tables, one of types and one of values, as in Rust. A name is
  declared once in each table. A hardware module and a unit struct occupy both tables. A
  `struct` with fields and a `fn` of one name occupy different tables and do not collide.
- **§3.4.8** An item or an explicit `use` shadows a glob import of the same name without a
  diagnostic. Two explicit `use` items, or an explicit `use` and an item, of one name in one
  table collide.
- **§3.4.9** Two glob imports that provide one name for different declarations are an error
  only where the name is used. Two that provide the same declaration are not. Two `fn` items of
  one name are `E0143`, not `E0162`.
- **§3.4.10** The items of a block (`fn`, `struct`, `const` and the others a statement position takes)
  form one scope with the same two tables. A name declared twice in one table of one block is
  `E0162` at the later name; this includes two `fn` of one name, since `E0143` is the overload rule
  of namespaces. A block's items shadow the items of the scopes around it without a diagnostic.

> **Resolution.** Rules 7 to 9 are Rust's rules for names, stated because the design is
> silent on duplicates. `E0162` is the one code for a collision; `E0143` keeps the overload.

> **Resolution.** Rule 5 places module names as Rust places a tuple struct's. Rule 1 fixes the
> vocabulary, since the design uses "module" for both.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| [§10](part_iv_behavior.md#10-behavior-blocks) | a behavior block for a module declared in another file (`E0401`) | elaboration |
| `E0161` | a path, `use` or `mod` that does not resolve, a first segment that names no dependency of the crate ([§3.5](#35-crates)), or a path that names a private item | elaboration |
| `E0162` | a name declared twice in one table of one namespace or of one block; an explicit `use` whose name collides with an item or with another explicit `use` of the same table; a name that is used while only two glob imports provide it, for different declarations | elaboration |

### 3.5 Crates

A **crate** is one tree of namespace modules rooted at one file. A crate reaches another only
through a dependency.

**Static semantics**

- **§3.5.1** The **crate root** is the file the tree starts from. Its namespace has no name.
  `mod` items add the files below it ([§3.6](#36-packages) rule 2).
- **§3.5.2** A crate has a name and a set of dependencies. A **dependency name** names one
  other crate inside this crate. It need not be the other crate's own name
  ([§3.6](#36-packages) rule 8).
- **§3.5.3** `std` is a dependency of every crate. No manifest names it and no dependency may
  be called `std`.
- **§3.5.4** The first segment of a path is looked up in this order, and the first hit wins.
  The path may sit in an expression, a type, a pattern or a `use`; the segment is not `crate`,
  `self` or `super`.
    1. the scopes of the enclosing blocks and declarations;
    2. the items and imports of the enclosing namespace;
    3. the dependency names of the crate;
    4. the prelude ([§3.4.2](#34-files-and-imports));
    5. the core names.
- **§3.5.5** Where an item or an import of the enclosing namespace has the name of a
  dependency, an ordinary path follows rule 4: the item or import wins, without a diagnostic.
  A `use` path whose first segment is that name is ambiguous (`E0163`). There is no leading
  `::` ([Appendix B.3.6](appendix_b_grammar.md)). Reach the dependency under another key,
  `other = { package = "<name>", .. }` ([§3.6](#36-packages) rule 8), and the local item as
  `crate::<name>`.
- **§3.5.6** A dependency name resolves to the root namespace of that crate. A crate's own name
  is not in scope inside it.
- **§3.5.7** `crate` names the root of the crate that contains the path. `self` and `super`
  are relative to the namespace that contains them. `super` at a crate root does not resolve
  (`E0161`).
- **§3.5.8** From another crate an item is visible only when it is `pub` and every namespace on
  the path from its crate root to it is `pub`. `pub use` re-exports, as in Rust.
- **§3.5.9** The prelude is implied in every file of every crate, the files of `std` included.
- **§3.5.10** The dependencies of the crates form no cycle ([§3.6](#36-packages), `E0174`).
- **§3.5.11** `extern crate` is not part of the language
  ([Appendix B.3.6](appendix_b_grammar.md)).

> **Resolution.** Rules 4 and 5 are Rust 2018's: dependency names sit below a namespace's own
> names in a path, and an import that could mean either is an error, never a silent choice.
> They are stated because no `::` exists to disambiguate. A dependency name against a prelude
> item is not a clash: the dependency wins.

> **Resolution.** Rule 9 has no exception for `std`, so one rule holds in every crate.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0163` | a `use` path whose first segment names both a dependency and an item or an import of the enclosing namespace | elaboration |
| [§3.4](#34-files-and-imports) | a first segment that names no item, import, dependency, prelude item or core name, or an item not visible from the crate (`E0161`) | elaboration |
| [§3.6](#36-packages) | a cycle among the dependencies (`E0174`) | package |

**Examples**

```rust
use util::cells::Cell;
```

`util` is a dependency name. A local `mod util;` makes this `use` ambiguous (`E0163`).

### 3.6 Packages

A **package** is a directory with a manifest. It holds one crate.

**Static semantics**

- **§3.6.1** The **manifest** is the file `Piperine.toml` in the package directory, written in
  TOML.
- **§3.6.2** The crate root is `src/lib.msl`. A source file has the extension `.msl`.
  `mod name;` loads `name.msl` or `name/mod.msl` below the declaring file, as Rust places
  `name.rs` and `name/mod.rs`; neither or both is `E0161`.
- **§3.6.3** The manifest holds the tables and keys of the table below and no other (`E0171`).
- **§3.6.4** A package name and a dependency name are identifiers (A.1) that are no keyword of
  the language and not `std`. The package name is the crate's name.
- **§3.6.5** A version is `MAJOR.MINOR.PATCH`, three decimal integers.
- **§3.6.6** A **dependency** is an entry of `[dependencies]`; its key is the dependency name.
  It has exactly one source: `path` or `git`.
- **§3.6.7** `path` is a directory, relative to the directory of the manifest that names it, or
  absolute. `git` is a repository URL; it takes at most one of `branch`, `tag` and `rev`, and
  none means the repository's default branch. A `rev` is a commit id, 7 to 40
  hexadecimal digits (`E0171`), never a branch, tag or other revision expression. The package is at the root of the repository.
- **§3.6.8** `package` gives the located package's own name when it differs from the key. The
  located package's name **MUST** equal `package`, or the key when there is none (`E0173`).
- **§3.6.9** `version` is a requirement the located package's version **MUST** satisfy
  (`E0173`), in one of the forms of the requirement table.
- **§3.6.10** Two dependencies that locate the same package (one directory; one repository at
  one commit) name the same crate.
- **§3.6.11** Dependencies are followed transitively. The graph **MUST NOT** hold a cycle
  (`E0174`).
- **§3.6.12** A crate is reached through a dependency and in no other way: an implementation
  **MUST NOT** find a crate through a search path, a registry or an environment variable.
- **§3.6.13** The **lockfile** is `Piperine.lock` beside the root package's manifest. It
  records the commit each git package resolved to. While an entry's source (URL and requested
  reference) equals a dependency's, the recorded commit **MUST** be used. An explicit update
  resolves again. A lockfile inside a dependency is not read.
- **§3.6.14** The lockfile is TOML: `version = 1`, then one `[[package]]` per git package with
  `name`, `version`, `source` (`git+<url>`, then `?branch=<b>`, `?tag=<t>` or `?rev=<r>` when
  one is given) and `commit` (40 hexadecimal digits, lowercase), sorted by `name`, then `source`.

Manifest keys:

| Table | Key | Type | Required |
| --- | --- | --- | --- |
| `[package]` | `name` | string | yes |
| `[package]` | `version` | string | yes |
| `[package]` | `authors` | array of strings | no |
| `[package]` | `description` | string | no |
| `[dependencies]` entry | `path` | string | one of `path`, `git` |
| `[dependencies]` entry | `git` | string | one of `path`, `git` |
| `[dependencies]` entry | `branch`, `tag`, `rev` | string | no, at most one, with `git` only |
| `[dependencies]` entry | `version` | string | no |
| `[dependencies]` entry | `package` | string | no |

Requirement table (Cargo's caret rule):

| Requirement | Admits |
| --- | --- |
| `1.2.3` | `>=1.2.3, <2.0.0` |
| `1.2` | `>=1.2.0, <2.0.0` |
| `1` | `>=1.0.0, <2.0.0` |
| `0.2.3` | `>=0.2.3, <0.3.0` |
| `0.2` | `>=0.2.0, <0.3.0` |
| `0.0.3` | `>=0.0.3, <0.0.4` |
| `0.0` | `>=0.0.0, <0.1.0` |
| `0` | `>=0.0.0, <1.0.0` |

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0170` | a package directory without `Piperine.toml`, or a manifest that is not UTF-8 or not valid TOML | package |
| `E0171` | a manifest entry rules 3 to 9 do not allow: an unknown table or key, a missing `name` or `version`, a value of the wrong type, a name that breaks rule 4, a version or requirement of another form, a dependency without exactly one source, a `branch`, `tag` or `rev` beside another or beside `path`, a `rev` that is not 7 to 40 hexadecimal digits | package |
| `E0172` | a dependency that cannot be located: a `path` with no manifest, a repository that cannot be fetched, a reference the repository does not hold | package |
| `E0173` | a located package that is not the one asked for: another name, or a version that does not satisfy `version` | package |
| `E0174` | a cycle among the dependencies | package |
| `E0175` | a package without its crate root `src/lib.msl` | package |
| `E0176` | a lockfile that cannot be used: not readable or not valid TOML, no `version` or one that is not `1`, an entry that is no table, has an unknown key, lacks one of its four keys or gives a value of the wrong form (a `commit` that is not 40 lowercase hexadecimal digits included), or one that cannot be written | package |

**Examples**

```toml
[package]
name = "cells"
version = "0.1.0"

[dependencies]
util = { path = "../util" }
```
