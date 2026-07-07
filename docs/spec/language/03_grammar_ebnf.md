# Part III — Grammar (EBNF)

*Piperine HDL — Grammar (EBNF)*

LL(1) after the inline left-factoring. Semantic distinctions (value vs net type, access vs call,
`[Expr]` as const-arg vs array-dim) are left to the type checker.

Notation: `::=` def, `|` alt, `{X}` zero+, `[X]` opt, `(X)` group, `"x"` terminal.

### 1. Lexical

```
Ident      ::= (letter|"_") { letter|digit|"_" }
RealLit    ::= Digits "." Digits [ ("e"|"E") ["+"|"-"] Digits ] [ SiSuffix ]
             | Digits ("e"|"E") ["+"|"-"] Digits [ SiSuffix ]
NatLit     ::= Digits [ SiSuffix ] | "0b" BinDigits | "0x" HexDigits
Digits     ::= digit { digit | "_" }                     -- '_' separators
SiSuffix   ::= "T"|"G"|"M"|"k" | "m"|"u"|"n"|"p"|"f"|"a" -- case-sensitive; M=mega, m=milli
QuadLit    ::= "0q" ("0"|"1"|"X"|"Z")
StringLit  ::= '"' {char} '"'
SysCall    ::= "$" Ident
```

Comments `//` and `/* */`. PascalCase vs snake_case is convention, not lexical. `@` prefixes
attributes (§9) and events (§7); position disambiguates.

Keywords: `mod analog digital impl fn bundle enum discipline capability const use pub param wire
var input output inout potential flow storage resolve tri or and for in if else match return
when self Self initial final posedge negedge change cross above`.

### 2. Compilation unit

```
CompilationUnit ::= { UseDecl | Item }
UseDecl   ::= "use" Path ";"
Path      ::= Ident { "::" Ident }
Item      ::= { Attribute } [ "pub" ] ItemKind
ItemKind  ::= ModDecl | BehaviorDecl | DisciplineDecl | BundleDecl | EnumDecl
            | CapabilityDecl | ImplDecl | FnDecl | ConstDecl
ConstDecl ::= "const" Ident ":" Type "=" Expr ";"
```

### 3. Attributes

```
Attribute ::= "@" Ident "(" [ AttrArg { "," AttrArg } ] ")"
AttrArg   ::= Ident "=" Expr
```

An attribute prefixes any declaration (item, port, `param`, `wire`, `var`, instance); stackable.
`Ident` is a plugin-registered schema; args are checked against it. Attributes are inert (§8 of
the language spec).

### 4. Modules

```
ModDecl    ::= "mod" Ident [ConstParams] [TypeParams] PortList [ModBody]
ConstParams::= "[" Ident {"," Ident} "]"
TypeParams ::= "<" TypeParam {"," TypeParam} ">"
TypeParam  ::= Ident [ ":" Bound ]
Bound      ::= Ident { "+" Ident }
PortList   ::= "(" [ Port {"," Port} [","] ] ")"
Port       ::= { Attribute } Direction Ident ":" Type
Direction  ::= "input" | "output" | "inout"
ModBody    ::= "{" { ModStmt } "}"
ModStmt    ::= { Attribute } ( ParamDecl | WireDecl | VarDecl | StructuralFor | StructuralIf
                             | AssertStmt | InstanceOrConnect )
ParamDecl  ::= "param" Ident ":" Type [ "=" Expr ] ";"
WireDecl   ::= "wire" Ident ":" Type ";"
VarDecl    ::= "var" Ident [ ":" Type ] [ "=" Expr ] ";"        -- type inferred if initialized
AssertStmt ::= "$assert" "(" Expr "," Expr ")" ";"
```

Instances / connections, left-factored on `Ident { Indexer | Field }`:

```
InstanceOrConnect ::= Ident { Indexer | Field } InstTail
InstTail  ::= ":" ModuleRef PortArgs [ParamArgs] ";"    -- named instance
            | ConstArgs PortArgs [ParamArgs] ";"         -- anon w/ const args
            | PortArgs [ParamArgs] ";"                   -- anon
            | "=" Expr ";"                               -- net connection
ModuleRef ::= Ident [ConstArgs] [TypeArgs]
ConstArgs ::= "[" Expr {"," Expr} "]"
TypeArgs  ::= "<" Type {"," Type} ">"
PortArgs  ::= "(" [ PortArg {"," PortArg} ] ")"
PortArg   ::= Expr | "." Ident "=" Expr                  -- positional or named
ParamArgs ::= "{" [ ParamArg {"," ParamArg} ] "}"
ParamArg  ::= "." Ident "=" Expr
Indexer   ::= "[" Expr "]"   ;   Field ::= "." Ident
StructuralFor ::= "for" Ident "in" Range ModBody
StructuralIf  ::= "if" "(" Expr ")" ModBody [ "else" (ModBody|StructuralIf) ]
Range     ::= Expr ("..'|"..=") Expr
```

