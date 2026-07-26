# T24 — `SimSession` → `Session` equivalence matrix

**Subject**: `crates/piperine-api/src/session.rs` at `e3c233b` (1329 lines, two
public session types). All `file:line` references below are to that file at that
commit unless stated otherwise.

**Purpose** (design §C1a): before `SimSession` loses a method, every one of its
public entry points is mapped onto a `Session` counterpart with a verdict —
*identical* / *differs (how)* / *missing (port it)*. `SimSession` is deleted
(T30) only after every row reads *identical*. This file is the safety net for
T25–T30; the design appendix carries the same verdicts in short form.

---

## 1. The two lifecycles (the root of every *differs* row)

| | `SimSession` | `Session` |
|---|---|---|
| Construction | `new(design: Design, module: String)` — takes the design **by value, unforked**; `:113` | `compile(&Design, &str) -> Result<Self, Error>` — `design.fork()` then `with_overrides_applied(module)?.fork()`; `:644` |
| Elaboration timing | **per analysis** — every `run_*` calls `build_circuit` (`:157`) | **once**, at `compile`; every analysis reuses the held `circuit`/`info` |
| Staged overrides | `stage()` (`:148`) writes `design.set_param`; consumed by the next `build_circuit` | dropped: `Design::fork` installs a fresh empty `OverrideMap` (`crates/piperine-lang/src/pom/design.rs:668`), so overrides staged on the caller's design never reach `Session::compile` |
| Param writes after build | not possible (re-stage + re-elaborate) | `set` (`:678`, restamp, fail-loud on structural) / `Sweep`'s `set_or_rebuild` (`:1059`, rebuild + count) |
| Hooks | fires `transform_design`, `before_lower` (`:160`,`:165`) and `after_solve` (`:130`, called by every `run_*`) | **fires nothing** — no hooks field at all |
| Device provider | `set_device_provider` (`:118`) | none |
| `.disto` kernels | opt-**in**: `build_circuit(false)` everywhere except `run_disto` (`:343`), purely to skip the Cranelift cost | always on — `Session::compile` never calls `.with_disto` (`:648`) and `CircuitCompiler::new` defaults `compile_disto: true` (`crates/piperine-codegen/src/device/circuit.rs:109`), which is why `Session::disto` works today (`tests/session_analyses.rs:156`, `tests/host_parity.rs:61`) |
| Receiver | `&self` throughout | `&mut self` for every analysis |

The consequence that drives T25: `Session` today has **no** provider, **no**
hooks and **no** disto-kernel switch. Those are the three build-time options the
`SessionBuilder` exists to carry (design §C1), plus staging.

---

## 2. Method-by-method matrix

`Session` counterparts are at the line given; `—` means the counterpart did not
exist before T25.

