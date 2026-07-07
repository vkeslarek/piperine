## 4. Measurement is through the result object

The bench adds no measurement syntax and does **not** reuse `V(a,b)`/`I(a,b)` — those stay
analog-only. An analysis returns a result object; potentials and flows are read from it by
method:

```phdl
var r = $op();
r.v(a, b)     // potential across (a, b)
r.v(a)        // potential of a vs. ground  (default second argument, Part I §9.1)
r.i(a, b)     // branch flow
```

`r.v(a)` and `r.v(a, b)` are the same method with a defaulted second argument (Part I §9.1). Because a
result is a value, there is no active-result state: two analyses are two values
(`var dc = $op(); var tr = $tran(TranConfig { .stop = 1e-3 });`). Results are immutable snapshots
(§9). The measurement return type follows the analysis: `OpResult` yields `Real`, a `Trace`
yields `Waveform` (§6).

---

