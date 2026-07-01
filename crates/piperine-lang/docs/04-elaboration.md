# Elaboration Phase

The elaboration phase transforms a parsed `SourceFile` (parse AST) into a fully resolved
`Design` (elaborated IR + POM).

```
SourceFile (parse AST)  ──Elaborator──▶  Design (elaborated IR + POM)
```

## What elaboration does

1. **`use` expansion** — `use` declarations are resolved by the `Resolver` and expanded
   into flat item lists before the elaborator sees them.
2. **Prelude injection** — standard capabilities are prepended so they are always in scope.
3. **Symbol registration** — disciplines, bundles, enums, modules, behaviors, functions,
   capabilities, impls are indexed into symbol tables.
4. **Semantic validation** — rejects illegal constructs per domain.
5. **Type resolution** — every `Type { name: String }` in the AST is resolved to a
   `NetType` or `ValueType`; array dimensions are evaluated to concrete `u64` values.
6. **Structural elaboration** — `StructuralFor` and `StructuralIf` are unrolled/evaluated.
7. **Bundle expansion** — net-capable bundle ports expand to flat `Port`s.
8. **Generic monomorphization** — generic modules are instantiated with concrete parameters.
9. **Behavioral for unrolling** — `for` loops in behavior blocks are fully unrolled.
10. **Event validation** — event names are looked up in the `EventRegistry`.

---

## The `Elaborator` struct

```rust
pub struct Elaborator {
    disciplines: HashMap<String, DisciplineDecl>,
    bundles: HashMap<String, BundleDecl>,
    enums: HashMap<String, EnumDecl>,
    module_decls: HashMap<String, ModDecl>,
    behavior_decls: Vec<BehaviorDecl>,
    fn_decls: HashMap<String, FnDecl>,
    capability_decls: HashMap<String, CapabilityDecl>,
    impl_decls: Vec<ImplDecl>,
    events: EventRegistry,
    mono_cache: HashMap<String, Module>,  // mangled name → elaborated module
}
```

Filed in `src/elab/lower/mod.rs`, methods spread across sibling files by concern:

| File | Concern |
|---|---|
| `mod.rs` | Struct + fields, `new()`, `elaborate()` driver |
| `register.rs` | Top-level symbol registration |
| `resolve.rs` | Type/net-type resolution, net references, port expansion |
| `module.rs` | `mod` body → `Module` |
| `behavior.rs` | `analog`/`digital` body → `Behavior` |
| `mono.rs` | `fn`/`impl` elaboration, generic monomorphization |

---

## The `elaborate()` driver method

```rust
pub fn elaborate(&mut self, source: SourceFile) -> Result<Design, ElabError>
```

Processing steps in order:

### 1. `register_items()` — populate symbol tables

Maps each `Item` variant to the correct symbol table:
- `DisciplineDecl` → `self.disciplines`
- `BundleDecl` → `self.bundles`
- `EnumDecl` → `self.enums`
- `ModDecl` → `self.module_decls`
- `BehaviorDecl` → `self.behavior_decls` (appended, multiple behaviors per module allowed)
- `FnDecl` → `self.fn_decls`
- `CapabilityDecl` → `self.capability_decls`
- `ImplDecl` → `self.impl_decls` (appended)
- `UseDecl` → ignored (already expanded by `Resolver`)

### 2. Validation pass

Borrows `self.events` immutably to validate:
- **Module bodies** — rejects `<+`/`<-` in `mod` body statements (via `validate_mod_body`).
- **Behavior bodies** — validates domain constraints (e.g. no `cross` in digital blocks)
  and registers valid event names (via `validate_behavior`).

### 3. Copy disciplines, enums, capabilities into `Design`

Direct clone from symbol tables into the `Design`'s permanent registries.

### 4. Elaborate `impl` blocks (`elab_impl`)

Processes each `ImplDecl`, producing a resolved impl block added to the program.

### 5. Elaborate free functions (`elab_fn`)

Each top-level `FnDecl` is elaborated and inserted into the program's function map.

### 6. Elaborate non-generic modules (`elab_mod_inner`)

Only modules with **no** const params and **no** type params are elaborated eagerly.
Generic module elaboration is triggered on-demand during `lower_mod_stmt` when an
instance with const args is encountered (monomorphization).

### 7. Attach behaviors to their modules

Each `Behavior` is inserted into its parent `Module`'s `behaviors` vector by matching
the behavior's `name` to the module name.

### 8. Merge monomorphized modules (`mono_cache`)

All on-demand monomorphized modules accumulated in `mono_cache` are drained into the
program's module map.

---

## Module elaboration (`module.rs`)

### `elab_mod_inner()`

The entry point for elaborating a `ModDecl` into a `Module`:

1. **Expand ports** — each port is expanded via `expand_port()`. Net-capable bundle
   ports fan out into one `Port` per field.
2. **Lower mod body** — `lower_mod_stmts()` processes the body into `ModBodyItem`s.
3. **Sort items** — items are sorted into `params`, `wires`, `instances`, and `connections`.

### `lower_mod_stmt()` — handling each `ModStmt` variant

- **`ParamDecl`**: resolves the value type via `resolve_value_type()`, evaluates the
  default expression via `ConstEnv`, produces a `Param`.
- **`WireDecl`**: resolves the net type via `resolve_net_type()`, produces a `Wire`.
- **`VarDecl`**: skipped at structural level (vars in mod body appear in behaviors).
- **`StructuralFor`**: evaluates range bounds via `ConstEnv`, pushes a new scope binding
  the loop variable, unrolls the body, pops the scope.
