# Appendix B. Reference

This appendix is normative. It lists what the language adds to Rust (B.1), where each addition is allowed (B.2), and the complete grammar of the additions (B.3).
B.3 is the grammar authority: syntax excerpts in the parts repeat its productions.

Tokens are defined in Appendix A. Everything not listed here is the value grammar of Rust 2024 with the changes of Part I.

## B.1 Added to Rust

Every name and form a model can use that Rust does not have, or uses differently.
The language adds **65** constructs. Everything else a model uses is the standard library (§22), written in the language.

### B.1.1 Count

| Kind | Items | Count |
| --- | --- | --- |
| keywords | `module` `nature` `signal` `on` `out` `flip` `after`; new uses of `in` and `extern` | 9 |
| contextual keywords | `analog`, `digital` before `for` or `<`; `potential`, `flow`, `value`, `resolve` in a nature (`potential` and `flow` are also the generic probes of those fields) | 6 |
| operators | `+>`, `<-` | 2 |
| calls | optional trailing `Option` parameters, named arguments (§3.2) | 2 |
| literals | SI suffixes, `Unknown` | 2 |
| types and traits | `Event`, `Analysis`, `uN`/`iN`/`fN`, `Repr`, `Nature` | 5 |
| carrier and value methods | `changed` `prev` `others` `inflow` `level` | 5 |
| events | `at` `cross` `settle` | 3 |
| analog | `ddt` `delay` `ddx` `limit` `noise` `ac_stim` `bound_step` | 7 |
| environment | `now` `freq` `temperature` `simparam` `analysis` | 5 |
| built-in `!` forms | `emit!`, `include_str!` | 2 |
| attributes | `#[derivative]`, `#[derive(Repr)]`; Rust's `#[link]` on an extern module | 3 |
| names | `gnd`; implicit params `m`, `dtemp`; ABI strings `"osdi"`, `"cosim"`; reserved streams `sim`, `checks` | 7 |
| forms | carrier fields in a `struct` (bundle); a module's name as a type and `Name { … }` as a value (template); the `Fn(P)` bound on modules; value inputs and a result on a module; `impl` on a nature or bundle, with a handle as `self`; `pub` on body items; `where` over const generics, on a module or a `fn` | 7 |
| **total** |  | **65** |

### B.1.2 The constructs, one by one

