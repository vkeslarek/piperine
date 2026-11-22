# Appendix B. Reference

This appendix is normative. It lists the mixed-signal constructs of the language (B.1), where each is allowed (B.2), and the complete grammar of the language (B.3).
B.3 is the grammar authority: syntax excerpts in the parts repeat its productions.

Tokens are defined in Appendix A. B.3.2 holds the productions of the mixed-signal constructs and B.3.7 those of the value language; together they derive every model.

## B.1 The mixed-signal constructs

Every name and form of the language that is not part of the value language of Part I (§1.1 to §1.8, §3.7 to §3.9) or of the value library (Appendix H): what the language has for hardware, time and analysis.
The language has **62** such constructs. Everything else a model uses is the standard library (§22), written in the language.

### B.1.1 Count

| Kind | Items | Count |
| --- | --- | --- |
| keywords | `module` `nature` `signal` `on` `out` `flip` `after`; the second uses of `in` and `extern` (A.1) | 9 |
| contextual keywords | `analog`, `digital` before `for` or `<`; `potential`, `flow`, `value` in a nature (`potential` and `flow` are also the generic probes of those fields) | 5 |
| operators | `+>`, `<-` | 2 |
| calls | optional trailing `Option` parameters, named arguments (§3.2) | 2 |
| literals | SI suffixes | 1 |
| types and traits | `Event`, `Analysis`, `uN`/`iN`/`fN`, `Repr`, `Nature` | 5 |
| carrier and value methods | `changed` `prev` `others` `inflow` | 4 |
| events | `at` `cross` `settle` | 3 |
| analog | `ddt` `delay` `ddx` `limit` `noise` `ac_stim` `bound_step` | 7 |
| environment | `now` `freq` `temperature` `simparam` `analysis` | 5 |
| built-in `!` forms | `emit!`, `include_str!` | 2 |
| attributes | `#[derivative]`, `#[derive(Repr)]`, `#[link]` on an extern module | 3 |
| names | `gnd`; implicit params `m`, `dtemp`; ABI strings `"osdi"`, `"cosim"`; reserved streams `sim`, `checks` | 7 |
| forms | carrier fields in a `struct` (bundle); a module's name as a type and `Name { … }` as a value (template); the `Fn(P)` bound on modules; value inputs and a result on a module; `impl` on a nature or bundle, with a handle as `self`; `pub` on body items; `where` over const generics, on a module or a `fn` | 7 |
| **total** |  | **62** |

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
| 15 | `+>` | operator | contribution to a branch | §11 |
| 16 | `<-` | operator | commit to a discrete carrier | §12.1 |
| 17 | optional trailing `Option` parameters | call rule | a left-out trailing parameter is `None` | §3.2 |
| 18 | named arguments | call rule | `name: value` after the positional arguments | §3.2 |
| 19 | SI suffixes | literal | `1k`, `2.2u`: an `f64` times a power of ten | A.2 |
| 20 | `Event` | type | something that fires at instants | §13 |
| 21 | `Analysis` | type | the closed enum `Dc`, `Transient`, `Ac`, `Noise`, `Static` | §11.1 |
| 22 | `uN` / `iN` / `fN` | types | `UInt<N>`, `Int<N>`, `Float<N>` and their short names | §1 |
| 23 | `Repr` | trait | lays a value type out on levels | §2 |
| 24 | `Nature` | trait | the bound satisfied by every nature | §6 rule 4, §8.3 |
| 25 | `changed` | carrier method | `Event`: the carrier or param changed in this microstep | §5.3, §13 |
| 26 | `prev` | carrier method | the value before the most recent change | §5.3 |
| 27 | `others` | carrier method | `resolve` of every drive but this module's own | §5.2 |
| 28 | `inflow` | port method | flow entering the module at a port | §6 |
| 29 | `at` | event, core module | fires when time reaches its argument | §13 rule 4 |
| 30 | `cross` | event, core module | fires on a rising crossing of zero | §13 rule 5, §17.3 |
| 31 | `settle` | event value | fires once per time point, after the instant went quiet | §13, §15 |
| 32 | `ddt` | core module | time derivative | §14.5, §15.2 |
| 33 | `delay` | core module | its argument as it was `td` ago | §14.6 |
| 34 | `ddx` | core `fn` | partial derivative with respect to a probe | §14.2 |
| 35 | `limit` | core module | Newton step limiting | §14.7, §18 |
| 36 | `noise` | core module | a noise source | §11.1.8, §14.8 |
| 37 | `ac_stim` | core `fn` | small-signal stimulus | §14.3 |
| 38 | `bound_step` | statement | caps the next time step | §14.10, §17.2.5 |
| 39 | `now` | environment `fn` | the time of the solve | §14.4, §16.2 |
| 40 | `freq` | environment `fn` | the frequency of a small-signal point | §14.4, §16.2 |
| 41 | `temperature` | environment `fn` | the instance's temperature | §8.5.5, §14.4 |
| 42 | `simparam` | environment `fn` | a simulator parameter by name | §14.4 |
| 43 | `analysis` | environment `fn` | whether the current solve belongs to an `Analysis` | §11.1.1, §14.4 |
| 44 | `emit!` | built-in `!` form | writes a record to a stream | §19 |
| 45 | `include_str!` | built-in `!` form | the text of a file, at elaboration | §20 |
| 46 | `#[derivative(f, …)]` | attribute | the derivatives of an `extern "C" fn` | §3 rules 2 and 11 |
| 47 | `#[derive(Repr)]` | attribute | derives the level layout | §2 |
| 48 | `#[link(…)]` on an extern module | attribute | names the foreign implementation | §8.5 |
| 49 | `gnd` | name | the reference node of every continuous nature | §6 rule 1 |
| 50 | `m` | implicit param | multiplicity of an instance | §8.5 |
| 51 | `dtemp` | implicit param | temperature offset of an instance | §8.5 |
| 52 | `"osdi"` | ABI string | an OSDI analog model | §8.5 |
| 53 | `"cosim"` | ABI string | a cosimulated discrete model | §8.5 |
| 54 | `sim` | reserved stream | a `finish` record ends the analysis | §19 rule 2 |
| 55 | `checks` | reserved stream | the records of `require`, `cover`, `target` | §19 rule 2 |
| 56 | bundle | form | a `struct` whose fields are carriers | §7 |
| 57 | template | form | a module's name as a type; `Name { … }` as a value | §8.4 |
| 58 | `Fn(P)` bound on modules | form | holds for every module whose port list is `P` | §8.4 |
| 59 | value inputs and a result | form | a module placed inside an expression | §8.3 |
| 60 | `impl` on a nature or bundle | form | functions with a handle as `self` | §3.1 |
| 61 | `pub` on body items | form | visible to the parent as `inst.name` | §8.2 |
| 62 | `where` over const generics | form | a boolean constraint on a module or a `fn` | §8.1, §3 |

