# Appendix A. Lexical structure

The lexical structure is close to Rust's, with five deltas. This appendix states the deltas and
the token-level errors. The syntactic grammar is [Appendix B.3](appendix_b_grammar.md).

Error codes of this appendix: `E09xx`.

## A.1 Close to Rust, not Rust

Identifiers, comments, attributes, literals and the value syntax are Rust's (edition 2024). The
lexer and the grammar are the language's own.

**Static semantics**

1. Source text is UTF-8. Whitespace, line comments `//` and block comments `/* */` separate
   tokens as in Rust.
2. The lexer adds the tokens of A.2, A.3 and A.5. The grammar adds the productions of
   [B.3](appendix_b_grammar.md).
3. The grammar drops what [§1](part_i_values.md#1-value-types) rule 2 leaves out: mutable
   references, heap types, `unsafe`, threads, `async` and `dyn`.
4. **Macros are dropped.** There is no macro definition and no macro invocation. The only two
   `!` forms are the built-ins `emit!` ([§19](part_vi_io.md#19-streams-and-checks)) and
   `include_str!` ([§20](part_vi_io.md#20-input-data)). They are grammar forms, not macros.
5. Any other `path '!'` followed by `(`, `[` or `{` is a parse error. So is `macro_rules`.

**Keywords**

| Class | Words | Reserved |
| --- | --- | --- |
| added keywords | `module` `nature` `signal` `on` `out` `flip` `after` | everywhere |
| Rust keywords with a new use | `in` (port direction), `extern` (`extern` modules) | everywhere, as in Rust |
| contextual keywords | `analog`, `digital` | only before `for` or `<` at item position |
| contextual keywords | `potential`, `flow`, `value`, `resolve` | only as a field name inside a `nature` declaration |
| Rust keywords | every strict and reserved keyword of Rust 2024 | everywhere |

6. An added keyword **MUST NOT** be used as an identifier. Raw identifiers for them
   (`r#out`) are an open item ([G.15](appendix_g_scope.md)).
7. A contextual keyword is an ordinary identifier outside its context. `potential` and `flow`
   are also the names of the generic probes ([§6](part_ii_carriers.md#6-continuous-carriers)).
8. `gnd`, `Nature`, `m`, `dtemp` and `str` are core names, as are `settle`, `inf`, the
   environment functions and the core primitives of [§14](part_iv_behavior.md#14-core-primitives).
   No core name is a keyword.

> **Resolution.** Rust's keywords stay reserved even where their construct is dropped
> (`unsafe`, `async`, `await`, `dyn`, `mut` outside `let mut`, and the reserved-for-future
> words). Using one where no production accepts it is a parse error. This keeps every model
> lexically valid Rust apart from the tokens this appendix adds.

> **Resolution.** `out` is reserved, so `signal out: Electrical;` is a parse error. A carrier
> that would be called `out` takes another name (`vout`, `y`). The same holds for `on`,
> `after`, `flip`, `signal`, `module` and `nature`.

> **Resolution.** Only the lifetime `'static` is accepted, and only in `&'static str`. Any
> other lifetime token is an error. `char` is not a value type
> ([§1](part_i_values.md#1-value-types)): character literals, byte literals and byte strings
> are errors ([G.61](appendix_g_scope.md)).

**Attributes.** The attribute syntax is Rust's. The recognized attributes are:

| Attribute | On | Defined in |
| --- | --- | --- |
| `#[derive(…)]` with `Repr` and the traits of [Appendix H](appendix_h_library.md) (H.8) | `struct`, `enum` | [§2](part_i_values.md#2-repr) |
| `#[repr(uN)]` | `enum` | [§2](part_i_values.md#2-repr) |
| `#[derivative(f, …)]` | `extern "C" fn` | [§3](part_i_values.md#3-functions) rules 2 and 11 |
| `#[link(…)]` | `extern` module | [§8.5](part_iii_structure.md#85-implicit-params-and-extern-modules) |
| `#[doc = "…"]` | any item | A.4: the form a doc comment stands for |

> **Resolution.** An attribute that is not in this table is an error, `#[cfg]` and `#[allow]`
> included: a design has one elaboration and the language has no lints. Attributes are not an
> extension point ([G.61](appendix_g_scope.md)).

## A.2 SI-suffixed reals

A decimal number followed by one of `T G M k m u n p f a` is a real literal scaled by a power
of ten. The suffix is a multiplier, not a unit.

**Syntax** (lexical)

```
si_literal ::= DEC_DIGITS ( '.' DEC_DIGITS )? SI_SUFFIX
SI_SUFFIX  ::= 'T' | 'G' | 'M' | 'k' | 'm' | 'u' | 'n' | 'p' | 'f' | 'a'
```

`DEC_DIGITS` is Rust's decimal digit sequence, underscores included.

> **Resolution.** This lexical grammar is not part of B.3, which lists syntactic productions
> only. It is the complete definition of the token.

| Suffix | Multiplier | Suffix | Multiplier |
| --- | --- | --- | --- |
| `T` | 1e12 | `u` | 1e-6 |
| `G` | 1e9 | `n` | 1e-9 |
| `M` | 1e6 | `p` | 1e-12 |
| `k` | 1e3 | `f` | 1e-15 |
| `m` | 1e-3 | `a` | 1e-18 |

**Static semantics**

1. An SI-suffixed literal has type `f64`, whether or not it has a fractional part: `1k` is
   `1000.0_f64`.
2. `1m` is milli and `1M` is mega. Suffixes are case-sensitive.
3. The suffix is recognized by longest match. A suffix letter that is followed by an
   identifier-continue character is not an SI suffix: `1u8` is a `u8`, `1f64` is an `f64`,
   `1u` is `1e-6`.
4. A literal with an SI suffix has no exponent and no type suffix. `1e3k` and `1k_f32` are
   errors.
5. Only decimal literals take an SI suffix. In `0xf` and `0b1a`, the letter is a digit or an
   error, as in Rust.
6. The value is the nearest `f64` to the exact decimal product. `2.2u` equals the literal
   `2.2e-6`. It is not the rounded product `2.2 * 1e-6`.
7. A leading `-` is the negation operator, not part of the literal.

> **Resolution.** Rules 3–6 settle the cases the design leaves to "a number followed by a
> suffix": the clash with Rust's type suffixes, non-decimal bases, exponents, and rounding.

> **Resolution.** An integer literal is never read as a float: `r: f64 = 100;` is a type
> error, as in Rust. Write `100.0`, or use a suffix. The only implicit conversion is that of
> [§3.2.3](part_i_values.md#32-parameters-and-calls).

**Examples**

```rust,fragment
r:  f64 = 1k;          // 1000.0
c:  f64 = 2.2u;        // 2.2e-6
cs: f64 = 10f;         // 1e-14
tr: f64 = 20p;
hv: f64 = 2.26M;       // 2.26e6
rl: f64 = 1T;
```

## A.3 Unknown values

`Unknown` is the literal of any `Repr` type with every level unknown
([§3.3](part_i_values.md#33-unknown-values)).

**Static semantics**

1. `Unknown` is a literal token, like `true` and `false`. It **MUST NOT** be redeclared or
   shadowed.
2. Its type is inferred from its context, as for an integer literal. The inferred type
   **MUST** implement `Repr`.
3. A value written level by level is the stdlib function `logic("10xx")`
   ([§22.1](part_vii_stdlib.md#221-values-and-bit)), not a literal form. Its argument is an
   ordinary string literal.

> **Resolution.** The design lists `Unknown` among the literals and never says whether it is a
> name. It is a reserved literal, so that no scope changes what `Unknown` means
> ([G.61](appendix_g_scope.md)).

## A.4 Doc comments

`///` and `//!` doc comments (and the block forms `/** */`, `/*! */`) are metadata ([§21](part_vi_io.md#21-metadata)).

**Static semantics**

1. A doc comment, like an attribute, attaches to the node that follows it, as in Rust: a module,
   a port, a param, a signal, a `let`, a nature, a type, a function. Beyond these, an attribute may precede
   every statement, a reaction, a definition, a generation `for` or `if`, a nature field, a
   contribution and a commit; a doc comment is an attribute and goes in the same places.
2. The text is kept verbatim in the metadata of that node. Of the docs, the doc comments of a quantity and of
   a reaction are metadata ([§21](part_vi_io.md#21-metadata)); those of a statement are kept in
   the tree and have no other effect.
3. An ordinary comment (`//`, `/* */`) is discarded.

## A.5 Two operators

`+>` (contribution) and `<-` (commit) are single tokens, read by longest match.

**Static semantics**

1. `+>` is accepted only in a `contribution`, inside `analog`
   ([§11](part_iv_behavior.md#11-analog)).
2. `<-` is accepted only in a `commit`, inside a reaction
   ([§12.1](part_iv_behavior.md#121-reactions)).
3. `a<-b` is the tokens `a`, `<-`, `b`: a commit. A comparison with a negative operand is
   written `a < -b`.
4. The same holds in generic arguments: `Int<-1>` does not parse; write `Int< -1 >` or
   `Int<{ -1 }>`.
5. `a+>b` is the tokens `a`, `+>`, `b`. No Rust expression contains `+` directly followed by
   `>`, so no value expression changes meaning.
6. Neither token is an expression operator. Neither has precedence or associativity: each ends
   the left side of its statement.

## A.6 Errors

| Code | Condition | Phase |
| --- | --- | --- |
| `E0900` | a syntax error no other row names: an unexpected token (the message names the token found and the constructs expected), an unterminated block comment or string literal, an unexpected character, an unknown literal suffix, an unmatched closing delimiter, or source that is not valid UTF-8 | parse |
| `E0901` | a macro invocation other than `emit!` or `include_str!`, or a macro definition | parse |
| `E0902` | a malformed SI literal: a suffix with an exponent, with a type suffix, or on a non-decimal literal | parse |
| `E0903` | a keyword used as an identifier, or a Rust keyword where no production accepts it | parse |
| `E0904` | an attribute that A.1 does not list, or a listed attribute on the wrong kind of item | elaboration |
| `E0905` | a token of a dropped class: a character or byte literal, a byte string, a lifetime other than `'static` | parse |
| `E0906` | `Unknown` where the inferred type does not implement `Repr`, or where no type can be inferred | elaboration |
| `E0907` | `+>` or `<-` inside an expression, where no statement form accepts it. A well-formed `contribution` or `commit` in a block of the wrong kind is `E0404` | parse |
| `E0908` | `Unknown` used as a binding, item or field name | parse |
| `E0910` | nesting deeper than 256 levels (delimiters and operator chains) | parse |
