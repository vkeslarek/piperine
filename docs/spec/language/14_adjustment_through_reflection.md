## 7. Adjustment through reflection

A bench tunes by staging overrides on the POM, then re-running:

```phdl
sw.ctrl = 1;                               // stage
select("//resistor").resistance = 2e6;     // stage across a set
var r = $op();                              // deterministic re-elaborate + solve
```

The design-closure loop is `measure → adjust → re-run`:

```phdl
fn tune_bias() {
    for _ in 0..20 {
        var r = $op();
        var err = r.v(out) - 1.0;
        if (abs(err) < 1e-3) { return; }
        bias.trim = bias.trim - 0.1 * err;
    }
    $error("bias did not converge");
}
```

Plugin-driven closure (extract parasitics → re-simulate) is the same loop with `extract(...)` and
`attach`/`meta` from the extensibility spec.

---

