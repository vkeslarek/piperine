# Lexer and Parser

## Lexer (`src/parse/lexer.rs`)

### Overview

The `Lexer` converts raw PHDL source text (`&str`) into a flat sequence of `Lexed` tokens.
Each token carries a byte-range span (`start..end`) for error reporting.

### `Lexer::tokenize()`

The `tokenize()` method processes the input character-by-charcter using `peek_char()` and
`advance()` helpers. On each iteration it:

1. Records the current `start` position.
2. Advances one UTF-8 character.
3. Matches the character to produce a `Tok`.
4. Pushes a `Lexed { tok, start, end: self.pos }` onto the result vector.
5. Calls `skip_whitespace_and_comments()` before the next iteration.

### Comment and whitespace skipping

`skip_whitespace_and_comments()` runs in a loop until the position stops advancing:

- **Whitespace** — any `char::is_whitespace()` characters are consumed.
- **Line comments** — `//` sequences consume characters until a `\n`.
- **Block comments** — `/* ... */` are consumed, including nested `*` followed by `/`.

### Tokens

The `Tok` enum has three categories:

#### Literals

| Token | Description |
|---|---|
| `Ident(String)` | Any identifier or keyword |
| `Real(f64)` | Floating-point literal |
| `Int(u64)` | Integer literal (any base) |
| `Quad(String)` | Four-valued logic literal (`0q...`) |
| `Str(String)` | Double-quoted string |
| `SysCall(String)` | System call (`$ident`) |

#### Punctuation

| Token | Character(s) |
|---|---|
| `LParen` | `(` |
| `RParen` | `)` |
| `LBrack` | `[` |
| `RBrack` | `]` |
| `LBrace` | `{` |
| `RBrace` | `}` |
| `Comma` | `,` |
| `Semi` | `;` |
| `Colon` | `:` |
| `DoubleColon` | `::` |
| `Dot` | `.` |
| `Assign` | `=` |
| `FatArrow` | `=>` |
| `Arrow` | `->` |
| `DotDot` | `..` |
| `DotDotEq` | `..=` |

#### Operators

| Token | Character(s) |
|---|---|
| `Contrib` | `<+` |
| `Force` | `<-` |
| `Plus` | `+` |
| `Minus` | `-` |
| `Star` | `*` |
| `Slash` | `/` |
| `Percent` | `%` |
| `EqEq` | `==` |
| `NotEq` | `!=` |
| `Lt` | `<` |
| `Le` | `<=` |
| `Gt` | `>` |
| `Ge` | `>=` |
| `Not` | `!` |
| `And` | `&&` |
| `Or` | `\|\|` |
| `BitAnd` | `&` |
| `BitOr` | `\|` |
| `BitXor` | `^` |
| `At` | `@` |

### Keyword-as-identifier approach

Keywords such as `mod`, `fn`, `for`, `if`, `else`, `return`, `analog`, `digital`, `input`,
`output`, `inout` are **not** distinguished at the lexer level. They all become `Tok::Ident`.
The parser matches keyword spellings using `eat_ident()`.

### Number lexing (`lex_number`)

The `lex_number(c, start)` method handles:

- **Decimal (base 10)** — digits `0-9`, optional `.` for real numbers, optional `e`/`E` exponent.
- **Hex (base 16)** — `0x` / `0X` prefix, hex digits `0-9a-fA-F`.
- **Binary (base 2)** — `0b` / `0B` prefix, digits `0-1`.
- **Octal (base 8)** — `0o` / `0O` prefix, digits `0-7`.
- **Quad (base 4)** — `0q` / `0Q` prefix, digits `0`, `1`, `x`, `X`, `z`, `Z`.

Underscores are allowed as digit separators in all bases and are discarded.

Real numbers are detected by the presence of a `.` or `e`/`E` exponent marker and parse to `Tok::Real(f64)`. The `.` is not consumed if followed by another `.` (to avoid clashing with `..` range operators). Non-quad, non-real literals parse to `Tok::Int(u64)`. Quad literals produce `Tok::Quad(String)` containing the digits after the `q` prefix.

