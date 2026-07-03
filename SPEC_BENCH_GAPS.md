# SPEC_BENCH_GAPS.md — Remaining gaps vs. `crates/piperine-lang/docs/SPEC_BENCH.md`

**Status: handoff draft (2026-07-04).** Everything the spec promises that the toolchain
does not do yet, after the conformance pass that closed config bundles, `$ac`, `$noise`,
`$write`, `select`-staging, `fft`/`rise_time`/`fall_time`, bare-name params, and `.i(a)`.
Each entry: what the gap is, where the spec demands it, what the code does today, and a
concrete implementation sketch. Ordered roughly easiest → hardest.

Conventions used below:
- "the interpreter" = `piperine_lang::eval::Interpreter` (+ `Host` trait, `eval/interp.rs`)
- "the bench runtime" = `crates/piperine-bench` (`SimHost` in `host.rs`, `SimTask`s in
  `tasks.rs`, result objects in `objects.rs`/`waveform.rs`, solve plumbing in `session.rs`)
- Every gap must stay **fail-loud** until closed: calling it is an elaboration error
  (`bench_task_implemented` allowlist in `piperine-lang/src/eval/tasks.rs`) or a typed
  runtime `EvalError` — never a silent no-op.

---

## G1 — `$plot(waveform, title)`

**Spec:** §8 table row `$plot/$write(path, …)` — "emit artifacts"; §11 lists `$plot(w, title)`
as bench-only output.

**Today:** elaboration-rejected (not in the `bench_task_implemented` allowlist). `$write`
(CSV) is implemented and is the reference `SimTask` to copy.

**Implementation sketch:**
1. Decide the artifact format first — the honest minimal options:
   (a) SVG line chart written next to the bench (self-contained, no deps);
   (b) CSV + a gnuplot/plotters sidecar script;
   (c) `plotters` crate PNG.
   Recommendation: (a) hand-rolled SVG — ~100 lines, zero new dependencies, viewable
   anywhere. Axis autoscale from `Waveform.points`, polyline, title text.
2. New `Plot` struct in `piperine-bench/src/tasks.rs` implementing `SimTask`; accepts
   `(Value::Object(Waveform | ComplexWaveform), Value::Str(title))`; downcast via
   `Object::as_any` exactly like `$noise` does for `NetRef`.
3. Output path: `<title>.svg` in the CWD (same convention `$write` uses for relative
   paths); sanitize the title into a filename.
4. Add `"plot"` to `bench_task_implemented`; update SPEC_BENCH §11 row; e2e test in
   `piperine-bench/tests/bench.rs` asserting the file exists and starts with `<svg`.

**Size:** small. **Risk:** none (leaf feature).

---

## G2 — `Waveform.map(f: fn(T) -> U) -> Waveform<U>`

**Spec:** §6 core `Waveform<T>` method list — "arbitrary transforms"; §6 closing prose makes
`map` the extension point for library-defined signal processing.

**Today:** `Undefined` method error. **This is the one architectural gap in the list**:
`Object::call_method(&self, name, args)` (`piperine-lang/src/value.rs`) has no way to
*invoke* a `Value::Closure` argument — closures are executed only by the `Interpreter`,
and host objects don't hold an interpreter.

**Implementation sketch (choose one):**
- **(a) Recommended — interpreter-side special case for callback-taking methods.** In
  `Interpreter::eval_call`'s method-dispatch arm (`eval/interp.rs`, the
  `Expr::Field(recv, m)` case): before delegating to `call_builtin_method`, check whether
  any argument is a `Value::Closure` **and** the receiver is an `Object`. If so, call a
  new optional `Object` hook:
  ```rust
  fn call_method_with(&self, name: &str, args: Vec<Value>,
                      invoke: &mut dyn FnMut(&Closure, Vec<Value>) -> Result<Value, EvalError>,
  ) -> Result<Value, EvalError> { ... default: Err(Undefined) }
  ```
  The interpreter passes `invoke = |c, args| self.call_closure(c, args)`. `Waveform`
  implements it for `"map"`: iterate points, invoke per sample, collect into `Waveform`
  (all-Real results) or `ComplexWaveform` (all-Complex) — anything else is a
  `TypeMismatch`. No global redesign; the hook composes with the existing `call_method`.
- (b) Alternative: make `map` an interpreter *built-in* on `Value::Object` recognized by
  name (like `push` on lists) that extracts `points()`, maps, and rebuilds. Less general
  (every future callback method needs its own arm) — only take this if (a)'s borrow
  gymnastics around `self.call_closure` inside `eval_call` get ugly (they shouldn't:
  collect args first, then dispatch).