> **Resolution.** Names that occur only inside a counted construct are counted with it. The quantity keys `access`, `units` and `abstol` belong to `nature` (2). The variants of `Analysis` belong to `Analysis` (21). `UInt`, `Int` and `Float` belong to row 22. `#[repr(uN)]` (§2) and `extern "C" fn` (§3) are counted with the value language, not here.

> **Resolution.** `where` on a param (§8.1) is counted with the param form of `module` (1). Row 62 covers only the `where` after a module's port list or a `fn`'s signature.

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

1. Inside a block or a `fn`, a `let` declares a value-level local, and every local may be assigned again (§1.5.12). A body `let` of a module is not a local (§8.2.12). Locals are allowed wherever statements are.
2. A reaction reads a body `let` that names an instance. It does not place one (§8.3.4).
3. `nature`, `module`, `analog for` and `digital for` are items at namespace scope: a file or an inline `mod`. A block may sit in any namespace of its module's file (§3.4.3).
4. `after` is allowed on a definition and on a commit, nowhere else.

### B.2.2 Expressions

Positions, left to right: a body `let` or a value-input argument; a definition or the argument of an `in` port; an event position (the event of an `on`, a body `let` or a result of type `Event`); a reaction body; an `analog` block; a `fn` body; a `const` item.

| Expression | `let`, value input | definition, `in` argument | event position | reaction body | `analog` | `fn` | `const` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| values, params, constants | yes | yes | yes | yes | yes | yes | constants only |
| reading a discrete carrier | yes | yes | yes | yes | yes (§3.3.6) | yes, through a handle | no |
| probes: `potential`, `flow`, aliases, `p.inflow()` | yes | no | yes | yes | yes | yes ¹ | no |
| `now()`, `freq()`, `gmin()`, `source_scale()`, `analysis(Static)` | yes | no | yes | yes | yes | yes ¹ | no |
| `temperature()`, `simparam`, `analysis` of a kind | yes | yes | yes | yes | yes | yes | no |
| `ddx`, `ac_stim` | yes ² | no | no | no | yes | yes ¹ | no |
| placing `ddt`, `delay`, `limit`, `noise` | yes | no ³ | yes | no | yes | no | no |
| placing any other module with a result | yes | yes ³ | yes | no | yes | no | no |
| placing `cross`, `at` | as an `Event` | no | yes | no | no | no | no |
| reading a named instance's result | yes | yes ³ | yes | yes | yes | no | no |
| `s.changed()`, `settle` | as an `Event` | no | yes | as a `bool` (§13 rule 2) | no | yes ⁴ | no |
| `s.prev()`, `s.others()` | yes | yes | yes | yes | yes | yes, through a handle | no |
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

