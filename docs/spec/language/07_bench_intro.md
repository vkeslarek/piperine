# Piperine Bench — Specification: The `bench` Block and the Uniform API

*Companion to the language specification (`crates/piperine-lang/docs/SPEC.md`). "Part I"
references below point there. Language-structural material that once lived here (default
parameter values, the config-bundle and `Waveform<T>` groundwork) is now Part I §9.1/§6; this
document keeps its original section numbering — code comments cite it as `§N`.*

`bench` is the effectful scripting layer of PHDL. It runs **after** elaboration and
monomorphization, over the concrete netlist, and is where a designer runs simulations, measures
results, and adjusts the design through reflection. Verification, parameter sweeps, and the
design-closure loop live here.

PHDL is one **strongly-typed** language with two faces: the *compiled* face (elaborated
`analog`/`digital` behavior) and the *interpreted* face (the `bench`, run interactively over an
elaborated design). The bench body is the **same `fn` grammar as a bundle `impl`** (Part I §9);
only the context differs — effectful, rooted at a module, with the simulation and reflection
tasks available (§11).

The same operations are exposed as a **uniform object-model API** (§8) callable identically from
a Piperine `bench`, from Piperine-as-a-library, from Python, and from Rust. `$op()` inside a
bench and `design.op()` from Python are the same operation with the same types.

```phdl
bench SwitchOpenTest {
    fn test_open_circuit() {
        var r = $op();
        $assert(r.v(vsrc, gnd) != 0, "voltage source should be active");
        $assert(r.i(resistor.p, resistor.n) == 0, "no current with the switch open");
    }
}
```

---