| # | `SimSession` method (`file:line`) | `Session` counterpart (`file:line`) | Verdict | Resolution |
|---|---|---|---|---|
| 1 | `new` `:113` | `compile` `:644` / `builder` `—` | **differs** — no fork vs double fork; ownership vs borrow; `new` is infallible, `compile` returns `Result` | T25: `Session::builder(&design, module)`; construction keeps `Session`'s fork-and-isolate semantics (rule: `Session` behavior wins), staging is re-expressed as `SessionBuilder::stage` so no call site loses its override |
| 2 | `set_device_provider` `:118` | — | **missing (port it)** | T25: `SessionBuilder::provider(Rc<dyn DeviceProvider>)` — same `Rc<dyn DeviceProvider>` argument, still infallible |
| 3 | `set_hooks` `:126` | — | **missing (port it)** | T25: `SessionBuilder::hooks(Rc<dyn SimHooks>)`; `Session` stores the `Rc` so the *solve*-time hook can fire (row 4) |
| 4 | `fire_after_solve` `:130` (private) — fired by all 11 `run_*` | — | **missing (port it)** | T26: private `Session::fire_after_solve`, called by `op`/`tran`/`ac`/`noise`/`sens`/`pss`/`pz`/`sp`/`disto`/`dc` with the **same analysis-name strings** and the same payload rule (node voltages only for operating points, empty slice otherwise). No-op when no hooks are wired, so no existing `Session` caller changes behavior |
| 5 | `design` `:137` | — | **missing (port it)** | T25: `Session::design(&self) -> &Design`, same signature |
| 6 | `module` `:141` | `module` `:663` | **identical** | none |
| 7 | `stage` `:148` | — | **differs by construction** — staging *after* compile is meaningless on a compiled session | T25: `SessionBuilder::stage(label, param, piperine_lang::Value)` (same three arguments, same `Design::set_param` call, applied to the fork before overrides are consumed). Design §C1 explicitly keeps `stage` off `Session` |
| 8 | `build_circuit` `:157` (private) | `Session::compile`'s body `:644` | **differs** — hook order + provider + `compile_disto` present in one, absent in the other; fork present in the other, absent in one | T25: `SessionBuilder::compile` = `SimSession::build_circuit` body **with** the fork, preserving the hook order verbatim: `transform_design(&forked)` → `with_overrides_applied` → `before_lower(&applied)` → `lower_bodies` → `CircuitCompiler::new(..).with_disto(self.disto)` → `with_device_provider` → `build_circuit_mapped` → `init_digital` → `rebuild_digital_topology` |
| 9 | `run_op` `:361` | `op` `:706` | **differs** — `run_op` re-elaborates first and fires `after_solve("op", node_voltages)`; otherwise the two bodies are line-for-line the same (`build_ivs` → `dc` → `policy` → `apply_initial_conditions` → `solve` → the three snapshots → `OpResult::new`) | rows 4 + 8; after that, identical |
| 10 | `run_op_sweep` `:390` | `dc` `:1010` **and** `sweep` `:1050` | **differs (three ways)** — (a) return shape: `Vec<OpResult>` vs `Trace<Waveform>`; (b) per-point `info` clone: `run_op_sweep` clones the mirrored `info` **per point** (`:428`) so `OpResult::i` recomputes with that point's param, while `dc` hands one final `info` to the whole trace (`:1040`); (c) structural writes: `run_op_sweep` ignores the `Invalidation` returned by `set_element_param` (`:401`) and restamps regardless, while `Sweep::next` rebuilds and counts (`:1153`) | **`Session::sweep(..)` + `point.op(..)` is the exact capability equal** of `run_op_sweep`: one build (MD-18), one `OpResult` per point, `info` cloned per point by `Session::op` (`:725`). Every retargeted call site uses that. `Session::dc`'s trace shape is kept as-is (it is HOST-05 surface, not a `SimSession` port), and the (c) difference is resolved in `Session`'s favour — restamping a structural change onto stale kernel state is the bug, rebuilding is the fix |
| 11 | `run_tran` `:506` | `tran` `:735` | **differs** — argument shape `tspan: (stop, start)` vs `stop, step, start`; `run_tran` re-elaborates and fires `after_solve("tran", &[])`; `tran` additionally drains `pending_sets` (a `Session`-only feature, no `SimSession` equivalent to lose) | rows 4 + 8. Arguments are a call-site rewrite, not a behavior change: `run_tran((stop, start), step, ..)` → `tran(stop, step, start, ..)`, same values in the same order to the same solver options |
| 12 | `run_ac` `:540` | `ac` `:796` | **differs (widening only)** — `run_ac` takes `f64` + `bool`; `ac` takes `impl Into<Freq>` + `impl Into<bool>`, which accepts every `f64`/`bool` call site unchanged (`:779` records this as a deliberate HOST-21 deviation) | rows 4 + 8; the signature is a strict superset, so no call site changes |
| 13 | `run_noise` `:565` | `noise` `:817` | **differs** — re-elaboration + `after_solve("noise", &[])` only; bodies otherwise identical | rows 4 + 8 |
| 14 | `run_sens` `:185` | `sens` `:847` | **differs** — re-elaboration + `after_solve("sens", &[])`; plus `run_sens` resolves output nets by scanning `circuit.nets()` for the `AnalogVariable` (`:196`-`:210`) with its own "not addressable" message, while `sens` resolves through the shared `resolve_net` helper (`:856`) and then scans. Both produce `Error::Measurement("net `x` is not addressable")` for an unknown net and `"net `x` is not a solved analog net"` for a non-analog one — same two messages, same order | rows 4 + 8; error shapes already match |
| 15 | `run_pss` `:235` | `pss` `:887` | **differs** — re-elaboration + `after_solve("pss", &[])` only | rows 4 + 8 |
| 16 | `run_pz` `:262` | `pz` `:904` | **differs** — re-elaboration + `after_solve("pz", &[])` only | rows 4 + 8 |
| 17 | `run_sp` `:293` | `sp` `:952` | **differs** — re-elaboration + `after_solve("sp", &[])` only; both read `self.design.rfports(&self.module)` (`:302` / `:960`) | rows 4 + 8 |
| 18 | `run_disto` `:334` | `disto` `:926` | **differs (cost, not result)** — `run_disto` is the only `SimSession` path that passes `compile_disto = true`; `Session` gets the kernels unconditionally because `CircuitCompiler::new` defaults the flag to `true` | T25: `SessionBuilder::disto(bool)` gates `CircuitCompiler::with_disto`, defaulting to **`true`** — see §3.2 for why the design's proposed `false` default is not takeable. Plus rows 4 + 8 |
| 19 | `snapshot_digital` `:438` (assoc. fn) | — | **missing (port it)** | T25: same associated function on `Session`, same name, same `(&CircuitBuildInfo, &CircuitInstance) -> HashMap<String, f64>` signature, body moved verbatim. `crates/piperine-python/src/live.rs:417,853` calls it as a free-standing utility and keeps compiling |
| 20 | `snapshot_opvars` `:461` (assoc. fn) | — | **missing (port it)** | as row 19; `(&CircuitInstance) -> HashMap<String, Vec<(String, f64)>>`; `live.rs:418` |
| 21 | `snapshot_introspect` `:477` (assoc. fn) | — | **missing (port it)** | as row 19; `(&CircuitInstance) -> IntrospectSnapshot`; `live.rs:419` |