## B.3 The grammar

### B.3.1 Conventions

The notation is that of the index (§0). In addition:

1. `stmt`, `expr`, `type`, `pat`, `block`, `generics`, `generic_args`, `path`, `attrs` and `const_item` are the productions of the value grammar, B.3.7.
2. `IDENT` and `STRING` are token classes (Appendix A). The tokens `+>`, `<-` and the SI-suffixed literal are defined in Appendix A and are not repeated here.
3. `( X ',' )*` denotes a comma-separated list of `X`. The comma after the last element is optional.
4. `gen<X>` is a production parameterized by the production `X`.

> **Resolution.** Convention 3 fixes the reading of every list in this grammar. Read literally, the design's productions would require a comma after the last element, which its own examples omit.

### B.3.2 Productions

```
// ---- items ---------------------------------------------------------------------------------

item           ::= nature_decl | module_decl | behavior_block | value_item

// ---- natures, bundles, functions ------------------------------------------------------------

nature_decl    ::= attrs 'pub'? 'nature' IDENT generics? '{' nature_field ( ',' nature_field )* ','? '}'
nature_field   ::= attrs ( 'potential' | 'flow' ) ':' IDENT ( '{' ( IDENT ':' expr ',' )* '}' )?
                 | attrs 'value' ':' type
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
include_str    ::= 'include_str' '!' '(' STRING ')'                            // a const expression of type str

// ---- attributes -------------------------------------------------------------------------------

attr_derivative ::= '#' '[' 'derivative' '(' ( path ',' )* ')' ']'               // on an extern "C" fn
attr_derive     ::= '#' '[' 'derive' '(' ( path ',' )* ')' ']'                   // Repr is derivable
attr_link       ::= '#' '[' 'link' '(' STRING ( ',' IDENT '=' STRING )* ')' ']'  // on an extern module
```

### B.3.3 Reading the grammar

1. `where` and `;` end a param's default without lookahead.
2. `flip` belongs to the type, so `rx: flip Stream<T>` and `[flip Stream<T>; N]` both parse.
3. A `{` after the head of `on`, `if`, `match`, `while` or `for` opens the block. An instance with a param block, or a template, is parenthesized there: `on cross(e) { … }` is a reaction.
4. A `place` takes no parentheses, in a definition and in a commit alike: `(q) = 1;` and `(q) <- 1;` are parse errors (`E0900`). A tuple index is not a `.IDENT` step: `q.0 = 1;` and `q.0 <- 1;` are parse errors too. A `place` names levels: a commit to `st.n` or `sh[3]` writes those levels of the carrier and no others.
5. A trailing `expr` in a module body is the module's result (§8.3.2). It is allowed only when the head has `-> type`.
6. `value_item` is the item of the value language (B.3.7): `fn`, `struct`, `enum`, `trait`, `impl`, `const`, `mod`, `use`. Their `struct_field`, `impl_block` and `fn_decl` are the productions above.
7. An `emit` followed by `;` is a statement. `include_str` is allowed only in the initializer of a `const` item (§20).

