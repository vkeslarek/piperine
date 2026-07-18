# Appendix B — Complete Grammar (EBNF)

The consolidated LL(1) grammar for PHDL. Every production in this appendix also appears
in the section it belongs to (Lexical in Part I §4, Types in §6, etc.); this appendix
gathers them in one place for reference.

## Notation

`::=` definition · `|` alternative · `{X}` zero or more · `[X]` optional · `(X)` group
· `"x"` literal terminal.

The grammar is LL(1) after one inline left-factoring (instance parsing — §B.4).
Semantic distinctions the parser cannot make on one token of lookahead (value vs net
type, access vs call, `[Expr]` as const-arg vs array-dim) are deferred to the type
checker.

## B.1 Lexical

```
Ident      ::= (letter|"_") { letter|digit|"_" }
RealLit    ::= Digits "." Digits [ ("e"|"E") ["+"|"-"] Digits ] [ SiSuffix ]
             | Digits ("e"|"E") ["+"|"-"] Digits [ SiSuffix ]
NatLit     ::= Digits [ SiSuffix ] | "0b" BinDigits | "0x" HexDigits
Digits     ::= digit { digit | "_" }                     -- '_' separators anywhere between digits
SiSuffix   ::= "T"|"G"|"M"|"k" | "m"|"u"|"n"|"p"|"f"|"a" -- case-sensitive; M=mega, m=milli
QuadLit    ::= "0q" ("0"|"1"|"X"|"Z")
StringLit  ::= '"' {char} '"'
SysCall    ::= "$" Ident
```

Comments: `//` line, `/* */` block. `@` prefixes both attributes (§B.3) and events
(§B.7); position disambiguates.

Reserved spellings are listed in Part I §4.3. Lexer-level sigils: `$`, `@`, `<+`
(contribution), `<-` (force), `?` (optional type; pattern wildcard), `..` / `..=`
(ranges), `::` (path), `=>` (match arm), `->` (fn return), `|` (lambda; event OR), `=`
(assignment).

## B.2 Compilation unit

```
CompilationUnit ::= { UseDecl | Item }
UseDecl   ::= "use" Path ";"
Path      ::= Ident { "::" Ident }
Item      ::= { Attribute } [ "pub" ] ItemKind
ItemKind  ::= ModDecl | BehaviorDecl | DisciplineDecl | BundleDecl | EnumDecl
             | CapabilityDecl | ImplDecl | FnDecl | ConstDecl
ConstDecl ::= "const" Ident ":" Type "=" Expr ";"
```

## B.3 Attributes

```
Attribute ::= "@" Ident "(" [ AttrArg { "," AttrArg } ] ")"
AttrArg   ::= Ident "=" Expr
```

An attribute prefixes any declaration (item, port, `param`, `wire`, `var`, instance);
stackable. `Ident` is a plugin-registered schema; arguments are checked against it.
Attributes are inert (Part I §8).

## B.4 Modules

```
ModDecl     ::= "mod" Ident [ConstParams] [TypeParams] PortList [ModBody]
ConstParams ::= "[" Ident {"," Ident} "]"
PortList    ::= "(" [ Port {"," Port} [","] ] ")"
Port        ::= { Attribute } Direction Ident ":" Type
Direction   ::= "input" | "output" | "inout"
ModBody     ::= "{" { ModStmt } "}"
ModStmt     ::= { Attribute } ( ParamDecl | WireDecl | VarDecl | StructuralFor
                                | StructuralIf | AssertStmt | InstanceOrConnect )
ParamDecl  ::= "param" Ident ":" Type [ "=" Expr ] ";"
WireDecl   ::= "wire" Ident ":" Type ";"
VarDecl    ::= "var" Ident [ ":" Type ] [ "=" Expr ] ";"
AssertStmt ::= "$assert" "(" Expr "," Expr ")" ";"
```

Instances and connections, left-factored on `Ident { Indexer | Field }`:

```
InstanceOrConnect ::= Ident { Indexer | Field } InstTail
InstTail  ::= ":" ModuleRef PortArgs [ParamArgs] ";"     -- named instance
             | ConstArgs PortArgs [ParamArgs] ";"        -- anon w/ const args
             | PortArgs [ParamArgs] ";"                   -- anon
             | "=" Expr ";"                               -- net connection
ModuleRef ::= Ident [ConstArgs] [TypeArgs]
ConstArgs ::= "[" Expr {"," Expr} "]"
TypeArgs  ::= "<" Type {"," Type} ">"
PortArgs  ::= "(" [ PortArg {"," PortArg} ] ")"
PortArg   ::= Expr | "." Ident "=" Expr
ParamArgs ::= "{" [ ParamArg {"," ParamArg} ] "}"
ParamArg  ::= "." Ident "=" Expr
Indexer   ::= "[" Expr "]"   ;   Field ::= "." Ident
StructuralFor ::= "for" Ident "in" Range ModBody
StructuralIf  ::= "if" "(" Expr ")" ModBody [ "else" (ModBody|StructuralIf) ]
Range     ::= Expr (".."|"..=") Expr
```