**Also unlocks:** future `filter`/`fold` on waveforms and selections without new machinery.

**Size:** medium. **Risk:** medium — touches the interpreter's dispatch hot path; keep the
non-closure fast path unchanged and add tests for closure-arg methods on non-Objects
(should still error cleanly).

---

## G3 — `Trace.i(a, b)` beyond ideal-source branches

**Spec:** §6 `Trace.i(a, b) -> Waveform<T>`; §14 (resolved node-reference question) says a
device current with no MNA branch unknown "is recomputed from the solved terminal voltages
via the device's own residual".

**Today (`waveform.rs::Trace::i`):** only devices with force rows (`<-` ideal sources,
`BranchIdentifier::new(label, "force0")`) — a resistor's current over time is an error.
`OpResult::i` already does the residual recomputation, but **DC-only**: it calls
`kernel.eval_residual(volts, params, &[], &[], …)` which excludes the reactive (charge)
part — correct at DC, wrong mid-transient for any device with `ddt`.

**Implementation sketch:**
1. Resistive part per timestep: reuse `OpResult::i`'s logic — for each `TransientStep`,
   gather terminal voltages (`step.get_node`), call `eval_residual`, take the signed
   terminal-0 entry. Factor the shared helper out of `objects.rs` into a free fn or onto
   `BuiltInstanceInfo` (both files already hold `kernel/params/terminals`).
2. Reactive part: `i_C = dQ/dt`. Kernel exposes `eval_charge(volts, …) -> Q per terminal`
   (`piperine-codegen/src/jit/analog.rs`). Numerically differentiate the charge series
   between consecutive accepted steps: `i ≈ (Q_k − Q_{k−1}) / (t_k − t_{k−1})` — this is
   exactly the backward-Euler companion the solver itself stamps, so it is consistent with
   the solve. First sample: reuse the second's value or 0 with a doc note.
3. Sum resistive + reactive per step → `Waveform`. Devices with `num_forces > 0` keep the
   branch-unknown path (it is exact).
4. `vars`/`state` banks: the per-step values of module vars (D2A) and operator states are
   **not recorded** in `TransientAnalysisResult` — for devices whose residual reads them
   (digital-reading analog blocks, `delay`/`idt` operators) the recomputation is wrong.
   Milestone split: implement for var-free/state-free kernels (the common R/C/nonlinear
   case, detectable via `AnalogKernel::read_bounds()`), fail loud otherwise
   ("`i()` over time on a device with runtime state is not yet recorded").

**Size:** medium. **Risk:** medium — sign conventions and the reactive term need the
existing `OpResult::i` sign test extended to a transient RC test with a known analytic
current.

---

## G4 — `TranConfig.start != 0` (delayed-start transient)

**Spec:** §5.1 `TranConfig { …, start : Real = 0.0, … }`.

**Today:** fail-loud `TaskUnavailable` in the `Tran` task (`tasks.rs`) when `.start != 0`.
The solver's `TransientAnalysisOptions` (`piperine-solver/src/analysis/transient.rs`) has
`stop_time`/`dt`/adaptive fields but no start-time / output-suppression concept.

**Implementation sketch:** ngspice semantics — simulate from 0, *record* from `start`:
1. Add `record_from: Second` (default 0) to `TransientAnalysisOptions`.
2. In `TransientSolver::solve`'s accept loop (`solver/transient.rs`), skip pushing
   `TransientStep`s while `t < record_from` (still solve them — the state evolution
   matters).
3. Thread `.start` through `SimSession::run_tran` → options; delete the fail-loud arm.
4. Test: settled-RC trace with `.start = 0.5e-3` has `axis().at(0)` ≥ 0.5e-3.

**Size:** small. **Risk:** low.

---

## G5 — `Map<K, V>` value type → `ic:`/`nodeset:` config fields

**Spec:** §5.1 — `OpConfig.nodeset : Map<Net, Real> = {}` and `TranConfig.ic : Map<Net,
Real> = {}`; "per-node hints are maps, not hidden state". Also SPEC.md §6.1 lists `Map` as
reserved.

