# Piperine Hardware Definition Language — Grammar (EBNF)

A complete grammar for PHDL, organized to mirror the specification. It is designed to be
LL(1): every alternative is decided by one token of lookahead, after the left-factoring noted
inline. Distinctions the grammar does **not** make (value type vs net type, access vs ordinary
call, which `[Expr]` is a const argument vs an array dimension) are deliberately left to the
type checker — they are semantic, not syntactic.

## Notation

```
::=        definition
|          alternation
{ X }      zero or more X
[ X ]      optional X
( X )      grouping
"x"        literal terminal
X*  X+     zero-or-more / one-or-more (used in prose where clearer)
UPPER/Camel nonterminal
lower      token class (defined in §1)
```

---

## 1. Lexical structure

```
Ident          ::= IdentStart { IdentCont }
IdentStart     ::= letter | "_"
IdentCont      ::= letter | digit | "_"
```

By convention (not enforced lexically) PascalCase names denote modules, bundles, value types,
net types, disciplines, enums, and capabilities; snake_case names denote functions, methods,
ports, parameters, variables, fields, and instances.

```
RealLit        ::= digit+ "." digit+ [ ("e"|"E") [ "+"|"-" ] digit+ ]
                 | digit+ ("e"|"E") [ "+"|"-" ] digit+
NaturalLit     ::= digit+
                 | "0b" bindigit+
                 | "0x" hexdigit+
QuadLit        ::= "0q" ( "0" | "1" | "X" | "Z" )
BoolLit        ::= "0" | "1"
StringLit      ::= '"' { stringchar } '"'

Literal        ::= RealLit | NaturalLit | QuadLit | StringLit
                 -- BoolLit overlaps NaturalLit lexically; resolved by type context.

LineComment    ::= "//" { any-but-newline } newline
BlockComment   ::= "/*" { any } "*/"
```

Built-in names referenced by the grammar but lexed as ordinary `Ident`: the access functions
`V` `I` (and any discipline accessor), the math/operator built-ins `ddt` `idt` `exp` `ln`
`sqrt` `pow` `tanh` `real` `int` `bit`, the root capabilities `Type` `Net`. System built-ins
are a distinct token class:

```
SysCall        ::= "$" Ident          -- $error $warn $info $assert $bound_step $analysis
```

### Keywords

```
mod analog digital impl fn bundle enum discipline capability
use pub param wire var
input output inout
potential flow storage resolve  tri or and
for in if else match return when self Self
initial final posedge negedge change cross above
```

---

## 2. Compilation unit and packages

```
CompilationUnit ::= { UseDecl | Item }

UseDecl         ::= "use" Path ";"
Path            ::= Ident { "::" Ident }

Item            ::= [ "pub" ] ItemKind
ItemKind        ::= ModDecl
                  | BehaviorDecl
                  | DisciplineDecl
                  | BundleDecl
                  | EnumDecl
                  | CapabilityDecl
                  | ImplDecl
                  | FnDecl
```

After an optional `pub`, the leading keyword (`mod` `analog` `digital` `discipline` `bundle`
`enum` `capability` `impl` `fn`) selects the item. LL(1).

---

## 3. Modules

```
ModDecl         ::= "mod" Ident [ ConstParams ] [ TypeParams ] PortList [ ModBody ]

ConstParams     ::= "[" Ident { "," Ident } "]"          -- each a compile-time Natural
TypeParams      ::= "<" TypeParam { "," TypeParam } ">"
TypeParam       ::= Ident [ ":" Bound ]
Bound           ::= Ident { "+" Ident }                  -- capabilities; Type / Net are roots

PortList        ::= "(" [ Port { "," Port } [ "," ] ] ")"
Port            ::= Direction Ident ":" Type
Direction       ::= "input" | "output" | "inout"

ModBody         ::= "{" { ModStmt } "}"
ModStmt         ::= ParamDecl
                  | WireDecl
                  | VarDecl
                  | StructuralFor
                  | StructuralIf
                  | InstanceOrConnect
```

### 3.1 Declarations inside a module

```
ParamDecl       ::= "param" Ident ":" Type [ "=" Expr ] ";"
WireDecl        ::= "wire" Ident ":" Type ";"            -- Type carries any [N] dimensions
VarDecl         ::= "var" Ident ":" Type [ "=" Expr ] ";"
```

### 3.2 Instances and connections (left-factored)

A module statement beginning with an identifier is an anonymous instance, a named instance, or
a net connection. They share the prefix `Ident { Indexer | Field }`, then branch on the next
token:

```
InstanceOrConnect ::= Ident { Indexer | Field } InstTail

InstTail        ::= ":" ModuleRef PortArgs [ ParamArgs ] ";"   -- named instance
                  | ConstArgs PortArgs [ ParamArgs ] ";"        -- anon instance w/ const args
                  | PortArgs [ ParamArgs ] ";"                  -- anon instance
                  | "=" Expr ";"                                 -- net connection

ModuleRef       ::= Ident [ ConstArgs ] [ TypeArgs ]
ConstArgs       ::= "[" Expr { "," Expr } "]"
TypeArgs        ::= "<" Type { "," Type } ">"
PortArgs        ::= "(" [ Expr { "," Expr } ] ")"
ParamArgs       ::= "{" [ ParamArg { "," ParamArg } ] "}"
ParamArg        ::= "." Ident "=" Expr

Indexer         ::= "[" Expr "]"
Field           ::= "." Ident
```

