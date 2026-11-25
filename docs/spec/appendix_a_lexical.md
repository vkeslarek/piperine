# Appendix A. Lexical structure

This appendix is normative. It defines the lexical structure of the language: how source text
becomes tokens, and the token-level errors. A.1 gives the token classes and the keywords, A.2
to A.5 the tokens with rules of their own, A.6 the errors, and A.7 to A.10 the identifiers, the
literals, the comments and the punctuation. The syntactic grammar is
[Appendix B.3](appendix_b_grammar.md).

Error codes of this appendix: `E09xx`.

## A.1 Source text and tokens

The lexer turns source text into a sequence of tokens. A token is an identifier (A.7), a
keyword (this section), a literal (A.2, A.8), a doc comment (A.4) or a
punctuation token (A.5, A.10).

**Static semantics**

1. Source text is UTF-8. Whitespace, line comments `//` and block comments `/* */` separate
   tokens and are otherwise ignored (A.9). A token is the longest sequence of characters that
   forms one, read left to right; A.10 rule 2 states the one exception.
2. The tokens are those of this appendix and no other. The SI-suffixed literal (A.2) and the
   operators `+>` and `<-` (A.5) have sections of their own. The
   grammar is [B.3](appendix_b_grammar.md).
3. The language has no token and no production for what
   [§1](part_i_values.md#1-value-types) rule 2 leaves out: mutable
   references, heap types, `unsafe`, threads, `async` and `dyn`.
4. **There are no macros.** There is no macro definition and no macro invocation. The only two
   `!` forms are the built-ins `emit!` ([§19](part_vi_io.md#19-streams-and-checks)) and
   `include_str!` ([§20](part_vi_io.md#20-input-data)). They are grammar forms, not macros.
5. Any other `path '!'` followed by `(`, `[` or `{` is a parse error. So is `macro_rules`.

**Keywords**

| Class | Words | Reserved |
| --- | --- | --- |
| added keywords | `module` `nature` `signal` `on` `out` `flip` `after` | everywhere |
| value keywords with a second use | `in` (port direction, beside `for … in`), `extern` (`extern` modules, beside `extern "C" fn`) | everywhere |
| contextual keywords | `analog`, `digital` | only before `for` or `<` at item position |
| contextual keywords | `potential`, `flow`, `value` | only as a field name inside a `nature` declaration |
| value keywords | `as` `break` `const` `continue` `crate` `else` `enum` `extern` `false` `fn` `for` `if` `impl` `in` `let` `loop` `match` `mod` `pub` `return` `self` `Self` `struct` `super` `trait` `true` `use` `where` `while` | everywhere |
| reserved words | `abstract` `async` `await` `become` `box` `do` `dyn` `final` `gen` `macro` `move` `mut` `override` `priv` `ref` `static` `try` `type` `typeof` `unsafe` `unsized` `virtual` `yield` | everywhere; no production accepts one |

The **added keywords** are the seven words of the first row: the keywords of the mixed-signal
constructs. The value keywords are the keywords of the value language ([Part I](part_i_values.md)).

6. An added keyword **MUST NOT** be used as an identifier, but `r#name` is an identifier for each
   of the seven added keywords and for no other word: `r#module`, `r#nature`, `r#signal`, `r#on`,
   `r#out`, `r#flip` and `r#after`. Any other `r#word` (`r#type`, `r#in`, `r#foo`) is `E0900`,
   and its message lists the seven. The identifier's name is the word without `r#`, and
   metadata lists the name without `r#` ([G.15](appendix_g_scope.md)). The printer writes the
   raw form back.
7. A contextual keyword is an ordinary identifier outside its context. `potential` and `flow`
   are also the names of the generic probes ([§6](part_ii_carriers.md#6-continuous-carriers)).
8. `gnd`, `Nature` and `str` are core names, as are `settle`, `inf`, the
   environment functions and the core primitives of [§14](part_iv_behavior.md#14-core-primitives).
   No core name is a keyword.

> **Resolution.** The reserved words name constructs the language does not have (`unsafe`,
> `async`, `await`, `dyn`, `static`, `type`) or are kept free for later use. Using one where no
> production accepts it is a parse error (`E0903`); so is a value keyword outside its
> productions. `mut`, `ref` and `move` are reserved words because the language has no
> reference and no binding that cannot be assigned ([§1](part_i_values.md#1-value-types)
> rule 2): `let mut x`, `ref x` and `move |x| …` are each `E0903` at the word, and
> [B.3.6](appendix_b_grammar.md#b36-forms-that-are-not-part-of-the-language) says what to
> write.

> **Note (informative).** The value keywords and the reserved words together are the strict and
> reserved keywords of Rust, edition 2024, so no MSL identifier is a Rust keyword.

> **Resolution.** `out` is reserved, so `signal out: Electrical;` is a parse error. A carrier
> that would be called `out` takes another name (`vout`, `y`) or the raw form,
> `signal r#out: Electrical;`, which declares a signal named `out`. The same holds for `on`,
> `after`, `flip`, `signal`, `module` and `nature`. `r#in` and `r#extern` stay errors: they are
> value keywords, and no value keyword has a raw form.

```rust
nature Bit { value: bool }
module Gate() {
    signal r#out: Bit;
    let y = r#out;
}
```

> **Resolution.** No lifetime token is accepted, `'static` included: the language has no
> reference for one to bound, and a string is the value type `str`
> ([§1.2.9](part_i_values.md#12-composite-types)). `char` is not a value type
> ([§1](part_i_values.md#1-value-types)): character literals, byte literals and byte strings
> are errors ([G.61](appendix_g_scope.md)).

**Attributes.** An attribute is `#[path]`, `#[path(arguments)]` or `#[path = expr]`
([B.3.7](appendix_b_grammar.md#b37-the-value-grammar), `attr`). It is written before the node it
applies to (A.4 rule 1). There is no inner form `#![…]`. The recognized attributes are:

| Attribute | On | Defined in |
| --- | --- | --- |
| `#[derive(…)]` with `Repr` and the traits of [Appendix H](appendix_h_library.md) (H.8) | `struct`, `enum` | [§2](part_i_values.md#2-repr) |
| `#[repr(uN)]` | `enum` | [§2](part_i_values.md#2-repr) |
| `#[derivative(f, …)]` | `extern "C" fn` | [§3](part_i_values.md#3-functions) rules 2 and 11 |
| `#[link(…)]` | `extern` module | [§8.5](part_iii_structure.md#85-run-settings-and-extern-modules) |
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

`DEC_DIGITS` is the decimal digit sequence of A.8: a digit, then digits and underscores.

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
5. Only decimal literals take an SI suffix. In `0xf` the letter is a hexadecimal digit, and in
   `0b1a` it is an unknown suffix (`E0900`).
6. The value is the nearest `f64` to the exact decimal product. `2.2u` equals the literal
   `2.2e-6`. It is not the rounded product `2.2 * 1e-6`.
7. A leading `-` is the negation operator, not part of the literal.
8. A literal out of range is `E0909`: an SI-suffixed float whose nearest `f64` is infinite (a
   310-digit number followed by `T`; an unsuffixed float is read in the format of its position, so
   `1e999` is `E0909` as an `f64` and is the finite value it names as an `f128`, A.8 rule 8), and an integer that exceeds `2^N - 1` with a suffix `uN`,
   `2^(N-1)` with a suffix `iN`, or `2^128 - 1` without a suffix. The token is still produced.

> **Resolution.** An `iN` literal admits `2^(N-1)`, so `128i8` lexes: the minus sign is an
> operator (rule 7), and `-128i8` is the negation of `128i8`.

> **Resolution.** Rules 3–6 settle the cases the design leaves to "a number followed by a
> suffix": the clash with the type suffixes of A.8, non-decimal bases, exponents, and rounding.

> **Resolution.** An integer literal is never read as a float: `r: f64 = 100;` is a type
> error ([§1.4.3](part_i_values.md#14-typing-inference-and-conversion)). Write `100.0`, or
> use a suffix. The only implicit conversion is that of
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

## A.3 Retired

This section number is retired and is not reused. It defined a literal that the language no
longer has: what a type may hold is what its declaration says
([§3.3](part_i_values.md#33-uncertainty-and-absence), [Appendix G.72](appendix_g_scope.md)).

## A.4 Doc comments

`///` and `//!` doc comments (and the block forms `/** */`, `/*! */`) are metadata ([§21](part_vi_io.md#21-metadata)).

**Static semantics**

1. A doc comment, like an attribute, attaches to the node that follows it: a module,
   a port, a param, a signal, a `let`, a nature, a type, a function. Beyond these, an attribute may precede
   every statement, a reaction, a definition, a generation `for` or `if`, a nature field, a
   contribution and a commit; a doc comment is an attribute and goes in the same places. Neither goes on a parameter, a generic
   parameter or a struct-literal field ([B.3.6](appendix_b_grammar.md#b36-forms-that-are-not-part-of-the-language)). A doc comment
   followed by `}`, by `else` or by the end of input documents nothing and is `E0900`.
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
5. `a+>b` is the tokens `a`, `+>`, `b`. No value expression contains `+` directly followed by
   `>`, so the token takes nothing from the value language.
6. Neither token is an expression operator. Neither has precedence or associativity: each ends
   the left side of its statement.

## A.6 Errors

| Code | Condition | Phase |
| --- | --- | --- |
| `E0900` | a syntax error no other row names: an unexpected token (the message names the token found and the constructs expected), an unterminated block comment or string literal, an unexpected character, an unknown literal suffix, an unmatched closing delimiter, or source that is not valid UTF-8 | parse |
| `E0901` | a macro invocation other than `emit!` or `include_str!`, or a macro definition | parse |
| `E0902` | a malformed SI literal: a suffix with an exponent, with a type suffix, or on a non-decimal literal | parse |
| `E0903` | a keyword used as an identifier, or a keyword or reserved word where no production accepts it | parse |
| `E0904` | an attribute that A.1 does not list, or a listed attribute on the wrong kind of item | elaboration |
| `E0905` | a token of a dropped class: a character or byte literal, a byte string, a lifetime | parse |
| `E0907` | `+>` or `<-` inside an expression, where no statement form accepts it. A well-formed `contribution` or `commit` in a block of the wrong kind is `E0404` | parse |
| `E0909` | a float literal whose nearest `f64` is infinite, or an integer literal that exceeds its suffix type or, without a suffix, 128 bits | parse |
| `E0910` | nesting deeper than 256 levels (delimiters, blocks, prefix operators, closures, `return` and `break` operands, type arguments), or more than 4096 chain links (binary operators, postfix `.`, call, index, `as` and `?`, `else if` arms) on one path of an expression | parse |

> **Resolution.** `E0906` is retired and `E0908` is retired (§0.2: numbers are not reused). They reported the literal that the language no longer has ([Appendix G.72](appendix_g_scope.md)); a word that names nothing is `E0161`.

## A.7 Identifiers

**Syntax** (lexical)

```
IDENT       ::= ( XID_START | '_' ) XID_CONTINUE*        // not a keyword, not `_` alone
RAW_IDENT   ::= 'r#' ADDED_KEYWORD                       // A.1 rule 6
```

`XID_START` and `XID_CONTINUE` are the Unicode character classes of those names (Unicode
Standard Annex 31).

**Static semantics**

1. An identifier starts with a character of class `XID_START` or with `_`, and continues with
   characters of class `XID_CONTINUE`. The ASCII identifiers are a letter or `_`, then letters,
   digits and `_`.
2. `_` alone is not an identifier. It is the punctuation token of the wildcard pattern and of
   the inferred type ([§1.6](part_i_values.md#16-patterns),
   [§1.4](part_i_values.md#14-typing-inference-and-conversion)).
3. A word of the keyword table of A.1 that is reserved everywhere is a keyword token, never an
   identifier.
4. Identifiers are case-sensitive. Two identifiers are the same name when they are the same
   sequence of code points: no normalization is applied.
5. An identifier has no length limit.
6. The raw form is that of A.1 rule 6 and exists for the seven added keywords only.

> **Resolution.** Rule 4 fixes what the design leaves open for non-ASCII names. A model that
> wants two spellings of one name to agree writes them the same way.

**Examples**

```rust,fragment
let vth = 0.6;
let _unused = 1;
let café = 2;
```

## A.8 Literals

A literal is an integer, a float, a string, or a boolean (`true`, `false`).
There is no character literal, no byte literal and no byte string (`E0905`).

**Syntax** (lexical)

```
INT_LITERAL   ::= ( DEC_DIGITS | '0x' HEX_DIGITS | '0o' OCT_DIGITS | '0b' BIN_DIGITS ) INT_SUFFIX?
FLOAT_LITERAL ::= DEC_DIGITS '.' DEC_DIGITS EXPONENT? FLOAT_SUFFIX?
                | DEC_DIGITS EXPONENT FLOAT_SUFFIX?
                | DEC_DIGITS FLOAT_SUFFIX
                | DEC_DIGITS '.'                              // rule 6
                | si_literal                                   // A.2
DEC_DIGITS    ::= DIGIT ( DIGIT | '_' )*
HEX_DIGITS    ::= ( HEX_DIGIT | '_' )* HEX_DIGIT ( HEX_DIGIT | '_' )*
OCT_DIGITS    ::= ( OCT_DIGIT | '_' )* OCT_DIGIT ( OCT_DIGIT | '_' )*
BIN_DIGITS    ::= ( BIN_DIGIT | '_' )* BIN_DIGIT ( BIN_DIGIT | '_' )*
EXPONENT      ::= ( 'e' | 'E' ) ( '+' | '-' )? DEC_DIGITS
INT_SUFFIX    ::= 'u' WIDTH | 'i' WIDTH | 'usize' | 'isize'    // WIDTH: 1 to 128, no leading zero
FLOAT_SUFFIX  ::= 'f32' | 'f64'
STRING        ::= '"' ( STRING_CHAR | ESCAPE | LINE_CONTINUATION )* '"'
                | 'r' HASHES '"' RAW_CHAR* '"' HASHES          // the same number of '#', 0 to 255
ESCAPE        ::= '\n' | '\r' | '\t' | '\\' | '\0' | '\'' | '\"'
                | '\x' OCT_DIGIT HEX_DIGIT
                | '\u{' HEX_DIGIT ( HEX_DIGIT | '_' )* '}'
```

`DIGIT` is `0` to `9`. `HEX_DIGIT` is a `DIGIT` or a letter `a` to `f` in either case.
`OCT_DIGIT` is `0` to `7`, `BIN_DIGIT` is `0` or `1`. `STRING_CHAR` is any character but `"`
and `\`. `RAW_CHAR` is any character; the raw string ends at the first `"` followed by its
number of `#`. `LINE_CONTINUATION` is a `\` directly followed by a line end.

**Static semantics**

*Integer literals*

1. An integer literal is written in decimal, or with the prefix `0x` (hexadecimal), `0o`
   (octal) or `0b` (binary). The prefix is lower case. A hexadecimal digit is in either case.
2. An underscore between or after digits is ignored: `1_000_000` is `1000000`. A literal starts
   with a digit, and a prefixed literal has at least one digit after the prefix.
3. A suffix `uN` or `iN`, `N` from 1 to 128, gives the literal the type `UInt<N>` or `Int<N>`.
   `usize` and `isize` give `UInt<64>` and `Int<64>`. Without a suffix the type is inferred
   ([§1.4.2](part_i_values.md#14-typing-inference-and-conversion)).
4. The value of a literal is not negative: a leading `-` is the negation operator (A.2 rule 7).
   The range rule is A.2 rule 8.

*Float literals*

5. A float literal is decimal. It has a fraction, an exponent, a float suffix, or an SI suffix
   (A.2). `1.5`, `1e3`, `1.5e-3`, `2f64` and `1k` are float literals; `1` is an integer literal.
6. `1.` is the float literal `1.0` when the dot follows a decimal integer literal directly and
   is not followed by `.`, `_` or an identifier start. In `1..2` the dots are the range token,
   and in `1.max(2)` and `t.0.1` the dot is the field token.
7. A suffix `f32` or `f64` gives the literal that type. Without a suffix the type is inferred
   ([§1.4.2](part_i_values.md#14-typing-inference-and-conversion)). There is no suffix for
   `f16` and `f128`: write `1.0 as f16`, or give the position the type.
8. The value of a float literal is the value of its type nearest to the exact decimal number,
   ties to even. For an SI-suffixed literal the exact number includes the multiplier (A.2
   rule 6).
9. An exponent without digits (`1e`, `1e+`) and letters after a number that form no suffix
   (`1meg`, `1kHz`, `0x1g`) are an unknown literal suffix (`E0900`).

*String literals*

10. A string literal has the type `str`
    ([§1.2.9](part_i_values.md#12-composite-types)). Its value is a sequence of Unicode
    scalar values.
11. In a string between plain quotes, an escape stands for one character: `\n` line feed, `\r`
    carriage return, `\t` tab, `\\` backslash, `\0` the null character, `\'` and `\"` the
    quotes, `\xHH` the character with code `HH` (two hexadecimal digits, at most `7F`), and
    `\u{H…}` the Unicode scalar value with that hexadecimal code (one to six digits). Any other
    escape is `E0900`.
12. A `\` directly before a line end is a line continuation: the line end and the whitespace
    that follows it, further line ends included, are not part of the value.
13. A line end written inside a string is part of the value, as it is written.
14. A raw string `r"…"`, `r#"…"#` has no escapes: every character between the quotes is part of
    the value. It ends at the first `"` followed by as many `#` as opened it.
15. Two string literals do not join: there is no concatenation of adjacent literals.

*Boolean literals*

16. `true` and `false` are the two values of `bool`. They are keywords.

> **Resolution.** Rules 1 to 16 state the literal forms in full. The suffixes follow the
> integer types of [§1](part_i_values.md#1-value-types) rule 5, so `7u3` is a `UInt<3>`.

**Examples**

```rust,fragment
let a = 255u8;
let b = 0xff_u8;
let c = 0b1010u4;
let d = 1_000;
let e = 2.5e-3;
let f = 4f64;
let g = 1.;
let s = "line\n";
let r = r#"a "quoted" word"#;
```

## A.9 Whitespace and comments

**Static semantics**

1. Whitespace is a space, a horizontal tab, a line feed, a carriage return or a form feed. It
   separates tokens and has no other meaning: no construct depends on a line end or on
   indentation, except the end of a line comment.
2. A byte order mark (U+FEFF) at the very start of a file is skipped.
3. A line comment starts with `//` and ends before the next line feed.
4. A block comment starts with `/*` and ends with the matching `*/`. Block comments nest:
   `/* a /* b */ c */` is one comment. A block comment that is not closed is `E0900`.
5. A comment is a doc comment (A.4) when it starts with `///` but not `////`, with `//!`, with
   `/**` but not `/***` (and is not the empty `/**/`), or with `/*!`. `///` and `/** */` are
   outer doc comments: they document the node that follows. `//!` and `/*! */` are inner doc
   comments: at the top of a file they document the file.
6. A comment that is not a doc comment is discarded (A.4 rule 3). Inside a string literal, `//`
   and `/*` are characters of the string.

## A.10 Punctuation

**Static semantics**

1. The punctuation tokens are those of the table. Each is one token, read by longest match.
2. `>` is always a token of its own. `>=`, `>>` and `>>=` are the operators formed by `>`
   tokens and `=` that touch, with no whitespace between them. `Bit<Option<u8>>` therefore closes
   two generic argument lists, and `a >> b` is a shift.
3. `$` and `~` are recognized so that they can be reported; no production accepts them.

| Tokens | Use |
| --- | --- |
| `+` `-` `*` `/` `%` | arithmetic ([§1.5](part_i_values.md#15-expressions)); `-` also negates, `+` also joins bounds |
| `&` `\|` `^` `!` `<<` `>>` | bitwise and logical operators; `\|` also separates pattern alternatives and delimits closure parameters, `!` also marks `emit!` and `include_str!` |
| `&&` `\|\|` | lazy boolean operators; `\|\|` also opens a closure without parameters |
| `==` `!=` `<` `>` `<=` `>=` | comparison; `<` and `>` also delimit generic lists |
| `=` `+=` `-=` `*=` `/=` `%=` `&=` `\|=` `^=` `<<=` `>>=` | assignment; `=` also binds in `let`, `const`, defaults and definitions |
| `+>` `<-` | contribution and commit (A.5) |
| `.` `..` `..=` | field access and method call; ranges and rest patterns |
| `,` `;` `:` `::` | separators; `::` joins path segments |
| `->` `=>` | return type; `match` arm |
| `#` `@` `_` `?` | attribute; pattern binding; wildcard and inferred type; the `?` operator |
| `(` `)` `[` `]` `{` `}` | delimiters; each opening delimiter has its closing one |
| `...` `$` `~` | recognized, accepted by no production |