| # | Construct | Kind | What it is | Defined in |
| --- | --- | --- | --- | --- |
| 1 | `module` | keyword | declares a module | §8 |
| 2 | `nature` | keyword | declares a nature | §4 |
| 3 | `signal` | keyword | declares a carrier in a module body | §5.1, §6, §8.2 |
| 4 | `on` | keyword | opens a reaction | §12.1 |
| 5 | `out` | keyword | port or bundle-field direction: driven | §7, §8.1 |
| 6 | `flip` | keyword | reverses every direction of a bundle type | §7 |
| 7 | `after` | keyword | delay of a commit or of a definition | §8.2.4, §12.1 |
| 8 | `in` (new use) | keyword | port or bundle-field direction: read | §7, §8.1 |
| 9 | `extern` (new use) | keyword | a module implemented outside the language | §8.5 |
| 10 | `analog` | contextual keyword | opens an `analog` block | §10, §11 |
| 11 | `digital` | contextual keyword | opens a `digital` block | §10, §12 |
| 12 | `potential` | contextual keyword | nature field; the generic potential probe | §4, §6 |
| 13 | `flow` | contextual keyword | nature field; the generic flow probe | §4, §6 |
| 14 | `value` | contextual keyword | nature field: the type a discrete carrier holds | §4 |
| 15 | `resolve` | contextual keyword | nature field: how drives combine | §4, §5.2 |
| 16 | `+>` | operator | contribution to a branch | §11 |
| 17 | `<-` | operator | commit to a discrete carrier | §12.1 |
| 18 | optional trailing `Option` parameters | call rule | a left-out trailing parameter is `None` | §3.2 |
| 19 | named arguments | call rule | `name: value` after the positional arguments | §3.2 |
| 20 | SI suffixes | literal | `1k`, `2.2u`: an `f64` times a power of ten | A.2 |
| 21 | `Unknown` | literal | every level unknown, of any `Repr` type | §3.3, A.3 |
| 22 | `Event` | type | something that fires at instants | §13 |
| 23 | `Analysis` | type | the closed enum `Dc`, `Tran`, `Ac`, `Noise`, `Static` | §11.1 |
| 24 | `uN` / `iN` / `fN` | types | `UInt<N>`, `Int<N>`, `Float<N>` and their short names | §1 |
| 25 | `Repr` | trait | lays a value type out on levels | §2 |
| 26 | `Nature` | trait | the bound satisfied by every nature | §6 rule 4, §8.3 |
| 27 | `changed` | carrier method | `Event`: the carrier or param changed in this microstep | §5.3, §13 |
| 28 | `prev` | carrier method | the value before the most recent change | §5.3 |
| 29 | `others` | carrier method | `resolve` of every drive but this module's own | §5.2 |
| 30 | `inflow` | port method | flow entering the module at a port | §6 |
| 31 | `level` | value method | one level as a known `Option<bool>` | §3.3.4 |
| 32 | `at` | event, core module | fires when time reaches its argument | §13 rule 4 |
| 33 | `cross` | event, core module | fires on a rising crossing of zero | §13 rule 5, §17.3 |
| 34 | `settle` | event value | fires once per time point, after the instant went quiet | §13, §15 |
| 35 | `ddt` | core module | time derivative | §14.5, §15.2 |
| 36 | `delay` | core module | its argument as it was `td` ago | §14.6 |
| 37 | `ddx` | core `fn` | partial derivative with respect to a probe | §14.2 |
| 38 | `limit` | core module | Newton step limiting | §14.7, §18 |
| 39 | `noise` | core module | a noise source | §11.1.8, §14.8 |
| 40 | `ac_stim` | core `fn` | small-signal stimulus | §14.3 |
| 41 | `bound_step` | statement | caps the next time step | §14.10, §17.2.5 |
| 42 | `now` | environment `fn` | the time of the solve | §14.4, §16.2 |
| 43 | `freq` | environment `fn` | the frequency of a small-signal point | §14.4, §16.2 |
| 44 | `temperature` | environment `fn` | the instance's temperature | §8.5.5, §14.4 |
| 45 | `simparam` | environment `fn` | a simulator parameter by name | §14.4 |
| 46 | `analysis` | environment `fn` | whether the current solve belongs to an `Analysis` | §11.1.1, §14.4 |
| 47 | `emit!` | built-in `!` form | writes a record to a stream | §19 |
| 48 | `include_str!` | built-in `!` form | the text of a file, at elaboration | §20 |
| 49 | `#[derivative(f, …)]` | attribute | the derivatives of an `extern "C" fn` | §3 rules 2 and 11 |
| 50 | `#[derive(Repr)]` | attribute | derives the level layout | §2 |
| 51 | `#[link(…)]` on an extern module | attribute | names the foreign implementation | §8.5 |
| 52 | `gnd` | name | the reference node of every continuous nature | §6 rule 1 |
| 53 | `m` | implicit param | multiplicity of an instance | §8.5 |
| 54 | `dtemp` | implicit param | temperature offset of an instance | §8.5 |
| 55 | `"osdi"` | ABI string | an OSDI analog model | §8.5 |
| 56 | `"cosim"` | ABI string | a cosimulated discrete model | §8.5 |
| 57 | `sim` | reserved stream | a `finish` record ends the analysis | §19 rule 2 |
| 58 | `checks` | reserved stream | the records of `require`, `cover`, `target` | §19 rule 2 |
| 59 | bundle | form | a `struct` whose fields are carriers | §7 |
| 60 | template | form | a module's name as a type; `Name { … }` as a value | §8.4 |
| 61 | `Fn(P)` bound on modules | form | holds for every module whose port list is `P` | §8.4 |
| 62 | value inputs and a result | form | a module placed inside an expression | §8.3 |
| 63 | `impl` on a nature or bundle | form | functions with a handle as `self` | §3.1 |
| 64 | `pub` on body items | form | visible to the parent as `inst.name` | §8.2 |
| 65 | `where` over const generics | form | a boolean constraint on a module or a `fn` | §8.1, §3 |

> **Resolution.** Names that occur only inside a counted construct are counted with it. The quantity keys `access`, `units` and `abstol` belong to `nature` (2). The variants of `Analysis` belong to `Analysis` (23). `UInt`, `Int` and `Float` belong to row 24. `#[repr(uN)]` and `extern "C" fn` are Rust's.

> **Resolution.** `where` on a param (§8.1) is counted with the param form of `module` (1). Row 65 covers only the `where` after a module's port list or a `fn`'s signature.

## B.2 Where each thing is allowed

A construct used outside the positions marked here is an elaboration error, reported by the part that defines the construct.

### B.2.1 Items and statements

