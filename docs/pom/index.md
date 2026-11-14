# The Piperine object model (POM)

The POM is the one in-memory model of a set of MSL sources. One call turns source text into an
immutable `Design`; every later stage (a type checker, an elaborator, a simulator front end, an
editor) starts from it. It is a Rust library, `piperine_lang::pom`, and it reads the way
Java reflection does: you ask a module for its ports, a signal for the module it is declared
in, an instance for the module it places.

```text
 sources ──► parse ──► lower ──► resolve ──► index ──► Design
 (text)      syntax    arenas    names,       reverse   (immutable,
             tree      of nodes  classes,     tables    Send + Sync,
                                 members                serializable)
```

## The one rule

> **The POM's navigability mirrors the source.** What you read back from a `Design` is the
> structure the author wrote: the modules, instances and hierarchy as written, a generation
> `for` held as one `for` (never unrolled), a `let` held as a `let`. Name resolution only *adds*
> links to the nodes lowering made; it never creates, removes or moves a node. Transforms that
> need another shape (flattening, unrolling) belong to side artifacts built *from* a design,
> never to the design itself.

The rule is mechanical, not aspirational: building the same file with and without the sources
its names refer to gives the same nodes in the same order (only the links differ), and
`Design::check()` lists any broken invariant of the arenas.

## Building a design

```text
Design::builder()
    .std_source("lib.msl", std_text)    // optional; replaces the embedded std; the `pub` items of its `prelude` are visible everywhere
    .source("top.msl", top_text)       // the first source is the root; others are loaded by `mod`
    .build()                           // never fails
```

* `build()` always returns a `Design`, however broken the input. A parse error keeps the
  recovered tree (holes are `Error` nodes with their spans) and is reported in `diagnostics()`.
* `Design::load(path)` reads a tree from disk: `mod b;` loads `b.msl` or `b/mod.msl` beside the
  declaring file, following Rust's layout. An unreadable root gives an empty root and one
  `E0161`.
* Every file keeps its text and the syntax tree the parser returned (`SourceFile::ast()`), and
  `SourceFile::to_ast()` rebuilds that tree from the POM alone, spans included: the POM is
  lossless.

## A quick tour

This is the `Front` model of the specification, §0.7: a comparator with hysteresis that drives a
flip-flop.

```msl
/// A comparator with hysteresis that drives a flip-flop.
module Front(vin: Electrical, in clk: Bit, out q: Bit) {
    vth: f64 = 0.6;
    hyst: f64 = 50m;
    signal above_th: Bit = false;        // state: a signal
    signal d: Bit;
    d = above_th;                        // structure: a combinational definition
}

digital for Front {
    on above(v(vin) - vth - hyst / 2.0) { above_th <- true; }    // an analog event
    on above(vth - hyst / 2.0 - v(vin)) { above_th <- false; }
    on clk.rise() { q <- d; }                                    // a digital event
}
```

The standard names the model uses (`Electrical`, `Bit`, `above`) come from `std::prelude`. The
tour supplies a small one, builds the design and asks it questions:

```rust
use piperine_lang::pom::{Design, Reflect};

let std = r#"
pub nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
pub nature Bit { value: bool }
impl Bit { pub fn rise(self) -> Event { 0 } }
pub module above(level: f64) -> Event {}
"#;
let src = r#"
/// A comparator with hysteresis that drives a flip-flop.
module Front(vin: Electrical, in clk: Bit, out q: Bit) {
    vth: f64 = 0.6;
    hyst: f64 = 50m;
    signal above_th: Bit = false;
    signal d: Bit;
    d = above_th;
}

digital for Front {
    on above(v(vin) - vth - hyst / 2.0) { above_th <- true; }
    on above(vth - hyst / 2.0 - v(vin)) { above_th <- false; }
    on clk.rise() { q <- d; }
}
"#;
let design = Design::builder().std_source("lib.msl", format!("pub mod prelude {{ {std} }}")).source("front.msl", src).build();
assert!(design.diagnostics().is_empty());

// The structure is what was written, by name.
let front = design.module("Front").unwrap();
let ports: Vec<_> = front.ports().iter().map(|p| p.name().unwrap()).collect();
assert_eq!(ports, ["vin", "clk", "q"]);
assert_eq!(front.port("vin").unwrap().nature(), design.nature("Electrical"));
assert_eq!(front.params().len(), 2);
assert_eq!(front.signals().len(), 2);

// A signal knows where it is declared.
let d = front.signal("d").unwrap();
assert_eq!(d.declared_in().unwrap().name(), Some("Front"));
assert_eq!(d.qualified_name(), "Front.d");

// The `above(..)` calls are instances of a module, and the behavior block belongs to `Front`.
assert_eq!(front.instances().len(), 2);
assert!(front.instances().iter().all(|i| i.module().unwrap().name() == Some("above")));
let block = &front.digital_blocks()[0];
assert_eq!(block.module(), Some(front));
let events: Vec<_> = block.reactions().iter().map(|r| r.event().text()).collect();
assert_eq!(events[2], "clk.rise()");

// Reverse queries: who writes `d`, and who writes `q`.
assert_eq!(d.writers().len(), 1);               // the definition `d = above_th;`
assert_eq!(front.port("q").unwrap().writers().len(), 1); // the commit `q <- d;`
```

## Where to go next

| Page | Read it for |
| --- | --- |
| [Model](model.md) | the storage: every id, node, payload and slot, with its fields |
| [Reflection](reflection.md) | the handles and every method, with the result and an example each |
| [Mapping](mapping.md) | language construct → POM: which node, which handle, which query |
| [Resolution](resolution.md) | the scope order, the built-in table, classifications, `Deferred`, every diagnostic code |

## What the POM does not do

* **No type checking.** A member whose receiver type cannot be read from a declaration is marked
  `Deferred` and no diagnostic is raised. Expressions have no types in the POM.
* **No evaluation.** Constants, `where` conditions and generation bounds are kept as expressions.
* **No elaboration or flattening.** Generics are not instantiated and generation is not
  unrolled: a `for k in 0..1000 { .. }` body is one declaration, and the instance inside it is one
  instance.
* **No simulation semantics.** The POM says what was written and what each name refers to, not
  what it does at run time.
* **No mutation.** A `Design` never changes after `build()`. Edit the source and build again.