**Today:** no `Map` variant in `Value` (`piperine-lang/src/value.rs`), no literal syntax,
fields omitted from the prelude bundles (see the prelude's GAPS comment).

**Implementation sketch (three layers):**
1. **Value:** `Value::Map(Rc<RefCell<Vec<(Value, Value)>>>)` — assoc-vec, not HashMap
   (`Value` keys aren't `Hash`/`Eq`-clean; N is tiny). Builtin methods mirroring `List`:
   `insert(k, v)`, `get(k) -> Option<V>`, `len()`; `PartialEq` structural.
2. **Syntax:** the spec writes `{}` for the empty map default. `{ }` currently parses as a
   block. Decision needed (flag for the elaborating AI): either (a) a `Map {}`-style
   literal (`Map { a: 1.0 }` — unambiguous, bundle-lit-like), or (b) context-typed `{k: v}`
   in bundle-field position only. (a) is simpler and honest.
3. **Consumption:** `ic`/`nodeset` reach the solver as initial values — the plumbing
   exists internally: `InitialValue<AnalogReference, f64>` +
   `solver.push_initial_conditions(…)` (see `math/iv.rs`, and how the transient solver
   seeds its DC point in `solver/transient.rs::compute_initial_conditions`). In
   `SimSession::run_op/run_tran`: resolve each map key (a `NetRef`) through
   `CircuitBuildInfo.nets`, build `InitialValue`s, push before `solve()`. `nodeset` =
   initial guess for DC Newton; `ic` = enforced initial state for transient — check
   whether the DC solver currently accepts seeded IVs (it may need the same
   `push_initial_conditions` hook the transient solver has).
4. Re-add the two fields to the prelude bundles with `= Map {}` defaults.

**Size:** medium-large (language + runtime + solver surfacing). **Risk:** medium; the
solver-side IC hook is the unknown — verify `DcSolver` honors pushed IVs before wiring.

---

## G6 — `Branch` value type → `NoiseConfig.out : Branch`

**Spec:** §5.1 `NoiseConfig { out : Branch, … }`.

**Today:** `$noise(out_net, NoiseConfig { … })` — the output is a **positional net**
argument; the config field doesn't exist. Documented divergence in §11.

**Implementation sketch:**
1. A `Branch` in bench terms is "a `(plus, minus)` net pair, or an instance branch". The
   cheap concrete step: accept a `Net` (today's behavior) *or* a 2-tuple of nets in an
   `out` config field, since `Value::Tuple` already exists. `NoiseAnalysisOptions` already
   takes `output_node` + `reference_node` — a pair maps 1:1.
2. Add `out` to the prelude `NoiseConfig` as an untyped-by-convention field (no default —
   required), have the `Noise` task read `field(cfg, "out")` (Net → (out, gnd);
   Tuple(Net, Net) → (out, ref)), and drop the positional argument (keep it one release
   as a deprecated alias).
3. A real `Branch` *type* (instance-branch handles, `resistor.p→n`) only matters when
   noise supports input-referred analysis (`input_source_name`) — defer that half until
   then; note it in SPEC_BENCH §14.

**Size:** small. **Risk:** low. Blocked-by: none (the bundle field can't be *typed*
`Branch` until the type exists, but bundles don't enforce field types at bench runtime).

---

## G7 — Generic bench targets (`bench Dac` attaching to `Dac__8`)

**Spec:** §3 — "Post monomorphization, generics appear in concrete form."

**Today:** `AttachBenches` pass (`piperine-lang/src/elab/lower/passes.rs`) errors when the
bench names a module that is generic or absent. A bench can only target a concrete,
non-generic module.

**Implementation sketch:** copy `AttachBehaviors`' suffix logic (same file, directly
above): after exact-name attach, also attach the bench to every module named
`{base}__{digits/underscores}`; a bench targeting a generic base with **zero**
monomorphized instances stays an error (nothing to run). `BenchRunner` needs no change —
it iterates `Design::benches()`. Decide reporting shape: entry points run once per
monomorph (`Dac__8::test_x`, `Dac__12::test_x`) — that naming falls out of
`BenchResult.module` for free.

**Size:** small. **Risk:** low — mirror of existing, tested logic.

---

## G8 — Default parameter values on user-defined fns (`fn foo(x: Real = 1.0)`)

**Spec:** §10 — "the one genuine language change"; §2 says bench bodies use "the `fn`
grammar of Part I §9 *with the default-parameter extension of §10*". Trailing params only;
defaults are elaboration constants. Also §14 open question (named args at call sites —
out of scope here).

**Today:** `FnParam::Typed(String, Type)` (`parse/ast.rs`) has no default; `FnSig::parse`
(`parse/parser/fn_decl.rs`) parses `name : Type` strictly. Built-in methods fake defaults
by matching `args.len()`.

**Implementation sketch:**
1. AST: `FnParam::Typed { name: String, ty: Type, default: Option<Expr> }` (struct-variant
   upgrade; ~6 pattern sites, compiler-enumerated).
2. Parser: after the param's `Type`, `if self.eat(&Tok::Assign) { default = parse_expr }`.
   Validate trailing-only at parse time (a defaulted param followed by a non-defaulted one
   is a parse error — cheapest place to enforce §10).
3. Interpreter (`Interpreter::call_fn_decl` / `call_with_params` in `eval/interp.rs`):
   if `args.len() < params.len()`, evaluate the missing trailing defaults (in the callee's
   scope, left to right, so later defaults may reference earlier params — document
   whether that's allowed; simplest: evaluate against the already-bound params). Arity
   error only when `args.len()` < number of *non-defaulted* params.
4. Elaborated path (impl/global fns lowered to IR): `convert_fn` +
   `Inliner::expand` (`piperine-codegen`… now `jit/flatten.rs`) check
   `function.params.len() != args.len()` — extend to fill defaults at *call-site
   lowering* (defaults are elaboration constants per spec, so const-eval them in
   `lowering/expr.rs::lower_call` before building `IrExpr::Call`).
5. Spec: flip the §11 deferred note; add a worked example.

**Size:** medium. **Risk:** medium — two consumers (interpreter + IR inliner) must agree;
test both (a bench helper with default, and an analog fn with default used in a
contribution).

---

## G9 — `select(...)` for **measurement** (expression position)

**Spec:** §7 "bulk staging and bulk measurement"; §13 Selector bullet.

**Today:** `select("path").param = value` (assignment position) stages — implemented in
`SimHost::assign`/`stage_selection` (`host.rs`). But `select(...)` in *expression*
position (`var rs = select("//resistor");` or reading `select("...").resistance`) is an
`Undefined` call — `SimHost::resolve_callable` only serves POM fns.

**Implementation sketch:**
1. New host object `SelectionRef { labels: Vec<String> }` in `objects.rs` (labels, not
   borrowed POM nodes — results objects must be `'static`).
2. `SimHost::resolve_callable`… wrong hook (returns `Callable`); instead intercept in
   `SimHost::syscall`? No — `select` is a plain call, not a `$`-syscall. The interpreter's
   `eval_call` falls back to `host.resolve_callable(name)` then math. Cleanest: extend the
   `Host` trait with a default-`None` `fn call_host_fn(&mut self, name, args) -> Option<Result<Value>>`
   consulted between `resolve_callable` and the math fallback; `SimHost` implements it for
   `"select"` (evaluate path against the forked design exactly as `stage_selection` does,
   return `SelectionRef`).
3. Methods on `SelectionRef`: `len()`, `labels()` (list of strings); field-read
   (`.resistance`) = read that param from each instance → `List` of values, or scalar when
   the selection is a singleton (decide and document — recommend always-List, no magic).
4. Staging via a held selection (`var s = select(...); s.ctrl = 1;`) — `SimHost::assign`
   gains an arm for `Field(<expr evaluating to SelectionRef>, param)`; note the target is
   an `Expr`, so this needs evaluating the base — mirror how `stage_selection` re-runs the
   selector, or store labels in the object and re-stage from them.

**Size:** medium. **Risk:** low-medium — the `Host` trait extension is additive
(default method), no interpreter surgery.

---

## G10 — Device noise PSDs are zero (`$noise` returns a structurally-correct zero)

**Spec:** §5/§6 expect physically meaningful `NoiseTrace` values, not just the surface.

**Today:** `$noise` runs the solver's noise analysis end-to-end, but
`PhdlDevice::noise_current_psd` (JIT device path) returns empty — a long-standing known
gap (CLAUDE.md "Known gaps"): `IrNoiseSource`s are collected by `scan_noise` into the IR
and compiled (`AnalogKernel::eval_noise` exists) but never stamped into the AC noise
analysis. OSDI devices (`solver/src/osdi/device.rs`) are the working reference.

**Implementation sketch:**
1. `piperine-codegen/src/device/mod.rs` (`PiperineDevice`) / `device/analog.rs`
   (`AnalogInstance::noise_current_psd`): evaluate `kernel.eval_noise(volts_at_dc, …)` per
   source, return `Vec<Noise>` with the source's `(plus, minus)` refs — the skeleton
   already exists (`noise_refs` is built in `AnalogInstance::new`); check what the current
   body actually returns and diff against `OsdiDevice`'s implementation.
2. Flicker noise needs the analysis frequency — check the `Noise` struct / solver call
   signature for where `f` enters (OSDI path shows it).
3. Test: resistor with `white_noise(4*NG_K*T/r)` in the header/example → `$noise` total
   matches the Johnson formula within tolerance.

**Size:** medium (mostly reading the OSDI reference). **Risk:** medium — physics
correctness needs the analytic test, not just plumbing.

---

## G11 — AC stimulus verification (`ac_stim`)

**Spec:** §5 `$ac` implies a stimulus; SPEC Part VI defines `ac_stim` as the analog
operator that injects it.

**Today:** `$ac` runs and returns the sweep, but whether an `ac_stim(...)` contribution in
an analog body actually drives a nonzero small-signal response through the JIT path is
**unverified** — `IrExpr::AcStim` exists ("zero outside AC analysis") and the e2e test
only asserts sweep mechanics (axis/lengths), not magnitudes. CLAUDE.md lists AC stimulus
among the recognized-but-unproven operators.

**Implementation sketch (verification-first):**
1. Write the truth test: RC low-pass with `V(in) <+ ac_stim(1.0)` (or via a source
   header), `$ac`, assert `db().at(f_3db) ≈ -3` and flat passband.
2. If it fails: follow `AcStim` through `jit/flatten.rs` → `emit.rs` → how `load_ac`
   composes the RHS; the likely gap is the stimulus vector (RHS `b`) never getting the
   `ac_stim` magnitude — compare with how force devices stamp `load_ac`.

**Size:** unknown until the test runs (could be zero, could be a codegen feature).
**Risk:** contained — test-first.

---

## G12 — The uniform API (§8): `load()`, `Design::op/tran/ac/noise`, Python/Rust hosts

**Spec:** §8 in full; §13 first bullet.

**Today:** absent by design (milestone 3). The internal pieces map cleanly:
`SimSession::run_*` already are "`Module.op(cfg)`" minus the naming; `BenchRunner` is the
bench-side driver; nothing exposes a public embedding API.

**Implementation sketch (coarse — this is a milestone, not a task):**
1. **Rust first.** Public crate surface in `piperine-bench` (or a new `piperine` facade
   crate): `fn load(path) -> Result<Design, LoadError>` (parse+elaborate with a project
   SourceMap — reuse `piperine-cli/commands/utils.rs::build_source_map`), plus
   `trait ModuleExt { fn op(&self, cfg) … }` or a `Handle<'d>` wrapper struct holding
   `(Design, module_name)` — i.e. `SimSession` renamed and made public with typed config
   structs instead of `Value` records.
2. Config types: the Rust-side `OpConfig`/`TranConfig`… structs with `Default` — mirror
   the prelude bundles; the bench task layer converts `Value::Record` → these structs
   (single conversion point, `tasks.rs` already half-does it via `solver_config`).
3. Result types are already host-neutral (`OpResult`/`Trace`/`Waveform` hold no
   interpreter state) — re-export.
4. Python: `pyo3` behind a feature flag, one `#[pyclass]` per handle/result type,
   after the Rust surface settles. Do NOT start here.
5. The spec's identical-shape rule (§8 "never a different shape") is the review gate for
   every signature.

**Size:** large. **Risk:** low technically, high API-design (get the Rust surface reviewed
before Python).

---

## G13 — `extract` / `.attach` / `.meta` (extensibility spec)

**Spec:** §7 last paragraph, §11 table row, §13 third bullet — all deferred to "the
extensibility spec", **which does not exist yet**.

**Today:** elaboration-rejected names. No extensibility spec document in the repo.

**Implementation sketch:** blocked on writing that spec first (plugin model: what is a
plugin, what does `extract` return, where do `.attach`ed annotations live on the POM —
likely `Attribute`s, which already exist on every POM node). Do not implement ahead of
the spec; the only prep worth doing now is keeping `Attribute` surfaces public on POM
nodes (they are).

**Size/Risk:** n/a — spec work first.

---

## Cross-cutting notes for the implementing AI

- **Allowlist discipline:** every new task name goes into `bench_task_implemented`
  (`piperine-lang/src/eval/tasks.rs`) *and* gets a `SimTask` impl — the elaboration-time
  gate and the runtime dispatch must move together, with the SPEC_BENCH §11 row flipped in
  the same change.
- **Test placement:** e2e bench behavior → `crates/piperine-bench/tests/bench.rs`
  (has the `elab` helper + `CIRCUIT` fixture with `SwitchOpenTest` and `RcCharge`);
  syntax/elaboration gates → `crates/piperine-lang/tests/bench.rs`; example-gallery runs →
  `crates/piperine-bench/tests/run_examples.rs` (all 21 `examples/*.phdl` benches must
  stay green).
- **Suite must stay at 45 green targets, zero warnings**, per SIMPLIFICATION.md policy.
- Update **both** SPEC_BENCH.md §11 (status per row) and SIMPLIFICATION.md Appendix A
  when closing an item — they are the two conformance ledgers.
