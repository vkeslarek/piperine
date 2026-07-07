## 2. The `bench` block

```phdl
bench ModName {
    fn name() { … }             // entry point (no args) — a test or a flow
    fn helper(x: T) -> U { … }  // reusable
}
```

Attached to a module by name, as `analog ModName`/`digital ModName` are. A **testbench module**
is the common case: a ports-less top instantiating the DUT plus stimulus (§11). Bodies use the
`fn` grammar of Part I §9 (with default parameter values, Part I §9.1). A zero-argument `fn` is
a runnable **entry point** the toolchain discovers (`piperine test`/`run`); a test asserts, a
flow sweeps/tunes/reports — the split is behavioral, not syntactic. A `bench` fn is **not pure**.

---

