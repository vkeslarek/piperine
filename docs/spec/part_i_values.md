# Part I. Values

A **value** is data with no time. Everything a model computes with is a value; what it connects
to is a carrier ([Part II](part_ii_carriers.md)). This part defines the value language in
full: the types ([§1](#1-value-types) to [§1.4](#14-typing-inference-and-conversion)), the
expressions, patterns, statements and control flow ([§1.5](#15-expressions) to
[§1.8](#18-control-flow)), the layout on levels ([§2](#2-repr)), and the items: functions,
`impl`, calls, names, crates, traits, generics and constants ([§3](#3-functions) to
[§3.9](#39-constants)). Its syntax is that of Rust's value language
([§0.4](index.md#04-a-complete-specification)); its rules are stated here. The first of them
is that data are values: a value is copied wherever it is used, and nothing in the value
language refers to another value ([§1.4.6](#14-typing-inference-and-conversion)).

Error codes of this part: `E01xx`.

**Terms defined here:** value, value type, level, `Repr`, function class, closure, handle,
uncertainty, absence, initial value, panic, optional parameters, namespace
module, prelude, crate, crate root, dependency name, package, manifest, dependency,
lockfile, copy, identity, place, local, irrefutable pattern, refutable pattern, const expression.

## 1. Value types

A **value type** is one of the kinds below. Only a type that implements `Repr`
([§2](#2-repr)) can be carried.

| Kind | Types | Notes |
| --- | --- | --- |
| boolean | `bool` |  |
| integer | `UInt<N>`, `Int<N>` (`u1` … `u128`, `i1` … `i128`) | any width, a const expression (`UInt<{A + 1}>`); arithmetic wraps |
| float | `Float<N>`, `N` in {16, 32, 64, 128} (`f16` … `f128`) | IEEE 754; analog code computes in `f64` |
| string | `str` | the text of a literal; never carried ([§1.2.9](#12-composite-types)) |
| composite | `()`, tuples, `[T; N]`, `struct`, `enum`, `Option<T>` | [§1.2](#12-composite-types), [§1.3](#13-structs-and-enums) |

**Static semantics**

- **§1 rule 1.** `as` converts between numeric types: among integers it truncates or extends
  ([§1.4.5](#14-typing-inference-and-conversion)). `checked_*` reports an overflow and
  `saturating_*` clamps it. The methods a model may call are those of
  [Appendix H](appendix_h_library.md), each with its meaning stated there.
- **§1 rule 2.** These are left out, because a model is fixed at elaboration and its data are
  values ([§1.4.6](#14-typing-inference-and-conversion)): references and pointers of every
  kind (`&T`, `&mut T`, `&x`, `*x`, a `&p` pattern, `&self`), lifetimes, the markers `mut`,
  `ref` and `move`, heap types, `unsafe`, threads, `async`, `dyn`. `&` is the bitwise operator
  only ([§1.5.3](#15-expressions)) and `*` the multiplication only: no type, expression or
  pattern makes a reference or reads through one.
- **§1 rule 3.** Constants are `const` items. The prelude has `Q`, `K_B`, `EPS0`, `PI`, `E`
  and `inf` (table below): they are `pub const` items of `std::constants`, re-exported by the
  prelude.
- **§1 rule 4.** Only a type with `Repr` can be carried: it is the `T` of `Bit<T>` and the
  `value` of a nature ([§4](part_ii_carriers.md#4-natures)).
- **§1 rule 5.** `uN` and `iN` are aliases of `UInt<N>` and `Int<N>` for `N` in 1..=128. `fN`
  is an alias of `Float<N>`. The width of `UInt<N>` and `Int<N>` is any const expression
  `N >= 1`.
- **§1 rule 6.** Integer arithmetic wraps modulo `2^N` in every build. `Int<N>` is two's
  complement. There is no overflow error.
- **§1 rule 7.** An integer literal has an integer type and a float literal a float type
  ([§1.4.2](#14-typing-inference-and-conversion)). Neither converts implicitly ([§3.2.3](#32-parameters-and-calls)). A literal out of
  its type's range is `E0909` ([A.2](appendix_a_lexical.md#a2-si-suffixed-reals) rule 8).
- **§1 rule 8.** Every string is the text of a literal: no operation builds a new one. A
  string has no `Repr`.
- **§1 rule 9.** `Event` ([§13](part_iv_behavior.md#13-events)), `Analysis`
  ([§11.1](part_iv_behavior.md#111-one-block-every-analysis)), closures
  ([§3](#3-functions) rule 3) and templates
  ([§8.4](part_iii_structure.md#84-templates-and-swappable-modules)) are values without
  `Repr`.

> **Resolution.** `usize` and `isize` are aliases of `UInt<64>` and `Int<64>` on every
> platform. A result may not depend on the machine that computes it
> ([§0.1.1](index.md#01-scope); [G.62](appendix_g_scope.md)).

> **Resolution.** There is no character type: the table above is closed
> ([G.61](appendix_g_scope.md)). There is no growable string, no growable array and no heap
> allocation of any kind (rule 2); a name such as `String`, `Vec` or `Box` does not resolve.

**Prelude constants**

The six constants are `pub const` items of `std::constants` ([§22](part_vii_stdlib.md#22-standard-library)), re-exported by the prelude.

| Name | Type | Value | Meaning |
| --- | --- | --- | --- |
| `Q` | `f64` | `1.602176634e-19` | elementary charge, C |
| `K_B` | `f64` | `1.380649e-23` | Boltzmann constant, J/K |
| `EPS0` | `f64` | `8.8541878128e-12` | vacuum permittivity, F/m |
| `PI` | `f64` | `3.141592653589793` | π, to the nearest `f64` |
| `E` | `f64` | `2.718281828459045` | Euler's number, to the nearest `f64` |
| `inf` | `f64` | `1.0 / 0.0` | positive infinity |

```rust
pub const Q: f64 = 1.602176634e-19;
pub const K_B: f64 = 1.380649e-23;
pub const EPS0: f64 = 8.8541878128e-12;
pub const PI: f64 = 3.141592653589793;
pub const E: f64 = 2.718281828459045;
pub const inf: f64 = 1.0 / 0.0;
```

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
- **§1 rule 13.** A panic is the failure of the path that runs. A path that control does not
  select does not run, so it cannot panic ([§3.3.8](#33-uncertainty-and-absence)).
- **§1 rule 14.** A plain identifier pattern, with no `@`, whose name resolves
  to a constant, a unit variant or a unit struct is a path to that item and binds nothing.
  Any other identifier pattern binds a new name ([§1.6.2](#16-patterns)).

> **Resolution.** Rule 14 decides by resolution, not by spelling. `match o { None => 0, Some(x) => x }` tests for
> `None`; it does not bind a name `None`. A name that resolves to a local or a `fn` is a
> binding that shadows it.

> **Resolution.** Rules 11–13 are not in the design, which is silent on panics. Control takes a
> `bool` ([§3.3.5](#33-uncertainty-and-absence)), so which path runs is always decided and a
> panic is always on a path that runs.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0101` | a syntactic form left out by rule 2: a reference or a pointer in a type, an expression or a pattern (`&T`, `&mut T`, `&x`, `*x`, `&p`, `&self`), `unsafe`, `async`, `dyn`. The markers `mut`, `ref` and `move` are reserved words (`E0903`, [A.1](appendix_a_lexical.md#a1-source-text-and-tokens)) and a lifetime is a token of a dropped class (`E0905`, [A.6](appendix_a_lexical.md)). A heap type or a thread API is a path that does not resolve (`E0161`) | parse |
| `E0103` | an integer width that is not a const expression `>= 1`, or a float width outside {16, 32, 64, 128} | elaboration |
| `E0104` | a panic while evaluating a const expression, a param default, a `where` or generation | elaboration |
| `E0105` | a panic during a run | run |

> **Resolution.** `E0102` is retired. It reported a `&` outside the positions where earlier
> revisions allowed a borrow. No position allows one now: a `&` that is not the bitwise
> operator is a form left out by rule 2 (`E0101`), found at parse.

### 1.1 Scalar types

The scalar types are `bool`, the integer types and the float types.

**Static semantics**

- **§1.1.1** `bool` has two values, `true` and `false`.
- **§1.1.2** `UInt<N>` holds the integers `0` to `2^N - 1`. `Int<N>` holds the integers
  `-2^(N-1)` to `2^(N-1) - 1`, in two's complement. `N` is a const expression `>= 1`
  ([§1](#1-value-types) rule 5); it has no upper limit.
- **§1.1.3** Two integer types are the same type when they have the same signedness and the
  same width: `u8`, `UInt<8>` and `UInt<{4 + 4}>` are one type. `usize` is `UInt<64>` and
  `isize` is `Int<64>`. Integer types of different width or signedness are different types.
- **§1.1.4** `Float<N>` is the IEEE 754 binary interchange format of `N` bits, for `N` in
  {16, 32, 64, 128}. Its values are the finite numbers of that format, the two zeros, the two
  infinities and NaN.
- **§1.1.5** A scalar type has no associated item other than the methods of
  [Appendix H](appendix_h_library.md). The largest value of a type is written as a literal;
  positive infinity is the prelude constant `inf`.
- **§1.1.6** The operators of each scalar type are those of [§1.5](#15-expressions). No scalar
  type converts to another implicitly ([§1.4.3](#14-typing-inference-and-conversion)).

**Dynamic semantics**

- **§1.1.7** A float operation (`+`, `-`, `*`, `/`, a conversion, `sqrt`) yields the IEEE 754
  result, rounded to nearest with ties to even. No operation traps and there is no rounding
  mode to select.
- **§1.1.8** An operation with no numeric result yields NaN: `0.0 / 0.0`, `inf - inf`,
  `0.0 * inf`, an operation with a NaN operand. The sign and payload of a NaN result are
  implementation-defined; every NaN behaves alike under the operators and methods of this
  specification, except through `encode` ([§2](#2-repr)).

> **Resolution.** Rule 1.1.5 closes the scalar types: `u8::MAX` and `f64::EPSILON` are paths
> that do not resolve (`E0161`). Rule 1.1.8 leaves one implementation-defined result, the bit
> pattern of a NaN ([G.69](appendix_g_scope.md)).

### 1.2 Composite types

**Syntax**

```
type           ::= path
                 | '(' ')' | '(' type ',' ( type ',' )* ')' | '(' type ')'
                 | '[' type ';' expr ']'
                 | 'fn' '(' ( type ',' )* ')' ( '->' type )?
                 | '_'
```

**Static semantics**

- **§1.2.1** The unit type `()` has one value, `()`. It is the type of a block with no final
  expression, of a function with no `-> type`, and of an assignment.
- **§1.2.2** A tuple type `(T1, T2, …)` has two or more fields; `(T,)` is the tuple of one
  field and `(T)` is `T`. The fields are read as `.0`, `.1`, …, and the tuple expression
  `(a, b)` builds one. Two tuple types are the same type when their fields are, in order.
- **§1.2.3** An array type `[T; N]` has `N` elements of type `T`. `N` is a const expression of
  an integer type, known and not negative. The length is part of the type: `[f64; 2]` and
  `[f64; 3]` are different types.
- **§1.2.4** An array expression is a list `[a, b, c]`, whose elements have one type, or a
  repetition `[x; N]`, which evaluates `x` once and holds `N` copies of it. In `[x; N]`,
  `N` is a const expression.
- **§1.2.5** Every run of elements is an array, and its length is part of its type: there is
  no slice type. A function over arrays of any length takes the length as a const parameter,
  `fn sum<const N: usize>(xs: [f64; N]) -> f64` ([§3.8](#38-generics-and-bounds)). A part of
  an array is an array too: `e[a..b]` ([§1.5.9](#15-expressions)).
- **§1.2.6** There is no reference type. A parameter, a field, a local, a constant and a
  result each hold a value of their type, never a view of another value. The library yields
  values as well: `iter` yields each element, and `first`, `last` and `get` an `Option` of the
  element ([Appendix H](appendix_h_library.md), H.5).
- **§1.2.7** An array, a tuple, a struct and an enum are passed, returned and assigned whole,
  as one value ([§1.4.6](#14-typing-inference-and-conversion)). A string, a function pointer
  and a closure are values in the same way; none of the three has `Repr`
  ([§1](#1-value-types) rules 8 and 9).
- **§1.2.8** `Option<T>` is the enum with the variants `None` and `Some(T)`
  ([§1.3](#13-structs-and-enums)). `Option`, `Some` and `None` are prelude names.
- **§1.2.9** `str` is the type of a string: a sequence of Unicode scalar values. It is a
  value type without `Repr`. Every string is the text of a literal
  ([§1](#1-value-types) rule 8), so its length is fixed at elaboration and is not part of the
  type. Two strings are compared with `==` and `!=`, which compare their scalar values in
  order. There is no lifetime in the language.
- **§1.2.10** A function-pointer type `fn(T1, T2) -> U` is the type of a value that names a
  function. The name of a `fn` item used as a value has this type, and so does a closure that
  captures nothing. The parameter types are written without names. A call through such a
  value is positional ([§3.2.11](#32-parameters-and-calls)).
- **§1.2.11** A closure expression has a type of its own, which no source text can name. A
  closure is passed to a parameter whose type is a generic bound by `Fn(T1, T2) -> U`
  ([§3.8](#38-generics-and-bounds)), or, when it captures nothing, to a function-pointer
  parameter.
- **§1.2.12** `_` in a type asks for the type to be inferred
  ([§1.4.4](#14-typing-inference-and-conversion)). It is allowed in the annotation of a `let`
  or of a closure parameter and in the generic arguments of an expression, and nowhere in the
  signature of an item.
- **§1.2.13** `Self` names the type an `impl` or a `trait` is about
  ([§3.7](#37-traits)).

> **Resolution.** Rules 1.2.5 to 1.2.7 replace the borrow rules of earlier revisions. Six
> special cases go with them: the slice type `[T]`, which existed only behind a borrow; the
> borrow type `&T`, written in two positions and yielded by four library methods; the four
> places where a borrow was read through without an operator; `*r` everywhere else; the
> acceptance of `&[T; N]` where a `&[T]` was required; and the lifetime `'static`, with the
> exception that let `&'static str` alone be stored in a `const`. One rule stands in their
> place: every type of the table of [§1](#1-value-types) is a value type, and a value is
> copied ([§1.4.6](#14-typing-inference-and-conversion)). [§3.2.3](#32-parameters-and-calls)
> stays the only implicit conversion ([G.71](appendix_g_scope.md)).

**Dynamic semantics**

- **§1.2.14** The elements of an array expression and the fields of a tuple expression are
  evaluated left to right.

**Examples**

```rust,fragment
let pair = (1.0, true);
let x = pair.0;
let taps = [0.25, 0.5, 0.25];
let zeros = [0u8; 16];
let unit = ();
```

### 1.3 Structs and enums

A `struct` and an `enum` declare a new value type. A `struct` with a carrier field is a bundle
([§7](part_ii_carriers.md#7-bundles)), not a value type.

**Syntax**

```
struct_decl    ::= 'struct' IDENT generics? ( '{' ( struct_field ',' )* '}' | '(' ( tuple_field ',' )* ')' ';' | ';' )
tuple_field    ::= attrs 'pub'? type
enum_decl      ::= 'enum' IDENT generics? '{' ( variant ',' )* '}'
variant        ::= attrs IDENT ( '{' ( struct_field ',' )* '}' | '(' ( tuple_field ',' )* ')' )? ( '=' expr )?
struct_expr    ::= path generic_args? '{' ( IDENT ( ':' expr )? ',' )* '}'
```

**Static semantics**

- **§1.3.1** A `struct` has named fields (`struct P { x: f64, y: f64 }`), positional fields
  (`struct P(f64, f64);`, a tuple struct) or no field (`struct Marker;`, a unit struct). Each
  declaration is a type of its own: two structs with the same fields are different types.
- **§1.3.2** The field names of a struct, and the variant names of an enum, are distinct.
- **§1.3.3** A struct expression `P { x: 1.0, y }` names every field exactly once, in any
  order. `y` alone stands for `y: y`. There is no form that copies the remaining fields from
  another value. A tuple struct is built by calling its name, `P(1.0, 2.0)`, and a unit struct
  by its name.
- **§1.3.4** `s.x` reads the field `x` of a struct, and `s.0` a positional field. A field is
  read only where it is visible ([§3.4.13](#34-files-and-imports)).
- **§1.3.5** An `enum` has variants. A variant has no field (`Idle`), positional fields
  (`Data(u8)`) or named fields (`Data { n: u3 }`). A value of the enum is exactly one variant
  with its fields.
- **§1.3.6** A variant is named through its enum, `Tx::Idle`. It is in scope by its bare name
  only where a `use` imports it (`use Tx::Idle;`, `use Tx::*;`). A variant with positional
  fields is built by a call, `Tx::Data(3)`; one with named fields by a struct expression,
  `Tx::Data { n: 3 }`; one without fields by its name.
- **§1.3.7** A variant may carry a discriminant, `Add = 0b00`, when no variant of the enum has
  a field. The discriminant is a const expression of an integer type. A variant without one
  has the discriminant of the variant before it plus one, and the first has 0. Two variants
  **MUST NOT** have the same discriminant (`E0111`, [§2](#2-repr)).
- **§1.3.8** A type **MUST NOT** contain itself, directly or through other structs, enums,
  tuples and arrays: a value has a fixed size and there is no indirection.
- **§1.3.9** A struct or an enum may be generic ([§3.8](#38-generics-and-bounds)) and may
  carry `#[derive(…)]` ([Appendix H](appendix_h_library.md), H.8) and, on an enum,
  `#[repr(uN)]` ([§2](#2-repr)).
- **§1.3.10** The operators `==`, `!=`, `<`, `<=`, `>` and `>=` apply to a struct or an enum
  only when it implements `PartialEq` or `PartialOrd`, derived or written
  ([Appendix H](appendix_h_library.md), H.8).

**Dynamic semantics**

- **§1.3.11** The field expressions of a struct expression are evaluated in the order they are
  written, not in the order of the declaration.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0186` | a field or a variant declared twice; a struct expression that leaves a field out, names one twice or names one the type does not have; a field access or tuple index the type does not have; a type that contains itself (§1.3.8); a discriminant on an enum that has a variant with fields | elaboration |

**Examples**

```rust
#[derive(PartialEq)]
struct Point { x: f64, y: f64 }

#[derive(PartialEq)]
enum Shape { Dot, Circle(f64), Rect { w: f64, h: f64 } }

fn area(s: Shape) -> f64 {
    match s {
        Shape::Dot => 0.0,
        Shape::Circle(r) => PI * r * r,
        Shape::Rect { w, h } => w * h,
    }
}

fn origin() -> Point { Point { x: 0.0, y: 0.0 } }
```

### 1.4 Typing, inference and conversion

**Static semantics**

- **§1.4.1** Every expression has one type, fixed at elaboration. The types of items are
  written: the parameters and the result of a function, a `const`, a field, a param. The type
  of a local, of a closure parameter and of a generic argument of an expression may be left
  out and is then inferred.
- **§1.4.2** An integer literal with a suffix has the type of the suffix
  ([A.8](appendix_a_lexical.md#a8-literals)). Without a suffix it takes the integer type its
  position requires, and `i32` when the position requires none. A float literal with a suffix
  has that type; without one it takes the float type its position requires, and `f64` when the
  position requires none. An SI-suffixed literal is always an `f64`
  ([A.2](appendix_a_lexical.md#a2-si-suffixed-reals) rule 1).
- **§1.4.3** There is no implicit conversion between types: an expression of type `T` is
  accepted only where a `T` is required. An integer literal is not a float, and an integer of
  one width is not an integer of another. The single exception is
  [§3.2.3](#32-parameters-and-calls).
- **§1.4.4** Inference is local to one body: a function body, the initializer of a `const`, or
  one expression of a module body. A `let` without a type takes the type of its initializer.
  Where the initializer does not fix its own type (a literal without a suffix, `None`,
  an empty array `[]`, a closure parameter), the uses of the binding in the same
  body fix it. A type that the body does not fix, other than by the defaults of rule 2, is
  `E0181`.
- **§1.4.5** `expr as T` converts between the types of the table. Any other pair is `E0182`.

| From | To | Result |
| --- | --- | --- |
| an integer type | an integer type of `N` bits | the value modulo `2^N`: a narrower target truncates, a wider one extends by the sign of the source type (zeros for `UInt`, the sign bit for `Int`) |
| an integer type | a float type | the nearest value of the float type, ties to even; a magnitude beyond the largest finite value gives an infinity |
| a float type | an integer type | the value truncated toward zero, then clamped to the range of the target; NaN gives 0 |
| a float type | a float type | the nearest value of the target, ties to even; a magnitude beyond the largest finite value gives an infinity; NaN gives NaN |
| `bool` | an integer type | 0 for `false`, 1 for `true` |
| an enum without fields | an integer type | the discriminant of the variant (§1.3.7), modulo `2^N` for a target of `N` bits |
| a type | the same type | the value |

- **§1.4.6** A value is **copied** wherever it is used. Reading a variable, a field or an
  element, passing an argument, returning a result, binding a name in a pattern, capturing in
  a closure, committing to a carrier and emitting a field each yield a **copy**: a value equal
  to the one read, which stays as it was and may be read again. No two names share a value, so
  assigning one never changes another. This holds for every value type; there is no trait and
  no derive that grants it, and no type that lacks it. An implementation **MAY** avoid a copy
  wherever no model can observe the difference.
- **§1.4.7** Hardware is not a value. A carrier, a node, an instance and a bundle each have an
  **identity**: a port, a connection and a handle ([§3.1](#31-impl)) name the same carrier,
  never a copy of it. State that outlives a block or is shared between blocks lives in
  carriers only, and a carrier changes only by a definition, a commit (`<-`) or a contribution
  (`+>`), each at the time its rule gives ([§5.2](part_ii_carriers.md#52-drives),
  [§12.1](part_iv_behavior.md#121-reactions), [§11](part_iv_behavior.md#11-analog)). Reading
  a carrier yields a value: a copy of what the carrier holds at that read
  ([§5.1](part_ii_carriers.md#51-declaring-and-reading)).
- **§1.4.8** A type is checked against a required type by equality. Two types are equal when
  they are the same scalar type, the same struct, enum, nature or module with equal generic
  arguments, or the same composite form with equal parts. Const arguments and array lengths
  are compared by value, at each instantiation ([§3.8.9](#38-generics-and-bounds)).

> **Resolution.** The enum row of the table makes `Op::Sub as u8` the discriminant.
> The float-to-integer row is total: no conversion panics.

> **Resolution.** Rules 1.4.6 and 1.4.7 are the language's whole answer to "by value or by
> reference": data are values and always copy; hardware is reached by identity and changes
> only at a defined time. Earlier revisions had a third kind between the two, a `Copy`
> property that some types had and others lacked, with a *move* for the rest. Removed with
> it: the traits `Clone` and `Copy`, their derives and the method `clone()`; the move of a
> value without `Copy` and the error for reading it again; the rule that a field of such a
> type could not be read out of its struct; and the `Copy` requirements of `[x; N]`, of `*r`
> and of `Repr` ([G.71](appendix_g_scope.md)).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0180` | an expression whose type is not the type its position requires, or operands whose types an operator does not accept ([§1.5](#15-expressions)) | elaboration |
| `E0181` | a type that cannot be inferred: a local, a closure parameter or a generic argument that its body does not fix (§1.4.4), or `_` in the signature of an item (§1.2.12) | elaboration |
| `E0182` | an `as` between two types that are not a row of §1.4.5 | elaboration |
| `E0183` | a variable read before it was assigned ([§1.7.5](#17-statements-and-blocks)); an assignment to something that is not a place ([§1.5.12](#15-expressions)) | elaboration |

**Examples**

```rust,fragment
let a = 300 as u8;            // 44
let b = -1i8 as u16;          // 65535
let c = 300.7 as u8;          // 255
let d = 7u3 as f64;           // 7.0
let e: u9 = 400;              // the literal is a u9
let f = 1;                    // i32
let g = 1.5;                  // f64
```

### 1.5 Expressions

An expression produces a value. Evaluating it may also assign a local, and, where the
placement matrix allows ([Appendix B.2](appendix_b_grammar.md)), emit a record or place an
instance.

**Syntax**

The productions of `expr` are [Appendix B.3.7](appendix_b_grammar.md#b37-the-value-grammar).
The table gives the operators from the tightest binding to the loosest.

| Level | Operators | Associativity |
| --- | --- | --- |
| 1 | a literal, a path, `( )`, `[ ]`, a block, `if`, `match`, a loop, a struct expression, `emit!`, `include_str!` | — |
| 2 | postfix: `.name`, `.0`, `.name(args)`, `(args)`, `[index]`, `?` | left to right |
| 3 | prefix: `-`, `!` | — |
| 4 | `as` | left to right |
| 5 | `*`, `/`, `%` | left to right |
| 6 | `+`, `-` | left to right |
| 7 | `<<`, `>>` | left to right |
| 8 | `&` | left to right |
| 9 | `^` | left to right |
| 10 | `\|` | left to right |
| 11 | `==`, `!=`, `<`, `>`, `<=`, `>=` | none: `a < b < c` is a parse error |
| 12 | `&&` | left to right |
| 13 | `\|\|` | left to right |
| 14 | `..`, `..=` | none |
| 15 | `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `\|=`, `^=`, `<<=`, `>>=` | right to left |
| 16 | `return`, `break`, a closure | the operand extends as far to the right as it can |

**Static semantics**

- **§1.5.1** The arithmetic operators `+`, `-`, `*`, `/` and `%` take two operands of one
  integer type or of one float type and yield that type.
- **§1.5.2** Prefix `-` takes an integer or a float and yields its type. Prefix `!` takes a
  `bool` or an integer and yields its type.
- **§1.5.3** `&`, `|` and `^` take two operands of one integer type, or two `bool`, and yield
  that type. Both operands are always evaluated.
- **§1.5.4** `<<` and `>>` take an integer on the left and an integer of any integer type on
  the right, and yield the type of the left operand.
- **§1.5.5** `==` and `!=` take two operands of one type that implements `PartialEq`; `<`,
  `<=`, `>` and `>=` take two operands of one type that implements `PartialOrd`. Each yields a
  `bool`. The scalar types, `()`, strings, and the tuples, arrays and `Option`s of comparable
  types are comparable; a struct or an enum is when it derives or implements the trait
  (§1.3.10). A comparison does not chain.
- **§1.5.6** `&&` and `||` take two `bool` and yield a `bool`.
- **§1.5.7** `expr as T` is the conversion of [§1.4.5](#14-typing-inference-and-conversion).
  After `as T`, a `<` is read as the start of generic arguments of `T`: write `(a as u8) < b`.
- **§1.5.8** A path expression names a local, a function parameter, a `const`, a function (a
  function pointer, §1.2.10), a unit struct, a variant, a param, a carrier (a read,
  [§5.1](part_ii_carriers.md#51-declaring-and-reading)) or an associated constant `T::C`.
- **§1.5.9** A field access `e.name`, a tuple index `e.0` and an index `e[i]` read one part of
  a value. In `e[i]`, `e` is an array and `i` has any integer type. `e[a..b]`, with a range
  as the index, is the array of the elements `a` to `b - 1`, of type `[T; b - a]`; its bounds
  are const expressions. The other range forms index alike: `e[a..=b]`, and `e[..b]`,
  `e[a..]` and `e[..]`, where a bound left out is 0 or the length. A range index is not a
  place.
- **§1.5.10** A call and a method call follow [§3.2](#32-parameters-and-calls). A method
  `e.f(args)` is looked up on the type of `e`: first the functions of the `impl` blocks of
  that type that take `self`, then the methods of [Appendix H](appendix_h_library.md) for
  that type, then the functions of the traits in scope that the type implements
  ([§3.7.8](#37-traits)). On a carrier the order is that of [§3.1.7](#31-impl). A name none
  of them has does not resolve (`E0161`).
- **§1.5.11** A range expression is `a..b` (from `a`, below `b`), `a..=b` (from `a` through
  `b`), `a..`, `..b`, `..=b` or `..`. Both bounds have one integer or float type. A range is a
  value of a range type, which no source text can name; its methods are those of
  [Appendix H](appendix_h_library.md), H.6, and `a..b` and `a..=b` over an integer type are
  iterators (H.7).
- **§1.5.12** An assignment `place = value` has the type `()`. A **place** is a local, a field
  of a place, or an element of a place. A **local** is a name bound by a `let`, or by the
  pattern of a `match` arm, of an `if let`, of a `while let` or of a `for`. Every local is a
  place: no marker makes it one. A carrier, a param, a `const`, a body `let` of a module
  ([§8.2.12](part_iii_structure.md#82-the-body)), a parameter of a function or of a closure,
  `self` included, and a name a closure captures are not places. Assigning a place replaces
  its value and changes no other name ([§1.4.6](#14-typing-inference-and-conversion)).
  `place op= value` is `place = place op value` for the operators of level 15, with `place`
  evaluated once.
- **§1.5.13** A block `{ … }` is an expression ([§1.7](#17-statements-and-blocks)); so are
  `if`, `match` and the loops ([§1.8](#18-control-flow)).
- **§1.5.14** A closure is `|a, b| expr` or `|a: T, b: U| -> R { … }`
  ([§3](#3-functions) rule 3). A parameter is an irrefutable pattern with an optional type; a
  return type, when written, is followed by a block. A closure may read the locals, the
  parameters and the `self` of the body that contains it: each is captured by value where the
  closure is written, so the closure holds a copy and a later assignment of the local does
  not reach it. A closure **MUST NOT** call itself.
- **§1.5.15** `e?` takes an `Option<T>` and is allowed only in a function or a closure whose
  result type is an `Option`. It yields the `T` of a `Some`.
- **§1.5.16** `return`, `break` and `continue` are expressions that never produce a value:
  each fits any required type. So does a `loop` with no `break`.
- **§1.5.17** In the head of `if`, `while`, `match`, `for` and `on`, a struct expression is
  written in parentheses ([Appendix B.3.3](appendix_b_grammar.md) item 3).

**Dynamic semantics**

- **§1.5.18** Operands are evaluated left to right: the left operand of a binary operator
  before the right, the receiver before the arguments, the indexed value before the index.
  An assignment is the exception: its right operand is evaluated before its place.
- **§1.5.19** Integer `+`, `-` and `*` yield the mathematical result modulo `2^N`
  ([§1](#1-value-types) rule 6). Integer `/` truncates toward zero, and `%` is the remainder
  of that division, so `a == (a / b) * b + a % b` and the remainder has the sign of `a`. A
  zero divisor is a panic ([§1](#1-value-types) rule 11). For `Int<N>`, the least value
  divided by `-1` wraps to the least value, and its remainder is 0.
- **§1.5.20** Prefix `-` on an integer is `0 - x` modulo `2^N`, for `UInt<N>` as for `Int<N>`:
  `-1u4` is 15. Prefix `!` on an integer inverts every bit; on a `bool` it negates.
- **§1.5.21** `a << n` is `a · 2^n` modulo `2^N`, and 0 when `n >= N`. `a >> n` is the floor
  of `a / 2^n`: zeros enter from the top for `UInt<N>`, copies of the sign bit for `Int<N>`;
  when `n >= N` the result is 0, or `-1` for a negative `Int<N>`. A negative `n` is a panic.
- **§1.5.22** Float `+`, `-`, `*` and `/` follow §1.1.7. Float `%` is the remainder of the
  division truncated toward zero: exact, with the sign of the dividend. Prefix `-` on a float
  flips its sign.
- **§1.5.23** Integers compare by value and `false < true`. Floats compare as IEEE 754
  defines: `-0.0 == 0.0`, and NaN is neither equal to, less than nor greater than any value,
  itself included, so `x != x` is true exactly for NaN. Tuples and arrays compare field by
  field in order, and order lexicographically. `None` is less than every `Some`, and two
  `Some` compare by their payloads. Strings order by their scalar values.
- **§1.5.24** `a && b` evaluates `b` only when `a` is `true`; `a || b` evaluates `b` only when
  `a` is `false`. Both operands are `bool` ([§3.3.5](#33-uncertainty-and-absence)).
- **§1.5.25** An index outside `0 .. len`, a negative index included, is a panic. So is a
  range index that ends beyond the length or whose start exceeds its end.
- **§1.5.26** `e?` evaluates `e`. On `Some(v)` its value is `v`. On `None` the enclosing
  function or closure returns `None` at once.
- **§1.5.27** Operators on a library type, `Logic` included, are the functions of its
  operator-trait `impl`s ([Appendix H.10](appendix_h_library.md#h10-operator-traits)); the
  operators of this section are those of the built-in types.

> **Resolution.** Rules 1.5.20 and 1.5.21 complete [§1](#1-value-types) rule 6: no integer
> operator has an overflow error, and a shift by the width or more gives the mathematical
> result. A shift by a negative amount has no mathematical reading and panics. Rule 1.5.18
> fixes the one place where evaluation is not left to right.

**Examples**

```rust,fragment
let q = -7i8 / 2;             // -3
let r = -7i8 % 2;             // -1
let w = 255u8 + 1;            // 0
let s = 1u8 << 9;             // 0
let t = -1i8 >> 9;            // -1
let m = !0u4;                 // 15
let ok = 1 < 2 && 2.0 <= 2.0;
```

### 1.6 Patterns

A pattern is matched against a value. It matches or it does not, and when it matches it binds
names to parts of the value.

**Syntax**

```
pat            ::= '|'? pat1 ( '|' pat1 )*
pat1           ::= '_'
                 | '..'                                                         // inside a tuple, tuple-struct or array pattern
                 | IDENT ( '@' pat1 )?
                 | pat_bound ( '..' pat_bound? | '..=' pat_bound )?            // alone: a literal or a path pattern
                 | '..=' pat_bound
                 | path '(' ( pat ',' )* ')'
                 | path '{' ( IDENT ( ':' pat )? ',' )* '..'? '}'
                 | '(' ( pat ',' )* ')'
                 | '[' ( pat ',' )* ']'
pat_bound      ::= '-'? ( INT_LITERAL | FLOAT_LITERAL ) | STRING | 'true' | 'false' | path
```

**Static semantics**

- **§1.6.1** `_` matches any value and binds nothing.
- **§1.6.2** An identifier pattern `x` matches any value and binds `x` to it, unless
  [§1](#1-value-types) rule 14 makes it a path. `x @ p` matches when `p` matches and binds `x`
  to the whole value.
- **§1.6.3** A literal pattern is an integer, a float, a string, `true` or `false`; a number
  may carry a `-`. It matches the value that `==` finds equal to it.
- **§1.6.4** A range pattern is `lo..=hi` (through `hi`), `lo..hi` (below `hi`), `lo..` or
  `..=hi`. A bound is a literal or a path to a `const`, of an integer or a float type. The
  range **MUST NOT** be empty. There is no form `..hi`
  ([Appendix B.3.6](appendix_b_grammar.md#b36-forms-that-are-not-part-of-the-language)).
- **§1.6.5** A path pattern names a `const`, a unit struct or a variant without fields, and
  matches the value equal to it.
- **§1.6.6** A tuple pattern `(p, q)`, a tuple-struct pattern `Some(p)`, `Tx::Data(n)` and a
  struct pattern `Point { x, y: p }` match when every sub-pattern matches its field. In a
  struct pattern `x` alone stands for `x: x`; the pattern names every field or ends with `..`.
  In a tuple or tuple-struct pattern one `..` stands for the fields not written.
- **§1.6.7** An array pattern `[p, q]`, `[first, .., last]` or `[first, rest @ ..]` matches an
  array `[T; N]`. Without `..` it has exactly `N` sub-patterns. With `..`, written at most
  once, it has `k <= N` sub-patterns, and `name @ ..` binds the array of the elements not
  written, of type `[T; N - k]`. A pattern with another number of sub-patterns is a pattern of
  another type (§1.6.12).
- **§1.6.8** An or-pattern `p | q` matches when one alternative matches. Every alternative
  binds the same names, each with one type.
- **§1.6.9** A name a pattern binds holds a copy of the part it matched
  ([§1.4.6](#14-typing-inference-and-conversion)). The value matched is not changed and may be
  used again.
- **§1.6.10** A pattern binds a name at most once.
- **§1.6.11** A pattern is **irrefutable** when it matches every value of its type, and
  **refutable** otherwise. `_`, a binding, and a tuple, struct or array pattern of irrefutable
  parts are irrefutable. A `let`
  without `else`, a `for`, a closure parameter and a `let` of a module body take an
  irrefutable pattern.
- **§1.6.12** The type of every pattern is fixed by the value it is matched against; a
  pattern of another type is `E0180`.

**Dynamic semantics**

- **§1.6.13** Matching reads the value and has no other effect. Sub-patterns are tried left to
  right. A float pattern never matches NaN.
- **§1.6.14** A match is control: it runs the one arm the value selects
  ([§3.3.8](#33-uncertainty-and-absence)).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0184` | a `match` that is not exhaustive ([§1.8.6](#18-control-flow)); a refutable pattern where §1.6.11 requires an irrefutable one; a name bound twice in one pattern; alternatives of an or-pattern that bind different names or types; an empty range pattern; a struct pattern that leaves a field out without `..` | elaboration |

**Examples**

```rust,fragment
let (lo, hi) = (0.0, 1.0);
let Point { x, .. } = p;
let [a, b, c] = taps;
match code {
    0 => idle(),
    1..=9 => digit(code),
    10 | 11 => sign(code),
    n @ 12..=15 => other(n),
    _ => fault(),
}
```

### 1.7 Statements and blocks

**Syntax**

```
block          ::= '{' stmt* expr? '}'
stmt           ::= ';'
                 | attrs 'let' pat1 ( ':' type )? ( '=' expr ( 'else' block )? )? ';'
                 | attrs expr ';'
                 | attrs block_expr ';'?
                 | value_item
```

`block_expr` is an `if`, a `match`, a `while`, a `loop`, a `for` or a block.

**Static semantics**

- **§1.7.1** A block holds statements and, optionally, a final expression without `;`. Its
  type and its value are those of the final expression, and `()` when there is none.
- **§1.7.2** A block is a scope. A name declared in it is visible to its end and not after
  it. Scopes nest: a name of an inner scope hides the same name of an outer one.
- **§1.7.3** `let pat = expr;` matches the value of `expr` against the pattern and binds its
  names for the rest of the block. A later `let` of the same name declares a new variable,
  which may have another type, and hides the earlier one.
- **§1.7.4** `let pat: T = expr;` requires `expr` to have the type `T`.
- **§1.7.5** `let x;` and `let x: T;` declare a variable without a value. It **MUST** be
  assigned on every path before it is read (`E0183`).
- **§1.7.6** `let pat = expr else { … };` takes a refutable pattern. When the pattern does not
  match, the `else` block runs; that block **MUST NOT** complete: it ends in `return`,
  `break`, `continue` or a `loop` without `break`.
- **§1.7.7** Every local may be assigned again, with a value of its type
  ([§1.5.12](#15-expressions)). An assignment keeps the variable; a later `let` of the same
  name declares a new one (§1.7.3).
- **§1.7.8** `expr;` evaluates the expression and drops its value. An `if`, a `match`, a loop
  or a block written as a statement needs no `;`; its type is then `()`, unless it is the
  final expression of its block.
- **§1.7.9** A block may hold items: `fn`, `const`, `struct`, `enum`, `trait`, `impl` and
  `use`. They are visible in the whole block, before their declaration too
  ([§3.4.10](#34-files-and-imports)). An item of a block does not see the locals of that
  block; a closure does (§1.5.14).
- **§1.7.10** Inside a behavior block the statements also include the contribution and the
  commit ([Appendix B.3.4](appendix_b_grammar.md)). A module body is not a block: its items
  are those of [§8.2](part_iii_structure.md#82-the-body).

**Dynamic semantics**

- **§1.7.11** The statements of a block run in order. The initializer of a `let` is evaluated
  before the pattern is matched.

**Examples**

```rust,fragment
let acc = 0.0;
let n = 3;
let n = n as f64;             // a new `n`, of type f64
let Some(first) = taps.first() else { return 0.0; };
acc += first * n;             // the same `acc`, assigned again
```

### 1.8 Control flow

**Syntax**

```
if_expr        ::= 'if' cond block ( 'else' ( if_expr | block ) )?
cond           ::= 'let' pat '=' expr | expr                                   // no struct expression at the top (§1.5.17)
match_expr     ::= 'match' expr '{' match_arm* '}'                              // no struct expression at the top of expr
match_arm      ::= attrs pat ( 'if' expr )? '=>' ( block ','? | expr ',' | expr )   // the last form: the final arm only
while_expr     ::= 'while' cond block
loop_expr      ::= 'loop' block
for_expr       ::= 'for' pat1 'in' expr block                                  // no struct expression at the top of expr
```

**Static semantics**

- **§1.8.1** `if c { a } else { b }`: `c` is a `bool`, and the two blocks have one type, which
  is the type of the `if`. Without `else` the block has the type `()`.
- **§1.8.2** `if let pat = expr { … }` runs the block when the pattern matches, with its names
  bound in the block. `while let pat = expr { … }` repeats while it matches. The `let` is the
  whole condition: it is not an operand of `&&`.
- **§1.8.3** `match e { … }` evaluates `e` once and runs the first arm, top to bottom, whose
  pattern matches and whose guard, when it has one, is `true`. The names of the pattern are
  bound in the guard and in the body.
- **§1.8.4** The arms of a `match` have one type, which is the type of the `match`. An arm
  that never completes (§1.5.16) fits any.
- **§1.8.5** An arm whose body is a block needs no comma. Any other arm ends with a comma,
  which the last arm may omit.
- **§1.8.6** A `match` **MUST** be exhaustive: every value of the type of `e` matches the
  pattern of some arm without a guard. For an enum that means every variant; for `bool`, both
  values; for an integer type, every value of its range, by literals and ranges; for a float,
  a string or any type whose values the patterns cannot list, a final `_` or a binding. An arm no value can reach is not an error.
- **§1.8.7** `while c { … }` has the type `()`. `c` is a `bool`.
- **§1.8.8** `loop { … }` repeats until a `break`. Its type is the type of the values its
  `break`s give, and `()` when they give none.
- **§1.8.9** `for pat in e { … }` runs the block once for each element of `e`, in order, and
  has the type `()`. `e` is a range `a..b` or `a..=b` over an integer type (its elements
  ascend from `a`), an array (its elements, by value), or an iterator of
  [Appendix H](appendix_h_library.md), H.7 (`xs.iter()`, `(0..N).rev()`, …).
- **§1.8.10** `break` leaves the innermost enclosing loop and `continue` starts its next
  iteration. `break value` is allowed in a `loop` only. There are no loop labels.
- **§1.8.11** `return` and `return value` leave the innermost enclosing function or closure
  with that value; the value has the result type, and `return` alone gives `()`. The value of
  the body block is the result when no `return` runs.
- **§1.8.12** `break` and `continue` do not cross the body of a closure or of a function.
- **§1.8.13** Where a loop may be written, and whether its trip count must be a constant, is
  decided by the block that contains it ([§3](#3-functions) rule 12,
  [§8.3.4](part_iii_structure.md#83-instances-inside-expressions),
  [§11](part_iv_behavior.md#11-analog) rule 12,
  [§12.1.11](part_iv_behavior.md#121-reactions)).

**Dynamic semantics**

- **§1.8.14** Exactly one branch of an `if` and one arm of a `match` runs
  ([§3.3.8](#33-uncertainty-and-absence)).
- **§1.8.15** A `for` evaluates `e` once, before the first iteration. An empty range or array
  runs the block zero times; `a..b` is empty when `a >= b`.
- **§1.8.16** A `while` evaluates its condition before each iteration.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0185` | `break` or `continue` outside a loop; `break` with a value outside a `loop`; `return` outside a function and a closure; `?` in a function or closure whose result type is not an `Option`; the `else` block of a `let … else` that can complete | elaboration |
| §1.6 | a `match` that is not exhaustive, or a refutable pattern in a `for` (`E0184`) | elaboration |
| [§1.4](#14-typing-inference-and-conversion) | a condition that is not a `bool`, or branches or arms of different types (`E0180`) | elaboration |

**Examples**

```rust
fn classify(x: f64) -> i32 {
    if x < 0.0 { -1 } else if x == 0.0 { 0 } else { 1 }
}

fn first_set(bits: [bool; 8]) -> Option<u8> {
    let k = 0u8;
    while k < 8 {
        if bits[k] { return Some(k); }
        k += 1;
    }
    None
}

fn sum_to(n: u32) -> u32 {
    let acc = 0;
    for k in 0..=n { acc += k; }
    acc
}
```

## 2. `Repr`

**`Repr`** lays a value type out on levels. It is the only bridge from values to carriers. A
**level** is one boolean position of that layout. A level holds `false` or `true` and nothing
else ([§3.3](#33-uncertainty-and-absence)).

```rust
pub trait Repr {
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
- **§2 rule 7.** Behavior never depends on the encoding, except through `encode`, `decode`
  and `cast`.

> **Resolution.** Rules 5–6, and the element order of arrays and tuples, complete the layout
> the design gives by example (`Tx`: a 2-level tag and a 3-level payload).

**Dynamic semantics**

- **§2 rule 8.** The built-in and derived `encode` and `decode` move levels unchanged.
- **§2 rule 9.** A derived `decode` returns `None` for a tag that names no variant. The caller
  receives that `None`; no carrier is changed by it ([§3.3.9](#33-uncertainty-and-absence)).
- **§2 rule 10.** A hand-written `encode` or `decode` is an ordinary function. It has no rule
  of its own.
- **§2 rule 11.** In the byte form of a value, all-zero bytes are the bytes of `Logic::X`, of
  `None` and of the `Default` of every derived type that has one.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0110` | `#[derive(Repr)]` on a type with a field that does not implement `Repr` | elaboration |
| `E0111` | `#[repr(uN)]` with a discriminant that does not fit `N` levels, or two equal discriminants | elaboration |
| `E0112` | a type without `Repr` where a carried type is required | elaboration |

**Examples**

```rust
#[derive(Repr, PartialEq)]
enum Tx { Idle, Start, Data { n: u3 }, Stop }      // 2-level tag + 3-level payload

#[derive(Repr, PartialEq)] #[repr(u2)]
enum Op { Add = 0b00, Sub = 0b01, And = 0b10, Or = 0b11 }

#[derive(Repr, PartialEq)]
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
| solve-reading | probes (`v(a)`, `i(a, b)`) or reads the environment of the solve (`now()`, `freq()`, `gmin()`, `source_scale()`, `analysis(Static)`) | `analog`, event expressions, reaction bodies, body `let`s, value inputs of instances |
| event | returns `Event` ([§13](part_iv_behavior.md#13-events)) | event positions; as `bool` inside a reaction |
| emitting | calls `emit!` ([§19](part_vi_io.md#19-streams-and-checks)) | reactions, `analog` |

- **§3 rule 1.** Inside `analog`, calls are differentiated through
  ([§11](part_iv_behavior.md#11-analog) rule 5).
- **§3 rule 2.** `extern "C" fn` and `extern "intrinsic" fn` are implemented outside the language
  and have no body; a function that is neither `extern` nor a trait item has a body. Either mismatch is `E0125`. An
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
- **§3 rule 10.** A function accepts two ABI strings, `"C"` and `"intrinsic"`. The string
  `"intrinsic"` is accepted only in the crate `std`. An `extern "C" fn` takes and returns
  `bool`, integer and float scalars only. It **MUST** be a function of its arguments alone.
  An `extern "intrinsic" fn` has no body, is not restricted to scalar types, takes no
  `#[derivative]`, and has the contract its entry in Part VII gives.
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

> **Resolution.** `"intrinsic"` names an item the language cannot express and the engine
> implements, such as a function that reads the characters of a `str`. A user crate that
> needs native code writes `extern "C"`.

> **Resolution.** Rule 12 mirrors the rule for a module that places itself
> ([§8.2](part_iii_structure.md#82-the-body)): `analog` code is straight-line after
> elaboration.

**Dynamic semantics**

- **§3 rule 13.** A call evaluates its arguments left to right, then the body.
  A value input of a module is not an argument in this sense: it is a `let`
  ([§8.3.1](part_iii_structure.md#83-instances-inside-expressions)).
- **§3 rule 14.** A function has the same meaning in every analysis. What differs between
  analyses is what the probes and the environment it reads return
  ([§16.2](part_v_execution.md#162-every-primitive-in-every-solve)).
- **§3 rule 15.** An argument, a local and a result are values of their types
  ([§3.3.1](#33-uncertainty-and-absence)). An `extern` function receives and returns the bytes
  of the types it declares.
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
| `E0125` | an ABI string other than `"C"` and `"intrinsic"` on a function, the ABI `"intrinsic"` outside the crate `std`, a non-scalar type in its signature, an `extern` function with a body, or a function without a body that is neither `extern` nor a trait item | elaboration |
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
| a nature | one carrier, read through a handle | events: `rise`, `fall`; capabilities: `Resolve` |
| a bundle ([§7](part_ii_carriers.md#7-bundles)) | the bundle | events and queries over several carriers |

A **handle** is a read-only reference to one carrier or bundle, bound at elaboration. It is
the `self` of an `impl` on a nature or bundle, and any function parameter of such a type. It
names hardware by its identity ([§1.4.7](#14-typing-inference-and-conversion)); it is the
only reference a function holds, and it is not a value.

- **§3.1.1** On a nature or a bundle, `self` is a handle that reads, never a copy.
- **§3.1.2** Generic names follow `impl`; bounds come from the declaration
  (`impl<T> Stream<T>`).
- **§3.1.3** Names may not repeat a carrier method: `changed`, `prev`, `others`, `inflow`.
- **§3.1.4** There is no `impl` on a module. What a module's blocks share is a `let` in its
  body ([§8.2](part_iii_structure.md#82-the-body)).
- **§3.1.5** A handle may be read, probed, passed to another function, and have carrier
  methods called on it. It **MUST NOT** be returned, stored in a value, or written through.
- **§3.1.6** Reading a handle of a discrete carrier yields the carrier's value
  ([§5.1.1](part_ii_carriers.md#51-declaring-and-reading)). Through a handle, `others()` is
  taken from the module that made the call.
- **§3.1.7** A method call on a carrier expression is looked up in this order: the carrier
  methods ([§14](part_iv_behavior.md#14-core-primitives)), the `impl` blocks of its nature
  (trait `impl`s included, rule 9), then the methods of the value it reads.
- **§3.1.8** An `impl` on a nature with a defaulted type argument and no argument list is the
  `impl` of the default: `impl Bit` is `impl Bit<bool>`.
- **§3.1.9** `impl Trait for Type` is accepted for a value type and for a nature. For a
  nature, `self`, where a function has one, is a handle (rule 1), and the `impl` holds exactly
  the items of the trait. A trait that a nature implements is a **capability** of every
  carrier of that nature ([§3.7.13](#37-traits)): `Resolve` is the capability of taking
  several drives ([§4](part_ii_carriers.md#4-natures) rule 8). `impl Trait for` a bundle is
  `E0134`.
- **§3.1.10** On a value type, `self` is a copy of the receiver
  ([§1.4.6](#14-typing-inference-and-conversion)) and, like every parameter, not a place
  ([§1.5.12](#15-expressions)). A method that changes a value returns the new one, and the
  caller assigns it: `s = s.step(x);`. There is no other receiver form and no shorthand for
  that assignment.

> **Resolution.** An `impl` on a value type may hold associated `const` items
> ([§3.9](#39-constants)), read as `Type::NAME` (`Logic::Z`,
> [§22.1](part_vii_stdlib.md#221-values-and-bit)). On a nature or bundle an `impl` holds
> functions only (`E0132`).

> **Resolution.** Rules 5–9 define the handle and the lookup order, which the design uses
> (`clk.rise()`, `ipk.max(x)`, `w.s`) without stating.

> **Resolution.** An earlier revision refused every trait `impl` on a nature (`E0134`) because a
> trait function's `self` was taken to be a copy of a value and a bound to stand for a value
> type. A handle (rule 1) answers the first, and a parameter bound by `Nature`
> ([§3.8.2](#38-generics-and-bounds)) may carry a further bound, `W: Nature + Resolve<Driven>`,
> that the nature's `impl`s satisfy. Only a bundle, which is not a carrier, keeps the refusal.

> **Resolution.** Rule 10 gives a method no way to change its receiver in place. A parameter
> that could be assigned would make `fn bump(self) { self.count += 1; }` legal and silent: the
> copy changes and the caller's value does not. The assignment is `E0183`, and the method
> returns the new value.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0130` | an `impl` on a module | elaboration |
| `E0131` | a method named `changed`, `prev`, `others` or `inflow` | elaboration |
| `E0132` | an item other than a function in an `impl` on a nature or bundle | elaboration |
| `E0133` | a handle returned, stored in a value, or written through | elaboration |
| `E0134` | `impl Trait for` a bundle | elaboration |

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

### 3.3 Uncertainty and absence

A value is what its type says it is. A binary type holds binary values and nothing else. A type
that may hold "not determined" or "nothing" says so in its declaration: `Logic<N>`
([§22.1](part_vii_stdlib.md#221-values-and-bit)) for the first, `Option<T>` for the second.
This section states the six rules that follow. It has no errors of its own: each refusal is
the error of the rule that applies (`E0161`, `E0180`, `E0187`).

**Static semantics**

- **§3.3.1 A value is a value.** A value of every type is a value of that type. No level and
  no value carries a record of being established or not. `bool`, `UInt<N>`, `Int<N>`,
  `Float<N>`, enums, structs, tuples and arrays are always determined, and a read of one yields
  it with no method between the read and its use.
- **§3.3.2 Uncertainty is a type.** A type that may hold "not determined" declares it.
  `Logic<N>` has the four states 0, 1, X and Z on each of its `N` levels. Its operators are
  ordinary library `impl`s of the operator traits of
  [Appendix H.10](appendix_h_library.md#h10-operator-traits) and follow four-state tables; no
  rule of the language names `Logic`. Converting a binary value to a `Logic` is total
  (`From`); converting a `Logic` to a binary value gives an `Option`, `None` for X or Z.
- **§3.3.3 Absence is a type.** A type that may hold "nothing" declares it: `Option<T>` is
  `None` or `Some(x)`. `Option<Logic<64>>` is wholly `None` or a present four-state vector;
  the two forms of absence never mix inside one value.
- **§3.3.4 A carrier, a drive and a param start with a value.** The value is the initial value
  written, or `Default::default()` of the type
  ([Appendix H.8](appendix_h_library.md#h8-traits)). A type that does not
  implement `Default` requires the initial value, and its absence is `E0187` naming the
  carrier and the trait. `Option<T>`, `Logic<N>`, `()`, and a tuple, an array or a struct whose
  parts all implement `Default`, implement it; **`bool`, `UInt<N>`, `Int<N>`, `Float<N>` and
  `str` do not implement `Default`.** In this the language differs from Rust on purpose: a
  register with no reset value has no meaning that a number can express, so the source says
  which one it has.
- **§3.3.5 Control takes a `bool`.** The condition of an `if` or a `while`, a guard, and an
  operand of `&&` or `||` have the type `bool`; any other type is `E0180` naming the type
  found and `bool`. A `Logic` therefore decides nothing by itself: a model writes how, with
  `x.is_high()`, `same(a, b)`, `match x.at(0) { .. }` or `x.to_bit()`. `a == b` on two `Logic`
  values is a `Logic` (X when a compared level is X or Z), and `same(a, b)` is the `bool`
  that says the four states are equal level by level.
- **§3.3.6 `analog` reads values.** `analog` reads a discrete carrier like any expression: a
  carrier of `bool`, an integer, a float or an enum is read directly, with no gate. A solve
  computes with reals and `bool`s, so a `Logic` reaches it through a function that returns one
  (`x.to_volts(vdd)`, `x.is_high()`); a `Logic` where an `f64` or a `bool` is required is
  `E0180`.
- **§3.3.7** A decision that fixes structure (a generation condition, a loop bound of
  generation, a const generic, an array length, a `where` condition) is a `bool` or an
  integer, so it is determined by its type. A `const` of type `Logic` takes part in one only
  through `is_high()`, `same` or a `match`.

> **Resolution.** An earlier revision gave every level of every value a hidden record of
> whether it was established, and merged the effects of a branch that such a value decided.
> Everything that model described is what a type already says: a model that wants "may be X or Z" writes `Logic`, and one that
> wants "may be absent" writes `Option`. Appendix G.72 records the change. `E0150` is retired and `E0151` is retired, the codes of that model (§0.2: numbers are not reused).

**Dynamic semantics**

- **§3.3.8** An `if`, a `while`, a `match`, a guard and a short-circuit operand run exactly
  one path: the one the value of a `bool` or an enum selects. A path that is not selected does
  not run and cannot fail.
- **§3.3.9** A value that a hand-written `decode` rejects is the `None` the function returns
  ([§2](#2-repr) rule 9); no carrier is changed by it.

**Interactions**

| With | Rule | Defined in |
| --- | --- | --- |
| carriers | the initial value, written or `Default::default()`, is committed at `(0, 0)` | [§5.1.1](part_ii_carriers.md#51-declaring-and-reading) |
| changes | a commit that changes at least one level is a change; before a later change `prev()` is the initial value | [§5.3](part_ii_carriers.md#53-history) |
| events | a guard is a `bool`; a reaction whose guard is false does not run | [§13](part_iv_behavior.md#13-events) |
| `analog` and every solve | a discrete carrier is read directly; a `Logic` through a function (rule 6) | this section |
| small-signal | a discrete carrier is a constant of the linearization | [§11.1](part_iv_behavior.md#111-one-block-every-analysis) |
| output | a field holds exactly the bytes of its type | [§19](part_vi_io.md#19-streams-and-checks) |
| elaboration | structure is decided by a `bool` or an integer (rule 7) | this section |

**Examples**

```rust,fragment
module Demo(in clk: Bit) {
    signal en: Bit = false;               // a bool has no default: the value is written
    signal last: Bit<Option<u8>>;         // starts at None, Option has a default
    signal bus: Bit<Logic<4>>;            // starts at xxxx, Logic has a default
}

digital for Demo {
    on clk.rise() {
        if en { last <- Some(1); }        // `if` takes a bool: one path runs
        if same(bus, Logic::X) { en <- true; }  // a Logic decides through a function
        match last {
            Some(v) => { last <- Some(v + 1); }
            None => {}
        }
    }
}
```

### 3.4 Files and imports

One file is one module of the namespace. `mod` declares submodules, `use` brings names into
scope, `pub` controls visibility, a package of models is a crate ([§3.5](#35-crates)), and
the package holds it ([§3.6](#36-packages)).

**Syntax**

```
mod_decl       ::= 'mod' IDENT ( ';' | '{' item* '}' )
use_decl       ::= 'use' use_tree ';'
use_tree       ::= ( use_seg '::' )* ( '*' | '{' ( use_tree ',' )* '}' | use_seg ( 'as' IDENT )? )
use_seg        ::= IDENT | 'self' | 'super' | 'crate'
path           ::= path_seg ( '::' path_seg )*
path_seg       ::= ( IDENT | 'self' | 'Self' | 'super' | 'crate' ) ( '::'? generic_args )?   // in an expression the '::' is required
```

**Static semantics**

- **§3.4.1** A **namespace module** is a `mod` item or a file: `mod name { … }` declares one
  in place, and `mod name;` one whose items are in a file of its own
  ([§3.6](#36-packages) rule 2). "Module" without
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
  items. Paths, `use` and `pub` apply to all of them alike (rules 11 to 16).
- **§3.4.5** A hardware module's name is in the type namespace, as the type of its templates,
  and in the value namespace, as a callable
  ([§8.4](part_iii_structure.md#84-templates-and-swappable-modules)).
- **§3.4.6** `pub` on an item of a module body has its own meaning: visibility to the parent
  instance ([§8.2](part_iii_structure.md#82-the-body)).
- **§3.4.7** A namespace has two tables, one of types and one of values. A name is
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
- **§3.4.11** A path is one or more segments joined by `::`. The first segment is looked up by
  [§3.5](#35-crates) rules 4 and 5. Each later segment is looked up in what the segment before
  it names: an item of a namespace, a variant of an enum, or an associated function or constant
  of a type or a trait. `self` is the enclosing namespace, `super` its parent and `crate` the
  crate root ([§3.5](#35-crates) rule 7); `Self` is the type of the enclosing `impl` or trait.
- **§3.4.12** `use path;` brings the item the path names into the enclosing namespace or block
  under its last segment, and `use path as name;` under `name`. `use path::*;` imports every
  visible name of a namespace, or every variant of an enum. `use a::{b, c::d, self};` is the
  three imports `use a::b;`, `use a::c::d;` and `use a;`. A `use` path is resolved as any other
  path is. The name an import binds is private unless the `use` is `pub`.
- **§3.4.13** An item is visible in the namespace that declares it and in every namespace
  nested in that one. With `pub` it is visible wherever its namespace is. A field of a struct
  follows the same rule, field by field; the variants of an enum and their fields, and the
  items of a trait, are as visible as the enum or the trait. There is no other visibility:
  `pub` takes no argument.
- **§3.4.14** The items of a namespace are visible in all of it: an item may be used before
  its declaration, and the order of items has no meaning.
- **§3.4.15** Inside an item, names are looked up from the innermost scope outwards: the
  locals and items of each enclosing block, the parameters of the function or closure, the
  generic parameters of the item and of the `impl` or trait around it, then the namespace.
  A local is in scope from the end of its `let` to the end of its block
  ([§1.7](#17-statements-and-blocks)). A `fn` item does not see the locals or the generic
  parameters of a function that contains it; a closure sees both.
- **§3.4.16** A method name is not looked up in these tables. It is found from the type of its
  receiver ([§1.5.10](#15-expressions)).

> **Resolution.** Rules 7 to 9 are stated because the design is
> silent on duplicates. `E0162` is the one code for a collision; `E0143` keeps the overload.

> **Resolution.** Rule 5 places a module's name in both tables, as a tuple struct's name is
> (rule 7): it is a type and it is callable. Rule 1 fixes the
> vocabulary, since the design uses "module" for both.

> **Resolution.** Rules 11 to 16 state how names are found and who may see them. They add no
> error code: a name that is not found, or not visible, is `E0161`.

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
- **§3.5.5** The first segment of a path of several segments is looked up in the type
  namespace. A `mod`, `struct`, `enum`, `nature`, `module` or `trait`, or an
  import of one, is in it. A `fn`, a `const` or a local binding is in the value namespace and
  hides no dependency: `util::K` and `use util::K` reach the dependency `util` beside a local
  `fn util`. Where an item or an import of the type namespace of the enclosing namespace has
  the name of a dependency, an ordinary path follows rule 4: the item or import wins, without a diagnostic.
  A `use` path whose first segment is that name is ambiguous (`E0163`); the name that `use` binds is an errored binding, and a later read of it reports nothing further. There is no leading
  `::` ([Appendix B.3.6](appendix_b_grammar.md)). Reach the dependency under another key,
  `other = { package = "<name>", .. }` ([§3.6](#36-packages) rule 8), and the local item as
  `crate::<name>`.
- **§3.5.6** A dependency name resolves to the root namespace of that crate. A crate's own name
  is not in scope inside it.
- **§3.5.7** `crate` names the root of the crate that contains the path. `self` and `super`
  are relative to the namespace that contains them. `super` at a crate root does not resolve
  (`E0161`).
- **§3.5.8** From another crate an item is visible only when it is `pub` and every namespace on
  the path from its crate root to it is `pub`. `pub use` re-exports: the name it binds is an
  item of the namespace that holds the `use`, visible as `pub` makes it.
- **§3.5.9** The prelude is implied in every file of every crate, the files of `std` included.
- **§3.5.10** The dependencies of the crates form no cycle ([§3.6](#36-packages), `E0174`).
- **§3.5.11** `extern crate` is not part of the language
  ([Appendix B.3.6](appendix_b_grammar.md)).

> **Resolution.** Rules 4 and 5 in short: dependency names sit below a namespace's own
> type-namespace names in a path, and an import that could mean either is an error, never a
> silent choice. A value item of the same name is not a clash.
> They are stated because no `::` exists to disambiguate. A dependency name against a prelude
> item is not a clash: the dependency wins.

> **Resolution.** Rule 9 has no exception for `std`, so one rule holds in every crate.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0163` | a `use` path whose first segment names both a dependency and an item or an import of the enclosing namespace (one of the type namespace, rule 5) | elaboration |
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
  `mod name;` loads `name.msl` or `name/mod.msl` below the declaring file; neither or both is `E0161`.
  "Below" means: beside the declaring file when that file is the crate root or is itself named
  `mod.msl`, and otherwise in the directory named after the declaring file, without its
  extension.
- **§3.6.3** The manifest holds the tables and keys of the table below and no other (`E0171`).
  The `[run]` table is allowed: its content belongs to the host and is not defined by the
  language, and the `[run]` table of a dependency's manifest is not read.
- **§3.6.4** A package name and a dependency name are identifiers (A.1) that are no keyword of
  the language and not `std`. The package name is the crate's name. A single root file that is no package names its crate
  by its file stem, each character that is no letter, digit or `_` replaced by `_` and a `_`
  before a leading digit; a stem that is then a keyword or `std` is `E0171`, and the crate is
  named with a `_` appended.
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
  one commit) name the same crate. A repository URL is compared without a trailing `/` and a
  trailing `.git`, so one repository spelled two ways is one repository.
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

Tables the language allows and does not define (rule 3):

| Table | Content | Read in a dependency |
| --- | --- | --- |
| `[run]` | a table, defined by the host | no; not read in a dependency |

Requirement table (a requirement admits the versions that keep its leftmost non-zero number):

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
| `E0171` | a manifest entry rules 3 to 9 do not allow: an unknown table or key, a missing `name` or `version`, a value of the wrong type, a name that breaks rule 4, a version or requirement of another form, a dependency without exactly one source, a `branch`, `tag` or `rev` beside another or beside `path`, a `rev` that is not 7 to 40 hexadecimal digits, a root file stem that rule 4 forbids | package |
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

### 3.7 Traits

A trait names a set of functions and constants that a type provides. Traits are used as
bounds on generic parameters and in no other way: there is no trait object.

**Syntax**

```
trait_decl     ::= 'trait' IDENT generics? ( ':' path ( '+' path )* )? '{' impl_item* '}'
impl_block     ::= 'impl' generics? path generic_args? ( 'for' path generic_args? )? '{' impl_item* '}'
impl_item      ::= attrs 'pub'? ( fn_decl | const_item )
```

**Static semantics**

- **§3.7.1** A trait holds functions and constants. A function may have a body, its default,
  or end in `;`. A constant may have a value, its default, or none. A trait holds no type.
- **§3.7.2** Inside a trait and inside an `impl`, `Self` is the type the trait is implemented
  for, and `Self::NAME` one of its associated items.
- **§3.7.3** `trait A: B + C` makes `B` and `C` supertraits of `A`: a type that implements `A`
  **MUST** implement `B` and `C`, and the items of `B` and `C` may be used on `Self` inside
  `A`.
- **§3.7.4** `impl Tr for T { … }` implements the trait `Tr` for the value type or nature `T`
  ([§3.1.9](#31-impl)). It defines every item of the trait that has no default and may define
  the others; it defines no item the trait does not have. Each item has the signature the
  trait gives, with `Self` standing for `T`.
- **§3.7.5** An `impl` may be generic, and its type may be a generic parameter:
  `impl<T: Repr> Describe for T {}` implements `Describe` for every type that implements `Repr`.
- **§3.7.6** For one trait and one type at most one `impl` applies. Two `impl`s of a trait
  that both apply to some type are an error, whether or not that type is used.
- **§3.7.7** `impl Tr for T` is written in the crate that declares `Tr` or in the crate that
  declares `T`. `impl T { … }`, without a trait, is written in the crate that declares `T`;
  the built-in types of [§1](#1-value-types) take no such `impl`.
- **§3.7.8** A function of a trait is called as a method, `x.f()`, where the trait is in scope:
  declared in, imported into, or reached through the prelude from the namespace of the call.
  It is also called by a path, `Tr::f(x)` or `T::f(x)`, where the trait need not be in scope.
  A function of the type's own `impl` is found before a trait's
  ([§1.5.10](#15-expressions)). Two traits in scope that both give the method are
  [G.67](appendix_g_scope.md).
- **§3.7.9** A function whose first parameter is `self` is a method. One without `self` is an
  associated function and is called by a path, `Logic::wired(d)`. `self` is written first and
  only in an `impl` or a trait.
- **§3.7.10** A trait is named in three places: after a generic parameter as a bound
  ([§3.8](#38-generics-and-bounds)), after `impl` in `impl Tr for T`, and inside
  `#[derive(…)]`. It is not a type.
- **§3.7.11** The items of a trait are visible wherever the trait is. The items of an `impl`
  of a trait take no `pub`.
- **§3.7.12** The traits of the language itself are `Repr` ([§2](#2-repr)), `Nature`
  ([§4](part_ii_carriers.md#4-natures)), the `Fn` bounds
  ([§3.8.6](#38-generics-and-bounds), [§8.4](part_iii_structure.md#84-templates-and-swappable-modules))
  and those of [Appendix H](appendix_h_library.md), H.8 and H.10, and `Resolve`
  ([§4](part_ii_carriers.md#4-natures) rule 8). `Nature` and `Fn` take no `impl`.
- **§3.7.13** A trait is a **capability**. What a type or a nature can do is the set of traits
  it implements, and a rule of the language that needs a capability asks for the trait and
  names it in its diagnostic: `Repr` is the capability of being carried, `Default` of starting
  without a value, `BitAnd` of `&`, `Resolve` of taking several drives. A generic parameter
  bounds a type or a nature by its capabilities (`T: Repr + Default`,
  `W: Nature + Resolve<Driven>`).

> **Resolution.** Rules 6 and 7 keep the set of `impl`s that apply to a type the same in every
> crate that sees it, so a model means the same wherever it is compiled.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0187` | an `impl Tr for T` that leaves out an item without a default, defines an item the trait does not have, or gives an item another signature; a type that does not implement a supertrait; two `impl`s that overlap (§3.7.6); an `impl` in a crate that §3.7.7 does not allow; `self` outside an `impl` or a trait, or not first; a generic argument that does not satisfy its bound, or a wrong number of generic arguments ([§3.8](#38-generics-and-bounds)); a `pub` on an item of an `impl Tr for T`; a carrier, a drive or a param with no initial value whose type does not implement `Default` (§3.3.4), `#[derive(Default)]` on a type with a field that does not, and `T::default()` on a type that does not (Appendix H.8 rule 4) | elaboration |
| [§3.1](#31-impl) | `impl Tr for` a bundle (`E0134`) | elaboration |

**Examples**

```rust
trait Area {
    const SIDES: u32;
    fn area(self) -> f64;
    fn is_empty(self) -> bool { self.area() == 0.0 }
}

struct Square { side: f64 }

impl Area for Square {
    const SIDES: u32 = 4;
    fn area(self) -> f64 { self.side * self.side }
}

fn total<T: Area, const N: usize>(shapes: [T; N]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}
```

### 3.8 Generics and bounds

An item is generic when it has parameters that stand for types, natures or constants. A
generic item is a recipe: elaboration makes one copy for each list of arguments it is used
with.

**Syntax**

```
generics       ::= '<' ( generic_param ',' )* '>'
generic_param  ::= IDENT ( ':' bound ( '+' bound )* )? ( '=' type )?
                 | 'const' IDENT ':' type ( '=' ( literal | block ) )?
bound          ::= path | path '(' ( type ',' )* ')' ( '->' type )?             // the second form: Fn(A, B) -> R
generic_args   ::= '<' ( generic_arg ',' )* '>'
generic_arg    ::= type | literal | '-' literal | block
```

**Static semantics**

- **§3.8.1** A `fn`, a `struct`, an `enum`, a `trait`, an `impl`, a `nature` and a `module` may
  have a generic parameter list. A parameter is a type parameter, `T`, or a const parameter,
  `const N: u32`. There are no lifetime parameters.
- **§3.8.2** A type parameter may have bounds, `T: Repr + PartialEq`, and a default, `T = bool`. It
  stands for a value type, or for a nature when it is bound by `Nature`
  ([§4](part_ii_carriers.md#4-natures) rule 11), or for a module when it is bound by `Fn(P)`
  ([§8.4](part_iii_structure.md#84-templates-and-swappable-modules)).
- **§3.8.3** A const parameter has an integer type or `bool`, and may have a default: a
  literal or a block. It is a const expression wherever it is in scope.
- **§3.8.4** Parameters with a default come after those without. The names of one list are
  distinct, and each is in scope in the whole item, the list itself included.
- **§3.8.5** Generic arguments are written after the name, `Bit<u8>`, `Delay<u8, 4>`, in the
  order of the parameters. In an expression they follow `::`, as in `idt::<Heat>(x)`, so that
  `<` is not read as a comparison. A const argument is a literal, a literal with `-`, a single
  name, or a block `{ N + 1 }` for any other const expression.
- **§3.8.6** A bound is a trait ([§3.7](#37-traits)), or the form `Fn(T1, T2) -> U`, which
  holds for every function and closure that can be called with arguments of those types and
  yields a `U`. On a module parameter the form is `Fn(P)`
  ([§8.4.5](part_iii_structure.md#84-templates-and-swappable-modules)).
- **§3.8.7** An argument **MUST** satisfy every bound of its parameter. An argument that is
  left out takes the parameter's default; in an expression, one with no default is inferred
  from the arguments and the required type of the call
  ([§1.4.4](#14-typing-inference-and-conversion)).
- **§3.8.8** Bounds are written in the parameter list. `where` holds a boolean const
  expression ([§3](#3-functions) rule 8), not a bound.
- **§3.8.9** A generic item is checked at each instantiation, with its parameters replaced by
  the arguments ([§3](#3-functions) rule 9): a type built from a const parameter, such as
  `[f64; N + 1]`, is compared by value there. Inside the item, a value of a type parameter
  offers what the bounds of the parameter offer.
- **§3.8.10** Two uses of a generic item with equal arguments are the same instance, and a
  generic type with equal arguments is the same type.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| [§3.7](#37-traits) | a generic argument that does not satisfy its bound, or a wrong number of arguments (`E0187`) | elaboration |
| [§1.4](#14-typing-inference-and-conversion) | a generic argument that cannot be inferred (`E0181`) | elaboration |
| [§3](#3-functions) | an error inside an instantiation, reported at elaboration; a false `where` (`E0127`) | elaboration |

**Examples**

```rust
struct Pair<T> { a: T, b: T }

fn swap<T>(p: Pair<T>) -> Pair<T> { Pair { a: p.b, b: p.a } }

fn apply<F: Fn(f64) -> f64>(f: F, x: f64) -> f64 { f(x) }

fn widen<const N: usize>(xs: [f64; N]) -> [f64; N + 1] {
    let wide = [0.0; N + 1];
    for k in 0..N { wide[k] = xs[k]; }
    wide
}
```

### 3.9 Constants

**Syntax**

```
const_item     ::= 'const' IDENT ':' type ( '=' expr )? ';'
```

**Static semantics**

- **§3.9.1** `const NAME: T = expr;` names a value fixed at elaboration. The type is written
  and the value is a const expression of that type. Only a constant of a trait may leave the
  value out ([§3.7.1](#37-traits)).
- **§3.9.2** A **const expression** is an expression that elaboration can evaluate. It is
  built from literals, constants, const parameters, params where the position
  allows them ([§8.1](part_iii_structure.md#81-ports-and-params)), the operators and
  conversions of [§1.5](#15-expressions), tuple, array and struct expressions, field accesses
  and indexes, blocks, `if`, `match` and loops, closures, the methods of
  [Appendix H](appendix_h_library.md), and calls of `const fn` functions.
- **§3.9.3** A const expression **MUST NOT** read a carrier, a probe or the environment
  ([§8.1.21](part_iii_structure.md#81-ports-and-params)), place an instance, call a function
  that is not `const fn`, or contain `emit!`. `include_str!` is a const expression and is
  allowed only as the initializer of a `const`
  ([§20](part_vi_io.md#20-input-data)).
- **§3.9.4** A `const fn` is a function that may be called in a const expression. Its body
  holds only what rule 2 allows. It may also be called outside a const context, with the same
  result.
- **§3.9.5** A constant may refer to constants declared after it. A constant **MUST NOT**
  depend on itself.
- **§3.9.6** A use of a constant is a copy of its value. A constant has no storage and is not
  a place ([§1.5.12](#15-expressions)).

**Dynamic semantics**

- **§3.9.7** A const expression is evaluated at elaboration, by the rules that evaluate it at
  a run: the same wrapping, the same rounding, the same panics. A panic is `E0104`
  ([§1](#1-value-types) rule 11).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0188` | the initializer of a `const`, an array length, a bound of a range index ([§1.5.9](#15-expressions)) or a const argument that is not a const expression (§3.9.2, §3.9.3); a `const fn` whose body holds a form rule 3 forbids; a `const` without a value outside a trait; a constant that depends on itself | elaboration |
| [§1](#1-value-types) | a panic while a const expression is evaluated (`E0104`) | elaboration |

**Examples**

```rust
const N_TAPS: usize = 4;
const GAIN: f64 = 2.0 * PI;
const fn square(x: f64) -> f64 { x * x }
const AREA: f64 = square(GAIN);
```