---

## Parser (`src/parse/parser/`)

### Overview

The parser is a hand-written recursive-descent LL(1) parser that converts `&[Lexed]`
into a `SourceFile` AST. All decisions are resolved by a single token of lookahead.

Grammar coverage mirrors the PHDL grammar specification sections 2–8.

### File organization

| File | Concern |
|---|---|
| `parser/mod.rs` | `Parser` struct, file-level entry point (`parse_file`), `parse_path` |
| `parser/items.rs` | Item declarations: `mod`, `discipline`, `bundle`, `enum`, `capability`, `impl`, `fn`, `block` |
| `parser/expr.rs` | Expressions: Pratt binary-operator parser, primary expressions, array literals |
| `parser/stmt.rs` | Statements: `mod`-body, behavior, function-body, events, ranges, patterns |

### Key parser methods

- **`peek()`** — returns the current token without consuming it.
- **`peek_at(offset)`** — returns the token at `pos + offset` without consuming.
- **`eat(tok)`** — consumes the current token if it matches `tok`.
- **`eat_ident(expected)`** — consumes the current token if it is `Tok::Ident(expected)`.
- **`expect(tok)`** — like `eat` but returns an error on mismatch.
- **`expect_ident_str(expected)`** — like `eat_ident` but returns an error on mismatch.
- **`parse_ident()`** — consumes and returns an identifier string, or errors.

### `parse_file()` entry point

Loops while tokens remain, dispatching to item parsers based on leading keyword:

- `use` → `parse_path()` → `Item::UseDecl`
- `pub` prefix is consumed first, then:
  - `mod` → `parse_mod_decl()`
  - `analog` → `parse_behavior(Analog)`
  - `digital` → `parse_behavior(Digital)`
  - `discipline` → `parse_discipline()`
  - `bundle` → `parse_bundle()`
  - `enum` → `parse_enum()`
  - `capability` → `parse_capability()`
  - `impl` → `parse_impl()`
  - `fn` → `parse_fn_decl()`

### `parse_path()`

Consumes a `::`-separated path like `devices::passives::Resistor`. Collects segments
into a `Path { segments: Vec<String> }`.

---

## Pratt parsing algorithm (`expr.rs`)

### `parse_binary_expr(precedence, stop_at_bitor)`

An iterative Pratt parser. It parses a primary expression as the initial LHS, then
enters a loop: peek the next binary operator, compare its precedence against the
current minimum, and if high enough, consume the operator and recursively parse
the RHS at `prec + 1`.

The `stop_at_bitor` flag causes the parser to stop at `BitOr` — this is used for
array comprehensions where `|` terminates the element expression.

### Operator precedence table (lowest to highest)

| Precedence | Operators | Tokens |
|---|---|---|
| 1 | `BitOr` | `\|` |
| 2 | `BitAnd` | `&` |
| 3 | `Eq`, `Neq` | `==`, `!=` |
| 4 | `Lt`, `Le`, `Gt`, `Ge` | `<`, `<=`, `>`, `>=` |
| 5 | `BitXor` | `^` |
| 6 | `Add`, `Sub` | `+`, `-` |
| 7 | `Mul`, `Div`, `Rem` | `*`, `/`, `%` |

### `parse_primary()`

Handles all atomic and prefix expression forms:

| Token | Produces |
|---|---|
| `Int(i)` | `Expr::Literal(Literal::Int(i))` |
| `Real(r)` | `Expr::Literal(Literal::Real(r))` |
| `Str(s)` | `Expr::Literal(Literal::String(s))` |
| `Quad(q)` | `Expr::Literal(Literal::Quad(q))` |
| `Ident("if")` | `Expr::If { cond, then_body, else_body }` |
| `Ident(s)` followed by `::` | `Expr::Path` (multi-segment path) |
| `Ident(s)` alone | `Expr::Ident(s)` |
| `SysCall(s)` | `Expr::SysCall(s, [])` |
| `[` | Delegates to `parse_array_expr()` |
| `(` | Grouped expression |
| `{` | `Expr::Block(parse_block())` |
| `!` | `Expr::Unary(UnaryOp::Not, ...)` |
| `-` | `Expr::Unary(UnaryOp::Neg, ...)` |
| `\|` | Lambda expression (params between bars, then body) |