|  | module body | `digital` block | reaction | `analog` | `fn` |
| --- | --- | --- | --- | --- | --- |
| param, `signal`, `let` naming an expression | yes |  |  |  |  |
| `=` (definitions) | yes |  |  |  |  |
| generation `for`/`if` over params | yes | yes |  |  |  |
| `on` |  | yes |  |  |  |
| `+>`, `bound_step` |  |  |  | yes |  |
| `<-` |  |  | yes |  |  |
| `emit!` |  |  | yes | yes, as a trace (§19 rule 4) | yes, and then the `fn` is callable only where `emit!` is |
| probes, environment, `ddx`, `ac_stim` (`fn`s) | in `let`s and value inputs; in definitions, only the environment that is constant during an analysis | in event expressions | yes | yes | yes, and then the `fn` is callable only where these are |
| an instance inside an expression (every module with a result, core or not) | in `let`s, definitions and arguments | in the event of an `on` |  | yes |  |

Notes:

1. Inside a block or a `fn`, an immutable `let` and `let mut` are value-level locals (§8.2). They are allowed wherever statements are.
2. A reaction reads a body `let` that names an instance. It does not place one (§8.3.4).
3. `nature`, `module`, `analog for` and `digital for` are items at namespace scope: a file or an inline `mod`. A block may sit in any namespace of its module's file (§3.4.3).
4. `after` is allowed on a definition and on a commit, nowhere else.

### B.2.2 Expressions

Positions, left to right: a body `let` or a value-input argument; a definition or the argument of an `in` port; an event position (the event of an `on`, a body `let` or a result of type `Event`); a reaction body; an `analog` block; a `fn` body; a `const` item.

| Expression | `let`, value input | definition, `in` argument | event position | reaction body | `analog` | `fn` | `const` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| values, params, constants | yes | yes | yes | yes | yes | yes | constants only |
| reading a discrete carrier | yes | yes | yes | yes | through `level` (§3.3.5) | yes, through a handle | no |
| `Unknown` | yes | yes | yes | yes | no (§3.3.5) | yes | yes |
| probes: `potential`, `flow`, aliases, `p.inflow()` | yes | no | yes | yes | yes | yes ¹ | no |
| `now()`, `freq()`, `gmin()`, `source_scale()`, `analysis(Static)` | yes | no | yes | yes | yes | yes ¹ | no |
| `temperature()`, `simparam`, `analysis` of a kind | yes | yes | yes | yes | yes | yes | no |
| `ddx`, `ac_stim` | yes ² | no | no | no | yes | yes ¹ | no |
| placing `ddt`, `delay`, `limit`, `noise` | yes | no ³ | yes | no | yes | no | no |
| placing any other module with a result | yes | yes ³ | yes | no | yes | no | no |
| placing `cross`, `at` | as an `Event` | no | yes | no | no | no | no |
| reading a named instance's result | yes | yes ³ | yes | yes | yes | no | no |
| `s.changed()`, `settle` | as an `Event` | no | yes | as a `bool` (§13 rule 2) | no | yes ⁴ | no |
| `s.prev()`, `s.others()` | yes | yes | yes | yes | through `level` | yes, through a handle | no |
| `x.level(k)` | yes | yes | yes | yes | yes | yes | yes |
| `emit!` | no | no | no | yes | yes, as a trace | yes ¹ | no |
| `include_str!` | no | no | no | no | no | no | yes |

¹ The `fn` is then callable only where the construct itself is allowed (§3).
² The `let` or value input is then readable only in `analog`.
³ Only when the result reads nothing that §8.2.5 forbids: no probe, no `now()`, no `freq()`, no `gmin()`, no `source_scale()`, no `analysis(Static)`.
⁴ In a `fn` that returns `Event`; the `fn` is callable in event positions, and as a `bool` inside a reaction.

> **Resolution.** Note ² reconciles §14, which allows `ddx` and `ac_stim` in `analog` only, with the design's placement table, which lists them with the probes. A `let` is evaluated where it is read (§8.2), so the restriction moves to the reader.

> **Resolution.** Note ³ follows from §8.3.2 (a result is a `let` read by the caller) and §8.2.5. `inertial` may appear in a definition; `since` and `ddt` may not.

> **Resolution.** The environment functions are not elaboration constants. None of them is allowed in a `const` item.

### B.2.3 Loops and instances

