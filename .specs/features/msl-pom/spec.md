# MSL POM (Piperine Object Model) Specification

**Approved**: 2026-11-13 - user: "Coloca o Fable para planejar essa feature, depois roda ela usando Sonnet impl) e Opus (verifier)"

## Problem Statement

`piperine-lang` parses MSL into a syntax tree and stops there. The old object model was Verilog-AMS-shaped and was deleted with that frontend, so nothing answers "what is declared in module `abc`", "which module does this instance place", or "what does this name refer to". The grammar itself leaves three classifications to name resolution (B.3.3: instance vs call, template vs struct literal vs bundle literal, carrier port vs value input), so the syntax tree alone cannot even say what a module contains. Every later stage (type checking, elaboration, codegen, the LSP, the host API) needs one faithful, navigable model of the authored code.

## Goals

- [ ] One immutable `Design` built from MSL sources represents every construct of the language, declarations and bodies, exactly where it was written: a signal written in module `abc` is found under `abc` and nowhere else.
- [ ] Every name that the source structure can resolve is linked to its declaration; what cannot be resolved without type inference is marked deferred, never guessed.
- [ ] A reflection API in the style of Java reflection: typed handles with lookups by name, navigation up (`declared_in`) and down (`signals`, `instances`, `reactions`), and reverse queries (`writers`, `references`, `instantiated_by`).
- [ ] The model is lossless: converting it back yields the parsed syntax tree for every code block of `docs/spec/`.
- [ ] A source tree on disk loads with Rust's `mod` rules, and a `Design` round-trips through `serde`.
- [ ] `docs/pom/` documents every public type and method and maps every grammar production to its POM form, and a test keeps it complete.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Type checking and type inference (expression types, generic argument inference, trait solving) | next feature; this one resolves through *declared* types only and marks the rest `Deferred` |
| Elaboration proper: const evaluation, generation unrolling, monomorphization, param binding and `where` checks, instance copies, hierarchical paths (§21), the flat netlist | the POM mirrors the authored code (project rule "POM navigability mirrors the source"); elaborated forms are later, separate side artifacts |
| Every elaboration-phase error except the eight binding errors listed under POM-08/POM-09/POM-10 (`E0161`, `E0330`, `E0401`, `E0402`, `E0403`, `E0457`, `E0130`, `E0343`) | they need types, const values or the elaborated netlist |
| Duplicate item names in one namespace (Rust `E0428`) | the spec gives the condition no code; lookup takes the first declaration in source order |
| A standard-library source tree (§22 as `.msl` files) | no stdlib source exists yet; this feature ships the mechanism (prelude sources) and the closed table of core names |
| `Piperine.toml`, dependencies, package and prelude discovery, `#[path]` on a `mod` | belongs to `piperine-project`; this feature loads one tree from a root file with Rust's `mod` rules. `#[path]` is not an attribute of the language (B.1 lists `derivative`, `derive`, `link`) |
| Reading the file named by `include_str!` | elaboration; the POM records the call |
| Python / host-API / LSP / CLI exposure of the POM | other crates do not build today; separate features |
| Fixing the other workspace crates | user: out of scope |
| Changes to the parser, the AST or the printer, beyond `serde` derive lines on the leaf types the POM reuses (`Span`, `Loc`, `Lit`, `LitKind`, `UnOp`, `BinOp`, `Dir`, `Quantity`, `BehaviorKind`) and `Diagnostic.code` becoming `Cow<'static, str>` | the POM consumes them as they are; those two edits are what `Deserialize` needs |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Scope of "elaboration" in this feature (Q1) | build the model for every construct + name resolution + the classifications B.3.3 leaves to name resolution + the eight binding errors; no types, no const evaluation, no unrolling | user (2026-11-13): "Paths + membros" | y |
| Member resolution (`inst.name`, `bundle.field`, `clk.rise()`) (Q1) | in scope (POM-10), through declared types only; a receiver whose type needs inference is `Deferred` with no diagnostic | user (2026-11-13): "Paths + membros" | y |
| Names of the standard library (§22: `Bit`, `Electrical`, `idt`, ...) (Q2) | resolved only when a prelude source that declares them is registered; otherwise `Unresolved` with `E0161`. Core primitives (§14), primitive types and prelude constants (§1) come from a built-in table | user (2026-11-13): "Mecanismo de prelude + tabela de builtins" | y |
| Several files in one `Design` (Q3) | Rust's rules on disk: `mod b;` loads `b.msl` or `b/mod.msl` relative to the declaring file, as rustc does; a loader takes a root file path. The same rules run over an in-memory file set for tests and tools | user (2026-11-13): "Regras do Rust no disco" | y |
| `serde` on the `Design` (Q4) | `Serialize` + `Deserialize` derived now on the `Design` and every POM type; a gallery design round-trips through `serde_json` | user (2026-11-13): "Derive já" | y |
| File extension of MSL sources | `.msl` | `docs/spec/` names no extension (§3.4 and Appendix A are silent); `.msl` is the language's short name and collides with nothing in the tree | y - planner default, the spec is silent; say so if another is wanted |
| `#[path = "..."]` on `mod` | not supported: it has no meaning and the two default candidates are tried | the spec lists the attributes of the language (B.1) and `path` is not one | y - settled by the spec |
| Error code of a `mod` that cannot be loaded (missing file, two candidates, cycle, unreadable file) | `E0161` with a message that names the candidate paths | §3.4: "a path, `use` or `mod` that does not resolve"; the spec has no finer code | y - settled by the spec |
| What the serialized form holds | everything except the retained syntax tree, which is rebuilt by parsing the retained text on deserialization | parsing is deterministic; keeps the nested AST out of the format (`serde_json` limits nesting to 128) | y - design decision |
| Where the POM lives | `piperine_lang::pom`, beside `piperine_lang::msl` | the orchestrator's brief; the crate is the MSL frontend | y - settled by the brief |
| Data model | arenas with typed ids inside one owning `Design`; public handles are `Copy` views `(design, id)` | reflection ergonomics, immutability, serde-readiness; detailed in design.md | y - design decision |
| Is the `Design` ever refused? | never: `build()` always returns a `Design` plus diagnostics, like `parse` | a tool (LSP, docs) must inspect broken code | y - follows the parser's contract |
| Vocabulary: "instance" | the handle is named `Instance` and stands for a *placement* (§8.3: one syntactic occurrence). Elaborated copies are not in the POM | the user's examples say `module.instances()`; the POM is source-level | y - documented in docs/pom |
| Language of `docs/pom/` | English, like `docs/spec/` | repo convention | y - settled by the repo |
| MSL fences inside `docs/pom/` | info string `msl` (API examples are `rust` and run as doctests) | `rust` fences in a file included as rustdoc are compiled as Rust | y - design decision |
| Pace | one task per commit; Sonnet workers implement, an Opus Verifier judges | the orchestrator's brief | y - settled by the brief |