Branch tokens after the prefix: `":"` (named instance), `"("` (anonymous instance), `"="`
(connection). An anonymous instance with const args (`Dac[N] ( ... )`) is the prefix's trailing
`Indexer` reinterpreted as `ConstArgs` once `"("` follows; the checker confirms the name is a
module. LL(1) by single-token branch.

### 3.3 Structural control

```
StructuralFor   ::= "for" Ident "in" Range ModBody
StructuralIf    ::= "if" "(" Expr ")" ModBody [ "else" ( ModBody | StructuralIf ) ]
Range           ::= Expr ( ".." | "..=" ) Expr
```

---

## 4. Types

```
Type            ::= Ident [ TypeArgs ] { Indexer }
```

`Indexer` after a type name is either a const argument (when the named type takes const
params, e.g. `UInt[N]`) or an array dimension (e.g. `Bit[8]`, `Bit[8][16]`); the distinction is
semantic. Value types and net types share this production.

### 4.1 Disciplines

```
DisciplineDecl  ::= "discipline" Ident "{" { DisciplineItem } "}"
DisciplineItem  ::= NatureDecl | StorageDecl | ResolveDecl

NatureDecl      ::= ( "potential" | "flow" ) Ident ":" Type [ AttrList ] ";"
AttrList        ::= "(" Attr { "," Attr } ")"
Attr            ::= Ident "=" Expr                       -- e.g. unit = "V", abstol = 1e-6

StorageDecl     ::= "storage" Type ";"
ResolveDecl     ::= "resolve" ( "tri" | "or" | "and" ) ";"
```

### 4.2 Enums

```
EnumDecl        ::= "enum" Ident [ ":" Type ] "{" EnumVariant { "," EnumVariant } [ "," ] "}"
EnumVariant     ::= Ident [ "=" Expr ]
```

### 4.3 Bundles

```
BundleDecl      ::= "bundle" Ident [ ConstParams ] [ TypeParams ] "{" [ Field { "," Field } [ "," ] ] "}"
Field           ::= Ident ":" Type [ "=" Expr ]
```

---

## 5. Capabilities and generics

```
CapabilityDecl  ::= "capability" Ident [ ":" SuperList ] "{" { CapItem } "}"
SuperList       ::= Ident { "," Ident }
CapItem         ::= FnSig | FnDecl                       -- signature, or default method
FnSig           ::= "fn" Ident [ TypeParams ] ParamList "->" Type ";"

ImplDecl        ::= "impl" [ Ident "for" ] TypeRef "{" { FnDecl } "}"
TypeRef         ::= Ident [ ConstArgs ] [ TypeArgs ]
```

`impl TypeRef { ... }` are inherent methods; `impl Capability for TypeRef { ... }` is a
capability implementation. The optional `Ident "for"` is decided by looking past the first
`Ident` for the `for` keyword (a two-token peek, or equivalently parse `Ident`, then branch on
`for` vs the start of `TypeRef`).

`ConstParams`, `TypeParams`, `Bound`, `ConstArgs`, `TypeArgs` are defined in §3.

---

## 6. Functions

```
FnDecl          ::= "fn" Ident [ TypeParams ] ParamList "->" Type Block

ParamList       ::= "(" [ Param { "," Param } ] ")"
Param           ::= "self"                               -- receiver, first position only
                  | Ident ":" Type

Block           ::= "{" { Stmt } [ Expr ] "}"            -- optional trailing Expr is the value
```

A `fn` body is pure: `Stmt` here excludes contributions, forces, drives, and events (§7).
Lambdas and comprehensions appear at the expression level (§8).

---

## 7. Behavior

```
BehaviorDecl    ::= ( "analog" | "digital" ) Ident BehaviorBlock
BehaviorBlock   ::= "{" { BehaviorStmt } "}"

BehaviorStmt    ::= VarDecl
                  | BindStmt
                  | IfStmt
                  | MatchStmt
                  | ForStmt
                  | EventBlock
                  | Diagnostic
                  | ExprStmt
```

The same statement set serves `analog` and `digital`; which operators and statements are legal
is a semantic rule (contributions/forces and `cross`/`above`/`initial` in analog; drives,
`=` state, edges, and `match` in digital). One grammar, two checked engines.

### 7.1 Binds: contribution, force, drive, assignment

```
BindStmt        ::= Expr BindOp Expr ";"
BindOp          ::= "<+" | "<-" | "="
```

The left side is parsed as an `Expr`; the checker then requires it to be an access
(`I(a,b)`, `V(n)`) for `<+`/`<-`, or an lvalue (`name`, `name.field`, `name[i]`) for `=`. After
the left `Expr`, the operator (or `";"` for `ExprStmt`) gives the single-token branch.