| Context | An instance may be placed |
| --- | --- |
| a generation `for` over params and constants | yes: one instance per iteration (§8.2) |
| a `for` with a constant trip count in `analog` or in an expression | yes: unrolled, one instance per iteration (§8.3.4) |
| any loop whose trip count is not a constant | no |
| under an `if` or `match` in `analog` | yes: the instance exists whatever the branch taken (§8.3.4) |

## B.3 Grammar of the additions

### B.3.1 Conventions

The notation is that of the index (§0). In addition:

1. `stmt`, `expr`, `type`, `pat`, `block`, `generics`, `generic_args`, `path`, `attrs` and `const_item` are Rust's, apart from Part I's changes and the forms B.3.6 removes.
2. `IDENT` and `STRING` are token classes (Appendix A). The tokens `+>`, `<-`, the SI-suffixed literal and `Unknown` are defined in Appendix A and are not repeated here.
3. `( X ',' )*` denotes a comma-separated list of `X`. The comma after the last element is optional, as in Rust.
4. `gen<X>` is a production parameterized by the production `X`.

> **Resolution.** Convention 3 fixes the reading of every list in this grammar. Read literally, the design's productions would require a comma after the last element, which its own examples omit.

### B.3.2 Productions

```
// ---- items ---------------------------------------------------------------------------------

item           ::= nature_decl | module_decl | behavior_block | rust_item

// ---- natures, bundles, functions ------------------------------------------------------------

nature_decl    ::= attrs 'pub'? 'nature' IDENT generics? '{' nature_field ( ',' nature_field )* ','? '}'
nature_field   ::= attrs ( 'potential' | 'flow' ) ':' IDENT ( '{' ( IDENT ':' expr ',' )* '}' )?
                 | attrs 'value' ':' type
                 | attrs 'resolve' ':' expr                                           // a fn(&[T]) -> T
struct_field   ::= attrs 'pub'? dir? IDENT ':' ( type | sigtype )               // carrier fields make a bundle
impl_block     ::= 'impl' generics? path generic_args? ( 'for' path generic_args? )? '{' impl_item* '}'
impl_item      ::= attrs 'pub'? ( fn_decl | const_item )
fn_decl        ::= 'const'? ( 'extern' STRING )? 'fn' IDENT generics? '(' ( fn_param ',' )* ')' ( '->' type )? ( 'where' expr )? ( block | ';' )
fn_param       ::= 'self' | IDENT ':' type

// ---- calls ----------------------------------------------------------------------------------

call           ::= path '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')'           // §3.2
method_call    ::= expr '.' IDENT generic_args? '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')'
instance       ::= callee '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')' ( '{' ( IDENT ( ':' expr )? ',' )* '}' )?
callee         ::= path generic_args? | call | '(' expr ')'
template       ::= path generic_args? '{' ( IDENT ( ':' expr )? ',' )* '}'

// ---- modules --------------------------------------------------------------------------------

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

// ---- behavior -------------------------------------------------------------------------------

behavior_block ::= 'analog'  names? 'for' path generic_args? '{' ( stmt | contribution )* '}'
                 | 'digital' names? 'for' path generic_args? '{' digital_item* '}'
names          ::= '<' IDENT ( ',' IDENT )* '>'
digital_item   ::= reaction | gen<digital_item>
gen<X>         ::= attrs 'for' pat 'in' expr '{' X* '}'
                 | attrs 'if' expr '{' X* '}' ( 'else' ( '{' X* '}' | gen<X> ) )?
reaction       ::= attrs 'on' expr '{' ( stmt | commit )* '}'                      // expr has type Event
contribution   ::= attrs expr '+>' expr ';'                                         // left side: a probe
commit         ::= attrs place '<-' expr ( 'after' expr )? ';'
place          ::= path ( '[' expr ']' | '.' IDENT )*                           // a carrier, an element or a field of one

// ---- built-in ! forms -------------------------------------------------------------------------

emit           ::= 'emit' '!' '(' STRING ( ',' IDENT '=' expr )* ','? ')'       // an expression of type ()
include_str    ::= 'include_str' '!' '(' STRING ')'                            // a const expression of type &'static str

// ---- attributes -------------------------------------------------------------------------------

attr_derivative ::= '#' '[' 'derivative' '(' ( path ',' )* ')' ']'               // on an extern "C" fn
attr_derive     ::= '#' '[' 'derive' '(' ( path ',' )* ')' ']'                   // Repr is derivable
attr_link       ::= '#' '[' 'link' '(' STRING ( ',' IDENT '=' STRING )* ')' ']'  // on an extern module
```

### B.3.3 Reading the grammar