In the param block of an `instance` and in a `template`, a field written as an `IDENT` with no `:` is the field-init shorthand of a struct expression (§1.3.3): `{ r }` means `{ r: r }`, so `R(p, n) { r }` and `R { r, c: 1p }` are well formed. The tree keeps the authored form: a field without `:` is a shorthand field, and `r: r` is not.

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

### B.3.6 Forms that are not part of the language

The grammar of B.3.2 and B.3.7 is complete: a form it does not derive is a parse error. The forms of this table are those a front end **MUST** report with the code of their row, once each, in place of a plain syntax error. This table, together with §1 rule 2, §3.2 and A.1, is the complete list of what a front end rejects at parse under a code of its own.

> **Note (informative).** Each form of the table is part of Rust. A reader who knows Rust will look for them; the third column says what to write.

> **Resolution.** The rows of references and of the three markers are the forms pure value semantics removes (§1.4.6, G.71). Each is reported under the code its token class already has: `E0101` for a form §1 rule 2 leaves out, `E0903` for a reserved word no production accepts.

| Form | Reported as | Write instead |
| --- | --- | --- |
| type aliases (`type T = f64;`) | `E0903` | write the type out |
| associated types (`type X;` in a trait or an `impl`) | `E0903` | name the type where it is used |
| `static` items | `E0903` | a `const` item (§1 rule 3); state lives in signals (§0) |
| trait-bound `where` clauses (`where T: Tr`) on a function, struct, enum, trait or `impl` | `E0900` | a bound in the generic list: `<T: Tr>` (§3.8.8) |
| `impl Trait` types | `E0900` | a named generic: `<T: Tr>` |
| qualified paths (`<T as Tr>::X`) | `E0900` | a path through the type: `T::X` |
| restricted visibility (`pub(crate)`, `pub(super)`) | `E0900` | `pub` or no visibility |
| loop labels (`'a: loop`, `break 'a`) | `E0905` | restructure the loop |
| references and borrows (`&T`, `&mut T`, `&x`, `&&x`, `*x`, a `&p` pattern, `&self`, `&mut self`) | `E0101` | the value itself: every value is copied where it is used (§1.4.6); a method takes `self` and returns what it changed (§3.1.10) |
| pattern parameters (`(a, b): (u8, u8)`) | `E0900` | a plain name, then `let` |
| the markers `mut`, `ref` and `move` (`let mut x`, `ref x`, a `move` closure) | `E0903` | the plain form: every local may be assigned (§1.5.12), a pattern binds a copy (§1.6.9), a closure captures by value (§1.5.14) |
| half-open range patterns (`..x`) | `E0900` | `..=x` or a guard |
| a leading `::` in a path or a `use` (`::a::b`) | `E0900` | a path from a name in scope |
| inner attributes (`#![..]`) | `E0900` | an outer attribute on the item |
| the never type (`!`) | `E0900` | a unit return type |
| raw-pointer types (`*const T`, `*mut T`) | `E0900` | a value or an array |
| named function-pointer parameters (`fn(x: f64) -> f64`) | `E0900` | the types only: `fn(f64) -> f64` |
| attributes and doc comments on a parameter, a generic parameter or a struct-literal field (`fn f(#[a] x: f64)`) | `E0900` | the attribute or comment on the item |
| `union` items | `E0900` | a `struct` or an `enum` |
| `extern` blocks | `E0900` | an `extern "C" fn` item |
| `extern crate` items | `E0900` | a dependency named under `[dependencies]` of the manifest |

Two forms that look absent are part of the language:

- function-pointer types `fn(T, ..) -> U` (§1.2.10);
- a negative literal as a const generic argument (`Int< -1 >`, `Int<{ -1 }>`; `Int<-1>` is reported at the `<-` token).

A front end cannot reject struct-update syntax in a template (§8.4) or a blocking write, `wait` or `sleep` (§12.2) at parse: the grammar has no production for them, so they are ordinary syntax errors.

> **Resolution.** `static` items are in the table because a model keeps state only in signals (§0). The design names `const` as the only constant form and does not mention `static`.

### B.3.7 The value grammar

