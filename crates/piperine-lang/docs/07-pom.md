# Piperine Object Model (POM)

The Piperine Object Model (POM) is the reflection API for elaborated designs. It lives in `src/pom/` and is the output of the elaboration phase — a fully type-resolved, monomorphized representation of the design that consumers (the lowering pass, CLI tools, plugins) query through a uniform interface.

---

## Design

`Design` (`design.rs`) is the POM root. It is produced by `SourceFile::elaborate()` and contains the complete elaborated design.

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `modules` | `HashMap<String, Module>` | All elaborated (monomorphized) modules |
| `disciplines` | `HashMap<String, DisciplineDecl>` | All discipline declarations |
| `enums` | `HashMap<String, EnumDecl>` | All enum declarations |
| `capabilities` | `HashMap<String, CapabilityDecl>` | All capability declarations |
| `functions` | `HashMap<String, Function>` | All global functions |
| `impls` | `Vec<ImplBlock>` | All impl blocks |
| `overrides` | `Rc<RefCell<OverrideMap>>` | Staged parameter overrides (single mutation surface) |
| `top_module` | `Option<String>` | The top module name, if set |

### Public Accessors

| Method | Returns | Description |
|--------|---------|-------------|
| `top()` | `Option<&Module>` | The elaborated top module, if set |
| `set_top(name)` | — | Set the top module by name |
| `module(name)` | `Option<&Module>` | Look up a module by name |
| `modules()` | `impl Iterator<Item = &Module>` | Iterate all elaborated modules |
| `module_count()` | `usize` | Number of elaborated modules |
| `function(name)` | `Option<&Function>` | Look up a global function by name |
| `disciplines()` | `impl Iterator<Item = (&String, &DisciplineDecl)>` | Iterate all disciplines |
| `discipline(name)` | `Option<&DisciplineDecl>` | Look up a discipline by name |
| `enums()` | `impl Iterator<Item = (&String, &EnumDecl)>` | Iterate all enums |
| `enum_(name)` | `Option<&EnumDecl>` | Look up an enum by name |
| `capabilities()` | `impl Iterator<Item = (&String, &CapabilityDecl)>` | Iterate all capabilities |
| `capability(name)` | `Option<&CapabilityDecl>` | Look up a capability by name |
| `functions()` | `impl Iterator<Item = &Function>` | Iterate all global functions |
| `impls()` | `&[ImplBlock]` | All impl blocks |

### Staging Layer

The staging layer provides parameter overrides without mutating the elaborated design in place. Overrides are staged for re-elaboration.

| Method | Description |
|--------|-------------|
| `set_param(path, param, value)` | Stage a parameter override by instance path and param name |
| `get_override(path, param)` | Look up a staged override, returns `Option<Value>` |
| `has_overrides()` | Returns `true` if any overrides are staged |
| `clear_overrides()` | Clear all staged overrides |

---

## Module

`Module` (`module.rs`) represents an elaborated (monomorphized) module.

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Module name |
| `ports` | `Vec<Port>` | Module ports |
| `params` | `Vec<Param>` | Module parameters |
| `wires` | `Vec<Wire>` | Internal wires |
| `instances` | `Vec<Instance>` | Child instances |
| `connections` | `Vec<Connection>` | Net aliasing connections |
| `behaviors` | `Vec<Behavior>` | Analog/digital behavior blocks |

### Accessors

`Module` provides accessor slices for each field: `ports()`, `params()`, `wires()`, `instances()`, `connections()`, `behaviors()`. Individual lookup methods (`port(name)`, `param(name)`, `wire(name)`, `instance(name)`) return `Option<&T>`.

---

## Port

A module port. Implements `Named`, `NetTyped`, `Kinded`.

| Field | Type | Description |
|-------|------|-------------|
| `direction` | `Direction` | Input, Output, or Inout |
| `name` | `String` | Port name |
| `ty` | `NetType` | Port net type (discipline or array) |

---

## Param

A module parameter. Implements `Named`, `Kinded`.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Parameter name |
| `ty` | `ValueType` | Parameter value type |
| `default` | `Option<ConstVal>` | Optional compile-time default |

`Param::value()` returns the default converted to a POM `Value`, or `None` if no default exists.

---

## Wire

An internal wire. Implements `Named`, `NetTyped`, `Kinded`.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Wire name |
| `ty` | `NetType` | Wire net type |

---

## Instance

A child instance. Implements `Named`, `Kinded`. The name returns the label if present, otherwise the module name.

| Field | Type | Description |
|-------|------|-------------|
| `label` | `Option<String>` | Optional instance label |
| `module` | `String` | Instantiated module name |
| `ports` | `Vec<NetRef>` | Port connections |
| `params` | `Vec<(String, ConstVal)>` | Parameter assignments |

