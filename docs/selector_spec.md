# Piperine Selector — A Query Language for the Object Model

The selector is the string query language over the Piperine Object Model (POM). It is the
"XPath of the circuit": a compact path expression that, evaluated against a design, returns a
`Selection<Node>`. It is the single addressing mechanism reused by reflection (find nodes),
overrides (address what to retune), and annotation (address where to attach).

The selector is deliberately thin. It introduces no new model of its own — its **axes are POM
relations** and its **predicates are POM attributes** (see the reflection API). Everything it
can express is navigation and filtering of the object graph that already exists.

---

## 1. Model

A selector is evaluated against a **context node** and produces an ordered, duplicate-free
`Selection<Node>`. The context is supplied by where evaluation starts:

- `design.select(path)` — context is the design root.
- `module.select(path)` — context is that module (the default inside a `bench`, whose module is
  the root).
- `selection.where(path)` — context is each node of an existing selection.

A path is a sequence of **steps**. Each step moves from the current node-set to a new one along
an **axis**, keeps the nodes matching a **node test**, and filters them through zero or more
**predicates**. Results across all context nodes are unioned, then deduplicated by node
identity (§1.2 of the reflection API), preserving first-seen order.

---

## 2. Grammar (EBNF)

```
Selector   ::= [ "/" | "//" ] Step { ( "/" | "//" ) Step }

Step       ::= [ Axis "::" ] NodeTest { Predicate }

Axis       ::= "inst" | "net" | "port" | "param" | "aspect" | "behavior"
             | "driver" | "load" | "parent" | "ancestor"

NodeTest   ::= Name | "*"

Predicate  ::= "[" ( Index | PredExpr ) "]"
Index      ::= NaturalLit | "last" "(" ")"

PredExpr   ::= OrExpr
OrExpr     ::= AndExpr { "or" AndExpr }
AndExpr    ::= NotExpr { "and" NotExpr }
NotExpr    ::= "not" "(" PredExpr ")" | Compare
Compare    ::= Operand [ CmpOp Operand ]
CmpOp      ::= "==" | "!=" | "<" | "<=" | ">" | ">=" | "~"

Operand    ::= AttrRef | AxisRef | Func | Literal
AttrRef    ::= "@" Name                          -- @name @direction @width @path @discipline ...
AxisRef    ::= Axis "::" NodeTest                 -- existence, or value when compared
Func       ::= "of" "(" StringLit ")" | "count" "(" AxisRef ")"
Literal    ::= NumberLit | StringLit | BoolLit | Ident   -- Ident: an enum value or a node name
```

A leading `/` makes the path absolute from the context; `//` starts at any descendant; neither
makes it relative. `~` is glob match on strings.

---

## 3. Axes

Each axis is a POM relation. The default axis (no `axis::` prefix) is `inst::`, since walking
the instance hierarchy is the common case.

| Axis | From → to | POM relation |
|------|-----------|--------------|
| `inst::` *(default)* | module/instance → child instances | `instances()` / `children()` |
| `net::` | module/instance → nets | `nets()` |
| `port::` | module/instance → ports | `ports()` |
| `param::` | module/instance → params | `params()` |
| `aspect::` | module → aspects | `aspects()` |
| `behavior::` | module → behaviors | `behaviors()` |
| `driver::` | net → driving ports | `drivers()` |
| `load::` | net → reading ports | `loads()` |
| `parent::` | node → containing instance/module | (reverse of `inst::`) |
| `ancestor::` | node → all enclosing instances | (transitive `parent::`) |

