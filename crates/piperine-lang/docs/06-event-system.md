# Event System

The event system in `src/elab/event.rs` provides a semantic validation layer for PHDL event specifications (`@ posedge(clk)`, `@ cross(v, 0)`, etc.). It separates parse-time event syntax from elaborate-time domain validation, allowing new event kinds to be registered without changing the parser or AST.

---

## EventKind Trait

All event kinds implement the `EventKind` trait:

```rust
pub trait EventKind: Send + Sync {
    fn name(&self) -> &str;
    fn is_digital_edge(&self) -> bool { false }
    fn is_analog_crossing(&self) -> bool { false }
    fn is_level(&self) -> bool { false }
    fn validate_arg(&self, _arg: &Expr) -> Result<(), String> { Ok(()) }
}
```

| Method | Default | Purpose |
|--------|---------|---------|
| `name()` | *(required)* | Returns the event's keyword string (e.g. `"posedge"`) |
| `is_digital_edge()` | `false` | Whether the event is edge-triggered (posedge/negedge/change) |
| `is_analog_crossing()` | `false` | Whether the event is an analog threshold crossing (cross/above) |
| `is_level()` | `false` | Whether the event is level-sensitive |
| `validate_arg()` | `Ok(())` | Optional argument validation hook |

---

## Built-in Event Kinds

Five built-in event kinds are provided:

| Struct | Keyword | Domain | Description |
|--------|---------|--------|-------------|
| `RisingEdge` | `"posedge"` | Digital edge | Fires on a rising digital transition (0→1) |
| `FallingEdge` | `"negedge"` | Digital edge | Fires on a falling digital transition (1→0) |
| `AnyChange` | `"change"` | Digital edge | Fires on any digital value change |
| `AnalogCross` | `"cross"` | Analog crossing | Fires when an analog expression crosses a threshold |
| `AnalogAbove` | `"above"` | Analog crossing | Fires when an analog expression stays above a threshold |

---

## EventRegistry

The `EventRegistry` stores a `HashMap<String, Box<dyn EventKind>>` keyed by each event's name. It is the central lookup table used by the elaborator.

### Construction

```rust
let registry = EventRegistry::with_builtins();
```

`with_builtins()` creates a registry pre-populated with all five built-in event kinds (RisingEdge, FallingEdge, AnyChange, AnalogCross, AnalogAbove).

### Registration

```rust
registry.register(MyCustomEvent);
```

`register()` accepts any type implementing `EventKind + 'static`. This makes the event model extensible: new event kinds can be added at any time without modifying the parser or AST.

### Lookup

```rust
registry.lookup("posedge")  // → Option<&dyn EventKind>
```

`lookup()` finds an event kind by name. Returns `None` if the name is not registered.

---

## Validation Methods

### `validate_mod_body(&self, stmts: &[ModStmt]) -> Result<(), ElabError>`

Walks module-body statements. Recurses into `StructuralFor` and `StructuralIf` bodies. Declarations (`ParamDecl`, `WireDecl`, `VarDecl`, `Instance`, `Connection`) pass through. Does not itself check for contributions/forces in mod bodies — that is done elsewhere during elaboration.

### `validate_behavior(&self, kind: BehaviorKind, stmts: &[BehaviorStmt]) -> Result<(), ElabError>`

Walks the top-level statements of an `analog` or `digital` behavior block, delegating to `validate_behavior_stmt()` for each statement.

### `validate_behavior_stmt(&self, kind: BehaviorKind, stmt: &BehaviorStmt)`

Key validation rules:

- **`Contrib` in `Digital`**: A `Bind { op: Contrib, .. }` in a digital block raises `ElabError::ContribInDigital`.
- **Recursive validation**: `If`, `Match`, and `For` blocks recursively validate their nested bodies via `validate_behavior()`.
- **`Event` blocks**: Validates the event spec via `validate_event_spec()` and the body statements via `validate_stmt_in_behavior()`.
- `VarDecl`, `Diagnostic`, and `Expr` statements require no validation.

### `validate_event_spec(&self, kind: BehaviorKind, spec: &EventSpec)`

Domain validation for event specifications:

- Analog crossing events (`cross`, `above`) inside a **digital** block → `ElabError::AnalogEventInDigital`
- Digital edge events (`posedge`, `negedge`, `change`) inside an **analog** block → `ElabError::DigitalEventInAnalog`
- Unknown event names (not in the registry) → `ElabError::UnknownEvent`
- `Or` event specs recurse into each sub-spec.
- `Initial` and `Final` pass unconditionally.

### `validate_stmt_in_behavior(&self, kind: BehaviorKind, stmt: &Stmt)`

Validates function-body-level `Stmt` nodes occurring inside event blocks. Applies the same `ContribInDigital` check and recurses into `If`, `Match`, and `For` control flow.

---

## Extensible Event Model

Events in the parser are represented as `EventSpec::Named { name, arg }`, where `name` is any identifier — the parser accepts arbitrary event names without knowing their semantics. Resolution happens later, during elaboration:

1. The elaborator calls `EventRegistry::lookup(name)`.
2. If the name is registered, the corresponding `EventKind` is returned.
3. Domain validation (`is_digital_edge` / `is_analog_crossing`) determines whether the event is allowed in the current block kind.

This separation of parse-time syntax from elaborate-time semantics means new event kinds can be added at any time by calling `EventRegistry::register()`. No parser changes, no AST changes, no grammar changes are required.
