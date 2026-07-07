## 13. Relationship to the companion specifications

- **Reflection API (POM)** — name resolution (§3), staging (§7), and the `Design`/`Module` handles
  (§8) are the object model; the result/waveform types are the simulation surface the reflection
  spec deferred here.
- **Selector** — `select(...)` for bulk staging and measurement.
- **Extensibility** — `extract`/`.attach`/`.meta` and plugin invocation, bench-only.
- **IR spec** — an analysis runs the codegen'd device over the solver; the bench never sees the IR.

---