**Open questions:** none - Q1 to Q4 were answered by the user on 2026-11-13 and are recorded above.

**Implicit-requirement dimensions.** Input validation & bounds: POM-01 AC 2, POM-14 (malformed and deeply nested sources). Failure / partial failure: POM-01 AC 2, 6, 7, 9, 10, POM-08 AC 9, POM-14 AC 2 (a `Design` is always produced). Idempotency: POM-01 AC 3 (deterministic ids). Concurrency / ordering: POM-01 AC 4 (`Send + Sync`), POM-08 AC 15 (diagnostic order). State-transition integrity: POM-12 (resolution never restructures). Auth boundaries & rate limits: N/A because the POM is an in-process library with no caller identity. Data lifecycle / expiry: N/A because a `Design` is an immutable value dropped by its owner. Observability: N/A because the only output channel is `diagnostics()`, covered above. External-dependency failure: the filesystem is the one dependency: POM-01 AC 6, 7, 9, 10 (missing, ambiguous, cyclic, unreadable files). Data lifecycle of the serialized form: POM-15 (no version field: the format is the crate version's own; a reader of another version is out of scope).

---

## User Stories

### P1: Build a design from sources ⭐ MVP

**User Story**: As a tool author, I want one call that turns MSL sources into an immutable `Design` so that every later stage starts from the same model.

**Why P1**: nothing else exists without it.

**Acceptance Criteria** (POM-01):

1. WHEN `Design::builder().source("top.msl", text).build()` is called THEN the system SHALL return a `Design` whose `files()` holds one `SourceFile` with `name() == "top.msl"`, `text() == text` and `ast()` equal to `msl::parser::parse(text).file`.
2. IF a source has parse diagnostics THEN the system SHALL still return a `Design` and SHALL list each parse diagnostic in `diagnostics()` with its code, its span and the id of its file.
3. WHEN the same sources are built twice THEN the system SHALL produce two designs whose `dump()` texts are byte-identical (ids, order and resolutions included).
4. The system SHALL make `Design` implement `Send` and `Sync`.
5. WHEN the root file `top.msl` holds `mod b;` and `b.msl` exists beside it THEN the system SHALL make `design.find("b::M")` return the module `M` declared in `b.msl`, with `M.file().path()` that file; `b/mod.msl` in place of `b.msl` SHALL give the same result.
6. IF `mod b;` is written and neither `b.msl` nor `b/mod.msl` exists THEN the system SHALL keep an empty namespace `b` in the tree and report `E0161` at the `mod` item with a message naming both candidate paths.
7. IF both `b.msl` and `b/mod.msl` exist THEN the system SHALL load neither, keep an empty namespace `b` and report `E0161` at the `mod` item with a message naming both paths.
8. WHEN `mod c;` is written in `b.msl` THEN the system SHALL load `b/c.msl` or `b/c/mod.msl`; written in `b/mod.msl` it SHALL load `b/c.msl` or `b/c/mod.msl`; written inside an inline `mod a { }` of the root it SHALL load `a/c.msl` or `a/c/mod.msl`.
9. IF a `mod` item would load a file that is already being loaded on the current chain (the same canonical path) THEN the system SHALL not load it again and SHALL report `E0161` at the `mod` item with a message saying the modules form a cycle.
10. WHEN `Design::load(path)` reads a tree from disk THEN the system SHALL produce a design whose `dump()` equals that of the in-memory build of the same files; IF the root file cannot be read THEN it SHALL return a `Design` with an empty root namespace and one `E0161` diagnostic naming the path.

**Independent Test**: build the `Resistor` example of §8 and print `design.dump()`.

---

### P1: The declaration tree mirrors the source ⭐ MVP

**User Story**: As a model author, I want to read back my modules, signals, natures and functions exactly where I wrote them so that the model never surprises me.

**Why P1**: this is the request: "if a wire is in module abc, the structure must reflect exactly that".

**Acceptance Criteria** (POM-02, the tree):

1. WHEN a file declares N items THEN the system SHALL make `design.root().children()` hold N declarations in source order, each with the kind of its item, its name, a `span()` equal to the item's AST span and a `declared_in()` equal to the root namespace.
2. WHEN module `abc` declares `signal w: Bit;` in its body THEN the system SHALL make `design.module("abc").signal("w").declared_in()` equal the module `abc`, and SHALL list `w` in the `signals()` of no other module.
3. WHEN a module body holds `for k in 0..4 { signal s: Bit; }` THEN the system SHALL hold exactly one `Signal` declaration named `s`, whose `declared_in()` is a `Generation` declaration whose `declared_in()` is the module, and whose `module()` is the module.
4. WHEN a generation is `if a { X } else if b { Y } else { Z }` THEN the system SHALL expose the three arms in order, each with its condition (none for the last) and its own items.
5. WHEN a file holds `mod a { module M() {} }` THEN the system SHALL make `design.find("a::M")` return `M` and `M.declared_in()` the namespace `a`.
6. WHEN an item is declared inside a function body THEN the system SHALL make that item's `declared_in()` the function.

**Acceptance Criteria** (POM-03, modules):

1. WHEN a module declares ports THEN the system SHALL list them in `ports()` in source order, each with its name, its direction (`In`, `Out` or none), its type, its default expression and its 0-based `index()`.
2. WHEN a module body declares `r: f64 where r > 0.0;` and `tc1: f64 = 0.0;` THEN the system SHALL expose two params with their types, `default()` (none, then `0.0`), `where_clause()` (`r > 0.0`, then none) and `is_required()` (`true`, then `false`); a param of type `Option<T>` with no default SHALL report `is_required() == false`.
3. WHEN a module body declares `pub signal sn: Electrical = 0.0;` THEN the system SHALL expose a signal with `is_pub() == true`, its type and its initial-value expression.
4. WHEN a module body holds `let (r, f) = (a, b);` THEN the system SHALL expose two named expressions `r` and `f` that share one `Let` item whose `value()` is the tuple expression.
5. WHEN a module body holds `q = !(r | qn) after tpd;` THEN the system SHALL expose one definition with its `place()`, `value()` and `after()` expressions.
6. WHEN a module is declared `#[link("bsim4.osdi", model = "bsim4")] extern "osdi" module ...` THEN the system SHALL report `is_extern() == true` and `abi() == "osdi"`.
7. WHEN a module is declared with `-> T` and a trailing expression THEN the system SHALL expose `result_type()` and `result()`; without them both SHALL be none.
8. WHEN a module declares `<T: Repr, const A: u32>` and `where A >= 1` THEN the system SHALL expose two generic params in order (a type param with bound `Repr`, a const param of type `u32`) and the `where_clause()` expression.

**Acceptance Criteria** (POM-04, natures and value items):

1. WHEN a nature declares `potential: Voltage { access: v, units: "V", abstol: 1u }` and a `flow` THEN the system SHALL expose two quantities with their role, name and the three attributes, and `kind()` SHALL be `Conservative`; `{ potential }` alone SHALL be `SignalFlow`, `{ value }` `Discrete`, `{ value, resolve }` `Resolved`, and any other combination `Invalid`.
2. WHEN a struct declares `out valid: Bit` and an enum declares `Data(u8) = 3` THEN the system SHALL expose each field with its name, direction, visibility and type, and each variant with its name, fields and discriminant.
3. WHEN a function is declared `#[derivative(f_dx)] extern "C" fn f(x: f64) -> f64;` THEN the system SHALL expose `abi() == "C"`, one param `x`, the return type and `body()` none; a method SHALL expose its `self` param as the first param.
4. WHEN a file holds `impl<T> Stream<T> { ... }`, `impl Tr for S { ... }` and `trait Tr: Base { ... }` THEN the system SHALL expose for each `impl` its self type, its trait (none for the first) and its items, and for the trait its supertraits and items.
5. WHEN a file holds `use a::{b, c as d, e::*};` THEN the system SHALL expose three imports: path `a::b`, path `a::c` with alias `d`, and a glob of `a::e`.
6. WHEN a file holds `const N: u32 = 8;` THEN the system SHALL expose a constant with its type and value expression.

**Acceptance Criteria** (POM-05, behavior):

1. WHEN a file holds `analog for M { ... }` THEN the system SHALL hold a `Behavior` declaration of kind `Analog` whose `declared_in()` is the file's namespace and whose `module()` is `M`, and `M.behaviors()` SHALL list the module's blocks in source order.
2. WHEN an `analog` block holds a contribution nested under an `if` THEN the system SHALL return it from `contributions()` with its `lhs()` and `rhs()` expressions.
3. WHEN a `digital` block holds reactions THEN the system SHALL list them in `reactions()` in source order, each with its `event()` expression, its `body()` and, from `commits()`, every commit at any depth with its `place()`, `value()` and `after()`.
4. WHEN a reaction is written inside a generation `for` of a `digital` block THEN the system SHALL make its `declared_in()` that `Generation`, whose `declared_in()` is the block.
5. WHEN a block is written `analog<N> for Short<N> { ... }` THEN the system SHALL expose `names() == ["N"]`.

**Acceptance Criteria** (POM-06, bodies):

1. WHEN any `rust` or file-parsable `rust,fragment` fence of `docs/spec/*.md` is built THEN the system SHALL make `file.to_ast()` equal (structurally) to the parsed `ast::File`.
2. WHEN an expression, statement, pattern or type node is read THEN the system SHALL return a `span()` whose source text (`text()`) is exactly what the author wrote for that node, and an `enclosing()` equal to the nearest declaration that contains it.
3. WHEN a body uses each expression form of the language (literal, path, unary, binary, assign, cast, call, method call, field, index, range, paren, tuple, array, repeat, struct, block, `if`, `let` condition, `match`, `while`, `loop`, `for`, `break`, `continue`, `return`, closure, `?`, `emit!`, `include_str!`) THEN the system SHALL expose one node of the matching `ExprKind` for each.

**Acceptance Criteria** (POM-07, docs and attributes):

1. WHEN `///` or `/** */` comments precede a module, port, param, signal, field or variant THEN the system SHALL return their text from `docs()`, one entry per comment in order; `//!` comments at the top of a file SHALL be the `docs()` of that file's namespace.
2. WHEN an item carries `#[link("bsim4.osdi", model = "bsim4")]` THEN the system SHALL return from `attr("link")` an attribute with its positional argument `"bsim4.osdi"` and its named argument `model = "bsim4"`.

**Independent Test**: build the `Front` example of §0.7 and walk module → signals → `declared_in()`; walk the `digital` block → reactions → commits.

---

### P1: Names are linked to their declarations ⭐ MVP

**User Story**: As a tool author, I want every reference to carry the declaration it names so that I can navigate from use to definition without re-implementing scoping.

**Why P1**: the grammar's own classifications (B.3.3) depend on it; without it `module.instances()` cannot exist.

**Acceptance Criteria** (POM-08, path resolution):

1. WHEN an `analog` or `digital` block of module `M` reads a name that is a port, param, signal, body `let` or generic of `M` THEN the system SHALL resolve that path to that declaration.
2. WHEN a body item reads a name declared later in the same body THEN the system SHALL resolve it to that declaration.
3. WHEN a block holds `let x = 1; let x = x + 1;` THEN the system SHALL resolve the `x` on the right side to the first local.
4. WHEN a name is bound by a closure parameter, a `match` arm pattern, a `for` pattern, an `if let` pattern or a function parameter THEN the system SHALL resolve reads inside that binder's scope to the binding, and reads outside it SHALL NOT resolve to it.
5. WHEN a type path names a nature, struct, enum, trait, module or generic param visible in the file THEN the system SHALL resolve it to that declaration.
6. WHEN a path names an entry of the built-in table (primitive types such as `f64`, `u8`, `Option`; core primitives of §14 such as `ddt`, `temperature`; prelude constants of §1 such as `K_B`) and no declaration shadows it THEN the system SHALL resolve it to `Builtin` naming that entry and its spec section.
7. WHEN a name is imported by `use` (simple, aliased, grouped or glob) or written with `self::`, `super::` or `crate::` THEN the system SHALL resolve it to the item the import or path names.
8. WHEN a source is registered as a prelude source THEN the system SHALL resolve its `pub` items from every other source without a `use`.
9. IF a path names nothing THEN the system SHALL mark it `Unresolved`, report exactly one `E0161` at the path's span, and complete the build.
10. IF a path names an item that is not `pub` from outside the namespace that declares it THEN the system SHALL link the path to the item and report `E0161` whose message says the item is private.
11. WHEN a path is `Type::name` THEN the system SHALL resolve it to the enum variant, or to the associated constant or function of an `impl` of that type, with that name.
12. WHEN `Self` is written inside an `impl` or a trait THEN the system SHALL resolve it to the declaration of the `impl`'s self type, or to the trait.
13. IF an item at namespace level has the name of a core primitive of §14 THEN the system SHALL report `E0457` at the item's name.
14. IF two of the ports, params, signals and body `let` names of one module are equal THEN the system SHALL report `E0330` at the second, with a secondary label at the first.
15. The system SHALL order `diagnostics()` by file, then by start offset, then by code.

**Acceptance Criteria** (POM-09, classification):

1. WHEN a port's type resolves to a nature or an array of natures THEN the system SHALL report `kind() == Carrier`; to a bundle, `Bundle`; to a built-in value type or a value struct or enum, `ValueInput`; an unresolved type SHALL give `Unknown`.
2. WHEN a struct has at least one field whose type resolves to a nature, an array of natures or a bundle THEN the system SHALL report `is_bundle() == true`.
3. WHEN a module body holds `let r1 = Resistor(a, b) { r: 1k };` and `Resistor` is a module THEN the system SHALL list in `instances()` an `Instance` with `name() == "r1"`, `module()` equal to `Resistor`, two connections and one param binding `r`.
4. WHEN an `analog` block calls `idt(v(inp), 0.0)` where `idt` resolves to a module, and `windup(u)` where `windup` resolves to a function THEN the system SHALL list the first as an anonymous `Instance` whose `declared_in()` is the block, and SHALL NOT list the second.
5. WHEN `Name { ... }` is written THEN the system SHALL classify it `Template` when `Name` resolves to a module, `BundleLiteral` when it resolves to a bundle, and `StructLiteral` when it resolves to any other struct or enum variant.
6. WHEN a call's callee is a param, value input or constant whose declared type resolves to a module or to a generic param bounded by `Fn(...)`, or is itself a call of a function whose declared return type resolves to a module THEN the system SHALL classify the call as an `Instance` with `via_template() == true`, and with `module()` that module when the type names one.
7. IF a behavior block's path names no module of its file THEN the system SHALL report `E0401` at the path and keep the block in the tree with `module()` none.
8. IF a behavior block targets an `extern` module THEN the system SHALL report `E0402` at the path.
9. IF a behavior block's `names` differ in count or spelling from the module's generic params, or the path's generic arguments are not exactly those names THEN the system SHALL report `E0403` at the block head.
10. WHEN a call `v(p, n)` is written and `p` is a port or signal whose nature declares a quantity with `access: v` THEN the system SHALL resolve `v` to that quantity; when the nature of the first argument cannot be read from a declaration and more than one nature declares `v` THEN the system SHALL mark the callee `Deferred`.
11. IF the self type of an `impl` resolves to a module THEN the system SHALL report `E0130` at the self type.

**Independent Test**: build the `SarAdc` gallery of Appendix F.1 with the fixture prelude and list `instances()` with their modules.

---

### P1: Reflection queries ⭐ MVP

**User Story**: As a tool author, I want Java-reflection-style handles so that I can ask the model questions without touching arenas or ids.

**Why P1**: it is the public face the request describes.

**Acceptance Criteria** (POM-11):

1. WHEN `design.module(name)`, `design.nature(name)` or `design.find(path)` names a declaration THEN the system SHALL return its handle; a name that matches nothing SHALL return none.
2. WHEN `qualified_name()` is read THEN the system SHALL return the namespace path joined by `::` followed by the member chain joined by `.` (for signal `w` of module `M` in `mod a`: `a::M.w`); a `Generation` adds no segment.
3. WHEN a generic declaration handle is cast with `as_module()`, `as_signal()` and the other casts THEN the system SHALL return the typed handle only when the kind matches.
4. WHEN `signal.nature()` or `port.nature()` is read and the type resolves, through arrays and `flip`, to a nature THEN the system SHALL return that nature's handle.
5. WHEN `instance.connection(port)` is read for an instance whose module is known THEN the system SHALL return the argument expression at that port's position.
6. The system SHALL make two handles equal exactly when they come from the same `Design` and name the same node.

**Independent Test**: on the `Front` example, `design.module("Front").signal("d").declared_in().name() == "Front"`.

---

### P1: Authored structure is never rewritten ⭐ MVP

**User Story**: As the project owner, I want resolution to only add links so that the unbreakable rule "POM navigability mirrors the source" holds mechanically.

**Why P1**: project rule; the old POM broke it once.

**Acceptance Criteria** (POM-12):

1. WHEN a module body holds `for k in 0..1000 { signal s: Bit; let u = Cell(s); }` THEN the system SHALL hold one signal declaration and one instance for that loop body.
2. WHEN a module places itself under a generation `if` THEN the system SHALL complete the build with one instance for that placement, whose `module()` is the enclosing module.
3. WHEN a source is built with and without its references resolvable (the same text with and without its prelude source) THEN the system SHALL produce the same number of declarations, statements, expressions, patterns and types for that file, in the same order.

---

### P1: Every source builds ⭐ MVP

**User Story**: As a tool author, I want the build to survive any input so that an editor can show the model of broken code.

**Why P1**: a panic in the frontend is a crash of every tool above it.

**Acceptance Criteria** (POM-14):

1. WHEN any ```` ```rust ````, ```` ```rust,fragment ```` or ```` ```rust,compile_fail ```` fence of `docs/spec/*.md` is built THEN the system SHALL return a `Design` whose `check()` reports no broken invariant (every parent and child link consistent, every id in range, every resolved link pointing at a declaration).
2. WHEN the AST holds an error node (item, module item, digital item, function parameter or expression) THEN the system SHALL keep a node of kind `Error` at that position with its span.
3. WHEN an expression is nested 256 levels deep (the parser's limit) THEN the system SHALL complete the build on a default 2 MiB test thread.

---

### P2: Members resolve through declared types

**User Story**: As a tool author, I want `inst.name`, `bundle.field` and `carrier.method()` linked when the declarations alone decide them so that the common navigation works before a type checker exists.

**Why P2**: valuable, but the model stands without it; full coverage needs type inference.

**Acceptance Criteria** (POM-10):

1. WHEN `r1.x` is written, `r1` names an instance of module `M` and `x` is a `pub` item of `M` THEN the system SHALL resolve the member to that item.
2. IF `r1.x` names an item of `M` that is not `pub` THEN the system SHALL resolve the member to the item and report `E0343` at the member name.
3. WHEN `tx.valid` is written and `tx` is a port or signal whose type resolves to a bundle with a field `valid` THEN the system SHALL resolve the member to that field.
4. WHEN `clk.rise()` is written, `clk` is a carrier of nature `Bit` and an `impl Bit` declares `fn rise` THEN the system SHALL resolve the method to that function; `s.changed()`, `s.prev()`, `s.others()`, `p.inflow()` and `x.level(k)` SHALL resolve to their `Builtin` entries.
5. WHEN the receiver's type cannot be read from a declaration THEN the system SHALL mark the member `Deferred` and report no diagnostic.

**Independent Test**: on F.3's `bank.cells`, the member `cells` resolves to the `pub let` of `MemoryBank`.

---

### P2: Reverse queries

**User Story**: As a tool author, I want to go from a declaration to its uses so that "find references" and "who drives this" are one call.

**Why P2**: built on resolution; not needed to read the structure.

**Acceptance Criteria** (POM-11, continued):

7. WHEN `references()` is read on a declaration THEN the system SHALL return every path and member node resolved to it, in source order.
8. WHEN `writers()` is read on a signal or port THEN the system SHALL return every definition and commit whose place has that carrier as its root, in source order.
9. WHEN `instantiated_by()` is read on a module THEN the system SHALL return every instance whose `module()` is that module.

---

### P1: The design serializes ⭐ MVP

**User Story**: As a tool author, I want to save a `Design` and load it back so that a cache, another process or another language can read the model.

**Why P1**: the user asked for the derive now ("Derive já").

**Acceptance Criteria** (POM-15):

1. The system SHALL make `Design` and every public storage type of `pom` implement `serde::Serialize` and `serde::Deserialize`.
2. WHEN the design of the `SarAdc` gallery (Appendix F.1, built with the fixture prelude) is serialized with `serde_json` and deserialized THEN the system SHALL yield a design equal (`==`) to the original, with an identical `dump()`, identical `diagnostics()` and, for each file, an `ast()` equal to the original's.
3. WHEN the same design is serialized twice THEN the system SHALL produce byte-identical JSON.

**Independent Test**: `serde_json::to_string(&design)`, then `serde_json::from_str::<Design>(..)`, then compare.

---

### P1: Reference documentation ⭐ MVP

**User Story**: As a reader of the docs, I want an in-depth reference of the model, its methods and how each language construct maps to it so that I can use the POM without reading its source.

**Why P1**: named as a deliverable of the request.

**Acceptance Criteria** (POM-13):

1. WHEN a `pub struct`, `pub enum` or `pub trait` is declared under `crates/piperine-lang/src/pom/` THEN the system SHALL carry a heading `` ### `Name` `` for it in a file of `docs/pom/`.
2. WHEN a `pub fn` is declared in the reflection handles (`src/pom/reflect/`) THEN the system SHALL mention it as `` `name(` `` in `docs/pom/reflection.md`.
3. WHEN a production is defined in Appendix B.3.2 THEN the system SHALL list its name in the mapping table of `docs/pom/mapping.md`, in a row that names a POM type.
4. WHEN an ```` ```msl ```` fence of `docs/pom/*.md` is built THEN the system SHALL report no parse diagnostic for it.
5. WHEN the ```` ```rust ```` fences of `docs/pom/*.md` are run as doctests of `piperine-lang` THEN the system SHALL pass every one, and there SHALL be at least 10.
6. The system SHALL list every `docs/pom/*.md` file in the `nav` of `mkdocs.yml`.

**Independent Test**: open `docs/pom/index.md`, follow the quick tour and run its examples with `cargo test -p piperine-lang --doc`.

---

## Edge Cases

- IF a source is empty THEN the system SHALL return a `Design` with one root namespace that has no children (POM-01 AC 1 with `text == ""`).
- IF a behavior block names a module declared in another file of the tree THEN the system SHALL report `E0401` (POM-09 AC 7: "of its file").
- IF a `use` glob imports a name that a local item also declares THEN the system SHALL resolve to the local item (POM-08 AC 7, Rust's rule).
- WHEN a module has several `analog` blocks THEN the system SHALL list them all, in source order (POM-05 AC 1).
- IF a call's callee is an error node or a path that is `Unresolved` THEN the system SHALL classify the call as a plain call and report nothing further (POM-09 AC 4).
- WHEN a struct's field type refers to the struct itself THEN the system SHALL decide `is_bundle()` without looping (POM-09 AC 2).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| POM-01 | P1: Build a design from sources | Tasks | Verified |
| POM-02 | P1: The declaration tree mirrors the source | Tasks | Verified |
| POM-03 | P1: The declaration tree mirrors the source | Tasks | Verified |
| POM-04 | P1: The declaration tree mirrors the source | Tasks | Verified |
| POM-05 | P1: The declaration tree mirrors the source | Tasks | Verified |
| POM-06 | P1: The declaration tree mirrors the source | Tasks | Verified |
| POM-07 | P1: The declaration tree mirrors the source | Tasks | Verified |
| POM-08 | P1: Names are linked to their declarations | Tasks | Verified |
| POM-09 | P1: Names are linked to their declarations | Tasks | Verified |
| POM-10 | P2: Members resolve through declared types | Tasks | Verified |
| POM-11 | P1: Reflection queries; P2: Reverse queries | Tasks | Verified |
| POM-12 | P1: Authored structure is never rewritten | Tasks | Verified |
| POM-13 | P1: Reference documentation | Tasks | Verified |
| POM-14 | P1: Every source builds | Tasks | Verified |
| POM-15 | P1: The design serializes | Tasks | Verified |

**Coverage:** 15 total, 15 mapped to tasks, 0 unmapped.

---

## Sources

- The user's request (translated): "The model must be faithful to the language's own structures. E.g. if a wire is in module abc, the structure must reflect exactly that. It can even keep the AST, resolution fields, etc., but it has to represent ALL the code in a friendly way, as if I were doing Java reflection. ... At the end I want the code + an artifact explaining the models in depth like external docs: the models, the methods, etc., how each language aspect translates to the new POM model" - the goals, POM-02, POM-06, POM-11, POM-13.
- `docs/spec/appendix_b_grammar.md` B.3.2, B.3.3 - the productions to cover and the classifications left to name resolution (POM-09).
- `docs/spec/part_i_values.md` §3.4, `part_iii_structure.md` §8-§8.4, `part_iv_behavior.md` §10, §14 - scoping rules and the error codes of POM-08/09/10.
- The user's answers to Q1-Q4 (2026-11-13): "Paths + membros"; "Mecanismo de prelude + tabela de builtins"; "Regras do Rust no disco"; "Derive já" - POM-10, POM-08 AC 6 and 8, POM-01 AC 5-10, POM-15.
- `CLAUDE.md`, "UNBREAKABLE RULE - POM navigability mirrors the source" - POM-12.
- `crates/piperine-lang/src/msl/ast.rs` - the syntax tree the POM is built from.

---

## Success Criteria

- [ ] `design.module("abc").signal("w").declared_in()` is module `abc` for every signal of every gallery of Appendices D, E and F.
- [ ] Every `rust` fence of `docs/spec/` round-trips through the POM back to its syntax tree, and none panics the build.
- [ ] `docs/pom/` passes its coverage gate: every public POM type, every reflection method and every B.3.2 production is documented, and its examples run.
- [ ] `cargo build -p piperine-lang --all-targets` prints no warning.
