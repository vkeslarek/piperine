## 9. Determinism and isolation

- Each entry-point fn runs against a fresh view; staged overrides do not leak between entry
  points.
- Within a fn, staged overrides accumulate until the next analysis, which re-elaborates from them
  and returns a new result.
- Result objects are **immutable snapshots**: a result computed before a staged change stays a
  valid value afterward, describing the earlier solve — nothing to invalidate.
- Analyses are pure functions of (elaborated design + staged overrides + config bundle);
  identical inputs give identical results and verdicts. Execution is blocking.

---

