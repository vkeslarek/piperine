# Error Catalog (`piperine-lang`)

This document lists all errors that can be emitted by the Piperine language frontend (Parser, Elaborator, and Reflection/POM).

## Parse Errors (`ParseError`)
Emitted during the conversion of source text to the Abstract Syntax Tree (AST).

* **UnexpectedEof**: The parser reached the end of the file prematurely while expecting more tokens (e.g., an unclosed block).
* **UnexpectedTok**: The parser encountered a token that makes no grammatical sense in the current context. The message includes the expected token.
* **Generic**: A generic parsing error with a custom text message and a span pointing to the location.
* **Legacy**: An older error format that hasn't been mapped to rich `miette` diagnostics (lacks span information).

## Elaboration Errors (`ElabError` / `ElabErrorKind`)
Emitted during semantic verification and expansion (AST → POM).

* **ConstEval**: Failed to evaluate a constant expression. This occurs when array dimensions or `param` values depend on expressions that cannot be resolved at compile-time.
* **UndefinedType**: The code references a type (discipline, bundle, enum) that has not been declared anywhere in the project or prelude.
* **UndefinedModule**: Attempted to instantiate a `mod` (module) that does not exist.
* **NotNetCapable**: A bundle was instantiated as a net, but it contains fields that are not disciplines or net-capable types.
* **ContribInDigital**: An analog contribution assignment (`<+`) was used incorrectly inside a `digital { ... }` block.
* **ContribInModBody**: A contribution assignment (`<+`) was used directly in the module body instead of inside an `analog { ... }` block.
* **ForceInModBody**: A digital force assignment (`<-`) was used directly in the module body.
* **UnknownEvent**: A named event was not recognized by the system (e.g., incorrect usage of `@(cross(...))`).
* **AnalogEventInDigital**: Attempted to listen to a purely analog event (like `cross` or `timer`) inside a digital block, violating mixed-signal rules.
* **DigitalEventInAnalog**: Attempted to listen to a digital event (like a clock edge) inside a block of analog differential equations.
* **MissingConstParam**: A module was instantiated without providing a required constant `param` that has no default value.
* **NotANetRef**: An expression expected a direct reference to a net, but found something else (like a literal or an incompatible type).
* **WidthMismatch**: Bit width mismatch when connecting two nets. Example: connecting a `Bit[8]` to a pin expecting `Bit[4]`.
* **DisciplineCrossing**: Two nets of different natures (e.g., `Electrical` and `Thermal`) were connected directly. This requires an explicit converter module.
* **UnknownBundle**: A parameter or field attempts to use a `bundle` that has not been declared.
* **BundleFieldUnknown**: A bundle literal passed to a parameter mentions a field that does not exist in the original bundle declaration.
* **BundleParamDefault**: The default value provided for a bundle-typed parameter is not a literal of the same bundle type.
* **BundleFieldNoDefault**: A field within a bundle-typed parameter received no value, and the original bundle had no fallback default for that field.
* **BundleParamNameCollision**: A naming conflict where an explicitly named scalar parameter collides with the compiler's flattening convention for bundle fields (e.g., `param_field`).
* **MultipleDrivers**: Detected multiple drivers driving the same analog net simultaneously, but the net's discipline has no resolution policy (resolve clause) defined.
* **Other**: A generic elaboration error describing the issue via free text.

## Reflection and POM Errors (`ReflectError`)
Emitted when inspecting or mutating the POM via the Reflection or Staging API.

* **NotFound**: The requested node, attribute, or path does not exist in the elaborated POM tree.
* **NotSettable**: The attribute was found, but it is read-only and cannot be written through the staging layer.
* **TypeMismatch**: The value type provided by the API does not match the expected type of the target attribute.
* **OutOfRange**: The value or index provided by the API is outside the permitted bounds for that parameter.
* **MultipleDrivers**: Multiple modifiers attempting to drive the same value via the staging API without a resolution policy.
* **Other**: A generic reflection error.