---

## Connection

A net aliasing connection (`connect lhs rhs;`).

| Field | Type | Description |
|-------|------|-------------|
| `lhs` | `NetRef` | Left-hand side net reference |
| `rhs` | `NetRef` | Right-hand side net reference |

---

## Behavior

A behavior block (`analog` or `digital`). Implements `Named`, `Kinded`. Located in `behavior.rs`.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Behavior block name |
| `kind` | `BehaviorKind` | `Analog` or `Digital` |
| `body` | `Vec<BehaviorStmt>` | Behavior body statements |

Convenience methods: `is_analog()` → `bool`, `is_digital()` → `bool`.

---

## BehaviorStmt

`BehaviorStmt` (`behavior.rs`) mirrors the parse AST `BehaviorStmt` but with resolved types. It is the statement node for both behavior blocks and function bodies.

### Variants

| Variant | Fields | Description |
|---------|--------|-------------|
| `VarDecl` | `name`, `ty`, `default` | Variable declaration with optional initializer |
| `Bind` | `dest`, `op`, `src` | Assignment (`=`), contribution (`<+`), or force (`<-`) |
| `If` | `cond`, `then_body`, `else_body` | Conditional |
| `Match` | `expr`, `arms` | Pattern match |
| `Event` | `spec`, `guard`, `body` | Event-triggered block |
| `Return` | `Expr` | Return from a function |
| `Diagnostic` | `sys`, `args` | System task (`$display`, `$error`, etc.) |
| `Expr` | `Expr` | Expression statement |

---

## MatchArm

| Field | Type | Description |
|-------|------|-------------|
| `pat` | `Pattern` | Match pattern (Path or Wildcard) |
| `body` | `Vec<BehaviorStmt>` | Arm body statements |

---

## Function

A global function or impl method. Implements `Named`.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Function name |
| `params` | `Vec<(String, TypeRef)>` | Parameter names and types |
| `ret` | `TypeRef` | Return type |
| `body` | `Vec<BehaviorStmt>` | Function body statements |

---

## ImplBlock

An impl block.

| Field | Type | Description |
|-------|------|-------------|
| `capability` | `Option<String>` | Optional capability being implemented |
| `ty` | `String` | Target type name |
| `const_args` | `Vec<ConstVal>` | Constant arguments |
| `methods` | `Vec<Function>` | Methods in this impl block |

---

## Value

`Value` (`value.rs`) is the value-layer scalar — the POM's representation of data values. It is never a net or piece of hardware.

### Variants

| Variant | Rust Type | Description |
|---------|-----------|-------------|
| `Real(f64)` | `f64` | 64-bit floating point |
| `Natural(u64)` | `u64` | Unsigned integer |
| `Integer(i64)` | `i64` | Signed integer |
| `Boolean(bool)` | `bool` | Boolean |
| `Quad(u8)` | `u8` | 4-state logic value (0/1/X/Z) |
| `String(String)` | `String` | String |
| `Complex(f64, f64)` | `(f64, f64)` | Complex number (real, imag) |

### Accessor Methods

`as_real()`, `as_natural()`, `as_integer()`, `as_boolean()`, `as_quad()`, `as_string()`, `as_complex()` — each returns `Option<T>`, succeeding only for the matching variant.

`type_name()` returns a `&'static str` like `"Real"`, `"Integer"`, etc.

### From Implementations

`From<f64>`, `From<u64>`, `From<i64>`, `From<bool>`, `From<String>`, `From<&str>`, `From<Complex64>`, `From<&ConstVal>`.

---

## NetRef

`NetRef` (`net_type.rs`) is a reference to a net, optionally indexed into a bus array.

| Field | Type | Description |
|-------|------|-------------|
| `net` | `String` | Net name |
| `index` | `Option<u64>` | Optional bus index |

Constructors: `NetRef::simple(name)` and `NetRef::indexed(name, index)`. Implements `Display` — formats as `name` or `name[index]`.

---

## NetType

`NetType` (`net_type.rs`) types a port or wire — always a discipline or a fixed-size array of one.

| Variant | Fields | Description |
|---------|--------|-------------|
| `Discipline(String)` | discipline name | Single discipline-typed net |
| `Array(Box<NetType>, u64)` | inner type, size | Fixed-size array of nets |

Methods: `discipline_name()` extracts the base discipline name (unwrapping arrays). `width()` returns the total number of nets.

---

## ValueType

`ValueType` (`net_type.rs`) types a param, variable, or function result.

### Variants

