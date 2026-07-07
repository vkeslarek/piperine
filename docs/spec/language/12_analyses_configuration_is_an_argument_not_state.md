## 5. Analyses (configuration is an argument, not state)

Four analyses. Each takes a **config bundle** and returns a result object. In a bench the `$`
form is sugar for the implicit design's method (`$op(cfg)` ≡ the rooted `Module.op(cfg)`, §8):

| Analysis | Signature | Result |
|----------|-----------|--------|
| operating point | `op(cfg: OpConfig = OpConfig {}) -> OpResult` | scalar `.v`/`.i` |
| transient | `tran(cfg: TranConfig) -> Trace` | `Waveform<Real>` over time |
| AC small-signal | `ac(cfg: AcConfig) -> Trace` | `Waveform<Complex>` over frequency |
| noise | `noise(cfg: NoiseConfig) -> NoiseTrace` | PSD over frequency |

Config with all-default fields lets `$op()` run with no argument; a config with required fields
(`tran` needs `stop`) must be given. A **sweep** is not a task — it is a bounded `for` that stages
a value and re-runs (§7); corners and Monte-Carlo are library patterns over these four.

### 5.1 Config bundles

Ordinary value bundles (Part I §6.5) with defaults — extensive parameters modeled as data, not
as stateful setter calls. Per-node hints (`ic`, `nodeset`) are maps, not hidden state (the `Map<Net, Real>`
value type is implemented; `nodeset` seeds the DC Newton initial guess, `ic` seeds the transient's t=0
node voltages — `Map { out: 0.0 }` / `Map { out: 5.0 }`):

```phdl
bundle Solver {
    temperature : Real = 300.15,     // K
    reltol : Real = 1e-3,  abstol : Real = 1e-12,  gmin : Real = 1e-12,
    max_iter : Natural = 100,
}
bundle OpConfig    { solver : Solver = Solver {},  nodeset : Map<Net, Real> = {} }
bundle TranConfig  { stop : Real,  step : Real = 0.0 /*auto*/,  start : Real = 0.0,
                     ic : Map<Net, Real> = {},  solver : Solver = Solver {} }
bundle AcConfig    { fstart : Real,  fstop : Real,  points : Natural = 100,
                     scale : Scale = Dec,  solver : Solver = Solver {} }
bundle NoiseConfig { out : Branch,  fstart : Real,  fstop : Real,  points : Natural = 100,
                     scale : Scale = Dec,  solver : Solver = Solver {} }

enum Scale { Lin, Dec, Oct }
```

These are stdlib bundles; a project may define its own config bundles and pass them, since the
analyses are ordinary methods taking bundle arguments.

---

