# Part V — Selector

*Piperine Selector — Query Language for the Object Model*

The selector ("XPath of the circuit") evaluates against a design and returns `Selection<Node>`.
It is the one addressing mechanism for reflection, overrides, and annotation. It adds no model:
**axes are POM relations, predicates are POM attributes.**

### 1. Model

Evaluated against a **context node** (`design.select`, `module.select`, or `selection.where`),
producing an ordered, duplicate-free `Selection<Node>`. A path is **steps**; each step moves
along an **axis**, keeps nodes matching a **node test**, filters through **predicates**. Results
union across context nodes, dedup by identity, first-seen order.

### 2. Grammar (EBNF)

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
CmpOp     ::= "=="|"!="|"<"|"<="|">"|">="|"~"          -- '~' = glob
Operand   ::= AttrRef | AxisRef | Func | Literal
AttrRef   ::= "@" Name [ "." Name ]                     -- @direction ; @layout.min_width
AxisRef   ::= Axis "::" NodeTest
Func      ::= "of" "(" StringLit ")" | "count" "(" AxisRef ")"
Literal   ::= NumberLit | StringLit | BoolLit | Ident
```

`/` absolute from context; `//` descendant closure over `inst::`.

### 3. Axes

| Axis | POM relation |
|------|--------------|
| `inst::` *(default)* | `instances()`/`children()` |
| `net:: port:: param:: attr:: behavior::` | `nets/ports/params/attributes/behaviors()` |
| `driver:: load::` | net `drivers()/loads()` |
| `parent:: ancestor::` | reverse / transitive |

`//X` = `X` at any instance depth; a step after `//` may switch axis (`//*/net::clk`).

### 4. Node tests

`*` = any. A name matches by node name; on `inst::`, **PascalCase** matches by module type
(`of()`, source name — `Dac` matches `Dac__8`), **snake_case** by instance name. An instance
array `leg[N]` shares base name `leg` (matches all replicas; index predicate picks one).

### 5. Predicates

Positional `[i]` (0-based) / `[last()]`. Attribute `@name`/`@schema.field` compared
(`[@direction == Input]`, `[@width > 1]`, `[@layout.min_width > 1u]`, `[@name ~ "cmp*"]`);
enum/node names are bare identifiers, strings quoted. A bare `axis::test` is existence
(`[attr::layout]`, `[net::clk]`); compared, it tests the matched value (`[param::r > 1k]`).
Boolean `and`/`or`/`not(...)`; `and` binds tighter; sequential predicates are conjoined
left-to-right.

### 6. Evaluation

Per step over node-set S: for each n, follow the axis, keep node-test matches, apply predicates
left-to-right (boolean filters; positional keeps the ordinal), union+dedup by identity. Empty
result is valid (not an error; use `is_empty()`/`one()`). Pure function of the elaborated design
+ staged overrides → deterministic.

### 7. Integration

```piperine
for r in select("//Resistor") { $info("{}", r.param("r").value()); }
select("//dac/param::vref").set(1.8);
select("//leg").set("w", 2.0);
var big = select("//Resistor").where("[param::r > 1k]");
select("//*/net::*[@layout.layer == \"m3\"]").attach( Capacitor { .c = 4.2f } );
```

Adding a POM node type or attribute extends what the selector addresses, with no grammar change.

### 8. Open questions

Reverse axes (`parent::`/`ancestor::`) ship v1 or later. Param shorthand `[r > 1k]` (bare-name
default `param::`) left explicit (no-magic). Terminal value form (`@attr` yielding a value vs a
node) open. Array-replica index alignment after monomorphization to be pinned against the
elaborator.

---