### 7.2 Events

```
EventBlock      ::= "@" EventSpec [ "when" "(" Expr ")" ] Block
EventSpec       ::= EventTerm
                  | "(" EventTerm { ( "|" | "or" ) EventTerm } ")"
EventTerm       ::= EdgeCall | "initial" | "final"
EdgeCall        ::= ( "posedge" | "negedge" | "change" | "cross" | "above" ) "(" Expr ")"
```

### 7.3 Diagnostics and timestep

```
Diagnostic      ::= SysCall "(" [ Expr { "," Expr } ] ")" ";"
                 -- $error/$warn/$info(msg); $assert(cond, msg); $bound_step(dt)
```

`$analysis` is an expression (§8), not a statement.

---

## 8. Statements and expressions

### 8.1 Shared statements

```
Stmt            ::= VarDecl
                  | ReturnStmt
                  | IfStmt
                  | MatchStmt
                  | ForStmt
                  | BindStmt
                  | ExprStmt

ReturnStmt      ::= "return" Expr ";"
ExprStmt        ::= Expr ";"

IfStmt          ::= "if" "(" Expr ")" Block [ "else" ( Block | IfStmt ) ]
ForStmt         ::= "for" Ident "in" Range Block
MatchStmt       ::= "match" Expr "{" { MatchArm } "}"
MatchArm        ::= Pattern "=>" Block [ "," ]
Pattern         ::= Path | "_"
```

### 8.2 Expressions (lowest to highest precedence)

```
Expr            ::= OrExpr
OrExpr          ::= AndExpr        { "|"  AndExpr }       -- bitwise/logical or
AndExpr         ::= EqExpr         { "&"  EqExpr }
EqExpr          ::= RelExpr        { ( "==" | "!=" ) RelExpr }
RelExpr         ::= XorExpr        { ( "<" | "<=" | ">" | ">=" ) XorExpr }
XorExpr         ::= AddExpr        { "^"  AddExpr }
AddExpr         ::= MulExpr        { ( "+" | "-" ) MulExpr }
MulExpr         ::= UnaryExpr      { ( "*" | "/" | "%" ) UnaryExpr }
UnaryExpr       ::= ( "!" | "-" ) UnaryExpr
                  | PostfixExpr
PostfixExpr     ::= Primary { Call | Indexer | Slice | Field | PathSeg }
Call            ::= "(" [ Expr { "," Expr } ] ")"
Slice           ::= "[" Expr Range_rest "]"              -- Range_rest = (".."|"..=") Expr
PathSeg         ::= "::" Ident
```

`%` and the relational/bitwise operator set are listed as used in the specification
(`(i + 1) % N`, `^ & |` in the adder, comparisons in conditions). Precedence above is a
starting point and may be tuned during implementation.

### 8.3 Primaries

```
Primary         ::= Literal
                  | SysCall                              -- e.g. $analysis
                  | Ident
                  | "(" Expr ")"
                  | Block                                 -- block expression, yields trailing Expr
                  | IfExpr
                  | ArrayExpr
                  | BundleLit
                  | Lambda

IfExpr          ::= "if" "(" Expr ")" Block "else" Block  -- value-producing form

ArrayExpr       ::= "[" ArrayBody "]"
ArrayBody       ::= Expr ";" Expr                         -- repeat:        [value; N]
                  | Expr "|" Ident "in" Range             -- comprehension: [expr | i in 0..N]
                  | Expr { "," Expr }                      -- element list:  [a, b, c]

BundleLit       ::= TypeRef "{" [ FieldInit { "," FieldInit } [ "," ] ] "}"
FieldInit       ::= "." Ident "=" Expr

Lambda          ::= "|" [ Ident { "," Ident } ] "|" Expr
```

`ArrayBody` is left-factored on the first `Expr`: a following `";"` is a repeat, a `"|"` is a
comprehension, otherwise an element list. `BundleLit` is distinguished from a bare `TypeRef`
expression by the following `"{"`; in statement position this can collide with a block, so a
bundle literal at the start of a statement is parenthesized (or only appears in value position
after `=`, `<-`, `return`), a rule the implementation fixes.

---

## 9. Cross-references to the specification

| Grammar section | Spec section |
|-----------------|--------------|
| §2 Compilation unit and packages | §4 Top-level items and packages |
| §3 Modules | §5 Modules |
| §4 Types / Disciplines / Enums / Bundles | §6.1–§6.5 |
| §5 Capabilities and generics | §6.6 |
| §6 Functions | §7 |
| §7 Behavior | §8 |
| §8 Statements and expressions | §7.1, §8 |

Open points to settle during implementation, all noted above: operator precedence (§8.2), the
bundle-literal-vs-block disambiguation in statement position (§8.3), and the source of an
array-length const such as `N` in `fn reduce<T>(xs: T[N], ...)` — whether it is an implicit
const parameter bound by the argument or written explicitly.