| Variant | Description |
|---------|-------------|
| `Real` | 64-bit floating point |
| `Natural` | Unsigned integer |
| `Integer` | Signed integer |
| `Complex` | Complex number |
| `Boolean` | Boolean |
| `Quad` | 4-state logic |
| `Str` | String |
| `Enum(String)` | Named enum type |
| `Array(Box<ValueType>, u64)` | Fixed-size array |
| `FnPtr(Vec<TypeRef>, Box<TypeRef>)` | Function pointer (param types, return type) |

---

## TypeRef

`TypeRef` (`net_type.rs`) is either half of the value/net split — the type that a param, function argument, or return type resolves to.

| Variant | Fields | Description |
|---------|--------|-------------|
| `Net(NetType)` | net type | Net-capable type (discipline or bundle) |
| `Value(ValueType)` | value type | Value-level type (primitive, enum, etc.) |

Methods: `as_net()` → `Option<&NetType>`, `as_value()` → `Option<&ValueType>`.

---

## Traits

POM capability traits (`traits.rs`) provide orthogonal axes for generic code to work against.

| Trait | Method | Implementors | Description |
|-------|--------|--------------|-------------|
| `Named` | `name() → &str` | Module, Instance, Port, Param, Wire, Behavior, Function | Plain-text name |
| `NetTyped` | `net_type() → &NetType` | Port, Wire | Net type (discipline or bundle) |
| `Kinded` | `kind() → Kind` | Module, Instance, Port, Param, Wire, Behavior | Discriminant kind |

---

## Kind

`Kind` (`node.rs`) is the discriminant for concrete POM node types.

```
Module | Instance | Port | Param | Wire | Behavior | Discipline | Enum | Bundle | Capability
```

---

## Id

`Id` (`node.rs`) is a `u64` stable node identity that survives re-elaboration as long as the source construct is unchanged. Created via `Id::new(u64)`, accessed via `Id::as_u64()`. Implements `Display` as `#<value>`.

---

## Selection\<T\>

`Selection<T>` (`selection.rs`) is the universal result type for relation navigation — an ordered, immutable collection returned by all relation accessors.

### Methods

| Method | Description |
|--------|-------------|
| `new()` | Create an empty selection |
| `from_vec(items)` | Create from a `Vec<T>` |
| `len()` | Number of elements |
| `is_empty()` | Whether empty |
| `first()` | First element, if any |
| `get(i)` | Element at index `i`, if present |
| `iter()` | Iterator over elements |
| `one()` | Exactly-one: returns `T` or `ReflectError` |
| `filter(pred)` | Filter and return a new `Selection<T>` |
| `map(f)` | Map each element, returns `Vec<U>` |

Implements `IntoIterator` and `From<Vec<T>>`.

---

## OverrideMap

`OverrideMap` (`staging.rs`) is the staging layer for parameter overrides. It is the single mutation surface in the POM — everything else is read-only.

Keyed by `(instance_path, param_name)` where the path is a hierarchical dotted name (e.g. `"top.dac"`) and the param name is the declared parameter name (e.g. `"r"`).

### Methods

| Method | Description |
|--------|-------------|
| `set(path, param, value)` | Stage an override, replacing any existing one for the same key |
| `get(path, param)` | Look up a staged override |
| `is_empty()` | Whether no overrides are staged |
| `len()` | Number of staged overrides |
| `clear()` | Remove all staged overrides |
| `iter()` | Iterate all `(path, param, value)` triples |

---

## Error Types

### ElabError

`ElabError` (`error.rs`) represents elaboration failures.

| Variant | Description |
|---------|-------------|
| `ConstEval { context, source }` | Constant evaluation failed |
| `UndefinedType(String)` | Unresolved type name |
| `UndefinedModule(String)` | Unresolved module name |
| `NotNetCapable(String)` | Bundle contains non-net fields |
| `ContribInDigital` | Contribution `<+` in a digital block |
| `ContribInModBody` | Contribution `<+` in a module body |
| `ForceInModBody` | Force `<-` in a module body |
| `UnknownEvent(String)` | Event name not found in registry |
| `AnalogEventInDigital(String)` | Analog crossing event in digital block |
| `DigitalEventInAnalog(String)` | Digital edge event in analog block |
| `MissingConstParam { param, module }` | Required constant parameter not provided |
| `NotANetRef(String)` | Expression cannot be reduced to a net reference |
| `Other(String)` | Catch-all for other elaboration errors |

### ReflectError

`ReflectError` (`error.rs`) represents reflection-layer failures (navigation, staging).

| Variant | Description |
|---------|-------------|
| `NotFound(String)` | Element not found |
| `NotSettable(String)` | Attribute is not settable |
| `TypeMismatch(String)` | Type mismatch on assignment |
| `OutOfRange(String)` | Index out of range |
| `Other(String)` | Catch-all for other reflection errors |
