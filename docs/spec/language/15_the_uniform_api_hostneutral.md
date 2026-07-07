## 8. The uniform API (host-neutral)

The complete operation set, modeled once and exposed identically in every host. This is the QoL
payoff: Piperine-as-a-library, Python, and Rust drive the same interface with the same types.

```
// entry
load(path: String) -> Result<Design, LoadError>

// Design — reflection root (reflection spec §2) + analyses at design scope
Design
  top() -> Module
  module(name: String) -> Option<Module>
  modules() -> Selection<Module>
  select(path: String) -> Selection<Node>

// Module — reflection nav + staging (reflection spec) + the four analyses
Module
  // navigation / staging (reflection spec): ports() nets() instances() params()
  //   net(n) param(n) instance(n) ; param.set(v) ; select(path)
  op(cfg: OpConfig = OpConfig {}) -> OpResult
  tran(cfg: TranConfig) -> Trace
  ac(cfg: AcConfig) -> Trace
  noise(cfg: NoiseConfig) -> NoiseTrace
```

**In a bench**, the design is the implicit root and `$op(cfg)` ≡ `<this module>.op(cfg)`; names
resolve against the module (§3).

**As a library**, the same calls are explicit and chain identically across languages:

```
Piperine :  load("chip.ppr").module("Amp").op(OpConfig { .solver = Solver { .temperature = 350.0 } })
Python   :  load("chip.ppr").module("Amp").op(OpConfig(solver=Solver(temperature=350.0)))
Rust     :  load("chip.ppr")?.module("Amp")?.op(OpConfig { solver: Solver { temperature: 350.0, ..default() }, ..default() })?
```

Each returns the identical `OpResult` interface; `r.v(out)` reads the same value everywhere. The
result and waveform types, config bundles, and reflection surface are the one contract the ABI
(reflection spec §7) serializes; each host presents it idiomatically (Piperine/Python property
sugar, Rust explicit `..default()`), never a different shape.

---