The productions of the value language. Their meaning is Part I; the tokens are Appendix A. `fn_decl`, `fn_param`, `impl_block`, `impl_item`, `struct_field`, `call`, `method_call`, `emit` and `include_str` are in B.3.2.

```
// ---- items ----------------------------------------------------------------------------------

value_item     ::= attrs 'pub'? ( fn_decl | struct_decl | enum_decl | trait_decl | impl_block | const_item | use_decl | mod_decl )
struct_decl    ::= 'struct' IDENT generics? ( '{' ( struct_field ',' )* '}' | '(' ( tuple_field ',' )* ')' ';' | ';' )
tuple_field    ::= attrs 'pub'? type
enum_decl      ::= 'enum' IDENT generics? '{' ( variant ',' )* '}'
variant        ::= attrs IDENT ( '{' ( struct_field ',' )* '}' | '(' ( tuple_field ',' )* ')' )? ( '=' expr )?
trait_decl     ::= 'trait' IDENT generics? ( ':' path ( '+' path )* )? '{' impl_item* '}'
const_item     ::= 'const' IDENT ':' type ( '=' expr )? ';'
mod_decl       ::= 'mod' IDENT ( ';' | '{' item* '}' )
use_decl       ::= 'use' use_tree ';'
use_tree       ::= ( use_seg '::' )* ( '*' | '{' ( use_tree ',' )* '}' | use_seg ( 'as' IDENT )? )
use_seg        ::= IDENT | 'self' | 'super' | 'crate'

// ---- attributes -----------------------------------------------------------------------------

attrs          ::= ( DOC_COMMENT | attr )*                                      // DOC_COMMENT: an outer doc comment (A.4, A.9)
attr           ::= '#' '[' path ( '(' ( attr_arg ',' )* ')' | '=' expr )? ']'
attr_arg       ::= literal | IDENT '=' literal | path

// ---- paths and generics ---------------------------------------------------------------------

path           ::= path_seg ( '::' path_seg )*
path_seg       ::= ( IDENT | 'self' | 'Self' | 'super' | 'crate' ) ( '::'? generic_args )?   // in an expression the '::' is required
generics       ::= '<' ( generic_param ',' )* '>'
generic_param  ::= IDENT ( ':' bound ( '+' bound )* )? ( '=' type )?
                 | 'const' IDENT ':' type ( '=' ( literal | block ) )?
bound          ::= path | path '(' ( type ',' )* ')' ( '->' type )?             // the second form: Fn(A, B) -> R
generic_args   ::= '<' ( generic_arg ',' )* '>'
generic_arg    ::= type | literal | '-' literal | block

// ---- types ----------------------------------------------------------------------------------

type           ::= path
                 | '(' ')' | '(' type ',' ( type ',' )* ')' | '(' type ')'
                 | '[' type ';' expr ']'
                 | 'fn' '(' ( type ',' )* ')' ( '->' type )?
                 | '_'

// ---- patterns -------------------------------------------------------------------------------

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

// ---- statements -----------------------------------------------------------------------------

block          ::= '{' stmt* expr? '}'
stmt           ::= ';'
                 | attrs 'let' pat1 ( ':' type )? ( '=' expr ( 'else' block )? )? ';'
                 | attrs expr ';'
                 | attrs block_expr ';'?
                 | value_item

// ---- expressions, loosest binding first -----------------------------------------------------

expr           ::= range_expr ( assign_op expr )?
assign_op      ::= '=' | '+=' | '-=' | '*=' | '/=' | '%=' | '&=' | '|=' | '^=' | '<<=' | '>>='
range_expr     ::= or_expr ( '..' or_expr? | '..=' or_expr )?
                 | '..' or_expr? | '..=' or_expr
or_expr        ::= and_expr ( '||' and_expr )*
and_expr       ::= cmp_expr ( '&&' cmp_expr )*
cmp_expr       ::= bitor_expr ( cmp_op bitor_expr )?
cmp_op         ::= '==' | '!=' | '<' | '>' | '<=' | '>='
bitor_expr     ::= bitxor_expr ( '|' bitxor_expr )*
bitxor_expr    ::= bitand_expr ( '^' bitand_expr )*
bitand_expr    ::= shift_expr ( '&' shift_expr )*
shift_expr     ::= add_expr ( ( '<<' | '>>' ) add_expr )*
add_expr       ::= mul_expr ( ( '+' | '-' ) mul_expr )*
mul_expr       ::= cast_expr ( ( '*' | '/' | '%' ) cast_expr )*
cast_expr      ::= unary_expr ( 'as' type )*
unary_expr     ::= ( '-' | '!' ) unary_expr | postfix_expr
postfix_expr   ::= primary postfix*
postfix        ::= '.' IDENT                                                   // a field
                 | '.' INT_LITERAL                                             // a tuple index
                 | '.' IDENT generic_args? args                                // method_call (B.3.2); generic_args after '::'
                 | args ( '{' ( IDENT ( ':' expr )? ',' )* '}' )?               // call; with the braces, an instance (B.3.2)
                 | '[' expr ']'
                 | '?'
args           ::= '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')'
primary        ::= literal
                 | path
                 | struct_expr
                 | '(' expr ')'
                 | '(' ')' | '(' expr ',' ( expr ',' )* ')'                     // a tuple
                 | '[' ( expr ',' )* ']' | '[' expr ';' expr ']'               // an array
                 | block_expr
                 | closure
                 | 'return' expr? | 'break' expr? | 'continue'
                 | emit | include_str                                          // B.3.2
literal        ::= INT_LITERAL | FLOAT_LITERAL | STRING | 'true' | 'false'
struct_expr    ::= path generic_args? '{' ( IDENT ( ':' expr )? ',' )* '}'
closure        ::= ( '||' | '|' ( closure_param ',' )* '|' ) ( expr | '->' type block )
closure_param  ::= pat1 ( ':' type )?
block_expr     ::= block | if_expr | match_expr | while_expr | loop_expr | for_expr
if_expr        ::= 'if' cond block ( 'else' ( if_expr | block ) )?
cond           ::= 'let' pat '=' expr | expr                                   // no struct expression at the top (§1.5.17)
match_expr     ::= 'match' expr '{' match_arm* '}'                              // no struct expression at the top of expr
match_arm      ::= attrs pat ( 'if' expr )? '=>' ( block ','? | expr ',' | expr )   // the last form: the final arm only
while_expr     ::= 'while' cond block
loop_expr      ::= 'loop' block
for_expr       ::= 'for' pat1 'in' expr block                                  // no struct expression at the top of expr
```