### `Session`-only surface (nothing to reconcile — no `SimSession` twin to lose)

`rebuilds` `:670`, `set` `:678`, `schedule_set` `:701`, `tf` `:985`,
`dc` `:1010`, `sweep` `:1050`, `sweep_grid` `:1197`, and the private
`set_or_rebuild` `:1059` / `rebuild` `:1079`. `SimSession` has no `.tf` at all,
so the collapse *adds* transfer-function reach to every former `SimSession`
call site rather than removing anything.

---

## 3. The dangerous rows (same name / same role, different observable behavior)

Five rows are behaviorally different in a way a compile error would **not**
catch. Each is resolved by porting `SimSession`'s behavior onto `Session`, never
by dropping it:

1. **Hook firing (row 4).** `Session` fires no `after_solve`. Retargeting the
   four `piperine-plugin` suites without porting this would make
   `solved.load(Ordering::SeqCst) == 1` assertions read `0` — a silently
   weakened test, not a compile error. Ported in T26 with the analysis-name
   strings unchanged (`"op"`, `"tran"`, `"ac"`, `"noise"`, `"sens"`, `"pss"`,
   `"pz"`, `"sp"`, `"disto"`).
2. **`compile_disto` gating (row 18) — and a design deviation.** Design §C1
   specifies `SessionBuilder::disto(bool)` "default `false`, `disto()` sets it".
   That default is **not takeable**: `CircuitCompiler::new` defaults
   `compile_disto: true` (`crates/piperine-codegen/src/device/circuit.rs:109`),
   so `Session::compile` has always produced disto-capable circuits, and
   `tests/session_analyses.rs:156` + `tests/host_parity.rs:61` both call
   `Session::disto` straight after a plain `Session::compile` and assert it
   solves. A `false` default would turn those into failures — a behavior change
   to `Session` inside a merge whose rule is "preserve `Session`'s behavior".
   **Resolution: the builder defaults `disto: true`; `disto(false)` opts out.**
   What `SimSession` used the flag for was skipping Cranelift cost on the ~10
   non-`disto` analyses, never a different answer, so nothing observable is
   lost by defaulting the other way; call sites that want the saving pass
   `disto(false)` explicitly.
3. **Per-point `info` mirror (row 10b).** `OpResult::i` on a force-less
   two-terminal device recomputes current from kernel + params, so a sweep that
   hands every point the *final* param set reports the wrong current for every
   earlier point. `tests/ngspice_validation.rs:246` reads exactly that
   (`op.i((branch_a, branch_b))` per sweep point). Preserved by retargeting to
   `Session::sweep` + `point.op()`, which clones `info` per point (`:725`), not
   to `Session::dc`.
4. **Staged overrides vs `fork` (row 1/7).** A mechanical rewrite of
   `SimSession::new(design, m)` + `stage(..)` into `Session::compile(&design, m)`
   would **silently discard every staged override** (`Design::fork` clears the
   override map). This is the single most dangerous rewrite in the phase; the
   builder's `stage()` is what makes it safe, and every retargeted call site
   that staged must go through it.
5. **Structural writes inside a sweep (row 10c).** Resolved in `Session`'s
   favour (rebuild + count, per the merge rule "preserve `Session`'s
   behavior"). This is the one row where `SimSession`'s behavior is deliberately
   *not* carried over, and the reason is recorded here rather than resolved
   silently: `run_op_sweep` discards the `Invalidation` verdict and restamps a
   structural change onto kernel state compiled for the old value. No existing
   call site sweeps a structural param (`tests/urc_compile_count.rs` sweeps
   `.r`, `tests/ngspice_validation.rs` and `tests/compile_once_sweep.rs` sweep
   `.dc` — all non-structural), so no test observes the difference.

---

## 4. Porting work this matrix hands to T25/T26

**T25 (`SessionBuilder` + ported capability)** — rows 1, 2, 3, 5, 7, 8, 18, 19,
20, 21:
- `Session::builder(&Design, &str) -> SessionBuilder`
- `SessionBuilder::{provider, hooks, disto, stage, compile}`
- `Session::design()`
- `Session::{snapshot_digital, snapshot_opvars, snapshot_introspect}` (bodies
  moved; `SimSession`'s become forwards until T30 deletes them)
- `Session::compile(&Design, &str)` stays as the no-options shorthand

**T26 (resolve the remaining *differs* rows)** — rows 4, 9, 11–17, plus the
duplicated param mirror named in design Risks row 3:
- private `Session::fire_after_solve`, wired into all ten analyses
- one private `Session::mirror_param(label, param, value)` replacing the four
  hand-inlined copies of the `info.instances` mirror (`:408`, `:690`, `:770`,
  `:1024`)

**Not porting (recorded, deliberate)**: row 10's `Vec<OpResult>` return shape
(`Session::sweep` + `op` covers it), row 10c's ignored `Invalidation`, and
`stage`-after-compile.

---

## 5. Verdict

Every `SimSession` capability has a `Session` home once T25 and T26 land. No row
requires a design change beyond the `SessionBuilder` the design already
specifies, so **Phase 5 may proceed to T25**.