1. `where` and `;` end a param's default without lookahead.
2. `flip` belongs to the type, so `rx: flip Stream<T>` and `[flip Stream<T>; N]` both parse.
3. As in Rust, a `{` after the head of `on`, `if`, `match`, `while` or `for` opens the block. An instance with a param block, or a template, is parenthesized there: `on cross(e) { … }` is a reaction.
4. A `place` takes no parentheses, in a definition and in a commit alike: `(q) = 1;` and `(q) <- 1;` are parse errors (`E0900`). A tuple index is not a `.IDENT` step: `q.0 = 1;` and `q.0 <- 1;` are parse errors too. A `place` names levels: a commit to `st.n` or `sh[3]` writes those levels of the carrier and no others.
5. A trailing `expr` in a module body is the module's result (§8.3.2). It is allowed only when the head has `-> type`.
6. `rust_item` is every Rust item that Part I keeps: `fn`, `struct`, `enum`, `trait`, `impl`, `const`, `mod`, `use`. Their `struct_field`, `impl_block` and `fn_decl` are the productions above.
7. An `emit` followed by `;` is a statement. `include_str` is allowed only in the initializer of a `const` item (§20).

In the param block of an `instance` and in a `template`, a field written as an `IDENT` with no `:` is the field-init shorthand, as in a Rust struct expression: `{ r }` means `{ r: r }`, so `R(p, n) { r }` and `R { r, c: 1p }` are well formed. The tree keeps the authored form: a field without `:` is a shorthand field, and `r: r` is not.

Classification that the grammar leaves to name resolution:

| Shape | Is | When |
| --- | --- | --- |
| `path '(' … ')'` | an `instance` | the callee names a module, or a value of a module type (§8.4) |
| | a `call` | otherwise |
| `path '{' … '}'` | a `template` | the path names a module |
| | a bundle literal | the path names a bundle (§7) |
| | a struct expression | otherwise |
| `IDENT ':' T` in a port list | a carrier or bundle port | `T` names a nature, a bundle, or an array of them |
| | a value input | `T` is a value type |
| `IDENT ':' T` in a `struct` | a carrier field, making the struct a bundle | `T` names a nature or a bundle |

Constraints on the productions that are checked after parsing; item 8 is checked at parse and items 9 to 14 at elaboration, during name resolution:

8. In an argument list no positional argument follows a named one (§3.2.4) (parse, `E0142`).
9. A param block `{ … }` after an argument list is allowed only on an `instance` (elaboration, at name resolution).
10. The body of an `extern` module holds `param` items only, and no result (§8.5) (elaboration, at name resolution).
11. `dir` on a `port` or `struct_field` is allowed only on a carrier of a one-driver discrete nature, or on a bundle as `in` (§7, §8.1) (elaboration, at name resolution).
12. `= expr` on a `port` is allowed only on a carrier the module drives (§8.1) (elaboration, at name resolution).
13. The left side of a `contribution` is a probe call (§11 rule 1) (elaboration, at name resolution).
14. `names` repeats the generic names of the module, the same names in the same order; bounds come from the module (§10 rule 1, rule 6) (elaboration, at name resolution).

### B.3.4 Statements inside behavior blocks

The productions `behavior_block` and `reaction` extend `stmt` at the top level of their blocks. The extension reaches nested blocks:

1. Inside an `analog` block, every nested block takes `stmt | contribution`. Nested blocks are the blocks of `if`, `else`, `match` arms, `for`, `while`, `loop` and plain `{ }`.
2. Inside a reaction, every nested block takes `stmt | commit`, in the same positions.
3. The body of a closure and the body of a `fn` are not nested blocks in this sense. A `contribution` or a `commit` there is a parse error.
4. A `contribution` and a `commit` are statements. Neither is an expression; a `match` arm that commits has a block body.
5. A `contribution` or a `commit` may be written in a nested block that is used as a value, as in `let x = { i(a) +> 1.0; 2.0 };`: the block still sits inside the behavior block.

> **Resolution.** The design's productions list `contribution` and `commit` only at the top level of their blocks. Its examples write them under `if`, `match` and `for` (§11, §12.1), so the extension is recursive. Part IV states the same rule (§10 rule 8).

### B.3.5 Amendments to the design's listing

The productions of B.3.2 differ from the design's listing in the places below. Each is a resolution.