Reading the value grammar:

1. The operator productions give the precedence and associativity of §1.5: a production lower in the list binds tighter, `cmp_expr` and `range_expr` do not chain, and `expr` assigns right to left.
2. In an expression path, generic arguments follow `::` (`idt::<Heat>(x)`); in a type they follow the name directly (`Bit<u8>`). A generic argument that is a single name is read as a `type`; name resolution finds the const parameter or constant it names (§3.8.5).
3. After `as type`, a `<` or `<<` belongs to the type: `a as u8 < b` is a parse error; write `(a as u8) < b`.
4. A struct expression, and the param block of an instance, are not allowed at the top level of the expression in the head of `if`, `while`, `match`, `for` and `on`, where the `{` opens the body (B.3.3 item 3).
5. A `block_expr` at the start of a statement is the whole statement: `{ a } - b;` is a block and then the statement `-b;`. Parenthesize a block that is an operand.
6. The operand of `return` and `break`, and the body of a closure without `-> type`, extend as far to the right as an `expr` can.
7. `>=`, `>>` and `>>=` are formed as A.10 rule 2 says. `t.0.1` is two tuple indexes.
8. At the start of an operand `||` opens a closure without parameters. A `&`, `&&` or `*` at the start of an operand is not an operator: it is a reference form (B.3.6).
9. `(e)` is a parenthesized expression, `(e,)` a tuple of one field. `(T)` is the type `T`, `(T,)` a tuple type. `(p)` is the pattern `p`.
10. In a `stmt`, the `;` after an expression may be left out only before the `}` that closes the block: the expression is then the value of the block (§1.7.1).
11. `signal` and `on` are items of a module body and of a `digital` block (B.3.2), never statements; written in another block they are reported by §10 rule 8.
12. A `value_item` in a `stmt` carries its own `attrs`.

> **Resolution.** B.3.7 writes out the grammar of every value form the parts use. It adds no construct: each production is a form of Part I.
