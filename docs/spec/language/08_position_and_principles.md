## 1. Position and principles

| Layer | Runs | Pure? |
|-------|------|-------|
| elaboration | once | pure |
| `analog`/`digital` | inside the solver | pure per step |
| **`bench`** | after elaboration | **effectful** |

Effects are **gated by context**: only a `bench` fn holds the toolchain handle and may run
analyses, measure, stage overrides, or do I/O. A `bench` may loop unbounded and is otherwise a
driver like `main`.

Two invariants carry the language's principles into the bench, and they are the reason there are
no special cases:

- **No hidden state.** An analysis takes all of its configuration as an explicit argument (§5),
  never from prior stateful calls. There is no active result, no ambient options, no implicit
  temperature. (This is why `$option`/`$ic`/`$nodeset`-style config tasks do **not** exist — they
  would be hidden state; configuration is a value passed in.)
- **No in-place mutation.** Design changes stage overrides consumed by the next analysis (§7).
  Every analysis is a pure, deterministic elaborate-and-solve of (design + staged overrides +
  config). Reproducibility is a property of the design; the bench sequences reproducible runs.

Everything is **blocking / synchronous** for now; concurrency of analyses is deferred and the
immutable-result model (§9) keeps it safe to add later.

---

