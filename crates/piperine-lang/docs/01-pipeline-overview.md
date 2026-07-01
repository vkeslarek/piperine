# Pipeline Overview

The `piperine-lang` pipeline converts PHDL source code into a `CircuitInstance` ready for the solver. Below is the complete flow:

```
&str
 │
 ▼  parse::Lexer                            [src/parse/lexer.rs]
Vec<Lexed>              (token sequence with byte-range spans)
 │
 ▼  parse::Parser                           [src/parse/parser/]
parse::SourceFile       (unresolved AST — types are strings)
 │
 ▼  resolve::Resolver                       [src/resolve.rs]
parse::SourceFile       (with `use` expanded and prelude injected)
 │
 ▼  elab::Elaborator                        [src/elab/lower/]
Design                  (elaborated design + POM root)
 │
 ▼  lowering::ppr_to_ir                     [src/lowering/]
IrProgram               (central IR for piperine-codegen)
 │
 ▼  runtime::from_ir                        [src/runtime/]
CircuitInstance         (ready for piperine-solver)
```

---

## Pipeline Layers

### 1. Parse (Lexer + Parser)
- **Lexer**: `Lexer::tokenize()` → `Vec<Lexed>`. Converts raw text to tokens with byte-range spans.
- **Parser**: `Parser::parse_file()` → `SourceFile`. Hand-written recursive-descent LL(1) parser. Produces a syntactic AST with no name resolution.

### 2. Resolve (Resolver)
- `Resolver::expand()` resolves `use foo::bar;` transitively (including diamond dependencies).
- `Resolver::prelude_items()` injects the stdlib (`piperine::capabilities`, `piperine::collections`, `piperine::prelude`).
- Built-ins are embedded via `include_str!`; external files resolve from the project `root`.

### 3. Elaborate (Elaborator)
- **Register**: Populates symbol tables (modules, disciplines, bundles, enums, capabilities, fns, impls).
- **Validate**: Rejects `<+` in digital blocks, `<-` in mod bodies, domain-mismatched events.
- **Type Resolution**: Converts type strings to `NetType`/`ValueType`, evaluates array dimensions.
- **Structural Elaboration**: Unrolls `StructuralFor`/`StructuralIf`, expands bundle ports.
- **Monomorphization**: Generic modules (`mod Foo[N]`) are specialized on demand.
- **Behavioral Elaboration**: Unrolls `for` loops in behavior blocks, validates events via `EventRegistry`.

### 4. Lowering (ppr_to_ir)
- Converts `Design` (POM) → `IrProgram` (piperine-codegen IR).
- Each `Module` becomes an `IrModule` (ports, params, wires, instances, connections).
- Behaviors (`analog`/`digital`) become `IrAnalogBody`/`IrDigitalBody`.
- PHDL expressions are lowered to `IrExpr`, system calls to `SimQuery`.

### 5. Runtime (from_ir)
- `from_ir()` builds a `CircuitInstance` from an `IrProgram`.
- For each child instance of the top module:
  - Compiles the analog body via `ir_analog_to_device()` (Cranelift JIT).
  - Compiles the digital body via `ir_digital_to_interp()` (tree-walking interpreter).
  - Creates a `PhdlDevice` that wraps both.
- Allocates analog nodes (`NodeIdentifier`) and digital nets (`DigitalNet`).
- Assembles the `Netlist` and returns the `CircuitInstance`.

---

## Crate Modules

| Module | Purpose |
|--------|---------|
| `parse` | Lexer, parser, parse-AST types |
| `elab` | Elaborator, event registry, const evaluator |
| `pom` | Piperine Object Model — Design, Module, Port, Param, Value, etc. |
| `lowering` | Design → IrProgram (`ppr_to_ir`) |
| `runtime` | IrProgram → Device/CircuitInstance (`from_ir`, `PhdlDevice`, `DigitalInterpreter`) |
| `resolve` | `use` declaration resolver |

---

## Conventions

- **Never use `unwrap()`/`expect()`** on user input paths. Always return `Result<String, ...>`.
- **Dependency direction**: `piperine-solver` does NOT depend on `piperine-codegen`. The codegen depends on the solver.
- **Analog values**: `f64`. **Digital values**: `LogicValue`. **Mixed nets**: anonymous `usize` indices.