- **`StructuralIf`**: evaluates the condition, selects the then or else branch, lowers
  only the taken branch.
- **`Instance`**: resolves const args, triggers monomorphization via `monomorphize()`,
  resolves port connections to `NetRef` via `eval_net_ref()`, resolves param overrides,
  produces an `Instance`.
- **`Connection`**: resolves lhs/rhs to `NetRef` via `eval_net_ref()`, produces a `Connection`.

---

## Type resolution (`resolve.rs`)

### `resolve_type()`: `Type` → `TypeRef`

The main type resolution function handles:

1. **Array dimensions**: if the type has dimensions, the inner type (without dimensions)
   is resolved first, then each dimension is evaluated via `ConstEnv::eval_nat()` and
   wrapped in `NetType::Array` or `ValueType::Array`.

2. **Value primitives**: matches the type name against built-in value primitives:
   `Real`, `Natural`, `Integer`, `Complex`, `Boolean`, `Quad`, `String` — producing
   the corresponding `ValueType` variant wrapped in `TypeRef::Value`.

3. **Disciplines**: if the name exists in `self.disciplines`, produces
   `TypeRef::Net(NetType::Discipline(name))`.

4. **Enums**: if the name exists in `self.enums`, produces
   `TypeRef::Value(ValueType::Enum(name))`.

5. **Bundles**: if the name exists in `self.bundles`, `is_net_capable_bundle(name)` is
   checked. Net-capable bundles produce `TypeRef::Net(NetType::Discipline(name))`.
   Non-net-capable bundles produce an error.

6. **Function pointers**: if the name is `fn`, the generic args are resolved as
   parameter and return types, producing `ValueType::FnPtr(params, ret)`.

7. **Type substitution**: type params are substituted via `type_subst` before lookup
   (used during monomorphization).

### `resolve_net_type()` and `resolve_value_type()`

Wrapper functions that call `resolve_type()` and ensure the result is a `NetType` or
`ValueType` respectively, returning errors on mismatch.

### `is_net_capable_bundle()`

A bundle is net-capable if **all** fields resolve to net types (disciplines or other
net-capable bundles). This is checked recursively through `is_net_type_name()`.

---

## Net reference resolution (`eval_net_ref`)

Reduces port-connection or net-connection expressions to concrete `NetRef` values:

- **`name`** → `NetRef::simple(name)` — simple wire/port reference.
- **`name[i]`** → `NetRef::indexed(name, i)` — index evaluated via `ConstEnv`.
- **`base.field`** → `NetRef::simple("{base}_{field}")` — bundle-field naming convention.

---

## Port expansion (`expand_port`)

If a port's type is a net-capable bundle, it expands to one `Port` per bundle field,
named `{port_name}_{field_name}`. Each expanded port carries the original direction.
Non-bundle ports pass through unchanged after type resolution.

---

## Behavior elaboration (`behavior.rs`)

### `elab_behavior()`

Creates a fresh `ConstEnv`, then calls `lower_behavior_stmts()` to process the
AST behavior statements into POM `BehaviorStmt`s.

### `lower_behavior_stmt()` — handling each `AstBehaviorStmt` variant

- **`VarDecl`**: resolves the value type, produces a `BehaviorStmt::VarDecl`.
- **`Bind`**: passes through with cloned dest/op/src.
- **`If`**: constant-folding — if the condition evaluates to `Bool(true)` or `Nat(1)`
  at elaboration time, only the `then_body` is kept. If `Bool(false)` or `Nat(0)`,
  only the `else_body` is kept. Otherwise the full `If` is preserved.
- **`Match`**: arms are lowered recursively; the match is preserved.
- **`For`**: evaluates range bounds, unrolls the loop body, extending the output with
  each unrolled iteration. A new scope is pushed/popped per iteration to bind the
  loop variable.
- **`Event`**: the event block's body (`Block`) contains function-body `Stmt`s, not
  `BehaviorStmt`s. These are lowered via `lower_stmt_to_behavior()`.
- **`Diagnostic`**: passes through with cloned sys/args.
- **`Expr`**: passes through as `BehaviorStmt::Expr`.

### `lower_stmt_to_behavior()`

Converts function-body `Stmt`s to `BehaviorStmt`s (used inside event blocks and
function bodies):

- `Stmt::VarDecl` → `BehaviorStmt::VarDecl` (with type resolution)
- `Stmt::Bind` → `BehaviorStmt::Bind`
- `Stmt::If` → `BehaviorStmt::If` (recursively lowering block contents)
- `Stmt::Match` → `BehaviorStmt::Match` (recursively lowering block contents)
- `Stmt::For` → unrolled into flat `Vec<BehaviorStmt>` (range bounds must be constant)
- `Stmt::Return(expr)` → `BehaviorStmt::Return(expr)` — preserved as a distinct variant
  so the codegen can find the trailing return value of a user `fn` body.
- `Stmt::Expr(e)` → `BehaviorStmt::Expr(e)`

---

## Entry point: `SourceFile::elaborate()`

```rust
impl SourceFile {
    pub fn elaborate(self) -> Result<Design, ElabError>
    pub fn elaborate_with(self, resolver: &mut Resolver) -> Result<Design, ElabError>
}
```

The `elaborate()` method uses a default `Resolver`. The `elaborate_with()` variant
allows supplying a custom `Resolver` (e.g. with a project root for file-based
module resolution). Both methods:

1. Inject the standard-library prelude items.
2. Expand all `use` declarations through the `Resolver`.
3. Construct an augmented `SourceFile` containing prelude + expanded items.
4. Run `Elaborator::elaborate()` on the augmented source.