### 5. Types

```
Type ::= Ident [TypeArgs] { Indexer }     -- Indexer = const-arg or array-dim (semantic)
```

Disciplines / enums / bundles:

```
DisciplineDecl ::= "discipline" Ident "{" { DisciplineItem } "}"
DisciplineItem ::= NatureDecl | StorageDecl | ResolveDecl
NatureDecl     ::= ("potential"|"flow") Ident ":" Type [ AttrList ] ";"
AttrList       ::= "(" NamedAttr {"," NamedAttr} ")"     -- unit="V", abstol=1e-6
NamedAttr      ::= Ident "=" Expr
StorageDecl    ::= "storage" Type ";"
ResolveDecl    ::= "resolve" ("tri"|"or"|"and"|"sum"|"avg"|"max"|"min") ";"
EnumDecl       ::= "enum" Ident [ ":" Type ] "{" EnumVariant {"," EnumVariant} [","] "}"
EnumVariant    ::= Ident [ "=" Expr ]
BundleDecl     ::= "bundle" Ident [ConstParams] [TypeParams] "{" [ Field {"," Field} [","] ] "}"
Field          ::= { Attribute } Ident ":" Type [ "=" Expr ]
```

### 6. Capabilities, generics, functions

```
CapabilityDecl ::= "capability" Ident [ ":" Ident {"," Ident} ] "{" { FnSig | FnDecl } "}"
FnSig          ::= "fn" Ident [TypeParams] ParamList "->" Type ";"
ImplDecl       ::= "impl" [ Ident "for" ] TypeRef "{" { FnDecl } "}"
TypeRef        ::= Ident [ConstArgs] [TypeArgs]
FnDecl         ::= "fn" Ident [TypeParams] ParamList "->" Type Block
ParamList      ::= "(" [ Param {"," Param} ] ")"
Param          ::= "self" | Ident ":" Type
Block          ::= "{" { Stmt } [ Expr ] "}"             -- trailing Expr = value
```

`impl Cap for T` vs `impl T`: peek for `for` after the first `Ident`.

### 7. Behavior

```
BehaviorDecl  ::= ("analog"|"digital") Ident "{" { BehaviorStmt } "}"
BehaviorStmt  ::= VarDecl | BindStmt | IfStmt | MatchStmt | ForStmt | EventBlock | Diagnostic | ExprStmt
BindStmt      ::= Expr BindOp Expr ";"     ;   BindOp ::= "<+" | "<-" | "="
EventBlock    ::= "@" EventSpec [ "when" "(" Expr ")" ] Block
EventSpec     ::= EventTerm | "(" EventTerm { ("|"|"or") EventTerm } ")"
EventTerm     ::= Ident "(" [Expr] ")" | "initial" | "final"     -- name resolved by registry
Diagnostic    ::= SysCall "(" [ Expr {"," Expr} ] ")" ";"
```

LHS of `<+`/`<-` must be an access (checker); of `=`, an lvalue. After the LHS `Expr`, the
operator (or `";"`) gives the single-token branch.

### 8. Statements and expressions

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
Call      ::= "(" [ Expr {"," Expr} ] ")"   ;   Slice ::= "[" Expr ("..'|"..=") Expr "]"
PathSeg   ::= "::" Ident

Primary   ::= Literal | SysCall | Ident | "(" Expr ")" | Block | IfExpr | ArrayExpr | BundleLit | Lambda
IfExpr    ::= "if" "(" Expr ")" Block "else" Block
ArrayExpr ::= "[" ( Expr ";" Expr | Expr "|" Ident "in" Range | Expr {"," Expr} ) "]"
BundleLit ::= TypeRef "{" [ FieldInit {"," FieldInit} [","] ] "}"
FieldInit ::= "." Ident "=" Expr
Lambda    ::= "|" [ Ident {"," Ident} ] "|" Expr
```

`BundleLit` (needs `{` after `TypeRef`) vs a block collides in statement position; a
statement-leading bundle literal is parenthesized or appears in value position (after
`=`/`<-`/`return`). Operator precedence is a starting point, tunable.

Native canonical spellings only: `|` (not `or`) for event OR, one print/diagnostic per severity.
Verilog-AMS aliases (`or`, `log`, `$warning`, `$stop`, `$strobe`/`$monitor`) are accepted only in
the AMS ingestion front end.

---