| # | Production | Change | Required by |
| --- | --- | --- | --- |
| 1 | `nature_decl`, `module_decl` | `attrs 'pub'?` in front | `pub module idt` (§8.3), the prelude exports of §22 |
| 2 | `nature_decl` | fields separated by `,` with an optional last one | the declarations of §4 |
| 3 | `fn_decl`, `fn_param` | optional `const`; `self` as a parameter | `const fn` parsers (§20); `pub fn celsius(self)` (§3.1) |
| 4 | `impl_block`, `impl_item` | an `impl` may hold `const` items | `pub const Z: Logic` (§22.3) |
| 5 | `method_call` | new: a method call takes named arguments too | §3.2: one rule covers methods |
| 6 | `instance`, `callee` | the callee may be a call or a parenthesized expression | `nch(0.13u, 2u)(d, g, s, b)` (§8.4) |
| 7 | `gen<X>` | `else` may be followed by another `if` | chained generation conditions |
| 8 | `emit`, `include_str` | new productions | §19, §20 |
| 9 | `attr_*` | new productions for the three attributes of B.1; `attr_derivative` takes a list | §2, §3 rule 11, §8.5 |
| 10 | `item` | new: the additions at file scope | §3.4 |

> **Resolution.** Amendments 1–10 complete the grammar so that every example of this specification parses. None adds a construct to B.1.

> **Resolution.** Amendment 4 keeps §3.1's rule that an `impl` has no time: a `const` item is a value, like a function.

> **Resolution.** The head of a module or of a `fn` with `where expr` is followed by `{`. B.3.3 item 3 applies there too: a template or an instance with a param block inside that `expr` is parenthesized.

> **Resolution.** `emit` takes a string literal as its stream name (§19). A body `let` takes no type annotation: its type is that of its expression.

### B.3.6 Removed from Rust's grammar

This table, together with §1 rule 2, §3.2 and A.1, is the complete list of what a front end rejects at parse. A Rust form that none of the four removes is part of the language. Each removed form is reported once, with the code of its row.

| Removed | Reported as | Write instead |
| --- | --- | --- |
| type aliases (`type T = f64;`) | `E0903` | write the type out |
| associated types (`type X;` in a trait or an `impl`) | `E0903` | name the type where it is used |
| `static` items | `E0903` | a `const` item (§1 rule 3); state lives in signals (§0) |
| trait-bound `where` clauses (`where T: Tr`) on a function, struct, enum, trait or `impl` | `E0900` | a bound in the generic list: `<T: Tr>` |
| `impl Trait` types | `E0900` | a named generic: `<T: Tr>` |
| qualified paths (`<T as Tr>::X`) | `E0900` | a path through the type: `T::X` |
| restricted visibility (`pub(crate)`, `pub(super)`) | `E0900` | `pub` or no visibility |
| loop labels (`'a: loop`, `break 'a`) | `E0905` | restructure the loop |
| `&self` and `&mut self` receivers | `E0900` | `self` |
| `mut` and pattern parameters (`mut x: u8`, `(a, b): (u8, u8)`) | `E0900` | a plain name, then `let` |
| half-open range patterns (`..x`) | `E0900` | `..=x` or a guard |
| a leading `::` in a path or a `use` (`::a::b`) | `E0900` | a path from a name in scope |
| inner attributes (`#![..]`) | `E0900` | an outer attribute on the item |
| the never type (`!`) | `E0900` | a unit return type |
| raw-pointer types (`*const T`, `*mut T`) | `E0900` | a value or an array |
| named function-pointer parameters (`fn(x: f64) -> f64`) | `E0900` | the types only: `fn(f64) -> f64` |
| attributes and doc comments on a parameter, a generic parameter or a struct-literal field (`fn f(#[a] x: f64)`) | `E0900` | the attribute or comment on the item |
| double borrows (`&&x`) | `E0900` | one borrow: `&x` |
| `union` items | `E0900` | a `struct` or an `enum` |
| `extern` blocks | `E0900` | an `extern "C" fn` item |
| `extern crate` items | `E0900` | a dependency named under `[dependencies]` of the manifest |

Taken from Rust. Two forms look removed and are part of the language:

- function-pointer types `fn(T, ..) -> U`, the type §4 rule 8 names;
- a negative literal as a const generic argument (`Int< -1 >`, `Int<{ -1 }>`; `Int<-1>` is reported at the `<-` token).

A front end cannot reject struct-update syntax in a template (§8.4) or a blocking write, `wait` or `sleep` (§12.2) at parse: the grammar has no production for them, so they are ordinary syntax errors.

> **Resolution.** `static` items are listed as removed because a model keeps state only in signals (§0). The design names `const` as the only constant form and does not mention `static`.