`//` is shorthand for the descendant closure over `inst::`: `//X` matches `X` at any instance
depth. A step after `//` may switch axis (`//*/net::clk` is "every descendant instance's net
`clk`").

---

## 4. Node tests

A node test selects which nodes on the axis are kept:

- **`*`** — any node on the axis.
- **A name** — matches by node name. On the `inst::` axis the matching follows the language's
  naming convention: a **PascalCase** test matches by *module type* (the instance's `of()`,
  by source name, so `Dac` matches a monomorphized `Dac__8`); a **snake_case** test matches by
  *instance name*. On the other axes (nets, ports, params, …) names are lowercase, so a name
  test always matches by name.

An instance array — `leg[N]` in source — produces N instances sharing the base name `leg`. The
test `leg` matches all replicas; an index predicate (§5) picks one.

```
//Resistor          every Resistor instance, any depth        (type test, PascalCase)
/dac                the child instance named dac               (name test, snake_case)
//leg               all replicas of the leg array              (array base name)
net::out            the net named out on the context
port::*             every port of the context
```

---

## 5. Predicates

A predicate filters the node-set. It is either a positional index or a boolean expression.

### 5.1 Positional

```
//leg[0]            the first leg replica       (0-based, matching Selection.get)
//leg[last()]       the last replica
```

### 5.2 Attributes

`@name` reads a scalar attribute of the candidate node (the attributes of §4–§5 of the
reflection API) and compares it:

```
//*/port::*[@direction == Input]          all input ports, any depth
//*/net::*[@discipline == Electrical]     all electrical nets
//*/net::*[@width > 1]                     all buses
//Comparator[@name ~ "cmp*"]               Comparators whose name matches the glob
```

Enum values (`Input`, `Output`, `Inout`) and node names like `Electrical` are written as bare
identifiers; strings use quotes and `~` for glob.

### 5.3 Relations

A bare `axis::test` in a predicate is an **existence** test (true when the sub-selection is
non-empty). Compared against a value, it tests the matched node's value ("there exists one
satisfying"):

```
//*[aspect::layout]                  instances whose module declares a layout aspect
//Resistor[param::r > 1e3]           Resistors whose param r exceeds 1 kΩ
//*[count(load::*) == 0]             nets with no readers (dangling) — via net context
//*[net::clk]                        instances that have a net named clk
```

### 5.4 Boolean combination

```
//Resistor[param::r > 1e3 and param::r < 1e6]
//*[port::clk or port::clock]
//*[not(aspect::layout)]                       instances missing a layout aspect
```

`and` binds tighter than `or`; `not(...)` is a function form. Predicates in sequence are
implicitly conjoined and applied left to right: `//leg[param::w > 1.0][0]` is "the first leg
with `w > 1.0`."

---

## 6. Abbreviations

| Written | Means |
|---------|-------|
| `X` | `inst::X` (default axis) |
| `/X` | child `X` of the context |
| `//X` | descendant `X` at any depth |
| `[i]` | the i-th match (0-based) |
| `@a` | attribute `a` of the candidate |

---

## 7. Evaluation semantics

A step is evaluated against the current node-set `S`:

1. For each node `n` in `S`, follow the step's axis from `n`, producing candidate nodes.
2. Keep candidates passing the node test.
3. Apply each predicate left to right; a boolean predicate keeps nodes for which it holds, a
   positional predicate keeps the node at that ordinal of the current candidate list.
4. Union the surviving candidates across all `n`, deduplicate by node identity, preserve
   first-seen order.

The result of the final step is the selector's `Selection<Node>`. An empty result is a valid
empty selection, never an error — callers test `is_empty()` or use `Selection.one()` when
exactly one node is required.

Determinism: evaluation is a pure function of the elaborated design (plus any staged
overrides), so the same selector over the same design always yields the same ordered selection.

---

## 8. Integration with reflection and staging

A selector returns a `Selection<Node>`, on which the reflection and staging operations apply
directly:

```piperine
// read
for r in select("//Resistor") { $info(r.param("r").value()); }

// stage an override across a whole set (re-elaborated purely on the next simulate)
select("//dac/param::vref").set(1.8);
select("//leg").set("w", 2.0);

// narrow further from a selection
var big = select("//Resistor").where("[param::r > 1e3]");

// attach (annotation) — defined in the extensibility spec
select("//*/net::*[@layer == \"m3\"]").attach( Capacitor { .c = 4.2e-15 } );
```

Because the selector's axes and predicates are POM relations and attributes, the language is
fully determined by the reflection model: adding a node type or attribute there extends what
the selector can address, with no change to this grammar.

---

## 9. Open questions

- **Reverse axes.** `parent::` and `ancestor::` are listed but are the least-used; whether they
  ship in v1 or wait depends on real query needs.
- **Param shorthand in predicates.** `[param::r > 1e3]` is explicit; a shorthand `[r > 1e3]`
  (bare name defaulting to the `param::` axis on instances) would be terser but reintroduces an
  implicit-axis rule. Left explicit for now, in keeping with no-magic.
- **Value extraction vs node selection.** A selector always returns *nodes*; reading a value is
  a reflection call on the result (`.value()`). Whether a terminal `@attr` form that yields
  values directly is worth adding is open.
- **Identity of array replicas.** Selecting `leg` returns all replicas in index order; whether
  the index predicate `[i]` and the source index always coincide after monomorphization needs
  to be pinned down against the elaborator.