## B.5 Types

```
Type      ::= TupleType | FnType | NamedType
NamedType ::= Ident [TypeArgs] { Indexer } [ "?" ]
FnType    ::= "fn" "(" [ Type {"," Type} [","] ] ")" [ "->" Type ] [ "?" ]
TupleType ::= "(" Type "," Type {"," Type} [","] ")" [ "?" ]
```

A trailing `?` marks an optional type (`Real?`, Part I §6.1). `fn(T, U) -> R` is a
function type (Part I §9.2). `(T, U)` is a tuple type; a single parenthesized type is
not a 1-tuple.

### B.5.1 Disciplines

```
DisciplineDecl ::= "discipline" Ident "{" { DisciplineItem } "}"
DisciplineItem ::= NatureDecl | StorageDecl | ResolveDecl
NatureDecl     ::= ("potential"|"flow") Ident ":" Type [ AttrList ] ";"
AttrList       ::= "(" NamedAttr {"," NamedAttr} ")"
NamedAttr      ::= Ident "=" Expr
StorageDecl    ::= "storage" Type ";"
ResolveDecl    ::= "resolve" ("tri"|"or"|"and"|"sum"|"avg"|"max"|"min") ";"
```

### B.5.2 Enums

```
EnumDecl    ::= "enum" Ident [ ":" Type ] "{" EnumVariant {"," EnumVariant} [","] "}"
EnumVariant ::= Ident [ "=" Expr ]
```

### B.5.3 Bundles

```
BundleDecl ::= "bundle" Ident [ConstParams] [TypeParams] "{" [ Field {"," Field} [","] ] "}"
Field      ::= { Attribute } Ident ":" Type [ "=" Expr ]
```

## B.6 Capabilities, generics, functions

```
CapabilityDecl ::= "capability" Ident [ ":" Ident {"," Ident} ] "{" { FnSig | FnDecl } "}"
FnSig          ::= "fn" Ident [TypeParams] ParamList "->" Type ";"
ImplDecl       ::= "impl" [ Ident "for" ] TypeRef "{" { FnDecl } "}"
TypeRef        ::= Ident [ConstArgs] [TypeArgs]
FnDecl         ::= "fn" Ident [TypeParams] ParamList "->" Type Block
ParamList      ::= "(" [ Param {"," Param} ] ")"
Param          ::= "self" | Ident ":" Type [ "=" Expr ]
Block          ::= "{" { Stmt } [ Expr ] "}"
TypeParams     ::= "<" TypeParam {"," TypeParam} ">"
TypeParam      ::= Ident [ ":" Bound ]
Bound          ::= Ident { "+" Ident }
```

`impl Cap for T` vs `impl T`: peek for `for` after the first identifier.

## B.7 Behavior

```
BehaviorDecl  ::= ("analog"|"digital") Ident "{" { BehaviorStmt } "}"
BehaviorStmt  ::= VarDecl | BindStmt | IfStmt | MatchStmt | ForStmt | EventBlock
                | Diagnostic | ExprStmt
BindStmt      ::= Expr BindOp Expr ";"     ;   BindOp ::= "<+" | "<-" | "="
EventBlock    ::= "@" EventSpec [ "when" "(" Expr ")" ] Block
EventSpec     ::= EventTerm | "(" EventTerm { ("|"|"or") EventTerm } ")"
EventTerm     ::= Ident "(" [Expr] ")" | "initial" | "final"
Diagnostic    ::= SysCall "(" [ Expr {"," Expr} ] ")" ";"
```

The left-hand side of `<+` / `<-` must be an access expression (type-checked); the
left-hand side of `=` must be an lvalue.

## B.8 Statements and expressions