### Postfix loop

After parsing a primary, a loop applies postfix forms:

- **Call** `(...)` — `Expr::Call(func, args)`. If the base is `Expr::SysCall`, args are appended directly.
- **Index** `[i]` — `Expr::Index(base, index)`.
- **Slice** `[i..j]` or `[i..=j]` — `Expr::Slice(base, Range)`.
- **Field** `.name` — `Expr::Field(base, field_name)`.
- **PathSeg** `::seg` — extends an `Expr::Ident` or `Expr::Path` with an additional segment.

### Bundle literal detection

After the postfix loop, the parser performs a lookahead for `{ .` or `{}` following
an expression. If detected, the preceding expression is interpreted as a type name
and the braces form a `BundleLit { ty, fields }`.

### Array expression parsing (`parse_array_expr()`)

Parses `[...]` after the leading `[` has been consumed. Uses a lookahead scan for a
top-level `|` to distinguish comprehension from list/repeat:

- **Repeat**: `[value; count]` → `ArrayBody::Repeat(value, count)`
- **Comprehension**: `[expr | var in range]` → `ArrayBody::Comprehension(expr, var, range)`
- **List**: `[a, b, c]` → `ArrayBody::List([a, b, c])`

---

## Statement parsing (`stmt.rs`)

### `parse_mod_stmt()`

Handles `mod`-body statements with the following left-factored grammar on a leading `Ident`:

1. **Keyword-driven**: `param`, `wire`, `var`, `for`, `if` — dispatched on the keyword.
2. **`StructuralFor`**: `for var in range { body }` — unrolled at elaboration.
3. **`StructuralIf`**: `if (cond) { body } [else { body }]` — evaluated at elaboration.
4. **InstanceOrConnect**: left-factored on leading `Ident`:
   - If followed by `=` → `Connection` (`lhs = rhs`)
   - If followed by `:` → named instance (`name: Module [...] <...> (...) {...}`)
   - Otherwise → anonymous instance (`Module [...] <...> (...) {...}`)

### `parse_behavior_stmt()`

Parses `analog`/`digital` body statements:

- **`var`** → `BehaviorStmt::VarDecl`
- **`if`** → `BehaviorStmt::If` (with optional `else if` / `else` chains)
- **`match`** → `BehaviorStmt::Match` (arms with `Pattern => { body }`)
- **`for`** → `BehaviorStmt::For`
- **`@`** → `BehaviorStmt::Event { spec, guard?, body }` via `parse_event_spec()`
- **`$...`** → `BehaviorStmt::Diagnostic` (system call like `$display(...)`)
- Otherwise → expression followed by `<+`, `<-`, `=`, or `;` (bind or expr statement)

### `parse_event_spec()`

An open event model where any identifier becomes `EventSpec::Named { name, arg }`.
Built-in names (`initial`, `final`) are parsed as keyword-like terminals. Or-expressions
are parsed as `(spec | spec | ...)`.

### `parse_range()`

Parses `start .. end` or `start ..= end`, producing a `Range` with `start: Box<Expr>`,
`end: Box<Expr>`, and an `inclusive: bool` flag.

### `parse_pattern()`

Parses `_` as `Pattern::Wildcard` and any `::`-separated path as `Pattern::Path`.

### Block parsing (`parse_block()`)

A block `{ stmt* [expr] }` is parsed by consuming statements (terminated by `;`) and
stopping at `}`. If an expression is encountered without a trailing `;`, it becomes the
block's value (Rust-style), producing a `Block { stmts, expr: Some(trailing) }`.