```
Stmt      ::= VarDecl | ReturnStmt | IfStmt | MatchStmt | ForStmt | BindStmt | ExprStmt
ReturnStmt::= "return" Expr ";"   ;   ExprStmt ::= Expr ";"
IfStmt    ::= "if" "(" Expr ")" Block [ "else" (Block|IfStmt) ]
ForStmt   ::= "for" Ident "in" Range Block
MatchStmt ::= "match" Expr "{" { MatchArm } "}"
MatchArm  ::= Pattern "=>" Block [","]
Pattern   ::= Path | "_" | BitPattern            -- BitPattern: "0b" {"0"|"1"|"?"}

Expr      ::= OrExpr
OrExpr    ::= AndExpr  { "|"  AndExpr }
AndExpr   ::= EqExpr   { "&"  EqExpr }
EqExpr    ::= RelExpr  { ("=="|"!=") RelExpr }
RelExpr   ::= XorExpr  { ("<"|"<="|">"|">=") XorExpr }
XorExpr   ::= AddExpr  { "^"  AddExpr }
AddExpr   ::= MulExpr  { ("+"|"-") MulExpr }
MulExpr   ::= UnaryExpr{ ("*"|"/"|"%") UnaryExpr }
UnaryExpr ::= ("!"|"-") UnaryExpr | PostfixExpr
PostfixExpr::= Primary { Call | Indexer | Slice | Field | PathSeg }
Call      ::= "(" [ Expr {"," Expr} ] ")"
Slice     ::= "[" Expr (".."|"..=") Expr "]"
PathSeg   ::= "::" Ident

Primary   ::= Literal | SysCall | Ident | "(" Expr ")" | Block | IfExpr
             | ArrayExpr | BundleLit | MapLit | SetLit | Lambda
IfExpr    ::= "if" "(" Expr ")" Block "else" Block
ArrayExpr ::= "[" ( Expr ";" Expr | Expr "|" Ident "in" Range | Expr {"," Expr} ) "]"
BundleLit ::= TypeRef "{" [ FieldInit {"," FieldInit} [","] ] "}"
FieldInit ::= "." Ident "=" Expr
MapLit    ::= "Map" "{" [ MapEntry {"," MapEntry} [","] ] "}"
MapEntry  ::= Ident ":" Expr | StringLit ":" Expr
SetLit    ::= "Set" "{" [ Expr {"," Expr} [","] ] "}"
Lambda    ::= "|" [ Ident {"," Ident} ] "|" Expr
```

A `BundleLit` in statement position (where it might collide with a block) must be
parenthesized or appear in value position (after `=`, `<-`, or `return`).

Native canonical spellings only: `|` (not `or`) for event OR; one diagnostic per
severity. Verilog-AMS aliases are accepted only in the AMS ingestion front end.

## B.10 Selector

```
Selector  ::= [ "/" | "//" ] Step { ( "/" | "//" ) Step }
Step      ::= [ Axis "::" ] NodeTest { Predicate }
Axis      ::= "inst"|"net"|"port"|"param"|"attr"|"behavior"|"driver"|"load"|"parent"|"ancestor"
NodeTest  ::= Name | "*"
Predicate ::= "[" ( Index | PredExpr ) "]"
Index     ::= NatLit | "last" "(" ")"
PredExpr  ::= OrExpr
OrExpr    ::= AndExpr { "or" AndExpr }
AndExpr   ::= NotExpr { "and" NotExpr }
NotExpr   ::= "not" "(" PredExpr ")" | Compare
Compare   ::= Operand [ CmpOp Operand ]
CmpOp     ::= "=="|"!="|"<"|"<="|">"|">="|"~"
Operand   ::= AttrRef | AxisRef | Func | Literal
AttrRef   ::= "@" Name [ "." Name ]
AxisRef   ::= Axis "::" NodeTest
Func      ::= "of" "(" StringLit ")" | "count" "(" AxisRef ")"
Literal   ::= NumberLit | StringLit | BoolLit | Ident
```

`/` is absolute from the context node; `//` is descendant closure over `inst::`. Full
semantics: Part IV §8–§14.

## B.11 Leaf enums

```
Direction      ::= Input | Output | Inout
Domain         ::= Analog | Digital
DisciplineKind ::= Conservative | Storage
Resolution     ::= Single | Tri | Or | And | Sum | Avg | Max | Min | Kcl
NatureKind     ::= Potential | Flow
ReflectError   ::= NotFound | NotSettable | TypeMismatch | OutOfRange
                 | MultipleDrivers | Other
```

## B.12 Const-expression subset

```
ConstExpr     ::= ConstOrExpr
ConstOrExpr   ::= ConstAndExpr { "or" ConstAndExpr }
ConstAndExpr  ::= ConstNotExpr { "and" ConstNotExpr }
ConstNotExpr  ::= "not" "(" ConstExpr ")" | ConstCompare
ConstCompare  ::= ConstUnary [ CmpOp ConstUnary ]
ConstUnary    ::= ("!"|"-") ConstUnary | ConstPostfix
ConstPostfix  ::= ConstPrimary { "[" ConstExpr "]" }
ConstPrimary  ::= Literal | Ident | "(" ConstExpr ")" | BlockWithValue
```

Evaluatable: literals (including SI-suffixed and `_`-separated numerics), named
bindings, unary `-`/`!`, binary arithmetic/comparison/bitwise, `if`/`else`, and a block
with a trailing value. Anything else (general calls, field access, comprehensions,
runtime values) is rejected as `NotConst`.
